use std::fmt;
use std::sync::Arc;

use agent::{AgentConfig, ContextPolicy};
use config::{
    ChatGptAuthConfig, DoricConfig, ModelCache, ProviderAuthMode, ReasoningLevel, TaskModelSlot,
};
use llms::{
    Cancellation, LlmDebugLogger, LlmProvider, Model, OpenAiAuthRecord, OpenAiAuthStore,
    OpenAiProvider, OpenRouterProvider, ProviderCall, ProviderCapabilities, ProviderError,
    ProviderMetadata, ProviderRequest, ValidationMode, OPENAI_PROVIDER_ID, OPENROUTER_PROVIDER_ID,
};

use crate::error::{WorkerError, WorkerResult};

/// Worker-local runtime selection resolved from persisted config and model cache.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkerRuntimeSelection {
    pub provider_type: String,
    pub provider_name: String,
    pub model_key: String,
    pub model: String,
    pub reasoning: ReasoningLevel,
    pub provider_auth: WorkerProviderAuth,
    pub context_window_tokens: Option<usize>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum WorkerProviderAuth {
    ApiKey { api_key: String },
    Oauth { provider_name: String },
}

/// Config persistence seam used by worker-owned OpenAI OAuth storage.
pub trait WorkerConfigIo: Send + Sync {
    fn read_config_or_default(&self) -> WorkerResult<DoricConfig>;

    fn write_config(&self, config: &DoricConfig) -> WorkerResult<()>;
}

/// Worker provider dependencies with debug logging disabled by default.
#[derive(Clone)]
pub struct WorkerProviderDeps {
    debug_logger: LlmDebugLogger,
    config_io: Arc<dyn WorkerConfigIo>,
}

impl WorkerProviderDeps {
    pub fn new(debug_logger: LlmDebugLogger, config_io: Arc<dyn WorkerConfigIo>) -> Self {
        Self {
            debug_logger,
            config_io,
        }
    }

    pub fn offline() -> Self {
        Self::default()
    }

    pub fn debug_log_path(&self) -> Option<&std::path::Path> {
        self.debug_logger.path()
    }
}

impl Default for WorkerProviderDeps {
    fn default() -> Self {
        Self {
            debug_logger: LlmDebugLogger::disabled(),
            config_io: Arc::new(ProductionConfigIo),
        }
    }
}

/// Worker-owned provider wrapper that hides concrete LLM implementations.
pub enum WorkerProvider {
    OpenRouter(OpenRouterProvider),
    OpenAi(OpenAiProvider),
}

impl fmt::Debug for WorkerProvider {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("WorkerProvider")
            .field(&self.metadata().id())
            .finish()
    }
}

impl LlmProvider for WorkerProvider {
    fn metadata(&self) -> ProviderMetadata {
        match self {
            Self::OpenRouter(provider) => provider.metadata(),
            Self::OpenAi(provider) => provider.metadata(),
        }
    }

    fn validate(&self, mode: ValidationMode, value: &str) -> Result<(), ProviderError> {
        match self {
            Self::OpenRouter(provider) => provider.validate(mode, value),
            Self::OpenAi(provider) => provider.validate(mode, value),
        }
    }

    fn models(&self, api_key: &str) -> Result<Vec<Model>, ProviderError> {
        match self {
            Self::OpenRouter(provider) => provider.models(api_key),
            Self::OpenAi(provider) => provider.models(api_key),
        }
    }

    fn capabilities(&self) -> ProviderCapabilities {
        match self {
            Self::OpenRouter(provider) => provider.capabilities(),
            Self::OpenAi(provider) => provider.capabilities(),
        }
    }

    fn context_window_tokens(&self, model: &str) -> Option<usize> {
        match self {
            Self::OpenRouter(provider) => provider.context_window_tokens(model),
            Self::OpenAi(provider) => provider.context_window_tokens(model),
        }
    }

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

/// OpenAI auth store backed by the worker config I/O seam.
pub struct WorkerOpenAiAuthStore {
    provider_name: String,
    config_io: Arc<dyn WorkerConfigIo>,
}

impl WorkerOpenAiAuthStore {
    pub fn new(provider_name: impl Into<String>, config_io: Arc<dyn WorkerConfigIo>) -> Self {
        Self {
            provider_name: provider_name.into(),
            config_io,
        }
    }

