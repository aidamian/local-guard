#![warn(missing_docs)]
//! # local-guard-core
//!
//! ## Purpose
//! Defines the pure data model used across the `local-guard` workspace.
//!
//! ## Responsibilities
//! - Represent captured frames and bounded frame batches.
//! - Build deterministic batch metadata used by upload payloads.
//! - Encode/decode versioned mosaic payloads for transport.
//!
//! ## Data flow
//! Capture code emits [`Frame`] objects into [`FrameBatch`].
//! When a batch is complete, callers derive [`BatchMetadata`] and package the
//! mosaic bytes into [`MosaicPayload`].
//!
//! ## Ownership and lifetimes
//! Frames and payloads own their backing buffers (`Vec<u8>`) to avoid hidden
//! borrow/lifetime coupling between async pipeline stages.
//!
//! ## Error model
//! Validation failures (shape mismatch, empty session id, invalid capacity)
//! return [`CoreError`] variants with caller-actionable categorization.
//!
//! ## Security and privacy notes
//! This crate intentionally avoids logging frame bytes or session secrets.
//! Session identifiers are treated as opaque values and are never transformed.
//!
//! ## Example
//! ```rust
//! use local_guard_core::{deterministic_tile_order, Frame, FrameBatch};
//!
//! let mut batch = FrameBatch::new(9).expect("valid batch capacity");
//! for index in 0..9 {
//!     let frame = Frame::new("display-1", 2, 2, index, vec![0; 16]).unwrap();
//!     let _ = batch.push_frame(frame).unwrap();
//! }
//! assert_eq!(deterministic_tile_order(9).unwrap(), vec![0, 1, 2, 3, 4, 5, 6, 7, 8]);
//! ```

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Canonical schema tag for v1 mosaic payloads.
pub const SCHEMA_VERSION_V1: &str = "v1";

/// Represents one captured frame from a selected display.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Frame {
    /// Stable display identity from capture backend.
    pub screen_id: String,
    /// Source frame width in pixels.
    pub width: u32,
    /// Source frame height in pixels.
    pub height: u32,
    /// Capture time in Unix epoch milliseconds.
    pub captured_at_ms: u64,
    /// Raw RGBA pixel buffer (`width * height * 4` bytes).
    pub rgba: Vec<u8>,
}

impl Frame {
    /// Constructs a validated frame.
    ///
    /// # Errors
    /// Returns [`CoreError::InvalidFrameShape`] when the pixel buffer length is
    /// not exactly `width * height * 4`.
    pub fn new(
        screen_id: impl Into<String>,
        width: u32,
        height: u32,
        captured_at_ms: u64,
        rgba: Vec<u8>,
    ) -> Result<Self, CoreError> {
        let expected_len = required_rgba_len(width, height)?;
        if rgba.len() != expected_len {
            return Err(CoreError::InvalidFrameShape {
                expected: expected_len,
                actual: rgba.len(),
            });
        }

        Ok(Self {
            screen_id: screen_id.into(),
            width,
            height,
            captured_at_ms,
            rgba,
        })
    }
}

/// Bounded buffer that emits complete frame batches.
#[derive(Debug, Clone)]
pub struct FrameBatch {
    capacity: usize,
    frames: Vec<Frame>,
}

impl FrameBatch {
    /// Creates a new bounded frame batch buffer.
    ///
    /// # Errors
    /// Returns [`CoreError::InvalidBatchCapacity`] when `capacity == 0`.
    pub fn new(capacity: usize) -> Result<Self, CoreError> {
        if capacity == 0 {
            return Err(CoreError::InvalidBatchCapacity);
        }

        Ok(Self {
            capacity,
            frames: Vec::with_capacity(capacity),
        })
    }

    /// Pushes one frame into the batch buffer.
    ///
    /// # Returns
    /// - `Ok(None)` when the buffer is not yet full.
    /// - `Ok(Some(Vec<Frame>))` when exactly `capacity` frames have been buffered.
    ///
    /// # Side effects
    /// On full batch emission, the internal buffer is drained and reset for the
    /// next chronological window.
    pub fn push_frame(&mut self, frame: Frame) -> Result<Option<Vec<Frame>>, CoreError> {
        if self.frames.is_empty() {
            self.frames.push(frame);
            return Ok(None);
        }

        // Invariant:
        // - All frames in one batch must come from the same display and geometry.
        let first = &self.frames[0];
        if first.screen_id != frame.screen_id
            || first.width != frame.width
            || first.height != frame.height
        {
            return Err(CoreError::BatchInvariantViolation(
                "frame does not match active batch display or geometry".to_string(),
            ));
        }

        self.frames.push(frame);
        if self.frames.len() == self.capacity {
            let emitted = std::mem::take(&mut self.frames);
            self.frames = Vec::with_capacity(self.capacity);
            return Ok(Some(emitted));
        }

        Ok(None)
    }

    /// Returns current buffered frame count.
    pub fn len(&self) -> usize {
        self.frames.len()
    }

