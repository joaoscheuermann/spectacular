use super::*;

/// Verifies copied text does not insert artificial line breaks for visual wrapping.
#[test]

fn selection_projection_when_selected_text_wrapped_transcript_row_preserves_source_line() {
    let mut state = state();

    state.session.transcript = vec![user_prompt_item("abcdef")];

    reduce(
        &mut state,
        ChatTuiAction::Resize {
            width: 6,

            height: 6,
        },
    );

    state.app_selection = spectacular_tui::selection::start_selection_at(&state, 1, 0).unwrap();

    state.app_selection = spectacular_tui::selection::drag_selection_to(&state, 1, 1).unwrap();

    assert_eq!(selected_text(&state).as_deref(), Some("bcdef"));
}

/// Verifies transcript rows can start rendered selection in trailing virtual whitespace.

#[test]

fn selection_projection_when_start_selection_at_transcript_virtual_whitespace_returns_selection() {
    let mut state = state();

    state.session.transcript = vec![user_prompt_item("short")];

    reduce(
        &mut state,
        ChatTuiAction::Resize {
            width: 20,

            height: 6,
        },
    );

    let selection = spectacular_tui::selection::start_selection_at(&state, 10, 0)
        .expect("virtual whitespace start");

    assert_eq!(selection.anchor.expect("anchor point").column, 10);
}

/// Verifies selecting only virtual whitespace is visual-only for copied text.

#[test]

fn selection_projection_when_selected_text_virtual_whitespace_only_omits_padding() {
    let mut state = state();

    state.session.transcript = vec![user_prompt_item("short")];

    reduce(
        &mut state,
        ChatTuiAction::Resize {
            width: 20,

            height: 6,
        },
    );

    state.app_selection =
        spectacular_tui::selection::start_selection_at(&state, 10, 0).expect("virtual start");

    state.app_selection =
        spectacular_tui::selection::drag_selection_to(&state, 15, 0).expect("virtual drag");

    assert!(state.app_selection.has_selection());

    assert_eq!(selected_text(&state).as_deref(), Some(""));
}

/// Verifies dragging from real transcript text into virtual whitespace copies only real text.

#[test]

fn selection_projection_when_selected_text_drag_from_text_to_virtual_whitespace_copies_real_text_only(
) {
    let mut state = state();

    state.session.transcript = vec![user_prompt_item("short")];

    reduce(
        &mut state,
        ChatTuiAction::Resize {
            width: 20,

            height: 6,
        },
    );

    state.app_selection =
        spectacular_tui::selection::start_selection_at(&state, 0, 0).expect("text start");

    state.app_selection =
        spectacular_tui::selection::drag_selection_to(&state, 12, 0).expect("virtual drag");

    assert_eq!(selected_text(&state).as_deref(), Some("short"));
}

/// Verifies blank separator rows can participate in visual rendered selection.

#[test]

fn selection_projection_when_start_selection_at_blank_separator_row_returns_selection() {
    let mut state = state();

    state.session.transcript = vec![user_prompt_item("one")];

    reduce(
        &mut state,
        ChatTuiAction::Resize {
            width: 20,

            height: 6,
        },
    );

    let separator_row = SelectableProjection::for_state(&state)
        .rows()
        .iter()
        .find(|row| row.surface == SelectableSurface::Transcript && row.text.is_empty())
        .expect("separator row")
        .screen_row as u16;

    state.app_selection = spectacular_tui::selection::start_selection_at(&state, 5, separator_row)
        .expect("separator start");

    state.app_selection = spectacular_tui::selection::drag_selection_to(&state, 10, separator_row)
        .expect("separator drag");

    assert!(state.app_selection.has_selection());

    assert_eq!(selected_text(&state).as_deref(), Some(""));
}

/// Verifies wide Unicode characters are selected by display-column overlap.

#[test]

fn selection_projection_when_selected_text_with_wide_unicode_copies_complete_character() {
    let mut state = state();

    state.session.transcript = vec![user_prompt_item("a\u{6f22}b")];

    reduce(
        &mut state,
        ChatTuiAction::Resize {
            width: 20,

            height: 6,
        },
    );

    state.app_selection = spectacular_tui::selection::start_selection_at(&state, 1, 0).unwrap();

    state.app_selection = spectacular_tui::selection::drag_selection_to(&state, 3, 0).unwrap();

    assert_eq!(selected_text(&state).as_deref(), Some("\u{6f22}"));
}

