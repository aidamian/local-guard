//! Integration tests for runtime status projection.

use local_guard_app::project_runtime_status;
use local_guard_ui::{StageStatus, UiAuthState, UiState};

#[test]
fn runtime_status_projection_tests_reflects_ui_state() {
    let mut state = UiState::new("v0.1.0");
    state.auth = UiAuthState::Authenticated;
    state.set_consent(true);
    state.select_display("display-1");
    state.capture = StageStatus::Running;
    state.network = StageStatus::Healthy;
    state.upload = StageStatus::Degraded;
    state.analysis_status = "Medium risk".to_string();

    let snapshot = project_runtime_status(&state);
    assert!(snapshot.capture_allowed);
    assert_eq!(snapshot.capture, "Running");
    assert_eq!(snapshot.network, "Healthy");
    assert_eq!(snapshot.upload, "Degraded");
    assert_eq!(snapshot.analysis, "Medium risk");
}
