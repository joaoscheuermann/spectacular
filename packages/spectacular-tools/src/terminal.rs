//! Host shell execution for the built-in terminal tool.
//!
//! Shell selection follows the v1 built-in-tool contract: Windows prefers
//! `pwsh`, falls back to `powershell.exe`, then `cmd.exe`; Unix-like platforms
//! run commands with `bash -lc`.
//!
//! Timeouts default to 120000ms and clamp at 600000ms so provider-supplied
//! values cannot create unbounded command executions.
//!
//! Process-tree termination is best effort in v1. On Windows the tool calls
//! `taskkill /T /F` when a child PID is available. On Unix platforms it starts
//! the shell in a new process group and signals that group. In both cases the
//! direct child is also killed as a fallback, but OS permissions and shell
//! behavior can still leave grandchildren behind.

use crate::display::paint;
use crate::path::resolve_workspace_path;
use anstyle::AnsiColor;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use spectacular_agent::{Cancellation, Tool, ToolDisplay, ToolExecution, ToolManifest};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;
use tokio::io::{AsyncRead, AsyncReadExt};
use tokio::process::{Child, Command};
use tokio::time::Instant;

pub const TERMINAL_TOOL_NAME: &str = "terminal";

const DEFAULT_TIMEOUT_MS: u64 = 120_000;
const MAX_TIMEOUT_MS: u64 = 600_000;
const CANCELLATION_POLL_MS: u64 = 25;
const TERMINAL_TOOL_DESCRIPTION: &str =
    "Executes shell commands on the host machine. Returns stdout, stderr, and exit_code.";

#[derive(Clone, Debug)]
pub struct TerminalTool {
    workspace_root: PathBuf,
}

impl TerminalTool {
    pub fn new(workspace_root: impl Into<PathBuf>) -> Self {
        Self {
            workspace_root: workspace_root.into(),
        }
    }
}

impl Tool for TerminalTool {
    fn name(&self) -> &str {
        TERMINAL_TOOL_NAME
    }

    fn manifest(&self) -> ToolManifest {
        ToolManifest::new(
            TERMINAL_TOOL_NAME,
            TERMINAL_TOOL_DESCRIPTION,
            json!({
                "type": "object",
                "properties": {
                    "command": {
                        "type": "string",
                        "description": "Shell command to execute"
                    },
                    "working_directory": {
                        "type": "string",
                        "description": "Working directory for the command. Relative paths resolve against the workspace root."
                    },
                    "timeout_ms": {
                        "type": "integer",
                        "description": "Command timeout in milliseconds (default: 120000, maximum: 600000)"
                    }
                },
                "required": ["command"],
                "additionalProperties": false
            }),
        )
    }

    fn format_input(&self, arguments: &Value) -> ToolDisplay {
        let command = arguments
            .get("command")
            .and_then(Value::as_str)
            .unwrap_or("<missing command>");
        let working_directory = arguments
            .get("working_directory")
            .and_then(Value::as_str)
            .filter(|value| !value.is_empty())
            .unwrap_or(".");
        format!("{command} in {working_directory}")
    }

    fn format_output(&self, raw_output: &str, parsed_output: Option<&Value>) -> ToolDisplay {
        let Some(output) = parsed_output else {
            return raw_output.to_string();
        };

        let exit_code = output
            .get("exit_code")
            .and_then(Value::as_i64)
            .unwrap_or(-1);
        let status = if exit_code == 0 {
            paint(AnsiColor::BrightGreen.on_default().bold(), "exited")
        } else {
            paint(AnsiColor::BrightRed.on_default().bold(), "failed")
        };
        format!("{status} with code {exit_code}")
    }

    fn execute<'a>(&'a self, arguments: Value, cancellation: Cancellation) -> ToolExecution<'a> {
        let workspace_root = self.workspace_root.clone();

        Box::pin(async move {
            let input = match serde_json::from_value::<TerminalInput>(arguments) {
                Ok(input) => input,
                Err(error) => {
                    return Ok(terminal_error(format!("Invalid input JSON: {error}")));
                }
            };

            Ok(serialize_output(
                &execute_terminal(&workspace_root, input, cancellation).await,
            ))
        })
    }
}

#[derive(Debug, Deserialize)]
struct TerminalInput {
    command: String,
    #[serde(default)]
    working_directory: Option<String>,
    #[serde(default)]
    timeout_ms: Option<u64>,
}

