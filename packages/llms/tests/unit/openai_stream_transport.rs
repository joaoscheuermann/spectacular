#[test]
fn openai_http_status_diagnostics_preserve_status_code_and_redacted_body() {
    let body = r#"{"error":{"code":"invalid_api_key","message":"bad sk-secret_value"}}"#;

    let diagnostics = http_status_body_diagnostics(401, body, "responses_error_body");

    assert_eq!(diagnostics.stage, Some(ProviderErrorStage::HttpStatus));
    assert_eq!(diagnostics.http_status, Some(401));
    assert_eq!(diagnostics.provider_code.as_deref(), Some("invalid_api_key"));
    assert_eq!(diagnostics.debug_events, vec!["responses_error_body"]);
    let excerpt = diagnostics.excerpt.unwrap();
    assert!(excerpt.contains("[redacted]"));
    assert!(!excerpt.contains("sk-secret_value"));
}
