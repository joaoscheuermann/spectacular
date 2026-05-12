use super::provider_stream::ProviderStreamHandler;
use super::recorder::RunRecorder;
use super::Agent;
use crate::context::TokenCounter;
use crate::error::AgentError;
use crate::event::AgentEvent;
use crate::schema::OutputSchema;
use spectacular_llms::{
    FinishReason, LlmProvider, ProviderFinished, ProviderMessageRole, ProviderStreamEvent,
    ProviderToolCall,
};

const PROVIDER_CANCELLED_MESSAGE: &str = "provider cancelled the response";

/// Terminal outcome requested by provider stream event handling.
pub(super) enum ProviderEventOutcome {
    ContinueStream,
    CompleteRun,
    ContinueCompletion,
    ExecuteTools(Vec<ProviderToolCall>),
}

/// Converts visible provider stream events into durable agent events and outcomes.
pub(super) struct AgentProviderEventHandler<'a, P, C>
where
    P: LlmProvider,
    C: TokenCounter + Clone,
{
    agent: &'a Agent<P, C>,
    run_event_start: usize,
}

impl<'a, P, C> AgentProviderEventHandler<'a, P, C>
where
    P: LlmProvider,
    C: TokenCounter + Clone,
{
    /// Creates a provider event handler for one visible agent completion.
    pub(super) fn new(agent: &'a Agent<P, C>, run_event_start: usize) -> Self {
        Self {
            agent,
            run_event_start,
        }
    }

    /// Records one visible provider event and returns any terminal run outcome.
    async fn record_provider_event(
        &self,
        recorder: &mut RunRecorder<'_, P, C>,
        provider_event: ProviderStreamEvent,
    ) -> Result<ProviderEventOutcome, AgentError> {
        match provider_event {
            ProviderStreamEvent::MessageDelta(delta) => {
                recorder.record(AgentEvent::MessageDelta(delta)).await?;
            }
            ProviderStreamEvent::ReasoningDelta(delta) => {
                recorder.record(AgentEvent::ReasoningDelta(delta)).await?;
            }
            ProviderStreamEvent::Finished(finished) => {
                return self.record_finished_event(recorder, finished).await;
            }
        }

        Ok(ProviderEventOutcome::ContinueStream)
    }

    /// Converts a provider finish payload into stored metadata, validation, or next actions.
    async fn record_finished_event(
        &self,
        recorder: &mut RunRecorder<'_, P, C>,
        finished: ProviderFinished,
    ) -> Result<ProviderEventOutcome, AgentError> {
        if let Some(usage) = finished.usage {
            recorder.record(AgentEvent::UsageMetadata(usage)).await?;
        }
        if let Some(reasoning) = finished.reasoning.clone() {
            recorder
                .record(AgentEvent::ReasoningMetadata(reasoning))
                .await?;
        }

        match finished.finish_reason {
            FinishReason::ToolCalls => self.handle_tool_calls_finish(finished.tool_calls),
            FinishReason::Length => self.handle_length_finish(finished.tool_calls),
            FinishReason::ContentFilter => recorder.record_error(AgentError::ContentFiltered).await,
            FinishReason::Cancelled => {
                recorder
                    .record_cancelled_with_reason(PROVIDER_CANCELLED_MESSAGE)
                    .await;
                Err(AgentError::CancellationError)
            }
            FinishReason::Error => {
                recorder
                    .record_error(AgentError::ProviderFinishError {
                        reason: "provider reported finish_reason=error".to_owned(),
                    })
                    .await
            }
            FinishReason::Stop => self.handle_stop_finish(recorder, finished).await,
        }
    }

    /// Validates tool-call finish payloads and returns the requested tool calls.
    fn handle_tool_calls_finish(
        &self,
        tool_calls: Vec<ProviderToolCall>,
    ) -> Result<ProviderEventOutcome, AgentError> {
        if tool_calls.is_empty() {
            return Err(AgentError::MalformedProviderResponse {
                reason: "tool-call finish did not include tool calls".to_owned(),
            });
        }

        if let Some(tool_call) = tool_calls
            .iter()
            .find(|tool_call| tool_call.id.trim().is_empty() || tool_call.name.trim().is_empty())
        {
            return Err(AgentError::MalformedProviderResponse {
                reason: format!(
                    "tool call has empty id or name: id={:?}, name={:?}",
                    tool_call.id, tool_call.name
                ),
            });
        }

        Ok(ProviderEventOutcome::ExecuteTools(tool_calls))
    }

    /// Validates length finish payloads and requests a continuation completion.
    fn handle_length_finish(
        &self,
        tool_calls: Vec<ProviderToolCall>,
    ) -> Result<ProviderEventOutcome, AgentError> {
        if !tool_calls.is_empty() {
            return Err(AgentError::MalformedProviderResponse {
                reason: "non-tool finish included tool calls".to_owned(),
            });
        }

        Ok(ProviderEventOutcome::ContinueCompletion)
    }

    /// Validates stop finish payloads, schema-checks final content, and records completion.
    async fn handle_stop_finish(
        &self,
        recorder: &mut RunRecorder<'_, P, C>,
        finished: ProviderFinished,
    ) -> Result<ProviderEventOutcome, AgentError> {
        if !finished.tool_calls.is_empty() {
            return recorder
                .record_error(AgentError::MalformedProviderResponse {
                    reason: "non-tool finish included tool calls".to_owned(),
                })
                .await;
        }

        if self.agent.config.require_usage_metadata && finished.usage.is_none() {
            return recorder
                .record_error(AgentError::MalformedProviderResponse {
                    reason: "provider omitted required usage metadata".to_owned(),
                })
                .await;
        }

        if let Some(output_schema) = self.agent.config.output_schema.as_ref() {
            self.validate_final_response(recorder, output_schema)
                .await?;
        }

        recorder.record(AgentEvent::finished(finished)).await?;
        Ok(ProviderEventOutcome::CompleteRun)
    }

    /// Validates the final assistant text against the configured output schema.
    async fn validate_final_response(
        &self,
        recorder: &mut RunRecorder<'_, P, C>,
        output_schema: &OutputSchema,
    ) -> Result<(), AgentError> {
        let final_response = {
            let store = self.agent.store.lock().unwrap();
            final_assistant_response(&store.events()[self.run_event_start..])
        };
        if let Err(error) = output_schema.validate_response(&final_response) {
            let message = error.to_string();
            recorder
                .record(AgentEvent::validation_error(message.clone()))
                .await?;
            return recorder
                .record_error(AgentError::ValidationError { message })
                .await;
        }

        Ok(())
    }
}