#[derive(Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct TerminalOutput {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
}

async fn execute_terminal(
    workspace_root: &Path,
    input: TerminalInput,
    cancellation: Cancellation,
) -> TerminalOutput {
    let timeout_ms = effective_timeout_ms(input.timeout_ms);
    let working_directory = resolve_working_directory(workspace_root, input.working_directory);
    let shell = ShellSpec::detect();
    let mut command = shell.command(&input.command);
    command
        .current_dir(working_directory)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    configure_process_group(&mut command);

    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(error) => {
            return TerminalOutput {
                stdout: String::new(),
                stderr: format!("Failed to spawn command: {error}"),
                exit_code: -1,
            };
        }
    };

    let child_id = child.id();
    let stdout_reader = spawn_reader(child.stdout.take());
    let stderr_reader = spawn_reader(child.stderr.take());
    let completion = wait_for_completion(&mut child, child_id, timeout_ms, cancellation).await;
    let stdout = read_joined(stdout_reader).await;
    let stderr = read_joined(stderr_reader).await;

    match completion {
        CommandCompletion::Exited(status) => TerminalOutput {
            stdout,
            stderr,
            exit_code: status.code().unwrap_or(-1),
        },
        CommandCompletion::WaitError(error) => TerminalOutput {
            stdout,
            stderr: append_message(stderr, format!("Command execution error: {error}")),
            exit_code: -1,
        },
        CommandCompletion::TimedOut => TerminalOutput {
            stdout,
            stderr: append_message(stderr, format!("Command timed out after {timeout_ms}ms")),
            exit_code: -1,
        },
        CommandCompletion::Cancelled => TerminalOutput {
            stdout,
            stderr: append_message(stderr, "Command cancelled".to_owned()),
            exit_code: -1,
        },
    }
}

enum CommandCompletion {
    Exited(std::process::ExitStatus),
    WaitError(std::io::Error),
    TimedOut,
    Cancelled,
}

async fn wait_for_completion(
    child: &mut Child,
    child_id: Option<u32>,
    timeout_ms: u64,
    cancellation: Cancellation,
) -> CommandCompletion {
    let deadline = Instant::now() + Duration::from_millis(timeout_ms);

    loop {
        if cancellation.is_cancelled() {
            terminate_process_tree(child_id, child).await;
            let _ = child.wait().await;
            return CommandCompletion::Cancelled;
        }

        let now = Instant::now();
        if now >= deadline {
            terminate_process_tree(child_id, child).await;
            let _ = child.wait().await;
            return CommandCompletion::TimedOut;
        }

        let poll_duration = Duration::from_millis(CANCELLATION_POLL_MS)
            .min(deadline.saturating_duration_since(now));
        tokio::select! {
            result = child.wait() => {
                return match result {
                    Ok(status) => CommandCompletion::Exited(status),
                    Err(error) => CommandCompletion::WaitError(error),
                };
            }
            _ = tokio::time::sleep(poll_duration) => {}
        }
    }
}

fn resolve_working_directory(workspace_root: &Path, working_directory: Option<String>) -> PathBuf {
    match working_directory
        .as_deref()
        .filter(|value| !value.is_empty())
    {
        Some(path) => resolve_workspace_path(workspace_root, path),
        None => workspace_root.to_path_buf(),
    }
}

fn effective_timeout_ms(timeout_ms: Option<u64>) -> u64 {
    timeout_ms.unwrap_or(DEFAULT_TIMEOUT_MS).min(MAX_TIMEOUT_MS)
}

fn spawn_reader<R>(reader: Option<R>) -> tokio::task::JoinHandle<String>
where
    R: AsyncRead + Send + Unpin + 'static,
{
    tokio::spawn(async move {
        let Some(mut reader) = reader else {
            return String::new();
        };

        let mut bytes = Vec::new();
        if reader.read_to_end(&mut bytes).await.is_err() {
            return String::new();
        }

        String::from_utf8_lossy(&bytes).into_owned()
    })
}

async fn read_joined(reader: tokio::task::JoinHandle<String>) -> String {
    reader.await.unwrap_or_default()
}

fn append_message(mut existing: String, message: String) -> String {
    if existing.is_empty() {
        return message;
    }

    if !existing.ends_with('\n') {
        existing.push('\n');
    }
    existing.push_str(&message);
    existing
}

