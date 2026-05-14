use crate::ids::TranscriptItemId;

/// Current high-level runtime status rendered by the status line.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Status {
    Idle,
    Running {
        activity: Activity,
        cancellable: bool,
    },
    Cancelling,
    Failed {
        message: String,
    },
}

impl Status {
    /// Returns whether the current status can transition to cancellation.
    pub fn is_cancellable(&self) -> bool {
        matches!(
            self,
            Self::Running {
                cancellable: true,
                ..
            }
        )
    }
}

/// Specific in-flight activity displayed while the runtime is busy.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Activity {
    WaitingForModel,
    StreamingAssistant {
        id: TranscriptItemId,
    },
    StreamingReasoning {
        id: TranscriptItemId,
    },
    RunningTool {
        id: TranscriptItemId,
        name: String,
    },
    RunningCommand {
        id: TranscriptItemId,
        command_id: String,
    },
    Retrying {
        attempt: usize,
    },
}
