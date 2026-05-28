use super::*;
use crate::chat::commands::{ChatCommandContext, ChatCommandControl, ChatCommandResult};
use crate::chat::model::ChatModel;
use crate::chat::session::SessionManager;
use crate::chat::RuntimeSelection;
use ::agent::ToolStorage;
use ::config::{ProviderAuthMode, ReasoningLevel};
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

/// Verifies that provider add rejects unavailable provider types before config I/O.
#[tokio::test]
async fn provider_add_unknown_provider_returns_provider_type_error() {
    let mut model = test_model("add-unknown");
    let tools = ToolStorage::default();
    let mut control = ChatCommandControl::default();
    let context = ChatCommandContext::new(&mut model, &tools, &mut control);

    let result = execute(
        context,
        vec![
            "add".to_owned(),
            "provider:missing".to_owned(),
            "apikey:test".to_owned(),
        ],
    )
    .await;

    assert_eq!(
        result,
        ChatCommandResult::Error("provider type `missing` is not available".to_owned())
    );
}

/// Verifies that provider auth rejects non-OpenAI providers before browser auth.
#[tokio::test]
async fn provider_auth_non_openai_returns_provider_error() {
    let mut model = test_model("auth-non-openai");
    let tools = ToolStorage::default();
    let mut control = ChatCommandControl::default();
    let context = ChatCommandContext::new(&mut model, &tools, &mut control);

    let result = execute(
        context,
        vec!["auth".to_owned(), "provider:openrouter".to_owned()],
    )
    .await;

    assert_eq!(
        result,
        ChatCommandResult::Error(
            "/provider auth supports only provider type `openai`; got `openrouter`".to_owned()
        )
    );
}

/// Verifies that provider remove without confirmation stops before config mutation.
#[tokio::test]
async fn provider_remove_without_confirm_returns_success() {
    let mut model = test_model("remove-unconfirmed");
    let tools = ToolStorage::default();
    let mut control = ChatCommandControl::default();
    let context = ChatCommandContext::new(&mut model, &tools, &mut control);

    let result = execute(
        context,
        vec!["remove".to_owned(), "name:openrouter".to_owned()],
    )
    .await;

    assert_eq!(result, ChatCommandResult::Success);
}

/// Builds a chat model configured for provider command tests.
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

    std::env::temp_dir().join(format!("doric-provider-command-{name}-{suffix}"))
}
