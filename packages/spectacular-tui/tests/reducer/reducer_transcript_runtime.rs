use super::*;

/// Verifies submitting a prompt appends semantic prompt content and clears prompt state.
#[test]

fn reduce_when_submit_prompt_appends_user_prompt_and_clears_prompt() {
    let mut state = state();

    state.session.prompt = PromptState::from_text("run this");

    reduce(
        &mut state,
        ChatTuiAction::SubmitPrompt {
            id: TranscriptItemId::new("prompt-1"),

            text: "run this".to_owned(),
        },
    );

    assert_eq!(state.session.prompt, PromptState::empty());

    assert_eq!(state.status, Status::Idle);

    assert_eq!(state.session.transcript.len(), 1);

    assert_eq!(state.session.transcript[0].id.as_str(), "prompt-1");

    assert_eq!(state.session.transcript[0].timestamp.value(), 0);

    assert!(matches!(

        &state.session.transcript[0].content,

        TranscriptItemContent::UserPrompt(item) if item.text == "run this"

    ));
}

/// Verifies duplicate prompt actions with the same ID update instead of appending.

#[test]

fn reduce_when_submit_prompt_is_idempotent_by_transcript_id() {
    let mut state = state();

    reduce(
        &mut state,
        ChatTuiAction::SubmitPrompt {
            id: TranscriptItemId::new("prompt-1"),

            text: "first text".to_owned(),
        },
    );

    reduce(
        &mut state,
        ChatTuiAction::SubmitPrompt {
            id: TranscriptItemId::new("prompt-1"),

            text: "updated text".to_owned(),
        },
    );

    assert_eq!(state.session.transcript.len(), 1);

    assert!(matches!(

        &state.session.transcript[0].content,

        TranscriptItemContent::UserPrompt(item) if item.text == "updated text"

    ));
}

/// Verifies identical prompt text with different IDs remains distinct turns.

#[test]

fn reduce_when_submit_prompt_keeps_same_text_with_different_ids_distinct() {
    let mut state = state();

    reduce(
        &mut state,
        ChatTuiAction::SubmitPrompt {
            id: TranscriptItemId::new("prompt-1"),

            text: "repeat".to_owned(),
        },
    );

    reduce(
        &mut state,
        ChatTuiAction::SubmitPrompt {
            id: TranscriptItemId::new("prompt-2"),

            text: "repeat".to_owned(),
        },
    );

    assert_eq!(state.session.transcript.len(), 2);

    assert_eq!(state.session.transcript[0].id.as_str(), "prompt-1");

    assert_eq!(state.session.transcript[1].id.as_str(), "prompt-2");
}

/// Verifies agent start and finish update status deterministically.

#[test]

fn reduce_when_agent_started_and_finished_update_status() {
    let mut state = state();

    let prior_context_usage = ContextTokenUsage::new(90, Some(100));

    state.session.context_usage = Some(prior_context_usage);

    state.display.context_usage = Some(prior_context_usage);

    state.session.turn_usage = Some(TurnTokenUsage {
        input_tokens: 10,

        output_tokens: 20,

        total_tokens: 30,

        has_provider_metadata: true,
    });

    state.display.turn_usage = state.session.turn_usage;

    state.session.total_usage = Some(TurnTokenUsage {
        input_tokens: 100,

        output_tokens: 200,

        total_tokens: 300,

        has_provider_metadata: true,
    });

    state.display.total_usage = state.session.total_usage;

    reduce(&mut state, ChatTuiAction::AgentStarted);

    assert_eq!(
        state.status,
        Status::Running {
            activity: Activity::WaitingForModel,

            cancellable: true,
        }
    );

    assert_eq!(state.session.context_usage, None);

    assert_eq!(state.display.context_usage, Some(prior_context_usage));

    assert_eq!(state.session.turn_usage, None);

    assert_eq!(state.display.turn_usage, None);

    assert_eq!(
        state.session.total_usage,
        Some(TurnTokenUsage {
            input_tokens: 100,

            output_tokens: 200,

            total_tokens: 300,

            has_provider_metadata: true,
        })
    );

    assert_eq!(state.display.total_usage, state.session.total_usage);

    reduce(&mut state, ChatTuiAction::AgentFinished);

    assert_eq!(state.status, Status::Idle);
}

/// Verifies failed and cancelled actions move out of running status deterministically.

#[test]

