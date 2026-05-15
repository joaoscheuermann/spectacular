use iocraft::prelude::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers, TerminalEvent};
use spectacular_tui::{
    reduce, ChatTuiAction, DisplayMetadata, ReasoningLevel, RuntimeSelection, SessionId, State,
    TranscriptItemContent, TranscriptItemId,
};

/// Builds runtime metadata for shell behavior tests.
fn runtime() -> RuntimeSelection {
    RuntimeSelection::new(
        "openai-compatible",
        "provider",
        "model",
        ReasoningLevel::Low,
        None,
    )
}

/// Builds display metadata for shell behavior tests.
fn display() -> DisplayMetadata {
    DisplayMetadata::new("provider", "model", "low", "/workspace", "session-1", None)
}

/// Builds the initial runtime shell state.
fn state() -> State {
    State::new(SessionId::new("session-1"), runtime(), display())
}

/// Builds a key terminal event with optional modifier flags.
fn key(code: KeyCode, modifiers: KeyModifiers) -> TerminalEvent {
    let mut event = KeyEvent::new(KeyEventKind::Press, code);
    event.modifiers = modifiers;
    TerminalEvent::Key(event)
}

/// Verifies the IOCraft shell emits submitted prompts to the runtime controller.
#[tokio::test]
async fn shell_submit_prompt_emits_runtime_intent() {
    let mut state = state();
    state.session.prompt.text = "hello runtime".to_owned();

    let (mut shell, mut intents) = spectacular_tui::RuntimeShell::new(state);
    shell.apply_terminal_event(key(KeyCode::Enter, KeyModifiers::empty()));

    assert_eq!(
        intents.recv().await,
        Some(spectacular_tui::RuntimeIntent::SubmitPrompt {
            id: TranscriptItemId::new("local-prompt-1"),
            text: "hello runtime".to_owned(),
        })
    );
}

/// Verifies controller actions reduce into shell state for rendered transcript data.
#[tokio::test]
async fn shell_reduces_controller_actions_into_transcript() {
    let (mut shell, _intents) = spectacular_tui::RuntimeShell::new(state());

    shell.apply_action(ChatTuiAction::SubmitPrompt {
        id: TranscriptItemId::new("prompt-1"),
        text: "hello".to_owned(),
    });
    shell.apply_action(ChatTuiAction::AgentStarted);
    shell.apply_action(ChatTuiAction::MessageStarted {
        id: TranscriptItemId::new("message-1"),
    });
    shell.apply_action(ChatTuiAction::MessageDelta {
        id: TranscriptItemId::new("message-1"),
        text: "hi".to_owned(),
    });
    shell.apply_action(ChatTuiAction::AgentFinished);

    let state = shell.state();
    assert_eq!(state.session.transcript.len(), 2);
    assert!(matches!(
        &state.session.transcript[1].content,
        TranscriptItemContent::AssistantMessage(message) if message.text == "hi"
    ));
}

/// Verifies shell cancellation emits runtime cancellation and keeps reducer semantics.
#[tokio::test]
async fn shell_cancel_run_emits_cancel_intent() {
    let mut state = state();
    reduce(&mut state, ChatTuiAction::AgentStarted);

    let (mut shell, mut intents) = spectacular_tui::RuntimeShell::new(state);
    shell.apply_terminal_event(key(KeyCode::Char('c'), KeyModifiers::CONTROL));

    assert_eq!(
        intents.recv().await,
        Some(spectacular_tui::RuntimeIntent::CancelRun)
    );
    assert_eq!(shell.state().status, spectacular_tui::Status::Cancelling);
}
