    use super::*;
    use spectacular_config::{ConfigError, ReasoningLevel};
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    /// Verifies that chat model start new session returns active session ID.
    #[test]
    fn chat_model_start_new_session_returns_active_session_id() {
        let session = crate::chat::session::SessionManager::new_in(temp_session_dir("start"))
            .expect("session manager should be created");
        let mut model = ChatModel::new(session, test_runtime());

        let started = model
            .start_new_session()
            .expect("session should be started");

        assert_eq!(started.id, model.current_session_id());
    }

    /// Verifies that chat model records reads started session event.
    #[test]
    fn chat_model_records_reads_started_session_event() {
        let session = crate::chat::session::SessionManager::new_in(temp_session_dir("records"))
            .expect("session manager should be created");
        let mut model = ChatModel::new(session, test_runtime());

        model.start_new_session().unwrap();

        assert!(matches!(
            model.records().unwrap()[0].event(),
            Some(crate::chat::session::ChatEvent::SessionStarted { .. })
        ));
    }

    /// Verifies that chat model append agent event persists user prompt.
    #[test]
    fn chat_model_append_agent_event_persists_user_prompt() {
        let session = crate::chat::session::SessionManager::new_in(temp_session_dir("append"))
            .expect("session manager should be created");
        let mut model = ChatModel::new(session, test_runtime());
        model.start_new_session().unwrap();

        model
            .append_agent_event(&spectacular_agent::AgentEvent::UserPrompt {
                content: "hello".to_owned(),
            })
            .unwrap();

        assert!(model.records().unwrap().iter().any(|record| matches!(
            record.event(),
            Some(crate::chat::session::ChatEvent::UserPrompt { content, .. }) if content == "hello"
        )));
    }

    /// Verifies that chat model resume session returns resumed session ID.
    #[test]
    fn chat_model_resume_session_returns_resumed_session_id() {
        let session = crate::chat::session::SessionManager::new_in(temp_session_dir("resume"))
            .expect("session manager should be created");
        let mut model = ChatModel::new(session, test_runtime());
        let started = model.start_new_session().unwrap();
        model.start_new_session().unwrap();

        let resumed = model.resume_session(&started.id).unwrap();

        assert_eq!(resumed.id, started.id);
    }

    /// Verifies that chat prompt footer model uses runtime and directory.
    #[test]
    fn chat_prompt_footer_model_uses_runtime_and_directory() {
        let runtime = test_runtime();
        let directory = PathBuf::from("workspace");

        let footer = ChatPromptFooterModel::from_runtime(&directory, &runtime);

        assert_eq!(footer.directory, directory);
        assert_eq!(footer.model, "test/model");
        assert_eq!(footer.reasoning, ReasoningLevel::Medium);
    }

    /// Verifies that provider notice propagates config load error.
    #[test]
    fn provider_notice_propagates_config_load_error() {
        let session = crate::chat::session::SessionManager::new_in(temp_session_dir("provider"))
            .expect("session manager should be created");
        let model = ChatModel::new(session, test_runtime());

        let error = model
            .provider_notice_with_loader(|| Err(ConfigError::ConfigDirUnavailable))
            .unwrap_err();

        assert!(matches!(
            error,
            ChatError::Config(ConfigError::ConfigDirUnavailable)
        ));
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

    /// Builds a temporary session directory path for a named test case.
    fn temp_session_dir(name: &str) -> PathBuf {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();

        std::env::temp_dir().join(format!("spectacular-chat-model-{name}-{suffix}"))
    }
