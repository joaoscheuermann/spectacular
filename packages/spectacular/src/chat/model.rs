use super::RuntimeSelection;
use crate::chat::command_event::CommandEvent;
use crate::chat::commands::CompletionEnvironment;
use crate::chat::session::{ChatRecord, HistoryQuery, HistorySummary, SessionManager};
use crate::chat::ChatError;
use spectacular_agent::{AgentEvent, ContextTokenUsage};
use spectacular_config::{
    ConfigError, ModelCache, ModelConfig, ProviderAuthMode, ProviderConfig, SpectacularConfig,
    TaskAssignments, TaskModelSlot,
};
use spectacular_llms::LlmDebugLogger;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};

/// State model for chat sessions, runtime selection, and config-backed commands.
pub struct ChatModel {
    session: SessionManager,
    runtime: RuntimeSelection,
    debug_logger: LlmDebugLogger,
    config_io: ChatConfigIo,
    context_token_usage: Mutex<Option<ContextTokenUsage>>,
}

/// Injected config I/O operations used by chat commands that mutate persisted config.
#[derive(Clone, Copy)]
pub(crate) struct ChatConfigIo {
    read_config_or_default: fn() -> Result<SpectacularConfig, ConfigError>,
    read_model_cache_or_default: fn() -> Result<ModelCache, ConfigError>,
    backup_config: fn() -> Result<Option<PathBuf>, ConfigError>,
    write_config: fn(&SpectacularConfig) -> Result<(), ConfigError>,
    write_model_cache: fn(&ModelCache) -> Result<(), ConfigError>,
}

impl ChatConfigIo {
    /// Reads the current config, returning a default config when no persisted file exists.
    pub(crate) fn read_config_or_default(&self) -> Result<SpectacularConfig, ConfigError> {
        (self.read_config_or_default)()
    }

    /// Reads the cached provider model metadata, returning an empty cache when absent.
    pub(crate) fn read_model_cache_or_default(&self) -> Result<ModelCache, ConfigError> {
        (self.read_model_cache_or_default)()
    }

    /// Creates a backup of the persisted config before destructive mutations.
    pub(crate) fn backup_config(&self) -> Result<Option<PathBuf>, ConfigError> {
        (self.backup_config)()
    }

    /// Writes the updated config through the configured persistence function.
    pub(crate) fn write_config(&self, config: &SpectacularConfig) -> Result<(), ConfigError> {
        (self.write_config)(config)
    }

    /// Writes the provider model metadata cache through the configured persistence function.
    pub(crate) fn write_model_cache(&self, cache: &ModelCache) -> Result<(), ConfigError> {
        (self.write_model_cache)(cache)
    }
}

impl Default for ChatConfigIo {
    /// Wires config I/O to the production `spectacular_config` persistence functions.
    fn default() -> Self {
        Self {
            read_config_or_default: spectacular_config::read_config_or_default,
            read_model_cache_or_default: spectacular_config::read_model_cache_or_default,
            backup_config: spectacular_config::backup_config,
            write_config: spectacular_config::write_config,
            write_model_cache: spectacular_config::write_model_cache,
        }
    }
}

impl ChatModel {
    #[cfg(test)]
    /// Creates a chat model with logging disabled for focused unit tests.
    pub fn new(session: SessionManager, runtime: RuntimeSelection) -> Self {
        Self::new_with_debug_logger(session, runtime, LlmDebugLogger::disabled())
    }

    /// Creates a chat model with production config I/O and an injected debug logger.
    pub fn new_with_debug_logger(
        session: SessionManager,
        runtime: RuntimeSelection,
        debug_logger: LlmDebugLogger,
    ) -> Self {
        Self::new_with_config_io(session, runtime, debug_logger, ChatConfigIo::default())
    }

    /// Creates a chat model with fully injected config I/O for tests and alternate callers.
    pub(crate) fn new_with_config_io(
        session: SessionManager,
        runtime: RuntimeSelection,
        debug_logger: LlmDebugLogger,
        config_io: ChatConfigIo,
    ) -> Self {
        Self {
            session,
            runtime,
            debug_logger,
            config_io,
            context_token_usage: Mutex::new(None),
        }
    }

    /// Starts a new persisted chat session using the current runtime selection.
    pub fn start_new_session(&mut self) -> Result<SessionStartedModel, ChatError> {
        self.session.create(self.runtime.clone())?;
        self.clear_context_token_usage();
        Ok(SessionStartedModel {
            id: self.session.current_id().to_owned(),
        })
    }

    /// Returns the active session identifier.
    pub fn current_session_id(&self) -> &str {
        self.session.current_id()
    }

    /// Loads the records for the active chat session.
    pub fn records(&self) -> Result<Vec<ChatRecord>, ChatError> {
        self.session.records()
    }

    /// Returns a display-ready history page for the requested history query.
    pub fn history(&self, query: HistoryQuery) -> Result<HistoryTableModel, ChatError> {
        let page = self.session.history(query)?;
        Ok(HistoryTableModel {
            rows: page
                .sessions
                .into_iter()
                .map(HistoryRowModel::from)
                .collect(),
            remaining: page.remaining,
        })
    }

