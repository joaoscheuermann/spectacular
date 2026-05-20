use serde_json::{json, Value};
use spectacular_agent::{Cancellation, Tool, ToolExecution, ToolManifest};
use spectacular_llms::{
    provider_by_id, FinishReason, LlmProvider, MessageDelta, Model, ProviderCall,
    ProviderCapabilities, ProviderContextLimits, ProviderError, ProviderFinished, ProviderMetadata,
    ProviderRequest, ProviderStream, ProviderStreamEvent, UsageMetadata, ValidationMode,
    OPENROUTER_PROVIDER_ID,
};
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc, Mutex,
};

/// Returns provider capabilities advertised by this implementation.
pub(crate) fn capabilities() -> ProviderCapabilities {
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

/// Builds a finished stream event with usage metadata for the supplied reason.
pub(crate) fn finished_with_reason(finish_reason: FinishReason) -> ProviderFinished {
    ProviderFinished {
        finish_reason,
        tool_calls: Vec::new(),
        usage: Some(UsageMetadata {
            input_tokens: Some(1),
            output_tokens: Some(1),
            total_tokens: Some(2),
        }),
        reasoning: None,
    }
}

/// Builds a stop-finished stream event that includes usage metadata.
pub(crate) fn finished_stop_with_usage() -> ProviderFinished {
    finished_with_reason(FinishReason::Stop)
}

/// Builds a length-finished stream event without usage metadata.
pub(crate) fn finished_length_without_usage() -> ProviderFinished {
    ProviderFinished {
        finish_reason: FinishReason::Length,
        tool_calls: Vec::new(),
        usage: None,
        reasoning: None,
    }
}

/// Builds a fake provider-unavailable error for retry tests.
pub(crate) fn provider_unavailable() -> ProviderError {
    ProviderError::ProviderUnavailable {
        provider_name: "Fake".to_owned(),
    }
}

/// Builds the recovered provider stream used by retry tests.
pub(crate) fn recovered_events() -> Vec<Result<ProviderStreamEvent, ProviderError>> {
    vec![
        Ok(ProviderStreamEvent::MessageDelta(MessageDelta::assistant(
            "recovered",
        ))),
        Ok(ProviderStreamEvent::Finished(finished_stop_with_usage())),
    ]
}

#[derive(Clone, Debug)]
pub(crate) struct FakeProvider {
    pub(crate) calls: Arc<AtomicUsize>,
    pub(crate) capabilities: ProviderCapabilities,
    pub(crate) events: Vec<ProviderStreamEvent>,
}

impl FakeProvider {
    /// Builds a fake provider that streams the supplied assistant text.
    pub(crate) fn text(content: impl Into<String>) -> Self {
        Self::with_events(vec![
            ProviderStreamEvent::MessageDelta(MessageDelta::assistant(content)),
            ProviderStreamEvent::Finished(finished_stop_with_usage()),
        ])
    }

    /// Builds a fake provider with the supplied stream events.
    pub(crate) fn with_events(events: Vec<ProviderStreamEvent>) -> Self {
        Self {
            calls: Arc::new(AtomicUsize::new(0)),
            capabilities: capabilities(),
            events,
        }
    }
}

impl LlmProvider for FakeProvider {
    /// Returns provider metadata for this implementation.
    fn metadata(&self) -> ProviderMetadata {
        provider_by_id(OPENROUTER_PROVIDER_ID).unwrap()
    }

    /// Validates provider-specific input for the requested validation mode.
    fn validate(&self, _mode: ValidationMode, _value: &str) -> Result<(), ProviderError> {
        Ok(())
    }

    /// Fetches model metadata available to the supplied API key.
    fn models(&self, _api_key: &str) -> Result<Vec<Model>, ProviderError> {
        Ok(Vec::new())
    }

    /// Returns provider capabilities advertised by this implementation.
    fn capabilities(&self) -> ProviderCapabilities {
        self.capabilities
    }

    /// Starts a streaming completion request and returns the provider call future.
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
pub(crate) struct RecordingProvider {
    pub(crate) calls: Arc<AtomicUsize>,
    pub(crate) requests: Arc<Mutex<Vec<ProviderRequest>>>,
    call_attempts: Vec<ProviderAttempt>,
}

#[derive(Clone, Debug)]
pub(crate) enum ProviderAttempt {
    Error(ProviderError),
    Events(Vec<Result<ProviderStreamEvent, ProviderError>>),
}

impl RecordingProvider {
    /// Creates a new value from the supplied inputs.
    pub(crate) fn new(call_events: Vec<Vec<ProviderStreamEvent>>) -> Self {
        Self {
            calls: Arc::new(AtomicUsize::new(0)),
            requests: Arc::new(Mutex::new(Vec::new())),
            call_attempts: call_events
                .into_iter()
                .map(|events| ProviderAttempt::Events(events.into_iter().map(Ok).collect()))
                .collect(),
        }
    }

    /// Builds a recording provider with explicit call attempts.
    pub(crate) fn with_attempts(call_attempts: Vec<ProviderAttempt>) -> Self {
        Self {
            calls: Arc::new(AtomicUsize::new(0)),
            requests: Arc::new(Mutex::new(Vec::new())),
            call_attempts,
        }
    }
}

impl LlmProvider for RecordingProvider {
    /// Returns provider metadata for this implementation.
    fn metadata(&self) -> ProviderMetadata {
        provider_by_id(OPENROUTER_PROVIDER_ID).unwrap()
    }

    /// Validates provider-specific input for the requested validation mode.
    fn validate(&self, _mode: ValidationMode, _value: &str) -> Result<(), ProviderError> {
        Ok(())
    }

    /// Fetches model metadata available to the supplied API key.
    fn models(&self, _api_key: &str) -> Result<Vec<Model>, ProviderError> {
        Ok(Vec::new())
    }

    /// Returns provider capabilities advertised by this implementation.
    fn capabilities(&self) -> ProviderCapabilities {
        capabilities()
    }

    /// Starts a streaming completion request and returns the provider call future.
    fn stream_completion<'a>(
        &'a self,
        request: ProviderRequest,
        _cancellation: Cancellation,
    ) -> ProviderCall<'a> {
        Box::pin(async move {
            let call_index = self.calls.fetch_add(1, Ordering::SeqCst);
            self.requests.lock().unwrap().push(request);
            let attempt = self
                .call_attempts
                .get(call_index)
                .cloned()
                .unwrap_or_else(|| {
                    ProviderAttempt::Events(vec![
                        Ok(ProviderStreamEvent::MessageDelta(MessageDelta::assistant(
                            "done",
                        ))),
                        Ok(ProviderStreamEvent::Finished(finished_stop_with_usage())),
                    ])
                });
            match attempt {
                ProviderAttempt::Error(error) => Err(error),
                ProviderAttempt::Events(events) => Ok(ProviderStream::from_events(events)),
            }
        })
    }
}

