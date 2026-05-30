use std::collections::HashMap;
use std::str::FromStr;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use lifecycle::identity::{RequestId, WorkerId};
use lifecycle::proto::doric::lifecycle::v1 as pb;
use lifecycle::status::WorkerStatus;

use crate::error::{DaemonError, DaemonResult};
use crate::event::RegistryEvent;
use crate::process::ChildExit;
use crate::registry::{Registry, WorkerMode};
use crate::root::WorkerLayout;
use crate::service::{AnswerCommand, CommandSender};

#[derive(Clone)]
pub struct WorkerSessionConfig {
    pub registry: Arc<Mutex<Registry>>,
    pub attach_deadline: Duration,
}

#[derive(Clone)]
pub struct SessionManager {
    registry: Arc<Mutex<Registry>>,
    attach_deadline: Duration,
    state: Arc<Mutex<SessionState>>,
}

impl SessionManager {
    pub fn new(config: WorkerSessionConfig) -> Self {
        Self {
            registry: config.registry,
            attach_deadline: config.attach_deadline,
            state: Arc::new(Mutex::new(SessionState::default())),
        }
    }

    pub fn register_session_worker(&self, worker: SessionWorker) -> DaemonResult<()> {
        let mut state = self.state.lock().expect("session state lock poisoned");

        if state.workers.contains_key(&worker.worker_id) {
            return Err(session_error(format!(
                "worker `{}` session is already registered",
                worker.worker_id
            )));
        }

        state.workers.insert(
            worker.worker_id.clone(),
            SessionEntry {
                worker,
                attached: false,
                token_claimed: false,
                commands: Vec::new(),
            },
        );

        Ok(())
    }

    pub fn attach(&self, request: AttachRequest) -> DaemonResult<AttachDecision> {
        let worker_id = parse_worker_id(&request.worker_id)?;
        let status = self.worker_status(&worker_id)?;

        if is_terminal(status) {
            return Err(DaemonError::TerminalWorker { worker_id });
        }

        let start_job = {
            let mut state = self.state.lock().expect("session state lock poisoned");
            let entry = state.workers.get_mut(&request.worker_id).ok_or_else(|| {
                DaemonError::UnknownWorker {
                    worker_id: worker_id.clone(),
                }
            })?;

            if entry.attached {
                return Err(session_error(format!(
                    "worker `{}` is already attached",
                    request.worker_id
                )));
            }

            if entry.worker.token != request.token {
                return Err(session_error(format!(
                    "worker `{}` attach token was rejected",
                    request.worker_id
                )));
            }

            entry.attached = true;
            entry.token_claimed = true;
            start_job_frame(&entry.worker)
        };

        {
            let mut registry = self.registry.lock().expect("registry lock poisoned");
            registry.update_status(&worker_id, WorkerStatus::Running, "worker attached")?;
            registry.append_event(
                &worker_id,
                RegistryEvent::current_activity("worker attached"),
            )?;
        }

        Ok(AttachDecision::Attached { start_job })
    }

    pub fn expire_attach_deadlines(&self, elapsed: Duration) -> DaemonResult<()> {
        if elapsed < self.attach_deadline {
            return Ok(());
        }

        let expired = {
            let state = self.state.lock().expect("session state lock poisoned");
            state
                .workers
                .values()
                .filter(|entry| !entry.attached && !entry.token_claimed)
                .map(|entry| entry.worker.worker_id.clone())
                .collect::<Vec<_>>()
        };

        for id in expired {
            let worker_id = parse_worker_id(&id)?;
            let status = self.worker_status(&worker_id)?;

            if is_terminal(status) || status == WorkerStatus::Running {
                continue;
            }

            let mut registry = self.registry.lock().expect("registry lock poisoned");
            registry.update_status(
                &worker_id,
                WorkerStatus::Failed,
                "attach deadline exceeded before worker session started",
            )?;
            registry.append_event(
                &worker_id,
                RegistryEvent::Failed(
                    "attach deadline exceeded before worker session started".to_owned(),
                ),
            )?;
        }

        Ok(())
    }

    pub fn record_spawn_failure(&self, worker: &str, _reason: impl AsRef<str>) -> DaemonResult<()> {
        let worker_id = parse_worker_id(worker)?;
        let reason = "spawn failed: worker process could not be started".to_owned();

        {
            let mut registry = self.registry.lock().expect("registry lock poisoned");
            registry.update_status(&worker_id, WorkerStatus::Failed, reason.clone())?;
            registry.append_event(&worker_id, RegistryEvent::Failed(reason))?;
        }

        self.close_session(worker);
        Ok(())
    }

