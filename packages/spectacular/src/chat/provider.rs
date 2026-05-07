use crate::chat::{ChatError, RuntimeSelection};
use spectacular_llms::{
    Cancellation, LlmDebugLogger, LlmProvider, Model, OpenRouterProvider, ProviderCall,
    ProviderCapabilities, ProviderError, ProviderMetadata, ProviderRequest, ValidationMode,
    OPENROUTER_PROVIDER_ID,
};

/// Chat-facing provider wrapper that keeps concrete LLM implementations behind a local seam.
pub enum ChatProvider {
    /// OpenRouter-backed provider used by the current chat runtime.
    OpenRouter(OpenRouterProvider),
}

impl LlmProvider for ChatProvider {
    /// Delegates static provider metadata to the wrapped provider implementation.
    fn metadata(&self) -> ProviderMetadata {
        match self {
            Self::OpenRouter(provider) => provider.metadata(),
        }
    }

    /// Delegates provider-specific value validation to the wrapped provider implementation.
    fn validate(&self, mode: ValidationMode, value: &str) -> Result<(), ProviderError> {
        match self {
            Self::OpenRouter(provider) => provider.validate(mode, value),
        }
    }

    /// Delegates model discovery to the wrapped provider implementation.
    fn models(&self, api_key: &str) -> Result<Vec<Model>, ProviderError> {
        match self {
            Self::OpenRouter(provider) => provider.models(api_key),
        }
    }

    /// Delegates capability reporting to the wrapped provider implementation.
    fn capabilities(&self) -> ProviderCapabilities {
        match self {
            Self::OpenRouter(provider) => provider.capabilities(),
        }
    }

    /// Delegates streaming completion calls to the wrapped provider implementation.
    fn stream_completion<'a>(
        &'a self,
        request: ProviderRequest,
        cancellation: Cancellation,
    ) -> ProviderCall<'a> {
        match self {
            Self::OpenRouter(provider) => provider.stream_completion(request, cancellation),
        }
    }
}

/// Builds the provider for an already-resolved chat runtime selection.
pub fn provider_for_runtime(
    runtime: &RuntimeSelection,
    debug_logger: LlmDebugLogger,
) -> Result<ChatProvider, ChatError> {
    if runtime.provider_type == OPENROUTER_PROVIDER_ID {
        return Ok(ChatProvider::OpenRouter(
            OpenRouterProvider::with_debug_logger(runtime.api_key.clone(), debug_logger),
        ));
    }

    Err(ChatError::Session(format!(
        "provider type `{}` is not supported by chat",
        runtime.provider_type
    )))
}

/// Builds the provider from raw config parts before a full runtime selection exists.
pub fn provider_for_parts(
    provider_type: &str,
    api_key: String,
    debug_logger: LlmDebugLogger,
) -> Result<ChatProvider, ChatError> {
    if provider_type == OPENROUTER_PROVIDER_ID {
        return Ok(ChatProvider::OpenRouter(
            OpenRouterProvider::with_debug_logger(api_key, debug_logger),
        ));
    }

    Err(ChatError::Session(format!(
        "provider type `{provider_type}` is not supported by chat"
    )))
}
