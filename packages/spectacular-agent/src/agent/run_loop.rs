use super::context_compaction::ContextCompactor;
use super::context_usage::record_context_token_usage;
use super::provider_events::{ProviderEventHandler, ProviderEventOutcome};
use super::provider_stream::{run_retryable_provider_stream, ProviderRetryConfig};
use super::recorder::RunRecorder;
use super::request::{effective_system_prompt, validate_provider_capabilities};
use super::run_control::{RunControl, HARD_ABORT_GRACE};
use super::{Agent, LENGTH_CONTINUATION_PROMPT};
use crate::context::{
    ContextAssembler, ContextAssembly, ContextAssemblyError, ContextAssemblyInput,
    ContextSummaryRequest, TokenCounter,
};
use crate::error::AgentError;
use crate::event::AgentEvent;
use crate::queue::{RunId, RunRequest};
use spectacular_llms::{
    LlmProvider, ProviderCapabilities, ProviderMessage, ProviderRequest, ProviderToolCall,
    ToolManifest,
};
use std::sync::Arc;
use tokio::sync::mpsc;

impl<P, C> Agent<P, C>
where
    P: LlmProvider,
    C: TokenCounter + Clone,
{
    /// Supervises a started run and drops non-cooperative work after hard abort grace.
    pub(super) async fn run_request_with_abort(
        &self,
        run: RunRequest,
        control: Arc<RunControl>,
        sender: Option<mpsc::Sender<AgentEvent>>,
    ) -> Result<RunId, AgentError> {
        let abort_sender = sender.clone();
        let run_future = self.run_request(run, Arc::clone(&control), sender);
        tokio::pin!(run_future);

        tokio::select! {
            biased;
            result = &mut run_future => result,
            _ = control.hard_abort_requested() => {
                control.cancel();
                tokio::select! {
                    biased;
                    result = &mut run_future => result,
                    _ = tokio::time::sleep(HARD_ABORT_GRACE) => {
                        self.record_cancelled_after_abort(Arc::clone(&control), abort_sender.as_ref()).await;
                        Err(AgentError::CancellationError)
                    }
                }
            }
        }
    }

    /// Returns the retry policy used by visible provider calls and hidden summary calls.
    pub(super) fn provider_retry_config(&self) -> ProviderRetryConfig {
        ProviderRetryConfig {
            max_provider_retries: self.config.max_provider_retries,
            provider_retry_delay: self.config.provider_retry_delay,
        }
    }

    async fn run_request(
        &self,
        run: RunRequest,
        control: Arc<RunControl>,
        sender: Option<mpsc::Sender<AgentEvent>>,
    ) -> Result<RunId, AgentError> {
        let mut recorder = RunRecorder::new(self, control, sender);
        let run_event_start = self.record_user_prompt(&mut recorder, &run).await?;
        let context = self.request_context();

        if let Err(error) =
            validate_provider_capabilities(context.capabilities, &self.config, context.has_tools)
        {
            return recorder.record_error(error).await;
        }

        self.run_provider_loop(&mut recorder, run_event_start, context)
            .await?;
        Ok(run.id())
    }

    async fn record_user_prompt(
        &self,
        recorder: &mut RunRecorder<'_, P, C>,
        run: &RunRequest,
    ) -> Result<usize, AgentError> {
        let prompt = run.prompt().to_owned();
        let run_event_start = self.store.lock().unwrap().checkpoint() + 1;
        recorder
            .record(user_prompt_event(run.prompt_event_id(), prompt))
            .await?;
        Ok(run_event_start)
    }

    fn request_context(&self) -> RequestContext {
        let capabilities = self.provider.capabilities();
        let tool_manifests = self.tools.read().unwrap().manifests();
        let has_tools = !tool_manifests.is_empty();

        RequestContext {
            capabilities,
            tool_manifests,
            has_tools,
        }
    }

    async fn run_provider_loop(
        &self,
        recorder: &mut RunRecorder<'_, P, C>,
        run_event_start: usize,
        context: RequestContext,
    ) -> Result<(), AgentError> {
        let mut state = LoopState::default();

        loop {
            match self
                .assemble_provider_messages(recorder, &context, &mut state)
                .await?
            {
                ContextStep::Summarized => continue,
                ContextStep::Ready(messages) => {
                    let outcome = self
                        .run_provider_attempt(
                            recorder,
                            &context,
                            ProviderAttempt {
                                run_event_start,
                                messages,
                            },
                        )
                        .await?;

                    if matches!(
                        self.apply_provider_outcome(recorder, &mut state, outcome)
                            .await?,
                        RunLoopControl::Complete
                    ) {
                        break;
                    }
                }
            }
        }

        Ok(())
    }

    async fn assemble_provider_messages(
        &self,
        recorder: &mut RunRecorder<'_, P, C>,
        context: &RequestContext,
        state: &mut LoopState,
    ) -> Result<ContextStep, AgentError> {
        recorder.return_if_cancelled().await?;

        match self.assemble_context(context, state) {
            Ok(ContextAssembly::Ready(context)) => {
                record_context_token_usage(self, recorder, &context.diagnostics).await?;
                Ok(ContextStep::Ready(context.messages))
            }
            Ok(ContextAssembly::NeedsSummary(summary_request)) => {
                self.compact_context(
                    recorder,
                    state,
                    SummaryCompaction {
                        context,
                        request: &summary_request,
                    },
                )
                .await?;
                Ok(ContextStep::Summarized)
            }
            Err(error) => recorder.record_error(context_assembly_error(error)).await,
        }
    }

    fn assemble_context(
        &self,
        context: &RequestContext,
        state: &LoopState,
    ) -> Result<ContextAssembly, ContextAssemblyError> {
        let store = self.store.lock().unwrap();
        let assembler = ContextAssembler::new(
            self.token_counter.clone(),
            self.config.context_policy.clone(),
        );

        assembler.assemble(ContextAssemblyInput {
            system_prompt: effective_system_prompt(
                &self.config.system_prompt,
                &context.tool_manifests,
            ),
            store: &store,
            provider_limits: context.capabilities.context_limits,
            continuation_prompt: state
                .continuing_after_length
                .then_some(LENGTH_CONTINUATION_PROMPT),
        })
    }

    async fn compact_context(
        &self,
        recorder: &mut RunRecorder<'_, P, C>,
        state: &mut LoopState,
        compaction: SummaryCompaction<'_>,
    ) -> Result<(), AgentError> {
        if state.summary_passes_for_request
            >= self.config.context_policy.max_summary_passes_per_request
        {
            return recorder
                .record_error(context_still_over_limit_error(
                    state.summary_passes_for_request,
                ))
                .await;
        }

        state.summary_passes_for_request += 1;
        let summary_event = match ContextCompactor::new(self, compaction.context.capabilities)
            .compact(recorder, compaction.request)
            .await
        {
            Ok(summary_event) => summary_event,
            Err(AgentError::CancellationError) => return Err(AgentError::CancellationError),
            Err(error) => return recorder.record_error(error).await,
        };
        recorder.record(summary_event).await
    }

    async fn run_provider_attempt(
        &self,
        recorder: &mut RunRecorder<'_, P, C>,
        context: &RequestContext,
        attempt: ProviderAttempt,
    ) -> Result<ProviderEventOutcome, AgentError> {
        let mut handler = ProviderEventHandler::new(self, attempt.run_event_start);
        match run_retryable_provider_stream(
            &self.provider,
            self.provider_request(context, attempt.messages),
            recorder,
            self.provider_retry_config(),
            &mut handler,
        )
        .await
        {
            Ok(outcome) => Ok(outcome),
            Err(AgentError::CancellationError) => Err(AgentError::CancellationError),
            Err(error) => recorder.record_error(error).await,
        }
    }

    fn provider_request(
        &self,
        context: &RequestContext,
        messages: Vec<ProviderMessage>,
    ) -> ProviderRequest {
        let mut request = ProviderRequest::new(messages);
        if let Some(model) = self.config.model.clone() {
            request = request.with_model(model);
        }
        request.capabilities = context.capabilities;
        request.flags.allow_tools = context.has_tools;
        request.flags.include_reasoning =
            self.config.include_reasoning || self.config.reasoning_effort.is_some();
        request.flags.reasoning_effort = self.config.reasoning_effort.clone();
        request.tools = context.tool_manifests.clone();
        request
    }

    async fn apply_provider_outcome(
        &self,
        recorder: &mut RunRecorder<'_, P, C>,
        state: &mut LoopState,
        outcome: ProviderEventOutcome,
    ) -> Result<RunLoopControl, AgentError> {
        match outcome {
            ProviderEventOutcome::ContinueStream => {
                unreachable!("provider attempts only finish on terminal outcomes")
            }
            ProviderEventOutcome::CompleteRun => Ok(RunLoopControl::Complete),
            ProviderEventOutcome::ContinueCompletion => {
                state.continue_after_length();
                Ok(RunLoopControl::Continue)
            }
            ProviderEventOutcome::ExecuteTools(tool_calls) => {
                state.continue_after_tools();
                self.execute_tool_calls(recorder, &tool_calls).await?;
                Ok(RunLoopControl::Continue)
            }
        }
    }

    async fn record_cancelled_after_abort(
        &self,
        control: Arc<RunControl>,
        sender: Option<&mpsc::Sender<AgentEvent>>,
    ) {
        self.queue.cancel_pending().await;
        if !control.try_record_cancelled() {
            return;
        }

        let event = AgentEvent::cancelled(control.cancellation_reason());
        self.store.lock().unwrap().append(event.clone());
        if let Some(sender) = sender {
            let _ = sender.send(event).await;
        }
    }

    async fn execute_tool_calls(
        &self,
        recorder: &mut RunRecorder<'_, P, C>,
        tool_calls: &[ProviderToolCall],
    ) -> Result<(), AgentError> {
        let tools = self.tools.read().unwrap().clone();
        for tool_call in tool_calls {
            recorder.return_if_cancelled().await?;

            recorder
                .record(AgentEvent::tool_call_start(
                    tool_call.id.clone(),
                    tool_call.name.clone(),
                    tool_call.arguments.clone(),
                ))
                .await?;
            let result = tools.execute(tool_call, recorder.cancellation()).await;
            recorder.return_if_cancelled().await?;

            recorder
                .record(AgentEvent::tool_call_finish(
                    tool_call.id.clone(),
                    tool_call.name.clone(),
                    result,
                ))
                .await?;
        }

        Ok(())
    }
}