    pub fn load_openai_auth(&self) -> Result<OpenAiAuthRecord, ProviderError> {
        <Self as OpenAiAuthStore>::load_openai_auth(self)
    }

    pub fn save_openai_auth(&self, auth: OpenAiAuthRecord) -> Result<(), ProviderError> {
        <Self as OpenAiAuthStore>::save_openai_auth(self, auth)
    }
}

impl OpenAiAuthStore for WorkerOpenAiAuthStore {
    fn load_openai_auth(&self) -> Result<OpenAiAuthRecord, ProviderError> {
        let config = self
            .config_io
            .read_config_or_default()
            .map_err(auth_failed)?;
        let auth = config
            .providers
            .get(&self.provider_name)
            .and_then(|provider| provider.oauth_config())
            .filter(has_oauth_tokens)
            .ok_or_else(openai_auth_required)?;

        Ok(auth_config_to_record(auth))
    }

    fn save_openai_auth(&self, auth: OpenAiAuthRecord) -> Result<(), ProviderError> {
        let mut config = self
            .config_io
            .read_config_or_default()
            .map_err(auth_failed)?;
        let provider_type = config
            .providers
            .get(&self.provider_name)
            .map(|provider| provider.provider_type.clone())
            .unwrap_or_else(|| OPENAI_PROVIDER_ID.to_owned());

        config.providers.insert(
            self.provider_name.clone(),
            config::ProviderConfig::oauth(provider_type, auth_record_to_config(auth)),
        );
        self.config_io.write_config(&config).map_err(auth_failed)
    }
}

/// Selects the coding runtime from persisted config and cached model metadata.
pub fn select_runtime(
    config: &DoricConfig,
    cache: &ModelCache,
) -> WorkerResult<WorkerRuntimeSelection> {
    let (model_key, model) = config
        .model_for_task(TaskModelSlot::Coding)
        .map_err(WorkerError::from)?;
    let provider = config
        .provider_for_model(model_key)
        .map_err(WorkerError::from)?;
    let provider_auth = match provider.auth_mode() {
        Some(ProviderAuthMode::ApiKey) => WorkerProviderAuth::ApiKey {
            api_key: provider.api_key().to_owned(),
        },
        Some(ProviderAuthMode::Oauth) => WorkerProviderAuth::Oauth {
            provider_name: model.provider.clone(),
        },
        None => {
            return Err(WorkerError::provider_credentials(format!(
                "provider `{}` has no configured credentials",
                model.provider
            )));
        }
    };

    Ok(WorkerRuntimeSelection {
        provider_type: provider.provider_type.clone(),
        provider_name: model.provider.clone(),
        model_key: model_key.to_owned(),
        model: model.model.clone(),
        reasoning: model.reasoning,
        provider_auth,
        context_window_tokens: cache
            .model(&model.provider, &model.model)
            .and_then(|metadata| metadata.context_window_tokens),
    })
}

/// Builds agent runtime configuration from a selected worker runtime.
pub fn agent_config_for_runtime(
    runtime: &WorkerRuntimeSelection,
    system_prompt: impl Into<String>,
) -> AgentConfig {
    let context_policy = ContextPolicy {
        model_context_window_tokens: runtime.context_window_tokens,
        ..ContextPolicy::default()
    };

    AgentConfig {
        system_prompt: system_prompt.into(),
        model: Some(runtime.model.clone()),
        include_reasoning: runtime.reasoning.non_none(),
        reasoning_effort: runtime
            .reasoning
            .non_none()
            .then(|| runtime.reasoning.as_str().to_owned()),
        context_policy,
        ..AgentConfig::default()
    }
}

/// Constructs a concrete provider for a selected worker runtime without network calls.
pub fn provider_for_runtime(
    runtime: &WorkerRuntimeSelection,
    deps: WorkerProviderDeps,
) -> WorkerResult<WorkerProvider> {
    match runtime.provider_type.as_str() {
        OPENROUTER_PROVIDER_ID => openrouter_provider(runtime, deps.debug_logger),
        OPENAI_PROVIDER_ID => openai_provider(runtime, deps),
        provider_type => Err(WorkerError::provider_unsupported(format!(
            "provider type `{provider_type}` is not supported"
        ))),
    }
}

struct ProductionConfigIo;

impl WorkerConfigIo for ProductionConfigIo {
    fn read_config_or_default(&self) -> WorkerResult<DoricConfig> {
        config::read_config_or_default().map_err(WorkerError::from)
    }

