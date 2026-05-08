mod auth;
mod client;
mod constants;
mod debug;
mod dto;
mod models;
mod parser;
mod sse;
mod stream;

use crate::{
    provider_by_id, Cancellation, LlmDebugLogger, LlmProvider, Model, ProviderCall,
    ProviderCapabilities, ProviderContextLimits, ProviderError, ProviderMetadata, ProviderRequest,
    ValidationMode, OPENAI_PROVIDER_ID,
};
pub use auth::{open_browser, OpenAiAuthRecord, OpenAiAuthStore, OpenAiBrowserAuthFlow};
use client::OpenAiHttpClient;
use models::openai_codex_models;
use std::sync::Arc;
use stream::openai_stream_completion;

pub struct OpenAiProvider {
    auth: OpenAiProviderAuth,
    client: OpenAiHttpClient,
    debug_logger: LlmDebugLogger,
}

#[derive(Clone)]
pub enum OpenAiProviderAuth {
    ApiKey(String),
    Oauth(Arc<dyn OpenAiAuthStore>),
}

impl OpenAiProvider {
    /// Creates an OpenAI provider with a caller-owned auth store.
    pub fn new(auth_store: Arc<dyn OpenAiAuthStore>) -> Self {
        Self::with_debug_logger(auth_store, LlmDebugLogger::disabled())
    }

    /// Creates an OpenAI provider backed by a public OpenAI API key.
    pub fn with_api_key(api_key: impl Into<String>) -> Self {
        Self::with_api_key_and_debug_logger(api_key, LlmDebugLogger::disabled())
    }

    /// Creates an OpenAI provider backed by a public OpenAI API key and debug logging.
    pub fn with_api_key_and_debug_logger(
        api_key: impl Into<String>,
        debug_logger: LlmDebugLogger,
    ) -> Self {
        Self::with_auth_and_debug_logger(OpenAiProviderAuth::ApiKey(api_key.into()), debug_logger)
    }

    /// Creates an OpenAI provider with a caller-owned auth store and debug logging.
    pub fn with_debug_logger(
        auth_store: Arc<dyn OpenAiAuthStore>,
        debug_logger: LlmDebugLogger,
    ) -> Self {
        Self::with_auth_and_debug_logger(OpenAiProviderAuth::Oauth(auth_store), debug_logger)
    }

    /// Creates an OpenAI provider with explicit auth mode and debug logging.
    fn with_auth_and_debug_logger(auth: OpenAiProviderAuth, debug_logger: LlmDebugLogger) -> Self {
        Self {
            auth,
            client: OpenAiHttpClient::new(),
            debug_logger,
        }
    }
}

impl LlmProvider for OpenAiProvider {
    /// Returns static OpenAI provider metadata from the registry.
    fn metadata(&self) -> ProviderMetadata {
        provider_by_id(OPENAI_PROVIDER_ID).expect("OpenAI metadata should be registered")
    }

    /// Validates the configured OpenAI auth mode before use.
    fn validate(&self, mode: ValidationMode, _value: &str) -> Result<(), ProviderError> {
        if mode != ValidationMode::ApiKey {
            return Err(ProviderError::UnsupportedValidationMode);
        }

        match &self.auth {
            OpenAiProviderAuth::ApiKey(api_key) if !api_key.trim().is_empty() => Ok(()),
            OpenAiProviderAuth::ApiKey(_) => Err(ProviderError::InvalidApiKey),
            OpenAiProviderAuth::Oauth(auth_store) => auth_store.load_openai_auth().map(|_| ()),
        }
    }

    /// Returns the built-in OpenAI Codex model list.
    fn models(&self, _api_key: &str) -> Result<Vec<Model>, ProviderError> {
        Ok(openai_codex_models())
    }

    /// Reports OpenAI provider feature support to the agent layer.
    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities {
            streaming: true,
            tool_calls: true,
            structured_output: false,
            reasoning: true,
            cancellation: false,
            usage_metadata: true,
            reasoning_metadata: false,
            context_limits: ProviderContextLimits::default(),
        }
    }

    /// Starts a streaming completion call using the configured OpenAI auth mode.
    fn stream_completion<'a>(
        &'a self,
        request: ProviderRequest,
        cancellation: Cancellation,
    ) -> ProviderCall<'a> {
        let auth = self.auth.clone();
        let client = self.client.clone();
        let debug_logger = self.debug_logger.clone();
        Box::pin(async move {
            openai_stream_completion(auth, client, debug_logger, request, cancellation).await
        })
    }
}

/// Starts the browser OAuth flow using the default OpenAI HTTP client.
pub fn start_openai_browser_auth() -> Result<OpenAiBrowserAuthFlow, ProviderError> {
    OpenAiBrowserAuthFlow::start(OpenAiHttpClient::new())
}