#[derive(Clone, Debug)]
pub(crate) struct FailingProvider {
    pub(crate) calls: Arc<AtomicUsize>,
}

impl LlmProvider for FailingProvider {
    /// Returns provider metadata for this implementation.
    fn metadata(&self) -> ProviderMetadata {
        provider_by_id(OPENROUTER_PROVIDER_ID).unwrap()
    }

    /// Validates provider-specific input for the requested validation mode.
    fn validate(&self, _mode: ValidationMode, _value: &str) -> Result<(), ProviderError> {
        Ok(())
    }

    /// Fetches model metadata available to the supplied API key.
    fn models(&self, _api_key: &str) -> Result<Vec<Model>, ProviderError> {
        Ok(Vec::new())
    }

    /// Returns provider capabilities advertised by this implementation.
    fn capabilities(&self) -> ProviderCapabilities {
        capabilities()
    }

    /// Starts a streaming completion request and returns the provider call future.
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
pub(crate) struct StreamErrorProvider;

impl LlmProvider for StreamErrorProvider {
    /// Returns provider metadata for this implementation.
    fn metadata(&self) -> ProviderMetadata {
        provider_by_id(OPENROUTER_PROVIDER_ID).unwrap()
    }

    /// Validates provider-specific input for the requested validation mode.
    fn validate(&self, _mode: ValidationMode, _value: &str) -> Result<(), ProviderError> {
        Ok(())
    }

    /// Fetches model metadata available to the supplied API key.
    fn models(&self, _api_key: &str) -> Result<Vec<Model>, ProviderError> {
        Ok(Vec::new())
    }

    /// Returns provider capabilities advertised by this implementation.
    fn capabilities(&self) -> ProviderCapabilities {
        capabilities()
    }

