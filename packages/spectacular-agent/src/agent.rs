mod constructors;
mod context_compaction;
mod provider_events;
mod provider_stream;
mod recorder;
mod request;

use crate::context::{
    ApproximateTokenCounter, ContextAssembler, ContextAssembly, ContextAssemblyError,
    ContextAssemblyInput, ContextPolicy, TokenCounter,
};
use crate::error::AgentError;
use crate::event::AgentEvent;
use crate::queue::{RunId, RunQueue, RunRequest};
use crate::schema::OutputSchema;
use crate::store::Store;
use crate::tool::{Tool, ToolRegistrationError, ToolStorage};
use context_compaction::ContextCompactor;
use provider_events::{AgentProviderEventHandler, ProviderEventOutcome};
use provider_stream::{run_retryable_provider_stream, ProviderRetryConfig};
use recorder::RunRecorder;
use request::{effective_system_prompt, validate_provider_capabilities};
use spectacular_llms::{
    Cancellation, LlmProvider, ProviderRequest, ProviderToolCall, ToolManifest,
};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex, RwLock,
};
use std::time::Duration;
use tokio::sync::mpsc;

const DEFAULT_SYSTEM_PROMPT: &str = "";
const DEFAULT_MAX_PROVIDER_RETRIES: usize = 2;
pub(super) const LENGTH_CONTINUATION_PROMPT: &str = "Continue from exactly where the previous assistant response stopped. Do not repeat any earlier text, and do not explain that you are continuing.";

/// Coordinates queued agent runs, provider calls, tool execution, and event storage.
#[derive(Debug)]
pub struct Agent<P, C = ApproximateTokenCounter> {
    provider: P,
    token_counter: C,
    queue: Arc<RunQueue>,
    store: Mutex<Store>,
    active_cancellation: Arc<Mutex<Option<Cancellation>>>,
    tools: RwLock<ToolStorage>,
    config: AgentConfig,
}

/// Runtime settings that shape provider requests and agent behavior.
#[derive(Clone, Debug, PartialEq)]
pub struct AgentConfig {
    /// System instructions prepended to each provider request.
    pub system_prompt: String,
    /// Optional provider model identifier to request.
    pub model: Option<String>,
    /// Requires final provider responses to include usage metadata.
    pub require_usage_metadata: bool,
    /// Requires final provider responses to include reasoning metadata.
    pub require_reasoning_metadata: bool,
    /// Requests reasoning deltas or metadata from capable providers.
    pub include_reasoning: bool,
    /// Optional provider-specific reasoning effort setting.
    pub reasoning_effort: Option<String>,
    /// Optional schema used to validate the final assistant response.
    pub output_schema: Option<OutputSchema>,
    /// Retries after the first provider attempt for transient provider/network failures.
    pub max_provider_retries: usize,
    /// Optional pause between transient provider retries.
    pub provider_retry_delay: Duration,
    /// Token budget and automatic context compaction policy.
    pub context_policy: ContextPolicy,
}

/// Streaming handle for a background agent run.
pub struct AgentRunStream {
    receiver: mpsc::Receiver<AgentEvent>,
    cancellation: Cancellation,
    queue: Arc<RunQueue>,
    completed: Arc<AtomicBool>,
    active_cancellation: Arc<Mutex<Option<Cancellation>>>,
}

impl AgentRunStream {
    /// Creates a stream handle around a background run channel and cancellation state.
    fn new(
        receiver: mpsc::Receiver<AgentEvent>,
        cancellation: Cancellation,
        queue: Arc<RunQueue>,
        completed: Arc<AtomicBool>,
        active_cancellation: Arc<Mutex<Option<Cancellation>>>,
    ) -> Self {
        Self {
            receiver,
            cancellation,
            queue,
            completed,
            active_cancellation,
        }
    }

    /// Receives the next persisted run event and marks terminal streams completed.
    pub async fn next(&mut self) -> Option<AgentEvent> {
        let event = self.receiver.recv().await;
        if is_terminal_event(&event) {
            self.completed.store(true, Ordering::SeqCst);
        }

        event
    }

