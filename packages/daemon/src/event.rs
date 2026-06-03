use std::collections::VecDeque;
use std::io::Write;
use std::sync::{Arc, Mutex};
use std::time::SystemTime;

use lifecycle::event::{StreamEvent, WorkerEvent};
use lifecycle::identity::{RequestId, WorkerId};
use lifecycle::proto::doric::lifecycle::v1 as pb;
use lifecycle::status::WorkerStatus;
use lifecycle::terminal;

/// Emits concise daemon lifecycle messages outside durable registry state.
pub trait LifecycleLogger: Send + Sync + 'static {
    fn log(&self, message: &str);
}

#[derive(Clone, Debug, Default)]
pub struct NoopLifecycleLogger;

impl LifecycleLogger for NoopLifecycleLogger {
    fn log(&self, _message: &str) {}
}

#[derive(Clone)]
pub struct TerminalLifecycleLogger {
    write: Arc<dyn Fn(&str) + Send + Sync>,
    timestamp: Arc<dyn Fn() -> SystemTime + Send + Sync>,
}

impl TerminalLifecycleLogger {
    pub fn stderr() -> Self {
        Self {
            write: Arc::new(|line| {
                let _ = std::io::stderr().write_all(line.as_bytes());
            }),
            timestamp: Arc::new(SystemTime::now),
        }
    }

    pub fn for_writer(output: Arc<Mutex<Vec<u8>>>, timestamp: SystemTime) -> Self {
        Self {
            write: Arc::new(move |line| {
                let _ = output
                    .lock()
                    .expect("terminal output lock poisoned")
                    .write_all(line.as_bytes());
            }),
            timestamp: Arc::new(move || timestamp),
        }
    }
}

