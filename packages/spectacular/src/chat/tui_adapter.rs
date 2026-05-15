use crate::chat::RuntimeSelection;
use spectacular_agent::{AgentEvent, CommandStatus};
use spectacular_commands::CommandRegistry;
use spectacular_llms::{FinishReason, ProviderMessageRole, UsageMetadata};
use spectacular_tui::{
    ChatTuiAction, CommandDescriptor, ContextTokenUsage as TuiContextTokenUsage,
    DisplayMetadata as TuiDisplayMetadata, ReasoningLevel as TuiReasoningLevel,
    RuntimeSelection as TuiRuntimeSelection, SessionId, TranscriptItemId,
};
use std::collections::BTreeMap;
use std::path::Path;

/// Converts runtime and agent events into pure TUI reducer actions.
#[derive(Default)]
pub(crate) struct TuiEventAdapter {
    active_message: Option<TranscriptItemId>,
    active_reasoning: Option<TranscriptItemId>,
    next_message_id: u64,
    next_reasoning_id: u64,
    next_tool_id: u64,
    next_user_prompt_id: u64,
    tool_transcript_ids: BTreeMap<String, TranscriptItemId>,
}

impl TuiEventAdapter {
    /// Creates an adapter with empty lifecycle state for one runtime event stream.
    pub(crate) fn new() -> Self {
        Self {
            next_message_id: 1,
            next_reasoning_id: 1,
            next_tool_id: 1,
            next_user_prompt_id: 1,
            ..Self::default()
        }
    }

    /// Converts one agent event into zero or more TUI actions without rendering terminal output.
    pub(crate) fn adapt_agent_event(&mut self, event: &AgentEvent) -> Vec<ChatTuiAction> {
        match event {
            AgentEvent::UserPrompt { content } => vec![self.user_prompt_action(content)],
            AgentEvent::MessageDelta(delta) if delta.role == ProviderMessageRole::Assistant => {
                self.message_delta_actions(&delta.content)
            }
            AgentEvent::ReasoningDelta(delta) => self.reasoning_delta_actions(&delta.content),
            AgentEvent::AssistantToolCallRequest {
                tool_call_id,
                name,
                arguments,
            } => self.tool_call_started_actions(tool_call_id, name, arguments),
            AgentEvent::ToolResult {
                tool_call_id,
                name,
                content,
            } => self.tool_result_actions(tool_call_id, name, content),
            AgentEvent::CommandStart(start) => vec![ChatTuiAction::CommandStarted {
                id: TranscriptItemId::new(format!("command-{}", start.command_id)),
                command_id: start.command_id.clone(),
                command: start.command.clone(),
            }],
            AgentEvent::CommandDelta(delta) => vec![ChatTuiAction::CommandOutput {
                command_id: delta.command_id.clone(),
                text: delta.content.clone(),
            }],
            AgentEvent::CommandFinished(finished) => vec![ChatTuiAction::CommandFinished {
                command_id: finished.command_id.clone(),
                exit_code: command_exit_code(finished.status),
            }],
            AgentEvent::UsageMetadata(usage) => {
                usage_action_from_metadata(usage).into_iter().collect()
            }
            AgentEvent::ContextTokenUsage(usage) => vec![ChatTuiAction::UsageUpdated(
                TuiContextTokenUsage::new(usage.input_tokens, usage.context_window_tokens),
            )],
            AgentEvent::ValidationError { message } | AgentEvent::Error { message } => self
                .finish_streaming_lifecycles(ChatTuiAction::AgentFailed {
                    message: message.clone(),
                }),
            AgentEvent::Cancelled { reason } => {
                self.finish_streaming_lifecycles(ChatTuiAction::AgentCancelled {
                    reason: reason.clone(),
                })
            }
            AgentEvent::Finished { finish_reason } => self.finished_actions(*finish_reason),
            AgentEvent::MessageDelta(_)
            | AgentEvent::ReasoningMetadata(_)
            | AgentEvent::ContextSummaryCreated(_)
            | AgentEvent::Internal { .. } => Vec::new(),
            _ => Vec::new(),
        }
    }

