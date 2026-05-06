use crate::chat::commands::{
    ChatCommand, ChatCommandContext, ChatCommandFuture, ChatCommandResult,
};
use crate::chat::ChatError;
use spectacular_commands::CommandError;
use spectacular_config::ConfigError;

pub fn command() -> ChatCommand {
    ChatCommand {
        name: "provider",
        usage: "/provider [configured-provider-id]",
        summary: "Show or switch provider",
        execute,
    }
}

fn execute<'a>(context: ChatCommandContext<'a>, args: Vec<String>) -> ChatCommandFuture<'a> {
    Box::pin(async move {
        match args.as_slice() {
            [] => match context.model.provider_notice() {
                Ok(message) => {
                    context.notice(&message);
                    ChatCommandResult::success()
                }
                Err(error) => ChatCommandResult::error(error.to_string()),
            },
            [provider] => match context.model.switch_provider(provider) {
                Ok(_) => {
                    context.success(&format!("active provider updated: {provider}"));
                    ChatCommandResult::success()
                }
                Err(ChatError::Config(error @ ConfigError::ProviderNotConfigured { .. })) => {
                    ChatCommandResult::error(error.to_string())
                }
                Err(error) => ChatCommandResult::error(error.to_string()),
            },
            _ => ChatCommandResult::error(
                CommandError::usage("/provider [configured-provider-id]").to_string(),
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
    async fn provider_without_args_returns_success_after_rendering_notice() {
        let mut model = test_model();
        let renderer = Renderer::default();
        let tools = ToolStorage::default();
        let runner = NoopRunner;
        let mut control = ChatCommandControl::default();
        let context = ChatCommandContext::new(&mut model, &renderer, &tools, &runner, &mut control);

        let result = execute(context, Vec::new()).await;

        assert_eq!(result, ChatCommandResult::Success);
    }

    #[tokio::test]
    async fn unknown_provider_returns_error_result() {
        let mut model = test_model();
        let renderer = Renderer::default();
        let tools = ToolStorage::default();
        let runner = NoopRunner;
        let mut control = ChatCommandControl::default();
        let context = ChatCommandContext::new(&mut model, &renderer, &tools, &runner, &mut control);

        let result = execute(context, vec!["missing-provider".to_owned()]).await;

        assert!(matches!(
            result,
            ChatCommandResult::Error(message) if message.contains("missing-provider")
        ));
    }

    fn test_model() -> ChatModel {
        ChatModel::new(
            SessionManager::new_in(temp_session_dir("provider")).unwrap(),
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

        std::env::temp_dir().join(format!("spectacular-provider-command-{name}-{suffix}"))
    }
}
