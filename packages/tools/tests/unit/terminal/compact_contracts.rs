use serde_json::Value;

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
    let output = compact_for(&"\u{00E9}".repeat(1_100), "", 0);

    let line = output.stdout.head.first().unwrap();
    assert!(line.starts_with('\u{00E9}'));
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
