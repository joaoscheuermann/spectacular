use crate::context::{provider_messages_from_store, validate_context_limits};
use crate::error::AgentError;
use crate::event::AgentEvent;
use crate::queue::{RunId, RunQueue, RunRequest};
use crate::schema::OutputSchema;
use crate::store::Store;
use crate::tool::{Tool, ToolRegistrationError, ToolStorage};
use spectacular_llms::{
    Cancellation, FinishReason, LlmProvider, ProviderCapabilities, ProviderError, ProviderFinished,
    ProviderMessage, ProviderMessageRole, ProviderRequest, ProviderStreamEvent, ProviderToolCall,
    ToolManifest,
};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex, RwLock,
};
use std::time::Duration;
use tokio::sync::mpsc;

const DEFAULT_SYSTEM_PROMPT: &str = "";
const DEFAULT_MAX_PROVIDER_RETRIES: usize = 2;
const LENGTH_CONTINUATION_PROMPT: &str = "Continue from exactly where the previous assistant response stopped. Do not repeat any earlier text, and do not explain that you are continuing.";
const PROVIDER_CANCELLED_MESSAGE: &str = "provider cancelled the response";

#[derive(Debug)]
pub struct Agent<P> {
    provider: P,
    queue: Arc<RunQueue>,
    store: Mutex<Store>,
    active_cancellation: Arc<Mutex<Option<Cancellation>>>,
    tools: RwLock<ToolStorage>,
    config: AgentConfig,
}

#[derive(Clone, Debug, PartialEq)]
pub struct AgentConfig {
    pub system_prompt: String,
    pub model: Option<String>,
    pub require_usage_metadata: bool,
    pub require_reasoning_metadata: bool,
    pub include_reasoning: bool,
    pub reasoning_effort: Option<String>,
    pub output_schema: Option<OutputSchema>,
    /// Retries after the first provider attempt for transient provider/network failures.
    pub max_provider_retries: usize,
    /// Optional pause between transient provider retries.
    pub provider_retry_delay: Duration,
}

pub struct AgentRunStream {
    receiver: mpsc::Receiver<AgentEvent>,
    cancellation: Cancellation,
    queue: Arc<RunQueue>,
    completed: Arc<AtomicBool>,
    active_cancellation: Arc<Mutex<Option<Cancellation>>>,
}

enum ProviderEventOutcome {
    ContinueStream,
    CompleteRun,
    ContinueCompletion,
    ExecuteTools(Vec<ProviderToolCall>),
}

impl AgentRunStream {
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

    pub async fn next(&mut self) -> Option<AgentEvent> {
        let event = self.receiver.recv().await;
        if matches!(
            event,
            Some(
                AgentEvent::Finished { .. }
                    | AgentEvent::Error { .. }
                    | AgentEvent::Cancelled { .. }
            )
        ) {
            self.completed.store(true, Ordering::SeqCst);
        }

        event
    }

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
    fn drop(&mut self) {
        self.cancel();
    }
}

impl Default for AgentConfig {
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
        }
    }
}

impl<P> Agent<P>
where
    P: LlmProvider,
{
    pub fn new(provider: P) -> Self {
        Self::with_config(provider, AgentConfig::default())
    }

    pub fn with_config(provider: P, config: AgentConfig) -> Self {
        Self::with_config_and_store(provider, config, Store::default())
    }

    pub fn with_config_and_store(provider: P, config: AgentConfig, store: Store) -> Self {
        Self {
            provider,
            queue: Arc::new(RunQueue::default()),
            store: Mutex::new(store),
            active_cancellation: Arc::new(Mutex::new(None)),
            tools: RwLock::new(ToolStorage::default()),
            config,
        }
    }

    pub fn with_tools(self, tools: ToolStorage) -> Self {
        Self {
            tools: RwLock::new(tools),
            ..self
        }
    }

    pub fn enqueue_prompt(&mut self, prompt: impl Into<String>) -> RunId {
        self.queue.enqueue_prompt(prompt)
    }

    pub fn register_tool<T>(&self, tool: T) -> Result<(), ToolRegistrationError>
    where
        T: Tool + 'static,
    {
        self.tools.write().unwrap().register(tool)
    }

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

    pub fn events(&self) -> Vec<AgentEvent> {
        self.store.lock().unwrap().events().to_vec()
    }

    pub fn store(&self) -> Store {
        self.store.lock().unwrap().clone()
    }

    pub fn tool_manifests(&self) -> Vec<ToolManifest> {
        self.tools.read().unwrap().manifests()
    }
}

