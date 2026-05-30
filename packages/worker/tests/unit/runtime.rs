use lifecycle::proto::doric::lifecycle::v1 as pb;
use worker::agents::prompt::PromptAgentEvent;
use worker::error::WorkerError;
use worker::runtime::{PromptRunState, RuntimeOutcome};

use super::runtime_support::*;

#[test]
fn run_worker_session_valid_attach_waits_for_start_job_before_repo_preparation() {
    let mut fx = RuntimeFixture::new("worker-runtime-attach");

    let outcome = fx.run().unwrap();

    assert_eq!(outcome, RuntimeOutcome::WaitingForStartJob);
    assert_worker_hello(&fx.frames()[0], "worker-runtime-attach", "one-time-token");
    assert!(fx.repo.requests().is_empty());
    assert!(fx.prompt.jobs().is_empty());
    assert_no_status_or_event_text_contains(fx.frames(), "one-time-token");
}

#[test]
fn run_worker_session_shutdown_before_start_job_returns_stopped_status() {
    let mut fx =
        RuntimeFixture::new("worker-runtime-pre-start-shutdown").with_shutdown("daemon shutdown");

    let outcome = fx.run().unwrap();

    assert_eq!(outcome, RuntimeOutcome::Stopped);
    assert!(fx.repo.requests().is_empty());
    assert!(fx.prompt.jobs().is_empty());
    assert_eq!(terminal_status_count(fx.frames()), 1);
    assert_status_sent(fx.frames(), pb::WorkerStatus::Stopped);
}

#[test]
fn run_worker_session_start_job_prepares_repo_before_prompt_runner() {
    let mut fx = RuntimeFixture::new("worker-runtime-start")
        .with_start("https://github.com/org/repo.git")
        .with_prompt([PromptRunState::succeeded([
            started(),
            PromptAgentEvent::completed("Prompt requirements artifact completed"),
        ])]);

    let outcome = fx.run().unwrap();

    let request = &fx.repo.requests()[0];
    assert_eq!(outcome, RuntimeOutcome::Succeeded);
    assert_eq!(request.repo_url(), "https://github.com/org/repo.git");
    assert_eq!(request.layout().repo(), fx.layout.repo());
    assert_eq!(fx.prompt.jobs()[0].layout().repo(), fx.layout.repo());
    assert_eq!(
        fx.session.event_names(),
        [
            "repo_preparation",
            "prompt_agent_started",
            "prompt_agent_completed"
        ]
    );
}

#[test]
fn run_worker_session_repo_preparation_failure_sends_failed_status_without_prompt_runner() {
    let mut fx = RuntimeFixture::new("worker-runtime-repo-failure")
        .with_start("https://user:pass@example.com/org/repo.git")
        .with_repo_failure(WorkerError::git_failed(
            128,
            "fatal token sk-secret in https://user:pass@example.com/org/repo.git",
            "https://user:pass@example.com/org/repo.git",
        ));

    let outcome = fx.run().unwrap();

    assert_eq!(outcome, RuntimeOutcome::Failed);
    assert!(fx.prompt.jobs().is_empty());
    assert_eq!(terminal_status_count(fx.frames()), 1);
    assert_status_sent(fx.frames(), pb::WorkerStatus::Failed);
    assert_no_frame_text_contains(fx.frames(), "sk-secret");
    assert_no_frame_text_contains(fx.frames(), "user:pass");
}

#[test]
fn run_worker_session_prompt_events_stream_to_daemon_in_sequence() {
    let mut fx = RuntimeFixture::new("worker-runtime-events")
        .with_start("https://github.com/org/repo.git")
        .with_prompt([PromptRunState::succeeded([
            started(),
            PromptAgentEvent::artifact_written(
                "artifacts/PROMPT.md",
                "Prompt artifact written to artifacts/PROMPT.md",
            ),
            PromptAgentEvent::completed("Prompt requirements artifact completed"),
        ])]);

    fx.run().unwrap();

    assert_eq!(
        fx.session.prompt_event_names(),
        [
            "prompt_agent_started",
            "prompt_artifact_written",
            "prompt_agent_completed"
        ]
    );
    assert_monotonic_sequences(&fx.session.event_sequences());
    assert_eq!(terminal_status_count(fx.frames()), 1);
    assert_status_sent(fx.frames(), pb::WorkerStatus::Succeeded);
}

#[test]
fn run_worker_session_input_request_waits_for_matching_answer_before_continuing() {
    let request_id = request_id("request-runtime-1");
    let mut fx = RuntimeFixture::new("worker-runtime-input")
        .with_start("https://github.com/org/repo.git")
        .with_answer(&request_id, "Use repo-local skills.")
        .with_prompt([
            PromptRunState::waiting_for_input([
                started(),
                PromptAgentEvent::input_requested(
                    request_id.clone(),
                    "Which repository constraints should the prompt include?",
                ),
            ]),
            PromptRunState::succeeded([
                PromptAgentEvent::answer_consumed(request_id.clone(), "Prompt answer consumed"),
                PromptAgentEvent::completed("Prompt requirements artifact completed"),
            ]),
        ]);

    fx.run().unwrap();

    assert_eq!(
        fx.prompt.answers(),
        [(
            "request-runtime-1".to_owned(),
            "Use repo-local skills.".to_owned()
        )]
    );
    assert_eq!(
        fx.session.prompt_event_names(),
        [
            "prompt_agent_started",
            "waiting_for_input",
            "prompt_answer_consumed",
            "prompt_agent_completed"
        ]
    );
    assert_status_sent(fx.frames(), pb::WorkerStatus::WaitingForInput);
    assert_status_sent(fx.frames(), pb::WorkerStatus::Succeeded);
}

