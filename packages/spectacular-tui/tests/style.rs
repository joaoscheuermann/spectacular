use anstyle::RgbColor;
use spectacular_tui::{
    diff_added_style, diff_removed_style, display_spans_from_ansi, paint, tool_arg_tool_arg_line,
    tool_line, DisplayLineStyle, DisplaySpan, TuiRgb, TuiSelectionColors,
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

/// Verifies missing TUI selection color values preserve the current defaults.
#[test]
fn tui_selection_colors_from_env_values_missing_values_use_defaults() {
    let colors = TuiSelectionColors::from_env_values(None, None);

    assert_eq!(colors, TuiSelectionColors::default());
    assert_eq!(colors.text, None);
    assert_eq!(colors.background, TuiRgb::new(179, 179, 179));
    assert_eq!(colors.selected_text(), TuiRgb::new(76, 76, 76));
    assert_eq!(colors.cursor, TuiRgb::new(255, 255, 255));
}

/// Verifies TUI selection color parsing accepts hash-prefixed uppercase RGB values.
#[test]
fn tui_selection_colors_from_env_values_hash_hex_applies_text_color() {
    let colors = TuiSelectionColors::from_env_values(Some("#A1B2C3"), None);

    assert_eq!(colors.text, Some(TuiRgb::new(161, 178, 195)));
    assert_eq!(colors.selected_text(), TuiRgb::new(161, 178, 195));
    assert_eq!(colors.background, TuiSelectionColors::default().background);
    assert_eq!(colors.cursor, TuiRgb::new(255, 255, 255));
}

/// Verifies TUI selection color parsing accepts bare lowercase RGB values.
#[test]
fn tui_selection_colors_from_env_values_lowercase_bare_hex_applies_background_color() {
    let colors = TuiSelectionColors::from_env_values(None, Some("0a1b2c"));

    assert_eq!(colors.text, TuiSelectionColors::default().text);
    assert_eq!(colors.background, TuiRgb::new(10, 27, 44));
    assert_eq!(colors.selected_text(), TuiRgb::new(245, 228, 211));
}

/// Verifies invalid TUI selection color values fall back independently.
#[test]
fn tui_selection_colors_from_env_values_invalid_hex_falls_back_independently() {
    let colors = TuiSelectionColors::from_env_values(Some("not-hex"), Some("#12345g"));

    assert_eq!(colors, TuiSelectionColors::default());
}

/// Verifies one valid TUI selection color override still applies when the other value is invalid.
#[test]
fn tui_selection_colors_from_env_values_partial_override_keeps_valid_color() {
    let colors = TuiSelectionColors::from_env_values(Some("#010203"), Some("#notok"));

    assert_eq!(colors.text, Some(TuiRgb::new(1, 2, 3)));
    assert_eq!(colors.background, TuiSelectionColors::default().background);
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
