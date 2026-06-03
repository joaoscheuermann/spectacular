use std::str::FromStr;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use lifecycle::identity::WorkerId;
use lifecycle::repo::RepoIdentity;
use lifecycle::status::WorkerStatus;

use crate::process::{
    ChildExit, ProcessSpawner, ProcessWorkerLauncher, WorkerBinaryConfig, WorkerBinaryResolver,
    WorkerCommandBuilder,
};
use crate::registry::{Registry, WorkerMode, WorkerRecord};
use crate::root::prepare_worker_layout;
use crate::service::{LaunchRequest, NoopLifecycleLogger, WorkerLauncher};
use crate::worker_session::{AttachRequest, SessionManager, WorkerSessionConfig};

mod support {
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/unit/process_support.rs"
    ));
}

use support::*;

/// Verifies explicit worker binary resolution uses the configured executable.
#[test]
fn resolve_worker_binary_explicit_path_returns_configured_executable() {
    let fixture = ProcessFixture::new("explicit-binary");
    let binary = fixture.touch_executable("custom-worker");
    let resolver = WorkerBinaryResolver::new(fixture.current_exe());

    let resolved = resolver
        .resolve(WorkerBinaryConfig::explicit(binary.clone()))
        .unwrap();

    assert_eq!(resolved.path(), binary.as_path());
}

/// Verifies default worker binary resolution uses the current executable sibling.
#[test]
fn resolve_worker_binary_without_config_uses_current_exe_sibling() {
    let fixture = ProcessFixture::new("sibling-binary");
    let sibling = fixture.touch_executable(worker_binary_name());
    let resolver = WorkerBinaryResolver::new(fixture.current_exe());

    let resolved = resolver.resolve(WorkerBinaryConfig::default()).unwrap();

    assert_eq!(resolved.path(), sibling.as_path());
}

/// Verifies worker command args contain only session bootstrap fields.
#[test]
fn build_worker_command_valid_launch_uses_worker_id_daemon_addr_token_and_worker_root() {
    let fixture = ProcessFixture::new("command-bootstrap");
    let binary = fixture.touch_executable(worker_binary_name());
    let launch = fixture.launch("worker-command", "attach-token");

    let command = WorkerCommandBuilder::new(binary).build(&launch);
    let args = command.args();

    assert!(args.contains(&"--worker-id".to_owned()));
    assert!(args.contains(&"worker-command".to_owned()));
    assert!(args.contains(&"--daemon-addr".to_owned()));
    assert!(args.contains(&"127.0.0.1:47821".to_owned()));
    assert!(args.contains(&"--token".to_owned()));
    assert!(args.contains(&"attach-token".to_owned()));
    assert!(args.contains(&"--worker-root".to_owned()));
    assert!(args.contains(&launch.layout.worker_root().display().to_string()));
}

/// Verifies raw job payload is not passed through process arguments.
#[test]
fn build_worker_command_valid_launch_omits_raw_prompt_repo_and_repo_identity() {
    let fixture = ProcessFixture::new("command-redaction");
    let binary = fixture.touch_executable(worker_binary_name());
    let launch = fixture.launch("worker-redaction", "secret-token");

    let command = WorkerCommandBuilder::new(binary).build(&launch);
    let joined = command.args().join(" ");

    assert!(!joined.contains("write private requirements"));
    assert!(!joined.contains("https://secret@example.com/org/private.git"));
    assert!(!joined.contains("https://example.com/org/private.git"));
    assert!(!command.display_without_secrets().contains("secret-token"));
}

/// Verifies process spawn records a child without leaking the one-time token.
#[test]
fn spawn_worker_success_records_child_handle_without_printing_token() {
    let fixture = ProcessFixture::new("spawn-success");
    let binary = fixture.touch_executable(worker_binary_name());
    let launch = fixture.launch("worker-spawn", "spawn-token");
    let command = WorkerCommandBuilder::new(binary).build(&launch);
    let spawner = RecordingSpawner::default();

    let child = spawner.spawn(command).unwrap();

    assert_eq!(child.id(), 42);
    assert_eq!(spawner.commands()[0].worker_id(), "worker-spawn");
    assert!(!spawner.commands()[0]
        .display_without_secrets()
        .contains("spawn-token"));
}

