use super::{
    cli_types::{
        ConfigArgs, ConfigCommand, ConfigModelCommand, ConfigOperation, ConfigProviderCommand,
        ConfigTaskCommand,
    },
    output::{
        format_config_report, format_confirmation_required_output,
        format_model_remove_confirmation_output, format_model_removed_output,
        format_model_saved_output, format_provider_added_output, format_provider_removed_output,
        format_task_saved_output,
    },
    plan_errors::AppError,
};
use spectacular_commands::NamedArgs;
use spectacular_config::{
    CachedModelMetadata, ConfigError, ModelCache, ReasoningLevel, SpectacularConfig, TaskModelSlot,
};
use spectacular_llms::{ProviderError, ProviderMetadata};
use std::path::PathBuf;

type ConfigResult<T> = Result<T, ConfigError>;
type LoadConfig<'a> = Box<dyn FnOnce() -> ConfigResult<SpectacularConfig> + 'a>;
type LoadCache<'a> = Box<dyn FnOnce() -> ConfigResult<ModelCache> + 'a>;
type BackupConfig<'a> = Box<dyn FnOnce() -> ConfigResult<Option<PathBuf>> + 'a>;
type WriteConfig<'a> = Box<dyn FnOnce(&SpectacularConfig) -> ConfigResult<()> + 'a>;

pub(super) struct ConfigIo<'a> {
    load_config: LoadConfig<'a>,
    load_cache: LoadCache<'a>,
    backup_config: BackupConfig<'a>,
    write_config: WriteConfig<'a>,
}

impl<'a> ConfigIo<'a> {
    pub(super) fn new(
        load_config: impl FnOnce() -> ConfigResult<SpectacularConfig> + 'a,
        load_cache: impl FnOnce() -> ConfigResult<ModelCache> + 'a,
        backup_config: impl FnOnce() -> ConfigResult<Option<PathBuf>> + 'a,
        write_config: impl FnOnce(&SpectacularConfig) -> ConfigResult<()> + 'a,
    ) -> Self {
        Self {
            load_config: Box::new(load_config),
            load_cache: Box::new(load_cache),
            backup_config: Box::new(backup_config),
            write_config: Box::new(write_config),
        }
    }
}

struct ModelAdd {
    provider: String,
    model_id: String,
    reasoning: ReasoningLevel,
    name: Option<String>,
}

struct ModelEdit {
    name: String,
    provider: Option<String>,
    model_id: Option<String>,
    reasoning: Option<ReasoningLevel>,
}

impl ModelAdd {
    fn new(
        provider: String,
        model_id: String,
        reasoning: ReasoningLevel,
        name: Option<String>,
    ) -> Self {
        Self {
            provider,
            model_id,
            reasoning,
            name,
        }
    }
}

impl ModelEdit {
    fn new(
        name: String,
        provider: Option<String>,
        model_id: Option<String>,
        reasoning: Option<ReasoningLevel>,
    ) -> Self {
        Self {
            name,
            provider,
            model_id,
            reasoning,
        }
    }
}

/// Executes a parsed config command with injected config and cache IO.
pub(super) fn handle_config_with_io(
    args: ConfigArgs,
    io: ConfigIo<'_>,
) -> Result<String, AppError> {
    let operation = config_operation(args)?;
    run_config_operation(operation, io)
}

fn run_config_operation(operation: ConfigOperation, io: ConfigIo<'_>) -> Result<String, AppError> {
    match operation {
        ConfigOperation::Show => show_config(io),
        ConfigOperation::AddProvider {
            provider_type,
            apikey,
        } => add_provider(provider_type, apikey, io),
        ConfigOperation::RemoveProvider { name, confirm } => remove_provider(name, confirm, io),
        ConfigOperation::AddModel {
            provider,
            model_id,
            reasoning,
            name,
        } => add_model(ModelAdd::new(provider, model_id, reasoning, name), io),
        ConfigOperation::EditModel {
            name,
            provider,
            model_id,
            reasoning,
        } => edit_model(ModelEdit::new(name, provider, model_id, reasoning), io),
        ConfigOperation::RemoveModel { name, confirm } => remove_model(name, confirm, io),
        ConfigOperation::SetTask { task, model } => set_task(task, model, io),
    }
}

fn show_config(io: ConfigIo<'_>) -> Result<String, AppError> {
    let config = (io.load_config)()?;
    Ok(format_config_report(&config))
}

