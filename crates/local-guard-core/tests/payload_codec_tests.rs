//! Tests payload serialization and deserialization stability.

use local_guard_core::{BatchMetadata, MosaicPayload, SCHEMA_VERSION_V1};

#[test]
fn payload_codec_tests_round_trip_json() {
    let payload = MosaicPayload {
        schema_version: SCHEMA_VERSION_V1.to_string(),
        metadata: BatchMetadata {
            start_timestamp_ms: 1,
            end_timestamp_ms: 9,
            screen_id: "display-a".to_string(),
            source_width: 2,
            source_height: 2,
            session_id: "session-abc".to_string(),
            frame_count: 9,
        },
        mosaic_width: 6,
        mosaic_height: 6,
        mosaic_rgba: vec![9; 6 * 6 * 4],
    };

    let encoded = payload.to_json_bytes().expect("encoding should succeed");
    let decoded = MosaicPayload::from_json_bytes(&encoded).expect("decoding should succeed");
    assert_eq!(decoded, payload);
}
