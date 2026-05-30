use super::*;
use crate::cli_types::{
    Command, LifecycleAddressArgs, LifecycleAnswerArgs, LifecycleDispatchArgs,
    LifecycleWorkerArgs,
};
use std::cell::RefCell;
use std::io::{self, Write};
use std::rc::Rc;

#[test]
fn handle_lifecycle_dispatch_feature_success_renders_worker_summary_and_prompt_scope() {
    let mut client = RecordingLifecycleClient::new().with_dispatch(Ok(dispatch_response(
        "feature-123",
        LifecycleDispatchMode::Feature,
        "https://github.com/org/repo.git",
        "accepted",
    )));

    let output = lifecycle_output(
        Command::Feature(dispatch_args(
            "write requirements",
            "https://github.com/org/repo.git",
        )),
        &mut client,
    );

    assert_eq!(
        client.calls(),
        &[ClientCall::Dispatch {
            mode: LifecycleDispatchMode::Feature,
            prompt: "write requirements".to_owned(),
            repo: "https://github.com/org/repo.git".to_owned(),
            addr: None,
        }]
    );
    assert_contains(&output, "feature-123");
    assert_contains(&output, "Mode: feature");
    assert_contains(&output, "Repo: https://github.com/org/repo.git");
    assert_contains(&output, "Status: accepted");
    assert_contains(&output, "prompt/requirements");
    assert_not_contains(&output, "daemon client not wired");
    assert_not_contains(&output, "PRD");
    assert_not_contains(&output, "technical design");
    assert_not_contains(&output, "implementation");
    assert_not_contains(&output, "handover");
}

#[test]
fn handle_lifecycle_dispatch_debug_success_renders_mode_label_and_shared_v1_wording() {
    let mut client = RecordingLifecycleClient::new().with_dispatch(Ok(dispatch_response(
        "debug-456",
        LifecycleDispatchMode::Debug,
        "https://github.com/org/repo.git",
        "accepted",
    )));

    let output = lifecycle_output(
        Command::Debug(dispatch_args(
            "investigate failure",
            "https://github.com/org/repo.git",
        )),
        &mut client,
    );

    assert_eq!(
        client.calls(),
        &[ClientCall::Dispatch {
            mode: LifecycleDispatchMode::Debug,
            prompt: "investigate failure".to_owned(),
            repo: "https://github.com/org/repo.git".to_owned(),
            addr: None,
        }]
    );
    assert_contains(&output, "debug-456");
    assert_contains(&output, "Mode: debug");
    assert_contains(&output, "prompt/requirements");
    assert_not_contains(&output, "distinct debug workflow");
    assert_not_contains(&output, "daemon client not wired");
}

#[test]
fn handle_lifecycle_dispatch_blank_prompt_rejects_without_client_call() {
    let mut client = RecordingLifecycleClient::new();

    let error = lifecycle_error(
        Command::Feature(dispatch_args(
            "   ",
            "https://github.com/org/repo.git",
        )),
        &mut client,
    );

    assert!(client.calls().is_empty(), "client calls: {:?}", client.calls());
    assert_contains(&error, "prompt");
    assert_contains(&error, "must not be empty");
}

#[test]
fn handle_lifecycle_dispatch_blank_repo_rejects_without_client_call() {
    let mut client = RecordingLifecycleClient::new();

    let error = lifecycle_error(
        Command::Feature(dispatch_args("write requirements", "   ")),
        &mut client,
    );

    assert!(client.calls().is_empty(), "client calls: {:?}", client.calls());
    assert_contains(&error, "repo");
    assert_contains(&error, "must not be empty");
}

#[test]
fn handle_lifecycle_list_empty_renders_clear_empty_state() {
    let mut client = RecordingLifecycleClient::new().with_list(Ok(LifecycleListResponse {
        workers: Vec::new(),
    }));

    let output = lifecycle_output(Command::List(LifecycleAddressArgs { addr: None }), &mut client);

    assert_eq!(client.calls(), &[ClientCall::List { addr: None }]);
    assert_contains(&output, "No lifecycle workers are tracked");
    assert_not_contains(&output, "daemon client not wired");
}

