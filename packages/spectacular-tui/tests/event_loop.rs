use iocraft::prelude::{
    FullscreenMouseEvent, KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseEventKind,
    TerminalEvent,
};
use spectacular_tui::{
    reduce, tui_event_effects, ChatTuiAction, CommandDescriptor, DisplayMetadata, EventEffect,
    PromptState, ReasoningLevel, RuntimeSelection, SessionId, State, Status, TranscriptItemContent,
    TranscriptItemId, TUI_SPINNER_TICK_INTERVAL,
};
use std::time::Duration;

/// Builds a representative runtime selection for event-loop tests.
fn runtime() -> RuntimeSelection {
    RuntimeSelection::new(
        "openai-compatible",
        "provider",
        "model",
        ReasoningLevel::Low,
        Some(4096),
    )
}

/// Builds visible display metadata for event-loop tests.
fn display() -> DisplayMetadata {
    DisplayMetadata::new("provider", "model", "low", "/workspace", "session", None)
}

/// Builds an initialized state with stable metadata.
fn state() -> State {
    State::new(SessionId::new("session-1"), runtime(), display())
}

/// Builds a key terminal event with optional modifier flags.
fn key(code: KeyCode, modifiers: KeyModifiers) -> TerminalEvent {
    let mut event = KeyEvent::new(KeyEventKind::Press, code);
    event.modifiers = modifiers;
    TerminalEvent::Key(event)
}

/// Extracts the single action produced by one terminal event.
fn single_action(state: &State, event: TerminalEvent) -> ChatTuiAction {
    let effects = tui_event_effects(state, event);
    assert_eq!(effects.len(), 1);
    match effects.into_iter().next().unwrap() {
        EventEffect::Action(action) => action,
        EventEffect::RequestExit => panic!("expected action effect"),
    }
}

/// Verifies typed characters are translated into reducer-owned prompt state updates.
#[test]
fn typing_text_updates_prompt_state_through_prompt_changed() {
    let mut state = state();

    let action = single_action(&state, key(KeyCode::Char('h'), KeyModifiers::empty()));
    reduce(&mut state, action);
    let action = single_action(&state, key(KeyCode::Char('i'), KeyModifiers::empty()));
    reduce(&mut state, action);

    assert_eq!(state.session.prompt.text, "hi");
    assert_eq!(state.session.prompt.cursor, 2);
}

/// Verifies multiline editing, cursor movement, selection replacement, and paste insertion stay in prompt state.
#[test]
fn prompt_editing_supports_multiline_cursor_selection_and_paste_state() {
    let mut prompt = PromptState::from_text("hello");

    prompt.move_left(false);
    prompt.insert_text("!");
    prompt.move_to_end(false);
    prompt.move_to_start(true);
    assert_eq!(prompt.selection_range(), Some(0..6));
    prompt.insert_text("say ");
    prompt.insert_paste("a\r\nb");
    prompt.move_up(false);

    assert_eq!(prompt.text, "say a\nb");
    assert_eq!(prompt.cursor, 1);
    assert_eq!(prompt.selection_range(), None);
    assert_eq!(prompt.paste_burst.buffer, "a\nb");

    prompt.move_down(false);
    assert_eq!(prompt.cursor, prompt.text.len());
}

/// Verifies modified Enter and Shift navigation are represented as prompt edits instead of submission.
#[test]
fn multiline_enter_and_shift_navigation_update_prompt_state() {
    let mut state = state();
    state.session.prompt = PromptState::from_text("one");

    let action = single_action(&state, key(KeyCode::Enter, KeyModifiers::SHIFT));
    reduce(&mut state, action);
    assert_eq!(state.session.prompt.text, "one\n");
    assert_eq!(state.session.transcript.len(), 0);

    let action = single_action(&state, key(KeyCode::Home, KeyModifiers::SHIFT));
    reduce(&mut state, action);
    assert_eq!(state.session.prompt.selection_range(), Some(0..4));
}

