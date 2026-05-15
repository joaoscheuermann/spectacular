use iocraft::prelude::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers, TerminalEvent};
use spectacular_tui::{
    reduce, tui_event_effects, ChatTuiAction, CommandDescriptor, DisplayMetadata, EventEffect,
    PromptState, ReasoningLevel, RenderStyle, RuntimeSelection, SelectionPromptAnswer,
    SelectionPromptChoice, SelectionPromptState, SessionId, State,
};

/// Builds runtime metadata for prompt parity tests.
fn runtime() -> RuntimeSelection {
    RuntimeSelection::new(
        "openai-compatible",
        "provider",
        "model",
        ReasoningLevel::Low,
        Some(4096),
    )
}

/// Builds display metadata for prompt parity tests.
fn display() -> DisplayMetadata {
    DisplayMetadata::new("provider", "model", "low", "/workspace", "session", None)
}

/// Builds an initialized state with stable metadata.
fn state() -> State {
    State::new(SessionId::new("session-1"), runtime(), display())
}

/// Builds one IOCraft key event for event-loop tests.
fn key(code: KeyCode, modifiers: KeyModifiers) -> TerminalEvent {
    let mut event = KeyEvent::new(KeyEventKind::Press, code);
    event.modifiers = modifiers;
    TerminalEvent::Key(event)
}

/// Extracts a single action from one terminal event.
fn single_action(state: &State, event: TerminalEvent) -> ChatTuiAction {
    let effects = tui_event_effects(state, event);
    assert_eq!(effects.len(), 1, "effects: {effects:?}");
    match effects.into_iter().next().unwrap() {
        EventEffect::Action(action) => action,
        EventEffect::RequestExit => panic!("expected action effect"),
    }
}

/// Applies one key event to reducer state.
fn press(state: &mut State, code: KeyCode, modifiers: KeyModifiers) {
    let action = single_action(state, key(code, modifiers));
    reduce(state, action);
}

#[test]
fn prompt_inserts_text_at_cursor() {
    let mut prompt = PromptState::from_text("helo");
    prompt.move_left(false);
    prompt.insert_text("l");

    assert_eq!(prompt.text, "hello");
    assert_eq!(prompt.cursor, 4);
}

#[test]
fn prompt_replaces_selected_text_on_insert() {
    let mut prompt = PromptState::from_text("hello");
    prompt.move_left(false);
    prompt.move_left(false);
    prompt.move_right(true);
    prompt.insert_text("p");

    assert_eq!(prompt.text, "helpo");
    assert_eq!(prompt.selection_range(), None);
}

#[test]
fn prompt_backspace_and_delete_match_original() {
    let mut prompt = PromptState::from_text("a😀b");
    prompt.move_left(false);
    prompt.backspace();
    assert_eq!(prompt.text, "ab");
    assert_eq!(prompt.cursor, 1);

    prompt.delete_forward();
    assert_eq!(prompt.text, "a");
}

#[test]
fn prompt_word_movement_matches_original() {
    let mut prompt = PromptState::from_text("alpha  beta.gamma");

    prompt.move_word_left(false);
    assert_eq!(prompt.cursor, "alpha  beta.".len());
    prompt.move_word_left(false);
    assert_eq!(prompt.cursor, "alpha  ".len());
    prompt.move_word_right(false);
    assert_eq!(prompt.cursor, "alpha  beta".len());
}

