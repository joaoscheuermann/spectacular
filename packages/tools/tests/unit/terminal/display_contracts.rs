use serde_json::json;

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

/// Verifies compact payload display uses the common UI preview without changing provider data.
#[test]
fn terminal_format_output_truncates_only_visible_stream_preview() {
    let tool = TerminalTool::new(PathBuf::from("workspace"));
    let output = compact_for(&numbered_lines("stdout-line", 200), "", 0);
    let value = serde_json::to_value(&output).unwrap();

    let rendered = tool.format_output(&value.to_string(), Some(&value));

    assert_eq!(output.stdout.head.len(), 24);
    assert_eq!(output.stdout.tail.len(), 48);
    assert!(rendered.contains("stdout-line-000"));
    assert!(rendered.contains("stdout-line-004"));
    assert!(!rendered.contains("stdout-line-005"));
    assert!(!rendered.contains("stdout-line-199"));
    assert!(rendered.contains("[truncated 195 lines]"));
}

/// Verifies command summaries identify cargo commands behind environment prefixes.
#[test]
fn command_summary_parses_cargo_after_env_prefix() {
    let summary =
        reducers::CommandSummary::parse("RUSTFLAGS=-Dwarnings cargo test -p tools");

    assert_eq!(summary.executable, "cargo");
    assert_eq!(summary.args.first().map(String::as_str), Some("test"));
}

/// Verifies command summaries identify Cargo after PowerShell environment assignments.
#[test]
fn command_summary_parses_cargo_after_powershell_env_prefix() {
    let summary =
        reducers::CommandSummary::parse("$env:RUSTFLAGS='-D warnings'; cargo test -p tools");

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
    let generic =
        compact::compact_terminal_execution(&execution, compact::CompactTraceMetadata::none());

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
    execution.command = "cargo test -p tools terminal".to_owned();
    let generic =
        compact::compact_terminal_execution(&execution, compact::CompactTraceMetadata::none());

    let reduced = reducers::reduce_terminal_output(&execution, generic);

    assert!(reduced.diagnostics.iter().any(|diagnostic| {
        diagnostic.kind == "cargo_test_failure"
            && diagnostic.text.contains("terminal_compacts_large_output")
    }));
}
