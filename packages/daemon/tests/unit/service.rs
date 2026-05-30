use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use lifecycle::proto::doric::lifecycle::v1 as pb;
use lifecycle::status::WorkerStatus;

use crate::registry::{Registry, WorkerMode};
use crate::service::{
    AnswerCommand, CommandSender, DispatchDeps, IdGenerator, LaunchRequest, LifecycleService,
    ServiceConfig, WorkerLauncher,
};

/// Verifies that dispatch validates prompt before registry writes or launch.
#[test]
fn dispatch_empty_prompt_rejects_without_record_or_launch() {
    let fixture = ServiceFixture::new();
    let service = fixture.service();

    let error = service.dispatch(dispatch_request(1, "", "https://example.com/org/repo.git"));

    assert!(error.unwrap_err().to_string().contains("prompt"));
    assert!(fixture.registry.lock().unwrap().list().is_empty());
    assert!(fixture.launcher.lock().unwrap().is_empty());
}

/// Verifies that dispatch validates repo before registry writes or launch.
#[test]
fn dispatch_empty_repo_rejects_without_record_or_launch() {
    let fixture = ServiceFixture::new();
    let service = fixture.service();

    let error = service.dispatch(dispatch_request(1, "write requirements", "   "));

    assert!(error.unwrap_err().to_string().contains("repo"));
    assert!(fixture.registry.lock().unwrap().list().is_empty());
    assert!(fixture.launcher.lock().unwrap().is_empty());
}

/// Verifies that dispatch rejects an unspecified mode before side effects.
#[test]
fn dispatch_unspecified_mode_rejects_without_record_or_launch() {
    let fixture = ServiceFixture::new();
    let service = fixture.service();

    let error = service.dispatch(dispatch_request(
        0,
        "write requirements",
        "https://example.com/org/repo.git",
    ));

    assert!(error.unwrap_err().to_string().contains("mode"));
    assert!(fixture.registry.lock().unwrap().list().is_empty());
    assert!(fixture.launcher.lock().unwrap().is_empty());
}

/// Verifies that dispatch prepares/validates root before registry writes or launch.
#[test]
fn dispatch_invalid_root_rejects_without_record_or_launch() {
    let fixture = ServiceFixture::new_with_missing_root();
    let service = fixture.service();

    let error = service.dispatch(dispatch_request(
        1,
        "write requirements",
        "https://example.com/org/repo.git",
    ));

    assert!(error.unwrap_err().to_string().contains("worker root"));
    assert!(fixture.registry.lock().unwrap().list().is_empty());
    assert!(fixture.launcher.lock().unwrap().is_empty());
}

/// Verifies that feature dispatch returns the public summary and launches once.
#[test]
fn dispatch_feature_request_returns_identity_status_and_launches_once() {
    let fixture = ServiceFixture::new();
    let service = fixture.service();

    let response = service
        .dispatch(dispatch_request(
            1,
            "write requirements",
            "https://token@example.com/org/repo.git",
        ))
        .unwrap();

    assert_eq!(response.worker_id, "worker-1");
    assert_eq!(response.mode, 1);
    assert_eq!(response.repo_identity, "https://example.com/org/repo.git");
    assert_eq!(response.status, 1);
    assert_eq!(fixture.registry.lock().unwrap().list().len(), 1);
    assert_eq!(fixture.launcher.lock().unwrap().len(), 1);
    assert_eq!(
        fixture.launcher.lock().unwrap()[0].mode,
        WorkerMode::Feature
    );
    assert_eq!(
        fixture.launcher.lock().unwrap()[0].prompt,
        "write requirements"
    );
    assert_eq!(
        fixture.launcher.lock().unwrap()[0].repo,
        "https://token@example.com/org/repo.git"
    );
}

/// Verifies that debug dispatch preserves the requested mode label.
#[test]
fn dispatch_debug_request_preserves_requested_mode_label() {
    let fixture = ServiceFixture::new();
    let service = fixture.service();

    let response = service
        .dispatch(dispatch_request(
            2,
            "debug failing prompt",
            "https://example.com/org/repo.git",
        ))
        .unwrap();

    assert_eq!(response.mode, 2);
    assert_eq!(
        fixture.registry.lock().unwrap().list()[0].mode(),
        WorkerMode::Debug
    );
    assert_eq!(fixture.launcher.lock().unwrap()[0].mode, WorkerMode::Debug);
}

