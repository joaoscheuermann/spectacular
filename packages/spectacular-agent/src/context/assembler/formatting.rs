use crate::context::provider_messages_from_events;
use spectacular_llms::{ProviderMessage, ProviderMessageRole};

/// Builds final provider messages from assembled sections.
pub(super) fn build_messages(
    system_prompt: String,
    summary_message: Option<ProviderMessage>,
    transcript_messages: Vec<ProviderMessage>,
    continuation_message: Option<ProviderMessage>,
) -> Vec<ProviderMessage> {
    let mut messages = provider_messages_from_events(system_prompt, &[]);
    if let Some(summary_message) = summary_message {
        messages.push(summary_message);
    }
    messages.extend(transcript_messages);
    if let Some(continuation_message) = continuation_message {
        messages.push(continuation_message);
    }

    messages
}

/// Formats the stored summary as a provider-visible session-state block.
pub(super) fn format_summary_message(summary: &str) -> String {
    format!("Compact session state:\n{summary}")
}

/// Formats compactable provider messages as transcript text for summarization.
pub(super) fn format_messages_for_summary(messages: &[ProviderMessage]) -> String {
    messages
        .iter()
        .map(format_message_for_summary)
        .collect::<Vec<_>>()
        .join("\n\n")
}

/// Formats one provider message while preserving tool-call identity.
fn format_message_for_summary(message: &ProviderMessage) -> String {
    if !message.tool_calls.is_empty() {
        let calls = message
            .tool_calls
            .iter()
            .map(|tool_call| {
                format!(
                    "Assistant tool call id={} name={}\nArguments:\n{}",
                    tool_call.id, tool_call.name, tool_call.arguments
                )
            })
            .collect::<Vec<_>>()
            .join("\n");
        return calls;
    }

    match message.role {
        ProviderMessageRole::System => format!("System:\n{}", message.content),
        ProviderMessageRole::User => format!("User:\n{}", message.content),
        ProviderMessageRole::Assistant => format!("Assistant:\n{}", message.content),
        ProviderMessageRole::Tool => format!(
            "Tool result id={}:\n{}",
            message.tool_call_id.as_deref().unwrap_or("unknown"),
            message.content
        ),
    }
}
