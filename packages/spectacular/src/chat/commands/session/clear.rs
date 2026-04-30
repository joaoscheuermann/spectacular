use crate::chat::ChatContext;
use spectacular_commands::{Command, CommandControl, CommandError, CommandFuture};

pub fn command() -> Command<ChatContext> {
    Command {
        name: "clear",
        usage: "/clear",
        summary: "Clear the terminal",
        execute,
    }
}

fn execute<'a>(context: &'a mut ChatContext, args: Vec<String>) -> CommandFuture<'a> {
    Box::pin(async move {
        if !args.is_empty() {
            return Err(CommandError::usage("/clear"));
        }

        context.renderer.clear_screen();
        context.renderer.resumed(context.session.current_id());
        Ok(CommandControl::Continue)
    })
}