#[test]
fn handle_lifecycle_list_rows_renders_mode_repo_status_activity_and_sequence() {
    let mut client = RecordingLifecycleClient::new().with_list(Ok(LifecycleListResponse {
        workers: vec![
            worker_summary(
                "feature-123",
                LifecycleDispatchMode::Feature,
                "https://github.com/org/repo.git",
                "accepted",
                0,
                "queued",
            ),
            {
                let mut summary = worker_summary(
                    "debug-456",
                    LifecycleDispatchMode::Debug,
                    "https://github.com/org/repo.git",
                    "waiting",
                    7,
                    "waiting for input",
                );
                summary.pending_request_id = Some("request-1".to_owned());
                summary
            },
        ],
    }));

    let output = lifecycle_output(
        Command::List(LifecycleAddressArgs {
            addr: Some("127.0.0.1:47822".to_owned()),
        }),
        &mut client,
    );

    assert_eq!(
        client.calls(),
        &[ClientCall::List {
            addr: Some("127.0.0.1:47822".to_owned()),
        }]
    );
    for expected in [
        "feature-123",
        "debug-456",
        "Mode: feature",
        "Mode: debug",
        "Status: accepted",
        "Status: waiting",
        "Sequence: 7",
        "Activity: waiting for input",
        "Request: request-1",
    ] {
        assert_contains(&output, expected);
    }
    assert_not_contains(&output, "daemon client not wired");
}

#[test]
fn handle_lifecycle_worker_stream_writes_first_event_before_stream_closes() {
    let shared = Rc::new(RefCell::new(Vec::new()));
    let observed_before_close = Rc::new(RefCell::new(false));
    let mut writer = SharedWriter(Rc::clone(&shared));
    let mut client = StreamingObservationClient {
        buffer: Rc::clone(&shared),
        observed_before_close: Rc::clone(&observed_before_close),
    };

    let result =
        handle_lifecycle_command_with_writer(Command::Worker(worker_args("worker-123")), &mut client, &mut writer)
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
        stream_event(5, "prompt_agent_completed", "succeeded", "prompt/requirements completed"),
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
        stream_event(42, "prompt_agent_started", "running", "prompt agent started"),
    ]));

    let output = lifecycle_output(Command::Worker(worker_args("worker-123")), &mut client);

    assert_order(&output, &["history was truncated", "42 prompt_agent_started"]);
    assert_not_contains(&output, "daemon client not wired");
}

#[test]
fn handle_lifecycle_worker_stream_waiting_for_input_renders_request_handle() {
    let mut client = RecordingLifecycleClient::new().with_stream(Ok(vec![
        LifecycleStreamItem::Event(LifecycleWorkerEvent {
            sequence: 6,
            name: "waiting_for_input".to_owned(),
            status: "waiting".to_owned(),
            message: "Approve repo access?".to_owned(),
            request_id: Some("request-1".to_owned()),
        }),
    ]));

    let output = lifecycle_output(Command::Worker(worker_args("worker-123")), &mut client);

    assert_contains(&output, "waiting for input");
    assert_contains(&output, "request-1");
    assert_contains(&output, "Approve repo access?");
    assert_contains(&output, "doric answer worker-123 request-1 --text");
    assert_not_contains(&output, "daemon client not wired");
}

#[test]
fn handle_lifecycle_answer_success_renders_acceptance_and_continuation() {
    let secret = "sk-test_secret_1234567890";
    let mut client = RecordingLifecycleClient::new().with_answer(Ok(LifecycleAnswerResponse {
        worker_id: "worker-123".to_owned(),
        request_id: "request-1".to_owned(),
        accepted: true,
    }));

    let output = lifecycle_output(
        Command::Answer(LifecycleAnswerArgs {
            worker_id: "worker-123".to_owned(),
            request_id: "request-1".to_owned(),
            text: secret.to_owned(),
            addr: None,
        }),
        &mut client,
    );

    assert_eq!(
        client.calls(),
        &[ClientCall::Answer {
            worker_id: "worker-123".to_owned(),
            request_id: "request-1".to_owned(),
            text: secret.to_owned(),
            addr: None,
        }]
    );
    assert_contains(&output, "worker-123");
    assert_contains(&output, "request-1");
    assert_contains(&output, "answer accepted");
    assert_not_contains(&output, secret);
    assert_not_contains(&output, "daemon client not wired");
}

