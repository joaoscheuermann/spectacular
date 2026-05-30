use std::path::PathBuf;
use std::str::FromStr;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use lifecycle::event::StreamEvent;
use lifecycle::identity::{RequestId, WorkerId};
use lifecycle::proto::doric::lifecycle::v1 as pb;
use lifecycle::repo::RepoIdentity;
use lifecycle::status::WorkerStatus;

use crate::error::{DaemonError, DaemonResult};
use crate::event::{RegistryEvent, RegistryWorkerEvent, ReplayItem};
use crate::registry::{
    EventSubscription, InputAnswer, InputRequest, Registry, WorkerMode, WorkerRecord,
};
use crate::root::{prepare_worker_layout, validate_worker_root, WorkerLayout};

/// Dependencies required by the daemon lifecycle service.
pub struct DispatchDeps<L, C, I> {
    pub config: ServiceConfig,
    pub registry: Arc<Mutex<Registry>>,
    pub launcher: L,
    pub command_sender: C,
    pub id_generator: I,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ServiceConfig {
    worker_root: PathBuf,
    event_capacity: usize,
}

impl ServiceConfig {
    pub fn new(worker_root: PathBuf, event_capacity: usize) -> Self {
        Self {
            worker_root,
            event_capacity,
        }
    }

    pub fn worker_root(&self) -> &PathBuf {
        &self.worker_root
    }

    pub fn event_capacity(&self) -> usize {
        self.event_capacity
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LaunchRequest {
    pub worker_id: String,
    pub mode: WorkerMode,
    pub prompt: String,
    pub repo: String,
    pub repo_identity: String,
    pub layout: WorkerLayout,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AnswerCommand {
    pub worker_id: String,
    pub request_id: String,
    pub text: String,
}

#[derive(Debug)]
pub struct WorkerEventStream {
    replay: Vec<pb::WorkerEvent>,
    subscription: EventSubscription,
}

impl WorkerEventStream {
    pub fn replay(&self) -> &[pb::WorkerEvent] {
        &self.replay
    }

    pub fn into_replay(self) -> Vec<pb::WorkerEvent> {
        self.replay
    }

    pub fn try_next(&self) -> Option<pb::WorkerEvent> {
        self.subscription.try_next().map(worker_event_to_proto)
    }

    pub fn next_blocking(&self) -> Option<pb::WorkerEvent> {
        self.subscription.next_blocking().map(worker_event_to_proto)
    }
}

/// Launches a worker after dispatch has been validated and recorded.
pub trait WorkerLauncher: Send + Sync + 'static {
    fn launch(&self, request: LaunchRequest) -> DaemonResult<()>;
}

/// Sends daemon-validated commands to an attached worker.
pub trait CommandSender: Send + Sync + 'static {
    fn send_answer(&self, command: AnswerCommand) -> DaemonResult<()>;
}

/// Produces worker ids at the dispatch boundary.
pub trait IdGenerator: Send + Sync + 'static {
    fn next_worker_id(&self, mode: WorkerMode) -> DaemonResult<String>;
}

/// Plain Rust implementation of the CLI-facing lifecycle service.
pub struct LifecycleService {
    config: ServiceConfig,
    registry: Arc<Mutex<Registry>>,
    launcher: Arc<dyn WorkerLauncher>,
    command_sender: Arc<dyn CommandSender>,
    id_generator: Arc<dyn IdGenerator>,
}

impl LifecycleService {
    pub fn new<L, C, I>(deps: DispatchDeps<L, C, I>) -> Self
    where
        L: WorkerLauncher,
        C: CommandSender,
        I: IdGenerator,
    {
        Self {
            config: deps.config,
            registry: deps.registry,
            launcher: Arc::new(deps.launcher),
            command_sender: Arc::new(deps.command_sender),
            id_generator: Arc::new(deps.id_generator),
        }
    }

