use super::dto::OpenRouterModelsResponse;
use crate::{Model, ProviderError};

const OPENROUTER_DEFAULT_CONTEXT_WINDOW_TOKENS: usize = 32_768;

/// Resolves OpenRouter model context windows when cache metadata is unavailable.
pub(crate) fn openrouter_context_window_tokens(model: &str) -> Option<usize> {
    if model.trim().is_empty() {
        return None;
    }

    Some(OPENROUTER_DEFAULT_CONTEXT_WINDOW_TOKENS)
}

/// Validates an OpenRouter API key using the supplied status request function.
pub(crate) fn validate_openrouter_api_key(
    api_key: &str,
    request_current_key: impl FnOnce(&str) -> Result<u16, ProviderError>,
) -> Result<(), ProviderError> {
    let api_key = api_key.trim();
    if api_key.is_empty() {
        return Err(ProviderError::InvalidApiKey);
    }

    match request_current_key(api_key)? {
        200 => Ok(()),
        401 | 403 => Err(ProviderError::InvalidApiKey),
        _ => Err(ProviderError::ProviderUnavailable {
            provider_name: "OpenRouter".to_owned(),
        }),
    }
}

/// Fetches and parses OpenRouter model metadata using the supplied request function.
pub(crate) fn fetch_openrouter_models(
    api_key: &str,
    request_models: impl FnOnce(&str) -> Result<(u16, String), ProviderError>,
) -> Result<Vec<Model>, ProviderError> {
    let api_key = api_key.trim();
    if api_key.is_empty() {
        return Err(ProviderError::InvalidApiKey);
    }

    let (status, body) = request_models(api_key)?;
    match status {
        200 => parse_openrouter_models(&body),
        401 | 403 => Err(ProviderError::InvalidApiKey),
        _ => Err(ProviderError::ModelFetchFailed {
            provider_name: "OpenRouter".to_owned(),
        }),
    }
}

/// Parses OpenRouter model metadata from the provider response body.
fn parse_openrouter_models(body: &str) -> Result<Vec<Model>, ProviderError> {
    let value: serde_json::Value =
        serde_json::from_str(body).map_err(|error| ProviderError::ResponseParsingFailed {
            provider_name: "OpenRouter".to_owned(),
            reason: error.to_string(),
        })?;
    let response: OpenRouterModelsResponse =
        serde_json::from_value(value).map_err(|error| ProviderError::MalformedResponse {
            provider_name: "OpenRouter".to_owned(),
            reason: error.to_string(),
        })?;
    let models = response
        .data
        .into_iter()
        .filter_map(|model| {
            let id = model.id.trim();
            if id.is_empty() {
                return None;
            }

            let display_name = model
                .name
                .as_deref()
                .map(str::trim)
                .filter(|name| !name.is_empty())
                .unwrap_or(id);

            Some(
                Model::with_supported_parameters(id, display_name, model.supported_parameters)
                    .with_context_window_tokens(model.context_length),
            )
        })
        .collect::<Vec<_>>();

    if models.is_empty() {
        return Err(ProviderError::NoModelsReturned {
            provider_name: "OpenRouter".to_owned(),
        });
    }

    Ok(models)
}
