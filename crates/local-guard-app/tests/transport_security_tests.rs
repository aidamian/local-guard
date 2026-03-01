//! Integration tests for transport security URL checks.

use local_guard_app::is_https_endpoint;

#[test]
fn transport_security_tests_rejects_non_https_endpoints() {
    assert!(is_https_endpoint("https://api.example.test/ingest"));
    assert!(!is_https_endpoint("http://api.example.test/ingest"));
}
