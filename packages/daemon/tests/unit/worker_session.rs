use std::fs;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use lifecycle::identity::{RequestId, WorkerId};
use lifecycle::proto::doric::lifecycle::v1 as pb;
use lifecycle::repo::RepoIdentity;
use lifecycle::status::WorkerStatus;

use crate::process::ChildExit;
use crate::registry::{InputRequest, Registry, WorkerMode, WorkerRecord};
use crate::root::prepare_worker_layout;
use crate::service::{CommandSender, LifecycleLogger};
use crate::worker_session::{
    AttachDecision, AttachRequest, SessionCommandSender, SessionManager, SessionWorker,
    WorkerSessionConfig,
};

/// Verifies a valid worker attach claims the token once and returns StartJob.
#[test]
fn attach_worker_valid_hello_claims_token_once_and_sends_start_job() {
    let fixture = SessionFixture::new();
    let manager = fixture.manager();
    fixture.register_worker("worker-attach", "token-1");

    let decision = manager
        .attach(AttachRequest::hello("worker-attach", "token-1"))
        .unwrap();

    assert!(matches!(decision, AttachDecision::Attached { .. }));
    assert_eq!(fixture.summary("worker-attach").status(), WorkerStatus::Running);
    assert!(fixture.worker("worker-attach").token_was_claimed());

    let frame = decision.into_first_daemon_frame().unwrap();
    let start = start_job(frame);
    assert_eq!(start.worker_id, "worker-attach");
    assert_eq!(start.prompt, "write requirements");
    assert_eq!(start.repo, "https://secret@example.com/org/repo.git");
}

/// Verifies representative session lifecycle events emit injected logger messages.
#[test]
fn session_manager_worker_lifecycle_events_log_representative_messages() {
    let fixture = SessionFixture::new();
    let manager = fixture.manager();
    fixture.register_worker("worker-logs", "token-1");

    manager
        .attach(AttachRequest::hello("worker-logs", "token-1"))
        .unwrap();
    manager
        .record_worker_frame(worker_event(
            "worker-logs",
            "repo_preparation",
            pb::WorkerStatus::Running,
            "cloning repo: https://example.com/org/repo.git",
        ))
        .unwrap();
    manager
        .record_worker_frame(input_event("worker-logs", "request-1", "Approve?"))
        .unwrap();
    manager
        .record_worker_frame(worker_status(
            "worker-logs",
            pb::WorkerStatus::Succeeded,
            "prompt artifact written",
        ))
        .unwrap();

    let logs = fixture.logs();
    assert_contains_log(&logs, "worker attached: worker-logs");
    assert_contains_log(&logs, "repo_preparation: worker-logs");
    assert_contains_log(&logs, "input requested: worker-logs request-1");
    assert_contains_log(&logs, "worker succeeded: worker-logs");
}

/// Verifies wrong-token and duplicate attaches do not receive StartJob.
#[test]
fn attach_worker_wrong_token_or_duplicate_attach_rejects_without_start_job() {
    let fixture = SessionFixture::new();
    let manager = fixture.manager();
    fixture.register_worker("worker-token", "token-1");

    let wrong = manager.attach(AttachRequest::hello("worker-token", "wrong-token"));
    assert!(wrong.unwrap_err().to_string().contains("token"));
    assert!(!fixture.worker("worker-token").token_was_claimed());

    manager
        .attach(AttachRequest::hello("worker-token", "token-1"))
        .unwrap();
    let duplicate = manager.attach(AttachRequest::hello("worker-token", "token-1"));

    assert!(duplicate.unwrap_err().to_string().contains("already attached"));
}

/// Verifies unknown and terminal workers cannot attach.
#[test]
fn attach_worker_unknown_or_terminal_worker_rejects_without_start_job() {
    let fixture = SessionFixture::new();
    let manager = fixture.manager();
    fixture.register_terminal_worker("terminal-worker", "token-1");

    let unknown = manager.attach(AttachRequest::hello("missing-worker", "token-1"));
    let terminal = manager.attach(AttachRequest::hello("terminal-worker", "token-1"));

    assert!(unknown.unwrap_err().to_string().contains("unknown"));
    assert!(terminal.unwrap_err().to_string().contains("terminal"));
}

/// Verifies missed attach deadlines fail the worker with a redacted reason.
#[test]
fn attach_deadline_without_hello_marks_worker_failed_and_redacts_reason() {
    let fixture = SessionFixture::new();
    let manager = fixture.manager();
    fixture.register_worker("worker-timeout", "deadline-token");

    manager
        .expire_attach_deadlines(Duration::from_secs(31))
        .unwrap();

    let summary = fixture.summary("worker-timeout");
    assert_eq!(summary.status(), WorkerStatus::Failed);
    assert!(summary.terminal_reason().unwrap().contains("attach deadline"));
    assert!(!summary.terminal_reason().unwrap().contains("deadline-token"));
}

