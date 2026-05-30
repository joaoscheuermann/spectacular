use std::collections::VecDeque;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use lifecycle::identity::RequestId;
use lifecycle::proto::doric::lifecycle::v1 as pb;
use worker::agents::prompt::PromptAgentEvent;
use worker::error::{WorkerError, WorkerResult};
use worker::runtime::{
    run_worker_session, DaemonSession, PreparedRuntimeRepo, PromptRunState, RepoPreparer,
    RuntimeAnswer, RuntimeConfig, RuntimeDeps, RuntimeLayout, RuntimeOutcome, RuntimePromptJob,
    RuntimePromptRunner, RuntimeRepoRequest,
};
use worker::state::WorkerId;

pub(super) fn started() -> PromptAgentEvent {
    PromptAgentEvent::started("Prompt requirements agent started")
}

pub(super) fn request_id(value: &str) -> RequestId {
    value.parse().unwrap()
}

fn worker_id(value: &str) -> WorkerId {
    value.parse().unwrap()
}

pub(super) fn assert_worker_hello(frame: &pb::WorkerFrame, worker_id: &str, token: &str) {
    match frame.frame.as_ref() {
        Some(pb::worker_frame::Frame::Hello(hello)) => {
            assert_eq!(hello.worker_id, worker_id);
            assert_eq!(hello.token, token);
        }
        other => panic!("expected WorkerHello frame, got {other:?}"),
    }
}

pub(super) fn assert_status_sent(frames: &[pb::WorkerFrame], expected: pb::WorkerStatus) {
    assert!(
        status_updates(frames)
            .iter()
            .any(|status| status.status == expected as i32),
        "expected status {expected:?} in {frames:?}"
    );
}

pub(super) fn assert_status_not_sent(frames: &[pb::WorkerFrame], expected: pb::WorkerStatus) {
    assert!(
        status_updates(frames)
            .iter()
            .all(|status| status.status != expected as i32),
        "did not expect status {expected:?} in {frames:?}"
    );
}

pub(super) fn terminal_status_count(frames: &[pb::WorkerFrame]) -> usize {
    status_updates(frames)
        .into_iter()
        .filter(|status| {
            matches!(
                pb::WorkerStatus::try_from(status.status),
                Ok(pb::WorkerStatus::Succeeded
                    | pb::WorkerStatus::Failed
                    | pb::WorkerStatus::Stopped)
            )
        })
        .count()
}

pub(super) fn assert_no_status_after_terminal(frames: &[pb::WorkerFrame]) {
    let mut terminal_seen = false;

    for status in status_updates(frames) {
        let parsed =
            pb::WorkerStatus::try_from(status.status).unwrap_or(pb::WorkerStatus::Unspecified);
        let is_terminal = matches!(
            parsed,
            pb::WorkerStatus::Succeeded | pb::WorkerStatus::Failed | pb::WorkerStatus::Stopped
        );

        assert!(
            !terminal_seen,
            "no status should follow terminal status in {frames:?}"
        );
        terminal_seen = is_terminal;
    }
}

pub(super) fn assert_monotonic_sequences(sequences: &[u64]) {
    for pair in sequences.windows(2) {
        assert!(
            pair[0] < pair[1],
            "event sequences should increase: {sequences:?}"
        );
    }
}

pub(super) fn assert_no_frame_text_contains(frames: &[pb::WorkerFrame], secret: &str) {
    assert!(
        !format!("{frames:?}").contains(secret),
        "`{secret}` leaked in {frames:?}"
    );
}

pub(super) fn assert_no_status_or_event_text_contains(frames: &[pb::WorkerFrame], secret: &str) {
    for frame in frames {
        match frame.frame.as_ref() {
            Some(pb::worker_frame::Frame::Status(status)) => {
                assert!(
                    !status.message.contains(secret),
                    "`{secret}` leaked in {status:?}"
                );
            }
            Some(pb::worker_frame::Frame::Event(event)) => {
                assert!(
                    !event.message.contains(secret),
                    "`{secret}` leaked in {event:?}"
                );
            }
            _ => {}
        }
    }
}

pub(super) fn status_or_event_messages(frames: &[pb::WorkerFrame]) -> Vec<String> {
    frames
        .iter()
        .filter_map(|frame| match frame.frame.as_ref() {
            Some(pb::worker_frame::Frame::Status(status)) => Some(status.message.clone()),
            Some(pb::worker_frame::Frame::Event(event)) => Some(event.message.clone()),
            _ => None,
        })
        .collect()
}

