use super::*;

/// Verifies Ctrl+C copies selected prompt text through the injected clipboard.
#[test]

fn ctrl_c_with_prompt_selection_copies_selection_to_clipboard() {
    let mut state = state();

    state.session.prompt = PromptState::from_text("copy me");

    state.session.prompt.move_to_start(true);

    let mut clipboard = FakeClipboard::default();

    let event_effects = effects_with_clipboard(
        &state,
        key(KeyCode::Char('c'), KeyModifiers::CONTROL),
        Some(&mut clipboard),
    );

    assert_eq!(clipboard.writes, vec!["copy me".to_owned()]);

    assert_eq!(event_effects.len(), 1);

    match event_effects.into_iter().next().unwrap() {
        EventEffect::Action(action) => {
            assert!(matches!(

                *action,

                ChatTuiAction::PromptChanged(prompt) if prompt.kill_buffer == "copy me"

            ));
        }

        EventEffect::ViewAction(_) => panic!("expected copy action"),

        EventEffect::RequestExit => panic!("expected copy action"),
    }
}

/// Verifies mouse drag builds an app-wide rendered-text selection.

#[test]

fn mouse_drag_selects_rendered_prompt_text() {
    let mut state = state();

    state.session.prompt = PromptState::from_text("select me");

    reduce(
        &mut state,
        ChatTuiAction::Resize {
            width: 80,

            height: 6,
        },
    );

    let action = single_view_action(&state, mouse(MouseEventKind::Down(MouseButton::Left), 2, 0));

    spectacular_tui::apply_view_action_to_state(&mut state, action);

    let action = single_view_action(&state, mouse(MouseEventKind::Drag(MouseButton::Left), 8, 0));

    spectacular_tui::apply_view_action_to_state(&mut state, action);

    let action = single_view_action(&state, mouse(MouseEventKind::Up(MouseButton::Left), 8, 0));

    spectacular_tui::apply_view_action_to_state(&mut state, action);

    assert_eq!(
        spectacular_tui::selected_text(&state).as_deref(),
        Some("select")
    );

    assert!(!state.app_selection.dragging);
}

/// Verifies Ctrl+C copies rendered selection before prompt-buffer selection.

#[test]

fn ctrl_c_with_rendered_selection_takes_priority_over_prompt_selection() {
    let mut state = state();

    state.session.prompt = PromptState::from_text("prompt buffer");

    state.session.prompt.select_all();

    reduce(
        &mut state,
        ChatTuiAction::Resize {
            width: 80,

            height: 6,
        },
    );

    state.app_selection =
        spectacular_tui::selection::start_selection_at(&state, 2, 0).expect("prompt point");

    state.app_selection =
        spectacular_tui::selection::drag_selection_to(&state, 8, 0).expect("prompt drag");

    let mut clipboard = FakeClipboard::default();

    let action = single_view_action_with_clipboard(
        &state,
        key(KeyCode::Char('c'), KeyModifiers::CONTROL),
        &mut clipboard,
    );

    assert_eq!(clipboard.writes, vec!["prompt".to_owned()]);

    assert_eq!(
        action,
        ViewAction::RenderedSelectionFeedbackReported {
            message: COPIED_SELECTION_NOTICE.to_owned(),
        }
    );

    spectacular_tui::apply_view_action_to_state(&mut state, action);

    assert_eq!(state.input_notice, None);

    assert_eq!(
        state.app_selection.copy_feedback,
        Some(COPIED_SELECTION_NOTICE.to_owned())
    );
}

/// Verifies rendered-selection copy failures stay on the prompt-adjacent input notice path.

#[test]

fn ctrl_c_with_rendered_selection_clipboard_write_failure_reports_input_notice() {
    let mut state = state();

    state.session.prompt = PromptState::from_text("copy me");

    reduce(
        &mut state,
        ChatTuiAction::Resize {
            width: 80,

            height: 6,
        },
    );

    state.app_selection =
        spectacular_tui::selection::start_selection_at(&state, 2, 0).expect("prompt point");

    state.app_selection =
        spectacular_tui::selection::drag_selection_to(&state, 6, 0).expect("prompt drag");

    let mut clipboard = FakeClipboard::failing_write();

    let action = single_action_with_clipboard(
        &state,
        key(KeyCode::Char('c'), KeyModifiers::CONTROL),
        &mut clipboard,
    );

    assert_eq!(
        action,
        ChatTuiAction::InputNoticeReported {
            message: "Clipboard write failed".to_owned(),
        }
    );

    reduce(&mut state, action);

    assert!(clipboard.writes.is_empty());

    assert_eq!(
        state.input_notice,
        Some("Clipboard write failed".to_owned())
    );

    assert_eq!(state.app_selection.copy_feedback, None);
}

/// Verifies Escape clears rendered selection before prompt or run handling.

#[test]

fn escape_with_rendered_selection_clears_selection() {
    let mut state = state();

    state.session.prompt = PromptState::from_text("clear me");

    reduce(
        &mut state,
        ChatTuiAction::Resize {
            width: 80,

            height: 6,
        },
    );

    state.app_selection =
        spectacular_tui::selection::start_selection_at(&state, 2, 0).expect("prompt point");

    state.app_selection =
        spectacular_tui::selection::drag_selection_to(&state, 7, 0).expect("prompt drag");

    let action = single_view_action(&state, key(KeyCode::Esc, KeyModifiers::empty()));

    assert_eq!(action, ViewAction::RenderedSelectionCleared);
}

/// Verifies dragging beyond the transcript viewport requests transcript auto-scroll.

#[test]

fn mouse_drag_beyond_transcript_viewport_requests_auto_scroll() {
    let mut state = state();

    for index in 0..6 {
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

    let transcript_row = SelectableProjection::for_state(&state)
        .rows()
        .iter()
        .find(|row| {
            row.surface == SelectableSurface::Transcript && !row.selectable_columns.is_empty()
        })
        .expect("selectable transcript row")
        .screen_row as u16;

    let action = single_view_action(
        &state,
        mouse(MouseEventKind::Down(MouseButton::Left), 0, transcript_row),
    );

    spectacular_tui::apply_view_action_to_state(&mut state, action);

    let action = single_view_action(
        &state,
        mouse(MouseEventKind::Drag(MouseButton::Left), 0, 20),
    );

    assert_eq!(
        action,
        ViewAction::RenderedSelectionAutoScroll(spectacular_tui::ViewportEdge::BelowTranscript)
    );
}
