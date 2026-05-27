use spectacular_tui::{
    fake_streaming_plan, fake_streaming_runtime_finding, render_state_to_string,
    FakeStreamingTimeline,
};

/// Runs the deterministic fake streaming prototype and prints the final IOCraft projection.
#[tokio::main(flavor = "current_thread")]
async fn main() {
    let mut timeline = FakeStreamingTimeline::new(fake_streaming_plan());
    timeline.run_until_finished().await;

    println!("{}", fake_streaming_runtime_finding());
    println!("{}", render_state_to_string(timeline.state(), Some(120)));
}
