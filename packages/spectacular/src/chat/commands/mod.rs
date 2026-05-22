pub(crate) mod completion;
pub mod config;
pub mod git;
pub mod runtime;
pub mod session;

use crate::chat::command_event::CommandEvent;
use crate::chat::model::{
    ChatModel, ChatPromptFooterModel, ChatRunRequestModel, HistoryTableModel,
};
use crate::chat::prompt::{SelectionPrompt, SelectionPromptAnswer, SelectionPromptRequest};
use crate::chat::renderer::Renderer;
use crate::chat::runner::ChatTurnRunner;
use crate::chat::session::ChatRecord;
use crate::chat::tui::TuiEventAdapter;
use crate::chat::ChatError;
pub(crate) use completion::{
    ChatCompletionContext, CompletionCommandSpec, CompletionEnvironment, CompletionFieldSpec,
    CompletionSubcommandSpec, CompletionValueValidation,
};
use spectacular_agent::{AgentEvent, ToolStorage};
use spectacular_commands::{
    Command, CommandControl, CommandError, CommandFuture, CommandInvocation, CommandRegistry,
};
use spectacular_tui::{ChatTuiAction, Intent};
use std::cell::RefCell;
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
    renderer: &'a Renderer,
    pub tools: &'a ToolStorage,
    runner: &'a dyn ChatTurnRunner,
    control: &'a mut ChatCommandControl,
    prompt_footer: Option<ChatPromptFooterModel>,
    tui: Option<TuiCommandBridge<'a>>,
}

pub(crate) struct TuiCommandBridge<'a> {
    dispatch: RefCell<&'a mut (dyn FnMut(ChatTuiAction) + Send)>,
    adapter: RefCell<TuiEventAdapter>,
    selection_receiver: RefCell<&'a mut tokio::sync::mpsc::UnboundedReceiver<Intent>>,
}