#[test]
fn prompt_word_deletion_matches_original() {
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
fn prompt_ctrl_a_selects_all() {
    let mut state = state();
    state.session.prompt = PromptState::from_text("select me");

    press(&mut state, KeyCode::Char('a'), KeyModifiers::CONTROL);

    assert_eq!(state.session.prompt.selection_range(), Some(0..9));
}

#[test]
fn prompt_kill_and_yank_match_original() {
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
fn prompt_kill_selection_updates_yank_buffer() {
    let mut prompt = PromptState::from_text("alpha beta");
    prompt.move_to_start(false);
    prompt.move_word_right(true);

    prompt.kill_to_line_end();

    assert_eq!(prompt.text, " beta");
    assert_eq!(prompt.kill_buffer, "alpha");
}

#[test]
fn prompt_escape_clears_contexts_like_original() {
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
fn prompt_ctrl_c_idle_clear_then_exit_matches_original() {
    let mut state = state();
    state.session.prompt = PromptState::from_text("draft");

    press(&mut state, KeyCode::Char('c'), KeyModifiers::CONTROL);
    assert_eq!(state.session.prompt, PromptState::empty());

    assert_eq!(
        tui_event_effects(&state, key(KeyCode::Char('c'), KeyModifiers::CONTROL)),
        vec![EventEffect::RequestExit]
    );
}

#[test]
fn prompt_enter_and_modified_enter_match_original() {
    let mut state = state();
    state.session.prompt = PromptState::from_text("one");

    press(&mut state, KeyCode::Enter, KeyModifiers::SHIFT);
    assert_eq!(state.session.prompt.text, "one\n");

    press(&mut state, KeyCode::Enter, KeyModifiers::empty());
    assert_eq!(state.session.transcript.len(), 1);
    assert_eq!(state.session.prompt, PromptState::empty());
}

#[test]
fn prompt_paste_burst_preserves_multiline_text() {
    let mut prompt = PromptState::empty();

    prompt.insert_paste("one\r\ntwo\rthree");

    assert_eq!(prompt.text, "one\ntwo\nthree");
    assert_eq!(prompt.paste_burst.buffer, "one\ntwo\nthree");
}

#[test]
fn prompt_selection_renders_styled_ranges() {
    let mut state = state();
    state.session.prompt = PromptState::from_text("hello");
    state.session.prompt.move_left(false);
    state.session.prompt.move_left(true);

    let lines = spectacular_tui::prompt_render_lines(&state);

    assert_eq!(lines[0].plain_text(), "> hello");
    assert!(lines[0]
        .spans
        .iter()
        .any(|span| span.text == "l" && span.style == RenderStyle::Selection));
}

#[test]
fn slash_suggestions_match_original_padding_and_selection() {
    let mut state = state();
    state.commands = vec![
        CommandDescriptor::new("config", "Manage configuration"),
        CommandDescriptor::new("continue", "Continue last run"),
    ];
    state.session.prompt = PromptState::from_text("/con");
    state.session.prompt.selected_completion = 1;

    let lines = spectacular_tui::prompt_render_lines(&state);

    assert_eq!(
        lines[1].plain_text(),
        "  /config            Manage configuration"
    );
    assert_eq!(
        lines[2].plain_text(),
        "  /continue          Continue last run"
    );
    assert_eq!(lines[1].spans[0].style, RenderStyle::Dim);
    assert_eq!(lines[2].spans[0].style, RenderStyle::User);
}

#[test]
fn slash_tab_enter_space_acceptance_matches_original() {
    let mut state = state();
    state.commands = vec![CommandDescriptor::with_usage(
        "config",
        "Manage configuration",
        "/config list",
    )];
    state.session.prompt = PromptState::from_text("/con");

    press(&mut state, KeyCode::Tab, KeyModifiers::empty());
    assert_eq!(state.session.prompt.text, "/config ");

    let output = spectacular_tui::prompt_lines(&state).join("\n");
    assert!(output.contains("/config list"));
}

#[test]
fn selection_prompt_navigation_matches_original() {
    let mut state = state();
    state.selection = Some(SelectionPromptState::new(
        "Pick one",
        "Choose carefully",
        vec!["alpha".to_owned(), "beta".to_owned()],
    ));

    press(&mut state, KeyCode::Down, KeyModifiers::empty());
    assert_eq!(state.selection.as_ref().unwrap().selected, 1);

    press(&mut state, KeyCode::Char('j'), KeyModifiers::empty());
    assert_eq!(state.selection.as_ref().unwrap().selected, 0);

    press(&mut state, KeyCode::Char('k'), KeyModifiers::empty());
    assert_eq!(state.selection.as_ref().unwrap().selected, 1);
}

#[test]
fn selection_prompt_custom_input_and_comment_mode_match_original() {
    let mut state = state();
    state.selection = Some(
        SelectionPromptState::new("Pick one", "", vec!["alpha".to_owned()]).with_inputs(true, true),
    );

    press(&mut state, KeyCode::Char('x'), KeyModifiers::empty());
    assert_eq!(state.selection.as_ref().unwrap().custom_input, "x");
    assert_eq!(state.selection.as_ref().unwrap().selected, 1);

    press(&mut state, KeyCode::Tab, KeyModifiers::empty());
    press(&mut state, KeyCode::Char('!'), KeyModifiers::empty());
    assert_eq!(state.selection.as_ref().unwrap().comment, "!");

    press(&mut state, KeyCode::Esc, KeyModifiers::empty());
    assert!(state.selection.as_ref().unwrap().is_options_mode());
}

#[test]
fn selection_prompt_submit_and_cancel_match_original() {
    let mut active_state = state();
    active_state.selection = Some(
        SelectionPromptState::new("Pick one", "", vec!["alpha".to_owned()]).with_inputs(true, true),
    );

    press(&mut active_state, KeyCode::Char('x'), KeyModifiers::empty());
    press(&mut active_state, KeyCode::Tab, KeyModifiers::empty());
    press(&mut active_state, KeyCode::Char('!'), KeyModifiers::empty());

    assert_eq!(
        tui_event_effects(&active_state, key(KeyCode::Enter, KeyModifiers::empty())),
        vec![EventEffect::Action(
            ChatTuiAction::SelectionPromptSubmitted(SelectionPromptAnswer {
                choice: SelectionPromptChoice::Custom("x".to_owned()),
                comment: Some("!".to_owned()),
            })
        )]
    );

    let mut state = state();
    state.selection = Some(SelectionPromptState::new(
        "Pick one",
        "",
        vec!["alpha".to_owned()],
    ));

    assert_eq!(
        tui_event_effects(&state, key(KeyCode::Esc, KeyModifiers::empty())),
        vec![EventEffect::Action(ChatTuiAction::SelectionPromptCancelled)]
    );
}
