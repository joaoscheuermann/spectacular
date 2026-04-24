use serde::Deserialize;
use std::error::Error;
use std::fmt::{self, Display};

pub const OPENROUTER_PROVIDER_ID: &str = "openrouter";
pub const OPENAI_PROVIDER_ID: &str = "openai";
pub const GOOGLE_GEMINI_PROVIDER_ID: &str = "google-gemini";

const OPENROUTER_API_KEY_URL: &str = "https://openrouter.ai/api/v1/key";
const OPENROUTER_MODELS_URL: &str = "https://openrouter.ai/api/v1/models";

static PROVIDERS: &[ProviderMetadata] = &[
    ProviderMetadata::enabled(OPENROUTER_PROVIDER_ID, "OpenRouter"),
    ProviderMetadata::disabled(OPENAI_PROVIDER_ID, "OpenAI"),
    ProviderMetadata::disabled(GOOGLE_GEMINI_PROVIDER_ID, "Google - Gemini"),
];

/// Static provider metadata used by the CLI setup flow.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProviderMetadata {
    id: &'static str,
    display_name: &'static str,
    enabled: bool,
}

impl ProviderMetadata {
    const fn enabled(id: &'static str, display_name: &'static str) -> Self {
        Self {
            id,
            display_name,
            enabled: true,
        }
    }

    const fn disabled(id: &'static str, display_name: &'static str) -> Self {
        Self {
            id,
            display_name,
            enabled: false,
        }
    }

    /// Stable identifier persisted in configuration.
    pub fn id(self) -> &'static str {
        self.id
    }

    /// Human-readable provider name shown in setup screens.
    pub fn display_name(self) -> &'static str {
        self.display_name
    }

    /// Whether the provider can be selected in the current release.
    pub fn is_enabled(self) -> bool {
        self.enabled
    }
}

/// Returns all providers visible in the setup UI.
pub fn provider_registry() -> &'static [ProviderMetadata] {
    PROVIDERS
}

/// Looks up provider metadata by stable identifier.
pub fn provider_by_id(provider_id: &str) -> Option<ProviderMetadata> {
    PROVIDERS
        .iter()
        .copied()
        .find(|provider| provider.id == provider_id)
}

/// Returns the only enabled provider name for placeholder setup routes.
pub fn enabled_provider_name() -> &'static str {
    PROVIDERS
        .iter()
        .find(|provider| provider.enabled)
        .map(|provider| provider.display_name)
        .unwrap_or("None")
}

/// Provider capability used by setup flows to validate credentials.
pub trait LlmProvider {
    fn metadata(&self) -> ProviderMetadata;

    fn validate(&self, mode: ValidationMode, value: &str) -> Result<(), ProviderError>;

    fn models(&self, api_key: &str) -> Result<Vec<Model>, ProviderError>;

    fn stream(
        &self,
        api_key: &str,
        history: &[Message],
        prompt: &Message,
    ) -> Result<ProviderStream, ProviderError>;
}

/// Model metadata exposed by a provider.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Model {
    id: String,
    display_name: String,
}

impl Model {
    pub fn new(id: impl Into<String>, display_name: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            display_name: display_name.into(),
        }
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn display_name(&self) -> &str {
        &self.display_name
    }
}

/// Chat message used by the future streaming provider seam.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Message {
    pub role: MessageRole,
    pub content: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MessageRole {
    System,
    User,
    Assistant,
}

/// Incremental content returned by future provider streams.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MessageDelta {
    pub content: String,
}

pub type ProviderStream = Box<dyn Iterator<Item = Result<MessageDelta, ProviderError>> + Send>;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ValidationMode {
    ApiKey,
}

#[derive(Debug)]
pub enum ProviderError {
    InvalidApiKey,
    ModelFetchFailed { provider_name: String },
    NoModelsReturned { provider_name: String },
    ProviderUnavailable { provider_name: String },
    StreamUnavailable { provider_name: String },
    UnsupportedProvider { provider_id: String },
    UnsupportedValidationMode,
}

