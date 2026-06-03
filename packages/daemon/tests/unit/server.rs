use std::fs;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::process::{ChildHandle, ProcessSpawner, WorkerBinaryConfig, WorkerBinaryResolver};
use crate::server::{
    build_process_service, build_service, serve_bundle_on_listener_with_shutdown, ServerConfig,
};
use crate::service::{
    CommandSender, IdGenerator, LifecycleLogger, TerminalLifecycleLogger, WorkerLauncher,
};
use lifecycle::proto::doric::lifecycle::v1 as pb;
use tokio::net::TcpListener;

/// Verifies that daemon server config defaults to loopback lifecycle port.
#[test]
fn server_config_default_loopback_uses_lifecycle_daemon_port() {
    let config = ServerConfig::default();

    assert_eq!(
        config.bind_addr(),
        &"127.0.0.1:47821".parse::<SocketAddr>().unwrap()
    );
}

/// Verifies that custom bind addresses parse without opening a listener.
#[test]
fn server_config_custom_addr_preserves_requested_bind_addr() {
    let config = ServerConfig::parse("127.0.0.1:49123", TempRoot::new("custom").path()).unwrap();

    assert_eq!(
        config.bind_addr(),
        &"127.0.0.1:49123".parse::<SocketAddr>().unwrap()
    );
}

/// Verifies that invalid bind addresses fail as configuration errors.
#[test]
fn server_config_invalid_addr_returns_configuration_error() {
    let error = ServerConfig::parse("not an address", TempRoot::new("invalid").path())
        .unwrap_err();

    assert!(error.to_string().contains("address"));
}

/// Verifies service/router construction is testable without binding a socket.
#[test]
fn build_service_valid_config_constructs_service_without_binding() {
    let root = TempRoot::new("valid");
    let config = ServerConfig::parse("127.0.0.1:49124", root.path()).unwrap();

    let service = build_service(
        config,
        NoopLauncher,
        NoopCommandSender,
        FixedIdGenerator("server-worker".to_owned()),
    )
    .unwrap();

    assert!(service.includes_lifecycle_service());
    assert!(!service.has_bound_listener());
}

/// Verifies production-style construction wires process spawning through sessions.
#[test]
fn build_process_service_valid_config_uses_process_launcher_and_session_sender() {
    let root = TempRoot::new("process-service");
    let binary = root.touch_executable(worker_binary_name());
    let config = ServerConfig::parse("127.0.0.1:49126", root.path()).unwrap();
    let spawner = SharedRecordingSpawner;

    let service = build_process_service(
        config,
        WorkerBinaryResolver::new(root.current_exe()),
        WorkerBinaryConfig::explicit(binary),
        spawner.clone(),
    )
    .unwrap();

    assert!(service.includes_lifecycle_service());
    assert!(service.uses_process_worker_launcher());
    assert!(service.uses_session_command_sender());
    assert!(service.uses_terminal_lifecycle_logger());
    assert!(!service.has_bound_listener());
}

/// Verifies injected test builders avoid real terminal logging by default.
#[test]
fn build_service_valid_config_uses_noop_lifecycle_logger_for_tests() {
    let root = TempRoot::new("noop-logger");
    let config = ServerConfig::parse("127.0.0.1:49128", root.path()).unwrap();

    let service = build_service(
        config,
        NoopLauncher,
        NoopCommandSender,
        FixedIdGenerator("server-worker".to_owned()),
    )
    .unwrap();

    assert!(service.uses_noop_lifecycle_logger());
    assert!(!service.uses_terminal_lifecycle_logger());
}

/// Verifies terminal lifecycle logger output uses shared safe timestamped lines.
#[test]
fn terminal_lifecycle_logger_controlled_message_formats_redacted_one_line() {
    let output = Arc::new(Mutex::new(Vec::new()));
    let timestamp = UNIX_EPOCH + Duration::from_secs(1_700_000_000);
    let logger = TerminalLifecycleLogger::for_writer(output.clone(), timestamp);

    logger.log(
        "cloning repo: https://user:secret@example.com/org/repo.git\nnext line \u{1b}[31mred",
    );

    let rendered = String::from_utf8(output.lock().unwrap().clone()).unwrap();
    assert_eq!(
        rendered,
        "[2023-11-14T22:13:20Z] cloning repo: https://example.com/org/repo.git next line red\n"
    );
    assert!(!rendered.contains("secret"));
    assert!(!rendered.contains('\u{1b}'));
    assert_eq!(rendered.lines().count(), 1);
}

/// Verifies production-style process services generate UUIDv6 worker ids.
#[test]
fn build_process_service_dispatch_generates_uuid_v6_worker_id() {
    let root = TempRoot::new("process-service-uuid");
    let binary = root.touch_executable(worker_binary_name());
    let config = ServerConfig::parse("127.0.0.1:49127", root.path()).unwrap();

    let service = build_process_service(
        config,
        WorkerBinaryResolver::new(root.current_exe()),
        WorkerBinaryConfig::explicit(binary),
        SharedRecordingSpawner,
    )
    .unwrap();

    let response = service
        .lifecycle()
        .dispatch(pb::DispatchRequest {
            mode: pb::JobMode::Feature as i32,
            prompt: "write requirements".to_owned(),
            repo: "https://example.com/org/repo.git".to_owned(),
        })
        .unwrap();

    assert!(
        is_uuid_v6(&response.worker_id),
        "worker id should be UUIDv6-shaped, got {}",
        response.worker_id
    );
}