/// Verifies Enter submits the current prompt through the reducer and clears prompt state.
#[test]
fn enter_submits_prompt_appends_user_transcript_and_clears_prompt() {
    let mut state = state();
    state.session.prompt = PromptState::from_text("run this");

    let action = single_action(&state, key(KeyCode::Enter, KeyModifiers::empty()));
    reduce(&mut state, action);

    assert_eq!(state.session.prompt, PromptState::empty());
    assert_eq!(state.session.transcript.len(), 1);
    assert_eq!(
        state.session.transcript[0].id,
        TranscriptItemId::new("local-prompt-1")
    );
    assert!(matches!(
        &state.session.transcript[0].content,
        TranscriptItemContent::UserPrompt(item) if item.text == "run this"
    ));
}

/// Verifies Ctrl+C cancels only cancellable running states.
#[test]
fn ctrl_c_while_running_and_cancellable_dispatches_cancel() {
    let mut state = state();
    reduce(&mut state, ChatTuiAction::AgentStarted);

    let action = single_action(&state, key(KeyCode::Char('c'), KeyModifiers::CONTROL));
    reduce(&mut state, action);

    assert_eq!(state.status, Status::Cancelling);
}

/// Verifies Ctrl+C while idle requests outer-shell exit and leaves transcript unchanged.
#[test]
fn ctrl_c_while_idle_requests_exit_without_mutating_transcript() {
    let state = state();

    let effects = tui_event_effects(&state, key(KeyCode::Char('c'), KeyModifiers::CONTROL));

    assert_eq!(effects, vec![EventEffect::RequestExit]);
    assert!(state.session.transcript.is_empty());
}

/// Verifies timer ticks are represented as explicit spinner actions at the documented cadence.
#[test]
fn timer_tick_dispatches_spinner_tick_without_terminal_output() {
    let state = state();

    assert_eq!(TUI_SPINNER_TICK_INTERVAL, Duration::from_millis(90));
    assert_eq!(
        single_action(&state, TerminalEvent::Resize(80, 24)),
        ChatTuiAction::Resize {
            width: 80,
            height: 24,
        }
    );
    assert_eq!(
        spectacular_tui::tui_timer_tick_effects(),
        vec![EventEffect::Action(ChatTuiAction::SpinnerTick)]
    );
}

/// Verifies scroll input maps into transcript scrolling actions and reducer tail-following behavior.
#[test]
fn transcript_scroll_input_updates_scroll_state_and_tail_following() {
    let mut state = state();

    let action = single_action(
        &state,
        TerminalEvent::FullscreenMouse(FullscreenMouseEvent::new(MouseEventKind::ScrollUp, 0, 0)),
    );
    reduce(&mut state, action);

    assert_eq!(state.scroll.offset, 3);
    assert!(!state.scroll.follow_tail);

    let action = single_action(
        &state,
        TerminalEvent::FullscreenMouse(FullscreenMouseEvent::new(MouseEventKind::ScrollDown, 0, 0)),
    );
    reduce(&mut state, action);

    assert_eq!(state.scroll.offset, 0);
    assert!(state.scroll.follow_tail);
}

/// Verifies slash-command suggestions render in the original terminal-flow shape.
#[test]
fn slash_command_prompt_ui_uses_state_commands_for_suggestions() {
    let mut state = state();
    state.commands = vec![
        CommandDescriptor::with_usage("config", "Manage configuration", "/config list"),
        CommandDescriptor::new("session", "Manage sessions"),
    ];
    state.session.prompt = PromptState::from_text("/con");

    let output = spectacular_tui::render_state_to_string(&state, Some(100));

    assert!(output.contains("> /con"));
    assert!(output.contains("  /config            Manage configuration"));
    assert!(!output.contains("/session           Manage sessions"));
    assert!(!output.contains("Completions:"));
    assert!(!output.contains("Guidance:"));

    state.session.prompt = PromptState::from_text("/config ");
    let output = spectacular_tui::render_state_to_string(&state, Some(100));

    assert!(output.contains("> /config"));
    assert!(output.contains("/config list"));
    assert!(!output.contains("Usage:"));
}
