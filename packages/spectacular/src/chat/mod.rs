mod auth;
mod commands;
mod config_mutation;
mod controller;
mod model;
mod paste_burst;
mod prompt;
mod provider;
mod renderer;
mod runner;
mod session;
mod title;
#[allow(dead_code)]
mod tui_adapter;
mod tui_runtime;

use crate::chat::renderer::Renderer;
use crate::chat::runner::main_chat_tool_storage;
use crate::chat::session::{ChatEvent, SessionManager};
use controller::ChatController;
use model::ChatModel;
use spectacular_agent::ToolStorage;
use spectacular_commands::CommandError;
use spectacular_config::{
    CachedModelMetadata, ConfigError, ModelCache, ProviderAuthMode, ReasoningLevel,
    SpectacularConfig, TaskModelSlot,
};
use spectacular_llms::{LlmDebugLogger, LlmProvider, Model};
use std::error::Error;
use std::fmt::{self, Display};
use std::io;
use std::path::PathBuf;
use std::str::FromStr;
use std::time::{SystemTime, UNIX_EPOCH};

/// Runs the test command implementation and returns its command future.
pub async fn run(debug_logger: LlmDebugLogger, tui: bool) -> Result<(), ChatError> {
    if tui {
        return run_tui(debug_logger).await;
    }

    run_legacy(debug_logger).await
}

/// Runs the legacy terminal renderer chat loop.
async fn run_legacy(debug_logger: LlmDebugLogger) -> Result<(), ChatError> {
    let ChatBootstrap {
        session,
        renderer,
        runtime,
        tools,
        workspace_root,
        debug_logger,
        warnings,
    } = ChatBootstrap::new(debug_logger)?;
    let mut model = ChatModel::new_with_debug_logger(session, runtime, debug_logger);
    let started = model.start_new_session()?;
    renderer.clear_screen();
    renderer.session_created(&started.id, model.runtime(), &workspace_root);
    for warning in warnings {
        renderer.warning(&warning);
    }
    let mut controller = ChatController::new(
        model,
        commands::registry()?,
        renderer,
        tools,
        workspace_root,
    );
    controller.run_loop().await
}

/// Runs the experimental IOCraft TUI chat loop.
async fn run_tui(debug_logger: LlmDebugLogger) -> Result<(), ChatError> {
    tui_runtime::run_iocraft_tui(debug_logger).await
}

pub(crate) struct ChatBootstrap {
    session: SessionManager,
    renderer: Renderer,
    runtime: RuntimeSelection,
    tools: ToolStorage,
    workspace_root: PathBuf,
    debug_logger: LlmDebugLogger,
    warnings: Vec<String>,
}