/// Verifies a successful attach cancels or ignores the deadline.
#[test]
fn attach_deadline_after_successful_attach_does_not_fail_worker() {
    let fixture = SessionFixture::new();
    let manager = fixture.manager();
    fixture.register_worker("worker-attached-timeout", "token-1");

    manager
        .attach(AttachRequest::hello("worker-attached-timeout", "token-1"))
        .unwrap();
    manager
        .expire_attach_deadlines(Duration::from_secs(31))
        .unwrap();

    assert_eq!(
        fixture.summary("worker-attached-timeout").status(),
        WorkerStatus::Running
    );
}

/// Verifies worker terminal status frames update registry and close the session.
#[test]
fn record_worker_status_terminal_succeeded_marks_succeeded_and_closes_session() {
    let fixture = SessionFixture::new();
    let manager = fixture.manager();
    fixture.register_worker("worker-success", "token-1");
    manager
        .attach(AttachRequest::hello("worker-success", "token-1"))
        .unwrap();

    manager
        .record_worker_frame(worker_status(
            "worker-success",
            pb::WorkerStatus::Succeeded,
            "prompt artifact written",
        ))
        .unwrap();

    assert_eq!(fixture.summary("worker-success").status(), WorkerStatus::Succeeded);
    assert!(!manager.is_attached("worker-success"));
    assert_contains_log(&fixture.logs(), "worker succeeded: worker-success");
}

/// Verifies representative process failures emit injected logger messages.
#[test]
fn session_manager_process_failures_log_spawn_failure_and_child_exit() {
    let fixture = SessionFixture::new();
    let manager = fixture.manager();
    fixture.register_worker("worker-spawn-failure", "token-1");
    fixture.register_worker("worker-child-exit", "token-2");

    manager
        .record_spawn_failure("worker-spawn-failure", "credential=super-secret")
        .unwrap();
    manager
        .record_child_exit("worker-child-exit", ChildExit::exited(7))
        .unwrap();

    let logs = fixture.logs();
    assert_contains_log(&logs, "spawn failed: worker-spawn-failure");
    assert_contains_log(&logs, "child exit: worker-child-exit");
}

/// Verifies daemon-validated answers are forwarded only through attached sessions.
#[test]
fn send_answer_attached_or_detached_worker_forwards_or_preserves_pending_input() {
    let fixture = SessionFixture::new();
    let manager = fixture.manager();
    fixture.register_worker("worker-answer", "token-1");
    fixture.mark_waiting("worker-answer", "request-1");
    manager
        .attach(AttachRequest::hello("worker-answer", "token-1"))
        .unwrap();

    let sender = SessionCommandSender::new(manager.clone());
    sender
        .send_answer(crate::service::AnswerCommand {
            worker_id: "worker-answer".to_owned(),
            request_id: "request-1".to_owned(),
            text: "yes".to_owned(),
        })
        .unwrap();

    let command = manager.sent_commands("worker-answer").remove(0);
    let answer = answer_command(command);
    assert_eq!(answer.request_id, "request-1");
    assert_eq!(answer.text, "yes");

    fixture.register_worker("worker-detached", "token-2");
    fixture.mark_waiting("worker-detached", "request-2");
    let detached = sender.send_answer(crate::service::AnswerCommand {
        worker_id: "worker-detached".to_owned(),
        request_id: "request-2".to_owned(),
        text: "later".to_owned(),
    });

    assert!(detached.unwrap_err().to_string().contains("session unavailable"));
    assert_eq!(
        fixture.summary("worker-detached").status(),
        WorkerStatus::WaitingForInput
    );
}

struct SessionFixture {
    registry: Arc<Mutex<Registry>>,
    manager: SessionManager,
    logs: Arc<Mutex<Vec<String>>>,
    root: TempRoot,
}

impl SessionFixture {
    fn new() -> Self {
        let registry = Arc::new(Mutex::new(Registry::in_memory()));
        let logs = Arc::new(Mutex::new(Vec::new()));
        let manager = SessionManager::new(WorkerSessionConfig {
            registry: registry.clone(),
            attach_deadline: Duration::from_secs(30),
            lifecycle_logger: RecordingLifecycleLogger(logs.clone()),
        });

        Self {
            registry,
            manager,
            logs,
            root: TempRoot::new("worker-session"),
        }
    }

    fn manager(&self) -> SessionManager {
        self.manager.clone()
    }