impl<P> Agent<P>
where
    P: LlmProvider + 'static,
{
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

impl<P> Agent<P>
where
    P: LlmProvider,
{
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
        loop {
            recorder.return_if_cancelled().await?;
            let mut messages = {
                let store = self.store.lock().unwrap();
                provider_messages_from_store(
                    effective_system_prompt(&self.config.system_prompt, &tool_manifests),
                    &store,
                )
            };
            if continuing_after_length {
                messages.push(ProviderMessage::user(LENGTH_CONTINUATION_PROMPT));
            }
            if let Err(error) = validate_context_limits(&messages, capabilities.context_limits) {
                let agent_error = AgentError::ContextLimitError {
                    reason: error.to_string(),
                };
                return recorder.record_error(agent_error).await;
            }

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

            let mut provider_retries = 0;
            let provider_outcome = 'provider_attempt: loop {
                recorder.return_if_cancelled().await?;

                let mut stream = match self
                    .provider
                    .stream_completion(request.clone(), recorder.cancellation())
                    .await
                {
                    Ok(stream) => stream,
                    Err(ProviderError::CancellationError) => {
                        recorder.cancel();
                        recorder.return_if_cancelled().await?;
                        return Err(AgentError::CancellationError);
                    }
                    Err(error)
                        if should_retry_provider_error(
                            &error,
                            provider_retries,
                            false,
                            &self.config,
                        ) =>
                    {
                        provider_retries += 1;
                        wait_before_provider_retry(&mut recorder, self.config.provider_retry_delay)
                            .await?;
                        continue;
                    }
                    Err(error) => return recorder.record_error(error.into()).await,
                };

                recorder.return_if_cancelled().await?;

                let mut saw_provider_event = false;
                while let Some(provider_event) = stream.next().await {
                    recorder.return_if_cancelled().await?;

                    let provider_event = match provider_event {
                        Ok(provider_event) => provider_event,
                        Err(ProviderError::CancellationError) => {
                            recorder.cancel();
                            recorder.return_if_cancelled().await?;
                            return Err(AgentError::CancellationError);
                        }
                        Err(error)
                            if should_retry_provider_error(
                                &error,
                                provider_retries,
                                saw_provider_event,
                                &self.config,
                            ) =>
                        {
                            provider_retries += 1;
                            wait_before_provider_retry(
                                &mut recorder,
                                self.config.provider_retry_delay,
                            )
                            .await?;
                            continue 'provider_attempt;
                        }
                        Err(error) => return recorder.record_error(error.into()).await,
                    };

                    saw_provider_event = true;
                    match self
                        .record_provider_event(&mut recorder, provider_event, run_event_start)
                        .await?
                    {
                        ProviderEventOutcome::ContinueStream => {}
                        ProviderEventOutcome::CompleteRun => {
                            break 'provider_attempt ProviderEventOutcome::CompleteRun;
                        }
                        ProviderEventOutcome::ContinueCompletion => {
                            break 'provider_attempt ProviderEventOutcome::ContinueCompletion;
                        }
                        ProviderEventOutcome::ExecuteTools(tool_calls) => {
                            break 'provider_attempt ProviderEventOutcome::ExecuteTools(tool_calls);
                        }
                    }
                }

                break ProviderEventOutcome::CompleteRun;
            };

            match provider_outcome {
                ProviderEventOutcome::ContinueStream => {
                    unreachable!("provider attempts only break on terminal outcomes")
                }
                ProviderEventOutcome::CompleteRun => break,
                ProviderEventOutcome::ContinueCompletion => {
                    continuing_after_length = true;
                    continue;
                }
                ProviderEventOutcome::ExecuteTools(tool_calls) => {
                    continuing_after_length = false;
                    self.execute_tool_calls(&mut recorder, &tool_calls).await?;
                }
            }
        }

        Ok(run.id())
    }

    async fn record_provider_event(
        &self,
        recorder: &mut RunRecorder<'_, P>,
        provider_event: ProviderStreamEvent,
        run_event_start: usize,
    ) -> Result<ProviderEventOutcome, AgentError> {
        match provider_event {
            ProviderStreamEvent::MessageDelta(delta) => {
                recorder.record(AgentEvent::MessageDelta(delta)).await?;
            }
            ProviderStreamEvent::ReasoningDelta(delta) => {
                recorder.record(AgentEvent::ReasoningDelta(delta)).await?;
            }
            ProviderStreamEvent::Finished(finished) => {
                return self
                    .record_finished_event(
                        recorder,
                        finished,
                        run_event_start,
                        self.config.require_usage_metadata,
                        self.config.output_schema.as_ref(),
                    )
                    .await;
            }
        }

        Ok(ProviderEventOutcome::ContinueStream)
    }

    async fn record_finished_event(
        &self,
        recorder: &mut RunRecorder<'_, P>,
        finished: ProviderFinished,
        run_event_start: usize,
        require_usage_metadata: bool,
        output_schema: Option<&OutputSchema>,
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
            FinishReason::ToolCalls => {
                if finished.tool_calls.is_empty() {
                    return recorder
                        .record_error(AgentError::MalformedProviderResponse {
                            reason: "tool-call finish did not include tool calls".to_owned(),
                        })
                        .await;
                }

                if let Some(tool_call) = finished.tool_calls.iter().find(|tool_call| {
                    tool_call.id.trim().is_empty() || tool_call.name.trim().is_empty()
                }) {
                    return recorder
                        .record_error(AgentError::MalformedProviderResponse {
                            reason: format!(
                                "tool call has empty id or name: id={:?}, name={:?}",
                                tool_call.id, tool_call.name
                            ),
                        })
                        .await;
                }

                Ok(ProviderEventOutcome::ExecuteTools(finished.tool_calls))
            }
            FinishReason::Length => {
                if !finished.tool_calls.is_empty() {
                    return recorder
                        .record_error(AgentError::MalformedProviderResponse {
                            reason: "non-tool finish included tool calls".to_owned(),
                        })
                        .await;
                }

                Ok(ProviderEventOutcome::ContinueCompletion)
            }
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
            FinishReason::Stop => {
                if !finished.tool_calls.is_empty() {
                    return recorder
                        .record_error(AgentError::MalformedProviderResponse {
                            reason: "non-tool finish included tool calls".to_owned(),
                        })
                        .await;
                }

                if require_usage_metadata && finished.usage.is_none() {
                    return recorder
                        .record_error(AgentError::MalformedProviderResponse {
                            reason: "provider omitted required usage metadata".to_owned(),
                        })
                        .await;
                }

                if let Some(output_schema) = output_schema {
                    let final_response = {
                        let store = self.store.lock().unwrap();
                        final_assistant_response(&store.events()[run_event_start..])
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
                }

                recorder.record(AgentEvent::finished(finished)).await?;
                Ok(ProviderEventOutcome::CompleteRun)
            }
        }
    }

    fn start_cancellation(&self) -> Cancellation {
        let cancellation = Cancellation::default();
        self.activate_cancellation(cancellation.clone());
        cancellation
    }

    fn activate_cancellation(&self, cancellation: Cancellation) {
        *self.active_cancellation.lock().unwrap() = Some(cancellation);
    }

    async fn finish_run(&self, result: &Result<RunId, AgentError>) {
        *self.active_cancellation.lock().unwrap() = None;
        if matches!(result, Err(AgentError::CancellationError)) {
            self.queue.finish_cancelled_active().await;
            return;
        }

        self.queue.finish_active().await;
    }

    async fn execute_tool_calls(
        &self,
        recorder: &mut RunRecorder<'_, P>,
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
}

