use crate::chat::commands::{
    ChatCommand, ChatCommandContext, ChatCommandFuture, ChatCommandResult, SOURCE_PROVIDERS,
    SOURCE_PROVIDER_TYPES,
};
use crate::config_fields::{named_args, provider_type_enabled};
use spectacular_commands::{
    CommandError, CompletionFieldSpec, CompletionSubcommandSpec, CompletionValueSource,
};

const CONFIRM_VALUES: &[&str] = &["true"];

const PROVIDER_ADD_FIELDS: &[CompletionFieldSpec] = &[
    CompletionFieldSpec {
        name: "name",
        summary: "provider display name",
        required: true,
        value_source: CompletionValueSource::Static(&[]),
    },
    CompletionFieldSpec {
        name: "type",
        summary: "provider backend",
        required: true,
        value_source: CompletionValueSource::Dynamic(SOURCE_PROVIDER_TYPES),
    },
    CompletionFieldSpec {
        name: "apikey",
        summary: "provider API key",
        required: true,
        value_source: CompletionValueSource::Static(&[]),
    },
];

const PROVIDER_REMOVE_FIELDS: &[CompletionFieldSpec] = &[
    CompletionFieldSpec {
        name: "name",
        summary: "configured provider name",
        required: true,
        value_source: CompletionValueSource::Dynamic(SOURCE_PROVIDERS),
    },
    CompletionFieldSpec {
        name: "confirm",
        summary: "explicit deletion confirmation",
        required: false,
        value_source: CompletionValueSource::Static(CONFIRM_VALUES),
    },
];

const PROVIDER_SUBCOMMANDS: &[CompletionSubcommandSpec] = &[
    CompletionSubcommandSpec {
        name: "add",
        summary: "Add provider",
        fields: PROVIDER_ADD_FIELDS,
    },
    CompletionSubcommandSpec {
        name: "remove",
        summary: "Remove provider",
        fields: PROVIDER_REMOVE_FIELDS,
    },
];

pub fn command() -> ChatCommand {
    ChatCommand {
        name: "provider",
        usage: "/provider add name:<name> type:<type> apikey:<apikey> | /provider remove name:<name> confirm:true",
        summary: "Manage configured providers",
        completion: PROVIDER_SUBCOMMANDS,
        execute,
    }
}

fn execute<'a>(context: ChatCommandContext<'a>, args: Vec<String>) -> ChatCommandFuture<'a> {
    Box::pin(async move {
        match args.split_first() {
            None => match context.model.provider_notice() {
                Ok(message) => {
                    context.notice(&message);
                    ChatCommandResult::success()
                }
                Err(error) => ChatCommandResult::error(error.to_string()),
            },
            Some((subcommand, fields)) if subcommand == "add" => provider_add(context, fields),
            Some((subcommand, fields)) if subcommand == "remove" => {
                provider_remove(context, fields)
            }
            _ => ChatCommandResult::error(CommandError::usage(command().usage).to_string()),
        }
    })
}

fn provider_add(context: ChatCommandContext<'_>, fields: &[String]) -> ChatCommandResult {
    let args = match named_args(fields, &["name", "type", "apikey"]) {
        Ok(args) => args,
        Err(error) => return ChatCommandResult::error(error.to_string()),
    };
    let name = match args.require("name") {
        Ok(value) => value,
        Err(error) => return ChatCommandResult::error(error.to_string()),
    };
    let provider_type = match args.require("type") {
        Ok(value) => value,
        Err(error) => return ChatCommandResult::error(error.to_string()),
    };
    let apikey = match args.require("apikey") {
        Ok(value) => value,
        Err(error) => return ChatCommandResult::error(error.to_string()),
    };
    if !provider_type_enabled(provider_type) {
        return ChatCommandResult::error(format!(
            "provider type `{provider_type}` is not available"
        ));
    }

    if let Err(error) = context.model.add_provider(name, provider_type, apikey) {
        return ChatCommandResult::error(error.to_string());
    }

    context.success(&format!("provider added: {name} ({provider_type})"));
    ChatCommandResult::success()
}

fn provider_remove(context: ChatCommandContext<'_>, fields: &[String]) -> ChatCommandResult {
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
            .notice("provider removal requires confirm:true and does not delete associated models");
        return ChatCommandResult::success();
    }

    match context.model.remove_provider(name) {
        Ok(orphaned) => {
            context.success(&format!("provider removed: {name}"));
            if !orphaned.is_empty() {
                context.notice(&format!("orphaned models: {}", orphaned.join(", ")));
            }
            ChatCommandResult::success()
        }
        Err(error) => ChatCommandResult::error(error.to_string()),
    }
}
