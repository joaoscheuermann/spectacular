use std::cell::RefCell;
use std::io::{self, Write};
use std::rc::Rc;

#[test]
fn handle_lifecycle_worker_stream_writes_first_event_before_stream_closes() {
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
        "first event should be visible before stream closure"
    );
    let output = strip_ansi_codes(&String::from_utf8(shared.borrow().clone()).unwrap());
    assert_order(&output, &["0 accepted", "1 succeeded"]);
}

#[test]
fn handle_lifecycle_worker_stream_replays_events_in_daemon_order() {
    let mut client = RecordingLifecycleClient::new().with_stream(Ok(vec![
        stream_event(0, "accepted", "accepted", "worker accepted"),
        stream_event(1, "starting", "starting", "worker starting"),
        stream_event(2, "repo_preparation", "running", "repo ready"),
        stream_event(3, "prompt_agent_started", "running", "prompt agent started"),
        stream_event(4, "prompt_artifact_written", "running", "PROMPT.md written"),
        stream_event(
            5,
            "prompt_agent_completed",
            "succeeded",
            "prompt/requirements completed",
        ),
    ]));

    let output = lifecycle_output(Command::Worker(worker_args("worker-123")), &mut client);

    assert_eq!(
        client.calls(),
        &[ClientCall::Stream {
            worker_id: "worker-123".to_owned(),
            addr: None,
        }]
    );
    assert_order(
        &output,
        &[
            "0 accepted",
            "1 starting",
            "2 repo_preparation",
            "3 prompt_agent_started",
            "4 prompt_artifact_written",
            "5 prompt_agent_completed",
        ],
    );
    assert_contains(&output, "prompt/requirements completed");
    assert_not_contains(&output, "daemon client not wired");
}

#[test]
fn handle_lifecycle_worker_stream_history_truncated_renders_truncation_notice_before_events() {
    let mut client = RecordingLifecycleClient::new().with_stream(Ok(vec![
        LifecycleStreamItem::HistoryTruncated {
            requested_from_sequence: 0,
            first_available_sequence: 42,
        },
        stream_event(
            42,
            "prompt_agent_started",
            "running",
            "prompt agent started",
        ),
    ]));

    let output = lifecycle_output(Command::Worker(worker_args("worker-123")), &mut client);

    assert_order(
        &output,
        &["history was truncated", "42 prompt_agent_started"],
    );
    assert_not_contains(&output, "daemon client not wired");
}

#[test]
fn handle_lifecycle_worker_stream_waiting_for_input_renders_request_handle() {
    let mut client =
        RecordingLifecycleClient::new().with_stream(Ok(vec![LifecycleStreamItem::Event(
            LifecycleWorkerEvent {
                sequence: 6,
                name: "waiting_for_input".to_owned(),
                status: "waiting".to_owned(),
                message: "Approve repo access?".to_owned(),
                request_id: Some("request-1".to_owned()),
            },
        )]));

    let output = lifecycle_output(Command::Worker(worker_args("worker-123")), &mut client);

    assert_contains(&output, "waiting for input");
    assert_contains(&output, "request-1");
    assert_contains(&output, "Approve repo access?");
    assert_contains(&output, "doric answer worker-123 request-1 --text");
    assert_not_contains(&output, "daemon client not wired");
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
            .contains("0 accepted");
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
