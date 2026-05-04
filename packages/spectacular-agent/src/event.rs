use spectacular_llms::{
    FinishReason, MessageDelta, ReasoningDelta, ReasoningMetadata, UsageMetadata,
};
use std::fmt::{self, Display};

#[derive(Clone, Debug, Eq, PartialEq)]
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
    Internal {
        message: String,
    },
}

impl AgentEvent {
    pub fn user_prompt(content: impl Into<String>) -> Self {
        Self::UserPrompt {
            content: content.into(),
        }
    }

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

    pub fn validation_error(message: impl Into<String>) -> Self {
        Self::ValidationError {
            message: message.into(),
        }
    }

    pub fn error(message: impl Into<String>) -> Self {
        Self::Error {
            message: message.into(),
        }
    }

    pub fn cancelled(reason: impl Into<String>) -> Self {
        Self::Cancelled {
            reason: reason.into(),
        }
    }

    pub fn finished(finished: spectacular_llms::ProviderFinished) -> Self {
        Self::Finished {
            finish_reason: finished.finish_reason,
        }
    }

    pub fn internal(message: impl Into<String>) -> Self {
        Self::Internal {
            message: message.into(),
        }
    }
}

impl Display for AgentEvent {
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
            AgentEvent::Internal { message } => write!(formatter, "Internal({message})"),
        }
    }
}
