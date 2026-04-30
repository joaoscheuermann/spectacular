use crate::chat::ChatContext;
use spectacular_commands::{Command, CommandControl, CommandError, CommandFuture};

pub fn command() -> Command<ChatContext> {
    Command {
        name: "retry",
        usage: "/retry",
        summary: "Retry the latest prompt",
        execute,
    }
}

fn execute<'a>(context: &'a mut ChatContext, args: Vec<String>) -> CommandFuture<'a> {
    Box::pin(async move {
        if !args.is_empty() {
            return Err(CommandError::usage("/retry"));
        }

        let prompt = context
            .session
            .truncate_after_latest_user_prompt()
            .map_err(|error| CommandError::message(error.to_string()))?;
        context
            .session
            .append_runtime_defaults(&context.runtime, "retry")
            .map_err(|error| CommandError::message(error.to_string()))?;
        context.renderer.dim("retrying latest prompt...");
        context
            .run_user_prompt(prompt, false, true)
            .await
            .map_err(|error| CommandError::message(error.to_string()))?;
        Ok(CommandControl::Continue)
    })
}
