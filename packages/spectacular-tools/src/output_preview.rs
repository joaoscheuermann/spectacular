const DEFAULT_PREVIEW_LINE_LIMIT: usize = 5;

/// Line collection paired with the complete source line count.
pub(crate) struct PreviewLines {
    lines: Vec<String>,
    total_lines: usize,
}

impl PreviewLines {
    /// Creates a preview-line collection from already selected display lines.
    pub(crate) fn new(lines: Vec<String>, total_lines: usize) -> Self {
        Self { lines, total_lines }
    }
}

/// Formats raw text as a line-limited command-output preview.
pub(crate) fn preview_text(text: &str) -> String {
    preview_lines(text.lines().map(str::to_owned).collect())
}

/// Formats output lines with the default command-output preview limit.
pub(crate) fn preview_lines(lines: Vec<String>) -> String {
    let total_lines = lines.len();
    preview_lines_with_total(PreviewLines::new(lines, total_lines))
}

/// Formats output lines and reports omitted lines using a known total count.
pub(crate) fn preview_lines_with_total(preview: PreviewLines) -> String {
    if preview.lines.is_empty() {
        return "no output".to_owned();
    }

    let shown_count = preview.lines.len().min(DEFAULT_PREVIEW_LINE_LIMIT);
    let mut output = preview
        .lines
        .into_iter()
        .take(DEFAULT_PREVIEW_LINE_LIMIT)
        .collect::<Vec<_>>();
    if preview.total_lines > shown_count {
        output.push(format!("[truncated {} lines]", preview.total_lines - shown_count));
    }

    output.join("\n")
}
