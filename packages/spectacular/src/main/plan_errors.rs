use super::chat;
use spectacular_config::{ConfigError, SpectacularConfig};
use spectacular_llms::ProviderError;
use spectacular_plan::PlanError;

pub(super) fn handle_plan(prompt: &str) -> Result<String, AppError> {
    handle_plan_with_loader(prompt, spectacular_config::read_config)
}
pub(super) fn handle_plan_with_loader(
    prompt: &str,
    load_config: impl FnOnce() -> Result<SpectacularConfig, ConfigError>,
) -> Result<String, AppError> {
    spectacular_plan::run(prompt, load_config)
        .map(str::to_owned)
        .map_err(AppError::Plan)
}

#[derive(Debug)]
pub(super) enum AppError {
    Chat(chat::ChatError),
    Config(ConfigError),
    DebugLog { source: std::io::Error },
    InvalidConfigCommand(String),
    Plan(PlanError),
    Provider { source: Box<ProviderError> },
}

impl From<ConfigError> for AppError {
    fn from(error: ConfigError) -> Self {
        Self::Config(error)
    }
}

impl From<chat::ChatError> for AppError {
    fn from(error: chat::ChatError) -> Self {
        Self::Chat(error)
    }
}

impl From<spectacular_commands::CommandError> for AppError {
    fn from(error: spectacular_commands::CommandError) -> Self {
        Self::InvalidConfigCommand(error.to_string())
    }
}

pub(super) fn user_facing_error(error: &AppError) -> String {
    match error {
        AppError::Plan(PlanError::EmptyPrompt) => {
            "A non-empty prompt is required. Usage: spectacular plan <prompt>".to_owned()
        }
        AppError::Chat(chat::ChatError::Exit) => String::new(),
        AppError::Chat(error) => error.to_string(),
        AppError::Plan(PlanError::Config(config_error)) | AppError::Config(config_error) => {
            format_config_error(config_error)
        }
        AppError::DebugLog { source } => {
            format!("Failed to create LLM debug log beside the executable: {source}.")
        }
        AppError::InvalidConfigCommand(message) => message.to_owned(),
        AppError::Provider { source } => format_provider_error(source),
    }
}

fn format_provider_error(error: &ProviderError) -> String {
    match error {
        ProviderError::CancellationError
        | ProviderError::InvalidApiKey
        | ProviderError::UnsupportedProvider { .. }
        | ProviderError::UnsupportedValidationMode => format_general_provider_error(error),
        ProviderError::ModelFetchFailed { .. }
        | ProviderError::NoModelsReturned { .. }
        | ProviderError::ProviderUnavailable { .. }
        | ProviderError::NetworkError { .. }
        | ProviderError::ContextLimitExceeded { .. } => format_provider_availability_error(error),
        ProviderError::AuthenticationRequired { .. }
        | ProviderError::AuthenticationFailed { .. } => format_provider_auth_error(error),
        ProviderError::StreamUnavailable { provider_name } => {
            format!("{provider_name} streaming is not available yet.")
        }
        ProviderError::StreamError { .. } => format_provider_stream_error(error),
        ProviderError::MalformedResponse { .. }
        | ProviderError::ResponseParsingFailed { .. }
        | ProviderError::CapabilityMismatch { .. } => format_provider_response_error(error),
    }
}

fn format_general_provider_error(error: &ProviderError) -> String {
    match error {
        ProviderError::CancellationError => "Provider call was cancelled.".to_owned(),
        ProviderError::InvalidApiKey => "Invalid API key.".to_owned(),
        ProviderError::UnsupportedProvider { provider_id } => {
            format!("Provider `{provider_id}` is not available.")
        }
        ProviderError::UnsupportedValidationMode => {
            "The selected provider does not support API key validation.".to_owned()
        }
        _ => unreachable!("general provider error formatter received another variant"),
    }
}

fn format_provider_availability_error(error: &ProviderError) -> String {
    match error {
        ProviderError::ModelFetchFailed { provider_name, .. } => {
            format!("Failed to fetch models from {provider_name}.")
        }
        ProviderError::NoModelsReturned { provider_name } => {
            format!("{provider_name} returned no models.")
        }
        ProviderError::ProviderUnavailable { provider_name, .. } => {
            format!("{provider_name} is unavailable. Try again later.")
        }
        ProviderError::NetworkError {
            provider_name,
            reason,
            ..
        } => format!("{provider_name} network request failed: {reason}."),
        ProviderError::ContextLimitExceeded {
            provider_name,
            reason,
        } => format!("{provider_name} context limit exceeded: {reason}."),
        _ => unreachable!("availability formatter received another provider error"),
    }
}

fn format_provider_auth_error(error: &ProviderError) -> String {
    match error {
        ProviderError::AuthenticationRequired { provider_name } => {
            format!("{provider_name} authentication is required.")
        }
        ProviderError::AuthenticationFailed {
            provider_name,
            reason,
            ..
        } => format!("{provider_name} authentication failed: {reason}."),
        _ => unreachable!("auth formatter received another provider error"),
    }
}

