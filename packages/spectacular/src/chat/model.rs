use super::RuntimeSelection;
use crate::chat::session::{ChatRecord, HistoryQuery, HistorySummary, SessionManager};
use crate::chat::ChatError;
use spectacular_agent::AgentEvent;
use spectacular_config::{
    ConfigError, ModelCache, ModelConfig, ProviderConfig, ReasoningLevel, SpectacularConfig,
    TaskAssignments, TaskModelSlot,
};
use spectacular_llms::{provider_registry, LlmDebugLogger};
use std::collections::BTreeMap;
use std::path::PathBuf;

/// State model for chat sessions, runtime selection, and config-backed commands.
pub struct ChatModel {
    session: SessionManager,
    runtime: RuntimeSelection,
    debug_logger: LlmDebugLogger,
    config_io: ChatConfigIo,
}

/// Injected config I/O operations used by chat commands that mutate persisted config.
#[derive(Clone, Copy)]
pub(crate) struct ChatConfigIo {
    read_config_or_default: fn() -> Result<SpectacularConfig, ConfigError>,
    read_model_cache_or_default: fn() -> Result<ModelCache, ConfigError>,
    backup_config: fn() -> Result<Option<PathBuf>, ConfigError>,
    write_config: fn(&SpectacularConfig) -> Result<(), ConfigError>,
}

impl ChatConfigIo {
    /// Reads the current config, returning a default config when no persisted file exists.
    fn read_config_or_default(&self) -> Result<SpectacularConfig, ConfigError> {
        (self.read_config_or_default)()
    }

    /// Reads the cached provider model metadata, returning an empty cache when absent.
    fn read_model_cache_or_default(&self) -> Result<ModelCache, ConfigError> {
        (self.read_model_cache_or_default)()
    }

    /// Creates a backup of the persisted config before destructive mutations.
    fn backup_config(&self) -> Result<Option<PathBuf>, ConfigError> {
        (self.backup_config)()
    }

