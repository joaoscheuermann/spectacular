use crate::terminal_style::*;
use spectacular_terminal_ui::tool_style;

#[test]
fn paint_wraps_value_with_style_and_reset() {
    let rendered = paint(error_style(), "boom");

    assert!(rendered.contains("boom"));
    assert!(rendered.ends_with(&format!("{}", error_style().render_reset())));
}

#[test]
fn named_styles_use_expected_color_sequences() {
    assert!(paint(command_output_style(), "output").contains("\x1b[38;2;107;114;128m"));
    assert!(paint(tool_style(), "tool").contains("\x1b[1m\x1b[38;2;217;70;239m"));
    assert!(paint(user_style(), "user").contains("\x1b[38;2;34;197;94m"));
}

#[test]
fn semantic_styles_share_expected_base_styles() {
    assert_eq!(assistant_style(), text_style());
    assert_eq!(model_style(), text_style());
    assert_eq!(task_style(), tool_style());
}
