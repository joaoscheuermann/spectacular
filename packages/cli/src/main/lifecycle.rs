use super::{
    cli_types::LifecycleWorkerArgs,
    cli_types::{Command, LifecycleAddressArgs, LifecycleAnswerArgs, LifecycleDispatchArgs},
    lifecycle_client::GrpcLifecycleDaemonClient,
    lifecycle_output::{
        format_answer_output, format_connected_line, format_connecting_line,
        format_created_worker_line, format_creating_worker_line, format_dispatch_output,
        format_list_output, format_worker_output_header, format_worker_output_item,
    },
};
use std::fmt::{self, Display};
use std::io::Write;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum LifecycleDispatchMode {
    Feature,
    Debug,
}

#[allow(dead_code)]
pub(super) trait LifecycleDaemonClient {
    fn connect(&mut self, addr: Option<String>) -> Result<(), LifecycleError>;
    fn dispatch(
        &mut self,
        request: LifecycleDispatchRequest,
    ) -> Result<LifecycleDispatchResponse, LifecycleError>;
    fn list(
        &mut self,
        request: LifecycleListRequest,
    ) -> Result<LifecycleListResponse, LifecycleError>;
    fn stream_worker(
        &mut self,
        request: LifecycleWorkerRequest,
        sink: &mut dyn FnMut(LifecycleStreamItem) -> Result<(), LifecycleError>,
    ) -> Result<(), LifecycleError>;
    fn answer(
        &mut self,
        request: LifecycleAnswerRequest,
    ) -> Result<LifecycleAnswerResponse, LifecycleError>;
}

#[allow(dead_code)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct LifecycleDispatchRequest {
    pub(super) mode: LifecycleDispatchMode,
    pub(super) prompt: String,
    pub(super) repo: String,
    pub(super) addr: Option<String>,
}

#[allow(dead_code)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct LifecycleDispatchResponse {
    pub(super) worker_id: String,
    pub(super) mode: LifecycleDispatchMode,
    pub(super) repo: String,
    pub(super) status: String,
}

#[allow(dead_code)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct LifecycleListRequest {
    pub(super) addr: Option<String>,
}

#[allow(dead_code)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct LifecycleListResponse {
    pub(super) workers: Vec<LifecycleWorkerSummary>,
}

#[allow(dead_code)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct LifecycleWorkerSummary {
    pub(super) worker_id: String,
    pub(super) mode: LifecycleDispatchMode,
    pub(super) repo: String,
    pub(super) status: String,
    pub(super) latest_sequence: u64,
    pub(super) activity: String,
    pub(super) terminal_reason: Option<String>,
    pub(super) pending_request_id: Option<String>,
}

#[allow(dead_code)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct LifecycleWorkerRequest {
    pub(super) worker_id: String,
    pub(super) addr: Option<String>,
}

#[allow(dead_code)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum LifecycleStreamItem {
    HistoryTruncated {
        requested_from_sequence: u64,
        first_available_sequence: u64,
    },
    Event(LifecycleWorkerEvent),
}

#[allow(dead_code)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct LifecycleWorkerEvent {
    pub(super) sequence: u64,
    pub(super) name: String,
    pub(super) status: String,
    pub(super) message: String,
    pub(super) request_id: Option<String>,
}

#[allow(dead_code)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct LifecycleAnswerRequest {
    pub(super) worker_id: String,
    pub(super) request_id: String,
    pub(super) text: String,
    pub(super) addr: Option<String>,
}

#[allow(dead_code)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct LifecycleAnswerResponse {
    pub(super) worker_id: String,
    pub(super) request_id: String,
    pub(super) accepted: bool,
}

#[allow(dead_code)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum LifecycleError {
    InvalidInput(String),
    DaemonUnavailable(String),
    UnknownWorker(String),
    UnknownRequest {
        worker_id: String,
        request_id: String,
    },
    Daemon(String),
}

