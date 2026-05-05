use super::dto::OpenRouterChatRequest;
use crate::ProviderError;
use std::future::Future;

const OPENROUTER_API_KEY_URL: &str = "https://openrouter.ai/api/v1/key";
const OPENROUTER_MODELS_URL: &str = "https://openrouter.ai/api/v1/models";
const OPENROUTER_CHAT_COMPLETIONS_URL: &str = "https://openrouter.ai/api/v1/chat/completions";

#[derive(Clone)]
pub(crate) struct OpenRouterHttpClient {
    http: reqwest::Client,
}

impl OpenRouterHttpClient {
    pub(crate) fn new() -> Self {
        Self {
            http: reqwest::Client::new(),
        }
    }

    pub(crate) fn current_key_status(&self, api_key: &str) -> Result<u16, ProviderError> {
        let api_key = api_key.to_owned();
        run_sync_openrouter_http(async move {
            let response = reqwest::Client::new()
                .get(OPENROUTER_API_KEY_URL)
                .bearer_auth(api_key)
                .send()
                .await
                .map_err(|error| ProviderError::NetworkError {
                    provider_name: "OpenRouter".to_owned(),
                    reason: error.to_string(),
                })?;

            Ok(response.status().as_u16())
        })
    }

    pub(crate) fn models_response(&self, api_key: &str) -> Result<(u16, String), ProviderError> {
        let api_key = api_key.to_owned();
        run_sync_openrouter_http(async move {
            let response = reqwest::Client::new()
                .get(OPENROUTER_MODELS_URL)
                .bearer_auth(api_key)
                .send()
                .await
                .map_err(|error| ProviderError::NetworkError {
                    provider_name: "OpenRouter".to_owned(),
                    reason: error.to_string(),
                })?;
            let status = response.status().as_u16();
            let body = response
                .text()
                .await
                .map_err(|error| ProviderError::NetworkError {
                    provider_name: "OpenRouter".to_owned(),
                    reason: error.to_string(),
                })?;

            Ok((status, body))
        })
    }

    pub(crate) async fn stream_response(
        &self,
        api_key: &str,
        body: &OpenRouterChatRequest,
    ) -> Result<reqwest::Response, ProviderError> {
        self.http
            .post(OPENROUTER_CHAT_COMPLETIONS_URL)
            .bearer_auth(api_key)
            .header(reqwest::header::ACCEPT, "text/event-stream")
            .json(body)
            .send()
            .await
            .map_err(|error| ProviderError::NetworkError {
                provider_name: "OpenRouter".to_owned(),
                reason: error.to_string(),
            })
    }
}

fn run_sync_openrouter_http<T, F>(future: F) -> Result<T, ProviderError>
where
    T: Send + 'static,
    F: Future<Output = Result<T, ProviderError>> + Send + 'static,
{
    std::thread::spawn(move || {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|error| ProviderError::NetworkError {
                provider_name: "OpenRouter".to_owned(),
                reason: error.to_string(),
            })?;

        runtime.block_on(future)
    })
    .join()
    .map_err(|_| ProviderError::NetworkError {
        provider_name: "OpenRouter".to_owned(),
        reason: "OpenRouter HTTP worker panicked".to_owned(),
    })?
}