impl<'a> ChatCommandContext<'a> {
    /// Creates a command execution context from the active chat services.
    #[allow(
        dead_code,
        reason = "unit tests and embedders use contexts without prompt footer metadata"
    )]
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
            tui: None,
        }
    }

    /// Creates a command execution context that projects command output into TUI actions.
    pub(crate) fn new_tui(
        model: &'a mut ChatModel,
        renderer: &'a Renderer,
        tools: &'a ToolStorage,
        runner: &'a dyn ChatTurnRunner,
        control: &'a mut ChatCommandControl,
        prompt_footer: Option<ChatPromptFooterModel>,
        dispatch: &'a mut (dyn FnMut(ChatTuiAction) + Send),
        selection_receiver: &'a mut tokio::sync::mpsc::UnboundedReceiver<Intent>,
    ) -> Self {
        Self {
            model,
            renderer,
            tools,
            runner,
            control,
            prompt_footer,
            tui: Some(TuiCommandBridge {
                dispatch: RefCell::new(dispatch),
                adapter: RefCell::new(TuiEventAdapter::new()),
                selection_receiver: RefCell::new(selection_receiver),
            }),
        }
    }

    #[allow(
        dead_code,
        reason = "command context exposes persistence for commands that need history records"
    )]
    pub fn append_agent_event(&self, event: &AgentEvent) -> Result<(), ChatError> {
        self.model.append_agent_event(event)
    }

    /// Appends an app-owned command lifecycle event to the active session transcript.
    pub fn append_command_event(&self, event: &CommandEvent) -> Result<(), ChatError> {
        self.model.append_command_event(event)?;
        if let Some(tui) = &self.tui {
            tui.dispatch_command_event(event);
        }
        Ok(())
    }

    /// Renders chat records through the injected renderer and tool storage.
    pub async fn render_records(&self, records: &[ChatRecord]) -> Result<(), ChatError> {
        if let Some(tui) = &self.tui {
            tui.render_records(records, self.tools);
            return Ok(());
        }

        self.renderer.render_records(records, self.tools).await
    }

    /// Renders a chat history table through the injected renderer.
    pub fn render_history(&self, table: &HistoryTableModel) {
        if let Some(tui) = &self.tui {
            tui.render_history(table);
            return;
        }

        self.renderer.history_table(table);
    }

    /// Clears the terminal screen through the injected renderer.
    pub fn clear_screen(&self) {
        if let Some(tui) = &self.tui {
            tui.dispatch(ChatTuiAction::SessionChanged {
                id: spectacular_tui::SessionId::new(self.model.current_session_id()),
            });
            return;
        }

        self.renderer.clear_screen();
    }

    /// Renders a session-created notice for a new chat session.
    pub fn session_created(&self, id: &str, directory: &Path) {
        if let Some(tui) = &self.tui {
            tui.dispatch(crate::chat::tui::state::session_created_action(
                id, self.model, directory,
            ));
            return;
        }

        self.renderer
            .session_created(id, self.model.runtime(), directory);
    }

    /// Renders a session-resumed notice for an existing chat session.
    pub fn session_resumed(&self, id: &str) {
        if let Some(tui) = &self.tui {
            tui.dispatch(ChatTuiAction::SessionChanged {
                id: spectacular_tui::SessionId::new(id),
            });
            tui.dispatch(ChatTuiAction::NoticeReported {
                message: format!("resumed session {id}"),
            });
            return;
        }

        self.renderer.resumed(id);
    }

    /// Renders a low-emphasis informational command message.
    pub fn notice(&self, message: &str) {
        if let Some(tui) = &self.tui {
            tui.dispatch(ChatTuiAction::NoticeReported {
                message: message.to_owned(),
            });
            return;
        }

        self.renderer.dim(message);
    }

    /// Renders a successful command message.
    pub fn success(&self, message: &str) {
        if let Some(tui) = &self.tui {
            tui.dispatch(ChatTuiAction::SuccessReported {
                message: message.to_owned(),
            });
            return;
        }

        self.renderer.success(message);
    }

    /// Renders a blank line when the active command output supports line-oriented spacing.
    pub fn blank_line(&self) {
        if self.tui.is_some() {
            return;
        }

        self.renderer.blank_line();
    }

    /// Renders a command lifecycle start record.
    pub fn command_start(&self, title: &str, command: &str) {
        if self.tui.is_some() {
            return;
        }

        self.renderer.command_start(title, command);
    }

    /// Renders a command lifecycle progress record.
    pub fn command_delta(&self, content: &str) {
        if self.tui.is_some() {
            return;
        }

        self.renderer.command_delta(content);
    }

    /// Renders a command lifecycle completion record.
    pub fn command_finished(
        &self,
        status: crate::chat::command_event::CommandStatus,
        summary: &str,
    ) {
        if self.tui.is_some() {
            return;
        }

        self.renderer.command_finished(status, summary);
    }

    /// Renders an interactive option selection prompt and returns the user's answer.
    pub async fn ask(
        &self,
        request: SelectionPromptRequest,
    ) -> Result<SelectionPromptAnswer, ChatError> {
        if let Some(tui) = &self.tui {
            return tui.ask(request).await;
        }

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
        if self.tui.is_some() {
            return f.await;
        }

        use std::pin::pin;

        let mut future = pin!(f);
        let mut frame = 0usize;
        let mut interval = tokio::time::interval(Duration::from_millis(90));
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

        self.renderer.working();

        let result = loop {
            tokio::select! {
                _ = interval.tick() => {
                    self.renderer.working_frame(frame, None);
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

impl TuiCommandBridge<'_> {
    /// Dispatches one reducer action into the TUI controller.
    fn dispatch(&self, action: ChatTuiAction) {
        (self.dispatch.borrow_mut())(action);
    }

    /// Converts and dispatches one persisted command lifecycle event.
    fn dispatch_command_event(&self, event: &CommandEvent) {
        let actions = self.adapter.borrow_mut().adapt_command_event(event);
        for action in actions {
            self.dispatch(action);
        }
    }

    /// Replays records into TUI reducer actions without writing to the terminal.
    fn render_records(&self, records: &[ChatRecord], tools: &ToolStorage) {
        let mut adapter = TuiEventAdapter::new();
        for record in records {
            let Some(event) = record.event() else {
                continue;
            };
            if let Some(command_event) = event.to_command_event() {
                for action in adapter.adapt_command_event(&command_event) {
                    self.dispatch(action);
                }
                continue;
            }
            let Some(agent_event) = event.to_agent_event() else {
                continue;
            };
            for action in adapter.adapt_agent_event_with_tools(&agent_event, tools) {
                self.dispatch(action);
            }
        }
    }

    /// Renders a compact session history listing as TUI notices.
    fn render_history(&self, table: &HistoryTableModel) {
        self.dispatch(ChatTuiAction::NoticeReported {
            message: "sessions".to_owned(),
        });
        for row in &table.rows {
            let marker = if row.corrupt { "*" } else { "" };
            self.dispatch(ChatTuiAction::NoticeReported {
                message: format!(
                    "{}  {}  {}  {}{}",
                    row.id, row.updated, row.title, row.messages, marker
                ),
            });
        }
        if table.remaining > 0 {
            self.dispatch(ChatTuiAction::NoticeReported {
                message: format!("{} more sessions", table.remaining),
            });
        }
    }

    /// Projects a command-owned selection prompt into TUI state and awaits the answer.
    async fn ask(
        &self,
        request: SelectionPromptRequest,
    ) -> Result<SelectionPromptAnswer, ChatError> {
        if request.options.is_empty() && !request.allow_custom {
            return Err(ChatError::Session(
                "selection prompt requires an option or custom input".to_owned(),
            ));
        }

        self.dispatch(ChatTuiAction::SelectionPromptChanged(Some(
            selection_state_from_request(&request),
        )));

        loop {
            let intent = {
                let mut receiver = self.selection_receiver.borrow_mut();
                receiver.recv().await
            };
            match intent {
                Some(Intent::SelectionPromptSubmitted(answer)) => {
                    self.dispatch(ChatTuiAction::SelectionPromptSubmitted(answer.clone()));
                    return Ok(selection_answer_from_tui(answer));
                }
                Some(Intent::SelectionPromptCancelled) | None => {
                    self.dispatch(ChatTuiAction::SelectionPromptCancelled);
                    return Err(ChatError::Exit);
                }
                Some(_) => {}
            }
        }
    }
}

/// Converts a command-owned selection request into reducer-owned TUI state.
fn selection_state_from_request(
    request: &SelectionPromptRequest,
) -> spectacular_tui::SelectionPromptState {
    spectacular_tui::SelectionPromptState::new(
        request.title.clone(),
        request.description.clone(),
        request.options.clone(),
    )
    .with_inputs(request.allow_custom, request.allow_comment)
}

/// Converts a TUI selection answer back into the command-owned answer type.
fn selection_answer_from_tui(
    answer: spectacular_tui::SelectionPromptAnswer,
) -> SelectionPromptAnswer {
    SelectionPromptAnswer {
        choice: match answer.choice {
            spectacular_tui::SelectionPromptChoice::Option { index, label } => {
                crate::chat::prompt::SelectionPromptChoice::Option { index, label }
            }
            spectacular_tui::SelectionPromptChoice::Custom(value) => {
                crate::chat::prompt::SelectionPromptChoice::Custom(value)
            }
        },
        comment: answer.comment,
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
