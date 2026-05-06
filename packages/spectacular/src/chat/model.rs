use super::RuntimeSelection;
use crate::chat::session::{ChatRecord, HistoryQuery, HistorySummary, SessionManager};
use crate::chat::ChatError;
use spectacular_agent::AgentEvent;
use spectacular_config::{
    ConfigError, ProviderConfig, ProvidersConfig, SpectacularConfig, TaskModelConfig,
    TaskModelSlot, TaskModels,
};
use std::collections::BTreeMap;

pub struct ChatModel {
    session: SessionManager,
    runtime: RuntimeSelection,
}

impl ChatModel {
    pub fn new(session: SessionManager, runtime: RuntimeSelection) -> Self {
        Self { session, runtime }
    }

    pub fn start_new_session(&mut self) -> Result<SessionStartedModel, ChatError> {
        self.session.create(self.runtime.clone())?;
        Ok(SessionStartedModel {
            id: self.session.current_id().to_owned(),
        })
    }

    pub fn current_session_id(&self) -> &str {
        self.session.current_id()
    }

    pub fn records(&self) -> Result<Vec<ChatRecord>, ChatError> {
        self.session.records()
    }

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

    pub fn resume_session(&mut self, prefix: &str) -> Result<ResumeResultModel, ChatError> {
        let records = self.session.resume(prefix)?;
        self.restore_runtime_from_records(&records)?;
        Ok(ResumeResultModel {
            id: self.session.current_id().to_owned(),
            records,
        })
    }

    pub fn provider_notice(&self) -> Result<String, ChatError> {
        self.provider_notice_with_loader(spectacular_config::read_config_or_default)
    }

    fn provider_notice_with_loader(
        &self,
        load_config: impl FnOnce() -> Result<SpectacularConfig, ConfigError>,
    ) -> Result<String, ChatError> {
        let config = load_config()?;
        Ok(self.format_provider_notice(&config))
    }

    fn format_provider_notice(&self, config: &SpectacularConfig) -> String {
        let active = config
            .providers
            .selected
            .as_deref()
            .unwrap_or(self.runtime.provider.as_str());
        let mut lines = vec![
            format!("active provider: {active}"),
            "configured providers".to_owned(),
        ];
        lines.extend(
            config
                .providers
                .available
                .keys()
                .map(|provider| format!("- {provider}")),
        );
        if config.providers.available.is_empty() {
            lines.push(format!("- {}", self.runtime.provider));
        }
        lines.join("\n")
    }

    pub fn switch_provider(&mut self, provider: &str) -> Result<RuntimeSelection, ChatError> {
        let mut config = spectacular_config::read_config_or_default()?;
        config.select_provider(provider)?;
        let runtime = RuntimeSelection::from_config(&config)?;
        spectacular_config::write_config(&config)?;
        self.runtime = runtime.clone();
        self.append_runtime_defaults("command")?;
        Ok(runtime)
    }

    pub fn coding_model_notice(&self) -> String {
        format!(
            "coding model\nprovider: {}\nmodel: {}\nreasoning: {}",
            self.runtime.provider, self.runtime.model, self.runtime.reasoning
        )
    }

    pub fn update_coding_model(
        &mut self,
        model: &str,
        reasoning: spectacular_config::ReasoningLevel,
    ) -> Result<RuntimeSelection, ChatError> {
        let mut config = spectacular_config::read_config_or_default()?;
        let provider = config
            .providers
            .selected
            .clone()
            .ok_or_else(|| ChatError::Session("no provider is selected".to_owned()))?;
        config.set_provider_task_model(&provider, TaskModelSlot::Coding, model, reasoning);
        self.save_runtime_config(config)
    }

    pub fn reasoning_notice(&self) -> String {
        format!("coding reasoning: {}", self.runtime.reasoning)
    }

    pub fn update_reasoning(
        &mut self,
        reasoning: spectacular_config::ReasoningLevel,
    ) -> Result<RuntimeSelection, ChatError> {
        let model = self.runtime.model.clone();
        let mut config = spectacular_config::read_config_or_default()?;
        let provider = config
            .providers
            .selected
            .clone()
            .ok_or_else(|| ChatError::Session("no provider is selected".to_owned()))?;
        config.set_provider_task_model(&provider, TaskModelSlot::Coding, model, reasoning);
        self.save_runtime_config(config)
    }

    pub fn append_agent_event(&self, event: &AgentEvent) -> Result<(), ChatError> {
        self.session.append_agent_event(event)
    }

