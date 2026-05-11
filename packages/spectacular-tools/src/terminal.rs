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

#[path = "terminal/io.rs"]
mod io;
#[path = "terminal/process.rs"]
mod process;
#[path = "terminal/shell.rs"]
mod shell;

use crate::display::tool_arg_tool_arg_line;
use crate::output_preview::preview_text;
use crate::path::resolve_workspace_path;
use io::{append_message, read_joined, spawn_reader};
use process::{configure_process_group, wait_for_completion, CommandCompletion};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use shell::ShellSpec;
use spectacular_agent::{Cancellation, Tool, ToolDisplay, ToolExecution, ToolManifest};
use std::path::{Path, PathBuf};
use std::process::Stdio;

pub const TERMINAL_TOOL_NAME: &str = "terminal";

const DEFAULT_TIMEOUT_MS: u64 = 120_000;
const MAX_TIMEOUT_MS: u64 = 600_000;
const CANCELLATION_POLL_MS: u64 = 25;
const OUTPUT_DRAIN_TIMEOUT_MS: u64 = 1_000;
const TERMINAL_TOOL_DESCRIPTION: &str =
    "Executes shell commands on the host machine. Returns stdout, stderr, and exit_code.";

#[derive(Clone, Debug)]
pub struct TerminalTool {
    workspace_root: PathBuf,
}

impl TerminalTool {
    /// Creates a terminal tool scoped to the provided workspace root.
    pub fn new(workspace_root: impl Into<PathBuf>) -> Self {
        Self {
            workspace_root: workspace_root.into(),
        }
    }
}

impl Tool for TerminalTool {
    /// Returns the stable tool name used for registration and dispatch.
    fn name(&self) -> &str {
        TERMINAL_TOOL_NAME
    }

    /// Builds the terminal tool manifest and JSON parameter schema.
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
                        "description": "Working directory for the command. Relative paths resolve against the workspace root; absolute paths and .. traversal are allowed intentionally."
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

    /// Formats terminal arguments as command and working-directory text.
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

    /// Formats terminal arguments as a styled renderer call line.
    fn format_call(&self, arguments: &Value) -> ToolDisplay {
        let command = arguments
            .get("command")
            .and_then(Value::as_str)
            .unwrap_or("<missing command>");
        let working_directory = arguments
            .get("working_directory")
            .and_then(Value::as_str)
            .filter(|value| !value.is_empty())
            .unwrap_or(".");
        tool_arg_tool_arg_line("Run", command, "in", working_directory)
    }

    /// Formats terminal output as a bounded combined stdout/stderr preview.
    fn format_output(&self, raw_output: &str, parsed_output: Option<&Value>) -> ToolDisplay {
        let Some(output) = parsed_output else {
            return preview_text(raw_output);
        };

        preview_text(&terminal_output_text(output))
    }

    /// Executes the shell command and serializes the terminal output payload.
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

/// Runs one terminal command with shell detection, timeout, cancellation, and output capture.
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
    let output_drain_timeout = std::time::Duration::from_millis(OUTPUT_DRAIN_TIMEOUT_MS);
    let stdout = read_joined(stdout_reader, output_drain_timeout).await;
    let stderr = read_joined(stderr_reader, output_drain_timeout).await;

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

/// Resolves an optional working-directory argument against the workspace root.
fn resolve_working_directory(workspace_root: &Path, working_directory: Option<String>) -> PathBuf {
    match working_directory
        .as_deref()
        .filter(|value| !value.is_empty())
    {
        Some(path) => resolve_workspace_path(workspace_root, path),
        None => workspace_root.to_path_buf(),
    }
}

/// Applies default and maximum timeout policy to a requested timeout.
fn effective_timeout_ms(timeout_ms: Option<u64>) -> u64 {
    timeout_ms.unwrap_or(DEFAULT_TIMEOUT_MS).min(MAX_TIMEOUT_MS)
}

/// Serializes a failed terminal output payload with stderr populated.
fn terminal_error(message: impl Into<String>) -> String {
    serialize_output(&TerminalOutput {
        stdout: String::new(),
        stderr: message.into(),
        exit_code: -1,
    })
}

/// Serializes a terminal output payload to JSON.
fn serialize_output(output: &TerminalOutput) -> String {
    serde_json::to_string(output).expect("terminal output should serialize")
}

/// Joins command stdout and stderr so the visible preview shows actual process output.
fn terminal_output_text(output: &Value) -> String {
    let stdout = output.get("stdout").and_then(Value::as_str).unwrap_or("");
    let stderr = output.get("stderr").and_then(Value::as_str).unwrap_or("");

    match (stdout.is_empty(), stderr.is_empty()) {
        (false, false) => format!(
            "{stdout}{separator}{stderr}",
            separator = stream_separator(stdout)
        ),
        (false, true) => stdout.to_owned(),
        (true, false) => stderr.to_owned(),
        (true, true) => output.to_string(),
    }
}

/// Returns a line separator only when stdout does not already end with one.
fn stream_separator(stdout: &str) -> &'static str {
    if stdout.ends_with('\n') || stdout.ends_with('\r') {
        return "";
    }

    "\n"
}

#[cfg(test)]
mod tests {
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/unit/terminal.rs"
    ));
}