struct RunRecorder<'a, P>
where
    P: LlmProvider,
{
    agent: &'a Agent<P>,
    cancellation: Cancellation,
    sender: Option<mpsc::Sender<AgentEvent>>,
    cancelled_recorded: bool,
}

impl<'a, P> RunRecorder<'a, P>
where
    P: LlmProvider,
{
    fn new(
        agent: &'a Agent<P>,
        cancellation: Cancellation,
        sender: Option<mpsc::Sender<AgentEvent>>,
    ) -> Self {
        Self {
            agent,
            cancellation,
            sender,
            cancelled_recorded: false,
        }
    }

    fn cancellation(&self) -> Cancellation {
        self.cancellation.clone()
    }

    fn cancel(&self) {
        self.cancellation.cancel();
    }

    async fn record(&mut self, event: AgentEvent) -> Result<(), AgentError> {
        {
            self.agent.store.lock().unwrap().append(event.clone());
        }

        let Some(sender) = self.sender.as_ref() else {
            return Ok(());
        };

        if sender.send(event).await.is_ok() {
            return Ok(());
        }

        self.cancellation.cancel();
        self.record_cancelled().await;
        Err(AgentError::CancellationError)
    }

    async fn record_error<T>(&mut self, error: AgentError) -> Result<T, AgentError> {
        if self.cancellation.is_cancelled() {
            self.record_cancelled().await;
            return Err(AgentError::CancellationError);
        }

        self.record(AgentEvent::error(error.to_string())).await?;
        Err(error)
    }

    async fn return_if_cancelled(&mut self) -> Result<(), AgentError> {
        if !self.cancellation.is_cancelled() {
            return Ok(());
        }

        self.record_cancelled().await;
        Err(AgentError::CancellationError)
    }

    async fn record_cancelled(&mut self) {
        self.record_cancelled_with_reason("active run cancelled")
            .await;
    }

    async fn record_cancelled_with_reason(&mut self, reason: impl Into<String>) {
        self.agent.queue.cancel_pending().await;
        if self.cancelled_recorded {
            return;
        }
        self.cancelled_recorded = true;

        let event = AgentEvent::cancelled(reason);
        {
            self.agent.store.lock().unwrap().append(event.clone());
        }

        if let Some(sender) = self.sender.as_ref() {
            let _ = sender.send(event).await;
        }
    }
}

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

