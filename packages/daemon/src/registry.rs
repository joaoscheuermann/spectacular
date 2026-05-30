use std::collections::{HashMap, VecDeque};
use std::sync::mpsc::{self, Receiver, Sender};

use lifecycle::event::StreamEvent;
use lifecycle::identity::{RequestId, WorkerId};
use lifecycle::repo::RepoIdentity;
use lifecycle::status::WorkerStatus;

use crate::error::{DaemonError, DaemonResult};
use crate::event::{RegistryEvent, RegistryWorkerEvent, ReplayItem};

const DEFAULT_EVENT_CAPACITY: usize = 1_000;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WorkerMode {
    Feature,
    Debug,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkerRecord {
    id: WorkerId,
    mode: WorkerMode,
    repo: RepoIdentity,
    status: WorkerStatus,
    activity: Option<String>,
    terminal_reason: Option<String>,
    pending_request_id: Option<RequestId>,
}

impl WorkerRecord {
    pub fn new(
        id: WorkerId,
        mode: WorkerMode,
        repo: RepoIdentity,
        status: WorkerStatus,
        activity: impl Into<String>,
    ) -> Self {
        let activity = Some(activity.into());
        let terminal_reason = terminal_reason(status, activity.as_deref());

        Self {
            id,
            mode,
            repo,
            status,
            activity,
            terminal_reason,
            pending_request_id: None,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkerSummary {
    id: WorkerId,
    mode: WorkerMode,
    repo: RepoIdentity,
    status: WorkerStatus,
    activity: Option<String>,
    terminal_reason: Option<String>,
    pending_request_id: Option<RequestId>,
    last_sequence: Option<u64>,
}

impl WorkerSummary {
    pub fn id(&self) -> &WorkerId {
        &self.id
    }

    pub fn mode(&self) -> WorkerMode {
        self.mode
    }

    pub fn repo(&self) -> &RepoIdentity {
        &self.repo
    }

    pub fn status(&self) -> WorkerStatus {
        self.status
    }

    pub fn activity(&self) -> Option<&str> {
        self.activity.as_deref()
    }

    pub fn terminal_reason(&self) -> Option<&str> {
        self.terminal_reason.as_deref()
    }

    pub fn pending_request_id(&self) -> Option<&RequestId> {
        self.pending_request_id.as_ref()
    }

    pub fn last_sequence(&self) -> Option<u64> {
        self.last_sequence
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InputRequest {
    worker_id: WorkerId,
    request_id: RequestId,
    prompt: String,
}

impl InputRequest {
    pub fn new(worker_id: WorkerId, request_id: RequestId, prompt: impl Into<String>) -> Self {
        Self {
            worker_id,
            request_id,
            prompt: prompt.into(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InputAnswer {
    worker_id: WorkerId,
    request_id: RequestId,
}

impl InputAnswer {
    pub fn new(worker_id: WorkerId, request_id: RequestId, _text: impl Into<String>) -> Self {
        Self {
            worker_id,
            request_id,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PendingInput {
    request_id: RequestId,
    prompt: String,
}

pub struct Registry {
    records: HashMap<WorkerId, WorkerRecord>,
    order: Vec<WorkerId>,
    events: HashMap<WorkerId, EventLog>,
    subscribers: HashMap<WorkerId, Vec<Sender<RegistryWorkerEvent>>>,
    pending: HashMap<WorkerId, PendingInput>,
    answered: HashMap<(WorkerId, RequestId), ()>,
    event_capacity: usize,
}

#[derive(Debug)]
pub struct EventSubscription {
    replay: Vec<ReplayItem>,
    receiver: Receiver<RegistryWorkerEvent>,
}

impl EventSubscription {
    pub fn replay(&self) -> &[ReplayItem] {
        &self.replay
    }

    pub fn into_replay(self) -> Vec<ReplayItem> {
        self.replay
    }

    pub fn try_next(&self) -> Option<RegistryWorkerEvent> {
        self.receiver.try_recv().ok()
    }
}

impl Registry {
    /// Creates a fresh in-memory registry; no state survives process restart.
    pub fn in_memory() -> Self {
        Self::in_memory_with_event_capacity(DEFAULT_EVENT_CAPACITY)
    }

    /// Creates a fresh in-memory registry with a bounded per-worker event ring.
    pub fn in_memory_with_event_capacity(event_capacity: usize) -> Self {
        Self {
            records: HashMap::new(),
            order: Vec::new(),
            events: HashMap::new(),
            subscribers: HashMap::new(),
            pending: HashMap::new(),
            answered: HashMap::new(),
            event_capacity: event_capacity.max(1),
        }
    }

    pub fn insert(&mut self, record: WorkerRecord) -> DaemonResult<()> {
        if self.records.contains_key(&record.id) {
            return Err(DaemonError::DuplicateWorker {
                worker_id: record.id,
            });
        }

        self.events
            .insert(record.id.clone(), EventLog::new(self.event_capacity));
        self.subscribers.insert(record.id.clone(), Vec::new());
        self.order.push(record.id.clone());
        self.records.insert(record.id.clone(), record);

        Ok(())
    }

    pub fn list(&self) -> Vec<WorkerSummary> {
        self.order
            .iter()
            .filter_map(|id| self.records.get(id).map(|record| self.summary(record)))
            .collect()
    }

    pub fn update_status(
        &mut self,
        worker_id: &WorkerId,
        status: WorkerStatus,
        activity: impl Into<String>,
    ) -> DaemonResult<()> {
        let record = self.record_mut(worker_id)?;
        let activity = activity.into();

        record.status = status;
        record.terminal_reason = terminal_reason(status, Some(&activity));
        record.activity = Some(activity);

        if status != WorkerStatus::WaitingForInput {
            record.pending_request_id = None;
        }

        if is_terminal(status) {
            self.pending.remove(worker_id);
        }

        Ok(())
    }

    pub fn append_event(
        &mut self,
        worker_id: &WorkerId,
        event: RegistryEvent,
    ) -> DaemonResult<RegistryWorkerEvent> {
        self.ensure_known(worker_id)?;
        let log = self
            .events
            .get_mut(worker_id)
            .expect("known worker has log");
        let sequence = log.next_sequence;
        let event = event.into_worker_event(worker_id.clone(), sequence);
        log.push(event.clone());
        self.publish_event(worker_id, event.clone());
        Ok(event)
    }

    pub fn replay(
        &self,
        worker_id: &WorkerId,
        from_sequence: u64,
    ) -> DaemonResult<Vec<ReplayItem>> {
        self.ensure_known(worker_id)?;
        let log = self.events.get(worker_id).expect("known worker has log");
        Ok(log.replay(worker_id, from_sequence))
    }

    pub fn subscribe(
        &mut self,
        worker_id: &WorkerId,
        from_sequence: u64,
    ) -> DaemonResult<EventSubscription> {
        self.ensure_known(worker_id)?;
        let replay = self
            .events
            .get(worker_id)
            .expect("known worker has log")
            .replay(worker_id, from_sequence);
        let (sender, receiver) = mpsc::channel();
        self.subscribers
            .get_mut(worker_id)
            .expect("known worker has subscribers")
            .push(sender);

        Ok(EventSubscription { replay, receiver })
    }

    pub fn request_input(&mut self, request: InputRequest) -> DaemonResult<()> {
        let record = self.record_mut(&request.worker_id)?;

        if is_terminal(record.status) {
            return Err(DaemonError::TerminalWorker {
                worker_id: request.worker_id,
            });
        }

        record.status = WorkerStatus::WaitingForInput;
        record.activity = Some(request.prompt.clone());
        record.terminal_reason = None;
        record.pending_request_id = Some(request.request_id.clone());
        self.pending.insert(
            request.worker_id.clone(),
            PendingInput {
                request_id: request.request_id.clone(),
                prompt: request.prompt.clone(),
            },
        );

        self.append_event(
            &request.worker_id,
            RegistryEvent::waiting_for_input(request.request_id, request.prompt),
        )?;

        Ok(())
    }

    pub fn answer_input(&mut self, answer: InputAnswer) -> DaemonResult<()> {
        self.validate_answer(&answer)?;

        self.pending.remove(&answer.worker_id);
        self.answered
            .insert((answer.worker_id.clone(), answer.request_id.clone()), ());

        let record = self.record_mut(&answer.worker_id)?;
        record.status = WorkerStatus::Running;
        record.activity = Some("input answered".to_owned());
        record.terminal_reason = None;
        record.pending_request_id = None;

        self.append_event(
            &answer.worker_id,
            RegistryEvent::answer_provided(answer.request_id),
        )?;

        Ok(())
    }

    pub fn validate_answer(&self, answer: &InputAnswer) -> DaemonResult<()> {
        let status = self.record(&answer.worker_id)?.status;

        if self
            .answered
            .contains_key(&(answer.worker_id.clone(), answer.request_id.clone()))
        {
            return Err(DaemonError::DuplicateAnswer {
                worker_id: answer.worker_id.clone(),
                request_id: answer.request_id.clone(),
            });
        }

        if is_terminal(status) {
            return Err(DaemonError::TerminalWorker {
                worker_id: answer.worker_id.clone(),
            });
        }

        if status != WorkerStatus::WaitingForInput {
            return Err(DaemonError::NotWaitingForInput {
                worker_id: answer.worker_id.clone(),
            });
        }

        let pending =
            self.pending
                .get(&answer.worker_id)
                .ok_or_else(|| DaemonError::NoPendingInput {
                    worker_id: answer.worker_id.clone(),
                })?;

        if pending.request_id != answer.request_id {
            return Err(DaemonError::StaleRequest {
                worker_id: answer.worker_id.clone(),
                request_id: answer.request_id.clone(),
            });
        }

        Ok(())
    }

    pub fn pending_input(
        &self,
        worker_id: &WorkerId,
        request_id: &RequestId,
    ) -> Option<&PendingInput> {
        self.pending
            .get(worker_id)
            .filter(|pending| &pending.request_id == request_id)
    }

    fn summary(&self, record: &WorkerRecord) -> WorkerSummary {
        WorkerSummary {
            id: record.id.clone(),
            mode: record.mode,
            repo: record.repo.clone(),
            status: record.status,
            activity: record.activity.clone(),
            terminal_reason: record.terminal_reason.clone(),
            pending_request_id: record.pending_request_id.clone(),
            last_sequence: self
                .events
                .get(&record.id)
                .and_then(EventLog::last_sequence),
        }
    }

    fn ensure_known(&self, worker_id: &WorkerId) -> DaemonResult<()> {
        if self.records.contains_key(worker_id) {
            Ok(())
        } else {
            Err(DaemonError::UnknownWorker {
                worker_id: worker_id.clone(),
            })
        }
    }

    fn record(&self, worker_id: &WorkerId) -> DaemonResult<&WorkerRecord> {
        self.records
            .get(worker_id)
            .ok_or_else(|| DaemonError::UnknownWorker {
                worker_id: worker_id.clone(),
            })
    }

    fn record_mut(&mut self, worker_id: &WorkerId) -> DaemonResult<&mut WorkerRecord> {
        self.records
            .get_mut(worker_id)
            .ok_or_else(|| DaemonError::UnknownWorker {
                worker_id: worker_id.clone(),
            })
    }

    fn publish_event(&mut self, worker_id: &WorkerId, event: RegistryWorkerEvent) {
        if let Some(subscribers) = self.subscribers.get_mut(worker_id) {
            subscribers.retain(|sender| sender.send(event.clone()).is_ok());
        }
    }
}

struct EventLog {
    retained: VecDeque<RegistryWorkerEvent>,
    next_sequence: u64,
    capacity: usize,
}

impl EventLog {
    fn new(capacity: usize) -> Self {
        Self {
            retained: VecDeque::with_capacity(capacity),
            next_sequence: 0,
            capacity,
        }
    }

    fn push(&mut self, event: RegistryWorkerEvent) {
        if self.retained.len() == self.capacity {
            self.retained.pop_front();
        }

        self.next_sequence += 1;
        self.retained.push_back(event);
    }

    fn replay(&self, worker_id: &WorkerId, from_sequence: u64) -> Vec<ReplayItem> {
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

    fn last_sequence(&self) -> Option<u64> {
        self.retained.back().map(RegistryWorkerEvent::sequence)
    }
}

fn terminal_reason(status: WorkerStatus, activity: Option<&str>) -> Option<String> {
    is_terminal(status)
        .then(|| activity.map(str::to_owned))
        .flatten()
}

fn is_terminal(status: WorkerStatus) -> bool {
    matches!(
        status,
        WorkerStatus::Succeeded | WorkerStatus::Failed | WorkerStatus::Stopped
    )
}