    fn register_worker(&self, id: &str, token: &str) {
        let worker_id = worker_id(id);
        let layout = prepare_worker_layout(self.root.path(), worker_id.clone()).unwrap();
        self.registry
            .lock()
            .unwrap()
            .insert(WorkerRecord::new(
                worker_id,
                WorkerMode::Feature,
                RepoIdentity::from_raw_url("https://example.com/org/repo.git").unwrap(),
                WorkerStatus::Starting,
                "starting",
            ))
            .unwrap();

        self.manager
            .register_session_worker(SessionWorker {
                worker_id: id.to_owned(),
                mode: WorkerMode::Feature,
                token: token.to_owned(),
                prompt: "write requirements".to_owned(),
                repo: "https://secret@example.com/org/repo.git".to_owned(),
                layout,
            })
            .unwrap();
    }

    fn register_terminal_worker(&self, id: &str, token: &str) {
        self.register_worker(id, token);
        self.registry
            .lock()
            .unwrap()
            .update_status(&worker_id(id), WorkerStatus::Failed, "process exited")
            .unwrap();
    }

    fn mark_waiting(&self, id: &str, request: &str) {
        self.registry
            .lock()
            .unwrap()
            .request_input(InputRequest::new(
                worker_id(id),
                RequestId::from_str(request).unwrap(),
                "Approve?",
            ))
            .unwrap();
    }

    fn summary(&self, id: &str) -> crate::registry::WorkerSummary {
        self.registry
            .lock()
            .unwrap()
            .list()
            .into_iter()
            .find(|summary| summary.id().as_str() == id)
            .unwrap()
    }

    fn worker(&self, id: &str) -> SessionWorker {
        self.manager.worker_for_test(id).unwrap()
    }

    fn logs(&self) -> Vec<String> {
        self.logs.lock().unwrap().clone()
    }
}

#[derive(Clone)]
struct RecordingLifecycleLogger(Arc<Mutex<Vec<String>>>);

impl LifecycleLogger for RecordingLifecycleLogger {
    fn log(&self, message: &str) {
        self.0.lock().unwrap().push(message.to_owned());
    }
}

struct TempRoot {
    path: PathBuf,
}

impl TempRoot {
    fn new(name: &str) -> Self {
        let path = std::env::temp_dir().join(format!("doric-daemon-{name}-{}", suffix()));
        fs::create_dir_all(&path).unwrap();
        Self { path }
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TempRoot {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

fn start_job(frame: pb::DaemonFrame) -> pb::StartJob {
    match frame.frame.unwrap() {
        pb::daemon_frame::Frame::StartJob(start) => start,
        other => panic!("expected start job, got {other:?}"),
    }
}

fn answer_command(frame: pb::DaemonFrame) -> pb::AnswerInputCommand {
    match frame.frame.unwrap() {
        pb::daemon_frame::Frame::Answer(answer) => answer,
        other => panic!("expected answer command, got {other:?}"),
    }
}

fn worker_status(worker: &str, status: pb::WorkerStatus, message: &str) -> pb::WorkerFrame {
    pb::WorkerFrame {
        frame: Some(pb::worker_frame::Frame::Status(pb::WorkerStatusUpdate {
            worker_id: worker.to_owned(),
            status: status as i32,
            message: message.to_owned(),
        })),
    }
}

fn worker_event(
    worker: &str,
    name: &str,
    status: pb::WorkerStatus,
    message: &str,
) -> pb::WorkerFrame {
    pb::WorkerFrame {
        frame: Some(pb::worker_frame::Frame::Event(pb::WorkerEvent {
            worker_id: worker.to_owned(),
            sequence: 0,
            status: status as i32,
            name: name.to_owned(),
            message: message.to_owned(),
            input: None,
            occurred_at: None,
        })),
    }
}

fn input_event(worker: &str, request: &str, prompt: &str) -> pb::WorkerFrame {
    pb::WorkerFrame {
        frame: Some(pb::worker_frame::Frame::Event(pb::WorkerEvent {
            worker_id: worker.to_owned(),
            sequence: 0,
            status: pb::WorkerStatus::WaitingForInput as i32,
            name: "waiting_for_input".to_owned(),
            message: prompt.to_owned(),
            input: Some(pb::InputRequest {
                request_id: request.to_owned(),
                prompt: prompt.to_owned(),
                choices: Vec::new(),
            }),
            occurred_at: None,
        })),
    }
}

fn assert_contains_log(logs: &[String], expected: &str) {
    assert!(
        logs.iter().any(|log| log.contains(expected)),
        "expected log containing `{expected}`, got {logs:?}"
    );
}

fn worker_id(value: &str) -> WorkerId {
    WorkerId::from_str(value).unwrap()
}

fn suffix() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos()
}
