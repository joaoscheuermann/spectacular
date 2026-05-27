use super::runner::{AgentRunner, TurnRunner};
use super::state::{initial, session_created_action};
use crate::chat::commands::{
    self, ChatCommandAdapter, ChatCommandContext, ChatCommandControl, ChatCommandResult,
};
use crate::chat::model::{ChatModel, ChatPromptFooterModel, ChatRunRequestModel};
use crate::chat::session::SessionManager;
use crate::chat::worktree::current_worktree_metadata;
use crate::chat::{ChatBootstrap, ChatError, RuntimeSelection};
use spectacular_commands::{parse_line, CommandInvocation, ParseOutcome};
use spectacular_llms::LlmDebugLogger;
use spectacular_tui::{ChatTuiAction, Intent, Shell, State, TranscriptItemId};
use std::path::PathBuf;
use tokio::sync::mpsc;

/// Controller for the opt-in IOCraft TUI runtime path.
pub(crate) struct Controller<R = AgentRunner> {
    shell: Shell,
    model: ChatModel,
    tools: spectacular_agent::ToolStorage,
    commands: ChatCommandAdapter,
    runner: R,
    workspace_root: PathBuf,
}

/// Bootstrap data needed to initialize the TUI runtime path without terminal output.
pub(crate) struct Bootstrap {
    pub session: SessionManager,
    pub runtime: RuntimeSelection,
    pub tools: spectacular_agent::ToolStorage,
    pub workspace_root: PathBuf,
    pub debug_logger: LlmDebugLogger,
    pub warnings: Vec<String>,
}

enum SubmittedPrompt {
    Command(CommandInvocation),
    Prompt {
        request: ChatRunRequestModel,
        item_id: TranscriptItemId,
    },
    Rejected(ChatTuiAction),
}

struct CommandRun {
    result: ChatCommandResult,
    control: ChatCommandControl,
}

impl CommandRun {
    fn succeeded(&self) -> bool {
        matches!(&self.result, ChatCommandResult::Success)
    }
}

impl Controller<AgentRunner> {
    /// Creates a TUI runtime controller using the production agent turn runner.
    pub(crate) fn new(bootstrap: Bootstrap) -> Result<Self, ChatError> {
        Self::new_with_runner(bootstrap, AgentRunner::default())
    }
}