    pub fn append_runtime_defaults(&self, source: &str) -> Result<(), ChatError> {
        self.session.append_runtime_defaults(&self.runtime, source)
    }

    pub fn truncate_after_latest_user_prompt(&self) -> Result<String, ChatError> {
        self.session.truncate_after_latest_user_prompt()
    }

    pub fn runtime(&self) -> &RuntimeSelection {
        &self.runtime
    }

    pub(super) fn session_manager(&self) -> &SessionManager {
        &self.session
    }

    fn restore_runtime_from_records(&mut self, records: &[ChatRecord]) -> Result<(), ChatError> {
        let config = spectacular_config::read_config_or_default()?;
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

    fn save_runtime_config(
        &mut self,
        config: SpectacularConfig,
    ) -> Result<RuntimeSelection, ChatError> {
        let runtime = RuntimeSelection::from_config(&config)?;
        spectacular_config::write_config(&config)?;
        self.runtime = runtime.clone();
        self.append_runtime_defaults("command")?;
        Ok(runtime)
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

fn config_for_runtime(runtime: &RuntimeSelection) -> SpectacularConfig {
    let mut available = BTreeMap::new();
    available.insert(
        runtime.provider.clone(),
        ProviderConfig {
            key: Some(runtime.api_key.clone()),
            tasks: TaskModels {
                planning: None,
                labeling: None,
                coding: Some(TaskModelConfig::new(
                    runtime.model.clone(),
                    runtime.reasoning,
                )),
            },
        },
    );

    SpectacularConfig {
        providers: ProvidersConfig {
            selected: Some(runtime.provider.clone()),
            available,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use spectacular_config::{ConfigError, ReasoningLevel};
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn chat_model_start_new_session_returns_active_session_id() {
        let session = crate::chat::session::SessionManager::new_in(temp_session_dir("start"))
            .expect("session manager should be created");
        let mut model = ChatModel::new(session, test_runtime());

        let started = model
            .start_new_session()
            .expect("session should be started");

        assert_eq!(started.id, model.current_session_id());
    }

    #[test]
    fn chat_model_records_reads_started_session_event() {
        let session = crate::chat::session::SessionManager::new_in(temp_session_dir("records"))
            .expect("session manager should be created");
        let mut model = ChatModel::new(session, test_runtime());

        model.start_new_session().unwrap();

        assert!(matches!(
            model.records().unwrap()[0].event(),
            Some(crate::chat::session::ChatEvent::SessionStarted { .. })
        ));
    }

    #[test]
    fn chat_model_append_agent_event_persists_user_prompt() {
        let session = crate::chat::session::SessionManager::new_in(temp_session_dir("append"))
            .expect("session manager should be created");
        let mut model = ChatModel::new(session, test_runtime());
        model.start_new_session().unwrap();

        model
            .append_agent_event(&spectacular_agent::AgentEvent::UserPrompt {
                content: "hello".to_owned(),
            })
            .unwrap();

        assert!(model.records().unwrap().iter().any(|record| matches!(
            record.event(),
            Some(crate::chat::session::ChatEvent::UserPrompt { content, .. }) if content == "hello"
        )));
    }

    #[test]
    fn chat_model_resume_session_returns_resumed_session_id() {
        let session = crate::chat::session::SessionManager::new_in(temp_session_dir("resume"))
            .expect("session manager should be created");
        let mut model = ChatModel::new(session, test_runtime());
        let started = model.start_new_session().unwrap();
        model.start_new_session().unwrap();

        let resumed = model.resume_session(&started.id).unwrap();

        assert_eq!(resumed.id, started.id);
    }

    #[test]
    fn provider_notice_propagates_config_load_error() {
        let session = crate::chat::session::SessionManager::new_in(temp_session_dir("provider"))
            .expect("session manager should be created");
        let model = ChatModel::new(session, test_runtime());

        let error = model
            .provider_notice_with_loader(|| Err(ConfigError::ConfigDirUnavailable))
            .unwrap_err();

        assert!(matches!(
            error,
            ChatError::Config(ConfigError::ConfigDirUnavailable)
        ));
    }

    fn test_runtime() -> RuntimeSelection {
        RuntimeSelection {
            provider: "openrouter".to_owned(),
            api_key: "sk-or-v1-test".to_owned(),
            model: "test/model".to_owned(),
            reasoning: ReasoningLevel::Medium,
        }
    }

    fn temp_session_dir(name: &str) -> PathBuf {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();

        std::env::temp_dir().join(format!("spectacular-chat-model-{name}-{suffix}"))
    }
}
