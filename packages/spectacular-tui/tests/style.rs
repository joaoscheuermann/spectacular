use anstyle::RgbColor;
use spectacular_tui::{
    diff_added_style, diff_removed_style, display_spans_from_ansi, paint, tool_arg_tool_arg_line,
    tool_line, DisplayLineStyle, DisplaySpan,
};

/// Verifies that diff additions color text without painting the full row background.
#[test]
fn diff_added_style_uses_green_text_on_default_background() {
    let rendered = paint(diff_added_style(), "added line");

    assert_eq!(
        rendered,
        paint(RgbColor(34, 197, 94).on_default(), "added line")
    );
    assert!(!rendered.contains("48;2"));
}

/// Verifies that diff removals color text without painting the full row background.
#[test]
fn diff_removed_style_uses_red_text_on_default_background() {
    let rendered = paint(diff_removed_style(), "removed line");

    assert_eq!(
        rendered,
        paint(RgbColor(248, 113, 113).on_default(), "removed line")
    );
    assert!(!rendered.contains("48;2"));
}

/// Verifies ANSI tool-line styles convert to semantic display spans for active TUI rendering.
#[test]
fn display_spans_from_ansi_tool_line_preserves_tool_text_and_metadata_styles() {
    let spans = display_spans_from_ansi(
        &tool_line("Write", "README.md", Some("(10 bytes)")),
        DisplayLineStyle::Text,
    );

    assert_eq!(
        spans,
        vec![
            DisplaySpan::new("Write", DisplayLineStyle::Tool),
            DisplaySpan::new(" ", DisplayLineStyle::Text),
            DisplaySpan::new("README.md", DisplayLineStyle::Text),
            DisplaySpan::new(" ", DisplayLineStyle::Text),
            DisplaySpan::new("(10 bytes)", DisplayLineStyle::Dim),
        ]
    );
}

/// Verifies two-label tool-call helpers retain label and argument style boundaries.
#[test]
fn display_spans_from_ansi_tool_arg_tool_arg_line_preserves_repeated_label_styles() {
    let spans = display_spans_from_ansi(
        &tool_arg_tool_arg_line("Run", "cargo test", "in", "packages/spectacular"),
        DisplayLineStyle::Text,
    );

    assert_eq!(
        spans,
        vec![
            DisplaySpan::new("Run", DisplayLineStyle::Tool),
            DisplaySpan::new(" ", DisplayLineStyle::Text),
            DisplaySpan::new("cargo test", DisplayLineStyle::Text),
            DisplaySpan::new(" ", DisplayLineStyle::Text),
            DisplaySpan::new("in", DisplayLineStyle::Tool),
            DisplaySpan::new(" ", DisplayLineStyle::Text),
            DisplaySpan::new("packages/spectacular", DisplayLineStyle::Text),
        ]
    );
}
