use super::display::{
    command_finished_action, command_output_action, command_started_action, ToolDisplayAdapter,
};
use crate::chat::command_event::CommandEvent;
#[cfg(test)]
pub(crate) use crate::chat::tui::state::{display_metadata_action, runtime_selection_action};
use spectacular_agent::{AgentEvent, ToolStorage};
use spectacular_commands::CommandRegistry;
use spectacular_llms::FinishReason;
use spectacular_tui::{
    ChatTuiAction, CommandDescriptor, ContextTokenUsage as TuiContextTokenUsage,
    ProviderUsageMetadata as TuiProviderUsageMetadata, TranscriptItemId,
};

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
pub(crate) fn commands_loaded_action<C>(registry: &CommandRegistry<C>) -> ChatTuiAction {
    ChatTuiAction::CommandsLoaded(
        registry
            .command_metadata()
            .map(|metadata| {
                CommandDescriptor::with_usage(metadata.name, metadata.summary, metadata.usage)
            })
            .collect(),
    )
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
