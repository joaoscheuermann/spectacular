use super::*;
use crate::test_support::{remove_workspace, temp_workspace, write_file};
use serde_json::json;
use std::time::Duration;
use tokio::io::AsyncWriteExt;
use tokio::time::Instant;

/// Verifies that a short shell command runs in a temporary workspace and returns stdout.
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

/// Verifies that missing or empty working-directory values default to the workspace root.
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

/// Verifies that a timed-out command returns a negative exit code and timeout message.
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

/// Verifies output capture does not wait forever when a process stream stays open.
#[tokio::test]
async fn output_reader_returns_when_stream_stays_open_after_timeout() {
    let (mut writer, reader) = tokio::io::duplex(64);
    let output_reader = io::spawn_reader(Some(reader));
    writer.write_all(b"partial output").await.unwrap();
    let started = Instant::now();

    let output = io::read_joined(output_reader, Duration::from_millis(50)).await;

    assert_eq!(output, "partial output");
    assert!(started.elapsed() < Duration::from_secs(1));
    drop(writer);
}

/// Verifies timeout clamping without running an external process.
#[test]
fn timeout_above_maximum_is_clamped_without_waiting() {
    assert_eq!(
        effective_timeout_ms(Some(MAX_TIMEOUT_MS + 1)),
        MAX_TIMEOUT_MS
    );
    assert_eq!(effective_timeout_ms(None), DEFAULT_TIMEOUT_MS);
}

/// Verifies that cancellation kills the direct child quickly and returns a cancelled output.
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

/// Verifies that commands can create files inside the configured workspace root.
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

/// Executes the terminal tool and deserializes JSON output for assertions.
async fn execute_terminal_json(
    tool: &TerminalTool,
    arguments: Value,
    cancellation: Cancellation,
) -> TerminalOutput {
    let result = tool.execute(arguments, cancellation).await.unwrap();
    serde_json::from_str(&result).unwrap()
}

/// Returns a platform-specific command that sleeps long enough for timeout tests.
fn sleep_command() -> &'static str {
    if cfg!(windows) {
        "ping 127.0.0.1 -n 6 > NUL"
    } else {
        "sleep 5"
    }
}

/// Returns a shell command that redirects echoed text to a file.
fn redirect_echo_command(text: &str, file_name: &str) -> String {
    format!("echo {text} > {file_name}")
}
