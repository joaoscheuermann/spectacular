mod constructors;
mod context_compaction;
mod context_usage;
mod provider_events;
mod provider_stream;
mod recorder;
mod request;
mod run_control;
mod run_loop;
mod run_stream;

use crate::context::{ContextPolicy, TokenCounter, TokenCounterChoice};
use crate::error::AgentError;
use crate::event::AgentEvent;
use crate::queue::{RunId, RunQueue};
use crate::schema::OutputSchema;
use crate::store::Store;
use crate::tool::{Tool, ToolRegistrationError, ToolStorage};
use run_control::RunControl;
pub use run_stream::AgentRunStream;
use spectacular_llms::{LlmProvider, ToolManifest};
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
pub struct Agent<P, C = TokenCounterChoice> {
    provider: P,
    token_counter: C,
    queue: Arc<RunQueue>,
    store: Mutex<Store>,
    active_run_control: Arc<Mutex<Option<Arc<RunControl>>>>,
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
        self.run_with_prompt_event_id(prompt, None::<String>).await
    }

    /// Enqueues and executes one prompt with a caller-owned prompt event ID.
    pub async fn run_with_prompt_event_id(
        &self,
        prompt: impl Into<String>,
        prompt_event_id: Option<impl Into<String>>,
    ) -> Result<RunId, AgentError> {
        let run = self
            .queue
            .enqueue_and_wait_with_event_id(prompt, prompt_event_id)
            .await
            .map_err(|_| AgentError::CancellationError)?;
        let control = self.start_run_control();
        let result = self.run_request_with_abort(run, control, None).await;
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
        let control = self.start_run_control();
        let result = self.run_request_with_abort(run, control, None).await;
        self.finish_run(&result).await;
        result
    }

    /// Cancels the currently active run and pending queued prompts.
    pub async fn cancel_active(&self) -> bool {
        let cancelled = {
            let active_control = self.active_run_control.lock().unwrap();
            let Some(control) = active_control.as_ref() else {
                return false;
            };

            control.cancel();
            true
        };
        self.queue.cancel_pending().await;
        cancelled
    }

    /// Hard-aborts the currently active run and pending queued prompts.
    pub async fn hard_abort_active(&self) -> bool {
        let cancelled = {
            let active_control = self.active_run_control.lock().unwrap();
            let Some(control) = active_control.as_ref() else {
                return false;
            };

            control.request_hard_abort();
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
        self.run_stream_with_prompt_event_id(prompt, None::<String>)
    }

    /// Starts a background run with a caller-owned prompt event ID and returns a stream of stored events.
    pub fn run_stream_with_prompt_event_id(
        self: Arc<Self>,
        prompt: impl Into<String>,
        prompt_event_id: Option<impl Into<String>>,
    ) -> AgentRunStream {
        let control = Arc::new(RunControl::new());
        let (sender, receiver) = mpsc::channel(128);
        let completed = Arc::new(AtomicBool::new(false));
        let stream = AgentRunStream::new(
            receiver,
            Arc::clone(&control),
            Arc::clone(&self.queue),
            Arc::clone(&completed),
            Arc::clone(&self.active_run_control),
        );
        let prompt = prompt.into();
        let prompt_event_id = prompt_event_id.map(Into::into);
        let agent = Arc::clone(&self);

        tokio::spawn(async move {
            let mut acquired_active_run = false;
            let result = match agent
                .queue
                .enqueue_and_wait_with_event_id(prompt, prompt_event_id)
                .await
            {
                Ok(run) => {
                    acquired_active_run = true;
                    agent.activate_run_control(Arc::clone(&control));
                    agent
                        .run_request_with_abort(run, control, Some(sender))
                        .await
                }
                Err(_) => Err(AgentError::CancellationError),
            };
            if acquired_active_run {
                agent.finish_run(&result).await;
            }
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
    /// Creates run control state and marks it as active.
    fn start_run_control(&self) -> Arc<RunControl> {
        let control = Arc::new(RunControl::new());
        self.activate_run_control(Arc::clone(&control));
        control
    }

    /// Replaces the active run control with the provided handle.
    fn activate_run_control(&self, control: Arc<RunControl>) {
        *self.active_run_control.lock().unwrap() = Some(control);
    }

    /// Clears active run control and marks the queue state according to run outcome.
    async fn finish_run(&self, result: &Result<RunId, AgentError>) {
        *self.active_run_control.lock().unwrap() = None;
        if matches!(result, Err(AgentError::CancellationError)) {
            self.queue.finish_cancelled_active().await;
            return;
        }

        self.queue.finish_active().await;
    }
}
