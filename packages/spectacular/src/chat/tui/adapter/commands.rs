use crate::chat::commands::{
    ChatCommandAdapter, CompletionCommandSpec, CompletionFieldSpec, CompletionSubcommandSpec,
    CompletionValueValidation,
};
use crate::chat::model::ChatModel;
use spectacular_commands::CommandRegistry;
use spectacular_config::{ModelCache, ReasoningLevel, SpectacularConfig, TaskModelSlot};
use spectacular_tui::{
    CachedModelValues, ChatTuiAction, CommandDescriptor,
    CommandFieldDescriptor as TuiCommandFieldDescriptor,
    CommandSubcommandDescriptor as TuiCommandSubcommandDescriptor,
    CommandValueValidation as TuiCommandValueValidation, CompletionValues,
};
use std::collections::BTreeMap;

type FieldKey<'a> = (&'a str, &'a str, &'a str);

/// Projects a command registry into command descriptors for prompt UI state.
#[cfg(test)]
pub(crate) fn commands_loaded_action<C>(registry: &CommandRegistry<C>) -> ChatTuiAction {
    ChatTuiAction::CommandsLoaded(command_descriptors(registry))
}

/// Projects chat command metadata and structured completion snapshots into TUI state.
pub(crate) fn commands_loaded_with_completions_action(
    commands: &ChatCommandAdapter,
    model: &ChatModel,
) -> ChatTuiAction {
    let mut descriptors = command_descriptors(commands.metadata());
    attach_completion_specs(&mut descriptors, commands.completion_specs(), model);
    ChatTuiAction::CommandsLoaded(descriptors)
}

/// Converts registry metadata into basic TUI command descriptors.
fn command_descriptors<C>(registry: &CommandRegistry<C>) -> Vec<CommandDescriptor> {
    registry
        .command_metadata()
        .map(|metadata| {
            CommandDescriptor::with_usage(metadata.name, metadata.summary, metadata.usage)
        })
        .collect()
}

/// Attaches structured subcommands and reducer-safe completion values to descriptors.
fn attach_completion_specs(
    descriptors: &mut [CommandDescriptor],
    specs: &[CompletionCommandSpec],
    model: &ChatModel,
) {
    let snapshot = CompletionSnapshot::from_model(model);
    for spec in specs {
        let Some(descriptor) = descriptors
            .iter_mut()
            .find(|descriptor| descriptor.name == spec.name)
        else {
            continue;
        };

        descriptor.subcommands = spec
            .subcommands
            .iter()
            .map(|subcommand| subcommand_descriptor(spec.name, subcommand, &snapshot))
            .collect();
    }
}

/// Converts one app-owned subcommand spec into TUI-safe metadata.
fn subcommand_descriptor(
    command: &str,
    subcommand: &CompletionSubcommandSpec,
    snapshot: &CompletionSnapshot,
) -> TuiCommandSubcommandDescriptor {
    TuiCommandSubcommandDescriptor::new(
        subcommand.name,
        subcommand.summary,
        subcommand
            .fields
            .iter()
            .map(|field| field_descriptor(command, subcommand.name, field, snapshot)),
    )
}

/// Converts one app-owned field spec into TUI-safe metadata.
fn field_descriptor(
    command: &str,
    subcommand: &str,
    field: &CompletionFieldSpec,
    snapshot: &CompletionSnapshot,
) -> TuiCommandFieldDescriptor {
    TuiCommandFieldDescriptor::new(
        field.name,
        field.summary,
        field.required,
        completion_values((command, subcommand, field.name), snapshot),
        validation(field.validation),
    )
}

/// Converts app-owned validation metadata into reducer-safe validation metadata.
fn validation(validation: CompletionValueValidation) -> TuiCommandValueValidation {
    match validation {
        CompletionValueValidation::None => TuiCommandValueValidation::None,
        CompletionValueValidation::OneOfValues => TuiCommandValueValidation::OneOfValues,
    }
}

/// Returns reducer-safe value metadata for known structured command fields.
fn completion_values(key: FieldKey<'_>, snapshot: &CompletionSnapshot) -> CompletionValues {
    provider_values(key, snapshot)
        .or_else(|| model_values(key, snapshot))
        .or_else(|| task_values(key, snapshot))
        .unwrap_or(CompletionValues::None)
}