fn add_provider(
    provider_type: String,
    apikey: String,
    io: ConfigIo<'_>,
) -> Result<String, AppError> {
    let provider = supported_provider_type(&provider_type)?;
    let mut config = (io.load_config)()?;
    config.set_provider_api_key(provider.id(), apikey)?;
    (io.write_config)(&config)?;

    Ok(format_provider_added_output(
        provider.id(),
        provider.display_name(),
    ))
}

fn remove_provider(name: String, confirm: bool, io: ConfigIo<'_>) -> Result<String, AppError> {
    if !confirm {
        return Ok(format_confirmation_required_output(
            "Provider removal requires confirm:true and will leave any saved models that reference it orphaned.",
        ));
    }

    let mut config = (io.load_config)()?;
    (io.backup_config)()?;
    config.remove_provider(&name)?;
    (io.write_config)(&config)?;

    Ok(format_provider_removed_output(&name))
}

fn add_model(input: ModelAdd, io: ConfigIo<'_>) -> Result<String, AppError> {
    let mut config = (io.load_config)()?;
    let cache = (io.load_cache)()?;
    validate_model_from_cache(&cache, &input.provider, &input.model_id, input.reasoning)?;

    let key = config.add_model(input.provider, input.model_id, input.reasoning, input.name)?;
    (io.write_config)(&config)?;

    Ok(format_model_saved_output("added", &key))
}

fn edit_model(input: ModelEdit, io: ConfigIo<'_>) -> Result<String, AppError> {
    let mut config = (io.load_config)()?;
    let cache = (io.load_cache)()?;
    validate_model_edit(&config, &cache, &input)?;

    config.edit_model(&input.name, input.provider, input.model_id, input.reasoning)?;
    (io.write_config)(&config)?;

    Ok(format_model_saved_output("updated", &input.name))
}

fn validate_model_edit(
    config: &SpectacularConfig,
    cache: &ModelCache,
    input: &ModelEdit,
) -> Result<(), AppError> {
    let current =
        config
            .models
            .get(&input.name)
            .ok_or_else(|| ConfigError::ModelNotConfigured {
                model: input.name.clone(),
            })?;
    let provider = input
        .provider
        .as_deref()
        .unwrap_or(current.provider.as_str());
    let model = input.model_id.as_deref().unwrap_or(current.model.as_str());
    let reasoning = input.reasoning.unwrap_or(current.reasoning);

    validate_model_from_cache(cache, provider, model, reasoning)
}

fn remove_model(name: String, confirm: bool, io: ConfigIo<'_>) -> Result<String, AppError> {
    let mut config = (io.load_config)()?;
    let references = config.tasks.references_to(&name);
    if !confirm {
        return Ok(format_model_remove_confirmation_output(&name, &references));
    }

    (io.backup_config)()?;
    config.remove_model(&name)?;
    (io.write_config)(&config)?;

    Ok(format_model_removed_output(&name, &references))
}

fn set_task(task: TaskModelSlot, model: String, io: ConfigIo<'_>) -> Result<String, AppError> {
    let mut config = (io.load_config)()?;
    config.set_task_model(task, model.as_str())?;
    (io.write_config)(&config)?;

    Ok(format_task_saved_output(task, &model))
}

/// Converts parsed config CLI args into a typed operation.
pub(super) fn config_operation(args: ConfigArgs) -> Result<ConfigOperation, AppError> {
    let Some(command) = args.command else {
        return Ok(ConfigOperation::Show);
    };

    match command {
        ConfigCommand::Provider { command } => provider_operation(command),
        ConfigCommand::Model { command } => model_operation(command),
        ConfigCommand::Task { command } => task_operation(command),
    }
}

fn provider_operation(command: ConfigProviderCommand) -> Result<ConfigOperation, AppError> {
    match command {
        ConfigProviderCommand::Add { fields } => {
            let args = parse_named_fields(&fields, &["provider", "apikey"])?;
            Ok(ConfigOperation::AddProvider {
                provider_type: args.require("provider")?.to_owned(),
                apikey: args.require("apikey")?.to_owned(),
            })
        }
        ConfigProviderCommand::Remove { fields } => {
            let args = parse_named_fields(&fields, &["name", "confirm"])?;
            Ok(ConfigOperation::RemoveProvider {
                name: args.require("name")?.to_owned(),
                confirm: parse_confirm(args.optional("confirm")),
            })
        }
    }
}

