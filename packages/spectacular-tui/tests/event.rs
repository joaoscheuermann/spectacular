use iocraft::prelude::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers, TerminalEvent};
use spectacular_tui::{
    effects, effects_with_clipboard, reduce, ChatTuiAction, ClipboardError, ClipboardService,
    CommandDescriptor, DisplayMetadata, EventEffect, PromptLayoutMetrics, PromptState,
    ReasoningLevel, RuntimeSelection, SelectionPromptState, SessionId, State, Status,
    TranscriptItemContent, TranscriptItemId, MAX_PASTE_BYTES, SPINNER_TICK_INTERVAL,
};
use std::collections::VecDeque;
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

#[derive(Default)]
struct FakeClipboard {
    reads: VecDeque<String>,
    writes: Vec<String>,
    read_error: bool,
    write_error: bool,
}

impl FakeClipboard {
    /// Creates a fake clipboard that returns one text value when pasted.
    fn with_text(text: &str) -> Self {
        Self {
            reads: VecDeque::from([text.to_owned()]),
            writes: Vec::new(),
            read_error: false,
            write_error: false,
        }
    }

    /// Creates a fake clipboard that fails when text is read.
    fn failing_read() -> Self {
        Self {
            read_error: true,
            ..Self::default()
        }
    }

    /// Creates a fake clipboard that fails when text is written.
    fn failing_write() -> Self {
        Self {
            write_error: true,
            ..Self::default()
        }
    }
}

impl ClipboardService for FakeClipboard {
    /// Reads the next queued fake clipboard value.
    fn get_text(&mut self) -> Result<String, ClipboardError> {
        if self.read_error {
            return Err(ClipboardError::message("read failed"));
        }

        Ok(self.reads.pop_front().unwrap_or_default())
    }

    /// Records text written by copy or cut.
    fn set_text(&mut self, text: &str) -> Result<(), ClipboardError> {
        if self.write_error {
            return Err(ClipboardError::message("write failed"));
        }

        self.writes.push(text.to_owned());
        Ok(())
    }
}

/// Extracts the single action produced by one terminal event.
fn single_action(state: &State, event: TerminalEvent) -> ChatTuiAction {
    let effects = effects(state, event);
    action_from_effects(effects)
}

/// Extracts the single action produced using a fake clipboard service.
fn single_action_with_clipboard(
    state: &State,
    event: TerminalEvent,
    clipboard: &mut dyn ClipboardService,
) -> ChatTuiAction {
    let effects = effects_with_clipboard(state, event, Some(clipboard));
    action_from_effects(effects)
}

