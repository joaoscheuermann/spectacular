use super::*;
use crate::chat::runner::main_chat_tool_storage;
use spectacular_agent::{AgentEvent, ToolStorage};
use spectacular_config::{ProviderAuthMode, ReasoningLevel};
use spectacular_llms::FinishReason;
use spectacular_tui::{
    RuntimeIntent, SelectionPromptChoice as TuiSelectionPromptChoice, State, TranscriptItemContent,
    TranscriptItemId,
};
use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::{mpsc, oneshot};

/// Verifies TUI runtime state starts from controller-owned metadata and warnings.
#[test]
fn initial_state_uses_runtime_metadata_and_warnings() {
    let bootstrap = TestTuiBootstrap::new("session-1").with_warning("configuration warning");
    let controller = TuiRuntimeController::new(bootstrap).unwrap();

    let state = controller.state();

    assert!(!state.display.session_label.is_empty());
    assert_eq!(state.display.provider_label, "provider");
    assert_eq!(state.display.model_label, "model");
    assert_eq!(state.display.current_directory, "/workspace");
    assert!(matches!(
        &state.session.transcript[0].content,
        TranscriptItemContent::Notice(notice) if notice.message == "configuration warning"
    ));
    assert!(state
        .commands
        .iter()
        .any(|command| command.name == "provider" && !command.usage.is_empty()));
}

/// Verifies streaming runtime actions are published before the full prompt run completes.
#[tokio::test]
async fn controller_publishes_state_while_prompt_run_is_streaming() {
    let (release_sender, release_receiver) = oneshot::channel();
    let bootstrap = TestTuiBootstrap::new("streaming-session");
    let controller = TuiRuntimeController::new_with_runner(
        bootstrap,
        PausingTuiTurnRunner {
            release: Some(release_receiver),
        },
    )
    .unwrap();
    let (intent_sender, intent_receiver) = mpsc::unbounded_channel();
    let (_cancellation_sender, cancellation_receiver) = mpsc::unbounded_channel();
    let (state_sender, mut state_receiver) = mpsc::unbounded_channel();
    let controller_task = tokio::spawn(run_controller_loop(
        controller,
        intent_receiver,
        cancellation_receiver,
        state_sender,
    ));

    intent_sender
        .send(RuntimeIntent::SubmitPrompt {
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
    intent_sender.send(RuntimeIntent::RequestExit).unwrap();
    controller_task.await.unwrap().unwrap();
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
    let bootstrap = TestTuiBootstrap::new("session-1");
    let mut controller = TuiRuntimeController::new_with_runner(bootstrap, runner).unwrap();

    controller
        .handle_intent(RuntimeIntent::SubmitPrompt {
            id: TranscriptItemId::new("prompt-1"),
            text: "hello runtime".to_owned(),
        })
        .await
        .unwrap();

    assert_eq!(controller.runner().requests, vec!["hello runtime".to_owned()]);
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
    let bootstrap = TestTuiBootstrap::new("snapshot-session");
    let mut controller = TuiRuntimeController::new_with_runner(bootstrap, runner).unwrap();

    controller
        .handle_intent(RuntimeIntent::SubmitPrompt {
            id: TranscriptItemId::new("prompt-1"),
            text: "save this".to_owned(),
        })
        .await
        .unwrap();

    let snapshot = controller.model.session_manager().load_snapshot().unwrap();
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
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/src/chat/tui_runtime.rs"
    ));

    for forbidden in ["print!(", "println!(", "eprint!(", "eprintln!("] {
        assert!(!source.contains(forbidden), "found {forbidden} in TUI runtime");
    }
}

/// Verifies TUI cancellation intents cancel active runtime work through the runner seam.
#[tokio::test]
async fn cancel_intent_cancels_active_runner() {
    let bootstrap = TestTuiBootstrap::new("session-1");
    let mut controller = TuiRuntimeController::new_with_runner(
        bootstrap,
        RecordingTuiTurnRunner {
            running: true,
            ..RecordingTuiTurnRunner::default()
        },
    )
    .unwrap();

    controller.handle_intent(RuntimeIntent::CancelRun).await.unwrap();

    assert_eq!(controller.runner().cancel_count, 1);
    assert_eq!(controller.state().status, spectacular_tui::Status::Cancelling);
}

