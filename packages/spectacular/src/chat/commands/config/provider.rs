use crate::chat::commands::{
    ChatCommand, ChatCommandContext, ChatCommandFuture, ChatCommandResult, SOURCE_PROVIDERS,
    SOURCE_PROVIDER_TYPES,
};
use crate::config_fields::{named_args, provider_type_enabled};
use spectacular_commands::{
    CommandError, CompletionFieldSpec, CompletionSubcommandSpec, CompletionValueSource,
};
use spectacular_llms::{open_browser, start_openai_browser_auth, OPENAI_PROVIDER_ID};

const CONFIRM_VALUES: &[&str] = &["true"];
const AUTH_PROVIDER_VALUES: &[&str] = &[OPENAI_PROVIDER_ID];

const PROVIDER_ADD_FIELDS: &[CompletionFieldSpec] = &[
    CompletionFieldSpec {
        name: "provider",
        summary: "provider backend",
        required: true,
        value_source: CompletionValueSource::Dynamic(SOURCE_PROVIDER_TYPES),
    },
    CompletionFieldSpec {
        name: "apikey",
        summary: "provider API key",
        required: false,
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

const PROVIDER_AUTH_FIELDS: &[CompletionFieldSpec] = &[CompletionFieldSpec {
    name: "provider",
    summary: "OpenAI provider type",
    required: true,
    value_source: CompletionValueSource::Static(AUTH_PROVIDER_VALUES),
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

pub fn command() -> ChatCommand {
    ChatCommand {
        name: "provider",
        usage: "/provider add provider:<provider> apikey:<apikey> | /provider auth provider:openai | /provider remove name:<name> confirm:true",
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
