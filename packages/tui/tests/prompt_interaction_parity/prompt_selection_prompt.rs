use super::*;

#[test]

fn selection_prompt_when_selection_prompt_navigation_matches_original() {
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

fn selection_prompt_when_selection_prompt_custom_input_and_comment_mode_match_original() {
    let mut state = state();

    state.selection = Some(
        SelectionPromptState::new("Pick one", "", vec!["alpha".to_owned()]).with_inputs(true, true),
    );

    press(&mut state, KeyCode::Char('x'), KeyModifiers::empty());

    assert_eq!(state.selection.as_ref().unwrap().custom_input, "x");

    assert_eq!(state.selection.as_ref().unwrap().selected, 1);

    press(&mut state, KeyCode::Char('e'), KeyModifiers::empty());

    press(&mut state, KeyCode::Char('\u{301}'), KeyModifiers::empty());

    press(&mut state, KeyCode::Backspace, KeyModifiers::empty());

    assert_eq!(state.selection.as_ref().unwrap().custom_input, "x");

    press(&mut state, KeyCode::Tab, KeyModifiers::empty());

    press(&mut state, KeyCode::Char('!'), KeyModifiers::empty());

    assert_eq!(state.selection.as_ref().unwrap().comment, "!");

    press(&mut state, KeyCode::Esc, KeyModifiers::empty());

    assert!(state.selection.as_ref().unwrap().is_options_mode());
}

#[test]

fn selection_prompt_when_selection_prompt_editable_cursor_keys_match_original() {
    let mut state = state();

    state.selection = Some(
        SelectionPromptState::new("Pick one", "", vec!["alpha".to_owned()]).with_inputs(true, true),
    );

    for character in ['a', 'b', 'c'] {
        press(&mut state, KeyCode::Char(character), KeyModifiers::empty());
    }

    press(&mut state, KeyCode::Left, KeyModifiers::empty());

    press(&mut state, KeyCode::Backspace, KeyModifiers::empty());

    assert_eq!(state.selection.as_ref().unwrap().custom_input, "ac");

    press(&mut state, KeyCode::Home, KeyModifiers::empty());

    press(&mut state, KeyCode::Delete, KeyModifiers::empty());

    assert_eq!(state.selection.as_ref().unwrap().custom_input, "c");

    press(&mut state, KeyCode::End, KeyModifiers::empty());

    press(&mut state, KeyCode::Char('d'), KeyModifiers::empty());

    assert_eq!(state.selection.as_ref().unwrap().custom_input, "cd");

    press(&mut state, KeyCode::Tab, KeyModifiers::empty());

    for character in ['n', 'o', 't', 'e'] {
        press(&mut state, KeyCode::Char(character), KeyModifiers::empty());
    }

    press(&mut state, KeyCode::Left, KeyModifiers::empty());

    press(&mut state, KeyCode::Backspace, KeyModifiers::empty());

    assert_eq!(state.selection.as_ref().unwrap().comment, "noe");
}

#[test]

fn selection_prompt_when_selection_prompt_submit_and_cancel_match_original() {
    let mut active_state = state();

    active_state.selection = Some(
        SelectionPromptState::new("Pick one", "", vec!["alpha".to_owned()]).with_inputs(true, true),
    );

    press(&mut active_state, KeyCode::Char('x'), KeyModifiers::empty());

    press(&mut active_state, KeyCode::Tab, KeyModifiers::empty());

    press(&mut active_state, KeyCode::Char('!'), KeyModifiers::empty());

    assert_eq!(
        effects(&active_state, key(KeyCode::Enter, KeyModifiers::empty())),
        vec![EventEffect::Action(Box::new(
            ChatTuiAction::SelectionPromptSubmitted(SelectionPromptAnswer {
                choice: SelectionPromptChoice::Custom("x".to_owned()),

                comment: Some("!".to_owned()),
            })
        ))]
    );

    let mut state = state();

    state.selection = Some(SelectionPromptState::new(
        "Pick one",
        "",
        vec!["alpha".to_owned()],
    ));

    assert_eq!(
        effects(&state, key(KeyCode::Esc, KeyModifiers::empty())),
        vec![EventEffect::Action(Box::new(
            ChatTuiAction::SelectionPromptCancelled
        ))]
    );
}
