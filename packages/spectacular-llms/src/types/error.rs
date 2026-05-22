use std::error::Error;
use std::fmt::{self, Display};

const ERROR_EXCERPT_LIMIT: usize = 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ValidationMode {
    ApiKey,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProviderErrorStage {
    RequestBuild,
    HttpRequest,
    HttpStatus,
    SseDecode,
    PayloadParse,
    ProviderStream,
}

impl ProviderErrorStage {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::RequestBuild => "request_build",
            Self::HttpRequest => "http_request",
            Self::HttpStatus => "http_status",
            Self::SseDecode => "sse_decode",
            Self::PayloadParse => "payload_parse",
            Self::ProviderStream => "provider_stream",
        }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ProviderErrorDiagnostics {
    pub stage: Option<ProviderErrorStage>,
    pub http_status: Option<u16>,
    pub provider_code: Option<String>,
    pub excerpt: Option<String>,
    pub debug_events: Vec<String>,
}

impl ProviderErrorDiagnostics {
    pub fn new(stage: ProviderErrorStage) -> Self {
        Self {
            stage: Some(stage),
            ..Self::default()
        }
    }

    pub fn with_http_status(mut self, status: u16) -> Self {
        self.http_status = Some(status);
        self
    }

    pub fn with_provider_code(mut self, code: impl Into<String>) -> Self {
        let code = code.into();
        if !code.trim().is_empty() {
            self.provider_code = Some(code);
        }
        self
    }

    pub fn with_excerpt(mut self, value: impl AsRef<str>) -> Self {
        let excerpt = provider_error_excerpt(value.as_ref());
        if !excerpt.is_empty() {
            self.excerpt = Some(excerpt);
        }
        self
    }

    pub fn with_debug_event(mut self, event: impl Into<String>) -> Self {
        let event = event.into();
        if !event.trim().is_empty() {
            self.debug_events.push(event);
        }
        self
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProviderError {
    CancellationError,
    InvalidApiKey,
    ModelFetchFailed {
        provider_name: String,
    },
    NoModelsReturned {
        provider_name: String,
    },
    ProviderUnavailable {
        provider_name: String,
        diagnostics: Option<ProviderErrorDiagnostics>,
    },
    AuthenticationRequired {
        provider_name: String,
    },
    AuthenticationFailed {
        provider_name: String,
        reason: String,
    },
    StreamUnavailable {
        provider_name: String,
    },
    MalformedResponse {
        provider_name: String,
        reason: String,
        diagnostics: Option<ProviderErrorDiagnostics>,
    },
    ResponseParsingFailed {
        provider_name: String,
        reason: String,
        diagnostics: Option<ProviderErrorDiagnostics>,
    },
    StreamError {
        provider_name: String,
        code: Option<String>,
        message: String,
        diagnostics: Option<ProviderErrorDiagnostics>,
    },
    NetworkError {
        provider_name: String,
        reason: String,
        diagnostics: Option<ProviderErrorDiagnostics>,
    },
    ContextLimitExceeded {
        provider_name: String,
        reason: String,
    },
    CapabilityMismatch {
        provider_name: String,
        capability: String,
    },
    UnsupportedProvider {
        provider_id: String,
    },
    UnsupportedValidationMode,
}

impl Display for ProviderError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ProviderError::CancellationError => formatter.write_str("provider call was cancelled"),
            ProviderError::InvalidApiKey => formatter.write_str("invalid API key"),
            ProviderError::ModelFetchFailed { provider_name } => {
                write!(formatter, "failed to fetch models from {provider_name}")
            }
            ProviderError::NoModelsReturned { provider_name } => {
                write!(formatter, "{provider_name} returned no models")
            }
            ProviderError::ProviderUnavailable { provider_name, .. } => {
                write!(formatter, "{provider_name} is unavailable")
            }
            ProviderError::AuthenticationRequired { provider_name } => {
                write!(formatter, "{provider_name} authentication is required")
            }
            ProviderError::AuthenticationFailed {
                provider_name,
                reason,
            } => write!(formatter, "{provider_name} authentication failed: {reason}"),
            ProviderError::StreamUnavailable { provider_name } => {
                write!(
                    formatter,
                    "{provider_name} streaming is not implemented yet"
                )
            }
            ProviderError::MalformedResponse {
                provider_name,
                reason,
                ..
            } => write!(
                formatter,
                "{provider_name} returned a malformed response: {reason}"
            ),
            ProviderError::ResponseParsingFailed {
                provider_name,
                reason,
                ..
            } => write!(
                formatter,
                "failed to parse {provider_name} response: {reason}"
            ),
            ProviderError::StreamError {
                provider_name,
                code,
                message,
                ..
            } => {
                if let Some(code) = code {
                    write!(
                        formatter,
                        "{provider_name} stream returned error `{code}`: {message}"
                    )
                } else {
                    write!(
                        formatter,
                        "{provider_name} stream returned error: {message}"
                    )
                }
            }
            ProviderError::NetworkError {
                provider_name,
                reason,
                ..
            } => write!(
                formatter,
                "{provider_name} network request failed: {reason}"
            ),
            ProviderError::ContextLimitExceeded {
                provider_name,
                reason,
            } => write!(
                formatter,
                "{provider_name} context limit exceeded: {reason}"
            ),
            ProviderError::CapabilityMismatch {
                provider_name,
                capability,
            } => write!(
                formatter,
                "{provider_name} does not support required capability `{capability}`"
            ),
            ProviderError::UnsupportedProvider { provider_id } => {
                write!(formatter, "provider `{provider_id}` is not supported")
            }
            ProviderError::UnsupportedValidationMode => {
                formatter.write_str("validation mode is not supported")
            }
        }
    }
}

