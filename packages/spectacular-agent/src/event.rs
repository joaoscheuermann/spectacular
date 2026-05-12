use spectacular_llms::{
    FinishReason, MessageDelta, ReasoningDelta, ReasoningMetadata, UsageMetadata,
};
use std::fmt::{self, Display};

#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum AgentEvent {
    UserPrompt {
        content: String,
    },
    MessageDelta(MessageDelta),
    ReasoningDelta(ReasoningDelta),
    UsageMetadata(UsageMetadata),
    ReasoningMetadata(ReasoningMetadata),
    AssistantToolCallRequest {
        tool_call_id: String,
        name: String,
        arguments: String,
    },
    ToolResult {
        tool_call_id: String,
        name: String,
        content: String,
    },
    ValidationError {
        message: String,
    },
    Error {
        message: String,
    },
    Cancelled {
        reason: String,
    },
    Finished {
        finish_reason: FinishReason,
    },
    ContextSummaryCreated(ContextSummary),
    Internal {
        message: String,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContextSummary {
    pub id: String,
    pub replaces: Option<String>,
    pub source_event_start: usize,
    pub source_event_end: usize,
    pub content: String,
    pub estimated_tokens: usize,
}

impl AgentEvent {
    /// Creates a stored user prompt event.
    pub fn user_prompt(content: impl Into<String>) -> Self {
        Self::UserPrompt {
            content: content.into(),
        }
    }

    /// Creates a stored assistant tool-call request event.
    pub fn assistant_tool_call_request(
        tool_call_id: impl Into<String>,
        name: impl Into<String>,
        arguments: impl Into<String>,
    ) -> Self {
        Self::AssistantToolCallRequest {
            tool_call_id: tool_call_id.into(),
            name: name.into(),
            arguments: arguments.into(),
        }
    }

    /// Creates a stored tool result event tied to a provider tool-call id.
    pub fn tool_result(
        tool_call_id: impl Into<String>,
        name: impl Into<String>,
        content: impl Into<String>,
    ) -> Self {
        Self::ToolResult {
            tool_call_id: tool_call_id.into(),
            name: name.into(),
            content: content.into(),
        }
    }

    /// Creates a structured-output validation error event.
    pub fn validation_error(message: impl Into<String>) -> Self {
        Self::ValidationError {
            message: message.into(),
        }
    }

    /// Creates a terminal run error event.
    pub fn error(message: impl Into<String>) -> Self {
        Self::Error {
            message: message.into(),
        }
    }

    /// Creates a terminal run cancellation event.
    pub fn cancelled(reason: impl Into<String>) -> Self {
        Self::Cancelled {
            reason: reason.into(),
        }
    }

    /// Creates a terminal run finish event from provider finish metadata.
    pub fn finished(finished: spectacular_llms::ProviderFinished) -> Self {
        Self::Finished {
            finish_reason: finished.finish_reason,
        }
    }

    /// Creates an event that stores a compact summary replacing earlier transcript context.
    pub fn context_summary_created(
        id: impl Into<String>,
        replaces: Option<String>,
        source_event_start: usize,
        source_event_end: usize,
        content: impl Into<String>,
        estimated_tokens: usize,
    ) -> Self {
        Self::ContextSummaryCreated(ContextSummary {
            id: id.into(),
            replaces,
            source_event_start,
            source_event_end,
            content: content.into(),
            estimated_tokens,
        })
    }

    /// Creates an internal diagnostic event not replayed to providers.
    pub fn internal(message: impl Into<String>) -> Self {
        Self::Internal {
            message: message.into(),
        }
    }
}

impl Display for AgentEvent {
    /// Formats an agent event for compact logs and debug output.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AgentEvent::UserPrompt { content } => write!(formatter, "UserPrompt({content})"),
            AgentEvent::MessageDelta(delta) => {
                write!(
                    formatter,
                    "MessageDelta({:?}, {})",
                    delta.role, delta.content
                )
            }
            AgentEvent::ReasoningDelta(delta) => {
                write!(formatter, "ReasoningDelta({})", delta.content)
            }
            AgentEvent::UsageMetadata(usage) => write!(
                formatter,
                "UsageMetadata(input={:?}, output={:?}, total={:?})",
                usage.input_tokens, usage.output_tokens, usage.total_tokens
            ),
            AgentEvent::ReasoningMetadata(metadata) => write!(
                formatter,
                "ReasoningMetadata(effort={:?}, summary={:?})",
                metadata.effort, metadata.summary
            ),
            AgentEvent::AssistantToolCallRequest {
                tool_call_id,
                name,
                arguments,
            } => {
                write!(
                    formatter,
                    "AssistantToolCallRequest(id={tool_call_id}, name={name}, arguments={arguments})"
                )
            }
            AgentEvent::ToolResult {
                tool_call_id,
                name,
                content,
            } => write!(
                formatter,
                "ToolResult(id={tool_call_id}, name={name}, content={content})"
            ),
            AgentEvent::ValidationError { message } => {
                write!(formatter, "ValidationError({message})")
            }
            AgentEvent::Error { message } => write!(formatter, "Error({message})"),
            AgentEvent::Cancelled { reason } => write!(formatter, "Cancelled(reason={reason:?})"),
            AgentEvent::Finished { finish_reason } => {
                write!(formatter, "Finished(reason={finish_reason:?})")
            }
            AgentEvent::ContextSummaryCreated(summary) => write!(
                formatter,
                "ContextSummaryCreated(id={}, source={}..{}, tokens={})",
                summary.id,
                summary.source_event_start,
                summary.source_event_end,
                summary.estimated_tokens
            ),
            AgentEvent::Internal { message } => write!(formatter, "Internal({message})"),
        }
    }
}
