use super::shell::{shell_escape, ShellSpec};
use super::GitError;

use std::path::PathBuf;
use std::process::Stdio;
use tokio::io::{AsyncRead, AsyncReadExt};
use tokio::process::{Child, Command};

pub(super) struct CommandOutput {
    pub(super) stdout: String,
    pub(super) stderr: String,
    pub(super) exit_code: i32,
}

pub(super) async fn run_git(args: &[&str]) -> Result<CommandOutput, GitError> {
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

pub(super) async fn run_git_capture(args: &[&str]) -> Result<CommandOutput, GitError> {
    let cwd = std::env::current_dir().map_err(GitError::Io)?;
    let child = spawn_git(args, cwd)?;
    collect_output(child).await
}

pub(super) fn git_command_label(args: &[&str]) -> String {
    format!("git {}", args.join(" "))
}

fn spawn_git(args: &[&str], cwd: PathBuf) -> Result<Child, GitError> {
    let git_command = escaped_git_command(args);
    let mut command = ShellSpec::detect().to_command(&git_command);
    configure_command(&mut command, cwd);

    command.spawn().map_err(|error| GitError::SpawnFailed {
        command: git_command_label(args),
        error: error.to_string(),
    })
}

fn escaped_git_command(args: &[&str]) -> String {
    let escaped_args = args.iter().map(|arg| shell_escape(arg)).collect::<Vec<_>>();
    format!("git {}", escaped_args.join(" "))
}

fn configure_command(command: &mut Command, cwd: PathBuf) {
    command
        .current_dir(cwd)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
}

async fn collect_output(mut child: Child) -> Result<CommandOutput, GitError> {
    let stdout_task = tokio::spawn(read_pipe(child.stdout.take()));
    let stderr_task = tokio::spawn(read_pipe(child.stderr.take()));
    let status = child.wait().await.map_err(GitError::Io)?;

    Ok(CommandOutput {
        stdout: stdout_task.await.unwrap_or_default(),
        stderr: stderr_task.await.unwrap_or_default(),
        exit_code: status.code().unwrap_or(-1),
    })
}

async fn read_pipe<T>(pipe: Option<T>) -> String
where
    T: AsyncRead + Send + Unpin + 'static,
{
    let Some(mut reader) = pipe else {
        return String::new();
    };

    let mut buf = Vec::new();
    let _ = reader.read_to_end(&mut buf).await;
    String::from_utf8_lossy(&buf).into_owned()
}
