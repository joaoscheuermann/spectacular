use spectacular_agent::{AgentError, AgentErrorKind, AgentErrorReport, AgentErrorStage};
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
                .with_debug_event("fake_error_body"),
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
fn error_report_post_output_provider_error_is_not_retryable() {
    let error = AgentError::from(ProviderError::ProviderUnavailable {
        provider_name: "Fake".to_owned(),
        diagnostics: Some(
            ProviderErrorDiagnostics::new(ProviderErrorStage::HttpStatus).with_http_status(503),
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
                .with_debug_event("payload_parse_error"),
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
