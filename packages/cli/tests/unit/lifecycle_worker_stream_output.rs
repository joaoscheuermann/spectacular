use std::cell::RefCell;
use std::io::{self, Write};
use std::rc::Rc;

#[test]
fn handle_lifecycle_worker_stream_empty_stream_starts_with_timestamped_started_line() {
    let mut client = RecordingLifecycleClient::new().with_stream(Ok(Vec::new()));

    let output = lifecycle_output(Command::Worker(worker_args("worker-123")), &mut client);

    assert_eq!(
        client.calls(),
        &[ClientCall::Stream {
            worker_id: "worker-123".to_owned(),
            addr: None,
        }]
    );
    assert_line_count(&output, 1);
    assert_first_timestamped_line(&output, "started");
    assert_no_stream_scaffolding(&output);
}

#[test]
fn handle_lifecycle_worker_stream_writes_started_before_first_event_and_flushes_during_stream() {
    let shared = Rc::new(RefCell::new(Vec::new()));
    let observed_before_close = Rc::new(RefCell::new(false));
    let mut writer = SharedWriter(Rc::clone(&shared));
    let mut client = StreamingObservationClient {
        buffer: Rc::clone(&shared),
        observed_before_close: Rc::clone(&observed_before_close),
    };

    let result = handle_lifecycle_command_with_writer(
        Command::Worker(worker_args("worker-123")),
        &mut client,
        &mut writer,
    )
    .expect("streaming worker command should succeed");

    assert!(result.is_none());
    assert!(
        *observed_before_close.borrow(),
        "started event should be visible before stream closure"
    );
    let output = strip_ansi_codes(&String::from_utf8(shared.borrow().clone()).unwrap());
    assert_first_timestamped_line(&output, "started");
    assert_order(&output, &["started", "worker accepted", "prompt/requirements completed"]);
    assert_all_lifecycle_lines_are_timestamped(&output);
    assert_no_stream_scaffolding(&output);
}

#[test]
fn handle_lifecycle_worker_stream_events_render_timestamped_lines_in_daemon_order() {
    let mut client = RecordingLifecycleClient::new().with_stream(Ok(vec![
        stream_event(0, "accepted", "accepted", "worker accepted"),
        stream_event(1, "starting", "starting", "worker starting"),
        stream_event(2, "repo_preparation", "running", "repo ready"),
    ]));

    let output = lifecycle_output(Command::Worker(worker_args("worker-123")), &mut client);

    assert_eq!(
        client.calls(),
        &[ClientCall::Stream {
            worker_id: "worker-123".to_owned(),
            addr: None,
        }]
    );
    assert_first_timestamped_line(&output, "started");
    assert_line_count(&output, 4);
    assert_all_lifecycle_lines_are_timestamped(&output);
    assert_order(
        &output,
        &["started", "worker accepted", "worker starting", "repo ready"],
    );
    assert_no_stream_scaffolding(&output);
    assert_not_contains(&output, "daemon client not wired");
}

#[test]
fn handle_lifecycle_worker_stream_event_occurred_at_renders_fixed_timestamp() {
    let mut client = RecordingLifecycleClient::new().with_stream(Ok(vec![stream_event_at(
        0,
        "accepted",
        "accepted",
        "worker accepted",
        fixed_time(3_661),
    )]));

    let output = lifecycle_output(Command::Worker(worker_args("worker-123")), &mut client);

    assert_contains(&output, &format!("[{}]", fixed_timestamp(3_661)));
    assert_contains(&output, "worker accepted");
    assert_all_lifecycle_lines_are_timestamped(&output);
    assert_no_stream_scaffolding(&output);
}

#[test]
fn handle_lifecycle_worker_stream_event_without_occurred_at_uses_timestamped_fallback() {
    let mut client = RecordingLifecycleClient::new().with_stream(Ok(vec![stream_event(
        1,
        "starting",
        "starting",
        "worker starting",
    )]));

    let output = lifecycle_output(Command::Worker(worker_args("worker-123")), &mut client);
    let lines = output_lines(&output);

    assert_line_count(&output, 2);
    assert_first_timestamped_line(&output, "started");
    assert_timestamped_line_contains(lines[1], "worker starting");
    assert_no_stream_scaffolding(&output);
}

#[test]
fn handle_lifecycle_worker_stream_history_truncated_renders_one_timestamped_line() {
    let mut client =
        RecordingLifecycleClient::new().with_stream(Ok(vec![LifecycleStreamItem::HistoryTruncated {
            requested_from_sequence: 0,
            first_available_sequence: 42,
        }]));

    let output = lifecycle_output(Command::Worker(worker_args("worker-123")), &mut client);
    let lines = output_lines(&output);

    assert_line_count(&output, 2);
    assert_first_timestamped_line(&output, "started");
    assert_timestamped_line_contains(lines[1], "history was truncated");
    assert_timestamped_line_contains(lines[1], "requested sequence 0");
    assert_timestamped_line_contains(lines[1], "first available sequence 42");
    assert_no_stream_scaffolding(&output);
}

