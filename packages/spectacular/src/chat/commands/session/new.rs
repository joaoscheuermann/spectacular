use crate::chat::commands::{
    ChatCommand, ChatCommandContext, ChatCommandFuture, ChatCommandResult,
};
use spectacular_commands::CommandError;

pub fn command() -> ChatCommand {
    ChatCommand {
        name: "new",
        usage: "/new",
        summary: "Start a new chat session",
        completion: &[],
        execute,
    }
}

fn execute<'a>(context: ChatCommandContext<'a>, args: Vec<String>) -> ChatCommandFuture<'a> {
    Box::pin(async move {
        if !args.is_empty() {
            return ChatCommandResult::error(CommandError::usage("/new").to_string());
        }

        let directory = match std::env::current_dir() {
            Ok(directory) => directory,
            Err(error) => return ChatCommandResult::error(error.to_string()),
        };
        let started = match context.model.start_new_session() {
            Ok(started) => started,
            Err(error) => return ChatCommandResult::error(error.to_string()),
        };

        context.clear_screen();
        context.session_created(&started.id, &directory);

        ChatCommandResult::success()
    })
}

#[cfg(test)]
mod tests {
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/unit/chat/commands/session/new.rs"
    ));
}
