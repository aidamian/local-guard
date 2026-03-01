#![warn(missing_docs)]
//! # local-guard-mosaic
//!
//! ## Purpose
//! Composes deterministic temporal 3x3 mosaics from validated frame batches.
//!
//! ## Responsibilities
//! - Validate the frame count and geometry for one mosaic batch.
//! - Map chronological frames into row-major tile coordinates.
//! - Return upload-ready mosaic image bytes.
//!
//! ## Data flow
//! Completed frame batch -> [`compose_temporal_mosaic`] -> [`MosaicImage`]
//! consumed by payload assembly.
//!
//! ## Ownership and lifetimes
//! Mosaic output owns its byte buffer, enabling downstream upload retries
//! without borrowing the source frame collection.
//!
//! ## Error model
//! Non-9-frame inputs or geometry mismatches fail with [`MosaicError`].
//!
//! ## Security and privacy notes
//! Mosaic composition mutates no content; it only rearranges existing frame
//! pixels according to deterministic temporal ordering.

use local_guard_core::Frame;
use thiserror::Error;

/// Required frame count for one 3x3 temporal mosaic.
pub const MOSAIC_FRAME_COUNT: usize = 9;

/// Mosaic image produced from one chronological frame batch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MosaicImage {
    /// Mosaic width in pixels (`tile_width * 3`).
    pub width: u32,
    /// Mosaic height in pixels (`tile_height * 3`).
    pub height: u32,
    /// RGBA bytes in row-major order.
    pub rgba: Vec<u8>,
}

/// Composes a deterministic 3x3 temporal mosaic.
///
/// # Parameters
/// - `frames`: Chronological frame batch in ascending capture-time order.
///
/// # Errors
/// Returns [`MosaicError::InvalidFrameCount`] when frame count is not exactly 9.
/// Returns [`MosaicError::GeometryMismatch`] when any frame geometry differs.
pub fn compose_temporal_mosaic(frames: &[Frame]) -> Result<MosaicImage, MosaicError> {
    if frames.len() != MOSAIC_FRAME_COUNT {
        return Err(MosaicError::InvalidFrameCount {
            expected: MOSAIC_FRAME_COUNT,
            actual: frames.len(),
        });
    }

    let tile_width = frames[0].width;
    let tile_height = frames[0].height;

    for frame in frames {
        if frame.width != tile_width || frame.height != tile_height {
            return Err(MosaicError::GeometryMismatch);
        }
    }

    let mosaic_width = tile_width * 3;
    let mosaic_height = tile_height * 3;
    let mosaic_len = (mosaic_width as usize)
        .checked_mul(mosaic_height as usize)
        .and_then(|pixels| pixels.checked_mul(4))
        .ok_or(MosaicError::Overflow)?;

    let mut mosaic_rgba = vec![0_u8; mosaic_len];

    for (frame_index, frame) in frames.iter().enumerate() {
        // Why:
        // - Temporal ordering requirement is left-to-right then top-to-bottom.
        // Invariant:
        // - `frame_index` maps directly to tile row/column for deterministic
        //   chronological layout.
        let tile_row = frame_index / 3;
        let tile_col = frame_index % 3;

        for y in 0..tile_height as usize {
            let src_offset = y * tile_width as usize * 4;
            let dst_y = tile_row * tile_height as usize + y;
            let dst_x = tile_col * tile_width as usize;
            let dst_offset = (dst_y * mosaic_width as usize + dst_x) * 4;
            let row_len = tile_width as usize * 4;

            mosaic_rgba[dst_offset..dst_offset + row_len]
                .copy_from_slice(&frame.rgba[src_offset..src_offset + row_len]);
        }
    }

    Ok(MosaicImage {
        width: mosaic_width,
        height: mosaic_height,
        rgba: mosaic_rgba,
    })
}

/// Error type for mosaic assembly.
#[derive(Debug, Error)]
pub enum MosaicError {
    /// Input does not contain required frame count.
    #[error("invalid frame count: expected {expected}, got {actual}")]
    InvalidFrameCount {
        /// Required frame count.
        expected: usize,
        /// Actual frame count.
        actual: usize,
    },
    /// Frames are not homogeneous in geometry.
    #[error("all frames in a batch must share the same geometry")]
    GeometryMismatch,
    /// Integer overflow occurred while computing output geometry.
    #[error("mosaic dimension overflow")]
    Overflow,
}

#[cfg(test)]
mod tests {
    //! Unit tests for mosaic composition.

    use local_guard_core::Frame;

    use super::*;

    #[test]
    fn compose_places_first_and_last_tiles_correctly() {
        let mut frames = Vec::new();
        for index in 0..MOSAIC_FRAME_COUNT {
            frames.push(
                Frame::new(
                    "display-1",
                    1,
                    1,
                    index as u64,
                    vec![index as u8, 0, 0, 255],
                )
                .expect("frame should be valid"),
            );
        }

        let mosaic = compose_temporal_mosaic(&frames).expect("mosaic should compose");
        assert_eq!(mosaic.width, 3);
        assert_eq!(mosaic.height, 3);

        // First tile should remain in top-left pixel.
        assert_eq!(mosaic.rgba[0], 0);

        // Last tile should end up bottom-right pixel.
        let bottom_right_offset = ((3 * 3) - 1) * 4;
        assert_eq!(mosaic.rgba[bottom_right_offset], 8);
    }
}
