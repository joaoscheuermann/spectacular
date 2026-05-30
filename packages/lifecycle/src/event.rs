use crate::identity::{RequestId, WorkerId};
use crate::redaction::redact_failure_text;
use crate::status::WorkerStatus;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkerEvent {
    worker_id: WorkerId,
    sequence: u64,
    status: WorkerStatus,
    name: &'static str,
    message: String,
    request_id: Option<RequestId>,
}

impl WorkerEvent {
    pub fn accepted(worker_id: WorkerId, sequence: u64, message: impl Into<String>) -> Self {
        Self::new(
            worker_id,
            sequence,
            WorkerStatus::Accepted,
            "accepted",
            message,
        )
    }

    pub fn starting(worker_id: WorkerId, sequence: u64, message: impl Into<String>) -> Self {
        Self::new(
            worker_id,
            sequence,
            WorkerStatus::Starting,
            "starting",
            message,
        )
    }

    pub fn repo_preparation(
        worker_id: WorkerId,
        sequence: u64,
        message: impl Into<String>,
    ) -> Self {
        Self::new(
            worker_id,
            sequence,
            WorkerStatus::Running,
            "repo_preparation",
            message,
        )
    }

    pub fn prompt_agent_started(
        worker_id: WorkerId,
        sequence: u64,
        message: impl Into<String>,
    ) -> Self {
        Self::new(
            worker_id,
            sequence,
            WorkerStatus::Running,
            "prompt_agent_started",
            message,
        )
    }

    pub fn prompt_artifact_written(
        worker_id: WorkerId,
        sequence: u64,
        message: impl Into<String>,
    ) -> Self {
        Self::new(
            worker_id,
            sequence,
            WorkerStatus::Running,
            "prompt_artifact_written",
            message,
        )
    }

    pub fn prompt_answer_consumed(
        worker_id: WorkerId,
        sequence: u64,
        request_id: RequestId,
        message: impl Into<String>,
    ) -> Self {
        Self::new(
            worker_id,
            sequence,
            WorkerStatus::Running,
            "prompt_answer_consumed",
            message,
        )
        .with_request_id(request_id)
    }

    pub fn prompt_agent_completed(
        worker_id: WorkerId,
        sequence: u64,
        message: impl Into<String>,
    ) -> Self {
        Self::new(
            worker_id,
            sequence,
            WorkerStatus::Succeeded,
            "prompt_agent_completed",
            message,
        )
    }

    pub fn prompt_agent_failed(
        worker_id: WorkerId,
        sequence: u64,
        message: impl Into<String>,
    ) -> Self {
        let message = redact_failure_text(&message.into());
        Self::new(
            worker_id,
            sequence,
            WorkerStatus::Failed,
            "prompt_agent_failed",
            message,
        )
    }

    pub fn current_activity(
        worker_id: WorkerId,
        sequence: u64,
        message: impl Into<String>,
    ) -> Self {
        Self::new(
            worker_id,
            sequence,
            WorkerStatus::Running,
            "current_activity",
            message,
        )
    }

    pub fn waiting_for_input(
        worker_id: WorkerId,
        sequence: u64,
        request_id: RequestId,
        message: impl Into<String>,
    ) -> Self {
        Self::new(
            worker_id,
            sequence,
            WorkerStatus::WaitingForInput,
            "waiting_for_input",
            message,
        )
        .with_request_id(request_id)
    }

    pub fn failed(worker_id: WorkerId, sequence: u64, message: impl Into<String>) -> Self {
        let message = redact_failure_text(&message.into());
        Self::new(worker_id, sequence, WorkerStatus::Failed, "failed", message)
    }

    pub fn succeeded(worker_id: WorkerId, sequence: u64, message: impl Into<String>) -> Self {
        Self::new(
            worker_id,
            sequence,
            WorkerStatus::Succeeded,
            "succeeded",
            message,
        )
    }

    pub fn stopped(worker_id: WorkerId, sequence: u64, message: impl Into<String>) -> Self {
        Self::new(
            worker_id,
            sequence,
            WorkerStatus::Stopped,
            "stopped",
            message,
        )
    }

    pub fn worker_id(&self) -> &WorkerId {
        &self.worker_id
    }

    pub fn sequence(&self) -> u64 {
        self.sequence
    }

    pub fn status(&self) -> WorkerStatus {
        self.status
    }

    pub fn name(&self) -> &str {
        self.name
    }

    pub fn message(&self) -> &str {
        &self.message
    }

    pub fn request_id(&self) -> Option<&RequestId> {
        self.request_id.as_ref()
    }

    fn new(
        worker_id: WorkerId,
        sequence: u64,
        status: WorkerStatus,
        name: &'static str,
        message: impl Into<String>,
    ) -> Self {
        Self {
            worker_id,
            sequence,
            status,
            name,
            message: message.into(),
            request_id: None,
        }
    }

    fn with_request_id(mut self, request_id: RequestId) -> Self {
        self.request_id = Some(request_id);
        self
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StreamEvent {
    worker_id: WorkerId,
    name: &'static str,
    message: &'static str,
    requested_from_sequence: u64,
    first_available_sequence: u64,
}

impl StreamEvent {
    pub fn history_truncated(
        worker_id: WorkerId,
        requested_from_sequence: u64,
        first_available_sequence: u64,
    ) -> Self {
        Self {
            worker_id,
            name: "history_truncated",
            message: "worker event history was truncated",
            requested_from_sequence,
            first_available_sequence,
        }
    }

    pub fn worker_id(&self) -> &WorkerId {
        &self.worker_id
    }

    pub fn name(&self) -> &str {
        self.name
    }

    pub fn message(&self) -> &str {
        self.message
    }

    pub fn requested_from_sequence(&self) -> u64 {
        self.requested_from_sequence
    }

    pub fn first_available_sequence(&self) -> u64 {
        self.first_available_sequence
    }
}
