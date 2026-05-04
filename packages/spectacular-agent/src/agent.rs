use crate::context::{provider_messages_from_store, validate_context_limits};
use crate::error::AgentError;
use crate::event::AgentEvent;
use crate::queue::{RunId, RunQueue, RunRequest};
use crate::schema::OutputSchema;
use crate::store::Store;
use crate::tool::{Tool, ToolRegistrationError, ToolStorage};
use spectacular_llms::{
    Cancellation, FinishReason, LlmProvider, ProviderCapabilities, ProviderError, ProviderFinished,
    ProviderMessageRole, ProviderRequest, ProviderStreamEvent, ProviderToolCall, ToolManifest,
};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex, RwLock,
};
use tokio::sync::mpsc;

const DEFAULT_SYSTEM_PROMPT: &str = "";

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
    pub output_schema: Option<OutputSchema>,
}

pub struct AgentRunStream {
    receiver: mpsc::Receiver<AgentEvent>,
    cancellation: Cancellation,
    queue: Arc<RunQueue>,
    completed: Arc<AtomicBool>,
    active_cancellation: Arc<Mutex<Option<Cancellation>>>,
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
            output_schema: None,
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

        loop {
            recorder.return_if_cancelled().await?;
            let messages = {
                let store = self.store.lock().unwrap();
                provider_messages_from_store(
                    effective_system_prompt(&self.config.system_prompt, &tool_manifests),
                    &store,
                )
            };
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
            request.flags.include_reasoning = self.config.include_reasoning;
            request.tools = tool_manifests.clone();

            let mut stream = match self
                .provider
                .stream_completion(request, recorder.cancellation())
                .await
            {
                Ok(stream) => stream,
                Err(ProviderError::CancellationError) => {
                    recorder.cancel();
                    recorder.return_if_cancelled().await?;
                    return Err(AgentError::CancellationError);
                }
                Err(error) => return recorder.record_error(error.into()).await,
            };

            recorder.return_if_cancelled().await?;

            let mut requested_tools = Vec::new();
            while let Some(provider_event) = stream.next().await {
                recorder.return_if_cancelled().await?;

                let provider_event = match provider_event {
                    Ok(provider_event) => provider_event,
                    Err(ProviderError::CancellationError) => {
                        recorder.cancel();
                        recorder.return_if_cancelled().await?;
                        return Err(AgentError::CancellationError);
                    }
                    Err(error) => return recorder.record_error(error.into()).await,
                };

                if let Some(tool_calls) = self
                    .record_provider_event(&mut recorder, provider_event, run_event_start)
                    .await?
                {
                    requested_tools = tool_calls;
                }
            }

            if requested_tools.is_empty() {
                break;
            }

            self.execute_tool_calls(&mut recorder, &requested_tools)
                .await?;
        }