impl ChatBootstrap {
    /// Creates a new value from the supplied inputs.
    pub(crate) fn new(debug_logger: LlmDebugLogger) -> Result<Self, ChatError> {
        let config = spectacular_config::read_config_or_default()?;
        let refresh = refresh_model_cache(&config, debug_logger.clone())?;
        let mut warnings = refresh.warnings;
        let runtime = match RuntimeSelection::from_config_and_cache(&config, &refresh.cache) {
            Ok(runtime) => runtime,
            Err(error) => {
                warnings.push(format!(
                    "configuration is incomplete ({error}); only setup commands are available"
                ));
                RuntimeSelection::setup()
            }
        };
        let workspace_root = std::env::current_dir().map_err(ChatError::Io)?;
        let trace_dir = spectacular_config::config_dir()?.join("tool-output");
        let tools = main_chat_tool_storage(workspace_root.clone(), trace_dir)
            .map_err(|error| ChatError::Session(error.to_string()))?;
        Ok(Self {
            session: SessionManager::new()?,
            renderer: Renderer::default(),
            runtime,
            tools,
            workspace_root,
            debug_logger,
            warnings,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeSelection {
    pub(crate) provider_type: String,
    pub(crate) provider_auth: Option<ProviderAuthMode>,
    pub(crate) provider: String,
    pub(crate) api_key: String,
    pub(crate) model_key: String,
    pub(crate) model: String,
    pub(crate) reasoning: ReasoningLevel,
    pub(crate) context_window_tokens: Option<usize>,
}

impl RuntimeSelection {
    /// Creates the setup-only runtime used when configuration is incomplete.
    pub(crate) fn setup() -> Self {
        Self {
            provider_type: "setup".to_owned(),
            provider_auth: None,
            provider: "setup".to_owned(),
            api_key: String::new(),
            model_key: "not configured".to_owned(),
            model: "not configured".to_owned(),
            reasoning: ReasoningLevel::None,
            context_window_tokens: None,
        }
    }

    /// Returns whether this runtime can execute normal prompts.
    pub(crate) fn is_ready(&self) -> bool {
        self.provider_type != "setup"
    }

    /// Creates a runtime selection using cached provider metadata for the selected model.
    pub(crate) fn from_config_and_cache(
        config: &SpectacularConfig,
        cache: &ModelCache,
    ) -> Result<Self, ChatError> {
        let (model_key, coding) = config.model_for_task(TaskModelSlot::Coding)?;
        let provider_config = config.provider_for_model(model_key)?;

        Ok(Self {
            provider_type: provider_config.provider_type.clone(),
            provider_auth: provider_config.auth_mode(),
            provider: coding.provider.clone(),
            api_key: provider_config.api_key().to_owned(),
            model_key: model_key.to_owned(),
            model: coding.model.clone(),
            reasoning: coding.reasoning,
            context_window_tokens: cached_context_window_tokens(
                cache,
                &coding.provider,
                &coding.model,
            ),
        })
    }

    /// Restores runtime selection from session records and cached provider metadata.
    fn from_session_records_and_cache(
        config: &SpectacularConfig,
        cache: &ModelCache,
        records: &[session::ChatRecord],
    ) -> Result<Option<Self>, ChatError> {
        let model = records
            .iter()
            .rev()
            .find_map(|record| match record.event()? {
                ChatEvent::ModelChanged {
                    slot,
                    provider,
                    model,
                    reasoning,
                    ..
                } if slot == TaskModelSlot::Coding.as_str()
                    && !provider.trim().is_empty()
                    && !model.trim().is_empty() =>
                {
                    Some((
                        provider.clone(),
                        model.clone(),
                        ReasoningLevel::from_str(reasoning).unwrap_or_default(),
                    ))
                }
                _ => None,
            });

        let Some((provider, model, reasoning)) = model else {
            return Ok(None);
        };

        let Some(provider_config) = config.providers.get(&provider) else {
            return Ok(None);
        };
        if !provider_config.has_credentials() {
            return Ok(None);
        }
        let model_key = config
            .models
            .iter()
            .find(|(_, candidate)| candidate.provider == provider && candidate.model == model)
            .map(|(key, _)| key.clone())
            .unwrap_or_else(|| spectacular_config::composite_model_key(&provider, &model));

        let context_window_tokens = cached_context_window_tokens(cache, &provider, &model);

        Ok(Some(Self {
            provider_type: provider_config.provider_type.clone(),
            provider_auth: provider_config.auth_mode(),
            provider,
            api_key: provider_config.api_key().to_owned(),
            model_key,
            model,
            reasoning,
            context_window_tokens,
        }))
    }
}

struct ModelCacheRefresh {
    cache: ModelCache,
    warnings: Vec<String>,
}

/// Refreshes provider model metadata and records non-fatal cache warnings.
fn refresh_model_cache(
    config: &SpectacularConfig,
    debug_logger: LlmDebugLogger,
) -> Result<ModelCacheRefresh, ChatError> {
    let mut cache = spectacular_config::read_model_cache_or_default()?;
    let mut changed = false;
    let mut warnings = Vec::new();
    let now = unix_timestamp();

    for (provider_name, provider_config) in &config.providers {
        if !provider_config.has_credentials() {
            continue;
        }

        let result = crate::chat::provider::provider_for_parts(
            &provider_config.provider_type,
            provider_config.api_key().to_owned(),
            debug_logger.clone(),
        )
        .and_then(|provider| {
            provider
                .models(provider_config.api_key())
                .map_err(|error| ChatError::Session(error.to_string()))
        });

        match result {
            Ok(models) => {
                cache.put_provider(
                    provider_name.clone(),
                    provider_config.provider_type.clone(),
                    now,
                    models.into_iter().map(cached_model_metadata),
                );
                changed = true;
            }
            Err(error) => {
                warnings.push(format_model_cache_warning(
                    provider_name,
                    cache.provider(provider_name),
                    &error,
                    now,
                ));
            }
        }
    }

    if changed {
        spectacular_config::write_model_cache(&cache)?;
    }

    Ok(ModelCacheRefresh { cache, warnings })
}

/// Converts provider model metadata into the persisted model-cache shape.
fn cached_model_metadata(model: Model) -> CachedModelMetadata {
    CachedModelMetadata::new(
        model.id().to_owned(),
        model.display_name().to_owned(),
        model.supported_parameters().iter().cloned(),
    )
    .with_context_window_tokens(model.context_window_tokens())
}

/// Looks up a cached context window for a provider/model pair.
fn cached_context_window_tokens(cache: &ModelCache, provider: &str, model: &str) -> Option<usize> {
    cache
        .model(provider, model)
        .and_then(|metadata| metadata.context_window_tokens)
}

/// Validates that cached provider metadata supports the requested reasoning level.
pub(crate) fn validate_cached_model_reasoning(
    cache: &ModelCache,
    provider: &str,
    model_id: &str,
    reasoning: ReasoningLevel,
) -> Result<(), ChatError> {
    let metadata = cache.model(provider, model_id).ok_or_else(|| {
        ChatError::Session(format!(
            "model `{model_id}` is not available in API metadata cache for provider `{provider}`"
        ))
    })?;

    if !reasoning.non_none() || metadata.supports_reasoning() {
        return Ok(());
    }

    Err(ChatError::Session(format!(
        "model `{model_id}` does not advertise `reasoning` in supported_parameters"
    )))
}

/// Formats a user-facing warning for model metadata refresh failures.
fn format_model_cache_warning(
    provider_name: &str,
    cached: Option<&spectacular_config::ProviderModelCache>,
    error: &ChatError,
    now: u64,
) -> String {
    let Some(cached) = cached else {
        return format!(
            "could not refresh model metadata for provider `{provider_name}` ({error}); dynamic autocomplete is unavailable"
        );
    };
    let age = now.saturating_sub(cached.fetched_at);
    if age > 24 * 60 * 60 {
        return format!(
            "could not refresh model metadata for provider `{provider_name}` ({error}); using stale cache from {} hours ago",
            age / 3600
        );
    }

    format!(
        "could not refresh model metadata for provider `{provider_name}` ({error}); using cached API metadata"
    )
}

/// Returns the current Unix timestamp in seconds.
fn unix_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[derive(Debug)]
pub enum ChatError {
    Config(ConfigError),
    Command(CommandError),
    Io(io::Error),
    Session(String),
    Exit,
}

impl From<ConfigError> for ChatError {
    /// Converts the source value into this error type.
    fn from(error: ConfigError) -> Self {
        Self::Config(error)
    }
}

impl From<CommandError> for ChatError {
    /// Converts the source value into this error type.
    fn from(error: CommandError) -> Self {
        Self::Command(error)
    }
}

impl Display for ChatError {
    /// Formats this value for user-facing display.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ChatError::Config(error) => write!(formatter, "{error}"),
            ChatError::Command(error) => write!(formatter, "{error}"),
            ChatError::Io(error) => write!(formatter, "{error}"),
            ChatError::Session(message) => formatter.write_str(message),
            ChatError::Exit => formatter.write_str("chat exited"),
        }
    }
}

impl Error for ChatError {}

#[cfg(test)]
mod tests {
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/unit/chat/runtime_selection.rs"
    ));
}