    pub fn record_child_exit(&self, worker: &str, exit: ChildExit) -> DaemonResult<()> {
        let worker_id = parse_worker_id(worker)?;
        let prior_status = self.worker_status(&worker_id)?;

        if is_terminal(prior_status) {
            self.close_session(worker);
            return Ok(());
        }

        let mapped = exit.map_status(None);
        {
            let mut registry = self.registry.lock().expect("registry lock poisoned");
            registry.update_status(&worker_id, mapped.status(), mapped.reason().to_owned())?;
            registry.append_event(
                &worker_id,
                event_for_status(mapped.status(), mapped.reason().to_owned()),
            )?;
        }

        self.close_session(worker);
        Ok(())
    }

    pub fn record_worker_frame(&self, frame: pb::WorkerFrame) -> DaemonResult<()> {
        match frame.frame {
            Some(pb::worker_frame::Frame::Status(status)) => self.record_status(status),
            Some(pb::worker_frame::Frame::Event(event)) => self.record_event(event),
            Some(pb::worker_frame::Frame::Hello(hello)) => self
                .attach(AttachRequest {
                    worker_id: hello.worker_id,
                    token: hello.token,
                })
                .map(|_| ()),
            None => Err(session_error("worker frame was empty")),
        }
    }

    pub fn is_attached(&self, worker: &str) -> bool {
        self.state
            .lock()
            .expect("session state lock poisoned")
            .workers
            .get(worker)
            .is_some_and(|entry| entry.attached)
    }

    pub fn sent_commands(&self, worker: &str) -> Vec<pb::DaemonFrame> {
        self.state
            .lock()
            .expect("session state lock poisoned")
            .workers
            .get(worker)
            .map(|entry| entry.commands.clone())
            .unwrap_or_default()
    }

    pub fn worker_for_test(&self, worker: &str) -> Option<SessionWorker> {
        self.state
            .lock()
            .expect("session state lock poisoned")
            .workers
            .get(worker)
            .map(|entry| {
                let mut worker = entry.worker.clone();
                if entry.token_claimed {
                    worker.token.clear();
                }
                worker
            })
    }

    fn send_command(&self, worker: &str, frame: pb::DaemonFrame) -> DaemonResult<()> {
        let mut state = self.state.lock().expect("session state lock poisoned");
        let entry = state
            .workers
            .get_mut(worker)
            .ok_or_else(|| session_error(format!("worker `{worker}` session unavailable")))?;

        if !entry.attached {
            return Err(session_error(format!(
                "worker `{worker}` session unavailable"
            )));
        }

        entry.commands.push(frame);
        Ok(())
    }

    fn record_status(&self, update: pb::WorkerStatusUpdate) -> DaemonResult<()> {
        let worker_id = parse_worker_id(&update.worker_id)?;
        let status = proto_status(update.status)?;
        let message = if update.message.trim().is_empty() {
            status.to_string()
        } else {
            update.message
        };

        {
            let mut registry = self.registry.lock().expect("registry lock poisoned");
            registry.update_status(&worker_id, status, message.clone())?;
            registry.append_event(&worker_id, event_for_status(status, message))?;
        }

        if is_terminal(status) {
            self.close_session(&update.worker_id);
        }

        Ok(())
    }

    fn record_event(&self, event: pb::WorkerEvent) -> DaemonResult<()> {
        let worker_id = parse_worker_id(&event.worker_id)?;

        if let Some(input) = event.input {
            let request_id = RequestId::from_str(&input.request_id)
                .map_err(|error| session_error(error.to_string()))?;
            return self
                .registry
                .lock()
                .expect("registry lock poisoned")
                .request_input(crate::registry::InputRequest::new(
                    worker_id,
                    request_id,
                    input.prompt,
                ));
        }

        let status = proto_status(event.status).unwrap_or(WorkerStatus::Running);
        let name = event.name;
        let message = if event.message.trim().is_empty() {
            name.clone()
        } else {
            event.message
        };
        self.registry
            .lock()
            .expect("registry lock poisoned")
            .append_event(&worker_id, event_for_worker_event(&name, status, message))?;

        Ok(())
    }

    fn close_session(&self, worker: &str) {
        if let Some(entry) = self
            .state
            .lock()
            .expect("session state lock poisoned")
            .workers
            .get_mut(worker)
        {
            entry.attached = false;
        }
    }