/// Verifies the real launcher registers an authenticated session before spawn.
#[test]
fn launch_worker_valid_request_registers_session_and_sends_payload_only_after_attach() {
    let fixture = ProcessFixture::new("launcher-session");
    let binary = fixture.touch_executable(worker_binary_name());
    let registry = Arc::new(Mutex::new(Registry::in_memory()));
    let manager = SessionManager::new(WorkerSessionConfig {
        registry: registry.clone(),
        attach_deadline: Duration::from_secs(30),
        lifecycle_logger: NoopLifecycleLogger,
    });
    let spawner = SharedRecordingSpawner::default();
    let launcher = ProcessWorkerLauncher::new(
        manager.clone(),
        spawner.clone(),
        binary,
        "127.0.0.1:47821",
    );
    let worker_id = WorkerId::from_str("worker-launcher").unwrap();
    let layout = prepare_worker_layout(&fixture.root, worker_id.clone()).unwrap();

    registry
        .lock()
        .unwrap()
        .insert(WorkerRecord::new(
            worker_id,
            WorkerMode::Feature,
            RepoIdentity::from_raw_url("https://example.com/org/repo.git").unwrap(),
            WorkerStatus::Accepted,
            "accepted",
        ))
        .unwrap();
    launcher
        .launch(LaunchRequest {
            worker_id: "worker-launcher".to_owned(),
            mode: WorkerMode::Feature,
            prompt: "write private requirements".to_owned(),
            repo: "https://secret@example.com/org/private.git".to_owned(),
            repo_identity: "https://example.com/org/private.git".to_owned(),
            layout,
        })
        .unwrap();

    let token = manager.worker_for_test("worker-launcher").unwrap().token;
    let decision = manager
        .attach(AttachRequest::hello("worker-launcher", token))
        .unwrap();
    let command_args = spawner.commands()[0].args().join(" ");
    let start_job = start_job(decision.into_first_daemon_frame().unwrap());

    assert!(!command_args.contains("write private requirements"));
    assert!(!command_args.contains("https://secret@example.com/org/private.git"));
    assert_eq!(start_job.prompt, "write private requirements");
    assert_eq!(start_job.repo, "https://secret@example.com/org/private.git");
}

/// Verifies spawn failures fail the accepted registry worker without leaking payload.
#[test]
fn launch_worker_spawn_failure_marks_worker_failed_with_redacted_reason() {
    let fixture = ProcessFixture::new("launcher-spawn-failure");
    let binary = fixture.touch_executable(worker_binary_name());
    let registry = Arc::new(Mutex::new(Registry::in_memory()));
    let manager = SessionManager::new(WorkerSessionConfig {
        registry: registry.clone(),
        attach_deadline: Duration::from_secs(30),
        lifecycle_logger: NoopLifecycleLogger,
    });
    let launcher = ProcessWorkerLauncher::with_monitor(
        manager,
        FailingSpawner,
        NoopChildMonitor,
        binary,
        "127.0.0.1:47821",
    );
    let worker_id = WorkerId::from_str("worker-spawn-failure").unwrap();
    let layout = prepare_worker_layout(&fixture.root, worker_id.clone()).unwrap();

    registry
        .lock()
        .unwrap()
        .insert(WorkerRecord::new(
            worker_id,
            WorkerMode::Feature,
            RepoIdentity::from_raw_url("https://example.com/org/repo.git").unwrap(),
            WorkerStatus::Accepted,
            "accepted",
        ))
        .unwrap();
    let error = launcher
        .launch(LaunchRequest {
            worker_id: "worker-spawn-failure".to_owned(),
            mode: WorkerMode::Feature,
            prompt: "write private requirements".to_owned(),
            repo: "https://secret@example.com/org/private.git".to_owned(),
            repo_identity: "https://example.com/org/private.git".to_owned(),
            layout,
        })
        .unwrap_err();

    let summary = summary(&registry, "worker-spawn-failure");
    let events = registry
        .lock()
        .unwrap()
        .replay(&WorkerId::from_str("worker-spawn-failure").unwrap(), 0)
        .unwrap();

    assert!(error.to_string().contains("spawn failed"));
    assert_eq!(summary.status(), WorkerStatus::Failed);
    assert!(summary.terminal_reason().unwrap().contains("spawn failed"));
    assert!(!summary
        .terminal_reason()
        .unwrap()
        .contains("https://secret@example.com"));
    assert!(events
        .iter()
        .any(|event| matches!(event, crate::event::ReplayItem::Worker(worker) if worker.name() == "failed")));
}

