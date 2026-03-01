//! Integration tests for privacy guardrails.

use local_guard_app::redact_sensitive;

#[test]
fn privacy_guardrail_tests_avoid_password_leakage_in_logs() {
    let raw = "password=supersecret";
    let redacted = redact_sensitive(raw);
    assert!(!redacted.contains("supersecret"));
    assert!(redacted.contains("<redacted>"));
}