impl Display for ProviderError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
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

/// Validates a provider value using the enabled provider implementation.
pub fn validate_provider_value(
    provider_id: &str,
    mode: ValidationMode,
    value: &str,
) -> Result<(), ProviderError> {
    match provider_id {
        OPENROUTER_PROVIDER_ID => OpenRouterProvider::default().validate(mode, value),
        _ => Err(ProviderError::UnsupportedProvider {
            provider_id: provider_id.to_owned(),
        }),
    }
}

/// Fetches models for an enabled provider.
pub fn fetch_provider_models(
    provider_id: &str,
    api_key: &str,
) -> Result<Vec<Model>, ProviderError> {
    match provider_id {
        OPENROUTER_PROVIDER_ID => OpenRouterProvider::default().models(api_key),
        _ => Err(ProviderError::UnsupportedProvider {
            provider_id: provider_id.to_owned(),
        }),
    }
}

#[derive(Default)]
pub struct OpenRouterProvider {
    client: OpenRouterHttpClient,
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

    fn stream(
        &self,
        _api_key: &str,
        _history: &[Message],
        _prompt: &Message,
    ) -> Result<ProviderStream, ProviderError> {
        Err(ProviderError::StreamUnavailable {
            provider_name: "OpenRouter".to_owned(),
        })
    }
}

#[derive(Default)]
struct OpenRouterHttpClient;

impl OpenRouterHttpClient {
    fn current_key_status(&self, api_key: &str) -> Result<u16, ProviderError> {
        let response = reqwest::blocking::Client::new()
            .get(OPENROUTER_API_KEY_URL)
            .bearer_auth(api_key)
            .send()
            .map_err(|_| ProviderError::ProviderUnavailable {
                provider_name: "OpenRouter".to_owned(),
            })?;

        Ok(response.status().as_u16())
    }

    fn models_response(&self, api_key: &str) -> Result<(u16, String), ProviderError> {
        let response = reqwest::blocking::Client::new()
            .get(OPENROUTER_MODELS_URL)
            .bearer_auth(api_key)
            .send()
            .map_err(|_| ProviderError::ModelFetchFailed {
                provider_name: "OpenRouter".to_owned(),
            })?;
        let status = response.status().as_u16();
        let body = response
            .text()
            .map_err(|_| ProviderError::ModelFetchFailed {
                provider_name: "OpenRouter".to_owned(),
            })?;

        Ok((status, body))
    }
}

