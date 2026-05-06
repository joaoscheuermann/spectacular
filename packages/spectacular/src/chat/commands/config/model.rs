use crate::chat::commands::{
    ChatCommand, ChatCommandContext, ChatCommandFuture, ChatCommandResult,
};
use spectacular_commands::CommandError;
use spectacular_config::ReasoningLevel;
use std::str::FromStr;

pub fn command() -> ChatCommand {
    ChatCommand {
        name: "model",
        usage: "/model [model-id none|low|medium|high]",
        summary: "Show or update coding model",
        execute,
    }
}

fn execute<'a>(context: ChatCommandContext<'a>, args: Vec<String>) -> ChatCommandFuture<'a> {
    Box::pin(async move {
        match args.as_slice() {
            [] => {
                context.notice(&context.model.coding_model_notice());
                ChatCommandResult::success()
            }
            [model, reasoning] => {
                let reasoning = match ReasoningLevel::from_str(reasoning) {
                    Ok(reasoning) => reasoning,
                    Err(error) => return ChatCommandResult::error(error.to_string()),
                };

                if let Err(error) = context.model.update_coding_model(model, reasoning) {
                    return ChatCommandResult::error(error.to_string());
                }

                context.success("coding model updated");

                ChatCommandResult::success()
            }
            _ => ChatCommandResult::error(
                CommandError::usage("/model [model-id none|low|medium|high]").to_string(),
            ),
        }
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
    async fn model_without_args_returns_success_after_rendering_notice() {
        let mut model = test_model();
        let renderer = Renderer::default();
        let tools = ToolStorage::default();
        let runner = NoopRunner;
        let mut control = ChatCommandControl::default();
        let context = ChatCommandContext::new(
            &mut model,
            &renderer,
            &tools,
            &runner,
            &mut control,
        );

        let result = execute(context, Vec::new()).await;

        assert_eq!(result, ChatCommandResult::Success);
    }

    fn test_model() -> ChatModel {
        ChatModel::new(
            SessionManager::new_in(temp_session_dir("model")).unwrap(),
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

        std::env::temp_dir().join(format!("spectacular-model-command-{name}-{suffix}"))
    }
}
