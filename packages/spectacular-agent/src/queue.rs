use std::collections::VecDeque;
use std::sync::Mutex;
use tokio::sync::oneshot;

/// Stable identifier assigned to an enqueued or active run.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RunId(usize);

impl RunId {
    /// Returns the numeric run identifier.
    pub fn value(self) -> usize {
        self.0
    }
}

/// Prompt request selected by the run queue for execution.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RunRequest {
    id: RunId,
    prompt: String,
    prompt_event_id: Option<String>,
}

impl RunRequest {
    fn new(id: RunId, prompt: impl Into<String>, prompt_event_id: Option<String>) -> Self {
        Self {
            id,
            prompt: prompt.into(),
            prompt_event_id,
        }
    }

    /// Returns the queue-assigned run identifier.
    pub fn id(&self) -> RunId {
        self.id
    }

    /// Returns the prompt text to execute.
    pub fn prompt(&self) -> &str {
        &self.prompt
    }

    /// Returns the caller-owned prompt event ID, when one was provided.
    pub fn prompt_event_id(&self) -> Option<&str> {
        self.prompt_event_id.as_deref()
    }
}

/// Coordinates manual and immediately awaited agent run requests.
#[derive(Debug, Default)]
pub struct RunQueue {
    state: Mutex<QueueState>,
}

#[derive(Debug, Default)]
struct QueueState {
    next_id: usize,
    active: bool,
    rejecting: bool,
    manual: VecDeque<RunRequest>,
    waiting: VecDeque<WaitingRun>,
}

#[derive(Debug)]
struct WaitingRun {
    request: RunRequest,
    ready: oneshot::Sender<Result<RunRequest, ()>>,
}

impl RunQueue {
    /// Adds a prompt to the manual queue and returns its assigned run ID.
    pub fn enqueue_prompt(&self, prompt: impl Into<String>) -> RunId {
        self.enqueue_prompt_with_event_id(prompt, None::<String>)
    }

    /// Adds a prompt with a caller-owned prompt event ID to the manual queue.
    pub fn enqueue_prompt_with_event_id(
        &self,
        prompt: impl Into<String>,
        prompt_event_id: Option<impl Into<String>>,
    ) -> RunId {
        let mut state = self.state.lock().unwrap();
        let request = next_request(&mut state, prompt, prompt_event_id);
        let id = request.id();
        state.manual.push_back(request);
        id
    }

    /// Starts the next manually queued run when no run is active.
    pub async fn start_next(&self) -> Option<RunRequest> {
        let mut state = self.state.lock().unwrap();
        if state.active || state.rejecting {
            return None;
        }

        let request = state.manual.pop_front()?;
        state.active = true;
        Some(request)
    }

    /// Enqueues a prompt and waits until it becomes the active request.
    pub async fn enqueue_and_wait(&self, prompt: impl Into<String>) -> Result<RunRequest, ()> {
        self.enqueue_and_wait_with_event_id(prompt, None::<String>)
            .await
    }

    /// Enqueues a prompt with a caller-owned prompt event ID and waits for activation.
    pub async fn enqueue_and_wait_with_event_id(
        &self,
        prompt: impl Into<String>,
        prompt_event_id: Option<impl Into<String>>,
    ) -> Result<RunRequest, ()> {
        let receiver = {
            let mut state = self.state.lock().unwrap();
            if state.rejecting {
                return Err(());
            }

            let request = next_request(&mut state, prompt, prompt_event_id);
            if !state.active {
                state.active = true;
                return Ok(request);
            }

            let (sender, receiver) = oneshot::channel();
            state.waiting.push_back(WaitingRun {
                request,
                ready: sender,
            });
            receiver
        };

        receiver.await.unwrap_or(Err(()))
    }

    /// Marks the active run as finished and wakes the next waiting request.
    pub async fn finish_active(&self) {
        let next = {
            let mut state = self.state.lock().unwrap();
            let Some(next) = state.waiting.pop_front() else {
                state.active = false;
                return;
            };
            next
        };
        let _ = next.ready.send(Ok(next.request));
    }

    /// Marks the active run as cancelled and releases pending waiters.
    pub async fn finish_cancelled_active(&self) {
        let mut state = self.state.lock().unwrap();
        state.active = false;
        state.rejecting = false;
        cancel_waiting(&mut state);
    }

    /// Cancels pending runs asynchronously.
    pub async fn cancel_pending(&self) {
        self.cancel_pending_now();
    }

    /// Cancels pending runs without awaiting.
    pub fn cancel_pending_now(&self) {
        let mut state = self.state.lock().unwrap();
        state.rejecting = true;
        cancel_waiting(&mut state);
    }
}

fn next_request(
    state: &mut QueueState,
    prompt: impl Into<String>,
    prompt_event_id: Option<impl Into<String>>,
) -> RunRequest {
    let id = RunId(state.next_id);
    state.next_id += 1;
    RunRequest::new(id, prompt, prompt_event_id.map(Into::into))
}

fn cancel_waiting(state: &mut QueueState) {
    state.manual.clear();
    for waiting in state.waiting.drain(..) {
        let _ = waiting.ready.send(Err(()));
    }
}
