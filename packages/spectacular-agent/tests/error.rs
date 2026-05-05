use spectacular_agent::AgentError;
use spectacular_llms::ProviderError;

#[test]
fn provider_malformed_response_maps_to_agent_category() {
    let error = AgentError::from(ProviderError::MalformedResponse {
        provider_name: "Fake".to_owned(),
        reason: "missing choices".to_owned(),
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
    });

    assert!(matches!(error, AgentError::ProviderParsingError { .. }));
}

#[test]
fn provider_network_failure_maps_to_agent_category() {
    let error = AgentError::from(ProviderError::NetworkError {
        provider_name: "Fake".to_owned(),
        reason: "disconnect".to_owned(),
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
