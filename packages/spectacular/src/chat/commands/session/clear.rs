use crate::chat::commands::{
    ChatCommand, ChatCommandContext, ChatCommandFuture, ChatCommandResult,
};
use spectacular_commands::CommandError;

pub fn command() -> ChatCommand {
    ChatCommand {
        name: "clear",
        usage: "/clear",
        summary: "Clear the terminal",
        completion: &[],
        execute,
    }
}

fn execute<'a>(context: ChatCommandContext<'a>, args: Vec<String>) -> ChatCommandFuture<'a> {
    Box::pin(async move {
        if !args.is_empty() {
            return ChatCommandResult::error(CommandError::usage("/clear").to_string());
        }

        let id = context.model.current_session_id().to_owned();
        context.clear_screen();
        context.session_resumed(&id);

        ChatCommandResult::success()
    })
}

#[cfg(test)]
mod tests {
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/unit/chat/commands/session/clear.rs"
    ));
}