async fn terminate_process_tree(child_id: Option<u32>, child: &mut Child) {
    if let Some(child_id) = child_id {
        terminate_platform_process_tree(child_id).await;
    }

    if matches!(child.try_wait(), Ok(None)) {
        let _ = child.start_kill();
    }
}

#[cfg(windows)]
async fn terminate_platform_process_tree(child_id: u32) {
    let _ = Command::new("taskkill")
        .args(["/PID", &child_id.to_string(), "/T", "/F"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .await;
}

#[cfg(unix)]
async fn terminate_platform_process_tree(child_id: u32) {
    let process_group = format!("-{child_id}");
    let _ = Command::new("kill")
        .args(["-TERM", &process_group])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .await;
    tokio::time::sleep(Duration::from_millis(50)).await;
    let _ = Command::new("kill")
        .args(["-KILL", &process_group])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .await;
}

#[cfg(not(any(unix, windows)))]
async fn terminate_platform_process_tree(_child_id: u32) {}

#[cfg(unix)]
fn configure_process_group(command: &mut Command) {
    command.process_group(0);
}

#[cfg(not(unix))]
fn configure_process_group(_command: &mut Command) {}

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
            windows_shell_spec()
        }

        #[cfg(not(windows))]
        {
            Self::Bash
        }
    }

    fn command(&self, command_text: &str) -> Command {
        match self {
            #[cfg(windows)]
            Self::PowerShell { program } => {
                let mut command = Command::new(program);
                command.args([
                    "-NoLogo",
                    "-NoProfile",
                    "-NonInteractive",
                    "-Command",
                    command_text,
                ]);
                command
            }
            #[cfg(windows)]
            Self::Cmd { program } => {
                let mut command = Command::new(program);
                command.args(["/C", command_text]);
                command
            }
            #[cfg(not(windows))]
            Self::Bash => {
                let mut command = Command::new("bash");
                command.args(["-lc", command_text]);
                command
            }
        }
    }
}

#[cfg(windows)]
fn windows_shell_spec() -> ShellSpec {
    if executable_in_path("pwsh") {
        return ShellSpec::PowerShell {
            program: "pwsh".to_owned(),
        };
    }

    if executable_in_path("powershell.exe") {
        return ShellSpec::PowerShell {
            program: "powershell.exe".to_owned(),
        };
    }

    let system_powershell = std::env::var_os("SystemRoot")
        .map(PathBuf::from)
        .map(|root| {
            root.join("System32")
                .join("WindowsPowerShell")
                .join("v1.0")
                .join("powershell.exe")
        })
        .filter(|path| path.is_file());
    if let Some(program) = system_powershell {
        return ShellSpec::PowerShell {
            program: program.to_string_lossy().into_owned(),
        };
    }

    ShellSpec::Cmd {
        program: std::env::var("ComSpec").unwrap_or_else(|_| "cmd.exe".to_owned()),
    }
}

#[cfg(windows)]
fn executable_in_path(command: &str) -> bool {
    let Some(path) = std::env::var_os("PATH") else {
        return false;
    };
    let has_extension = Path::new(command).extension().is_some();
    let extensions = std::env::var_os("PATHEXT")
        .map(|value| {
            value
                .to_string_lossy()
                .split(';')
                .map(str::to_owned)
                .collect::<Vec<_>>()
        })
        .unwrap_or_else(|| vec![".EXE".to_owned(), ".CMD".to_owned(), ".BAT".to_owned()]);

    std::env::split_paths(&path).any(|directory| {
        if has_extension && directory.join(command).is_file() {
            return true;
        }

        extensions
            .iter()
            .any(|extension| directory.join(format!("{command}{extension}")).is_file())
    })
}

fn terminal_error(message: impl Into<String>) -> String {
    serialize_output(&TerminalOutput {
        stdout: String::new(),
        stderr: message.into(),
        exit_code: -1,
    })
}