    pub fn dispatch(&self, request: pb::DispatchRequest) -> DaemonResult<pb::DispatchResponse> {
        let mode = parse_mode(request.mode)?;
        let prompt = parse_required(request.prompt, "prompt")?;
        let repo = parse_required(request.repo, "repo")?;
        let root = validate_worker_root(self.config.worker_root())?;
        let repo_identity = RepoIdentity::from_raw_url(&repo)
            .map_err(|error| invalid_request(format!("repo is invalid: {error}")))?;
        let worker_id = WorkerId::from_str(&self.id_generator.next_worker_id(mode)?)
            .map_err(|error| invalid_request(error.to_string()))?;
        let layout = prepare_worker_layout(&root, worker_id.clone())?;

        {
            let mut registry = self.registry.lock().expect("registry lock poisoned");
            registry.insert(WorkerRecord::new(
                worker_id.clone(),
                mode,
                repo_identity.clone(),
                WorkerStatus::Accepted,
                "accepted",
            ))?;
            registry.append_event(&worker_id, RegistryEvent::accepted("accepted"))?;
        }

        self.launcher.launch(LaunchRequest {
            worker_id: worker_id.as_str().to_owned(),
            mode,
            prompt,
            repo,
            repo_identity: repo_identity.as_str().to_owned(),
            layout,
        })?;

        Ok(pb::DispatchResponse {
            worker_id: worker_id.as_str().to_owned(),
            mode: mode_to_proto(mode),
            repo_identity: repo_identity.as_str().to_owned(),
            status: status_to_proto(WorkerStatus::Accepted),
        })
    }

    pub fn list(&self, _request: pb::ListWorkersRequest) -> DaemonResult<pb::ListWorkersResponse> {
        let workers = self
            .registry
            .lock()
            .expect("registry lock poisoned")
            .list()
            .into_iter()
            .map(|summary| pb::WorkerSummary {
                worker_id: summary.id().as_str().to_owned(),
                mode: mode_to_proto(summary.mode()),
                status: status_to_proto(summary.status()),
                repo_identity: summary.repo().as_str().to_owned(),
                latest_sequence: summary.last_sequence().unwrap_or_default(),
                updated_at: None,
                activity: summary.activity().unwrap_or_default().to_owned(),
                terminal_reason: summary.terminal_reason().unwrap_or_default().to_owned(),
                pending_request_id: summary
                    .pending_request_id()
                    .map(|request_id| request_id.as_str().to_owned())
                    .unwrap_or_default(),
            })
            .collect();

        Ok(pb::ListWorkersResponse { workers })
    }

    pub fn stream(&self, request: pb::StreamWorkerRequest) -> DaemonResult<WorkerEventStream> {
        let worker_id = WorkerId::from_str(&request.worker_id)
            .map_err(|error| invalid_request(error.to_string()))?;
        let subscription = self
            .registry
            .lock()
            .expect("registry lock poisoned")
            .subscribe(&worker_id, request.from_sequence)?;
        let replay = subscription
            .replay()
            .iter()
            .cloned()
            .map(replay_to_proto)
            .collect();

        Ok(WorkerEventStream {
            replay,
            subscription,
        })
    }

    pub fn answer_input(
        &self,
        request: pb::AnswerInputRequest,
    ) -> DaemonResult<pb::AnswerInputResponse> {
        let worker_id = WorkerId::from_str(&request.worker_id)
            .map_err(|error| invalid_request(error.to_string()))?;
        let request_id = RequestId::from_str(&request.request_id)
            .map_err(|error| invalid_request(error.to_string()))?;

        let answer = InputAnswer::new(worker_id.clone(), request_id.clone(), request.text.clone());

        self.registry
            .lock()
            .expect("registry lock poisoned")
            .validate_answer(&answer)?;

        self.command_sender.send_answer(AnswerCommand {
            worker_id: worker_id.as_str().to_owned(),
            request_id: request_id.as_str().to_owned(),
            text: request.text,
        })?;

        self.registry
            .lock()
            .expect("registry lock poisoned")
            .answer_input(answer)?;

        Ok(pb::AnswerInputResponse { accepted: true })
    }

