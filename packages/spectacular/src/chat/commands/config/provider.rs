use crate::chat::commands::{
    ChatCommand, ChatCommandContext, ChatCommandFuture, ChatCommandResult, ChatCompletionContext,
    CompletionFieldSpec, CompletionSubcommandSpec, CompletionValueValidation,
};
use crate::chat::ChatError;
use crate::config_fields::{named_args, provider_type_enabled};
use spectacular_commands::CommandError;
use spectacular_llms::{open_browser, start_openai_browser_auth, OPENAI_PROVIDER_ID};

/// Returns enabled provider backend ids from the provider registry.
fn enabled_provider_type_values(ctx: &ChatCompletionContext<'_>) -> Result<Vec<String>, ChatError> {
    Ok(ctx.enabled_provider_type_ids())
}

/// Returns no suggestions for free-form or secret command fields.
fn no_values(_: &ChatCompletionContext<'_>) -> Result<Vec<String>, ChatError> {
    Ok(Vec::new())
}

/// Returns configured provider names from persisted chat configuration.
fn configured_provider_values(ctx: &ChatCompletionContext<'_>) -> Result<Vec<String>, ChatError> {
    ctx.configured_provider_names()
}

/// Returns the OpenAI provider id used by the provider browser-auth flow.
fn available_provider_values(_: &ChatCompletionContext<'_>) -> Result<Vec<String>, ChatError> {
    Ok(vec![OPENAI_PROVIDER_ID.to_owned()])
}

const PROVIDER_ADD_FIELDS: &[CompletionFieldSpec] = &[
    CompletionFieldSpec {
        name: "provider",
        summary: "provider backend",
        required: true,
        values: enabled_provider_type_values,
        validation: CompletionValueValidation::OneOfValues,
    },
    CompletionFieldSpec {
        name: "apikey",
        summary: "provider API key",
        required: true,
        values: no_values,
        validation: CompletionValueValidation::None,
    },
];

const PROVIDER_REMOVE_FIELDS: &[CompletionFieldSpec] = &[CompletionFieldSpec {
    name: "name",
    summary: "configured provider name",
    required: true,
    values: configured_provider_values,
    validation: CompletionValueValidation::None,
}];

const PROVIDER_AUTH_FIELDS: &[CompletionFieldSpec] = &[CompletionFieldSpec {
    name: "provider",
    summary: "available providers to perform auth",
    required: true,
    values: available_provider_values,
    validation: CompletionValueValidation::OneOfValues,
}];

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
    CompletionSubcommandSpec {
        name: "auth",
        summary: "Authenticate provider",
        fields: PROVIDER_AUTH_FIELDS,
    },
];

/// Builds the `/provider` chat command metadata and completion definition.
pub fn command() -> ChatCommand {
    ChatCommand {
        name: "provider",
        usage: "/provider add provider:<provider> apikey:<apikey> | /provider auth provider:openai | /provider remove name:<name> confirm:true",
        summary: "Manage configured providers",
        completion: PROVIDER_SUBCOMMANDS,
        execute,
    }
}

/// Routes `/provider` subcommands to the matching provider configuration handler.
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
            Some((subcommand, fields)) if subcommand == "auth" => {
                provider_auth(context, fields).await
            }
            Some((subcommand, fields)) if subcommand == "remove" => {
                provider_remove(context, fields)
            }
            _ => ChatCommandResult::error(CommandError::usage(command().usage).to_string()),
        }
    })
}

/// Persists API-key credentials for a provider and refreshes its model cache.
fn provider_add(context: ChatCommandContext<'_>, fields: &[String]) -> ChatCommandResult {
    let args = match named_args(fields, &["provider", "apikey"]) {
        Ok(args) => args,
        Err(error) => return ChatCommandResult::error(error.to_string()),
    };
    let provider_type = match args.require("provider") {
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
    if let Err(error) = context.model.set_provider_api_key(provider_type, apikey) {
        return ChatCommandResult::error(error.to_string());
    }

    match context.model.refresh_provider_model_cache(provider_type) {
        Ok(count) => context.notice(&format!("cached {count} {provider_type} models")),
        Err(error) => context.notice(&format!(
            "could not refresh {provider_type} models: {error}"
        )),
    }

    context.success(&format!("provider added: {provider_type}"));
    ChatCommandResult::success()
}

/// Runs the OpenAI browser auth flow and persists refreshed provider credentials.
async fn provider_auth(context: ChatCommandContext<'_>, fields: &[String]) -> ChatCommandResult {
    let args = match named_args(fields, &["provider"]) {
        Ok(args) => args,
        Err(error) => return ChatCommandResult::error(error.to_string()),
    };
    let provider = match args.require("provider") {
        Ok(value) => value,
        Err(error) => return ChatCommandResult::error(error.to_string()),
    };
    if let Err(error) = context.model.validate_openai_auth_provider(provider) {
        return ChatCommandResult::error(error.to_string());
    }

    let flow = match start_openai_browser_auth() {
        Ok(flow) => flow,
        Err(error) => return ChatCommandResult::error(error.to_string()),
    };
    context.notice(&format!(
        "open this URL to sign in: {}",
        flow.authorize_url()
    ));
    if let Err(error) = open_browser(flow.authorize_url()) {
        context.notice(&format!("could not open browser automatically: {error}"));
    }

    let auth = match context.work(flow.finish()).await {
        Ok(auth) => auth,
        Err(error) => return ChatCommandResult::error(error.to_string()),
    };
    let email = auth.email.clone();
    let plan = auth.plan_type.clone();
    if let Err(error) = context.model.set_openai_provider_auth(provider, auth) {
        return ChatCommandResult::error(error.to_string());
    }
    match context.model.refresh_provider_model_cache(provider) {
        Ok(count) => context.notice(&format!("cached {count} OpenAI models")),
        Err(error) => context.notice(&format!("could not refresh OpenAI models: {error}")),
    }

    context.success(&format!(
        "provider authenticated: {provider}{}{}",
        email
            .as_deref()
            .map(|email| format!(" ({email})"))
            .unwrap_or_default(),
        plan.as_deref()
            .map(|plan| format!(" [{plan}]"))
            .unwrap_or_default()
    ));
    ChatCommandResult::success()
}

/// Removes a provider after explicit confirmation and reports orphaned model keys.
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
            .notice("provider removal requires confirm:true; existing models will become invalid");
        return ChatCommandResult::success();
    }

    match context.model.remove_provider(name) {
        Ok(models) => {
            context.success(&format!("provider removed: {name}"));
            if !models.is_empty() {
                context.notice(&format!("orphaned models: {}", models.join(", ")));
            }
            ChatCommandResult::success()
        }
        Err(error) => ChatCommandResult::error(error.to_string()),
    }
}
