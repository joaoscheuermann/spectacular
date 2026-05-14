use crate::chat::auth::{EmptyOpenAiAuthStore, config_openai_auth_store};
use crate::chat::model::ChatConfigIo;
use crate::chat::{ChatError, RuntimeSelection};
use spectacular_config::ProviderAuthMode;
use spectacular_llms::{
    Cancellation, LlmDebugLogger, LlmProvider, Model, OPENAI_PROVIDER_ID, OPENROUTER_PROVIDER_ID,
    OpenAiProvider, OpenRouterProvider, ProviderCall, ProviderCapabilities, ProviderError,
    ProviderMetadata, ProviderRequest, ValidationMode,
};
use std::sync::Arc;

/// Chat-facing provider wrapper that keeps concrete LLM implementations behind a local seam.
pub enum ChatProvider {
    /// OpenRouter-backed provider used by the current chat runtime.
    OpenRouter(OpenRouterProvider),
    /// OpenAI Codex backend provider authenticated with ChatGPT OAuth.
    OpenAi(OpenAiProvider),
}

impl LlmProvider for ChatProvider {
    /// Delegates static provider metadata to the wrapped provider implementation.
    fn metadata(&self) -> ProviderMetadata {
        match self {
            Self::OpenRouter(provider) => provider.metadata(),
            Self::OpenAi(provider) => provider.metadata(),
        }
    }

    /// Delegates provider-specific value validation to the wrapped provider implementation.
    fn validate(&self, mode: ValidationMode, value: &str) -> Result<(), ProviderError> {
        match self {
            Self::OpenRouter(provider) => provider.validate(mode, value),
            Self::OpenAi(provider) => provider.validate(mode, value),
        }
    }

    /// Delegates model discovery to the wrapped provider implementation.
    fn models(&self, api_key: &str) -> Result<Vec<Model>, ProviderError> {
        match self {
            Self::OpenRouter(provider) => provider.models(api_key),
            Self::OpenAi(provider) => provider.models(api_key),
        }
    }

    /// Delegates capability reporting to the wrapped provider implementation.
    fn capabilities(&self) -> ProviderCapabilities {
        match self {
            Self::OpenRouter(provider) => provider.capabilities(),
            Self::OpenAi(provider) => provider.capabilities(),
        }
    }

    /// Delegates provider-owned model context-window lookup to the wrapped provider.
    fn context_window_tokens(&self, model: &str) -> Option<usize> {
        match self {
            Self::OpenRouter(provider) => provider.context_window_tokens(model),
            Self::OpenAi(provider) => provider.context_window_tokens(model),
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
            Self::OpenAi(provider) => provider.stream_completion(request, cancellation),
        }
    }
}

/// Builds the provider for an already-resolved chat runtime selection.
pub fn provider_for_runtime(
    runtime: &RuntimeSelection,
    debug_logger: LlmDebugLogger,
    config_io: ChatConfigIo,
) -> Result<ChatProvider, ChatError> {
    if runtime.provider_type == OPENROUTER_PROVIDER_ID {
        return Ok(ChatProvider::OpenRouter(
            OpenRouterProvider::with_debug_logger(runtime.api_key.clone(), debug_logger),
        ));
    }
    if runtime.provider_type == OPENAI_PROVIDER_ID {
        if runtime.provider_auth == Some(ProviderAuthMode::ApiKey) {
            return Ok(ChatProvider::OpenAi(
                OpenAiProvider::with_api_key_and_debug_logger(
                    runtime.api_key.clone(),
                    debug_logger,
                ),
            ));
        }

        return Ok(ChatProvider::OpenAi(OpenAiProvider::with_debug_logger(
            Arc::new(config_openai_auth_store(
                runtime.provider.clone(),
                config_io,
            )),
            debug_logger,
        )));
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
    if provider_type == OPENAI_PROVIDER_ID {
        if !api_key.trim().is_empty() {
            return Ok(ChatProvider::OpenAi(
                OpenAiProvider::with_api_key_and_debug_logger(api_key, debug_logger),
            ));
        }

        return Ok(ChatProvider::OpenAi(OpenAiProvider::with_debug_logger(
            Arc::new(EmptyOpenAiAuthStore),
            debug_logger,
        )));
    }

    Err(ChatError::Session(format!(
        "provider type `{provider_type}` is not supported by chat"
    )))
}
