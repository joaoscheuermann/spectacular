/// Handles the handle config with io step for parsed config command input.
fn handle_config_with_io(
    args: ConfigArgs,
    load_config: impl FnOnce() -> Result<SpectacularConfig, ConfigError>,
    load_cache: impl FnOnce() -> Result<ModelCache, ConfigError>,
    backup_config: impl FnOnce() -> Result<Option<std::path::PathBuf>, ConfigError>,
    write_config: impl FnOnce(&SpectacularConfig) -> Result<(), ConfigError>,
) -> Result<String, AppError> {
    let operation = config_operation(args)?;

    match operation {
        ConfigOperation::Show => {
            let config = load_config()?;
            Ok(format_config_report(&config))
        }
        ConfigOperation::AddProvider {
            name,
            provider_type,
            apikey,
        } => {
            let provider = supported_provider_type(&provider_type)?;
            let mut config = load_config()?;
            config.add_provider(name.as_str(), provider.id(), apikey)?;
            write_config(&config)?;
            Ok(format_provider_added_output(&name, provider.display_name()))
        }
        ConfigOperation::RemoveProvider { name, confirm } => {
            if !confirm {
                return Ok(format_confirmation_required_output(
                    "Provider removal requires confirm:true and will leave any saved models that reference it orphaned.",
                ));
            }

            let mut config = load_config()?;
            backup_config()?;
            config.remove_provider(&name)?;
            write_config(&config)?;
            Ok(format_provider_removed_output(&name))
        }
        ConfigOperation::AddModel {
            provider,
            model_id,
            reasoning,
            name,
        } => {
            let mut config = load_config()?;
            let cache = load_cache()?;
            validate_model_from_cache(&cache, &provider, &model_id, reasoning)?;
            let key = config.add_model(provider, model_id, reasoning, name)?;
            write_config(&config)?;
            Ok(format_model_saved_output("added", &key))
        }
        ConfigOperation::EditModel {
            name,
            provider,
            model_id,
            reasoning,
        } => {
            let mut config = load_config()?;
            let cache = load_cache()?;
            let current =
                config
                    .models
                    .get(&name)
                    .ok_or_else(|| ConfigError::ModelNotConfigured {
                        model: name.clone(),
                    })?;
            let next_provider = provider.as_deref().unwrap_or(current.provider.as_str());
            let next_model = model_id.as_deref().unwrap_or(current.model.as_str());
            let next_reasoning = reasoning.unwrap_or(current.reasoning);
            validate_model_from_cache(&cache, next_provider, next_model, next_reasoning)?;
            config.edit_model(&name, provider, model_id, reasoning)?;
            write_config(&config)?;
            Ok(format_model_saved_output("updated", &name))
        }
        ConfigOperation::RemoveModel { name, confirm } => {
            let mut config = load_config()?;
            let references = config.tasks.references_to(&name);
            if !confirm {
                return Ok(format_model_remove_confirmation_output(&name, &references));
            }

            backup_config()?;
            config.remove_model(&name)?;
            write_config(&config)?;
            Ok(format_model_removed_output(&name, &references))
        }
        ConfigOperation::SetTask { task, model } => {
            let mut config = load_config()?;
            config.set_task_model(task, model.as_str())?;
            write_config(&config)?;
            Ok(format_task_saved_output(task, &model))
        }
    }
}

/// Handles the config operation step for parsed config command input.
fn config_operation(args: ConfigArgs) -> Result<ConfigOperation, AppError> {
    let Some(command) = args.command else {
        return Ok(ConfigOperation::Show);
    };

    match command {
        ConfigCommand::Provider { command } => provider_operation(command),
        ConfigCommand::Model { command } => model_operation(command),
        ConfigCommand::Task { command } => task_operation(command),
    }
}

/// Handles the provider operation step for parsed config command input.
fn provider_operation(command: ConfigProviderCommand) -> Result<ConfigOperation, AppError> {
    match command {
        ConfigProviderCommand::Add { fields } => {
            let args = parse_named_fields(&fields, &["name", "type", "apikey"])?;
            Ok(ConfigOperation::AddProvider {
                name: args.require("name")?.to_owned(),
                provider_type: args.require("type")?.to_owned(),
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

/// Handles the model operation step for parsed config command input.
fn model_operation(command: ConfigModelCommand) -> Result<ConfigOperation, AppError> {
    match command {
        ConfigModelCommand::Add { fields } => {
            let args = parse_named_fields(&fields, &["provider", "id", "reasoning", "name"])?;
            Ok(ConfigOperation::AddModel {
                provider: args.require("provider")?.to_owned(),
                model_id: args.require("id")?.to_owned(),
                reasoning: parse_reasoning_level(args.require("reasoning")?)?,
                name: args.optional("name").map(str::to_owned),
            })
        }
        ConfigModelCommand::Edit { fields } => {
            let args = parse_named_fields(&fields, &["name", "provider", "id", "reasoning"])?;
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
        ConfigModelCommand::Remove { fields } => {
            let args = parse_named_fields(&fields, &["name", "confirm"])?;
            Ok(ConfigOperation::RemoveModel {
                name: args.require("name")?.to_owned(),
                confirm: parse_confirm(args.optional("confirm")),
            })
        }
    }
}

/// Handles the task operation step for parsed config command input.
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

/// Handles the parse named fields step for parsed config command input.
fn parse_named_fields(fields: &[String], allowed: &[&str]) -> Result<NamedArgs, AppError> {
    crate::config_fields::named_args(fields, allowed)
        .map_err(|error| AppError::InvalidConfigCommand(error.to_string()))
}

/// Handles the parse confirm step for parsed config command input.
fn parse_confirm(value: Option<&str>) -> bool {
    value == Some("true")
}

/// Handles the supported provider type step for parsed config command input.
fn supported_provider_type(provider_type: &str) -> Result<ProviderMetadata, AppError> {
    let provider =
        spectacular_llms::provider_by_id(provider_type).ok_or_else(|| AppError::Provider {
            source: ProviderError::UnsupportedProvider {
                provider_id: provider_type.to_owned(),
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

/// Handles the validate model from cache step for parsed config command input.
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
                "Model `{model_id}` is not available in API metadata cache for provider `{provider}`. Start `spectacular chat` with a configured API key to refresh model metadata."
            )))?;

    validate_reasoning_for_cached_model(metadata, model_id, reasoning)
}

/// Handles the validate reasoning for cached model step for parsed config command input.
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

/// Handles the parse task model slot step for parsed config command input.
fn parse_task_model_slot(value: &str) -> Result<TaskModelSlot, AppError> {
    value
        .parse::<TaskModelSlot>()
        .map_err(|error| AppError::InvalidConfigCommand(error.to_string()))
}

/// Handles the parse reasoning level step for parsed config command input.
fn parse_reasoning_level(value: &str) -> Result<ReasoningLevel, AppError> {
    value
        .parse::<ReasoningLevel>()
        .map_err(|error| AppError::InvalidConfigCommand(error.to_string()))
}
