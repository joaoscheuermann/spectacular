use crate::event::AgentEvent;
use crate::store::Store;
use spectacular_llms::{ProviderContextLimits, ProviderMessage, ProviderToolCall};
use std::error::Error;
use std::fmt::{self, Display};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContextLimitFailure {
    reason: String,
}

impl Display for ContextLimitFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.reason)
    }
}

impl Error for ContextLimitFailure {}

pub fn provider_messages_from_store(
    system_prompt: impl Into<String>,
    store: &Store,
) -> Vec<ProviderMessage> {
    let mut messages = vec![ProviderMessage::system(system_prompt)];
    messages.extend(
        store
            .events()
            .iter()
            .filter_map(provider_message_from_event),
    );
    messages
}

pub fn validate_context_limits(
    messages: &[ProviderMessage],
    limits: ProviderContextLimits,
) -> Result<(), ContextLimitFailure> {
    if let Some(max_messages) = limits.max_messages {
        if messages.len() > max_messages {
            return Err(ContextLimitFailure {
                reason: format!("{} messages exceeds limit {max_messages}", messages.len()),
            });
        }
    }

    if let Some(max_chars) = limits.max_chars {
        let chars = messages
            .iter()
            .map(|message| message.content.chars().count())
            .sum::<usize>();
        if chars > max_chars {
            return Err(ContextLimitFailure {
                reason: format!("{chars} characters exceeds limit {max_chars}"),
            });
        }
    }

    Ok(())
}

fn provider_message_from_event(event: &AgentEvent) -> Option<ProviderMessage> {
    match event {
        AgentEvent::UserPrompt { content } => Some(ProviderMessage::user(content.clone())),
        AgentEvent::MessageDelta(delta) => Some(ProviderMessage::assistant(delta.content.clone())),
        AgentEvent::AssistantToolCallRequest {
            tool_call_id,
            name,
            arguments,
        } => Some(ProviderMessage::assistant_tool_call(ProviderToolCall::new(
            tool_call_id.clone(),
            name.clone(),
            arguments.clone(),
        ))),
        AgentEvent::ToolResult {
            tool_call_id,
            content,
            ..
        } => Some(ProviderMessage::tool_result(
            tool_call_id.clone(),
            content.clone(),
        )),
        AgentEvent::ReasoningDelta(_)
        | AgentEvent::UsageMetadata(_)
        | AgentEvent::ReasoningMetadata(_)
        | AgentEvent::ValidationError { .. }
        | AgentEvent::Error { .. }
        | AgentEvent::Cancelled { .. }
        | AgentEvent::Finished { .. }
        | AgentEvent::Internal { .. } => None,
    }
}
