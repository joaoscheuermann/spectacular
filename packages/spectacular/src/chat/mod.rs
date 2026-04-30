mod commands;
mod prompt;
mod provider;
mod renderer;
mod runner;
mod session;

use crate::chat::prompt::PromptEditor;
use crate::chat::renderer::Renderer;
use crate::chat::runner::{ChatRunRequest, ChatRunner};
use crate::chat::session::{ChatEvent, SessionManager};
use spectacular_commands::{
    parse_line, CommandControl, CommandError, CommandRegistry, ParseOutcome,
};
use spectacular_config::{
    ConfigError, ReasoningLevel, SpectacularConfig, TaskModelConfig, TaskModelSlot,
};
use std::error::Error;
use std::fmt::{self, Display};
use std::io::{self, IsTerminal, Write};
use std::str::FromStr;
use std::sync::Arc;

pub async fn run() -> Result<(), ChatError> {
    let mut context = ChatContext::new()?;
    context.start_new_session()?;
    context.renderer.clear_screen();
    context
        .renderer
        .session_created(context.session.current_id());
    context.run_loop().await
}

pub struct ChatContext {
    session: SessionManager,
    renderer: Renderer,
    registry: Arc<CommandRegistry<ChatContext>>,
    runtime: RuntimeSelection,
}

impl ChatContext {
    fn new() -> Result<Self, ChatError> {
        let config = spectacular_config::read_config_or_default()?;
        let runtime = RuntimeSelection::from_config(&config)?;
        Ok(Self {
            session: SessionManager::new()?,
            renderer: Renderer::default(),
            registry: Arc::new(commands::registry()?),
            runtime,
        })
    }

    async fn run_loop(&mut self) -> Result<(), ChatError> {
        loop {
            let line = self.read_prompt_line()?;
            let line = line.trim_end_matches(['\r', '\n']).to_owned();
            if line.trim().is_empty() {
                continue;
            }

            match parse_line(&line) {
                Ok(ParseOutcome::NotCommand) => self.run_user_prompt(line, true, false).await?,
                Ok(ParseOutcome::Command(invocation)) => {
                    let registry = Arc::clone(&self.registry);
                    match registry.execute(self, invocation).await {
                        Ok(CommandControl::Continue) => {}
                        Ok(CommandControl::Exit) => return Ok(()),
                        Err(error) => self.renderer.command_error(&error),
                    }
                }
                Err(error) => self.renderer.command_error(&error),
            }
        }
    }

    fn read_prompt_line(&self) -> Result<String, ChatError> {
        if io::stdin().is_terminal() && io::stdout().is_terminal() {
            return PromptEditor::new(&self.renderer, &self.registry).read_line();
        }

        self.renderer.prompt();
        io::stdout().flush().map_err(ChatError::Io)?;
        let mut line = String::new();
        let read = io::stdin().read_line(&mut line).map_err(ChatError::Io)?;
        if read == 0 {
            return Err(ChatError::Exit);
        }

        Ok(line)
    }

    pub(super) fn start_new_session(&mut self) -> Result<(), ChatError> {
        self.session.create(self.runtime.clone())?;
        Ok(())
    }

    pub(super) async fn run_user_prompt(
        &mut self,
        prompt: String,
        render_user_prompt: bool,
        retry_existing_prompt: bool,
    ) -> Result<(), ChatError> {
        ChatRunner::new(&self.session, &self.renderer)
            .run(ChatRunRequest {
                prompt,
                render_user_prompt,
                retry_existing_prompt,
                runtime: self.runtime.clone(),
            })
            .await
    }

    pub(super) fn restore_runtime_from_records(
        &mut self,
        records: &[session::ChatRecord],
    ) -> Result<(), ChatError> {
        let config = spectacular_config::read_config_or_default()?;
        if let Some(runtime) = RuntimeSelection::from_session_records(&config, records)? {
            self.runtime = runtime;
            return Ok(());
        }

        self.runtime = RuntimeSelection::from_config(&config)?;
        self.session
            .append_runtime_defaults(&self.runtime, "resume_fallback")
    }

