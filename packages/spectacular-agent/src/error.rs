use spectacular_llms::ProviderError;
use std::error::Error;
use std::fmt::{self, Display};

#[derive(Debug)]
pub enum AgentError {
    EmptyRunQueue,
    CancellationError,
    CapabilityMismatch { capability: &'static str },
    ContentFiltered,
    ContextLimitError { reason: String },
    MalformedProviderResponse { reason: String },
    ProviderCapabilityError { reason: String },
    ProviderFinishError { reason: String },
    ProviderNetworkError { reason: String },
    ProviderParsingError { reason: String },
    ValidationError { message: String },
    Provider(ProviderError),
}

impl Display for AgentError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AgentError::EmptyRunQueue => formatter.write_str("no queued agent run is available"),
            AgentError::CancellationError => formatter.write_str("agent run was cancelled"),
            AgentError::CapabilityMismatch { capability } => {
                write!(
                    formatter,
                    "provider does not support required capability `{capability}`"
                )
            }
            AgentError::ContentFiltered => {
                formatter.write_str("request was blocked by the model's safety guardrails")
            }
            AgentError::ContextLimitError { reason } => {
                write!(formatter, "provider context limit exceeded: {reason}")
            }
            AgentError::MalformedProviderResponse { reason } => {
                write!(
                    formatter,
                    "provider returned a malformed response: {reason}"
                )
            }
            AgentError::ProviderCapabilityError { reason } => {
                write!(formatter, "provider capability error: {reason}")
            }
            AgentError::ProviderFinishError { reason } => {
                write!(formatter, "provider finished with an error: {reason}")
            }
            AgentError::ProviderNetworkError { reason } => {
                write!(formatter, "provider network error: {reason}")
            }
            AgentError::ProviderParsingError { reason } => {
                write!(formatter, "provider response parsing failed: {reason}")
            }
            AgentError::ValidationError { message } => {
                write!(formatter, "structured output validation failed: {message}")
            }
            AgentError::Provider(error) => write!(formatter, "provider call failed: {error}"),
        }
    }
}

impl Error for AgentError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            AgentError::Provider(error) => Some(error),
            AgentError::EmptyRunQueue
            | AgentError::CancellationError
            | AgentError::CapabilityMismatch { .. }
            | AgentError::ContentFiltered
            | AgentError::ContextLimitError { .. }
            | AgentError::MalformedProviderResponse { .. }
            | AgentError::ProviderCapabilityError { .. }
            | AgentError::ProviderFinishError { .. }
            | AgentError::ProviderNetworkError { .. }
            | AgentError::ProviderParsingError { .. }
            | AgentError::ValidationError { .. } => None,
        }
    }
}

impl From<ProviderError> for AgentError {
    fn from(error: ProviderError) -> Self {
        match error {
            ProviderError::CancellationError => Self::CancellationError,
            ProviderError::MalformedResponse {
                provider_name,
                reason,
            } => Self::MalformedProviderResponse {
                reason: format!("{provider_name}: {reason}"),
            },
            ProviderError::ResponseParsingFailed {
                provider_name,
                reason,
            } => Self::ProviderParsingError {
                reason: format!("{provider_name}: {reason}"),
            },
            ProviderError::NetworkError {
                provider_name,
                reason,
            } => Self::ProviderNetworkError {
                reason: format!("{provider_name}: {reason}"),
            },
            ProviderError::ContextLimitExceeded {
                provider_name,
                reason,
            } => Self::ContextLimitError {
                reason: format!("{provider_name}: {reason}"),
            },
            ProviderError::CapabilityMismatch {
                provider_name,
                capability,
            } => Self::ProviderCapabilityError {
                reason: format!("{provider_name}: unsupported capability `{capability}`"),
            },
            error => Self::Provider(error),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
