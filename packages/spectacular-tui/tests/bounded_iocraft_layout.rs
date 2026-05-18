use iocraft::prelude::*;
use spectacular_tui::{
    reduce, ChatTuiAction, DisplayMetadata, PromptState, ReasoningLevel, RuntimeSelection,
    SessionId, State, TranscriptItemId,
};

/// Builds a representative runtime selection for bounded layout tests.
fn runtime() -> RuntimeSelection {
    RuntimeSelection::new(
        "openai-compatible",
        "openrouter",
        "gpt-5.1",
        ReasoningLevel::High,
        Some(200_000),
    )
}

/// Builds display metadata for bounded layout tests.
fn display() -> DisplayMetadata {
    DisplayMetadata::new(
        "OpenRouter",
        "GPT 5.1",
        "high",
        "/workspace/spectacular",
        "session-123",
        None,
    )
}

/// Builds an initialized state with stable metadata.
fn state() -> State {
    State::new(SessionId::new("session-123"), runtime(), display())
}

/// Renders state through a fixed-height IOCraft container.
fn render_fixed_height(state: State, height: u32) -> String {
    element!(View(width: 100, height) { spectacular_tui::components::AppState(state) })
        .render(Some(100))
        .to_string()
}

/// Counts occurrences of a substring in rendered output.
fn occurrences(output: &str, needle: &str) -> usize {
    output.match_indices(needle).count()
}

/// Verifies transcript overflow is clipped by ScrollView without duplicating fixed rows.
#[test]
fn transcript_overflow_is_bounded_above_working_prompt_and_footer() {
    let mut state = state();
    for index in 0..20 {
        reduce(
            &mut state,
            ChatTuiAction::SubmitPrompt {
                id: TranscriptItemId::new(format!("prompt-{index}")),
                text: format!("submitted prompt {index}"),
            },
        );
    }
    reduce(&mut state, ChatTuiAction::AgentStarted);
    state.session.prompt = PromptState::from_text("draft prompt");

    let output = render_fixed_height(state, 8);

    assert_eq!(output.lines().count(), 8);
    assert!(output.contains("Working (CTRL + C to stop)"));
    assert_eq!(occurrences(&output, "Working (CTRL + C to stop)"), 1);
    assert_eq!(occurrences(&output, "> draft prompt"), 1);
    assert!(output.contains("/workspace/spectacular"));
    assert!(output.contains("GPT 5.1 (high)"));
    assert!(output.contains("submitted prompt 0"));
    assert!(!output.contains("submitted prompt 19"));
}

/// Verifies streaming updates stay in the bounded transcript region without duplicating footer rows.
#[test]
fn streaming_assistant_updates_remain_bounded_with_fixed_chrome() {
    let mut state = state();
    reduce(
        &mut state,
        ChatTuiAction::SubmitPrompt {
            id: TranscriptItemId::new("prompt-1"),
            text: "hello".to_string(),
        },
    );
    reduce(&mut state, ChatTuiAction::AgentStarted);
    let assistant_id = TranscriptItemId::new("assistant-1");
    reduce(
        &mut state,
        ChatTuiAction::MessageStarted {
            id: assistant_id.clone(),
        },
    );
    reduce(
        &mut state,
        ChatTuiAction::MessageDelta {
            id: assistant_id.clone(),
            text: "streaming assistant response".to_string(),
        },
    );
    reduce(
        &mut state,
        ChatTuiAction::AssistantRevealTick { id: assistant_id },
    );
    state.session.prompt = PromptState::from_text("draft prompt");

    let output = render_fixed_height(state, 6);

    assert_eq!(output.lines().count(), 6);
    assert_eq!(occurrences(&output, "Working (CTRL + C to stop)"), 1);
    assert_eq!(occurrences(&output, "> draft prompt"), 1);
    assert_eq!(occurrences(&output, "/workspace/spectacular"), 1);
    assert!(output.contains("streaming assistant response"));
}
