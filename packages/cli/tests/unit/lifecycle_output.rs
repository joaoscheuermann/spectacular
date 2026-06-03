use super::*;
use crate::cli_types::{
    Command, LifecycleAddressArgs, LifecycleAnswerArgs, LifecycleDispatchArgs, LifecycleWorkerArgs,
};

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
