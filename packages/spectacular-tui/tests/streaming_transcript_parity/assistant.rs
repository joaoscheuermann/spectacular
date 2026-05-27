use super::*;

#[test]
fn render_state_to_string_when_assistant_delta_is_visible_immediately() {
    let mut state = state();
    let item_id = id("assistant-1");

    reduce(
        &mut state,
        ChatTuiAction::MessageStarted {
            id: item_id.clone(),
        },
    );
    reduce(
        &mut state,
        ChatTuiAction::MessageDelta {
            id: item_id,
            text: "streamed text".to_owned(),
        },
    );

    assert_eq!(assistant_text(&state), "streamed text");
}

/// Verifies assistant deltas append directly without reveal chunking.
#[test]
fn render_state_to_string_when_assistant_deltas_are_not_typewriter_chunked() {
    let mut state = state();
    let item_id = id("assistant-1");
    let text = format!("{}🙂{}", "a".repeat(30), "b".repeat(69));

    reduce(
        &mut state,
        ChatTuiAction::MessageStarted {
            id: item_id.clone(),
        },
    );
    reduce(
        &mut state,
        ChatTuiAction::MessageDelta {
            id: item_id,
            text: text.clone(),
        },
    );

    assert_eq!(assistant_text(&state), text);
}

/// Verifies visible text remains available after stream completion.
#[test]
fn render_state_to_string_when_assistant_text_remains_after_stream_finish() {
    let mut state = state();
    let item_id = id("assistant-1");

    reduce(
        &mut state,
        ChatTuiAction::MessageStarted {
            id: item_id.clone(),
        },
    );
    reduce(
        &mut state,
        ChatTuiAction::MessageDelta {
            id: item_id.clone(),
            text: "x".repeat(35),
        },
    );
    reduce(&mut state, ChatTuiAction::MessageFinished { id: item_id });

    assert_eq!(assistant_text(&state), "x".repeat(35));
}

/// Verifies assistant deltas do not advance the spinner frame.
#[test]
fn render_state_to_string_when_assistant_delta_does_not_advance_spinner() {
    let mut state = state();
    let item_id = id("assistant-1");
    let frame = state.spinner.current_frame().to_owned();

    reduce(
        &mut state,
        ChatTuiAction::MessageStarted {
            id: item_id.clone(),
        },
    );
    reduce(
        &mut state,
        ChatTuiAction::MessageDelta {
            id: item_id,
            text: "hello".to_owned(),
        },
    );

    assert_eq!(state.spinner.current_frame(), frame);
}

/// Verifies spinner ticks do not mutate visible assistant text.
#[test]
fn render_state_to_string_when_spinner_tick_does_not_mutate_assistant_text() {
    let mut state = state();
    let item_id = id("assistant-1");

    reduce(
        &mut state,
        ChatTuiAction::MessageStarted {
            id: item_id.clone(),
        },
    );
    reduce(
        &mut state,
        ChatTuiAction::MessageDelta {
            id: item_id,
            text: "visible".to_owned(),
        },
    );
    reduce(&mut state, ChatTuiAction::SpinnerTick);

    assert_eq!(assistant_text(&state), "visible");
}
