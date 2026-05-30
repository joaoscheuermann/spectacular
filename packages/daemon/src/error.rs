use std::error::Error;
use std::fmt::{self, Display};
use std::io;
use std::path::{Path, PathBuf};

use lifecycle::identity::{RequestId, WorkerId};

#[derive(Debug)]
pub enum DaemonError {
    RootConfiguration {
        path: Option<PathBuf>,
        message: String,
        source: Option<Box<dyn Error + Send + Sync>>,
    },
    DuplicateWorker {
        worker_id: WorkerId,
    },
    UnknownWorker {
        worker_id: WorkerId,
    },
    TerminalWorker {
        worker_id: WorkerId,
    },
    StaleRequest {
        worker_id: WorkerId,
        request_id: RequestId,
    },
    DuplicateAnswer {
        worker_id: WorkerId,
        request_id: RequestId,
    },
    NoPendingInput {
        worker_id: WorkerId,
    },
    NotWaitingForInput {
        worker_id: WorkerId,
    },
}

impl DaemonError {
    pub fn root_configuration(
        path: Option<PathBuf>,
        message: impl Into<String>,
        source: Option<Box<dyn Error + Send + Sync>>,
    ) -> Self {
        Self::RootConfiguration {
            path,
            message: message.into(),
            source,
        }
    }

    pub fn is_root_configuration(&self) -> bool {
        matches!(self, Self::RootConfiguration { .. })
    }

    pub fn is_duplicate_worker(&self) -> bool {
        matches!(self, Self::DuplicateWorker { .. })
    }

    pub fn is_unknown_worker(&self) -> bool {
        matches!(self, Self::UnknownWorker { .. })
    }

    pub fn is_terminal_worker(&self) -> bool {
        matches!(self, Self::TerminalWorker { .. })
    }

    pub fn is_stale_request(&self) -> bool {
        matches!(self, Self::StaleRequest { .. })
    }

    pub fn is_duplicate_answer(&self) -> bool {
        matches!(self, Self::DuplicateAnswer { .. })
    }

    pub fn is_no_pending_input(&self) -> bool {
        matches!(self, Self::NoPendingInput { .. })
    }

    pub fn is_not_waiting_for_input(&self) -> bool {
        matches!(self, Self::NotWaitingForInput { .. })
    }

    pub fn path(&self) -> Option<&Path> {
        match self {
            Self::RootConfiguration { path, .. } => path.as_deref(),
            _ => None,
        }
    }
}

impl Display for DaemonError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RootConfiguration { path, message, .. } => match path {
                Some(path) => write!(
                    f,
                    "worker root configuration error at {}: {message}",
                    path.display()
                ),
                None => write!(f, "worker root configuration error: {message}"),
            },
            Self::DuplicateWorker { worker_id } => {
                write!(f, "worker `{worker_id}` is already tracked")
            }
            Self::UnknownWorker { worker_id } => {
                write!(f, "worker `{worker_id}` is unknown or untracked")
            }
            Self::TerminalWorker { worker_id } => {
                write!(f, "worker `{worker_id}` is terminal")
            }
            Self::StaleRequest {
                worker_id,
                request_id,
            } => write!(
                f,
                "request `{request_id}` is stale for worker `{worker_id}`"
            ),
            Self::DuplicateAnswer {
                worker_id,
                request_id,
            } => write!(
                f,
                "request `{request_id}` for worker `{worker_id}` was already answered"
            ),
            Self::NoPendingInput { worker_id } => {
                write!(f, "worker `{worker_id}` has no pending input")
            }
            Self::NotWaitingForInput { worker_id } => {
                write!(f, "worker `{worker_id}` is not waiting for input")
            }
        }
    }
}

impl Error for DaemonError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::RootConfiguration {
                source: Some(source),
                ..
            } => Some(source.as_ref()),
            _ => None,
        }
    }
}

impl From<io::Error> for DaemonError {
    fn from(source: io::Error) -> Self {
        Self::root_configuration(None, source.to_string(), Some(Box::new(source)))
    }
}

impl From<config::ConfigError> for DaemonError {
    fn from(source: config::ConfigError) -> Self {
        Self::root_configuration(None, source.to_string(), Some(Box::new(source)))
    }
}

pub type DaemonResult<T> = Result<T, DaemonError>;
