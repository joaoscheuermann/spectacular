use serde::{Deserialize, Serialize};
use spectacular_llms::{ProviderError, ProviderErrorDiagnostics, ProviderErrorStage};
use std::error::Error;
use std::fmt::{self, Display};

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
        diagnostics: Option<ProviderErrorDiagnostics>,
    },
    MalformedProviderResponse {
        reason: String,
        provider: Option<String>,
        diagnostics: Option<ProviderErrorDiagnostics>,
    },
    ProviderCapabilityError {
        reason: String,
        provider: Option<String>,
        diagnostics: Option<ProviderErrorDiagnostics>,
    },
    ProviderFinishError {
        reason: String,
    },
    ProviderNetworkError {
        reason: String,
        provider: Option<String>,
        diagnostics: Option<ProviderErrorDiagnostics>,
    },
    ProviderParsingError {
        reason: String,
        provider: Option<String>,
        diagnostics: Option<ProviderErrorDiagnostics>,
    },
    ValidationError {
        message: String,
    },
    Provider(ProviderError),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AgentErrorReport {
    pub message: String,
    pub details: Option<AgentErrorDetails>,
}

impl AgentErrorReport {
    pub fn from_error(error: &AgentError) -> Self {
        Self::from_error_with_provider_output(error, false)
    }

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

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AgentErrorDetails {
    pub kind: AgentErrorKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stage: Option<AgentErrorStage>,
    pub retryable: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub http_status: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider_code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub excerpt: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub debug_events: Vec<String>,
}

impl AgentErrorDetails {
    fn from_error(error: &AgentError, provider_output_started: bool) -> Option<Self> {
        match error {
            AgentError::EmptyRunQueue | AgentError::CancellationError => None,
            AgentError::CapabilityMismatch { .. } => Some(Self::agent_validation(
                AgentErrorKind::Capability,
                None,
                false,
            )),
            AgentError::ContentFiltered => Some(Self::new(
                AgentErrorKind::ContentFilter,
                None,
                Some(AgentErrorStage::ProviderStream),
                false,
                None,
            )),
            AgentError::ContextLimitError {
                provider,
                diagnostics,
                ..
            } => Some(Self::from_diagnostics(
                AgentErrorKind::ContextLimit,
                provider.clone(),
                diagnostics.as_ref(),
                false,
            )),
            AgentError::MalformedProviderResponse {
                provider,
                diagnostics,
                ..
            } => Some(Self::from_diagnostics(
                AgentErrorKind::ProviderMalformedResponse,
                provider.clone(),
                diagnostics.as_ref(),
                false,
            )),
            AgentError::ProviderCapabilityError {
                provider,
                diagnostics,
                ..
            } => Some(Self::from_diagnostics(
                AgentErrorKind::Capability,
                provider.clone(),
                diagnostics.as_ref(),
                false,
            )),
            AgentError::ProviderFinishError { .. } => Some(Self::new(
                AgentErrorKind::ProviderStream,
                None,
                Some(AgentErrorStage::ProviderStream),
                false,
                None,
            )),
            AgentError::ProviderNetworkError {
                provider,
                diagnostics,
                ..
            } => Some(Self::from_diagnostics(
                AgentErrorKind::ProviderNetwork,
                provider.clone(),
                diagnostics.as_ref(),
                !provider_output_started,
            )),
            AgentError::ProviderParsingError {
                provider,
                diagnostics,
                ..
            } => Some(Self::from_diagnostics(
                AgentErrorKind::ProviderResponseParse,
                provider.clone(),
                diagnostics.as_ref(),
                false,
            )),
            AgentError::ValidationError { .. } => Some(Self::agent_validation(
                AgentErrorKind::Validation,
                None,
                false,
            )),
            AgentError::Provider(error) => {
                Self::from_provider_error(error, provider_output_started)
            }
        }
    }

    fn from_provider_error(error: &ProviderError, provider_output_started: bool) -> Option<Self> {
        if matches!(error, ProviderError::CancellationError) {
            return None;
        }

        let kind = match error {
            ProviderError::NetworkError { .. } => AgentErrorKind::ProviderNetwork,
            ProviderError::ResponseParsingFailed { .. } => AgentErrorKind::ProviderResponseParse,
            ProviderError::MalformedResponse { .. } => AgentErrorKind::ProviderMalformedResponse,
            ProviderError::StreamError { .. } => AgentErrorKind::ProviderStream,
            ProviderError::ContextLimitExceeded { .. } => AgentErrorKind::ContextLimit,
            ProviderError::CapabilityMismatch { .. } => AgentErrorKind::Capability,
            ProviderError::UnsupportedProvider { .. } => AgentErrorKind::UnknownProvider,
            ProviderError::InvalidApiKey | ProviderError::UnsupportedValidationMode => {
                AgentErrorKind::Validation
            }
            ProviderError::ModelFetchFailed { .. }
            | ProviderError::NoModelsReturned { .. }
            | ProviderError::ProviderUnavailable { .. }
            | ProviderError::AuthenticationRequired { .. }
            | ProviderError::AuthenticationFailed { .. }
            | ProviderError::StreamUnavailable { .. } => AgentErrorKind::ProviderUnavailable,
            ProviderError::CancellationError => return None,
        };

        let retryable = provider_error_retryable(error) && !provider_output_started;
        let mut details = Self::from_diagnostics(
            kind,
            error.provider_name().map(str::to_owned),
            error.diagnostics(),
            retryable,
        );
        if details.provider_code.is_none() {
            details.provider_code = error.provider_code().map(str::to_owned);
        }

        Some(details)
    }