    fn write_config(&self, config: &DoricConfig) -> WorkerResult<()> {
        config::write_config(config).map_err(WorkerError::from)
    }
}

impl From<config::ConfigError> for WorkerError {
    fn from(error: config::ConfigError) -> Self {
        match error {
            config::ConfigError::MissingProviderApiKey { .. } => {
                Self::provider_credentials(error.to_string())
            }
            config::ConfigError::MissingTaskModel { .. }
            | config::ConfigError::InvalidTaskModelReference { .. }
            | config::ConfigError::ModelNotConfigured { .. }
            | config::ConfigError::ModelProviderNotConfigured { .. }
            | config::ConfigError::ProviderNotConfigured { .. }
            | config::ConfigError::InvalidProviderType { .. } => {
                Self::provider_configuration(error.to_string())
            }
            _ => Self::provider_setup_failed(error.to_string()),
        }
    }
}

fn openrouter_provider(
    runtime: &WorkerRuntimeSelection,
    debug_logger: LlmDebugLogger,
) -> WorkerResult<WorkerProvider> {
    let WorkerProviderAuth::ApiKey { api_key } = &runtime.provider_auth else {
        return Err(WorkerError::provider_unsupported_auth(
            "OpenRouter supports API-key authentication for worker runs",
        ));
    };
    if api_key.trim().is_empty() {
        return Err(WorkerError::provider_credentials(
            "OpenRouter API-key credentials are missing",
        ));
    }

    Ok(WorkerProvider::OpenRouter(
        OpenRouterProvider::with_debug_logger(api_key.clone(), debug_logger),
    ))
}

fn openai_provider(
    runtime: &WorkerRuntimeSelection,
    deps: WorkerProviderDeps,
) -> WorkerResult<WorkerProvider> {
    match &runtime.provider_auth {
        WorkerProviderAuth::ApiKey { api_key } if !api_key.trim().is_empty() => {
            Ok(WorkerProvider::OpenAi(
                OpenAiProvider::with_api_key_and_debug_logger(api_key.clone(), deps.debug_logger),
            ))
        }
        WorkerProviderAuth::ApiKey { .. } => Err(WorkerError::provider_credentials(
            "OpenAI API-key credentials are missing",
        )),
        WorkerProviderAuth::Oauth { provider_name } => {
            Ok(WorkerProvider::OpenAi(OpenAiProvider::with_debug_logger(
                Arc::new(WorkerOpenAiAuthStore::new(
                    provider_name.clone(),
                    deps.config_io,
                )),
                deps.debug_logger,
            )))
        }
    }
}

fn has_oauth_tokens(auth: &ChatGptAuthConfig) -> bool {
    !auth.access_token.trim().is_empty() && !auth.refresh_token.trim().is_empty()
}

fn auth_config_to_record(auth: ChatGptAuthConfig) -> OpenAiAuthRecord {
    OpenAiAuthRecord {
        id_token: auth.id_token,
        access_token: auth.access_token,
        refresh_token: auth.refresh_token,
        account_id: auth.account_id,
        email: auth.email,
        plan_type: auth.plan_type,
        user_id: auth.user_id,
        fedramp: auth.fedramp,
        last_refresh_epoch_seconds: auth.last_refresh_epoch_seconds,
    }
}

fn auth_record_to_config(auth: OpenAiAuthRecord) -> ChatGptAuthConfig {
    ChatGptAuthConfig {
        id_token: auth.id_token,
        access_token: auth.access_token,
        refresh_token: auth.refresh_token,
        account_id: auth.account_id,
        email: auth.email,
        plan_type: auth.plan_type,
        user_id: auth.user_id,
        fedramp: auth.fedramp,
        last_refresh_epoch_seconds: auth.last_refresh_epoch_seconds,
    }
}

fn auth_failed(error: WorkerError) -> ProviderError {
    ProviderError::AuthenticationFailed {
        provider_name: "OpenAI".to_owned(),
        reason: error.lifecycle_failure_message(),
        diagnostics: None,
    }
}

fn openai_auth_required() -> ProviderError {
    ProviderError::AuthenticationRequired {
        provider_name: "OpenAI".to_owned(),
    }
}