#[test]
fn run_worker_session_answer_before_wait_does_not_continue_prompt_or_leak_text() {
    let request_id = request_id("request-too-early");
    let mut fx = RuntimeFixture::new("worker-runtime-early-answer")
        .with_answer(&request_id, "secret answer text")
        .with_start("https://github.com/org/repo.git")
        .with_prompt([PromptRunState::succeeded([
            started(),
            PromptAgentEvent::completed("Prompt requirements artifact completed"),
        ])]);

    fx.run().unwrap();

    assert!(fx.prompt.answers().is_empty());
    assert!(!fx
        .session
        .prompt_event_names()
        .contains(&"prompt_answer_consumed".to_owned()));
    assert_no_frame_text_contains(fx.frames(), "secret answer text");
}

#[test]
fn run_worker_session_shutdown_before_terminal_prompt_sends_stopped_status() {
    let mut fx = RuntimeFixture::new("worker-runtime-shutdown")
        .with_start("https://github.com/org/repo.git")
        .with_shutdown("daemon shutdown")
        .with_prompt([PromptRunState::running([started()])]);

    let outcome = fx.run().unwrap();

    assert_eq!(outcome, RuntimeOutcome::Stopped);
    assert_eq!(fx.prompt.stop_reasons(), ["daemon shutdown".to_owned()]);
    assert_eq!(terminal_status_count(fx.frames()), 1);
    assert_status_sent(fx.frames(), pb::WorkerStatus::Stopped);
    assert_status_not_sent(fx.frames(), pb::WorkerStatus::Succeeded);
    assert_status_not_sent(fx.frames(), pb::WorkerStatus::Failed);
}

#[test]
fn run_worker_session_running_without_pending_input_returns_running_outcome() {
    let mut fx = RuntimeFixture::new("worker-runtime-running")
        .with_start("https://github.com/org/repo.git")
        .with_prompt([PromptRunState::running([started()])]);

    let outcome = fx.run().unwrap();

    assert_eq!(outcome, RuntimeOutcome::Running);
    assert_status_sent(fx.frames(), pb::WorkerStatus::Running);
    assert_eq!(terminal_status_count(fx.frames()), 0);
}

#[test]
fn run_worker_session_prompt_failure_sends_failed_terminal_status_and_redacted_reason() {
    let mut fx = RuntimeFixture::new("worker-runtime-prompt-failure")
        .with_start("https://github.com/org/repo.git")
        .with_prompt([PromptRunState::failed(WorkerError::provider_setup_failed(
            "provider sk-test_secret access_token=abc worker token one-time-token",
        ))]);

    fx.run().unwrap();

    assert_eq!(terminal_status_count(fx.frames()), 1);
    assert_status_sent(fx.frames(), pb::WorkerStatus::Failed);
    assert!(fx
        .session
        .event_names()
        .contains(&"prompt_agent_failed".to_owned()));
    assert_no_frame_text_contains(fx.frames(), "sk-test_secret");
    assert_no_frame_text_contains(fx.frames(), "access_token=abc");
    assert_no_status_or_event_text_contains(fx.frames(), "one-time-token");
}

#[test]
fn run_worker_session_blank_token_preserves_outbound_status_and_event_text() {
    let mut fx = RuntimeFixture::new("worker-runtime-blank-token")
        .with_start("https://github.com/org/repo.git")
        .with_prompt([PromptRunState::succeeded([
            started(),
            PromptAgentEvent::completed("Prompt requirements artifact completed"),
        ])]);

    let outcome = fx.run_with_token("").unwrap();
    let messages = status_or_event_messages(fx.frames());

    assert_eq!(outcome, RuntimeOutcome::Succeeded);
    assert_worker_hello(&fx.frames()[0], "worker-runtime-blank-token", "");
    assert!(messages
        .iter()
        .any(|message| message == "Worker session running"));
    assert!(messages
        .iter()
        .any(|message| message == "Preparing repository for prompt requirements"));
    assert!(messages
        .iter()
        .any(|message| message == "Prompt requirements agent started"));
    assert!(messages
        .iter()
        .any(|message| message == "Prompt requirements artifact completed"));
    assert!(messages
        .iter()
        .any(|message| message == "Prompt requirements workflow completed"));
    assert!(
        messages
            .iter()
            .all(|message| !message.contains("[REDACTED]")),
        "blank token should not redact ordinary runtime messages: {messages:?}"
    );
}

#[test]
fn run_worker_session_success_and_failure_send_one_terminal_status() {
    let mut success = RuntimeFixture::new("worker-runtime-terminal-success")
        .with_start("https://github.com/org/repo.git")
        .with_prompt([PromptRunState::succeeded([PromptAgentEvent::completed(
            "Prompt requirements artifact completed",
        )])]);
    let mut failure = RuntimeFixture::new("worker-runtime-terminal-failure")
        .with_start("https://github.com/org/repo.git")
        .with_prompt([PromptRunState::failed(WorkerError::provider_setup_failed(
            "provider failure",
        ))]);

    success.run().unwrap();
    failure.run().unwrap();

    assert_eq!(terminal_status_count(success.frames()), 1);
    assert_eq!(terminal_status_count(failure.frames()), 1);
    assert_no_status_after_terminal(success.frames());
    assert_no_status_after_terminal(failure.frames());
}
