use super::lifecycle::{
    redact_text, LifecycleAnswerRequest, LifecycleAnswerResponse, LifecycleDaemonClient,
    LifecycleDispatchMode, LifecycleDispatchRequest, LifecycleDispatchResponse, LifecycleError,
    LifecycleListRequest, LifecycleListResponse, LifecycleStreamItem, LifecycleWorkerEvent,
    LifecycleWorkerRequest, LifecycleWorkerSummary,
};
use ::lifecycle::proto::doric::lifecycle::v1 as pb;
use std::future::Future;
use std::time::SystemTime;

const DEFAULT_DAEMON_ADDR: &str = "127.0.0.1:47821";

type ConnectedLifecycleClient =
    pb::lifecycle_service_client::LifecycleServiceClient<tonic::transport::Channel>;

#[derive(Default)]
pub(super) struct GrpcLifecycleDaemonClient {
    connected: Option<ConnectedLifecycleClient>,
}

impl LifecycleDaemonClient for GrpcLifecycleDaemonClient {
    fn connect(&mut self, addr: Option<String>) -> Result<(), LifecycleError> {
        self.connected = Some(run_rpc(connect_client(addr.as_deref()))?);

        Ok(())
    }

    fn dispatch(
        &mut self,
        request: LifecycleDispatchRequest,
    ) -> Result<LifecycleDispatchResponse, LifecycleError> {
        run_rpc(dispatch_rpc(request, self.connected.take()))
    }

    fn list(
        &mut self,
        request: LifecycleListRequest,
    ) -> Result<LifecycleListResponse, LifecycleError> {
        run_rpc(list_rpc(request, self.connected.take()))
    }

    fn stream_worker(
        &mut self,
        request: LifecycleWorkerRequest,
        sink: &mut dyn FnMut(LifecycleStreamItem) -> Result<(), LifecycleError>,
    ) -> Result<(), LifecycleError> {
        run_rpc(stream_worker_rpc(request, sink))
    }

    fn answer(
        &mut self,
        request: LifecycleAnswerRequest,
    ) -> Result<LifecycleAnswerResponse, LifecycleError> {
        run_rpc(answer_rpc(request))
    }
}

async fn dispatch_rpc(
    request: LifecycleDispatchRequest,
    connected: Option<ConnectedLifecycleClient>,
) -> Result<LifecycleDispatchResponse, LifecycleError> {
    let mut client = match connected {
        Some(client) => client,
        None => connect_client(request.addr.as_deref()).await?,
    };
    let response = client
        .dispatch(pb::DispatchRequest {
            mode: mode_to_proto(request.mode),
            prompt: request.prompt,
            repo: request.repo,
        })
        .await
        .map_err(map_status)?
        .into_inner();

    Ok(LifecycleDispatchResponse {
        worker_id: response.worker_id,
        mode: mode_from_proto(response.mode)?,
        repo: response.repo_identity,
        status: status_label(response.status)?.to_owned(),
    })
}

async fn list_rpc(
    request: LifecycleListRequest,
    connected: Option<ConnectedLifecycleClient>,
) -> Result<LifecycleListResponse, LifecycleError> {
    let mut client = match connected {
        Some(client) => client,
        None => connect_client(request.addr.as_deref()).await?,
    };
    let response = client
        .list_workers(pb::ListWorkersRequest {})
        .await
        .map_err(map_status)?
        .into_inner();
    let workers = response
        .workers
        .into_iter()
        .map(worker_summary_from_proto)
        .collect::<Result<Vec<_>, _>>()?;

    Ok(LifecycleListResponse { workers })
}

async fn stream_worker_rpc(
    request: LifecycleWorkerRequest,
    sink: &mut dyn FnMut(LifecycleStreamItem) -> Result<(), LifecycleError>,
) -> Result<(), LifecycleError> {
    let mut client = connect_client(request.addr.as_deref()).await?;
    let mut stream = client
        .stream_worker(pb::StreamWorkerRequest {
            worker_id: request.worker_id,
            from_sequence: 0,
        })
        .await
        .map_err(map_status)?
        .into_inner();

    while let Some(event) = stream.message().await.map_err(map_status)? {
        sink(stream_item_from_proto(event)?)?;
    }

    Ok(())
}

async fn answer_rpc(
    request: LifecycleAnswerRequest,
) -> Result<LifecycleAnswerResponse, LifecycleError> {
    let mut client = connect_client(request.addr.as_deref()).await?;
    let worker_id = request.worker_id;
    let request_id = request.request_id;
    let response = client
        .answer_input(pb::AnswerInputRequest {
            worker_id: worker_id.clone(),
            request_id: request_id.clone(),
            text: request.text,
        })
        .await
        .map_err(map_status)?
        .into_inner();

    Ok(LifecycleAnswerResponse {
        worker_id,
        request_id,
        accepted: response.accepted,
    })
}

async fn connect_client(
    addr: Option<&str>,
) -> Result<
    pb::lifecycle_service_client::LifecycleServiceClient<tonic::transport::Channel>,
    LifecycleError,
> {
    pb::lifecycle_service_client::LifecycleServiceClient::connect(endpoint(addr))
        .await
        .map_err(|source| {
            LifecycleError::DaemonUnavailable(format!(
                "daemon unavailable: failed to connect to lifecycle daemon ({source})"
            ))
        })
}

