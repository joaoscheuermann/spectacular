use spectacular_agent::{
    AgentError, AgentErrorDetails, AgentErrorKind, AgentErrorReport, AgentErrorStage,
};
use spectacular_llms::{ProviderError, ProviderErrorDiagnostics, ProviderErrorStage};

#[test]
fn provider_malformed_response_maps_to_agent_category() {
    let error = AgentError::from(ProviderError::MalformedResponse {
        provider_name: "Fake".to_owned(),
        reason: "missing choices".to_owned(),
        diagnostics: None,
    });

    assert!(matches!(
        error,
        AgentError::MalformedProviderResponse { .. }
    ));
    assert_eq!(
        error.to_string(),
        "provider returned a malformed response: Fake: missing choices"
    );
}

#[test]
fn provider_parsing_failure_maps_to_agent_category() {
    let error = AgentError::from(ProviderError::ResponseParsingFailed {
        provider_name: "Fake".to_owned(),
        reason: "bad chunk".to_owned(),
        diagnostics: None,
    });

    assert!(matches!(error, AgentError::ProviderParsingError { .. }));
}

#[test]
fn provider_network_failure_maps_to_agent_category() {
    let error = AgentError::from(ProviderError::NetworkError {
        provider_name: "Fake".to_owned(),
        reason: "disconnect".to_owned(),
        diagnostics: None,
    });

    assert!(matches!(error, AgentError::ProviderNetworkError { .. }));
}

#[test]
fn provider_context_limit_maps_to_agent_category() {
    let error = AgentError::from(ProviderError::ContextLimitExceeded {
        provider_name: "Fake".to_owned(),
        reason: "too long".to_owned(),
    });

    assert!(matches!(error, AgentError::ContextLimitError { .. }));
}

#[test]
fn provider_capability_mismatch_maps_to_agent_category() {
    let error = AgentError::from(ProviderError::CapabilityMismatch {
        provider_name: "Fake".to_owned(),
        capability: "tools".to_owned(),
    });

    assert!(matches!(error, AgentError::ProviderCapabilityError { .. }));
}

#[test]
fn provider_cancellation_maps_to_agent_cancellation() {
    let error = AgentError::from(ProviderError::CancellationError);

    assert!(matches!(error, AgentError::CancellationError));
}

#[test]
fn error_report_provider_http_status_exposes_structured_details() {
    let error = AgentError::from(ProviderError::ProviderUnavailable {
        provider_name: "Fake".to_owned(),
        diagnostics: Some(
            ProviderErrorDiagnostics::new(ProviderErrorStage::HttpStatus)
                .with_http_status(503)
                .with_excerpt("temporary outage")
                .with_debug_event("fake_error_body")
                .boxed(),
        ),
    });

    let report = AgentErrorReport::from_error(&error);
    let details = report.details.unwrap();

    assert_eq!(details.kind, AgentErrorKind::ProviderUnavailable);
    assert_eq!(details.provider.as_deref(), Some("Fake"));
    assert_eq!(details.stage, Some(AgentErrorStage::HttpStatus));
    assert!(details.retryable);
    assert_eq!(details.http_status, Some(503));
    assert_eq!(details.excerpt.as_deref(), Some("temporary outage"));
    assert_eq!(details.debug_events, vec!["fake_error_body"]);
}

#[test]
fn error_report_formats_compact_display_diagnostics() {
    let details = AgentErrorDetails {
        kind: AgentErrorKind::ProviderUnavailable,
        provider: Some("Fake".to_owned()),
        stage: Some(AgentErrorStage::HttpStatus),
        retryable: true,
        http_status: Some(503),
        provider_code: Some("temporarily_unavailable".to_owned()),
        excerpt: Some("temporary outage".to_owned()),
        debug_events: vec!["fake_error_body".to_owned(), "stream_error".to_owned()],
    };

    assert_eq!(
        details.to_string(),
        "kind: provider_unavailable\nprovider: Fake\nstage: http_status\nretryable: true\nhttp status: 503\nprovider code: temporarily_unavailable\nexcerpt: temporary outage\ndebug events: fake_error_body, stream_error"
    );
}

#[test]
fn error_report_provider_auth_failure_has_authentication_kind() {
    let error = AgentError::from(ProviderError::AuthenticationFailed {
        provider_name: "OpenRouter".to_owned(),
        reason: "credentials rejected with status 401".to_owned(),
        diagnostics: Some(
            ProviderErrorDiagnostics::new(ProviderErrorStage::HttpStatus)
                .with_http_status(401)
                .with_provider_code("invalid_api_key")
                .with_excerpt("invalid key")
                .with_debug_event("chat_response_error_body")
                .boxed(),
        ),
    });

    let report = AgentErrorReport::from_error(&error);
    let details = report.details.unwrap();

    assert_eq!(details.kind, AgentErrorKind::Authentication);
    assert_eq!(details.provider.as_deref(), Some("OpenRouter"));
    assert_eq!(details.stage, Some(AgentErrorStage::HttpStatus));
    assert!(!details.retryable);
    assert_eq!(details.http_status, Some(401));
    assert_eq!(details.provider_code.as_deref(), Some("invalid_api_key"));
    assert_eq!(details.excerpt.as_deref(), Some("invalid key"));
}

#[test]
fn error_report_post_output_provider_error_is_not_retryable() {
    let error = AgentError::from(ProviderError::ProviderUnavailable {
        provider_name: "Fake".to_owned(),
        diagnostics: Some(
            ProviderErrorDiagnostics::new(ProviderErrorStage::HttpStatus)
                .with_http_status(503)
                .boxed(),
        ),
    });

    let report = AgentErrorReport::from_error_with_provider_output(&error, true);
    let details = report.details.unwrap();

    assert_eq!(details.kind, AgentErrorKind::ProviderUnavailable);
    assert!(!details.retryable);
}

#[test]
fn error_report_provider_parse_failure_preserves_payload_breadcrumbs() {
    let error = AgentError::from(ProviderError::ResponseParsingFailed {
        provider_name: "Fake".to_owned(),
        reason: "expected value".to_owned(),
        diagnostics: Some(
            ProviderErrorDiagnostics::new(ProviderErrorStage::PayloadParse)
                .with_excerpt("{bad json")
                .with_debug_event("payload_parse_error")
                .boxed(),
        ),
    });

    let report = AgentErrorReport::from_error(&error);
    let details = report.details.unwrap();

    assert_eq!(details.kind, AgentErrorKind::ProviderResponseParse);
    assert_eq!(details.stage, Some(AgentErrorStage::PayloadParse));
    assert_eq!(details.provider.as_deref(), Some("Fake"));
    assert_eq!(details.excerpt.as_deref(), Some("{bad json"));
    assert_eq!(details.debug_events, vec!["payload_parse_error"]);
}
