//! Integration tests for auth capture gating.

use local_guard_app::auth_allows_capture;
use local_guard_auth::{AuthStateMachine, SessionToken};

#[test]
fn auth_guard_tests_blocks_capture_after_expiry() {
    let mut machine = AuthStateMachine::new();
    machine.on_login_success(SessionToken {
        access_token: "token".to_string(),
        session_id: "session".to_string(),
        expires_at_ms: 50,
    });

    assert!(auth_allows_capture(&machine, 49));
    assert!(!auth_allows_capture(&machine, 50));
}
