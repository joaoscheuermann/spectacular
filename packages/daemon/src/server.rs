use std::error::Error;
use std::fmt::{self, Display};
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::pin::Pin;
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
    CommandSender, DispatchDeps, IdGenerator, LifecycleService as CoreLifecycleService,
    NoopLifecycleLogger, ServiceConfig, TerminalLifecycleLogger, UuidV6IdGenerator, WorkerLauncher,
};
use crate::worker_session::{SessionCommandSender, SessionManager, WorkerSessionConfig};
use lifecycle::proto::doric::lifecycle::v1 as pb;
use tokio::net::TcpListener;
use tokio::sync::mpsc;
use tokio_stream::wrappers::{ReceiverStream, TcpListenerStream};
use tonic::transport::Server;

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
    lifecycle: CoreLifecycleService,
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

    pub fn lifecycle(&self) -> &CoreLifecycleService {
        &self.lifecycle
    }

    pub fn lifecycle_mut(&mut self) -> &mut CoreLifecycleService {
        &mut self.lifecycle
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

    pub fn uses_terminal_lifecycle_logger(&self) -> bool {
        self.wiring.terminal_lifecycle_logger
    }

    pub fn uses_noop_lifecycle_logger(&self) -> bool {
        self.wiring.noop_lifecycle_logger
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct BundleWiring {
    process_worker_launcher: bool,
    session_command_sender: bool,
    terminal_lifecycle_logger: bool,
    noop_lifecycle_logger: bool,
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
    let lifecycle = CoreLifecycleService::new(DispatchDeps {
        config: ServiceConfig::new(worker_root, config.event_capacity),
        registry: Arc::new(Mutex::new(Registry::in_memory_with_event_capacity(
            config.event_capacity,
        ))),
        launcher,
        command_sender,
        id_generator,
        lifecycle_logger: NoopLifecycleLogger,
    });

    Ok(ServiceBundle {
        lifecycle,
        bind_addr: config.bind_addr,
        bound_listener: false,
        wiring: BundleWiring {
            noop_lifecycle_logger: true,
            ..BundleWiring::default()
        },
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
    let lifecycle_logger = TerminalLifecycleLogger::stderr();
    let session_manager = SessionManager::new(WorkerSessionConfig {
        registry: registry.clone(),
        attach_deadline: DEFAULT_ATTACH_DEADLINE,
        lifecycle_logger: lifecycle_logger.clone(),
    });
    let command_sender = SessionCommandSender::new(session_manager.clone());
    let binary = resolver.resolve(worker_binary)?;
    let launcher = ProcessWorkerLauncher::new(
        session_manager,
        spawner,
        binary.path().to_path_buf(),
        config.bind_addr.to_string(),
    );
    let lifecycle = CoreLifecycleService::new(DispatchDeps {
        config: ServiceConfig::new(worker_root, config.event_capacity),
        registry,
        launcher,
        command_sender,
        id_generator: UuidV6IdGenerator::default(),
        lifecycle_logger,
    });

    Ok(ServiceBundle {
        lifecycle,
        bind_addr: config.bind_addr,
        bound_listener: false,
        wiring: BundleWiring {
            process_worker_launcher: true,
            session_command_sender: true,
            terminal_lifecycle_logger: true,
            noop_lifecycle_logger: false,
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

#[derive(Debug)]
pub struct ServerRunError {
    message: String,
    source: Option<Box<dyn Error + Send + Sync>>,
}

impl ServerRunError {
    fn new(message: impl Into<String>, source: impl Error + Send + Sync + 'static) -> Self {
        Self {
            message: message.into(),
            source: Some(Box::new(source)),
        }
    }
}

impl Display for ServerRunError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl Error for ServerRunError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        self.source
            .as_deref()
            .map(|source| source as &(dyn Error + 'static))
    }
}

impl From<ServerBuildError> for ServerRunError {
    fn from(source: ServerBuildError) -> Self {
        Self {
            message: source.to_string(),
            source: Some(Box::new(source)),
        }
    }
}

pub async fn serve_production(config: ServerConfig) -> Result<(), ServerRunError> {
    let bundle = build_production_service(config)?;
    serve_bundle(bundle).await
}

pub fn run_production(config: ServerConfig) -> Result<(), ServerRunError> {
    match tokio::runtime::Handle::try_current() {
        Ok(handle) => tokio::task::block_in_place(|| handle.block_on(serve_production(config))),
        Err(_) => tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .map_err(|source| ServerRunError::new("failed to start daemon runtime", source))?
            .block_on(serve_production(config)),
    }
}

pub async fn serve_bundle(bundle: ServiceBundle) -> Result<(), ServerRunError> {
    let listener = TcpListener::bind(bundle.bind_addr)
        .await
        .map_err(|source| {
            ServerRunError::new(
                format!("failed to bind daemon listener at {}", bundle.bind_addr),
                source,
            )
        })?;

    serve_bundle_on_listener(bundle, listener).await
}

pub async fn serve_bundle_on_listener(
    bundle: ServiceBundle,
    listener: TcpListener,
) -> Result<(), ServerRunError> {
    serve_bundle_on_listener_with_shutdown(bundle, listener, std::future::pending::<()>()).await
}

pub async fn serve_bundle_on_listener_with_shutdown<F>(
    bundle: ServiceBundle,
    listener: TcpListener,
    shutdown: F,
) -> Result<(), ServerRunError>
where
    F: std::future::Future<Output = ()> + Send + 'static,
{
    Server::builder()
        .add_service(pb::lifecycle_service_server::LifecycleServiceServer::new(
            TonicLifecycleService::new(bundle.lifecycle),
        ))
        .serve_with_incoming_shutdown(TcpListenerStream::new(listener), shutdown)
        .await
        .map_err(|source| ServerRunError::new("daemon server stopped with an error", source))
}

struct TonicLifecycleService {
    inner: CoreLifecycleService,
}

impl TonicLifecycleService {
    fn new(inner: CoreLifecycleService) -> Self {
        Self { inner }
    }
}

#[tonic::async_trait]
impl pb::lifecycle_service_server::LifecycleService for TonicLifecycleService {
    type StreamWorkerStream = Pin<
        Box<
            dyn tonic::codegen::tokio_stream::Stream<Item = Result<pb::WorkerEvent, tonic::Status>>
                + Send,
        >,
    >;

    async fn dispatch(
        &self,
        request: tonic::Request<pb::DispatchRequest>,
    ) -> Result<tonic::Response<pb::DispatchResponse>, tonic::Status> {
        self.inner
            .dispatch(request.into_inner())
            .map(tonic::Response::new)
            .map_err(status_from_daemon)
    }

    async fn list_workers(
        &self,
        request: tonic::Request<pb::ListWorkersRequest>,
    ) -> Result<tonic::Response<pb::ListWorkersResponse>, tonic::Status> {
        self.inner
            .list(request.into_inner())
            .map(tonic::Response::new)
            .map_err(status_from_daemon)
    }

    async fn stream_worker(
        &self,
        request: tonic::Request<pb::StreamWorkerRequest>,
    ) -> Result<tonic::Response<Self::StreamWorkerStream>, tonic::Status> {
        let stream = self
            .inner
            .stream(request.into_inner())
            .map_err(status_from_daemon)?;
        let (sender, receiver) = mpsc::channel(32);

        tokio::task::spawn_blocking(move || {
            let mut terminal = false;

            for event in stream.replay().iter().cloned() {
                terminal = terminal || is_terminal_event(&event);

                if sender.blocking_send(Ok(event)).is_err() || terminal {
                    return;
                }
            }

            while let Some(event) = stream.next_blocking() {
                terminal = is_terminal_event(&event);

                if sender.blocking_send(Ok(event)).is_err() || terminal {
                    return;
                }
            }
        });

        Ok(tonic::Response::new(Box::pin(ReceiverStream::new(
            receiver,
        ))))
    }

    async fn answer_input(
        &self,
        request: tonic::Request<pb::AnswerInputRequest>,
    ) -> Result<tonic::Response<pb::AnswerInputResponse>, tonic::Status> {
        self.inner
            .answer_input(request.into_inner())
            .map(tonic::Response::new)
            .map_err(status_from_daemon)
    }
}

fn status_from_daemon(error: DaemonError) -> tonic::Status {
    let message = error.to_string();

    match error {
        DaemonError::RootConfiguration { .. } => tonic::Status::invalid_argument(message),
        DaemonError::DuplicateWorker { .. } => tonic::Status::already_exists(message),
        DaemonError::UnknownWorker { .. } => tonic::Status::not_found(message),
        DaemonError::StaleRequest { .. } | DaemonError::NoPendingInput { .. } => {
            tonic::Status::not_found(message)
        }
        DaemonError::TerminalWorker { .. }
        | DaemonError::DuplicateAnswer { .. }
        | DaemonError::NotWaitingForInput { .. } => tonic::Status::failed_precondition(message),
    }
}

fn is_terminal_event(event: &pb::WorkerEvent) -> bool {
    matches!(event.name.as_str(), "failed" | "succeeded" | "stopped")
}
