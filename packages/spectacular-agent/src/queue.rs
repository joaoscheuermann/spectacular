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
}

impl RunRequest {
    fn new(id: RunId, prompt: impl Into<String>) -> Self {
        Self {
            id,
            prompt: prompt.into(),
        }
    }

    pub fn id(&self) -> RunId {
        self.id
    }

    pub fn prompt(&self) -> &str {
        &self.prompt
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
        let mut state = self.state.lock().unwrap();
        let request = next_request(&mut state, prompt);
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
        let receiver = {
            let mut state = self.state.lock().unwrap();
            if state.rejecting {
                return Err(());
            }

            let request = next_request(&mut state, prompt);
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

fn next_request(state: &mut QueueState, prompt: impl Into<String>) -> RunRequest {
    let id = RunId(state.next_id);
    state.next_id += 1;
    RunRequest::new(id, prompt)
}

fn cancel_waiting(state: &mut QueueState) {
    state.manual.clear();
    for waiting in state.waiting.drain(..) {
        let _ = waiting.ready.send(Err(()));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[tokio::test]
    async fn manual_queue_starts_enqueued_runs_in_fifo_order() {
        let queue = RunQueue::default();
        let first = queue.enqueue_prompt("first");
        let second = queue.enqueue_prompt("second");

        let active = queue.start_next().await.unwrap();
        assert_eq!(active.id(), first);
        assert_eq!(active.prompt(), "first");

        queue.finish_active().await;
        let active = queue.start_next().await.unwrap();
        assert_eq!(active.id(), second);
        assert_eq!(active.prompt(), "second");
    }

    #[tokio::test]
    async fn concurrent_waiters_resume_in_fifo_order() {
        let queue = Arc::new(RunQueue::default());
        let first = queue.enqueue_and_wait("first").await.unwrap();

        let second = tokio::spawn({
            let queue = Arc::clone(&queue);
            async move { queue.enqueue_and_wait("second").await }
        });
        let third = tokio::spawn({
            let queue = Arc::clone(&queue);
            async move { queue.enqueue_and_wait("third").await }
        });
        tokio::task::yield_now().await;

        assert_eq!(first.prompt(), "first");
        queue.finish_active().await;
        assert_eq!(second.await.unwrap().unwrap().prompt(), "second");
        queue.finish_active().await;
        assert_eq!(third.await.unwrap().unwrap().prompt(), "third");
    }

    #[tokio::test]
    async fn cancelling_pending_drops_waiters_and_manual_queue() {
        let queue = Arc::new(RunQueue::default());
        let active = queue.enqueue_and_wait("active").await.unwrap();
        let queued = tokio::spawn({
            let queue = Arc::clone(&queue);
            async move { queue.enqueue_and_wait("queued").await }
        });
        queue.enqueue_prompt("manual");
        tokio::task::yield_now().await;

        assert_eq!(active.prompt(), "active");
        queue.cancel_pending().await;
        assert!(queued.await.unwrap().is_err());
        queue.finish_cancelled_active().await;
        assert!(queue.start_next().await.is_none());
    }

    #[tokio::test]
    async fn late_waiter_during_rejection_fails_immediately() {
        let queue = RunQueue::default();
        let active = queue.enqueue_and_wait("active").await.unwrap();
        queue.cancel_pending().await;

        assert_eq!(active.prompt(), "active");
        assert!(queue.enqueue_and_wait("late").await.is_err());
    }
}
