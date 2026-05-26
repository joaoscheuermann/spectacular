use super::*;
use chrono::{TimeZone, Utc};
use serde_json::Value;
use std::fs;
use std::path::PathBuf;

mod compact_contracts {
    use super::*;
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/unit/terminal/compact_contracts.rs"
    ));
}

mod display_contracts {
    use super::*;
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/unit/terminal/display_contracts.rs"
    ));
}

mod execution_contracts {
    use super::*;
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/unit/terminal/execution_contracts.rs"
    ));
}

/// Executes the terminal tool and deserializes JSON output for assertions.
async fn execute_terminal_compact(
    tool: &TerminalTool,
    arguments: Value,
    cancellation: Cancellation,
) -> compact::CompactTerminalOutput {
    let result = tool.execute(arguments, cancellation).await.unwrap();
    serde_json::from_str(&result).unwrap()
}

/// Builds a compact terminal output for pure compaction assertions.
fn compact_for(stdout: &str, stderr: &str, exit_code: i32) -> compact::CompactTerminalOutput {
    compact::compact_terminal_execution(
        &execution_for(stdout, stderr, exit_code),
        compact::CompactTraceMetadata::none(),
    )
}

/// Creates a deterministic terminal execution fixture for unit tests.
fn execution_for(stdout: &str, stderr: &str, exit_code: i32) -> TerminalExecution {
    let started_at = Utc.with_ymd_and_hms(2026, 5, 11, 17, 30, 12).unwrap();
    TerminalExecution {
        command: "test command".to_owned(),
        working_directory: PathBuf::from(r"C:\workspace"),
        started_at,
        completed_at: started_at,
        duration_ms: 15,
        stdout: stdout.to_owned(),
        stderr: stderr.to_owned(),
        exit_code,
    }
}

/// Returns all visible lines for a compact stream.
fn stream_text(stream: &compact::CompactStream) -> String {
    stream
        .head
        .iter()
        .chain(stream.tail.iter())
        .cloned()
        .collect::<Vec<_>>()
        .join("\n")
}

/// Builds enough fixed-width lines to exceed the default short-output byte limit.
fn numbered_lines(prefix: &str, count: usize) -> String {
    (0..count)
        .map(|index| format!("{prefix}-{index:03} {}", "x".repeat(80)))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Creates a unique synchronous temp directory for trace-store tests.
fn unique_sync_temp_dir(test_name: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "spectacular_tools_{test_name}_{}_{}",
        std::process::id(),
        Utc::now().timestamp_nanos_opt().unwrap()
    ));
    fs::create_dir_all(&path).unwrap();
    path
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
