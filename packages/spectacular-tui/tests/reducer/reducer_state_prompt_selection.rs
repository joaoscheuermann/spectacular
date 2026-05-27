use super::*;

#[test]

fn reduce_when_state_new_initializes_foundation_defaults() {
    let runtime = runtime("openrouter", "anthropic/claude");

    let display = display("OpenRouter", "Claude");

    let state = State::new(
        SessionId::new("session-1"),
        runtime.clone(),
        display.clone(),
    );

    assert_eq!(state.session.id.as_str(), "session-1");

    assert!(state.session.transcript.is_empty());

    assert_eq!(state.session.prompt, PromptState::empty());

    assert!(state.commands.is_empty());

    let mut expected_display = display.clone();

    expected_display.context_usage =
        ContextTokenUsage::default_for_window(runtime.context_window_tokens);

    assert_eq!(state.runtime, runtime);

    assert_eq!(state.display, expected_display);

    assert_eq!(state.status, Status::Idle);

    assert!(!state.exit_requested);

    assert_eq!(state.input_notice, None);

    assert_eq!(state.app_selection, Default::default());

    assert_eq!(state.spinner.current_frame(), "\u{2819}");

    assert_eq!(state.scroll.offset, 0);

    assert!(state.scroll.follow_tail);
}

/// Verifies prompt changes are isolated to the prompt field.

#[test]

fn reduce_when_prompt_changed_updates_only_prompt_state() {
    let mut state = state();

    let original = state.clone();

    let prompt = PromptState::from_text("hello");

    reduce(&mut state, ChatTuiAction::PromptChanged(prompt.clone()));

    assert_eq!(state.session.prompt, prompt);

    let mut expected = original;

    expected.session.prompt = prompt;

    assert_eq!(state, expected);
}

/// Verifies input notices are reducer-owned transient prompt feedback.

#[test]

fn reduce_when_input_notice_reported_and_cleared_updates_only_notice_state() {
    let mut state = state();

    reduce(
        &mut state,
        ChatTuiAction::InputNoticeReported {
            message: "Use Ctrl+V to paste".to_owned(),
        },
    );

    assert_eq!(state.input_notice, Some("Use Ctrl+V to paste".to_owned()));

    assert!(state.session.transcript.is_empty());

    reduce(&mut state, ChatTuiAction::InputNoticeCleared);

    assert_eq!(state.input_notice, None);

    assert!(state.session.transcript.is_empty());
}

/// Verifies normal prompt edits clear transient input notices.

#[test]

fn reduce_when_prompt_changed_with_input_notice_clears_notice() {
    let mut state = state();

    state.input_notice = Some("Use Ctrl+V to paste".to_owned());

    reduce(
        &mut state,
        ChatTuiAction::PromptChanged(PromptState::from_text("hello")),
    );

    assert_eq!(state.input_notice, None);
}

/// Verifies prompt changes clear stale app-wide rendered selection.

#[test]

fn reduce_when_prompt_changed_leaves_rendered_selection_to_view_state() {
    let mut state = state();

    state.session.prompt = PromptState::from_text("selected text");

    reduce(
        &mut state,
        ChatTuiAction::Resize {
            width: 80,

            height: 6,
        },
    );

    state.app_selection =
        spectacular_tui::selection::start_selection_at(&state, 2, 0).expect("prompt point");

    state.app_selection =
        spectacular_tui::selection::drag_selection_to(&state, 10, 0).expect("prompt drag");

    state.app_selection.copy_feedback = Some("Copied Selection".to_owned());
    let selection = state.app_selection.clone();

    reduce(
        &mut state,
        ChatTuiAction::PromptChanged(PromptState::from_text("next")),
    );

    assert_eq!(state.app_selection, selection);
}

/// Verifies modal and session changes do not mutate view-owned rendered selection state.

#[test]

fn reduce_when_modal_and_session_changes_leave_rendered_selection_to_view_state() {
    let mut state = state();

    state.session.prompt = PromptState::from_text("selected text");

    reduce(
        &mut state,
        ChatTuiAction::Resize {
            width: 80,

            height: 6,
        },
    );

    state.app_selection =
        spectacular_tui::selection::start_selection_at(&state, 2, 0).expect("prompt point");

    state.app_selection =
        spectacular_tui::selection::drag_selection_to(&state, 10, 0).expect("prompt drag");

    state.app_selection.copy_feedback = Some("Copied Selection".to_owned());
    let selection = state.app_selection.clone();

    reduce(
        &mut state,
        ChatTuiAction::SelectionPromptChanged(Some(spectacular_tui::SelectionPromptState::new(
            "Pick one",
            "",
            vec!["alpha".to_owned()],
        ))),
    );

    assert_eq!(state.app_selection, selection);

    state.app_selection =
        spectacular_tui::selection::start_selection_at(&state, 0, 0).expect("modal point");

    state.app_selection =
        spectacular_tui::selection::drag_selection_to(&state, 4, 0).expect("modal drag");

    state.app_selection.copy_feedback = Some("Copied Selection".to_owned());
    let selection = state.app_selection.clone();

    reduce(
        &mut state,
        ChatTuiAction::SessionChanged {
            id: SessionId::new("session-2"),
        },
    );

    assert_eq!(state.app_selection, selection);
}

/// Verifies rendered-selection copy feedback is scoped to the active rendered selection.

#[test]

fn reduce_when_rendered_selection_feedback_lifecycle_tracks_active_selection() {
    let mut state = state();

    spectacular_tui::apply_view_action_to_state(
        &mut state,
        ViewAction::RenderedSelectionFeedbackReported {
            message: "Copied Selection".to_owned(),
        },
    );

    assert_eq!(state.app_selection.copy_feedback, None);

    state.session.prompt = PromptState::from_text("selected text");

    reduce(
        &mut state,
        ChatTuiAction::Resize {
            width: 80,

            height: 6,
        },
    );

    state.app_selection =
        spectacular_tui::selection::start_selection_at(&state, 2, 0).expect("prompt point");

    state.app_selection =
        spectacular_tui::selection::drag_selection_to(&state, 10, 0).expect("prompt drag");

    spectacular_tui::apply_view_action_to_state(
        &mut state,
        ViewAction::RenderedSelectionFeedbackReported {
            message: "Copied Selection".to_owned(),
        },
    );

    assert_eq!(
        state.app_selection.copy_feedback,
        Some("Copied Selection".to_owned())
    );

    let replacement =
        spectacular_tui::selection::start_selection_at(&state, 4, 0).expect("replacement point");

    spectacular_tui::apply_view_action_to_state(
        &mut state,
        ViewAction::RenderedSelectionChanged(replacement),
    );

    assert_eq!(state.app_selection.copy_feedback, None);

    state.app_selection =
        spectacular_tui::selection::drag_selection_to(&state, 12, 0).expect("replacement drag");

    spectacular_tui::apply_view_action_to_state(
        &mut state,
        ViewAction::RenderedSelectionFeedbackReported {
            message: "Copied Selection".to_owned(),
        },
    );

    spectacular_tui::apply_view_action_to_state(&mut state, ViewAction::RenderedSelectionCleared);

    assert_eq!(state.app_selection.copy_feedback, None);
}