    /// Builds lifecycle actions for an implicit assistant delta stream.
    fn message_delta_actions(&mut self, content: &str) -> Vec<ChatTuiAction> {
        let (id, started) = self.active_message_id();
        let delta = ChatTuiAction::MessageDelta {
            id: id.clone(),
            text: content.to_owned(),
        };
        if !started {
            return vec![delta];
        }

        vec![ChatTuiAction::MessageStarted { id }, delta]
    }

    /// Builds lifecycle actions for an implicit reasoning delta stream.
    fn reasoning_delta_actions(&mut self, content: &str) -> Vec<ChatTuiAction> {
        let (id, started) = self.active_reasoning_id();
        let delta = ChatTuiAction::ReasoningDelta {
            id: id.clone(),
            text: content.to_owned(),
        };
        if !started {
            return vec![delta];
        }

        vec![ChatTuiAction::ReasoningStarted { id }, delta]
    }

    /// Returns the active assistant message ID and whether it was just created.
    fn active_message_id(&mut self) -> (TranscriptItemId, bool) {
        if let Some(id) = &self.active_message {
            return (id.clone(), false);
        }

        let id = TranscriptItemId::new(format!("message-{}", self.next_message_id));
        self.next_message_id = self.next_message_id.saturating_add(1);
        self.active_message = Some(id.clone());
        (id, true)
    }

    /// Returns the active reasoning item ID and whether it was just created.
    fn active_reasoning_id(&mut self) -> (TranscriptItemId, bool) {
        if let Some(id) = &self.active_reasoning {
            return (id.clone(), false);
        }

        let id = TranscriptItemId::new(format!("reasoning-{}", self.next_reasoning_id));
        self.next_reasoning_id = self.next_reasoning_id.saturating_add(1);
        self.active_reasoning = Some(id.clone());
        (id, true)
    }

    /// Builds semantic tool-call lifecycle actions with adapter-owned transcript identity.
    fn tool_call_started_actions(
        &mut self,
        tool_call_id: &str,
        name: &str,
        arguments: &str,
    ) -> Vec<ChatTuiAction> {
        let id = TranscriptItemId::new(format!("tool-call-{}", self.next_tool_id));
        self.next_tool_id = self.next_tool_id.saturating_add(1);
        self.tool_transcript_ids
            .insert(tool_call_id.to_owned(), id.clone());
        vec![ChatTuiAction::ToolCallStarted {
            id,
            tool_call_id: tool_call_id.to_owned(),
            name: name.to_owned(),
            arguments: arguments.to_owned(),
        }]
    }

    /// Builds semantic tool-call completion actions for known and implicit tool starts.
    fn tool_result_actions(
        &mut self,
        tool_call_id: &str,
        name: &str,
        content: &str,
    ) -> Vec<ChatTuiAction> {
        if self.tool_transcript_ids.remove(tool_call_id).is_some() {
            return vec![ChatTuiAction::ToolCallFinished {
                tool_call_id: tool_call_id.to_owned(),
                name: name.to_owned(),
                output: content.to_owned(),
            }];
        }

        let mut actions = self.tool_call_started_actions(tool_call_id, name, "");
        actions.push(ChatTuiAction::ToolCallFinished {
            tool_call_id: tool_call_id.to_owned(),
            name: name.to_owned(),
            output: content.to_owned(),
        });
        actions
    }

    /// Builds a semantic user prompt action with adapter-owned transcript identity.
    fn user_prompt_action(&mut self, content: &str) -> ChatTuiAction {
        let id = TranscriptItemId::new(format!("user-prompt-{}", self.next_user_prompt_id));
        self.next_user_prompt_id = self.next_user_prompt_id.saturating_add(1);
        ChatTuiAction::SubmitPrompt {
            id,
            text: content.to_owned(),
        }
    }

    /// Finishes any active implicit streams before appending a terminal run action.
    fn finish_streaming_lifecycles(
        &mut self,
        terminal_action: ChatTuiAction,
    ) -> Vec<ChatTuiAction> {
        let mut actions = self.finish_active_streams();
        actions.push(terminal_action);
        actions
    }

