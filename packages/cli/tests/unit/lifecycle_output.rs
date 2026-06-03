use super::*;
use crate::cli_types::{
    Command, LifecycleAddressArgs, LifecycleAnswerArgs, LifecycleDispatchArgs, LifecycleWorkerArgs,
};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

#[derive(Clone, Debug, Eq, PartialEq)]
enum ClientCall {
    Connect {
        addr: Option<String>,
    },
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
    connect: Result<(), LifecycleError>,
    dispatch: Result<LifecycleDispatchResponse, LifecycleError>,
    list: Result<LifecycleListResponse, LifecycleError>,
    stream: Result<Vec<LifecycleStreamItem>, LifecycleError>,
    answer: Result<LifecycleAnswerResponse, LifecycleError>,
}

impl RecordingLifecycleClient {
    fn new() -> Self {
        Self {
            calls: Vec::new(),
            connect: Ok(()),
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
            connect: Err(error.clone()),
            dispatch: Err(error.clone()),
            list: Err(error.clone()),
            stream: Err(error.clone()),
            answer: Err(error),
        }
    }

    fn with_dispatch(
        mut self,
        response: Result<LifecycleDispatchResponse, LifecycleError>,
    ) -> Self {
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
    fn connect(&mut self, addr: Option<String>) -> Result<(), LifecycleError> {
        self.calls.push(ClientCall::Connect { addr });

        self.connect.clone()
    }

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

    fn list(
        &mut self,
        request: LifecycleListRequest,
    ) -> Result<LifecycleListResponse, LifecycleError> {
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

fn stream_event(sequence: u64, name: &str, status: &str, message: &str) -> LifecycleStreamItem {
    LifecycleStreamItem::Event(LifecycleWorkerEvent {
        sequence,
        name: name.to_owned(),
        status: status.to_owned(),
        message: message.to_owned(),
        request_id: None,
        occurred_at: None,
    })
}

fn stream_event_at(
    sequence: u64,
    name: &str,
    status: &str,
    message: &str,
    occurred_at: SystemTime,
) -> LifecycleStreamItem {
    LifecycleStreamItem::Event(LifecycleWorkerEvent {
        sequence,
        name: name.to_owned(),
        status: status.to_owned(),
        message: message.to_owned(),
        request_id: None,
        occurred_at: Some(occurred_at),
    })
}

fn waiting_input_event(sequence: u64, message: &str, request_id: &str) -> LifecycleStreamItem {
    LifecycleStreamItem::Event(LifecycleWorkerEvent {
        sequence,
        name: "waiting_for_input".to_owned(),
        status: "waiting_for_input".to_owned(),
        message: message.to_owned(),
        request_id: Some(request_id.to_owned()),
        occurred_at: None,
    })
}

fn fixed_time(offset_seconds: u64) -> SystemTime {
    UNIX_EPOCH + Duration::from_secs(offset_seconds)
}

fn fixed_timestamp(offset_seconds: u64) -> String {
    ::lifecycle::terminal::format_timestamp(fixed_time(offset_seconds))
}

fn output_lines(output: &str) -> Vec<&str> {
    output.lines().collect()
}

fn assert_line_count(output: &str, expected: usize) {
    let lines = output_lines(output);

    assert_eq!(
        lines.len(),
        expected,
        "expected {expected} lifecycle output lines:\n{output}"
    );
}

fn assert_first_timestamped_line(output: &str, expected_message: &str) {
    let lines = output_lines(output);
    let Some(first_line) = lines.first() else {
        panic!("expected first lifecycle output line:\n{output}");
    };

    assert_timestamped_message(first_line, expected_message);
}

fn assert_all_lifecycle_lines_are_timestamped(output: &str) {
    for line in output.lines() {
        assert!(
            timestamped_message(line).is_some(),
            "expected timestamped lifecycle line, got `{line}`:\n{output}"
        );
    }
}

fn assert_timestamped_message(line: &str, expected_message: &str) {
    let Some(message) = timestamped_message(line) else {
        panic!("expected timestamped lifecycle line, got `{line}`");
    };

    assert_eq!(message, expected_message);
}

fn assert_timestamped_line_contains(line: &str, expected: &str) {
    let Some(message) = timestamped_message(line) else {
        panic!("expected timestamped lifecycle line, got `{line}`");
    };

    assert!(
        message.contains(expected),
        "expected timestamped line message to contain `{expected}`, got `{message}`"
    );
}

fn assert_no_stream_scaffolding(output: &str) {
    assert_not_contains(output, "[stream]");
    assert_not_contains(output, "Lifecycle worker");
    assert_not_contains(output, "No retained events");
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

fn timestamped_message(line: &str) -> Option<&str> {
    let body = line.strip_prefix('[')?;
    let (timestamp, message) = body.split_once("] ")?;

    is_utc_timestamp(timestamp).then_some(message)
}

fn is_utc_timestamp(timestamp: &str) -> bool {
    let bytes = timestamp.as_bytes();

    bytes.len() == 20
        && bytes[4] == b'-'
        && bytes[7] == b'-'
        && bytes[10] == b'T'
        && bytes[13] == b':'
        && bytes[16] == b':'
        && bytes[19] == b'Z'
        && bytes
            .iter()
            .enumerate()
            .filter(|(index, _)| ![4, 7, 10, 13, 16, 19].contains(index))
            .all(|(_, byte)| byte.is_ascii_digit())
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
