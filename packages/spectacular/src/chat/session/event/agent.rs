use super::{
    wire::{finish_reason_from_str, finish_reason_to_str, role},
    ChatEvent,
};
use spectacular_agent::{AgentEvent, AgentTranscriptItemId, ContextSummary};
use spectacular_llms::ProviderMessageRole;

impl ChatEvent {
    /// Converts an agent event into a persisted chat event when it is session-visible.
    pub fn from_agent_event(event: &AgentEvent, created_at: String) -> Option<Self> {
        from_prompt_event(event, &created_at)
            .or_else(|| from_message_event(event, &created_at))
            .or_else(|| from_reasoning_event(event, &created_at))
            .or_else(|| from_tool_event(event, &created_at))
            .or_else(|| from_status_event(event, &created_at))
            .or_else(|| from_context_event(event, &created_at))
    }

    /// Converts a persisted chat event back into an agent event when replayable.
    pub fn to_agent_event(&self) -> Option<AgentEvent> {
        to_prompt_event(self)
            .or_else(|| to_message_event(self))
            .or_else(|| to_reasoning_event(self))
            .or_else(|| to_tool_event(self))
            .or_else(|| to_status_event(self))
            .or_else(|| to_context_event(self))
    }
}

fn from_prompt_event(event: &AgentEvent, created_at: &str) -> Option<ChatEvent> {
    match event {
        AgentEvent::UserPrompt { id, content } => Some(ChatEvent::UserPrompt {
            id: id.as_ref().map(|id| id.as_str().to_owned()),
            content: content.clone(),
            created_at: created_at.to_owned(),
        }),
        _ => None,
    }
}

fn from_message_event(event: &AgentEvent, created_at: &str) -> Option<ChatEvent> {
    match event {
        AgentEvent::MessageStart { id } => Some(ChatEvent::MessageStart {
            id: id.as_str().to_owned(),
            created_at: created_at.to_owned(),
        }),
        AgentEvent::MessageDelta { id, content } => Some(ChatEvent::AssistantDelta {
            role: role(ProviderMessageRole::Assistant).to_owned(),
            id: id.as_str().to_owned(),
            content: content.clone(),
            created_at: created_at.to_owned(),
        }),
        AgentEvent::MessageFinish { id } => Some(ChatEvent::MessageFinish {
            id: id.as_str().to_owned(),
            created_at: created_at.to_owned(),
        }),
        _ => None,
    }
}

fn from_reasoning_event(event: &AgentEvent, created_at: &str) -> Option<ChatEvent> {
    match event {
        AgentEvent::ReasoningStart { id } => Some(ChatEvent::ReasoningStart {
            id: id.as_str().to_owned(),
            created_at: created_at.to_owned(),
        }),
        AgentEvent::ReasoningDelta { id, content } => Some(ChatEvent::ReasoningDelta {
            id: id.as_str().to_owned(),
            content: content.clone(),
            created_at: created_at.to_owned(),
        }),
        AgentEvent::ReasoningFinish { id } => Some(ChatEvent::ReasoningFinish {
            id: id.as_str().to_owned(),
            created_at: created_at.to_owned(),
        }),
        _ => None,
    }
}

fn from_tool_event(event: &AgentEvent, created_at: &str) -> Option<ChatEvent> {
    match event {
        AgentEvent::ToolCallStart {
            tool_call_id,
            name,
            arguments,
        } => Some(ChatEvent::ToolCall {
            tool_call_id: tool_call_id.clone(),
            name: name.clone(),
            arguments: arguments.clone(),
            created_at: created_at.to_owned(),
        }),
        AgentEvent::ToolCallFinish {
            tool_call_id,
            name,
            output,
        } => Some(ChatEvent::ToolResult {
            tool_call_id: tool_call_id.clone(),
            name: name.clone(),
            content: output.clone(),
            created_at: created_at.to_owned(),
        }),
        _ => None,
    }
}

