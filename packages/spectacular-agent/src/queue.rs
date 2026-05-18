use std::collections::VecDeque;
use std::sync::Mutex;
use tokio::sync::oneshot;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RunId(usize);

impl RunId {
    pub fn value(self) -> usize {
        self.0
    }
}

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

    pub fn id(&self) -> RunId {
        self.id
    }

    pub fn prompt(&self) -> &str {
        &self.prompt
    }

    pub fn prompt_event_id(&self) -> Option<&str> {
        self.prompt_event_id.as_deref()
    }
}

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
    pub fn enqueue_prompt(&self, prompt: impl Into<String>) -> RunId {
        self.enqueue_prompt_with_event_id(prompt, None::<String>)
    }

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

    pub async fn start_next(&self) -> Option<RunRequest> {
        let mut state = self.state.lock().unwrap();
        if state.active || state.rejecting {
            return None;
        }

        let request = state.manual.pop_front()?;
        state.active = true;
        Some(request)
    }

    pub async fn enqueue_and_wait(&self, prompt: impl Into<String>) -> Result<RunRequest, ()> {
        self.enqueue_and_wait_with_event_id(prompt, None::<String>).await
    }

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

    pub async fn finish_cancelled_active(&self) {
        let mut state = self.state.lock().unwrap();
        state.active = false;
        state.rejecting = false;
        cancel_waiting(&mut state);
    }

    pub async fn cancel_pending(&self) {
        self.cancel_pending_now();
    }

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