fn validate_openrouter_api_key(
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

fn fetch_openrouter_models(
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

fn parse_openrouter_models(body: &str) -> Result<Vec<Model>, ProviderError> {
    let response: OpenRouterModelsResponse =
        serde_json::from_str(body).map_err(|_| ProviderError::ModelFetchFailed {
            provider_name: "OpenRouter".to_owned(),
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

            Some(Model::new(id, display_name))
        })
        .collect::<Vec<_>>();

    if models.is_empty() {
        return Err(ProviderError::NoModelsReturned {
            provider_name: "OpenRouter".to_owned(),
        });
    }

    Ok(models)
}

#[derive(Deserialize)]
struct OpenRouterModelsResponse {
    data: Vec<OpenRouterModelResponse>,
}

#[derive(Deserialize)]
struct OpenRouterModelResponse {
    id: String,
    name: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enabled_provider_is_openrouter() {
        assert_eq!(enabled_provider_name(), "OpenRouter");
    }

    #[test]
    fn registry_contains_enabled_openrouter_and_disabled_placeholders() {
        let providers = provider_registry();

        assert_eq!(providers[0].id(), OPENROUTER_PROVIDER_ID);
        assert!(providers[0].is_enabled());
        assert_eq!(providers[1].id(), OPENAI_PROVIDER_ID);
        assert!(!providers[1].is_enabled());
        assert_eq!(providers[2].id(), GOOGLE_GEMINI_PROVIDER_ID);
        assert!(!providers[2].is_enabled());
    }

    #[test]
    fn provider_lookup_uses_stable_ids() {
        let provider = provider_by_id(OPENROUTER_PROVIDER_ID).unwrap();

        assert_eq!(provider.display_name(), "OpenRouter");
    }

    #[test]
    fn openrouter_validation_rejects_blank_key_without_http_call() {
        let error =
            validate_openrouter_api_key("   ", |_| panic!("HTTP must not be called")).unwrap_err();

        assert!(matches!(error, ProviderError::InvalidApiKey));
    }

    #[test]
    fn openrouter_validation_accepts_success_status() {
        let result = validate_openrouter_api_key("sk-or-v1-valid", |_| Ok(200));

        assert!(result.is_ok());
    }

    #[test]
    fn openrouter_validation_rejects_unauthorized_status() {
        let error = validate_openrouter_api_key("sk-or-v1-invalid", |_| Ok(401)).unwrap_err();

        assert!(matches!(error, ProviderError::InvalidApiKey));
    }

    #[test]
    fn openrouter_validation_maps_other_statuses_to_unavailable() {
        let error = validate_openrouter_api_key("sk-or-v1-valid", |_| Ok(500)).unwrap_err();

        assert!(matches!(error, ProviderError::ProviderUnavailable { .. }));
    }

    #[test]
    fn provider_value_validation_rejects_unsupported_provider() {
        let error =
            validate_provider_value("openai", ValidationMode::ApiKey, "sk-test").unwrap_err();

        assert!(matches!(error, ProviderError::UnsupportedProvider { .. }));
    }

    #[test]
    fn openrouter_model_fetch_rejects_blank_key_without_http_call() {
        let error =
            fetch_openrouter_models("   ", |_| panic!("HTTP must not be called")).unwrap_err();

        assert!(matches!(error, ProviderError::InvalidApiKey));
    }

    #[test]
    fn openrouter_model_fetch_parses_model_ids_and_names() {
        let models = fetch_openrouter_models("sk-or-v1-valid", |_| {
            Ok((
                200,
                r#"{
                    "data": [
                        {"id": "openai/gpt-4o", "name": "GPT-4o"},
                        {"id": "anthropic/claude-sonnet-4.5", "name": null}
                    ]
                }"#
                .to_owned(),
            ))
        })
        .unwrap();

        assert_eq!(models[0], Model::new("openai/gpt-4o", "GPT-4o"));
        assert_eq!(
            models[1],
            Model::new("anthropic/claude-sonnet-4.5", "anthropic/claude-sonnet-4.5")
        );
    }

    #[test]
    fn openrouter_model_fetch_rejects_unauthorized_status() {
        let error = fetch_openrouter_models("sk-or-v1-invalid", |_| Ok((401, "{}".to_owned())))
            .unwrap_err();

        assert!(matches!(error, ProviderError::InvalidApiKey));
    }

    #[test]
    fn openrouter_model_fetch_rejects_empty_model_list() {
        let error =
            fetch_openrouter_models("sk-or-v1-valid", |_| Ok((200, r#"{"data":[]}"#.to_owned())))
                .unwrap_err();

        assert!(matches!(error, ProviderError::NoModelsReturned { .. }));
    }

    #[test]
    fn provider_model_fetch_rejects_unsupported_provider() {
        let error = fetch_provider_models("openai", "sk-test").unwrap_err();

        assert!(matches!(error, ProviderError::UnsupportedProvider { .. }));
    }

    #[test]
    fn openrouter_exposes_stream_seam() {
        let provider = OpenRouterProvider::default();
        let prompt = Message {
            role: MessageRole::User,
            content: "hello".to_owned(),
        };
        let error = match provider.stream("sk-or-v1-test", &[], &prompt) {
            Ok(_) => panic!("stream should not be implemented yet"),
            Err(error) => error,
        };

        assert!(matches!(error, ProviderError::StreamUnavailable { .. }));
    }
}
