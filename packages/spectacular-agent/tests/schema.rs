use serde_json::json;
use spectacular_agent::OutputSchema;

fn release_schema() -> OutputSchema {
    OutputSchema::new(json!({
        "type": "object",
        "required": ["summary", "status"],
        "properties": {
            "summary": {"type": "string"},
            "status": {"const": "ready"}
        },
        "additionalProperties": false
    }))
    .unwrap()
}

#[test]
fn schema_from_json_str_accepts_valid_schema() {
    let schema = OutputSchema::from_json_str(r#"{"type":"object"}"#);

    assert!(schema.is_ok());
}

#[test]
fn schema_from_json_str_rejects_invalid_json() {
    let error = OutputSchema::from_json_str("{").unwrap_err();

    assert!(error.to_string().contains("EOF"));
}

#[test]
fn schema_new_rejects_invalid_schema_shape() {
    let error = OutputSchema::new(json!({"type": 5})).unwrap_err();

    assert!(error.to_string().contains("not valid"));
}

#[test]
fn validate_response_accepts_matching_json() {
    let schema = release_schema();

    assert!(schema
        .validate_response(r#"{"summary":"done","status":"ready"}"#)
        .is_ok());
}

#[test]
fn validate_response_rejects_invalid_json() {
    let schema = release_schema();

    let error = schema.validate_response("not json").unwrap_err();

    assert!(error.to_string().contains("expected ident"));
}

#[test]
fn validate_response_rejects_schema_mismatch() {
    let schema = release_schema();

    let error = schema
        .validate_response(r#"{"summary":"done","status":"draft"}"#)
        .unwrap_err();

    assert!(error.to_string().contains("ready"));
}
