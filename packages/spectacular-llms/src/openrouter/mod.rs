mod client;
mod dto;
mod sse;
mod stream;

use crate::{
    provider_by_id, Cancellation, LlmProvider, Model, ProviderCall, ProviderCapabilities,
    ProviderContextLimits, ProviderError, ProviderMetadata, ProviderRequest, ValidationMode,
    OPENROUTER_PROVIDER_ID,
};
use client::OpenRouterHttpClient;
use stream::{fetch_openrouter_models, openrouter_stream_completion, validate_openrouter_api_key};

pub struct OpenRouterProvider {
    client: OpenRouterHttpClient,
    api_key: String,
}

impl OpenRouterProvider {
    pub fn new(api_key: String) -> Self {
        Self::with_client(api_key, OpenRouterHttpClient::new())
    }

    pub(crate) fn with_client(api_key: String, client: OpenRouterHttpClient) -> Self {
        Self { client, api_key }
    }
}

impl LlmProvider for OpenRouterProvider {
    fn metadata(&self) -> ProviderMetadata {
        provider_by_id(OPENROUTER_PROVIDER_ID).expect("OpenRouter metadata should be registered")
    }

    fn validate(&self, mode: ValidationMode, value: &str) -> Result<(), ProviderError> {
        if mode != ValidationMode::ApiKey {
            return Err(ProviderError::UnsupportedValidationMode);
        }

        validate_openrouter_api_key(value, |api_key| self.client.current_key_status(api_key))
    }

    fn models(&self, api_key: &str) -> Result<Vec<Model>, ProviderError> {
        fetch_openrouter_models(api_key, |api_key| self.client.models_response(api_key))
    }

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

    fn stream_completion<'a>(
        &'a self,
        request: ProviderRequest,
        cancellation: Cancellation,
    ) -> ProviderCall<'a> {
        let api_key = self.api_key.clone();
        let client = self.client.clone();
        Box::pin(async move {
            openrouter_stream_completion(api_key, client, request, cancellation).await
        })
    }
}
