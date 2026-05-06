use crate::chat::commands::{
    ChatCommand, ChatCommandContext, ChatCommandFuture, ChatCommandResult,
};
use spectacular_commands::CommandError;

pub fn command() -> ChatCommand {
    ChatCommand {
        name: "new",
        usage: "/new",
        summary: "Start a new chat session",
        execute,
    }
}

fn execute<'a>(context: ChatCommandContext<'a>, args: Vec<String>) -> ChatCommandFuture<'a> {
    Box::pin(async move {
        if !args.is_empty() {
            return ChatCommandResult::error(CommandError::usage("/new").to_string());
        }

        let started = match context.model.start_new_session() {
            Ok(started) => started,
            Err(error) => return ChatCommandResult::error(error.to_string()),
        };

        context.clear_screen();
        context.session_created(&started.id);

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
    async fn new_returns_success_after_starting_session() {
        let mut model = test_model();
        let renderer = Renderer::default();
        let tools = ToolStorage::default();
        let runner = NoopRunner;
        let mut control = ChatCommandControl::default();
        let context = ChatCommandContext::new(&mut model, &renderer, &tools, &runner, &mut control);

        let result = execute(context, Vec::new()).await;

        assert_eq!(result, ChatCommandResult::Success);
    }

    fn test_model() -> ChatModel {
        ChatModel::new(
            SessionManager::new_in(temp_session_dir("new")).unwrap(),
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

        std::env::temp_dir().join(format!("spectacular-new-command-{name}-{suffix}"))
    }
}
