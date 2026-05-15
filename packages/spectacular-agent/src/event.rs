use crate::usage::ContextTokenUsage;
use spectacular_llms::{FinishReason, ReasoningMetadata, UsageMetadata};
use std::fmt::{self, Display};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AgentTranscriptItemId(String);

impl AgentTranscriptItemId {
    /// Creates an agent-owned transcript item ID without coupling to TUI types.
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// Returns the raw identifier string for downstream adapters and storage.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Display for AgentTranscriptItemId {
    /// Formats the transcript item ID as its raw stable string value.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum AgentEvent {
    UserPrompt {
        content: String,
    },
    MessageStart {
        id: AgentTranscriptItemId,
    },
    MessageDelta {
        id: AgentTranscriptItemId,
        content: String,
    },
    MessageFinish {
        id: AgentTranscriptItemId,
    },
    ReasoningStart {
        id: AgentTranscriptItemId,
    },
    ReasoningDelta {
        id: AgentTranscriptItemId,
        content: String,
    },
    ReasoningFinish {
        id: AgentTranscriptItemId,
    },
    UsageMetadata(UsageMetadata),
    ContextTokenUsage(ContextTokenUsage),
    ReasoningMetadata(ReasoningMetadata),
    ToolCallStart {
        tool_call_id: String,
        name: String,
        arguments: String,
    },
    ToolCallDelta {
        tool_call_id: String,
        content: String,
    },
    ToolCallFinish {
        tool_call_id: String,
        name: String,
        output: String,
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

    /// Creates an explicit assistant message lifecycle start event.
    pub fn message_start(id: impl Into<String>) -> Self {
        Self::MessageStart {
            id: AgentTranscriptItemId::new(id),
        }
    }

    /// Creates an explicit assistant message lifecycle delta event.
    pub fn message_delta(id: impl Into<String>, content: impl Into<String>) -> Self {
        Self::MessageDelta {
            id: AgentTranscriptItemId::new(id),
            content: content.into(),
        }
    }

    /// Creates an explicit assistant message lifecycle finish event.
    pub fn message_finish(id: impl Into<String>) -> Self {
        Self::MessageFinish {
            id: AgentTranscriptItemId::new(id),
        }
    }

    /// Creates an explicit reasoning lifecycle start event.
    pub fn reasoning_start(id: impl Into<String>) -> Self {
        Self::ReasoningStart {
            id: AgentTranscriptItemId::new(id),
        }
    }

    /// Creates an explicit reasoning lifecycle delta event.
    pub fn reasoning_delta(id: impl Into<String>, content: impl Into<String>) -> Self {
        Self::ReasoningDelta {
            id: AgentTranscriptItemId::new(id),
            content: content.into(),
        }
    }

    /// Creates an explicit reasoning lifecycle finish event.
    pub fn reasoning_finish(id: impl Into<String>) -> Self {
        Self::ReasoningFinish {
            id: AgentTranscriptItemId::new(id),
        }
    }

    /// Creates a stored tool-call lifecycle start event.
    pub fn tool_call_start(
        tool_call_id: impl Into<String>,
        name: impl Into<String>,
        arguments: impl Into<String>,
    ) -> Self {
        Self::ToolCallStart {
            tool_call_id: tool_call_id.into(),
            name: name.into(),
            arguments: arguments.into(),
        }
    }

    /// Creates a stored tool-call lifecycle progress event.
    pub fn tool_call_delta(tool_call_id: impl Into<String>, content: impl Into<String>) -> Self {
        Self::ToolCallDelta {
            tool_call_id: tool_call_id.into(),
            content: content.into(),
        }
    }

    /// Creates a stored tool-call lifecycle finish event tied to a provider tool-call id.
    pub fn tool_call_finish(
        tool_call_id: impl Into<String>,
        name: impl Into<String>,
        output: impl Into<String>,
    ) -> Self {
        Self::ToolCallFinish {
            tool_call_id: tool_call_id.into(),
            name: name.into(),
            output: output.into(),
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
            AgentEvent::MessageStart { id } => write!(formatter, "MessageStart(id={id})"),
            AgentEvent::MessageDelta { id, content } => {
                write!(formatter, "MessageDelta(id={id}, content={content})")
            }
            AgentEvent::MessageFinish { id } => write!(formatter, "MessageFinish(id={id})"),
            AgentEvent::ReasoningStart { id } => write!(formatter, "ReasoningStart(id={id})"),
            AgentEvent::ReasoningDelta { id, content } => {
                write!(formatter, "ReasoningDelta(id={id}, content={content})")
            }
            AgentEvent::ReasoningFinish { id } => write!(formatter, "ReasoningFinish(id={id})"),
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
            AgentEvent::ToolCallStart {
                tool_call_id,
                name,
                arguments,
            } => write!(
                formatter,
                "ToolCallStart(id={tool_call_id}, name={name}, arguments={arguments})"
            ),
            AgentEvent::ToolCallDelta {
                tool_call_id,
                content,
            } => write!(
                formatter,
                "ToolCallDelta(id={tool_call_id}, content={content})"
            ),
            AgentEvent::ToolCallFinish {
                tool_call_id,
                name,
                output,
            } => write!(
                formatter,
                "ToolCallFinish(id={tool_call_id}, name={name}, output={output})"
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
