use std::path::PathBuf;

use lifecycle::event::WorkerEvent;
use lifecycle::identity::RequestId;
use lifecycle::proto::doric::lifecycle::v1 as pb;
use lifecycle::status::WorkerStatus;
use lifecycle::terminal::safe_message;

use crate::agents::prompt::PromptAgentEvent;
use crate::error::{WorkerError, WorkerResult};
use crate::event::{
    prompt_agent_event_to_worker_event, sanitize_event_text, sanitize_prompt_agent_event,
    worker_event_to_proto, worker_status_to_proto,
};
pub use crate::state::{
    PreparedRuntimeRepo, RuntimeAnswer, RuntimeLayout, RuntimePromptJob, RuntimeRepoRequest,
};

use crate::state::WorkerId;

/// Runtime configuration supplied by the worker process composition root.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeConfig {
    worker_id: WorkerId,
    token: String,
    worker_root: PathBuf,
}

impl RuntimeConfig {
    pub fn new(
        worker_id: WorkerId,
        token: impl Into<String>,
        worker_root: impl Into<PathBuf>,
    ) -> Self {
        Self {
            worker_id,
            token: token.into(),
            worker_root: worker_root.into(),
        }
    }
}

/// Runtime dependencies kept explicit so unit paths remain offline and fakeable.
pub struct RuntimeDeps<'a> {
    session: &'a mut dyn DaemonSession,
    repo_preparer: &'a mut dyn RepoPreparer,
    prompt_runner: &'a mut dyn RuntimePromptRunner,
}

impl<'a> RuntimeDeps<'a> {
    pub fn new(
        session: &'a mut dyn DaemonSession,
        repo_preparer: &'a mut dyn RepoPreparer,
        prompt_runner: &'a mut dyn RuntimePromptRunner,
    ) -> Self {
        Self {
            session,
            repo_preparer,
            prompt_runner,
        }
    }
}

/// Bidirectional worker session boundary to the daemon transport.
pub trait DaemonSession {
    fn send(&mut self, frame: pb::WorkerFrame) -> WorkerResult<()>;
    fn receive(&mut self) -> WorkerResult<Option<pb::DaemonFrame>>;
}

/// Repo preparation boundary above Git and filesystem clone behavior.
pub trait RepoPreparer {
    fn prepare(&mut self, request: RuntimeRepoRequest) -> WorkerResult<PreparedRuntimeRepo>;
}

/// Session-oriented prompt runner that can pause, resume, or stop.
pub trait RuntimePromptRunner {
    fn start(&mut self, job: RuntimePromptJob) -> WorkerResult<PromptRunState>;
    fn resume(&mut self, answer: RuntimeAnswer) -> WorkerResult<PromptRunState>;
    fn stop(&mut self, reason: &str) -> WorkerResult<PromptRunState>;
}

#[derive(Debug)]
pub enum PromptRunState {
    Running { events: Vec<PromptAgentEvent> },
    WaitingForInput { events: Vec<PromptAgentEvent> },
    Succeeded { events: Vec<PromptAgentEvent> },
    Failed { error: WorkerError },
    Stopped { events: Vec<PromptAgentEvent> },
}

impl PromptRunState {
    pub fn running(events: impl IntoIterator<Item = PromptAgentEvent>) -> Self {
        Self::Running {
            events: events.into_iter().collect(),
        }
    }

    pub fn waiting_for_input(events: impl IntoIterator<Item = PromptAgentEvent>) -> Self {
        Self::WaitingForInput {
            events: events.into_iter().collect(),
        }
    }

    pub fn succeeded(events: impl IntoIterator<Item = PromptAgentEvent>) -> Self {
        Self::Succeeded {
            events: events.into_iter().collect(),
        }
    }

    pub fn failed(error: WorkerError) -> Self {
        Self::Failed { error }
    }

