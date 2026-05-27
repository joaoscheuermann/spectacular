mod auth;
mod command_event;
mod commands;
mod config_mutation;
mod display;
mod model;
mod model_cache;
mod provider;
mod runner;
mod runtime_selection;
mod selection;
mod session;
mod tui;
mod worktree;

use crate::chat::model_cache::refresh_model_cache;
use crate::chat::runner::main_chat_tool_storage;
use crate::chat::session::SessionManager;
#[cfg(test)]
use session::ChatEvent;
use spectacular_agent::ToolStorage;
use spectacular_commands::CommandError;
#[cfg(test)]
use spectacular_config::SpectacularConfig;
use spectacular_config::{ConfigError, ProviderAuthMode, ReasoningLevel};
use spectacular_llms::LlmDebugLogger;
use std::error::Error;
use std::fmt::{self, Display};
use std::io;
use std::path::PathBuf;

/// Runs the IOCraft TUI chat loop and returns the closed session ID.
pub async fn run(debug_logger: LlmDebugLogger) -> Result<String, ChatError> {
    tui::run(debug_logger).await
}

pub(crate) struct ChatBootstrap {
    session: SessionManager,
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
            runtime,
            tools,
            workspace_root,
            debug_logger,
            warnings,
        })
    }
}

/// Runtime provider, model, and reasoning settings for a chat session.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeSelection {
    /// Configured provider implementation type.
    pub(crate) provider_type: String,
    /// Authentication mode used by the selected provider, when configured.
    pub(crate) provider_auth: Option<ProviderAuthMode>,
    /// Configured provider name.
    pub(crate) provider: String,
    /// API key resolved from provider configuration.
    pub(crate) api_key: String,
    /// Config model key selected for coding tasks.
    pub(crate) model_key: String,
    /// Provider model identifier selected for coding tasks.
    pub(crate) model: String,
    /// Reasoning level requested for the selected model.
    pub(crate) reasoning: ReasoningLevel,
    /// Cached context window advertised by provider metadata.
    pub(crate) context_window_tokens: Option<usize>,
}

/// Validates that cached provider metadata supports the requested reasoning level.
pub(crate) fn validate_cached_model_reasoning(
    cache: &spectacular_config::ModelCache,
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

/// Error type returned by chat setup, runtime, session, and command flows.
#[derive(Debug)]
pub enum ChatError {
    /// Configuration loading, validation, or persistence failed.
    Config(ConfigError),
    /// A slash command or app-owned command failed.
    Command(CommandError),
    /// Terminal, filesystem, or process I/O failed.
    Io(io::Error),
    /// Session, provider, or user-facing chat state failed.
    Session(String),
    /// The chat loop was intentionally exited.
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
