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

/// Builds provider chat messages from recorded agent events.
///
/// Streaming assistant deltas are coalesced back into turn-sized assistant
/// messages so the next provider request sees normal transcript shape.
pub fn provider_messages_from_store(
    system_prompt: impl Into<String>,
    store: &Store,
) -> Vec<ProviderMessage> {
    let mut messages = vec![ProviderMessage::system(system_prompt)];
    let mut pending_assistant = String::new();

    for event in store.events() {
        match event {
            AgentEvent::MessageDelta(delta) => pending_assistant.push_str(&delta.content),
            AgentEvent::UserPrompt { content } => {
                flush_pending_assistant(&mut messages, &mut pending_assistant);
                messages.push(ProviderMessage::user(content.clone()));
            }
            AgentEvent::AssistantToolCallRequest {
                tool_call_id,
                name,
                arguments,
            } => {
                flush_pending_assistant(&mut messages, &mut pending_assistant);
                messages.push(ProviderMessage::assistant_tool_call(ProviderToolCall::new(
                    tool_call_id.clone(),
                    name.clone(),
                    arguments.clone(),
                )));
            }
            AgentEvent::ToolResult {
                tool_call_id,
                content,
                ..
            } => {
                flush_pending_assistant(&mut messages, &mut pending_assistant);
                messages.push(ProviderMessage::tool_result(
                    tool_call_id.clone(),
                    content.clone(),
                ));
            }
            AgentEvent::ReasoningDelta(_)
            | AgentEvent::UsageMetadata(_)
            | AgentEvent::ReasoningMetadata(_)
            | AgentEvent::ValidationError { .. }
            | AgentEvent::Error { .. }
            | AgentEvent::Cancelled { .. }
            | AgentEvent::Finished { .. }
            | AgentEvent::Internal { .. } => {}
        }
    }

    flush_pending_assistant(&mut messages, &mut pending_assistant);
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

/// Flushes accumulated assistant stream text into the provider message list.
fn flush_pending_assistant(messages: &mut Vec<ProviderMessage>, pending_assistant: &mut String) {
    if pending_assistant.is_empty() {
        return;
    }

    messages.push(ProviderMessage::assistant(std::mem::take(
        pending_assistant,
    )));
}
