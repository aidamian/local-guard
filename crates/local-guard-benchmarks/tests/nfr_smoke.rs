//! Benchmark smoke test for deterministic core/mosaic/upload loop.

use std::time::Instant;

use local_guard_core::Frame;
use local_guard_mosaic::compose_temporal_mosaic;
use local_guard_upload::idempotency_key_for_payload;

#[test]
fn benchmark_pipeline_smoke_prints_latency() {
    let mut frames = Vec::new();
    for index in 0..9_u64 {
        frames.push(
            Frame::new("display-1", 64, 64, index, vec![index as u8; 64 * 64 * 4])
                .expect("frame should be valid"),
        );
    }

    let start = Instant::now();
    let mut key_lengths = 0usize;

    for _ in 0..100 {
        let mosaic = compose_temporal_mosaic(&frames).expect("mosaic should compose");
        let payload = local_guard_core::MosaicPayload {
            schema_version: "v1".to_string(),
            metadata: local_guard_core::BatchMetadata {
                start_timestamp_ms: 1,
                end_timestamp_ms: 9,
                screen_id: "display-1".to_string(),
                source_width: 64,
                source_height: 64,
                session_id: "bench-session".to_string(),
                frame_count: 9,
            },
            mosaic_width: mosaic.width,
            mosaic_height: mosaic.height,
            mosaic_rgba: mosaic.rgba,
        };
        key_lengths += idempotency_key_for_payload(&payload).len();
    }

    let elapsed_ms = start.elapsed().as_millis();
    println!("benchmark_pipeline_elapsed_ms={elapsed_ms}");
    println!("benchmark_idempotency_key_total_len={key_lengths}");

    // This is a lightweight guardrail; strict NFR checks are environment-specific.
    assert!(
        elapsed_ms < 5_000,
        "pipeline smoke benchmark should stay bounded"
    );
}
