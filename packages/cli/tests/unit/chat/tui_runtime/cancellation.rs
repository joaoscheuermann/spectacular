use super::*;

/// Verifies TUI cancellation intents cancel active runtime work through the runner seam.
#[tokio::test]
async fn cancel_intent_cancels_active_runner() {
    let bootstrap = TestTuiBootstrap::create("session-1");
    let mut controller = TuiRuntimeController::new_with_runner(
        bootstrap,
        RecordingTuiTurnRunner {
            running: true,
            ..RecordingTuiTurnRunner::default()
        },
    )
    .unwrap();

    controller.handle_intent(Intent::CancelRun).await.unwrap();

    assert_eq!(controller.runner().cancel_count, 1);
    assert_eq!(
        controller.state().status,
        tui::Status::Cancelling
    );
}

/// Verifies cancellation reaches an active prompt even while the controller awaits the turn.
#[tokio::test]
async fn cancel_signal_reaches_active_prompt_run() {
    let bootstrap = TestTuiBootstrap::create("active-cancel-session");
    let controller = TuiRuntimeController::new_with_runner(bootstrap, CancellingTuiTurnRunner)
        .unwrap();
    let (intent_sender, intent_receiver) = mpsc::unbounded_channel();
    let (cancellation_sender, cancellation_receiver) = mpsc::unbounded_channel();
    let (_selection_sender, selection_receiver) = mpsc::unbounded_channel();
    let (state_sender, mut state_receiver) = mpsc::unbounded_channel();
    let expected_session_id = controller.current_session_id().to_owned();
    let controller_loop = run_controller_loop(
        controller,
        intent_receiver,
        cancellation_receiver,
        selection_receiver,
        state_sender,
    );
    let driver = async move {
        intent_sender
            .send(Intent::SubmitPrompt {
                id: TranscriptItemId::new("prompt-1"),
                text: "cancel me".to_owned(),
            })
            .unwrap();
        cancellation_sender.send(()).unwrap();

        let cancelled_state = next_state_matching(&mut state_receiver, |state| {
            matches!(state.status, tui::Status::Idle)
                && state.session.transcript.iter().any(|item| {
                    matches!(
                        &item.content,
                        TranscriptItemContent::Cancellation(cancellation)
                            if cancellation.reason == "test cancellation"
                    )
                })
        })
        .await;

        assert!(cancelled_state.is_some());
        intent_sender.send(Intent::RequestExit).unwrap();
    };
    let (controller_result, _) = tokio::join!(controller_loop, driver);
    assert_eq!(controller_result.unwrap(), expected_session_id);
}

/// Verifies cancellation reaches a prompt run queued by a TUI retry command.
#[tokio::test]
async fn cancel_signal_reaches_retry_follow_up_prompt_run() {
    let bootstrap = TestTuiBootstrap::create("retry-cancel-session");
    let controller = TuiRuntimeController::new_with_runner(bootstrap, CancellingTuiTurnRunner)
        .unwrap();
    controller
        .model()
        .append_agent_event(&AgentEvent::user_prompt("retry cancel"))
        .unwrap();
    let (intent_sender, intent_receiver) = mpsc::unbounded_channel();
    let (cancellation_sender, cancellation_receiver) = mpsc::unbounded_channel();
    let (_selection_sender, selection_receiver) = mpsc::unbounded_channel();
    let (state_sender, mut state_receiver) = mpsc::unbounded_channel();
    let expected_session_id = controller.current_session_id().to_owned();
    let controller_loop = run_controller_loop(
        controller,
        intent_receiver,
        cancellation_receiver,
        selection_receiver,
        state_sender,
    );
    let driver = async move {
        intent_sender
            .send(Intent::SubmitPrompt {
                id: TranscriptItemId::new("prompt-1"),
                text: "/retry".to_owned(),
            })
            .unwrap();
        cancellation_sender.send(()).unwrap();

        let cancelled_state = next_state_matching(&mut state_receiver, |state| {
            matches!(state.status, tui::Status::Idle)
                && state.session.transcript.iter().any(|item| {
                    matches!(
                        &item.content,
                        TranscriptItemContent::Cancellation(cancellation)
                            if cancellation.reason == "test cancellation"
                    )
                })
        })
        .await;

        assert!(cancelled_state.is_some());
        intent_sender.send(Intent::RequestExit).unwrap();
    };
    let (controller_result, _) = tokio::join!(controller_loop, driver);
    assert_eq!(controller_result.unwrap(), expected_session_id);
}
