    use super::*;
    use crate::chat::commands::{ChatCommandContext, ChatCommandControl, ChatCommandResult};
    use crate::chat::model::ChatModel;
            use crate::chat::session::SessionManager;
    use crate::chat::RuntimeSelection;
    use spectacular_agent::{AgentEvent, ToolStorage};
    use spectacular_config::ReasoningLevel;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    /// Verifies that retry queues latest prompt for controller execution.
    #[tokio::test]
    async fn retry_queues_latest_prompt_for_controller_execution() {
        let mut model = test_model();
        model
            .append_agent_event(&AgentEvent::user_prompt("try again"))
            .unwrap();
        let tools = ToolStorage::default();
        let mut control = ChatCommandControl::default();
        let context = ChatCommandContext::new(&mut model, &tools, &mut control);

        let result = execute(context, Vec::new()).await;

        assert_eq!(result, ChatCommandResult::Success);
        assert!(control
            .take_follow_up_prompt()
            .is_some_and(|request| request.prompt == "try again" && request.retry_existing_prompt));
    }

    /// Builds a chat model configured for command tests.
    fn test_model() -> ChatModel {
        let session = SessionManager::new_in(temp_session_dir("retry")).unwrap();
        let mut model = ChatModel::new(
            session,
            RuntimeSelection {
                provider_type: "openrouter".to_owned(),
                provider_auth: Some(spectacular_config::ProviderAuthMode::ApiKey),
                provider: "openrouter".to_owned(),
                api_key: "sk-or-v1-test".to_owned(),
                model_key: "test-model".to_owned(),
                model: "test/model".to_owned(),
                reasoning: ReasoningLevel::Medium,
                context_window_tokens: None,
            },
        );
        model.start_new_session().unwrap();
        model
    }

    /// Builds a temporary session directory path for a named test case.
    fn temp_session_dir(name: &str) -> PathBuf {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();

        std::env::temp_dir().join(format!("spectacular-retry-command-{name}-{suffix}"))
    }