/// Verifies selected blank semantic rows remain blank lines in copied text.

#[test]

fn selection_projection_when_selected_text_includes_blank_rows_between_sources() {
    let mut state = state();

    state.session.transcript = vec![user_prompt_item("one"), user_prompt_item("two")];

    reduce(
        &mut state,
        ChatTuiAction::Resize {
            width: 20,

            height: 8,
        },
    );

    state.app_selection = spectacular_tui::selection::start_selection_at(&state, 0, 0).unwrap();

    state.app_selection = spectacular_tui::selection::drag_selection_to(&state, 3, 2).unwrap();

    assert_eq!(selected_text(&state).as_deref(), Some("one\n\ntwo"));
}

/// Verifies transcript selection remains copyable after the selected row scrolls away.
#[test]
fn selection_projection_when_selected_text_transcript_selection_survives_scroll_out_of_view() {
    let mut state = state();

    for index in 0..8 {
        reduce(
            &mut state,
            ChatTuiAction::SubmitPrompt {
                id: TranscriptItemId::new(format!("prompt-{index}")),
                text: format!("submitted prompt {index}"),
            },
        );
    }
    reduce(
        &mut state,
        ChatTuiAction::Resize {
            width: 80,
            height: 6,
        },
    );
    spectacular_tui::apply_view_action_to_state(
        &mut state,
        spectacular_tui::ViewAction::Resize {
            width: 80,
            height: 6,
        },
    );

    let tail_row = SelectableProjection::for_state(&state)
        .rows()
        .iter()
        .find(|row| row.text == "submitted prompt 7")
        .expect("tail prompt row")
        .screen_row as u16;

    state.app_selection =
        spectacular_tui::selection::start_selection_at(&state, 0, tail_row).unwrap();
    state.app_selection =
        spectacular_tui::selection::drag_selection_to(&state, 18, tail_row).unwrap();

    assert_eq!(selected_text(&state).as_deref(), Some("submitted prompt 7"));

    spectacular_tui::apply_view_action_to_state(
        &mut state,
        spectacular_tui::ViewAction::ScrollTranscript(4),
    );

    assert!(!SelectableProjection::for_state(&state)
        .rows()
        .iter()
        .any(|row| row.text == "submitted prompt 7"));
    assert_eq!(selected_text(&state).as_deref(), Some("submitted prompt 7"));
}

/// Verifies edge autoscroll extends rendered selection over stable transcript rows.
#[test]
fn selection_projection_when_rendered_selection_auto_scroll_extends_focus_over_stable_transcript_rows(
) {
    let mut state = state();

    for index in 0..8 {
        reduce(
            &mut state,
            ChatTuiAction::SubmitPrompt {
                id: TranscriptItemId::new(format!("prompt-{index}")),
                text: format!("submitted prompt {index}"),
            },
        );
    }
    reduce(
        &mut state,
        ChatTuiAction::Resize {
            width: 80,
            height: 6,
        },
    );
    spectacular_tui::apply_view_action_to_state(
        &mut state,
        spectacular_tui::ViewAction::Resize {
            width: 80,
            height: 6,
        },
    );
    spectacular_tui::apply_view_action_to_state(
        &mut state,
        spectacular_tui::ViewAction::ScrollTranscript(4),
    );

    let start_row = SelectableProjection::for_state(&state)
        .rows()
        .iter()
        .find(|row| row.text == "submitted prompt 5")
        .expect("reviewed prompt row")
        .screen_row as u16;

    state.app_selection =
        spectacular_tui::selection::start_selection_at(&state, 0, start_row).unwrap();
    let start = state.app_selection.focus.expect("start focus");
    state.app_selection = spectacular_tui::selection::drag_selection_to(&state, 0, 20)
        .expect("below-edge drag selection");
    assert_eq!(
        state.app_selection.viewport_edge,
        Some(spectacular_tui::ViewportEdge::BelowTranscript)
    );

    spectacular_tui::apply_view_action_to_state(
        &mut state,
        spectacular_tui::ViewAction::RenderedSelectionAutoScroll(
            spectacular_tui::ViewportEdge::BelowTranscript,
        ),
    );

    let focus = state.app_selection.focus.expect("autoscrolled focus");
    assert_eq!(focus.logical_row.surface, SelectableSurface::Transcript);
    assert!(focus.logical_row.index > start.logical_row.index);
    assert_eq!(
        selected_text(&state).as_deref(),
        Some("submitted prompt 5\n\nsubmitted prompt 6")
    );
}