    /// Writes the updated config through the configured persistence function.
    fn write_config(&self, config: &SpectacularConfig) -> Result<(), ConfigError> {
        (self.write_config)(config)
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
        }
    }

    /// Starts a new persisted chat session using the current runtime selection.
    pub fn start_new_session(&mut self) -> Result<SessionStartedModel, ChatError> {
        self.session.create(self.runtime.clone())?;
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

    /// Adds a provider to config and persists the updated config.
    pub fn add_provider(
        &mut self,
        name: &str,
        provider_type: &str,
        apikey: &str,
    ) -> Result<(), ChatError> {
        let mut config = self.config_io.read_config_or_default()?;
        config.add_provider(name, provider_type, apikey)?;
        self.config_io.write_config(&config)?;
        Ok(())
    }

    /// Removes a provider after backing up config and returns model keys that reference it.
    pub fn remove_provider(&mut self, name: &str) -> Result<Vec<String>, ChatError> {
        let mut config = self.config_io.read_config_or_default()?;
        let orphaned_models = config
            .models
            .iter()
            .filter(|(_, model)| model.provider == name)
            .map(|(key, _)| key.clone())
            .collect::<Vec<_>>();
        self.config_io.backup_config()?;
        config.remove_provider(name)?;
        self.config_io.write_config(&config)?;
        Ok(orphaned_models)
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

    /// Adds a model definition to config and returns the saved model key.
    pub fn add_model(
        &mut self,
        provider: &str,
        model_id: &str,
        reasoning: ReasoningLevel,
        name: Option<String>,
    ) -> Result<String, ChatError> {
        let mut config = self.config_io.read_config_or_default()?;
        let key = config.add_model(provider, model_id, reasoning, name)?;
        self.config_io.write_config(&config)?;
        Ok(key)
    }

    /// Edits a saved model and refreshes the active runtime when the coding model changed.
    pub fn edit_model(
        &mut self,
        name: &str,
        provider: Option<String>,
        model_id: Option<String>,
        reasoning: Option<ReasoningLevel>,
    ) -> Result<Option<RuntimeSelection>, ChatError> {
        let mut config = self.config_io.read_config_or_default()?;
        config.edit_model(name, provider, model_id, reasoning)?;
        self.config_io.write_config(&config)?;
        self.refresh_runtime_if_coding_model_changed(&config, name)
    }

    /// Removes a saved model after backing up config and returns task slots that referenced it.
    pub fn remove_model(&mut self, name: &str) -> Result<Vec<TaskModelSlot>, ChatError> {
        let mut config = self.config_io.read_config_or_default()?;
        let references = config.tasks.references_to(name);
        self.config_io.backup_config()?;
        config.remove_model(name)?;
        self.config_io.write_config(&config)?;
        Ok(references)
    }

    /// Assigns a model to a task slot and refreshes the runtime for coding-slot changes.
    pub fn set_task_model(
        &mut self,
        slot: TaskModelSlot,
        model_key: &str,
    ) -> Result<Option<RuntimeSelection>, ChatError> {
        let mut config = self.config_io.read_config_or_default()?;
        config.set_task_model(slot, model_key)?;
        self.config_io.write_config(&config)?;
        if slot != TaskModelSlot::Coding {
            return Ok(None);
        }

        let runtime = RuntimeSelection::from_config(&config)?;
        self.runtime = runtime.clone();
        self.append_runtime_defaults("command")?;
        Ok(Some(runtime))
    }

    /// Appends an agent event to the active session transcript.
    pub fn append_agent_event(&self, event: &AgentEvent) -> Result<(), ChatError> {
        self.session.append_agent_event(event)
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

    /// Builds dynamic prompt-completion sources from config and cached model metadata.
    pub fn prompt_completion_sources(&self) -> BTreeMap<String, Vec<String>> {
        let config = self.config_io.read_config_or_default().unwrap_or_default();
        let cache = self
            .config_io
            .read_model_cache_or_default()
            .unwrap_or_default();
        let mut sources = BTreeMap::new();
        sources.insert(
            crate::chat::commands::SOURCE_PROVIDER_TYPES.to_owned(),
            provider_registry()
                .iter()
                .filter(|provider| provider.is_enabled())
                .map(|provider| provider.id().to_owned())
                .collect(),
        );
        sources.insert(
            crate::chat::commands::SOURCE_PROVIDERS.to_owned(),
            config.providers.keys().cloned().collect(),
        );
        sources.insert(
            crate::chat::commands::SOURCE_MODELS.to_owned(),
            config.models.keys().cloned().collect(),
        );

        let mut all_model_ids = Vec::new();
        for (provider, provider_cache) in cache.providers {
            let model_ids = provider_cache.models.keys().cloned().collect::<Vec<_>>();
            all_model_ids.extend(model_ids.iter().cloned());
            sources.insert(
                format!("{}:{provider}", crate::chat::commands::SOURCE_MODEL_IDS),
                model_ids,
            );
        }
        all_model_ids.sort();
        all_model_ids.dedup();
        sources.insert(
            crate::chat::commands::SOURCE_MODEL_IDS.to_owned(),
            all_model_ids,
        );

        sources
    }

    /// Returns the underlying session manager for controller and runner orchestration.
    pub(super) fn session_manager(&self) -> &SessionManager {
        &self.session
    }

    /// Restores runtime selection from session metadata, falling back to current config.
    fn restore_runtime_from_records(&mut self, records: &[ChatRecord]) -> Result<(), ChatError> {
        let config = self
            .config_io
            .read_config_or_default()
            .unwrap_or_else(|_| config_for_runtime(&self.runtime));
        if let Some(runtime) = RuntimeSelection::from_session_records(&config, records)? {
            self.runtime = runtime;
            return Ok(());
        }

        if let Some(runtime) =
            RuntimeSelection::from_session_records(&config_for_runtime(&self.runtime), records)?
        {
            self.runtime = runtime;
            return Ok(());
        }

        self.runtime = RuntimeSelection::from_config(&config)?;
        self.append_runtime_defaults("resume_fallback")
    }

    /// Refreshes the runtime when the changed model is assigned to the coding task.
    fn refresh_runtime_if_coding_model_changed(
        &mut self,
        config: &SpectacularConfig,
        model_key: &str,
    ) -> Result<Option<RuntimeSelection>, ChatError> {
        if config.tasks.get(TaskModelSlot::Coding) != Some(model_key) {
            return Ok(None);
        }

        let runtime = RuntimeSelection::from_config(config)?;
        self.runtime = runtime.clone();
        self.append_runtime_defaults("command")?;
        Ok(Some(runtime))
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
    providers.insert(
        runtime.provider.clone(),
        ProviderConfig::new(runtime.provider_type.clone(), runtime.api_key.clone()),
    );
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
