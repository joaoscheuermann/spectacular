use serde::{Deserialize, Serialize};

const DIAGNOSTIC_CONTEXT_RADIUS: usize = 2;

/// One diagnostic line extracted from terminal output for model-visible summaries.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub(crate) struct Diagnostic {
    pub kind: String,
    pub stream: String,
    pub line: usize,
    pub text: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub context: Vec<DiagnosticContextLine>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub repeat_count: Option<usize>,
}

/// One source line preserved around a matched diagnostic.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub(crate) struct DiagnosticContextLine {
    pub line: usize,
    pub text: String,
}

struct DiagnosticCandidate {
    diagnostic: Diagnostic,
    priority: u8,
}

/// Extracts deterministic diagnostics from stdout and stderr, capped by line count.
pub(crate) fn extract_diagnostics(
    stdout: &str,
    stderr: &str,
    line_limit: usize,
    max_line_chars: usize,
) -> Vec<Diagnostic> {
    let mut candidates = Vec::new();
    append_stream_diagnostics(&mut candidates, "stdout", stdout, max_line_chars);
    append_stream_diagnostics(&mut candidates, "stderr", stderr, max_line_chars);
    candidates.sort_by_key(|candidate| {
        (
            candidate.priority,
            candidate.diagnostic.stream.clone(),
            candidate.diagnostic.line,
        )
    });

    candidates
        .into_iter()
        .take(line_limit)
        .map(|candidate| candidate.diagnostic)
        .collect()
}

/// Appends diagnostics from one stream while deduplicating identical extracted lines.
fn append_stream_diagnostics(
    candidates: &mut Vec<DiagnosticCandidate>,
    stream: &str,
    text: &str,
    max_line_chars: usize,
) {
    let lines = text.lines().collect::<Vec<_>>();
    for (line_index, line) in lines.iter().enumerate() {
        let Some((kind, priority)) = classify_line(line) else {
            continue;
        };
        let diagnostic = Diagnostic {
            kind: kind.to_owned(),
            stream: stream.to_owned(),
            line: line_index + 1,
            text: cap_line(line, max_line_chars),
            context: diagnostic_context(&lines, line_index, max_line_chars),
            repeat_count: None,
        };
        merge_candidate(candidates, diagnostic, priority);
    }
}

/// Builds a bounded context window around one matched diagnostic line.
fn diagnostic_context(
    lines: &[&str],
    line_index: usize,
    max_line_chars: usize,
) -> Vec<DiagnosticContextLine> {
    let start = line_index.saturating_sub(DIAGNOSTIC_CONTEXT_RADIUS);
    let end = (line_index + DIAGNOSTIC_CONTEXT_RADIUS + 1).min(lines.len());

    lines[start..end]
        .iter()
        .enumerate()
        .map(|(offset, line)| DiagnosticContextLine {
            line: start + offset + 1,
            text: cap_line(line, max_line_chars),
        })
        .collect()
}

/// Merges an extracted diagnostic with an existing exact match when present.
fn merge_candidate(
    candidates: &mut Vec<DiagnosticCandidate>,
    diagnostic: Diagnostic,
    priority: u8,
) {
    if let Some(existing) = candidates.iter_mut().find(|candidate| {
        candidate.diagnostic.kind == diagnostic.kind
            && candidate.diagnostic.stream == diagnostic.stream
            && candidate.diagnostic.text == diagnostic.text
    }) {
        let next_count = existing.diagnostic.repeat_count.unwrap_or(1) + 1;
        existing.diagnostic.repeat_count = Some(next_count);
        return;
    }

    candidates.push(DiagnosticCandidate {
        diagnostic,
        priority,
    });
}

/// Classifies one terminal-output line into a diagnostic kind and display priority.
fn classify_line(line: &str) -> Option<(&'static str, u8)> {
    let lower = line.to_ascii_lowercase();

    if line.contains("error[E") {
        return Some(("rust_error", 0));
    }
    if lower.contains("test result: failed")
        || lower == "failures:"
        || lower.contains(" failures")
        || line.contains("FAILED")
    {
        return Some(("test_failure", 0));
    }
    if lower.contains("could not compile") || lower.contains("test failed") {
        return Some(("cargo_error", 0));
    }
    if lower.contains("panicked at") || lower.contains(" panicked") {
        return Some(("panic", 0));
    }
    if line.contains("Traceback") || line.contains("Exception") || line.contains("Caused by:") {
        return Some(("exception", 0));
    }
    if line.contains("error TS") || looks_like_typescript_error(line) {
        return Some(("typescript_error", 0));
    }
    if lower.contains("parsing error")
        || lower.contains("no-unused-vars")
        || lower.contains("problems")
    {
        return Some(("eslint_error", 1));
    }
    if lower.starts_with("error:")
        || lower.contains(" error:")
        || line.contains("ERROR")
        || line.contains("Error:")
    {
        return Some(("error", 1));
    }
    if lower.starts_with("warning:")
        || lower.contains(" warning:")
        || line.contains("WARN")
        || line.contains("Warning:")
    {
        return Some(("warning", 2));
    }

    None
}

/// Reports whether a line contains a simple `TSxxxx` TypeScript diagnostic token.
fn looks_like_typescript_error(line: &str) -> bool {
    line.as_bytes().windows(6).any(|window| {
        window[0] == b'T' && window[1] == b'S' && window[2..].iter().all(u8::is_ascii_digit)
    })
}

/// Caps a diagnostic line without splitting UTF-8 characters.
fn cap_line(line: &str, max_line_chars: usize) -> String {
    if line.chars().count() <= max_line_chars {
        return line.to_owned();
    }

    let prefix = line.chars().take(max_line_chars).collect::<String>();
    let omitted = line.chars().count() - max_line_chars;
    format!("{prefix} [line truncated; {omitted} chars omitted]")
}
