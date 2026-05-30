use std::error::Error;
use std::fmt::{self, Display};
use std::path::PathBuf;
use std::time::Duration;

use lifecycle::repo::RepoIdentity;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum WorkerFailureReason {
    RootConfiguration,
    RepoCloneTargetConflict,
    GitUnavailable,
    GitCloneFailed,
    GitCloneTimedOut,
    GitCloneCancelled,
}

#[derive(Debug)]
pub enum WorkerError {
    InvalidWorkerRoot {
        path: PathBuf,
    },
    CloneTargetConflict {
        path: PathBuf,
    },
    GitMissing {
        program: String,
    },
    GitFailed {
        status: i32,
        stderr: String,
        repo: String,
    },
    GitTimeout {
        duration: Duration,
    },
    GitCancelled,
}

impl WorkerError {
    pub fn invalid_worker_root(path: PathBuf) -> Self {
        Self::InvalidWorkerRoot { path }
    }

    pub fn clone_target_conflict(path: PathBuf) -> Self {
        Self::CloneTargetConflict { path }
    }

    pub fn git_missing(program: impl Into<String>) -> Self {
        Self::GitMissing {
            program: program.into(),
        }
    }

    pub fn git_failed(status: i32, stderr: &str, repo_url: &str) -> Self {
        Self::GitFailed {
            status,
            stderr: redact_git_stderr(stderr, repo_url),
            repo: redacted_repo(repo_url),
        }
    }

    pub fn git_timeout(duration: Duration) -> Self {
        Self::GitTimeout { duration }
    }

    pub fn git_cancelled() -> Self {
        Self::GitCancelled
    }

    pub fn is_invalid_worker_root(&self) -> bool {
        matches!(self, Self::InvalidWorkerRoot { .. })
    }

    pub fn is_clone_target_conflict(&self) -> bool {
        matches!(self, Self::CloneTargetConflict { .. })
    }

    pub fn is_git_missing(&self) -> bool {
        matches!(self, Self::GitMissing { .. })
    }

    pub fn is_git_failure(&self) -> bool {
        matches!(self, Self::GitFailed { .. })
    }

    pub fn is_git_timeout(&self) -> bool {
        matches!(self, Self::GitTimeout { .. })
    }

    pub fn is_git_cancelled(&self) -> bool {
        matches!(self, Self::GitCancelled)
    }

    pub fn lifecycle_failure_reason(&self) -> WorkerFailureReason {
        match self {
            Self::InvalidWorkerRoot { .. } => WorkerFailureReason::RootConfiguration,
            Self::CloneTargetConflict { .. } => WorkerFailureReason::RepoCloneTargetConflict,
            Self::GitMissing { .. } => WorkerFailureReason::GitUnavailable,
            Self::GitFailed { .. } => WorkerFailureReason::GitCloneFailed,
            Self::GitTimeout { .. } => WorkerFailureReason::GitCloneTimedOut,
            Self::GitCancelled => WorkerFailureReason::GitCloneCancelled,
        }
    }

    pub fn lifecycle_failure_message(&self) -> String {
        self.to_string()
    }
}

impl Display for WorkerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidWorkerRoot { path } => {
                write!(f, "worker root configuration error at {}", path.display())
            }
            Self::CloneTargetConflict { path } => write!(
                f,
                "worker repo clone target is not empty at {}",
                path.display()
            ),
            Self::GitMissing { program } => {
                write!(f, "required Git executable `{program}` is unavailable")
            }
            Self::GitFailed {
                status,
                stderr,
                repo,
            } => write!(
                f,
                "git clone failed for {repo} with status {status}: {stderr}"
            ),
            Self::GitTimeout { duration } => {
                write!(
                    f,
                    "git clone timed out after {} seconds",
                    duration.as_secs()
                )
            }
            Self::GitCancelled => f.write_str("git clone was cancelled"),
        }
    }
}

impl Error for WorkerError {}

pub type WorkerResult<T> = Result<T, WorkerError>;

fn redact_git_stderr(stderr: &str, repo_url: &str) -> String {
    lifecycle::redaction::redact_failure_text(stderr).replace(repo_url, &redacted_repo(repo_url))
}

fn redacted_repo(repo_url: &str) -> String {
    RepoIdentity::from_raw_url(repo_url)
        .map(|identity| identity.to_string())
        .unwrap_or_else(|_| "[REDACTED]".to_owned())
}
