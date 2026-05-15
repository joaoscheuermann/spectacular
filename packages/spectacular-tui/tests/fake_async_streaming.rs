use spectacular_tui::{
    fake_streaming_plan, FakeStreamingTickOutcome, FakeStreamingTimeline, PromptState,
    TUI_SPINNER_TICK_INTERVAL,
};
use std::time::Duration;

/// Verifies the fake stream drives one assistant message through incremental deltas.
#[tokio::test]
async fn fake_stream_appends_assistant_deltas_to_one_item() {
    let mut harness = FakeStreamingTimeline::new(fake_streaming_plan());

    harness.run_until_finished().await;

    assert_eq!(
        harness.assistant_text("assistant-1"),
        Some("Hello from the fake async stream.".to_owned())
    );
    assert_eq!(harness.assistant_item_count(), 1);
}

/// Verifies reasoning lifecycle events accumulate onto one active reasoning item.
#[tokio::test]
async fn fake_reasoning_stream_appends_deltas_to_one_item() {
    let mut harness = FakeStreamingTimeline::new(fake_streaming_plan());

    harness.run_until_finished().await;

    assert_eq!(
        harness.reasoning_text("reasoning-1"),
        Some("Plan the response, then stream it."),
    );
    assert_eq!(harness.reasoning_item_count(), 1);
}

/// Verifies tool lifecycle updates one transcript item from running to finished.
#[tokio::test]
async fn fake_tool_lifecycle_finishes_one_tool_item() {
    let mut harness = FakeStreamingTimeline::new(fake_streaming_plan());

    harness.run_until_finished().await;

    let tool = harness.tool("tool-item-1").expect("tool item should exist");
    assert_eq!(tool.tool_call_id, "tool-call-1");
    assert_eq!(
        tool.output_preview.as_deref(),
        Some("found match in src/lib.rs")
    );
    assert!(harness.is_tool_finished("tool-item-1"));
}

/// Verifies command lifecycle updates one semantic command item without terminal output rows.
#[tokio::test]
async fn fake_command_lifecycle_finishes_one_command_item() {
    let mut harness = FakeStreamingTimeline::new(fake_streaming_plan());

    harness.run_until_finished().await;

    let command = harness
        .command("command-item-1")
        .expect("command item should exist");
    assert_eq!(command.command_id, "command-1");
    assert_eq!(command.output, "checking fake workspace\nfinished\n");
    assert!(harness.is_command_finished("command-item-1"));
    assert!(!harness.rendered_output().contains("raw terminal"));
}

/// Verifies AgentFinished returns status to idle after fake streaming completes.
#[tokio::test]
async fn agent_finished_returns_status_to_idle() {
    let mut harness = FakeStreamingTimeline::new(fake_streaming_plan());

    harness.run_until_finished().await;

    assert!(harness.is_idle());
}

/// Verifies no additional spinner ticks are emitted after a terminal fake event.
#[tokio::test]
async fn terminal_fake_events_stop_spinner_ticks() {
    let mut harness = FakeStreamingTimeline::new(fake_streaming_plan());

    harness.run_until_finished().await;
    let ticks_at_finish = harness.spinner_tick_count();
    harness.run_for(Duration::from_secs(1)).await;

    assert_eq!(harness.spinner_tick_count(), ticks_at_finish);
}

/// Verifies cancellation and failure scenarios deterministically leave running state.
#[tokio::test]
async fn terminal_fake_events_leave_streaming_state() {
    let mut cancelled = FakeStreamingTimeline::new(spectacular_tui::fake_cancellation_plan());
    cancelled.run_until_finished().await;

    assert!(cancelled.is_idle());
    assert!(cancelled
        .rendered_output()
        .contains("cancelled by fake runtime"));

    let mut failed = FakeStreamingTimeline::new(spectacular_tui::fake_failure_plan());
    failed.run_until_finished().await;

    assert!(failed.is_failed());
    assert!(failed.rendered_output().contains("fake runtime failure"));
}

/// Verifies prompt changes applied during streaming survive later fake runtime actions.
#[tokio::test]
async fn prompt_changes_during_fake_streaming_are_retained() {
    let mut harness = FakeStreamingTimeline::new(fake_streaming_plan());

    harness.run_for(Duration::from_millis(25)).await;
    harness.apply_prompt(PromptState::from_text("local edit while streaming"));
    harness.run_until_finished().await;

    assert_eq!(
        harness.state().session.prompt.text,
        "local edit while streaming"
    );
}

/// Verifies spinner ticks are independent of agent deltas at the intended cadence.
#[tokio::test]
async fn spinner_ticks_independently_at_documented_cadence() {
    let mut harness = FakeStreamingTimeline::new(fake_streaming_plan());

    let first = harness.step().await;
    harness.run_for(Duration::from_millis(90)).await;

    assert_eq!(TUI_SPINNER_TICK_INTERVAL, Duration::from_millis(90));
    assert_eq!(first, FakeStreamingTickOutcome::AgentAction);
    assert_eq!(harness.spinner_tick_count(), 1);
}

/// Verifies the Tokio/IOCraft compatibility finding is exposed for Effort 09.
#[test]
fn tokio_iocraft_compatibility_finding_is_documented() {
    let finding = spectacular_tui::fake_streaming_runtime_finding();

    assert!(finding.contains("Tokio"));
    assert!(finding.contains("IOCraft"));
    assert!(finding.contains("isolated prototype executor"));
}

/// Verifies normal fake streaming updates render from state without direct terminal printing.
#[tokio::test]
async fn fake_streaming_requires_no_direct_terminal_printing() {
    let mut harness = FakeStreamingTimeline::new(fake_streaming_plan());

    harness.run_until_finished().await;

    assert_eq!(harness.direct_terminal_writes(), 0);
    assert!(harness.rendered_output().contains("Hello from"));
    assert!(!harness.rendered_output().contains("Assistant:"));
}
