use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Child, Command};
use std::sync::Arc;
use std::thread;
use std::time::{SystemTime, UNIX_EPOCH};

use lifecycle::redaction::redact_failure_text;
use lifecycle::status::WorkerStatus;

use crate::error::{DaemonError, DaemonResult};
use crate::registry::WorkerMode;
use crate::root::WorkerLayout;
use crate::service::{LaunchRequest, WorkerLauncher};
use crate::worker_session::{SessionManager, SessionWorker};

const WORKER_BINARY: &str = "doric-worker";

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct WorkerBinaryConfig {
    explicit: Option<PathBuf>,
}

impl WorkerBinaryConfig {
    pub fn explicit(path: PathBuf) -> Self {
        Self {
            explicit: Some(path),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkerBinary {
    path: PathBuf,
}

impl WorkerBinary {
    pub fn path(&self) -> &Path {
        &self.path
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkerBinaryResolver {
    current_exe: PathBuf,
}

impl WorkerBinaryResolver {
    pub fn new(current_exe: PathBuf) -> Self {
        Self { current_exe }
    }

    pub fn current_exe() -> DaemonResult<Self> {
        Ok(Self::new(std::env::current_exe()?))
    }

    pub fn resolve(&self, config: WorkerBinaryConfig) -> DaemonResult<WorkerBinary> {
        let path = config
            .explicit
            .unwrap_or_else(|| self.current_exe_sibling(worker_binary_name()));

        if fs::metadata(&path)
            .map(|metadata| metadata.is_file())
            .unwrap_or(false)
        {
            Ok(WorkerBinary { path })
        } else {
            Err(DaemonError::root_configuration(
                Some(path),
                "worker binary does not exist",
                None,
            ))
        }
    }

    fn current_exe_sibling(&self, name: &str) -> PathBuf {
        self.current_exe
            .parent()
            .map(|parent| parent.join(executable_name(name)))
            .unwrap_or_else(|| PathBuf::from(executable_name(name)))
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProcessLaunch {
    pub worker_id: String,
    pub mode: WorkerMode,
    pub daemon_addr: String,
    pub token: String,
    pub prompt: String,
    pub repo: String,
    pub repo_identity: String,
    pub layout: WorkerLayout,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkerProcessCommand {
    program: PathBuf,
    args: Vec<String>,
    worker_id: String,
}

impl WorkerProcessCommand {
    pub fn program(&self) -> &Path {
        &self.program
    }

    pub fn args(&self) -> Vec<String> {
        self.args.clone()
    }

    pub fn worker_id(&self) -> &str {
        &self.worker_id
    }

    pub fn display_without_secrets(&self) -> String {
        let mut parts = vec![self.program.display().to_string()];
        let mut redact_next = false;

        for arg in &self.args {
            if redact_next {
                parts.push("[REDACTED]".to_owned());
                redact_next = false;
                continue;
            }

            parts.push(arg.clone());
            redact_next = arg == "--token";
        }

        parts.join(" ")
    }
}

#[derive(Clone, Debug)]
pub struct WorkerCommandBuilder {
    binary: PathBuf,
}

impl WorkerCommandBuilder {
    pub fn new(binary: PathBuf) -> Self {
        Self { binary }
    }

    pub fn build(&self, launch: &ProcessLaunch) -> WorkerProcessCommand {
        WorkerProcessCommand {
            program: self.binary.clone(),
            worker_id: launch.worker_id.clone(),
            args: vec![
                "--worker-id".to_owned(),
                launch.worker_id.clone(),
                "--daemon-addr".to_owned(),
                launch.daemon_addr.clone(),
                "--token".to_owned(),
                launch.token.clone(),
                "--worker-root".to_owned(),
                launch.layout.worker_root().display().to_string(),
            ],
        }
    }
}

pub struct ChildHandle {
    id: u32,
    waiter: Option<Box<dyn ChildWaiter>>,
}

impl ChildHandle {
    pub fn new(id: u32) -> Self {
        Self { id, waiter: None }
    }

    fn with_waiter(id: u32, waiter: impl ChildWaiter) -> Self {
        Self {
            id,
            waiter: Some(Box::new(waiter)),
        }
    }

    pub fn with_exit_for_test(id: u32, exit: ChildExit) -> Self {
        Self::with_waiter(id, StaticChildWaiter { exit })
    }

    pub fn id(&self) -> u32 {
        self.id
    }

    pub fn wait(self) -> ChildExit {
        self.waiter
            .map(|waiter| waiter.wait())
            .unwrap_or_else(|| ChildExit::unavailable("child exit handle unavailable"))
    }
}

impl std::fmt::Debug for ChildHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ChildHandle")
            .field("id", &self.id)
            .finish_non_exhaustive()
    }
}

trait ChildWaiter: Send + 'static {
    fn wait(self: Box<Self>) -> ChildExit;
}

struct OsChildWaiter {
    child: Child,
}

impl ChildWaiter for OsChildWaiter {
    fn wait(mut self: Box<Self>) -> ChildExit {
        match self.child.wait() {
            Ok(status) => status
                .code()
                .map(ChildExit::exited)
                .unwrap_or_else(|| ChildExit::unavailable("worker process terminated by signal")),
            Err(error) => ChildExit::unavailable(format!("worker process wait failed: {error}")),
        }
    }
}

struct StaticChildWaiter {
    exit: ChildExit,
}

impl ChildWaiter for StaticChildWaiter {
    fn wait(self: Box<Self>) -> ChildExit {
        self.exit
    }
}

pub trait ProcessSpawner: Send + Sync + 'static {
    fn spawn(&self, command: WorkerProcessCommand) -> DaemonResult<ChildHandle>;
}

pub trait ChildMonitor: Send + Sync + 'static {
    fn monitor(&self, worker_id: String, child: ChildHandle, manager: SessionManager);
}

#[derive(Clone, Debug, Default)]
pub struct ThreadChildMonitor;

impl ChildMonitor for ThreadChildMonitor {
    fn monitor(&self, worker_id: String, child: ChildHandle, manager: SessionManager) {
        thread::spawn(move || {
            let exit = child.wait();
            let _ = manager.record_child_exit(&worker_id, exit);
        });
    }
}

#[derive(Clone, Debug, Default)]
pub struct OsProcessSpawner;

impl ProcessSpawner for OsProcessSpawner {
    fn spawn(&self, command: WorkerProcessCommand) -> DaemonResult<ChildHandle> {
        let mut process = Command::new(command.program());
        process.args(command.args());
        let child = process.spawn().map_err(|source| {
            DaemonError::root_configuration(
                Some(command.program().to_path_buf()),
                format!(
                    "could not spawn worker process `{}`",
                    command.display_without_secrets()
                ),
                Some(Box::new(source)),
            )
        })?;

        Ok(ChildHandle::with_waiter(
            child.id(),
            OsChildWaiter { child },
        ))
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ChildExit {
    Exited(i32),
    Unavailable(String),
}

impl ChildExit {
    pub fn exited(code: i32) -> Self {
        Self::Exited(code)
    }

    pub fn unavailable(reason: impl Into<String>) -> Self {
        Self::Unavailable(reason.into())
    }

    pub fn map_status(&self, prior_terminal: Option<WorkerStatus>) -> ChildExitStatus {
        if let Some(WorkerStatus::Succeeded) = prior_terminal {
            return ChildExitStatus::new(WorkerStatus::Succeeded, "worker reported success");
        }

        match self {
            Self::Exited(0) => ChildExitStatus::new(WorkerStatus::Stopped, "worker process exited"),
            Self::Exited(code) => ChildExitStatus::new(
                WorkerStatus::Failed,
                format!("worker process exited with status {code}"),
            ),
            Self::Unavailable(reason) => {
                ChildExitStatus::new(WorkerStatus::Unavailable, redact_failure_text(reason))
            }
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ChildExitStatus {
    status: WorkerStatus,
    reason: String,
}

impl ChildExitStatus {
    fn new(status: WorkerStatus, reason: impl Into<String>) -> Self {
        Self {
            status,
            reason: reason.into(),
        }
    }

    pub fn status(&self) -> WorkerStatus {
        self.status
    }

    pub fn reason(&self) -> &str {
        &self.reason
    }
}

/// Daemon-owned launcher that registers the authenticated worker session before spawning.
pub struct ProcessWorkerLauncher<S, M = ThreadChildMonitor> {
    session_manager: SessionManager,
    spawner: Arc<S>,
    monitor: M,
    binary: PathBuf,
    daemon_addr: String,
}

impl<S> ProcessWorkerLauncher<S>
where
    S: ProcessSpawner,
{
    pub fn new(
        session_manager: SessionManager,
        spawner: S,
        binary: PathBuf,
        daemon_addr: impl Into<String>,
    ) -> Self {
        Self {
            session_manager,
            spawner: Arc::new(spawner),
            monitor: ThreadChildMonitor,
            binary,
            daemon_addr: daemon_addr.into(),
        }
    }
}

impl<S, M> ProcessWorkerLauncher<S, M>
where
    S: ProcessSpawner,
    M: ChildMonitor,
{
    pub fn with_monitor(
        session_manager: SessionManager,
        spawner: S,
        monitor: M,
        binary: PathBuf,
        daemon_addr: impl Into<String>,
    ) -> Self {
        Self {
            session_manager,
            spawner: Arc::new(spawner),
            monitor,
            binary,
            daemon_addr: daemon_addr.into(),
        }
    }
}

impl<S, M> WorkerLauncher for ProcessWorkerLauncher<S, M>
where
    S: ProcessSpawner,
    M: ChildMonitor,
{
    fn launch(&self, request: LaunchRequest) -> DaemonResult<()> {
        let token = one_time_token(&request.worker_id);
        let worker_id = request.worker_id.clone();

        self.session_manager
            .register_session_worker(SessionWorker {
                worker_id: request.worker_id.clone(),
                mode: request.mode,
                token: token.clone(),
                prompt: request.prompt.clone(),
                repo: request.repo.clone(),
                layout: request.layout.clone(),
            })?;

        let launch = ProcessLaunch {
            worker_id: request.worker_id,
            mode: request.mode,
            daemon_addr: self.daemon_addr.clone(),
            token,
            prompt: request.prompt,
            repo: request.repo,
            repo_identity: request.repo_identity,
            layout: request.layout,
        };
        let command = WorkerCommandBuilder::new(self.binary.clone()).build(&launch);
        let child = match self.spawner.spawn(command) {
            Ok(child) => child,
            Err(error) => {
                self.session_manager
                    .record_spawn_failure(&worker_id, error.to_string())?;
                return Err(error);
            }
        };

        self.monitor
            .monitor(worker_id, child, self.session_manager.clone());

        Ok(())
    }
}

fn one_time_token(worker_id: &str) -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    format!("worker-session-{worker_id}-{nanos}")
}

fn worker_binary_name() -> &'static str {
    WORKER_BINARY
}

fn executable_name(name: &str) -> String {
    if cfg!(windows) {
        format!("{name}.exe")
    } else {
        name.to_owned()
    }
}
