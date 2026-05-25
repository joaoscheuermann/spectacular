use super::*;
use crate::chat::model::{ChatModel, ChatRunRequestModel};
use crate::chat::runner::main_chat_tool_storage;
use crate::chat::session::SessionManager;
use crate::chat::RuntimeSelection;
use spectacular_agent::{AgentEvent, ToolStorage};
use spectacular_config::{ProviderAuthMode, ReasoningLevel};
use spectacular_llms::{FinishReason, LlmDebugLogger};
use spectacular_tui::{
    merge_controller_state_update, ChatTuiAction, DisplayMetadata, Intent, PromptState,
    SessionId, State, TranscriptItemContent, TranscriptItemId,
};
use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::{mpsc, oneshot};

#[path = "tui_runtime/cancellation.rs"]
mod cancellation;

/// Verifies TUI runtime state starts from controller-owned metadata and warnings.
#[test]
fn initial_state_uses_runtime_metadata_and_warnings() {
    let bootstrap = TestTuiBootstrap::create("session-1").with_warning("configuration warning");
    let controller = TuiRuntimeController::new(bootstrap).unwrap();

    let state = controller.state();

    assert!(!state.display.session_label.is_empty());
    assert_eq!(state.display.provider_label, "provider");
    assert_eq!(state.display.model_label, "model");
    assert_eq!(state.display.current_directory, "/workspace");
    assert_eq!(state.display.worktree, None);
    assert_eq!(
        state.display.context_usage,
        Some(spectacular_tui::ContextTokenUsage::new(0, Some(128_000)))
    );
    assert!(matches!(
        &state.session.transcript[0].content,
        TranscriptItemContent::OpeningBanner(banner)
            if banner.model == "model"
                && banner.reasoning == "low"
                && banner.directory == "/workspace"
                && banner.session_id == state.session.id.as_str()
    ));
    assert!(matches!(
        &state.session.transcript[1].content,
        TranscriptItemContent::Notice(notice) if notice.message == "configuration warning"
    ));
    assert!(state
        .commands
        .iter()
        .any(|command| command.name == "provider" && !command.usage.is_empty()));
}

/// Verifies controller metadata updates preserve prompt input owned by the runtime root.
#[test]
fn controller_state_update_preserves_same_session_prompt_input() {
    let mut local_state = State::new(
        SessionId::new("session-1"),
        tui_runtime(),
        DisplayMetadata::new("provider", "model", "low", "/workspace", "session-1", None),
    );
    local_state.session.prompt = PromptState::from_text("draft prompt");
    let mut controller_state = local_state.clone();
    controller_state.display.current_directory = "/workspace/updated".to_owned();
    controller_state.session.prompt = PromptState::empty();

    let (merged, prompt_reset) = merge_controller_state_update(
        &local_state,
        controller_state,
        &PromptState::from_text("draft prompt"),
    );

    assert_eq!(merged.display.current_directory, "/workspace/updated");
    assert_eq!(merged.session.prompt.text, "draft prompt");
    assert_eq!(prompt_reset, None);
}

/// Verifies session replacement accepts the controller-owned prompt snapshot.
#[test]
fn controller_state_update_resets_prompt_on_session_change() {
    let local_state = State::new(
        SessionId::new("session-1"),
        tui_runtime(),
        DisplayMetadata::new("provider", "model", "low", "/workspace", "session-1", None),
    );
    let mut controller_state = State::new(
        SessionId::new("session-2"),
        tui_runtime(),
        DisplayMetadata::new("provider", "model", "low", "/workspace", "session-2", None),
    );
    controller_state.session.prompt = PromptState::from_text("restored");

    let (merged, prompt_reset) = merge_controller_state_update(
        &local_state,
        controller_state,
        &PromptState::from_text("draft"),
    );

    assert_eq!(merged.session.id, SessionId::new("session-2"));
    assert_eq!(merged.session.prompt.text, "restored");
    assert_eq!(prompt_reset, Some(PromptState::from_text("restored")));
}