    /// Cancels the active run and any queued waiters unless the stream already completed.
    pub fn cancel(&self) {
        if self.completed.load(Ordering::SeqCst) {
            return;
        }

        self.cancellation.cancel();
        if let Some(active_cancellation) = self.active_cancellation.lock().unwrap().as_ref() {
            active_cancellation.cancel();
        }
        self.queue.cancel_pending_now();
    }
}

impl Drop for AgentRunStream {
    /// Cancels unfinished background work when the stream handle is dropped.
    fn drop(&mut self) {
        self.cancel();
    }
}

/// Returns true when an optional event represents a terminal run state.
fn is_terminal_event(event: &Option<AgentEvent>) -> bool {
    matches!(
        event,
        Some(AgentEvent::Finished { .. } | AgentEvent::Error { .. } | AgentEvent::Cancelled { .. })
    )
}

impl Default for AgentConfig {
    /// Builds the default agent configuration with safe provider retry and context policy defaults.
    fn default() -> Self {
        Self {
            system_prompt: DEFAULT_SYSTEM_PROMPT.to_owned(),
            model: None,
            require_usage_metadata: true,
            require_reasoning_metadata: false,
            include_reasoning: false,
            reasoning_effort: None,
            output_schema: None,
            max_provider_retries: DEFAULT_MAX_PROVIDER_RETRIES,
            provider_retry_delay: Duration::ZERO,
            context_policy: ContextPolicy::default(),
        }
    }
}

impl<P, C> Agent<P, C>
where
    P: LlmProvider,
    C: TokenCounter + Clone,
{
    /// Creates an agent with explicit configuration and a custom token counter.
    pub fn with_config_and_token_counter(
        provider: P,
        config: AgentConfig,
        token_counter: C,
    ) -> Self {
        Self::with_config_store_and_token_counter(provider, config, Store::default(), token_counter)
    }

    /// Returns a copy of the agent with a replacement tool registry.
    pub fn with_tools(self, tools: ToolStorage) -> Self {
        Self {
            tools: RwLock::new(tools),
            ..self
        }
    }

    /// Adds a prompt to the run queue without immediately executing it.
    pub fn enqueue_prompt(&mut self, prompt: impl Into<String>) -> RunId {
        self.queue.enqueue_prompt(prompt)
    }

    /// Registers a callable tool that can be advertised to capable providers.
    pub fn register_tool<T>(&self, tool: T) -> Result<(), ToolRegistrationError>
    where
        T: Tool + 'static,
    {
        self.tools.write().unwrap().register(tool)
    }

    /// Enqueues and executes one prompt, returning the completed run id or stored error.
    pub async fn run(&self, prompt: impl Into<String>) -> Result<RunId, AgentError> {
        let run = self
            .queue
            .enqueue_and_wait(prompt)
            .await
            .map_err(|_| AgentError::CancellationError)?;
        let cancellation = self.start_cancellation();
        let result = self.run_request(run, cancellation, None).await;
        self.finish_run(&result).await;
        result
    }

    /// Executes the next queued prompt if one exists.
    pub async fn run_next(&mut self) -> Result<RunId, AgentError> {
        let run = self
            .queue
            .start_next()
            .await
            .ok_or(AgentError::EmptyRunQueue)?;
        let cancellation = self.start_cancellation();
        let result = self.run_request(run, cancellation, None).await;
        self.finish_run(&result).await;
        result
    }

    /// Cancels the currently active run and pending queued prompts.
    pub async fn cancel_active(&self) -> bool {
        let cancelled = {
            let active_cancellation = self.active_cancellation.lock().unwrap();
            let Some(cancellation) = active_cancellation.as_ref() else {
                return false;
            };

            cancellation.cancel();
            true
        };
        self.queue.cancel_pending().await;
        cancelled
    }

    /// Returns a snapshot of all stored events.
    pub fn events(&self) -> Vec<AgentEvent> {
        self.store.lock().unwrap().events().to_vec()
    }

    /// Returns a clone of the current event store.
    pub fn store(&self) -> Store {
        self.store.lock().unwrap().clone()
    }

    /// Returns manifests for tools currently registered with the agent.
    pub fn tool_manifests(&self) -> Vec<ToolManifest> {
        self.tools.read().unwrap().manifests()
    }
}

