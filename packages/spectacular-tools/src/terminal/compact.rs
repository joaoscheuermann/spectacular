use super::diagnostics::{extract_diagnostics, Diagnostic};
use super::TerminalExecution;
use crate::output_preview::{preview_lines_with_total, PreviewLines};
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub(crate) const TERMINAL_COMPACT_SCHEMA: &str = "terminal.compact.v1";

const SHORT_OUTPUT_BYTE_LIMIT: usize = 12_000;
const HEAD_LINE_LIMIT: usize = 24;
const TAIL_LINE_LIMIT: usize = 48;
const DIAGNOSTIC_LINE_LIMIT: usize = 80;
const MAX_LINE_CHARS: usize = 1_000;
const MAX_COMPACT_CHARS: usize = 16_000;
const REPEAT_COLLAPSE_THRESHOLD: usize = 3;

/// Trace fields copied into the provider-visible compact terminal payload.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct CompactTraceMetadata {
    trace_id: Option<String>,
    raw_output_ref: Option<String>,
    trace_error: Option<String>,
}

impl CompactTraceMetadata {
    /// Returns empty trace metadata for trace-disabled terminal tools.
    pub(crate) fn none() -> Self {
        Self::default()
    }

    /// Returns trace metadata for a successfully written raw-output artifact.
    pub(crate) fn written(trace_id: impl Into<String>, path: impl Into<String>) -> Self {
        Self {
            trace_id: Some(trace_id.into()),
            raw_output_ref: Some(path.into()),
            trace_error: None,
        }
    }

    /// Returns trace metadata for a failed raw-output artifact write.
    pub(crate) fn failed(trace_id: impl Into<String>, error: impl Into<String>) -> Self {
        Self {
            trace_id: Some(trace_id.into()),
            raw_output_ref: None,
            trace_error: Some(error.into()),
        }
    }
}

/// Provider-visible compact terminal output serialized as `ToolResult.content`.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub(crate) struct CompactTerminalOutput {
    pub schema: String,
    pub trace_id: Option<String>,
    pub command: String,
    pub working_directory: String,
    pub exit_code: i32,
    pub duration_ms: u128,
    pub success: bool,
    pub stdout: CompactStream,
    pub stderr: CompactStream,
    pub diagnostics: Vec<Diagnostic>,
    pub truncation: TruncationSummary,
    pub raw_output_ref: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trace_error: Option<String>,
}

/// Compact representation of one terminal stream.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub(crate) struct CompactStream {
    pub bytes: usize,
    pub lines: usize,
    pub head: Vec<String>,
    pub tail: Vec<String>,
    pub omitted_lines: usize,
    pub omitted_bytes: usize,
    pub truncated: bool,
}

/// Top-level truncation metadata for compact terminal output.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub(crate) struct TruncationSummary {
    pub truncated: bool,
    pub message: String,
}

#[derive(Clone, Copy, Debug)]
struct CompactPolicy {
    short_output_byte_limit: usize,
    head_line_limit: usize,
    tail_line_limit: usize,
    diagnostic_line_limit: usize,
    max_line_chars: usize,
    max_compact_chars: usize,
    repeat_collapse_threshold: usize,
}

impl Default for CompactPolicy {
    /// Returns the default deterministic terminal compaction policy.
    fn default() -> Self {
        Self {
            short_output_byte_limit: SHORT_OUTPUT_BYTE_LIMIT,
            head_line_limit: HEAD_LINE_LIMIT,
            tail_line_limit: TAIL_LINE_LIMIT,
            diagnostic_line_limit: DIAGNOSTIC_LINE_LIMIT,
            max_line_chars: MAX_LINE_CHARS,
            max_compact_chars: MAX_COMPACT_CHARS,
            repeat_collapse_threshold: REPEAT_COLLAPSE_THRESHOLD,
        }
    }
}

struct SelectedLines {
    lines: Vec<String>,
    omitted_lines: usize,
    omitted_bytes: usize,
    truncated: bool,
}

/// Builds the compact provider-visible terminal output for one execution.
pub(crate) fn compact_terminal_execution(
    execution: &TerminalExecution,
    trace: CompactTraceMetadata,
) -> CompactTerminalOutput {
    let policy = CompactPolicy::default();
    let stdout = compact_stream(&execution.stdout, &policy);
    let stderr = compact_stream(&execution.stderr, &policy);
    let diagnostics = extract_diagnostics(
        &execution.stdout,
        &execution.stderr,
        policy.diagnostic_line_limit,
        policy.max_line_chars,
    );
    let truncated = stdout.truncated || stderr.truncated;

    CompactTerminalOutput {
        schema: TERMINAL_COMPACT_SCHEMA.to_owned(),
        trace_id: trace.trace_id,
        command: execution.command.clone(),
        working_directory: execution.working_directory.to_string_lossy().into_owned(),
        exit_code: execution.exit_code,
        duration_ms: execution.duration_ms,
        success: execution.exit_code == 0,
        stdout,
        stderr,
        diagnostics,
        truncation: TruncationSummary {
            truncated,
            message: truncation_message(truncated),
        },
        raw_output_ref: trace.raw_output_ref,
        trace_error: trace.trace_error,
    }
}

