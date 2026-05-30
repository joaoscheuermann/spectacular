use std::str::FromStr;

use lifecycle::event::StreamEvent;
use lifecycle::identity::{RequestId, WorkerId};
use lifecycle::repo::RepoIdentity;
use lifecycle::status::WorkerStatus;

use crate::event::{RegistryEvent, ReplayItem};
use crate::registry::{InputAnswer, InputRequest, Registry, WorkerMode, WorkerRecord};

/// Verifies that a new in-memory registry has no synthetic workers.
#[test]
fn new_registry_empty_list_returns_empty_summaries() {
    let registry = Registry::in_memory();

    assert!(registry.list().is_empty());
}

/// Verifies that inserted workers list every CLI-visible summary field.
#[test]
fn insert_worker_valid_record_lists_summary_with_required_fields() {
    let mut registry = Registry::in_memory();
    let worker_id = worker_id("worker-summary");
    let repo = RepoIdentity::from_raw_url("https://token@example.com/org/repo.git").unwrap();

    registry
        .insert(WorkerRecord::new(
            worker_id.clone(),
            WorkerMode::Feature,
            repo.clone(),
            WorkerStatus::Accepted,
            "queued",
        ))
        .unwrap();

    let summaries = registry.list();

    assert_eq!(summaries.len(), 1);
    let summary = &summaries[0];
    assert_eq!(summary.id(), &worker_id);
    assert_eq!(summary.mode(), WorkerMode::Feature);
    assert_eq!(summary.repo(), &repo);
    assert_eq!(summary.status(), WorkerStatus::Accepted);
    assert_eq!(summary.activity(), Some("queued"));
    assert_eq!(summary.terminal_reason(), None);
    assert_eq!(summary.last_sequence(), None);
}

/// Verifies that duplicate worker ids cannot corrupt registry state.
#[test]
fn insert_worker_duplicate_id_rejects_record() {
    let mut registry = seeded_registry("duplicate-worker");
    let duplicate = record(
        "duplicate-worker",
        WorkerMode::Debug,
        WorkerStatus::Accepted,
        "queued",
    );

    let error = registry.insert(duplicate).unwrap_err();

    assert!(error.is_duplicate_worker());
    assert_eq!(registry.list().len(), 1);
}

/// Verifies that status updates are reflected in summaries with terminal reason.
#[test]
fn update_status_known_worker_records_current_activity() {
    let mut registry = seeded_registry("status-worker");
    let worker_id = worker_id("status-worker");

    registry
        .update_status(
            &worker_id,
            WorkerStatus::Failed,
            "repo preparation failed: missing remote",
        )
        .unwrap();

    let summary = registry.list().into_iter().next().unwrap();
    assert_eq!(summary.status(), WorkerStatus::Failed);
    assert_eq!(
        summary.terminal_reason(),
        Some("repo preparation failed: missing remote")
    );
}

/// Verifies that unknown worker updates remain explicit unknown/untracked errors.
#[test]
fn update_status_unknown_worker_returns_unknown_worker_error() {
    let mut registry = Registry::in_memory();
    let worker_id = worker_id("missing-worker");

    let error = registry
        .update_status(&worker_id, WorkerStatus::Running, "starting")
        .unwrap_err();

    assert!(error.is_unknown_worker());
    assert!(error.to_string().contains("unknown") || error.to_string().contains("untracked"));
}

/// Verifies that events receive monotonic sequence numbers from zero.
#[test]
fn append_event_allocates_monotonic_sequences_from_zero() {
    let mut registry = seeded_registry("sequence-worker");
    let worker_id = worker_id("sequence-worker");

    let first = registry
        .append_event(&worker_id, RegistryEvent::accepted("accepted"))
        .unwrap();
    let second = registry
        .append_event(&worker_id, RegistryEvent::current_activity("running"))
        .unwrap();

    assert_eq!(first.sequence(), 0);
    assert_eq!(second.sequence(), 1);
}

/// Verifies that replay from zero returns retained worker events in sequence order.
#[test]
fn replay_events_from_zero_returns_retained_events_in_order() {
    let mut registry = seeded_registry("replay-worker");
    let worker_id = worker_id("replay-worker");
    append_events(
        &mut registry,
        &worker_id,
        [
            SampleEvent::Accepted,
            SampleEvent::Starting,
            SampleEvent::RepoPreparation,
            SampleEvent::PromptAgentStarted,
        ],
    );

    let replay = registry.replay(&worker_id, 0).unwrap();

    assert_eq!(
        event_names(&replay),
        vec![
            "accepted",
            "starting",
            "repo_preparation",
            "prompt_agent_started"
        ]
    );
    assert_eq!(event_sequences(&replay), vec![0, 1, 2, 3]);
}

