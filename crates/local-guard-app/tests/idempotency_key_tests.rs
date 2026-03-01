//! Integration tests for idempotency key stability.

mod common;

use local_guard_upload::idempotency_key_for_payload;

#[test]
fn idempotency_key_tests_stable_for_identical_payloads() {
    let payload_a = common::fixture_payload();
    let payload_b = common::fixture_payload();

    let key_a = idempotency_key_for_payload(&payload_a);
    let key_b = idempotency_key_for_payload(&payload_b);

    assert_eq!(key_a, key_b);
}
