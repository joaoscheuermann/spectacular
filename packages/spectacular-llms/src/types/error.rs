use std::error::Error;
use std::fmt::{self, Display};

const ERROR_EXCERPT_LIMIT: usize = 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ValidationMode {
    /// API-key validation against a provider-owned credential endpoint.
    ApiKey,
}

/// Provider pipeline stage that produced diagnostics for a failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProviderErrorStage {
    /// The provider-neutral request could not be converted to provider wire format.
    RequestBuild,
    /// The HTTP request failed before a provider response was received.
    HttpRequest,
    /// The provider returned a non-success HTTP status.
    HttpStatus,
    /// Server-sent event framing could not be decoded.
    SseDecode,
    /// A response payload could not be parsed into provider events.
    PayloadParse,
    /// A provider stream failed while reading or interpreting events.
    ProviderStream,
}

impl ProviderErrorStage {
    /// Returns the stable diagnostic label for this stage.
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

/// Structured details preserved for provider failures and debug surfaces.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ProviderErrorDiagnostics {
    /// Pipeline stage that produced the error when it is known.
    pub stage: Option<ProviderErrorStage>,
    /// HTTP status returned by the provider for non-success responses.
    pub http_status: Option<u16>,
    /// Provider-specific error code extracted from a response body or stream event.
    pub provider_code: Option<String>,
    /// Bounded, redacted response excerpt useful for diagnostics.
    pub excerpt: Option<String>,
    /// Debug-log event names that contain related raw provider context.
    pub debug_events: Vec<String>,
}

impl ProviderErrorDiagnostics {
    /// Starts diagnostics for a known provider stage.
    pub fn new(stage: ProviderErrorStage) -> Self {
        Self {
            stage: Some(stage),
            ..Self::default()
        }
    }

    /// Attaches an HTTP status to these diagnostics.
    pub fn with_http_status(mut self, status: u16) -> Self {
        self.http_status = Some(status);
        self
    }

    /// Attaches a non-empty provider-specific error code.
    pub fn with_provider_code(mut self, code: impl Into<String>) -> Self {
        let code = code.into();
        if !code.trim().is_empty() {
            self.provider_code = Some(code);
        }
        self
    }

    /// Extracts a provider error code from a JSON body when one is present.
    pub fn with_provider_code_from_body(self, body: &str) -> Self {
        let Some(code) = provider_code_from_json(body) else {
            return self;
        };

        self.with_provider_code(code)
    }

    /// Attaches a bounded, redacted excerpt from provider text.
    pub fn with_excerpt(mut self, value: impl AsRef<str>) -> Self {
        let excerpt = provider_error_excerpt(value.as_ref());
        if !excerpt.is_empty() {
            self.excerpt = Some(excerpt);
        }
        self
    }

    /// Records a debug-log event name that can provide more context.
    pub fn with_debug_event(mut self, event: impl Into<String>) -> Self {
        let event = event.into();
        if !event.trim().is_empty() {
            self.debug_events.push(event);
        }
        self
    }

    /// Moves these diagnostics behind a box for storage in compact error variants.
    pub fn boxed(self) -> Box<Self> {
        Box::new(self)
    }
}

/// Error contract returned by provider implementations.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProviderError {
    /// The caller cancelled the provider call.
    CancellationError,
    /// The supplied API key was rejected or empty.
    InvalidApiKey,
    /// Model discovery failed before a usable list was returned.
    ModelFetchFailed {
        /// Provider that failed model discovery.
        provider_name: String,
        /// Structured diagnostics from the model discovery request.
        diagnostics: Option<Box<ProviderErrorDiagnostics>>,
    },
    /// Model discovery succeeded but the provider returned no models.
    NoModelsReturned {
        /// Provider that returned an empty model list.
        provider_name: String,
    },
    /// The provider could not complete the request.
    ProviderUnavailable {
        /// Provider that was unavailable.
        provider_name: String,
        /// Structured diagnostics from the failed provider request.
        diagnostics: Option<Box<ProviderErrorDiagnostics>>,
    },
    /// The provider requires authentication before the request can run.
    AuthenticationRequired {
        /// Provider that needs authentication.
        provider_name: String,
    },
    /// Authentication failed with provider-specific detail.
    AuthenticationFailed {
        /// Provider that rejected authentication.
        provider_name: String,
        /// Human-readable rejection reason.
        reason: String,
        /// Structured diagnostics from the authentication failure.
        diagnostics: Option<Box<ProviderErrorDiagnostics>>,
    },
    /// Streaming is not implemented for the selected provider.
    StreamUnavailable {
        /// Provider without streaming support.
        provider_name: String,
    },
    /// The provider returned a response that violated the expected shape.
    MalformedResponse {
        /// Provider that returned malformed data.
        provider_name: String,
        /// Human-readable parse or validation reason.
        reason: String,
        /// Structured diagnostics from the malformed response.
        diagnostics: Option<Box<ProviderErrorDiagnostics>>,
    },
    /// Raw provider data could not be parsed.
    ResponseParsingFailed {
        /// Provider whose response could not be parsed.
        provider_name: String,
        /// Parser failure reason.
        reason: String,
        /// Structured diagnostics from the parse failure.
        diagnostics: Option<Box<ProviderErrorDiagnostics>>,
    },
    /// The provider emitted an error event inside a stream.
    StreamError {
        /// Provider that emitted the stream error.
        provider_name: String,
        /// Provider-specific stream error code when available.
        code: Option<String>,
        /// Provider-supplied stream error message.
        message: String,
        /// Structured diagnostics from the stream error.
        diagnostics: Option<Box<ProviderErrorDiagnostics>>,
    },
    /// Network I/O failed before the provider response could be handled.
    NetworkError {
        /// Provider whose network request failed.
        provider_name: String,
        /// Network-layer failure reason.
        reason: String,
        /// Structured diagnostics from the network failure.
        diagnostics: Option<Box<ProviderErrorDiagnostics>>,
    },
    /// The request exceeded a provider-owned context bound.
    ContextLimitExceeded {
        /// Provider enforcing the context limit.
        provider_name: String,
        /// Context-limit failure reason.
        reason: String,
    },
    /// The request required a provider capability that is not available.
    CapabilityMismatch {
        /// Provider missing the capability.
        provider_name: String,
        /// Capability required by the call.
        capability: String,
    },
    /// The requested provider identifier is not registered.
    UnsupportedProvider {
        /// Unknown provider identifier.
        provider_id: String,
    },
    /// The provider does not support the requested validation mode.
    UnsupportedValidationMode,
}

