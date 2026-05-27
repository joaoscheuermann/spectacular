mod details;

pub use details::{AgentErrorDetails, AgentErrorKind, AgentErrorStage};

use spectacular_llms::{ProviderError, ProviderErrorDiagnostics};
use std::error::Error;
use std::fmt::{self, Display};

/// Errors returned by agent run orchestration, validation, and provider calls.
#[derive(Debug)]
#[non_exhaustive]
pub enum AgentError {
    EmptyRunQueue,
    CancellationError,
    CapabilityMismatch {
        capability: &'static str,
    },
    ContentFiltered,
    ContextLimitError {
        reason: String,
        provider: Option<String>,
        diagnostics: Option<Box<ProviderErrorDiagnostics>>,
    },
    MalformedProviderResponse {
        reason: String,
        provider: Option<String>,
        diagnostics: Option<Box<ProviderErrorDiagnostics>>,
    },
    ProviderCapabilityError {
        reason: String,
        provider: Option<String>,
        diagnostics: Option<Box<ProviderErrorDiagnostics>>,
    },
    ProviderFinishError {
        reason: String,
    },
    ProviderNetworkError {
        reason: String,
        provider: Option<String>,
        diagnostics: Option<Box<ProviderErrorDiagnostics>>,
    },
    ProviderParsingError {
        reason: String,
        provider: Option<String>,
        diagnostics: Option<Box<ProviderErrorDiagnostics>>,
    },
    ValidationError {
        message: String,
    },
    Provider(ProviderError),
}

/// User-facing error message plus optional structured diagnostics.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AgentErrorReport {
    /// Compact message suitable for visible transcript or CLI output.
    pub message: String,
    /// Machine-readable diagnostics for details panes, logs, and persisted sessions.
    pub details: Option<AgentErrorDetails>,
}

impl AgentErrorReport {
    /// Builds a report for an error before any provider output has escaped.
    pub fn from_error(error: &AgentError) -> Self {
        Self::from_error_with_provider_output(error, false)
    }

    /// Builds a report with retryability adjusted for already-streamed provider output.
    pub fn from_error_with_provider_output(
        error: &AgentError,
        provider_output_started: bool,
    ) -> Self {
        Self {
            message: error.to_string(),
            details: AgentErrorDetails::from_error(error, provider_output_started),
        }
    }
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
            AgentError::ContextLimitError { reason, .. } => {
                write!(formatter, "provider context limit exceeded: {reason}")
            }
            AgentError::MalformedProviderResponse { reason, .. } => {
                write!(
                    formatter,
                    "provider returned a malformed response: {reason}"
                )
            }
            AgentError::ProviderCapabilityError { reason, .. } => {
                write!(formatter, "provider capability error: {reason}")
            }
            AgentError::ProviderFinishError { reason } => {
                write!(formatter, "provider finished with an error: {reason}")
            }
            AgentError::ProviderNetworkError { reason, .. } => {
                write!(formatter, "provider network error: {reason}")
            }
            AgentError::ProviderParsingError { reason, .. } => {
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
                diagnostics,
            } => Self::MalformedProviderResponse {
                reason: format!("{provider_name}: {reason}"),
                provider: Some(provider_name),
                diagnostics,
            },
            ProviderError::ResponseParsingFailed {
                provider_name,
                reason,
                diagnostics,
            } => Self::ProviderParsingError {
                reason: format!("{provider_name}: {reason}"),
                provider: Some(provider_name),
                diagnostics,
            },
            ProviderError::NetworkError {
                provider_name,
                reason,
                diagnostics,
            } => Self::ProviderNetworkError {
                reason: format!("{provider_name}: {reason}"),
                provider: Some(provider_name),
                diagnostics,
            },
            ProviderError::ContextLimitExceeded {
                provider_name,
                reason,
            } => Self::ContextLimitError {
                reason: format!("{provider_name}: {reason}"),
                provider: Some(provider_name),
                diagnostics: None,
            },
            ProviderError::CapabilityMismatch {
                provider_name,
                capability,
            } => Self::ProviderCapabilityError {
                reason: format!("{provider_name}: unsupported capability `{capability}`"),
                provider: Some(provider_name),
                diagnostics: None,
            },
            error => Self::Provider(error),
        }
    }
}
