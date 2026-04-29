use anstyle::{AnsiColor, Style};
use clap::{Args, Parser, Subcommand};
use spectacular_config::{
    ConfigError, ProviderConfig, ReasoningLevel, SpectacularConfig, TaskModelConfig, TaskModelSlot,
};
use spectacular_llms::{ProviderError, ProviderMetadata};
use spectacular_plan::PlanError;
use std::process::ExitCode;

#[derive(Debug, Parser)]
#[command(name = "spectacular")]
#[command(about = "Spec Driven Development workflow assistant")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Inspect or update Spectacular configuration.
    Config(ConfigArgs),
    /// Run the first SDD planning step.
    Plan {
        /// Prompt to plan from.
        prompt: String,
    },
}

#[derive(Debug, Args)]
struct ConfigArgs {
    /// Select an already configured provider for runtime commands.
    #[arg(long = "use", value_name = "PROVIDER")]
    use_provider: Option<String>,
    /// Provider identifier for the update.
    #[arg(long, value_name = "PROVIDER")]
    provider: Option<String>,
    /// API key to store for the provider.
    #[arg(long, value_name = "API_KEY")]
    key: Option<String>,
    /// Required task slot to configure.
    #[arg(long, value_name = "TASK", value_parser = parse_task_model_slot)]
    task: Option<TaskModelSlot>,
    /// Model identifier to store for the task.
    #[arg(long, value_name = "MODEL_ID")]
    model: Option<String>,
    /// Reasoning effort for the task model assignment.
    #[arg(long, value_name = "LEVEL", value_parser = parse_reasoning_level)]
    reasoning: Option<ReasoningLevel>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum ConfigOperation {
    Show,
    UseProvider {
        provider_id: String,
    },
    SaveProviderKey {
        provider_id: String,
        key: String,
    },
    SaveTaskModel {
        provider_id: String,
        slot: TaskModelSlot,
        model: String,
        reasoning: ReasoningLevel,
    },
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match handle(cli) {
        Ok(output) => {
            println!("{output}");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("{}", user_facing_error(&error));
            ExitCode::FAILURE
        }
    }
}

fn handle(cli: Cli) -> Result<String, AppError> {
    match cli.command {
        Command::Config(args) => handle_config(args),
        Command::Plan { prompt } => handle_plan(&prompt),
    }
}

fn handle_config(args: ConfigArgs) -> Result<String, AppError> {
    handle_config_with_io(
        args,
        spectacular_config::read_config_or_default,
        spectacular_config::write_config,
    )
}

fn handle_config_with_io(
    args: ConfigArgs,
    load_config: impl FnOnce() -> Result<SpectacularConfig, ConfigError>,
    write_config: impl FnOnce(&SpectacularConfig) -> Result<(), ConfigError>,
) -> Result<String, AppError> {
    let operation = config_operation(args)?;

    match operation {
        ConfigOperation::Show => {
            let config = load_config()?;
            Ok(format_config_report(&config))
        }
        ConfigOperation::UseProvider { provider_id } => {
            let provider = supported_provider(&provider_id)?;
            let mut config = load_config()?;
            config.select_provider(provider.id())?;
            write_config(&config)?;

            Ok(format_selected_provider_output(provider.display_name()))
        }
        ConfigOperation::SaveProviderKey { provider_id, key } => {
            let provider = supported_provider(&provider_id)?;
            let mut config = load_config()?;
            config.set_provider_api_key(provider.id(), key);
            write_config(&config)?;

            Ok(format_api_key_saved_output(provider.display_name()))
        }
        ConfigOperation::SaveTaskModel {
            provider_id,
            slot,
            model,
            reasoning,
        } => {
            let provider = supported_provider(&provider_id)?;
            let mut config = load_config()?;
            config.set_provider_task_model(provider.id(), slot, model.as_str(), reasoning);
            write_config(&config)?;

            Ok(format_task_model_saved_output(
                provider.display_name(),
                slot,
                &model,
                reasoning,
            ))
        }
    }
}

fn config_operation(args: ConfigArgs) -> Result<ConfigOperation, AppError> {
    if let Some(provider_id) = args.use_provider {
        if args.provider.is_some()
            || args.key.is_some()
            || args.task.is_some()
            || args.model.is_some()
            || args.reasoning.is_some()
        {
            return Err(AppError::InvalidConfigCommand(
                "`--use <provider>` cannot be combined with other config options.".to_owned(),
            ));
        }

        return require_non_empty("provider", provider_id)
            .map(|provider_id| ConfigOperation::UseProvider { provider_id });
    }

    let has_updates = args.provider.is_some()
        || args.key.is_some()
        || args.task.is_some()
        || args.model.is_some()
        || args.reasoning.is_some();

    if !has_updates {
        return Ok(ConfigOperation::Show);
    }

    let Some(provider_id) = args.provider else {
        return Err(AppError::InvalidConfigCommand(
            "`--provider <provider>` is required when setting keys or task models.".to_owned(),
        ));
    };
    let provider_id = require_non_empty("provider", provider_id)?;

    if let Some(key) = args.key {
        if args.task.is_some() || args.model.is_some() || args.reasoning.is_some() {
            return Err(AppError::InvalidConfigCommand(
                "Configure provider keys and task models with separate commands.".to_owned(),
            ));
        }

        return require_non_empty("key", key)
            .map(|key| ConfigOperation::SaveProviderKey { provider_id, key });
    }

    if args.task.is_none() && args.model.is_none() && args.reasoning.is_none() {
        return Err(AppError::InvalidConfigCommand(
            "`--provider` must be paired with `--key` or `--task` and `--model`.".to_owned(),
        ));
    }

    let Some(slot) = args.task else {
        return Err(AppError::InvalidConfigCommand(
            "`--task <task>` is required when setting a task model.".to_owned(),
        ));
    };
    let Some(model) = args.model else {
        return Err(AppError::InvalidConfigCommand(
            "`--model <model-id>` is required when setting a task model.".to_owned(),
        ));
    };

    Ok(ConfigOperation::SaveTaskModel {
        provider_id,
        slot,
        model: require_non_empty("model", model)?,
        reasoning: args.reasoning.unwrap_or_default(),
    })
}

fn supported_provider(provider_id: &str) -> Result<ProviderMetadata, AppError> {
    let provider =
        spectacular_llms::provider_by_id(provider_id).ok_or_else(|| AppError::Provider {
            source: ProviderError::UnsupportedProvider {
                provider_id: provider_id.to_owned(),
            },
        })?;

    if provider.is_enabled() {
        return Ok(provider);
    }

    Err(AppError::Provider {
        source: ProviderError::UnsupportedProvider {
            provider_id: provider.id().to_owned(),
        },
    })
}

fn require_non_empty(name: &'static str, value: String) -> Result<String, AppError> {
    if !value.trim().is_empty() {
        return Ok(value);
    }

    Err(AppError::InvalidConfigCommand(format!(
        "`--{name}` cannot be empty."
    )))
}

fn parse_task_model_slot(value: &str) -> Result<TaskModelSlot, String> {
    value
        .parse::<TaskModelSlot>()
        .map_err(|error| error.to_string())
}

fn parse_reasoning_level(value: &str) -> Result<ReasoningLevel, String> {
    value
        .parse::<ReasoningLevel>()
        .map_err(|error| error.to_string())
}

fn format_config_report(config: &SpectacularConfig) -> String {
    let mut lines = Vec::new();
    let selected_provider = config.providers.selected.as_deref().unwrap_or("none");

    lines.push(paint(title_style(), "Spectacular config"));
    lines.push(format!(
        "{} {}",
        paint(label_style(), "Selected provider:"),
        format_optional_value(selected_provider)
    ));
    lines.push(String::new());
    lines.push(paint(section_style(), "Configured providers"));

    if config.providers.available.is_empty() {
        lines.push(format!("  {}", paint(missing_style(), "None")));
        return lines.join("\n");
    }

    for (provider_id, provider_config) in &config.providers.available {
        let provider_line = if Some(provider_id.as_str()) == config.providers.selected.as_deref() {
            format!(
                "  {} {}",
                paint(provider_style(), provider_id),
                paint(success_style(), "(selected)")
            )
        } else {
            format!("  {}", paint(provider_style(), provider_id))
        };

        lines.push(provider_line);
        append_provider_config(&mut lines, provider_config);
    }

    lines.join("\n")
}

fn append_provider_config(lines: &mut Vec<String>, provider_config: &ProviderConfig) {
    lines.push(format!(
        "    {} {}",
        paint(label_style(), "API key:"),
        match provider_config.key.as_deref() {
            Some(key) if !key.trim().is_empty() => paint(secret_style(), key),
            _ => paint(missing_style(), "not configured"),
        }
    ));
    lines.push(format!("    {}", paint(section_style(), "Tasks")));
    append_task_config(
        lines,
        TaskModelSlot::Planning,
        provider_config.tasks.planning.as_ref(),
    );
    append_task_config(
        lines,
        TaskModelSlot::Labeling,
        provider_config.tasks.labeling.as_ref(),
    );
    append_task_config(
        lines,
        TaskModelSlot::Coding,
        provider_config.tasks.coding.as_ref(),
    );
}

fn append_task_config(
    lines: &mut Vec<String>,
    slot: TaskModelSlot,
    task_config: Option<&TaskModelConfig>,
) {
    let Some(task_config) = task_config else {
        lines.push(format!(
            "      {} {}",
            paint(task_style(), format!("{slot}:")),
            paint(missing_style(), "not configured")
        ));
        return;
    };

    if task_config.model.trim().is_empty() {
        lines.push(format!(
            "      {} {}",
            paint(task_style(), format!("{slot}:")),
            paint(missing_style(), "not configured")
        ));
        return;
    }

    lines.push(format!(
        "      {} {} {} {}",
        paint(task_style(), format!("{slot}:")),
        paint(model_style(), task_config.model.as_str()),
        paint(label_style(), "(reasoning:"),
        paint_reasoning(task_config.reasoning, ")")
    ));
}

fn format_selected_provider_output(provider_name: &str) -> String {
    format!(
        "{} {}\n  {} {}",
        paint(success_style(), "[selected]"),
        paint(title_style(), "Active provider updated"),
        paint(label_style(), "Provider:"),
        paint(provider_style(), provider_name)
    )
}

fn format_api_key_saved_output(provider_name: &str) -> String {
    format!(
        "{} {}\n  {} {}\n  {} {}",
        paint(success_style(), "[saved]"),
        paint(title_style(), "API key configured"),
        paint(label_style(), "Provider:"),
        paint(provider_style(), provider_name),
        paint(label_style(), "State:"),
        paint(success_style(), "configured")
    )
}

fn format_task_model_saved_output(
    provider_name: &str,
    slot: TaskModelSlot,
    model: &str,
    reasoning: ReasoningLevel,
) -> String {
    format!(
        "{} {}\n  {} {}\n  {} {}\n  {} {}\n  {} {}",
        paint(success_style(), "[saved]"),
        paint(title_style(), "Task model configured"),
        paint(label_style(), "Provider:"),
        paint(provider_style(), provider_name),
        paint(label_style(), "Task:"),
        paint(task_style(), slot.as_str()),
        paint(label_style(), "Model:"),
        paint(model_style(), model),
        paint(label_style(), "Reasoning:"),
        paint_reasoning(reasoning, "")
    )
}

fn format_optional_value(value: &str) -> String {
    if value == "none" || value.trim().is_empty() {
        return paint(missing_style(), value);
    }

    paint(provider_style(), value)
}

fn paint(style: Style, value: impl AsRef<str>) -> String {
    let value = value.as_ref();
    format!("{style}{value}{style:#}")
}

fn paint_reasoning(reasoning: ReasoningLevel, suffix: &str) -> String {
    paint(reasoning_style(reasoning), format!("{reasoning}{suffix}"))
}

fn title_style() -> Style {
    AnsiColor::BrightCyan.on_default().bold()
}

fn section_style() -> Style {
    AnsiColor::BrightWhite.on_default().bold()
}

fn label_style() -> Style {
    AnsiColor::BrightBlack.on_default()
}

fn success_style() -> Style {
    AnsiColor::BrightGreen.on_default().bold()
}

fn provider_style() -> Style {
    AnsiColor::Cyan.on_default().bold()
}

fn task_style() -> Style {
    AnsiColor::Magenta.on_default().bold()
}

fn model_style() -> Style {
    AnsiColor::BrightWhite.on_default()
}

fn secret_style() -> Style {
    AnsiColor::Yellow.on_default()
}

fn missing_style() -> Style {
    AnsiColor::BrightYellow.on_default().bold()
}

fn reasoning_style(reasoning: ReasoningLevel) -> Style {
    match reasoning {
        ReasoningLevel::None => AnsiColor::BrightBlack.on_default(),
        ReasoningLevel::Low => AnsiColor::Blue.on_default(),
        ReasoningLevel::Medium => AnsiColor::Yellow.on_default(),
        ReasoningLevel::High => AnsiColor::BrightRed.on_default().bold(),
    }
}

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
    Config(ConfigError),
    InvalidConfigCommand(String),
    Plan(PlanError),
    Provider { source: ProviderError },
}

