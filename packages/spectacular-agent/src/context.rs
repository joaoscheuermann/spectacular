use crate::event::AgentEvent;
use crate::store::Store;
use spectacular_llms::{ProviderContextLimits, ProviderMessage, ProviderToolCall};
use std::error::Error;
use std::fmt::{self, Display};

pub(crate) mod assembler;
pub(crate) mod diagnostics;
pub(crate) use diagnostics::ContextDiagnostics;
mod policy;
pub(crate) mod token_count;

pub(crate) use assembler::{
    ContextAssembler, ContextAssembly, ContextAssemblyError, ContextAssemblyInput,
    ContextSummaryRequest,
};
pub use policy::ContextPolicy;
pub(crate) use token_count::TokenCounterChoice;
pub use token_count::{TiktokenTokenCounter, TokenCounter};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContextLimitFailure {
    reason: String,
}

impl Display for ContextLimitFailure {
    /// Formats the context limit failure reason for logs and agent errors.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.reason)
    }
}

impl Error for ContextLimitFailure {}

/// Builds provider chat messages from recorded agent events.
///
/// Explicit assistant lifecycle deltas are coalesced back into turn-sized
/// assistant messages so the next provider request sees normal transcript shape.
pub fn provider_messages_from_store(
    system_prompt: impl Into<String>,
    store: &Store,
) -> Vec<ProviderMessage> {
    provider_messages_from_events(system_prompt, store.events())
}

/// Builds provider messages from an event slice and prepends one system message.
pub(crate) fn provider_messages_from_events(
    system_prompt: impl Into<String>,
    events: &[AgentEvent],
) -> Vec<ProviderMessage> {
    let mut messages = vec![ProviderMessage::system(system_prompt)];
    append_transcript_messages(&mut messages, events);
    messages
}

/// Builds provider messages from transcript events without adding a system prompt.
pub(crate) fn transcript_messages_from_events(events: &[AgentEvent]) -> Vec<ProviderMessage> {
    let mut messages = Vec::new();
    append_transcript_messages(&mut messages, events);
    messages
}

/// Appends provider-visible transcript messages from recorded agent events.
fn append_transcript_messages(messages: &mut Vec<ProviderMessage>, events: &[AgentEvent]) {
    let mut pending_assistant = String::new();

    for event in events {
        match event {
            AgentEvent::MessageDelta { content, .. } => pending_assistant.push_str(content),
            AgentEvent::MessageFinish { .. } => {
                flush_pending_assistant(messages, &mut pending_assistant);
            }
            AgentEvent::UserPrompt { content, .. } => {
                flush_pending_assistant(messages, &mut pending_assistant);
                messages.push(ProviderMessage::user(content.clone()));
            }
            AgentEvent::ToolCallStart {
                tool_call_id,
                name,
                arguments,
            } => {
                flush_pending_assistant(messages, &mut pending_assistant);
                messages.push(ProviderMessage::assistant_tool_call(ProviderToolCall::new(
                    tool_call_id.clone(),
                    name.clone(),
                    arguments.clone(),
                )));
            }
            AgentEvent::ToolCallFinish {
                tool_call_id,
                output,
                ..
            } => {
                flush_pending_assistant(messages, &mut pending_assistant);
                messages.push(ProviderMessage::tool_result(
                    tool_call_id.clone(),
                    output.clone(),
                ));
            }
            AgentEvent::MessageStart { .. }
            | AgentEvent::ReasoningStart { .. }
            | AgentEvent::ReasoningDelta { .. }
            | AgentEvent::ReasoningFinish { .. }
            | AgentEvent::UsageMetadata(_)
            | AgentEvent::ContextTokenUsage(_)
            | AgentEvent::ReasoningMetadata(_)
            | AgentEvent::ToolCallDelta { .. }
            | AgentEvent::ValidationError { .. }
            | AgentEvent::Error { .. }
            | AgentEvent::Cancelled { .. }
            | AgentEvent::Finished { .. }
            | AgentEvent::ContextSummaryCreated(_)
            | AgentEvent::Internal { .. } => {}
        }
    }

    flush_pending_assistant(messages, &mut pending_assistant);
}

/// Validates provider message counts and character totals against provider limits.
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

#[cfg(test)]
mod tests {
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/unit/context.rs"
    ));
}
