//! Integration tests for runtime kill-switch behavior.

use local_guard_app::capture_enabled_from_env;

#[test]
fn kill_switch_behavior_tests_disables_capture_when_env_is_false() {
    // Safety:
    // - Integration tests mutate process env in a single-threaded test body.
    // - We reset the variable before returning.
    unsafe { std::env::set_var("LOCAL_GUARD_CAPTURE_ENABLED", "false") };
    assert!(!capture_enabled_from_env());

    // Safety: see rationale above.
    unsafe { std::env::set_var("LOCAL_GUARD_CAPTURE_ENABLED", "true") };
    assert!(capture_enabled_from_env());

    // Safety: see rationale above.
    unsafe { std::env::remove_var("LOCAL_GUARD_CAPTURE_ENABLED") };
}
