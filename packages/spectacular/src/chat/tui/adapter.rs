use super::display::{
    command_finished_action, command_output_action, command_started_action, ToolDisplayAdapter,
};
use crate::chat::command_event::CommandEvent;
use crate::chat::commands::{
    ChatCommandAdapter, CompletionCommandSpec, CompletionFieldSpec, CompletionSubcommandSpec,
    CompletionValueValidation,
};
use crate::chat::model::ChatModel;
#[cfg(test)]
pub(crate) use crate::chat::tui::state::{display_metadata_action, runtime_selection_action};
use spectacular_agent::{AgentEvent, ToolStorage};
use spectacular_commands::CommandRegistry;
use spectacular_config::{ModelCache, ReasoningLevel, SpectacularConfig, TaskModelSlot};
use spectacular_llms::FinishReason;
use spectacular_tui::{
    CachedModelValues, ChatTuiAction, CommandDescriptor,
    CommandFieldDescriptor as TuiCommandFieldDescriptor,
    CommandSubcommandDescriptor as TuiCommandSubcommandDescriptor,
    CommandValueValidation as TuiCommandValueValidation, CompletionValues,
    ContextTokenUsage as TuiContextTokenUsage, ProviderUsageMetadata as TuiProviderUsageMetadata,
    TranscriptItemId,
};
use std::collections::BTreeMap;

/// Converts runtime and agent events into pure TUI reducer actions.
#[derive(Default)]
pub(crate) struct TuiEventAdapter {
    tool_display: ToolDisplayAdapter,
}

impl TuiEventAdapter {
    /// Creates an adapter with empty lifecycle state for one runtime event stream.
    pub(crate) fn new() -> Self {
        Self {
            tool_display: ToolDisplayAdapter::new(),
        }
    }

    /// Converts one agent event into zero or more TUI actions without rendering terminal output.
    #[cfg(test)]
    pub(crate) fn adapt_agent_event(&mut self, event: &AgentEvent) -> Vec<ChatTuiAction> {
        self.adapt_agent_event_with_tools(event, &ToolStorage::default())
    }

    /// Converts one agent event into TUI actions using registered tool display formatters.
    pub(crate) fn adapt_agent_event_with_tools(
        &mut self,
        event: &AgentEvent,
        tools: &ToolStorage,
    ) -> Vec<ChatTuiAction> {
        match event {
            AgentEvent::UserPrompt {
                id: Some(id),
                content,
            } => {
                vec![self.user_prompt_action(id.as_str(), content)]
            }
            AgentEvent::UserPrompt { id: None, .. } => Vec::new(),
            AgentEvent::MessageStart { id } => vec![ChatTuiAction::MessageStarted {
                id: transcript_item_id(id.as_str()),
            }],
            AgentEvent::MessageDelta { id, content } => vec![ChatTuiAction::MessageDelta {
                id: transcript_item_id(id.as_str()),
                text: content.clone(),
            }],
            AgentEvent::MessageFinish { id } => vec![ChatTuiAction::MessageFinished {
                id: transcript_item_id(id.as_str()),
            }],
            AgentEvent::ReasoningStart { id } => vec![ChatTuiAction::ReasoningStarted {
                id: transcript_item_id(id.as_str()),
            }],
            AgentEvent::ReasoningDelta { id, content } => vec![ChatTuiAction::ReasoningDelta {
                id: transcript_item_id(id.as_str()),
                text: content.clone(),
            }],
            AgentEvent::ReasoningFinish { id } => vec![ChatTuiAction::ReasoningFinished {
                id: transcript_item_id(id.as_str()),
            }],
            AgentEvent::ToolCallStart {
                tool_call_id,
                name,
                arguments,
            } => self
                .tool_display
                .started_actions(tool_call_id, name, arguments, tools),
            AgentEvent::ToolCallDelta {
                tool_call_id,
                content,
            } => vec![ChatTuiAction::ToolCallDelta {
                tool_call_id: tool_call_id.clone(),
                text: content.clone(),
            }],
            AgentEvent::ToolCallFinish {
                tool_call_id,
                name,
                output,
            } => self
                .tool_display
                .result_actions(tool_call_id, name, output, tools),
            AgentEvent::UsageMetadata(usage) => vec![ChatTuiAction::ProviderUsageReported(
                TuiProviderUsageMetadata::new(
                    usage.input_tokens,
                    usage.output_tokens,
                    usage.total_tokens,
                ),
            )],
            AgentEvent::ContextTokenUsage(usage) => vec![ChatTuiAction::ContextUsageUpdated(
                TuiContextTokenUsage::new(usage.input_tokens, usage.context_window_tokens),
            )],
            AgentEvent::ValidationError { message } | AgentEvent::Error { message } => {
                vec![ChatTuiAction::AgentFailed {
                    message: message.clone(),
                }]
            }
            AgentEvent::Cancelled { reason } => vec![ChatTuiAction::AgentCancelled {
                reason: reason.clone(),
            }],
            AgentEvent::Finished { finish_reason } => self.finished_actions(*finish_reason),
            AgentEvent::ReasoningMetadata(_)
            | AgentEvent::ContextSummaryCreated(_)
            | AgentEvent::Internal { .. } => Vec::new(),
            _ => Vec::new(),
        }
    }

