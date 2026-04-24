use clap::{Args, Parser, Subcommand};
use spectacular_config::{ConfigError, SpectacularConfig, TaskModels};
use spectacular_core::{
    ApiKeyInputError, ModelAssignmentError, ModelOption, ProviderOption, ProviderSelectionError,
    TaskModelSelection,
};
use spectacular_llms::{Model, ProviderError, ProviderMetadata, ValidationMode};
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
    /// Configure Spectacular.
    Config(ConfigArgs),
    /// Run the first SDD planning step.
    Plan {
        /// Prompt to plan from.
        prompt: String,
    },
}

#[derive(Debug, Args)]
struct ConfigArgs {
    /// Configure the active provider.
    #[arg(long)]
    provider: bool,
    /// Configure the API key for the active provider.
    #[arg(long)]
    apikey: bool,
    /// Configure task model assignments.
    #[arg(long)]
    models: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ConfigRoute {
    FullSetup,
    Provider,
    ApiKey,
    Models,
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
    if config_route(&args) == ConfigRoute::Provider {
        return handle_provider_config();
    }

    if config_route(&args) == ConfigRoute::ApiKey {
        return handle_api_key_config();
    }

    if config_route(&args) == ConfigRoute::Models {
        return handle_models_config();
    }

    handle_full_config()
}

fn handle_api_key_config() -> Result<String, AppError> {
    handle_api_key_config_with_input(
        spectacular_config::read_config_or_default,
        spectacular_config::write_config,
        |provider, current_api_key| {
            spectacular_core::run_api_key_screen(
                provider.display_name(),
                current_api_key,
                |api_key| {
                    spectacular_llms::validate_provider_value(
                        provider.id(),
                        ValidationMode::ApiKey,
                        api_key,
                    )
                    .map_err(|error| error.to_string())
                },
            )
        },
    )
}

fn handle_api_key_config_with_input(
    load_config: impl FnOnce() -> Result<SpectacularConfig, ConfigError>,
    write_config: impl FnOnce(&SpectacularConfig) -> Result<(), ConfigError>,
    read_validated_api_key: impl FnOnce(
        &ProviderMetadata,
        Option<&str>,
    ) -> Result<String, ApiKeyInputError>,
) -> Result<String, AppError> {
    let mut config = load_config()?;
    let selected_provider_id = config
        .selected_provider
        .as_deref()
        .filter(|provider_id| !provider_id.trim().is_empty())
        .ok_or(ConfigError::NoSelectedProvider)?;

    let selected_provider =
        spectacular_llms::provider_by_id(selected_provider_id).ok_or_else(|| {
            ProviderError::UnsupportedProvider {
                provider_id: selected_provider_id.to_owned(),
            }
        })?;

    if !selected_provider.is_enabled() {
        return Err(AppError::Provider(ProviderError::UnsupportedProvider {
            provider_id: selected_provider.id().to_owned(),
        }));
    }

    let current_api_key = config
        .provider_api_keys
        .get(selected_provider.id())
        .map(String::as_str);
    let api_key = read_validated_api_key(&selected_provider, current_api_key)?;
    config.set_provider_api_key(selected_provider.id(), api_key);
    write_config(&config)?;

    Ok(format!(
        "Saved API key for {}.",
        selected_provider.display_name()
    ))
}

fn handle_provider_config() -> Result<String, AppError> {
    handle_provider_config_with_selection(
        spectacular_config::read_config_or_default,
        spectacular_config::write_config,
        spectacular_core::run_provider_selection_screen,
    )
}

fn handle_models_config() -> Result<String, AppError> {
    handle_models_config_with_selection(
        spectacular_config::read_config_or_default,
        spectacular_config::write_config,
        |provider, api_key| {
            spectacular_llms::validate_provider_value(
                provider.id(),
                ValidationMode::ApiKey,
                api_key,
            )?;
            spectacular_llms::fetch_provider_models(provider.id(), api_key)
        },
        |provider, models, current_selection| {
            spectacular_core::run_model_assignment_screen(
                provider.display_name(),
                models,
                current_selection,
            )
        },
    )
}

fn handle_models_config_with_selection(
    load_config: impl FnOnce() -> Result<SpectacularConfig, ConfigError>,
    write_config: impl FnOnce(&SpectacularConfig) -> Result<(), ConfigError>,
    fetch_models: impl FnOnce(&ProviderMetadata, &str) -> Result<Vec<Model>, ProviderError>,
    assign_models: impl FnOnce(
        &ProviderMetadata,
        &[ModelOption],
        Option<TaskModelSelection>,
    ) -> Result<TaskModelSelection, ModelAssignmentError>,
) -> Result<String, AppError> {
    let mut config = load_config()?;
    let (selected_provider, api_key) = selected_provider_and_api_key(&config)?;
    let models = fetch_models(&selected_provider, &api_key)?;
    let model_options = model_options_from_models(&models);
    let selection = assign_models(
        &selected_provider,
        &model_options,
        current_task_model_selection(&config.task_models),
    )?;

    config.set_task_models(
        selection.planning(),
        selection.labeling(),
        selection.coding(),
    );
    write_config(&config)?;

    Ok(format!(
        "Saved model assignments for {}.",
        selected_provider.display_name()
    ))
}

fn handle_full_config() -> Result<String, AppError> {
    handle_full_config_with_steps(
        handle_provider_config,
        handle_api_key_config,
        handle_models_config,
    )
}

fn handle_full_config_with_steps(
    configure_provider: impl FnOnce() -> Result<String, AppError>,
    configure_api_key: impl FnOnce() -> Result<String, AppError>,
    configure_models: impl FnOnce() -> Result<String, AppError>,
) -> Result<String, AppError> {
    let provider_output = configure_provider()?;
    let api_key_output = configure_api_key()?;
    let models_output = configure_models()?;

    Ok(format!(
        "{provider_output}\n{api_key_output}\n{models_output}"
    ))
}

fn handle_provider_config_with_selection(
    load_config: impl FnOnce() -> Result<SpectacularConfig, ConfigError>,
    write_config: impl FnOnce(&SpectacularConfig) -> Result<(), ConfigError>,
    select_provider: impl FnOnce(&[ProviderOption]) -> Result<String, ProviderSelectionError>,
) -> Result<String, AppError> {
    let mut config = load_config()?;
    let providers = spectacular_llms::provider_registry();
    let options = provider_options_from_config(providers, &config);
    let selected_provider_id = select_provider(&options)?;
    let selected_provider =
        spectacular_llms::provider_by_id(&selected_provider_id).ok_or_else(|| {
            ProviderSelectionError::UnknownProvider {
                provider_id: selected_provider_id.clone(),
            }
        })?;

    if !selected_provider.is_enabled() {
        return Err(AppError::ProviderSelection(
            ProviderSelectionError::DisabledProvider {
                provider_name: selected_provider.display_name().to_owned(),
            },
        ));
    }

    config.select_provider(selected_provider.id());
    write_config(&config)?;

    Ok(format!(
        "Selected provider: {}",
        selected_provider.display_name()
    ))
}

fn provider_options_from_config(
    providers: &[ProviderMetadata],
    config: &SpectacularConfig,
) -> Vec<ProviderOption> {
    providers
        .iter()
        .copied()
        .map(|provider| {
            ProviderOption::new(
                provider.id(),
                provider.display_name(),
                provider.is_enabled(),
                config
                    .provider_api_keys
                    .get(provider.id())
                    .map(String::as_str),
                config.selected_provider.as_deref() == Some(provider.id()),
            )
        })
        .collect()
}

fn selected_provider_and_api_key(
    config: &SpectacularConfig,
) -> Result<(ProviderMetadata, String), AppError> {
    let selected_provider_id = config
        .selected_provider
        .as_deref()
        .filter(|provider_id| !provider_id.trim().is_empty())
        .ok_or(ConfigError::NoSelectedProvider)?;
    let selected_provider =
        spectacular_llms::provider_by_id(selected_provider_id).ok_or_else(|| {
            ProviderError::UnsupportedProvider {
                provider_id: selected_provider_id.to_owned(),
            }
        })?;

    if !selected_provider.is_enabled() {
        return Err(AppError::Provider(ProviderError::UnsupportedProvider {
            provider_id: selected_provider.id().to_owned(),
        }));
    }

    let api_key = config
        .provider_api_keys
        .get(selected_provider.id())
        .map(String::as_str)
        .filter(|api_key| !api_key.trim().is_empty())
        .ok_or_else(|| ConfigError::MissingProviderApiKey {
            provider: selected_provider.id().to_owned(),
        })?;

    Ok((selected_provider, api_key.to_owned()))
}

fn model_options_from_models(models: &[Model]) -> Vec<ModelOption> {
    models
        .iter()
        .map(|model| ModelOption::new(model.id(), model.display_name()))
        .collect()
}

fn current_task_model_selection(task_models: &TaskModels) -> Option<TaskModelSelection> {
    if task_models.planning.is_none()
        && task_models.labeling.is_none()
        && task_models.coding.is_none()
    {
        return None;
    }

    Some(TaskModelSelection::new(
        task_models.planning.as_deref().unwrap_or_default(),
        task_models.labeling.as_deref().unwrap_or_default(),
        task_models.coding.as_deref().unwrap_or_default(),
    ))
}

fn config_route(args: &ConfigArgs) -> ConfigRoute {
    if args.provider {
        return ConfigRoute::Provider;
    }

    if args.apikey {
        return ConfigRoute::ApiKey;
    }

    if args.models {
        return ConfigRoute::Models;
    }

    ConfigRoute::FullSetup
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
    ApiKeyInput(ApiKeyInputError),
    ModelAssignment(ModelAssignmentError),
    Plan(PlanError),
    Provider(ProviderError),
    ProviderSelection(ProviderSelectionError),
}

impl From<ConfigError> for AppError {
    fn from(error: ConfigError) -> Self {
        Self::Config(error)
    }
}

impl From<ProviderSelectionError> for AppError {
    fn from(error: ProviderSelectionError) -> Self {
        Self::ProviderSelection(error)
    }
}

impl From<ApiKeyInputError> for AppError {
    fn from(error: ApiKeyInputError) -> Self {
        Self::ApiKeyInput(error)
    }
}

impl From<ProviderError> for AppError {
    fn from(error: ProviderError) -> Self {
        Self::Provider(error)
    }
}

impl From<ModelAssignmentError> for AppError {
    fn from(error: ModelAssignmentError) -> Self {
        Self::ModelAssignment(error)
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
        AppError::ApiKeyInput(error) => error.to_string(),
        AppError::ModelAssignment(error) => error.to_string(),
        AppError::Provider(error) => format_provider_error(error),
        AppError::ProviderSelection(error) => error.to_string(),
    }
}

fn format_provider_error(error: &ProviderError) -> String {
    match error {
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
        ProviderError::UnsupportedProvider { provider_id } => {
            format!("Provider `{provider_id}` is not available.")
        }
        ProviderError::UnsupportedValidationMode => {
            "The selected provider does not support API key validation.".to_owned()
        }
    }
}

fn format_config_error(error: &ConfigError) -> String {
    let setup_instruction = "Run `spectacular config` to complete setup.";

    match error {
        ConfigError::MissingConfigFile { .. } => {
            format!("Configuration is missing. {setup_instruction}")
        }
        ConfigError::InvalidJson { path, .. } => format!(
            "Configuration file contains invalid JSON at {}. {setup_instruction}",
            path.display()
        ),
        ConfigError::NoSelectedProvider => {
            format!("Configuration is incomplete: no provider is selected. {setup_instruction}")
        }
        ConfigError::MissingProviderApiKey { provider } => format!(
            "Configuration is incomplete: provider `{provider}` has no API key. {setup_instruction}"
        ),
        ConfigError::MissingTaskModel { slot } => format!(
            "Configuration is incomplete: missing `{slot}` model assignment. {setup_instruction}"
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
    use spectacular_config::TaskModels;
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
    fn config_help_lists_setup_flags() {
        let mut command = Cli::command();
        let config = command
            .find_subcommand_mut("config")
            .expect("config subcommand should exist");
        let mut buffer = Vec::new();

        config.write_long_help(&mut buffer).unwrap();
        let help = String::from_utf8(buffer).unwrap();

        assert!(help.contains("--provider"));
        assert!(help.contains("--apikey"));
        assert!(help.contains("--models"));
    }

    #[test]
    fn config_routes_to_provider_placeholder() {
        let args = ConfigArgs {
            provider: true,
            apikey: false,
            models: false,
        };

        assert_eq!(config_route(&args), ConfigRoute::Provider);
    }

    #[test]
    fn config_routes_to_api_key_flow() {
        let args = ConfigArgs {
            provider: false,
            apikey: true,
            models: false,
        };

        assert_eq!(config_route(&args), ConfigRoute::ApiKey);
    }

    #[test]
    fn config_defaults_to_full_setup_route() {
        let args = ConfigArgs {
            provider: false,
            apikey: false,
            models: false,
        };

        assert_eq!(config_route(&args), ConfigRoute::FullSetup);
    }

    #[test]
    fn full_setup_runs_provider_api_key_and_models_in_order() {
        let calls = RefCell::new(Vec::new());

        let output = handle_full_config_with_steps(
            || {
                calls.borrow_mut().push("provider");
                Ok("provider done".to_owned())
            },
            || {
                calls.borrow_mut().push("apikey");
                Ok("apikey done".to_owned())
            },
            || {
                calls.borrow_mut().push("models");
                Ok("models done".to_owned())
            },
        )
        .unwrap();

        assert_eq!(calls.into_inner(), ["provider", "apikey", "models"]);
        assert_eq!(output, "provider done\napikey done\nmodels done");
    }

    #[test]
    fn provider_config_saves_enabled_provider_selection() {
        let saved_config = RefCell::new(None);

        let output = handle_provider_config_with_selection(
            || Ok(SpectacularConfig::default()),
            |config| {
                saved_config.replace(Some(config.clone()));
                Ok(())
            },
            |_| Ok("openrouter".to_owned()),
        )
        .unwrap();

        let saved_config = saved_config.into_inner().unwrap();
        assert_eq!(output, "Selected provider: OpenRouter");
        assert_eq!(
            saved_config.selected_provider.as_deref(),
            Some("openrouter")
        );
    }

    #[test]
    fn provider_config_rejects_disabled_provider_selection() {
        let output = handle_provider_config_with_selection(
            || Ok(SpectacularConfig::default()),
            |_| panic!("disabled provider must not be written"),
            |_| Ok("openai".to_owned()),
        )
        .unwrap_err();

        assert!(matches!(
            output,
            AppError::ProviderSelection(ProviderSelectionError::DisabledProvider { .. })
        ));
        assert!(user_facing_error(&output).contains("disabled"));
    }

    #[test]
    fn provider_config_options_mask_stored_keys() {
        let mut config = SpectacularConfig::default();
        config.provider_api_keys.insert(
            "openrouter".to_owned(),
            "sk-or-v1-1de08abcdefghijklmnopqrstuvwxyz".to_owned(),
        );

        let options = provider_options_from_config(spectacular_llms::provider_registry(), &config);
        let openrouter = options
            .iter()
            .find(|provider| provider.id() == "openrouter")
            .unwrap();

        assert!(openrouter
            .masked_api_key()
            .unwrap()
            .starts_with("sk-or-v1-1de"));
        assert!(!openrouter
            .masked_api_key()
            .unwrap()
            .contains("abcdefghijklmnopqrstuvwxyz"));
    }

    #[test]
    fn api_key_config_requires_selected_provider() {
        let output = handle_api_key_config_with_input(
            || Ok(SpectacularConfig::default()),
            |_| panic!("missing provider must not be written"),
            |_, _| panic!("missing provider must not open input screen"),
        )
        .unwrap_err();

        assert!(matches!(
            output,
            AppError::Config(ConfigError::NoSelectedProvider)
        ));
        assert!(user_facing_error(&output).contains("spectacular config"));
    }

    #[test]
    fn api_key_config_saves_validated_key_for_selected_provider_only() {
        let saved_config = RefCell::new(None);
        let mut config = SpectacularConfig {
            selected_provider: Some("openrouter".to_owned()),
            ..SpectacularConfig::default()
        };
        config.set_provider_api_key("openrouter", "old-openrouter-key");
        config.set_provider_api_key("openai", "existing-openai-key");

        let output = handle_api_key_config_with_input(
            || Ok(config),
            |config| {
                saved_config.replace(Some(config.clone()));
                Ok(())
            },
            |provider, current_api_key| {
                assert_eq!(provider.id(), "openrouter");
                assert_eq!(current_api_key, Some("old-openrouter-key"));
                Ok("sk-or-v1-new-secret".to_owned())
            },
        )
        .unwrap();

        let saved_config = saved_config.into_inner().unwrap();
        assert_eq!(output, "Saved API key for OpenRouter.");
        assert!(!output.contains("sk-or-v1-new-secret"));
        assert_eq!(
            saved_config
                .provider_api_keys
                .get("openrouter")
                .map(String::as_str),
            Some("sk-or-v1-new-secret")
        );
        assert_eq!(
            saved_config
                .provider_api_keys
                .get("openai")
                .map(String::as_str),
            Some("existing-openai-key")
        );
    }

    #[test]
    fn api_key_config_does_not_save_when_input_is_cancelled() {
        let config = SpectacularConfig {
            selected_provider: Some("openrouter".to_owned()),
            ..SpectacularConfig::default()
        };

        let output = handle_api_key_config_with_input(
            || Ok(config),
            |_| panic!("cancelled input must not be written"),
            |_, _| Err(ApiKeyInputError::Cancelled),
        )
        .unwrap_err();

        assert!(matches!(
            output,
            AppError::ApiKeyInput(ApiKeyInputError::Cancelled)
        ));
    }

    #[test]
    fn models_config_requires_selected_provider() {
        let output = handle_models_config_with_selection(
            || Ok(SpectacularConfig::default()),
            |_| panic!("missing provider must not be written"),
            |_, _| panic!("missing provider must not fetch models"),
            |_, _, _| panic!("missing provider must not open model assignment"),
        )
        .unwrap_err();

        assert!(matches!(
            output,
            AppError::Config(ConfigError::NoSelectedProvider)
        ));
        assert!(user_facing_error(&output).contains("spectacular config"));
    }

    #[test]
    fn models_config_requires_selected_provider_api_key() {
        let config = SpectacularConfig {
            selected_provider: Some("openrouter".to_owned()),
            ..SpectacularConfig::default()
        };

        let output = handle_models_config_with_selection(
            || Ok(config),
            |_| panic!("missing API key must not be written"),
            |_, _| panic!("missing API key must not fetch models"),
            |_, _, _| panic!("missing API key must not open model assignment"),
        )
        .unwrap_err();

        assert!(matches!(
            output,
            AppError::Config(ConfigError::MissingProviderApiKey { .. })
        ));
        assert!(user_facing_error(&output).contains("spectacular config"));
    }

    #[test]
    fn models_config_fetches_models_and_saves_all_assignments() {
        let saved_config = RefCell::new(None);
        let mut config = SpectacularConfig {
            selected_provider: Some("openrouter".to_owned()),
            ..SpectacularConfig::default()
        };
        config.set_provider_api_key("openrouter", "sk-or-v1-valid");

        let output = handle_models_config_with_selection(
            || Ok(config),
            |config| {
                saved_config.replace(Some(config.clone()));
                Ok(())
            },
            |provider, api_key| {
                assert_eq!(provider.id(), "openrouter");
                assert_eq!(api_key, "sk-or-v1-valid");
                Ok(vec![
                    Model::new("openai/gpt-4o", "GPT-4o"),
                    Model::new("anthropic/claude-sonnet-4.5", "Claude Sonnet"),
                ])
            },
            |provider, models, current_selection| {
                assert_eq!(provider.display_name(), "OpenRouter");
                assert_eq!(models[0].id(), "openai/gpt-4o");
                assert!(current_selection.is_none());
                Ok(TaskModelSelection::new(
                    "openai/gpt-4o",
                    "anthropic/claude-sonnet-4.5",
                    "openai/gpt-4o",
                ))
            },
        )
        .unwrap();

        let saved_config = saved_config.into_inner().unwrap();
        assert_eq!(output, "Saved model assignments for OpenRouter.");
        assert_eq!(
            saved_config.task_models,
            TaskModels {
                planning: Some("openai/gpt-4o".to_owned()),
                labeling: Some("anthropic/claude-sonnet-4.5".to_owned()),
                coding: Some("openai/gpt-4o".to_owned()),
            }
        );
    }

    #[test]
    fn models_config_does_not_save_when_assignment_is_cancelled() {
        let mut config = SpectacularConfig {
            selected_provider: Some("openrouter".to_owned()),
            ..SpectacularConfig::default()
        };
        config.set_provider_api_key("openrouter", "sk-or-v1-valid");

        let output = handle_models_config_with_selection(
            || Ok(config),
            |_| panic!("cancelled assignment must not be written"),
            |_, _| Ok(vec![Model::new("openai/gpt-4o", "GPT-4o")]),
            |_, _, _| Err(ModelAssignmentError::Cancelled),
        )
        .unwrap_err();

        assert!(matches!(
            output,
            AppError::ModelAssignment(ModelAssignmentError::Cancelled)
        ));
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
    fn incomplete_config_errors_tell_user_to_run_config() {
        let error = AppError::Plan(PlanError::Config(ConfigError::NoSelectedProvider));

        assert!(user_facing_error(&error).contains("spectacular config"));
    }

    fn complete_config() -> SpectacularConfig {
        let mut provider_api_keys = BTreeMap::new();
        provider_api_keys.insert("openrouter".to_owned(), "sk-or-v1-test".to_owned());

        SpectacularConfig {
            selected_provider: Some("openrouter".to_owned()),
            provider_api_keys,
            task_models: TaskModels {
                planning: Some("openrouter/planning".to_owned()),
                labeling: Some("openrouter/labeling".to_owned()),
                coding: Some("openrouter/coding".to_owned()),
            },
        }
    }
}
