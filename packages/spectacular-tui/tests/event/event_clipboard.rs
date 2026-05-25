use super::*;

/// Verifies Ctrl+V pastes clipboard text through prompt paste normalization.
#[test]

fn effects_with_clipboard_and_paste_when_ctrl_v_with_clipboard_text_inserts_normalized_prompt_text()
{
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

fn effects_with_clipboard_and_paste_when_ctrl_v_with_control_characters_filters_unsupported_controls(
) {
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

fn effects_with_clipboard_and_paste_when_ctrl_v_with_crlf_clipboard_text_normalizes_to_line_feeds()
{
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

fn effects_with_clipboard_and_paste_when_ctrl_v_with_oversized_clipboard_text_reports_input_notice()
{
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

fn effects_with_clipboard_and_paste_when_ctrl_v_with_multiline_text_in_slash_command_reports_input_notice(
) {
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

fn effects_with_clipboard_and_paste_when_ctrl_v_without_clipboard_reports_input_notice() {
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

fn effects_with_clipboard_and_paste_when_ctrl_v_with_clipboard_read_failure_reports_input_notice() {
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

fn effects_with_clipboard_and_paste_when_ctrl_c_with_clipboard_write_failure_reports_input_notice()
{
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

fn effects_with_clipboard_and_paste_when_terminal_paste_event_inserts_normalized_prompt_text() {
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

fn effects_with_clipboard_and_paste_when_terminal_paste_event_in_selection_prompt_inserts_single_line_text(
) {
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

fn effects_with_clipboard_and_paste_when_ctrl_x_with_prompt_selection_cuts_selection_to_clipboard()
{
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

fn effects_with_clipboard_and_paste_when_ctrl_q_while_idle_requests_exit() {
    let state = state();

    let event_effects = effects(&state, key(KeyCode::Char('q'), KeyModifiers::CONTROL));

    assert_eq!(event_effects, vec![EventEffect::RequestExit]);
}

/// Verifies Ctrl+Q remains the explicit quit binding inside modal selection prompts.

#[test]

fn effects_with_clipboard_and_paste_when_ctrl_q_while_selection_prompt_active_requests_exit() {
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

fn effects_with_clipboard_and_paste_when_ctrl_c_while_selection_prompt_active_does_not_exit() {
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

fn effects_with_clipboard_and_paste_when_ctrl_v_while_selection_prompt_active_inserts_single_line_clipboard_text(
) {
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

fn effects_with_clipboard_and_paste_when_ctrl_v_while_selection_prompt_comment_mode_active_inserts_comment_text(
) {
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

fn effects_with_clipboard_and_paste_when_selection_prompt_newline_char_keys_insert_space_without_submission(
) {
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