/// Verifies that retained-history gaps are surfaced before replayed events.
#[test]
fn replay_events_before_first_available_prepends_history_truncated() {
    let mut registry = Registry::in_memory_with_event_capacity(2);
    let worker_id = worker_id("truncated-worker");
    registry
        .insert(record(
            "truncated-worker",
            WorkerMode::Feature,
            WorkerStatus::Accepted,
            "queued",
        ))
        .unwrap();
    append_events(
        &mut registry,
        &worker_id,
        [
            SampleEvent::Accepted,
            SampleEvent::Starting,
            SampleEvent::CurrentActivity,
        ],
    );

    let replay = registry.replay(&worker_id, 0).unwrap();

    assert!(matches!(
        &replay[0],
        ReplayItem::HistoryTruncated(event)
            if event == &StreamEvent::history_truncated(worker_id.clone(), 0, 1)
    ));
    assert_eq!(
        event_names(&replay[1..]),
        vec!["starting", "current_activity"]
    );
}

/// Verifies that replay from a retained sequence returns only that suffix.
#[test]
fn replay_events_from_middle_returns_only_requested_suffix() {
    let mut registry = seeded_registry("middle-replay-worker");
    let worker_id = worker_id("middle-replay-worker");
    append_events(
        &mut registry,
        &worker_id,
        [
            SampleEvent::Accepted,
            SampleEvent::Starting,
            SampleEvent::CurrentActivity,
        ],
    );

    let replay = registry.replay(&worker_id, 1).unwrap();

    assert_eq!(event_names(&replay), vec!["starting", "current_activity"]);
    assert!(replay
        .iter()
        .all(|item| matches!(item, ReplayItem::Worker(event) if event.sequence() >= 1)));
}

/// Verifies that input requests mark workers waiting and store a pending request.
#[test]
fn request_input_known_running_worker_marks_waiting_and_stores_pending_request() {
    let mut registry = seeded_registry("input-worker");
    let worker_id = worker_id("input-worker");
    registry
        .update_status(&worker_id, WorkerStatus::Running, "running prompt")
        .unwrap();
    let request_id = request_id("request-1");

    registry
        .request_input(InputRequest::new(
            worker_id.clone(),
            request_id.clone(),
            "Approve this plan?",
        ))
        .unwrap();

    let summary = registry.list().into_iter().next().unwrap();
    assert_eq!(summary.status(), WorkerStatus::WaitingForInput);
    assert_eq!(summary.pending_request_id(), Some(&request_id));
    assert_eq!(summary.activity(), Some("Approve this plan?"));
    assert!(registry.pending_input(&worker_id, &request_id).is_some());
}

/// Verifies that pending input accepts one answer and emits continuation.
#[test]
fn answer_input_pending_request_accepts_once_and_emits_continuation() {
    let mut registry = waiting_registry("answer-worker", "request-1");
    let worker_id = worker_id("answer-worker");
    let request_id = request_id("request-1");

    registry
        .answer_input(InputAnswer::new(
            worker_id.clone(),
            request_id.clone(),
            "yes, continue",
        ))
        .unwrap();

    assert!(registry.pending_input(&worker_id, &request_id).is_none());
    assert_eq!(
        registry.list().into_iter().next().unwrap().status(),
        WorkerStatus::Running
    );
    assert_eq!(
        event_names(&registry.replay(&worker_id, 0).unwrap()),
        vec!["waiting_for_input", "answer_provided"]
    );
    assert!(
        registry
            .replay(&worker_id, 0)
            .unwrap()
            .iter()
            .all(|item| !matches!(item, ReplayItem::Worker(event) if event.message() == "yes, continue"))
    );
}

/// Verifies that answers for unknown workers have no side effects.
#[test]
fn answer_input_unknown_worker_rejects_without_side_effects() {
    let mut registry = Registry::in_memory();
    let worker_id = worker_id("unknown-answer-worker");
    let request_id = request_id("request-1");

    let error = registry
        .answer_input(InputAnswer::new(worker_id, request_id, "ignored"))
        .unwrap_err();

    assert!(error.is_unknown_worker());
    assert!(registry.list().is_empty());
}

/// Verifies that duplicate answers for the same request are rejected.
#[test]
fn answer_input_duplicate_request_rejects_second_answer() {
    let mut registry = waiting_registry("duplicate-answer-worker", "request-1");
    let worker_id = worker_id("duplicate-answer-worker");
    let request_id = request_id("request-1");

    registry
        .answer_input(InputAnswer::new(
            worker_id.clone(),
            request_id.clone(),
            "first",
        ))
        .unwrap();
    let error = registry
        .answer_input(InputAnswer::new(worker_id, request_id, "second"))
        .unwrap_err();

    assert!(error.is_duplicate_answer() || error.is_stale_request() || error.is_no_pending_input());
}

