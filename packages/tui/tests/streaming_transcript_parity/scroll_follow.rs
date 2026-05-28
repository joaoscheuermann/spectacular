use super::*;

#[test]
fn render_state_to_string_when_scroll_follow_mode_tracks_bottom_on_new_output() {
    let mut state = state();
    reduce(
        &mut state,
        ChatTuiAction::Resize {
            width: 100,
            height: 3,
        },
    );
    tui::apply_view_action_to_state(
        &mut state,
        tui::ViewAction::Resize {
            width: 100,
            height: 3,
        },
    );
    reduce(
        &mut state,
        ChatTuiAction::MessageStarted {
            id: id("assistant-1"),
        },
    );
    reduce(
        &mut state,
        ChatTuiAction::MessageDelta {
            id: id("assistant-1"),
            text: "tail".to_owned(),
        },
    );

    assert_eq!(state.scroll.offset, 0);
    assert!(state.scroll.follow_tail);
    assert!(app_render_lines(&state)
        .iter()
        .any(|line| line.plain_text() == "tail"));
}

/// Verifies manual review mode is preserved while new output arrives.
#[test]
fn render_state_to_string_when_scroll_manual_mode_does_not_snap_on_new_output() {
    let mut state = state();
    reduce(
        &mut state,
        ChatTuiAction::Resize {
            width: 100,
            height: 2,
        },
    );
    tui::apply_view_action_to_state(
        &mut state,
        tui::ViewAction::Resize {
            width: 100,
            height: 2,
        },
    );
    for message in ["older 1", "older 2", "older 3"] {
        reduce(
            &mut state,
            ChatTuiAction::NoticeReported {
                message: message.to_owned(),
            },
        );
    }
    tui::apply_view_action_to_state(&mut state, tui::ViewAction::ScrollTranscript(1));
    let old_rows = tui::total_transcript_rows(&state);
    let mut view = tui::ViewState::from_state(&state);
    reduce(
        &mut state,
        ChatTuiAction::MessageStarted {
            id: id("assistant-1"),
        },
    );
    reduce(
        &mut state,
        ChatTuiAction::MessageDelta {
            id: id("assistant-1"),
            text: "tail".to_owned(),
        },
    );
    let new_rows = tui::total_transcript_rows(&state);
    tui::preserve_review_position_for_growth(&mut view, old_rows, new_rows);
    state = tui::materialize_state(&state, &view);

    assert_eq!(state.scroll.offset, 3);
    assert!(!state.scroll.follow_tail);
}