/// Counts display lines in a terminal stream using Rust's Unicode-safe line splitting.
pub(crate) fn count_lines(text: &str) -> usize {
    text.lines().count()
}

/// Formats compact terminal output for human-visible transcript rendering.
pub(crate) fn format_compact_output(output: &Value) -> String {
    let Ok(output) = serde_json::from_value::<CompactTerminalOutput>(output.clone()) else {
        return output.to_string();
    };
    let mut lines = vec![format!(
        "exit {} in {}",
        output.exit_code,
        format_duration(output.duration_ms)
    )];
    lines.push(stream_summary("stdout", &output.stdout));
    lines.push(stream_summary("stderr", &output.stderr));
    append_diagnostics(&mut lines, &output.diagnostics);
    append_stream_blocks(&mut lines, "stdout", &output.stdout);
    append_stream_blocks(&mut lines, "stderr", &output.stderr);
    append_trace_lines(
        &mut lines,
        output.raw_output_ref.as_deref(),
        output.trace_error.as_deref(),
    );
    lines.join("\n")
}

/// Builds a compact stream summary with short-output and head-tail paths.
fn compact_stream(text: &str, policy: &CompactPolicy) -> CompactStream {
    let lines = text.lines().map(str::to_owned).collect::<Vec<_>>();
    let bytes = text.len();
    let line_count = count_lines(text);
    let block_budget = policy.max_compact_chars;

    if bytes <= policy.short_output_byte_limit {
        let selected = select_lines(&lines, policy, block_budget);
        return CompactStream {
            bytes,
            lines: line_count,
            head: selected.lines,
            tail: Vec::new(),
            omitted_lines: selected.omitted_lines,
            omitted_bytes: selected.omitted_bytes,
            truncated: selected.truncated,
        };
    }

    if line_count <= policy.head_line_limit + policy.tail_line_limit {
        let selected = select_lines(&lines, policy, block_budget);
        return CompactStream {
            bytes,
            lines: line_count,
            head: selected.lines,
            tail: Vec::new(),
            omitted_lines: selected.omitted_lines,
            omitted_bytes: selected.omitted_bytes,
            truncated: true,
        };
    }

    let tail_start = line_count - policy.tail_line_limit;
    let head = select_lines(
        &lines[..policy.head_line_limit],
        policy,
        policy.max_compact_chars / 2,
    );
    let tail = select_lines(&lines[tail_start..], policy, policy.max_compact_chars / 2);
    let omitted_middle = &lines[policy.head_line_limit..tail_start];

    CompactStream {
        bytes,
        lines: line_count,
        head: head.lines,
        tail: tail.lines,
        omitted_lines: omitted_middle.len() + head.omitted_lines + tail.omitted_lines,
        omitted_bytes: approximate_bytes(omitted_middle) + head.omitted_bytes + tail.omitted_bytes,
        truncated: true,
    }
}

/// Selects lines for display, capping long lines and collapsing repeated runs.
fn select_lines(lines: &[String], policy: &CompactPolicy, budget_chars: usize) -> SelectedLines {
    let mut selected = Vec::new();
    let mut omitted_lines = 0usize;
    let mut omitted_bytes = 0usize;
    let mut used_chars = 0usize;
    let mut index = 0usize;

    while index < lines.len() {
        let repeat_count = repeated_count(lines, index);
        let display_group = display_group(lines, index, repeat_count, policy);
        let group_chars = display_group
            .iter()
            .map(|line| line.text.chars().count())
            .sum::<usize>();

        if !selected.is_empty() && used_chars + group_chars > budget_chars {
            let remaining = &lines[index..];
            selected.push(format!(
                "[display budget reached; {} lines omitted]",
                remaining.len()
            ));
            omitted_lines += remaining.len();
            omitted_bytes += approximate_bytes(remaining);
            return SelectedLines {
                lines: selected,
                omitted_lines,
                omitted_bytes,
                truncated: true,
            };
        }

        used_chars += group_chars;
        omitted_bytes += display_group
            .iter()
            .map(|line| line.omitted_bytes)
            .sum::<usize>();

        if repeat_count >= policy.repeat_collapse_threshold {
            omitted_lines += repeat_count - 1;
            omitted_bytes += approximate_bytes(&lines[index + 1..index + repeat_count]);
        }

        selected.extend(display_group.into_iter().map(|line| line.text));
        index += repeat_count;
    }

    SelectedLines {
        lines: selected,
        omitted_lines,
        omitted_bytes,
        truncated: omitted_lines > 0 || omitted_bytes > 0,
    }
}

struct DisplayLine {
    text: String,
    omitted_bytes: usize,
}

