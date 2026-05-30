use lifecycle::identity::RequestId;
use lifecycle::status::WorkerStatus;
use worker::agents::prompt::PromptAgentEvent;
use worker::event::prompt_agent_event_to_worker_event;
use worker::state::WorkerId;

#[test]
fn prompt_agent_event_to_worker_event_started_maps_prompt_agent_started() {
    let worker_id = worker_id("worker-event-started");

    let event = prompt_agent_event_to_worker_event(
        worker_id.clone(),
        7,
        PromptAgentEvent::started("Prompt requirements agent started"),
    );

    assert_eq!(event.worker_id(), &worker_id);
    assert_eq!(event.sequence(), 7);
    assert_eq!(event.name(), "prompt_agent_started");
    assert_eq!(event.status(), WorkerStatus::Running);
    assert_prompt_only(event.message());
}

#[test]
fn prompt_agent_event_to_worker_event_input_requested_preserves_request_text_and_request_id() {
    let worker_id = worker_id("worker-event-input");
    let request_id = request_id("request-event-input");
    let request_text = "Which repository constraints should the prompt include?";

    let event = prompt_agent_event_to_worker_event(
        worker_id,
        11,
        PromptAgentEvent::input_requested(request_id.clone(), request_text),
    );

    assert_eq!(event.sequence(), 11);
    assert_eq!(event.name(), "waiting_for_input");
    assert_eq!(event.status(), WorkerStatus::WaitingForInput);
    assert_eq!(event.request_id(), Some(&request_id));
    assert!(event.message().contains(request_text));
}

#[test]
fn prompt_agent_event_to_worker_event_failed_redacts_failure_text() {
    let event = prompt_agent_event_to_worker_event(
        worker_id("worker-event-failed"),
        13,
        PromptAgentEvent::failed("provider returned sk-test_secret_1234567890abcdef"),
    );

    assert_eq!(event.name(), "prompt_agent_failed");
    assert_eq!(event.status(), WorkerStatus::Failed);
    assert!(event.message().contains("[REDACTED]"));
    assert!(!event.message().contains("sk-test_secret"));
}

#[test]
fn prompt_agent_event_to_worker_event_names_are_prompt_only() {
    let events = [
        PromptAgentEvent::started("Prompt requirements agent started"),
        PromptAgentEvent::artifact_written(
            "artifacts/PROMPT.md",
            "Prompt artifact written to artifacts/PROMPT.md",
        ),
        PromptAgentEvent::input_requested(
            request_id("request-event-scope"),
            "Which repository constraints should the prompt include?",
        ),
        PromptAgentEvent::answer_consumed(
            request_id("request-event-scope"),
            "Prompt answer consumed",
        ),
        PromptAgentEvent::completed("Prompt requirements artifact completed"),
        PromptAgentEvent::failed("Prompt requirements agent failed"),
    ];

    for (offset, prompt_event) in events.into_iter().enumerate() {
        let event = prompt_agent_event_to_worker_event(
            worker_id("worker-event-scope"),
            offset as u64,
            prompt_event,
        );

        assert_prompt_only(event.name());
        assert_prompt_only(event.message());
    }
}

fn worker_id(value: &str) -> WorkerId {
    value.parse().unwrap()
}

fn request_id(value: &str) -> RequestId {
    value.parse().unwrap()
}

fn assert_prompt_only(value: &str) {
    for forbidden in [
        "prd",
        "tdd",
        "technical_design",
        "technical design",
        "decomposition",
        "implementation",
        "tests",
        "validation",
        "handover",
    ] {
        assert!(
            !value.to_ascii_lowercase().contains(forbidden),
            "`{value}` should not claim later Doric phase `{forbidden}`"
        );
    }
}
