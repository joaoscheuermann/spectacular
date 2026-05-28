use crate::chat::commands::{
    ChatCommand, ChatCommandContext, ChatCommandFuture, ChatCommandResult, CompletionFieldSpec,
    CompletionSubcommandSpec, CompletionValueValidation,
};
use crate::chat::validate_cached_model_reasoning;
use crate::config_fields::{named_args, parse_reasoning};
use ::commands::{CommandError, NamedArgs};
use ::config::{ModelCache, ReasoningLevel};

const MODEL_USAGE: &str = "/model add provider:<provider> id:<model-id> reasoning:<level> [name:<name>] | /model edit name:<name> [provider:<provider>] [id:<model-id>] [reasoning:<level>] | /model remove name:<name> confirm:true";

const MODEL_ADD_FIELDS: &[CompletionFieldSpec] = &[
    CompletionFieldSpec {
        name: "provider",
        summary: "configured provider name",
        required: true,
        validation: CompletionValueValidation::None,
    },
    CompletionFieldSpec {
        name: "id",
        summary: "model ID from the selected provider",
        required: true,
        validation: CompletionValueValidation::None,
    },
    CompletionFieldSpec {
        name: "reasoning",
        summary: "reasoning level",
        required: true,
        validation: CompletionValueValidation::OneOfValues,
    },
    CompletionFieldSpec {
        name: "name",
        summary: "optional saved model name",
        required: false,
        validation: CompletionValueValidation::None,
    },
];

const MODEL_EDIT_FIELDS: &[CompletionFieldSpec] = &[
    CompletionFieldSpec {
        name: "name",
        summary: "saved model key",
        required: true,
        validation: CompletionValueValidation::None,
    },
    CompletionFieldSpec {
        name: "provider",
        summary: "replacement provider name",
        required: false,
        validation: CompletionValueValidation::None,
    },
    CompletionFieldSpec {
        name: "id",
        summary: "replacement model ID",
        required: false,
        validation: CompletionValueValidation::None,
    },
    CompletionFieldSpec {
        name: "reasoning",
        summary: "replacement reasoning level",
        required: false,
        validation: CompletionValueValidation::OneOfValues,
    },
];

const MODEL_REMOVE_FIELDS: &[CompletionFieldSpec] = &[CompletionFieldSpec {
    name: "name",
    summary: "saved model key",
    required: true,
    validation: CompletionValueValidation::None,
}];

const MODEL_SUBCOMMANDS: &[CompletionSubcommandSpec] = &[
    CompletionSubcommandSpec {
        name: "add",
        summary: "Add model",
        fields: MODEL_ADD_FIELDS,
    },
    CompletionSubcommandSpec {
        name: "edit",
        summary: "Edit model",
        fields: MODEL_EDIT_FIELDS,
    },
    CompletionSubcommandSpec {
        name: "remove",
        summary: "Remove model",
        fields: MODEL_REMOVE_FIELDS,
    },
];

/// Builds the `/model` chat command metadata and completion definition.
pub fn command() -> ChatCommand {
    ChatCommand {
        name: "model",
        usage: MODEL_USAGE,
        summary: "Manage saved models",
        completion: MODEL_SUBCOMMANDS,
        execute,
    }
}

/// Routes `/model` subcommands to saved-model configuration handlers.
fn execute<'a>(context: ChatCommandContext<'a>, args: Vec<String>) -> ChatCommandFuture<'a> {
    Box::pin(async move {
        match args.split_first() {
            None => {
                context.notice(&context.model.coding_model_notice());
                ChatCommandResult::success()
            }
            Some((subcommand, fields)) if subcommand == "add" => model_add(context, fields),
            Some((subcommand, fields)) if subcommand == "edit" => model_edit(context, fields),
            Some((subcommand, fields)) if subcommand == "remove" => model_remove(context, fields),
            _ => ChatCommandResult::error(CommandError::usage(MODEL_USAGE).to_string()),
        }
    })
}

/// Adds a saved model after validating cached provider metadata and reasoning support.
fn model_add(context: ChatCommandContext<'_>, fields: &[String]) -> ChatCommandResult {
    finish(run_model_add(context, fields))
}

/// Updates an existing saved model and refreshes the runtime when needed.
fn model_edit(context: ChatCommandContext<'_>, fields: &[String]) -> ChatCommandResult {
    finish(run_model_edit(context, fields))
}

/// Removes a saved model after explicit confirmation and reports invalid task references.
fn model_remove(context: ChatCommandContext<'_>, fields: &[String]) -> ChatCommandResult {
    finish(run_model_remove(context, fields))
}

fn run_model_add(
    context: ChatCommandContext<'_>,
    fields: &[String],
) -> Result<ChatCommandResult, String> {
    let spec = ModelAddSpec::parse(fields)?;
    validate_target(&context, &spec.target)?;
    let key = context
        .model
        .add_model(
            &spec.target.provider,
            &spec.target.model_id,
            spec.target.reasoning,
            spec.name,
        )
        .map_err(|error| error.to_string())?;
    context.success(&format!("model added: {key}"));
    Ok(ChatCommandResult::success())
}

fn run_model_edit(
    context: ChatCommandContext<'_>,
    fields: &[String],
) -> Result<ChatCommandResult, String> {
    let spec = ModelEditSpec::parse(fields)?;
    if let Some(target) = edit_validation_target(&context, &spec)? {
        validate_target(&context, &target)?;
    }

    let name = spec.name.clone();
    context
        .model
        .edit_model(&name, spec.provider, spec.model_id, spec.reasoning)
        .map_err(|error| error.to_string())?;
    context.success(&format!("model updated: {name}"));
    Ok(ChatCommandResult::success())
}

