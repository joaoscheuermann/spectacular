use spectacular_agent::RunQueue;
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