/// Verifies that list returns active, waiting, failed, and terminal rows.
#[test]
fn list_mixed_registry_returns_all_public_summaries() {
    let fixture = ServiceFixture::new();
    fixture.seed_mixed_workers();
    let service = fixture.service();

    let response = service.list(pb::ListWorkersRequest {}).unwrap();
    let rows: Vec<_> = response
        .workers
        .iter()
        .map(|worker| (worker.worker_id.as_str(), worker.status))
        .collect();

    assert_eq!(rows.len(), 5);
    assert!(rows.contains(&("active-worker", 3)));
    assert!(rows.contains(&("waiting-worker", 4)));
    assert!(rows.contains(&("failed-worker", 6)));
    assert!(rows.contains(&("succeeded-worker", 5)));
    assert!(rows.contains(&("stopped-worker", 7)));
    assert!(response
        .workers
        .iter()
        .all(|worker| !worker.repo_identity.contains("token@")));
    let waiting = response
        .workers
        .iter()
        .find(|worker| worker.worker_id == "waiting-worker")
        .expect("waiting worker should be listed");
    assert_eq!(waiting.activity, "waiting");
    assert_eq!(waiting.pending_request_id, "request-1");

    let failed = response
        .workers
        .iter()
        .find(|worker| worker.worker_id == "failed-worker")
        .expect("failed worker should be listed");
    assert_eq!(failed.terminal_reason, "failed");
}

/// Verifies that stream rejects unknown workers without opening a tail.
#[test]
fn stream_unknown_worker_returns_unknown_worker_error_without_hanging() {
    let fixture = ServiceFixture::new();
    let service = fixture.service();

    let error = service.stream(pb::StreamWorkerRequest {
        worker_id: "missing-worker".to_owned(),
        from_sequence: 0,
    });

    assert!(error.unwrap_err().to_string().contains("unknown"));
}

/// Verifies replay order, suffix replay, and explicit truncation reporting.
#[test]
fn stream_known_worker_replays_retained_events_and_truncation() {
    let fixture = ServiceFixture::new_with_event_capacity(2);
    fixture.seed_truncated_worker();
    let service = fixture.service();

    let replay = service
        .stream(pb::StreamWorkerRequest {
            worker_id: "stream-worker".to_owned(),
            from_sequence: 0,
        })
        .unwrap();
    let names: Vec<_> = replay
        .replay()
        .iter()
        .map(|event| event.name.as_str())
        .collect();

    assert_eq!(
        names,
        vec![
            "history_truncated",
            "repo_preparation",
            "prompt_agent_started"
        ]
    );

    let suffix = service
        .stream(pb::StreamWorkerRequest {
            worker_id: "stream-worker".to_owned(),
            from_sequence: 1,
        })
        .unwrap();

    assert!(suffix.replay().iter().all(|event| event.sequence >= 1));
}

/// Verifies stream returns retained replay and then receives live events appended later.
#[test]
fn stream_known_worker_replays_then_tails_live_events() {
    let fixture = ServiceFixture::new();
    fixture.seed_stream_worker();
    let service = fixture.service();

    let stream = service
        .stream(pb::StreamWorkerRequest {
            worker_id: "stream-worker".to_owned(),
            from_sequence: 0,
        })
        .unwrap();

    let replay_names: Vec<_> = stream
        .replay()
        .iter()
        .map(|event| event.name.as_str())
        .collect();
    assert_eq!(replay_names, vec!["accepted", "starting"]);

    service
        .append_event_for_test("stream-worker", "prompt_agent_started")
        .unwrap();

    let live = stream.try_next().expect("live event should be delivered");
    assert_eq!(live.name, "prompt_agent_started");
    assert_eq!(live.sequence, 2);
}

/// Verifies daemon validation before forwarding a human-input answer.
#[test]
fn answer_input_pending_and_rejection_cases_validate_before_forwarding() {
    let fixture = ServiceFixture::new();
    fixture.seed_waiting_worker("answer-worker", "request-1");
    let service = fixture.service();

    let response = service
        .answer_input(pb::AnswerInputRequest {
            worker_id: "answer-worker".to_owned(),
            request_id: "request-1".to_owned(),
            text: "yes, continue".to_owned(),
        })
        .unwrap();

    assert!(response.accepted);
    assert_eq!(fixture.commands.lock().unwrap().len(), 1);
    assert_eq!(
        fixture.commands.lock().unwrap()[0].text,
        "yes, continue"
    );

    let duplicate = service.answer_input(pb::AnswerInputRequest {
        worker_id: "answer-worker".to_owned(),
        request_id: "request-1".to_owned(),
        text: "second answer".to_owned(),
    });
    assert!(duplicate.is_err());
    assert_eq!(fixture.commands.lock().unwrap().len(), 1);

    let unknown_worker = service.answer_input(pb::AnswerInputRequest {
        worker_id: "missing-worker".to_owned(),
        request_id: "request-1".to_owned(),
        text: "ignored".to_owned(),
    });
    assert!(unknown_worker.is_err());
    assert_eq!(fixture.commands.lock().unwrap().len(), 1);
}

/// Verifies command-delivery failure preserves pending input and waiting status.
#[test]
fn answer_input_command_failure_preserves_pending_input_without_running_status() {
    let fixture = ServiceFixture::new();
    fixture.seed_waiting_worker("answer-worker", "request-1");
    let service = fixture.failing_command_service();

    let error = service.answer_input(pb::AnswerInputRequest {
        worker_id: "answer-worker".to_owned(),
        request_id: "request-1".to_owned(),
        text: "yes, continue".to_owned(),
    });

    assert!(error.unwrap_err().to_string().contains("command channel unavailable"));
    let summary = fixture.registry.lock().unwrap().list().remove(0);
    assert_eq!(summary.status(), WorkerStatus::WaitingForInput);
    assert!(summary.pending_request_id().is_some());
    assert!(fixture.commands.lock().unwrap().is_empty());
}

