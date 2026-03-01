//! Integration tests for analysis response parsing.

use local_guard_app::parse_analysis;

#[test]
fn analysis_response_parsing_tests_accepts_valid_payload() {
    let raw = r#"{
        "schema_version":"v1",
        "request_id":"req-1",
        "model_results":[{"model":"m1","label":"ok","confidence":0.9}],
        "categories":[{"category":"credential_theft","severity":70}]
    }"#;

    let signals = parse_analysis(raw).expect("analysis payload should parse");
    assert_eq!(signals.len(), 1);
    assert_eq!(signals[0].category, "credential_theft");
}
