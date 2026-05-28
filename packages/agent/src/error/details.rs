use super::AgentError;
use llms::{ProviderError, ProviderErrorDiagnostics, ProviderErrorStage};
use serde::{Deserialize, Serialize};
use std::fmt::{self, Display};

/// Structured diagnostics attached to terminal agent errors when available.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AgentErrorDetails {
    /// Stable category used by durable logs and UI projections.
    pub kind: AgentErrorKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Provider that produced the failure, when the error came from provider I/O.
    pub provider: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Pipeline stage that best describes where the failure occurred.
    pub stage: Option<AgentErrorStage>,
    /// Whether retrying is safe before provider output has escaped to callers.
    pub retryable: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// HTTP status reported by the provider diagnostics.
    pub http_status: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Provider-specific error code, when one was returned.
    pub provider_code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Bounded provider response excerpt for debugging parse and status failures.
    pub excerpt: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    /// Sanitized debug events collected while handling the provider request.
    pub debug_events: Vec<String>,
}

impl AgentErrorDetails {
    pub(super) fn from_error(error: &AgentError, provider_output_started: bool) -> Option<Self> {
        match error {
            AgentError::EmptyRunQueue | AgentError::CancellationError => None,
            AgentError::CapabilityMismatch { .. } => Some(Self::agent_validation(
                AgentErrorKind::Capability,
                None,
                false,
            )),
            AgentError::ContentFiltered => Some(Self::new(DetailsInput {
                kind: AgentErrorKind::ContentFilter,
                provider: None,
                stage: Some(AgentErrorStage::ProviderStream),
                retryable: false,
                diagnostics: None,
            })),
            AgentError::ContextLimitError {
                provider,
                diagnostics,
                ..
            } => Some(Self::from_diagnostics(DiagnosticsInput {
                kind: AgentErrorKind::ContextLimit,
                provider: provider.clone(),
                diagnostics: diagnostics.as_deref(),
                retryable: false,
            })),
            AgentError::MalformedProviderResponse {
                provider,
                diagnostics,
                ..
            } => Some(Self::from_diagnostics(DiagnosticsInput {
                kind: AgentErrorKind::ProviderMalformedResponse,
                provider: provider.clone(),
                diagnostics: diagnostics.as_deref(),
                retryable: false,
            })),
            AgentError::ProviderCapabilityError {
                provider,
                diagnostics,
                ..
            } => Some(Self::from_diagnostics(DiagnosticsInput {
                kind: AgentErrorKind::Capability,
                provider: provider.clone(),
                diagnostics: diagnostics.as_deref(),
                retryable: false,
            })),
            AgentError::ProviderFinishError { .. } => Some(Self::new(DetailsInput {
                kind: AgentErrorKind::ProviderStream,
                provider: None,
                stage: Some(AgentErrorStage::ProviderStream),
                retryable: false,
                diagnostics: None,
            })),
            AgentError::ProviderNetworkError {
                provider,
                diagnostics,
                ..
            } => Some(Self::from_diagnostics(DiagnosticsInput {
                kind: AgentErrorKind::ProviderNetwork,
                provider: provider.clone(),
                diagnostics: diagnostics.as_deref(),
                retryable: !provider_output_started,
            })),
            AgentError::ProviderParsingError {
                provider,
                diagnostics,
                ..
            } => Some(Self::from_diagnostics(DiagnosticsInput {
                kind: AgentErrorKind::ProviderResponseParse,
                provider: provider.clone(),
                diagnostics: diagnostics.as_deref(),
                retryable: false,
            })),
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
        let kind = provider_error_kind(error)?;
        let retryable = provider_error_retryable(error) && !provider_output_started;
        let mut details = Self::from_diagnostics(DiagnosticsInput {
            kind,
            provider: error.provider_name().map(str::to_owned),
            diagnostics: error.diagnostics(),
            retryable,
        });
        if details.provider_code.is_none() {
            details.provider_code = error.provider_code().map(str::to_owned);
        }

        Some(details)
    }

    fn agent_validation(kind: AgentErrorKind, provider: Option<String>, retryable: bool) -> Self {
        Self::new(DetailsInput {
            kind,
            provider,
            stage: Some(AgentErrorStage::AgentValidation),
            retryable,
            diagnostics: None,
        })
    }

    fn from_diagnostics(input: DiagnosticsInput<'_>) -> Self {
        let stage = input
            .diagnostics
            .and_then(|diagnostics| diagnostics.stage)
            .map(AgentErrorStage::from);
        let mut details = Self::new(DetailsInput {
            kind: input.kind,
            provider: input.provider,
            stage,
            retryable: input.retryable,
            diagnostics: input.diagnostics,
        });
        if details.stage.is_none() {
            details.stage = default_stage_for_kind(input.kind);
        }
        details
    }

    fn new(input: DetailsInput<'_>) -> Self {
        Self {
            kind: input.kind,
            provider: input.provider,
            stage: input.stage,
            retryable: input.retryable,
            http_status: input
                .diagnostics
                .and_then(|diagnostics| diagnostics.http_status),
            provider_code: input
                .diagnostics
                .and_then(|diagnostics| diagnostics.provider_code.clone()),
            excerpt: input
                .diagnostics
                .and_then(|diagnostics| diagnostics.excerpt.clone()),
            debug_events: input
                .diagnostics
                .map(|diagnostics| diagnostics.debug_events.clone())
                .unwrap_or_default(),
        }
    }
}

impl Display for AgentErrorDetails {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut lines = Vec::new();
        lines.push(format!("kind: {}", self.kind.as_str()));
        if let Some(provider) = self.provider.as_deref() {
            lines.push(format!("provider: {provider}"));
        }
        if let Some(stage) = self.stage {
            lines.push(format!("stage: {}", stage.as_str()));
        }
        lines.push(format!("retryable: {}", self.retryable));
        if let Some(status) = self.http_status {
            lines.push(format!("http status: {status}"));
        }
        if let Some(code) = self.provider_code.as_deref() {
            lines.push(format!("provider code: {code}"));
        }
        if let Some(excerpt) = self.excerpt.as_deref() {
            lines.push(format!("excerpt: {excerpt}"));
        }
        if !self.debug_events.is_empty() {
            lines.push(format!("debug events: {}", self.debug_events.join(", ")));
        }