struct RequestContext {
    capabilities: ProviderCapabilities,
    tool_manifests: Vec<ToolManifest>,
    has_tools: bool,
}

#[derive(Default)]
struct LoopState {
    continuing_after_length: bool,
    summary_passes_for_request: usize,
}

impl LoopState {
    fn continue_after_length(&mut self) {
        self.continuing_after_length = true;
        self.summary_passes_for_request = 0;
    }

    fn continue_after_tools(&mut self) {
        self.continuing_after_length = false;
        self.summary_passes_for_request = 0;
    }
}

enum ContextStep {
    Ready(Vec<ProviderMessage>),
    Summarized,
}

struct SummaryCompaction<'a> {
    context: &'a RequestContext,
    request: &'a ContextSummaryRequest,
}

struct ProviderAttempt {
    run_event_start: usize,
    messages: Vec<ProviderMessage>,
}

enum RunLoopControl {
    Complete,
    Continue,
}

fn user_prompt_event(prompt_event_id: Option<&str>, prompt: String) -> AgentEvent {
    let Some(prompt_event_id) = prompt_event_id else {
        return AgentEvent::user_prompt(prompt);
    };

    AgentEvent::user_prompt_with_id(prompt_event_id, prompt)
}

fn context_assembly_error(error: ContextAssemblyError) -> AgentError {
    AgentError::ContextLimitError {
        reason: error.to_string(),
        provider: None,
        diagnostics: None,
    }
}

fn context_still_over_limit_error(summary_passes_for_request: usize) -> AgentError {
    AgentError::ContextLimitError {
        reason: format!(
            "context remains above compaction threshold after {summary_passes_for_request} summary pass(es)"
        ),
        provider: None,
        diagnostics: None,
    }
}
