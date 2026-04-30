use crate::chat::ChatContext;
use spectacular_commands::{Command, CommandControl, CommandError, CommandFuture};
use spectacular_config::ReasoningLevel;
use std::str::FromStr;

pub fn command() -> Command<ChatContext> {
    Command {
        name: "reasoning",
        usage: "/reasoning [none|low|medium|high]",
        summary: "Show or update coding reasoning",
        execute,
    }
}

fn execute<'a>(context: &'a mut ChatContext, args: Vec<String>) -> CommandFuture<'a> {
    Box::pin(async move {
        match args.as_slice() {
            [] => {
                context.show_reasoning();
                Ok(CommandControl::Continue)
            }
            [reasoning] => {
                let reasoning = ReasoningLevel::from_str(reasoning)
                    .map_err(|error| CommandError::message(error.to_string()))?;
                context
                    .update_reasoning(reasoning)
                    .map_err(|error| CommandError::message(error.to_string()))?;
                Ok(CommandControl::Continue)
            }
            _ => Err(CommandError::usage("/reasoning [none|low|medium|high]")),
        }
    })
}
