use super::provider_stream::{run_retryable_provider_stream, ProviderStreamHandler};
use super::recorder::RunRecorder;
use super::Agent;
use crate::context::{ContextSummaryRequest, TokenCounter};
use crate::error::AgentError;
use crate::event::AgentEvent;
use spectacular_llms::{
    FinishReason, LlmProvider, ProviderCapabilities, ProviderFinished, ProviderMessage,
    ProviderMessageRole, ProviderRequest, ProviderStreamEvent,
};

const CONTEXT_SUMMARY_SYSTEM_PROMPT: &str = "Summarize compacted Spectacular session context into a concise state block. Do not invent facts. Use the requested headings exactly. Mark unknown or empty fields as None.";

/// Runs hidden context summary requests and returns stored summary events.
pub(super) struct ContextCompactor<'a, P, C>
where
    P: LlmProvider,
    C: TokenCounter + Clone,
{
    agent: &'a Agent<P, C>,
    capabilities: ProviderCapabilities,
}

impl<'a, P, C> ContextCompactor<'a, P, C>
where
    P: LlmProvider,
    C: TokenCounter + Clone,
{
    /// Creates a compactor for the current agent and provider capabilities.
    pub(super) fn new(agent: &'a Agent<P, C>, capabilities: ProviderCapabilities) -> Self {
        Self {
            agent,
            capabilities,
        }
    }

    /// Summarizes compactable context and returns the summary-created event to persist.
    pub(super) async fn compact(
        &self,
        recorder: &mut RunRecorder<'_, P, C>,
        summary_request: &ContextSummaryRequest,
    ) -> Result<AgentEvent, AgentError> {
        let summary = self.run_context_summary(recorder, summary_request).await?;
        Ok(context_summary_event(
            recorder.event_count(),
            summary_request,
            summary,
        ))
    }

    /// Executes the hidden provider call that produces compact session state.
    async fn run_context_summary(
        &self,
        recorder: &mut RunRecorder<'_, P, C>,
        summary_request: &ContextSummaryRequest,
    ) -> Result<String, AgentError> {
        let request = self.summary_request(summary_request);
        let mut handler = SummaryStreamHandler::default();
        run_retryable_provider_stream(
            &self.agent.provider,
            request,
            recorder,
            self.agent.provider_retry_config(),
            &mut handler,
        )
        .await
    }

    /// Builds the provider request used for hidden context summarization.
    fn summary_request(&self, summary_request: &ContextSummaryRequest) -> ProviderRequest {
        let mut request = ProviderRequest::new(vec![
            ProviderMessage::system(CONTEXT_SUMMARY_SYSTEM_PROMPT),
            ProviderMessage::user(SummaryPrompt::default().user_prompt(
                summary_request,
                self.agent.config.context_policy.summary_max_tokens,
            )),
        ]);
        if let Some(model) = self.agent.config.model.clone() {
            request = request.with_model(model);
        }
        request.capabilities = self.capabilities;
        request.flags.stream = true;
        request.flags.allow_tools = false;
        request.flags.include_reasoning = false;
        request.flags.reasoning_effort = None;
        request.tools = Vec::new();
        request
    }
}

/// Builds prompts for hidden context summary requests.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct SummaryPrompt;

impl SummaryPrompt {
    /// Builds the user prompt containing previous summary state and transcript text.
    fn user_prompt(
        &self,
        summary_request: &ContextSummaryRequest,
        summary_max_tokens: usize,
    ) -> String {
        let previous_summary = summary_request
            .previous_summary
            .as_ref()
            .map(|summary| summary.content.as_str())
            .unwrap_or("None");

        format!(
            "Create a replacement compact session summary no longer than about {summary_max_tokens} tokens.\n\nPrevious summary:\n{previous_summary}\n\nTranscript to fold into the summary:\n{}\n\nUse exactly these Markdown headings:\n\n# Goal\n# Current Task\n# Hard Constraints\n# Decisions\n# Superseded Decisions\n# Files And Symbols\n# Commands And Outcomes\n# Known Failures\n# Open Questions\n# Evidence Handles",
            summary_request.transcript
        )
    }
}

/// Collects hidden summary stream text and validates its finish event.
#[derive(Default)]
struct SummaryStreamHandler {
    summary: String,
}

impl<P, C> ProviderStreamHandler<P, C> for SummaryStreamHandler
where
    P: LlmProvider,
    C: TokenCounter + Clone,
{
    type Output = String;

    /// Appends assistant deltas to summary text and finalizes on provider finish.
    async fn handle_event(
        &mut self,
        _recorder: &mut RunRecorder<'_, P, C>,
        event: ProviderStreamEvent,
    ) -> Result<Option<Self::Output>, AgentError> {
        match event {
            ProviderStreamEvent::MessageDelta(delta)
                if delta.role == ProviderMessageRole::Assistant =>
            {
                self.summary.push_str(&delta.content);
                Ok(None)
            }
            ProviderStreamEvent::MessageDelta(_) | ProviderStreamEvent::ReasoningDelta(_) => {
                Ok(None)
            }
            ProviderStreamEvent::Finished(finished) => {
                finish_context_summary(std::mem::take(&mut self.summary), finished).map(Some)
            }
        }
    }

    /// Rejects a hidden summary stream that yielded events but no finish event.
    fn stream_finished_without_event(
        &mut self,
        saw_provider_event: bool,
    ) -> Result<Self::Output, AgentError> {
        if saw_provider_event {
            return Err(AgentError::MalformedProviderResponse {
                reason: "context summary stream ended without a finish event".to_owned(),
            });
        }

        Ok(String::new())
    }
}

/// Creates an auditable summary event for a successful compaction pass.
fn context_summary_event(
    event_count: usize,
    summary_request: &ContextSummaryRequest,
    summary: String,
) -> AgentEvent {
    AgentEvent::context_summary_created(
        format!("context-summary-{event_count}"),
        summary_request
            .previous_summary
            .as_ref()
            .map(|summary| summary.id.clone()),
        summary_request.source_event_start,
        summary_request.source_event_end,
        summary,
        summary_request.estimated_tokens,
    )
}

/// Converts the hidden summary finish event into a stored summary or safe failure.
fn finish_context_summary(
    summary: String,
    finished: ProviderFinished,
) -> Result<String, AgentError> {
    if !finished.tool_calls.is_empty() {
        return Err(AgentError::MalformedProviderResponse {
            reason: "context summary response included tool calls".to_owned(),
        });
    }

    match finished.finish_reason {
        FinishReason::Stop => Ok(summary),
        FinishReason::Length => Err(AgentError::ContextLimitError {
            reason: "context summary response was truncated".to_owned(),
        }),
        FinishReason::ToolCalls => Err(AgentError::MalformedProviderResponse {
            reason: "context summary finished with tool_calls".to_owned(),
        }),
        FinishReason::ContentFilter => Err(AgentError::ContentFiltered),
        FinishReason::Cancelled => Err(AgentError::CancellationError),
        FinishReason::Error => Err(AgentError::ProviderFinishError {
            reason: "provider reported finish_reason=error during context summary".to_owned(),
        }),
    }
}