    /// Starts a streaming completion request and returns the provider call future.
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
pub(crate) struct PartialThenPendingEventProvider {
    pub(crate) partial_sent: Arc<tokio::sync::Notify>,
}

impl LlmProvider for PartialThenPendingEventProvider {
    /// Returns provider metadata for this implementation.
    fn metadata(&self) -> ProviderMetadata {
        provider_by_id(OPENROUTER_PROVIDER_ID).unwrap()
    }

    /// Validates provider-specific input for the requested validation mode.
    fn validate(&self, _mode: ValidationMode, _value: &str) -> Result<(), ProviderError> {
        Ok(())
    }

    /// Fetches model metadata available to the supplied API key.
    fn models(&self, _api_key: &str) -> Result<Vec<Model>, ProviderError> {
        Ok(Vec::new())
    }

    /// Returns provider capabilities advertised by this implementation.
    fn capabilities(&self) -> ProviderCapabilities {
        capabilities()
    }

    /// Starts a stream that emits partial output, then waits for cancellation before another event.
    fn stream_completion<'a>(
        &'a self,
        _request: ProviderRequest,
        cancellation: Cancellation,
    ) -> ProviderCall<'a> {
        let partial_sent = Arc::clone(&self.partial_sent);
        Box::pin(async move {
            let (sender, receiver) = tokio::sync::mpsc::channel(4);
            tokio::spawn(async move {
                let _ = sender
                    .send(Ok(ProviderStreamEvent::MessageDelta(
                        MessageDelta::assistant("partial"),
                    )))
                    .await;
                partial_sent.notify_waiters();

                while !cancellation.is_cancelled() {
                    tokio::time::sleep(std::time::Duration::from_millis(5)).await;
                }

                let _ = sender
                    .send(Ok(ProviderStreamEvent::MessageDelta(
                        MessageDelta::assistant("ignored"),
                    )))
                    .await;
            });

            Ok(ProviderStream::new(receiver))
        })
    }
}

#[derive(Clone, Debug)]
pub(crate) struct SlowProvider {
    pub(crate) started: Arc<tokio::sync::Notify>,
}

impl LlmProvider for SlowProvider {
    /// Returns provider metadata for this implementation.
    fn metadata(&self) -> ProviderMetadata {
        provider_by_id(OPENROUTER_PROVIDER_ID).unwrap()
    }

    /// Validates provider-specific input for the requested validation mode.
    fn validate(&self, _mode: ValidationMode, _value: &str) -> Result<(), ProviderError> {
        Ok(())
    }

    /// Fetches model metadata available to the supplied API key.
    fn models(&self, _api_key: &str) -> Result<Vec<Model>, ProviderError> {
        Ok(Vec::new())
    }

    /// Returns provider capabilities advertised by this implementation.
    fn capabilities(&self) -> ProviderCapabilities {
        capabilities()
    }

    /// Starts a streaming completion request and returns the provider call future.
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

#[derive(Clone, Debug)]
pub(crate) struct EchoTool;

impl Tool for EchoTool {
    /// Returns the stable tool name used in manifests and calls.
    fn name(&self) -> &str {
        "echo"
    }

    /// Builds the tool manifest exposed to provider requests.
    fn manifest(&self) -> ToolManifest {
        ToolManifest::new(
            self.name(),
            "Echo parsed arguments as provider-visible JSON.",
            json!({"type": "object", "additionalProperties": true}),
        )
    }

    /// Executes the tool with the provided arguments and cancellation handle.
    fn execute<'a>(&'a self, arguments: Value, _cancellation: Cancellation) -> ToolExecution<'a> {
        Box::pin(async move { Ok(arguments.to_string()) })
    }
}

#[derive(Clone, Debug)]
pub(crate) struct BuiltInStyleWriteTool;

impl Tool for BuiltInStyleWriteTool {
    /// Returns the stable tool name used in manifests and calls.
    fn name(&self) -> &str {
        "write"
    }

    /// Builds the tool manifest exposed to provider requests.
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

    /// Executes the tool with the provided arguments and cancellation handle.
    fn execute<'a>(&'a self, arguments: Value, _cancellation: Cancellation) -> ToolExecution<'a> {
        Box::pin(async move {
            Ok(json!({
                "success": true,
                "path": arguments.get("path").and_then(Value::as_str).unwrap_or_default()
            })
            .to_string())
        })
    }
}