    /// Resumes a matching session and restores runtime metadata from its records.
    pub fn resume_session(&mut self, prefix: &str) -> Result<ResumeResultModel, ChatError> {
        let records = self.session.resume(prefix)?;
        self.restore_runtime_from_records(&records)?;
        Ok(ResumeResultModel {
            id: self.session.current_id().to_owned(),
            records,
        })
    }

    /// Returns a provider summary for the active runtime and configured providers.
    pub fn provider_notice(&self) -> Result<String, ChatError> {
        self.provider_notice_with_loader(|| self.config_io.read_config_or_default())
    }

    /// Builds provider notice text from an injected config loader.
    fn provider_notice_with_loader(
        &self,
        load_config: impl FnOnce() -> Result<SpectacularConfig, ConfigError>,
    ) -> Result<String, ChatError> {
        let config = load_config()?;
        Ok(self.format_provider_notice(&config))
    }

    /// Formats the active provider and all configured providers as terminal text.
    fn format_provider_notice(&self, config: &SpectacularConfig) -> String {
        let mut lines = vec![
            format!(
                "active provider: {} ({})",
                self.runtime.provider, self.runtime.provider_type
            ),
            "configured providers".to_owned(),
        ];
        lines.extend(
            config
                .providers
                .iter()
                .map(|(provider, config)| format!("- {provider} ({})", config.provider_type)),
        );
        if config.providers.is_empty() {
            lines.push(format!(
                "- {} ({})",
                self.runtime.provider, self.runtime.provider_type
            ));
        }
        lines.join("\n")
    }

    /// Returns a terminal summary for the currently selected coding model.
    pub fn coding_model_notice(&self) -> String {
        format!(
            "coding model\nname: {}\nprovider: {}\nmodel: {}\nreasoning: {}",
            self.runtime.model_key,
            self.runtime.provider,
            self.runtime.model,
            self.runtime.reasoning
        )
    }

    /// Appends an agent event to the active session transcript.
    pub fn append_agent_event(&self, event: &AgentEvent) -> Result<(), ChatError> {
        self.session.append_agent_event(event)
    }

    /// Appends an app-owned command lifecycle event to the active session transcript.
    pub fn append_command_event(&self, event: &CommandEvent) -> Result<(), ChatError> {
        self.session.append_command_event(event)
    }

    /// Stores the latest provider-context token usage for prompt footer rendering.
    pub fn set_context_token_usage(&self, usage: ContextTokenUsage) {
        *self.context_token_usage_state() = Some(usage);
    }

    /// Returns the latest provider-context token usage known to the chat model.
    pub fn context_token_usage(&self) -> Option<ContextTokenUsage> {
        *self.context_token_usage_state()
    }

    /// Clears the latest provider-context token usage from prompt footer state.
    fn clear_context_token_usage(&self) {
        *self.context_token_usage_state() = None;
    }

    /// Appends runtime metadata defaults to the active session transcript.
    pub fn append_runtime_defaults(&self, source: &str) -> Result<(), ChatError> {
        self.session.append_runtime_defaults(&self.runtime, source)
    }

    /// Truncates the active session after the latest user prompt and returns that prompt.
    pub fn truncate_after_latest_user_prompt(&self) -> Result<String, ChatError> {
        self.session.truncate_after_latest_user_prompt()
    }

    /// Returns the active runtime selection.
    pub fn runtime(&self) -> &RuntimeSelection {
        &self.runtime
    }

    /// Returns the debug logger configured for provider calls.
    pub fn debug_logger(&self) -> &LlmDebugLogger {
        &self.debug_logger
    }

    /// Returns the underlying session manager for controller and runner orchestration.
    pub(super) fn session_manager(&self) -> &SessionManager {
        &self.session
    }

    /// Returns the injected config I/O used by provider auth stores.
    pub(crate) fn config_io(&self) -> ChatConfigIo {
        self.config_io
    }

    /// Builds the narrow environment exposed to prompt completion value resolvers.
    pub(crate) fn completion_environment(&self) -> CompletionEnvironment {
        CompletionEnvironment::new(self.config_io, spectacular_llms::provider_registry())
    }

    /// Restores runtime selection from session metadata, falling back to current config.
    fn restore_runtime_from_records(&mut self, records: &[ChatRecord]) -> Result<(), ChatError> {
        let cache = self
            .config_io
            .read_model_cache_or_default()
            .unwrap_or_default();
        let config = self
            .config_io
            .read_config_or_default()
            .unwrap_or_else(|_| config_for_runtime(&self.runtime));
        if let Some(runtime) =
            RuntimeSelection::from_session_records_and_cache(&config, &cache, records)?
        {
            self.runtime = runtime;
            return Ok(());
        }

        if let Some(runtime) = RuntimeSelection::from_session_records_and_cache(
            &config_for_runtime(&self.runtime),
            &cache,
            records,
        )? {
            self.runtime = runtime;
            return Ok(());
        }

        self.runtime = RuntimeSelection::from_config_and_cache(&config, &cache)?;
        self.append_runtime_defaults("resume_fallback")
    }