impl<P, C> ProviderStreamHandler<P, C> for AgentProviderEventHandler<'_, P, C>
where
    P: LlmProvider,
    C: TokenCounter + Clone,
{
    type Output = ProviderEventOutcome;

    /// Records one provider stream event and returns terminal outcomes to the run loop.
    async fn handle_event(
        &mut self,
        recorder: &mut RunRecorder<'_, P, C>,
        event: ProviderStreamEvent,
    ) -> Result<Option<Self::Output>, AgentError> {
        let outcome = self.record_provider_event(recorder, event).await?;
        if matches!(outcome, ProviderEventOutcome::ContinueStream) {
            return Ok(None);
        }

        Ok(Some(outcome))
    }

    /// Treats a stream that closes without finish as a completed visible run.
    fn stream_finished_without_event(
        &mut self,
        _saw_provider_event: bool,
    ) -> Result<Self::Output, AgentError> {
        Ok(ProviderEventOutcome::CompleteRun)
    }
}

/// Coalesces assistant deltas from stored events into final response text.
fn final_assistant_response(events: &[AgentEvent]) -> String {
    events
        .iter()
        .filter_map(|event| match event {
            AgentEvent::MessageDelta(delta) if delta.role == ProviderMessageRole::Assistant => {
                Some(delta.content.as_str())
            }
            _ => None,
        })
        .collect::<String>()
}
