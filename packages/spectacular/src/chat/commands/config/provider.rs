use crate::chat::ChatContext;
use spectacular_commands::{Command, CommandControl, CommandError, CommandFuture};

pub fn command() -> Command<ChatContext> {
    Command {
        name: "provider",
        usage: "/provider [configured-provider-id]",
        summary: "Show or switch provider",
        execute,
    }
}

fn execute<'a>(context: &'a mut ChatContext, args: Vec<String>) -> CommandFuture<'a> {
    Box::pin(async move {
        match args.as_slice() {
            [] => {
                context
                    .show_provider()
                    .map_err(|error| CommandError::message(error.to_string()))?;
                Ok(CommandControl::Continue)
            }
            [provider] => {
                context
                    .switch_provider(provider)
                    .map_err(|error| CommandError::message(error.to_string()))?;
                Ok(CommandControl::Continue)
            }
            _ => Err(CommandError::usage("/provider [configured-provider-id]")),
        }
    })
}
