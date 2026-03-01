//! Integration tests for consent gating behavior.

use local_guard_ui::{UiAuthState, UiState};

#[test]
fn consent_gate_tests_requires_explicit_consent_before_capture() {
    let mut state = UiState::new("v0.1.0");
    state.auth = UiAuthState::Authenticated;
    state.select_display("display-1");
    assert!(!state.can_start_capture());

    state.set_consent(true);
    assert!(state.can_start_capture());
}
