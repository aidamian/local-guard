//! Validates contract fixtures against frozen JSON schemas.

use jsonschema::JSONSchema;
use serde_json::Value;

fn load_json(path: &str) -> Value {
    let raw = std::fs::read_to_string(path).expect("json file should be readable");
    serde_json::from_str(&raw).expect("json file should be valid")
}

fn compile_validator(schema_path: &str) -> JSONSchema {
    let schema = load_json(schema_path);
    JSONSchema::compile(&schema).expect("schema should compile")
}

#[test]
fn ingest_fixture_matches_schema() {
    let validator = compile_validator(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../contracts/ingest-request.schema.json"
    ));
    let fixture = load_json(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../contracts/fixtures/ingest-request.valid.json"
    ));
    assert!(
        validator.is_valid(&fixture),
        "ingest fixture should validate against schema"
    );
}

#[test]
fn analysis_fixture_matches_schema() {
    let validator = compile_validator(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../contracts/analysis-response.schema.json"
    ));
    let fixture = load_json(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../contracts/fixtures/analysis-response.valid.json"
    ));
    assert!(
        validator.is_valid(&fixture),
        "analysis fixture should validate against schema"
    );
}
