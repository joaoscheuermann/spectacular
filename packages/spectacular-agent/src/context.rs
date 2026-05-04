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

#[cfg(test)]
mod tests {
    use super::*;
    use spectacular_llms::{
        FinishReason, MessageDelta, ProviderMessageRole, ReasoningDelta, ReasoningMetadata,
        UsageMetadata,
    };

    #[test]
    fn provider_context_includes_only_model_relevant_roles() {
        let mut store = Store::default();
        store.append(AgentEvent::user_prompt("user prompt"));
        store.append(AgentEvent::MessageDelta(MessageDelta::assistant(
            "assistant response",
        )));
        store.append(AgentEvent::assistant_tool_call_request(
            "call-1", "read", "{}",
        ));
        store.append(AgentEvent::tool_result("call-1", "read", r#"{"ok":true}"#));
        store.append(AgentEvent::ReasoningDelta(ReasoningDelta {
            content: "private thought".to_owned(),
            metadata: Some(ReasoningMetadata::default()),
        }));
        store.append(AgentEvent::UsageMetadata(UsageMetadata {
            input_tokens: Some(1),
            output_tokens: Some(2),
            total_tokens: Some(3),
        }));
        store.append(AgentEvent::ReasoningMetadata(ReasoningMetadata {
            effort: Some("low".to_owned()),
            summary: Some("private summary".to_owned()),
        }));
        store.append(AgentEvent::validation_error("invalid json"));
        store.append(AgentEvent::error("provider failed"));
        store.append(AgentEvent::cancelled("cancelled"));
        store.append(AgentEvent::Finished {
            finish_reason: FinishReason::Stop,
        });
        store.append(AgentEvent::internal("queue bookkeeping"));

        let messages = provider_messages_from_store("system prompt", &store);

        assert_eq!(
            messages
                .iter()
                .map(|message| (message.role, message.content.as_str()))
                .collect::<Vec<_>>(),
            vec![
                (ProviderMessageRole::System, "system prompt"),
                (ProviderMessageRole::User, "user prompt"),
                (ProviderMessageRole::Assistant, "assistant response"),
                (ProviderMessageRole::Assistant, ""),
                (ProviderMessageRole::Tool, r#"{"ok":true}"#),
            ]
        );
        assert_eq!(messages[3].tool_calls[0].id, "call-1");
        assert_eq!(messages[3].tool_calls[0].name, "read");
        assert_eq!(messages[3].tool_calls[0].arguments, "{}");
        assert_eq!(messages[4].tool_call_id.as_deref(), Some("call-1"));
    }

    #[test]
    fn context_limits_allow_messages_within_bounds() {
        let messages = vec![
            ProviderMessage::system("system"),
            ProviderMessage::user("short"),
        ];

        assert!(validate_context_limits(
            &messages,
            ProviderContextLimits {
                max_messages: Some(2),
                max_chars: Some(11),
            },
        )
        .is_ok());
    }

    #[test]
    fn context_limits_reject_too_many_messages() {
        let messages = vec![
            ProviderMessage::system("system"),
            ProviderMessage::user("user"),
        ];

        let error = validate_context_limits(
            &messages,
            ProviderContextLimits {
                max_messages: Some(1),
                max_chars: None,
            },
        )
        .unwrap_err();

        assert_eq!(error.to_string(), "2 messages exceeds limit 1");
    }

    #[test]
    fn context_limits_reject_too_many_chars() {
        let messages = vec![
            ProviderMessage::system("system"),
            ProviderMessage::user("user"),
        ];

        let error = validate_context_limits(
            &messages,
            ProviderContextLimits {
                max_messages: None,
                max_chars: Some(5),
            },
        )
        .unwrap_err();

        assert_eq!(error.to_string(), "10 characters exceeds limit 5");
    }
}
