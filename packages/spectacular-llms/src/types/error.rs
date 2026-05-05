use std::error::Error;
use std::fmt::{self, Display};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ValidationMode {
    ApiKey,
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
    },
    StreamUnavailable {
        provider_name: String,
    },
    MalformedResponse {
        provider_name: String,
        reason: String,
    },
    ResponseParsingFailed {
        provider_name: String,
        reason: String,
    },
    StreamError {
        provider_name: String,
        code: Option<String>,
        message: String,
    },
    NetworkError {
        provider_name: String,
        reason: String,
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
            ProviderError::ProviderUnavailable { provider_name } => {
                write!(formatter, "{provider_name} is unavailable")
            }
            ProviderError::StreamUnavailable { provider_name } => {
                write!(
                    formatter,
                    "{provider_name} streaming is not implemented yet"
                )
            }
            ProviderError::MalformedResponse {
                provider_name,
                reason,
            } => write!(
                formatter,
                "{provider_name} returned a malformed response: {reason}"
            ),
            ProviderError::ResponseParsingFailed {
                provider_name,
                reason,
            } => write!(
                formatter,
                "failed to parse {provider_name} response: {reason}"
            ),
            ProviderError::StreamError {
                provider_name,
                code,
                message,
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