fn reduce_when_agent_failed_and_cancelled_leave_running_state() {
    let mut failed = state();

    reduce(&mut failed, ChatTuiAction::AgentStarted);

    reduce(
        &mut failed,
        ChatTuiAction::AgentFailed {
            message: "network".to_owned(),

            details: None,
        },
    );

    assert_eq!(
        failed.status,
        Status::Failed {
            message: "network".to_owned(),
        }
    );

    assert!(matches!(

        &failed.session.transcript[0].content,

        TranscriptItemContent::Error(item) if item.message == "network" && item.details.is_none()

    ));

    let mut failed_with_details = state();

    reduce(
        &mut failed_with_details,
        ChatTuiAction::AgentFailed {
            message: "provider failed".to_owned(),

            details: Some("kind: authentication\nhttp status: 401".to_owned()),
        },
    );

    assert!(matches!(

        &failed_with_details.session.transcript[0].content,

        TranscriptItemContent::Error(item)

            if item.message == "provider failed"

                && item.details.as_deref() == Some("kind: authentication\nhttp status: 401")

    ));

    let mut cancelled = state();

    reduce(&mut cancelled, ChatTuiAction::AgentStarted);

    reduce(
        &mut cancelled,
        ChatTuiAction::AgentCancelled {
            reason: "user".to_owned(),
        },
    );

    assert_eq!(cancelled.status, Status::Idle);

    assert!(matches!(

        &cancelled.session.transcript[0].content,

        TranscriptItemContent::Cancellation(item) if item.reason == "user"

    ));
}

/// Verifies cancellable running state transitions to cancellation.

#[test]

fn reduce_when_cancel_run_moves_cancellable_running_state_to_cancelling() {
    let mut state = state();

    reduce(&mut state, ChatTuiAction::AgentStarted);

    reduce(&mut state, ChatTuiAction::CancelRun);

    assert_eq!(state.status, Status::Cancelling);
}

/// Verifies metadata replacement actions update reducer-owned visible state.

#[test]

fn reduce_when_runtime_and_display_metadata_actions_replace_state() {
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

    let mut expected_display = display.clone();

    expected_display.context_usage =
        ContextTokenUsage::default_for_window(runtime.context_window_tokens);

    assert_eq!(state.runtime, runtime);

    assert_eq!(state.display, expected_display);
}

/// Verifies worktree refreshes update only reducer-owned footer metadata.

#[test]

fn reduce_when_worktree_metadata_action_updates_footer_metadata() {
    let mut state = state();

    reduce(
        &mut state,
        ChatTuiAction::WorktreeMetadataChanged(Some(WorktreeMetadata::new("main*"))),
    );

    assert_eq!(state.display.worktree, Some(WorktreeMetadata::new("main*")));

    reduce(&mut state, ChatTuiAction::WorktreeMetadataChanged(None));

    assert_eq!(state.display.worktree, None);
}

/// Verifies spinner ticks only advance explicit spinner state.

#[test]

fn reduce_when_spinner_tick_advances_spinner_state_without_terminal_output() {
    let mut state = state();

    let first = state.spinner.current_frame();

    reduce(&mut state, ChatTuiAction::SpinnerTick);

    assert_ne!(state.spinner.current_frame(), first);

    assert_eq!(state.status, Status::Idle);
}

/// Verifies command metadata loading replaces the full command descriptor list.

#[test]

fn reduce_when_commands_loaded_replaces_command_metadata() {
    let mut state = state();

    let commands = vec![
        CommandDescriptor::new("config", "Manage configuration"),
        CommandDescriptor::new("session", "Manage sessions"),
    ];

    reduce(&mut state, ChatTuiAction::CommandsLoaded(commands.clone()));

    assert_eq!(state.commands, commands);
}

/// Verifies session creation updates semantic session state and appends opening banner content.

#[test]

fn reduce_when_session_created_starts_new_session_with_opening_banner() {
    let mut state = state();

    state.scroll.offset = 5;

    state.scroll.follow_tail = false;

    reduce(
        &mut state,
        ChatTuiAction::SessionCreated {
            id: SessionId::new("session-2"),

            banner: spectacular_tui::OpeningBannerItem::new(
                "1.2.3",
                "model",
                "low",
                "/workspace",
                "session-2",
            ),
        },
    );

    assert_eq!(state.session.id.as_str(), "session-2");

    assert_eq!(state.scroll.offset, 5);

    assert!(!state.scroll.follow_tail);

    assert_eq!(state.session.transcript.len(), 1);

    assert_eq!(
        state.session.transcript[0].id.as_str(),
        "opening-banner-session-2"
    );

    assert_eq!(state.session.transcript[0].timestamp.value(), 0);

    assert!(matches!(

        &state.session.transcript[0].content,

        TranscriptItemContent::OpeningBanner(banner)

            if banner.version == "1.2.3"

                && banner.model == "model"

                && banner.reasoning == "low"

                && banner.directory == "/workspace"

                && banner.session_id == "session-2"

    ));
}
