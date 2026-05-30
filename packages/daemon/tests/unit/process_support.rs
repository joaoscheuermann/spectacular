use std::fs;
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use lifecycle::identity::WorkerId;

use crate::process::{
    ChildExit, ChildHandle, ChildMonitor, ProcessLaunch, ProcessSpawner, WorkerProcessCommand,
};
use crate::registry::{Registry, WorkerMode};
use crate::root::prepare_worker_layout;
use crate::worker_session::SessionManager;

#[derive(Clone, Default)]
pub struct SharedRecordingSpawner {
    commands: Arc<Mutex<Vec<WorkerProcessCommand>>>,
}

impl SharedRecordingSpawner {
    pub fn commands(&self) -> Vec<WorkerProcessCommand> {
        self.commands.lock().unwrap().clone()
    }
}

impl ProcessSpawner for SharedRecordingSpawner {
    fn spawn(&self, command: WorkerProcessCommand) -> crate::error::DaemonResult<ChildHandle> {
        self.commands.lock().unwrap().push(command);
        Ok(ChildHandle::new(43))
    }
}

pub struct FailingSpawner;

impl ProcessSpawner for FailingSpawner {
    fn spawn(&self, _command: WorkerProcessCommand) -> crate::error::DaemonResult<ChildHandle> {
        Err(crate::error::DaemonError::root_configuration(
            None,
            "spawn failed for https://secret@example.com/org/private.git",
            None,
        ))
    }
}

#[derive(Clone)]
pub struct StaticExitSpawner {
    exit: ChildExit,
}

impl StaticExitSpawner {
    pub fn new(exit: ChildExit) -> Self {
        Self { exit }
    }
}

impl ProcessSpawner for StaticExitSpawner {
    fn spawn(&self, _command: WorkerProcessCommand) -> crate::error::DaemonResult<ChildHandle> {
        Ok(ChildHandle::with_exit_for_test(44, self.exit.clone()))
    }
}

#[derive(Clone)]
pub struct NoopChildMonitor;

impl ChildMonitor for NoopChildMonitor {
    fn monitor(&self, _worker_id: String, _child: ChildHandle, _manager: SessionManager) {}
}

#[derive(Clone)]
pub struct ImmediateChildMonitor;

impl ChildMonitor for ImmediateChildMonitor {
    fn monitor(&self, worker_id: String, child: ChildHandle, manager: SessionManager) {
        manager.record_child_exit(&worker_id, child.wait()).unwrap();
    }
}

#[derive(Clone)]
pub struct SuccessBeforeExitMonitor;

impl ChildMonitor for SuccessBeforeExitMonitor {
    fn monitor(&self, worker_id: String, child: ChildHandle, manager: SessionManager) {
        manager
            .record_worker_frame(lifecycle::proto::doric::lifecycle::v1::WorkerFrame {
                frame: Some(
                    lifecycle::proto::doric::lifecycle::v1::worker_frame::Frame::Status(
                        lifecycle::proto::doric::lifecycle::v1::WorkerStatusUpdate {
                            worker_id: worker_id.clone(),
                            status: lifecycle::proto::doric::lifecycle::v1::WorkerStatus::Succeeded
                                as i32,
                            message: "prompt artifact written".to_owned(),
                        },
                    ),
                ),
            })
            .unwrap();
        manager.record_child_exit(&worker_id, child.wait()).unwrap();
    }
}

#[derive(Default)]
pub struct RecordingSpawner {
    commands: Mutex<Vec<WorkerProcessCommand>>,
}

impl RecordingSpawner {
    pub fn commands(&self) -> Vec<WorkerProcessCommand> {
        self.commands.lock().unwrap().clone()
    }
}

impl ProcessSpawner for RecordingSpawner {
    fn spawn(&self, command: WorkerProcessCommand) -> crate::error::DaemonResult<ChildHandle> {
        self.commands.lock().unwrap().push(command);
        Ok(ChildHandle::new(42))
    }
}

pub struct ProcessFixture {
    pub root: PathBuf,
}

impl ProcessFixture {
    pub fn new(name: &str) -> Self {
        let root = std::env::temp_dir().join(format!("doric-daemon-process-{name}-{}", suffix()));
        fs::create_dir_all(&root).unwrap();
        Self { root }
    }

    pub fn current_exe(&self) -> PathBuf {
        self.root.join(executable_name("doric-daemon"))
    }

    pub fn touch_executable(&self, name: &str) -> PathBuf {
        let path = self.root.join(executable_name(name));
        fs::write(&path, "").unwrap();
        path
    }

    pub fn launch(&self, worker: &str, token: &str) -> ProcessLaunch {
        let worker_id = WorkerId::from_str(worker).unwrap();
        let layout = prepare_worker_layout(&self.root, worker_id).unwrap();

        ProcessLaunch {
            worker_id: worker.to_owned(),
            mode: WorkerMode::Feature,
            daemon_addr: "127.0.0.1:47821".to_owned(),
            token: token.to_owned(),
            prompt: "write private requirements".to_owned(),
            repo: "https://secret@example.com/org/private.git".to_owned(),
            repo_identity: "https://example.com/org/private.git".to_owned(),
            layout,
        }
    }
}

impl Drop for ProcessFixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

pub fn worker_binary_name() -> &'static str {
    "doric-worker"
}

fn executable_name(name: &str) -> String {
    if cfg!(windows) {
        format!("{name}.exe")
    } else {
        name.to_owned()
    }
}

fn suffix() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos()
}

pub fn start_job(
    frame: lifecycle::proto::doric::lifecycle::v1::DaemonFrame,
) -> lifecycle::proto::doric::lifecycle::v1::StartJob {
    match frame.frame.unwrap() {
        lifecycle::proto::doric::lifecycle::v1::daemon_frame::Frame::StartJob(start) => start,
        other => panic!("expected start job, got {other:?}"),
    }
}

pub fn summary(registry: &Arc<Mutex<Registry>>, worker: &str) -> crate::registry::WorkerSummary {
    registry
        .lock()
        .unwrap()
        .list()
        .into_iter()
        .find(|summary| summary.id().as_str() == worker)
        .unwrap()
}
