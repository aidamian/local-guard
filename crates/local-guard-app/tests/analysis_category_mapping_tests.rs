//! Integration tests for analysis category mapping.

use local_guard_analysis_contract::RiskLevel;
use local_guard_app::parse_analysis;

#[test]
fn analysis_category_mapping_tests_preserve_unknown_categories() {
    let raw = r#"{
        "schema_version":"v1",
        "request_id":"req-2",
        "categories":[{"category":"future_category","severity":33}]
    }"#;

    let signals = parse_analysis(raw).expect("analysis payload should parse");
    assert_eq!(signals[0].category, "future_category");
    assert_eq!(signals[0].level, RiskLevel::Medium);
}