/// Verifies that an old request id cannot answer a newer wait state.
#[test]
fn answer_input_stale_request_rejects_old_request_after_new_wait() {
    let mut registry = waiting_registry("stale-answer-worker", "request-1");
    let worker_id = worker_id("stale-answer-worker");

    registry
        .request_input(InputRequest::new(
            worker_id.clone(),
            request_id("request-2"),
            "Second prompt?",
        ))
        .unwrap();
    let error = registry
        .answer_input(InputAnswer::new(
            worker_id,
            request_id("request-1"),
            "stale answer",
        ))
        .unwrap_err();

    assert!(error.is_stale_request());
}

/// Verifies that non-waiting workers reject answers.
#[test]
fn answer_input_non_waiting_worker_rejects_answer() {
    let mut registry = seeded_registry("non-waiting-worker");
    let worker_id = worker_id("non-waiting-worker");

    let error = registry
        .answer_input(InputAnswer::new(
            worker_id,
            request_id("request-1"),
            "answer",
        ))
        .unwrap_err();

    assert!(error.is_no_pending_input() || error.is_not_waiting_for_input());
}

/// Verifies that terminal workers reject new input and preserve terminal reason.
#[test]
fn terminal_worker_rejects_new_input_and_preserves_terminal_reason() {
    let mut registry = seeded_registry("terminal-worker");
    let worker_id = worker_id("terminal-worker");
    registry
        .update_status(&worker_id, WorkerStatus::Failed, "process exited")
        .unwrap();

    let error = registry
        .request_input(InputRequest::new(
            worker_id.clone(),
            request_id("request-1"),
            "Should not ask",
        ))
        .unwrap_err();

    let summary = registry.list().into_iter().next().unwrap();
    assert!(error.is_terminal_worker());
    assert_eq!(summary.status(), WorkerStatus::Failed);
    assert_eq!(summary.terminal_reason(), Some("process exited"));
}

/// Verifies that a fresh in-memory registry does not restore previous workers.
#[test]
fn new_registry_after_previous_instance_has_no_workers() {
    let mut previous = seeded_registry("restart-worker");
    let old_worker = worker_id("restart-worker");
    previous
        .append_event(&old_worker, RegistryEvent::accepted("accepted"))
        .unwrap();

    let registry = Registry::in_memory();
    let error = registry.replay(&old_worker, 0).unwrap_err();

    assert!(registry.list().is_empty());
    assert!(error.is_unknown_worker());
}

fn seeded_registry(id: &str) -> Registry {
    let mut registry = Registry::in_memory();
    registry
        .insert(record(
            id,
            WorkerMode::Feature,
            WorkerStatus::Accepted,
            "queued",
        ))
        .unwrap();
    registry
}

fn waiting_registry(worker: &str, request: &str) -> Registry {
    let mut registry = seeded_registry(worker);
    let worker_id = worker_id(worker);
    registry
        .update_status(&worker_id, WorkerStatus::Running, "running prompt")
        .unwrap();
    registry
        .request_input(InputRequest::new(
            worker_id,
            request_id(request),
            "Approve this plan?",
        ))
        .unwrap();
    registry
}

fn record(id: &str, mode: WorkerMode, status: WorkerStatus, activity: &str) -> WorkerRecord {
    WorkerRecord::new(
        worker_id(id),
        mode,
        RepoIdentity::from_raw_url("https://token@example.com/org/repo.git").unwrap(),
        status,
        activity,
    )
}

fn append_events<const N: usize>(
    registry: &mut Registry,
    worker_id: &WorkerId,
    events: [SampleEvent; N],
) {
    for event in events {
        registry
            .append_event(worker_id, event.into_registry_event())
            .unwrap();
    }
}

#[derive(Clone, Copy)]
enum SampleEvent {
    Accepted,
    Starting,
    RepoPreparation,
    PromptAgentStarted,
    CurrentActivity,
}

impl SampleEvent {
    fn into_registry_event(self) -> RegistryEvent {
        match self {
            Self::Accepted => RegistryEvent::accepted("accepted"),
            Self::Starting => RegistryEvent::starting("starting"),
            Self::RepoPreparation => RegistryEvent::repo_preparation("repo preparation"),
            Self::PromptAgentStarted => RegistryEvent::prompt_agent_started("prompt agent"),
            Self::CurrentActivity => RegistryEvent::current_activity("running"),
        }
    }
}

fn event_names(items: &[ReplayItem]) -> Vec<&str> {
    items
        .iter()
        .filter_map(|item| match item {
            ReplayItem::Worker(event) => Some(event.name()),
            ReplayItem::HistoryTruncated(_) => None,
        })
        .collect()
}

fn event_sequences(items: &[ReplayItem]) -> Vec<u64> {
    items
        .iter()
        .filter_map(|item| match item {
            ReplayItem::Worker(event) => Some(event.sequence()),
            ReplayItem::HistoryTruncated(_) => None,
        })
        .collect()
}

fn worker_id(value: &str) -> WorkerId {
    WorkerId::from_str(value).unwrap()
}

fn request_id(value: &str) -> RequestId {
    RequestId::from_str(value).unwrap()
}
