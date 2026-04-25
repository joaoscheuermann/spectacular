use crate::context::{provider_messages_from_store, validate_context_limits};
use crate::error::AgentError;
use crate::event::AgentEvent;
use crate::queue::{RunId, RunQueue, RunRequest};
use crate::schema::OutputSchema;
use crate::store::Store;
use crate::tool::{format_tool_call_request, Tool, ToolStorage};
use spectacular_llms::{
    Cancellation, FinishReason, LlmProvider, ProviderCapabilities, ProviderError, ProviderFinished,
    ProviderMessageRole, ProviderRequest, ProviderStreamEvent, ProviderToolCall,
};
use std::sync::Mutex;
use tokio::sync::RwLock;

const DEFAULT_SYSTEM_PROMPT: &str = "You are Spectacular, a focused coding assistant.";

#[derive(Debug)]
pub struct Agent<P> {
    provider: P,
    queue: RunQueue,
    store: Mutex<Store>,
    active_cancellation: Mutex<Option<Cancellation>>,
    tools: RwLock<ToolStorage>,
    config: AgentConfig,
}

#[derive(Clone, Debug, PartialEq)]
pub struct AgentConfig {
    pub system_prompt: String,
    pub require_usage_metadata: bool,
    pub require_reasoning_metadata: bool,
    pub include_reasoning: bool,
    pub output_schema: Option<OutputSchema>,
}

