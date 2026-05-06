//! Git helper functions for chat commands.
//!
//! These utilities execute git commands using `tokio::process::Command`
//! and parse the output for use by git-related chat commands.

use std::path::PathBuf;
use std::process::Stdio;
use tokio::process::Command;

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

// --------------------------------------------------------------------------------
// Internal helpers
// --------------------------------------------------------------------------------

struct CommandOutput {
    stdout: String,
    stderr: String,
    exit_code: i32,
}

/// Escapes a single argument for safe inclusion in a shell command string.
fn shell_escape(arg: &str) -> String {
    // If the argument contains spaces, quotes, parentheses, newlines, or other
    // shell-special characters, wrap it in quotes and escape internal quotes.
    let needs_quoting = arg.chars().any(|c| {
        matches!(
            c,
            ' ' | '\t'
                | '\n'
                | '\r'
                | '"'
                | '\''
                | '('
                | ')'
                | '{'
                | '}'
                | '|'
                | '&'
                | ';'
                | '<'
                | '>'
                | '`'
                | '$'
                | '\\'
        )
    });

    if needs_quoting {
        // Escape any existing double quotes, then wrap in double quotes.
        let escaped = arg.replace('"', "\\\"");
        format!("\"{}\"", escaped)
    } else {
        arg.to_owned()
    }
}

async fn run_git(args: &[&str]) -> Result<CommandOutput, GitError> {
    let output = run_git_capture(args).await?;

    if output.exit_code == 0 {
        return Ok(output);
    }

    Err(GitError::CommandFailed {
        command: git_command_label(args),
        exit_code: output.exit_code,
        stderr: output.stderr,
    })
}

async fn run_git_capture(args: &[&str]) -> Result<CommandOutput, GitError> {
    let cwd = std::env::current_dir().map_err(GitError::Io)?;
    let shell = ShellSpec::detect();

    let escaped_args: Vec<String> = args.iter().map(|a| shell_escape(a)).collect();
    let git_command = format!("git {}", escaped_args.join(" "));
    let mut command = shell.to_command(&git_command);
    command
        .current_dir(cwd)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);

    let mut child = command.spawn().map_err(|e| GitError::SpawnFailed {
        command: format!("git {}", args.join(" ")),
        error: e.to_string(),
    })?;

    let stdout_handle = child.stdout.take();
    let stderr_handle = child.stderr.take();

    let stdout_task = tokio::spawn(async move {
        if let Some(mut reader) = stdout_handle {
            use tokio::io::AsyncReadExt;
            let mut buf = Vec::new();
            let _ = reader.read_to_end(&mut buf).await;
            String::from_utf8_lossy(&buf).into_owned()
        } else {
            String::new()
        }
    });

    let stderr_task = tokio::spawn(async move {
        if let Some(mut reader) = stderr_handle {
            use tokio::io::AsyncReadExt;
            let mut buf = Vec::new();
            let _ = reader.read_to_end(&mut buf).await;
            String::from_utf8_lossy(&buf).into_owned()
        } else {
            String::new()
        }
    });

    let status = child.wait().await.map_err(GitError::Io)?;
    let stdout = stdout_task.await.unwrap_or_default();
    let stderr = stderr_task.await.unwrap_or_default();
    let exit_code = status.code().unwrap_or(-1);

    Ok(CommandOutput {
        stdout,
        stderr,
        exit_code,
    })
}

fn git_command_label(args: &[&str]) -> String {
    format!("git {}", args.join(" "))
}

// Minimal shell detection reused from the terminal tool patterns.
#[derive(Clone, Debug)]
enum ShellSpec {
    #[cfg(windows)]
    PowerShell { program: String },
    #[cfg(windows)]
    Cmd { program: String },
    #[cfg(not(windows))]
    Bash,
}

impl ShellSpec {
    fn detect() -> Self {
        #[cfg(windows)]
        {
            if is_in_path("pwsh") {
                return ShellSpec::PowerShell {
                    program: "pwsh".to_owned(),
                };
            }
            if is_in_path("powershell.exe") {
                return ShellSpec::PowerShell {
                    program: "powershell.exe".to_owned(),
                };
            }
            let system_ps = std::env::var_os("SystemRoot")
                .map(PathBuf::from)
                .map(|root| {
                    root.join("System32")
                        .join("WindowsPowerShell")
                        .join("v1.0")
                        .join("powershell.exe")
                })
                .filter(|p| p.is_file());
            if let Some(program) = system_ps {
                return ShellSpec::PowerShell {
                    program: program.to_string_lossy().into_owned(),
                };
            }
            ShellSpec::Cmd {
                program: std::env::var("ComSpec").unwrap_or_else(|_| "cmd.exe".to_owned()),
            }
        }

        #[cfg(not(windows))]
        {
            ShellSpec::Bash
        }
    }