    fn agent_validation(kind: AgentErrorKind, provider: Option<String>, retryable: bool) -> Self {
        Self::new(
            kind,
            provider,
            Some(AgentErrorStage::AgentValidation),
            retryable,
            None,
        )
    }

    fn from_diagnostics(
        kind: AgentErrorKind,
        provider: Option<String>,
        diagnostics: Option<&ProviderErrorDiagnostics>,
        retryable: bool,
    ) -> Self {
        let stage = diagnostics
            .and_then(|diagnostics| diagnostics.stage)
            .map(AgentErrorStage::from);
        let mut details = Self::new(kind, provider, stage, retryable, diagnostics);
        if details.stage.is_none() {
            details.stage = default_stage_for_kind(kind);
        }
        details
    }

    fn new(
        kind: AgentErrorKind,
        provider: Option<String>,
        stage: Option<AgentErrorStage>,
        retryable: bool,
        diagnostics: Option<&ProviderErrorDiagnostics>,
    ) -> Self {
        Self {
            kind,
            provider,
            stage,
            retryable,
            http_status: diagnostics.and_then(|diagnostics| diagnostics.http_status),
            provider_code: diagnostics.and_then(|diagnostics| diagnostics.provider_code.clone()),
            excerpt: diagnostics.and_then(|diagnostics| diagnostics.excerpt.clone()),
            debug_events: diagnostics
                .map(|diagnostics| diagnostics.debug_events.clone())
                .unwrap_or_default(),
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentErrorKind {
    ProviderUnavailable,
    ProviderNetwork,
    ProviderResponseParse,
    ProviderMalformedResponse,
    ProviderStream,
    ContextLimit,
    Capability,
    ContentFilter,
    Validation,
    UnknownProvider,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentErrorStage {
    RequestBuild,
    HttpRequest,
    HttpStatus,
    SseDecode,
    PayloadParse,
    ProviderStream,
    AgentValidation,
}

impl From<ProviderErrorStage> for AgentErrorStage {
    fn from(stage: ProviderErrorStage) -> Self {
        match stage {
            ProviderErrorStage::RequestBuild => Self::RequestBuild,
            ProviderErrorStage::HttpRequest => Self::HttpRequest,
            ProviderErrorStage::HttpStatus => Self::HttpStatus,
            ProviderErrorStage::SseDecode => Self::SseDecode,
            ProviderErrorStage::PayloadParse => Self::PayloadParse,
            ProviderErrorStage::ProviderStream => Self::ProviderStream,
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

fn default_stage_for_kind(kind: AgentErrorKind) -> Option<AgentErrorStage> {
    match kind {
        AgentErrorKind::ProviderUnavailable => Some(AgentErrorStage::HttpRequest),
        AgentErrorKind::ProviderNetwork => Some(AgentErrorStage::HttpRequest),
        AgentErrorKind::ProviderResponseParse => Some(AgentErrorStage::PayloadParse),
        AgentErrorKind::ProviderMalformedResponse => Some(AgentErrorStage::PayloadParse),
        AgentErrorKind::ProviderStream => Some(AgentErrorStage::ProviderStream),
        AgentErrorKind::ContextLimit => Some(AgentErrorStage::AgentValidation),
        AgentErrorKind::Capability => Some(AgentErrorStage::AgentValidation),
        AgentErrorKind::ContentFilter => Some(AgentErrorStage::ProviderStream),
        AgentErrorKind::Validation => Some(AgentErrorStage::AgentValidation),
        AgentErrorKind::UnknownProvider => Some(AgentErrorStage::AgentValidation),
    }
}

fn provider_error_retryable(error: &ProviderError) -> bool {
    match error {
        ProviderError::NetworkError { .. } => true,
        ProviderError::ProviderUnavailable { .. } => error
            .http_status()
            .map(transient_http_status)
            .unwrap_or(true),
        _ => error.http_status().is_some_and(transient_http_status),
    }
}

fn transient_http_status(status: u16) -> bool {
    matches!(status, 408 | 409 | 425 | 429 | 500 | 502 | 503 | 504)
}
