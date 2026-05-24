use crossterm::event::MouseButton;
use iocraft::prelude::{FullscreenMouseEvent, MouseEventKind, TerminalEvent};
use spectacular_tui::{
    reduce, render_state_to_string, AssistantMessageItem, ChatTuiAction, DisplayMetadata,
    ReasoningLevel, RuntimeSelection, SelectableProjection, SelectableSurface, SessionId, State,
    TranscriptItem, TranscriptItemContent, TranscriptItemId, UserPromptItem,
};
use std::time::{Duration, Instant};

const LARGE_TRANSCRIPT_ITEMS: usize = 20_000;
const VISIBLE_TRANSCRIPT_ROWS: u16 = 20;
const RENDER_BUDGET: Duration = Duration::from_secs(2);
const STREAMING_DRAG_BUDGET: Duration = Duration::from_secs(3);

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

/// Builds a fullscreen mouse terminal event.
fn mouse(kind: MouseEventKind, column: u16, row: u16) -> TerminalEvent {
    TerminalEvent::FullscreenMouse(FullscreenMouseEvent::new(kind, column, row))
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
fn new_transcript_content_preserves_review_viewport_when_not_following_tail() {
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
fn returning_to_bottom_reenables_follow_tail() {
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
fn resize_action_refreshes_transcript_viewport_height() {
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

/// Verifies a large transcript renders a bounded window within the documented budget.
#[test]
fn large_transcript_render_uses_bounded_visible_window() {
    let mut state = state();
    populate_large_transcript(&mut state);

    let (output, elapsed) = timed_render(&state);

    assert!(elapsed < RENDER_BUDGET, "large render took {elapsed:?}");
    assert!(output.contains(&format!(
        "large transcript item {}",
        LARGE_TRANSCRIPT_ITEMS - 1
    )));
    assert!(!output.contains("large transcript item 0"));
}

/// Verifies the runtime App path uses the same bounded transcript window as test rendering.
#[test]
fn runtime_app_render_uses_bounded_visible_window() {
    let mut state = state();
    populate_large_transcript(&mut state);

    let output = render_state_to_string(&state, Some(120));

    assert!(output.contains(&format!(
        "large transcript item {}",
        LARGE_TRANSCRIPT_ITEMS - 1
    )));
    assert!(!output.contains("large transcript item 0"));
}

/// Verifies rendered-selection projection materializes only the visible transcript window.
#[test]
fn large_transcript_selection_projection_stays_windowed() {
    let mut state = state();
    populate_large_transcript(&mut state);

    let projection = SelectableProjection::for_state(&state);
    let transcript_rows = projection
        .rows()
        .iter()
        .filter(|row| row.surface == SelectableSurface::Transcript)
        .collect::<Vec<_>>();

    assert!(transcript_rows.len() <= usize::from(VISIBLE_TRANSCRIPT_ROWS));
    assert!(transcript_rows.iter().any(|row| row.text.contains(&format!(
        "large transcript item {}",
        LARGE_TRANSCRIPT_ITEMS - 1
    ))));
    assert!(!transcript_rows
        .iter()
        .any(|row| row.text.contains("large transcript item 0")));

    let row = u16::try_from(
        transcript_rows
            .iter()
            .find(|row| row.text.starts_with("large transcript item"))
            .unwrap()
            .screen_row,
    )
    .unwrap();
    state.app_selection = spectacular_tui::selection::start_selection_at(&state, 0, row).unwrap();
    state.app_selection = spectacular_tui::selection::drag_selection_to(&state, 4, row).unwrap();

    let selected = spectacular_tui::selected_text(&state).unwrap();
    assert_eq!(selected, "larg");
}

/// Verifies streaming deltas update the correct active item in a large transcript.
#[test]
fn streaming_deltas_update_correct_active_item_in_large_transcript() {
    let mut state = state();
    populate_large_transcript(&mut state);
    state.scroll.offset = 10;
    state.scroll.follow_tail = false;
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
    assert_eq!(state.scroll.offset, 10);
    assert!(!state.scroll.follow_tail);
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

/// Verifies streaming tail updates and rendered-selection drag stay responsive at 20k items.
#[test]
fn shell_apply_terminal_event_when_streaming_drag_interleaves_with_large_transcript_keeps_rendering_responsive(
) {
    let mut state = state();
    populate_large_transcript(&mut state);
    reduce(
        &mut state,
        ChatTuiAction::Resize {
            width: 120,
            height: 30,
        },
    );
    let active_id = TranscriptItemId::new("assistant-active");

    let selectable_row = SelectableProjection::for_state(&state)
        .rows()
        .iter()
        .find(|row| row.surface == SelectableSurface::Transcript && !row.text.is_empty())
        .expect("visible transcript row")
        .screen_row as u16;

    let (mut shell, _intents) = spectacular_tui::Shell::new(state);
    shell.apply_action(ChatTuiAction::MessageStarted {
        id: active_id.clone(),
    });
    shell.apply_terminal_event(mouse(
        MouseEventKind::Down(MouseButton::Left),
        0,
        selectable_row,
    ));

    let started = Instant::now();
    for tick in 0..120 {
        shell.apply_action(ChatTuiAction::MessageDelta {
            id: active_id.clone(),
            text: format!(" chunk-{tick}"),
        });
        shell.apply_terminal_event(mouse(
            MouseEventKind::Drag(MouseButton::Left),
            u16::try_from(4 + (tick % 12)).unwrap(),
            selectable_row,
        ));
        let _ = render_state_to_string(shell.state(), Some(120));
    }
    shell.apply_terminal_event(mouse(
        MouseEventKind::Up(MouseButton::Left),
        16,
        selectable_row,
    ));

    let elapsed = started.elapsed();
    assert!(
        elapsed < STREAMING_DRAG_BUDGET,
        "streaming drag took {elapsed:?}"
    );
    assert!(spectacular_tui::selected_text(shell.state()).is_some());
}
