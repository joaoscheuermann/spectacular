use super::*;

/// Verifies footer copy feedback is centered in the gap between metadata and usage.
#[test]

fn footer_projection_when_footer_render_line_with_width_centers_copy_feedback_between_left_and_right(
) {
    let state = state_with_copied_selection_feedback(100);

    let line = footer_render_line_with_width(&state, 100);

    let text = line.plain_text();

    let left = footer_left_render_line(&state).plain_text();

    let right = footer_right_render_line(&state)
        .expect("state has context usage")
        .plain_text();

    let copied_start = text
        .find(COPIED_SELECTION_NOTICE)
        .expect("copy feedback is visible");

    let actual_start = UnicodeWidthStr::width(&text[..copied_start]);

    let expected_start = UnicodeWidthStr::width(left.as_str())
        + (100
            - UnicodeWidthStr::width(left.as_str())
            - UnicodeWidthStr::width(right.as_str())
            - UnicodeWidthStr::width(COPIED_SELECTION_NOTICE))
            / 2;

    assert!(text.starts_with(&left));

    assert!(text.ends_with(&right));

    assert_eq!(actual_start, expected_start);
}

/// Verifies narrow footer layouts hide only the center copy feedback.

#[test]

fn footer_projection_when_gap_is_too_narrow_hides_copy_feedback() {
    let state = state_with_copied_selection_feedback(100);

    let left = footer_left_render_line(&state).plain_text();

    let right = footer_right_render_line(&state)
        .expect("state has context usage")
        .plain_text();

    let narrow_width = UnicodeWidthStr::width(left.as_str())
        + UnicodeWidthStr::width(right.as_str())
        + UnicodeWidthStr::width(COPIED_SELECTION_NOTICE);

    let text = footer_text_with_width(&state, u16::try_from(narrow_width).unwrap());

    assert!(!text.contains(COPIED_SELECTION_NOTICE));

    assert_eq!(text, footer_render_line(&state).plain_text());

    assert!(text.contains(&left));

    assert!(text.contains(&right));
}

/// Verifies footer selection styling still applies when centered feedback is visible.

#[test]

fn footer_projection_when_style_line_for_footer_with_center_feedback_preserves_selected_columns() {
    let mut state = state_with_copied_selection_feedback(100);

    let footer_row = SelectableProjection::for_state(&state)
        .rows()
        .iter()
        .find(|row| row.surface == SelectableSurface::Footer)
        .expect("footer row")
        .clone();

    let copied_start = footer_row
        .text
        .find(COPIED_SELECTION_NOTICE)
        .expect("copy feedback is visible");

    let copied_column = u16::try_from(UnicodeWidthStr::width(&footer_row.text[..copied_start]))
        .expect("copy feedback column fits u16");

    let footer_screen_row = u16::try_from(footer_row.screen_row).unwrap();

    state.app_selection =
        spectacular_tui::selection::start_selection_at(&state, copied_column, footer_screen_row)
            .expect("footer selection start");

    state.app_selection =
        spectacular_tui::selection::drag_selection_to(&state, copied_column + 6, footer_screen_row)
            .expect("footer selection drag");

    state.app_selection.copy_feedback = Some(COPIED_SELECTION_NOTICE.to_owned());

    let styled = spectacular_tui::style_line_for_source(
        &state,
        footer_render_line_with_width(&state, 100),
        spectacular_tui::SelectableSource::Footer,
    );

    assert!(styled.spans.iter().any(|span| {
        span.style == RenderStyle::Dim
            && span.highlight == Some(RenderHighlight::Selection)
            && COPIED_SELECTION_NOTICE.starts_with(span.text.as_str())
    }));
}