/// Verifies slash-command execution can request a TUI-owned selection prompt.
#[tokio::test]
async fn tui_command_can_request_selection_prompt() {
    let bootstrap = TestTuiBootstrap::new("selection-request-session");
    let mut controller = TuiRuntimeController::new_with_runner(
        bootstrap,
        RecordingTuiTurnRunner::default(),
    )
    .unwrap();

    controller
        .handle_intent(RuntimeIntent::SubmitPrompt {
            id: TranscriptItemId::new("prompt-1"),
            text: "/git commit".to_owned(),
        })
        .await
        .unwrap();

    let selection = controller.state().selection.as_ref().unwrap();
    assert_eq!(selection.title, "Use generated commit message?");
    assert_eq!(
        selection.options,
        vec!["Use generated message", "Cancel commit"]
    );
    assert!(selection.allow_custom);
    assert!(!selection.allow_comment);
}

/// Verifies TUI selection answers resume the command path waiting on that prompt.
#[tokio::test]
async fn tui_selection_answer_returns_to_waiting_runtime_flow() {
    let bootstrap = TestTuiBootstrap::new("selection-answer-session");
    let mut controller = TuiRuntimeController::new_with_runner(
        bootstrap,
        RecordingTuiTurnRunner::default(),
    )
    .unwrap();

    controller
        .handle_intent(RuntimeIntent::SubmitPrompt {
            id: TranscriptItemId::new("prompt-1"),
            text: "/git commit".to_owned(),
        })
        .await
        .unwrap();
    controller
        .handle_intent(RuntimeIntent::SelectionPromptSubmitted(
            spectacular_tui::SelectionPromptAnswer {
                choice: TuiSelectionPromptChoice::Option {
                    index: 1,
                    label: "Cancel commit".to_owned(),
                },
                comment: None,
            },
        ))
        .await
        .unwrap();

    assert!(controller.state().selection.is_none());
    assert!(controller.state().session.transcript.iter().any(|item| {
        matches!(
            &item.content,
            TranscriptItemContent::Notice(notice) if notice.message == "commit cancelled"
        )
    }));
}

/// Verifies TUI selection cancellation maps to the original selection prompt exit result.
#[tokio::test]
async fn tui_selection_cancel_maps_to_original_exit_result() {
    let bootstrap = TestTuiBootstrap::new("selection-cancel-session");
    let mut controller = TuiRuntimeController::new_with_runner(
        bootstrap,
        RecordingTuiTurnRunner::default(),
    )
    .unwrap();

    controller
        .handle_intent(RuntimeIntent::SubmitPrompt {
            id: TranscriptItemId::new("prompt-1"),
            text: "/git commit".to_owned(),
        })
        .await
        .unwrap();
    controller
        .handle_intent(RuntimeIntent::SelectionPromptCancelled)
        .await
        .unwrap();

    assert!(controller.state().selection.is_none());
    assert!(controller.state().session.transcript.iter().any(|item| {
        matches!(
            &item.content,
            TranscriptItemContent::Error(error) if error.message == "chat exited"
        )
    }));
}

/// Verifies cancellation reaches an active prompt even while the controller awaits the turn.
#[tokio::test]
async fn cancel_signal_reaches_active_prompt_run() {
    let bootstrap = TestTuiBootstrap::new("active-cancel-session");
    let controller = TuiRuntimeController::new_with_runner(bootstrap, CancellingTuiTurnRunner).unwrap();
    let (intent_sender, intent_receiver) = mpsc::unbounded_channel();
    let (cancellation_sender, cancellation_receiver) = mpsc::unbounded_channel();
    let (state_sender, mut state_receiver) = mpsc::unbounded_channel();
    let controller_task = tokio::spawn(run_controller_loop(
        controller,
        intent_receiver,
        cancellation_receiver,
        state_sender,
    ));

    intent_sender
        .send(RuntimeIntent::SubmitPrompt {
            id: TranscriptItemId::new("prompt-1"),
            text: "cancel me".to_owned(),
        })
        .unwrap();
    cancellation_sender.send(()).unwrap();

    let cancelled_state = next_state_matching(&mut state_receiver, |state| {
        matches!(state.status, spectacular_tui::Status::Idle)
            && state.session.transcript.iter().any(|item| {
                matches!(
                    &item.content,
                    TranscriptItemContent::Cancellation(cancellation)
                        if cancellation.reason == "test cancellation"
                )
            })
    })
    .await;

    assert!(cancelled_state.is_some());
    intent_sender.send(RuntimeIntent::RequestExit).unwrap();
    controller_task.await.unwrap().unwrap();
}

#[derive(Default)]
struct RecordingTuiTurnRunner {
    events: Vec<AgentEvent>,
    requests: Vec<String>,
    prompt_event_ids: Vec<Option<String>>,
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
    fn new(session_id: &str) -> TuiBootstrap {
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
