use crate::chat::commands::{
    ChatCommand, ChatCommandContext, ChatCommandFuture, ChatCommandResult,
};
use spectacular_commands::CommandError;
use spectacular_config::ReasoningLevel;
use std::str::FromStr;

pub fn command() -> ChatCommand {
    ChatCommand {
        name: "reasoning",
        usage: "/reasoning [none|low|medium|high]",
        summary: "Show or update coding reasoning",
        completion: &[],
        execute,
    }
}

fn execute<'a>(context: ChatCommandContext<'a>, args: Vec<String>) -> ChatCommandFuture<'a> {
    Box::pin(async move {
        match args.as_slice() {
            [] => {
                context.notice(&context.model.reasoning_notice());
                ChatCommandResult::success()
            }
            [reasoning] => {
                let reasoning = match ReasoningLevel::from_str(reasoning) {
                    Ok(reasoning) => reasoning,
                    Err(error) => return ChatCommandResult::error(error.to_string()),
                };
                if let Err(error) = context.model.update_reasoning(reasoning) {
                    return ChatCommandResult::error(error.to_string());
                }

                context.success("coding reasoning updated");

                ChatCommandResult::success()
            }
            _ => ChatCommandResult::error(
                CommandError::usage("/reasoning [none|low|medium|high]").to_string(),
            ),
        }
    })
}

#[cfg(test)]
mod tests {
    include!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/unit/chat/commands/config/reasoning.rs"));
}
