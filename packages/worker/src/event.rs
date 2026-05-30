use lifecycle::event::WorkerEvent;
use lifecycle::proto::doric::lifecycle::v1 as pb;
use lifecycle::redaction::redact_failure_text;
use lifecycle::status::WorkerStatus;

use crate::agents::prompt::PromptAgentEvent;
use crate::state::WorkerId;

const REDACTED: &str = "[REDACTED]";

pub fn prompt_agent_event_to_worker_event(
    worker_id: WorkerId,
    sequence: u64,
    event: PromptAgentEvent,
) -> WorkerEvent {
    match event {
        PromptAgentEvent::Started { message } => {
            WorkerEvent::prompt_agent_started(worker_id, sequence, message)
        }
        PromptAgentEvent::ArtifactWritten { message, .. } => {
            WorkerEvent::prompt_artifact_written(worker_id, sequence, message)
        }
        PromptAgentEvent::InputRequested {
            request_id,
            request_text,
        } => WorkerEvent::waiting_for_input(worker_id, sequence, request_id, request_text),
        PromptAgentEvent::AnswerConsumed {
            request_id,
            message,
        } => WorkerEvent::prompt_answer_consumed(worker_id, sequence, request_id, message),
        PromptAgentEvent::Completed { message } => {
            WorkerEvent::prompt_agent_completed(worker_id, sequence, message)
        }
        PromptAgentEvent::Failed { message } => {
            WorkerEvent::prompt_agent_failed(worker_id, sequence, message)
        }
    }
}

pub fn sanitize_prompt_agent_event(event: PromptAgentEvent, token: &str) -> PromptAgentEvent {
    match event {
        PromptAgentEvent::Started { message } => {
            PromptAgentEvent::started(sanitize_event_text(&message, token))
        }
        PromptAgentEvent::ArtifactWritten { artifact, message } => {
            PromptAgentEvent::artifact_written(artifact, sanitize_event_text(&message, token))
        }
        PromptAgentEvent::InputRequested {
            request_id,
            request_text,
        } => {
            PromptAgentEvent::input_requested(request_id, sanitize_event_text(&request_text, token))
        }
        PromptAgentEvent::AnswerConsumed {
            request_id,
            message,
        } => PromptAgentEvent::answer_consumed(request_id, sanitize_event_text(&message, token)),
        PromptAgentEvent::Completed { message } => {
            PromptAgentEvent::completed(sanitize_event_text(&message, token))
        }
        PromptAgentEvent::Failed { message } => {
            PromptAgentEvent::failed(sanitize_event_text(&message, token))
        }
    }
}

pub fn sanitize_event_text(text: &str, token: &str) -> String {
    let redacted = redact_provider_assignments(&redact_failure_text(text));
    if token.is_empty() {
        redacted
    } else {
        redacted.replace(token, REDACTED)
    }
}

pub fn worker_event_to_proto(event: WorkerEvent) -> pb::WorkerEvent {
    let input = event.request_id().map(|request_id| pb::InputRequest {
        request_id: request_id.as_str().to_owned(),
        prompt: event.message().to_owned(),
        choices: Vec::new(),
    });

    pb::WorkerEvent {
        worker_id: event.worker_id().as_str().to_owned(),
        sequence: event.sequence(),
        status: worker_status_to_proto(event.status()) as i32,
        name: event.name().to_owned(),
        message: event.message().to_owned(),
        input,
        occurred_at: None,
    }
}

pub fn worker_status_to_proto(status: WorkerStatus) -> pb::WorkerStatus {
    match status {
        WorkerStatus::Accepted => pb::WorkerStatus::Accepted,
        WorkerStatus::Starting => pb::WorkerStatus::Starting,
        WorkerStatus::Running => pb::WorkerStatus::Running,
        WorkerStatus::WaitingForInput => pb::WorkerStatus::WaitingForInput,
        WorkerStatus::Succeeded => pb::WorkerStatus::Succeeded,
        WorkerStatus::Failed => pb::WorkerStatus::Failed,
        WorkerStatus::Stopped => pb::WorkerStatus::Stopped,
        WorkerStatus::Unavailable | WorkerStatus::Untracked => pb::WorkerStatus::Unspecified,
    }
}

fn redact_provider_assignments(text: &str) -> String {
    text.split_whitespace()
        .map(|word| {
            let lowered = word.to_ascii_lowercase();
            if lowered.starts_with("sk-")
                || lowered.contains("access_token")
                || lowered.contains("refresh_token")
            {
                REDACTED.to_owned()
            } else {
                word.to_owned()
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}
