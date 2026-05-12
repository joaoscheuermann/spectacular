use super::*;
use crate::chat::commands;
use crate::chat::model::ChatRunRequestModel;
use crate::chat::runner::ChatTurnFuture;
use crate::chat::session::SessionManager;
use crate::chat::RuntimeSelection;
use spectacular_agent::AgentEvent;
use spectacular_config::ReasoningLevel;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

/// Verifies that retry command runs prompt during dispatch.
#[tokio::test]
async fn retry_command_runs_prompt_during_dispatch() {
    let recorded = Arc::new(Mutex::new(None));
    let model = test_model();
    model
        .append_agent_event(&AgentEvent::UserPrompt {
            content: "again".to_owned(),
        })
        .unwrap();
    let mut controller = ChatController::with_runner(
        model,
        commands::registry().unwrap(),
        Renderer::default(),
        ToolStorage::default(),
        test_workspace_root(),
        RecordingRunner {
            recorded: Arc::clone(&recorded),
        },
    );

    controller
        .dispatch_command(CommandInvocation {
            name: "retry".to_owned(),
            args: Vec::new(),
        })
        .await
        .unwrap();

    assert!(recorded
        .lock()
        .unwrap()
        .as_ref()
        .is_some_and(|request| request.retry_existing_prompt));
}

/// Verifies that retry runner error continues repl.
#[tokio::test]
async fn retry_runner_error_continues_repl() {
    let model = test_model();
    model
        .append_agent_event(&AgentEvent::UserPrompt {
            content: "again".to_owned(),
        })
        .unwrap();
    let mut controller = ChatController::with_runner(
        model,
        commands::registry().unwrap(),
        Renderer::default(),
        ToolStorage::default(),
        test_workspace_root(),
        FailingRunner,
    );

    let control = controller
        .handle_line("/retry".to_owned())
        .await
        .expect("follow-up runner failures should stay inside the REPL");

    assert_eq!(control, CommandControl::Continue);
}

/// Verifies that prompt dispatch runs user prompt request.
#[tokio::test]
async fn prompt_dispatch_runs_user_prompt_request() {
    let recorded = Arc::new(Mutex::new(None));
    let mut controller = ChatController::with_runner(
        test_model(),
        commands::registry().unwrap(),
        Renderer::default(),
        ToolStorage::default(),
        test_workspace_root(),
        RecordingRunner {
            recorded: Arc::clone(&recorded),
        },
    );

    controller
        .dispatch_prompt("hello".to_owned())
        .await
        .unwrap();

    let request = recorded
        .lock()
        .unwrap()
        .clone()
        .expect("normal prompts should run through the runner");

    assert_eq!(request.prompt, "hello");
    assert!(request.render_user_prompt);
    assert!(!request.retry_existing_prompt);
    assert_eq!(request.runtime, test_runtime());
}

/// Verifies that setup runtime blocks prompt without runner call.
#[tokio::test]
async fn setup_runtime_blocks_prompt_without_runner_call() {
    let recorded = Arc::new(Mutex::new(None));
    let mut controller = ChatController::with_runner(
        setup_model(),
        commands::registry().unwrap(),
        Renderer::default(),
        ToolStorage::default(),
        test_workspace_root(),
        RecordingRunner {
            recorded: Arc::clone(&recorded),
        },
    );

    controller
        .dispatch_prompt("hello".to_owned())
        .await
        .unwrap();

    assert!(recorded.lock().unwrap().is_none());
}

/// Verifies that blank line is ignored without runner call.
#[tokio::test]
async fn blank_line_is_ignored_without_runner_call() {
    let recorded = Arc::new(Mutex::new(None));
    let mut controller = ChatController::with_runner(
        test_model(),
        commands::registry().unwrap(),
        Renderer::default(),
        ToolStorage::default(),
        test_workspace_root(),
        RecordingRunner {
            recorded: Arc::clone(&recorded),
        },
    );

    let control = controller.handle_line("   \n".to_owned()).await.unwrap();

    assert!(control == CommandControl::Continue && recorded.lock().unwrap().is_none());
}

/// Verifies that command parse error continues repl without runner call.
#[tokio::test]
async fn command_parse_error_continues_repl_without_runner_call() {
    let recorded = Arc::new(Mutex::new(None));
    let mut controller = ChatController::with_runner(
        test_model(),
        commands::registry().unwrap(),
        Renderer::default(),
        ToolStorage::default(),
        test_workspace_root(),
        RecordingRunner {
            recorded: Arc::clone(&recorded),
        },
    );

    let control = controller.handle_line("/".to_owned()).await.unwrap();

    assert!(control == CommandControl::Continue && recorded.lock().unwrap().is_none());
}

#[derive(Clone)]
struct RecordingRunner {
    recorded: Arc<Mutex<Option<ChatRunRequestModel>>>,
}

impl ChatTurnRunner for RecordingRunner {
    /// Runs the test command implementation and returns its command future.
    fn run<'a>(
        &'a self,
        _model: &'a mut ChatModel,
        _renderer: &'a Renderer,
        _tools: &'a ToolStorage,
        request: ChatRunRequestModel,
    ) -> ChatTurnFuture<'a> {
        Box::pin(async move {
            *self.recorded.lock().unwrap() = Some(request);
            Ok(())
        })
    }
}

#[derive(Clone)]
struct FailingRunner;

impl ChatTurnRunner for FailingRunner {
    /// Runs the test command implementation and returns its command future.
    fn run<'a>(
        &'a self,
        _model: &'a mut ChatModel,
        _renderer: &'a Renderer,
        _tools: &'a ToolStorage,
        _request: ChatRunRequestModel,
    ) -> ChatTurnFuture<'a> {
        Box::pin(async { Err(ChatError::Session("runner failed".to_owned())) })
    }
}

/// Builds a chat model configured for command tests.
fn test_model() -> ChatModel {
    let session = SessionManager::new_in(temp_session_dir("controller-retry")).unwrap();
    let mut model = ChatModel::new(session, test_runtime());
    model.start_new_session().unwrap();
    model
}

/// Builds a runtime selection for chat tests.
fn test_runtime() -> RuntimeSelection {
    RuntimeSelection {
        provider_type: "openrouter".to_owned(),
        provider_auth: Some(spectacular_config::ProviderAuthMode::ApiKey),
        provider: "openrouter".to_owned(),
        api_key: "sk-or-v1-test".to_owned(),
        model_key: "test-model".to_owned(),
        model: "test/model".to_owned(),
        reasoning: ReasoningLevel::Medium,
        context_window_tokens: None,
    }
}

/// Builds a setup-only chat model for controller tests.
fn setup_model() -> ChatModel {
    let session = SessionManager::new_in(temp_session_dir("controller-setup")).unwrap();
    let mut model = ChatModel::new(session, RuntimeSelection::setup());
    model.start_new_session().unwrap();
    model
}

/// Returns the workspace root path used by controller tests.
fn test_workspace_root() -> PathBuf {
    PathBuf::from("workspace")
}

/// Builds a temporary session directory path for a named test case.
fn temp_session_dir(name: &str) -> PathBuf {
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();

    std::env::temp_dir().join(format!("spectacular-{name}-{suffix}"))
}