impl Display for LifecycleError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidInput(message)
            | Self::DaemonUnavailable(message)
            | Self::Daemon(message) => f.write_str(&redact_text(message)),
            Self::UnknownWorker(worker_id) => {
                write!(f, "unknown worker `{}`", redact_text(worker_id))
            }
            Self::UnknownRequest {
                worker_id,
                request_id,
            } => write!(
                f,
                "unknown request `{}` for worker `{}`",
                redact_text(request_id),
                redact_text(worker_id)
            ),
        }
    }
}

pub(super) fn handle_lifecycle_command(command: Command) -> Result<Option<String>, LifecycleError> {
    let mut client = GrpcLifecycleDaemonClient::default();
    let mut stdout = std::io::stdout();

    handle_lifecycle_command_with_writer(command, &mut client, &mut stdout)
}

#[allow(dead_code)]
pub(super) fn handle_lifecycle_command_with_client<Client>(
    command: Command,
    client: &mut Client,
) -> Result<String, LifecycleError>
where
    Client: LifecycleDaemonClient,
{
    let mut output = Vec::new();
    let result = handle_lifecycle_command_with_writer(command, client, &mut output)?;

    Ok(result.unwrap_or_else(|| {
        String::from_utf8(output)
            .expect("lifecycle output is utf-8")
            .trim_end_matches('\n')
            .to_owned()
    }))
}

#[allow(dead_code)]
pub(super) fn handle_lifecycle_command_with_writer<Client, Output>(
    command: Command,
    client: &mut Client,
    output: &mut Output,
) -> Result<Option<String>, LifecycleError>
where
    Client: LifecycleDaemonClient,
    Output: Write,
{
    match command {
        Command::Feature(args) => handle_feature(args, client, output),
        Command::Debug(args) => handle_dispatch(LifecycleDispatchMode::Debug, args, client),
        Command::List(args) => handle_list(args, client, output),
        Command::Worker(args) => handle_worker(args, client, output),
        Command::Answer(args) => handle_answer(args, client),
        Command::Daemon(_) | Command::Config(_) => Err(LifecycleError::InvalidInput(
            "unsupported lifecycle command".to_owned(),
        )),
    }
}

fn handle_feature<Client, Output>(
    args: LifecycleDispatchArgs,
    client: &mut Client,
    output: &mut Output,
) -> Result<Option<String>, LifecycleError>
where
    Client: LifecycleDaemonClient,
    Output: Write,
{
    let prompt = required_arg(args.prompt, "prompt")?;
    let repo = required_feature_repo(args.repo)?;
    let addr = args.addr;

    write_lifecycle_line(output, format_connecting_line())?;
    client.connect(addr.clone())?;
    write_lifecycle_line(output, format_connected_line())?;
    write_lifecycle_line(output, format_creating_worker_line())?;

    let response = client.dispatch(LifecycleDispatchRequest {
        mode: LifecycleDispatchMode::Feature,
        prompt,
        repo,
        addr,
    })?;

    write_lifecycle_line(output, &format_created_worker_line(&response.worker_id))?;

    Ok(None)
}

fn handle_dispatch<Client>(
    mode: LifecycleDispatchMode,
    args: LifecycleDispatchArgs,
    client: &mut Client,
) -> Result<Option<String>, LifecycleError>
where
    Client: LifecycleDaemonClient,
{
    let prompt = required_arg(args.prompt, "prompt")?;
    let repo = required_arg(args.repo, "repo")?;
    let response = client.dispatch(LifecycleDispatchRequest {
        mode,
        prompt,
        repo,
        addr: args.addr,
    })?;

    Ok(Some(format_dispatch_output(&response)))
}