/// Verifies streaming runtime actions are published before the full prompt run completes.
#[tokio::test]
async fn controller_publishes_state_while_prompt_run_is_streaming() {
    let (release_sender, release_receiver) = oneshot::channel();
    let bootstrap = TestTuiBootstrap::create("streaming-session");
    let controller = TuiRuntimeController::new_with_runner(
        bootstrap,
        PausingTuiTurnRunner {
            release: Some(release_receiver),
        },
    )
    .unwrap();
    let (intent_sender, intent_receiver) = mpsc::unbounded_channel();
    let (_cancellation_sender, cancellation_receiver) = mpsc::unbounded_channel();
    let (_selection_sender, selection_receiver) = mpsc::unbounded_channel();
    let (state_sender, mut state_receiver) = mpsc::unbounded_channel();
    let controller_loop = run_controller_loop(
        controller,
        intent_receiver,
        cancellation_receiver,
        selection_receiver,
        state_sender,
    );
    let driver = async move {
        intent_sender
            .send(Intent::SubmitPrompt {
                id: TranscriptItemId::new("prompt-1"),
                text: "stream please".to_owned(),
            })
            .unwrap();

        let streamed_state = next_state_matching(&mut state_receiver, |state| {
            state.session.transcript.iter().any(|item| {
                matches!(
                    &item.content,
                    TranscriptItemContent::AssistantMessage(message)
                        if message.text == "streamed before completion"
                )
            })
        })
        .await;

        assert!(streamed_state.is_some());
        release_sender.send(()).unwrap();
        intent_sender.send(Intent::RequestExit).unwrap();
    };
    let (controller_result, _) = tokio::join!(controller_loop, driver);
    controller_result.unwrap();
}

/// Verifies TUI submit intents use the injected turn runner and reducer state.
#[tokio::test]
async fn submit_prompt_intent_runs_real_controller_path() {
    let mut runner = RecordingTuiTurnRunner::default();
    runner.events.extend([
        AgentEvent::message_start("message-1"),
        AgentEvent::message_delta("message-1", "hello from runtime"),
        AgentEvent::message_finish("message-1"),
        AgentEvent::Finished {
            finish_reason: FinishReason::Stop,
        },
    ]);
    let bootstrap = TestTuiBootstrap::create("session-1");
    let mut controller = TuiRuntimeController::new_with_runner(bootstrap, runner).unwrap();

    controller
        .handle_intent(Intent::SubmitPrompt {
            id: TranscriptItemId::new("prompt-1"),
            text: "hello runtime".to_owned(),
        })
        .await
        .unwrap();

    assert_eq!(
        controller.runner().requests,
        vec!["hello runtime".to_owned()]
    );
    assert_eq!(
        controller.runner().prompt_event_ids,
        vec![Some("prompt-1".to_owned())]
    );
    assert!(controller.state().session.transcript.iter().any(|item| {
        matches!(
            &item.content,
            TranscriptItemContent::AssistantMessage(message) if message.text == "hello from runtime"
        )
    }));
}

/// Verifies TUI retry commands run the latest prompt through the TUI runner path.
#[tokio::test]
async fn retry_command_intent_runs_latest_prompt_through_tui_runner() {
    let mut runner = RecordingTuiTurnRunner::default();
    runner.events.push(AgentEvent::Finished {
        finish_reason: FinishReason::Stop,
    });
    let bootstrap = TestTuiBootstrap::create("retry-session");
    let mut controller = TuiRuntimeController::new_with_runner(bootstrap, runner).unwrap();
    controller
        .model()
        .append_agent_event(&AgentEvent::user_prompt("retry this"))
        .unwrap();

    let should_exit = controller
        .handle_intent(Intent::SubmitPrompt {
            id: TranscriptItemId::new("prompt-1"),
            text: "/retry".to_owned(),
        })
        .await
        .unwrap();

    assert!(!should_exit);
    assert_eq!(controller.runner().requests, vec!["retry this".to_owned()]);
    assert_eq!(controller.runner().prompt_event_ids, vec![None]);
    assert_eq!(controller.runner().retry_existing_prompts, vec![true]);
    assert!(controller.state().session.transcript.iter().any(|item| {
        matches!(
            &item.content,
            TranscriptItemContent::Notice(notice) if notice.message == "retrying latest prompt..."
        )
    }));
    assert!(!controller.state().session.transcript.iter().any(|item| {
        matches!(
            &item.content,
            TranscriptItemContent::Error(error)
                if error.message.contains("nested prompt execution")
        )
    }));
}

/// Verifies slash command control requests propagate through the TUI controller.
#[tokio::test]
async fn exit_command_intent_requests_runtime_exit() {
    let bootstrap = TestTuiBootstrap::create("exit-session");
    let mut controller =
        TuiRuntimeController::new_with_runner(bootstrap, RecordingTuiTurnRunner::default())
            .unwrap();

    let should_exit = controller
        .handle_intent(Intent::SubmitPrompt {
            id: TranscriptItemId::new("prompt-1"),
            text: "/exit".to_owned(),
        })
        .await
        .unwrap();

    assert!(should_exit);
    assert!(controller.state().exit_requested);
}

