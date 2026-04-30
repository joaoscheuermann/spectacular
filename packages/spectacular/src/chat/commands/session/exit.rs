use crate::chat::ChatContext;
use spectacular_commands::{Command, CommandControl, CommandError, CommandFuture};

pub fn command() -> Command<ChatContext> {
    Command {
        name: "exit",
        usage: "/exit",
        summary: "Exit chat",
        execute,
    }
}

fn execute<'a>(_context: &'a mut ChatContext, args: Vec<String>) -> CommandFuture<'a> {
    Box::pin(async move {
        if !args.is_empty() {
            return Err(CommandError::usage("/exit"));
        }

        Ok(CommandControl::Exit)
    })
}
