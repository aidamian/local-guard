//! Integration tests for VERSION propagation into runtime display.

use std::fs;

use local_guard_app::app_version;

#[test]
fn version_display_tests_matches_root_version_file() {
    let root_version_path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../VERSION");
    let root_version = fs::read_to_string(root_version_path).expect("VERSION should be readable");
    assert_eq!(app_version(), root_version.trim());
}
