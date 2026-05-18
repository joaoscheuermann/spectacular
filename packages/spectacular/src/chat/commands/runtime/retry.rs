use crate::chat::commands::{
    ChatCommand, ChatCommandContext, ChatCommandFuture, ChatCommandResult,
};
use crate::chat::model::ChatRunRequestModel;
use spectacular_commands::CommandError;

/// Handles command for this module.
pub fn command() -> ChatCommand {
    ChatCommand {
        name: "retry",
        usage: "/retry",
        summary: "Retry the latest prompt",
        completion: &[],
        execute,
    }
}

/// Executes the tool with the provided arguments and cancellation handle.
fn execute<'a>(mut context: ChatCommandContext<'a>, args: Vec<String>) -> ChatCommandFuture<'a> {
    Box::pin(async move {
        if !args.is_empty() {
            return ChatCommandResult::error(CommandError::usage("/retry").to_string());
        }

        let prompt = match context.model.truncate_after_latest_user_prompt() {
            Ok(prompt) => prompt,
            Err(error) => return ChatCommandResult::error(error.to_string()),
        };
        if let Err(error) = context.model.append_runtime_defaults("retry") {
            return ChatCommandResult::error(error.to_string());
        }
        let request = ChatRunRequestModel {
            prompt,
            prompt_event_id: None,
            render_user_prompt: false,
            retry_existing_prompt: true,
            runtime: context.model.runtime().clone(),
        };

        context.notice("retrying latest prompt...");
        if let Err(error) = context.run_prompt(request).await {
            return ChatCommandResult::error(error.to_string());
        }

        ChatCommandResult::success()
    })
}

#[cfg(test)]
mod tests {
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/unit/chat/commands/runtime/retry.rs"
    ));
}