fn from_status_event(event: &AgentEvent, created_at: &str) -> Option<ChatEvent> {
    match event {
        AgentEvent::UsageMetadata(usage) => Some(ChatEvent::UsageMetadata {
            input_tokens: usage.input_tokens,
            output_tokens: usage.output_tokens,
            total_tokens: usage.total_tokens,
            created_at: created_at.to_owned(),
        }),
        AgentEvent::ValidationError { message } => Some(ChatEvent::ValidationError {
            message: message.clone(),
            created_at: created_at.to_owned(),
        }),
        AgentEvent::Error { message, details } => Some(ChatEvent::Error {
            message: message.clone(),
            details: details.clone(),
            created_at: created_at.to_owned(),
        }),
        AgentEvent::Cancelled { reason } => Some(ChatEvent::Cancelled {
            reason: reason.clone(),
            created_at: created_at.to_owned(),
        }),
        AgentEvent::Finished { finish_reason } => Some(ChatEvent::Finished {
            reason: finish_reason_to_str(*finish_reason).to_owned(),
            created_at: created_at.to_owned(),
        }),
        _ => None,
    }
}

fn from_context_event(event: &AgentEvent, created_at: &str) -> Option<ChatEvent> {
    match event {
        AgentEvent::ContextSummaryCreated(summary) => Some(ChatEvent::ContextSummary {
            id: summary.id.clone(),
            replaces: summary.replaces.clone(),
            source_event_start: summary.source_event_start,
            source_event_end: summary.source_event_end,
            content: summary.content.clone(),
            estimated_tokens: summary.estimated_tokens,
            created_at: created_at.to_owned(),
        }),
        _ => None,
    }
}

fn to_prompt_event(event: &ChatEvent) -> Option<AgentEvent> {
    match event {
        ChatEvent::UserPrompt { id, content, .. } => Some(AgentEvent::UserPrompt {
            id: id.as_ref().map(AgentTranscriptItemId::new),
            content: content.clone(),
        }),
        _ => None,
    }
}

fn to_message_event(event: &ChatEvent) -> Option<AgentEvent> {
    match event {
        ChatEvent::MessageStart { id, .. } => Some(AgentEvent::message_start(id.clone())),
        ChatEvent::AssistantDelta { id, content, .. } => {
            Some(AgentEvent::message_delta(id.clone(), content.clone()))
        }
        ChatEvent::MessageFinish { id, .. } => Some(AgentEvent::message_finish(id.clone())),
        _ => None,
    }
}

fn to_reasoning_event(event: &ChatEvent) -> Option<AgentEvent> {
    match event {
        ChatEvent::ReasoningStart { id, .. } => Some(AgentEvent::reasoning_start(id.clone())),
        ChatEvent::ReasoningDelta { id, content, .. } => {
            Some(AgentEvent::reasoning_delta(id.clone(), content.clone()))
        }
        ChatEvent::ReasoningFinish { id, .. } => Some(AgentEvent::reasoning_finish(id.clone())),
        _ => None,
    }
}

fn to_tool_event(event: &ChatEvent) -> Option<AgentEvent> {
    match event {
        ChatEvent::ToolCall {
            tool_call_id,
            name,
            arguments,
            ..
        } => Some(AgentEvent::tool_call_start(
            tool_call_id.clone(),
            name.clone(),
            arguments.clone(),
        )),
        ChatEvent::ToolResult {
            tool_call_id,
            name,
            content,
            ..
        } => Some(AgentEvent::tool_call_finish(
            tool_call_id.clone(),
            name.clone(),
            content.clone(),
        )),
        _ => None,
    }
}

fn to_status_event(event: &ChatEvent) -> Option<AgentEvent> {
    match event {
        ChatEvent::ValidationError { message, .. } => {
            Some(AgentEvent::validation_error(message.clone()))
        }
        ChatEvent::Error {
            message, details, ..
        } => Some(match details.clone() {
            Some(details) => AgentEvent::error_with_details(message.clone(), details),
            None => AgentEvent::error(message.clone()),
        }),
        ChatEvent::Cancelled { reason, .. } => Some(AgentEvent::cancelled(reason.clone())),
        ChatEvent::Finished { reason, .. } => Some(AgentEvent::Finished {
            finish_reason: finish_reason_from_str(reason),
        }),
        _ => None,
    }
}

fn to_context_event(event: &ChatEvent) -> Option<AgentEvent> {
    match event {
        ChatEvent::ContextSummary {
            id,
            replaces,
            source_event_start,
            source_event_end,
            content,
            estimated_tokens,
            ..
        } => Some(AgentEvent::ContextSummaryCreated(ContextSummary {
            id: id.clone(),
            replaces: replaces.clone(),
            source_event_start: *source_event_start,
            source_event_end: *source_event_end,
            content: content.clone(),
            estimated_tokens: *estimated_tokens,
        })),
        _ => None,
    }
}