fn handle_list<Client>(
    args: LifecycleAddressArgs,
    client: &mut Client,
    output: &mut impl Write,
) -> Result<Option<String>, LifecycleError>
where
    Client: LifecycleDaemonClient,
{
    let addr = args.addr;

    write_lifecycle_line(output, format_connecting_line())?;
    client.connect(addr.clone())?;
    write_lifecycle_line(output, format_connected_line())?;

    let response = client.list(LifecycleListRequest { addr })?;

    for line in format_list_output(&response).lines() {
        write_lifecycle_line(output, line)?;
    }

    Ok(None)
}

fn handle_worker<Client, Output>(
    args: LifecycleWorkerArgs,
    client: &mut Client,
    output: &mut Output,
) -> Result<Option<String>, LifecycleError>
where
    Client: LifecycleDaemonClient,
    Output: Write,
{
    let worker_id = args.id;
    write_lifecycle_line(output, &format_worker_output_header(&worker_id))?;
    let mut wrote_item = false;
    let mut sink = |item| {
        wrote_item = true;

        for line in format_worker_output_item(&worker_id, &item) {
            write_lifecycle_line(output, &line)?;
        }

        Ok(())
    };

    client.stream_worker(
        LifecycleWorkerRequest {
            worker_id: worker_id.clone(),
            addr: args.addr,
        },
        &mut sink,
    )?;

    if !wrote_item {
        write_lifecycle_line(output, "  No retained events for this worker")?;
    }

    Ok(None)
}

fn handle_answer<Client>(
    args: LifecycleAnswerArgs,
    client: &mut Client,
) -> Result<Option<String>, LifecycleError>
where
    Client: LifecycleDaemonClient,
{
    let response = client.answer(LifecycleAnswerRequest {
        worker_id: args.worker_id,
        request_id: args.request_id,
        text: args.text,
        addr: args.addr,
    })?;

    Ok(Some(format_answer_output(&response)))
}

fn write_lifecycle_line(output: &mut impl Write, line: &str) -> Result<(), LifecycleError> {
    writeln!(output, "{line}").map_err(|source| {
        LifecycleError::Daemon(format!("failed to write lifecycle output: {source}"))
    })
}

fn required_arg(value: String, name: &'static str) -> Result<String, LifecycleError> {
    let trimmed = value.trim();

    if trimmed.is_empty() {
        Err(LifecycleError::InvalidInput(format!(
            "{name} must not be empty"
        )))
    } else {
        Ok(trimmed.to_owned())
    }
}

fn required_feature_repo(value: String) -> Result<String, LifecycleError> {
    if value.trim().is_empty() {
        return Err(LifecycleError::InvalidInput(
            "repo must not be empty".to_owned(),
        ));
    }

    let repo = lifecycle::repo::RepoUrl::try_from(value.as_str())
        .map_err(|error| LifecycleError::InvalidInput(error.to_string()))?;

    Ok(repo.as_clone_input().to_owned())
}

pub(super) fn redact_text(value: &str) -> String {
    redact_repo_credentials(&lifecycle::redaction::redact_failure_text(value))
}

fn redact_repo_credentials(value: &str) -> String {
    let mut redacted = String::with_capacity(value.len());
    let mut rest = value;

    while let Some(scheme_start) = rest.find("://") {
        let authority_start = scheme_start + 3;
        let after_scheme = &rest[authority_start..];

        let Some(authority_offset) = after_scheme.find('@') else {
            let split = authority_start;
            redacted.push_str(&rest[..split]);
            rest = &rest[split..];
            continue;
        };

        let userinfo_end = authority_start + authority_offset;
        let host_start = userinfo_end + 1;
        redacted.push_str(&rest[..authority_start]);
        rest = &rest[host_start..];
    }

    redacted.push_str(rest);
    redacted
}

#[cfg(test)]
mod tests {
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/unit/lifecycle_output.rs"
    ));
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/unit/lifecycle_feature_list_output.rs"
    ));
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/unit/lifecycle_worker_stream_output.rs"
    ));
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/unit/lifecycle_answer_error_redaction_output.rs"
    ));
}