#[test]
fn handle_lifecycle_worker_unknown_worker_returns_clear_error() {
    let mut client = RecordingLifecycleClient::new()
        .with_stream(Err(LifecycleError::UnknownWorker("unknown-worker".to_owned())));

    let error = lifecycle_error(Command::Worker(worker_args("unknown-worker")), &mut client);

    assert_eq!(
        client.calls(),
        &[ClientCall::Stream {
            worker_id: "unknown-worker".to_owned(),
            addr: None,
        }]
    );
    assert_contains(&error, "unknown worker");
    assert_contains(&error, "unknown-worker");
}

#[test]
fn handle_lifecycle_answer_unknown_request_returns_clear_error() {
    let mut client = RecordingLifecycleClient::new().with_answer(Err(
        LifecycleError::UnknownRequest {
            worker_id: "worker-123".to_owned(),
            request_id: "missing-request".to_owned(),
        },
    ));

    let error = lifecycle_error(
        Command::Answer(LifecycleAnswerArgs {
            worker_id: "worker-123".to_owned(),
            request_id: "missing-request".to_owned(),
            text: "continue".to_owned(),
            addr: None,
        }),
        &mut client,
    );

    assert_contains(&error, "unknown request");
    assert_contains(&error, "worker-123");
    assert_contains(&error, "missing-request");
}

#[test]
fn handle_lifecycle_commands_daemon_unavailable_return_nonzero_without_direct_worker_fallback() {
    for command in [
        Command::Feature(dispatch_args("write requirements", "https://github.com/org/repo.git")),
        Command::Debug(dispatch_args("investigate failure", "https://github.com/org/repo.git")),
        Command::List(LifecycleAddressArgs { addr: None }),
        Command::Worker(worker_args("worker-123")),
        Command::Answer(LifecycleAnswerArgs {
            worker_id: "worker-123".to_owned(),
            request_id: "request-1".to_owned(),
            text: "continue".to_owned(),
            addr: None,
        }),
    ] {
        let mut client = RecordingLifecycleClient::unavailable();

        let error = lifecycle_error(command, &mut client);

        assert_contains(&error, "daemon unavailable");
        assert_not_contains(&error, "direct worker fallback");
        assert_eq!(client.calls().len(), 1, "client calls: {:?}", client.calls());
    }
}

#[test]
fn handle_lifecycle_output_redacts_repo_credentials_everywhere() {
    let raw_repo = "https://user:pass@github.com/org/repo.git";
    let token_repo = "https://ghp_secret@github.com/org/repo.git";
    let mut dispatch_client = RecordingLifecycleClient::new().with_dispatch(Ok(dispatch_response(
        "feature-123",
        LifecycleDispatchMode::Feature,
        "https://github.com/org/repo.git",
        "accepted",
    )));
    let mut list_client = RecordingLifecycleClient::new().with_list(Ok(LifecycleListResponse {
        workers: vec![worker_summary(
            "feature-123",
            LifecycleDispatchMode::Feature,
            "https://github.com/org/repo.git",
            "accepted",
            0,
            "queued",
        )],
    }));
    let mut stream_client = RecordingLifecycleClient::new().with_stream(Ok(vec![stream_event(
        2,
        "repo_preparation",
        "running",
        token_repo,
    )]));

    let dispatch = lifecycle_output(
        Command::Feature(dispatch_args("write requirements", raw_repo)),
        &mut dispatch_client,
    );
    let list = lifecycle_output(Command::List(LifecycleAddressArgs { addr: None }), &mut list_client);
    let stream = lifecycle_output(Command::Worker(worker_args("worker-123")), &mut stream_client);
    let combined = format!("{dispatch}\n{list}\n{stream}");

    assert_not_contains(&combined, "user:pass");
    assert_not_contains(&combined, "ghp_secret");
    assert_not_contains(&combined, raw_repo);
    assert_not_contains(&combined, token_repo);
    assert_contains(&combined, "github.com/org/repo.git");
}