/// Verifies completed TUI runs persist the durable semantic session snapshot.
#[tokio::test]
async fn completed_tui_run_saves_session_snapshot() {
    let mut runner = RecordingTuiTurnRunner::default();
    runner.events.extend([
        AgentEvent::message_start("message-1"),
        AgentEvent::message_delta("message-1", "snapshot response"),
        AgentEvent::message_finish("message-1"),
        AgentEvent::Finished {
            finish_reason: FinishReason::Stop,
        },
    ]);
    let bootstrap = TestTuiBootstrap::create("snapshot-session");
    let mut controller = TuiRuntimeController::new_with_runner(bootstrap, runner).unwrap();

    controller
        .handle_intent(Intent::SubmitPrompt {
            id: TranscriptItemId::new("prompt-1"),
            text: "save this".to_owned(),
        })
        .await
        .unwrap();

    let snapshot = controller.model().session_manager().load_snapshot().unwrap();
    assert_eq!(snapshot.id, controller.state().session.id);
    assert!(controller.state().session.transcript.iter().any(|item| {
        matches!(
            &item.content,
            TranscriptItemContent::AssistantMessage(message) if message.text == "snapshot response"
        )
    }));
    assert!(snapshot.transcript.iter().any(|item| {
        matches!(
            &item.content,
            TranscriptItemContent::AssistantMessage(message) if message.text == "snapshot response"
        )
    }));
}

/// Verifies the new IOCraft runtime path does not bypass rendering with print macros.
#[test]
fn tui_runtime_path_has_no_direct_terminal_print_macros() {
    let source = [
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/src/chat/tui/controller.rs"
        )),
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/src/chat/tui/launch.rs"
        )),
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/src/chat/tui/runner.rs"
        )),
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/src/chat/tui/state.rs"
        )),
    ]
    .join("\n");

    for forbidden in ["print!(", "println!(", "eprint!(", "eprintln!("] {
        assert!(
            !source.contains(forbidden),
            "found {forbidden} in TUI runtime"
        );
    }
}


#[derive(Default)]
struct RecordingTuiTurnRunner {
    events: Vec<AgentEvent>,
    requests: Vec<String>,
    prompt_event_ids: Vec<Option<String>>,
    retry_existing_prompts: Vec<bool>,
    cancel_count: usize,
    running: bool,
}

impl TuiTurnRunner for RecordingTuiTurnRunner {
    /// Records requests and dispatches configured fake events as adapted TUI actions.
    fn run<'a>(
        &'a mut self,
        model: &'a ChatModel,
        _tools: &'a ToolStorage,
        request: ChatRunRequestModel,
        dispatch: &'a mut (dyn FnMut(ChatTuiAction) + Send),
        _cancellation: &'a mut mpsc::UnboundedReceiver<()>,
    ) -> TuiTurnFuture<'a> {
        Box::pin(async move {
            self.prompt_event_ids.push(request.prompt_event_id.clone());
            self.retry_existing_prompts
                .push(request.retry_existing_prompt);
            self.requests.push(request.prompt);
            let mut adapter = TuiEventAdapter::new();
            for event in self.events.clone() {
                model.append_agent_event(&event)?;
                for action in adapter.adapt_agent_event(&event) {
                    dispatch(action);
                }
            }
            self.running = false;
            Ok(())
        })
    }

    /// Records cancellation requests from the controller.
    fn cancel(&mut self) {
        if self.running {
            self.cancel_count = self.cancel_count.saturating_add(1);
        }
    }
}

struct PausingTuiTurnRunner {
    release: Option<oneshot::Receiver<()>>,
}

struct CancellingTuiTurnRunner;

