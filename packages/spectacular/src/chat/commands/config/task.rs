use crate::chat::commands::{
    ChatCommand, ChatCommandContext, ChatCommandFuture, ChatCommandResult, ChatCompletionContext,
    CompletionFieldSpec, CompletionSubcommandSpec, CompletionValueValidation,
};
use crate::config_fields::{named_args, parse_task};
use spectacular_commands::CommandError;
use spectacular_config::TaskModelSlot;

const TASK_SET_FIELDS: &[CompletionFieldSpec] = &[
    CompletionFieldSpec {
        name: "task",
        summary: "task slot",
        required: true,
        values: |_: &ChatCompletionContext<'_>| {
            Ok(TaskModelSlot::ALL
                .into_iter()
                .map(|slot| slot.as_str().to_owned())
                .collect())
        },
        validation: CompletionValueValidation::OneOfValues,
    },
    CompletionFieldSpec {
        name: "model",
        summary: "saved model key",
        required: true,
        values: |ctx: &ChatCompletionContext<'_>| ctx.saved_model_names(),
        validation: CompletionValueValidation::None,
    },
];

const TASK_SUBCOMMANDS: &[CompletionSubcommandSpec] = &[CompletionSubcommandSpec {
    name: "set",
    summary: "Assign task model",
    fields: TASK_SET_FIELDS,
}];

/// Builds the `/task` chat command metadata and completion definition.
pub fn command() -> ChatCommand {
    ChatCommand {
        name: "task",
        usage: "/task set task:<general|coding|labeling> model:<model-key>",
        summary: "Assign task models",
        completion: TASK_SUBCOMMANDS,
        execute,
    }
}

/// Routes `/task` subcommands to task-model assignment handlers.
fn execute<'a>(context: ChatCommandContext<'a>, args: Vec<String>) -> ChatCommandFuture<'a> {
    Box::pin(async move {
        match args.split_first() {
            Some((subcommand, fields)) if subcommand == "set" => task_set(context, fields),
            _ => ChatCommandResult::error(CommandError::usage(command().usage).to_string()),
        }
    })
}

/// Assigns a saved model to a task slot and reports the updated mapping.
fn task_set(context: ChatCommandContext<'_>, fields: &[String]) -> ChatCommandResult {
    let args = match named_args(fields, &["task", "model"]) {
        Ok(args) => args,
        Err(error) => return ChatCommandResult::error(error.to_string()),
    };
    let task = match args.require("task").and_then(parse_task) {
        Ok(task) => task,
        Err(error) => return ChatCommandResult::error(error.to_string()),
    };
    let model = match args.require("model") {
        Ok(value) => value,
        Err(error) => return ChatCommandResult::error(error.to_string()),
    };

    match context.model.set_task_model(task, model) {
        Ok(_) => {
            context.success(&format!("task updated: {} -> {model}", task.as_str()));
            ChatCommandResult::success()
        }
        Err(error) => ChatCommandResult::error(error.to_string()),
    }
}