/// Verifies worker-root validation happens before any listener is bound.
#[test]
fn build_service_invalid_worker_root_fails_before_binding() {
    let root = TempRoot::new("missing");
    let missing = root.path().join("missing-workers");
    let config = ServerConfig::parse("127.0.0.1:49125", &missing).unwrap();

    let error = build_service(
        config,
        NoopLauncher,
        NoopCommandSender,
        FixedIdGenerator("server-worker".to_owned()),
    )
    .unwrap_err();

    assert!(error.to_string().contains("worker root"));
    assert!(!error.bound_listener());
}

/// Verifies a foreground daemon server serves list over the generated gRPC path.
#[tokio::test]
async fn serve_bundle_generated_lifecycle_service_serves_list_over_grpc() {
    let root = TempRoot::new("grpc-list");
    let config = ServerConfig::parse("127.0.0.1:0", root.path()).unwrap();
    let mut service = build_service(
        config,
        NoopLauncher,
        NoopCommandSender,
        FixedIdGenerator("server-worker".to_owned()),
    )
    .unwrap();
    service
        .lifecycle_mut()
        .seed_for_test([(
            "server-worker",
            crate::registry::WorkerMode::Feature,
            pb::WorkerStatus::WaitingForInput as i32,
            "waiting for approval",
        )])
        .unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let (shutdown_sender, shutdown_receiver) = tokio::sync::oneshot::channel();
    let server = tokio::spawn(serve_bundle_on_listener_with_shutdown(
        service,
        listener,
        async move {
            let _ = shutdown_receiver.await;
        },
    ));

    let mut client =
        pb::lifecycle_service_client::LifecycleServiceClient::connect(format!("http://{addr}"))
            .await
            .unwrap();
    let response = client
        .list_workers(pb::ListWorkersRequest {})
        .await
        .unwrap()
        .into_inner();

    shutdown_sender.send(()).unwrap();
    server.await.unwrap().unwrap();
    assert_eq!(response.workers.len(), 1);
    assert_eq!(response.workers[0].worker_id, "server-worker");
    assert_eq!(response.workers[0].activity, "waiting for approval");
    assert_eq!(response.workers[0].pending_request_id, "request-1");
}

struct NoopLauncher;

impl WorkerLauncher for NoopLauncher {
    fn launch(&self, _request: crate::service::LaunchRequest) -> crate::error::DaemonResult<()> {
        Ok(())
    }
}

struct NoopCommandSender;

impl CommandSender for NoopCommandSender {
    fn send_answer(
        &self,
        _command: crate::service::AnswerCommand,
    ) -> crate::error::DaemonResult<()> {
        Ok(())
    }
}

struct FixedIdGenerator(String);

impl IdGenerator for FixedIdGenerator {
    fn next_worker_id(
        &self,
        _mode: crate::registry::WorkerMode,
    ) -> crate::error::DaemonResult<String> {
        Ok(self.0.clone())
    }
}

struct TempRoot {
    path: PathBuf,
}

impl TempRoot {
    fn new(name: &str) -> Self {
        let path = std::env::temp_dir().join(format!("doric-daemon-server-{name}-{}", suffix()));
        fs::create_dir_all(&path).unwrap();
        Self { path }
    }

    fn path(&self) -> &Path {
        &self.path
    }

    fn current_exe(&self) -> PathBuf {
        self.path.join(executable_name("doric-daemon"))
    }

    fn touch_executable(&self, name: &str) -> PathBuf {
        let path = self.path.join(executable_name(name));
        fs::write(&path, "").unwrap();
        path
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

#[derive(Clone, Default)]
struct SharedRecordingSpawner;

impl ProcessSpawner for SharedRecordingSpawner {
    fn spawn(
        &self,
        _command: crate::process::WorkerProcessCommand,
    ) -> crate::error::DaemonResult<ChildHandle> {
        Ok(ChildHandle::new(52))
    }
}

fn worker_binary_name() -> &'static str {
    "doric-worker"
}

fn executable_name(name: &str) -> String {
    if cfg!(windows) {
        format!("{name}.exe")
    } else {
        name.to_owned()
    }
}

fn is_uuid_v6(value: &str) -> bool {
    let bytes = value.as_bytes();

    bytes.len() == 36
        && bytes[8] == b'-'
        && bytes[13] == b'-'
        && bytes[14] == b'6'
        && bytes[18] == b'-'
        && bytes[23] == b'-'
        && bytes
            .iter()
            .enumerate()
            .all(|(index, byte)| matches!(index, 8 | 13 | 18 | 23) || byte.is_ascii_hexdigit())
}