impl From<ConfigError> for AppError {
    fn from(error: ConfigError) -> Self {
        Self::Config(error)
    }
}

fn user_facing_error(error: &AppError) -> String {
    match error {
        AppError::Plan(PlanError::EmptyPrompt) => {
            "A non-empty prompt is required. Usage: spectacular plan <prompt>".to_owned()
        }
        AppError::Plan(PlanError::Config(config_error)) | AppError::Config(config_error) => {
            format_config_error(config_error)
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
        "Run `spectacular config --provider <provider> --key <api-key>` to configure a provider.";

    match error {
        ConfigError::MissingConfigFile { .. } => {
            format!("Configuration is missing. {setup_instruction}")
        }
        ConfigError::InvalidJson { path, .. } => format!(
            "Configuration file contains invalid JSON at {}. {setup_instruction}",
            path.display()
        ),
        ConfigError::NoSelectedProvider => {
            "Configuration is incomplete: no provider is selected. Run `spectacular config --use <provider>`.".to_owned()
        }
        ConfigError::ProviderNotConfigured { provider } => format!(
            "Configuration is incomplete: provider `{provider}` is not configured. {setup_instruction}"
        ),
        ConfigError::MissingProviderApiKey { provider } => format!(
            "Configuration is incomplete: provider `{provider}` has no API key. {setup_instruction}"
        ),
        ConfigError::MissingTaskModel { slot } => format!(
            "Configuration is incomplete: missing `{slot}` model assignment. Run `spectacular config --provider <provider> --task {slot} --model <model-id>`."
        ),
        ConfigError::InvalidTaskModel { slot } => format!(
            "Configuration is incomplete: `{slot}` model assignment is blank. Run `spectacular config --provider <provider> --task {slot} --model <model-id>`."
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
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;
    use spectacular_config::{ProviderConfig, ProvidersConfig, TaskModelConfig, TaskModels};
    use std::cell::RefCell;
    use std::collections::BTreeMap;

    #[test]
    fn top_level_help_lists_config_and_plan() {
        let mut command = Cli::command();
        let mut buffer = Vec::new();

        command.write_long_help(&mut buffer).unwrap();
        let help = String::from_utf8(buffer).unwrap();

        assert!(help.contains("config"));
        assert!(help.contains("plan"));
    }

    #[test]
    fn config_help_lists_non_interactive_flags() {
        let mut command = Cli::command();
        let config = command
            .find_subcommand_mut("config")
            .expect("config subcommand should exist");
        let mut buffer = Vec::new();

        config.write_long_help(&mut buffer).unwrap();
        let help = String::from_utf8(buffer).unwrap();

        assert!(help.contains("--use"));
        assert!(help.contains("--provider"));
        assert!(help.contains("--key"));
        assert!(help.contains("--task"));
        assert!(help.contains("--model"));
        assert!(help.contains("--reasoning"));
        assert!(!help.contains("--apikey"));
        assert!(!help.contains("--models"));
    }

    #[test]
    fn config_defaults_to_show_operation() {
        let args = ConfigArgs {
            use_provider: None,
            provider: None,
            key: None,
            task: None,
            model: None,
            reasoning: None,
        };

        assert_eq!(config_operation(args).unwrap(), ConfigOperation::Show);
    }

    #[test]
    fn config_uses_provider_only_through_use_flag() {
        let args = ConfigArgs {
            use_provider: Some("openrouter".to_owned()),
            provider: None,
            key: None,
            task: None,
            model: None,
            reasoning: None,
        };

        assert_eq!(
            config_operation(args).unwrap(),
            ConfigOperation::UseProvider {
                provider_id: "openrouter".to_owned()
            }
        );
    }

    #[test]
    fn config_rejects_use_with_other_flags() {
        let args = ConfigArgs {
            use_provider: Some("openrouter".to_owned()),
            provider: Some("openrouter".to_owned()),
            key: None,
            task: None,
            model: None,
            reasoning: None,
        };

        assert!(matches!(
            config_operation(args).unwrap_err(),
            AppError::InvalidConfigCommand(_)
        ));
    }

    #[test]
    fn config_saves_provider_key_operation() {
        let args = ConfigArgs {
            use_provider: None,
            provider: Some("openrouter".to_owned()),
            key: Some("sk-or-v1-test".to_owned()),
            task: None,
            model: None,
            reasoning: None,
        };

        assert_eq!(
            config_operation(args).unwrap(),
            ConfigOperation::SaveProviderKey {
                provider_id: "openrouter".to_owned(),
                key: "sk-or-v1-test".to_owned()
            }
        );
    }

    #[test]
    fn config_saves_task_model_with_default_reasoning() {
        let args = ConfigArgs {
            use_provider: None,
            provider: Some("openrouter".to_owned()),
            key: None,
            task: Some(TaskModelSlot::Planning),
            model: Some("deepseek/deepseek-v4-pro".to_owned()),
            reasoning: None,
        };

        assert_eq!(
            config_operation(args).unwrap(),
            ConfigOperation::SaveTaskModel {
                provider_id: "openrouter".to_owned(),
                slot: TaskModelSlot::Planning,
                model: "deepseek/deepseek-v4-pro".to_owned(),
                reasoning: ReasoningLevel::None,
            }
        );
    }

    #[test]
    fn config_saves_task_model_with_explicit_reasoning() {
        let args = ConfigArgs {
            use_provider: None,
            provider: Some("openrouter".to_owned()),
            key: None,
            task: Some(TaskModelSlot::Planning),
            model: Some("deepseek/deepseek-v4-pro".to_owned()),
            reasoning: Some(ReasoningLevel::High),
        };

        assert_eq!(
            config_operation(args).unwrap(),
            ConfigOperation::SaveTaskModel {
                provider_id: "openrouter".to_owned(),
                slot: TaskModelSlot::Planning,
                model: "deepseek/deepseek-v4-pro".to_owned(),
                reasoning: ReasoningLevel::High,
            }
        );
    }

    #[test]
    fn config_requires_provider_for_key_updates() {
        let args = ConfigArgs {
            use_provider: None,
            provider: None,
            key: Some("sk-or-v1-test".to_owned()),
            task: None,
            model: None,
            reasoning: None,
        };

        assert!(matches!(
            config_operation(args).unwrap_err(),
            AppError::InvalidConfigCommand(_)
        ));
    }

    #[test]
    fn config_requires_model_for_task_updates() {
        let args = ConfigArgs {
            use_provider: None,
            provider: Some("openrouter".to_owned()),
            key: None,
            task: Some(TaskModelSlot::Planning),
            model: None,
            reasoning: Some(ReasoningLevel::High),
        };

        assert!(matches!(
            config_operation(args).unwrap_err(),
            AppError::InvalidConfigCommand(_)
        ));
    }

    #[test]
    fn config_rejects_key_and_task_in_same_command() {
        let args = ConfigArgs {
            use_provider: None,
            provider: Some("openrouter".to_owned()),
            key: Some("sk-or-v1-test".to_owned()),
            task: Some(TaskModelSlot::Planning),
            model: Some("deepseek/deepseek-v4-pro".to_owned()),
            reasoning: None,
        };

        assert!(matches!(
            config_operation(args).unwrap_err(),
            AppError::InvalidConfigCommand(_)
        ));
    }

    #[test]
    fn config_show_prints_human_readable_config_with_raw_key() {
        let output = handle_config_with_io(
            ConfigArgs {
                use_provider: None,
                provider: None,
                key: None,
                task: None,
                model: None,
                reasoning: None,
            },
            || Ok(complete_config()),
            |_| panic!("show must not write config"),
        )
        .unwrap();
        let output = strip_ansi_codes(&output);

        assert!(output.contains("Spectacular config"));
        assert!(output.contains("Selected provider: openrouter"));
        assert!(output.contains("  openrouter (selected)"));
        assert!(output.contains("    API key: sk-or-v1-test"));
        assert!(output.contains("      planning: openrouter/planning (reasoning: high)"));
        assert!(output.contains("      labeling: openrouter/labeling (reasoning: low)"));
        assert!(output.contains("      coding: openrouter/coding (reasoning: medium)"));
        assert!(!output.contains("\"providers\""));
    }

    #[test]
    fn config_show_prints_empty_config_clearly() {
        let output = strip_ansi_codes(&format_config_report(&SpectacularConfig::default()));

        assert_eq!(
            output,
            "Spectacular config\nSelected provider: none\n\nConfigured providers\n  None"
        );
    }

    #[test]
    fn use_provider_requires_provider_to_be_configured() {
        let output = handle_config_with_io(
            ConfigArgs {
                use_provider: Some("openrouter".to_owned()),
                provider: None,
                key: None,
                task: None,
                model: None,
                reasoning: None,
            },
            || Ok(SpectacularConfig::default()),
            |_| panic!("unconfigured provider must not be written"),
        )
        .unwrap_err();

        assert!(matches!(
            output,
            AppError::Config(ConfigError::ProviderNotConfigured { .. })
        ));
    }

    #[test]
    fn use_provider_saves_selected_provider_without_requiring_complete_tasks() {
        let saved_config = RefCell::new(None);
        let mut config = SpectacularConfig::default();
        config.set_provider_api_key("openrouter", "sk-or-v1-test");

        let output = handle_config_with_io(
            ConfigArgs {
                use_provider: Some("openrouter".to_owned()),
                provider: None,
                key: None,
                task: None,
                model: None,
                reasoning: None,
            },
            || Ok(config),
            |config| {
                saved_config.replace(Some(config.clone()));
                Ok(())
            },
        )
        .unwrap();
        let output = strip_ansi_codes(&output);

        assert_eq!(
            output,
            "[selected] Active provider updated\n  Provider: OpenRouter"
        );
        assert_eq!(
            saved_config
                .into_inner()
                .unwrap()
                .providers
                .selected
                .as_deref(),
            Some("openrouter")
        );
    }

    #[test]
    fn provider_key_update_validates_provider_locally_without_key_validation() {
        let saved_config = RefCell::new(None);

        let output = handle_config_with_io(
            ConfigArgs {
                use_provider: None,
                provider: Some("openrouter".to_owned()),
                key: Some("not-validated".to_owned()),
                task: None,
                model: None,
                reasoning: None,
            },
            || Ok(SpectacularConfig::default()),
            |config| {
                saved_config.replace(Some(config.clone()));
                Ok(())
            },
        )
        .unwrap();
        let output = strip_ansi_codes(&output);

        let saved_config = saved_config.into_inner().unwrap();
        assert_eq!(
            output,
            "[saved] API key configured\n  Provider: OpenRouter\n  State: configured"
        );
        assert_eq!(
            saved_config
                .providers
                .available
                .get("openrouter")
                .and_then(|provider| provider.key.as_deref()),
            Some("not-validated")
        );
    }

    #[test]
    fn provider_key_update_rejects_unknown_provider() {
        let output = handle_config_with_io(
            ConfigArgs {
                use_provider: None,
                provider: Some("unknown".to_owned()),
                key: Some("key".to_owned()),
                task: None,
                model: None,
                reasoning: None,
            },
            || Ok(SpectacularConfig::default()),
            |_| panic!("unknown provider must not be written"),
        )
        .unwrap_err();

        assert!(matches!(output, AppError::Provider { .. }));
    }

    #[test]
    fn task_model_update_saves_under_provider() {
        let saved_config = RefCell::new(None);

        let output = handle_config_with_io(
            ConfigArgs {
                use_provider: None,
                provider: Some("openrouter".to_owned()),
                key: None,
                task: Some(TaskModelSlot::Planning),
                model: Some("deepseek/deepseek-v4-pro".to_owned()),
                reasoning: Some(ReasoningLevel::High),
            },
            || Ok(SpectacularConfig::default()),
            |config| {
                saved_config.replace(Some(config.clone()));
                Ok(())
            },
        )
        .unwrap();
        let output = strip_ansi_codes(&output);

        let saved_config = saved_config.into_inner().unwrap();
        assert_eq!(
            output,
            "[saved] Task model configured\n  Provider: OpenRouter\n  Task: planning\n  Model: deepseek/deepseek-v4-pro\n  Reasoning: high"
        );
        assert_eq!(
            saved_config
                .providers
                .available
                .get("openrouter")
                .unwrap()
                .tasks
                .planning,
            Some(TaskModelConfig::new(
                "deepseek/deepseek-v4-pro",
                ReasoningLevel::High
            ))
        );
        assert!(saved_config.providers.selected.is_none());
    }

    #[test]
    fn plan_reports_missing_config_before_placeholder_output() {
        let output = handle_plan_with_loader("Create a login flow", || {
            Err(ConfigError::MissingConfigFile {
                path: "config.json".into(),
            })
        })
        .unwrap_err();

        assert!(matches!(
            output,
            AppError::Plan(PlanError::Config(ConfigError::MissingConfigFile { .. }))
        ));
    }

    #[test]
    fn plan_with_complete_config_prints_placeholder_output() {
        let output =
            handle_plan_with_loader("Create a login flow", || Ok(complete_config())).unwrap();

        assert_eq!(output, "Hello World");
    }

    #[test]
    fn empty_plan_prompt_has_clear_error_message() {
        let error = AppError::Plan(PlanError::EmptyPrompt);

        assert_eq!(
            user_facing_error(&error),
            "A non-empty prompt is required. Usage: spectacular plan <prompt>"
        );
    }

    #[test]
    fn incomplete_config_errors_tell_user_to_run_new_config_commands() {
        let error = AppError::Plan(PlanError::Config(ConfigError::NoSelectedProvider));

        assert!(user_facing_error(&error).contains("spectacular config --use"));
    }

    fn complete_config() -> SpectacularConfig {
        let mut available = BTreeMap::new();
        available.insert(
            "openrouter".to_owned(),
            ProviderConfig {
                key: Some("sk-or-v1-test".to_owned()),
                tasks: TaskModels {
                    planning: Some(TaskModelConfig::new(
                        "openrouter/planning",
                        ReasoningLevel::High,
                    )),
                    labeling: Some(TaskModelConfig::new(
                        "openrouter/labeling",
                        ReasoningLevel::Low,
                    )),
                    coding: Some(TaskModelConfig::new(
                        "openrouter/coding",
                        ReasoningLevel::Medium,
                    )),
                },
            },
        );

        SpectacularConfig {
            providers: ProvidersConfig {
                selected: Some("openrouter".to_owned()),
                available,
            },
        }
    }

    fn strip_ansi_codes(value: &str) -> String {
        let mut output = String::new();
        let mut chars = value.chars().peekable();

        while let Some(character) = chars.next() {
            if character != '\u{1b}' {
                output.push(character);
                continue;
            }

            if chars.peek() == Some(&'[') {
                chars.next();
                for code_character in chars.by_ref() {
                    if code_character.is_ascii_alphabetic() {
                        break;
                    }
                }
            }
        }

        output
    }
}
