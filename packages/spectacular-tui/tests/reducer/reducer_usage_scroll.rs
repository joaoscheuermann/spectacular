use super::*;
use spectacular_tui::{NoticeItem, Timestamp, TranscriptItem, UserPromptItem};

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

/// Verifies visual transcript clears preserve footer metadata for the active session.

#[test]

fn reduce_when_transcript_cleared_preserves_footer_session_and_usage_metadata() {
    let mut state = state_with_usage_metadata();
    state.session.transcript = vec![
        transcript_item(
            "prompt-1",
            3,
            TranscriptItemContent::UserPrompt(UserPromptItem::new("old prompt")),
        ),
        transcript_item(
            "notice-1",
            7,
            TranscriptItemContent::Notice(NoticeItem::new("old notice")),
        ),
    ];
    state.session.refresh_next_timestamp();

    reduce(&mut state, ChatTuiAction::TranscriptCleared);

    assert_eq!(state.session.id.as_str(), "session-1");
    assert_eq!(state.display.session_label, "session");
    assert_eq!(state.session.transcript, Vec::new());
    assert_eq!(state.session.next_timestamp, Timestamp::default());
    assert_eq!(state.session.context_usage, Some(context_usage()));
    assert_eq!(state.display.context_usage, Some(context_usage()));
    assert_eq!(state.session.turn_usage, Some(token_usage()));
    assert_eq!(state.display.turn_usage, Some(token_usage()));
    assert_eq!(state.session.total_usage, Some(token_usage()));
    assert_eq!(state.display.total_usage, Some(token_usage()));
}

/// Verifies session changes reset stale footer metadata from the prior session.

#[test]

fn reduce_when_session_changed_resets_footer_session_and_usage_metadata() {
    let mut state = state_with_usage_metadata();

    reduce(
        &mut state,
        ChatTuiAction::SessionChanged {
            id: SessionId::new("session-2"),
        },
    );

    assert_eq!(state.session.id.as_str(), "session-2");
    assert_eq!(state.display.session_label, "session-2");
    assert_eq!(state.session.context_usage, None);
    assert_eq!(state.session.turn_usage, None);
    assert_eq!(state.session.total_usage, None);
    assert_eq!(
        state.display.context_usage,
        ContextTokenUsage::default_for_window(Some(4096))
    );
    assert_eq!(state.display.turn_usage, None);
    assert_eq!(state.display.total_usage, None);
}

fn state_with_usage_metadata() -> State {
    let mut state = state();
    state.session.context_usage = Some(context_usage());
    state.display.context_usage = Some(context_usage());
    state.session.turn_usage = Some(token_usage());
    state.display.turn_usage = Some(token_usage());
    state.session.total_usage = Some(token_usage());
    state.display.total_usage = Some(token_usage());
    state
}

fn context_usage() -> ContextTokenUsage {
    ContextTokenUsage::new(100, Some(1000))
}

fn token_usage() -> TurnTokenUsage {
    TurnTokenUsage {
        input_tokens: 10,
        output_tokens: 20,
        total_tokens: 30,
        has_provider_metadata: true,
    }
}

fn transcript_item(id: &str, timestamp: u64, content: TranscriptItemContent) -> TranscriptItem {
    TranscriptItem::new(
        TranscriptItemId::new(id),
        Timestamp::new(timestamp),
        content,
    )
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