impl<R> Controller<R>
where
    R: TurnRunner,
{
    /// Creates a TUI runtime controller with an explicitly injected turn runner.
    pub(crate) fn new_with_runner(bootstrap: Bootstrap, runner: R) -> Result<Self, ChatError> {
        let Bootstrap {
            session,
            runtime,
            tools,
            workspace_root,
            debug_logger,
            warnings,
        } = bootstrap;
        let mut model = ChatModel::new_with_debug_logger(session, runtime, debug_logger);
        let started = model.start_new_session()?;
        let state = initial(&model, &workspace_root);
        let (mut shell, _intents) = Shell::new(state);
        shell.apply_action(session_created_action(&started.id, &model, &workspace_root));
        for warning in warnings {
            shell.apply_action(ChatTuiAction::NoticeReported { message: warning });
        }
        let commands = commands::registry()?;
        shell.apply_action(super::adapter::commands_loaded_with_completions_action(
            &commands, &model,
        ));
        Ok(Self {
            shell,
            model,
            tools,
            commands,
            runner,
            workspace_root,
        })
    }

    /// Returns the current TUI reducer state for rendering or tests.
    #[cfg(test)]
    pub(crate) fn state(&self) -> &State {
        self.shell.state()
    }

    /// Clones the current TUI reducer state for render-loop synchronization.
    pub(crate) fn state_snapshot(&self) -> State {
        self.shell.state().clone()
    }

    /// Returns the active chat session identifier.
    pub(crate) fn current_session_id(&self) -> &str {
        self.model.current_session_id()
    }

    /// Returns the injected runner for tests that need to inspect runner effects.
    #[cfg(test)]
    pub(crate) fn runner(&self) -> &R {
        &self.runner
    }

    /// Returns the controller-owned chat model for state persistence tests.
    #[cfg(test)]
    pub(crate) fn model(&self) -> &ChatModel {
        &self.model
    }

    /// Handles one user intent emitted by the TUI shell.
    pub(crate) async fn handle_intent(&mut self, intent: Intent) -> Result<bool, ChatError> {
        match intent {
            Intent::SubmitPrompt { id, text } => self.handle_submit_intent(id, text).await,
            Intent::CancelRun => Ok(self.cancel_run()),
            Intent::SelectionPromptSubmitted(answer) => Ok(self.submit_selection_prompt(answer)),
            Intent::SelectionPromptCancelled => Ok(self.cancel_selection_prompt()),
            Intent::RequestExit => Ok(true),
        }
    }

    async fn handle_submit_intent(
        &mut self,
        id: TranscriptItemId,
        text: String,
    ) -> Result<bool, ChatError> {
        let (_cancellation_sender, mut cancellation_receiver) = mpsc::unbounded_channel();
        let (_selection_sender, mut selection_receiver) = mpsc::unbounded_channel();
        self.handle_submit_prompt(
            id,
            text,
            None,
            &mut cancellation_receiver,
            &mut selection_receiver,
        )
        .await
    }

    fn cancel_run(&mut self) -> bool {
        self.runner.cancel();
        if !self.shell.state().status.is_cancellable() {
            self.shell.apply_action(ChatTuiAction::AgentStarted);
        }
        self.shell.apply_action(ChatTuiAction::CancelRun);
        false
    }

    fn submit_selection_prompt(&mut self, answer: spectacular_tui::SelectionPromptAnswer) -> bool {
        self.shell
            .apply_action(ChatTuiAction::SelectionPromptSubmitted(answer));
        false
    }

    fn cancel_selection_prompt(&mut self) -> bool {
        self.shell
            .apply_action(ChatTuiAction::SelectionPromptCancelled);
        false
    }

    /// Handles one user intent and publishes state while long-running work streams.
    pub(super) async fn handle_intent_with_state_sender(
        &mut self,
        intent: Intent,
        state_sender: &mpsc::UnboundedSender<State>,
        cancellation_receiver: &mut mpsc::UnboundedReceiver<()>,
        selection_receiver: &mut mpsc::UnboundedReceiver<Intent>,
    ) -> Result<bool, ChatError> {
        match intent {
            Intent::SubmitPrompt { id, text } => {
                self.handle_submit_prompt(
                    id,
                    text,
                    Some(state_sender),
                    cancellation_receiver,
                    selection_receiver,
                )
                .await
            }
            intent => self.handle_intent(intent).await,
        }
    }

    /// Runs a submitted prompt through the runtime and reduces streamed TUI actions.
    async fn handle_submit_prompt(
        &mut self,
        id: TranscriptItemId,
        text: String,
        state_sender: Option<&mpsc::UnboundedSender<State>>,
        cancellation_receiver: &mut mpsc::UnboundedReceiver<()>,
        selection_receiver: &mut mpsc::UnboundedReceiver<Intent>,
    ) -> Result<bool, ChatError> {
        match self.submitted_prompt(id, text) {
            SubmittedPrompt::Command(invocation) => {
                self.run_submitted_command(
                    invocation,
                    state_sender,
                    cancellation_receiver,
                    selection_receiver,
                )
                .await
            }
            SubmittedPrompt::Prompt { request, item_id } => {
                self.run_submitted_prompt(request, item_id, state_sender, cancellation_receiver)
                    .await
            }
            SubmittedPrompt::Rejected(action) => Ok(self.reject_submission(action)),
        }
    }

    async fn run_submitted_command(
        &mut self,
        invocation: CommandInvocation,
        state_sender: Option<&mpsc::UnboundedSender<State>>,
        cancellation_receiver: &mut mpsc::UnboundedReceiver<()>,
        selection_receiver: &mut mpsc::UnboundedReceiver<Intent>,
    ) -> Result<bool, ChatError> {
        self.dispatch_command(
            invocation,
            state_sender,
            cancellation_receiver,
            selection_receiver,
        )
        .await
    }

    async fn run_submitted_prompt(
        &mut self,
        request: ChatRunRequestModel,
        item_id: TranscriptItemId,
        state_sender: Option<&mpsc::UnboundedSender<State>>,
        cancellation_receiver: &mut mpsc::UnboundedReceiver<()>,
    ) -> Result<bool, ChatError> {
        self.run_prompt_request(request, Some(item_id), state_sender, cancellation_receiver)
            .await?;
        Ok(false)
    }

    fn reject_submission(&mut self, action: ChatTuiAction) -> bool {
        self.shell.apply_action(action);
        false
    }

    fn submitted_prompt(&self, id: TranscriptItemId, text: String) -> SubmittedPrompt {
        match parse_line(&text) {
            Ok(ParseOutcome::Command(invocation)) => SubmittedPrompt::Command(invocation),
            Ok(ParseOutcome::NotCommand) => self.prompt_submission(id, text),
            Err(error) => SubmittedPrompt::Rejected(error_action(error.to_string())),
        }
    }

    fn prompt_submission(&self, id: TranscriptItemId, text: String) -> SubmittedPrompt {
        if !self.model.runtime().is_ready() {
            return SubmittedPrompt::Rejected(error_action(
                "configuration is incomplete; run setup commands first",
            ));
        }

        SubmittedPrompt::Prompt {
            request: ChatRunRequestModel {
                prompt: text,
                prompt_event_id: Some(id.as_str().to_owned()),
                render_user_prompt: false,
                retry_existing_prompt: false,
                runtime: self.model.runtime().clone(),
            },
            item_id: id,
        }
    }

    /// Runs a prepared prompt request through the TUI runtime without command parsing.
    async fn run_prompt_request(
        &mut self,
        request: ChatRunRequestModel,
        prompt_item_id: Option<TranscriptItemId>,
        state_sender: Option<&mpsc::UnboundedSender<State>>,
        cancellation_receiver: &mut mpsc::UnboundedReceiver<()>,
    ) -> Result<(), ChatError> {
        self.begin_prompt_run(&request, prompt_item_id, state_sender);
        let run_result = self
            .run_turn(request, state_sender, cancellation_receiver)
            .await;
        self.refresh_worktree_metadata(state_sender).await;
        run_result?;
        self.save_session_snapshot()
    }

    fn begin_prompt_run(
        &mut self,
        request: &ChatRunRequestModel,
        prompt_item_id: Option<TranscriptItemId>,
        state_sender: Option<&mpsc::UnboundedSender<State>>,
    ) {
        if let Some(id) = prompt_item_id {
            self.apply_action(
                ChatTuiAction::SubmitPrompt {
                    id,
                    text: request.prompt.clone(),
                },
                state_sender,
            );
        }
        self.apply_action(ChatTuiAction::AgentStarted, state_sender);
    }

    async fn run_turn(
        &mut self,
        request: ChatRunRequestModel,
        state_sender: Option<&mpsc::UnboundedSender<State>>,
        cancellation_receiver: &mut mpsc::UnboundedReceiver<()>,
    ) -> Result<(), ChatError> {
        let model = &self.model;
        let tools = &self.tools;
        let runner = &mut self.runner;
        let mut dispatch = reducer_dispatch(&mut self.shell, state_sender);
        runner
            .run(model, tools, request, &mut dispatch, cancellation_receiver)
            .await
    }

    fn save_session_snapshot(&self) -> Result<(), ChatError> {
        self.model
            .session_manager()
            .save_snapshot(&self.shell.state().session)
    }

    /// Executes a parsed slash command using TUI-safe output and prompt bridges.
    async fn dispatch_command(
        &mut self,
        invocation: CommandInvocation,
        state_sender: Option<&mpsc::UnboundedSender<State>>,
        cancellation_receiver: &mut mpsc::UnboundedReceiver<()>,
        selection_receiver: &mut mpsc::UnboundedReceiver<Intent>,
    ) -> Result<bool, ChatError> {
        let mut run = self
            .execute_command(invocation, state_sender, selection_receiver)
            .await?;
        let command_succeeded = run.succeeded();
        let should_exit = run.control.exit_requested();

        self.apply_command_result(run.result, should_exit);
        self.refresh_command_completions();
        self.publish_state(state_sender);
        if command_succeeded && !should_exit {
            self.run_follow_up_prompt(&mut run.control, state_sender, cancellation_receiver)
                .await?;
        }
        Ok(should_exit)
    }

    async fn execute_command(
        &mut self,
        invocation: CommandInvocation,
        state_sender: Option<&mpsc::UnboundedSender<State>>,
        selection_receiver: &mut mpsc::UnboundedReceiver<Intent>,
    ) -> Result<CommandRun, ChatError> {
        let mut control = ChatCommandControl::default();
        let prompt_footer = self.prompt_footer();
        let result = {
            let mut dispatch = reducer_dispatch(&mut self.shell, state_sender);
            let context = ChatCommandContext::new_tui(
                &mut self.model,
                &self.tools,
                &mut control,
                Some(prompt_footer),
                &mut dispatch,
                selection_receiver,
            );
            self.commands.execute(context, invocation).await
        };

        Ok(CommandRun { result, control })
    }

    fn prompt_footer(&self) -> ChatPromptFooterModel {
        ChatPromptFooterModel::from_runtime_and_usage(
            &self.workspace_root,
            self.model.runtime(),
            self.model.context_token_usage(),
        )
    }

    fn apply_command_result(&mut self, result: ChatCommandResult, should_exit: bool) {
        if let ChatCommandResult::Error(message) = result {
            self.shell.apply_action(error_action(message));
        }
        if should_exit {
            self.shell.apply_action(ChatTuiAction::ExitRequested);
        }
    }

    async fn run_follow_up_prompt(
        &mut self,
        control: &mut ChatCommandControl,
        state_sender: Option<&mpsc::UnboundedSender<State>>,
        cancellation_receiver: &mut mpsc::UnboundedReceiver<()>,
    ) -> Result<(), ChatError> {
        let Some(request) = control.take_follow_up_prompt() else {
            return Ok(());
        };

        self.run_prompt_request(request, None, state_sender, cancellation_receiver)
            .await
    }

    fn apply_action(
        &mut self,
        action: ChatTuiAction,
        state_sender: Option<&mpsc::UnboundedSender<State>>,
    ) {
        self.shell.apply_action(action);
        self.publish_state(state_sender);
    }

    /// Refreshes reducer-safe command completion metadata after command-owned config changes.
    fn refresh_command_completions(&mut self) {
        self.shell
            .apply_action(super::adapter::commands_loaded_with_completions_action(
                &self.commands,
                &self.model,
            ));
    }

    /// Refreshes display-safe worktree metadata from the controller-owned workspace.
    pub(super) async fn refresh_worktree_metadata(
        &mut self,
        state_sender: Option<&mpsc::UnboundedSender<State>>,
    ) {
        let workspace_root = self.workspace_root.clone();
        let worktree =
            tokio::task::spawn_blocking(move || current_worktree_metadata(&workspace_root))
                .await
                .unwrap_or(None);
        self.apply_action(
            ChatTuiAction::WorktreeMetadataChanged(worktree),
            state_sender,
        );
    }

    /// Publishes the current reducer state for IOCraft rendering when a sender is available.
    fn publish_state(&self, state_sender: Option<&mpsc::UnboundedSender<State>>) {
        if let Some(state_sender) = state_sender {
            let _ = state_sender.send(self.shell.state().clone());
        }
    }
}

fn reducer_dispatch<'a>(
    shell: &'a mut Shell,
    state_sender: Option<&'a mpsc::UnboundedSender<State>>,
) -> impl FnMut(ChatTuiAction) + Send + 'a {
    move |action| {
        shell.apply_action(action);
        if let Some(state_sender) = state_sender {
            let _ = state_sender.send(shell.state().clone());
        }
    }
}

fn error_action(message: impl Into<String>) -> ChatTuiAction {
    ChatTuiAction::ErrorReported {
        message: message.into(),
        details: None,
    }
}

impl Bootstrap {
    /// Converts chat bootstrap data into IOCraft runtime bootstrap data.
    pub(super) fn from_chat_bootstrap(bootstrap: ChatBootstrap) -> Result<Self, ChatError> {
        let ChatBootstrap {
            session,
            runtime,
            tools,
            workspace_root,
            debug_logger,
            warnings,
        } = bootstrap;
        Ok(Self {
            session,
            runtime,
            tools,
            workspace_root,
            debug_logger,
            warnings,
        })
    }
}
