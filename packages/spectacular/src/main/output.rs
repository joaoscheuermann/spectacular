use super::{chat, entry::AppError, terminal_style};
use anstyle::Style;
use spectacular_config::{
    mask_api_key, ConfigError, ReasoningLevel, SpectacularConfig, TaskModelSlot,
};
use spectacular_llms::ProviderError;

pub(super) fn user_facing_error(error: &AppError) -> String {
    match error {
        AppError::Chat(chat::ChatError::Exit) => String::new(),
        AppError::Chat(error) => error.to_string(),
        AppError::Config(config_error) => format_config_error(config_error),
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

pub(super) fn format_config_report(config: &SpectacularConfig) -> String {
    let mut lines = Vec::new();

    append_report_title(&mut lines);
    append_provider_section(&mut lines, config);
    append_model_section(&mut lines, config);
    append_task_section(&mut lines, config);

    lines.join("\n")
}

fn append_report_title(lines: &mut Vec<String>) {
    lines.push(paint(title_style(), "Spectacular config"));
    lines.push(String::new());
}

fn append_provider_section(lines: &mut Vec<String>, config: &SpectacularConfig) {
    lines.push(paint(section_style(), "Providers"));

    if config.providers.is_empty() {
        lines.push(format!("  {}", paint(missing_style(), "None")));
    } else {
        for (name, provider) in &config.providers {
            append_provider_report(lines, name, provider);
        }
    }
}

fn append_provider_report(
    lines: &mut Vec<String>,
    name: &str,
    provider: &spectacular_config::ProviderConfig,
) {
    lines.push(format!(
        "  {} {} {} {}",
        paint(provider_style(), name),
        paint(label_style(), "type:"),
        paint(provider_style(), &provider.provider_type),
        paint(secret_style(), provider_credential(provider))
    ));
}

fn provider_credential(provider: &spectacular_config::ProviderConfig) -> String {
    match provider.auth_mode() {
        Some(spectacular_config::ProviderAuthMode::Oauth) => "authenticated".to_owned(),
        _ => mask_api_key(provider.api_key()),
    }
}

fn append_model_section(lines: &mut Vec<String>, config: &SpectacularConfig) {
    lines.push(String::new());
    lines.push(paint(section_style(), "Models"));

    if config.models.is_empty() {
        lines.push(format!("  {}", paint(missing_style(), "None")));
    } else {
        for (name, model) in &config.models {
            append_model_report(lines, config, name, model);
        }
    }
}

fn append_model_report(
    lines: &mut Vec<String>,
    config: &SpectacularConfig,
    name: &str,
    model: &spectacular_config::ModelConfig,
) {
    lines.push(format!(
        "  {} {} {} {} {} {}{}",
        paint(model_style(), name),
        paint(label_style(), "provider:"),
        paint(provider_style(), &model.provider),
        paint(label_style(), "id:"),
        paint(model_style(), &model.model),
        paint_reasoning(model.reasoning, ""),
        provider_state(config, &model.provider)
    ));
}

fn provider_state(config: &SpectacularConfig, provider: &str) -> String {
    if config.providers.contains_key(provider) {
        String::new()
    } else {
        format!(" {}", paint(missing_style(), "(provider missing)"))
    }
}

fn append_task_section(lines: &mut Vec<String>, config: &SpectacularConfig) {
    lines.push(String::new());
    lines.push(paint(section_style(), "Tasks"));

    for slot in TaskModelSlot::ALL {
        append_task_report(lines, config, slot);
    }
}

fn append_task_report(lines: &mut Vec<String>, config: &SpectacularConfig, slot: TaskModelSlot) {
    let Some(model_key) = config
        .tasks
        .get(slot)
        .filter(|value| !value.trim().is_empty())
    else {
        lines.push(format!(
            "  {} {}",
            paint(task_style(), format!("{slot}:")),
            paint(missing_style(), "not configured")
        ));
        return;
    };
    let state = if config.models.contains_key(model_key) {
        String::new()
    } else {
        format!(" {}", paint(missing_style(), "(model not found)"))
    };

    lines.push(format!(
        "  {} {}{}",
        paint(task_style(), format!("{slot}:")),
        paint(model_style(), model_key),
        state
    ));
}

pub(super) fn format_provider_added_output(name: &str, provider_type: &str) -> String {
    format!(
        "{} {}\n  {} {}\n  {} {}",
        paint(success_style(), "[saved]"),
        paint(title_style(), "Provider added"),
        paint(label_style(), "Name:"),
        paint(provider_style(), name),
        paint(label_style(), "Type:"),
        paint(provider_style(), provider_type)
    )
}

pub(super) fn format_provider_removed_output(name: &str) -> String {
    format!(
        "{} {}\n  {} {}",
        paint(success_style(), "[removed]"),
        paint(title_style(), "Provider removed"),
        paint(label_style(), "Name:"),
        paint(provider_style(), name)
    )
}

pub(super) fn format_model_saved_output(action: &str, key: &str) -> String {
    format!(
        "{} {}\n  {} {}",
        paint(success_style(), "[saved]"),
        paint(title_style(), format!("Model {action}")),
        paint(label_style(), "Name:"),
        paint(model_style(), key)
    )
}

pub(super) fn format_model_remove_confirmation_output(
    name: &str,
    references: &[TaskModelSlot],
) -> String {
    if references.is_empty() {
        return format_confirmation_required_output(
            "Model removal requires confirm:true. No tasks currently reference this model.",
        );
    }

    format_confirmation_required_output(&format!(
        "Model `{name}` is used by tasks: {}. Re-run with confirm:true to delete it and leave those task references invalid.",
        references
            .iter()
            .map(|slot| slot.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    ))
}

pub(super) fn format_model_removed_output(name: &str, references: &[TaskModelSlot]) -> String {
    let warning = if references.is_empty() {
        String::new()
    } else {
        format!(
            "\n  {} {}",
            paint(label_style(), "Invalid tasks:"),
            paint(
                missing_style(),
                references
                    .iter()
                    .map(|slot| slot.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        )
    };

    format!(
        "{} {}\n  {} {}{}",
        paint(success_style(), "[removed]"),
        paint(title_style(), "Model removed"),
        paint(label_style(), "Name:"),
        paint(model_style(), name),
        warning
    )
}

pub(super) fn format_task_saved_output(slot: TaskModelSlot, model: &str) -> String {
    format!(
        "{} {}\n  {} {}\n  {} {}",
        paint(success_style(), "[saved]"),
        paint(title_style(), "Task model assigned"),
        paint(label_style(), "Task:"),
        paint(task_style(), slot.as_str()),
        paint(label_style(), "Model:"),
        paint(model_style(), model)
    )
}

pub(super) fn format_confirmation_required_output(message: &str) -> String {
    format!(
        "{} {}",
        paint(missing_style(), "[confirmation required]"),
        message
    )
}

fn paint(style: Style, value: impl AsRef<str>) -> String {
    terminal_style::paint(style, value)
}

fn paint_reasoning(reasoning: ReasoningLevel, suffix: &str) -> String {
    paint(reasoning_style(reasoning), format!("{reasoning}{suffix}"))
}

fn title_style() -> Style {
    terminal_style::title_style()
}

fn section_style() -> Style {
    terminal_style::text_style().bold()
}

fn label_style() -> Style {
    terminal_style::dim_style()
}

fn success_style() -> Style {
    terminal_style::success_style()
}

fn provider_style() -> Style {
    terminal_style::provider_style()
}

fn task_style() -> Style {
    terminal_style::task_style()
}

fn model_style() -> Style {
    terminal_style::model_style()
}

fn secret_style() -> Style {
    terminal_style::secret_style()
}

fn missing_style() -> Style {
    terminal_style::warning_style()
}

fn reasoning_style(reasoning: ReasoningLevel) -> Style {
    match reasoning {
        ReasoningLevel::None => terminal_style::dim_style(),
        ReasoningLevel::Minimal | ReasoningLevel::Low => terminal_style::low_reasoning_style(),
        ReasoningLevel::Medium => terminal_style::warning_style(),
        ReasoningLevel::High | ReasoningLevel::Xhigh => terminal_style::error_style(),
    }
}