    /// Refreshes the runtime when the changed model is assigned to the coding task.
    pub(crate) fn refresh_runtime_if_coding_model_changed(
        &mut self,
        config: &SpectacularConfig,
        model_key: &str,
    ) -> Result<Option<RuntimeSelection>, ChatError> {
        if config.tasks.get(TaskModelSlot::Coding) != Some(model_key) {
            return Ok(None);
        }

        let cache = self.config_io.read_model_cache_or_default()?;
        let runtime = RuntimeSelection::from_config_and_cache(config, &cache)?;
        self.replace_runtime(runtime.clone());
        self.append_runtime_defaults("command")?;
        Ok(Some(runtime))
    }

    /// Replaces the active runtime after a config-backed command changes it.
    pub(crate) fn replace_runtime(&mut self, runtime: RuntimeSelection) {
        self.runtime = runtime;
        self.clear_context_token_usage();
    }

    /// Locks context token usage state, recovering from poisoned locks for UI continuity.
    fn context_token_usage_state(&self) -> MutexGuard<'_, Option<ContextTokenUsage>> {
        self.context_token_usage
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

#[derive(Debug, Eq, PartialEq)]
pub struct SessionStartedModel {
    pub id: String,
}

#[derive(Debug, Eq, PartialEq)]
pub struct ResumeResultModel {
    pub id: String,
    pub records: Vec<ChatRecord>,
}

#[derive(Debug, Eq, PartialEq)]
pub struct HistoryTableModel {
    pub rows: Vec<HistoryRowModel>,
    pub remaining: usize,
}

#[derive(Debug, Eq, PartialEq)]
pub struct HistoryRowModel {
    pub id: String,
    pub updated: String,
    pub title: String,
    pub messages: usize,
    pub corrupt: bool,
}

impl From<HistorySummary> for HistoryRowModel {
    /// Converts a persisted history summary into a terminal table row model.
    fn from(summary: HistorySummary) -> Self {
        Self {
            id: summary.id,
            updated: crate::chat::session::format_local_time(summary.updated),
            title: truncate(&summary.title, 22),
            messages: summary.messages,
            corrupt: summary.corrupt,
        }
    }
}

/// Data carried with a new prompt so the renderer can show contextual footer text.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ChatPromptFooterModel {
    pub directory: PathBuf,
    pub model: String,
    pub reasoning: spectacular_config::ReasoningLevel,
    pub token_usage: Option<ContextTokenUsage>,
}

impl ChatPromptFooterModel {
    /// Builds prompt footer data with optional latest context token usage.
    pub fn from_runtime_and_usage(
        directory: &Path,
        runtime: &RuntimeSelection,
        token_usage: Option<ContextTokenUsage>,
    ) -> Self {
        Self {
            directory: directory.to_path_buf(),
            model: runtime.model.clone(),
            reasoning: runtime.reasoning,
            token_usage: token_usage.or_else(|| {
                runtime
                    .context_window_tokens
                    .map(|context_window_tokens| ContextTokenUsage {
                        input_tokens: 0,
                        context_window_tokens: Some(context_window_tokens as u64),
                    })
            }),
        }
    }
}

/// Request data for running one chat turn through the runner service.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ChatRunRequestModel {
    pub prompt: String,
    pub render_user_prompt: bool,
    pub retry_existing_prompt: bool,
    pub runtime: RuntimeSelection,
}

/// Truncates display text to a character limit with an ellipsis when needed.
fn truncate(value: &str, limit: usize) -> String {
    if value.chars().count() <= limit {
        return value.to_owned();
    }

    value
        .chars()
        .take(limit.saturating_sub(3))
        .collect::<String>()
        + "..."
}

/// Builds a minimal config containing the active runtime for session-resume fallback logic.
fn config_for_runtime(runtime: &RuntimeSelection) -> SpectacularConfig {
    let mut providers = BTreeMap::new();
    let provider = match runtime.provider_auth {
        Some(ProviderAuthMode::Oauth) => ProviderConfig {
            provider_type: runtime.provider_type.clone(),
            credentials: None,
        },
        Some(ProviderAuthMode::ApiKey) | None => {
            ProviderConfig::new(runtime.provider_type.clone(), runtime.api_key.clone())
        }
    };
    providers.insert(runtime.provider.clone(), provider);
    let mut models = BTreeMap::new();
    models.insert(
        runtime.model_key.clone(),
        ModelConfig::new(
            runtime.provider.clone(),
            runtime.model.clone(),
            runtime.reasoning,
        ),
    );

    SpectacularConfig {
        providers,
        models,
        tasks: TaskAssignments {
            general: None,
            coding: Some(runtime.model_key.clone()),
            labeling: None,
        },
    }
}

#[cfg(test)]
mod tests {
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/unit/chat/model.rs"
    ));
}