    /// Converts a run finish into lifecycle completion plus a deterministic terminal action.
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

        self.finish_streaming_lifecycles(terminal_action)
    }

    /// Drains active implicit assistant and reasoning lifecycle finish actions.
    fn finish_active_streams(&mut self) -> Vec<ChatTuiAction> {
        let mut actions = Vec::new();
        if let Some(id) = self.active_message.take() {
            actions.push(ChatTuiAction::MessageFinished { id });
        }
        if let Some(id) = self.active_reasoning.take() {
            actions.push(ChatTuiAction::ReasoningFinished { id });
        }

        actions
    }
}

/// Builds the TUI action for a controller-owned agent run start.
pub(crate) fn agent_started_action() -> ChatTuiAction {
    ChatTuiAction::AgentStarted
}

/// Builds the TUI action for a submitted prompt with caller-owned identity.
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

/// Projects runtime selection into the UI-safe TUI runtime model.
pub(crate) fn runtime_selection_action(runtime: &RuntimeSelection) -> ChatTuiAction {
    ChatTuiAction::RuntimeSelectionChanged(TuiRuntimeSelection::new(
        runtime.provider_type.clone(),
        runtime.provider.clone(),
        runtime.model.clone(),
        tui_reasoning_level(runtime.reasoning),
        runtime.context_window_tokens.map(|value| value as u64),
    ))
}

/// Projects header and footer metadata into UI-safe display state.
pub(crate) fn display_metadata_action(
    session_id: &str,
    runtime: &RuntimeSelection,
    current_directory: &Path,
    usage: Option<spectacular_agent::ContextTokenUsage>,
) -> ChatTuiAction {
    ChatTuiAction::DisplayMetadataChanged(TuiDisplayMetadata::new(
        runtime.provider.clone(),
        runtime.model.clone(),
        runtime.reasoning.to_string(),
        current_directory.to_string_lossy(),
        session_id,
        usage.map(tui_context_usage),
    ))
}

/// Builds the TUI action for switching to another session state root.
pub(crate) fn session_changed_action(session_id: &str) -> ChatTuiAction {
    ChatTuiAction::SessionChanged {
        id: SessionId::new(session_id),
    }
}

/// Converts provider usage metadata into a compact TUI usage action when token data exists.
fn usage_action_from_metadata(usage: &UsageMetadata) -> Option<ChatTuiAction> {
    let tokens = usage.total_tokens.or_else(|| {
        Some(
            usage
                .input_tokens?
                .saturating_add(usage.output_tokens.unwrap_or_default()),
        )
    })?;

    Some(ChatTuiAction::UsageUpdated(TuiContextTokenUsage::new(
        tokens, None,
    )))
}

/// Converts runtime context token usage into TUI token usage metadata.
fn tui_context_usage(usage: spectacular_agent::ContextTokenUsage) -> TuiContextTokenUsage {
    TuiContextTokenUsage::new(usage.input_tokens, usage.context_window_tokens)
}

/// Maps Spectacular runtime reasoning levels into the TUI display subset.
fn tui_reasoning_level(reasoning: spectacular_config::ReasoningLevel) -> TuiReasoningLevel {
    match reasoning {
        spectacular_config::ReasoningLevel::None => TuiReasoningLevel::None,
        spectacular_config::ReasoningLevel::Minimal | spectacular_config::ReasoningLevel::Low => {
            TuiReasoningLevel::Low
        }
        spectacular_config::ReasoningLevel::Medium => TuiReasoningLevel::Medium,
        spectacular_config::ReasoningLevel::High | spectacular_config::ReasoningLevel::Xhigh => {
            TuiReasoningLevel::High
        }
    }
}

/// Converts command lifecycle status into the reducer's exit-code based command completion.
fn command_exit_code(status: CommandStatus) -> Option<i32> {
    match status {
        CommandStatus::Success => Some(0),
        CommandStatus::Failed
        | CommandStatus::Cancelled
        | CommandStatus::TimedOut
        | CommandStatus::Error => Some(1),
    }
}

#[cfg(test)]
mod tests {
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/unit/chat/tui_adapter.rs"
    ));
}
