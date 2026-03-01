//! Integration tests for capture scheduling.

use local_guard_app::schedule_capture;

#[test]
fn capture_scheduler_tests_generates_one_hz_schedule() {
    let times = schedule_capture(1, 1_000, 3).expect("schedule should be generated");
    assert_eq!(times, vec![1_000, 2_000, 3_000]);
}
