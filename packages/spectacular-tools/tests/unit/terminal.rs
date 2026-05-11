use super::*;
use crate::test_support::{remove_workspace, temp_workspace, write_file};
use chrono::{TimeZone, Utc};
use serde_json::json;
use std::fs;
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

    let output = execute_terminal_compact(
        &tool,
        json!({
            "command": "echo spectacular-terminal-ok",
            "working_directory": "nested"
        }),
        Cancellation::default(),
    )
    .await;

    assert_eq!(output.exit_code, 0);
    assert!(output.success);
    assert_eq!(output.schema, compact::TERMINAL_COMPACT_SCHEMA);
    assert_eq!(
        output.stdout.head,
        vec!["spectacular-terminal-ok".to_owned()]
    );
    assert!(output.raw_output_ref.is_none());

    remove_workspace(workspace_root).await;
}

/// Verifies that missing or empty working-directory values default to the workspace root.
#[tokio::test]
async fn missing_or_empty_working_directory_runs_from_workspace_root() {
    let workspace_root = temp_workspace("terminal_default_working_directory").await;
    let tool = TerminalTool::new(&workspace_root);

    let missing_output = execute_terminal_compact(
        &tool,
        json!({"command": redirect_echo_command("missing", "missing-wd.txt")}),
        Cancellation::default(),
    )
    .await;
    let empty_output = execute_terminal_compact(
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
    assert!(missing_output.success);
    assert!(empty_output.success);
    assert!(workspace_root.join("missing-wd.txt").is_file());
    assert!(workspace_root.join("empty-wd.txt").is_file());

    remove_workspace(workspace_root).await;
}

/// Verifies that a timed-out command returns a negative exit code and timeout message.
#[tokio::test]
async fn timeout_returns_negative_exit_code_and_stderr_message() {
    let workspace_root = temp_workspace("terminal_timeout").await;
    let tool = TerminalTool::new(&workspace_root);

    let output = execute_terminal_compact(
        &tool,
        json!({
            "command": sleep_command(),
            "timeout_ms": 50
        }),
        Cancellation::default(),
    )
    .await;

    assert_eq!(output.exit_code, -1);
    assert!(!output.success);
    assert!(stream_text(&output.stderr).contains("Command timed out after 50ms"));

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
    let output = execute_terminal_compact(
        &tool,
        json!({
            "command": sleep_command(),
            "timeout_ms": MAX_TIMEOUT_MS
        }),
        cancellation,
    )
    .await;

    assert_eq!(output.exit_code, -1);
    assert!(!output.success);
    assert!(stream_text(&output.stderr).contains("Command cancelled"));
    assert!(started.elapsed() < Duration::from_secs(2));

    remove_workspace(workspace_root).await;
}

/// Verifies that commands can create files inside the configured workspace root.
#[tokio::test]
async fn command_can_write_inside_workspace_root() {
    let workspace_root = temp_workspace("terminal_workspace_write").await;
    write_file(&workspace_root, "seed.txt", "seed").await;
    let tool = TerminalTool::new(&workspace_root);

    let output = execute_terminal_compact(
        &tool,
        json!({"command": redirect_echo_command("created", "created-by-terminal.txt")}),
        Cancellation::default(),
    )
    .await;

    assert_eq!(output.exit_code, 0);
    assert!(workspace_root.join("created-by-terminal.txt").is_file());

    remove_workspace(workspace_root).await;
}

/// Verifies trace-enabled terminal execution writes exact raw output outside the compact payload.
#[tokio::test]
async fn trace_enabled_command_writes_raw_output_ref() {
    let workspace_root = temp_workspace("terminal_trace_enabled").await;
    let trace_root = workspace_root.join("tool-output");
    let tool = TerminalTool::with_trace_dir(&workspace_root, &trace_root);

    let output = execute_terminal_compact(
        &tool,
        json!({"command": "echo compact-trace-output"}),
        Cancellation::default(),
    )
    .await;

    let raw_output_ref = output
        .raw_output_ref
        .as_ref()
        .expect("trace-enabled execution should include a raw output reference");
    let trace = fs::read_to_string(raw_output_ref).unwrap();
    let trace_json: Value = serde_json::from_str(&trace).unwrap();

    assert_eq!(output.exit_code, 0);
    assert!(output.success);
    assert!(trace_json["stdout"]
        .as_str()
        .unwrap()
        .contains("compact-trace-output"));
    assert_eq!(trace_json["stderr"].as_str().unwrap(), "");

    remove_workspace(workspace_root).await;
}

/// Verifies short streams remain visible without head/tail truncation.
#[test]
fn compact_short_output_keeps_all_lines_without_truncation() {
    let output = compact_for("alpha\nbeta\n", "", 0);

    assert_eq!(output.stdout.head, vec!["alpha".to_owned(), "beta".to_owned()]);
    assert!(output.stdout.tail.is_empty());
    assert_eq!(output.stdout.omitted_lines, 0);
    assert!(!output.stdout.truncated);
    assert!(output.success);
}

/// Verifies long stdout uses head/tail compaction with omitted counts.
#[test]
fn compact_long_stdout_keeps_head_tail_and_omitted_counts() {
    let stdout = numbered_lines("stdout-line", 200);

    let output = compact_for(&stdout, "", 0);

    assert!(output.stdout.truncated);
    assert!(output.stdout.head.first().unwrap().starts_with("stdout-line-000"));
    assert!(output.stdout.tail.last().unwrap().starts_with("stdout-line-199"));
    assert_eq!(output.stdout.omitted_lines, 128);
    assert!(output.stdout.omitted_bytes > 0);
}

/// Verifies long stderr is compacted independently from stdout.
#[test]
fn compact_long_stderr_keeps_head_tail_and_omitted_counts() {
    let stderr = numbered_lines("stderr-line", 200);

    let output = compact_for("", &stderr, 101);

    assert!(output.stderr.truncated);
    assert!(output.stderr.head.first().unwrap().starts_with("stderr-line-000"));
    assert!(output.stderr.tail.last().unwrap().starts_with("stderr-line-199"));
    assert_eq!(output.stderr.omitted_lines, 128);
    assert!(!output.success);
}

/// Verifies long lines are capped without splitting UTF-8 characters.
#[test]
fn compact_caps_long_single_lines_at_utf8_boundary() {
    let output = compact_for(&"é".repeat(1_100), "", 0);

    let line = output.stdout.head.first().unwrap();
    assert!(line.starts_with('é'));
    assert!(line.contains("[line truncated"));
    assert!(output.stdout.truncated);
}

/// Verifies diagnostic extraction scans stdout and stderr.
#[test]
fn compact_extracts_diagnostics_from_both_streams() {
    let stdout = "running 1 test\ntest terminal_compacts ... FAILED\nfailures:\n";
    let stderr = "warning: unused import\nerror[E0308]: mismatched types\n";

    let output = compact_for(stdout, stderr, 101);
    let diagnostics = output
        .diagnostics
        .iter()
        .map(|diagnostic| (diagnostic.kind.as_str(), diagnostic.stream.as_str()))
        .collect::<Vec<_>>();

    assert!(diagnostics.contains(&("test_failure", "stdout")));
    assert!(diagnostics.contains(&("rust_error", "stderr")));
    assert!(diagnostics.contains(&("warning", "stderr")));
}

/// Verifies diagnostics retain nearby lines needed to understand failures.
#[test]
fn compact_diagnostics_include_context_window() {
    let stdout = [
        "running 1 test",
        "expected left == right",
        "left: 1",
        "right: 2",
        "test terminal_compacts ... FAILED",
        "failures:",
        "terminal_compacts",
        "test result: FAILED",
    ]
    .join("\n");

    let output = compact_for(&stdout, "", 101);
    let failure = output
        .diagnostics
        .iter()
        .find(|diagnostic| diagnostic.text == "test terminal_compacts ... FAILED")
        .expect("failure diagnostic should be extracted");

    assert_eq!(failure.line, 5);
    assert_eq!(
        failure
            .context
            .iter()
            .map(|line| (line.line, line.text.as_str()))
            .collect::<Vec<_>>(),
        vec![
            (3, "left: 1"),
            (4, "right: 2"),
            (5, "test terminal_compacts ... FAILED"),
            (6, "failures:"),
            (7, "terminal_compacts"),
        ]
    );
}

/// Verifies repeated diagnostics collapse while preserving repeat counts.
#[test]
fn compact_deduplicates_repeated_diagnostics_with_repeat_counts() {
    let stderr = [
        "error: repeated failure",
        "error: repeated failure",
        "error: repeated failure",
        "error: repeated failure",
    ]
    .join("\n");

    let output = compact_for("", &stderr, 1);

    assert_eq!(output.diagnostics.len(), 1);
    assert_eq!(output.diagnostics[0].repeat_count, Some(4));
}

/// Verifies trace writing uses date-based directories and preserves raw streams exactly.
#[test]
fn trace_store_writes_date_partitioned_raw_output() {
    let trace_root = unique_sync_temp_dir("terminal_trace_store");
    let execution = execution_for("raw stdout\n", "raw stderr\n", 101);
    let store = trace::TerminalTraceStore::new(&trace_root);

    let result = store.write(&execution);
    let written = result.written().expect("trace should be written");
    let trace_text = fs::read_to_string(&written.path).unwrap();
    let trace_json: Value = serde_json::from_str(&trace_text).unwrap();

    assert_eq!(written.path.parent().unwrap(), trace_root.join("2026-05-11"));
    assert_eq!(trace_json["stdout"], "raw stdout\n");
    assert_eq!(trace_json["stderr"], "raw stderr\n");
    assert_eq!(trace_json["session_id"], Value::Null);
    assert_eq!(trace_json["tool_call_id"], Value::Null);

    let _ = fs::remove_dir_all(trace_root);
}

/// Verifies compact provider payloads omit raw middle output while traces preserve it.
#[test]
fn compact_payload_omits_raw_middle_output_that_trace_preserves() {
    let trace_root = unique_sync_temp_dir("terminal_compact_trace_contract");
    let mut stdout = numbered_lines("before", 120);
    stdout.push_str("\nraw-middle-secret-line\n");
    stdout.push_str(&numbered_lines("after", 120));
    let execution = execution_for(&stdout, "", 0);

    let content = serialize_execution_output(&execution, Some(&trace_root));
    let payload: compact::CompactTerminalOutput = serde_json::from_str(&content).unwrap();
    let trace_ref = payload.raw_output_ref.clone().unwrap();
    let trace_text = fs::read_to_string(trace_ref).unwrap();

    assert!(!content.contains("raw-middle-secret-line"));
    assert!(trace_text.contains("raw-middle-secret-line"));
    assert!(payload.stdout.truncated);

    let _ = fs::remove_dir_all(trace_root);
}

/// Verifies trace failures remain non-fatal in compact terminal output.
#[test]
fn compact_trace_error_does_not_mark_successful_command_failed() {
    let execution = execution_for("ok\n", "", 0);
    let output = compact::compact_terminal_execution(
        &execution,
        compact::CompactTraceMetadata::failed("term_trace", "cannot write trace"),
    );

    assert!(output.success);
    assert_eq!(output.raw_output_ref, None);
    assert_eq!(output.trace_error.as_deref(), Some("cannot write trace"));
}

/// Verifies legacy raw terminal payloads still render as process output.
#[test]
fn terminal_format_output_keeps_legacy_raw_payload_support() {
    let tool = TerminalTool::new(PathBuf::from("workspace"));
    let output = json!({"stdout": "legacy stdout\n", "stderr": "legacy stderr\n", "exit_code": 0});

    let rendered = tool.format_output(&output.to_string(), Some(&output));

    assert!(rendered.contains("legacy stdout"));
    assert!(rendered.contains("legacy stderr"));
}

/// Verifies compact payload rendering shows status, diagnostics, and raw trace location.
#[test]
fn terminal_format_output_renders_compact_payload() {
    let tool = TerminalTool::new(PathBuf::from("workspace"));
    let mut output = compact_for(
        "running 1 test\ntest terminal_compacts ... FAILED\n",
        "error[E0308]: mismatched types\n",
        101,
    );
    output.raw_output_ref = Some(r"C:\tmp\trace.json".to_owned());
    let value = serde_json::to_value(&output).unwrap();

    let rendered = tool.format_output(&value.to_string(), Some(&value));

    assert!(rendered.contains("exit 101 in"));
    assert!(rendered.contains("diagnostics:"));
    assert!(rendered.contains("test terminal_compacts ... FAILED"));
    assert!(rendered.contains("raw output:"));
    assert!(rendered.contains(r"C:\tmp\trace.json"));
}

/// Verifies command summaries identify cargo commands behind environment prefixes.
#[test]
fn command_summary_parses_cargo_after_env_prefix() {
    let summary = reducers::CommandSummary::parse("RUSTFLAGS=-Dwarnings cargo test -p spectacular-tools");

    assert_eq!(summary.executable, "cargo");
    assert_eq!(summary.args.first().map(String::as_str), Some("test"));
}

/// Verifies command summaries identify Cargo after PowerShell environment assignments.
#[test]
fn command_summary_parses_cargo_after_powershell_env_prefix() {
    let summary =
        reducers::CommandSummary::parse("$env:RUSTFLAGS='-D warnings'; cargo test -p spectacular-tools");

    assert_eq!(summary.executable, "cargo");
    assert_eq!(summary.args.first().map(String::as_str), Some("test"));
}

/// Verifies injected reducers can enrich generic compact output through the reducer seam.
#[test]
fn injected_reducer_can_enrich_generic_compact_output() {
    struct FakeReducer;

    impl reducers::TerminalReducer for FakeReducer {
        /// Matches the synthetic command used to prove reducer injection works.
        fn matches(&self, command: &reducers::CommandSummary) -> bool {
            command.executable == "fake"
        }

        /// Adds a deterministic diagnostic while preserving the generic compact payload.
        fn reduce(
            &self,
            _execution: &TerminalExecution,
            mut generic: compact::CompactTerminalOutput,
        ) -> compact::CompactTerminalOutput {
            generic.diagnostics.push(diagnostics::Diagnostic {
                kind: "fake_reducer".to_owned(),
                stream: "stdout".to_owned(),
                line: 1,
                text: "fake reducer enriched output".to_owned(),
                context: Vec::new(),
                repeat_count: None,
            });
            generic
        }
    }

    let mut execution = execution_for("generic stdout\n", "", 0);
    execution.command = "fake test".to_owned();
    let generic = compact::compact_terminal_execution(&execution, compact::CompactTraceMetadata::none());

    let reduced = reducers::reduce_with_reducers(&execution, generic, &[&FakeReducer]);

    assert!(reduced
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.kind == "fake_reducer"));
}

/// Verifies the cargo reducer preserves failed Rust test names as diagnostics.
#[test]
fn cargo_reducer_preserves_failed_test_names() {
    let mut execution = execution_for(
        "running 1 test\ntest terminal_compacts_large_output ... FAILED\nfailures:\n",
        "",
        101,
    );
    execution.command = "cargo test -p spectacular-tools terminal".to_owned();
    let generic = compact::compact_terminal_execution(&execution, compact::CompactTraceMetadata::none());

    let reduced = reducers::reduce_terminal_output(&execution, generic);

    assert!(reduced.diagnostics.iter().any(|diagnostic| {
        diagnostic.kind == "cargo_test_failure"
            && diagnostic.text.contains("terminal_compacts_large_output")
    }));
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
