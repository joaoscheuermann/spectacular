/// App-owned status for slash-command lifecycle records.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CommandStatus {
    Success,
    Failed,
    Cancelled,
    TimedOut,
    Error,
}

/// App-owned slash-command lifecycle event payload.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum CommandEvent {
    Start(CommandStart),
    Delta(CommandDelta),
    Finished(CommandFinished),
}

/// App-owned slash-command lifecycle start payload.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CommandStart {
    pub command_id: String,
    pub source: String,
    pub name: String,
    pub title: String,
    pub command: String,
    pub working_directory: Option<String>,
}

/// App-owned slash-command lifecycle output payload.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CommandDelta {
    pub command_id: String,
    pub channel: String,
    pub content: String,
    pub sequence: u64,
}

/// App-owned slash-command lifecycle completion payload.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CommandFinished {
    pub command_id: String,
    pub status: CommandStatus,
    pub summary: String,
}

impl CommandStatus {
    /// Converts command lifecycle status into its persisted string value.
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Success => "success",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
            Self::TimedOut => "timed_out",
            Self::Error => "error",
        }
    }

    /// Parses command lifecycle status from persisted session data.
    pub(crate) fn from_str(status: &str) -> Self {
        match status {
            "success" => Self::Success,
            "cancelled" => Self::Cancelled,
            "timed_out" => Self::TimedOut,
            "error" => Self::Error,
            _ => Self::Failed,
        }
    }
}
