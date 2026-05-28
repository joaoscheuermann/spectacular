use super::*;
use crate::action::ChatTuiAction;
use crate::ids::{SessionId, TranscriptItemId};
use crate::metadata::{DisplayMetadata, ReasoningLevel, RuntimeSelection};
use crate::reducer::reduce;
use crate::selection::{SelectableProjection, SelectableSurface};
use crate::session::Session;

fn state() -> State {
    State::new(
        SessionId::new("layout-session-1"),
        RuntimeSelection::new(
            "openai-compatible",
            "provider",
            "model",
            ReasoningLevel::Low,
            Some(4096),
        ),
        DisplayMetadata::new("provider", "model", "low", "/workspace", "session", None),
    )
}

fn submit_prompt(state: &mut State, id: &str, text: &str) {
    reduce(
        state,
        ChatTuiAction::SubmitPrompt {
            id: TranscriptItemId::new(id),
            text: text.to_owned(),
        },
    );
}

#[test]
fn snapshot_for_state_when_item_appends_recomputes_suffix_rows() {
    let mut state = state();
    let mut cache = TranscriptLayoutCache::default();
    submit_prompt(&mut state, "prompt-1", "first");
    cache.note_change(0);
    let before = cache.snapshot_for_state(&state, 80);

    submit_prompt(&mut state, "prompt-2", "second");
    cache.note_change(1);
    let after = cache.snapshot_for_state(&state, 80);

    assert_eq!(before.layout.total_rows, 2);
    assert_eq!(after.layout.total_rows, 4);
    assert_eq!(after.layout.items[0], before.layout.items[0]);
    assert_eq!(after.layout.items[1].start_row, 2);
}

#[test]
fn snapshot_for_state_when_tail_mutates_recomputes_tail_rows() {
    let mut state = state();
    let mut cache = TranscriptLayoutCache::default();
    let id = TranscriptItemId::new("assistant-1");
    reduce(&mut state, ChatTuiAction::MessageStarted { id: id.clone() });
    cache.note_change(0);
    let before = cache.snapshot_for_state(&state, 12);

    reduce(
        &mut state,
        ChatTuiAction::MessageDelta {
            id,
            text: "alpha beta gamma delta epsilon".to_owned(),
        },
    );
    cache.note_change(0);
    let after = cache.snapshot_for_state(&state, 12);

    assert!(after.layout.total_rows > before.layout.total_rows);
    assert_eq!(after.layout.items.len(), before.layout.items.len());
}

#[test]
fn snapshot_for_state_when_width_changes_rebuilds_wrapped_rows() {
    let mut state = state();
    let mut cache = TranscriptLayoutCache::default();
    submit_prompt(
        &mut state,
        "prompt-1",
        "alpha beta gamma delta epsilon zeta eta theta",
    );
    cache.note_change(0);

    let wide = cache.snapshot_for_state(&state, 80);
    let narrow = cache.snapshot_for_state(&state, 12);

    assert!(narrow.layout.total_rows > wide.layout.total_rows);
}

#[test]
fn snapshot_for_state_when_scroll_window_changes_reuses_layout_for_projection() {
    let mut state = state();
    let mut cache = TranscriptLayoutCache::default();
    for index in 0..8 {
        submit_prompt(
            &mut state,
            &format!("prompt-{index}"),
            &format!("submitted prompt {index}"),
        );
        cache.note_change(index);
    }
    state.scroll.visible_rows = 4;
    let snapshot = cache.snapshot_for_state(&state, 80);

    let tail_projection = SelectableProjection::for_state_with_layout(&state, &snapshot.layout);
    state.scroll.offset = 8;
    state.scroll.follow_tail = false;
    let review_projection =
        SelectableProjection::for_state_with_layout(&state, &snapshot.layout);

    assert!(tail_projection.rows().iter().any(|row| {
        row.surface == SelectableSurface::Transcript && row.text.contains("submitted prompt 7")
    }));
    assert!(!review_projection.rows().iter().any(|row| {
        row.surface == SelectableSurface::Transcript && row.text.contains("submitted prompt 7")
    }));
}

#[test]
fn snapshot_for_state_when_session_changes_rebuilds_from_new_session() {
    let mut state = state();
    let mut cache = TranscriptLayoutCache::default();
    submit_prompt(&mut state, "prompt-1", "first");
    cache.note_change(0);
    let before = cache.snapshot_for_state(&state, 80);

    state.session = Session::new(SessionId::new("layout-session-2"));
    cache.reset_for_session();
    let after = cache.snapshot_for_state(&state, 80);

    assert_eq!(before.layout.total_rows, 2);
    assert_eq!(after.layout.total_rows, 0);
    assert_eq!(after.session_id, SessionId::new("layout-session-2"));
}