    pub fn seed_for_test<const N: usize>(
        &mut self,
        workers: [(&str, WorkerMode, i32, &str); N],
    ) -> DaemonResult<()> {
        let mut registry = self.registry.lock().expect("registry lock poisoned");

        for (id, mode, status, activity) in workers {
            let worker_id =
                WorkerId::from_str(id).map_err(|error| invalid_request(error.to_string()))?;
            let status = proto_status(status)?;
            let repo = RepoIdentity::from_raw_url("https://token@example.com/org/repo.git")
                .map_err(|error| invalid_request(error.to_string()))?;
            registry.insert(WorkerRecord::new(
                worker_id.clone(),
                mode,
                repo,
                status,
                activity,
            ))?;

            if status == WorkerStatus::WaitingForInput {
                registry.request_input(InputRequest::new(
                    worker_id,
                    RequestId::from_str("request-1")
                        .map_err(|error| invalid_request(error.to_string()))?,
                    activity,
                ))?;
            }
        }

        Ok(())
    }

    pub fn seed_events_for_test<const N: usize>(
        &mut self,
        worker: &str,
        mode: WorkerMode,
        names: [&str; N],
    ) -> DaemonResult<()> {
        let worker_id =
            WorkerId::from_str(worker).map_err(|error| invalid_request(error.to_string()))?;
        let repo = RepoIdentity::from_raw_url("https://example.com/org/repo.git")
            .map_err(|error| invalid_request(error.to_string()))?;
        let mut registry = self.registry.lock().expect("registry lock poisoned");
        registry.insert(WorkerRecord::new(
            worker_id.clone(),
            mode,
            repo,
            WorkerStatus::Accepted,
            "accepted",
        ))?;

        for name in names {
            registry.append_event(&worker_id, event_for_name(name))?;
        }

        Ok(())
    }

    pub fn seed_waiting_for_test(
        &mut self,
        worker: &str,
        mode: WorkerMode,
        request: &str,
        prompt: &str,
    ) -> DaemonResult<()> {
        let worker_id =
            WorkerId::from_str(worker).map_err(|error| invalid_request(error.to_string()))?;
        let request_id =
            RequestId::from_str(request).map_err(|error| invalid_request(error.to_string()))?;
        let repo = RepoIdentity::from_raw_url("https://example.com/org/repo.git")
            .map_err(|error| invalid_request(error.to_string()))?;
        let mut registry = self.registry.lock().expect("registry lock poisoned");

        registry.insert(WorkerRecord::new(
            worker_id.clone(),
            mode,
            repo,
            WorkerStatus::Running,
            "running",
        ))?;
        registry.request_input(InputRequest::new(worker_id, request_id, prompt))?;

        Ok(())
    }