/// Verifies monitored child exits update registry status and append terminal events.
#[test]
fn launch_worker_child_monitor_nonzero_exit_marks_failed_and_appends_event() {
    let fixture = ProcessFixture::new("launcher-monitor-failure");
    let binary = fixture.touch_executable(worker_binary_name());
    let registry = Arc::new(Mutex::new(Registry::in_memory()));
    let manager = SessionManager::new(WorkerSessionConfig {
        registry: registry.clone(),
        attach_deadline: Duration::from_secs(30),
        lifecycle_logger: NoopLifecycleLogger,
    });
    let launcher = ProcessWorkerLauncher::with_monitor(
        manager,
        StaticExitSpawner::new(ChildExit::exited(9)),
        ImmediateChildMonitor,
        binary,
        "127.0.0.1:47821",
    );
    let worker_id = WorkerId::from_str("worker-monitor-failure").unwrap();
    let layout = prepare_worker_layout(&fixture.root, worker_id.clone()).unwrap();

    registry
        .lock()
        .unwrap()
        .insert(WorkerRecord::new(
            worker_id,
            WorkerMode::Feature,
            RepoIdentity::from_raw_url("https://example.com/org/repo.git").unwrap(),
            WorkerStatus::Accepted,
            "accepted",
        ))
        .unwrap();
    launcher
        .launch(LaunchRequest {
            worker_id: "worker-monitor-failure".to_owned(),
            mode: WorkerMode::Feature,
            prompt: "write private requirements".to_owned(),
            repo: "https://secret@example.com/org/private.git".to_owned(),
            repo_identity: "https://example.com/org/private.git".to_owned(),
            layout,
        })
        .unwrap();

    let summary = summary(&registry, "worker-monitor-failure");
    let events = registry
        .lock()
        .unwrap()
        .replay(&WorkerId::from_str("worker-monitor-failure").unwrap(), 0)
        .unwrap();

    assert_eq!(summary.status(), WorkerStatus::Failed);
    assert!(summary.terminal_reason().unwrap().contains("status 9"));
    assert!(events
        .iter()
        .any(|event| matches!(event, crate::event::ReplayItem::Worker(worker) if worker.name() == "failed")));
}

/// Verifies a successful terminal worker frame is preserved when the child exits.
#[test]
fn launch_worker_child_monitor_after_terminal_success_preserves_succeeded_status() {
    let fixture = ProcessFixture::new("launcher-monitor-success");
    let binary = fixture.touch_executable(worker_binary_name());
    let registry = Arc::new(Mutex::new(Registry::in_memory()));
    let manager = SessionManager::new(WorkerSessionConfig {
        registry: registry.clone(),
        attach_deadline: Duration::from_secs(30),
        lifecycle_logger: NoopLifecycleLogger,
    });
    let launcher = ProcessWorkerLauncher::with_monitor(
        manager,
        StaticExitSpawner::new(ChildExit::exited(0)),
        SuccessBeforeExitMonitor,
        binary,
        "127.0.0.1:47821",
    );
    let worker_id = WorkerId::from_str("worker-monitor-success").unwrap();
    let layout = prepare_worker_layout(&fixture.root, worker_id.clone()).unwrap();

    registry
        .lock()
        .unwrap()
        .insert(WorkerRecord::new(
            worker_id,
            WorkerMode::Feature,
            RepoIdentity::from_raw_url("https://example.com/org/repo.git").unwrap(),
            WorkerStatus::Accepted,
            "accepted",
        ))
        .unwrap();
    launcher
        .launch(LaunchRequest {
            worker_id: "worker-monitor-success".to_owned(),
            mode: WorkerMode::Feature,
            prompt: "write private requirements".to_owned(),
            repo: "https://secret@example.com/org/private.git".to_owned(),
            repo_identity: "https://example.com/org/private.git".to_owned(),
            layout,
        })
        .unwrap();

    let summary = summary(&registry, "worker-monitor-success");

    assert_eq!(summary.status(), WorkerStatus::Succeeded);
    assert_eq!(summary.terminal_reason(), Some("prompt artifact written"));
}

/// Verifies child exits map to daemon-visible terminal status without overwriting success.
#[test]
fn map_child_exit_preserves_terminal_success_and_maps_nonzero_to_failed() {
    let success = ChildExit::exited(0).map_status(Some(WorkerStatus::Succeeded));
    let failure = ChildExit::exited(9).map_status(None);
    let unavailable = ChildExit::unavailable("lost child handle").map_status(None);

    assert_eq!(success.status(), WorkerStatus::Succeeded);
    assert_eq!(failure.status(), WorkerStatus::Failed);
    assert!(failure.reason().contains("exit"));
    assert!(matches!(
        unavailable.status(),
        WorkerStatus::Stopped | WorkerStatus::Unavailable
    ));
    assert!(!unavailable.reason().contains("secret"));
}