    fn to_command(&self, command_text: &str) -> Command {
        match self {
            #[cfg(windows)]
            ShellSpec::PowerShell { program } => {
                let mut cmd = Command::new(program);
                cmd.args([
                    "-NoLogo",
                    "-NoProfile",
                    "-NonInteractive",
                    "-Command",
                    command_text,
                ]);
                cmd
            }
            #[cfg(windows)]
            ShellSpec::Cmd { program } => {
                let mut cmd = Command::new(program);
                cmd.args(["/C", command_text]);
                cmd
            }
            #[cfg(not(windows))]
            ShellSpec::Bash => {
                let mut cmd = Command::new("bash");
                cmd.args(["-lc", command_text]);
                cmd
            }
        }
    }
}

#[cfg(windows)]
fn is_in_path(executable: &str) -> bool {
    let Some(path_var) = std::env::var_os("PATH") else {
        return false;
    };
    let has_ext = PathBuf::from(executable).extension().is_some();
    let extensions = std::env::var_os("PATHEXT")
        .map(|v| {
            v.to_string_lossy()
                .split(';')
                .map(str::to_owned)
                .collect::<Vec<_>>()
        })
        .unwrap_or_else(|| vec![".EXE".to_owned(), ".CMD".to_owned(), ".BAT".to_owned()]);

    std::env::split_paths(&path_var).any(|dir| {
        if has_ext && dir.join(executable).is_file() {
            return true;
        }
        extensions
            .iter()
            .any(|ext| dir.join(format!("{}{}", executable, ext)).is_file())
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};
    use tokio::fs;

    async fn temp_git_repo(name: &str) -> PathBuf {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("spectacular-git-helpers-{}-{}", name, suffix));
        fs::create_dir_all(&dir).await.unwrap();

        let shell = ShellSpec::detect();
        for cmd in &[
            "git init",
            "git config user.email \"test@test.com\"",
            "git config user.name \"Test\"",
        ] {
            let mut c = shell.to_command(cmd);
            c.current_dir(&dir);
            c.status().await.unwrap();
        }

        dir
    }

    #[tokio::test]
    async fn staged_change_detection_reports_false_then_true() {
        let dir = temp_git_repo("staged-detection").await;
        let original = std::env::current_dir().unwrap();
        let _ = std::env::set_current_dir(&dir);

        let has_staged = has_staged_changes().await.unwrap();
        assert!(!has_staged);

        fs::write(dir.join("file.txt"), "staged content\n")
            .await
            .unwrap();
        let shell = ShellSpec::detect();
        let mut add = shell.to_command("git add file.txt");
        add.current_dir(&dir);
        assert!(add.status().await.unwrap().success());

        let has_staged = has_staged_changes().await.unwrap();

        let _ = std::env::set_current_dir(&original);
        let _ = fs::remove_dir_all(&dir).await;

        assert!(has_staged);
    }

    #[test]
    fn shell_escape_wraps_args_with_parentheses() {
        let arg = "feat(chat): add new feature";
        let escaped = shell_escape(arg);
        assert_eq!(escaped, "\"feat(chat): add new feature\"");
    }

    #[test]
    fn shell_escape_wraps_multiline_args() {
        let arg = "fix: bug\n\nThis fixes a bug.";
        let escaped = shell_escape(arg);
        assert!(escaped.starts_with('"') && escaped.ends_with('"'));
    }

    #[test]
    fn shell_escape_escapes_internal_quotes() {
        let arg = "fix: \"something\" broke";
        let escaped = shell_escape(arg);
        assert_eq!(escaped, "\"fix: \\\"something\\\" broke\"");
    }

    #[test]
    fn shell_escape_plain_arg_unchanged() {
        let arg = "simple-arg";
        let escaped = shell_escape(arg);
        assert_eq!(escaped, "simple-arg");
    }
}