fn model_operation(command: ConfigModelCommand) -> Result<ConfigOperation, AppError> {
    match command {
        ConfigModelCommand::Add { fields } => model_add_operation(&fields),
        ConfigModelCommand::Edit { fields } => model_edit_operation(&fields),
        ConfigModelCommand::Remove { fields } => model_remove_operation(&fields),
    }
}

fn model_add_operation(fields: &[String]) -> Result<ConfigOperation, AppError> {
    let args = parse_named_fields(fields, &["provider", "id", "reasoning", "name"])?;
    Ok(ConfigOperation::AddModel {
        provider: args.require("provider")?.to_owned(),
        model_id: args.require("id")?.to_owned(),
        reasoning: parse_reasoning_level(args.require("reasoning")?)?,
        name: args.optional("name").map(str::to_owned),
    })
}

fn model_edit_operation(fields: &[String]) -> Result<ConfigOperation, AppError> {
    let args = parse_named_fields(fields, &["name", "provider", "id", "reasoning"])?;
    Ok(ConfigOperation::EditModel {
        name: args.require("name")?.to_owned(),
        provider: args.optional("provider").map(str::to_owned),
        model_id: args.optional("id").map(str::to_owned),
        reasoning: args
            .optional("reasoning")
            .map(parse_reasoning_level)
            .transpose()?,
    })
}

fn model_remove_operation(fields: &[String]) -> Result<ConfigOperation, AppError> {
    let args = parse_named_fields(fields, &["name", "confirm"])?;
    Ok(ConfigOperation::RemoveModel {
        name: args.require("name")?.to_owned(),
        confirm: parse_confirm(args.optional("confirm")),
    })
}

fn task_operation(command: ConfigTaskCommand) -> Result<ConfigOperation, AppError> {
    match command {
        ConfigTaskCommand::Set { fields } => {
            let args = parse_named_fields(&fields, &["task", "model"])?;
            Ok(ConfigOperation::SetTask {
                task: parse_task_model_slot(args.require("task")?)?,
                model: args.require("model")?.to_owned(),
            })
        }
    }
}

fn parse_named_fields(fields: &[String], allowed: &[&str]) -> Result<NamedArgs, AppError> {
    crate::config_fields::named_args(fields, allowed)
        .map_err(|error| AppError::InvalidConfigCommand(error.to_string()))
}

fn parse_confirm(value: Option<&str>) -> bool {
    value == Some("true")
}

fn supported_provider_type(provider_type: &str) -> Result<ProviderMetadata, AppError> {
    let provider =
        spectacular_llms::provider_by_id(provider_type).ok_or_else(|| AppError::Provider {
            source: Box::new(ProviderError::UnsupportedProvider {
                provider_id: provider_type.to_owned(),
            }),
        })?;

    if provider.is_enabled() {
        return Ok(provider);
    }

    Err(AppError::Provider {
        source: Box::new(ProviderError::UnsupportedProvider {
            provider_id: provider.id().to_owned(),
        }),
    })
}

fn validate_model_from_cache(
    cache: &ModelCache,
    provider: &str,
    model_id: &str,
    reasoning: ReasoningLevel,
) -> Result<(), AppError> {
    let metadata =
        cache
            .model(provider, model_id)
            .ok_or_else(|| AppError::InvalidConfigCommand(format!(
                "Model `{model_id}` is not available in API metadata cache for provider `{provider}`. Start `spectacular` with a configured API key to refresh model metadata."
            )))?;

    validate_reasoning_for_cached_model(metadata, model_id, reasoning)
}

fn validate_reasoning_for_cached_model(
    metadata: &CachedModelMetadata,
    model_id: &str,
    reasoning: ReasoningLevel,
) -> Result<(), AppError> {
    if !reasoning.non_none() || metadata.supports_reasoning() {
        return Ok(());
    }

    Err(AppError::InvalidConfigCommand(format!(
        "Model `{model_id}` does not advertise `reasoning` in supported_parameters; use reasoning:none."
    )))
}

fn parse_task_model_slot(value: &str) -> Result<TaskModelSlot, AppError> {
    value
        .parse::<TaskModelSlot>()
        .map_err(|error| AppError::InvalidConfigCommand(error.to_string()))
}

fn parse_reasoning_level(value: &str) -> Result<ReasoningLevel, AppError> {
    value
        .parse::<ReasoningLevel>()
        .map_err(|error| AppError::InvalidConfigCommand(error.to_string()))
}