    /// Converts one app-owned command lifecycle event into TUI actions.
    pub(crate) fn adapt_command_event(&mut self, event: &CommandEvent) -> Vec<ChatTuiAction> {
        match event {
            CommandEvent::Start(start) => {
                vec![command_started_action(&start.command_id, &start.command)]
            }
            CommandEvent::Delta(delta) => {
                vec![command_output_action(&delta.command_id, &delta.content)]
            }
            CommandEvent::Finished(finished) => vec![command_finished_action(
                &finished.command_id,
                finished.status,
                &finished.summary,
            )],
        }
    }

    /// Builds a semantic user prompt action with agent-provided transcript identity.
    fn user_prompt_action(&self, id: &str, content: &str) -> ChatTuiAction {
        ChatTuiAction::SubmitPrompt {
            id: transcript_item_id(id),
            text: content.to_owned(),
        }
    }

    /// Converts a run finish into a deterministic terminal action.
    fn finished_actions(&mut self, finish_reason: FinishReason) -> Vec<ChatTuiAction> {
        let terminal_action = match finish_reason {
            FinishReason::Cancelled => ChatTuiAction::AgentCancelled {
                reason: "provider cancelled run".to_owned(),
            },
            FinishReason::Length => ChatTuiAction::AgentFailed {
                message: "provider response reached the length limit".to_owned(),
            },
            FinishReason::ToolCalls => ChatTuiAction::AgentFailed {
                message: "provider requested tool calls without completing the run".to_owned(),
            },
            FinishReason::ContentFilter | FinishReason::Error => ChatTuiAction::AgentFailed {
                message: format!("provider finished with {finish_reason:?}"),
            },
            FinishReason::Stop => ChatTuiAction::AgentFinished,
        };

        vec![terminal_action]
    }
}

/// Converts an agent transcript item identifier into the TUI identifier type.
pub(crate) fn transcript_item_id(id: &str) -> TranscriptItemId {
    TranscriptItemId::new(id)
}

/// Builds the TUI action for a controller-owned agent run start.
#[cfg(test)]
pub(crate) fn agent_started_action() -> ChatTuiAction {
    ChatTuiAction::AgentStarted
}

/// Builds the TUI action for a submitted prompt with caller-owned identity.
#[cfg(test)]
pub(crate) fn submit_prompt_action(
    id: impl Into<String>,
    text: impl Into<String>,
) -> ChatTuiAction {
    ChatTuiAction::SubmitPrompt {
        id: TranscriptItemId::new(id),
        text: text.into(),
    }
}

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
        completion_values(command, subcommand, field.name, snapshot),
        match field.validation {
            CompletionValueValidation::None => TuiCommandValueValidation::None,
            CompletionValueValidation::OneOfValues => TuiCommandValueValidation::OneOfValues,
        },
    )
}

/// Returns reducer-safe value metadata for known structured command fields.
fn completion_values(
    command: &str,
    subcommand: &str,
    field: &str,
    snapshot: &CompletionSnapshot,
) -> CompletionValues {
    match (command, subcommand, field) {
        ("provider", "add", "provider") => {
            CompletionValues::Static(snapshot.enabled_provider_type_ids.clone())
        }
        ("provider", "auth", "provider") => {
            CompletionValues::Static(vec![spectacular_llms::OPENAI_PROVIDER_ID.to_owned()])
        }
        ("provider", "remove", "name")
        | ("model", "add", "provider")
        | ("model", "edit", "provider") => snapshot.configured_provider_values(),
        ("model", "add", "id") | ("model", "edit", "id") => snapshot.cached_model_values(),
        ("model", "add", "reasoning") | ("model", "edit", "reasoning") => CompletionValues::Static(
            ReasoningLevel::ALL
                .into_iter()
                .map(|level| level.as_str().to_owned())
                .collect(),
        ),
        ("model", "add", "name") | ("model", "edit", "name") | ("model", "remove", "name") => {
            snapshot.saved_model_values()
        }
        ("task", "set", "task") => CompletionValues::Static(
            TaskModelSlot::ALL
                .into_iter()
                .map(|slot| slot.as_str().to_owned())
                .collect(),
        ),
        ("task", "set", "model") => snapshot.saved_model_values(),
        _ => CompletionValues::None,
    }
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
            enabled_provider_type_ids: model.completion_environment().enabled_provider_type_ids(),
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
        let saved_model_providers = self
            .config
            .as_ref()
            .map(|config| {
                config
                    .models
                    .iter()
                    .map(|(name, model)| (name.clone(), model.provider.clone()))
                    .collect()
            })
            .unwrap_or_default();
        let model_ids_by_provider = cache
            .providers
            .iter()
            .map(|(provider, cache)| {
                (
                    provider.clone(),
                    cache.models.keys().cloned().collect::<Vec<_>>(),
                )
            })
            .collect::<BTreeMap<_, _>>();

        CompletionValues::CachedModelIds(CachedModelValues::new(
            model_ids_by_provider,
            saved_model_providers,
        ))
    }
}

/// Builds the TUI action for switching to another session state root.
#[cfg(test)]
pub(crate) fn session_changed_action(session_id: &str) -> ChatTuiAction {
    ChatTuiAction::SessionChanged {
        id: spectacular_tui::SessionId::new(session_id),
    }
}

#[cfg(test)]
mod tests {
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/unit/chat/tui_adapter.rs"
    ));
}
