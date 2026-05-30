use lifecycle::proto::doric::lifecycle::v1;

#[test]
fn proto_namespace_exposes_cli_facing_messages() {
    type DispatchRequest = v1::DispatchRequest;
    type DispatchResponse = v1::DispatchResponse;
    type ListWorkersRequest = v1::ListWorkersRequest;
    type ListWorkersResponse = v1::ListWorkersResponse;
    type StreamWorkerRequest = v1::StreamWorkerRequest;
    type AnswerInputRequest = v1::AnswerInputRequest;
    type AnswerInputResponse = v1::AnswerInputResponse;
    type WorkerSummary = v1::WorkerSummary;
    type WorkerEvent = v1::WorkerEvent;
    type InputRequest = v1::InputRequest;
    type JobMode = v1::JobMode;
    type WorkerStatus = v1::WorkerStatus;

    assert_type::<DispatchRequest>();
    assert_type::<DispatchResponse>();
    assert_type::<ListWorkersRequest>();
    assert_type::<ListWorkersResponse>();
    assert_type::<StreamWorkerRequest>();
    assert_type::<AnswerInputRequest>();
    assert_type::<AnswerInputResponse>();
    assert_type::<WorkerSummary>();
    assert_type::<WorkerEvent>();
    assert_type::<InputRequest>();
    assert_type::<JobMode>();
    assert_type::<WorkerStatus>();
}

#[test]
fn proto_namespace_exposes_worker_session_messages() {
    type WorkerFrame = v1::WorkerFrame;
    type DaemonFrame = v1::DaemonFrame;
    type WorkerHello = v1::WorkerHello;
    type WorkerStatusUpdate = v1::WorkerStatusUpdate;
    type StartJob = v1::StartJob;
    type AnswerInputCommand = v1::AnswerInputCommand;
    type ShutdownWorker = v1::ShutdownWorker;

    assert_type::<WorkerFrame>();
    assert_type::<DaemonFrame>();
    assert_type::<WorkerHello>();
    assert_type::<WorkerStatusUpdate>();
    assert_type::<StartJob>();
    assert_type::<AnswerInputCommand>();
    assert_type::<ShutdownWorker>();
}

#[test]
fn proto_namespace_exposes_generated_service_modules() {
    type LifecycleClient<T> = v1::lifecycle_service_client::LifecycleServiceClient<T>;
    type LifecycleServer<T> = v1::lifecycle_service_server::LifecycleServiceServer<T>;
    type WorkerSessionClient<T> = v1::worker_session_service_client::WorkerSessionServiceClient<T>;
    type WorkerSessionServer<T> = v1::worker_session_service_server::WorkerSessionServiceServer<T>;

    assert_generic_type::<LifecycleClient<()>>();
    assert_lifecycle_service_trait::<LifecycleServiceMarker>();
    assert_generic_type::<LifecycleServer<()>>();
    assert_generic_type::<WorkerSessionClient<()>>();
    assert_worker_session_service_trait::<WorkerSessionServiceMarker>();
    assert_generic_type::<WorkerSessionServer<()>>();
}

#[test]
fn proto_frames_expose_expected_oneof_variants() {
    let _: fn(v1::WorkerHello) -> v1::worker_frame::Frame = v1::worker_frame::Frame::Hello;
    let _: fn(v1::WorkerStatusUpdate) -> v1::worker_frame::Frame = v1::worker_frame::Frame::Status;
    let _: fn(v1::WorkerEvent) -> v1::worker_frame::Frame = v1::worker_frame::Frame::Event;

    let _: fn(v1::StartJob) -> v1::daemon_frame::Frame = v1::daemon_frame::Frame::StartJob;
    let _: fn(v1::AnswerInputCommand) -> v1::daemon_frame::Frame = v1::daemon_frame::Frame::Answer;
    let _: fn(v1::ShutdownWorker) -> v1::daemon_frame::Frame = v1::daemon_frame::Frame::Shutdown;
}

fn assert_type<T>() {}

fn assert_generic_type<T>() {}

fn assert_lifecycle_service_trait<T>()
where
    T: v1::lifecycle_service_server::LifecycleService,
{
}

fn assert_worker_session_service_trait<T>()
where
    T: v1::worker_session_service_server::WorkerSessionService,
{
}

struct LifecycleServiceMarker;

#[tonic::async_trait]
impl v1::lifecycle_service_server::LifecycleService for LifecycleServiceMarker {
    type StreamWorkerStream = std::pin::Pin<
        Box<
            dyn tonic::codegen::tokio_stream::Stream<
                    Item = Result<v1::WorkerEvent, tonic::Status>,
                > + Send,
        >,
    >;

    async fn dispatch(
        &self,
        _: tonic::Request<v1::DispatchRequest>,
    ) -> Result<tonic::Response<v1::DispatchResponse>, tonic::Status> {
        unimplemented!()
    }

    async fn list_workers(
        &self,
        _: tonic::Request<v1::ListWorkersRequest>,
    ) -> Result<tonic::Response<v1::ListWorkersResponse>, tonic::Status> {
        unimplemented!()
    }

    async fn stream_worker(
        &self,
        _: tonic::Request<v1::StreamWorkerRequest>,
    ) -> Result<tonic::Response<Self::StreamWorkerStream>, tonic::Status> {
        unimplemented!()
    }

    async fn answer_input(
        &self,
        _: tonic::Request<v1::AnswerInputRequest>,
    ) -> Result<tonic::Response<v1::AnswerInputResponse>, tonic::Status> {
        unimplemented!()
    }
}

struct WorkerSessionServiceMarker;

#[tonic::async_trait]
impl v1::worker_session_service_server::WorkerSessionService for WorkerSessionServiceMarker {
    type SessionStream = std::pin::Pin<
        Box<
            dyn tonic::codegen::tokio_stream::Stream<
                    Item = Result<v1::DaemonFrame, tonic::Status>,
                > + Send,
        >,
    >;

    async fn session(
        &self,
        _: tonic::Request<tonic::Streaming<v1::WorkerFrame>>,
    ) -> Result<tonic::Response<Self::SessionStream>, tonic::Status> {
        unimplemented!()
    }
}
