use super::ids::transcript_item_id;
use crate::chat::command_event::CommandEvent;
use crate::chat::tui::display::{
    command_finished_action, command_output_action, command_started_action, ToolDisplayAdapter,
};
use ::agent::{AgentEvent, ToolStorage};
use ::llms::FinishReason;
use ::tui::{
    ChatTuiAction, ContextTokenUsage as TuiContextTokenUsage,
    ProviderUsageMetadata as TuiProviderUsageMetadata,
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
            AgentEvent::UserPrompt { .. }
            | AgentEvent::MessageStart { .. }
            | AgentEvent::MessageDelta { .. }
            | AgentEvent::MessageFinish { .. }
            | AgentEvent::ReasoningStart { .. }
            | AgentEvent::ReasoningDelta { .. }
            | AgentEvent::ReasoningFinish { .. } => self.transcript_actions(event),
            AgentEvent::ToolCallStart { .. }
            | AgentEvent::ToolCallDelta { .. }
            | AgentEvent::ToolCallFinish { .. } => self.tool_actions(event, tools),
            AgentEvent::UsageMetadata(_) | AgentEvent::ContextTokenUsage(_) => {
                one(metadata_action(event))
            }
            AgentEvent::ValidationError { .. }
            | AgentEvent::Error { .. }
            | AgentEvent::Cancelled { .. }
            | AgentEvent::Finished { .. } => one(terminal_action(event)),
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

    fn transcript_actions(&self, event: &AgentEvent) -> Vec<ChatTuiAction> {
        match event {
            AgentEvent::UserPrompt {
                id: Some(id),
                content,
            } => one(self.user_prompt_action(id.as_str(), content)),
            AgentEvent::UserPrompt { id: None, .. } => Vec::new(),
            AgentEvent::MessageStart { .. }
            | AgentEvent::MessageDelta { .. }
            | AgentEvent::MessageFinish { .. } => one(message_action(event)),
            AgentEvent::ReasoningStart { .. }
            | AgentEvent::ReasoningDelta { .. }
            | AgentEvent::ReasoningFinish { .. } => one(reasoning_action(event)),
            _ => unreachable!("transcript_actions only handles transcript agent events"),
        }
    }

    fn tool_actions(&mut self, event: &AgentEvent, tools: &ToolStorage) -> Vec<ChatTuiAction> {
        match event {
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
            } => one(ChatTuiAction::ToolCallDelta {
                tool_call_id: tool_call_id.clone(),
                text: content.clone(),
            }),
            AgentEvent::ToolCallFinish {
                tool_call_id,
                name,
                output,
            } => self
                .tool_display
                .result_actions(tool_call_id, name, output, tools),
            _ => unreachable!("tool_actions only handles tool agent events"),
        }
    }

    /// Builds a semantic user prompt action with agent-provided transcript identity.
    fn user_prompt_action(&self, id: &str, content: &str) -> ChatTuiAction {
        ChatTuiAction::SubmitPrompt {
            id: transcript_item_id(id),
            text: content.to_owned(),
        }
    }
}

fn one(action: ChatTuiAction) -> Vec<ChatTuiAction> {
    vec![action]
}

fn message_action(event: &AgentEvent) -> ChatTuiAction {
    match event {
        AgentEvent::MessageStart { id } => ChatTuiAction::MessageStarted {
            id: transcript_item_id(id.as_str()),
        },
        AgentEvent::MessageDelta { id, content } => ChatTuiAction::MessageDelta {
            id: transcript_item_id(id.as_str()),
            text: content.clone(),
        },
        AgentEvent::MessageFinish { id } => ChatTuiAction::MessageFinished {
            id: transcript_item_id(id.as_str()),
        },
        _ => unreachable!("message_action only handles message agent events"),
    }
}

fn reasoning_action(event: &AgentEvent) -> ChatTuiAction {
    match event {
        AgentEvent::ReasoningStart { id } => ChatTuiAction::ReasoningStarted {
            id: transcript_item_id(id.as_str()),
        },
        AgentEvent::ReasoningDelta { id, content } => ChatTuiAction::ReasoningDelta {
            id: transcript_item_id(id.as_str()),
            text: content.clone(),
        },
        AgentEvent::ReasoningFinish { id } => ChatTuiAction::ReasoningFinished {
            id: transcript_item_id(id.as_str()),
        },
        _ => unreachable!("reasoning_action only handles reasoning agent events"),
    }
}

fn metadata_action(event: &AgentEvent) -> ChatTuiAction {
    match event {
        AgentEvent::UsageMetadata(usage) => {
            ChatTuiAction::ProviderUsageReported(TuiProviderUsageMetadata::new(
                usage.input_tokens,
                usage.output_tokens,
                usage.total_tokens,
            ))
        }
        AgentEvent::ContextTokenUsage(usage) => ChatTuiAction::ContextUsageUpdated(
            TuiContextTokenUsage::new(usage.input_tokens, usage.context_window_tokens),
        ),
        _ => unreachable!("metadata_action only handles metadata agent events"),
    }
}

fn agent_failed_action(message: &str, details: Option<String>) -> ChatTuiAction {
    ChatTuiAction::AgentFailed {
        message: message.to_owned(),
        details,
    }
}

fn terminal_action(event: &AgentEvent) -> ChatTuiAction {
    match event {
        AgentEvent::ValidationError { message } => agent_failed_action(message, None),
        AgentEvent::Error { message, details } => {
            agent_failed_action(message, details.as_ref().map(ToString::to_string))
        }
        AgentEvent::Cancelled { reason } => ChatTuiAction::AgentCancelled {
            reason: reason.clone(),
        },
        AgentEvent::Finished { finish_reason } => finished_action(*finish_reason),
        _ => unreachable!("terminal_action only handles terminal agent events"),
    }
}

fn finished_action(finish_reason: FinishReason) -> ChatTuiAction {
    match finish_reason {
        FinishReason::Cancelled => ChatTuiAction::AgentCancelled {
            reason: "provider cancelled run".to_owned(),
        },
        FinishReason::Length => {
            agent_failed_action("provider response reached the length limit", None)
        }
        FinishReason::ToolCalls => agent_failed_action(
            "provider requested tool calls without completing the run",
            None,
        ),
        FinishReason::ContentFilter | FinishReason::Error => {
            agent_failed_action(&format!("provider finished with {finish_reason:?}"), None)
        }
        FinishReason::Stop => ChatTuiAction::AgentFinished,
    }
}
