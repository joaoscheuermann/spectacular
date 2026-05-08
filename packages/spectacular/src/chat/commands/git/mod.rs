//! Git command module.
//!
//! Provides `/git` command with sub-commands: status and commit.

pub mod commit;
pub mod helpers;
pub mod status;

use crate::chat::commands::{
    ChatCommand, ChatCommandContext, ChatCommandFuture, ChatCommandResult, CompletionSubcommandSpec,
};
use spectacular_commands::CommandError;

/// Git command sub-commands for completion
const GIT_SUBCOMMANDS: &[CompletionSubcommandSpec] = &[
    CompletionSubcommandSpec {
        name: "status",
        summary: "Show current git status and staged changes",
        fields: &[],
    },
    CompletionSubcommandSpec {
        name: "commit",
        summary: "Generate a conventional commit message and commit staged changes",
        fields: &[],
    },
];

/// Main `/git` command
pub fn command() -> ChatCommand {
    ChatCommand {
        name: "git",
        usage: "/git status | /git commit",
        summary: "Git operations",
        completion: GIT_SUBCOMMANDS,
        execute,
    }
}

fn execute<'a>(context: ChatCommandContext<'a>, args: Vec<String>) -> ChatCommandFuture<'a> {
    Box::pin(async move {
        if args.is_empty() {
            return ChatCommandResult::error(CommandError::usage(command().usage).to_string());
        }

        let subcommand = &args[0];
        let sub_args = &args[1..];

        match subcommand.as_str() {
            "status" => {
                if !sub_args.is_empty() {
                    return ChatCommandResult::error(
                        CommandError::usage("/git status").to_string(),
                    );
                }
                status::execute(context, sub_args.to_vec()).await
            }
            "commit" => {
                if !sub_args.is_empty() {
                    return ChatCommandResult::error(
                        CommandError::usage("/git commit").to_string(),
                    );
                }
                commit::execute(context, sub_args.to_vec()).await
            }
            _ => ChatCommandResult::error(CommandError::usage(command().usage).to_string()),
        }
    })
}