fn endpoint(addr: Option<&str>) -> String {
    let addr = addr
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(DEFAULT_DAEMON_ADDR);

    if addr.contains("://") {
        addr.to_owned()
    } else {
        format!("http://{addr}")
    }
}

fn run_rpc<T>(
    future: impl Future<Output = Result<T, LifecycleError>>,
) -> Result<T, LifecycleError> {
    match tokio::runtime::Handle::try_current() {
        Ok(handle) => tokio::task::block_in_place(|| handle.block_on(future)),
        Err(_) => tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|source| LifecycleError::Daemon(source.to_string()))?
            .block_on(future),
    }
}

fn worker_summary_from_proto(
    summary: pb::WorkerSummary,
) -> Result<LifecycleWorkerSummary, LifecycleError> {
    Ok(LifecycleWorkerSummary {
        worker_id: summary.worker_id,
        mode: mode_from_proto(summary.mode)?,
        repo: summary.repo_identity,
        status: status_label(summary.status)?.to_owned(),
        latest_sequence: summary.latest_sequence,
        activity: if summary.activity.trim().is_empty() {
            status_label(summary.status)?.to_owned()
        } else {
            summary.activity
        },
        terminal_reason: non_empty(summary.terminal_reason),
        pending_request_id: non_empty(summary.pending_request_id),
    })
}

fn non_empty(value: String) -> Option<String> {
    (!value.trim().is_empty()).then_some(value)
}

fn stream_item_from_proto(event: pb::WorkerEvent) -> Result<LifecycleStreamItem, LifecycleError> {
    if event.name == "history_truncated" {
        return Ok(LifecycleStreamItem::HistoryTruncated {
            requested_from_sequence: 0,
            first_available_sequence: event.sequence,
        });
    }

    let occurred_at: Option<SystemTime> = event
        .occurred_at
        .and_then(|timestamp| timestamp.try_into().ok());
    let message = event
        .input
        .as_ref()
        .map(|input| input.prompt.as_str())
        .filter(|prompt| !prompt.trim().is_empty())
        .unwrap_or(&event.message)
        .to_owned();
    let request_id = event.input.map(|input| input.request_id);

    Ok(LifecycleStreamItem::Event(LifecycleWorkerEvent {
        sequence: event.sequence,
        name: event.name,
        status: status_label(event.status)?.to_owned(),
        message,
        request_id,
        occurred_at,
    }))
}

fn mode_to_proto(mode: LifecycleDispatchMode) -> i32 {
    match mode {
        LifecycleDispatchMode::Feature => pb::JobMode::Feature as i32,
        LifecycleDispatchMode::Debug => pb::JobMode::Debug as i32,
    }
}

fn mode_from_proto(mode: i32) -> Result<LifecycleDispatchMode, LifecycleError> {
    match pb::JobMode::try_from(mode) {
        Ok(pb::JobMode::Feature) => Ok(LifecycleDispatchMode::Feature),
        Ok(pb::JobMode::Debug) => Ok(LifecycleDispatchMode::Debug),
        _ => Err(LifecycleError::Daemon(
            "daemon returned unknown mode".to_owned(),
        )),
    }
}

fn status_label(status: i32) -> Result<&'static str, LifecycleError> {
    match pb::WorkerStatus::try_from(status) {
        Ok(pb::WorkerStatus::Accepted) => Ok("accepted"),
        Ok(pb::WorkerStatus::Starting) => Ok("starting"),
        Ok(pb::WorkerStatus::Running) => Ok("running"),
        Ok(pb::WorkerStatus::WaitingForInput) => Ok("waiting_for_input"),
        Ok(pb::WorkerStatus::Succeeded) => Ok("succeeded"),
        Ok(pb::WorkerStatus::Failed) => Ok("failed"),
        Ok(pb::WorkerStatus::Stopped) => Ok("stopped"),
        _ => Err(LifecycleError::Daemon(
            "daemon returned unknown worker status".to_owned(),
        )),
    }
}

fn map_status(status: tonic::Status) -> LifecycleError {
    let message = status.message().to_owned();

    match status.code() {
        tonic::Code::InvalidArgument => LifecycleError::InvalidInput(message),
        tonic::Code::NotFound if message.contains("request") => {
            let mut ids = backticked_values(&message);
            let request_id = ids.next().unwrap_or("unknown").to_owned();
            let worker_id = ids.next().unwrap_or("unknown").to_owned();

            LifecycleError::UnknownRequest {
                worker_id,
                request_id,
            }
        }
        tonic::Code::NotFound if message.contains("worker") => {
            let worker_id = backticked_values(&message)
                .next()
                .unwrap_or("unknown")
                .to_owned();

            LifecycleError::UnknownWorker(worker_id)
        }
        tonic::Code::Unavailable => LifecycleError::DaemonUnavailable(format!(
            "daemon unavailable: {}",
            redact_text(&message)
        )),
        _ => LifecycleError::Daemon(message),
    }
}

fn backticked_values(message: &str) -> impl Iterator<Item = &str> {
    message
        .split('`')
        .skip(1)
        .step_by(2)
        .filter(|value| !value.trim().is_empty())
}