        Ok(run.id())
    }

    async fn record_provider_event(
        &self,
        recorder: &mut RunRecorder<'_, P>,
        provider_event: ProviderStreamEvent,
        run_event_start: usize,
    ) -> Result<Option<Vec<ProviderToolCall>>, AgentError> {
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

        Ok(None)
    }

    async fn record_finished_event(
        &self,
        recorder: &mut RunRecorder<'_, P>,
        finished: ProviderFinished,
        run_event_start: usize,
        require_usage_metadata: bool,
        output_schema: Option<&OutputSchema>,
    ) -> Result<Option<Vec<ProviderToolCall>>, AgentError> {
        if let Some(usage) = finished.usage {
            recorder.record(AgentEvent::UsageMetadata(usage)).await?;
        }
        if let Some(reasoning) = finished.reasoning.clone() {
            recorder
                .record(AgentEvent::ReasoningMetadata(reasoning))
                .await?;
        }

        if finished.finish_reason == FinishReason::ToolCalls {
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

            return Ok(Some(finished.tool_calls));
        }

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
        Ok(None)
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
        self.agent.queue.cancel_pending().await;
        if self.cancelled_recorded {
            return;
        }
        self.cancelled_recorded = true;

        let event = AgentEvent::cancelled("active run cancelled");
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

    if config.include_reasoning && !capabilities.reasoning {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{OutputSchema, Tool, ToolExecution, ToolManifest};
    use serde_json::{json, Value};
    use spectacular_llms::{
        provider_by_id, MessageDelta, Model, ProviderCall, ProviderContextLimits, ProviderMessage,
        ProviderMetadata, ProviderStream, UsageMetadata, ValidationMode, OPENROUTER_PROVIDER_ID,
    };
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, Mutex,
    };

    fn capabilities() -> ProviderCapabilities {
        ProviderCapabilities {
            streaming: true,
            tool_calls: true,
            structured_output: true,
            reasoning: false,
            cancellation: true,
            usage_metadata: true,
            reasoning_metadata: false,
            context_limits: ProviderContextLimits::default(),
        }
    }

    fn finished_stop_with_usage() -> ProviderFinished {
        ProviderFinished {
            finish_reason: FinishReason::Stop,
            tool_calls: Vec::new(),
            usage: Some(UsageMetadata {
                input_tokens: Some(1),
                output_tokens: Some(1),
                total_tokens: Some(2),
            }),
            reasoning: None,
        }
    }

    #[derive(Clone, Debug)]
    struct FakeProvider {
        calls: Arc<AtomicUsize>,
        capabilities: ProviderCapabilities,
        events: Vec<ProviderStreamEvent>,
    }

    impl FakeProvider {
        fn text(content: impl Into<String>) -> Self {
            Self {
                calls: Arc::new(AtomicUsize::new(0)),
                capabilities: capabilities(),
                events: vec![
                    ProviderStreamEvent::MessageDelta(MessageDelta::assistant(content)),
                    ProviderStreamEvent::Finished(finished_stop_with_usage()),
                ],
            }
        }
    }

    impl LlmProvider for FakeProvider {
        fn metadata(&self) -> ProviderMetadata {
            provider_by_id(OPENROUTER_PROVIDER_ID).unwrap()
        }

        fn validate(&self, _mode: ValidationMode, _value: &str) -> Result<(), ProviderError> {
            Ok(())
        }

        fn models(&self, _api_key: &str) -> Result<Vec<Model>, ProviderError> {
            Ok(Vec::new())
        }

        fn capabilities(&self) -> ProviderCapabilities {
            self.capabilities
        }

        fn stream_completion<'a>(
            &'a self,
            request: ProviderRequest,
            _cancellation: Cancellation,
        ) -> ProviderCall<'a> {
            let calls = Arc::clone(&self.calls);
            let events = self.events.clone();
            Box::pin(async move {
                let call_index = calls.fetch_add(1, Ordering::SeqCst);
                assert!(!request.messages.is_empty());
                let events = if call_index > 0
                    && matches!(
                        events.first(),
                        Some(ProviderStreamEvent::Finished(ProviderFinished {
                            finish_reason: FinishReason::ToolCalls,
                            ..
                        }))
                    ) {
                    events[1..].to_vec()
                } else {
                    events
                };
                let stream = ProviderStream::from_events(events.into_iter().map(Ok));
                Ok(stream)
            })
        }
    }

    #[derive(Clone, Debug)]
    struct RecordingProvider {
        calls: Arc<AtomicUsize>,
        requests: Arc<Mutex<Vec<ProviderRequest>>>,
        call_events: Vec<Vec<ProviderStreamEvent>>,
    }

    impl RecordingProvider {
        fn new(call_events: Vec<Vec<ProviderStreamEvent>>) -> Self {
            Self {
                calls: Arc::new(AtomicUsize::new(0)),
                requests: Arc::new(Mutex::new(Vec::new())),
                call_events,
            }
        }
    }

    impl LlmProvider for RecordingProvider {
        fn metadata(&self) -> ProviderMetadata {
            provider_by_id(OPENROUTER_PROVIDER_ID).unwrap()
        }

        fn validate(&self, _mode: ValidationMode, _value: &str) -> Result<(), ProviderError> {
            Ok(())
        }

        fn models(&self, _api_key: &str) -> Result<Vec<Model>, ProviderError> {
            Ok(Vec::new())
        }

        fn capabilities(&self) -> ProviderCapabilities {
            capabilities()
        }

        fn stream_completion<'a>(
            &'a self,
            request: ProviderRequest,
            _cancellation: Cancellation,
        ) -> ProviderCall<'a> {
            Box::pin(async move {
                let call_index = self.calls.fetch_add(1, Ordering::SeqCst);
                self.requests.lock().unwrap().push(request);
                let events = self
                    .call_events
                    .get(call_index)
                    .cloned()
                    .unwrap_or_else(|| {
                        vec![
                            ProviderStreamEvent::MessageDelta(MessageDelta::assistant("done")),
                            ProviderStreamEvent::Finished(finished_stop_with_usage()),
                        ]
                    });
                Ok(ProviderStream::from_events(events.into_iter().map(Ok)))
            })
        }
    }

    #[test]
    fn no_tool_run_stores_events_in_order() {
        let mut agent = Agent::new(FakeProvider::text("hello"));
        agent.enqueue_prompt("prompt");

        futures::executor::block_on(agent.run_next()).unwrap();

        assert!(matches!(agent.events()[0], AgentEvent::UserPrompt { .. }));
        assert!(matches!(agent.events()[1], AgentEvent::MessageDelta(_)));
        assert!(matches!(agent.events()[2], AgentEvent::UsageMetadata(_)));
        assert!(matches!(agent.events()[3], AgentEvent::Finished { .. }));
    }

    #[test]
    fn structured_output_capability_mismatch_happens_before_provider_io() {
        let mut provider = FakeProvider::text(r#"{"status":"ready"}"#);
        provider.capabilities.structured_output = false;
        let calls = Arc::clone(&provider.calls);
        let mut agent = Agent::with_config(
            provider,
            AgentConfig {
                output_schema: Some(OutputSchema::new(json!({"type":"object"})).unwrap()),
                ..AgentConfig::default()
            },
        );
        agent.enqueue_prompt("prompt");

        let error = futures::executor::block_on(agent.run_next()).unwrap_err();

        assert!(matches!(
            error,
            AgentError::CapabilityMismatch {
                capability: "structured_output"
            }
        ));
        assert_eq!(calls.load(Ordering::SeqCst), 0);
        assert!(matches!(
            agent.events().last(),
            Some(AgentEvent::Error { .. })
        ));
    }

    #[test]
    fn usage_metadata_capability_mismatch_happens_before_provider_io() {
        let mut provider = FakeProvider::text("unused");
        provider.capabilities.usage_metadata = false;
        let calls = Arc::clone(&provider.calls);
        let mut agent = Agent::new(provider);
        agent.enqueue_prompt("prompt");

        let error = futures::executor::block_on(agent.run_next()).unwrap_err();

        assert!(matches!(
            error,
            AgentError::CapabilityMismatch {
                capability: "usage_metadata"
            }
        ));
        assert_eq!(calls.load(Ordering::SeqCst), 0);
        assert!(matches!(
            agent.events().last(),
            Some(AgentEvent::Error { .. })
        ));
    }

    #[test]
    fn reasoning_request_capability_mismatch_happens_before_provider_io() {
        let mut provider = FakeProvider::text("unused");
        provider.capabilities.reasoning = false;
        let calls = Arc::clone(&provider.calls);
        let mut agent = Agent::with_config(
            provider,
            AgentConfig {
                include_reasoning: true,
                ..AgentConfig::default()
            },
        );
        agent.enqueue_prompt("prompt");

        let error = futures::executor::block_on(agent.run_next()).unwrap_err();

        assert!(matches!(
            error,
            AgentError::CapabilityMismatch {
                capability: "reasoning"
            }
        ));
        assert_eq!(calls.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn reasoning_metadata_requirement_capability_mismatch_happens_before_provider_io() {
        let mut provider = FakeProvider::text("unused");
        provider.capabilities.reasoning_metadata = false;
        let calls = Arc::clone(&provider.calls);
        let mut agent = Agent::with_config(
            provider,
            AgentConfig {
                require_reasoning_metadata: true,
                ..AgentConfig::default()
            },
        );
        agent.enqueue_prompt("prompt");

        let error = futures::executor::block_on(agent.run_next()).unwrap_err();

        assert!(matches!(
            error,
            AgentError::CapabilityMismatch {
                capability: "reasoning_metadata"
            }
        ));
        assert_eq!(calls.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn registered_tool_requires_provider_tool_capability_before_provider_io() {
        let mut provider = FakeProvider::text("unused");
        provider.capabilities.tool_calls = false;
        let calls = Arc::clone(&provider.calls);
        let mut agent = Agent::new(provider);
        agent.register_tool(EchoTool).unwrap();
        agent.enqueue_prompt("prompt");

        let error = futures::executor::block_on(agent.run_next()).unwrap_err();

        assert!(matches!(
            error,
            AgentError::CapabilityMismatch {
                capability: "tool_calls"
            }
        ));
        assert_eq!(calls.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn structured_output_validation_rejects_invalid_response() {
        let mut agent = Agent::with_config(
            FakeProvider::text(r#"{"status":"draft"}"#),
            AgentConfig {
                output_schema: Some(
                    OutputSchema::new(json!({
                        "type": "object",
                        "properties": {"status": {"const": "ready"}},
                        "required": ["status"]
                    }))
                    .unwrap(),
                ),
                ..AgentConfig::default()
            },
        );
        agent.enqueue_prompt("prompt");

        let error = futures::executor::block_on(agent.run_next()).unwrap_err();

        assert!(matches!(error, AgentError::ValidationError { .. }));
        assert!(agent
            .events()
            .iter()
            .any(|event| matches!(event, AgentEvent::ValidationError { .. })));
    }

    #[test]
    fn structured_output_validation_allows_valid_response() {
        let mut agent = Agent::with_config(
            FakeProvider::text(r#"{"status":"ready"}"#),
            AgentConfig {
                output_schema: Some(
                    OutputSchema::new(json!({
                        "type": "object",
                        "properties": {"status": {"const": "ready"}},
                        "required": ["status"]
                    }))
                    .unwrap(),
                ),
                ..AgentConfig::default()
            },
        );
        agent.enqueue_prompt("prompt");

        futures::executor::block_on(agent.run_next()).unwrap();

        assert!(matches!(
            agent.events().last(),
            Some(AgentEvent::Finished {
                finish_reason: FinishReason::Stop
            })
        ));
    }

    #[derive(Clone, Debug)]
    struct EchoTool;

    impl Tool for EchoTool {
        fn name(&self) -> &str {
            "echo"
        }

        fn manifest(&self) -> ToolManifest {
            ToolManifest::new(
                self.name(),
                "Echo parsed arguments as provider-visible JSON.",
                json!({"type": "object", "additionalProperties": true}),
            )
        }

        fn execute<'a>(
            &'a self,
            arguments: Value,
            _cancellation: Cancellation,
        ) -> ToolExecution<'a> {
            Box::pin(async move { Ok(arguments.to_string()) })
        }
    }

    #[derive(Clone, Debug)]
    struct BuiltInStyleWriteTool;

    impl Tool for BuiltInStyleWriteTool {
        fn name(&self) -> &str {
            "write"
        }

        fn manifest(&self) -> ToolManifest {
            ToolManifest::new(
                self.name(),
                "Writes UTF-8 text to a file in the workspace.",
                json!({
                    "type": "object",
                    "properties": {
                        "path": {"type": "string"},
                        "content": {"type": "string"}
                    },
                    "required": ["path", "content"],
                    "additionalProperties": false
                }),
            )
        }

        fn execute<'a>(
            &'a self,
            arguments: Value,
            _cancellation: Cancellation,
        ) -> ToolExecution<'a> {
            Box::pin(async move {
                Ok(json!({
                    "success": true,
                    "path": arguments.get("path").and_then(Value::as_str).unwrap_or_default()
                })
                .to_string())
            })
        }
    }

    #[test]
    fn tool_call_loop_stores_tool_result_then_finishes() {
        let provider = FakeProvider {
            calls: Arc::new(AtomicUsize::new(0)),
            capabilities: capabilities(),
            events: vec![
                ProviderStreamEvent::Finished(ProviderFinished::tool_calls(vec![
                    ProviderToolCall::new("call-1", "echo", r#"{"ok":true}"#),
                ])),
                ProviderStreamEvent::MessageDelta(MessageDelta::assistant("done")),
                ProviderStreamEvent::Finished(finished_stop_with_usage()),
            ],
        };
        let mut agent = Agent::new(provider);
        agent.register_tool(EchoTool).unwrap();
        agent.enqueue_prompt("prompt");

        futures::executor::block_on(agent.run_next()).unwrap();

        assert!(agent
            .events()
            .iter()
            .any(|event| matches!(event, AgentEvent::ToolResult { .. })));
    }

    #[test]
    fn provider_request_includes_tool_manifest_and_tool_summary_system_prompt() {
        let provider = RecordingProvider::new(vec![vec![
            ProviderStreamEvent::MessageDelta(MessageDelta::assistant("done")),
            ProviderStreamEvent::Finished(finished_stop_with_usage()),
        ]]);
        let requests = Arc::clone(&provider.requests);
        let mut agent = Agent::new(provider);
        agent.register_tool(EchoTool).unwrap();
        agent.enqueue_prompt("prompt");

        futures::executor::block_on(agent.run_next()).unwrap();

        let requests = requests.lock().unwrap();
        let request = requests.first().unwrap();
        assert_eq!(request.tools.len(), 1);
        assert_eq!(request.tools[0].name, "echo");
        assert_eq!(
            request.tools[0].description,
            "Echo parsed arguments as provider-visible JSON."
        );
        assert_eq!(
            request.messages[0].content,
            "You have access to the following tools:\n* echo - Echo parsed arguments as provider-visible JSON."
        );
        assert!(!request.messages[0].content.contains("Parameters:"));
        assert!(!request.messages[0].content.contains("additionalProperties"));
        assert_eq!(agent.config.system_prompt, DEFAULT_SYSTEM_PROMPT);
    }

    #[test]
    fn tool_call_loop_emits_structured_tool_events_with_matching_id() {
        let provider = RecordingProvider::new(vec![
            vec![ProviderStreamEvent::Finished(ProviderFinished::tool_calls(
                vec![ProviderToolCall::new("call-1", "echo", r#"{"ok":true}"#)],
            ))],
            vec![
                ProviderStreamEvent::MessageDelta(MessageDelta::assistant("done")),
                ProviderStreamEvent::Finished(finished_stop_with_usage()),
            ],
        ]);
        let mut agent = Agent::new(provider);
        agent.register_tool(EchoTool).unwrap();
        agent.enqueue_prompt("prompt");

        futures::executor::block_on(agent.run_next()).unwrap();

        let tool_call = agent.events().into_iter().find_map(|event| match event {
            AgentEvent::AssistantToolCallRequest {
                tool_call_id,
                name,
                arguments,
            } => Some((tool_call_id, name, arguments)),
            _ => None,
        });
        let tool_result = agent.events().into_iter().find_map(|event| match event {
            AgentEvent::ToolResult {
                tool_call_id,
                name,
                content,
            } => Some((tool_call_id, name, content)),
            _ => None,
        });

        assert_eq!(
            tool_call,
            Some((
                "call-1".to_owned(),
                "echo".to_owned(),
                r#"{"ok":true}"#.to_owned()
            ))
        );
        assert_eq!(
            tool_result,
            Some((
                "call-1".to_owned(),
                "echo".to_owned(),
                r#"{"ok":true}"#.to_owned()
            ))
        );
    }

    #[test]
    fn follow_up_provider_request_replays_assistant_tool_call_and_tool_result() {
        let provider = RecordingProvider::new(vec![
            vec![ProviderStreamEvent::Finished(ProviderFinished::tool_calls(
                vec![ProviderToolCall::new("call-1", "echo", r#"{"ok":true}"#)],
            ))],
            vec![
                ProviderStreamEvent::MessageDelta(MessageDelta::assistant("done")),
                ProviderStreamEvent::Finished(finished_stop_with_usage()),
            ],
        ]);
        let requests = Arc::clone(&provider.requests);
        let mut agent = Agent::new(provider);
        agent.register_tool(EchoTool).unwrap();
        agent.enqueue_prompt("prompt");

        futures::executor::block_on(agent.run_next()).unwrap();

        let requests = requests.lock().unwrap();
        assert_eq!(requests.len(), 2);
        let initial = &requests[0];
        assert_eq!(initial.tools.len(), 1);
        assert_eq!(initial.tools[0].name, "echo");
        let follow_up = &requests[1];
        assert_eq!(follow_up.tools.len(), 1);
        assert_eq!(follow_up.tools[0].name, "echo");

        let assistant_tool_call = follow_up
            .messages
            .iter()
            .find(|message| !message.tool_calls.is_empty())
            .unwrap();
        let tool_result = follow_up
            .messages
            .iter()
            .find(|message| message.tool_call_id.as_deref() == Some("call-1"))
            .unwrap();

        assert_eq!(assistant_tool_call.tool_calls[0].id, "call-1");
        assert_eq!(assistant_tool_call.tool_calls[0].name, "echo");
        assert_eq!(
            assistant_tool_call.tool_calls[0].arguments,
            r#"{"ok":true}"#
        );
        assert_eq!(tool_result.content, r#"{"ok":true}"#);
        assert_eq!(tool_result.tool_call_id.as_deref(), Some("call-1"));
    }

    #[test]
    fn fake_provider_receives_built_in_style_tool_result_with_matching_id() {
        let provider = RecordingProvider::new(vec![
            vec![ProviderStreamEvent::Finished(ProviderFinished::tool_calls(
                vec![ProviderToolCall::new(
                    "call-write-1",
                    "write",
                    r#"{"path":"foo.txt","content":"hello"}"#,
                )],
            ))],
            vec![
                ProviderStreamEvent::MessageDelta(MessageDelta::assistant("done")),
                ProviderStreamEvent::Finished(finished_stop_with_usage()),
            ],
        ]);
        let requests = Arc::clone(&provider.requests);
        let mut agent = Agent::new(provider);
        agent.register_tool(BuiltInStyleWriteTool).unwrap();
        agent.enqueue_prompt("prompt");

        futures::executor::block_on(agent.run_next()).unwrap();

        let tool_result_event = agent.events().into_iter().find_map(|event| match event {
            AgentEvent::ToolResult {
                tool_call_id,
                name,
                content,
            } => Some((tool_call_id, name, content)),
            _ => None,
        });
        let requests = requests.lock().unwrap();
        let follow_up_tool_result = requests[1]
            .messages
            .iter()
            .find(|message| message.tool_call_id.as_deref() == Some("call-write-1"))
            .unwrap();

        assert_eq!(
            tool_result_event,
            Some((
                "call-write-1".to_owned(),
                "write".to_owned(),
                r#"{"path":"foo.txt","success":true}"#.to_owned()
            ))
        );
        assert_eq!(
            follow_up_tool_result.tool_call_id.as_deref(),
            Some("call-write-1")
        );
        assert_eq!(
            follow_up_tool_result.content,
            r#"{"path":"foo.txt","success":true}"#
        );
    }

    #[test]
    fn malformed_tool_call_finish_is_rejected_and_stored() {
        let provider = FakeProvider {
            calls: Arc::new(AtomicUsize::new(0)),
            capabilities: capabilities(),
            events: vec![ProviderStreamEvent::Finished(ProviderFinished::tool_calls(
                vec![ProviderToolCall::new("call-1", "", "{}")],
            ))],
        };
        let mut agent = Agent::new(provider);
        agent.register_tool(EchoTool).unwrap();
        agent.enqueue_prompt("prompt");

        let error = futures::executor::block_on(agent.run_next()).unwrap_err();

        assert!(matches!(
            error,
            AgentError::MalformedProviderResponse { .. }
        ));
        assert!(matches!(
            agent.events().last(),
            Some(AgentEvent::Error { .. })
        ));
    }

    #[test]
    fn missing_usage_metadata_on_final_response_is_rejected_and_stored() {
        let provider = FakeProvider {
            calls: Arc::new(AtomicUsize::new(0)),
            capabilities: capabilities(),
            events: vec![ProviderStreamEvent::Finished(ProviderFinished::stopped())],
        };
        let mut agent = Agent::new(provider);
        agent.enqueue_prompt("prompt");

        let error = futures::executor::block_on(agent.run_next()).unwrap_err();

        assert!(matches!(
            error,
            AgentError::MalformedProviderResponse { .. }
        ));
        assert!(matches!(
            agent.events().last(),
            Some(AgentEvent::Error { .. })
        ));
    }

    #[test]
    fn context_limit_failure_is_stored_before_provider_io() {
        let mut provider = FakeProvider::text("unused");
        provider.capabilities.context_limits = ProviderContextLimits {
            max_messages: Some(1),
            max_chars: None,
        };
        let calls = Arc::clone(&provider.calls);
        let mut agent = Agent::new(provider);
        agent.enqueue_prompt("prompt");

        let error = futures::executor::block_on(agent.run_next()).unwrap_err();

        assert!(matches!(error, AgentError::ContextLimitError { .. }));
        assert_eq!(calls.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn provider_errors_are_stored() {
        let provider = FailingProvider {
            calls: Arc::new(AtomicUsize::new(0)),
        };
        let mut agent = Agent::new(provider);
        agent.enqueue_prompt("prompt");

        let error = futures::executor::block_on(agent.run_next()).unwrap_err();

        assert!(matches!(error, AgentError::ProviderNetworkError { .. }));
        assert!(matches!(
            agent.events().last(),
            Some(AgentEvent::Error { .. })
        ));
    }

    #[test]
    fn stream_provider_errors_keep_partial_events_then_store_error() {
        let mut agent = Agent::new(StreamErrorProvider);
        agent.enqueue_prompt("prompt");

        let error = futures::executor::block_on(agent.run_next()).unwrap_err();

        assert!(matches!(error, AgentError::ProviderParsingError { .. }));
        assert!(agent.events().iter().any(|event| matches!(
            event,
            AgentEvent::MessageDelta(MessageDelta { content, .. }) if content == "partial"
        )));
        assert!(matches!(
            agent.events().last(),
            Some(AgentEvent::Error { .. })
        ));
    }

    #[tokio::test]
    async fn cancelling_active_run_keeps_partial_events_and_drops_waiters() {
        let provider = SlowProvider {
            started: Arc::new(tokio::sync::Notify::new()),
        };
        let agent = Arc::new(Agent::new(provider));
        let active = tokio::spawn({
            let agent = Arc::clone(&agent);
            async move { agent.run("active").await }
        });
        agent.provider.started.notified().await;

        let queued = tokio::spawn({
            let agent = Arc::clone(&agent);
            async move { agent.run("queued").await }
        });
        tokio::task::yield_now().await;

        assert!(agent.cancel_active().await);
        assert!(matches!(
            active.await.unwrap(),
            Err(AgentError::CancellationError)
        ));
        assert!(matches!(
            queued.await.unwrap(),
            Err(AgentError::CancellationError)
        ));
        assert_eq!(
            agent.events(),
            vec![
                AgentEvent::user_prompt("active"),
                AgentEvent::cancelled("active run cancelled")
            ]
        );
    }

    #[tokio::test]
    async fn streaming_run_emits_events_in_store_order() {
        let agent = Arc::new(Agent::new(FakeProvider::text("hello")));
        let mut stream = Arc::clone(&agent).run_stream("prompt");
        let mut events = Vec::new();

        while let Some(event) = stream.next().await {
            let terminal = matches!(
                event,
                AgentEvent::Finished { .. }
                    | AgentEvent::Error { .. }
                    | AgentEvent::Cancelled { .. }
            );
            events.push(event);
            if terminal {
                break;
            }
        }

        assert_eq!(events, agent.events());
    }

    #[tokio::test]
    async fn dropping_stream_cancels_active_run_and_pending_queue() {
        let provider = SlowProvider {
            started: Arc::new(tokio::sync::Notify::new()),
        };
        let agent = Arc::new(Agent::new(provider));
        let stream = Arc::clone(&agent).run_stream("active");
        agent.provider.started.notified().await;

        let queued = tokio::spawn({
            let agent = Arc::clone(&agent);
            async move { agent.run("queued").await }
        });
        tokio::task::yield_now().await;

        drop(stream);

        for _ in 0..20 {
            if matches!(agent.events().last(), Some(AgentEvent::Cancelled { .. })) {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        }

        assert!(matches!(
            queued.await.unwrap(),
            Err(AgentError::CancellationError)
        ));
        assert_eq!(
            agent.events(),
            vec![
                AgentEvent::user_prompt("active"),
                AgentEvent::cancelled("active run cancelled")
            ]
        );
    }

    #[tokio::test]
    async fn dropping_queued_stream_cancels_current_active_run() {
        let provider = SlowProvider {
            started: Arc::new(tokio::sync::Notify::new()),
        };
        let agent = Arc::new(Agent::new(provider));
        let active = tokio::spawn({
            let agent = Arc::clone(&agent);
            async move { agent.run("active").await }
        });
        agent.provider.started.notified().await;

        let queued_stream = Arc::clone(&agent).run_stream("queued");
        tokio::task::yield_now().await;

        drop(queued_stream);

        assert!(matches!(
            active.await.unwrap(),
            Err(AgentError::CancellationError)
        ));
        assert_eq!(
            agent.events(),
            vec![
                AgentEvent::user_prompt("active"),
                AgentEvent::cancelled("active run cancelled")
            ]
        );
    }

    #[tokio::test]
    async fn dropping_completed_stream_does_not_reject_next_run() {
        let agent = Arc::new(Agent::new(FakeProvider::text("hello")));
        let mut stream = Arc::clone(&agent).run_stream("first");

        while let Some(event) = stream.next().await {
            if matches!(
                event,
                AgentEvent::Finished { .. }
                    | AgentEvent::Error { .. }
                    | AgentEvent::Cancelled { .. }
            ) {
                break;
            }
        }

        drop(stream);

        agent.run("second").await.unwrap();
        assert!(agent.events().iter().any(|event| {
            matches!(event, AgentEvent::UserPrompt { content } if content == "second")
        }));
    }

    #[derive(Clone, Debug)]
    struct FailingProvider {
        calls: Arc<AtomicUsize>,
    }

    impl LlmProvider for FailingProvider {
        fn metadata(&self) -> ProviderMetadata {
            provider_by_id(OPENROUTER_PROVIDER_ID).unwrap()
        }

        fn validate(&self, _mode: ValidationMode, _value: &str) -> Result<(), ProviderError> {
            Ok(())
        }

        fn models(&self, _api_key: &str) -> Result<Vec<Model>, ProviderError> {
            Ok(Vec::new())
        }

        fn capabilities(&self) -> ProviderCapabilities {
            capabilities()
        }

        fn stream_completion<'a>(
            &'a self,
            _request: ProviderRequest,
            _cancellation: Cancellation,
        ) -> ProviderCall<'a> {
            let calls = Arc::clone(&self.calls);
            Box::pin(async move {
                calls.fetch_add(1, Ordering::SeqCst);
                Err(ProviderError::NetworkError {
                    provider_name: "Fake".to_owned(),
                    reason: "disconnect".to_owned(),
                })
            })
        }
    }

    #[derive(Clone, Debug)]
    struct StreamErrorProvider;

    impl LlmProvider for StreamErrorProvider {
        fn metadata(&self) -> ProviderMetadata {
            provider_by_id(OPENROUTER_PROVIDER_ID).unwrap()
        }

        fn validate(&self, _mode: ValidationMode, _value: &str) -> Result<(), ProviderError> {
            Ok(())
        }

        fn models(&self, _api_key: &str) -> Result<Vec<Model>, ProviderError> {
            Ok(Vec::new())
        }

        fn capabilities(&self) -> ProviderCapabilities {
            capabilities()
        }

        fn stream_completion<'a>(
            &'a self,
            _request: ProviderRequest,
            _cancellation: Cancellation,
        ) -> ProviderCall<'a> {
            Box::pin(async {
                let stream = ProviderStream::from_events(vec![
                    Ok(ProviderStreamEvent::MessageDelta(MessageDelta::assistant(
                        "partial",
                    ))),
                    Err(ProviderError::ResponseParsingFailed {
                        provider_name: "Fake".to_owned(),
                        reason: "bad chunk".to_owned(),
                    }),
                ]);
                Ok(stream)
            })
        }
    }

    #[derive(Clone, Debug)]
    struct SlowProvider {
        started: Arc<tokio::sync::Notify>,
    }

    impl LlmProvider for SlowProvider {
        fn metadata(&self) -> ProviderMetadata {
            provider_by_id(OPENROUTER_PROVIDER_ID).unwrap()
        }

        fn validate(&self, _mode: ValidationMode, _value: &str) -> Result<(), ProviderError> {
            Ok(())
        }

        fn models(&self, _api_key: &str) -> Result<Vec<Model>, ProviderError> {
            Ok(Vec::new())
        }

        fn capabilities(&self) -> ProviderCapabilities {
            capabilities()
        }

        fn stream_completion<'a>(
            &'a self,
            _request: ProviderRequest,
            cancellation: Cancellation,
        ) -> ProviderCall<'a> {
            let started = Arc::clone(&self.started);
            Box::pin(async move {
                started.notify_waiters();
                loop {
                    if cancellation.is_cancelled() {
                        return Err(ProviderError::CancellationError);
                    }

                    tokio::time::sleep(std::time::Duration::from_millis(5)).await;
                }
            })
        }
    }

    #[test]
    fn request_defaults_keep_flags_off_except_streaming() {
        let request = ProviderRequest::new(vec![ProviderMessage::user("hello")]);

        assert!(request.flags.stream);
        assert!(!request.flags.allow_tools);
        assert!(!request.flags.include_reasoning);
    }
}
