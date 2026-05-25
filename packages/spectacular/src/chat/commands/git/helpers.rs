//! Git helper functions for chat commands.
//!
//! These utilities execute git commands using `tokio::process::Command`
//! and parse the output for use by git-related chat commands.

mod process;
mod shell;

use process::{git_command_label, run_git, run_git_capture};

#[cfg(test)]
use shell::{shell_escape, ShellSpec};

/// Errors that can occur during git operations.
#[derive(Debug)]
pub enum GitError {
    /// Git command exited with a non-zero status.
    CommandFailed {
        command: String,
        exit_code: i32,
        stderr: String,
    },
    /// Failed to spawn the git process.
    SpawnFailed { command: String, error: String },
    /// IO error during execution.
    Io(std::io::Error),
}

impl std::fmt::Display for GitError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GitError::CommandFailed {
                command,
                exit_code,
                stderr,
            } => {
                if stderr.trim().is_empty() {
                    write!(f, "git command failed (exit {}): {}", exit_code, command)
                } else {
                    write!(
                        f,
                        "git command failed (exit {}): {}\n{}",
                        exit_code, command, stderr
                    )
                }
            }
            GitError::SpawnFailed { command, error } => {
                write!(f, "failed to run '{}': {}", command, error)
            }
            GitError::Io(e) => write!(f, "io error: {}", e),
        }
    }
}

impl std::error::Error for GitError {}

impl From<std::io::Error> for GitError {
    fn from(error: std::io::Error) -> Self {
        GitError::Io(error)
    }
}

/// Returns the full staged diff as a string.
pub async fn get_staged_diff() -> Result<String, GitError> {
    let output = run_git(&["diff", "--cached"]).await?;
    Ok(output.stdout)
}

/// Commits staged changes with the given message and returns the git output.
pub async fn commit_with_message(message: &str) -> Result<String, GitError> {
    let output = run_git(&["commit", "-m", message]).await?;
    let mut result = output.stdout.trim().to_owned();
    if result.is_empty() {
        result = output.stderr.trim().to_owned();
    }
    Ok(result)
}

/// Returns `true` when there are staged changes ready to be committed.
pub async fn has_staged_changes() -> Result<bool, GitError> {
    // `git diff --cached --quiet` exits 0 when there are no changes, 1 when there are.
    let output = run_git_capture(&["diff", "--cached", "--quiet"]).await?;
    match output.exit_code {
        0 => Ok(false),
        1 => Ok(true),
        exit_code => Err(GitError::CommandFailed {
            command: git_command_label(&["diff", "--cached", "--quiet"]),
            exit_code,
            stderr: output.stderr,
        }),
    }
}

#[cfg(test)]
mod tests {
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/unit/chat/commands/git/helpers.rs"
    ));
}
