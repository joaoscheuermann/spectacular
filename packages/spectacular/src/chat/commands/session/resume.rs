use crate::chat::commands::{
    ChatCommand, ChatCommandContext, ChatCommandFuture, ChatCommandResult,
};
use spectacular_commands::CommandError;

pub fn command() -> ChatCommand {
    ChatCommand {
        name: "resume",
        usage: "/resume <session-id>",
        summary: "Resume a saved session",
        execute,
    }
}

fn execute<'a>(context: ChatCommandContext<'a>, args: Vec<String>) -> ChatCommandFuture<'a> {
    Box::pin(async move {
        let [prefix] = args.as_slice() else {
            return ChatCommandResult::error(
                CommandError::usage("/resume <session-id>").to_string(),
            );
        };

        let resumed = match context.model.resume_session(prefix) {
            Ok(resumed) => resumed,
            Err(error) => return ChatCommandResult::error(error.to_string()),
        };

        context.clear_screen();
        context.session_resumed(&resumed.id);
        if let Err(error) = context.render_records(&resumed.records).await {
            return ChatCommandResult::error(error.to_string());
        }

        ChatCommandResult::success()
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chat::commands::{
        test_support::NoopRunner, ChatCommandContext, ChatCommandControl, ChatCommandResult,
    };
    use crate::chat::model::ChatModel;
    use crate::chat::renderer::Renderer;
    use crate::chat::session::SessionManager;
    use crate::chat::RuntimeSelection;
    use spectacular_agent::ToolStorage;
    use spectacular_config::ReasoningLevel;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[tokio::test]
    async fn resume_returns_success_after_replaying_records() {
        let mut model = test_model();
        let started = model.start_new_session().unwrap();
        model.start_new_session().unwrap();
        let renderer = Renderer::default();
        let tools = ToolStorage::default();
        let runner = NoopRunner;
        let mut control = ChatCommandControl::default();
        let context = ChatCommandContext::new(&mut model, &renderer, &tools, &runner, &mut control);

        let result = execute(context, vec![started.id]).await;

        assert_eq!(result, ChatCommandResult::Success);
    }

    fn test_model() -> ChatModel {
        ChatModel::new(
            SessionManager::new_in(temp_session_dir("resume-command")).unwrap(),
            RuntimeSelection {
                provider: "openrouter".to_owned(),
                api_key: "sk-or-v1-test".to_owned(),
                model: "test/model".to_owned(),
                reasoning: ReasoningLevel::Medium,
            },
        )
    }

    fn temp_session_dir(name: &str) -> PathBuf {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();

        std::env::temp_dir().join(format!("spectacular-resume-command-{name}-{suffix}"))
    }
}
