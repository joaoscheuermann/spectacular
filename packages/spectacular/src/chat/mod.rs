mod commands;
mod controller;
mod model;
mod paste_burst;
mod prompt;
mod provider;
mod renderer;
mod runner;
mod session;

use crate::chat::renderer::Renderer;
use crate::chat::runner::main_chat_tool_storage;
use crate::chat::session::{ChatEvent, SessionManager};
use controller::ChatController;
use model::ChatModel;
use spectacular_agent::ToolStorage;
use spectacular_commands::CommandError;
use spectacular_config::{ConfigError, ReasoningLevel, SpectacularConfig, TaskModelSlot};
use std::error::Error;
use std::fmt::{self, Display};
use std::io;
use std::str::FromStr;

pub async fn run() -> Result<(), ChatError> {
    let bootstrap = ChatBootstrap::new()?;
    let mut model = ChatModel::new(bootstrap.session, bootstrap.runtime);
    let started = model.start_new_session()?;
    bootstrap.renderer.clear_screen();
    bootstrap.renderer.session_created(&started.id);
    let mut controller = ChatController::new(
        model,
        commands::registry()?,
        bootstrap.renderer,
        bootstrap.tools,
    );
    controller.run_loop().await
}

struct ChatBootstrap {
    session: SessionManager,
    renderer: Renderer,
    runtime: RuntimeSelection,
    tools: ToolStorage,
}

impl ChatBootstrap {
    fn new() -> Result<Self, ChatError> {
        let config = spectacular_config::read_config_or_default()?;
        let runtime = RuntimeSelection::from_config(&config)?;
        let workspace_root = std::env::current_dir().map_err(ChatError::Io)?;
        let tools = main_chat_tool_storage(workspace_root)
            .map_err(|error| ChatError::Session(error.to_string()))?;
        Ok(Self {
            session: SessionManager::new()?,
            renderer: Renderer,
            runtime,
            tools,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
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
    use spectacular_commands::CommandControl;
    use spectacular_config::{ProviderConfig, ProvidersConfig, TaskModelConfig, TaskModels};
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

    #[tokio::test]
    async fn chat_controller_dispatches_exit_command() {
        let session = session::SessionManager::new_in(temp_session_dir("controller-exit"))
            .expect("session manager should be created");
        let mut model = super::model::ChatModel::new(
            session,
            RuntimeSelection {
                provider: "openrouter".to_owned(),
                api_key: "sk-or-v1-test".to_owned(),
                model: "test/model".to_owned(),
                reasoning: ReasoningLevel::Medium,
            },
        );
        model.start_new_session().unwrap();
        let mut controller = super::controller::ChatController::new(
            model,
            commands::registry().unwrap(),
            Renderer::default(),
            ToolStorage::default(),
        );

        let control = controller
            .dispatch_command(spectacular_commands::CommandInvocation {
                name: "exit".to_owned(),
                args: Vec::new(),
            })
            .await
            .unwrap();

        assert_eq!(control, CommandControl::Exit);
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

    fn temp_session_dir(name: &str) -> std::path::PathBuf {
        let suffix = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();

        std::env::temp_dir().join(format!("spectacular-chat-{name}-{suffix}"))
    }
}