        formatter.write_str(&lines.join("\n"))
    }
}

/// Stable classification for agent and provider failures.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentErrorKind {
    ProviderUnavailable,
    ProviderNetwork,
    Authentication,
    ProviderResponseParse,
    ProviderMalformedResponse,
    ProviderStream,
    ContextLimit,
    Capability,
    ContentFilter,
    Validation,
    UnknownProvider,
}

impl AgentErrorKind {
    /// Returns the durable snake_case label for this error category.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ProviderUnavailable => "provider_unavailable",
            Self::ProviderNetwork => "provider_network",
            Self::Authentication => "authentication",
            Self::ProviderResponseParse => "provider_response_parse",
            Self::ProviderMalformedResponse => "provider_malformed_response",
            Self::ProviderStream => "provider_stream",
            Self::ContextLimit => "context_limit",
            Self::Capability => "capability",
            Self::ContentFilter => "content_filter",
            Self::Validation => "validation",
            Self::UnknownProvider => "unknown_provider",
        }
    }
}

/// Stage of request or response processing where an error occurred.
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

impl AgentErrorStage {
    /// Returns the durable snake_case label for this processing stage.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::RequestBuild => "request_build",
            Self::HttpRequest => "http_request",
            Self::HttpStatus => "http_status",
            Self::SseDecode => "sse_decode",
            Self::PayloadParse => "payload_parse",
            Self::ProviderStream => "provider_stream",
            Self::AgentValidation => "agent_validation",
        }
    }
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

struct DiagnosticsInput<'a> {
    kind: AgentErrorKind,
    provider: Option<String>,
    diagnostics: Option<&'a ProviderErrorDiagnostics>,
    retryable: bool,
}

struct DetailsInput<'a> {
    kind: AgentErrorKind,
    provider: Option<String>,
    stage: Option<AgentErrorStage>,
    retryable: bool,
    diagnostics: Option<&'a ProviderErrorDiagnostics>,
}

fn provider_error_kind(error: &ProviderError) -> Option<AgentErrorKind> {
    match error {
        ProviderError::NetworkError { .. } => Some(AgentErrorKind::ProviderNetwork),
        ProviderError::AuthenticationRequired { .. }
        | ProviderError::AuthenticationFailed { .. }
        | ProviderError::InvalidApiKey => Some(AgentErrorKind::Authentication),
        ProviderError::ResponseParsingFailed { .. } => Some(AgentErrorKind::ProviderResponseParse),
        ProviderError::MalformedResponse { .. } => Some(AgentErrorKind::ProviderMalformedResponse),
        ProviderError::StreamError { .. } => Some(AgentErrorKind::ProviderStream),
        ProviderError::ContextLimitExceeded { .. } => Some(AgentErrorKind::ContextLimit),
        ProviderError::CapabilityMismatch { .. } => Some(AgentErrorKind::Capability),
        ProviderError::UnsupportedProvider { .. } => Some(AgentErrorKind::UnknownProvider),
        ProviderError::UnsupportedValidationMode => Some(AgentErrorKind::Validation),
        ProviderError::ModelFetchFailed { .. }
        | ProviderError::NoModelsReturned { .. }
        | ProviderError::ProviderUnavailable { .. }
        | ProviderError::StreamUnavailable { .. } => Some(AgentErrorKind::ProviderUnavailable),
        ProviderError::CancellationError => None,
    }
}

fn default_stage_for_kind(kind: AgentErrorKind) -> Option<AgentErrorStage> {
    match kind {
        AgentErrorKind::ProviderUnavailable => Some(AgentErrorStage::HttpRequest),
        AgentErrorKind::ProviderNetwork => Some(AgentErrorStage::HttpRequest),
        AgentErrorKind::Authentication => Some(AgentErrorStage::AgentValidation),
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
        ProviderError::AuthenticationRequired { .. }
        | ProviderError::AuthenticationFailed { .. }
        | ProviderError::InvalidApiKey => false,
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