/// Extracts the reducer action from one event effect.
fn action_from_effects(effects: Vec<EventEffect>) -> ChatTuiAction {
    assert_eq!(effects.len(), 1);
    match effects.into_iter().next().unwrap() {
        EventEffect::Action(action) => *action,
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

/// Verifies public text access derives from reducer-owned logical lines.
#[test]
fn prompt_text_accessor_uses_logical_lines_as_source_of_truth() {
    let mut prompt = PromptState::from_text("line one");
    prompt.text = "stale text".to_owned();
    prompt.lines = vec!["actual".to_owned(), "text".to_owned()];

    assert_eq!(prompt.text(), "actual\ntext");
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

/// Verifies Shift+Up and Shift+Down extend multiline prompt selection instead of plain navigation.
#[test]
fn shift_up_and_down_extend_multiline_prompt_selection() {
    let mut state = state();
    state.session.prompt = PromptState::from_text("one\ntwo\nthree");

    let action = single_action(&state, key(KeyCode::Up, KeyModifiers::SHIFT));
    reduce(&mut state, action);
    assert_eq!(state.session.prompt.cursor, "one\ntwo".len());
    assert_eq!(
        state.session.prompt.selection_range(),
        Some("one\ntwo".len().."one\ntwo\nthree".len())
    );

    let action = single_action(&state, key(KeyCode::Down, KeyModifiers::SHIFT));
    reduce(&mut state, action);
    assert_eq!(state.session.prompt.cursor, "one\ntwo\nthree".len());
    assert_eq!(state.session.prompt.selection_range(), None);
}

/// Verifies plain Enter inserts a line break instead of submitting.
#[test]
fn enter_with_text_inserts_newline_without_transcript_change() {
    let mut state = state();
    state.session.prompt = PromptState::from_text("run this");

    let action = single_action(&state, key(KeyCode::Enter, KeyModifiers::empty()));
    reduce(&mut state, action);

    assert_eq!(state.session.prompt.text, "run this\n");
    assert_eq!(state.session.transcript.len(), 0);
}

/// Verifies vertical movement uses actual prompt content width instead of default fallback width.
#[test]
fn prompt_up_uses_current_layout_width_for_wrapped_rows() {
    let mut state = state();
    state.prompt_layout = PromptLayoutMetrics {
        content_width: 3,
        viewport_height: 10,
    };
    state.session.prompt = PromptState::from_text("abcdef");

    let action = single_action(&state, key(KeyCode::Up, KeyModifiers::empty()));
    reduce(&mut state, action);

    assert_eq!(state.session.prompt.cursor, 3);
}

/// Verifies prompt changes keep the cursor visible within the current textarea viewport.
#[test]
fn prompt_changed_scrolls_prompt_viewport_to_cursor() {
    let mut state = state();
    state.prompt_layout = PromptLayoutMetrics {
        content_width: 3,
        viewport_height: 1,
    };

    reduce(
        &mut state,
        ChatTuiAction::PromptChanged(PromptState::from_text("abcdef")),
    );

    assert_eq!(state.session.prompt.scroll_top_row, 1);
}

/// Verifies resize updates prompt layout metrics while transcript viewport remains component-owned.
#[test]
fn resize_updates_prompt_layout_metrics() {
    let mut state = state();

    reduce(
        &mut state,
        ChatTuiAction::Resize {
            width: 10,
            height: 4,
        },
    );

    assert_eq!(
        state.prompt_layout,
        PromptLayoutMetrics {
            content_width: 8,
            viewport_height: 2,
        }
    );
}

/// Verifies Ctrl+Enter submits the current prompt through the reducer and clears prompt state.
#[test]
fn ctrl_enter_submits_prompt_appends_user_transcript_and_clears_prompt() {
    let mut state = state();
    state.session.prompt = PromptState::from_text("run this");

    let action = single_action(&state, key(KeyCode::Enter, KeyModifiers::CONTROL));
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

/// Verifies Ctrl+newline character events insert line breaks instead of submitting.
#[test]
fn control_newline_char_inserts_newline_without_submission() {
    let mut state = state();
    state.session.prompt = PromptState::from_text("run this");

    let action = single_action(&state, key(KeyCode::Char('\n'), KeyModifiers::CONTROL));
    reduce(&mut state, action);

    assert_eq!(state.session.prompt.text, "run this\n");
    assert_eq!(state.session.transcript.len(), 0);
}

/// Verifies Ctrl+carriage-return character events normalize to line feeds.
#[test]
fn control_carriage_return_char_inserts_normalized_newline_without_submission() {
    let mut state = state();
    state.session.prompt = PromptState::from_text("run this");

    let action = single_action(&state, key(KeyCode::Char('\r'), KeyModifiers::CONTROL));
    reduce(&mut state, action);

    assert_eq!(state.session.prompt.text, "run this\n");
    assert_eq!(state.session.transcript.len(), 0);
}

/// Verifies terminal newline chars are paste-like prompt edits, not submissions.
#[test]
fn newline_char_key_stream_preserves_multiline_prompt_without_submission() {
    let mut state = state();

    for code in [KeyCode::Char('a'), KeyCode::Char('\n'), KeyCode::Char('b')] {
        let action = single_action(&state, key(code, KeyModifiers::empty()));
        reduce(&mut state, action);
    }

    assert_eq!(state.session.prompt.text, "a\nb");
    assert_eq!(state.session.transcript.len(), 0);
}

/// Verifies control-newline in an unbracketed paste stream is preserved as prompt text.
#[test]
fn control_newline_char_key_stream_preserves_multiline_prompt_without_submission() {
    let mut state = state();

    for (code, modifiers) in [
        (KeyCode::Char('a'), KeyModifiers::empty()),
        (KeyCode::Char('\n'), KeyModifiers::CONTROL),
        (KeyCode::Char('b'), KeyModifiers::empty()),
    ] {
        let action = single_action(&state, key(code, modifiers));
        reduce(&mut state, action);
    }

    assert_eq!(state.session.prompt.text, "a\nb");
    assert_eq!(state.session.transcript.len(), 0);
}

/// Verifies Escape cancels only cancellable running states.
#[test]
fn escape_while_running_and_cancellable_dispatches_cancel() {
    let mut state = state();
    reduce(&mut state, ChatTuiAction::AgentStarted);

    let action = single_action(&state, key(KeyCode::Esc, KeyModifiers::empty()));
    reduce(&mut state, action);

    assert_eq!(state.status, Status::Cancelling);
}

/// Verifies Ctrl+C is reserved for copy and no longer requests outer-shell exit.
#[test]
fn ctrl_c_while_idle_without_selection_does_not_exit() {
    let state = state();
    let mut clipboard = FakeClipboard::default();

    let effects = effects_with_clipboard(
        &state,
        key(KeyCode::Char('c'), KeyModifiers::CONTROL),
        Some(&mut clipboard),
    );

    assert_eq!(effects, Vec::new());
    assert!(state.session.transcript.is_empty());
    assert!(clipboard.writes.is_empty());
}

/// Verifies Ctrl+C copies selected prompt text through the injected clipboard.
#[test]
fn ctrl_c_with_prompt_selection_copies_selection_to_clipboard() {
    let mut state = state();
    state.session.prompt = PromptState::from_text("copy me");
    state.session.prompt.move_to_start(true);
    let mut clipboard = FakeClipboard::default();

    let event_effects = effects_with_clipboard(
        &state,
        key(KeyCode::Char('c'), KeyModifiers::CONTROL),
        Some(&mut clipboard),
    );

    assert_eq!(clipboard.writes, vec!["copy me".to_owned()]);
    assert_eq!(event_effects.len(), 1);
    match event_effects.into_iter().next().unwrap() {
        EventEffect::Action(action) => {
            assert!(matches!(
                *action,
                ChatTuiAction::PromptChanged(prompt) if prompt.kill_buffer == "copy me"
            ));
        }
        EventEffect::RequestExit => panic!("expected copy action"),
    }
}

/// Verifies Ctrl+V pastes clipboard text through prompt paste normalization.
#[test]
fn ctrl_v_with_clipboard_text_inserts_normalized_prompt_text() {
    let state = state();
    let mut clipboard = FakeClipboard::with_text("one\r\ntwo");

    let action = single_action_with_clipboard(
        &state,
        key(KeyCode::Char('v'), KeyModifiers::CONTROL),
        &mut clipboard,
    );

    assert!(matches!(
        action,
        ChatTuiAction::PromptChanged(prompt)
            if prompt.text == "one\ntwo" && prompt.paste_burst.buffer == "one\ntwo"
    ));
}

/// Verifies Ctrl+V strips unsupported control bytes while preserving tab and newline.
#[test]
fn ctrl_v_with_control_characters_filters_unsupported_controls() {
    let state = state();
    let mut clipboard = FakeClipboard::with_text("a\u{0000}b\tc\u{001b}d");

    let action = single_action_with_clipboard(
        &state,
        key(KeyCode::Char('v'), KeyModifiers::CONTROL),
        &mut clipboard,
    );

    assert!(matches!(
        action,
        ChatTuiAction::PromptChanged(prompt) if prompt.text == "ab\tcd"
    ));
}

/// Verifies Ctrl+V normalizes CRLF and CR to LF after paste sanitization.
#[test]
fn ctrl_v_with_crlf_clipboard_text_normalizes_to_line_feeds() {
    let state = state();
    let mut clipboard = FakeClipboard::with_text("one\r\ntwo\rthree\u{0007}");

    let action = single_action_with_clipboard(
        &state,
        key(KeyCode::Char('v'), KeyModifiers::CONTROL),
        &mut clipboard,
    );

    assert!(matches!(
        action,
        ChatTuiAction::PromptChanged(prompt) if prompt.text == "one\ntwo\nthree"
    ));
}

/// Verifies oversized paste is rejected before prompt mutation.
#[test]
fn ctrl_v_with_oversized_clipboard_text_reports_input_notice() {
    let state = state();
    let mut clipboard = FakeClipboard::with_text(&"a".repeat(MAX_PASTE_BYTES + 1));

    let action = single_action_with_clipboard(
        &state,
        key(KeyCode::Char('v'), KeyModifiers::CONTROL),
        &mut clipboard,
    );

    assert_eq!(
        action,
        ChatTuiAction::InputNoticeReported {
            message: "Paste is too large (limit 1 MB)".to_owned(),
        }
    );
}

/// Verifies multiline paste is rejected while composing a slash command.
#[test]
fn ctrl_v_with_multiline_text_in_slash_command_reports_input_notice() {
    let mut state = state();
    state.session.prompt = PromptState::from_text("/config");
    let mut clipboard = FakeClipboard::with_text("one\ntwo");

    let action = single_action_with_clipboard(
        &state,
        key(KeyCode::Char('v'), KeyModifiers::CONTROL),
        &mut clipboard,
    );

    assert_eq!(
        action,
        ChatTuiAction::InputNoticeReported {
            message: "Slash commands accept single-line paste only".to_owned(),
        }
    );
}

/// Verifies unavailable clipboard feedback is reducer-owned input notice.
#[test]
fn ctrl_v_without_clipboard_reports_input_notice() {
    let state = state();

    let action = action_from_effects(effects_with_clipboard(
        &state,
        key(KeyCode::Char('v'), KeyModifiers::CONTROL),
        None,
    ));

    assert_eq!(
        action,
        ChatTuiAction::InputNoticeReported {
            message: "Clipboard is unavailable".to_owned(),
        }
    );
}

/// Verifies clipboard read failures are visible without transcript spam.
#[test]
fn ctrl_v_with_clipboard_read_failure_reports_input_notice() {
    let state = state();
    let mut clipboard = FakeClipboard::failing_read();

    let action = single_action_with_clipboard(
        &state,
        key(KeyCode::Char('v'), KeyModifiers::CONTROL),
        &mut clipboard,
    );

    assert_eq!(
        action,
        ChatTuiAction::InputNoticeReported {
            message: "Clipboard read failed".to_owned(),
        }
    );
}

/// Verifies clipboard write failures are visible and do not mutate prompt state.
#[test]
fn ctrl_c_with_clipboard_write_failure_reports_input_notice() {
    let mut state = state();
    state.session.prompt = PromptState::from_text("copy me");
    state.session.prompt.move_to_start(true);
    let mut clipboard = FakeClipboard::failing_write();

    let action = single_action_with_clipboard(
        &state,
        key(KeyCode::Char('c'), KeyModifiers::CONTROL),
        &mut clipboard,
    );

    assert_eq!(
        action,
        ChatTuiAction::InputNoticeReported {
            message: "Clipboard write failed".to_owned(),
        }
    );
    assert!(clipboard.writes.is_empty());
}

/// Verifies terminal-native paste events insert sanitized text directly.
#[test]
fn terminal_paste_event_inserts_normalized_prompt_text() {
    let state = state();
    let mut clipboard = FakeClipboard::with_text("clipboard text");

    let effects = effects_with_clipboard(
        &state,
        TerminalEvent::Paste("terminal\r\npasted text".to_owned()),
        Some(&mut clipboard),
    );

    assert!(matches!(
        action_from_effects(effects),
        ChatTuiAction::PromptChanged(prompt)
            if prompt.text == "terminal\npasted text"
                && prompt.paste_burst.buffer == "terminal\npasted text"
    ));
}

/// Verifies terminal-native paste events become spaces in selection prompt editable fields.
#[test]
fn terminal_paste_event_in_selection_prompt_inserts_single_line_text() {
    let mut state = state();
    state.selection = Some(
        SelectionPromptState::new("Pick one", "", vec!["alpha".to_owned()]).with_inputs(true, true),
    );
    let mut clipboard = FakeClipboard::with_text("clipboard text");

    let effects = effects_with_clipboard(
        &state,
        TerminalEvent::Paste("terminal\r\npasted text".to_owned()),
        Some(&mut clipboard),
    );

    assert!(matches!(
        action_from_effects(effects),
        ChatTuiAction::SelectionPromptChanged(Some(selection))
            if selection.custom_input == "terminal pasted text" && selection.selected == 1
    ));
}

/// Verifies Ctrl+X copies and removes the selected prompt text.
#[test]
fn ctrl_x_with_prompt_selection_cuts_selection_to_clipboard() {
    let mut state = state();
    state.session.prompt = PromptState::from_text("cut me");
    state.session.prompt.move_to_start(true);
    let mut clipboard = FakeClipboard::default();

    let action = single_action_with_clipboard(
        &state,
        key(KeyCode::Char('x'), KeyModifiers::CONTROL),
        &mut clipboard,
    );

    assert_eq!(clipboard.writes, vec!["cut me".to_owned()]);
    assert!(matches!(
        action,
        ChatTuiAction::PromptChanged(prompt) if prompt.text.is_empty() && prompt.kill_buffer == "cut me"
    ));
}

/// Verifies Ctrl+Q is the explicit quit binding for the TUI.
#[test]
fn ctrl_q_while_idle_requests_exit() {
    let state = state();

    let event_effects = effects(&state, key(KeyCode::Char('q'), KeyModifiers::CONTROL));

    assert_eq!(event_effects, vec![EventEffect::RequestExit]);
}

/// Verifies Ctrl+Q remains the explicit quit binding inside modal selection prompts.
#[test]
fn ctrl_q_while_selection_prompt_active_requests_exit() {
    let mut state = state();
    state.selection = Some(SelectionPromptState::new(
        "Pick one",
        "",
        vec!["alpha".to_owned()],
    ));

    let event_effects = effects(&state, key(KeyCode::Char('q'), KeyModifiers::CONTROL));

    assert_eq!(event_effects, vec![EventEffect::RequestExit]);
}

/// Verifies Ctrl+C remains reserved for copy inside modal selection prompts.
#[test]
fn ctrl_c_while_selection_prompt_active_does_not_exit() {
    let mut state = state();
    state.selection = Some(SelectionPromptState::new(
        "Pick one",
        "",
        vec!["alpha".to_owned()],
    ));

    let event_effects = effects(&state, key(KeyCode::Char('c'), KeyModifiers::CONTROL));

    assert_eq!(event_effects, Vec::new());
}

/// Verifies Ctrl+V pastes into an active selection prompt custom input field.
#[test]
fn ctrl_v_while_selection_prompt_active_inserts_single_line_clipboard_text() {
    let mut state = state();
    state.selection = Some(
        SelectionPromptState::new("Pick one", "", vec!["alpha".to_owned()]).with_inputs(true, true),
    );
    let mut clipboard = FakeClipboard::with_text("one\r\ntwo\u{0007}");

    let action = single_action_with_clipboard(
        &state,
        key(KeyCode::Char('v'), KeyModifiers::CONTROL),
        &mut clipboard,
    );

    assert!(matches!(
        action,
        ChatTuiAction::SelectionPromptChanged(Some(selection))
            if selection.custom_input == "one two" && selection.selected == 1
    ));
}

/// Verifies Ctrl+V pastes into selection prompt comment mode when active.
#[test]
fn ctrl_v_while_selection_prompt_comment_mode_active_inserts_comment_text() {
    let mut selection =
        SelectionPromptState::new("Pick one", "", vec!["alpha".to_owned()]).with_inputs(true, true);
    selection.toggle_comment_mode();
    let mut state = state();
    state.selection = Some(selection);
    let mut clipboard = FakeClipboard::with_text("note\r\ntext");

    let action = single_action_with_clipboard(
        &state,
        key(KeyCode::Char('v'), KeyModifiers::CONTROL),
        &mut clipboard,
    );

    assert!(matches!(
        action,
        ChatTuiAction::SelectionPromptChanged(Some(selection))
            if selection.comment == "note text" && selection.custom_input.is_empty()
    ));
}

/// Verifies selection prompt newline chars become editable spaces instead of submissions.
#[test]
fn selection_prompt_newline_char_keys_insert_space_without_submission() {
    for (code, modifiers) in [
        (KeyCode::Char('\n'), KeyModifiers::empty()),
        (KeyCode::Char('\n'), KeyModifiers::CONTROL),
        (KeyCode::Char('\r'), KeyModifiers::CONTROL),
    ] {
        let mut state = state();
        state.selection = Some(
            SelectionPromptState::new("Pick one", "", vec!["alpha".to_owned()])
                .with_inputs(true, true),
        );

        let action = single_action(&state, key(code, modifiers));

        assert!(matches!(
            action,
            ChatTuiAction::SelectionPromptChanged(Some(selection))
                if selection.custom_input == " " && selection.selected == 1
        ));
    }
}

/// Verifies release events remain ignored instead of replaying key actions.
#[test]
fn key_release_event_does_not_emit_effects() {
    let state = state();
    let mut event = KeyEvent::new(KeyEventKind::Release, KeyCode::Char('q'));
    event.modifiers = KeyModifiers::CONTROL;

    assert_eq!(effects(&state, TerminalEvent::Key(event)), Vec::new());
}

/// Verifies timer ticks are represented as explicit spinner actions at the documented cadence.
#[test]
fn timer_tick_dispatches_spinner_tick_without_terminal_output() {
    assert_eq!(SPINNER_TICK_INTERVAL, Duration::from_millis(90));
    assert_eq!(
        spectacular_tui::timer_tick_effects(),
        vec![EventEffect::Action(Box::new(ChatTuiAction::SpinnerTick))]
    );
}

/// Verifies assistant deltas are reducer-visible without a separate reveal timer.
#[test]
fn assistant_delta_is_visible_without_reveal_timer_effects() {
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

/// Verifies transcript scroll input is left to the layout-owned viewport.
#[test]
fn transcript_scroll_input_does_not_emit_reducer_actions() {
    let state = state();

    assert_eq!(
        effects(
            &state,
            TerminalEvent::Key(KeyEvent::new(KeyEventKind::Press, KeyCode::PageUp))
        ),
        Vec::new()
    );
    assert_eq!(
        effects(
            &state,
            TerminalEvent::Key(KeyEvent::new(KeyEventKind::Press, KeyCode::PageDown))
        ),
        Vec::new()
    );
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
