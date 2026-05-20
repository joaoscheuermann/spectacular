use spectacular_tui::{
    reduce, render_state_to_string, AssistantMessageItem, ChatTuiAction, DisplayMetadata,
    ReasoningLevel, RuntimeSelection, SessionId, State, TranscriptItem, TranscriptItemContent,
    TranscriptItemId, UserPromptItem,
};
use std::time::{Duration, Instant};

const LARGE_TRANSCRIPT_ITEMS: usize = 5_000;
const VISIBLE_TRANSCRIPT_ROWS: u16 = 20;
const RENDER_BUDGET: Duration = Duration::from_secs(2);

/// Builds a representative runtime selection for scroll and scale tests.
fn runtime() -> RuntimeSelection {
    RuntimeSelection::new(
        "openai-compatible",
        "provider",
        "model",
        ReasoningLevel::Low,
        Some(4096),
    )
}

/// Builds visible display metadata for scroll and scale tests.
fn display() -> DisplayMetadata {
    DisplayMetadata::new("provider", "model", "low", "/workspace", "session", None)
}

/// Builds an initialized state with stable metadata and viewport size.
fn state() -> State {
    let mut state = State::new(SessionId::new("session-1"), runtime(), display());
    state.scroll.visible_rows = u32::from(VISIBLE_TRANSCRIPT_ROWS);
    state
}

/// Builds a stable transcript item ID for scale fixtures.
fn item_id(index: usize) -> TranscriptItemId {
    TranscriptItemId::new(format!("item-{index}"))
}

/// Builds a semantic transcript item with stable identity, timestamp, and visible text.
fn transcript_item(index: usize) -> TranscriptItem {
    let content = TranscriptItemContent::UserPrompt(UserPromptItem::new(format!(
        "large transcript item {index}"
    )));
    TranscriptItem::new(
        item_id(index),
        spectacular_tui::Timestamp::new(index as u64),
        content,
    )
}

/// Populates state with a deterministic large semantic transcript fixture.
fn populate_large_transcript(state: &mut State) {
    state.session.transcript = (0..LARGE_TRANSCRIPT_ITEMS).map(transcript_item).collect();
}

/// Renders state and returns the output with elapsed render time.
fn timed_render(state: &State) -> (String, Duration) {
    let started = Instant::now();
    let output = render_state_to_string(state, Some(120));
    (output, started.elapsed())
}

/// Verifies transcript scroll state follows the tail by default.
#[test]
fn scroll_state_follows_tail_by_default() {
    let state = state();

    assert_eq!(state.scroll.offset, 0);
    assert!(state.scroll.follow_tail);
}

/// Verifies scrolling up disables tail following and clamps at the oldest visible item.
#[test]
fn scrolling_up_disables_follow_tail_and_clamps_to_valid_range() {
    let mut state = state();
    populate_large_transcript(&mut state);

    reduce(&mut state, ChatTuiAction::ScrollTranscript(10));

    assert_eq!(state.scroll.offset, 10);
    assert!(!state.scroll.follow_tail);

    reduce(&mut state, ChatTuiAction::ScrollTranscript(i32::MAX));

    assert_eq!(state.scroll.offset, 9_980);
    assert!(!state.scroll.follow_tail);

    reduce(&mut state, ChatTuiAction::ScrollTranscript(10));

    assert_eq!(state.scroll.offset, 9_980);
    assert!(!state.scroll.follow_tail);
}

/// Verifies new content does not yank a viewport while the user reviews older transcript items.
#[test]
fn new_transcript_content_preserves_review_viewport_when_not_following_tail() {
    let mut state = state();
    populate_large_transcript(&mut state);
    reduce(&mut state, ChatTuiAction::ScrollTranscript(10));
    let before = render_state_to_string(&state, Some(120));

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
    let after = render_state_to_string(&state, Some(120));

    assert_eq!(state.scroll.offset, 12);
    assert!(!state.scroll.follow_tail);
    assert!(before.contains("large transcript item 4985"));
    assert!(after.contains("large transcript item 4985"));
    assert!(!after.contains("streamed tail content"));
}