fn effective_system_prompt(base_prompt: &str, tool_manifests: &[ToolManifest]) -> String {
    if tool_manifests.is_empty() {
        return base_prompt.to_owned();
    }

    let tool_summary = format_tool_summary(tool_manifests);
    if base_prompt.trim().is_empty() {
        return tool_summary;
    }

    format!("{base_prompt}\n\n{tool_summary}")
}

fn format_tool_summary(tool_manifests: &[ToolManifest]) -> String {
    let tools = tool_manifests
        .iter()
        .map(|manifest| format!("* {} - {}", manifest.name, manifest.description))
        .collect::<Vec<_>>()
        .join("\n");

    format!("You have access to the following tools:\n{tools}")
}

fn validate_provider_capabilities(
    capabilities: ProviderCapabilities,
    config: &AgentConfig,
    needs_tools: bool,
) -> Result<(), AgentError> {
    if !capabilities.streaming {
        return Err(AgentError::CapabilityMismatch {
            capability: "streaming",
        });
    }

    if config.require_usage_metadata && !capabilities.usage_metadata {
        return Err(AgentError::CapabilityMismatch {
            capability: "usage_metadata",
        });
    }

    if (config.include_reasoning || config.reasoning_effort.is_some()) && !capabilities.reasoning {
        return Err(AgentError::CapabilityMismatch {
            capability: "reasoning",
        });
    }

    if config.require_reasoning_metadata && !capabilities.reasoning_metadata {
        return Err(AgentError::CapabilityMismatch {
            capability: "reasoning_metadata",
        });
    }

    if needs_tools && !capabilities.tool_calls {
        return Err(AgentError::CapabilityMismatch {
            capability: "tool_calls",
        });
    }

    if config.output_schema.is_some() && !capabilities.structured_output {
        return Err(AgentError::CapabilityMismatch {
            capability: "structured_output",
        });
    }

    Ok(())
}

async fn wait_before_provider_retry<P>(
    recorder: &mut RunRecorder<'_, P>,
    delay: Duration,
) -> Result<(), AgentError>
where
    P: LlmProvider,
{
    recorder.return_if_cancelled().await?;
    if !delay.is_zero() {
        tokio::time::sleep(delay).await;
    }
    recorder.return_if_cancelled().await
}

fn should_retry_provider_error(
    error: &ProviderError,
    retries_used: usize,
    saw_provider_event: bool,
    config: &AgentConfig,
) -> bool {
    if saw_provider_event || retries_used >= config.max_provider_retries {
        return false;
    }

    matches!(
        error,
        ProviderError::NetworkError { .. } | ProviderError::ProviderUnavailable { .. }
    )
}