fn format_provider_stream_error(error: &ProviderError) -> String {
    let ProviderError::StreamError {
        provider_name,
        code,
        message,
        ..
    } = error
    else {
        unreachable!("stream formatter received another provider error");
    };

    match code {
        Some(code) => format!("{provider_name} stream returned error `{code}`: {message}."),
        None => format!("{provider_name} stream returned error: {message}."),
    }
}

fn format_provider_response_error(error: &ProviderError) -> String {
    match error {
        ProviderError::MalformedResponse {
            provider_name,
            reason,
            ..
        } => format!("{provider_name} returned a malformed response: {reason}."),
        ProviderError::ResponseParsingFailed {
            provider_name,
            reason,
            ..
        } => format!("Failed to parse {provider_name} response: {reason}."),
        ProviderError::CapabilityMismatch {
            provider_name,
            capability,
        } => format!("{provider_name} does not support required capability `{capability}`."),
        _ => unreachable!("response formatter received another provider error"),
    }
}

const SETUP_INSTRUCTION: &str = "Run `spectacular config provider add provider:<provider> apikey:<api-key>` to configure a provider.";

fn format_config_error(error: &ConfigError) -> String {
    match error {
        ConfigError::MissingConfigFile { .. }
        | ConfigError::InvalidJson { .. }
        | ConfigError::SchemaChanged
        | ConfigError::ProviderNotConfigured { .. }
        | ConfigError::MissingProviderApiKey { .. } => format_config_setup_error(error),
        ConfigError::ModelNotConfigured { .. }
        | ConfigError::ModelProviderNotConfigured { .. }
        | ConfigError::MissingTaskModel { .. }
        | ConfigError::InvalidTaskModelReference { .. } => format_config_model_error(error),
        ConfigError::ConfigDirUnavailable
        | ConfigError::ReadFailed { .. }
        | ConfigError::WriteFailed { .. }
        | ConfigError::SerializeFailed { .. } => format_config_io_error(error),
        ConfigError::InvalidProviderType { .. } => format_config_shape_error(error),
        ConfigError::EmptyValue { .. }
        | ConfigError::ProviderAlreadyExists { .. }
        | ConfigError::ModelAlreadyExists { .. } => error.to_string(),
    }
}

fn format_config_setup_error(error: &ConfigError) -> String {
    match error {
        ConfigError::MissingConfigFile { .. } => {
            format!("Configuration is missing. {SETUP_INSTRUCTION}")
        }
        ConfigError::InvalidJson { path, .. } => format!(
            "Configuration file contains invalid JSON at {}. {SETUP_INSTRUCTION}",
            path.display()
        ),
        ConfigError::SchemaChanged => format!("{error}. {SETUP_INSTRUCTION}"),
        ConfigError::ProviderNotConfigured { provider } => format!(
            "Configuration is incomplete: provider `{provider}` is not configured. {SETUP_INSTRUCTION}"
        ),
        ConfigError::MissingProviderApiKey { provider } => format!(
            "Configuration is incomplete: provider `{provider}` has no credentials. {SETUP_INSTRUCTION}"
        ),
        _ => unreachable!("setup formatter received another config error"),
    }
}

fn format_config_model_error(error: &ConfigError) -> String {
    match error {
        ConfigError::ModelNotConfigured { model } => format!(
            "Configuration is incomplete: model `{model}` is not configured. Run `spectacular config model add provider:<provider> id:<model-id> reasoning:<level> [name:<name>]`."
        ),
        ConfigError::ModelProviderNotConfigured { model, provider } => format!(
            "Configuration is incomplete: model `{model}` references missing provider `{provider}`."
        ),
        ConfigError::MissingTaskModel { slot } => format!(
            "Configuration is incomplete: missing `{slot}` model assignment. Run `spectacular config task set task:{slot} model:<model-key>`."
        ),
        ConfigError::InvalidTaskModelReference { slot, model } => format!(
            "Configuration is incomplete: `{slot}` references missing model `{model}`. Run `spectacular config task set task:{slot} model:<model-key>`."
        ),
        _ => unreachable!("model formatter received another config error"),
    }
}

fn format_config_io_error(error: &ConfigError) -> String {
    match error {
        ConfigError::ConfigDirUnavailable => {
            "Could not resolve the Spectacular config directory.".to_owned()
        }
        ConfigError::ReadFailed { path, .. } => {
            format!("Failed to read configuration at {}.", path.display())
        }
        ConfigError::WriteFailed { path, .. } => {
            format!("Failed to write configuration at {}.", path.display())
        }
        ConfigError::SerializeFailed { path, .. } => {
            format!("Failed to serialize configuration at {}.", path.display())
        }
        _ => unreachable!("io formatter received another config error"),
    }
}

fn format_config_shape_error(error: &ConfigError) -> String {
    let ConfigError::InvalidProviderType { provider } = error else {
        unreachable!("shape formatter received another config error");
    };

    format!("Configuration is incomplete: provider `{provider}` has no type.")
}
