use super::*;

/// Verifies context usage updates both session usage and display metadata usage.
#[test]

fn reduce_when_context_usage_updated_updates_session_and_display_usage() {
    let mut state = state();

    let usage = ContextTokenUsage::new(100, Some(1000));

    reduce(&mut state, ChatTuiAction::ContextUsageUpdated(usage));

    assert_eq!(state.session.context_usage, Some(usage));

    assert_eq!(state.display.context_usage, Some(usage));
}

/// Verifies provider usage reports accumulate into active turn and session usage.

#[test]

fn reduce_when_provider_usage_reported_accumulates_turn_and_total_usage() {
    let mut state = state();

    reduce(
        &mut state,
        ChatTuiAction::ProviderUsageReported(ProviderUsageMetadata::new(Some(10), Some(20), None)),
    );

    reduce(
        &mut state,
        ChatTuiAction::ProviderUsageReported(ProviderUsageMetadata::new(
            Some(5),
            Some(15),
            Some(25),
        )),
    );

    let expected = Some(TurnTokenUsage {
        input_tokens: 15,

        output_tokens: 35,

        total_tokens: 55,

        has_provider_metadata: true,
    });

    assert_eq!(state.session.turn_usage, expected);

    assert_eq!(state.display.turn_usage, expected);

    assert_eq!(state.session.total_usage, expected);

    assert_eq!(state.display.total_usage, expected);
}

/// Verifies transcript scrolling updates offset and tail following rules.

#[test]

fn reduce_when_scroll_transcript_updates_offset_and_follow_tail() {
    let mut state = state();

    spectacular_tui::apply_view_action_to_state(
        &mut state,
        spectacular_tui::ViewAction::ScrollTranscript(3),
    );

    assert_eq!(state.scroll.offset, 3);

    assert!(!state.scroll.follow_tail);

    spectacular_tui::apply_view_action_to_state(
        &mut state,
        spectacular_tui::ViewAction::ScrollTranscript(-2),
    );

    assert_eq!(state.scroll.offset, 1);

    assert!(!state.scroll.follow_tail);

    spectacular_tui::apply_view_action_to_state(
        &mut state,
        spectacular_tui::ViewAction::ScrollTranscript(-5),
    );

    assert_eq!(state.scroll.offset, 0);

    assert!(state.scroll.follow_tail);
}

/// Verifies large scroll deltas saturate instead of wrapping the scroll offset.

#[test]

fn reduce_when_scroll_transcript_handles_large_deltas_without_overflow() {
    let mut state = state();

    spectacular_tui::apply_view_action_to_state(
        &mut state,
        spectacular_tui::ViewAction::ScrollTranscript(i32::MAX),
    );

    assert_eq!(state.scroll.offset, i32::MAX as u32);

    assert!(!state.scroll.follow_tail);

    spectacular_tui::apply_view_action_to_state(
        &mut state,
        spectacular_tui::ViewAction::ScrollTranscript(i32::MAX),
    );

    assert_eq!(state.scroll.offset, 4_294_967_294);

    assert!(!state.scroll.follow_tail);

    spectacular_tui::apply_view_action_to_state(
        &mut state,
        spectacular_tui::ViewAction::ScrollTranscript(i32::MIN),
    );

    assert_eq!(state.scroll.offset, 2_147_483_646);

    assert!(!state.scroll.follow_tail);
}
