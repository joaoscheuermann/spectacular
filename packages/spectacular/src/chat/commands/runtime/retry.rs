use crate::chat::commands::{
    ChatCommand, ChatCommandContext, ChatCommandFuture, ChatCommandResult,
};
use crate::chat::model::ChatRunRequestModel;
use spectacular_commands::CommandError;

pub fn command() -> ChatCommand {
    ChatCommand {
        name: "retry",
        usage: "/retry",
        summary: "Retry the latest prompt",
        execute,
    }
}

fn execute<'a>(mut context: ChatCommandContext<'a>, args: Vec<String>) -> ChatCommandFuture<'a> {
    Box::pin(async move {
        if !args.is_empty() {
            return ChatCommandResult::error(CommandError::usage("/retry").to_string());
        }

        let prompt = match context.model.truncate_after_latest_user_prompt() {
            Ok(prompt) => prompt,
            Err(error) => return ChatCommandResult::error(error.to_string()),
        };
        if let Err(error) = context.model.append_runtime_defaults("retry") {
            return ChatCommandResult::error(error.to_string());
        }
        let request = ChatRunRequestModel {
            prompt,
            render_user_prompt: false,
            retry_existing_prompt: true,
            runtime: context.model.runtime().clone(),
        };

        context.notice("retrying latest prompt...");
        if let Err(error) = context.run_prompt(request).await {
            return ChatCommandResult::error(error.to_string());
        }

        ChatCommandResult::success()
    })
}

#[cfg(test)]
mod tests {
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
                provider: "openrouter".to_owned(),
                api_key: "sk-or-v1-test".to_owned(),
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
}
