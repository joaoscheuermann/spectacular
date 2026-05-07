pub mod config;
pub mod git;
pub mod runtime;
pub mod session;

use crate::chat::model::{ChatModel, ChatRunRequestModel, HistoryTableModel};
use crate::chat::renderer::Renderer;
use crate::chat::runner::ChatTurnRunner;
use crate::chat::session::ChatRecord;
use crate::chat::ChatError;
use spectacular_agent::{AgentEvent, ToolStorage};
use spectacular_commands::{
    Command, CommandControl, CommandError, CommandFuture, CommandInvocation, CommandRegistry,
    CompletionCommandSpec, CompletionSubcommandSpec,
};
use std::collections::BTreeMap;
use std::future::Future;
use std::path::Path;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

pub(crate) const SOURCE_PROVIDER_TYPES: &str = "provider-types";
pub(crate) const SOURCE_PROVIDERS: &str = "providers";
pub(crate) const SOURCE_MODELS: &str = "models";
pub(crate) const SOURCE_MODEL_IDS: &str = "model-ids";

pub type ChatCommandFuture<'a> = Pin<Box<dyn Future<Output = ChatCommandResult> + 'a>>;

pub type ChatCommandHandler =
    for<'a> fn(ChatCommandContext<'a>, Vec<String>) -> ChatCommandFuture<'a>;

#[derive(Debug, Eq, PartialEq)]
pub enum ChatCommandResult {
    Success,
    Error(String),
}

impl ChatCommandResult {
    pub fn success() -> Self {
        Self::Success
    }

    pub fn error(message: impl Into<String>) -> Self {
        Self::Error(message.into())
    }
}

#[derive(Debug, Default)]
pub struct ChatCommandControl {
    exit_requested: bool,
}

impl ChatCommandControl {
    pub fn request_exit(&mut self) {
        self.exit_requested = true;
    }

    pub fn exit_requested(&self) -> bool {
        self.exit_requested
    }
}

pub struct ChatCommand {
    pub name: &'static str,
    pub usage: &'static str,
    pub summary: &'static str,
    pub completion: &'static [CompletionSubcommandSpec],
    pub execute: ChatCommandHandler,
}

impl Clone for ChatCommand {
    fn clone(&self) -> Self {
        *self
    }
}

impl Copy for ChatCommand {}

pub struct ChatCommandContext<'a> {
    pub model: &'a mut ChatModel,
    pub renderer: &'a Renderer,
    pub tools: &'a ToolStorage,
    runner: &'a dyn ChatTurnRunner,
    control: &'a mut ChatCommandControl,
}

impl<'a> ChatCommandContext<'a> {
    pub fn new(
        model: &'a mut ChatModel,
        renderer: &'a Renderer,
        tools: &'a ToolStorage,
        runner: &'a dyn ChatTurnRunner,
        control: &'a mut ChatCommandControl,
    ) -> Self {
        Self {
            model,
            renderer,
            tools,
            runner,
            control,
        }
    }

