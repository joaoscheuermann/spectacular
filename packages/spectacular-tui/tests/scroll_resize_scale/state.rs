use super::*;

#[test]
fn scroll_state_when_scroll_state_follows_tail_by_default() {
    let state = state();

    assert_eq!(state.scroll.offset, 0);
    assert!(state.scroll.follow_tail);
}

/// Verifies scrolling up disables tail following and clamps at the oldest visible item.
#[test]
fn scroll_state_when_scrolling_up_disables_follow_tail_and_clamps_to_valid_range() {
    let mut state = state();
    populate_large_transcript(&mut state);

    spectacular_tui::apply_view_action_to_state(
        &mut state,
        spectacular_tui::ViewAction::ScrollTranscript(10),
    );

    assert_eq!(state.scroll.offset, 10);
    assert!(!state.scroll.follow_tail);

    spectacular_tui::apply_view_action_to_state(
        &mut state,
        spectacular_tui::ViewAction::ScrollTranscript(i32::MAX),
    );

    assert_eq!(
        state.scroll.offset,
        u32::try_from(LARGE_TRANSCRIPT_ITEMS.saturating_mul(2))
            .unwrap()
            .saturating_sub(u32::from(VISIBLE_TRANSCRIPT_ROWS))
    );
    assert!(!state.scroll.follow_tail);

    spectacular_tui::apply_view_action_to_state(
        &mut state,
        spectacular_tui::ViewAction::ScrollTranscript(10),
    );

    assert_eq!(
        state.scroll.offset,
        u32::try_from(LARGE_TRANSCRIPT_ITEMS.saturating_mul(2))
            .unwrap()
            .saturating_sub(u32::from(VISIBLE_TRANSCRIPT_ROWS))
    );
    assert!(!state.scroll.follow_tail);
}

/// Verifies new content does not yank a viewport while the user reviews older transcript items.
#[test]
fn scroll_state_when_not_following_tail_preserves_review_viewport_on_new_transcript_content() {
    let mut state = state();
    populate_large_transcript(&mut state);
    spectacular_tui::apply_view_action_to_state(
        &mut state,
        spectacular_tui::ViewAction::ScrollTranscript(10),
    );
    let before = render_state_to_string(&state, Some(120));
    let old_rows = spectacular_tui::total_transcript_rows(&state);
    let mut view = spectacular_tui::ViewState::from_state(&state);

    reduce(
        &mut state,
        ChatTuiAction::MessageStarted {
            id: TranscriptItemId::new("assistant-active"),
        },
    );
    reduce(
        &mut state,
        ChatTuiAction::MessageDelta {
            id: TranscriptItemId::new("assistant-active"),
            text: "streamed tail content".to_string(),
        },
    );
    let new_rows = spectacular_tui::total_transcript_rows(&state);
    spectacular_tui::preserve_review_position_for_growth(&mut view, old_rows, new_rows);
    state = spectacular_tui::materialize_state(&state, &view);
    let after = render_state_to_string(&state, Some(120));

    assert_eq!(state.scroll.offset, 12);
    assert!(!state.scroll.follow_tail);
    let reviewed_item = format!("large transcript item {}", LARGE_TRANSCRIPT_ITEMS - 15);
    assert!(before.contains(&reviewed_item));
    assert!(after.contains(&reviewed_item));
    assert!(!after.contains("streamed tail content"));
}

/// Verifies returning to the bottom re-enables tail following and shows streamed tail content.
#[test]
fn scroll_state_when_returning_to_bottom_reenables_follow_tail() {
    let mut state = state();
    populate_large_transcript(&mut state);
    spectacular_tui::apply_view_action_to_state(
        &mut state,
        spectacular_tui::ViewAction::ScrollTranscript(10),
    );

    spectacular_tui::apply_view_action_to_state(
        &mut state,
        spectacular_tui::ViewAction::ScrollTranscript(-10),
    );
    reduce(
        &mut state,
        ChatTuiAction::MessageStarted {
            id: TranscriptItemId::new("assistant-active"),
        },
    );
    reduce(
        &mut state,
        ChatTuiAction::MessageDelta {
            id: TranscriptItemId::new("assistant-active"),
            text: "streamed tail content".to_string(),
        },
    );
    let output = render_state_to_string(&state, Some(120));

    assert_eq!(state.scroll.offset, 0);
    assert!(state.scroll.follow_tail);
    assert!(output.contains("streamed tail content"));
}

/// Verifies resize actions refresh view-owned transcript viewport height.
#[test]
fn scroll_state_when_resize_action_refreshes_transcript_viewport_height() {
    let mut state = state();
    populate_large_transcript(&mut state);
    spectacular_tui::apply_view_action_to_state(
        &mut state,
        spectacular_tui::ViewAction::ScrollTranscript(10),
    );

    reduce(
        &mut state,
        ChatTuiAction::Resize {
            width: 100,
            height: 100,
        },
    );
    spectacular_tui::apply_view_action_to_state(
        &mut state,
        spectacular_tui::ViewAction::Resize {
            width: 100,
            height: 100,
        },
    );

    assert_eq!(state.scroll.visible_rows, 97);
    assert_eq!(state.scroll.offset, 10);
    assert!(!state.scroll.follow_tail);
}