    /// Returns configured batch capacity.
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Returns `true` when no frames are buffered.
    pub fn is_empty(&self) -> bool {
        self.frames.is_empty()
    }
}

/// Metadata attached to each upload payload.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BatchMetadata {
    /// Inclusive start timestamp of the batch window.
    pub start_timestamp_ms: u64,
    /// Inclusive end timestamp of the batch window.
    pub end_timestamp_ms: u64,
    /// Display identity propagated from frame source.
    pub screen_id: String,
    /// Source frame width.
    pub source_width: u32,
    /// Source frame height.
    pub source_height: u32,
    /// Session identifier assigned by auth layer.
    pub session_id: String,
    /// Number of frames used in the batch.
    pub frame_count: usize,
}

/// Versioned payload sent to protected ingest API.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MosaicPayload {
    /// Payload schema version for server contract negotiation.
    pub schema_version: String,
    /// Batch metadata for traceability.
    pub metadata: BatchMetadata,
    /// Mosaic image width in pixels.
    pub mosaic_width: u32,
    /// Mosaic image height in pixels.
    pub mosaic_height: u32,
    /// Mosaic pixel bytes in RGBA row-major layout.
    pub mosaic_rgba: Vec<u8>,
}

impl MosaicPayload {
    /// Serializes payload to compact JSON bytes.
    ///
    /// # Errors
    /// Returns [`CoreError::Codec`] when JSON serialization fails.
    pub fn to_json_bytes(&self) -> Result<Vec<u8>, CoreError> {
        serde_json::to_vec(self).map_err(CoreError::Codec)
    }

    /// Deserializes payload from JSON bytes.
    ///
    /// # Errors
    /// Returns [`CoreError::Codec`] when JSON decoding fails.
    pub fn from_json_bytes(raw: &[u8]) -> Result<Self, CoreError> {
        serde_json::from_slice(raw).map_err(CoreError::Codec)
    }
}

/// Produces deterministic tile order for temporal mosaics.
///
/// # Semantics
/// The returned indices are left-to-right, top-to-bottom, chronological order.
pub fn deterministic_tile_order(batch_size: usize) -> Result<Vec<usize>, CoreError> {
    if batch_size == 0 {
        return Err(CoreError::InvalidBatchCapacity);
    }

    Ok((0..batch_size).collect())
}

/// Computes batch metadata from a completed frame set.
///
/// # Errors
/// Returns [`CoreError::EmptyFrameSet`] when `frames` is empty.
/// Returns [`CoreError::BatchInvariantViolation`] when frames mismatch by
/// display id or dimensions.
pub fn build_metadata(
    frames: &[Frame],
    session_id: impl Into<String>,
) -> Result<BatchMetadata, CoreError> {
    if frames.is_empty() {
        return Err(CoreError::EmptyFrameSet);
    }

    let session_id = session_id.into();
    if session_id.trim().is_empty() {
        return Err(CoreError::InvalidSessionId);
    }

    let first = &frames[0];
    let mut start = first.captured_at_ms;
    let mut end = first.captured_at_ms;

    for frame in frames {
        if frame.screen_id != first.screen_id
            || frame.width != first.width
            || frame.height != first.height
        {
            return Err(CoreError::BatchInvariantViolation(
                "metadata cannot be built from mixed display identities or dimensions".to_string(),
            ));
        }

        start = start.min(frame.captured_at_ms);
        end = end.max(frame.captured_at_ms);
    }

    Ok(BatchMetadata {
        start_timestamp_ms: start,
        end_timestamp_ms: end,
        screen_id: first.screen_id.clone(),
        source_width: first.width,
        source_height: first.height,
        session_id,
        frame_count: frames.len(),
    })
}

/// Error type for core domain validation and codec failures.
#[derive(Debug, Error)]
pub enum CoreError {
    /// Frame buffer shape does not match declared geometry.
    #[error("invalid frame shape: expected {expected} bytes, got {actual}")]
    InvalidFrameShape {
        /// Expected RGBA byte count.
        expected: usize,
        /// Actual RGBA byte count.
        actual: usize,
    },
    /// Batch capacity must be strictly positive.
    #[error("batch capacity must be greater than zero")]
    InvalidBatchCapacity,
    /// Frame set cannot be empty for metadata or mosaic operations.
    #[error("frame set is empty")]
    EmptyFrameSet,
    /// Session id cannot be empty.
    #[error("session id is empty")]
    InvalidSessionId,
    /// Frame batch invariants were violated.
    #[error("batch invariant violation: {0}")]
    BatchInvariantViolation(String),
    /// JSON encoding/decoding error.
    #[error("payload codec failure: {0}")]
    Codec(#[from] serde_json::Error),
}

fn required_rgba_len(width: u32, height: u32) -> Result<usize, CoreError> {
    let pixels = (width as usize)
        .checked_mul(height as usize)
        .ok_or_else(|| {
            CoreError::BatchInvariantViolation("frame dimensions overflow".to_string())
        })?;

    pixels
        .checked_mul(4)
        .ok_or_else(|| CoreError::BatchInvariantViolation("rgba length overflow".to_string()))
}
