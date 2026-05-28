use super::*;

/// Verifies the controller loop returns the closed session ID on exit.
#[tokio::test]
async fn run_controller_loop_when_exit_intent_returns_closed_session_id() {
    let controller = TuiRuntimeController::new_with_runner(
        TestTuiBootstrap::create("closed-session"),
        RecordingTuiTurnRunner::default(),
    )
    .unwrap();
    let expected_session_id = controller.current_session_id().to_owned();
    let (intent_sender, intent_receiver) = mpsc::unbounded_channel();
    let (_cancellation_sender, cancellation_receiver) = mpsc::unbounded_channel();
    let (_selection_sender, selection_receiver) = mpsc::unbounded_channel();
    let (state_sender, _state_receiver) = mpsc::unbounded_channel();

    intent_sender.send(Intent::RequestExit).unwrap();
    let closed_session_id = run_controller_loop(
        controller,
        intent_receiver,
        cancellation_receiver,
        selection_receiver,
        state_sender,
    )
    .await
    .unwrap();

    assert_eq!(closed_session_id, expected_session_id);
}
