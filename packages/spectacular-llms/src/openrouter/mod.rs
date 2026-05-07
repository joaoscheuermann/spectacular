mod client;
mod debug;
mod dto;
mod models;
mod parser;
mod sse;
mod stream;

use crate::{
    provider_by_id, Cancellation, LlmDebugLogger, LlmProvider, Model, ProviderCall,
    ProviderCapabilities, ProviderContextLimits, ProviderError, ProviderMetadata, ProviderRequest,
    ValidationMode, OPENROUTER_PROVIDER_ID,
};
use client::OpenRouterHttpClient;
use models::{fetch_openrouter_models, validate_openrouter_api_key};
use serde_json::json;
use stream::openrouter_stream_completion;

pub struct OpenRouterProvider {
    client: OpenRouterHttpClient,
    api_key: String,
    debug_logger: LlmDebugLogger,
}

impl OpenRouterProvider {
    pub fn new(api_key: String) -> Self {
        Self::with_debug_logger(api_key, LlmDebugLogger::disabled())
    }

    pub fn with_debug_logger(api_key: String, debug_logger: LlmDebugLogger) -> Self {
        Self::with_client_and_debug_logger(api_key, OpenRouterHttpClient::new(), debug_logger)
    }

    pub(crate) fn with_client_and_debug_logger(
        api_key: String,
        client: OpenRouterHttpClient,
        debug_logger: LlmDebugLogger,
    ) -> Self {
        Self {
            client,
            api_key,
            debug_logger,
        }
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

        let logger = self.debug_logger.clone();
        let result = validate_openrouter_api_key(value, |api_key| {
            debug::log_event(&logger, "api_key_validation_request", json!({}));
            let status = self.client.current_key_status(api_key)?;
            debug::log_event(
                &logger,
                "api_key_validation_response_status",
                json!({ "status": status }),
            );
            Ok(status)
        });
        if let Err(error) = &result {
            debug::log_error(&self.debug_logger, "api_key_validation_error", error);
        }

        result
    }

    fn models(&self, api_key: &str) -> Result<Vec<Model>, ProviderError> {
        let logger = self.debug_logger.clone();
        let result = fetch_openrouter_models(api_key, |api_key| {
            debug::log_event(&logger, "models_request", json!({}));
            let (status, body) = self.client.models_response(api_key)?;
            debug::log_event(
                &logger,
                "models_response_status",
                json!({ "status": status }),
            );
            debug::log_raw_text(&logger, "models_response_body", &body);
            Ok((status, body))
        });
        if let Err(error) = &result {
            debug::log_error(&self.debug_logger, "models_error", error);
        }

        result
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
        let debug_logger = self.debug_logger.clone();
        Box::pin(async move {
            openrouter_stream_completion(api_key, client, debug_logger, request, cancellation).await
        })
    }
}
