fn handle_plan(prompt: &str) -> Result<String, AppError> {
    handle_plan_with_loader(prompt, spectacular_config::read_config)
}
fn handle_plan_with_loader(
    prompt: &str,
    load_config: impl FnOnce() -> Result<SpectacularConfig, ConfigError>,
) -> Result<String, AppError> {
    spectacular_plan::run(prompt, load_config)
        .map(str::to_owned)
        .map_err(AppError::Plan)
}

#[derive(Debug)]
enum AppError {
    Chat(chat::ChatError),
    Config(ConfigError),
    DebugLog { source: std::io::Error },
    InvalidConfigCommand(String),
    Plan(PlanError),
    Provider { source: ProviderError },
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

fn user_facing_error(error: &AppError) -> String {
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
        ProviderError::CancellationError => "Provider call was cancelled.".to_owned(),
        ProviderError::InvalidApiKey => "Invalid API key.".to_owned(),
        ProviderError::ModelFetchFailed { provider_name } => {
            format!("Failed to fetch models from {provider_name}.")
        }
        ProviderError::NoModelsReturned { provider_name } => {
            format!("{provider_name} returned no models.")
        }
        ProviderError::ProviderUnavailable { provider_name } => {
            format!("{provider_name} is unavailable. Try again later.")
        }
        ProviderError::AuthenticationRequired { provider_name } => {
            format!("{provider_name} authentication is required.")
        }
        ProviderError::AuthenticationFailed {
            provider_name,
            reason,
        } => format!("{provider_name} authentication failed: {reason}."),
        ProviderError::StreamUnavailable { provider_name } => {
            format!("{provider_name} streaming is not available yet.")
        }
        ProviderError::MalformedResponse {
            provider_name,
            reason,
        } => format!("{provider_name} returned a malformed response: {reason}."),
        ProviderError::ResponseParsingFailed {
            provider_name,
            reason,
        } => format!("Failed to parse {provider_name} response: {reason}."),
        ProviderError::StreamError {
            provider_name,
            code,
            message,
        } => match code {
            Some(code) => {
                format!("{provider_name} stream returned error `{code}`: {message}.")
            }
            None => format!("{provider_name} stream returned error: {message}."),
        },
        ProviderError::NetworkError {
            provider_name,
            reason,
        } => format!("{provider_name} network request failed: {reason}."),
        ProviderError::ContextLimitExceeded {
            provider_name,
            reason,
        } => format!("{provider_name} context limit exceeded: {reason}."),
        ProviderError::CapabilityMismatch {
            provider_name,
            capability,
        } => format!("{provider_name} does not support required capability `{capability}`."),
        ProviderError::UnsupportedProvider { provider_id } => {
            format!("Provider `{provider_id}` is not available.")
        }
        ProviderError::UnsupportedValidationMode => {
            "The selected provider does not support API key validation.".to_owned()
        }
    }
}

fn format_config_error(error: &ConfigError) -> String {
    let setup_instruction =
        "Run `spectacular config provider add provider:<provider> apikey:<api-key>` to configure a provider.";

    match error {
        ConfigError::MissingConfigFile { .. } => {
            format!("Configuration is missing. {setup_instruction}")
        }
        ConfigError::InvalidJson { path, .. } => format!(
            "Configuration file contains invalid JSON at {}. {setup_instruction}",
            path.display()
        ),
        ConfigError::SchemaChanged => format!("{error}. {setup_instruction}"),
        ConfigError::ProviderNotConfigured { provider } => format!(
            "Configuration is incomplete: provider `{provider}` is not configured. {setup_instruction}"
        ),
        ConfigError::InvalidProviderType { provider } => {
            format!("Configuration is incomplete: provider `{provider}` has no type.")
        }
        ConfigError::MissingProviderApiKey { provider } => format!(
            "Configuration is incomplete: provider `{provider}` has no credentials. {setup_instruction}"
        ),
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
        ConfigError::EmptyValue { .. }
        | ConfigError::ProviderAlreadyExists { .. }
        | ConfigError::ModelAlreadyExists { .. } => error.to_string(),
    }
}
