use std::error::Error;
use std::fmt::{self, Display};

use crate::proto::doric::lifecycle::v1;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WorkerStatus {
    Accepted,
    Starting,
    Running,
    WaitingForInput,
    Succeeded,
    Failed,
    Stopped,
    Unavailable,
    Untracked,
}

impl Display for WorkerStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Accepted => "accepted",
            Self::Starting => "starting",
            Self::Running => "running",
            Self::WaitingForInput => "waiting_for_input",
            Self::Succeeded => "succeeded",
            Self::Failed => "failed",
            Self::Stopped => "stopped",
            Self::Unavailable => "unavailable",
            Self::Untracked => "untracked",
        })
    }
}

impl TryFrom<v1::WorkerStatus> for WorkerStatus {
    type Error = UnknownStatusError;

    fn try_from(value: v1::WorkerStatus) -> Result<Self, Self::Error> {
        match value {
            v1::WorkerStatus::Accepted => Ok(Self::Accepted),
            v1::WorkerStatus::Starting => Ok(Self::Starting),
            v1::WorkerStatus::Running => Ok(Self::Running),
            v1::WorkerStatus::WaitingForInput => Ok(Self::WaitingForInput),
            v1::WorkerStatus::Succeeded => Ok(Self::Succeeded),
            v1::WorkerStatus::Failed => Ok(Self::Failed),
            v1::WorkerStatus::Stopped => Ok(Self::Stopped),
            v1::WorkerStatus::Unspecified => Err(UnknownStatusError),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct UnknownStatusError;

impl Display for UnknownStatusError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("unknown worker status")
    }
}

impl Error for UnknownStatusError {}
