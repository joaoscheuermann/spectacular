use lifecycle::event::WorkerEvent;

use crate::agents::prompt::PromptAgentEvent;
use crate::state::WorkerId;

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
