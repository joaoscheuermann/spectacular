use crate::chat::ChatContext;
use spectacular_commands::{Command, CommandControl, CommandError, CommandFuture};
use spectacular_config::ReasoningLevel;
use std::str::FromStr;

pub fn command() -> Command<ChatContext> {
    Command {
        name: "model",
        usage: "/model [model-id none|low|medium|high]",
        summary: "Show or update coding model",
        execute,
    }
}

fn execute<'a>(context: &'a mut ChatContext, args: Vec<String>) -> CommandFuture<'a> {
    Box::pin(async move {
        match args.as_slice() {
            [] => {
                context.show_coding_model();
                Ok(CommandControl::Continue)
            }
            [model, reasoning] => {
                let reasoning = ReasoningLevel::from_str(reasoning)
                    .map_err(|error| CommandError::message(error.to_string()))?;
                context
                    .update_coding_model(model, reasoning)
                    .map_err(|error| CommandError::message(error.to_string()))?;
                Ok(CommandControl::Continue)
            }
            _ => Err(CommandError::usage(
                "/model [model-id none|low|medium|high]",
            )),
        }
    })
}
