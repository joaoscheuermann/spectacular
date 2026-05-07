    use super::*;
    use crate::chat::commands::{ChatCommandContext, ChatCommandControl, ChatCommandResult};
    use crate::chat::model::ChatModel;
    use crate::chat::renderer::Renderer;
    use crate::chat::runner::{ChatTurnFuture, ChatTurnRunner};
    use crate::chat::session::SessionManager;
    use crate::chat::RuntimeSelection;
    use spectacular_agent::{AgentEvent, ToolStorage};
    use spectacular_config::ReasoningLevel;
    use std::path::PathBuf;
    use std::sync::{Arc, Mutex};
    use std::time::{SystemTime, UNIX_EPOCH};

    #[tokio::test]
    async fn retry_runs_latest_prompt_through_context() {
        let recorded = Arc::new(Mutex::new(None));
        let mut model = test_model();
        model
            .append_agent_event(&AgentEvent::UserPrompt {
                content: "try again".to_owned(),
            })
            .unwrap();
        let renderer = Renderer::default();
        let tools = ToolStorage::default();
        let runner = RecordingRunner {
            recorded: Arc::clone(&recorded),
        };
        let mut control = ChatCommandControl::default();
        let context = ChatCommandContext::new(&mut model, &renderer, &tools, &runner, &mut control);

        let result = execute(context, Vec::new()).await;

        assert_eq!(result, ChatCommandResult::Success);
        assert!(recorded.lock().unwrap().as_ref().is_some_and(|request| {
            request.prompt == "try again" && request.retry_existing_prompt
        }));
    }

    struct RecordingRunner {
        recorded: Arc<Mutex<Option<ChatRunRequestModel>>>,
    }

    impl ChatTurnRunner for RecordingRunner {
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

    fn test_model() -> ChatModel {
        let session = SessionManager::new_in(temp_session_dir("retry")).unwrap();
        let mut model = ChatModel::new(
            session,
            RuntimeSelection {
                provider_type: "openrouter".to_owned(),
                provider: "openrouter".to_owned(),
                api_key: "sk-or-v1-test".to_owned(),
                model_key: "test-model".to_owned(),
                model: "test/model".to_owned(),
                reasoning: ReasoningLevel::Medium,
            },
        );
        model.start_new_session().unwrap();
        model
    }

    fn temp_session_dir(name: &str) -> PathBuf {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();

        std::env::temp_dir().join(format!("spectacular-retry-command-{name}-{suffix}"))
    }
