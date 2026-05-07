    use super::*;
    use spectacular_commands::CommandControl;
    use spectacular_config::{ModelConfig, ProviderConfig, TaskAssignments};
    use std::collections::BTreeMap;

    #[test]
    fn runtime_selection_uses_latest_session_provider_and_model_events() {
        let records = vec![
            chat_record(ChatEvent::ProviderChanged {
                provider: "openrouter".to_owned(),
                source: Some("global_default".to_owned()),
                created_at: "2026-04-29T14:00:00Z".to_owned(),
            }),
            chat_record(ChatEvent::ModelChanged {
                slot: "coding".to_owned(),
                provider: "openrouter".to_owned(),
                model: "old/model".to_owned(),
                reasoning: "low".to_owned(),
                source: Some("global_default".to_owned()),
                created_at: "2026-04-29T14:00:00Z".to_owned(),
            }),
            chat_record(ChatEvent::ModelChanged {
                slot: "coding".to_owned(),
                provider: "openrouter".to_owned(),
                model: "new/model".to_owned(),
                reasoning: "high".to_owned(),
                source: Some("command".to_owned()),
                created_at: "2026-04-29T14:01:00Z".to_owned(),
            }),
        ];

        let runtime = RuntimeSelection::from_session_records(&complete_config(), &records)
            .unwrap()
            .unwrap();

        assert_eq!(runtime.provider, "openrouter");
        assert_eq!(runtime.api_key, "sk-or-v1-test");
        assert_eq!(runtime.model, "new/model");
        assert_eq!(runtime.reasoning, ReasoningLevel::High);
    }

    #[test]
    fn runtime_selection_falls_back_when_session_provider_is_unavailable() {
        let records = vec![
            chat_record(ChatEvent::ProviderChanged {
                provider: "missing".to_owned(),
                source: Some("global_default".to_owned()),
                created_at: "2026-04-29T14:00:00Z".to_owned(),
            }),
            chat_record(ChatEvent::ModelChanged {
                slot: "coding".to_owned(),
                provider: "missing".to_owned(),
                model: "missing/model".to_owned(),
                reasoning: "medium".to_owned(),
                source: Some("global_default".to_owned()),
                created_at: "2026-04-29T14:00:00Z".to_owned(),
            }),
        ];

        let runtime = RuntimeSelection::from_session_records(&complete_config(), &records).unwrap();

        assert!(runtime.is_none());
    }

    #[tokio::test]
    async fn chat_controller_dispatches_exit_command() {
        let session = session::SessionManager::new_in(temp_session_dir("controller-exit"))
            .expect("session manager should be created");
        let mut model = super::model::ChatModel::new(session, test_runtime());
        model.start_new_session().unwrap();
        let mut controller = super::controller::ChatController::new(
            model,
            commands::registry().unwrap(),
            Renderer::default(),
            ToolStorage::default(),
        );

        let control = controller
            .dispatch_command(spectacular_commands::CommandInvocation {
                name: "exit".to_owned(),
                args: Vec::new(),
            })
            .await
            .unwrap();

        assert_eq!(control, CommandControl::Exit);
    }

    fn complete_config() -> SpectacularConfig {
        let mut providers = BTreeMap::new();
        providers.insert(
            "openrouter".to_owned(),
            ProviderConfig::new("openrouter", "sk-or-v1-test"),
        );
        let mut models = BTreeMap::new();
        models.insert(
            "global-coding".to_owned(),
            ModelConfig::new("openrouter", "global/coding", ReasoningLevel::Medium),
        );

        SpectacularConfig {
            providers,
            models,
            tasks: TaskAssignments {
                general: None,
                coding: Some("global-coding".to_owned()),
                labeling: None,
            },
        }
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

    fn chat_record(event: ChatEvent) -> session::ChatRecord {
        session::ChatRecord::Known { line: 1, event }
    }

    fn temp_session_dir(name: &str) -> std::path::PathBuf {
        let suffix = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();

        std::env::temp_dir().join(format!("spectacular-chat-{name}-{suffix}"))
    }
