use iocraft::prelude::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers, TerminalEvent};
use spectacular_tui::{
    merge_controller_state_update, reduce, ChatTuiAction, DisplayMetadata, PromptState,
    ReasoningLevel, RuntimeSelection, SelectionPromptAnswer, SelectionPromptChoice,
    SelectionPromptState, SessionId, State, TranscriptItemContent, TranscriptItemId,
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

/// Verifies the shell stays movable across the async runtime controller task.
#[test]
fn shell_is_send_for_controller_runtime_task() {
    fn assert_send<T: Send>() {}

    assert_send::<spectacular_tui::Shell>();
}

/// Builds a key terminal event with optional modifier flags.
fn key(code: KeyCode, modifiers: KeyModifiers) -> TerminalEvent {
    let mut event = KeyEvent::new(KeyEventKind::Press, code);
    event.modifiers = modifiers;
    TerminalEvent::Key(event)
}

/// Converts pasted text into the key shape seen from terminals that report line breaks as Ctrl+Enter.
fn control_enter_paste_events(value: &str) -> Vec<TerminalEvent> {
    value
        .chars()
        .map(|character| match character {
            '\n' => key(KeyCode::Enter, KeyModifiers::CONTROL),
            character => key(KeyCode::Char(character), KeyModifiers::empty()),
        })
        .collect()
}

/// Verifies controller snapshots do not clobber local prompt input feedback.
#[test]
fn merge_controller_state_update_same_session_preserves_local_input_state() {
    let mut local = state();
    local.session.prompt = PromptState::from_text("local prompt");
    local.input_notice = Some("Use Ctrl+V to paste".to_owned());
    local.exit_requested = true;

    let mut controller = state();
    controller.session.prompt = PromptState::from_text("controller prompt");

    let (merged, reset_prompt) =
        merge_controller_state_update(&local, controller, &local.session.prompt);

    assert_eq!(reset_prompt, None);
    assert_eq!(merged.session.prompt.text, "local prompt");
    assert_eq!(merged.input_notice, Some("Use Ctrl+V to paste".to_owned()));
    assert!(merged.exit_requested);
}

/// Verifies the IOCraft shell emits submitted prompts to the runtime controller.
#[tokio::test]
async fn shell_ctrl_enter_submit_prompt_emits_runtime_intent() {
    let mut state = state();
    state.session.prompt = PromptState::from_text("hello runtime");

    let (mut shell, mut intents) = spectacular_tui::Shell::new(state);
    shell.apply_terminal_event(key(KeyCode::Enter, KeyModifiers::CONTROL));

    assert_eq!(
        intents.recv().await,
        Some(spectacular_tui::Intent::SubmitPrompt {
            id: TranscriptItemId::new("local-prompt-1"),
            text: "hello runtime".to_owned(),
        })
    );
}

/// Verifies bracketed paste updates local prompt text without emitting a submit intent.
#[tokio::test]
async fn shell_terminal_paste_inserts_multiline_prompt_without_submit_intent() {
    let (mut shell, mut intents) = spectacular_tui::Shell::new(state());
    let pasted = "mod app;\r\nmod footer;";

    shell.apply_terminal_event(TerminalEvent::Paste(pasted.to_owned()));

    assert_eq!(shell.state().session.prompt.text, "mod app;\nmod footer;");
    assert_eq!(
        intents.try_recv(),
        Err(tokio::sync::mpsc::error::TryRecvError::Empty)
    );
}

/// Verifies unbracketed multiline paste with Ctrl+Enter line breaks does not autosubmit.
#[tokio::test]
async fn shell_control_enter_key_stream_paste_preserves_user_lines_without_submit_intent() {
    let (mut shell, mut intents) = spectacular_tui::Shell::new(state());
    let pasted =
        "mod app;\nmod footer;\nmod input_notice;\nmod prompt;\nmod transcript;\nmod working;";

    for event in control_enter_paste_events(pasted) {
        shell.apply_terminal_event(event);
    }

    assert_eq!(shell.state().session.prompt.text, pasted);
    assert!(shell.state().session.transcript.is_empty());
    assert_eq!(
        intents.try_recv(),
        Err(tokio::sync::mpsc::error::TryRecvError::Empty)
    );
}

/// Verifies a final pasted newline is retained as text instead of submitting the prompt.
#[tokio::test]
async fn shell_trailing_control_enter_paste_newline_does_not_submit() {
    let (mut shell, mut intents) = spectacular_tui::Shell::new(state());
    let pasted = "mod app;\n";

    for event in control_enter_paste_events(pasted) {
        shell.apply_terminal_event(event);
    }

    assert_eq!(shell.state().session.prompt.text, pasted);
    assert!(shell.state().session.transcript.is_empty());
    assert_eq!(
        intents.try_recv(),
        Err(tokio::sync::mpsc::error::TryRecvError::Empty)
    );
}

/// Verifies controller actions reduce into shell state for rendered transcript data.
#[tokio::test]
async fn shell_reduces_controller_actions_into_transcript() {
    let (mut shell, _intents) = spectacular_tui::Shell::new(state());

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

    let (mut shell, mut intents) = spectacular_tui::Shell::new(state);
    shell.apply_terminal_event(key(KeyCode::Esc, KeyModifiers::empty()));

    assert_eq!(
        intents.recv().await,
        Some(spectacular_tui::Intent::CancelRun)
    );
    assert_eq!(shell.state().status, spectacular_tui::Status::Cancelling);
}

/// Verifies Ctrl+C no longer emits runtime cancellation while a run is active.
#[tokio::test]
async fn shell_ctrl_c_while_running_does_not_cancel() {
    let mut state = state();
    reduce(&mut state, ChatTuiAction::AgentStarted);

    let (mut shell, mut intents) = spectacular_tui::Shell::new(state);
    shell.apply_terminal_event(key(KeyCode::Char('c'), KeyModifiers::CONTROL));

    assert_eq!(
        intents.try_recv(),
        Err(tokio::sync::mpsc::error::TryRecvError::Empty)
    );
    assert!(matches!(
        shell.state().status,
        spectacular_tui::Status::Running { .. }
    ));
}

/// Verifies selection prompt answers are emitted to the runtime controller.
#[tokio::test]
async fn shell_selection_submit_emits_intent() {
    let mut state = state();
    state.selection = Some(
        SelectionPromptState::new("Pick one", "", vec!["alpha".to_owned()]).with_inputs(true, true),
    );

    let (mut shell, mut intents) = spectacular_tui::Shell::new(state);
    shell.apply_terminal_event(key(KeyCode::Char('x'), KeyModifiers::empty()));
    shell.apply_terminal_event(key(KeyCode::Tab, KeyModifiers::empty()));
    shell.apply_terminal_event(key(KeyCode::Char('!'), KeyModifiers::empty()));
    shell.apply_terminal_event(key(KeyCode::Enter, KeyModifiers::empty()));

    assert_eq!(
        intents.recv().await,
        Some(spectacular_tui::Intent::SelectionPromptSubmitted(
            SelectionPromptAnswer {
                choice: SelectionPromptChoice::Custom("x".to_owned()),
                comment: Some("!".to_owned()),
            }
        ))
    );
    assert!(shell.state().selection.is_none());
}

/// Verifies selection prompt paste-like Ctrl+Enter becomes editable space instead of submit.
#[tokio::test]
async fn shell_selection_prompt_control_enter_paste_inserts_space_without_submit() {
    let mut state = state();
    state.selection = Some(
        SelectionPromptState::new("Pick one", "", vec!["alpha".to_owned()]).with_inputs(true, true),
    );

    let (mut shell, mut intents) = spectacular_tui::Shell::new(state);
    shell.apply_terminal_event(key(KeyCode::Char('x'), KeyModifiers::empty()));
    shell.apply_terminal_event(key(KeyCode::Enter, KeyModifiers::CONTROL));
    shell.apply_terminal_event(key(KeyCode::Char('y'), KeyModifiers::empty()));

    let selection = shell
        .state()
        .selection
        .as_ref()
        .expect("selection prompt should remain active");
    assert_eq!(selection.custom_input, "x y");
    assert_eq!(
        intents.try_recv(),
        Err(tokio::sync::mpsc::error::TryRecvError::Empty)
    );
}

/// Verifies selection prompt cancellation is emitted to the runtime controller.
#[tokio::test]
async fn shell_selection_cancel_emits_intent() {
    let mut state = state();
    state.selection = Some(SelectionPromptState::new(
        "Pick one",
        "",
        vec!["alpha".to_owned()],
    ));

    let (mut shell, mut intents) = spectacular_tui::Shell::new(state);
    shell.apply_terminal_event(key(KeyCode::Esc, KeyModifiers::empty()));

    assert_eq!(
        intents.recv().await,
        Some(spectacular_tui::Intent::SelectionPromptCancelled)
    );
    assert!(shell.state().selection.is_none());
}