    fn worker_status(&self, worker_id: &WorkerId) -> DaemonResult<WorkerStatus> {
        self.registry
            .lock()
            .expect("registry lock poisoned")
            .list()
            .into_iter()
            .find(|summary| summary.id() == worker_id)
            .map(|summary| summary.status())
            .ok_or_else(|| DaemonError::UnknownWorker {
                worker_id: worker_id.clone(),
            })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SessionWorker {
    pub worker_id: String,
    pub mode: WorkerMode,
    pub token: String,
    pub prompt: String,
    pub repo: String,
    pub layout: WorkerLayout,
}

impl SessionWorker {
    pub fn token_was_claimed(&self) -> bool {
        self.token.is_empty()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttachRequest {
    worker_id: String,
    token: String,
}

impl AttachRequest {
    pub fn hello(worker_id: impl Into<String>, token: impl Into<String>) -> Self {
        Self {
            worker_id: worker_id.into(),
            token: token.into(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AttachDecision {
    Attached { start_job: pb::DaemonFrame },
}

impl AttachDecision {
    pub fn into_first_daemon_frame(self) -> Option<pb::DaemonFrame> {
        match self {
            Self::Attached { start_job } => Some(start_job),
        }
    }
}

#[derive(Clone)]
pub struct SessionCommandSender {
    manager: SessionManager,
}

impl SessionCommandSender {
    pub fn new(manager: SessionManager) -> Self {
        Self { manager }
    }
}

impl CommandSender for SessionCommandSender {
    fn send_answer(&self, command: AnswerCommand) -> DaemonResult<()> {
        self.manager.send_command(
            &command.worker_id,
            pb::DaemonFrame {
                frame: Some(pb::daemon_frame::Frame::Answer(pb::AnswerInputCommand {
                    request_id: command.request_id,
                    text: command.text,
                })),
            },
        )
    }
}

#[derive(Default)]
struct SessionState {
    workers: HashMap<String, SessionEntry>,
}

struct SessionEntry {
    worker: SessionWorker,
    attached: bool,
    token_claimed: bool,
    commands: Vec<pb::DaemonFrame>,
}

fn start_job_frame(worker: &SessionWorker) -> pb::DaemonFrame {
    pb::DaemonFrame {
        frame: Some(pb::daemon_frame::Frame::StartJob(pb::StartJob {
            worker_id: worker.worker_id.clone(),
            mode: mode_to_proto(worker.mode),
            prompt: worker.prompt.clone(),
            repo: worker.repo.clone(),
            repo_dir: worker.layout.repo().display().to_string(),
            state_dir: worker.layout.state().display().to_string(),
            artifacts_dir: worker.layout.artifacts().display().to_string(),
            tool_output_dir: worker.layout.tool_output().display().to_string(),
        })),
    }
}

fn event_for_status(status: WorkerStatus, message: String) -> RegistryEvent {
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

fn event_for_worker_event(name: &str, status: WorkerStatus, message: String) -> RegistryEvent {
    match name {
        "repo_preparation" => RegistryEvent::repo_preparation(message),
        "prompt_agent_started" => RegistryEvent::prompt_agent_started(message),
        "prompt_artifact_written" => RegistryEvent::prompt_artifact_written(message),
        "prompt_agent_completed" => RegistryEvent::prompt_agent_completed(message),
        _ => event_for_status(status, message),
    }
}

fn mode_to_proto(mode: WorkerMode) -> i32 {
    match mode {
        WorkerMode::Feature => pb::JobMode::Feature as i32,
        WorkerMode::Debug => pb::JobMode::Debug as i32,
    }
}

fn proto_status(status: i32) -> DaemonResult<WorkerStatus> {
    let status = pb::WorkerStatus::try_from(status)
        .map_err(|_| session_error("worker status is unknown"))?;

    WorkerStatus::try_from(status).map_err(|error| session_error(error.to_string()))
}

fn parse_worker_id(value: &str) -> DaemonResult<WorkerId> {
    WorkerId::from_str(value).map_err(|error| session_error(error.to_string()))
}

fn is_terminal(status: WorkerStatus) -> bool {
    matches!(
        status,
        WorkerStatus::Succeeded | WorkerStatus::Failed | WorkerStatus::Stopped
    )
}

fn session_error(message: impl Into<String>) -> DaemonError {
    DaemonError::root_configuration(None, message, None)
}