#[test]
fn handle_lifecycle_worker_stream_waiting_for_input_renders_one_timestamped_line() {
    let mut client = RecordingLifecycleClient::new().with_stream(Ok(vec![waiting_input_event(
        6,
        "Approve repo access?",
        "request-1",
    )]));

    let output = lifecycle_output(Command::Worker(worker_args("worker-123")), &mut client);
    let lines = output_lines(&output);

    assert_line_count(&output, 2);
    assert_first_timestamped_line(&output, "started");
    assert_timestamped_line_contains(lines[1], "waiting for input");
    assert_timestamped_line_contains(lines[1], "request-1");
    assert_timestamped_line_contains(lines[1], "Approve repo access?");
    assert_not_contains(&output, "Answer:");
    assert_not_contains(&output, "doric answer worker-123 request-1 --text");
    assert_no_stream_scaffolding(&output);
}

#[test]
fn handle_lifecycle_worker_stream_clone_activity_displays_incoming_clone_message() {
    let repo = "https://github.com/org/repo.git";
    let mut client = RecordingLifecycleClient::new().with_stream(Ok(vec![stream_event(
        2,
        "repo_preparation",
        "running",
        &format!("cloning repo: {repo}"),
    )]));

    let output = lifecycle_output(Command::Worker(worker_args("worker-123")), &mut client);
    let lines = output_lines(&output);

    assert_line_count(&output, 2);
    assert_first_timestamped_line(&output, "started");
    assert_timestamped_line_contains(lines[1], "cloning repo: https://github.com/org/repo.git");
    assert_order(&output, &["started", "cloning repo: https://github.com/org/repo.git"]);
    assert_no_stream_scaffolding(&output);
}

#[test]
fn handle_lifecycle_worker_stream_fast_burst_preserves_event_order() {
    let events = (0_u64..8)
        .map(|sequence| {
            stream_event(
                sequence,
                "prompt_delta",
                "running",
                &format!("burst event {sequence}"),
            )
        })
        .collect();
    let mut client = RecordingLifecycleClient::new().with_stream(Ok(events));

    let output = lifecycle_output(Command::Worker(worker_args("worker-123")), &mut client);

    assert_line_count(&output, 9);
    assert_first_timestamped_line(&output, "started");
    assert_all_lifecycle_lines_are_timestamped(&output);
    assert_order(
        &output,
        &[
            "burst event 0",
            "burst event 1",
            "burst event 2",
            "burst event 3",
            "burst event 4",
            "burst event 5",
            "burst event 6",
            "burst event 7",
        ],
    );
    assert_no_stream_scaffolding(&output);
}

#[test]
fn handle_lifecycle_worker_stream_control_characters_and_secrets_render_one_safe_line() {
    let secret = "sk-test_secret_1234567890abcdef";
    let message = format!(
        "cloning repo: https://user:pass@github.com/org/repo.git\nforged line\r\u{1b}[31mred {secret}"
    );
    let mut client = RecordingLifecycleClient::new().with_stream(Ok(vec![stream_event(
        3,
        "repo_preparation",
        "running",
        &message,
    )]));

    let output = lifecycle_output(Command::Worker(worker_args("worker-123")), &mut client);
    let lines = output_lines(&output);

    assert_line_count(&output, 2);
    assert_first_timestamped_line(&output, "started");
    assert_timestamped_line_contains(lines[1], "cloning repo: https://github.com/org/repo.git");
    assert_timestamped_line_contains(lines[1], "forged line red");
    assert_not_contains(&output, "user:pass");
    assert_not_contains(&output, secret);
    assert_not_contains(&output, "\nforged line");
    assert_no_stream_scaffolding(&output);
}

struct SharedWriter(Rc<RefCell<Vec<u8>>>);

impl Write for SharedWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.0.borrow_mut().extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

struct StreamingObservationClient {
    buffer: Rc<RefCell<Vec<u8>>>,
    observed_before_close: Rc<RefCell<bool>>,
}

impl LifecycleDaemonClient for StreamingObservationClient {
    fn connect(&mut self, _addr: Option<String>) -> Result<(), LifecycleError> {
        Ok(())
    }

    fn dispatch(
        &mut self,
        _request: LifecycleDispatchRequest,
    ) -> Result<LifecycleDispatchResponse, LifecycleError> {
        unreachable!("streaming observation test only streams workers")
    }

    fn list(
        &mut self,
        _request: LifecycleListRequest,
    ) -> Result<LifecycleListResponse, LifecycleError> {
        unreachable!("streaming observation test only streams workers")
    }

    fn stream_worker(
        &mut self,
        _request: LifecycleWorkerRequest,
        sink: &mut dyn FnMut(LifecycleStreamItem) -> Result<(), LifecycleError>,
    ) -> Result<(), LifecycleError> {
        sink(stream_event(0, "accepted", "accepted", "worker accepted"))?;
        *self.observed_before_close.borrow_mut() = String::from_utf8(self.buffer.borrow().clone())
            .unwrap()
            .lines()
            .next()
            .and_then(timestamped_message)
            == Some("started");
        std::thread::sleep(std::time::Duration::from_millis(10));
        sink(stream_event(
            1,
            "succeeded",
            "succeeded",
            "prompt/requirements completed",
        ))?;
        Ok(())
    }

    fn answer(
        &mut self,
        _request: LifecycleAnswerRequest,
    ) -> Result<LifecycleAnswerResponse, LifecycleError> {
        unreachable!("streaming observation test only streams workers")
    }
}
