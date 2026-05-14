use spectacular_tui::{
    reduce, Activity, ChatTuiAction, CommandDescriptor, ContextTokenUsage, DisplayMetadata,
    PromptState, ReasoningLevel, RuntimeSelection, SessionId, State, Status,
};

/// Builds a representative runtime selection for reducer tests.
fn runtime(provider: &str, model: &str) -> RuntimeSelection {
    RuntimeSelection::new(
        "openai-compatible",
        provider,
        model,
        ReasoningLevel::Low,
        Some(4096),
    )
}

/// Builds visible display metadata for reducer tests.
fn display(provider: &str, model: &str) -> DisplayMetadata {
    DisplayMetadata::new(provider, model, "low", "/workspace", "session", None)
}

/// Builds an initialized state with stable metadata.
fn state() -> State {
    State::new(
        SessionId::new("session-1"),
        runtime("provider", "model"),
        display("provider", "model"),
    )
}

/// Verifies initial state has empty collections, supplied metadata, idle status, first spinner frame, and tail following scroll.
#[test]
fn state_new_initializes_foundation_defaults() {
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
    assert_eq!(state.runtime, runtime);
    assert_eq!(state.display, display);
    assert_eq!(state.status, Status::Idle);
    assert_eq!(state.spinner.current_frame(), "⠋");
    assert_eq!(state.scroll.offset, 0);
    assert!(state.scroll.follow_tail);
}

/// Verifies prompt changes are isolated to the prompt field.
#[test]
fn prompt_changed_updates_only_prompt_state() {
    let mut state = state();
    let original = state.clone();
    let prompt = PromptState::from_text("hello");

    reduce(&mut state, ChatTuiAction::PromptChanged(prompt.clone()));

    assert_eq!(state.session.prompt, prompt);
    let mut expected = original;
    expected.session.prompt = prompt;
    assert_eq!(state, expected);
}

/// Verifies submitting a prompt clears only reducer-owned prompt text.
#[test]
fn submit_prompt_clears_prompt_without_runtime_side_effects() {
    let mut state = state();
    state.session.prompt = PromptState::from_text("run this");

    reduce(
        &mut state,
        ChatTuiAction::SubmitPrompt("run this".to_owned()),
    );

    assert_eq!(state.session.prompt, PromptState::empty());
    assert_eq!(state.status, Status::Idle);
    assert!(state.session.transcript.is_empty());
}

/// Verifies agent start and finish update status deterministically.
#[test]
fn agent_started_and_finished_update_status() {
    let mut state = state();

    reduce(&mut state, ChatTuiAction::AgentStarted);

    assert_eq!(
        state.status,
        Status::Running {
            activity: Activity::WaitingForModel,
            cancellable: true,
        }
    );

    reduce(&mut state, ChatTuiAction::AgentFinished);

    assert_eq!(state.status, Status::Idle);
}

/// Verifies failed and cancelled actions move out of running status deterministically.
#[test]
fn agent_failed_and_cancelled_leave_running_state() {
    let mut failed = state();
    reduce(&mut failed, ChatTuiAction::AgentStarted);
    reduce(
        &mut failed,
        ChatTuiAction::AgentFailed {
            message: "network".to_owned(),
        },
    );
    assert_eq!(
        failed.status,
        Status::Failed {
            message: "network".to_owned(),
        }
    );

    let mut cancelled = state();
    reduce(&mut cancelled, ChatTuiAction::AgentStarted);
    reduce(
        &mut cancelled,
        ChatTuiAction::AgentCancelled {
            reason: "user".to_owned(),
        },
    );
    assert_eq!(
        cancelled.status,
        Status::Failed {
            message: "user".to_owned(),
        }
    );
}

/// Verifies cancellable running state transitions to cancellation.
#[test]
fn cancel_run_moves_cancellable_running_state_to_cancelling() {
    let mut state = state();
    reduce(&mut state, ChatTuiAction::AgentStarted);

    reduce(&mut state, ChatTuiAction::CancelRun);

    assert_eq!(state.status, Status::Cancelling);
}

/// Verifies metadata replacement actions update reducer-owned visible state.
#[test]
fn runtime_and_display_metadata_actions_replace_state() {
    let mut state = state();
    let runtime = runtime("openrouter", "new/model");
    let display = display("OpenRouter", "New Model");

    reduce(
        &mut state,
        ChatTuiAction::RuntimeSelectionChanged(runtime.clone()),
    );
    reduce(
        &mut state,
        ChatTuiAction::DisplayMetadataChanged(display.clone()),
    );

    assert_eq!(state.runtime, runtime);
    assert_eq!(state.display, display);
}

/// Verifies spinner ticks only advance explicit spinner state.
#[test]
fn spinner_tick_advances_spinner_state_without_terminal_output() {
    let mut state = state();
    let first = state.spinner.current_frame();

    reduce(&mut state, ChatTuiAction::SpinnerTick);

    assert_ne!(state.spinner.current_frame(), first);
    assert_eq!(state.status, Status::Idle);
}

/// Verifies command metadata loading replaces the full command descriptor list.
#[test]
fn commands_loaded_replaces_command_metadata() {
    let mut state = state();
    let commands = vec![
        CommandDescriptor::new("config", "Manage configuration"),
        CommandDescriptor::new("session", "Manage sessions"),
    ];

    reduce(&mut state, ChatTuiAction::CommandsLoaded(commands.clone()));

    assert_eq!(state.commands, commands);
}

/// Verifies usage updates both session usage and display metadata usage.
#[test]
fn usage_updated_updates_session_and_display_usage() {
    let mut state = state();
    let usage = ContextTokenUsage::new(100, Some(1000));

    reduce(&mut state, ChatTuiAction::UsageUpdated(usage));

    assert_eq!(state.session.usage, Some(usage));
    assert_eq!(state.display.usage, Some(usage));
}

/// Verifies transcript scrolling updates offset and tail following rules.
#[test]
fn scroll_transcript_updates_offset_and_follow_tail() {
    let mut state = state();

    reduce(&mut state, ChatTuiAction::ScrollTranscript(3));

    assert_eq!(state.scroll.offset, 3);
    assert!(!state.scroll.follow_tail);

    reduce(&mut state, ChatTuiAction::ScrollTranscript(-2));

    assert_eq!(state.scroll.offset, 1);
    assert!(!state.scroll.follow_tail);

    reduce(&mut state, ChatTuiAction::ScrollTranscript(-5));

    assert_eq!(state.scroll.offset, 0);
    assert!(state.scroll.follow_tail);
}

/// Verifies large scroll deltas saturate instead of wrapping the scroll offset.
#[test]
fn scroll_transcript_handles_large_deltas_without_overflow() {
    let mut state = state();

    reduce(&mut state, ChatTuiAction::ScrollTranscript(i32::MAX));

    assert_eq!(state.scroll.offset, i32::MAX as u32);
    assert!(!state.scroll.follow_tail);

    reduce(&mut state, ChatTuiAction::ScrollTranscript(i32::MAX));

    assert_eq!(state.scroll.offset, 4_294_967_294);
    assert!(!state.scroll.follow_tail);

    reduce(&mut state, ChatTuiAction::ScrollTranscript(i32::MIN));

    assert_eq!(state.scroll.offset, 2_147_483_646);
    assert!(!state.scroll.follow_tail);
}