/// Builds the display lines for one original line or one repeated-line run.
fn display_group(
    lines: &[String],
    index: usize,
    repeat_count: usize,
    policy: &CompactPolicy,
) -> Vec<DisplayLine> {
    if repeat_count < policy.repeat_collapse_threshold {
        return lines[index..index + repeat_count]
            .iter()
            .map(|line| cap_line(line, policy.max_line_chars))
            .collect();
    }

    let mut group = vec![cap_line(&lines[index], policy.max_line_chars)];
    group.push(DisplayLine {
        text: format!("[repeated {} more times]", repeat_count - 1),
        omitted_bytes: 0,
    });
    group
}

/// Counts consecutive identical lines starting at the supplied index.
fn repeated_count(lines: &[String], index: usize) -> usize {
    let mut count = 1usize;
    while index + count < lines.len() && lines[index + count] == lines[index] {
        count += 1;
    }
    count
}

/// Caps a display line without splitting UTF-8 characters and reports omitted bytes.
fn cap_line(line: &str, max_chars: usize) -> DisplayLine {
    if line.chars().count() <= max_chars {
        return DisplayLine {
            text: line.to_owned(),
            omitted_bytes: 0,
        };
    }

    let prefix = line.chars().take(max_chars).collect::<String>();
    let omitted_chars = line.chars().count() - max_chars;
    let omitted_bytes = line.len().saturating_sub(prefix.len());
    DisplayLine {
        text: format!("{prefix} [line truncated; {omitted_chars} chars omitted]"),
        omitted_bytes,
    }
}

/// Approximates bytes represented by omitted display lines, including line separators.
fn approximate_bytes(lines: &[String]) -> usize {
    lines.iter().map(|line| line.len() + 1).sum::<usize>()
}

/// Returns the human-readable truncation message for the compact payload.
fn truncation_message(truncated: bool) -> String {
    if truncated {
        return "terminal output compacted; full raw output is available at raw_output_ref"
            .to_owned();
    }

    "terminal output fits in compact payload".to_owned()
}

/// Formats a compact duration in milliseconds or seconds.
fn format_duration(duration_ms: u128) -> String {
    if duration_ms < 1_000 {
        return format!("{duration_ms}ms");
    }

    format!("{:.1}s", duration_ms as f64 / 1_000.0)
}

/// Formats one compact stream's size and truncation summary.
fn stream_summary(name: &str, stream: &CompactStream) -> String {
    if stream.bytes == 0 && stream.lines == 0 {
        return format!("{name}: no output");
    }

    let mut summary = format!("{name}: {} bytes, {} lines", stream.bytes, stream.lines);
    if stream.truncated {
        summary.push_str(", truncated");
    }
    summary
}

/// Appends diagnostic lines before generic stream excerpts.
fn append_diagnostics(lines: &mut Vec<String>, diagnostics: &[Diagnostic]) {
    if diagnostics.is_empty() {
        return;
    }

    lines.push(String::new());
    lines.push("diagnostics:".to_owned());
    for diagnostic in diagnostics {
        let repeat = diagnostic
            .repeat_count
            .map(|count| format!(" (repeated {count} times)"))
            .unwrap_or_default();
        lines.push(format!(
            "{}:{}:{}: {}{}",
            diagnostic.stream, diagnostic.line, diagnostic.kind, diagnostic.text, repeat
        ));
        append_diagnostic_context(lines, diagnostic);
    }
}

/// Appends surrounding diagnostic context without duplicating the headline line.
fn append_diagnostic_context(lines: &mut Vec<String>, diagnostic: &Diagnostic) {
    for context_line in &diagnostic.context {
        if context_line.line == diagnostic.line && context_line.text == diagnostic.text {
            continue;
        }

        lines.push(format!("  {}: {}", context_line.line, context_line.text));
    }
}

/// Appends a UI-only preview for one compact stream without changing provider content.
fn append_stream_blocks(lines: &mut Vec<String>, name: &str, stream: &CompactStream) {
    let visible_lines = stream
        .head
        .iter()
        .chain(stream.tail.iter())
        .cloned()
        .collect::<Vec<_>>();
    if visible_lines.is_empty() {
        return;
    }

    lines.push(String::new());
    lines.push(format!("{name}:"));
    lines.extend(
        preview_lines_with_total(PreviewLines::new(visible_lines, stream.lines))
            .lines()
            .map(str::to_owned),
    );
}

/// Appends trace file or trace error details to visible compact terminal output.
fn append_trace_lines(
    lines: &mut Vec<String>,
    raw_output_ref: Option<&str>,
    trace_error: Option<&str>,
) {
    if let Some(raw_output_ref) = raw_output_ref {
        lines.push(String::new());
        lines.push("raw output:".to_owned());
        lines.push(raw_output_ref.to_owned());
    }
    if let Some(trace_error) = trace_error {
        lines.push(String::new());
        lines.push(format!("trace error: {trace_error}"));
    }
}