impl Default for AgentConfig {
    fn default() -> Self {
        Self {
            system_prompt: DEFAULT_SYSTEM_PROMPT.to_owned(),
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
        Self {
            provider,
            queue: RunQueue::default(),
            store: Mutex::new(Store::default()),
            active_cancellation: Mutex::new(None),
            tools: RwLock::new(ToolStorage::default()),
            config,
        }
    }

    pub fn enqueue_prompt(&mut self, prompt: impl Into<String>) -> RunId {
        self.queue.enqueue_prompt(prompt)
    }

    pub async fn register_tool<T>(&self, tool: T)
    where
        T: Tool + 'static,
    {
        self.tools.write().await.register(tool).await;
    }

    pub async fn run(&self, prompt: impl Into<String>) -> Result<RunId, AgentError> {
        let run = self
            .queue
            .enqueue_and_wait(prompt)
            .await
            .map_err(|_| AgentError::CancellationError)?;
        let cancellation = self.start_cancellation();
        let result = self.run_request(run, cancellation).await;
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
        let result = self.run_request(run, cancellation).await;
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

    async fn run_request(
        &self,
        run: RunRequest,
        cancellation: Cancellation,
    ) -> Result<RunId, AgentError> {
        let prompt = run.prompt().to_owned();
        let run_event_checkpoint = {
            let mut store = self.store.lock().unwrap();
            let checkpoint = store.checkpoint();
            store.append(AgentEvent::user_prompt(prompt));
            checkpoint
        };
        let run_event_start = run_event_checkpoint + 1;

        let capabilities = self.provider.capabilities();
        let has_tools = !self.tools.read().await.is_empty();
        if let Err(error) = validate_provider_capabilities(capabilities, &self.config, has_tools) {
            return Err(self.store_error(error));
        }

        loop {
            self.return_if_cancelled(&cancellation, run_event_checkpoint)?;
            let messages = {
                let store = self.store.lock().unwrap();
                provider_messages_from_store(self.config.system_prompt.clone(), &store)
            };
            if let Err(error) = validate_context_limits(&messages, capabilities.context_limits) {
                let agent_error = AgentError::ContextLimitError {
                    reason: error.to_string(),
                };
                return Err(self.store_error(agent_error));
            }

            let mut request = ProviderRequest::new(messages);
            request.capabilities = capabilities;
            request.flags.allow_tools = has_tools;
            request.flags.include_reasoning = self.config.include_reasoning;

            let stream = match self
                .provider
                .stream_completion(request, cancellation.clone())
                .await
            {
                Ok(stream) => stream,
                Err(ProviderError::CancellationError) => {
                    cancellation.cancel();
                    self.return_if_cancelled(&cancellation, run_event_checkpoint)?;
                    return Err(AgentError::CancellationError);
                }
                Err(error) => return Err(self.store_error(error.into())),
            };
            self.return_if_cancelled(&cancellation, run_event_checkpoint)?;

            let mut requested_tools = Vec::new();
            for provider_event in stream {
                self.return_if_cancelled(&cancellation, run_event_checkpoint)?;
                let provider_event = match provider_event {
                    Ok(provider_event) => provider_event,
                    Err(ProviderError::CancellationError) => {
                        self.return_if_cancelled(&Cancellation::cancelled(), run_event_checkpoint)?;
                        return Err(AgentError::CancellationError);
                    }
                    Err(error) => return Err(self.store_error(error.into())),
                };

                if let Some(tool_calls) =
                    self.record_provider_event(provider_event, run_event_start)?
                {
                    requested_tools = tool_calls;
                }
            }

            if requested_tools.is_empty() {
                break;
            }

            self.execute_tool_calls(&requested_tools, cancellation.clone(), run_event_checkpoint)
                .await?;
        }

        Ok(run.id())
    }

    fn store_error(&self, error: AgentError) -> AgentError {
        self.store
            .lock()
            .unwrap()
            .append(AgentEvent::error(error.to_string()));
        error
    }

    fn record_provider_event(
        &self,
        provider_event: ProviderStreamEvent,
        run_event_start: usize,
    ) -> Result<Option<Vec<ProviderToolCall>>, AgentError> {
        let mut store = self.store.lock().unwrap();
        match provider_event {
            ProviderStreamEvent::MessageDelta(delta) => {
                store.append(AgentEvent::MessageDelta(delta));
            }
            ProviderStreamEvent::ReasoningDelta(delta) => {
                store.append(AgentEvent::ReasoningDelta(delta));
            }
            ProviderStreamEvent::Finished(finished) => {
                return Self::record_finished_event(
                    &mut store,
                    finished,
                    run_event_start,
                    self.config.require_usage_metadata,
                    self.config.output_schema.as_ref(),
                );
            }
        }

        Ok(None)
    }

    fn record_finished_event(
        store: &mut Store,
        finished: ProviderFinished,
        run_event_start: usize,
        require_usage_metadata: bool,
        output_schema: Option<&OutputSchema>,
    ) -> Result<Option<Vec<ProviderToolCall>>, AgentError> {
        if let Some(usage) = finished.usage {
            store.append(AgentEvent::UsageMetadata(usage));
        }
        if let Some(reasoning) = finished.reasoning.clone() {
            store.append(AgentEvent::ReasoningMetadata(reasoning));
        }

        if finished.finish_reason == FinishReason::ToolCalls {
            if finished.tool_calls.is_empty() {
                return Err(store_finished_error(
                    store,
                    AgentError::MalformedProviderResponse {
                        reason: "tool-call finish did not include tool calls".to_owned(),
                    },
                ));
            }

            if let Some(tool_call) = finished.tool_calls.iter().find(|tool_call| {
                tool_call.id.trim().is_empty() || tool_call.name.trim().is_empty()
            }) {
                return Err(store_finished_error(
                    store,
                    AgentError::MalformedProviderResponse {
                        reason: format!(
                            "tool call has empty id or name: id={:?}, name={:?}",
                            tool_call.id, tool_call.name
                        ),
                    },
                ));
            }

            return Ok(Some(finished.tool_calls));
        }

        if !finished.tool_calls.is_empty() {
            return Err(store_finished_error(
                store,
                AgentError::MalformedProviderResponse {
                    reason: "non-tool finish included tool calls".to_owned(),
                },
            ));
        }

        if require_usage_metadata && finished.usage.is_none() {
            return Err(store_finished_error(
                store,
                AgentError::MalformedProviderResponse {
                    reason: "provider omitted required usage metadata".to_owned(),
                },
            ));
        }

        if let Some(output_schema) = output_schema {
            let final_response = final_assistant_response(&store.events()[run_event_start..]);
            if let Err(error) = output_schema.validate_response(&final_response) {
                let message = error.to_string();
                store.append(AgentEvent::validation_error(message.clone()));
                return Err(store_finished_error(
                    store,
                    AgentError::ValidationError { message },
                ));
            }
        }

        store.append(AgentEvent::finished(finished));
        Ok(None)
    }

    fn start_cancellation(&self) -> Cancellation {
        let cancellation = Cancellation::default();
        *self.active_cancellation.lock().unwrap() = Some(cancellation.clone());
        cancellation
    }

    async fn finish_run(&self, result: &Result<RunId, AgentError>) {
        *self.active_cancellation.lock().unwrap() = None;
        if matches!(result, Err(AgentError::CancellationError)) {
            self.queue.finish_cancelled_active().await;
            return;
        }

        self.queue.finish_active().await;
    }

    fn return_if_cancelled(
        &self,
        cancellation: &Cancellation,
        checkpoint: usize,
    ) -> Result<(), AgentError> {
        if !cancellation.is_cancelled() {
            return Ok(());
        }

        let mut store = self.store.lock().unwrap();
        store.rollback(checkpoint);
        store.append(AgentEvent::cancelled("active run cancelled"));
        Err(AgentError::CancellationError)
    }

    async fn execute_tool_calls(
        &self,
        tool_calls: &[ProviderToolCall],
        cancellation: Cancellation,
        checkpoint: usize,
    ) -> Result<(), AgentError> {
        let tools = self.tools.read().await.clone();
        for tool_call in tool_calls {
            if cancellation.is_cancelled() {
                return self.return_if_cancelled(&cancellation, checkpoint);
            }

            self.store
                .lock()
                .unwrap()
                .append(AgentEvent::assistant_tool_call_request(
                    format_tool_call_request(tool_call),
                ));
            let result = tools.execute(tool_call, cancellation.clone()).await;
            if cancellation.is_cancelled() {
                return self.return_if_cancelled(&cancellation, checkpoint);
            }

            self.store
                .lock()
                .unwrap()
                .append(AgentEvent::tool_result(result));
        }

        Ok(())
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

fn store_finished_error(store: &mut Store, error: AgentError) -> AgentError {
    store.append(AgentEvent::error(error.to_string()));
    error
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
    use crate::{OutputSchema, Tool, ToolExecution};
    use serde_json::{json, Value};
    use spectacular_llms::{
        provider_by_id, MessageDelta, Model, ProviderCall, ProviderContextLimits, ProviderMessage,
        ProviderMetadata, ProviderStream, UsageMetadata, ValidationMode, OPENROUTER_PROVIDER_ID,
    };
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
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
                let stream: ProviderStream = Box::new(events.into_iter().map(Ok));
                Ok(stream)
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
        futures::executor::block_on(agent.register_tool(EchoTool));
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

        fn execute<'a>(
            &'a self,
            arguments: Value,
            _cancellation: Cancellation,
        ) -> ToolExecution<'a> {
            Box::pin(async move { Ok(arguments) })
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
        futures::executor::block_on(agent.register_tool(EchoTool));
        agent.enqueue_prompt("prompt");

        futures::executor::block_on(agent.run_next()).unwrap();

        assert!(agent
            .events()
            .iter()
            .any(|event| matches!(event, AgentEvent::ToolResult { .. })));
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
        futures::executor::block_on(agent.register_tool(EchoTool));
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
    async fn cancelling_active_run_rolls_back_events_and_drops_waiters() {
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
            vec![AgentEvent::cancelled("active run cancelled")]
        );
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
                let stream: ProviderStream = Box::new(
                    vec![
                        Ok(ProviderStreamEvent::MessageDelta(MessageDelta::assistant(
                            "partial",
                        ))),
                        Err(ProviderError::ResponseParsingFailed {
                            provider_name: "Fake".to_owned(),
                            reason: "bad chunk".to_owned(),
                        }),
                    ]
                    .into_iter(),
                );
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