fn provider_values(key: FieldKey<'_>, snapshot: &CompletionSnapshot) -> Option<CompletionValues> {
    match key {
        ("provider", "add", "provider") => Some(CompletionValues::Static(
            snapshot.enabled_provider_type_ids.clone(),
        )),
        ("provider", "auth", "provider") => Some(openai_provider_value()),
        ("provider", "remove", "name") => Some(snapshot.configured_provider_values()),
        _ => None,
    }
}

fn model_values(key: FieldKey<'_>, snapshot: &CompletionSnapshot) -> Option<CompletionValues> {
    match key {
        ("model", "add", "provider") | ("model", "edit", "provider") => {
            Some(snapshot.configured_provider_values())
        }
        ("model", "add", "id") | ("model", "edit", "id") => Some(snapshot.cached_model_values()),
        ("model", "add", "reasoning") | ("model", "edit", "reasoning") => Some(reasoning_values()),
        ("model", "add", "name") | ("model", "edit", "name") | ("model", "remove", "name") => {
            Some(snapshot.saved_model_values())
        }
        _ => None,
    }
}

fn task_values(key: FieldKey<'_>, snapshot: &CompletionSnapshot) -> Option<CompletionValues> {
    match key {
        ("task", "set", "task") => Some(task_slot_values()),
        ("task", "set", "model") => Some(snapshot.saved_model_values()),
        _ => None,
    }
}

fn openai_provider_value() -> CompletionValues {
    CompletionValues::Static(vec![spectacular_llms::OPENAI_PROVIDER_ID.to_owned()])
}

fn reasoning_values() -> CompletionValues {
    CompletionValues::Static(
        ReasoningLevel::ALL
            .into_iter()
            .map(|level| level.as_str().to_owned())
            .collect(),
    )
}

fn task_slot_values() -> CompletionValues {
    CompletionValues::Static(
        TaskModelSlot::ALL
            .into_iter()
            .map(|slot| slot.as_str().to_owned())
            .collect(),
    )
}

/// Snapshot of dynamic completion data safe to store in TUI reducer state.
struct CompletionSnapshot {
    enabled_provider_type_ids: Vec<String>,
    config: Result<SpectacularConfig, String>,
    cache: Result<ModelCache, String>,
}

impl CompletionSnapshot {
    /// Reads dynamic completion sources once at the adapter boundary.
    fn from_model(model: &ChatModel) -> Self {
        let config_io = model.config_io();
        Self {
            enabled_provider_type_ids: spectacular_llms::provider_registry()
                .iter()
                .filter(|provider| provider.is_enabled())
                .map(|provider| provider.id().to_owned())
                .collect(),
            config: config_io
                .read_config_or_default()
                .map_err(|error| error.to_string()),
            cache: config_io
                .read_model_cache_or_default()
                .map_err(|error| error.to_string()),
        }
    }

    /// Returns configured provider names or a value-unavailable marker.
    fn configured_provider_values(&self) -> CompletionValues {
        self.config
            .as_ref()
            .map(|config| {
                CompletionValues::ConfiguredProviders(config.providers.keys().cloned().collect())
            })
            .unwrap_or_else(|error| CompletionValues::Unavailable(error.clone()))
    }

    /// Returns saved model aliases or a value-unavailable marker.
    fn saved_model_values(&self) -> CompletionValues {
        self.config
            .as_ref()
            .map(|config| CompletionValues::SavedModels(config.models.keys().cloned().collect()))
            .unwrap_or_else(|error| CompletionValues::Unavailable(error.clone()))
    }

    /// Returns cached model values for provider-scoped model ID completion.
    fn cached_model_values(&self) -> CompletionValues {
        let cache = match self.cache.as_ref() {
            Ok(cache) => cache,
            Err(error) => return CompletionValues::Unavailable(error.clone()),
        };

        CompletionValues::CachedModelIds(CachedModelValues::new(
            model_ids_by_provider(cache),
            self.saved_model_providers(),
        ))
    }

    fn saved_model_providers(&self) -> BTreeMap<String, String> {
        self.config
            .as_ref()
            .map(|config| {
                config
                    .models
                    .iter()
                    .map(|(name, model)| (name.clone(), model.provider.clone()))
                    .collect()
            })
            .unwrap_or_default()
    }
}

fn model_ids_by_provider(cache: &ModelCache) -> BTreeMap<String, Vec<String>> {
    cache
        .providers
        .iter()
        .map(|(provider, cache)| {
            (
                provider.clone(),
                cache.models.keys().cloned().collect::<Vec<_>>(),
            )
        })
        .collect()
}
