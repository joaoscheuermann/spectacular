use crate::chat::commands::{
    ChatCommand, ChatCommandContext, ChatCommandFuture, ChatCommandResult, CompletionFieldSpec,
    CompletionSubcommandSpec, CompletionValueValidation,
};
use crate::config_fields::{named_args, provider_type_enabled};
use ::commands::{CommandError, NamedArgs};
use ::llms::{open_browser, start_openai_browser_auth};

const PROVIDER_USAGE: &str = "/provider add provider:<provider> apikey:<apikey> | /provider auth provider:openai | /provider remove name:<name> confirm:true";

const PROVIDER_ADD_FIELDS: &[CompletionFieldSpec] = &[
    CompletionFieldSpec {
        name: "provider",
        summary: "provider backend",
        required: true,
        validation: CompletionValueValidation::OneOfValues,
    },
    CompletionFieldSpec {
        name: "apikey",
        summary: "provider API key",
        required: true,
        validation: CompletionValueValidation::None,
    },
];

const PROVIDER_REMOVE_FIELDS: &[CompletionFieldSpec] = &[CompletionFieldSpec {
    name: "name",
    summary: "configured provider name",
    required: true,
    validation: CompletionValueValidation::None,
}];

const PROVIDER_AUTH_FIELDS: &[CompletionFieldSpec] = &[CompletionFieldSpec {
    name: "provider",
    summary: "available providers to perform auth",
    required: true,
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
        usage: PROVIDER_USAGE,
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
            _ => ChatCommandResult::error(CommandError::usage(PROVIDER_USAGE).to_string()),
        }
    })
}

/// Persists API-key credentials for a provider and refreshes its model cache.
fn provider_add(context: ChatCommandContext<'_>, fields: &[String]) -> ChatCommandResult {
    finish(run_provider_add(context, fields))
}

/// Runs the OpenAI browser auth flow and persists refreshed provider credentials.
async fn provider_auth(context: ChatCommandContext<'_>, fields: &[String]) -> ChatCommandResult {
    finish(run_provider_auth(context, fields).await)
}

/// Removes a provider after explicit confirmation and reports orphaned model keys.
fn provider_remove(context: ChatCommandContext<'_>, fields: &[String]) -> ChatCommandResult {
    finish(run_provider_remove(context, fields))
}

fn run_provider_add(
    context: ChatCommandContext<'_>,
    fields: &[String],
) -> Result<ChatCommandResult, String> {
    let spec = ProviderAddSpec::parse(fields)?;
    context
        .model
        .set_provider_api_key(&spec.provider, &spec.apikey)
        .map_err(|error| error.to_string())?;
    report_cache_refresh(&context, &spec.provider, &spec.provider);
    context.success(&format!("provider added: {}", spec.provider));
    Ok(ChatCommandResult::success())
}

async fn run_provider_auth(
    context: ChatCommandContext<'_>,
    fields: &[String],
) -> Result<ChatCommandResult, String> {
    let spec = ProviderAuthSpec::parse(fields)?;
    context
        .model
        .validate_openai_auth_provider(&spec.provider)
        .map_err(|error| error.to_string())?;
    let flow = start_openai_browser_auth().map_err(|error| error.to_string())?;
    announce_auth_url(&context, flow.authorize_url());

    let auth = context
        .work(flow.finish())
        .await
        .map_err(|error| error.to_string())?;
    let email = auth.email.clone();
    let plan = auth.plan_type.clone();
    context
        .model
        .set_openai_provider_auth(&spec.provider, auth)
        .map_err(|error| error.to_string())?;
    report_cache_refresh(&context, &spec.provider, "OpenAI");
    report_auth_success(&context, &spec.provider, email.as_deref(), plan.as_deref());
    Ok(ChatCommandResult::success())
}