fn run_model_remove(
    context: ChatCommandContext<'_>,
    fields: &[String],
) -> Result<ChatCommandResult, String> {
    let spec = ModelRemoveSpec::parse(fields)?;
    if !spec.confirmed {
        context
            .notice("model removal requires confirm:true; referenced tasks will be left invalid");
        return Ok(ChatCommandResult::success());
    }

    let references = context
        .model
        .remove_model(&spec.name)
        .map_err(|error| error.to_string())?;
    context.success(&format!("model removed: {}", spec.name));
    report_invalid_task_references(&context, &references);
    Ok(ChatCommandResult::success())
}

#[derive(Debug, Eq, PartialEq)]
struct ModelTarget {
    provider: String,
    model_id: String,
    reasoning: ReasoningLevel,
}

#[derive(Debug, Eq, PartialEq)]
struct ModelAddSpec {
    target: ModelTarget,
    name: Option<String>,
}

impl ModelAddSpec {
    fn parse(fields: &[String]) -> Result<Self, String> {
        let args = parse_fields(fields, &["provider", "id", "reasoning", "name"])?;
        let reasoning_value = require_field(&args, "reasoning")?;
        Ok(Self {
            target: ModelTarget {
                provider: require_field(&args, "provider")?,
                model_id: require_field(&args, "id")?,
                reasoning: parse_reasoning_value(&reasoning_value)?,
            },
            name: optional_field(&args, "name"),
        })
    }
}

#[derive(Debug, Eq, PartialEq)]
struct ModelEditSpec {
    name: String,
    provider: Option<String>,
    model_id: Option<String>,
    reasoning: Option<ReasoningLevel>,
}

impl ModelEditSpec {
    fn parse(fields: &[String]) -> Result<Self, String> {
        let args = parse_fields(fields, &["name", "provider", "id", "reasoning"])?;
        Ok(Self {
            name: require_field(&args, "name")?,
            provider: optional_field(&args, "provider"),
            model_id: optional_field(&args, "id"),
            reasoning: args
                .optional("reasoning")
                .map(parse_reasoning_value)
                .transpose()?,
        })
    }

    fn needs_validation(&self) -> bool {
        self.provider.is_some() || self.model_id.is_some() || self.reasoning.is_some()
    }
}

#[derive(Debug, Eq, PartialEq)]
struct ModelRemoveSpec {
    name: String,
    confirmed: bool,
}

impl ModelRemoveSpec {
    fn parse(fields: &[String]) -> Result<Self, String> {
        let args = parse_fields(fields, &["name", "confirm"])?;
        Ok(Self {
            name: require_field(&args, "name")?,
            confirmed: args.optional("confirm") == Some("true"),
        })
    }
}

fn edit_validation_target(
    context: &ChatCommandContext<'_>,
    spec: &ModelEditSpec,
) -> Result<Option<ModelTarget>, String> {
    if !spec.needs_validation() {
        return Ok(None);
    }

    let current = current_model(context, &spec.name)?;
    Ok(Some(ModelTarget {
        provider: spec
            .provider
            .clone()
            .unwrap_or_else(|| current.provider.clone()),
        model_id: spec
            .model_id
            .clone()
            .unwrap_or_else(|| current.model.clone()),
        reasoning: spec.reasoning.unwrap_or(current.reasoning),
    }))
}

fn current_model(
    context: &ChatCommandContext<'_>,
    name: &str,
) -> Result<config::ModelConfig, String> {
    let config = context
        .model
        .config_io()
        .read_config_or_default()
        .map_err(|error| error.to_string())?;
    config
        .models
        .get(name)
        .cloned()
        .ok_or_else(|| format!("model `{name}` is not configured"))
}

fn validate_target(context: &ChatCommandContext<'_>, target: &ModelTarget) -> Result<(), String> {
    let cache = context
        .model
        .config_io()
        .read_model_cache_or_default()
        .map_err(|error| error.to_string())?;
    validate_reasoning(&cache, &target.provider, &target.model_id, target.reasoning)
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

fn optional_field(args: &NamedArgs, name: &str) -> Option<String> {
    args.optional(name).map(str::to_owned)
}

fn parse_reasoning_value(value: &str) -> Result<ReasoningLevel, String> {
    parse_reasoning(value).map_err(|error| error.to_string())
}

fn report_invalid_task_references(
    context: &ChatCommandContext<'_>,
    references: &[config::TaskModelSlot],
) {
    if references.is_empty() {
        return;
    }

    let slots = references
        .iter()
        .map(|slot| slot.as_str())
        .collect::<Vec<_>>()
        .join(", ");
    context.notice(&format!("invalid task references: {slots}"));
}

fn finish(result: Result<ChatCommandResult, String>) -> ChatCommandResult {
    result.unwrap_or_else(ChatCommandResult::error)
}

/// Validates requested reasoning settings against already loaded model metadata cache.
fn validate_reasoning(
    cache: &ModelCache,
    provider: &str,
    model_id: &str,
    reasoning: ReasoningLevel,
) -> Result<(), crate::chat::ChatError> {
    validate_cached_model_reasoning(cache, provider, model_id, reasoning)
}

#[cfg(test)]
mod tests {
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/unit/chat/commands/config/model.rs"
    ));
}
