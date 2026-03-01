//! Tests metadata integrity from deterministic frame fixtures.

use local_guard_core::{Frame, build_metadata};

#[test]
fn metadata_integrity_tests_include_required_fields() {
    let mut frames = Vec::new();
    for timestamp in [100_u64, 200, 300] {
        frames.push(
            Frame::new("display-a", 2, 2, timestamp, vec![1; 16]).expect("frame should be valid"),
        );
    }

    let metadata = build_metadata(&frames, "session-123").expect("metadata should build");
    assert_eq!(metadata.start_timestamp_ms, 100);
    assert_eq!(metadata.end_timestamp_ms, 300);
    assert_eq!(metadata.screen_id, "display-a");
    assert_eq!(metadata.source_width, 2);
    assert_eq!(metadata.source_height, 2);
    assert_eq!(metadata.session_id, "session-123");
    assert_eq!(metadata.frame_count, 3);
}
