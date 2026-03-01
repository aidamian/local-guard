//! Integration tests for log redaction.

use local_guard_app::redact_sensitive;

#[test]
fn log_redaction_tests_removes_obvious_secret_markers() {
    let raw = "authorization=Bearer abc123";
    let redacted = redact_sensitive(raw);

    assert!(redacted.contains("<redacted>"));
    assert!(!redacted.contains("abc123"));
}
