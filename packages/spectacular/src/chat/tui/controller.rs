use super::runner::{AgentRunner, TurnRunner};
use super::state::{initial, session_created_action};
use crate::chat::commands::{
    self, ChatCommandAdapter, ChatCommandContext, ChatCommandControl, ChatCommandResult,
};
use crate::chat::model::{ChatModel, ChatRunRequestModel};
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
            Intent::SubmitPrompt { id, text } => {
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
            Intent::CancelRun => {
                self.runner.cancel();
                if !self.shell.state().status.is_cancellable() {
                    self.shell.apply_action(ChatTuiAction::AgentStarted);
                }
                self.shell.apply_action(ChatTuiAction::CancelRun);
                Ok(false)
            }
            Intent::SelectionPromptSubmitted(answer) => {
                self.shell
                    .apply_action(ChatTuiAction::SelectionPromptSubmitted(answer));
                Ok(false)
            }
            Intent::SelectionPromptCancelled => {
                self.shell
                    .apply_action(ChatTuiAction::SelectionPromptCancelled);
                Ok(false)
            }
            Intent::RequestExit => Ok(true),
        }
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
        match parse_line(&text) {
            Ok(ParseOutcome::Command(invocation)) => {
                return self
                    .dispatch_command(
                        invocation,
                        state_sender,
                        cancellation_receiver,
                        selection_receiver,
                    )
                    .await;
            }
            Ok(ParseOutcome::NotCommand) => {}
            Err(error) => {
                self.shell.apply_action(ChatTuiAction::ErrorReported {
                    message: error.to_string(),
                    details: None,
                });
                return Ok(false);
            }
        }

        if !self.model.runtime().is_ready() {
            self.shell.apply_action(ChatTuiAction::ErrorReported {
                message: "configuration is incomplete; run setup commands first".to_owned(),
                details: None,
            });
            return Ok(false);
        }

        let prompt_event_id = id.as_str().to_owned();
        let request = ChatRunRequestModel {
            prompt: text,
            prompt_event_id: Some(prompt_event_id),
            render_user_prompt: false,
            retry_existing_prompt: false,
            runtime: self.model.runtime().clone(),
        };
        self.run_prompt_request(request, Some(id), state_sender, cancellation_receiver)
            .await?;
        Ok(false)
    }

    /// Runs a prepared prompt request through the TUI runtime without command parsing.
    async fn run_prompt_request(
        &mut self,
        request: ChatRunRequestModel,
        prompt_item_id: Option<TranscriptItemId>,
        state_sender: Option<&mpsc::UnboundedSender<State>>,
        cancellation_receiver: &mut mpsc::UnboundedReceiver<()>,
    ) -> Result<(), ChatError> {
        self.refresh_worktree_metadata(state_sender).await;
        if let Some(id) = prompt_item_id {
            self.shell.apply_action(ChatTuiAction::SubmitPrompt {
                id,
                text: request.prompt.clone(),
            });
            self.publish_state(state_sender);
        }
        self.shell.apply_action(ChatTuiAction::AgentStarted);
        self.publish_state(state_sender);
        let run_result = {
            let model = &self.model;
            let tools = &self.tools;
            let runner = &mut self.runner;
            let shell = &mut self.shell;
            let mut dispatch = |action| {
                shell.apply_action(action);
                if let Some(state_sender) = state_sender {
                    let _ = state_sender.send(shell.state().clone());
                }
            };
            runner
                .run(model, tools, request, &mut dispatch, cancellation_receiver)
                .await
        };
        self.refresh_worktree_metadata(state_sender).await;
        run_result?;
        self.model
            .session_manager()
            .save_snapshot(&self.shell.state().session)?;
        Ok(())
    }

    /// Executes a parsed slash command using TUI-safe output and prompt bridges.
    async fn dispatch_command(
        &mut self,
        invocation: CommandInvocation,
        state_sender: Option<&mpsc::UnboundedSender<State>>,
        cancellation_receiver: &mut mpsc::UnboundedReceiver<()>,
        selection_receiver: &mut mpsc::UnboundedReceiver<Intent>,
    ) -> Result<bool, ChatError> {
        let mut control = ChatCommandControl::default();
        let prompt_footer = crate::chat::model::ChatPromptFooterModel::from_runtime_and_usage(
            &self.workspace_root,
            self.model.runtime(),
            self.model.context_token_usage(),
        );
        let result = {
            let shell = &mut self.shell;
            let mut dispatch = |action| {
                shell.apply_action(action);
                if let Some(state_sender) = state_sender {
                    let _ = state_sender.send(shell.state().clone());
                }
            };
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

        let command_succeeded = matches!(&result, ChatCommandResult::Success);
        if let ChatCommandResult::Error(message) = result {
            self.shell.apply_action(ChatTuiAction::ErrorReported {
                message,
                details: None,
            });
        }
        if control.exit_requested() {
            self.shell.apply_action(ChatTuiAction::ExitRequested);
        }
        self.refresh_command_completions();
        self.publish_state(state_sender);
        if command_succeeded && !control.exit_requested() {
            if let Some(request) = control.take_follow_up_prompt() {
                self.run_prompt_request(request, None, state_sender, cancellation_receiver)
                    .await?;
            }
        }
        Ok(control.exit_requested())
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
        self.shell
            .apply_action(ChatTuiAction::WorktreeMetadataChanged(worktree));
        self.publish_state(state_sender);
    }

    /// Publishes the current reducer state for IOCraft rendering when a sender is available.
    fn publish_state(&self, state_sender: Option<&mpsc::UnboundedSender<State>>) {
        if let Some(state_sender) = state_sender {
            let _ = state_sender.send(self.shell.state().clone());
        }
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