impl TuiTurnRunner for PausingTuiTurnRunner {
    /// Dispatches one assistant delta, then waits until the test releases completion.
    fn run<'a>(
        &'a mut self,
        model: &'a ChatModel,
        _tools: &'a ToolStorage,
        request: ChatRunRequestModel,
        dispatch: &'a mut (dyn FnMut(ChatTuiAction) + Send),
        _cancellation: &'a mut mpsc::UnboundedReceiver<()>,
    ) -> TuiTurnFuture<'a> {
        let release = self.release.take();
        Box::pin(async move {
            let events = [
                AgentEvent::message_start("message-1"),
                AgentEvent::message_delta("message-1", "streamed before completion"),
            ];
            if let Some(prompt_event_id) = request.prompt_event_id {
                model.append_agent_event(&AgentEvent::user_prompt_with_id(
                    prompt_event_id,
                    request.prompt,
                ))?;
            }
            let mut adapter = TuiEventAdapter::new();
            for event in events {
                model.append_agent_event(&event)?;
                for action in adapter.adapt_agent_event(&event) {
                    dispatch(action);
                }
            }
            if let Some(release) = release {
                let _ = release.await;
            }
            for action in adapter.adapt_agent_event(&AgentEvent::Finished {
                finish_reason: FinishReason::Stop,
            }) {
                dispatch(action);
            }
            Ok(())
        })
    }

    /// Records no cancellation behavior for the streaming publication test.
    fn cancel(&mut self) {}
}

impl TuiTurnRunner for CancellingTuiTurnRunner {
    /// Waits for cancellation and dispatches the corresponding terminal action.
    fn run<'a>(
        &'a mut self,
        _model: &'a ChatModel,
        _tools: &'a ToolStorage,
        _request: ChatRunRequestModel,
        dispatch: &'a mut (dyn FnMut(ChatTuiAction) + Send),
        cancellation: &'a mut mpsc::UnboundedReceiver<()>,
    ) -> TuiTurnFuture<'a> {
        Box::pin(async move {
            cancellation.recv().await;
            dispatch(ChatTuiAction::AgentCancelled {
                reason: "test cancellation".to_owned(),
            });
            Ok(())
        })
    }

    /// Records no direct cancellation behavior; the signal channel owns cancellation here.
    fn cancel(&mut self) {}
}

/// Receives state snapshots until one matches the requested predicate or times out.
async fn next_state_matching(
    receiver: &mut mpsc::UnboundedReceiver<State>,
    predicate: impl Fn(&State) -> bool,
) -> Option<State> {
    let deadline = tokio::time::sleep(Duration::from_secs(1));
    tokio::pin!(deadline);
    loop {
        tokio::select! {
            _ = &mut deadline => return None,
            state = receiver.recv() => {
                let state = state?;
                if predicate(&state) {
                    return Some(state);
                }
            }
        }
    }
}

struct TestTuiBootstrap;

impl TestTuiBootstrap {
    /// Builds a production bootstrap with isolated session and trace directories.
    fn create(session_id: &str) -> TuiBootstrap {
        let workspace_root = PathBuf::from("/workspace");
        let trace_dir = temp_dir("trace");
        TuiBootstrap {
            session: SessionManager::new_in(temp_dir(session_id)).unwrap(),
            runtime: test_runtime(),
            tools: main_chat_tool_storage(workspace_root.clone(), trace_dir).unwrap(),
            workspace_root,
            debug_logger: LlmDebugLogger::disabled(),
            warnings: Vec::new(),
        }
    }
}

trait TestTuiBootstrapExt {
    /// Adds one warning to a test bootstrap.
    fn with_warning(self, warning: &str) -> Self;
}

impl TestTuiBootstrapExt for TuiBootstrap {
    /// Adds one warning to a test bootstrap.
    fn with_warning(mut self, warning: &str) -> Self {
        self.warnings.push(warning.to_owned());
        self
    }
}

/// Builds a TUI runtime selection for local state merge tests.
fn tui_runtime() -> spectacular_tui::RuntimeSelection {
    spectacular_tui::RuntimeSelection::new(
        "openrouter",
        "provider",
        "model",
        spectacular_tui::ReasoningLevel::Low,
        Some(128_000),
    )
}

/// Builds a runtime selection for TUI runtime tests.
fn test_runtime() -> RuntimeSelection {
    RuntimeSelection {
        provider_type: "openrouter".to_owned(),
        provider_auth: Some(ProviderAuthMode::ApiKey),
        provider: "provider".to_owned(),
        api_key: "sk-or-v1-test".to_owned(),
        model_key: "test-model".to_owned(),
        model: "model".to_owned(),
        reasoning: ReasoningLevel::Low,
        context_window_tokens: Some(128_000),
    }
}

/// Builds a unique temporary directory path for a test case.
fn temp_dir(name: &str) -> PathBuf {
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();

    std::env::temp_dir().join(format!("spectacular-tui-runtime-{name}-{suffix}"))
}