fn status_updates(frames: &[pb::WorkerFrame]) -> Vec<&pb::WorkerStatusUpdate> {
    frames
        .iter()
        .filter_map(|frame| match frame.frame.as_ref() {
            Some(pb::worker_frame::Frame::Status(status)) => Some(status),
            _ => None,
        })
        .collect()
}

pub(super) struct RuntimeFixture {
    temp: TempRoot,
    id: &'static str,
    pub(super) layout: RuntimeLayout,
    pub(super) session: FakeDaemonSession,
    pub(super) repo: FakeRepoPreparer,
    pub(super) prompt: FakePromptRunner,
}

impl RuntimeFixture {
    pub(super) fn new(id: &'static str) -> Self {
        let temp = TempRoot::new(id);
        let layout = runtime_layout(temp.path(), id);
        Self {
            temp,
            id,
            layout: layout.clone(),
            session: FakeDaemonSession::new(),
            repo: FakeRepoPreparer::success(layout),
            prompt: FakePromptRunner::new([]),
        }
    }

    pub(super) fn with_start(mut self, repo: &str) -> Self {
        self.session
            .push(daemon_start(start_job(self.id, repo, &self.layout)));
        self
    }

    pub(super) fn with_answer(mut self, request_id: &RequestId, text: &str) -> Self {
        self.session.push(daemon_answer(request_id, text));
        self
    }

    pub(super) fn with_shutdown(mut self, reason: &str) -> Self {
        self.session.push(daemon_shutdown(reason));
        self
    }

    pub(super) fn with_repo_failure(mut self, error: WorkerError) -> Self {
        self.repo = FakeRepoPreparer::failure(error);
        self
    }

    pub(super) fn with_prompt(mut self, states: impl IntoIterator<Item = PromptRunState>) -> Self {
        self.prompt = FakePromptRunner::new(states);
        self
    }

    pub(super) fn run(&mut self) -> WorkerResult<RuntimeOutcome> {
        self.run_with_token("one-time-token")
    }

    pub(super) fn run_with_token(&mut self, token: &str) -> WorkerResult<RuntimeOutcome> {
        run_worker_session(
            RuntimeConfig::new(worker_id(self.id), token, self.temp.path().join("workers")),
            RuntimeDeps::new(&mut self.session, &mut self.repo, &mut self.prompt),
        )
    }

    pub(super) fn frames(&self) -> &[pb::WorkerFrame] {
        self.session.frames()
    }
}

fn runtime_layout(root: &Path, worker_id: &str) -> RuntimeLayout {
    let worker_root = root.join("workers").join(worker_id);
    RuntimeLayout::new(
        worker_root.clone(),
        worker_root.join("repo"),
        worker_root.join("state"),
        worker_root.join("artifacts"),
        worker_root.join("tool-output"),
    )
}

fn start_job(worker_id: &str, repo: &str, layout: &RuntimeLayout) -> pb::StartJob {
    pb::StartJob {
        worker_id: worker_id.to_owned(),
        mode: pb::JobMode::Feature as i32,
        prompt: "Gather prompt and architecture requirements".to_owned(),
        repo: repo.to_owned(),
        repo_dir: layout.repo().to_string_lossy().into_owned(),
        state_dir: layout.state().to_string_lossy().into_owned(),
        artifacts_dir: layout.artifacts().to_string_lossy().into_owned(),
        tool_output_dir: layout.tool_output().to_string_lossy().into_owned(),
    }
}

fn daemon_start(start_job: pb::StartJob) -> pb::DaemonFrame {
    pb::DaemonFrame {
        frame: Some(pb::daemon_frame::Frame::StartJob(start_job)),
    }
}

fn daemon_answer(request_id: &RequestId, text: &str) -> pb::DaemonFrame {
    pb::DaemonFrame {
        frame: Some(pb::daemon_frame::Frame::Answer(pb::AnswerInputCommand {
            request_id: request_id.as_str().to_owned(),
            text: text.to_owned(),
        })),
    }
}

fn daemon_shutdown(reason: &str) -> pb::DaemonFrame {
    pb::DaemonFrame {
        frame: Some(pb::daemon_frame::Frame::Shutdown(pb::ShutdownWorker {
            reason: reason.to_owned(),
        })),
    }
}

pub(super) struct FakeDaemonSession {
    inbound: VecDeque<pb::DaemonFrame>,
    outbound: Vec<pb::WorkerFrame>,
}

impl FakeDaemonSession {
    fn new() -> Self {
        Self {
            inbound: VecDeque::new(),
            outbound: Vec::new(),
        }
    }

