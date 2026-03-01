//! Integration tests for upload retry behavior.

mod common;

use std::sync::{Arc, Mutex};

use local_guard_upload::{RetryPolicy, UploadClient, UploadEnvelope, UploadError, UploadTransport};

#[derive(Debug)]
struct FlakyTransport {
    attempts: Mutex<u32>,
}

impl UploadTransport for FlakyTransport {
    fn send(&self, _envelope: &UploadEnvelope) -> Result<(), UploadError> {
        let mut attempts = self.attempts.lock().expect("attempt lock should work");
        *attempts += 1;
        if *attempts < 3 {
            Err(UploadError::Timeout)
        } else {
            Ok(())
        }
    }
}

#[test]
fn upload_retry_policy_tests_recovers_from_transient_failures() {
    let payload = common::fixture_payload();
    let transport = Arc::new(FlakyTransport {
        attempts: Mutex::new(0),
    });
    let client = UploadClient::new(
        "https://api.example.test/ingest",
        RetryPolicy {
            max_retries: 3,
            base_delay_ms: 1,
            max_delay_ms: 10,
            jitter_ms: 0,
        },
        transport,
    )
    .expect("upload client should build");

    let report = client
        .upload_payload(&payload, "token")
        .expect("upload should eventually succeed");
    assert_eq!(report.attempts, 3);
}
