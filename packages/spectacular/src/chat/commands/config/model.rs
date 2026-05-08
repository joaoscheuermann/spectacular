use crate::chat::commands::config::completion_values::{
    cached_model_id_values, configured_provider_values, confirm_true_values, reasoning_values,
    saved_model_values,
};
use crate::chat::commands::{
    ChatCommand, ChatCommandContext, ChatCommandFuture, ChatCommandResult, CompletionFieldSpec,
    CompletionSubcommandSpec, CompletionValueValidation,
};
use crate::chat::validate_cached_model_reasoning;
use crate::config_fields::{named_args, parse_reasoning};
use spectacular_commands::CommandError;
use spectacular_config::{ModelCache, ReasoningLevel};

const MODEL_ADD_FIELDS: &[CompletionFieldSpec] = &[
    CompletionFieldSpec {
        name: "provider",
        summary: "configured provider name",
        required: true,
        values: configured_provider_values,
        validation: CompletionValueValidation::None,
    },
    CompletionFieldSpec {
        name: "id",
        summary: "model ID from the selected provider",
        required: true,
        values: cached_model_id_values,
        validation: CompletionValueValidation::None,
    },
    CompletionFieldSpec {
        name: "reasoning",
        summary: "reasoning level",
        required: true,
        values: reasoning_values,
        validation: CompletionValueValidation::OneOfValues,
    },
    CompletionFieldSpec {
        name: "name",
        summary: "optional saved model name",
        required: false,
        values: saved_model_values,
        validation: CompletionValueValidation::None,
    },
];

const MODEL_EDIT_FIELDS: &[CompletionFieldSpec] = &[
    CompletionFieldSpec {
        name: "name",
        summary: "saved model key",
        required: true,
        values: saved_model_values,
        validation: CompletionValueValidation::None,
    },
    CompletionFieldSpec {
        name: "provider",
        summary: "replacement provider name",
        required: false,
        values: configured_provider_values,
        validation: CompletionValueValidation::None,
    },
    CompletionFieldSpec {
        name: "id",
        summary: "replacement model ID",
        required: false,
        values: cached_model_id_values,
        validation: CompletionValueValidation::None,
    },
    CompletionFieldSpec {
        name: "reasoning",
        summary: "replacement reasoning level",
        required: false,
        values: reasoning_values,
        validation: CompletionValueValidation::OneOfValues,
    },
];

const MODEL_REMOVE_FIELDS: &[CompletionFieldSpec] = &[
    CompletionFieldSpec {
        name: "name",
        summary: "saved model key",
        required: true,
        values: saved_model_values,
        validation: CompletionValueValidation::None,
    },
    CompletionFieldSpec {
        name: "confirm",
        summary: "explicit deletion confirmation",
        required: false,
        values: confirm_true_values,
        validation: CompletionValueValidation::OneOfValues,
    },
];

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
        usage: "/model add provider:<provider> id:<model-id> reasoning:<level> [name:<name>] | /model edit name:<name> [provider:<provider>] [id:<model-id>] [reasoning:<level>] | /model remove name:<name> confirm:true",
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
            _ => ChatCommandResult::error(CommandError::usage(command().usage).to_string()),
        }
    })
}

