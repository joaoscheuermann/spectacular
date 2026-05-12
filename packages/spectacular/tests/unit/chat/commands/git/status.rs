    use super::*;
    use crate::chat::commands::{test_support::NoopRunner, ChatCommandControl};
    use crate::chat::model::ChatModel;
    use crate::chat::renderer::Renderer;
    use crate::chat::session::SessionManager;
    use crate::chat::RuntimeSelection;
    use spectacular_agent::ToolStorage;
    use spectacular_config::ReasoningLevel;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    /// Verifies that git status returns success.
    #[tokio::test]
    async fn git_status_returns_success() {
        let mut model = test_model();
        let renderer = Renderer::default();
        let tools = ToolStorage::default();
        let runner = NoopRunner;
        let mut control = ChatCommandControl::default();
        let context = ChatCommandContext::new(&mut model, &renderer, &tools, &runner, &mut control);

        let result = execute(context, Vec::new()).await;

        assert_eq!(result, ChatCommandResult::Success);
    }

    /// Verifies that git status rejects args.
    #[tokio::test]
    async fn git_status_rejects_args() {
        let mut model = test_model();
        let renderer = Renderer::default();
        let tools = ToolStorage::default();
        let runner = NoopRunner;
        let mut control = ChatCommandControl::default();
        let context = ChatCommandContext::new(&mut model, &renderer, &tools, &runner, &mut control);

        let result = execute(context, vec!["extra".to_owned()]).await;

        assert!(matches!(result, ChatCommandResult::Error(_)));
    }

    /// Builds a chat model configured for command tests.
    fn test_model() -> ChatModel {
        let session = SessionManager::new_in(temp_session_dir("git-status")).unwrap();
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

        std::env::temp_dir().join(format!("spectacular-git-status-command-{name}-{suffix}"))
    }
