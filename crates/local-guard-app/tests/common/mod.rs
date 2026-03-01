//! Shared fixtures for app integration tests.

use local_guard_app::batch_to_payload;
use local_guard_core::{Frame, MosaicPayload};

/// Creates deterministic 9-frame fixture for mosaic pipeline tests.
#[allow(dead_code)]
pub fn fixture_frames() -> Vec<Frame> {
    let mut frames = Vec::new();
    for index in 0..9_u64 {
        let value = index as u8;
        frames.push(
            Frame::new("display-1", 1, 1, 1_000 + index, vec![value, 0, 0, 255])
                .expect("frame fixture should be valid"),
        );
    }
    frames
}

/// Creates deterministic payload fixture.
#[allow(dead_code)]
pub fn fixture_payload() -> MosaicPayload {
    batch_to_payload(&fixture_frames(), "session-xyz").expect("payload fixture should build")
}
