use super::*;
use crate::chat::commands::{ChatCommandContext, ChatCommandControl, ChatCommandResult};
use crate::chat::model::ChatModel;
use crate::chat::session::SessionManager;
use crate::chat::RuntimeSelection;
use ::agent::ToolStorage;
use ::config::{ProviderAuthMode, ReasoningLevel};
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

/// Verifies that model add parses a valid spec without touching config I/O.
#[test]
fn model_add_spec_valid_fields_returns_target_and_optional_name() {
    let spec = ModelAddSpec::parse(&[
        "provider:openrouter".to_owned(),
        "id:test/model".to_owned(),
        "reasoning:medium".to_owned(),
        "name:test-model".to_owned(),
    ])
    .unwrap();

    assert_eq!(
        spec,
        ModelAddSpec {
            target: ModelTarget {
                provider: "openrouter".to_owned(),
                model_id: "test/model".to_owned(),
                reasoning: ReasoningLevel::Medium,
            },
            name: Some("test-model".to_owned()),
        }
    );
}

/// Verifies that model edit rejects invalid reasoning before config I/O.
#[tokio::test]
async fn model_edit_invalid_reasoning_returns_error() {
    let mut model = test_model("edit-invalid-reasoning");
    let tools = ToolStorage::default();
    let mut control = ChatCommandControl::default();
    let context = ChatCommandContext::new(&mut model, &tools, &mut control);

    let result = execute(
        context,
        vec![
            "edit".to_owned(),
            "name:test-model".to_owned(),
            "reasoning:invalid".to_owned(),
        ],
    )
    .await;

    assert!(matches!(
        result,
        ChatCommandResult::Error(message) if message.contains("reasoning")
    ));
}

/// Verifies that model remove without confirmation stops before config mutation.
#[tokio::test]
async fn model_remove_without_confirm_returns_success() {
    let mut model = test_model("remove-unconfirmed");
    let tools = ToolStorage::default();
    let mut control = ChatCommandControl::default();
    let context = ChatCommandContext::new(&mut model, &tools, &mut control);

    let result = execute(
        context,
        vec!["remove".to_owned(), "name:test-model".to_owned()],
    )
    .await;

    assert_eq!(result, ChatCommandResult::Success);
}

/// Builds a chat model configured for model command tests.
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

    std::env::temp_dir().join(format!("doric-model-command-{name}-{suffix}"))
}
