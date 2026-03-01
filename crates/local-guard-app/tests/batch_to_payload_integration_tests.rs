//! Integration tests for batch-to-payload pipeline.

mod common;

use local_guard_app::batch_to_payload;

#[test]
fn batch_to_payload_integration_tests_produces_one_payload_for_nine_frames() {
    let frames = common::fixture_frames();
    let payload = batch_to_payload(&frames, "session-xyz").expect("payload should build");

    assert_eq!(payload.metadata.frame_count, 9);
    assert_eq!(payload.mosaic_width, 3);
    assert_eq!(payload.mosaic_height, 3);
    assert_eq!(payload.metadata.screen_id, "display-1");
}