fn run_provider_remove(
    context: ChatCommandContext<'_>,
    fields: &[String],
) -> Result<ChatCommandResult, String> {
    let spec = ProviderRemoveSpec::parse(fields)?;
    if !spec.confirmed {
        context
            .notice("provider removal requires confirm:true; existing models will become invalid");
        return Ok(ChatCommandResult::success());
    }

    let models = context
        .model
        .remove_provider(&spec.name)
        .map_err(|error| error.to_string())?;
    context.success(&format!("provider removed: {}", spec.name));
    report_orphaned_models(&context, &models);
    Ok(ChatCommandResult::success())
}

#[derive(Debug, Eq, PartialEq)]
struct ProviderAddSpec {
    provider: String,
    apikey: String,
}

impl ProviderAddSpec {
    fn parse(fields: &[String]) -> Result<Self, String> {
        let args = parse_fields(fields, &["provider", "apikey"])?;
        let provider = require_field(&args, "provider")?;
        if !provider_type_enabled(&provider) {
            return Err(format!("provider type `{provider}` is not available"));
        }

        Ok(Self {
            provider,
            apikey: require_field(&args, "apikey")?,
        })
    }
}

#[derive(Debug, Eq, PartialEq)]
struct ProviderAuthSpec {
    provider: String,
}

impl ProviderAuthSpec {
    fn parse(fields: &[String]) -> Result<Self, String> {
        let args = parse_fields(fields, &["provider"])?;
        Ok(Self {
            provider: require_field(&args, "provider")?,
        })
    }
}

#[derive(Debug, Eq, PartialEq)]
struct ProviderRemoveSpec {
    name: String,
    confirmed: bool,
}

impl ProviderRemoveSpec {
    fn parse(fields: &[String]) -> Result<Self, String> {
        let args = parse_fields(fields, &["name", "confirm"])?;
        Ok(Self {
            name: require_field(&args, "name")?,
            confirmed: args.optional("confirm") == Some("true"),
        })
    }
}

fn parse_fields(fields: &[String], allowed: &[&str]) -> Result<NamedArgs, String> {
    named_args(fields, allowed).map_err(|error| error.to_string())
}

fn require_field(args: &NamedArgs, name: &'static str) -> Result<String, String> {
    args.require(name)
        .map(str::to_owned)
        .map_err(|error| error.to_string())
}

fn announce_auth_url(context: &ChatCommandContext<'_>, authorize_url: &str) {
    context.notice(&format!("open this URL to sign in: {authorize_url}"));
    if let Err(error) = open_browser(authorize_url) {
        context.notice(&format!("could not open browser automatically: {error}"));
    }
}

fn report_cache_refresh(context: &ChatCommandContext<'_>, provider: &str, label: &str) {
    match context.model.refresh_provider_model_cache(provider) {
        Ok(count) => context.notice(&format!("cached {count} {label} models")),
        Err(error) => context.notice(&format!("could not refresh {label} models: {error}")),
    }
}

fn report_auth_success(
    context: &ChatCommandContext<'_>,
    provider: &str,
    email: Option<&str>,
    plan: Option<&str>,
) {
    context.success(&format!(
        "provider authenticated: {provider}{}{}",
        email_suffix(email),
        plan_suffix(plan)
    ));
}

fn email_suffix(email: Option<&str>) -> String {
    email.map(|email| format!(" ({email})")).unwrap_or_default()
}

fn plan_suffix(plan: Option<&str>) -> String {
    plan.map(|plan| format!(" [{plan}]")).unwrap_or_default()
}

fn report_orphaned_models(context: &ChatCommandContext<'_>, models: &[String]) {
    if !models.is_empty() {
        context.notice(&format!("orphaned models: {}", models.join(", ")));
    }
}

fn finish(result: Result<ChatCommandResult, String>) -> ChatCommandResult {
    result.unwrap_or_else(ChatCommandResult::error)
}

#[cfg(test)]
mod tests {
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/unit/chat/commands/config/provider.rs"
    ));
}
