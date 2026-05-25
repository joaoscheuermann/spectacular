use super::*;

#[test]

fn slash_completion_when_slash_suggestions_match_original_padding_and_selection() {
    let mut state = state();

    state.commands = vec![
        CommandDescriptor::new("config", "Manage configuration"),
        CommandDescriptor::new("continue", "Continue last run"),
    ];

    state.session.prompt = PromptState::from_text("/con");

    state.session.prompt.selected_completion = 1;

    let lines = prompt_render_lines(&state);

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

fn slash_completion_when_slash_suggestions_with_indented_slash_do_not_render() {
    let mut state = state();

    state.commands = vec![CommandDescriptor::new("config", "Manage configuration")];

    state.session.prompt = PromptState::from_text(" /con");

    let lines = prompt_render_lines(&state);

    assert_eq!(lines.len(), 1);

    assert_eq!(lines[0].plain_text(), ">  /con ");
}

#[test]

fn slash_completion_when_slash_suggestions_with_later_line_slash_do_not_render() {
    let mut state = state();

    state.commands = vec![CommandDescriptor::new("config", "Manage configuration")];

    state.session.prompt = PromptState::from_text("hello\n/con");

    let lines = prompt_render_lines(&state);

    assert_eq!(lines.len(), 2);

    assert_eq!(lines[0].plain_text(), "> hello");

    assert_eq!(lines[1].plain_text(), "  /con ");
}

#[test]

fn slash_completion_when_slash_usage_with_indented_command_does_not_render() {
    let mut state = state();

    state.commands = vec![CommandDescriptor::with_usage(
        "config",
        "Manage configuration",
        "/config list",
    )];

    state.session.prompt = PromptState::from_text(" /config list");

    let lines = prompt_render_lines(&state);

    assert_eq!(lines.len(), 1);

    assert_eq!(lines[0].plain_text(), ">  /config list ");
}

#[test]

fn slash_completion_when_slash_tab_enter_space_acceptance_matches_original() {
    let mut state = state();

    state.commands = vec![CommandDescriptor::with_usage(
        "config",
        "Manage configuration",
        "/config list",
    )];

    state.session.prompt = PromptState::from_text("/con");

    press(&mut state, KeyCode::Tab, KeyModifiers::empty());

    assert_eq!(state.session.prompt.text, "/config ");

    let output = prompt_lines(&state).join("\n");

    assert!(output.contains("/config list"));
}

#[test]

fn slash_completion_when_slash_command_suggestions_fuzzy_rank_limit_and_uppercase_match_legacy() {
    let mut state = state();

    state.commands = vec![
        CommandDescriptor::new("retry", "Retry run"),
        CommandDescriptor::new("history", "Show history"),
        CommandDescriptor::new("config", "Manage configuration"),
        CommandDescriptor::new("continue", "Continue run"),
        CommandDescriptor::new("context", "Show context"),
        CommandDescriptor::new("commit", "Commit changes"),
        CommandDescriptor::new("compare", "Compare changes"),
        CommandDescriptor::new("copy", "Copy text"),
        CommandDescriptor::new("clear", "Clear session"),
        CommandDescriptor::new("cancel", "Cancel run"),
    ];

    state.session.prompt = PromptState::from_text("/confg");

    let lines = prompt_render_lines(&state);

    assert_eq!(
        lines[1].plain_text(),
        "  /config            Manage configuration"
    );

    state.session.prompt = PromptState::from_text("/CON");

    let lines = prompt_render_lines(&state);

    assert_eq!(lines.len(), 1);

    state.session.prompt = PromptState::from_text("/");

    let lines = prompt_render_lines(&state);

    assert_eq!(lines.len(), 9);

    assert_eq!(lines[1].plain_text(), "  /cancel            Cancel run");

    assert_eq!(lines[8].plain_text(), "  /copy              Copy text");

    assert!(!lines
        .iter()
        .any(|line| line.plain_text().contains("/history")));
}

#[test]

fn slash_completion_when_slash_subcommand_field_and_value_suggestions_match_legacy() {
    let mut state = state();

    state.commands = vec![model_command()];

    state.session.prompt = PromptState::from_text("/model ");

    let output = prompt_lines(&state).join("\n");

    assert!(output.contains("  add                Add model"));

    assert!(output.contains("  edit               Edit model"));

    assert!(output.contains("  remove             Remove model"));

    state.session.prompt = PromptState::from_text("/model add pro");

    let output = prompt_lines(&state).join("\n");

    assert!(output.contains("  provider:          provider - configured provider name, required"));

    state.session.prompt = PromptState::from_text("/model add provider:work id:g");

    let output = prompt_lines(&state).join("\n");

    assert!(output.contains("  google/gemini      id value"));

    assert!(!output.contains("openai/gpt-5.5"));
}

#[test]

fn slash_completion_when_slash_guidance_missing_invalid_optional_and_ready_match_legacy() {
    let mut state = state();

    state.commands = vec![model_command()];

    state.session.prompt = PromptState::from_text("/model add ");

    let output = prompt_lines(&state).join("\n");

    assert!(output.contains("missing: provider, id, reasoning."));

    assert!(output.contains("provider - configured provider name, required"));

    state.session.prompt =
        PromptState::from_text("/model add provider:work id:google/gemini reasoning:ultra");

    let output = prompt_lines(&state).join("\n");

    assert!(output.contains("invalid: reasoning:ultra"));

    assert!(output.contains("allowed: none, minimal, low, medium, high, xhigh"));

    state.session.prompt =
        PromptState::from_text("/model add provider:work id:google/gemini reasoning:high ");

    let output = prompt_lines(&state).join("\n");

    assert!(output.contains("optional: name"));

    state.session.prompt = PromptState::from_text(
        "/model add provider:work id:google/gemini reasoning:high name:coding ",
    );

    let output = prompt_lines(&state).join("\n");

    assert!(output.contains("ready: Enter to run"));
}

#[test]

fn slash_completion_when_slash_value_suggestions_more_row_is_info_and_not_selectable() {
    let mut state = state();

    state.commands = vec![model_command()];

    state.session.prompt = PromptState::from_text("/model add provider:openrouter id:");

    state.session.prompt.selected_completion = 8;

    let lines = prompt_render_lines(&state);

    assert!(lines
        .iter()
        .any(|line| line.plain_text().contains("[more 1 items...]")));

    let info = lines
        .iter()
        .find(|line| line.plain_text().contains("[more 1 items...]"))
        .expect("info row should render");

    assert_eq!(info.spans[0].style, RenderStyle::Dim);

    press(&mut state, KeyCode::Tab, KeyModifiers::empty());

    assert_ne!(
        state.session.prompt.text,
        "/model add provider:openrouter id:"
    );

    assert!(!state.session.prompt.text.contains("[more 1 items...]"));
}

#[test]

fn slash_completion_when_slash_space_accepts_value_and_guides_next_field_match_legacy() {
    let mut state = state();

    state.commands = vec![model_command()];

    state.session.prompt = PromptState::from_text("/model add provider:w");

    press(&mut state, KeyCode::Char(' '), KeyModifiers::empty());

    assert_eq!(state.session.prompt.text, "/model add provider:work id:");

    assert_eq!(state.session.prompt.cursor, state.session.prompt.text.len());
}

#[test]

fn slash_completion_when_slash_tab_guides_missing_field_and_submit_blocks_incomplete_command() {
    let mut state = state();

    state.commands = vec![model_command()];

    state.session.prompt = PromptState::from_text("/model add provider:work");

    press(&mut state, KeyCode::Tab, KeyModifiers::empty());

    assert_eq!(state.session.prompt.text, "/model add provider:work id:");

    press(&mut state, KeyCode::Enter, KeyModifiers::CONTROL);

    assert_eq!(state.session.transcript.len(), 0);

    assert_eq!(
        state.session.prompt.text,
        "/model add provider:work id:google/gemini reasoning:"
    );
}

#[test]

fn slash_completion_when_slash_enter_accepts_subcommand_and_complete_command_submits() {
    let mut state = state();

    state.commands = vec![model_command()];

    state.session.prompt = PromptState::from_text("/model ad");

    press(&mut state, KeyCode::Enter, KeyModifiers::empty());

    assert_eq!(state.session.prompt.text, "/model add provider:");

    state.session.prompt = PromptState::from_text(
        "/model add provider:work id:google/gemini reasoning:high name:coding ",
    );

    press(&mut state, KeyCode::Enter, KeyModifiers::CONTROL);

    assert_eq!(state.session.transcript.len(), 1);

    assert_eq!(state.session.prompt, PromptState::empty());
}

#[test]

fn slash_completion_when_slash_escape_dismisses_current_suggestions_then_falls_through() {
    let mut state = state();

    state.commands = vec![model_command()];

    state.session.prompt = PromptState::from_text("/model ");

    press(&mut state, KeyCode::Esc, KeyModifiers::empty());

    let output = prompt_lines(&state).join("\n");

    assert!(!output.contains("  add                Add model"));

    assert_eq!(state.session.prompt.text, "/model ");

    press(&mut state, KeyCode::Esc, KeyModifiers::empty());

    assert_eq!(state.session.prompt, PromptState::empty());
}

#[test]

fn slash_completion_when_slash_completion_reuses_existing_whitespace_match_legacy() {
    let mut state = state();

    state.commands = vec![model_command()];

    state.session.prompt = PromptState::from_text("/mod existing");

    state.session.prompt.cursor = "/mod".len();

    press(&mut state, KeyCode::Tab, KeyModifiers::empty());

    assert_eq!(state.session.prompt.text, "/model existing");

    assert_eq!(state.session.prompt.cursor, "/model ".len());
}
