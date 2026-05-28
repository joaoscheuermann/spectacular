    use super::*;
    use crate::chat::commands::{
        ChatCommandContext, ChatCommandControl, ChatCommandResult,
    };
    use crate::chat::model::ChatModel;
        use crate::chat::session::SessionManager;
    use crate::chat::RuntimeSelection;
    use ::agent::ToolStorage;
    use ::config::ReasoningLevel;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    /// Verifies that new returns success after starting session.
    #[tokio::test]
    async fn new_returns_success_after_starting_session() {
        let mut model = test_model();
        let tools = ToolStorage::default();
        let mut control = ChatCommandControl::default();
        let context = ChatCommandContext::new(&mut model, &tools, &mut control);

        let result = execute(context, Vec::new()).await;

        assert_eq!(result, ChatCommandResult::Success);
    }

    /// Builds a chat model configured for command tests.
    fn test_model() -> ChatModel {
        ChatModel::new(
            SessionManager::new_in(temp_session_dir("new")).unwrap(),
            RuntimeSelection {
                provider_type: "openrouter".to_owned(),
                provider_auth: Some(config::ProviderAuthMode::ApiKey),
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

        std::env::temp_dir().join(format!("doric-new-command-{name}-{suffix}"))
    }