impl Display for ProviderError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ProviderError::CancellationError => formatter.write_str("provider call was cancelled"),
            ProviderError::InvalidApiKey => formatter.write_str("invalid API key"),
            ProviderError::ModelFetchFailed { provider_name, .. } => {
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
                ..
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
            } => write_stream_error(formatter, provider_name, code.as_deref(), message),
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
    /// Returns the provider name or identifier associated with this error.
    pub fn provider_name(&self) -> Option<&str> {
        match self {
            Self::ModelFetchFailed { provider_name, .. }
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

    /// Returns structured diagnostics attached to this error, if any.
    pub fn diagnostics(&self) -> Option<&ProviderErrorDiagnostics> {
        match self {
            Self::ModelFetchFailed { diagnostics, .. }
            | Self::ProviderUnavailable { diagnostics, .. }
            | Self::MalformedResponse { diagnostics, .. }
            | Self::ResponseParsingFailed { diagnostics, .. }
            | Self::StreamError { diagnostics, .. }
            | Self::NetworkError { diagnostics, .. }
            | Self::AuthenticationFailed { diagnostics, .. } => diagnostics.as_deref(),
            Self::CancellationError
            | Self::InvalidApiKey
            | Self::NoModelsReturned { .. }
            | Self::AuthenticationRequired { .. }
            | Self::StreamUnavailable { .. }
            | Self::ContextLimitExceeded { .. }
            | Self::CapabilityMismatch { .. }
            | Self::UnsupportedProvider { .. }
            | Self::UnsupportedValidationMode => None,
        }
    }

    /// Returns the HTTP status from diagnostics when present.
    pub fn http_status(&self) -> Option<u16> {
        self.diagnostics()
            .and_then(|diagnostics| diagnostics.http_status)
    }

    /// Returns the provider-specific code from stream data or diagnostics.
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

fn write_stream_error(
    formatter: &mut fmt::Formatter<'_>,
    provider_name: &str,
    code: Option<&str>,
    message: &str,
) -> fmt::Result {
    if let Some(code) = code {
        return write!(
            formatter,
            "{provider_name} stream returned error `{code}`: {message}"
        );
    }

    write!(
        formatter,
        "{provider_name} stream returned error: {message}"
    )
}

/// Produces a bounded, redacted provider response excerpt for diagnostics.
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

fn provider_code_from_json(body: &str) -> Option<String> {
    let value = serde_json::from_str::<serde_json::Value>(body).ok()?;
    let provider_code = [
        value.pointer("/error/code"),
        value.pointer("/error/type"),
        value.pointer("/code"),
        value.pointer("/type"),
    ]
    .into_iter()
    .flatten()
    .find_map(json_provider_code);

    provider_code
}

fn json_provider_code(value: &serde_json::Value) -> Option<String> {
    match value {
        serde_json::Value::Null => None,
        serde_json::Value::String(value) if value.trim().is_empty() => None,
        serde_json::Value::String(value) => Some(value.clone()),
        serde_json::Value::Number(value) => Some(value.to_string()),
        serde_json::Value::Bool(value) => Some(value.to_string()),
        serde_json::Value::Array(_) | serde_json::Value::Object(_) => Some(value.to_string()),
    }
}
