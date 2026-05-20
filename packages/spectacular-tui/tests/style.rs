use anstyle::RgbColor;
use spectacular_tui::{diff_added_style, diff_removed_style, paint};

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
