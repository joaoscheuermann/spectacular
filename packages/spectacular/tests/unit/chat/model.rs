    use super::*;
    use spectacular_config::{ConfigError, ReasoningLevel};
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

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

    fn test_runtime() -> RuntimeSelection {
        RuntimeSelection {
            provider_type: "openrouter".to_owned(),
            provider: "openrouter".to_owned(),
            api_key: "sk-or-v1-test".to_owned(),
            model_key: "test-model".to_owned(),
            model: "test/model".to_owned(),
            reasoning: ReasoningLevel::Medium,
        }
    }

    fn temp_session_dir(name: &str) -> PathBuf {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();

        std::env::temp_dir().join(format!("spectacular-chat-model-{name}-{suffix}"))
    }
