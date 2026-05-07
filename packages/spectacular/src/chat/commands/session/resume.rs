use crate::chat::commands::{
    ChatCommand, ChatCommandContext, ChatCommandFuture, ChatCommandResult,
};
use spectacular_commands::CommandError;

pub fn command() -> ChatCommand {
    ChatCommand {
        name: "resume",
        usage: "/resume <session-id>",
        summary: "Resume a saved session",
        completion: &[],
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
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/unit/chat/commands/session/resume.rs"
    ));
}
