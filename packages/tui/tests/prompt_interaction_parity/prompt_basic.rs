use super::*;

#[test]

fn prompt_state_when_prompt_inserts_text_at_cursor() {
    let mut prompt = PromptState::from_text("helo");

    prompt.move_left(false);

    prompt.insert_text("l");

    assert_eq!(prompt.text, "hello");

    assert_eq!(prompt.cursor, 4);
}

#[test]

fn prompt_state_when_prompt_replaces_selected_text_on_insert() {
    let mut prompt = PromptState::from_text("hello");

    prompt.move_left(false);

    prompt.move_left(false);

    prompt.move_right(true);

    prompt.insert_text("p");

    assert_eq!(prompt.text, "helpo");

    assert_eq!(prompt.selection_range(), None);
}

#[test]

fn prompt_state_when_prompt_backspace_and_delete_match_original() {
    let mut prompt = PromptState::from_text("a\u{1f9d1}\u{200d}\u{1f4bb}b");

    prompt.move_left(false);

    prompt.backspace();

    assert_eq!(prompt.text, "ab");

    assert_eq!(prompt.cursor, 1);

    prompt.delete_forward();

    assert_eq!(prompt.text, "a");
}

#[test]

fn prompt_state_when_prompt_word_movement_matches_original() {
    let mut prompt = PromptState::from_text("alpha  beta.gamma");

    prompt.move_word_left(false);

    assert_eq!(prompt.cursor, "alpha  beta.".len());

    prompt.move_word_left(false);

    assert_eq!(prompt.cursor, "alpha  ".len());

    prompt.move_word_right(false);

    assert_eq!(prompt.cursor, "alpha  beta".len());
}

#[test]

fn prompt_state_when_prompt_word_deletion_matches_original() {
    let mut prompt = PromptState::from_text("alpha  beta.gamma");

    prompt.delete_previous_word();

    assert_eq!(prompt.text, "alpha  beta.");

    prompt.delete_previous_word();

    assert_eq!(prompt.text, "alpha  beta");

    prompt.move_to_start(false);

    prompt.delete_next_word();

    assert_eq!(prompt.text, "  beta");
}

#[test]

fn prompt_state_when_prompt_ctrl_a_selects_all() {
    let mut state = state();

    state.session.prompt = PromptState::from_text("select me");

    press(&mut state, KeyCode::Char('a'), KeyModifiers::CONTROL);

    assert_eq!(state.session.prompt.selection_range(), Some(0..9));
}

#[test]

fn prompt_state_when_prompt_kill_and_yank_match_original() {
    let mut state = state();

    state.session.prompt = PromptState::from_text("first\nsecond third");

    state.session.prompt.move_left(false);

    state.session.prompt.move_left(false);

    state.session.prompt.move_left(false);

    state.session.prompt.move_left(false);

    state.session.prompt.move_left(false);

    press(&mut state, KeyCode::Char('u'), KeyModifiers::CONTROL);

    assert_eq!(state.session.prompt.text, "first\nthird");

    assert_eq!(state.session.prompt.kill_buffer, "second ");

    press(&mut state, KeyCode::Char('y'), KeyModifiers::CONTROL);

    assert_eq!(state.session.prompt.text, "first\nsecond third");

    press(&mut state, KeyCode::Char('k'), KeyModifiers::CONTROL);

    assert_eq!(state.session.prompt.text, "first\nsecond ");

    assert_eq!(state.session.prompt.kill_buffer, "third");
}

#[test]

fn prompt_state_when_prompt_kill_selection_updates_yank_buffer() {
    let mut prompt = PromptState::from_text("alpha beta");

    prompt.move_to_start(false);

    prompt.move_word_right(true);

    prompt.kill_to_line_end();

    assert_eq!(prompt.text, " beta");

    assert_eq!(prompt.kill_buffer, "alpha");
}

#[test]

fn prompt_state_when_prompt_escape_clears_contexts_like_original() {
    let mut state = state();

    state.session.prompt = PromptState::from_text("hello");

    state.session.prompt.move_to_start(true);

    press(&mut state, KeyCode::Esc, KeyModifiers::empty());

    assert_eq!(state.session.prompt.text, "hello");

    assert_eq!(state.session.prompt.selection_range(), None);

    press(&mut state, KeyCode::Esc, KeyModifiers::empty());

    assert_eq!(state.session.prompt, PromptState::empty());
}

#[test]

fn prompt_state_when_prompt_escape_idle_clear_then_exit_matches_original() {
    let mut state = state();

    state.session.prompt = PromptState::from_text("draft");

    press(&mut state, KeyCode::Esc, KeyModifiers::empty());

    assert_eq!(state.session.prompt, PromptState::empty());

    assert_eq!(
        effects(&state, key(KeyCode::Esc, KeyModifiers::empty())),
        vec![EventEffect::RequestExit]
    );
}

#[test]

fn prompt_state_when_prompt_enter_variants_insert_newline_and_ctrl_enter_submits() {
    let mut state = state();

    state.session.prompt = PromptState::from_text("one");

    press(&mut state, KeyCode::Enter, KeyModifiers::SHIFT);

    assert_eq!(state.session.prompt.text, "one\n");

    press(&mut state, KeyCode::Enter, KeyModifiers::empty());

    assert_eq!(state.session.prompt.text, "one\n\n");

    assert_eq!(state.session.transcript.len(), 0);

    press(&mut state, KeyCode::Enter, KeyModifiers::CONTROL);

    assert_eq!(state.session.transcript.len(), 1);

    assert_eq!(state.session.prompt, PromptState::empty());
}

#[test]

fn prompt_state_when_prompt_paste_burst_preserves_multiline_text() {
    let mut prompt = PromptState::empty();

    prompt.insert_paste("one\r\ntwo\rthree");

    assert_eq!(prompt.text, "one\ntwo\nthree");

    assert_eq!(prompt.paste_burst.buffer, "one\ntwo\nthree");
}

#[test]

fn prompt_state_when_prompt_cursor_renders_grapheme_cluster_cell() {
    let mut state = state();

    state.session.prompt = PromptState::from_text("e\u{301}x");

    state.session.prompt.move_to_start(false);

    let lines = prompt_render_lines(&state);

    let cursor_text = lines[0]
        .spans
        .iter()
        .find(|span| span.highlight == Some(RenderHighlight::Cursor))
        .map(|span| span.text.as_str());

    assert_eq!(cursor_text, Some("e\u{301}"));
}

#[test]

fn prompt_state_when_prompt_selection_renders_styled_ranges() {
    let mut state = state();

    state.session.prompt = PromptState::from_text("hello");

    state.session.prompt.move_left(false);

    state.session.prompt.move_left(true);

    state.session.prompt.move_left(true);

    let lines = prompt_render_lines(&state);

    assert_eq!(lines[0].plain_text(), "> hello");

    assert!(lines[0].spans.iter().any(|span| span.text == "l"
        && span.style == RenderStyle::Text
        && span.highlight == Some(RenderHighlight::Selection)));

    assert!(!lines[0].plain_text().contains('\u{2588}'));
}
