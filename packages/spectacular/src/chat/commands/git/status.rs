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
        completion: &[],
        execute,
    }
}

fn execute<'a>(context: ChatCommandContext<'a>, args: Vec<String>) -> ChatCommandFuture<'a> {
    Box::pin(async move {
        if !args.is_empty() {
            return ChatCommandResult::error(CommandError::usage("/git-status").to_string());
        }

        // Get git status
        let status_result = context
            .work(async {
                tokio::task::spawn_blocking(|| {
                    Command::new("git").arg("status").arg("--short").output()
                })
                .await
            })
            .await;

        match status_result {
            Ok(Ok(output)) => {
                let status_text = String::from_utf8_lossy(&output.stdout);
                if status_text.trim().is_empty() {
                    context.notice("No changes in working directory.");
                } else {
                    context.notice("Working directory changes:");
                    context.notice(&status_text);
                }
            }
            Ok(Err(e)) => {
                return ChatCommandResult::error(format!("Failed to run git status: {}", e));
            }
            Err(e) => {
                return ChatCommandResult::error(format!("Failed to run git status: {}", e));
            }
        }

        // Get staged changes
        let staged_result = context
            .work(async {
                tokio::task::spawn_blocking(|| {
                    Command::new("git")
                        .arg("diff")
                        .arg("--cached")
                        .arg("--stat")
                        .output()
                })
                .await
            })
            .await;

        match staged_result {
            Ok(Ok(output)) => {
                let staged_text = String::from_utf8_lossy(&output.stdout);
                if staged_text.trim().is_empty() {
                    context.notice("No staged changes.");
                } else {
                    context.notice("Staged changes:");
                    context.notice(&staged_text);
                }
            }
            Ok(Err(e)) => {
                return ChatCommandResult::error(format!("Failed to run git diff --cached: {}", e));
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
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/unit/chat/commands/git/status.rs"
    ));
}
