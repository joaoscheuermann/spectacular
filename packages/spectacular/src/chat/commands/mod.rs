pub(crate) mod completion;
pub mod config;
pub mod git;
pub mod runtime;
pub mod session;

use crate::chat::model::{ChatModel, ChatPromptFooterModel, ChatRunRequestModel, HistoryTableModel};
use crate::chat::prompt::{SelectionPrompt, SelectionPromptAnswer, SelectionPromptRequest};
use crate::chat::renderer::Renderer;
use crate::chat::runner::ChatTurnRunner;
use crate::chat::session::ChatRecord;
use crate::chat::ChatError;
pub(crate) use completion::{
    ChatCompletionContext, CompletionCommandSpec, CompletionEnvironment, CompletionFieldSpec,
    CompletionSubcommandSpec, CompletionValueValidation,
};
use spectacular_agent::{AgentEvent, ToolStorage};
use spectacular_commands::{
    Command, CommandControl, CommandError, CommandFuture, CommandInvocation, CommandRegistry,
};
use std::collections::BTreeMap;
use std::future::Future;
use std::path::Path;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

pub type ChatCommandFuture<'a> = Pin<Box<dyn Future<Output = ChatCommandResult> + 'a>>;

pub type ChatCommandHandler =
    for<'a> fn(ChatCommandContext<'a>, Vec<String>) -> ChatCommandFuture<'a>;

#[derive(Debug, Eq, PartialEq)]
pub enum ChatCommandResult {
    Success,
    Error(String),
}

impl ChatCommandResult {
    /// Creates a successful command result with no extra control flow.
    pub fn success() -> Self {
        Self::Success
    }

    /// Creates a command error result with user-facing text.
    pub fn error(message: impl Into<String>) -> Self {
        Self::Error(message.into())
    }
}

#[derive(Debug, Default)]
pub struct ChatCommandControl {
    exit_requested: bool,
}

impl ChatCommandControl {
    /// Marks that the command loop should exit after the current command finishes.
    pub fn request_exit(&mut self) {
        self.exit_requested = true;
    }

    /// Returns whether a command requested the chat loop to exit.
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
    /// Copies static command metadata and handler pointers.
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
    prompt_footer: Option<ChatPromptFooterModel>,
}

impl<'a> ChatCommandContext<'a> {
    /// Creates a command execution context from the active chat services.
    #[allow(dead_code, reason = "unit tests and embedders use contexts without prompt footer metadata")]
    pub fn new(
        model: &'a mut ChatModel,
        renderer: &'a Renderer,
        tools: &'a ToolStorage,
        runner: &'a dyn ChatTurnRunner,
        control: &'a mut ChatCommandControl,
    ) -> Self {
        Self::new_with_footer(model, renderer, tools, runner, control, None)
    }

    /// Creates a command execution context with optional prompt footer metadata.
    pub fn new_with_footer(
        model: &'a mut ChatModel,
        renderer: &'a Renderer,
        tools: &'a ToolStorage,
        runner: &'a dyn ChatTurnRunner,
        control: &'a mut ChatCommandControl,
        prompt_footer: Option<ChatPromptFooterModel>,
    ) -> Self {
        Self {
            model,
            renderer,
            tools,
            runner,
            control,
            prompt_footer,
        }
    }

    #[allow(
        dead_code,
        reason = "command context exposes persistence for commands that need history records"
    )]
    pub fn append_agent_event(&self, event: &AgentEvent) -> Result<(), ChatError> {
        self.model.append_agent_event(event)
    }

    /// Renders chat records through the injected renderer and tool storage.
    pub async fn render_records(&self, records: &[ChatRecord]) -> Result<(), ChatError> {
        self.renderer.render_records(records, self.tools).await
    }

    /// Renders a chat history table through the injected renderer.
    pub fn render_history(&self, table: &HistoryTableModel) {
        self.renderer.history_table(table);
    }

    /// Clears the terminal screen through the injected renderer.
    pub fn clear_screen(&self) {
        self.renderer.clear_screen();
    }

    /// Renders a session-created notice for a new chat session.
    pub fn session_created(&self, id: &str, directory: &Path) {
        self.renderer
            .session_created(id, self.model.runtime(), directory);
    }

    /// Renders a session-resumed notice for an existing chat session.
    pub fn session_resumed(&self, id: &str) {
        self.renderer.resumed(id);
    }

    /// Renders a low-emphasis informational command message.
    pub fn notice(&self, message: &str) {
        self.renderer.dim(message);
    }

    /// Renders a successful command message.
    pub fn success(&self, message: &str) {
        self.renderer.success(message);
    }

    /// Renders an interactive option selection prompt and returns the user's answer.
    pub fn ask(&self, request: SelectionPromptRequest) -> Result<SelectionPromptAnswer, ChatError> {
        let prompt = SelectionPrompt::new(self.renderer, request);
        if let Some(footer) = &self.prompt_footer {
            return prompt.with_footer(footer.clone()).read_selection();
        }

        prompt.read_selection()
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

    /// Requests that the chat command loop exit after this command.
    pub fn request_exit(&mut self) {
        self.control.request_exit();
    }

    /// Runs a prompt through the injected turn runner from inside a command.
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
    /// Registers command handlers and builds shared command metadata for prompt completion.
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

    /// Executes a registered chat command invocation against the provided context.
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

    /// Returns command metadata used by the prompt editor command picker.
    pub fn metadata(&self) -> &Arc<CommandRegistry<()>> {
        &self.metadata
    }

    /// Returns subcommand and field metadata used by prompt completion.
    pub fn completion_specs(&self) -> &[CompletionCommandSpec] {
        &self.completion_specs
    }
}

/// Provides a no-op executor for metadata-only command registry entries.
fn metadata_execute<'a>(_context: &'a mut (), _args: Vec<String>) -> CommandFuture<'a> {
    Box::pin(async { Ok(CommandControl::Continue) })
}

/// Builds the default registry of chat commands available in the REPL.
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
        git::command(),
    ])
}

#[cfg(test)]
pub(crate) mod test_support {
    use super::*;
    use crate::chat::runner::{ChatTurnFuture, ChatTurnRunner};

    pub(crate) struct NoopRunner;

    impl ChatTurnRunner for NoopRunner {
        /// Ignores prompt execution for command unit tests that only need a runner seam.
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