/// Adds a saved model after validating cached provider metadata and reasoning support.
fn model_add(context: ChatCommandContext<'_>, fields: &[String]) -> ChatCommandResult {
    let args = match named_args(fields, &["provider", "id", "reasoning", "name"]) {
        Ok(args) => args,
        Err(error) => return ChatCommandResult::error(error.to_string()),
    };
    let provider = match args.require("provider") {
        Ok(value) => value,
        Err(error) => return ChatCommandResult::error(error.to_string()),
    };
    let model_id = match args.require("id") {
        Ok(value) => value,
        Err(error) => return ChatCommandResult::error(error.to_string()),
    };
    let reasoning_value = match args.require("reasoning") {
        Ok(value) => value,
        Err(error) => return ChatCommandResult::error(error.to_string()),
    };
    let reasoning = match parse_reasoning(reasoning_value) {
        Ok(value) => value,
        Err(error) => return ChatCommandResult::error(error.to_string()),
    };

    let cache = match context.model.config_io().read_model_cache_or_default() {
        Ok(cache) => cache,
        Err(error) => return ChatCommandResult::error(error.to_string()),
    };
    if let Err(error) = validate_reasoning(&cache, provider, model_id, reasoning) {
        return ChatCommandResult::error(error.to_string());
    }

    match context.model.add_model(
        provider,
        model_id,
        reasoning,
        args.optional("name").map(str::to_owned),
    ) {
        Ok(key) => {
            context.success(&format!("model added: {key}"));
            ChatCommandResult::success()
        }
        Err(error) => ChatCommandResult::error(error.to_string()),
    }
}

/// Updates an existing saved model and refreshes the runtime when needed.
fn model_edit(context: ChatCommandContext<'_>, fields: &[String]) -> ChatCommandResult {
    let args = match named_args(fields, &["name", "provider", "id", "reasoning"]) {
        Ok(args) => args,
        Err(error) => return ChatCommandResult::error(error.to_string()),
    };
    let name = match args.require("name") {
        Ok(value) => value,
        Err(error) => return ChatCommandResult::error(error.to_string()),
    };
    let reasoning = match args.optional("reasoning").map(parse_reasoning).transpose() {
        Ok(value) => value,
        Err(error) => return ChatCommandResult::error(error.to_string()),
    };

    if args.optional("provider").is_some() || args.optional("id").is_some() || reasoning.is_some() {
        let config = match context.model.config_io().read_config_or_default() {
            Ok(config) => config,
            Err(error) => return ChatCommandResult::error(error.to_string()),
        };
        let current = match config.models.get(name) {
            Some(current) => current,
            None => return ChatCommandResult::error(format!("model `{name}` is not configured")),
        };
        let provider = args
            .optional("provider")
            .unwrap_or(current.provider.as_str());
        let model_id = args.optional("id").unwrap_or(current.model.as_str());
        let reasoning = reasoning.unwrap_or(current.reasoning);
        let cache = match context.model.config_io().read_model_cache_or_default() {
            Ok(cache) => cache,
            Err(error) => return ChatCommandResult::error(error.to_string()),
        };
        if let Err(error) = validate_reasoning(&cache, provider, model_id, reasoning) {
            return ChatCommandResult::error(error.to_string());
        }
    }

    match context.model.edit_model(
        name,
        args.optional("provider").map(str::to_owned),
        args.optional("id").map(str::to_owned),
        reasoning,
    ) {
        Ok(_) => {
            context.success(&format!("model updated: {name}"));
            ChatCommandResult::success()
        }
        Err(error) => ChatCommandResult::error(error.to_string()),
    }
}

/// Removes a saved model after explicit confirmation and reports invalid task references.
fn model_remove(context: ChatCommandContext<'_>, fields: &[String]) -> ChatCommandResult {
    let args = match named_args(fields, &["name", "confirm"]) {
        Ok(args) => args,
        Err(error) => return ChatCommandResult::error(error.to_string()),
    };
    let name = match args.require("name") {
        Ok(value) => value,
        Err(error) => return ChatCommandResult::error(error.to_string()),
    };

    if args.optional("confirm") != Some("true") {
        context
            .notice("model removal requires confirm:true; referenced tasks will be left invalid");
        return ChatCommandResult::success();
    }

    match context.model.remove_model(name) {
        Ok(references) => {
            context.success(&format!("model removed: {name}"));
            if !references.is_empty() {
                context.notice(&format!(
                    "invalid task references: {}",
                    references
                        .iter()
                        .map(|slot| slot.as_str())
                        .collect::<Vec<_>>()
                        .join(", ")
                ));
            }
            ChatCommandResult::success()
        }
        Err(error) => ChatCommandResult::error(error.to_string()),
    }
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
