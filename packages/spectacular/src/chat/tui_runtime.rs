use crate::chat::commands;
use crate::chat::model::{ChatModel, ChatRunRequestModel};
use crate::chat::provider::provider_for_runtime;
use crate::chat::runner::main_chat_agent;
use crate::chat::session::{
    agent_events_from_records, records_before_latest_user_prompt, SessionManager,
};
use crate::chat::tui_adapter::{commands_loaded_action, TuiEventAdapter};
use crate::chat::{ChatBootstrap, ChatError, RuntimeSelection};
use iocraft::prelude::*;
use spectacular_agent::{AgentEvent, Store, ToolStorage};
use spectacular_llms::LlmDebugLogger;
use spectacular_tui::{
    ChatTuiAction, DisplayMetadata, RuntimeIntent, RuntimeShell, SelectionPromptAnswer,
    SelectionPromptChoice, SelectionPromptState, SessionId, State, TranscriptItemContent,
    TranscriptItemId, ASSISTANT_REVEAL_TICK_INTERVAL,
};
use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;

pub(crate) type TuiTurnFuture<'a> =
    Pin<Box<dyn Future<Output = Result<(), ChatError>> + Send + 'a>>;

/// Async seam for executing one TUI chat turn without coupling the TUI to runtime APIs.
pub(crate) trait TuiTurnRunner: Send {
    /// Runs a prompt and sends controller-owned TUI actions to the supplied callback.
    fn run<'a>(
        &'a mut self,
        model: &'a ChatModel,
        tools: &'a ToolStorage,
        request: ChatRunRequestModel,
        dispatch: &'a mut (dyn FnMut(ChatTuiAction) + Send),
        cancellation: &'a mut mpsc::UnboundedReceiver<()>,
    ) -> TuiTurnFuture<'a>;

    /// Cancels the active runtime run if one exists.
    fn cancel(&mut self);
}

/// Controller for the opt-in IOCraft TUI runtime path.
pub(crate) struct TuiRuntimeController<R = AgentTuiTurnRunner> {
    shell: RuntimeShell,
    model: ChatModel,
    tools: ToolStorage,
    runner: R,
    pending_selection: Option<PendingTuiSelectionPrompt>,
}

/// Bootstrap data needed to initialize the TUI runtime path without terminal output.
pub(crate) struct TuiBootstrap {
    pub session: SessionManager,
    pub runtime: RuntimeSelection,
    pub tools: ToolStorage,
    pub workspace_root: PathBuf,
    pub debug_logger: LlmDebugLogger,
    pub warnings: Vec<String>,
}

/// Runtime-backed TUI turn runner that streams real agent events into reducer actions.
#[derive(Default)]
pub(crate) struct AgentTuiTurnRunner {
    active: Option<spectacular_agent::AgentRunStream>,
}

impl TuiRuntimeController<AgentTuiTurnRunner> {
    /// Creates a TUI runtime controller using the production agent turn runner.
    pub(crate) fn new(bootstrap: TuiBootstrap) -> Result<Self, ChatError> {
        Self::new_with_runner(bootstrap, AgentTuiTurnRunner::default())
    }
}