    pub(super) fn show_provider(&self) -> Result<(), ChatError> {
        let config = spectacular_config::read_config_or_default()?;
        println!(
            "active provider: {}",
            config.providers.selected.as_deref().unwrap_or("none")
        );
        println!("configured providers");
        for provider in config.providers.available.keys() {
            println!("- {provider}");
        }
        Ok(())
    }

    pub(super) fn switch_provider(&mut self, provider: &str) -> Result<(), ChatError> {
        let mut config = spectacular_config::read_config_or_default()?;
        config.select_provider(provider)?;
        let runtime = RuntimeSelection::from_config(&config)?;
        spectacular_config::write_config(&config)?;
        self.runtime = runtime;
        self.session
            .append_runtime_defaults(&self.runtime, "command")?;
        self.renderer
            .success(&format!("active provider updated: {provider}"));
        Ok(())
    }

    pub(super) fn show_coding_model(&self) {
        println!("coding model");
        println!("provider: {}", self.runtime.provider);
        println!("model: {}", self.runtime.model);
        println!("reasoning: {}", self.runtime.reasoning);
    }

    pub(super) fn update_coding_model(
        &mut self,
        model: &str,
        reasoning: ReasoningLevel,
    ) -> Result<(), ChatError> {
        let mut config = spectacular_config::read_config_or_default()?;
        let provider = config
            .providers
            .selected
            .clone()
            .ok_or_else(|| ChatError::Session("no provider is selected".to_owned()))?;
        config.set_provider_task_model(&provider, TaskModelSlot::Coding, model, reasoning);
        self.save_runtime_config(config)?;
        self.renderer.success("coding model updated");
        Ok(())
    }

    pub(super) fn show_reasoning(&self) {
        println!("coding reasoning: {}", self.runtime.reasoning);
    }

    pub(super) fn update_reasoning(&mut self, reasoning: ReasoningLevel) -> Result<(), ChatError> {
        let model = self.runtime.model.clone();
        let mut config = spectacular_config::read_config_or_default()?;
        let provider = config
            .providers
            .selected
            .clone()
            .ok_or_else(|| ChatError::Session("no provider is selected".to_owned()))?;
        config.set_provider_task_model(&provider, TaskModelSlot::Coding, model, reasoning);
        self.save_runtime_config(config)?;
        self.renderer.success("coding reasoning updated");
        Ok(())
    }

    fn save_runtime_config(&mut self, config: SpectacularConfig) -> Result<(), ChatError> {
        let runtime = RuntimeSelection::from_config(&config)?;
        spectacular_config::write_config(&config)?;
        self.runtime = runtime;
        self.session
            .append_runtime_defaults(&self.runtime, "command")
    }
}

#[derive(Clone, Debug)]
pub struct RuntimeSelection {
    pub(crate) provider: String,
    pub(crate) api_key: String,
    pub(crate) model: String,
    pub(crate) reasoning: ReasoningLevel,
}

impl RuntimeSelection {
    fn from_config(config: &SpectacularConfig) -> Result<Self, ChatError> {
        let provider = config
            .providers
            .selected
            .as_deref()
            .ok_or(ConfigError::NoSelectedProvider)?;
        let provider_config = config.providers.available.get(provider).ok_or_else(|| {
            ConfigError::ProviderNotConfigured {
                provider: provider.to_owned(),
            }
        })?;
        let api_key = provider_config
            .key
            .as_deref()
            .filter(|key| !key.trim().is_empty())
            .ok_or_else(|| ConfigError::MissingProviderApiKey {
                provider: provider.to_owned(),
            })?;
        let coding = provider_config
            .tasks
            .coding
            .as_ref()
            .filter(|task| !task.model.trim().is_empty())
            .ok_or(ConfigError::MissingTaskModel {
                slot: TaskModelSlot::Coding,
            })?;

        Ok(Self {
            provider: provider.to_owned(),
            api_key: api_key.to_owned(),
            model: coding.model.clone(),
            reasoning: coding.reasoning,
        })
    }