fn serialize_output(output: &TerminalOutput) -> String {
    serde_json::to_string(output).expect("terminal output should serialize")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::temp_workspace;
    use crate::test_support::{remove_workspace, write_file};
    use serde_json::json;

    #[tokio::test]
    async fn short_command_in_temp_workspace_returns_stdout_and_success() {
        let workspace_root = temp_workspace("terminal_short_command").await;
        let nested = workspace_root.join("nested");
        tokio::fs::create_dir_all(&nested).await.unwrap();
        let tool = TerminalTool::new(&workspace_root);

        let output = execute_terminal_json(
            &tool,
            json!({
                "command": "echo spectacular-terminal-ok",
                "working_directory": "nested"
            }),
            Cancellation::default(),
        )
        .await;

        assert_eq!(output.exit_code, 0);
        assert_eq!(output.stdout.trim(), "spectacular-terminal-ok");

        remove_workspace(workspace_root).await;
    }

    #[tokio::test]
    async fn missing_or_empty_working_directory_runs_from_workspace_root() {
        let workspace_root = temp_workspace("terminal_default_working_directory").await;
        let tool = TerminalTool::new(&workspace_root);

        let missing_output = execute_terminal_json(
            &tool,
            json!({"command": redirect_echo_command("missing", "missing-wd.txt")}),
            Cancellation::default(),
        )
        .await;
        let empty_output = execute_terminal_json(
            &tool,
            json!({
                "command": redirect_echo_command("empty", "empty-wd.txt"),
                "working_directory": ""
            }),
            Cancellation::default(),
        )
        .await;

        assert_eq!(missing_output.exit_code, 0);
        assert_eq!(empty_output.exit_code, 0);
        assert!(workspace_root.join("missing-wd.txt").is_file());
        assert!(workspace_root.join("empty-wd.txt").is_file());

        remove_workspace(workspace_root).await;
    }

    #[tokio::test]
    async fn timeout_returns_negative_exit_code_and_stderr_message() {
        let workspace_root = temp_workspace("terminal_timeout").await;
        let tool = TerminalTool::new(&workspace_root);

        let output = execute_terminal_json(
            &tool,
            json!({
                "command": sleep_command(),
                "timeout_ms": 50
            }),
            Cancellation::default(),
        )
        .await;

        assert_eq!(output.exit_code, -1);
        assert!(output.stderr.contains("Command timed out after 50ms"));

        remove_workspace(workspace_root).await;
    }

    #[test]
    fn timeout_above_maximum_is_clamped_without_waiting() {
        assert_eq!(
            effective_timeout_ms(Some(MAX_TIMEOUT_MS + 1)),
            MAX_TIMEOUT_MS
        );
        assert_eq!(effective_timeout_ms(None), DEFAULT_TIMEOUT_MS);
    }

    #[tokio::test]
    async fn cancellation_kills_direct_child_and_returns_cancelled_output() {
        let workspace_root = temp_workspace("terminal_cancellation").await;
        let tool = TerminalTool::new(&workspace_root);
        let cancellation = Cancellation::default();
        let cancellation_trigger = cancellation.clone();

        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(50)).await;
            cancellation_trigger.cancel();
        });
        let started = Instant::now();
        let output = execute_terminal_json(
            &tool,
            json!({
                "command": sleep_command(),
                "timeout_ms": MAX_TIMEOUT_MS
            }),
            cancellation,
        )
        .await;

        assert_eq!(output.exit_code, -1);
        assert!(output.stderr.contains("Command cancelled"));
        assert!(started.elapsed() < Duration::from_secs(2));

        remove_workspace(workspace_root).await;
    }

    #[tokio::test]
    async fn command_can_write_inside_workspace_root() {
        let workspace_root = temp_workspace("terminal_workspace_write").await;
        write_file(&workspace_root, "seed.txt", "seed").await;
        let tool = TerminalTool::new(&workspace_root);

        let output = execute_terminal_json(
            &tool,
            json!({"command": redirect_echo_command("created", "created-by-terminal.txt")}),
            Cancellation::default(),
        )
        .await;

        assert_eq!(output.exit_code, 0);
        assert!(workspace_root.join("created-by-terminal.txt").is_file());

        remove_workspace(workspace_root).await;
    }

    async fn execute_terminal_json(
        tool: &TerminalTool,
        arguments: Value,
        cancellation: Cancellation,
    ) -> TerminalOutput {
        let result = tool.execute(arguments, cancellation).await.unwrap();
        serde_json::from_str(&result).unwrap()
    }

    fn sleep_command() -> &'static str {
        if cfg!(windows) {
            "ping 127.0.0.1 -n 6 > NUL"
        } else {
            "sleep 5"
        }
    }

    fn redirect_echo_command(text: &str, file_name: &str) -> String {
        format!("echo {text} > {file_name}")
    }
}