impl LifecycleLogger for TerminalLifecycleLogger {
    fn log(&self, message: &str) {
        let line = format!("{}\n", terminal::format_line((self.timestamp)(), message));
        (self.write)(&line);
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ReplayItem {
    Worker(RegistryWorkerEvent),
    HistoryTruncated(StreamEvent),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RegistryWorkerEvent {
    worker_id: WorkerId,
    sequence: u64,
    name: &'static str,
    message: String,
    request_id: Option<RequestId>,
    occurred_at: SystemTime,
}

impl RegistryWorkerEvent {
    pub fn sequence(&self) -> u64 {
        self.sequence
    }

    pub fn name(&self) -> &str {
        self.name
    }

    pub fn worker_id(&self) -> &WorkerId {
        &self.worker_id
    }

    pub fn message(&self) -> &str {
        &self.message
    }

    pub fn request_id(&self) -> Option<&RequestId> {
        self.request_id.as_ref()
    }

    pub fn occurred_at(&self) -> Option<SystemTime> {
        Some(self.occurred_at)
    }

    fn from_worker_event(event: WorkerEvent, occurred_at: SystemTime) -> Self {
        Self {
            worker_id: event.worker_id().clone(),
            sequence: event.sequence(),
            name: match event.name() {
                "accepted" => "accepted",
                "starting" => "starting",
                "repo_preparation" => "repo_preparation",
                "prompt_agent_started" => "prompt_agent_started",
                "prompt_artifact_written" => "prompt_artifact_written",
                "prompt_agent_completed" => "prompt_agent_completed",
                "current_activity" => "current_activity",
                "waiting_for_input" => "waiting_for_input",
                "failed" => "failed",
                "succeeded" => "succeeded",
                "stopped" => "stopped",
                _ => "worker_event",
            },
            message: event.message().to_owned(),
            request_id: event.request_id().cloned(),
            occurred_at,
        }
    }
}

impl From<WorkerEvent> for RegistryWorkerEvent {
    fn from(event: WorkerEvent) -> Self {
        Self::from_worker_event(event, SystemTime::now())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RegistryEvent {
    Accepted(String),
    Starting(String),
    RepoPreparation(String),
    PromptAgentStarted(String),
    PromptArtifactWritten(String),
    PromptAgentCompleted(String),
    CurrentActivity(String),
    WaitingForInput {
        request_id: RequestId,
        message: String,
    },
    AnswerProvided {
        request_id: RequestId,
        message: String,
    },
    Failed(String),
    Succeeded(String),
    Stopped(String),
}

impl RegistryEvent {
    pub fn accepted(message: impl Into<String>) -> Self {
        Self::Accepted(message.into())
    }

    pub fn starting(message: impl Into<String>) -> Self {
        Self::Starting(message.into())
    }

    pub fn repo_preparation(message: impl Into<String>) -> Self {
        Self::RepoPreparation(message.into())
    }

    pub fn prompt_agent_started(message: impl Into<String>) -> Self {
        Self::PromptAgentStarted(message.into())
    }

    pub fn prompt_artifact_written(message: impl Into<String>) -> Self {
        Self::PromptArtifactWritten(message.into())
    }

    pub fn prompt_agent_completed(message: impl Into<String>) -> Self {
        Self::PromptAgentCompleted(message.into())
    }

    pub fn current_activity(message: impl Into<String>) -> Self {
        Self::CurrentActivity(message.into())
    }

    pub fn waiting_for_input(request_id: RequestId, message: impl Into<String>) -> Self {
        Self::WaitingForInput {
            request_id,
            message: message.into(),
        }
    }

    pub fn answer_provided(request_id: RequestId) -> Self {
        Self::AnswerProvided {
            request_id,
            message: "input answered; continuing".to_owned(),
        }
    }

    pub fn into_worker_event(
        self,
        worker_id: WorkerId,
        sequence: u64,
        occurred_at: SystemTime,
    ) -> RegistryWorkerEvent {
        match self {
            Self::Accepted(message) => RegistryWorkerEvent::from_worker_event(
                WorkerEvent::accepted(worker_id, sequence, message),
                occurred_at,
            ),
            Self::Starting(message) => RegistryWorkerEvent::from_worker_event(
                WorkerEvent::starting(worker_id, sequence, message),
                occurred_at,
            ),
            Self::RepoPreparation(message) => RegistryWorkerEvent::from_worker_event(
                WorkerEvent::repo_preparation(worker_id, sequence, message),
                occurred_at,
            ),
            Self::PromptAgentStarted(message) => RegistryWorkerEvent::from_worker_event(
                WorkerEvent::prompt_agent_started(worker_id, sequence, message),
                occurred_at,
            ),
            Self::PromptArtifactWritten(message) => RegistryWorkerEvent::from_worker_event(
                WorkerEvent::prompt_artifact_written(worker_id, sequence, message),
                occurred_at,
            ),
            Self::PromptAgentCompleted(message) => RegistryWorkerEvent::from_worker_event(
                WorkerEvent::prompt_agent_completed(worker_id, sequence, message),
                occurred_at,
            ),
            Self::CurrentActivity(message) => RegistryWorkerEvent::from_worker_event(
                WorkerEvent::current_activity(worker_id, sequence, message),
                occurred_at,
            ),
            Self::WaitingForInput {
                request_id,
                message,
            } => RegistryWorkerEvent::from_worker_event(
                WorkerEvent::waiting_for_input(worker_id, sequence, request_id, message),
                occurred_at,
            ),
            Self::AnswerProvided {
                request_id,
                message,
            } => RegistryWorkerEvent {
                worker_id,
                sequence,
                name: "answer_provided",
                message,
                request_id: Some(request_id),
                occurred_at,
            },
            Self::Failed(message) => RegistryWorkerEvent::from_worker_event(
                WorkerEvent::failed(worker_id, sequence, message),
                occurred_at,
            ),
            Self::Succeeded(message) => RegistryWorkerEvent::from_worker_event(
                WorkerEvent::succeeded(worker_id, sequence, message),
                occurred_at,
            ),
            Self::Stopped(message) => RegistryWorkerEvent::from_worker_event(
                WorkerEvent::stopped(worker_id, sequence, message),
                occurred_at,
            ),
        }
    }
}

pub(crate) struct EventLog {
    retained: VecDeque<RegistryWorkerEvent>,
    next_sequence: u64,
    capacity: usize,
}

impl EventLog {
    pub(crate) fn new(capacity: usize) -> Self {
        Self {
            retained: VecDeque::with_capacity(capacity),
            next_sequence: 0,
            capacity,
        }
    }

    pub(crate) fn next_sequence(&self) -> u64 {
        self.next_sequence
    }

    pub(crate) fn push(&mut self, event: RegistryWorkerEvent) {
        if self.retained.len() == self.capacity {
            self.retained.pop_front();
        }

        self.next_sequence += 1;
        self.retained.push_back(event);
    }

    pub(crate) fn replay(&self, worker_id: &WorkerId, from_sequence: u64) -> Vec<ReplayItem> {
        let mut items = Vec::new();
        let first_available = self.retained.front().map(RegistryWorkerEvent::sequence);

        if let Some(first_available) = first_available {
            if from_sequence < first_available {
                items.push(ReplayItem::HistoryTruncated(
                    StreamEvent::history_truncated(
                        worker_id.clone(),
                        from_sequence,
                        first_available,
                    ),
                ));
            }
        }

        items.extend(
            self.retained
                .iter()
                .filter(|event| event.sequence() >= from_sequence)
                .cloned()
                .map(ReplayItem::Worker),
        );

        items
    }

    pub(crate) fn last_sequence(&self) -> Option<u64> {
        self.retained.back().map(RegistryWorkerEvent::sequence)
    }
}

pub(crate) fn replay_to_proto(item: ReplayItem) -> pb::WorkerEvent {
    match item {
        ReplayItem::Worker(event) => worker_event_to_proto(event),
        ReplayItem::HistoryTruncated(event) => truncated_to_proto(event),
    }
}

pub(crate) fn worker_event_to_proto(event: RegistryWorkerEvent) -> pb::WorkerEvent {
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
        occurred_at: event.occurred_at().map(Into::into),
    }
}

pub(crate) fn event_for_status(status: WorkerStatus, message: String) -> RegistryEvent {
    match status {
        WorkerStatus::Accepted => RegistryEvent::accepted(message),
        WorkerStatus::Starting => RegistryEvent::starting(message),
        WorkerStatus::WaitingForInput => RegistryEvent::current_activity(message),
        WorkerStatus::Succeeded => RegistryEvent::Succeeded(message),
        WorkerStatus::Failed => RegistryEvent::Failed(message),
        WorkerStatus::Stopped => RegistryEvent::Stopped(message),
        WorkerStatus::Running | WorkerStatus::Unavailable | WorkerStatus::Untracked => {
            RegistryEvent::current_activity(message)
        }
    }
}

pub(crate) fn event_for_worker_event(
    name: &str,
    status: WorkerStatus,
    message: String,
) -> RegistryEvent {
    match name {
        "repo_preparation" => RegistryEvent::repo_preparation(message),
        "prompt_agent_started" => RegistryEvent::prompt_agent_started(message),
        "prompt_artifact_written" => RegistryEvent::prompt_artifact_written(message),
        "prompt_agent_completed" => RegistryEvent::prompt_agent_completed(message),
        _ => event_for_status(status, message),
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
        "prompt_agent_completed" => status_to_proto(WorkerStatus::Succeeded),
        "failed" => status_to_proto(WorkerStatus::Failed),
        "succeeded" => status_to_proto(WorkerStatus::Succeeded),
        "stopped" => status_to_proto(WorkerStatus::Stopped),
        _ => status_to_proto(WorkerStatus::Running),
    }
}

fn status_to_proto(status: WorkerStatus) -> i32 {
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
