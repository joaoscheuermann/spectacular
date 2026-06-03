use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use daemon::error::DaemonResult;
use daemon::registry::{Registry, WorkerMode};
use daemon::service::{
    DispatchDeps, IdGenerator, LaunchRequest, LifecycleService, NoopLifecycleLogger, ServiceConfig,
    WorkerLauncher,
};
use daemon::worker_session::{
    AttachRequest, SessionCommandSender, SessionManager, SessionWorker, WorkerSessionConfig,
};
use lifecycle::proto::doric::lifecycle::v1 as pb;

#[test]
fn lifecycle_service_worker_frames_replay_prompt_milestones_without_collapsing_names() {
    let fixture = LifecycleFixture::new();
    let service = fixture.service();
    let clone_message = "cloning repo: https://example.com/org/repo.git";

    service
        .dispatch(pb::DispatchRequest {
            mode: pb::JobMode::Feature as i32,
            prompt: "write requirements".to_owned(),
            repo: "https://token@example.com/org/repo.git".to_owned(),
        })
        .unwrap();
    fixture
        .manager
        .attach(AttachRequest::hello("worker-1", "session-token"))
        .unwrap();

    for frame in [
        worker_event_with_message("repo_preparation", pb::WorkerStatus::Running, clone_message),
        worker_event("prompt_agent_started", pb::WorkerStatus::Running),
        worker_event("prompt_artifact_written", pb::WorkerStatus::Running),
        worker_event("prompt_agent_completed", pb::WorkerStatus::Succeeded),
    ] {
        fixture.manager.record_worker_frame(frame).unwrap();
    }

    let replay = service
        .stream(pb::StreamWorkerRequest {
            worker_id: "worker-1".to_owned(),
            from_sequence: 0,
        })
        .unwrap();
    let events = replay.replay();
    let names = replay_names(events);
    let repo_preparation = replay_event(events, "repo_preparation");

    assert_eq!(repo_preparation.name, "repo_preparation");
    assert_eq!(repo_preparation.message, clone_message);
    assert_eq!(
        milestone_names(&names),
        vec![
            "repo_preparation",
            "prompt_agent_started",
            "prompt_artifact_written",
            "prompt_agent_completed"
        ]
    );
}

struct LifecycleFixture {
    registry: Arc<Mutex<Registry>>,
    manager: SessionManager,
    root: TempRoot,
}

impl LifecycleFixture {
    fn new() -> Self {
        let registry = Arc::new(Mutex::new(Registry::in_memory()));
        let manager = SessionManager::new(WorkerSessionConfig {
            registry: registry.clone(),
            attach_deadline: Duration::from_secs(30),
            lifecycle_logger: NoopLifecycleLogger,
        });

        Self {
            registry,
            manager,
            root: TempRoot::new("lifecycle-service"),
        }
    }

    fn service(&self) -> LifecycleService {
        LifecycleService::new(DispatchDeps {
            config: ServiceConfig::new(self.root.path().to_path_buf(), 1_000),
            registry: self.registry.clone(),
            launcher: RegisteringLauncher {
                manager: self.manager.clone(),
            },
            command_sender: SessionCommandSender::new(self.manager.clone()),
            id_generator: FixedId,
            lifecycle_logger: NoopLifecycleLogger,
        })
    }
}

struct RegisteringLauncher {
    manager: SessionManager,
}

impl WorkerLauncher for RegisteringLauncher {
    fn launch(&self, request: LaunchRequest) -> DaemonResult<()> {
        self.manager.register_session_worker(SessionWorker {
            worker_id: request.worker_id,
            mode: request.mode,
            token: "session-token".to_owned(),
            prompt: request.prompt,
            repo: request.repo,
            layout: request.layout,
        })
    }
}

struct FixedId;

impl IdGenerator for FixedId {
    fn next_worker_id(&self, _mode: WorkerMode) -> DaemonResult<String> {
        Ok("worker-1".to_owned())
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

fn worker_event(name: &str, status: pb::WorkerStatus) -> pb::WorkerFrame {
    worker_event_with_message(name, status, "")
}

fn worker_event_with_message(
    name: &str,
    status: pb::WorkerStatus,
    message: &str,
) -> pb::WorkerFrame {
    pb::WorkerFrame {
        frame: Some(pb::worker_frame::Frame::Event(pb::WorkerEvent {
            worker_id: "worker-1".to_owned(),
            sequence: 0,
            status: status as i32,
            name: name.to_owned(),
            message: message.to_owned(),
            input: None,
            occurred_at: None,
        })),
    }
}

fn replay_names(events: &[pb::WorkerEvent]) -> Vec<&str> {
    events.iter().map(|event| event.name.as_str()).collect()
}

fn milestone_names<'a>(names: &'a [&'a str]) -> Vec<&'a str> {
    names
        .iter()
        .copied()
        .filter(|name| {
            matches!(
                *name,
                "repo_preparation"
                    | "prompt_agent_started"
                    | "prompt_artifact_written"
                    | "prompt_agent_completed"
            )
        })
        .collect()
}

fn replay_event<'a>(events: &'a [pb::WorkerEvent], name: &str) -> &'a pb::WorkerEvent {
    events
        .iter()
        .find(|event| event.name == name)
        .unwrap_or_else(|| panic!("expected replay event `{name}` in {events:?}"))
}

fn suffix() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos()
}
