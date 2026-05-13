use crate::usage::ContextTokenUsage;
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
    ContextTokenUsage(ContextTokenUsage),
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
    CommandStart(CommandStart),
    CommandDelta(CommandDelta),
    CommandFinished(CommandFinished),
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CommandStart {
    pub command_id: String,
    pub source: String,
    pub name: String,
    pub title: String,
    pub command: String,
    pub working_directory: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CommandDelta {
    pub command_id: String,
    pub channel: String,
    pub content: String,
    pub sequence: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CommandFinished {
    pub command_id: String,
    pub status: CommandStatus,
    pub summary: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CommandStatus {
    Success,
    Failed,
    Cancelled,
    TimedOut,
    Error,
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

    /// Creates a user-visible command lifecycle start event.
    pub fn command_start(
        command_id: impl Into<String>,
        source: impl Into<String>,
        name: impl Into<String>,
        title: impl Into<String>,
        command: impl Into<String>,
        working_directory: Option<String>,
    ) -> Self {
        Self::CommandStart(CommandStart {
            command_id: command_id.into(),
            source: source.into(),
            name: name.into(),
            title: title.into(),
            command: command.into(),
            working_directory,
        })
    }

    /// Creates a bounded user-visible command lifecycle progress event.
    pub fn command_delta(
        command_id: impl Into<String>,
        channel: impl Into<String>,
        content: impl Into<String>,
        sequence: u64,
    ) -> Self {
        Self::CommandDelta(CommandDelta {
            command_id: command_id.into(),
            channel: channel.into(),
            content: content.into(),
            sequence,
        })
    }

    /// Creates a user-visible command lifecycle completion event.
    pub fn command_finished(
        command_id: impl Into<String>,
        status: CommandStatus,
        summary: impl Into<String>,
    ) -> Self {
        Self::CommandFinished(CommandFinished {
            command_id: command_id.into(),
            status,
            summary: summary.into(),
        })
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
            AgentEvent::ContextTokenUsage(usage) => write!(
                formatter,
                "ContextTokenUsage(input={}, window={:?})",
                usage.input_tokens, usage.context_window_tokens
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
            AgentEvent::CommandStart(start) => write!(
                formatter,
                "CommandStart(id={}, source={}, name={}, title={})",
                start.command_id, start.source, start.name, start.title
            ),
            AgentEvent::CommandDelta(delta) => write!(
                formatter,
                "CommandDelta(id={}, channel={}, sequence={}, content={})",
                delta.command_id, delta.channel, delta.sequence, delta.content
            ),
            AgentEvent::CommandFinished(finished) => write!(
                formatter,
                "CommandFinished(id={}, status={:?}, summary={})",
                finished.command_id, finished.status, finished.summary
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
