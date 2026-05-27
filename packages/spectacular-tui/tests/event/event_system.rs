use super::*;

/// Verifies release events remain ignored instead of replaying key actions.
#[test]

fn apply_terminal_event_when_key_release_event_does_not_emit_effects() {
    let state = state();

    let mut event = KeyEvent::new(KeyEventKind::Release, KeyCode::Char('q'));

    event.modifiers = KeyModifiers::CONTROL;

    assert_eq!(effects(&state, TerminalEvent::Key(event)), Vec::new());
}

/// Verifies timer ticks are represented as explicit spinner actions at the documented cadence.

#[test]

fn apply_terminal_event_when_timer_tick_dispatches_spinner_tick_without_terminal_output() {
    assert_eq!(SPINNER_TICK_INTERVAL, Duration::from_millis(90));

    assert_eq!(
        spectacular_tui::timer_tick_effects(),
        vec![EventEffect::Action(Box::new(ChatTuiAction::SpinnerTick))]
    );
}

/// Verifies assistant deltas are reducer-visible without a separate reveal timer.

#[test]

fn apply_terminal_event_when_assistant_delta_is_visible_without_reveal_timer_effects() {
    let mut state = state();

    reduce(
        &mut state,
        ChatTuiAction::MessageStarted {
            id: TranscriptItemId::new("message-1"),
        },
    );

    reduce(
        &mut state,
        ChatTuiAction::MessageDelta {
            id: TranscriptItemId::new("message-1"),

            text: "visible".to_owned(),
        },
    );

    assert!(state.session.transcript.iter().any(|item| {
        matches!(

            &item.content,

            TranscriptItemContent::AssistantMessage(message) if message.text == "visible"

        )
    }));
}

/// Verifies transcript scroll input is view-owned for selection-aware viewport state.

#[test]

fn apply_terminal_event_when_transcript_scroll_input_emits_view_scroll_actions() {
    let state = state();

    assert_eq!(
        effects(
            &state,
            TerminalEvent::Key(KeyEvent::new(KeyEventKind::Press, KeyCode::PageUp))
        ),
        vec![EventEffect::ViewAction(Box::new(
            ViewAction::ScrollTranscript(1)
        ))]
    );

    assert_eq!(
        effects(
            &state,
            TerminalEvent::Key(KeyEvent::new(KeyEventKind::Press, KeyCode::PageDown))
        ),
        vec![EventEffect::ViewAction(Box::new(
            ViewAction::ScrollTranscript(-1)
        ))]
    );
}

/// Verifies slash-command suggestions render in the original terminal-flow shape.

#[test]

fn apply_terminal_event_when_slash_command_prompt_ui_uses_state_commands_for_suggestions() {
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
