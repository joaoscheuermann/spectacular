use crate::chat::commands::{
    ChatCommand, ChatCommandContext, ChatCommandFuture, ChatCommandResult, CompletionFieldSpec,
    CompletionSubcommandSpec, CompletionValueValidation,
};
use crate::config_fields::{named_args, parse_task};
use ::commands::{CommandError, NamedArgs};

const TASK_USAGE: &str = "/task set task:<general|coding|labeling> model:<model-key>";

const TASK_SET_FIELDS: &[CompletionFieldSpec] = &[
    CompletionFieldSpec {
        name: "task",
        summary: "task slot",
        required: true,
        validation: CompletionValueValidation::OneOfValues,
    },
    CompletionFieldSpec {
        name: "model",
        summary: "saved model key",
        required: true,
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
        usage: TASK_USAGE,
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
            _ => ChatCommandResult::error(CommandError::usage(TASK_USAGE).to_string()),
        }
    })
}

/// Assigns a saved model to a task slot and reports the updated mapping.
fn task_set(context: ChatCommandContext<'_>, fields: &[String]) -> ChatCommandResult {
    finish(run_task_set(context, fields))
}

fn run_task_set(
    context: ChatCommandContext<'_>,
    fields: &[String],
) -> Result<ChatCommandResult, String> {
    let spec = TaskSetSpec::parse(fields)?;
    context
        .model
        .set_task_model(spec.task, &spec.model)
        .map_err(|error| error.to_string())?;
    context.success(&format!(
        "task updated: {} -> {}",
        spec.task.as_str(),
        spec.model
    ));
    Ok(ChatCommandResult::success())
}

#[derive(Debug, Eq, PartialEq)]
struct TaskSetSpec {
    task: config::TaskModelSlot,
    model: String,
}

impl TaskSetSpec {
    fn parse(fields: &[String]) -> Result<Self, String> {
        let args = parse_fields(fields, &["task", "model"])?;
        Ok(Self {
            task: parse_task_field(&args)?,
            model: require_field(&args, "model")?,
        })
    }
}

fn parse_task_field(args: &NamedArgs) -> Result<config::TaskModelSlot, String> {
    args.require("task")
        .and_then(parse_task)
        .map_err(|error| error.to_string())
}

fn parse_fields(fields: &[String], allowed: &[&str]) -> Result<NamedArgs, String> {
    named_args(fields, allowed).map_err(|error| error.to_string())
}

fn require_field(args: &NamedArgs, name: &'static str) -> Result<String, String> {
    args.require(name)
        .map(str::to_owned)
        .map_err(|error| error.to_string())
}

fn finish(result: Result<ChatCommandResult, String>) -> ChatCommandResult {
    result.unwrap_or_else(ChatCommandResult::error)
}

#[cfg(test)]
mod tests {
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/unit/chat/commands/config/task.rs"
    ));
}
