use std::fs;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::process::{ChildHandle, ProcessSpawner, WorkerBinaryConfig, WorkerBinaryResolver};
use crate::server::{build_process_service, build_service, ServerConfig};
use crate::service::{CommandSender, IdGenerator, WorkerLauncher};

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
    assert!(!service.has_bound_listener());
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