    #[allow(
        dead_code,
        reason = "command context exposes persistence for commands that need history records"
    )]
    pub fn append_agent_event(&self, event: &AgentEvent) -> Result<(), ChatError> {
        self.model.append_agent_event(event)
    }

    pub async fn render_records(&self, records: &[ChatRecord]) -> Result<(), ChatError> {
        self.renderer.render_records(records, self.tools).await
    }

    pub fn render_history(&self, table: &HistoryTableModel) {
        self.renderer.history_table(table);
    }

    pub fn clear_screen(&self) {
        self.renderer.clear_screen();
    }

    pub fn session_created(&self, id: &str, directory: &Path) {
        self.renderer
            .session_created(id, self.model.runtime(), directory);
    }

    pub fn session_resumed(&self, id: &str) {
        self.renderer.resumed(id);
    }

    pub fn notice(&self, message: &str) {
        self.renderer.dim(message);
    }

    pub fn success(&self, message: &str) {
        self.renderer.success(message);
    }

    /// Runs async command work with a transient "working" indicator until the
    /// operation completes.
    pub async fn work<F, T>(&self, f: F) -> T
    where
        F: Future<Output = T>,
    {
        use std::pin::pin;

        let mut future = pin!(f);
        let mut frame = 0usize;
        let mut interval = tokio::time::interval(Duration::from_millis(90));
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

        self.renderer.working();

        let result = loop {
            tokio::select! {
                _ = interval.tick() => {
                    self.renderer.working_frame(frame);
                    frame = frame.wrapping_add(1);
                }
                result = &mut future => {
                    break result;
                }
            }
        };

        self.renderer.clear_working();
        result
    }

    pub fn request_exit(&mut self) {
        self.control.request_exit();
    }

    pub async fn run_prompt(&mut self, request: ChatRunRequestModel) -> Result<(), ChatError> {
        self.runner
            .run(self.model, self.renderer, self.tools, request)
            .await
    }
}

pub struct ChatCommandAdapter {
    commands: BTreeMap<&'static str, ChatCommand>,
    metadata: Arc<CommandRegistry<()>>,
    completion_specs: Vec<CompletionCommandSpec>,
}

impl ChatCommandAdapter {
    pub fn new<const N: usize>(commands: [ChatCommand; N]) -> Result<Self, CommandError> {
        let mut handlers = BTreeMap::new();
        let mut metadata = CommandRegistry::new();
        let mut completion_specs = Vec::new();
        for command in commands {
            metadata.register(Command {
                name: command.name,
                usage: command.usage,
                summary: command.summary,
                execute: metadata_execute,
            })?;
            if !command.completion.is_empty() {
                completion_specs.push(CompletionCommandSpec {
                    name: command.name,
                    subcommands: command.completion,
                });
            }
            handlers.insert(command.name, command);
        }

        Ok(Self {
            commands: handlers,
            metadata: Arc::new(metadata),
            completion_specs,
        })
    }

    pub async fn execute(
        &self,
        context: ChatCommandContext<'_>,
        invocation: CommandInvocation,
    ) -> ChatCommandResult {
        let Some(command) = self.commands.get(invocation.name.as_str()) else {
            return ChatCommandResult::error(
                CommandError::UnknownCommand {
                    name: invocation.name,
                }
                .to_string(),
            );
        };

        (command.execute)(context, invocation.args).await
    }

    pub fn metadata(&self) -> &Arc<CommandRegistry<()>> {
        &self.metadata
    }

    pub fn completion_specs(&self) -> &[CompletionCommandSpec] {
        &self.completion_specs
    }
}

fn metadata_execute<'a>(_context: &'a mut (), _args: Vec<String>) -> CommandFuture<'a> {
    Box::pin(async { Ok(CommandControl::Continue) })
}

pub fn registry() -> Result<ChatCommandAdapter, CommandError> {
    ChatCommandAdapter::new([
        session::new::command(),
        session::history::command(),
        session::resume::command(),
        session::clear::command(),
        session::exit::command(),
        config::provider::command(),
        config::model::command(),
        config::task::command(),
        runtime::retry::command(),
        git::status::command(),
        git::commit::command(),
    ])
}

#[cfg(test)]
pub(crate) mod test_support {
    use super::*;
    use crate::chat::runner::{ChatTurnFuture, ChatTurnRunner};

    pub(crate) struct NoopRunner;

    impl ChatTurnRunner for NoopRunner {
        fn run<'a>(
            &'a self,
            _model: &'a mut ChatModel,
            _renderer: &'a Renderer,
            _tools: &'a ToolStorage,
            _request: ChatRunRequestModel,
        ) -> ChatTurnFuture<'a> {
            Box::pin(async { Ok(()) })
        }
    }
}

#[cfg(test)]
mod tests {
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/unit/chat/commands/adapter.rs"
    ));
}