    pub fn append_event_for_test(&self, worker: &str, name: &str) -> DaemonResult<()> {
        let worker_id =
            WorkerId::from_str(worker).map_err(|error| invalid_request(error.to_string()))?;
        self.registry
            .lock()
            .expect("registry lock poisoned")
            .append_event(&worker_id, event_for_name(name))?;

        Ok(())
    }
}

pub struct TimestampIdGenerator;

impl IdGenerator for TimestampIdGenerator {
    fn next_worker_id(&self, mode: WorkerMode) -> DaemonResult<String> {
        let label = match mode {
            WorkerMode::Feature => "feature",
            WorkerMode::Debug => "debug",
        };
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();

        Ok(format!("{label}-{now}"))
    }
}

fn parse_mode(mode: i32) -> DaemonResult<WorkerMode> {
    match pb::JobMode::try_from(mode) {
        Ok(pb::JobMode::Feature) => Ok(WorkerMode::Feature),
        Ok(pb::JobMode::Debug) => Ok(WorkerMode::Debug),
        _ => Err(invalid_request("mode must be feature or debug")),
    }
}

fn parse_required(value: String, name: &'static str) -> DaemonResult<String> {
    let trimmed = value.trim();

    if trimmed.is_empty() {
        Err(invalid_request(format!("{name} must not be blank")))
    } else {
        Ok(trimmed.to_owned())
    }
}

fn proto_status(status: i32) -> DaemonResult<WorkerStatus> {
    match pb::WorkerStatus::try_from(status) {
        Ok(value) => {
            WorkerStatus::try_from(value).map_err(|error| invalid_request(error.to_string()))
        }
        Err(_) => Err(invalid_request("status must be known")),
    }
}

fn replay_to_proto(item: ReplayItem) -> pb::WorkerEvent {
    match item {
        ReplayItem::Worker(event) => worker_event_to_proto(event),
        ReplayItem::HistoryTruncated(event) => truncated_to_proto(event),
    }
}

fn worker_event_to_proto(event: RegistryWorkerEvent) -> pb::WorkerEvent {
    let input = event.request_id().map(|request_id| pb::InputRequest {
        request_id: request_id.as_str().to_owned(),
        prompt: event.message().to_owned(),
        choices: Vec::new(),
    });

    pb::WorkerEvent {
        worker_id: event.worker_id().as_str().to_owned(),
        sequence: event.sequence(),
        status: event_status(event.name()),
        name: event.name().to_owned(),
        message: event.message().to_owned(),
        input,
        occurred_at: None,
    }
}

fn truncated_to_proto(event: StreamEvent) -> pb::WorkerEvent {
    pb::WorkerEvent {
        worker_id: event.worker_id().as_str().to_owned(),
        sequence: event.first_available_sequence(),
        status: status_to_proto(WorkerStatus::Untracked),
        name: event.name().to_owned(),
        message: event.message().to_owned(),
        input: None,
        occurred_at: None,
    }
}

fn event_status(name: &str) -> i32 {
    match name {
        "accepted" => status_to_proto(WorkerStatus::Accepted),
        "starting" => status_to_proto(WorkerStatus::Starting),
        "waiting_for_input" => status_to_proto(WorkerStatus::WaitingForInput),
        "failed" => status_to_proto(WorkerStatus::Failed),
        "succeeded" => status_to_proto(WorkerStatus::Succeeded),
        "stopped" => status_to_proto(WorkerStatus::Stopped),
        _ => status_to_proto(WorkerStatus::Running),
    }
}

fn event_for_name(name: &str) -> RegistryEvent {
    match name {
        "accepted" => RegistryEvent::accepted("accepted"),
        "starting" => RegistryEvent::starting("starting"),
        "repo_preparation" => RegistryEvent::repo_preparation("repo preparation"),
        "prompt_agent_started" => RegistryEvent::prompt_agent_started("prompt agent started"),
        "failed" => RegistryEvent::Failed("failed".to_owned()),
        "succeeded" => RegistryEvent::Succeeded("succeeded".to_owned()),
        "stopped" => RegistryEvent::Stopped("stopped".to_owned()),
        _ => RegistryEvent::current_activity(name.to_owned()),
    }
}

pub fn mode_to_proto(mode: WorkerMode) -> i32 {
    match mode {
        WorkerMode::Feature => pb::JobMode::Feature as i32,
        WorkerMode::Debug => pb::JobMode::Debug as i32,
    }
}

pub fn status_to_proto(status: WorkerStatus) -> i32 {
    match status {
        WorkerStatus::Accepted => pb::WorkerStatus::Accepted as i32,
        WorkerStatus::Starting => pb::WorkerStatus::Starting as i32,
        WorkerStatus::Running => pb::WorkerStatus::Running as i32,
        WorkerStatus::WaitingForInput => pb::WorkerStatus::WaitingForInput as i32,
        WorkerStatus::Succeeded => pb::WorkerStatus::Succeeded as i32,
        WorkerStatus::Failed => pb::WorkerStatus::Failed as i32,
        WorkerStatus::Stopped => pb::WorkerStatus::Stopped as i32,
        WorkerStatus::Unavailable | WorkerStatus::Untracked => pb::WorkerStatus::Unspecified as i32,
    }
}

fn invalid_request(message: impl Into<String>) -> DaemonError {
    DaemonError::root_configuration(None, message, None)
}