    fn push(&mut self, frame: pb::DaemonFrame) {
        self.inbound.push_back(frame);
    }

    fn frames(&self) -> &[pb::WorkerFrame] {
        &self.outbound
    }

    pub(super) fn event_names(&self) -> Vec<String> {
        self.outbound
            .iter()
            .filter_map(|frame| match frame.frame.as_ref() {
                Some(pb::worker_frame::Frame::Event(event)) => Some(event.name.clone()),
                _ => None,
            })
            .collect()
    }

    pub(super) fn prompt_event_names(&self) -> Vec<String> {
        self.event_names()
            .into_iter()
            .filter(|name| name.starts_with("prompt_") || name == "waiting_for_input")
            .collect()
    }

    pub(super) fn event_sequences(&self) -> Vec<u64> {
        self.outbound
            .iter()
            .filter_map(|frame| match frame.frame.as_ref() {
                Some(pb::worker_frame::Frame::Event(event)) => Some(event.sequence),
                _ => None,
            })
            .collect()
    }
}

impl DaemonSession for FakeDaemonSession {
    fn send(&mut self, frame: pb::WorkerFrame) -> WorkerResult<()> {
        self.outbound.push(frame);
        Ok(())
    }

    fn receive(&mut self) -> WorkerResult<Option<pb::DaemonFrame>> {
        Ok(self.inbound.pop_front())
    }
}

pub(super) struct FakeRepoPreparer {
    outcome: Option<WorkerResult<PreparedRuntimeRepo>>,
    requests: Vec<RuntimeRepoRequest>,
}

impl FakeRepoPreparer {
    fn success(layout: RuntimeLayout) -> Self {
        Self {
            outcome: Some(Ok(PreparedRuntimeRepo::new(layout))),
            requests: Vec::new(),
        }
    }

    fn failure(error: WorkerError) -> Self {
        Self {
            outcome: Some(Err(error)),
            requests: Vec::new(),
        }
    }

    pub(super) fn requests(&self) -> &[RuntimeRepoRequest] {
        &self.requests
    }
}

impl RepoPreparer for FakeRepoPreparer {
    fn prepare(&mut self, request: RuntimeRepoRequest) -> WorkerResult<PreparedRuntimeRepo> {
        self.requests.push(request);
        self.outcome
            .take()
            .unwrap_or_else(|| Err(WorkerError::provider_setup_failed("repo prepared twice")))
    }
}

pub(super) struct FakePromptRunner {
    states: VecDeque<PromptRunState>,
    jobs: Vec<RuntimePromptJob>,
    answers: Vec<(String, String)>,
    stop_reasons: Vec<String>,
}

impl FakePromptRunner {
    fn new(states: impl IntoIterator<Item = PromptRunState>) -> Self {
        Self {
            states: states.into_iter().collect(),
            jobs: Vec::new(),
            answers: Vec::new(),
            stop_reasons: Vec::new(),
        }
    }

    pub(super) fn jobs(&self) -> &[RuntimePromptJob] {
        &self.jobs
    }

    pub(super) fn answers(&self) -> Vec<(String, String)> {
        self.answers.clone()
    }

    pub(super) fn stop_reasons(&self) -> Vec<String> {
        self.stop_reasons.clone()
    }

    fn next_state(&mut self) -> WorkerResult<PromptRunState> {
        self.states
            .pop_front()
            .ok_or_else(|| WorkerError::provider_setup_failed("fake prompt state exhausted"))
    }
}

impl RuntimePromptRunner for FakePromptRunner {
    fn start(&mut self, job: RuntimePromptJob) -> WorkerResult<PromptRunState> {
        self.jobs.push(job);
        self.next_state()
    }

    fn resume(&mut self, answer: RuntimeAnswer) -> WorkerResult<PromptRunState> {
        self.answers.push((
            answer.request_id().as_str().to_owned(),
            answer.text().to_owned(),
        ));
        self.next_state()
    }

    fn stop(&mut self, reason: &str) -> WorkerResult<PromptRunState> {
        self.stop_reasons.push(reason.to_owned());
        Ok(PromptRunState::stopped([PromptAgentEvent::failed(
            "Prompt requirements agent stopped",
        )]))
    }
}

struct TempRoot {
    path: PathBuf,
}

impl TempRoot {
    fn new(name: &str) -> Self {
        let path = std::env::temp_dir().join(format!("doric-worker-runtime-{name}-{}", suffix()));
        fs::create_dir_all(path.join("workers")).unwrap();
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
