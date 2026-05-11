const DEFAULT_PREVIEW_LINE_LIMIT: usize = 5;

/// Formats raw text as a line-limited command-output preview.
pub(crate) fn preview_text(text: &str) -> String {
    preview_lines(text.lines().map(str::to_owned).collect())
}

/// Formats output lines with the default command-output preview limit.
pub(crate) fn preview_lines(lines: Vec<String>) -> String {
    preview_lines_with_total(lines.clone(), lines.len())
}

/// Formats output lines and reports omitted lines using a known total count.
pub(crate) fn preview_lines_with_total(lines: Vec<String>, total_lines: usize) -> String {
    if lines.is_empty() {
        return "no output".to_owned();
    }

    let shown_count = lines.len().min(DEFAULT_PREVIEW_LINE_LIMIT);
    let mut output = lines
        .into_iter()
        .take(DEFAULT_PREVIEW_LINE_LIMIT)
        .collect::<Vec<_>>();
    if total_lines > shown_count {
        output.push(format!("[truncated {} lines]", total_lines - shown_count));
    }

    output.join("\n")
}
