//! Integration tests for display selection.

use local_guard_app::select_display;
use local_guard_capture::DisplayInfo;

#[test]
fn display_selection_tests_selects_matching_display() {
    let displays = vec![
        DisplayInfo {
            id: "display-a".to_string(),
            name: "A".to_string(),
            width: 1920,
            height: 1080,
        },
        DisplayInfo {
            id: "display-b".to_string(),
            name: "B".to_string(),
            width: 1280,
            height: 720,
        },
    ];

    let selected = select_display(&displays, "display-b").expect("display should be found");
    assert_eq!(selected.id, "display-b");
}