#[test]
fn handle_lifecycle_output_redacts_provider_and_env_secret_text() {
    let secret = "sk-test_secret_1234567890abcdef";
    let mut client = RecordingLifecycleClient::new().with_answer(Err(LifecycleError::Daemon(
        format!("OPENAI_API_KEY={secret}"),
    )));

    let error = lifecycle_error(
        Command::Answer(LifecycleAnswerArgs {
            worker_id: "worker-123".to_owned(),
            request_id: "request-1".to_owned(),
            text: format!("OPENAI_API_KEY={secret}"),
            addr: None,
        }),
        &mut client,
    );

    assert_not_contains(&error, secret);
    assert_not_contains(&error, &format!("OPENAI_API_KEY={secret}"));
    assert_contains(&error, "[REDACTED]");
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum ClientCall {
    Dispatch {
        mode: LifecycleDispatchMode,
        prompt: String,
        repo: String,
        addr: Option<String>,
    },
    List {
        addr: Option<String>,
    },
    Stream {
        worker_id: String,
        addr: Option<String>,
    },
    Answer {
        worker_id: String,
        request_id: String,
        text: String,
        addr: Option<String>,
    },
}

struct RecordingLifecycleClient {
    calls: Vec<ClientCall>,
    dispatch: Result<LifecycleDispatchResponse, LifecycleError>,
    list: Result<LifecycleListResponse, LifecycleError>,
    stream: Result<Vec<LifecycleStreamItem>, LifecycleError>,
    answer: Result<LifecycleAnswerResponse, LifecycleError>,
}

impl RecordingLifecycleClient {
    fn new() -> Self {
        Self {
            calls: Vec::new(),
            dispatch: Ok(dispatch_response(
                "feature-123",
                LifecycleDispatchMode::Feature,
                "https://github.com/org/repo.git",
                "accepted",
            )),
            list: Ok(LifecycleListResponse {
                workers: Vec::new(),
            }),
            stream: Ok(Vec::new()),
            answer: Ok(LifecycleAnswerResponse {
                worker_id: "worker-123".to_owned(),
                request_id: "request-1".to_owned(),
                accepted: true,
            }),
        }
    }

    fn unavailable() -> Self {
        let error =
            LifecycleError::DaemonUnavailable("daemon unavailable: failed to connect".to_owned());

        Self {
            calls: Vec::new(),
            dispatch: Err(error.clone()),
            list: Err(error.clone()),
            stream: Err(error.clone()),
            answer: Err(error),
        }
    }

    fn with_dispatch(mut self, response: Result<LifecycleDispatchResponse, LifecycleError>) -> Self {
        self.dispatch = response;
        self
    }

    fn with_list(mut self, response: Result<LifecycleListResponse, LifecycleError>) -> Self {
        self.list = response;
        self
    }

    fn with_stream(mut self, response: Result<Vec<LifecycleStreamItem>, LifecycleError>) -> Self {
        self.stream = response;
        self
    }

    fn with_answer(mut self, response: Result<LifecycleAnswerResponse, LifecycleError>) -> Self {
        self.answer = response;
        self
    }

    fn calls(&self) -> &[ClientCall] {
        &self.calls
    }
}

impl LifecycleDaemonClient for RecordingLifecycleClient {
    fn dispatch(
        &mut self,
        request: LifecycleDispatchRequest,
    ) -> Result<LifecycleDispatchResponse, LifecycleError> {
        self.calls.push(ClientCall::Dispatch {
            mode: request.mode,
            prompt: request.prompt,
            repo: request.repo,
            addr: request.addr,
        });

        self.dispatch.clone()
    }

    fn list(&mut self, request: LifecycleListRequest) -> Result<LifecycleListResponse, LifecycleError> {
        self.calls.push(ClientCall::List { addr: request.addr });

        self.list.clone()
    }

    fn stream_worker(
        &mut self,
        request: LifecycleWorkerRequest,
        sink: &mut dyn FnMut(LifecycleStreamItem) -> Result<(), LifecycleError>,
    ) -> Result<(), LifecycleError> {
        self.calls.push(ClientCall::Stream {
            worker_id: request.worker_id,
            addr: request.addr,
        });

        for item in self.stream.clone()? {
            sink(item)?;
        }

        Ok(())
    }

    fn answer(
        &mut self,
        request: LifecycleAnswerRequest,
    ) -> Result<LifecycleAnswerResponse, LifecycleError> {
        self.calls.push(ClientCall::Answer {
            worker_id: request.worker_id,
            request_id: request.request_id,
            text: request.text,
            addr: request.addr,
        });

        self.answer.clone()
    }
}

fn dispatch_args(prompt: &str, repo: &str) -> LifecycleDispatchArgs {
    LifecycleDispatchArgs {
        prompt: prompt.to_owned(),
        repo: repo.to_owned(),
        addr: None,
    }
}

fn worker_args(id: &str) -> LifecycleWorkerArgs {
    LifecycleWorkerArgs {
        id: id.to_owned(),
        addr: None,
    }
}

fn lifecycle_output(command: Command, client: &mut RecordingLifecycleClient) -> String {
    strip_ansi_codes(
        &handle_lifecycle_command_with_client(command, client)
            .expect("lifecycle command should return user-facing output"),
    )
}

fn lifecycle_error(command: Command, client: &mut RecordingLifecycleClient) -> String {
    handle_lifecycle_command_with_client(command, client)
        .expect_err("lifecycle command should return an error")
        .to_string()
}

fn dispatch_response(
    worker_id: &str,
    mode: LifecycleDispatchMode,
    repo: &str,
    status: &str,
) -> LifecycleDispatchResponse {
    LifecycleDispatchResponse {
        worker_id: worker_id.to_owned(),
        mode,
        repo: repo.to_owned(),
        status: status.to_owned(),
    }
}

fn worker_summary(
    worker_id: &str,
    mode: LifecycleDispatchMode,
    repo: &str,
    status: &str,
    latest_sequence: u64,
    activity: &str,
) -> LifecycleWorkerSummary {
    LifecycleWorkerSummary {
        worker_id: worker_id.to_owned(),
        mode,
        repo: repo.to_owned(),
        status: status.to_owned(),
        latest_sequence,
        activity: activity.to_owned(),
        terminal_reason: None,
        pending_request_id: None,
    }
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
        *self.observed_before_close.borrow_mut() =
            String::from_utf8(self.buffer.borrow().clone())
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

fn stream_event(
    sequence: u64,
    name: &str,
    status: &str,
    message: &str,
) -> LifecycleStreamItem {
    LifecycleStreamItem::Event(LifecycleWorkerEvent {
        sequence,
        name: name.to_owned(),
        status: status.to_owned(),
        message: message.to_owned(),
        request_id: None,
    })
}

fn assert_contains(output: &str, expected: &str) {
    assert!(
        output.contains(expected),
        "expected output to contain `{expected}`:\n{output}"
    );
}

fn assert_not_contains(output: &str, unexpected: &str) {
    assert!(
        !output.contains(unexpected),
        "expected output not to contain `{unexpected}`:\n{output}"
    );
}

fn assert_order(output: &str, expected: &[&str]) {
    let mut search_start = 0;

    for item in expected {
        let Some(index) = output[search_start..].find(item) else {
            panic!("expected `{item}` after byte {search_start}:\n{output}");
        };
        search_start += index + item.len();
    }
}

fn strip_ansi_codes(value: &str) -> String {
    let mut output = String::new();
    let mut chars = value.chars().peekable();

    while let Some(character) = chars.next() {
        if character != '\u{1b}' {
            output.push(character);
            continue;
        }

        if chars.peek() == Some(&'[') {
            chars.next();
            for code_character in chars.by_ref() {
                if code_character.is_ascii_alphabetic() {
                    break;
                }
            }
        }
    }

    output
}
