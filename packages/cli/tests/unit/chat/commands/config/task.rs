use super::*;
use crate::chat::commands::{ChatCommandContext, ChatCommandControl, ChatCommandResult};
use crate::chat::model::ChatModel;
use crate::chat::session::SessionManager;
use crate::chat::RuntimeSelection;
use ::agent::ToolStorage;
use ::config::{ProviderAuthMode, ReasoningLevel, TaskModelSlot};
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

/// Verifies that task set parses valid named fields into a task assignment spec.
#[test]
fn task_set_spec_valid_fields_returns_task_and_model() {
    let spec =
        TaskSetSpec::parse(&["task:coding".to_owned(), "model:test-model".to_owned()]).unwrap();

    assert_eq!(
        spec,
        TaskSetSpec {
            task: TaskModelSlot::Coding,
            model: "test-model".to_owned(),
        }
    );
}

/// Verifies that task set rejects invalid task slots before config I/O.
#[tokio::test]
async fn task_set_invalid_task_returns_error() {
    let mut model = test_model("invalid-task");
    let tools = ToolStorage::default();
    let mut control = ChatCommandControl::default();
    let context = ChatCommandContext::new(&mut model, &tools, &mut control);

    let result = execute(
        context,
        vec![
            "set".to_owned(),
            "task:planning".to_owned(),
            "model:test-model".to_owned(),
        ],
    )
    .await;

    assert!(matches!(
        result,
        ChatCommandResult::Error(message) if message.contains("planning")
    ));
}

/// Builds a chat model configured for task command tests.
fn test_model(name: &str) -> ChatModel {
    let session = SessionManager::new_in(temp_session_dir(name)).unwrap();
    ChatModel::new(
        session,
        RuntimeSelection {
            provider_type: "openrouter".to_owned(),
            provider_auth: Some(ProviderAuthMode::ApiKey),
            provider: "openrouter".to_owned(),
            api_key: "sk-or-v1-test".to_owned(),
            model_key: "test-model".to_owned(),
            model: "test/model".to_owned(),
            reasoning: ReasoningLevel::Medium,
            context_window_tokens: None,
        },
    )
}

/// Builds a temporary session directory path for a named test case.
fn temp_session_dir(name: &str) -> PathBuf {
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();

    std::env::temp_dir().join(format!("doric-task-command-{name}-{suffix}"))
}