    fn from_session_records(
        config: &SpectacularConfig,
        records: &[session::ChatRecord],
    ) -> Result<Option<Self>, ChatError> {
        let provider = records
            .iter()
            .rev()
            .find_map(|record| match record.event()? {
                ChatEvent::ProviderChanged { provider, .. } if !provider.trim().is_empty() => {
                    Some(provider.clone())
                }
                _ => None,
            });

        let model = records
            .iter()
            .rev()
            .find_map(|record| match record.event()? {
                ChatEvent::ModelChanged {
                    slot,
                    model,
                    reasoning,
                    ..
                } if slot == TaskModelSlot::Coding.as_str() && !model.trim().is_empty() => Some((
                    model.clone(),
                    ReasoningLevel::from_str(reasoning).unwrap_or_default(),
                )),
                _ => None,
            });

        let (Some(provider), Some((model, reasoning))) = (provider, model) else {
            return Ok(None);
        };

        let Some(provider_config) = config.providers.available.get(&provider) else {
            return Ok(None);
        };
        let Some(api_key) = provider_config
            .key
            .as_deref()
            .filter(|value| !value.trim().is_empty())
        else {
            return Ok(None);
        };

        Ok(Some(Self {
            provider,
            api_key: api_key.to_owned(),
            model,
            reasoning,
        }))
    }
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
    fn from(error: ConfigError) -> Self {
        Self::Config(error)
    }
}

impl From<CommandError> for ChatError {
    fn from(error: CommandError) -> Self {
        Self::Command(error)
    }
}

impl Display for ChatError {
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
    use super::*;
    use spectacular_config::{ProviderConfig, ProvidersConfig, TaskModels};
    use std::collections::BTreeMap;

    #[test]
    fn runtime_selection_uses_latest_session_provider_and_model_events() {
        let records = vec![
            chat_record(ChatEvent::ProviderChanged {
                provider: "openrouter".to_owned(),
                source: Some("global_default".to_owned()),
                created_at: "2026-04-29T14:00:00Z".to_owned(),
            }),
            chat_record(ChatEvent::ModelChanged {
                slot: "coding".to_owned(),
                provider: "openrouter".to_owned(),
                model: "old/model".to_owned(),
                reasoning: "low".to_owned(),
                source: Some("global_default".to_owned()),
                created_at: "2026-04-29T14:00:00Z".to_owned(),
            }),
            chat_record(ChatEvent::ModelChanged {
                slot: "coding".to_owned(),
                provider: "openrouter".to_owned(),
                model: "new/model".to_owned(),
                reasoning: "high".to_owned(),
                source: Some("command".to_owned()),
                created_at: "2026-04-29T14:01:00Z".to_owned(),
            }),
        ];

        let runtime = RuntimeSelection::from_session_records(&complete_config(), &records)
            .unwrap()
            .unwrap();

        assert_eq!(runtime.provider, "openrouter");
        assert_eq!(runtime.api_key, "sk-or-v1-test");
        assert_eq!(runtime.model, "new/model");
        assert_eq!(runtime.reasoning, ReasoningLevel::High);
    }

    #[test]
    fn runtime_selection_falls_back_when_session_provider_is_unavailable() {
        let records = vec![
            chat_record(ChatEvent::ProviderChanged {
                provider: "missing".to_owned(),
                source: Some("global_default".to_owned()),
                created_at: "2026-04-29T14:00:00Z".to_owned(),
            }),
            chat_record(ChatEvent::ModelChanged {
                slot: "coding".to_owned(),
                provider: "missing".to_owned(),
                model: "missing/model".to_owned(),
                reasoning: "medium".to_owned(),
                source: Some("global_default".to_owned()),
                created_at: "2026-04-29T14:00:00Z".to_owned(),
            }),
        ];

        let runtime = RuntimeSelection::from_session_records(&complete_config(), &records).unwrap();

        assert!(runtime.is_none());
    }

    fn complete_config() -> SpectacularConfig {
        let mut available = BTreeMap::new();
        available.insert(
            "openrouter".to_owned(),
            ProviderConfig {
                key: Some("sk-or-v1-test".to_owned()),
                tasks: TaskModels {
                    planning: None,
                    labeling: None,
                    coding: Some(TaskModelConfig::new(
                        "global/coding",
                        ReasoningLevel::Medium,
                    )),
                },
            },
        );

        SpectacularConfig {
            providers: ProvidersConfig {
                selected: Some("openrouter".to_owned()),
                available,
            },
        }
    }

    fn chat_record(event: ChatEvent) -> session::ChatRecord {
        session::ChatRecord::Known { line: 1, event }
    }
}