fn dispatch_request(mode: i32, prompt: &str, repo: &str) -> pb::DispatchRequest {
    pb::DispatchRequest {
        mode,
        prompt: prompt.to_owned(),
        repo: repo.to_owned(),
    }
}

struct ServiceFixture {
    registry: Arc<Mutex<Registry>>,
    launcher: Arc<Mutex<Vec<LaunchRequest>>>,
    commands: Arc<Mutex<Vec<AnswerCommand>>>,
    ids: Arc<Mutex<Vec<String>>>,
    root: TempRoot,
    event_capacity: usize,
}

impl ServiceFixture {
    fn new() -> Self {
        Self::new_with_event_capacity(1_000)
    }

    fn new_with_event_capacity(event_capacity: usize) -> Self {
        Self {
            registry: Arc::new(Mutex::new(Registry::in_memory_with_event_capacity(
                event_capacity,
            ))),
            launcher: Arc::new(Mutex::new(Vec::new())),
            commands: Arc::new(Mutex::new(Vec::new())),
            ids: Arc::new(Mutex::new(vec!["worker-1".to_owned()])),
            root: TempRoot::new("service"),
            event_capacity,
        }
    }

    fn new_with_missing_root() -> Self {
        let fixture = Self::new();
        fs::remove_dir_all(fixture.root.path()).unwrap();
        fixture
    }

    fn service(&self) -> LifecycleService {
        LifecycleService::new(DispatchDeps {
            config: ServiceConfig::new(self.root.path().to_path_buf(), self.event_capacity),
            registry: self.registry.clone(),
            launcher: RecordingLauncher(self.launcher.clone()),
            command_sender: RecordingCommandSender(self.commands.clone()),
            id_generator: FixedIds(self.ids.clone()),
        })
    }

    fn failing_command_service(&self) -> LifecycleService {
        LifecycleService::new(DispatchDeps {
            config: ServiceConfig::new(self.root.path().to_path_buf(), self.event_capacity),
            registry: self.registry.clone(),
            launcher: RecordingLauncher(self.launcher.clone()),
            command_sender: FailingCommandSender,
            id_generator: FixedIds(self.ids.clone()),
        })
    }

    fn seed_mixed_workers(&self) {
        let mut service = self.service();
        service
            .seed_for_test([
                ("active-worker", WorkerMode::Feature, 3, "running"),
                ("waiting-worker", WorkerMode::Feature, 4, "waiting"),
                ("failed-worker", WorkerMode::Debug, 6, "failed"),
                ("succeeded-worker", WorkerMode::Feature, 5, "done"),
                ("stopped-worker", WorkerMode::Debug, 7, "stopped"),
            ])
            .unwrap();
    }

    fn seed_truncated_worker(&self) {
        let mut service = self.service();
        service
            .seed_events_for_test(
                "stream-worker",
                WorkerMode::Feature,
                [
                    "accepted",
                    "starting",
                    "repo_preparation",
                    "prompt_agent_started",
                ],
            )
            .unwrap();
    }

    fn seed_stream_worker(&self) {
        let mut service = self.service();
        service
            .seed_events_for_test(
                "stream-worker",
                WorkerMode::Feature,
                ["accepted", "starting"],
            )
            .unwrap();
    }

    fn seed_waiting_worker(&self, worker: &str, request: &str) {
        let mut service = self.service();
        service
            .seed_waiting_for_test(worker, WorkerMode::Feature, request, "Approve?")
            .unwrap();
    }
}

struct RecordingLauncher(Arc<Mutex<Vec<LaunchRequest>>>);

impl WorkerLauncher for RecordingLauncher {
    fn launch(&self, request: LaunchRequest) -> crate::error::DaemonResult<()> {
        self.0.lock().unwrap().push(request);
        Ok(())
    }
}

struct RecordingCommandSender(Arc<Mutex<Vec<AnswerCommand>>>);

impl CommandSender for RecordingCommandSender {
    fn send_answer(&self, command: AnswerCommand) -> crate::error::DaemonResult<()> {
        self.0.lock().unwrap().push(command);
        Ok(())
    }
}

struct FailingCommandSender;

impl CommandSender for FailingCommandSender {
    fn send_answer(&self, _command: AnswerCommand) -> crate::error::DaemonResult<()> {
        Err(crate::error::DaemonError::root_configuration(
            None,
            "command channel unavailable",
            None,
        ))
    }
}

struct FixedIds(Arc<Mutex<Vec<String>>>);

impl IdGenerator for FixedIds {
    fn next_worker_id(&self, mode: WorkerMode) -> crate::error::DaemonResult<String> {
        let fallback = match mode {
            WorkerMode::Feature => "feature-worker",
            WorkerMode::Debug => "debug-worker",
        };

        Ok(self
            .0
            .lock()
            .unwrap()
            .pop()
            .unwrap_or_else(|| fallback.to_owned()))
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

fn suffix() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos()
}