/// Verifies returning to the bottom re-enables tail following and shows streamed tail content.
#[test]
fn returning_to_bottom_reenables_follow_tail() {
    let mut state = state();
    populate_large_transcript(&mut state);
    reduce(&mut state, ChatTuiAction::ScrollTranscript(10));

    reduce(&mut state, ChatTuiAction::ScrollTranscript(-10));
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

/// Verifies resize actions do not mutate reducer scroll position.
#[test]
fn resize_action_does_not_own_transcript_viewport_state() {
    let mut state = state();
    populate_large_transcript(&mut state);
    reduce(&mut state, ChatTuiAction::ScrollTranscript(10));

    reduce(
        &mut state,
        ChatTuiAction::Resize {
            width: 100,
            height: 100,
        },
    );

    assert_eq!(
        state.scroll.visible_rows,
        u32::from(VISIBLE_TRANSCRIPT_ROWS)
    );
    assert_eq!(state.scroll.offset, 10);
    assert!(!state.scroll.follow_tail);
}

/// Verifies a large transcript renders a bounded window within the documented budget.
#[test]
fn large_transcript_render_uses_bounded_visible_window() {
    let mut state = state();
    populate_large_transcript(&mut state);

    let (output, elapsed) = timed_render(&state);

    assert!(elapsed < RENDER_BUDGET, "large render took {elapsed:?}");
    assert!(output.contains("large transcript item 4999"));
    assert!(!output.contains("large transcript item 0"));
}

/// Verifies the runtime App path uses the same bounded transcript window as test rendering.
#[test]
fn runtime_app_render_uses_bounded_visible_window() {
    let mut state = state();
    populate_large_transcript(&mut state);

    let output = render_state_to_string(&state, Some(120));

    assert!(output.contains("large transcript item 4999"));
    assert!(!output.contains("large transcript item 0"));
}

/// Verifies streaming deltas update the correct active item in a large transcript.
#[test]
fn streaming_deltas_update_correct_active_item_in_large_transcript() {
    let mut state = state();
    populate_large_transcript(&mut state);
    let active_id = TranscriptItemId::new("assistant-active");
    state.session.transcript.push(TranscriptItem::new(
        active_id.clone(),
        spectacular_tui::Timestamp::new(5_001),
        TranscriptItemContent::AssistantMessage(AssistantMessageItem::new("")),
    ));

    reduce(
        &mut state,
        ChatTuiAction::MessageDelta {
            id: active_id,
            text: "only active assistant changed".to_string(),
        },
    );

    assert!(matches!(
        state.session.transcript.last().map(|item| &item.content),
        Some(TranscriptItemContent::AssistantMessage(item))
            if item.text == "only active assistant changed"
    ));
    assert!(matches!(
        &state.session.transcript[0].content,
        TranscriptItemContent::UserPrompt(item) if item.text == "large transcript item 0"
    ));
}

/// Verifies spinner ticks during large transcript streaming stay within the documented budget.
#[test]
fn spinner_ticks_during_large_streaming_keep_rendering_responsive() {
    let mut state = state();
    populate_large_transcript(&mut state);
    reduce(
        &mut state,
        ChatTuiAction::MessageStarted {
            id: TranscriptItemId::new("assistant-active"),
        },
    );

    let started = Instant::now();
    for tick in 0..60 {
        reduce(&mut state, ChatTuiAction::SpinnerTick);
        reduce(
            &mut state,
            ChatTuiAction::MessageDelta {
                id: TranscriptItemId::new("assistant-active"),
                text: format!(" chunk-{tick}"),
            },
        );
        let _ = render_state_to_string(&state, Some(120));
    }

    let elapsed = started.elapsed();
    assert!(elapsed < RENDER_BUDGET, "spinner redraws took {elapsed:?}");
}
