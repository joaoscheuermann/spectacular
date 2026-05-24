use super::*;

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
