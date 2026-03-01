//! Integration tests for auth state machine transitions.

use local_guard_auth::{AuthState, AuthStateMachine, SessionToken};

#[test]
fn auth_state_machine_tests_transitions_to_reauth_on_expiry() {
    let mut machine = AuthStateMachine::new();
    machine.on_login_success(SessionToken {
        access_token: "token".to_string(),
        session_id: "session".to_string(),
        expires_at_ms: 10,
    });
    machine.on_tick(11);
    assert!(matches!(machine.state(), AuthState::ReauthRequired));
}