    pub fn stopped(events: impl IntoIterator<Item = PromptAgentEvent>) -> Self {
        Self::Stopped {
            events: events.into_iter().collect(),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeOutcome {
    WaitingForStartJob,
    Running,
    WaitingForInput,
    Succeeded,
    Failed,
    Stopped,
}

/// Runs one daemon-controlled worker session without binding sockets or reading config.
pub fn run_worker_session(
    config: RuntimeConfig,
    mut deps: RuntimeDeps<'_>,
) -> WorkerResult<RuntimeOutcome> {
    let mut runtime = Runtime::new(config, &mut deps);
    runtime.run()
}

struct Runtime<'a, 'b> {
    config: RuntimeConfig,
    deps: &'b mut RuntimeDeps<'a>,
    next_sequence: u64,
    terminal_sent: bool,
}

impl<'a, 'b> Runtime<'a, 'b> {
    fn new(config: RuntimeConfig, deps: &'b mut RuntimeDeps<'a>) -> Self {
        Self {
            config,
            deps,
            next_sequence: 0,
            terminal_sent: false,
        }
    }

    fn run(&mut self) -> WorkerResult<RuntimeOutcome> {
        self.send_hello()?;

        let start = match self.wait_for_start_job()? {
            StartJobWait::Start(start) => start,
            StartJobWait::Shutdown(reason) => return self.stop(&reason),
            StartJobWait::Pending => return Ok(RuntimeOutcome::WaitingForStartJob),
        };

        self.send_status(WorkerStatus::Running, "Worker session running")?;
        let sequence = self.sequence();
        self.send_event(WorkerEvent::repo_preparation(
            self.config.worker_id.clone(),
            sequence,
            clone_event_message(&start.repo),
        ))?;

        let repo = match self.deps.repo_preparer.prepare(self.repo_request(&start)) {
            Ok(repo) => repo,
            Err(error) => return self.fail(error),
        };

        let job = RuntimePromptJob::new(
            self.config.worker_id.clone(),
            repo.layout().clone(),
            start.prompt,
        );
        let state = match self.deps.prompt_runner.start(job) {
            Ok(state) => state,
            Err(error) => return self.prompt_fail(error),
        };

        self.process_prompt_state(state)
    }

    fn wait_for_start_job(&mut self) -> WorkerResult<StartJobWait> {
        while let Some(frame) = self.deps.session.receive()? {
            match frame.frame {
                Some(pb::daemon_frame::Frame::StartJob(start)) => {
                    return Ok(StartJobWait::Start(start));
                }
                Some(pb::daemon_frame::Frame::Shutdown(shutdown)) => {
                    return Ok(StartJobWait::Shutdown(shutdown.reason));
                }
                Some(pb::daemon_frame::Frame::Answer(_)) | None => {}
            }
        }

        Ok(StartJobWait::Pending)
    }

    fn process_prompt_state(&mut self, state: PromptRunState) -> WorkerResult<RuntimeOutcome> {
        match state {
            PromptRunState::Running { events } => {
                self.send_prompt_events(events)?;
                self.wait_for_runtime_command(None)
            }
            PromptRunState::WaitingForInput { events } => {
                let pending = pending_request(&events);
                self.send_prompt_events(events)?;
                self.send_status(WorkerStatus::WaitingForInput, "Waiting for input")?;
                self.wait_for_runtime_command(pending)
            }
            PromptRunState::Succeeded { events } => {
                self.send_prompt_events(events)?;
                self.succeed()
            }
            PromptRunState::Failed { error } => self.prompt_fail(error),
            PromptRunState::Stopped { .. } => self.stop("worker stopped"),
        }
    }

    fn wait_for_runtime_command(
        &mut self,
        pending: Option<RequestId>,
    ) -> WorkerResult<RuntimeOutcome> {
        while let Some(frame) = self.deps.session.receive()? {
            match frame.frame {
                Some(pb::daemon_frame::Frame::Answer(answer)) => {
                    if pending
                        .as_ref()
                        .is_some_and(|id| id.as_str() == answer.request_id)
                    {
                        let request_id = pending.expect("checked pending request");
                        let state = self
                            .deps
                            .prompt_runner
                            .resume(RuntimeAnswer::new(request_id, answer.text))?;
                        return self.process_prompt_state(state);
                    }
                }
                Some(pb::daemon_frame::Frame::Shutdown(shutdown)) => {
                    let state = self.deps.prompt_runner.stop(&shutdown.reason)?;
                    self.discard_prompt_state(state);
                    return self.stop(&shutdown.reason);
                }
                Some(pb::daemon_frame::Frame::StartJob(_)) | None => {}
            }
        }

        Ok(if pending.is_some() {
            RuntimeOutcome::WaitingForInput
        } else {
            RuntimeOutcome::Running
        })
    }

    fn repo_request(&self, start: &pb::StartJob) -> RuntimeRepoRequest {
        RuntimeRepoRequest::new(
            self.config.worker_id.clone(),
            start.repo.clone(),
            RuntimeLayout::new(
                self.config.worker_root.join(self.config.worker_id.as_str()),
                PathBuf::from(&start.repo_dir),
                PathBuf::from(&start.state_dir),
                PathBuf::from(&start.artifacts_dir),
                PathBuf::from(&start.tool_output_dir),
            ),
        )
    }

    fn send_hello(&mut self) -> WorkerResult<()> {
        self.deps.session.send(pb::WorkerFrame {
            frame: Some(pb::worker_frame::Frame::Hello(pb::WorkerHello {
                worker_id: self.config.worker_id.as_str().to_owned(),
                token: self.config.token.clone(),
            })),
        })
    }

    fn send_prompt_events(
        &mut self,
        events: impl IntoIterator<Item = PromptAgentEvent>,
    ) -> WorkerResult<()> {
        for event in events {
            let event = sanitize_prompt_event(event, &self.config.token);
            let worker_event = prompt_agent_event_to_worker_event(
                self.config.worker_id.clone(),
                self.sequence(),
                event,
            );
            self.send_event(worker_event)?;
        }
        Ok(())
    }

    fn send_event(&mut self, event: WorkerEvent) -> WorkerResult<()> {
        self.deps.session.send(pb::WorkerFrame {
            frame: Some(pb::worker_frame::Frame::Event(worker_event_to_proto(event))),
        })
    }

    fn send_status(&mut self, status: WorkerStatus, message: impl AsRef<str>) -> WorkerResult<()> {
        let message = sanitize_text(message.as_ref(), &self.config.token);
        self.deps.session.send(pb::WorkerFrame {
            frame: Some(pb::worker_frame::Frame::Status(pb::WorkerStatusUpdate {
                worker_id: self.config.worker_id.as_str().to_owned(),
                status: worker_status_to_proto(status) as i32,
                message,
            })),
        })
    }

    fn fail(&mut self, error: WorkerError) -> WorkerResult<RuntimeOutcome> {
        let message = sanitize_text(&error.lifecycle_failure_message(), &self.config.token);
        let sequence = self.sequence();
        self.send_event(WorkerEvent::failed(
            self.config.worker_id.clone(),
            sequence,
            message.clone(),
        ))?;
        self.send_terminal(WorkerStatus::Failed, message)?;
        Ok(RuntimeOutcome::Failed)
    }

    fn prompt_fail(&mut self, error: WorkerError) -> WorkerResult<RuntimeOutcome> {
        let message = sanitize_text(&error.lifecycle_failure_message(), &self.config.token);
        let sequence = self.sequence();
        self.send_event(WorkerEvent::prompt_agent_failed(
            self.config.worker_id.clone(),
            sequence,
            message.clone(),
        ))?;
        self.send_terminal(WorkerStatus::Failed, message)?;
        Ok(RuntimeOutcome::Failed)
    }

    fn succeed(&mut self) -> WorkerResult<RuntimeOutcome> {
        self.send_terminal(
            WorkerStatus::Succeeded,
            "Prompt requirements workflow completed",
        )?;
        Ok(RuntimeOutcome::Succeeded)
    }

    fn stop(&mut self, reason: &str) -> WorkerResult<RuntimeOutcome> {
        let message = sanitize_text(reason, &self.config.token);
        let sequence = self.sequence();
        self.send_event(WorkerEvent::stopped(
            self.config.worker_id.clone(),
            sequence,
            message.clone(),
        ))?;
        self.send_terminal(WorkerStatus::Stopped, message)?;
        Ok(RuntimeOutcome::Stopped)
    }

    fn send_terminal(
        &mut self,
        status: WorkerStatus,
        message: impl AsRef<str>,
    ) -> WorkerResult<()> {
        if self.terminal_sent {
            return Ok(());
        }

        self.terminal_sent = true;
        self.send_status(status, message)
    }

    fn sequence(&mut self) -> u64 {
        let sequence = self.next_sequence;
        self.next_sequence += 1;
        sequence
    }

    fn discard_prompt_state(&self, _state: PromptRunState) {}
}

enum StartJobWait {
    Start(pb::StartJob),
    Shutdown(String),
    Pending,
}

fn pending_request(events: &[PromptAgentEvent]) -> Option<RequestId> {
    events.iter().find_map(|event| match event {
        PromptAgentEvent::InputRequested { request_id, .. } => Some(request_id.clone()),
        _ => None,
    })
}

fn sanitize_prompt_event(event: PromptAgentEvent, token: &str) -> PromptAgentEvent {
    sanitize_prompt_agent_event(event, token)
}

fn sanitize_text(text: &str, token: &str) -> String {
    sanitize_event_text(text, token)
}

fn clone_event_message(repo: &str) -> String {
    format!("cloning repo: {}", safe_message(repo))
}
