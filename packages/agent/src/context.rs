use crate::event::AgentEvent;
use crate::store::Store;
use llms::{ProviderContextLimits, ProviderMessage, ProviderToolCall};
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
    let mut builder = TranscriptBuilder::new(messages);
    for event in events {
        builder.append(event);
    }
    builder.finish();
}

/// Validates provider message counts and character totals against provider limits.
pub fn validate_context_limits(
    messages: &[ProviderMessage],
    limits: ProviderContextLimits,
) -> Result<(), ContextLimitFailure> {
    validate_message_count(messages, limits.max_messages)?;
    validate_character_count(messages, limits.max_chars)?;
    Ok(())
}

struct TranscriptBuilder<'a> {
    messages: &'a mut Vec<ProviderMessage>,
    pending_assistant: String,
}

impl<'a> TranscriptBuilder<'a> {
    fn new(messages: &'a mut Vec<ProviderMessage>) -> Self {
        Self {
            messages,
            pending_assistant: String::new(),
        }
    }

    fn append(&mut self, event: &AgentEvent) {
        match event {
            AgentEvent::MessageDelta { content, .. } => self.pending_assistant.push_str(content),
            AgentEvent::MessageFinish { .. } => self.flush_pending_assistant(),
            AgentEvent::UserPrompt { content, .. } => self.push_user_prompt(content),
            AgentEvent::ToolCallStart {
                tool_call_id,
                name,
                arguments,
            } => self.push_tool_call(tool_call_id, name, arguments),
            AgentEvent::ToolCallFinish {
                tool_call_id,
                output,
                ..
            } => self.push_tool_result(tool_call_id, output),
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

    fn finish(&mut self) {
        self.flush_pending_assistant();
    }

    fn push_user_prompt(&mut self, content: &str) {
        self.flush_pending_assistant();
        self.messages
            .push(ProviderMessage::user(content.to_owned()));
    }

    fn push_tool_call(&mut self, tool_call_id: &str, name: &str, arguments: &str) {
        self.flush_pending_assistant();
        self.messages
            .push(ProviderMessage::assistant_tool_call(ProviderToolCall::new(
                tool_call_id.to_owned(),
                name.to_owned(),
                arguments.to_owned(),
            )));
    }

    fn push_tool_result(&mut self, tool_call_id: &str, output: &str) {
        self.flush_pending_assistant();
        self.messages.push(ProviderMessage::tool_result(
            tool_call_id.to_owned(),
            output.to_owned(),
        ));
    }

    fn flush_pending_assistant(&mut self) {
        if self.pending_assistant.is_empty() {
            return;
        }

        self.messages
            .push(ProviderMessage::assistant(std::mem::take(
                &mut self.pending_assistant,
            )));
    }
}

fn validate_message_count(
    messages: &[ProviderMessage],
    max_messages: Option<usize>,
) -> Result<(), ContextLimitFailure> {
    let Some(max_messages) = max_messages else {
        return Ok(());
    };
    if messages.len() <= max_messages {
        return Ok(());
    }

    Err(ContextLimitFailure {
        reason: format!("{} messages exceeds limit {max_messages}", messages.len()),
    })
}

fn validate_character_count(
    messages: &[ProviderMessage],
    max_chars: Option<usize>,
) -> Result<(), ContextLimitFailure> {
    let Some(max_chars) = max_chars else {
        return Ok(());
    };

    let chars = messages
        .iter()
        .map(|message| message.content.chars().count())
        .sum::<usize>();
    if chars <= max_chars {
        return Ok(());
    }

    Err(ContextLimitFailure {
        reason: format!("{chars} characters exceeds limit {max_chars}"),
    })
}

#[cfg(test)]
mod tests {
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/unit/context.rs"
    ));
}