impl<R> TuiRuntimeController<R>
where
    R: TuiTurnRunner,
{
    /// Creates a TUI runtime controller with an explicitly injected turn runner.
    pub(crate) fn new_with_runner(bootstrap: TuiBootstrap, runner: R) -> Result<Self, ChatError> {
        let TuiBootstrap {
            session,
            runtime,
            tools,
            workspace_root,
            debug_logger,
            warnings,
        } = bootstrap;
        let mut model = ChatModel::new_with_debug_logger(session, runtime, debug_logger);
        let _started = model.start_new_session()?;
        let state = initial_state(&model, &workspace_root, warnings);
        let (mut shell, _intents) = RuntimeShell::new(state);
        let command_registry = commands::registry()?;
        shell.apply_action(commands_loaded_action(command_registry.metadata()));
        Ok(Self {
            shell,
            model,
            tools,
            runner,
            pending_selection: None,
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

    /// Handles one user intent emitted by the TUI shell.
    pub(crate) async fn handle_intent(&mut self, intent: RuntimeIntent) -> Result<bool, ChatError> {
        match intent {
            RuntimeIntent::SubmitPrompt { id, text } => {
                let (_cancellation_sender, mut cancellation_receiver) = mpsc::unbounded_channel();
                self.handle_submit_prompt(id, text, None, &mut cancellation_receiver)
                    .await?;
                Ok(false)
            }
            RuntimeIntent::CancelRun => {
                self.runner.cancel();
                if !self.shell.state().status.is_cancellable() {
                    self.shell.apply_action(ChatTuiAction::AgentStarted);
                }
                self.shell.apply_action(ChatTuiAction::CancelRun);
                Ok(false)
            }
            RuntimeIntent::SelectionPromptSubmitted(answer) => {
                self.handle_selection_prompt_submitted(answer);
                Ok(false)
            }
            RuntimeIntent::SelectionPromptCancelled => {
                self.handle_selection_prompt_cancelled();
                Ok(false)
            }
            RuntimeIntent::RequestExit => Ok(true),
        }
    }

    /// Handles one user intent and publishes state while long-running work streams.
    async fn handle_intent_with_state_sender(
        &mut self,
        intent: RuntimeIntent,
        state_sender: &mpsc::UnboundedSender<State>,
        cancellation_receiver: &mut mpsc::UnboundedReceiver<()>,
    ) -> Result<bool, ChatError> {
        match intent {
            RuntimeIntent::SubmitPrompt { id, text } => {
                self.handle_submit_prompt(id, text, Some(state_sender), cancellation_receiver)
                    .await?;
                Ok(false)
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
    ) -> Result<(), ChatError> {
        if self.try_open_tui_selection_prompt(&text) {
            return Ok(());
        }

        if !self.model.runtime().is_ready() {
            self.shell.apply_action(ChatTuiAction::ErrorReported {
                message: "configuration is incomplete; run setup commands first".to_owned(),
                details: None,
            });
            return Ok(());
        }

        self.shell.apply_action(ChatTuiAction::SubmitPrompt {
            id,
            text: text.clone(),
        });
        publish_state(&self.shell, state_sender);
        self.shell.apply_action(ChatTuiAction::AgentStarted);
        publish_state(&self.shell, state_sender);
        let request = ChatRunRequestModel {
            prompt: text,
            render_user_prompt: false,
            retry_existing_prompt: false,
            runtime: self.model.runtime().clone(),
        };
        let mut dispatch = |action| {
            self.shell.apply_action(action);
            publish_state(&self.shell, state_sender);
        };
        self.runner
            .run(
                &self.model,
                &self.tools,
                request,
                &mut dispatch,
                cancellation_receiver,
            )
            .await?;
        self.model
            .session_manager()
            .save_snapshot(&self.shell.state().session)
    }

    /// Opens a controller-owned TUI selection prompt for slash commands that require one.
    fn try_open_tui_selection_prompt(&mut self, text: &str) -> bool {
        if text.trim() != "/git commit" {
            return false;
        }

        self.pending_selection = Some(PendingTuiSelectionPrompt::GitCommitMessage);
        self.shell
            .apply_action(ChatTuiAction::SelectionPromptChanged(Some(
                git_commit_selection_prompt_state(),
            )));
        true
    }

    /// Applies a submitted TUI selection answer to the pending command flow.
    fn handle_selection_prompt_submitted(&mut self, answer: SelectionPromptAnswer) {
        let pending = self.pending_selection.take();
        self.shell
            .apply_action(ChatTuiAction::SelectionPromptSubmitted(answer.clone()));
        if !matches!(pending, Some(PendingTuiSelectionPrompt::GitCommitMessage)) {
            return;
        }

        if is_git_commit_cancel_selection(&answer) {
            self.shell.apply_action(ChatTuiAction::NoticeReported {
                message: "commit cancelled".to_owned(),
            });
        }
    }

    /// Applies a TUI selection cancellation using the original prompt exit result.
    fn handle_selection_prompt_cancelled(&mut self) {
        self.pending_selection = None;
        self.shell
            .apply_action(ChatTuiAction::SelectionPromptCancelled);
        self.shell.apply_action(ChatTuiAction::ErrorReported {
            message: ChatError::Exit.to_string(),
            details: None,
        });
    }
}

/// Pending command-side prompt currently waiting for a TUI answer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PendingTuiSelectionPrompt {
    GitCommitMessage,
}

/// Builds the TUI state that mirrors the original `/git commit` selection prompt.
fn git_commit_selection_prompt_state() -> SelectionPromptState {
    SelectionPromptState::new(
        "Use generated commit message?",
        "Message: \"\"",
        vec![
            "Use generated message".to_owned(),
            "Cancel commit".to_owned(),
        ],
    )
    .with_inputs(true, false)
}

/// Returns whether the selection answer chooses the original cancel option.
fn is_git_commit_cancel_selection(answer: &SelectionPromptAnswer) -> bool {
    matches!(
        &answer.choice,
        SelectionPromptChoice::Option { index: 1, .. }
    )
}

/// Runs the production IOCraft render loop with the real Spectacular runtime controller.
pub(crate) async fn run_iocraft_tui(debug_logger: LlmDebugLogger) -> Result<(), ChatError> {
    let bootstrap = TuiBootstrap::from_chat_bootstrap(ChatBootstrap::new(debug_logger)?)?;
    run_iocraft_tui_with_controller(TuiRuntimeController::new(bootstrap)?).await
}

/// Coordinates IOCraft-rendered terminal UI with controller-owned async runtime work.
async fn run_iocraft_tui_with_controller<R>(
    controller: TuiRuntimeController<R>,
) -> Result<(), ChatError>
where
    R: TuiTurnRunner + 'static,
{
    let (intent_sender, intent_receiver) = mpsc::unbounded_channel();
    let (cancellation_sender, cancellation_receiver) = mpsc::unbounded_channel();
    let (state_sender, state_receiver) = mpsc::unbounded_channel();
    let initial_state = controller.state_snapshot();
    let state_receiver = Arc::new(Mutex::new(state_receiver));
    let runtime = tokio::runtime::Handle::current();
    let controller_task = tokio::spawn(run_controller_loop(
        controller,
        intent_receiver,
        cancellation_receiver,
        state_sender,
    ));
    let render_result = tokio::task::spawn_blocking(move || {
        runtime.block_on(async move {
            element!(TuiRuntimeRoot(
                initial_state,
                intent_sender,
                cancellation_sender,
                state_receiver
            ))
            .fullscreen()
            .await
        })
    })
    .await
    .map_err(|error| ChatError::Session(error.to_string()))?;
    render_result.map_err(ChatError::Io)?;
    controller_task
        .await
        .map_err(|error| ChatError::Session(error.to_string()))?
}

/// Processes intents emitted by the IOCraft shell and publishes reducer snapshots for rendering.
async fn run_controller_loop<R>(
    mut controller: TuiRuntimeController<R>,
    mut intent_receiver: mpsc::UnboundedReceiver<RuntimeIntent>,
    mut cancellation_receiver: mpsc::UnboundedReceiver<()>,
    state_sender: mpsc::UnboundedSender<State>,
) -> Result<(), ChatError>
where
    R: TuiTurnRunner,
{
    while let Some(intent) = intent_receiver.recv().await {
        let should_exit = controller
            .handle_intent_with_state_sender(intent, &state_sender, &mut cancellation_receiver)
            .await?;
        let _ = state_sender.send(controller.state_snapshot());
        if should_exit {
            return Ok(());
        }
    }
    Ok(())
}

/// Publishes the current reducer state for IOCraft rendering when a sender is available.
fn publish_state(shell: &RuntimeShell, state_sender: Option<&mpsc::UnboundedSender<State>>) {
    if let Some(state_sender) = state_sender {
        let _ = state_sender.send(shell.state().clone());
    }
}

/// Bridges IOCraft terminal events to runtime intents while rendering state through App.
#[component]
fn TuiRuntimeRoot(mut hooks: Hooks, props: &TuiRuntimeRootProps) -> impl Into<AnyElement<'static>> {
    let initial_state = props
        .initial_state
        .as_ref()
        .expect("TuiRuntimeRoot requires initial state")
        .clone();
    let intent_sender = props
        .intent_sender
        .as_ref()
        .expect("TuiRuntimeRoot requires intent sender")
        .clone();
    let cancellation_sender = props
        .cancellation_sender
        .as_ref()
        .expect("TuiRuntimeRoot requires cancellation sender")
        .clone();
    let state_receiver = props
        .state_receiver
        .as_ref()
        .expect("TuiRuntimeRoot requires state receiver")
        .clone();
    let mut system = hooks.use_context_mut::<SystemContext>();
    let local_state = hooks.use_state(|| initial_state);
    let exit_requested = hooks.use_state(|| false);
    synchronize_runtime_state(&mut hooks, local_state, state_receiver);
    reveal_assistant_streams(&mut hooks, local_state);
    emit_terminal_intents(
        &mut hooks,
        local_state,
        exit_requested,
        intent_sender,
        cancellation_sender,
    );
    if exit_requested.get() {
        system.exit();
    }
    let state = local_state.read().clone();
    element!(spectacular_tui::components::AppState(state))
}

/// Props for the IOCraft runtime bridge component.
#[derive(Default, Props)]
struct TuiRuntimeRootProps {
    initial_state: Option<State>,
    intent_sender: Option<mpsc::UnboundedSender<RuntimeIntent>>,
    cancellation_sender: Option<mpsc::UnboundedSender<()>>,
    state_receiver: Option<Arc<Mutex<mpsc::UnboundedReceiver<State>>>>,
}

/// Polls controller-published state snapshots and refreshes local IOCraft state.
fn synchronize_runtime_state(
    hooks: &mut Hooks,
    mut local_state: iocraft::prelude::State<State>,
    state_receiver: Arc<Mutex<mpsc::UnboundedReceiver<State>>>,
) {
    hooks.use_future(async move {
        loop {
            apply_state_updates(&mut local_state, &state_receiver);
            tokio::time::sleep(std::time::Duration::from_millis(16)).await;
        }
    });
}

/// Applies all pending state snapshots from the runtime controller.
fn apply_state_updates(
    local_state: &mut iocraft::prelude::State<State>,
    state_receiver: &Arc<Mutex<mpsc::UnboundedReceiver<State>>>,
) {
    loop {
        let Ok(state) = state_receiver
            .lock()
            .expect("TUI state receiver lock poisoned")
            .try_recv()
        else {
            return;
        };
        local_state.set(state);
    }
}

/// Advances assistant typewriter reveal state at the documented cadence.
fn reveal_assistant_streams(hooks: &mut Hooks, mut local_state: iocraft::prelude::State<State>) {
    hooks.use_future(async move {
        loop {
            tokio::time::sleep(ASSISTANT_REVEAL_TICK_INTERVAL).await;
            let state = local_state.read().clone();
            let (mut shell, _intents) = RuntimeShell::new(state.clone());
            shell.apply_assistant_reveal_tick();
            if shell.state() != &state {
                local_state.set(shell.state().clone());
            }
        }
    });
}

/// Registers terminal input handling that emits runtime intents without performing side effects.
fn emit_terminal_intents(
    hooks: &mut Hooks,
    local_state: iocraft::prelude::State<State>,
    exit_requested: iocraft::prelude::State<bool>,
    intent_sender: mpsc::UnboundedSender<RuntimeIntent>,
    cancellation_sender: mpsc::UnboundedSender<()>,
) {
    hooks.use_terminal_events({
        let mut local_state = local_state;
        let mut exit_requested = exit_requested;
        move |event| {
            let state = local_state.read().clone();
            let (mut shell, mut intents) = RuntimeShell::new(state);
            shell.apply_terminal_event(event);
            while let Ok(intent) = intents.try_recv() {
                match intent {
                    RuntimeIntent::RequestExit => {
                        exit_requested.set(true);
                        let _ = intent_sender.send(RuntimeIntent::RequestExit);
                    }
                    RuntimeIntent::CancelRun => {
                        let _ = cancellation_sender.send(());
                    }
                    intent => {
                        let _ = intent_sender.send(intent);
                    }
                }
            }
            local_state.set(shell.state().clone());
        }
    });
}

impl TuiTurnRunner for AgentTuiTurnRunner {
    /// Runs a real agent stream and maps runtime events into TUI actions.
    fn run<'a>(
        &'a mut self,
        model: &'a ChatModel,
        tools: &'a ToolStorage,
        request: ChatRunRequestModel,
        dispatch: &'a mut (dyn FnMut(ChatTuiAction) + Send),
        cancellation: &'a mut mpsc::UnboundedReceiver<()>,
    ) -> TuiTurnFuture<'a> {
        Box::pin(async move {
            self.run_agent_stream(model, tools, request, dispatch, cancellation)
                .await
        })
    }

    /// Cancels the active agent stream when a TUI cancel intent arrives.
    fn cancel(&mut self) {
        if let Some(stream) = &self.active {
            stream.cancel();
        }
    }
}

impl TuiBootstrap {
    /// Converts the legacy chat bootstrap into IOCraft runtime bootstrap data.
    fn from_chat_bootstrap(bootstrap: ChatBootstrap) -> Result<Self, ChatError> {
        let ChatBootstrap {
            session,
            renderer: _,
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

impl AgentTuiTurnRunner {
    /// Executes the real agent stream while keeping terminal output in the TUI reducer.
    async fn run_agent_stream(
        &mut self,
        model: &ChatModel,
        tools: &ToolStorage,
        request: ChatRunRequestModel,
        dispatch: &mut (dyn FnMut(ChatTuiAction) + Send),
        cancellation: &mut mpsc::UnboundedReceiver<()>,
    ) -> Result<(), ChatError> {
        let agent = main_chat_agent(
            provider_for_runtime(
                &request.runtime,
                model.debug_logger().clone(),
                model.config_io(),
            )?,
            &request.runtime,
            store_for_request(model, &request)?,
            tools.clone(),
        );
        self.active = Some(Arc::new(agent).run_stream(request.prompt));
        let mut adapter = TuiEventAdapter::new();
        while let Some(event) = self.next_event(cancellation).await {
            if let AgentEvent::ContextTokenUsage(usage) = event {
                model.set_context_token_usage(usage);
                for action in adapter
                    .adapt_agent_event_with_tools(&AgentEvent::ContextTokenUsage(usage), tools)
                {
                    dispatch(action);
                }
                continue;
            }
            let is_terminal_cancellation = matches!(event, AgentEvent::Cancelled { .. });
            model.append_agent_event(&event)?;
            for action in adapter.adapt_agent_event_with_tools(&event, tools) {
                dispatch(action);
            }
            if is_terminal_cancellation {
                break;
            }
        }
        self.active = None;
        Ok(())
    }

    /// Receives one event from the active agent stream or maps cancellation into an agent event.
    async fn next_event(
        &mut self,
        cancellation: &mut mpsc::UnboundedReceiver<()>,
    ) -> Option<AgentEvent> {
        let stream = self.active.as_mut()?;
        tokio::select! {
            event = stream.next() => event,
            cancellation = cancellation.recv() => {
                cancellation?;
                stream.cancel();
                Some(AgentEvent::Cancelled {
                    reason: "run cancelled".to_owned(),
                })
            }
        }
    }
}

/// Builds initial TUI state from chat model metadata without direct terminal writes.
fn initial_state(model: &ChatModel, workspace_root: &PathBuf, warnings: Vec<String>) -> State {
    let runtime = spectacular_tui::RuntimeSelection::new(
        model.runtime().provider_type.clone(),
        model.runtime().provider.clone(),
        model.runtime().model.clone(),
        tui_reasoning_level(model.runtime().reasoning),
        model
            .runtime()
            .context_window_tokens
            .map(|value| value as u64),
    );
    let display = DisplayMetadata::new(
        model.runtime().provider.clone(),
        model.runtime().model.clone(),
        model.runtime().reasoning.to_string(),
        workspace_root.to_string_lossy(),
        model.current_session_id(),
        None,
    );
    let mut state = State::new(SessionId::new(model.current_session_id()), runtime, display);
    for warning in warnings {
        let id = TranscriptItemId::new(format!("warning-{}", state.session.transcript.len() + 1));
        let timestamp = state.session.allocate_timestamp();
        state
            .session
            .transcript
            .push(spectacular_tui::TranscriptItem::new(
                id,
                timestamp,
                TranscriptItemContent::Notice(spectacular_tui::NoticeItem::new(warning)),
            ));
    }
    state
}

/// Converts configured reasoning into the TUI display enum.
fn tui_reasoning_level(
    reasoning: spectacular_config::ReasoningLevel,
) -> spectacular_tui::ReasoningLevel {
    match reasoning {
        spectacular_config::ReasoningLevel::None => spectacular_tui::ReasoningLevel::None,
        spectacular_config::ReasoningLevel::Minimal | spectacular_config::ReasoningLevel::Low => {
            spectacular_tui::ReasoningLevel::Low
        }
        spectacular_config::ReasoningLevel::Medium => spectacular_tui::ReasoningLevel::Medium,
        spectacular_config::ReasoningLevel::High | spectacular_config::ReasoningLevel::Xhigh => {
            spectacular_tui::ReasoningLevel::High
        }
    }
}

/// Builds an agent store from the current session records and retry mode.
fn store_for_request(model: &ChatModel, request: &ChatRunRequestModel) -> Result<Store, ChatError> {
    let records = model.records()?;
    let context_records = if request.retry_existing_prompt {
        records_before_latest_user_prompt(&records)
    } else {
        records.as_slice()
    };

    Ok(Store::from(agent_events_from_records(context_records)))
}

#[cfg(test)]
mod tests {
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/unit/chat/tui_runtime.rs"
    ));
}
