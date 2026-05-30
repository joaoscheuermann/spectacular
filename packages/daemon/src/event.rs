use lifecycle::event::{StreamEvent, WorkerEvent};
use lifecycle::identity::{RequestId, WorkerId};

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
}

impl From<WorkerEvent> for RegistryWorkerEvent {
    fn from(event: WorkerEvent) -> Self {
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
        }
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

    pub fn into_worker_event(self, worker_id: WorkerId, sequence: u64) -> RegistryWorkerEvent {
        match self {
            Self::Accepted(message) => WorkerEvent::accepted(worker_id, sequence, message).into(),
            Self::Starting(message) => WorkerEvent::starting(worker_id, sequence, message).into(),
            Self::RepoPreparation(message) => {
                WorkerEvent::repo_preparation(worker_id, sequence, message).into()
            }
            Self::PromptAgentStarted(message) => {
                WorkerEvent::prompt_agent_started(worker_id, sequence, message).into()
            }
            Self::PromptArtifactWritten(message) => {
                WorkerEvent::prompt_artifact_written(worker_id, sequence, message).into()
            }
            Self::PromptAgentCompleted(message) => {
                WorkerEvent::prompt_agent_completed(worker_id, sequence, message).into()
            }
            Self::CurrentActivity(message) => {
                WorkerEvent::current_activity(worker_id, sequence, message).into()
            }
            Self::WaitingForInput {
                request_id,
                message,
            } => WorkerEvent::waiting_for_input(worker_id, sequence, request_id, message).into(),
            Self::AnswerProvided {
                request_id,
                message,
            } => RegistryWorkerEvent {
                worker_id,
                sequence,
                name: "answer_provided",
                message,
                request_id: Some(request_id),
            },
            Self::Failed(message) => WorkerEvent::failed(worker_id, sequence, message).into(),
            Self::Succeeded(message) => WorkerEvent::succeeded(worker_id, sequence, message).into(),
            Self::Stopped(message) => WorkerEvent::stopped(worker_id, sequence, message).into(),
        }
    }
}
