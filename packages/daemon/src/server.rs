use std::error::Error;
use std::fmt::{self, Display};
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::error::DaemonError;
use crate::process::{
    OsProcessSpawner, ProcessSpawner, ProcessWorkerLauncher, WorkerBinaryConfig,
    WorkerBinaryResolver,
};
use crate::registry::Registry;
use crate::root::validate_worker_root;
use crate::service::{
    CommandSender, DispatchDeps, IdGenerator, LifecycleService, ServiceConfig,
    TimestampIdGenerator, WorkerLauncher,
};
use crate::worker_session::{SessionCommandSender, SessionManager, WorkerSessionConfig};

const DEFAULT_BIND_ADDR: &str = "127.0.0.1:47821";
const DEFAULT_EVENT_CAPACITY: usize = 1_000;
const DEFAULT_ATTACH_DEADLINE: Duration = Duration::from_secs(30);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ServerConfig {
    bind_addr: SocketAddr,
    worker_root: PathBuf,
    event_capacity: usize,
}

impl ServerConfig {
    pub fn parse(addr: &str, worker_root: &Path) -> Result<Self, ServerBuildError> {
        let bind_addr = addr.parse::<SocketAddr>().map_err(|source| {
            ServerBuildError::configuration(format!("invalid daemon bind address: {source}"))
        })?;

        Ok(Self {
            bind_addr,
            worker_root: worker_root.to_path_buf(),
            event_capacity: DEFAULT_EVENT_CAPACITY,
        })
    }

    pub fn bind_addr(&self) -> &SocketAddr {
        &self.bind_addr
    }

    pub fn worker_root(&self) -> &Path {
        &self.worker_root
    }
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            bind_addr: DEFAULT_BIND_ADDR
                .parse()
                .expect("default daemon bind address is valid"),
            worker_root: config::config_dir()
                .unwrap_or_else(|_| std::env::temp_dir().join("doric"))
                .join("workers"),
            event_capacity: DEFAULT_EVENT_CAPACITY,
        }
    }
}

pub struct ServiceBundle {
    lifecycle: LifecycleService,
    bind_addr: SocketAddr,
    bound_listener: bool,
    wiring: BundleWiring,
}

impl fmt::Debug for ServiceBundle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ServiceBundle")
            .field("bind_addr", &self.bind_addr)
            .field("bound_listener", &self.bound_listener)
            .finish_non_exhaustive()
    }
}

impl ServiceBundle {
    pub fn includes_lifecycle_service(&self) -> bool {
        true
    }

    pub fn lifecycle(&self) -> &LifecycleService {
        &self.lifecycle
    }

    pub fn has_bound_listener(&self) -> bool {
        self.bound_listener
    }

    pub fn bind_addr(&self) -> &SocketAddr {
        &self.bind_addr
    }

    pub fn uses_process_worker_launcher(&self) -> bool {
        self.wiring.process_worker_launcher
    }

    pub fn uses_session_command_sender(&self) -> bool {
        self.wiring.session_command_sender
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct BundleWiring {
    process_worker_launcher: bool,
    session_command_sender: bool,
}

#[derive(Debug)]
pub struct ServerBuildError {
    message: String,
    bound_listener: bool,
    source: Option<Box<dyn Error + Send + Sync>>,
}

impl ServerBuildError {
    fn configuration(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            bound_listener: false,
            source: None,
        }
    }

    pub fn bound_listener(&self) -> bool {
        self.bound_listener
    }
}

impl Display for ServerBuildError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl Error for ServerBuildError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        self.source
            .as_deref()
            .map(|source| source as &(dyn Error + 'static))
    }
}

impl From<DaemonError> for ServerBuildError {
    fn from(source: DaemonError) -> Self {
        Self {
            message: source.to_string(),
            bound_listener: false,
            source: Some(Box::new(source)),
        }
    }
}

pub fn build_service<L, C, I>(
    config: ServerConfig,
    launcher: L,
    command_sender: C,
    id_generator: I,
) -> Result<ServiceBundle, ServerBuildError>
where
    L: WorkerLauncher,
    C: CommandSender,
    I: IdGenerator,
{
    let worker_root = validate_worker_root(config.worker_root())?;
    let lifecycle = LifecycleService::new(DispatchDeps {
        config: ServiceConfig::new(worker_root, config.event_capacity),
        registry: Arc::new(Mutex::new(Registry::in_memory_with_event_capacity(
            config.event_capacity,
        ))),
        launcher,
        command_sender,
        id_generator,
    });

    Ok(ServiceBundle {
        lifecycle,
        bind_addr: config.bind_addr,
        bound_listener: false,
        wiring: BundleWiring::default(),
    })
}

pub fn build_process_service<S>(
    config: ServerConfig,
    resolver: WorkerBinaryResolver,
    worker_binary: WorkerBinaryConfig,
    spawner: S,
) -> Result<ServiceBundle, ServerBuildError>
where
    S: ProcessSpawner,
{
    let worker_root = validate_worker_root(config.worker_root())?;
    let registry = Arc::new(Mutex::new(Registry::in_memory_with_event_capacity(
        config.event_capacity,
    )));
    let session_manager = SessionManager::new(WorkerSessionConfig {
        registry: registry.clone(),
        attach_deadline: DEFAULT_ATTACH_DEADLINE,
    });
    let command_sender = SessionCommandSender::new(session_manager.clone());
    let binary = resolver.resolve(worker_binary)?;
    let launcher = ProcessWorkerLauncher::new(
        session_manager,
        spawner,
        binary.path().to_path_buf(),
        config.bind_addr.to_string(),
    );
    let lifecycle = LifecycleService::new(DispatchDeps {
        config: ServiceConfig::new(worker_root, config.event_capacity),
        registry,
        launcher,
        command_sender,
        id_generator: TimestampIdGenerator,
    });

    Ok(ServiceBundle {
        lifecycle,
        bind_addr: config.bind_addr,
        bound_listener: false,
        wiring: BundleWiring {
            process_worker_launcher: true,
            session_command_sender: true,
        },
    })
}

pub fn build_production_service(config: ServerConfig) -> Result<ServiceBundle, ServerBuildError> {
    build_process_service(
        config,
        WorkerBinaryResolver::current_exe()?,
        WorkerBinaryConfig::default(),
        OsProcessSpawner,
    )
}
