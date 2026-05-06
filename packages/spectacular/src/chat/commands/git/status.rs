use crate::chat::commands::{
    ChatCommand, ChatCommandContext, ChatCommandFuture, ChatCommandResult,
};
use spectacular_commands::CommandError;
use std::process::Command;

pub fn command() -> ChatCommand {
    ChatCommand {
        name: "git-status",
        usage: "/git-status",
        summary: "Show current git status and staged changes",
        execute,
    }
}

fn execute<'a>(context: ChatCommandContext<'a>, args: Vec<String>) -> ChatCommandFuture<'a> {
    Box::pin(async move {
        if !args.is_empty() {
            return ChatCommandResult::error(CommandError::usage("/git-status").to_string());
        }

        // Get git status
        let status_output = Command::new("git").arg("status").arg("--short").output();

        match status_output {
            Ok(output) => {
                let status_text = String::from_utf8_lossy(&output.stdout);
                if status_text.trim().is_empty() {
                    context.notice("No changes in working directory.");
                } else {
                    context.notice("Working directory changes:");
                    context.notice(&status_text);
                }
            }
            Err(e) => {
                return ChatCommandResult::error(format!("Failed to run git status: {}", e));
            }
        }

        // Get staged changes
        let staged_output = Command::new("git")
            .arg("diff")
            .arg("--cached")
            .arg("--stat")
            .output();

        match staged_output {
            Ok(output) => {
                let staged_text = String::from_utf8_lossy(&output.stdout);
                if staged_text.trim().is_empty() {
                    context.notice("No staged changes.");
                } else {
                    context.notice("Staged changes:");
                    context.notice(&staged_text);
                }
            }
            Err(e) => {
                return ChatCommandResult::error(format!("Failed to run git diff --cached: {}", e));
            }
        }

        ChatCommandResult::success()
    })
}

#[cfg(test)]
mod tests {
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

    fn test_model() -> ChatModel {
        let session = SessionManager::new_in(temp_session_dir("git-status")).unwrap();
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

        std::env::temp_dir().join(format!("spectacular-git-status-command-{name}-{suffix}"))
    }
}
