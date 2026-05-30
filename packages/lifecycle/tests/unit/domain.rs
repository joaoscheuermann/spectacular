use lifecycle::event::{StreamEvent, WorkerEvent};
use lifecycle::identity::{RequestId, WorkerId};
use lifecycle::status::WorkerStatus;

#[test]
fn worker_id_from_str_valid_value_round_trips_display() {
    let id = "worker-01HXYZ"
        .parse::<WorkerId>()
        .expect("worker id should parse");

    assert_eq!(id.as_str(), "worker-01HXYZ");
    assert_eq!(id.to_string(), "worker-01HXYZ");
}

#[test]
fn worker_id_from_str_blank_or_whitespace_rejects_value() {
    assert!("".parse::<WorkerId>().is_err());
    assert!("   ".parse::<WorkerId>().is_err());
}

#[test]
fn request_id_from_str_valid_value_round_trips_display() {
    let id = "request-01HXYZ"
        .parse::<RequestId>()
        .expect("request id should parse");

    assert_eq!(id.as_str(), "request-01HXYZ");
    assert_eq!(id.to_string(), "request-01HXYZ");
}

#[test]
fn request_id_from_str_blank_or_whitespace_rejects_value() {
    assert!("".parse::<RequestId>().is_err());
    assert!("   ".parse::<RequestId>().is_err());
}

#[test]
fn status_from_proto_known_values_maps_to_domain() {
    use lifecycle::proto::doric::lifecycle::v1::WorkerStatus as ProtoStatus;

    let cases = [
        (ProtoStatus::Accepted, WorkerStatus::Accepted, "accepted"),
        (ProtoStatus::Starting, WorkerStatus::Starting, "starting"),
        (ProtoStatus::Running, WorkerStatus::Running, "running"),
        (
            ProtoStatus::WaitingForInput,
            WorkerStatus::WaitingForInput,
            "waiting_for_input",
        ),
        (ProtoStatus::Succeeded, WorkerStatus::Succeeded, "succeeded"),
        (ProtoStatus::Failed, WorkerStatus::Failed, "failed"),
        (ProtoStatus::Stopped, WorkerStatus::Stopped, "stopped"),
    ];

    for (proto, expected, display) in cases {
        let status = WorkerStatus::try_from(proto).expect("known proto status should map");

        assert_eq!(status, expected);
        assert_eq!(status.to_string(), display);
    }
}

#[test]
fn status_domain_unavailable_and_untracked_have_stable_display() {
    assert_eq!(WorkerStatus::Unavailable.to_string(), "unavailable");
    assert_eq!(WorkerStatus::Untracked.to_string(), "untracked");
}

#[test]
fn event_constructors_lifecycle_milestones_preserve_status_and_name() {
    let worker_id = "worker-01HXYZ"
        .parse::<WorkerId>()
        .expect("worker id should parse");
    let request_id = "request-01HXYZ"
        .parse::<RequestId>()
        .expect("request id should parse");

    assert_worker_event(
        WorkerEvent::accepted(worker_id.clone(), 1, "dispatch accepted"),
        WorkerStatus::Accepted,
        "accepted",
        "dispatch accepted",
    );
    assert_worker_event(
        WorkerEvent::starting(worker_id.clone(), 2, "worker starting"),
        WorkerStatus::Starting,
        "starting",
        "worker starting",
    );
    assert_worker_event(
        WorkerEvent::repo_preparation(worker_id.clone(), 3, "preparing repository"),
        WorkerStatus::Running,
        "repo_preparation",
        "preparing repository",
    );
    assert_worker_event(
        WorkerEvent::prompt_agent_started(worker_id.clone(), 4, "prompt agent started"),
        WorkerStatus::Running,
        "prompt_agent_started",
        "prompt agent started",
    );
    assert_worker_event(
        WorkerEvent::prompt_artifact_written(worker_id.clone(), 5, "prompt artifact written"),
        WorkerStatus::Running,
        "prompt_artifact_written",
        "prompt artifact written",
    );
    assert_worker_event(
        WorkerEvent::current_activity(worker_id.clone(), 6, "reading requirements"),
        WorkerStatus::Running,
        "current_activity",
        "reading requirements",
    );
    assert_worker_event(
        WorkerEvent::waiting_for_input(
            worker_id.clone(),
            7,
            request_id.clone(),
            "need clarification",
        ),
        WorkerStatus::WaitingForInput,
        "waiting_for_input",
        "need clarification",
    );
    let answer_event =
        WorkerEvent::prompt_answer_consumed(worker_id.clone(), 8, request_id, "answer consumed");
    assert_eq!(
        answer_event.request_id().map(RequestId::as_str),
        Some("request-01HXYZ")
    );
    assert_worker_event(
        answer_event,
        WorkerStatus::Running,
        "prompt_answer_consumed",
        "answer consumed",
    );
    assert_worker_event(
        WorkerEvent::failed(worker_id.clone(), 9, "repo preparation failed"),
        WorkerStatus::Failed,
        "failed",
        "repo preparation failed",
    );
    assert_worker_event(
        WorkerEvent::prompt_agent_completed(worker_id.clone(), 10, "prompt agent completed"),
        WorkerStatus::Succeeded,
        "prompt_agent_completed",
        "prompt agent completed",
    );
    assert_worker_event(
        WorkerEvent::succeeded(worker_id.clone(), 11, "prompt artifact written"),
        WorkerStatus::Succeeded,
        "succeeded",
        "prompt artifact written",
    );
    assert_worker_event(
        WorkerEvent::stopped(worker_id, 12, "worker stopped"),
        WorkerStatus::Stopped,
        "stopped",
        "worker stopped",
    );
}

#[test]
fn event_constructors_prompt_agent_failure_redacts_secret_text() {
    let worker_id = "worker-01HXYZ"
        .parse::<WorkerId>()
        .expect("worker id should parse");

    let event = WorkerEvent::prompt_agent_failed(
        worker_id,
        13,
        "provider returned sk-test_secret_1234567890abcdef",
    );

    assert_eq!(event.status(), WorkerStatus::Failed);
    assert_eq!(event.name(), "prompt_agent_failed");
    assert!(event.message().contains("[REDACTED]"));
    assert!(!event.message().contains("sk-test_secret"));
}

#[test]
fn event_history_truncated_constructor_uses_stable_stream_event() {
    let worker_id = "worker-01HXYZ"
        .parse::<WorkerId>()
        .expect("worker id should parse");

    let event = StreamEvent::history_truncated(worker_id.clone(), 0, 42);

    assert_eq!(event.worker_id(), &worker_id);
    assert_eq!(event.name(), "history_truncated");
    assert_eq!(event.requested_from_sequence(), 0);
    assert_eq!(event.first_available_sequence(), 42);
    assert_eq!(event.message(), "worker event history was truncated");
}

fn assert_worker_event(
    event: WorkerEvent,
    status: WorkerStatus,
    name: &str,
    message: &str,
) {
    assert_eq!(event.status(), status);
    assert_eq!(event.name(), name);
    assert_eq!(event.message(), message);
}
