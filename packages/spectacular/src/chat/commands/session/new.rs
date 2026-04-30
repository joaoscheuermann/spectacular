use crate::chat::ChatContext;
use spectacular_commands::{Command, CommandControl, CommandError, CommandFuture};

pub fn command() -> Command<ChatContext> {
    Command {
        name: "new",
        usage: "/new",
        summary: "Start a new chat session",
        execute,
    }
}

fn execute<'a>(context: &'a mut ChatContext, args: Vec<String>) -> CommandFuture<'a> {
    Box::pin(async move {
        if !args.is_empty() {
            return Err(CommandError::usage("/new"));
        }

        context
            .start_new_session()
            .map_err(|error| CommandError::message(error.to_string()))?;
        context.renderer.clear_screen();
        context
            .renderer
            .session_created(context.session.current_id());
        Ok(CommandControl::Continue)
    })
}
