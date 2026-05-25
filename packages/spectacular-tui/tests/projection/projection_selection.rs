use super::*;

/// Verifies the app-wide projection includes every fixed visible surface.
#[test]

fn selectable_projection_when_selectable_projection_with_prompt_surfaces_includes_visible_rows() {
    let mut state = state();

    state.status = Status::Running {
        activity: spectacular_tui::Activity::WaitingForModel,

        cancellable: true,
    };

    state.input_notice = Some("Copied selection".to_owned());

    state.session.prompt = PromptState::from_text("draft");

    reduce(
        &mut state,
        ChatTuiAction::Resize {
            width: 40,

            height: 8,
        },
    );

    let surfaces = SelectableProjection::for_state(&state)
        .rows()
        .iter()
        .map(|row| row.surface)
        .collect::<Vec<_>>();

    assert!(surfaces.contains(&SelectableSurface::Working));

    assert!(surfaces.contains(&SelectableSurface::InputNotice));

    assert!(surfaces.contains(&SelectableSurface::Prompt));

    assert!(surfaces.contains(&SelectableSurface::Footer));
}

/// Verifies empty prompt chrome and placeholder text cannot start rendered selection.

#[test]

fn selectable_projection_when_selectable_projection_empty_prompt_chrome_has_no_selectable_point() {
    let mut state = state();

    reduce(
        &mut state,
        ChatTuiAction::Resize {
            width: 80,

            height: 6,
        },
    );

    let projection = SelectableProjection::for_state(&state);

    let prompt_row = projection
        .rows()
        .iter()
        .find(|row| row.surface == SelectableSurface::Prompt)
        .expect("prompt row");

    assert!(prompt_row.text.starts_with(">  What"));

    assert!(projection
        .point_at(0, prompt_row.screen_row as u16)
        .is_none());

    assert!(projection
        .point_at(1, prompt_row.screen_row as u16)
        .is_none());

    assert!(projection
        .point_at(2, prompt_row.screen_row as u16)
        .is_none());

    assert!(projection
        .point_at(3, prompt_row.screen_row as u16)
        .is_none());
}

/// Verifies non-empty prompt marker cells cannot start rendered selection.

#[test]

fn selectable_projection_when_start_selection_at_non_empty_prompt_marker_columns_returns_none() {
    let mut state = state();

    state.session.prompt = PromptState::from_text("draft");

    reduce(
        &mut state,
        ChatTuiAction::Resize {
            width: 80,

            height: 6,
        },
    );

    assert!(spectacular_tui::selection::start_selection_at(&state, 0, 0).is_none());

    assert!(spectacular_tui::selection::start_selection_at(&state, 1, 0).is_none());

    assert!(spectacular_tui::selection::start_selection_at(&state, 2, 0).is_some());
}

/// Verifies dragging from prompt text across the marker copies only prompt text.

#[test]

fn selectable_projection_when_selected_text_prompt_drag_across_marker_copies_only_buffer_text() {
    let mut state = state();

    state.session.prompt = PromptState::from_text("draft");

    reduce(
        &mut state,
        ChatTuiAction::Resize {
            width: 80,

            height: 6,
        },
    );

    state.app_selection =
        spectacular_tui::selection::start_selection_at(&state, 4, 0).expect("prompt text start");

    state.app_selection =
        spectacular_tui::selection::drag_selection_to(&state, 0, 0).expect("prompt text drag");

    assert_eq!(selected_text(&state).as_deref(), Some("dr"));
}

/// Verifies dragging past prompt text excludes the trailing synthetic cursor cell.

#[test]

fn selectable_projection_when_selected_text_prompt_drag_past_text_excludes_synthetic_cursor_cell() {
    let mut state = state();

    state.session.prompt = PromptState::from_text("draft");

    reduce(
        &mut state,
        ChatTuiAction::Resize {
            width: 80,

            height: 6,
        },
    );

    state.app_selection =
        spectacular_tui::selection::start_selection_at(&state, 2, 0).expect("prompt text start");

    state.app_selection =
        spectacular_tui::selection::drag_selection_to(&state, 40, 0).expect("prompt text drag");

    assert_eq!(selected_text(&state).as_deref(), Some("draft"));
}

/// Verifies the prompt's trailing synthetic cursor cell cannot start rendered selection.

#[test]

fn selectable_projection_when_start_selection_at_prompt_synthetic_cursor_cell_returns_none() {
    let mut state = state();

    state.session.prompt = PromptState::from_text("draft");

    reduce(
        &mut state,
        ChatTuiAction::Resize {
            width: 20,

            height: 6,
        },
    );

    assert!(spectacular_tui::selection::start_selection_at(&state, 7, 0).is_none());

    assert!(spectacular_tui::selection::start_selection_at(&state, 8, 0).is_some());
}

/// Verifies projected but non-selectable prompt chrome does not request transcript autoscroll.

#[test]

fn selectable_projection_when_drag_selection_to_empty_prompt_chrome_returns_no_autoscroll_selection(
) {
    let mut state = state();

    state.session.transcript = vec![user_prompt_item("one")];

    reduce(
        &mut state,
        ChatTuiAction::Resize {
            width: 80,

            height: 6,
        },
    );

    let projection = SelectableProjection::for_state(&state);

    let transcript_row = projection
        .rows()
        .iter()
        .find(|row| row.surface == SelectableSurface::Transcript)
        .expect("transcript row")
        .screen_row as u16;

    let prompt_row = projection
        .rows()
        .iter()
        .find(|row| row.surface == SelectableSurface::Prompt)
        .expect("prompt row")
        .screen_row as u16;

    state.app_selection = spectacular_tui::selection::start_selection_at(&state, 0, transcript_row)
        .expect("transcript selection start");

    assert!(spectacular_tui::selection::drag_selection_to(&state, 0, prompt_row).is_none());
}

/// Verifies dragging over a projected non-transcript row extends selection without autoscroll.

#[test]

fn selectable_projection_when_drag_selection_to_projected_non_transcript_row_does_not_request_autoscroll(
) {
    let mut state = state();

    state.session.transcript = vec![user_prompt_item("one")];

    state.status = Status::Running {
        activity: spectacular_tui::Activity::WaitingForModel,

        cancellable: true,
    };

    reduce(
        &mut state,
        ChatTuiAction::Resize {
            width: 40,

            height: 8,
        },
    );

    let projection = SelectableProjection::for_state(&state);

    let transcript_row = projection
        .rows()
        .iter()
        .find(|row| row.surface == SelectableSurface::Transcript && !row.text.is_empty())
        .expect("transcript row")
        .screen_row as u16;

    let working_row = projection
        .rows()
        .iter()
        .find(|row| row.surface == SelectableSurface::Working && !row.text.is_empty())
        .expect("working row")
        .screen_row as u16;

    state.app_selection = spectacular_tui::selection::start_selection_at(&state, 0, transcript_row)
        .expect("transcript selection start");

    let selection = spectacular_tui::selection::drag_selection_to(&state, 30, working_row)
        .expect("working-row drag");

    assert_eq!(selection.viewport_edge, None);

    assert_eq!(
        selection.focus.expect("focus point").surface,
        SelectableSurface::Working
    );
}