impl<P, C> Agent<P, C>
where
    P: LlmProvider + 'static,
    C: TokenCounter + Clone + Send + Sync + 'static,
{
    /// Starts a background run and returns a stream of stored events.
    pub fn run_stream(self: Arc<Self>, prompt: impl Into<String>) -> AgentRunStream {
        let cancellation = Cancellation::default();
        let (sender, receiver) = mpsc::channel(128);
        let completed = Arc::new(AtomicBool::new(false));
        let stream = AgentRunStream::new(
            receiver,
            cancellation.clone(),
            Arc::clone(&self.queue),
            Arc::clone(&completed),
            Arc::clone(&self.active_cancellation),
        );
        let prompt = prompt.into();
        let agent = Arc::clone(&self);

        tokio::spawn(async move {
            let result = match agent.queue.enqueue_and_wait(prompt).await {
                Ok(run) => {
                    agent.activate_cancellation(cancellation.clone());
                    agent.run_request(run, cancellation, Some(sender)).await
                }
                Err(_) => Err(AgentError::CancellationError),
            };
            agent.finish_run(&result).await;
            completed.store(true, Ordering::SeqCst);
        });

        stream
    }
}

impl<P, C> Agent<P, C>
where
    P: LlmProvider,
    C: TokenCounter + Clone,
{
    /// Executes a started run through context assembly, provider streaming, and tool loops.
    async fn run_request(
        &self,
        run: RunRequest,
        cancellation: Cancellation,
        sender: Option<mpsc::Sender<AgentEvent>>,
    ) -> Result<RunId, AgentError> {
        let mut recorder = RunRecorder::new(self, cancellation, sender);
        let prompt = run.prompt().to_owned();
        let run_event_start = self.store.lock().unwrap().checkpoint() + 1;
        recorder.record(AgentEvent::user_prompt(prompt)).await?;

        let capabilities = self.provider.capabilities();
        let tool_manifests = self.tools.read().unwrap().manifests();
        let has_tools = !tool_manifests.is_empty();
        if let Err(error) = validate_provider_capabilities(capabilities, &self.config, has_tools) {
            return recorder.record_error(error).await;
        }

        let mut continuing_after_length = false;
        let mut summary_passes_for_request = 0usize;
        loop {
            recorder.return_if_cancelled().await?;
            let assembled_context = {
                let store = self.store.lock().unwrap();
                let assembler = ContextAssembler::new(
                    self.token_counter.clone(),
                    self.config.context_policy.clone(),
                );
                assembler.assemble(ContextAssemblyInput {
                    system_prompt: effective_system_prompt(
                        &self.config.system_prompt,
                        &tool_manifests,
                    ),
                    store: &store,
                    provider_limits: capabilities.context_limits,
                    continuation_prompt: continuing_after_length
                        .then_some(LENGTH_CONTINUATION_PROMPT),
                })
            };
            let messages = match assembled_context {
                Ok(ContextAssembly::Ready(context)) => context.messages,
                Ok(ContextAssembly::NeedsSummary(summary_request)) => {
                    if summary_passes_for_request
                        >= self.config.context_policy.max_summary_passes_per_request
                    {
                        let agent_error = AgentError::ContextLimitError {
                            reason: format!(
                                "context remains above compaction threshold after {summary_passes_for_request} summary pass(es)"
                            ),
                        };
                        return recorder.record_error(agent_error).await;
                    }

                    summary_passes_for_request += 1;
                    let summary_event = match ContextCompactor::new(self, capabilities)
                        .compact(&mut recorder, &summary_request)
                        .await
                    {
                        Ok(summary_event) => summary_event,
                        Err(AgentError::CancellationError) => {
                            return Err(AgentError::CancellationError);
                        }
                        Err(error) => return recorder.record_error(error).await,
                    };
                    recorder.record(summary_event).await?;
                    continue;
                }
                Err(error) => return recorder.record_error(context_assembly_error(error)).await,
            };

            let mut request = ProviderRequest::new(messages);
            if let Some(model) = self.config.model.clone() {
                request = request.with_model(model);
            }
            request.capabilities = capabilities;
            request.flags.allow_tools = has_tools;
            request.flags.include_reasoning =
                self.config.include_reasoning || self.config.reasoning_effort.is_some();
            request.flags.reasoning_effort = self.config.reasoning_effort.clone();
            request.tools = tool_manifests.clone();

            let mut handler = AgentProviderEventHandler::new(self, run_event_start);
            let provider_outcome = match run_retryable_provider_stream(
                &self.provider,
                request,
                &mut recorder,
                self.provider_retry_config(),
                &mut handler,
            )
            .await
            {
                Ok(outcome) => outcome,
                Err(AgentError::CancellationError) => {
                    return Err(AgentError::CancellationError);
                }
                Err(error) => return recorder.record_error(error).await,
            };

            match provider_outcome {
                ProviderEventOutcome::ContinueStream => {
                    unreachable!("provider attempts only finish on terminal outcomes")
                }
                ProviderEventOutcome::CompleteRun => break,
                ProviderEventOutcome::ContinueCompletion => {
                    continuing_after_length = true;
                    summary_passes_for_request = 0;
                    continue;
                }
                ProviderEventOutcome::ExecuteTools(tool_calls) => {
                    continuing_after_length = false;
                    summary_passes_for_request = 0;
                    self.execute_tool_calls(&mut recorder, &tool_calls).await?;
                }
            }
        }

        Ok(run.id())
    }

    /// Creates a cancellation token and marks it as the active run cancellation.
    fn start_cancellation(&self) -> Cancellation {
        let cancellation = Cancellation::default();
        self.activate_cancellation(cancellation.clone());
        cancellation
    }

    /// Replaces the active cancellation token with the provided token.
    fn activate_cancellation(&self, cancellation: Cancellation) {
        *self.active_cancellation.lock().unwrap() = Some(cancellation);
    }

    /// Clears active cancellation and marks the queue state according to run outcome.
    async fn finish_run(&self, result: &Result<RunId, AgentError>) {
        *self.active_cancellation.lock().unwrap() = None;
        if matches!(result, Err(AgentError::CancellationError)) {
            self.queue.finish_cancelled_active().await;
            return;
        }

        self.queue.finish_active().await;
    }

    /// Executes provider-requested tools and records request/result transcript events.
    async fn execute_tool_calls(
        &self,
        recorder: &mut RunRecorder<'_, P, C>,
        tool_calls: &[ProviderToolCall],
    ) -> Result<(), AgentError> {
        let tools = self.tools.read().unwrap().clone();
        for tool_call in tool_calls {
            recorder.return_if_cancelled().await?;

            recorder
                .record(AgentEvent::assistant_tool_call_request(
                    tool_call.id.clone(),
                    tool_call.name.clone(),
                    tool_call.arguments.clone(),
                ))
                .await?;
            let result = tools.execute(tool_call, recorder.cancellation()).await;
            recorder.return_if_cancelled().await?;

            recorder
                .record(AgentEvent::tool_result(
                    tool_call.id.clone(),
                    tool_call.name.clone(),
                    result,
                ))
                .await?;
        }

        Ok(())
    }

    /// Returns the retry policy used by visible provider calls and hidden summary calls.
    pub(super) fn provider_retry_config(&self) -> ProviderRetryConfig {
        ProviderRetryConfig {
            max_provider_retries: self.config.max_provider_retries,
            provider_retry_delay: self.config.provider_retry_delay,
        }
    }
}

/// Maps context assembly failures into the stable agent-level context error.
fn context_assembly_error(error: ContextAssemblyError) -> AgentError {
    AgentError::ContextLimitError {
        reason: error.to_string(),
    }
}