impl Error for ProviderError {}

impl ProviderError {
    pub fn provider_name(&self) -> Option<&str> {
        match self {
            Self::ModelFetchFailed { provider_name }
            | Self::NoModelsReturned { provider_name }
            | Self::ProviderUnavailable { provider_name, .. }
            | Self::AuthenticationRequired { provider_name }
            | Self::AuthenticationFailed { provider_name, .. }
            | Self::StreamUnavailable { provider_name }
            | Self::MalformedResponse { provider_name, .. }
            | Self::ResponseParsingFailed { provider_name, .. }
            | Self::StreamError { provider_name, .. }
            | Self::NetworkError { provider_name, .. }
            | Self::ContextLimitExceeded { provider_name, .. }
            | Self::CapabilityMismatch { provider_name, .. } => Some(provider_name),
            Self::UnsupportedProvider { provider_id } => Some(provider_id),
            Self::CancellationError | Self::InvalidApiKey | Self::UnsupportedValidationMode => None,
        }
    }

    pub fn diagnostics(&self) -> Option<&ProviderErrorDiagnostics> {
        match self {
            Self::ProviderUnavailable { diagnostics, .. }
            | Self::MalformedResponse { diagnostics, .. }
            | Self::ResponseParsingFailed { diagnostics, .. }
            | Self::StreamError { diagnostics, .. }
            | Self::NetworkError { diagnostics, .. } => diagnostics.as_ref(),
            Self::CancellationError
            | Self::InvalidApiKey
            | Self::ModelFetchFailed { .. }
            | Self::NoModelsReturned { .. }
            | Self::AuthenticationRequired { .. }
            | Self::AuthenticationFailed { .. }
            | Self::StreamUnavailable { .. }
            | Self::ContextLimitExceeded { .. }
            | Self::CapabilityMismatch { .. }
            | Self::UnsupportedProvider { .. }
            | Self::UnsupportedValidationMode => None,
        }
    }

    pub fn http_status(&self) -> Option<u16> {
        self.diagnostics()
            .and_then(|diagnostics| diagnostics.http_status)
    }

    pub fn provider_code(&self) -> Option<&str> {
        match self {
            Self::StreamError {
                code: Some(code), ..
            } => Some(code),
            _ => self
                .diagnostics()
                .and_then(|diagnostics| diagnostics.provider_code.as_deref()),
        }
    }
}

pub fn provider_error_excerpt(value: &str) -> String {
    let stripped = value
        .chars()
        .filter(|character| !character.is_control())
        .collect::<String>();

    redact_api_key_like_tokens(&stripped)
        .chars()
        .take(ERROR_EXCERPT_LIMIT)
        .collect()
}

fn redact_api_key_like_tokens(value: &str) -> String {
    let chars = value.chars().collect::<Vec<_>>();
    let mut redacted = String::with_capacity(value.len());
    let mut index = 0;

    while index < chars.len() {
        if starts_with_api_key_prefix(&chars, index) {
            redacted.push_str("[redacted]");
            index += 3;
            while index < chars.len() && is_api_key_token_char(chars[index]) {
                index += 1;
            }
            continue;
        }

        redacted.push(chars[index]);
        index += 1;
    }

    redacted
}

fn starts_with_api_key_prefix(chars: &[char], index: usize) -> bool {
    chars.get(index) == Some(&'s')
        && chars.get(index + 1) == Some(&'k')
        && chars.get(index + 2) == Some(&'-')
}

fn is_api_key_token_char(character: char) -> bool {
    character.is_ascii_alphanumeric() || matches!(character, '-' | '_')
}
