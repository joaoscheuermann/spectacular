use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::error::Error;
use std::fmt::{self, Display};
use std::future::Future;
use std::io::{BufRead, BufReader};
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::mpsc;

pub const OPENROUTER_PROVIDER_ID: &str = "openrouter";
pub const OPENAI_PROVIDER_ID: &str = "openai";
pub const GOOGLE_GEMINI_PROVIDER_ID: &str = "google-gemini";

const OPENROUTER_API_KEY_URL: &str = "https://openrouter.ai/api/v1/key";
const OPENROUTER_MODELS_URL: &str = "https://openrouter.ai/api/v1/models";
const OPENROUTER_CHAT_COMPLETIONS_URL: &str = "https://openrouter.ai/api/v1/chat/completions";

static PROVIDERS: &[ProviderMetadata] = &[
    ProviderMetadata::enabled(OPENROUTER_PROVIDER_ID, "OpenRouter"),
    ProviderMetadata::disabled(OPENAI_PROVIDER_ID, "OpenAI"),
    ProviderMetadata::disabled(GOOGLE_GEMINI_PROVIDER_ID, "Google - Gemini"),
];

/// Static provider metadata used by the CLI setup flow.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProviderMetadata {
    id: &'static str,
    display_name: &'static str,
    enabled: bool,
}

impl ProviderMetadata {
    const fn enabled(id: &'static str, display_name: &'static str) -> Self {
        Self {
            id,
            display_name,
            enabled: true,
        }
    }

    const fn disabled(id: &'static str, display_name: &'static str) -> Self {
        Self {
            id,
            display_name,
            enabled: false,
        }
    }

    /// Stable identifier persisted in configuration.
    pub fn id(self) -> &'static str {
        self.id
    }

    /// Human-readable provider name shown in setup screens.
    pub fn display_name(self) -> &'static str {
        self.display_name
    }

    /// Whether the provider can be selected in the current release.
    pub fn is_enabled(self) -> bool {
        self.enabled
    }
}

/// Returns all providers visible in the setup UI.
pub fn provider_registry() -> &'static [ProviderMetadata] {
    PROVIDERS
}

/// Looks up provider metadata by stable identifier.
pub fn provider_by_id(provider_id: &str) -> Option<ProviderMetadata> {
    PROVIDERS
        .iter()
        .copied()
        .find(|provider| provider.id == provider_id)
}

/// Returns the only enabled provider name for placeholder setup routes.
pub fn enabled_provider_name() -> &'static str {
    PROVIDERS
        .iter()
        .find(|provider| provider.enabled)
        .map(|provider| provider.display_name)
        .unwrap_or("None")
}

/// Provider capability used by setup flows and agent runs.
pub trait LlmProvider: Send + Sync {
    fn metadata(&self) -> ProviderMetadata;

    fn validate(&self, mode: ValidationMode, value: &str) -> Result<(), ProviderError>;

    fn models(&self, api_key: &str) -> Result<Vec<Model>, ProviderError>;

    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities::default()
    }

    fn stream_completion<'a>(
        &'a self,
        request: ProviderRequest,
        cancellation: Cancellation,
    ) -> ProviderCall<'a>;
}

/// Model metadata exposed by a provider.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Model {
    id: String,
    display_name: String,
}

impl Model {
    pub fn new(id: impl Into<String>, display_name: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            display_name: display_name.into(),
        }
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn display_name(&self) -> &str {
        &self.display_name
    }
}

/// Chat message sent to provider completion calls.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProviderMessage {
    pub role: ProviderMessageRole,
    pub content: String,
    pub tool_calls: Vec<ProviderToolCall>,
    pub tool_call_id: Option<String>,
}

impl ProviderMessage {
    pub fn system(content: impl Into<String>) -> Self {
        Self {
            role: ProviderMessageRole::System,
            content: content.into(),
            tool_calls: Vec::new(),
            tool_call_id: None,
        }
    }

    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: ProviderMessageRole::User,
            content: content.into(),
            tool_calls: Vec::new(),
            tool_call_id: None,
        }
    }

    pub fn assistant(content: impl Into<String>) -> Self {
        Self {
            role: ProviderMessageRole::Assistant,
            content: content.into(),
            tool_calls: Vec::new(),
            tool_call_id: None,
        }
    }

    pub fn assistant_tool_call(tool_call: ProviderToolCall) -> Self {
        Self::assistant_tool_calls(vec![tool_call])
    }

    pub fn assistant_tool_calls(tool_calls: Vec<ProviderToolCall>) -> Self {
        Self {
            role: ProviderMessageRole::Assistant,
            content: String::new(),
            tool_calls,
            tool_call_id: None,
        }
    }

    pub fn tool(content: impl Into<String>) -> Self {
        Self {
            role: ProviderMessageRole::Tool,
            content: content.into(),
            tool_calls: Vec::new(),
            tool_call_id: None,
        }
    }

    pub fn tool_result(tool_call_id: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            role: ProviderMessageRole::Tool,
            content: content.into(),
            tool_calls: Vec::new(),
            tool_call_id: Some(tool_call_id.into()),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProviderMessageRole {
    System,
    User,
    Assistant,
    Tool,
}

pub use ProviderMessage as Message;
pub use ProviderMessageRole as MessageRole;

/// Incremental assistant content returned by provider streams.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MessageDelta {
    pub role: ProviderMessageRole,
    pub content: String,
}

impl MessageDelta {
    pub fn assistant(content: impl Into<String>) -> Self {
        Self {
            role: ProviderMessageRole::Assistant,
            content: content.into(),
        }
    }
}

/// Incremental reasoning content returned by providers that expose it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReasoningDelta {
    pub content: String,
    pub metadata: Option<ReasoningMetadata>,
}

/// Provider capabilities advertised before a call.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ProviderCapabilities {
    pub streaming: bool,
    pub tool_calls: bool,
    pub structured_output: bool,
    pub reasoning: bool,
    pub cancellation: bool,
    pub usage_metadata: bool,
    pub reasoning_metadata: bool,
    pub context_limits: ProviderContextLimits,
}

/// Provider-advertised context bounds checked before provider I/O.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ProviderContextLimits {
    pub max_messages: Option<usize>,
    pub max_chars: Option<usize>,
}

/// Per-call flags passed to provider implementations.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProviderCallFlags {
    pub stream: bool,
    pub allow_tools: bool,
    pub include_reasoning: bool,
}

impl Default for ProviderCallFlags {
    fn default() -> Self {
        Self {
            stream: true,
            allow_tools: false,
            include_reasoning: false,
        }
    }
}

/// Provider-visible function tool schema.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ToolManifest {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
}

impl ToolManifest {
    pub fn new(
        name: impl Into<String>,
        description: impl Into<String>,
        parameters: serde_json::Value,
    ) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            parameters,
        }
    }
}

/// Provider completion request consumed by async provider implementations.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProviderRequest {
    pub model: Option<String>,
    pub messages: Vec<ProviderMessage>,
    pub tools: Vec<ToolManifest>,
    pub capabilities: ProviderCapabilities,
    pub flags: ProviderCallFlags,
}

impl ProviderRequest {
    pub fn new(messages: Vec<ProviderMessage>) -> Self {
        Self {
            model: None,
            messages,
            tools: Vec::new(),
            capabilities: ProviderCapabilities::default(),
            flags: ProviderCallFlags::default(),
        }
    }

    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.model = Some(model.into());
        self
    }

    pub fn with_tools(mut self, tools: Vec<ToolManifest>) -> Self {
        self.tools = tools;
        self
    }
}

/// Cooperative cancellation input for provider calls.
#[derive(Clone, Debug, Default)]
pub struct Cancellation {
    cancelled: Arc<AtomicBool>,
}

impl Cancellation {
    pub fn cancelled() -> Self {
        let cancellation = Self::default();
        cancellation.cancel();
        cancellation
    }

    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::SeqCst);
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::SeqCst)
    }
}

impl PartialEq for Cancellation {
    fn eq(&self, other: &Self) -> bool {
        self.is_cancelled() == other.is_cancelled()
    }
}

impl Eq for Cancellation {}

/// Provider stream terminal reason.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FinishReason {
    Stop,
    Length,
    ToolCalls,
    Cancelled,
    Error,
}

/// Token usage metadata returned when a provider exposes it.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq)]
pub struct UsageMetadata {
    #[serde(alias = "prompt_tokens")]
    pub input_tokens: Option<u64>,
    #[serde(alias = "completion_tokens")]
    pub output_tokens: Option<u64>,
    pub total_tokens: Option<u64>,
}

/// Reasoning metadata returned when a provider exposes it.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ReasoningMetadata {
    pub effort: Option<String>,
    pub summary: Option<String>,
}

/// Tool call requested by a provider before a `ToolCalls` finish.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProviderToolCall {
    pub id: String,
    pub name: String,
    pub arguments: String,
}

impl ProviderToolCall {
    pub fn new(
        id: impl Into<String>,
        name: impl Into<String>,
        arguments: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            arguments: arguments.into(),
        }
    }
}

/// Terminal provider event.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProviderFinished {
    pub finish_reason: FinishReason,
    pub tool_calls: Vec<ProviderToolCall>,
    pub usage: Option<UsageMetadata>,
    pub reasoning: Option<ReasoningMetadata>,
}

impl ProviderFinished {
    pub fn stopped() -> Self {
        Self {
            finish_reason: FinishReason::Stop,
            tool_calls: Vec::new(),
            usage: None,
            reasoning: None,
        }
    }

    pub fn tool_calls(tool_calls: Vec<ProviderToolCall>) -> Self {
        Self {
            finish_reason: FinishReason::ToolCalls,
            tool_calls,
            usage: None,
            reasoning: None,
        }
    }
}

/// Stream event emitted by async providers.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProviderStreamEvent {
    MessageDelta(MessageDelta),
    ReasoningDelta(ReasoningDelta),
    Finished(ProviderFinished),
}

pub struct ProviderStream {
    receiver: mpsc::Receiver<Result<ProviderStreamEvent, ProviderError>>,
}

impl ProviderStream {
    pub fn new(receiver: mpsc::Receiver<Result<ProviderStreamEvent, ProviderError>>) -> Self {
        Self { receiver }
    }

    pub fn from_events(
        events: impl IntoIterator<Item = Result<ProviderStreamEvent, ProviderError>>,
    ) -> Self {
        let (sender, receiver) = mpsc::channel(128);
        for event in events {
            if sender.try_send(event).is_err() {
                break;
            }
        }
        drop(sender);
        Self { receiver }
    }

    pub async fn next(&mut self) -> Option<Result<ProviderStreamEvent, ProviderError>> {
        self.receiver.recv().await
    }
}

pub type ProviderCall<'a> =
    Pin<Box<dyn Future<Output = Result<ProviderStream, ProviderError>> + Send + 'a>>;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ValidationMode {
    ApiKey,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProviderError {
    CancellationError,
    InvalidApiKey,
    ModelFetchFailed {
        provider_name: String,
    },
    NoModelsReturned {
        provider_name: String,
    },
    ProviderUnavailable {
        provider_name: String,
    },
    StreamUnavailable {
        provider_name: String,
    },
    MalformedResponse {
        provider_name: String,
        reason: String,
    },
    ResponseParsingFailed {
        provider_name: String,
        reason: String,
    },
    NetworkError {
        provider_name: String,
        reason: String,
    },
    ContextLimitExceeded {
        provider_name: String,
        reason: String,
    },
    CapabilityMismatch {
        provider_name: String,
        capability: String,
    },
    UnsupportedProvider {
        provider_id: String,
    },
    UnsupportedValidationMode,
}

impl Display for ProviderError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ProviderError::CancellationError => formatter.write_str("provider call was cancelled"),
            ProviderError::InvalidApiKey => formatter.write_str("invalid API key"),
            ProviderError::ModelFetchFailed { provider_name } => {
                write!(formatter, "failed to fetch models from {provider_name}")
            }
            ProviderError::NoModelsReturned { provider_name } => {
                write!(formatter, "{provider_name} returned no models")
            }
            ProviderError::ProviderUnavailable { provider_name } => {
                write!(formatter, "{provider_name} is unavailable")
            }
            ProviderError::StreamUnavailable { provider_name } => {
                write!(
                    formatter,
                    "{provider_name} streaming is not implemented yet"
                )
            }
            ProviderError::MalformedResponse {
                provider_name,
                reason,
            } => write!(
                formatter,
                "{provider_name} returned a malformed response: {reason}"
            ),
            ProviderError::ResponseParsingFailed {
                provider_name,
                reason,
            } => write!(
                formatter,
                "failed to parse {provider_name} response: {reason}"
            ),
            ProviderError::NetworkError {
                provider_name,
                reason,
            } => write!(
                formatter,
                "{provider_name} network request failed: {reason}"
            ),
            ProviderError::ContextLimitExceeded {
                provider_name,
                reason,
            } => write!(
                formatter,
                "{provider_name} context limit exceeded: {reason}"
            ),
            ProviderError::CapabilityMismatch {
                provider_name,
                capability,
            } => write!(
                formatter,
                "{provider_name} does not support required capability `{capability}`"
            ),
            ProviderError::UnsupportedProvider { provider_id } => {
                write!(formatter, "provider `{provider_id}` is not supported")
            }
            ProviderError::UnsupportedValidationMode => {
                formatter.write_str("validation mode is not supported")
            }
        }
    }
}

impl Error for ProviderError {}

/// Validates a provider value using the enabled provider implementation.
pub fn validate_provider_value(
    provider_id: &str,
    mode: ValidationMode,
    value: &str,
) -> Result<(), ProviderError> {
    match provider_id {
        OPENROUTER_PROVIDER_ID => OpenRouterProvider::default().validate(mode, value),
        _ => Err(ProviderError::UnsupportedProvider {
            provider_id: provider_id.to_owned(),
        }),
    }
}

/// Fetches models for an enabled provider.
pub fn fetch_provider_models(
    provider_id: &str,
    api_key: &str,
) -> Result<Vec<Model>, ProviderError> {
    match provider_id {
        OPENROUTER_PROVIDER_ID => OpenRouterProvider::default().models(api_key),
        _ => Err(ProviderError::UnsupportedProvider {
            provider_id: provider_id.to_owned(),
        }),
    }
}

/// Provider metadata and Agent-facing capabilities that can be inspected without provider I/O.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProviderCapabilityReport {
    pub metadata: ProviderMetadata,
    pub capabilities: ProviderCapabilities,
}

/// Returns the Agent-facing capability report for a supported provider without network access.
pub fn provider_capability_report(
    provider_id: &str,
) -> Result<ProviderCapabilityReport, ProviderError> {
    match provider_id {
        OPENROUTER_PROVIDER_ID => {
            let provider = OpenRouterProvider::default();
            Ok(ProviderCapabilityReport {
                metadata: provider.metadata(),
                capabilities: provider.capabilities(),
            })
        }
        _ => Err(ProviderError::UnsupportedProvider {
            provider_id: provider_id.to_owned(),
        }),
    }
}

/// Returns the first enabled provider capability report without network access.
pub fn enabled_provider_capability_report() -> Result<ProviderCapabilityReport, ProviderError> {
    let Some(provider) = PROVIDERS.iter().find(|provider| provider.enabled) else {
        return Err(ProviderError::ProviderUnavailable {
            provider_name: "enabled provider".to_owned(),
        });
    };

    provider_capability_report(provider.id)
}

#[derive(Default)]
pub struct OpenRouterProvider {
    client: OpenRouterHttpClient,
    api_key: Option<String>,
}

impl OpenRouterProvider {
    pub fn with_api_key(api_key: impl Into<String>) -> Self {
        Self {
            client: OpenRouterHttpClient,
            api_key: Some(api_key.into()),
        }
    }
}

impl LlmProvider for OpenRouterProvider {
    fn metadata(&self) -> ProviderMetadata {
        provider_by_id(OPENROUTER_PROVIDER_ID).expect("OpenRouter metadata should be registered")
    }

    fn validate(&self, mode: ValidationMode, value: &str) -> Result<(), ProviderError> {
        if mode != ValidationMode::ApiKey {
            return Err(ProviderError::UnsupportedValidationMode);
        }

        validate_openrouter_api_key(value, |api_key| self.client.current_key_status(api_key))
    }

    fn models(&self, api_key: &str) -> Result<Vec<Model>, ProviderError> {
        fetch_openrouter_models(api_key, |api_key| self.client.models_response(api_key))
    }

    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities {
            streaming: true,
            tool_calls: true,
            structured_output: false,
            reasoning: false,
            cancellation: false,
            usage_metadata: false,
            reasoning_metadata: false,
            context_limits: ProviderContextLimits::default(),
        }
    }

    fn stream_completion<'a>(
        &'a self,
        request: ProviderRequest,
        cancellation: Cancellation,
    ) -> ProviderCall<'a> {
        let api_key = self.api_key.clone();
        Box::pin(async {
            let Some(api_key) = api_key else {
                return Err(ProviderError::InvalidApiKey);
            };

            openrouter_stream_completion(api_key, request, cancellation)
        })
    }
}

#[derive(Default)]
struct OpenRouterHttpClient;

impl OpenRouterHttpClient {
    fn current_key_status(&self, api_key: &str) -> Result<u16, ProviderError> {
        let response = reqwest::blocking::Client::new()
            .get(OPENROUTER_API_KEY_URL)
            .bearer_auth(api_key)
            .send()
            .map_err(|error| ProviderError::NetworkError {
                provider_name: "OpenRouter".to_owned(),
                reason: error.to_string(),
            })?;

        Ok(response.status().as_u16())
    }

    fn models_response(&self, api_key: &str) -> Result<(u16, String), ProviderError> {
        let response = reqwest::blocking::Client::new()
            .get(OPENROUTER_MODELS_URL)
            .bearer_auth(api_key)
            .send()
            .map_err(|error| ProviderError::NetworkError {
                provider_name: "OpenRouter".to_owned(),
                reason: error.to_string(),
            })?;
        let status = response.status().as_u16();
        let body = response
            .text()
            .map_err(|error| ProviderError::NetworkError {
                provider_name: "OpenRouter".to_owned(),
                reason: error.to_string(),
            })?;

        Ok((status, body))
    }
}

fn openrouter_stream_completion(
    api_key: String,
    request: ProviderRequest,
    cancellation: Cancellation,
) -> Result<ProviderStream, ProviderError> {
    if cancellation.is_cancelled() {
        return Err(ProviderError::CancellationError);
    }

    let (sender, receiver) = mpsc::channel(128);
    std::thread::spawn(move || {
        let result = stream_openrouter_response(&api_key, request, cancellation, sender.clone());
        if let Err(error) = result {
            let _ = sender.blocking_send(Err(error));
        }
    });

    Ok(ProviderStream::new(receiver))
}

fn stream_openrouter_response(
    api_key: &str,
    request: ProviderRequest,
    cancellation: Cancellation,
    sender: mpsc::Sender<Result<ProviderStreamEvent, ProviderError>>,
) -> Result<(), ProviderError> {
    let body = OpenRouterChatRequest::from_provider_request(request)?;
    let response = reqwest::blocking::Client::new()
        .post(OPENROUTER_CHAT_COMPLETIONS_URL)
        .bearer_auth(api_key)
        .json(&body)
        .send()
        .map_err(|error| ProviderError::NetworkError {
            provider_name: "OpenRouter".to_owned(),
            reason: error.to_string(),
        })?;

    let status = response.status().as_u16();
    if status == 401 || status == 403 {
        return Err(ProviderError::InvalidApiKey);
    }
    if !(200..300).contains(&status) {
        return Err(ProviderError::ProviderUnavailable {
            provider_name: "OpenRouter".to_owned(),
        });
    }

    let reader = BufReader::new(response);
    let mut saw_finished = false;
    let mut tool_call_accumulator = OpenRouterToolCallAccumulator::default();
    for line in reader.lines() {
        if cancellation.is_cancelled() {
            return Err(ProviderError::CancellationError);
        }

        let line = line.map_err(|error| ProviderError::NetworkError {
            provider_name: "OpenRouter".to_owned(),
            reason: error.to_string(),
        })?;
        let Some(payload) = line.strip_prefix("data: ") else {
            continue;
        };
        if payload.trim() == "[DONE]" {
            if tool_call_accumulator.has_pending() {
                return Err(ProviderError::MalformedResponse {
                    provider_name: "OpenRouter".to_owned(),
                    reason: "stream ended before tool-call finish".to_owned(),
                });
            }
            if !saw_finished
                && sender
                    .blocking_send(Ok(ProviderStreamEvent::Finished(
                        ProviderFinished::stopped(),
                    )))
                    .is_err()
            {
                return Err(ProviderError::CancellationError);
            }
            saw_finished = true;
            break;
        }

        let finished_in_payload =
            send_openrouter_payload_events(payload, &mut tool_call_accumulator, &sender)?;
        saw_finished |= finished_in_payload;
        if finished_in_payload {
            break;
        }
    }

    if !saw_finished && tool_call_accumulator.has_pending() {
        return Err(ProviderError::MalformedResponse {
            provider_name: "OpenRouter".to_owned(),
            reason: "stream ended before tool-call finish".to_owned(),
        });
    }

    if !saw_finished
        && sender
            .blocking_send(Ok(ProviderStreamEvent::Finished(
                ProviderFinished::stopped(),
            )))
            .is_err()
    {
        return Err(ProviderError::CancellationError);
    }

    Ok(())
}

fn send_openrouter_payload_events(
    payload: &str,
    accumulator: &mut OpenRouterToolCallAccumulator,
    sender: &mpsc::Sender<Result<ProviderStreamEvent, ProviderError>>,
) -> Result<bool, ProviderError> {
    let mut finished_in_payload = false;
    for event in parse_openrouter_chat_chunk_with_accumulator(payload, accumulator)? {
        finished_in_payload |= matches!(event, ProviderStreamEvent::Finished(_));
        if sender.blocking_send(Ok(event)).is_err() {
            return Err(ProviderError::CancellationError);
        }
    }

    Ok(finished_in_payload)
}

#[derive(Serialize)]
struct OpenRouterChatRequest {
    model: String,
    messages: Vec<OpenRouterChatMessage>,
    stream: bool,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    tools: Vec<OpenRouterToolManifest>,
    #[serde(skip_serializing_if = "Option::is_none")]
    parallel_tool_calls: Option<bool>,
}

impl OpenRouterChatRequest {
    fn from_provider_request(request: ProviderRequest) -> Result<Self, ProviderError> {
        let ProviderRequest {
            model,
            messages,
            tools,
            flags,
            ..
        } = request;
        let model = model
            .filter(|model| !model.trim().is_empty())
            .ok_or_else(|| ProviderError::MalformedResponse {
                provider_name: "OpenRouter".to_owned(),
                reason: "missing model for chat completion".to_owned(),
            })?;
        let tools = tools
            .into_iter()
            .map(OpenRouterToolManifest::from_tool_manifest)
            .collect::<Vec<_>>();
        let parallel_tool_calls = if tools.is_empty() { None } else { Some(false) };

        Ok(Self {
            model,
            messages: messages
                .into_iter()
                .map(OpenRouterChatMessage::from_provider_message)
                .collect(),
            stream: flags.stream,
            tools,
            parallel_tool_calls,
        })
    }
}

#[derive(Serialize)]
struct OpenRouterChatMessage {
    role: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    content: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    tool_calls: Vec<OpenRouterAssistantToolCall>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_call_id: Option<String>,
}

impl OpenRouterChatMessage {
    fn from_provider_message(message: ProviderMessage) -> Self {
        let role = match message.role {
            ProviderMessageRole::System => "system",
            ProviderMessageRole::User => "user",
            ProviderMessageRole::Assistant => "assistant",
            ProviderMessageRole::Tool => "tool",
        };
        let tool_calls = message
            .tool_calls
            .into_iter()
            .map(OpenRouterAssistantToolCall::from_provider_tool_call)
            .collect::<Vec<_>>();
        let content = if role == "assistant" && !tool_calls.is_empty() && message.content.is_empty()
        {
            None
        } else {
            Some(message.content)
        };

        Self {
            role,
            content,
            tool_calls,
            tool_call_id: message.tool_call_id,
        }
    }
}

#[derive(Serialize)]
struct OpenRouterToolManifest {
    #[serde(rename = "type")]
    kind: &'static str,
    function: OpenRouterFunctionManifest,
}

impl OpenRouterToolManifest {
    fn from_tool_manifest(manifest: ToolManifest) -> Self {
        Self {
            kind: "function",
            function: OpenRouterFunctionManifest {
                name: manifest.name,
                description: manifest.description,
                parameters: manifest.parameters,
                strict: true,
            },
        }
    }
}

#[derive(Serialize)]
struct OpenRouterFunctionManifest {
    name: String,
    description: String,
    parameters: serde_json::Value,
    strict: bool,
}

#[derive(Serialize)]
struct OpenRouterAssistantToolCall {
    id: String,
    #[serde(rename = "type")]
    kind: &'static str,
    function: OpenRouterAssistantToolCallFunction,
}

impl OpenRouterAssistantToolCall {
    fn from_provider_tool_call(tool_call: ProviderToolCall) -> Self {
        Self {
            id: tool_call.id,
            kind: "function",
            function: OpenRouterAssistantToolCallFunction {
                name: tool_call.name,
                arguments: tool_call.arguments,
            },
        }
    }
}

#[derive(Serialize)]
struct OpenRouterAssistantToolCallFunction {
    name: String,
    arguments: String,
}

#[cfg(test)]
fn parse_openrouter_chat_chunk(payload: &str) -> Result<Vec<ProviderStreamEvent>, ProviderError> {
    let mut accumulator = OpenRouterToolCallAccumulator::default();
    parse_openrouter_chat_chunk_with_accumulator(payload, &mut accumulator)
}

fn parse_openrouter_chat_chunk_with_accumulator(
    payload: &str,
    accumulator: &mut OpenRouterToolCallAccumulator,
) -> Result<Vec<ProviderStreamEvent>, ProviderError> {
    let chunk: OpenRouterChatChunk =
        serde_json::from_str(payload).map_err(|error| ProviderError::ResponseParsingFailed {
            provider_name: "OpenRouter".to_owned(),
            reason: error.to_string(),
        })?;
    let mut events = Vec::new();

    for choice in chunk.choices {
        let mut finish_reason = choice.finish_reason;
        let native_finish_reason = choice.native_finish_reason;
        let mut complete_tool_calls = Vec::new();
        if let Some(delta) = choice.delta {
            if let Some(tool_calls) = delta.tool_calls {
                accumulator.add_chunks(tool_calls)?;
            }
            if let Some(content) = delta.content {
                if !content.is_empty() {
                    events.push(ProviderStreamEvent::MessageDelta(MessageDelta::assistant(
                        content,
                    )));
                }
            }
            if finish_reason.is_none() {
                finish_reason = delta.finish_reason;
            }
        }

        if let Some(message) = choice.message {
            if let Some(content) = message.content {
                if !content.is_empty() {
                    events.push(ProviderStreamEvent::MessageDelta(MessageDelta::assistant(
                        content,
                    )));
                }
            }
            if let Some(tool_calls) = message.tool_calls {
                complete_tool_calls = tool_calls
                    .into_iter()
                    .enumerate()
                    .map(|(index, tool_call)| tool_call.into_provider_tool_call(index))
                    .collect::<Result<Vec<_>, _>>()?;
            }
        }

        if let Some(finish_reason) = finish_reason {
            let finish_reason = parse_openrouter_finish_reason(&finish_reason);
            let tool_calls = if finish_reason == FinishReason::ToolCalls {
                let mut accumulated = if accumulator.has_pending() {
                    accumulator.finish_tool_calls()?
                } else {
                    Vec::new()
                };
                accumulated.extend(complete_tool_calls);
                if accumulated.is_empty() {
                    return Err(ProviderError::MalformedResponse {
                        provider_name: "OpenRouter".to_owned(),
                        reason: openrouter_empty_tool_call_finish_reason(
                            native_finish_reason.as_deref(),
                            payload,
                        ),
                    });
                }
                accumulated
            } else {
                if accumulator.has_pending() || !complete_tool_calls.is_empty() {
                    return Err(ProviderError::MalformedResponse {
                        provider_name: "OpenRouter".to_owned(),
                        reason: format!(
                            "tool-call chunks ended without tool-call finish; OpenRouter response chunk JSON: {payload}"
                        ),
                    });
                }
                Vec::new()
            };

            events.push(ProviderStreamEvent::Finished(ProviderFinished {
                finish_reason,
                tool_calls,
                usage: chunk.usage,
                reasoning: None,
            }));
        }
    }

    Ok(events)
}

fn parse_openrouter_finish_reason(reason: &str) -> FinishReason {
    match reason {
        "length" => FinishReason::Length,
        "tool_calls" => FinishReason::ToolCalls,
        "cancelled" => FinishReason::Cancelled,
        "error" => FinishReason::Error,
        _ => FinishReason::Stop,
    }
}

fn openrouter_empty_tool_call_finish_reason(
    native_finish_reason: Option<&str>,
    payload: &str,
) -> String {
    let native_finish_reason = native_finish_reason
        .filter(|reason| !reason.trim().is_empty())
        .unwrap_or("unknown");

    format!(
        "OpenRouter reported finish_reason=tool_calls without any tool call data \
         (no delta.tool_calls and no message.tool_calls). \
         native_finish_reason={native_finish_reason}. \
         This usually means the selected model/provider route stopped without emitting a native function call, \
         even though tools were present. Try a different tool-capable model/provider route or disable tools for this model. \
         OpenRouter response chunk JSON: {payload}"
    )
}

#[derive(Deserialize)]
struct OpenRouterChatChunk {
    choices: Vec<OpenRouterChatChoice>,
    usage: Option<UsageMetadata>,
}

#[derive(Deserialize)]
struct OpenRouterChatChoice {
    delta: Option<OpenRouterChatDelta>,
    message: Option<OpenRouterChatChoiceMessage>,
    finish_reason: Option<String>,
    native_finish_reason: Option<String>,
}

#[derive(Deserialize)]
struct OpenRouterChatDelta {
    content: Option<String>,
    tool_calls: Option<Vec<OpenRouterChatDeltaToolCall>>,
    finish_reason: Option<String>,
}

#[derive(Deserialize)]
struct OpenRouterChatChoiceMessage {
    content: Option<String>,
    tool_calls: Option<Vec<OpenRouterChatMessageToolCall>>,
}

#[derive(Deserialize)]
struct OpenRouterChatMessageToolCall {
    id: String,
    #[serde(rename = "type")]
    kind: String,
    function: OpenRouterChatMessageToolCallFunction,
}

impl OpenRouterChatMessageToolCall {
    fn into_provider_tool_call(self, index: usize) -> Result<ProviderToolCall, ProviderError> {
        if self.kind != "function" {
            return Err(ProviderError::MalformedResponse {
                provider_name: "OpenRouter".to_owned(),
                reason: format!("unsupported tool-call type `{}`", self.kind),
            });
        }

        if self.id.trim().is_empty() {
            return Err(ProviderError::MalformedResponse {
                provider_name: "OpenRouter".to_owned(),
                reason: format!("tool-call index {index} omitted id"),
            });
        }

        if self.function.name.trim().is_empty() {
            return Err(ProviderError::MalformedResponse {
                provider_name: "OpenRouter".to_owned(),
                reason: format!("tool-call index {index} omitted function name"),
            });
        }

        Ok(ProviderToolCall::new(
            self.id,
            self.function.name,
            self.function.arguments,
        ))
    }
}

#[derive(Deserialize)]
struct OpenRouterChatMessageToolCallFunction {
    name: String,
    arguments: String,
}

#[derive(Deserialize)]
struct OpenRouterChatDeltaToolCall {
    index: usize,
    id: Option<String>,
    #[serde(rename = "type")]
    kind: Option<String>,
    function: Option<OpenRouterChatDeltaToolCallFunction>,
}

#[derive(Deserialize)]
struct OpenRouterChatDeltaToolCallFunction {
    name: Option<String>,
    arguments: Option<String>,
}

#[derive(Default)]
struct OpenRouterToolCallAccumulator {
    tool_calls: BTreeMap<usize, OpenRouterAccumulatedToolCall>,
}

impl OpenRouterToolCallAccumulator {
    fn add_chunks(
        &mut self,
        tool_calls: Vec<OpenRouterChatDeltaToolCall>,
    ) -> Result<(), ProviderError> {
        for tool_call in tool_calls {
            if let Some(kind) = tool_call.kind.as_deref() {
                if kind != "function" {
                    return Err(ProviderError::MalformedResponse {
                        provider_name: "OpenRouter".to_owned(),
                        reason: format!("unsupported tool-call type `{kind}`"),
                    });
                }
            }

            let accumulated = self.tool_calls.entry(tool_call.index).or_default();
            if let Some(id) = tool_call.id {
                if let Some(existing_id) = accumulated.id.as_deref() {
                    if existing_id != id {
                        return Err(ProviderError::MalformedResponse {
                            provider_name: "OpenRouter".to_owned(),
                            reason: format!(
                                "tool-call index {} changed id from `{existing_id}` to `{id}`",
                                tool_call.index
                            ),
                        });
                    }
                } else {
                    accumulated.id = Some(id);
                }
            }

            if let Some(function) = tool_call.function {
                if let Some(name) = function.name {
                    accumulated.name.push_str(&name);
                }
                if let Some(arguments) = function.arguments {
                    accumulated.arguments.push_str(&arguments);
                    accumulated.saw_arguments = true;
                }
            }
        }

        Ok(())
    }

    fn has_pending(&self) -> bool {
        !self.tool_calls.is_empty()
    }

    fn finish_tool_calls(&mut self) -> Result<Vec<ProviderToolCall>, ProviderError> {
        if self.tool_calls.is_empty() {
            return Err(ProviderError::MalformedResponse {
                provider_name: "OpenRouter".to_owned(),
                reason: "tool-call finish did not include tool-call chunks".to_owned(),
            });
        }

        let tool_calls = std::mem::take(&mut self.tool_calls);
        tool_calls
            .into_iter()
            .map(|(index, accumulated)| accumulated.into_provider_tool_call(index))
            .collect()
    }
}

#[derive(Default)]
struct OpenRouterAccumulatedToolCall {
    id: Option<String>,
    name: String,
    arguments: String,
    saw_arguments: bool,
}

impl OpenRouterAccumulatedToolCall {
    fn into_provider_tool_call(self, index: usize) -> Result<ProviderToolCall, ProviderError> {
        let Some(id) = self.id.filter(|id| !id.trim().is_empty()) else {
            return Err(ProviderError::MalformedResponse {
                provider_name: "OpenRouter".to_owned(),
                reason: format!("tool-call index {index} omitted id"),
            });
        };

        if self.name.trim().is_empty() {
            return Err(ProviderError::MalformedResponse {
                provider_name: "OpenRouter".to_owned(),
                reason: format!("tool-call index {index} omitted function name"),
            });
        }

        if !self.saw_arguments {
            return Err(ProviderError::MalformedResponse {
                provider_name: "OpenRouter".to_owned(),
                reason: format!("tool-call index {index} omitted function arguments"),
            });
        }

        Ok(ProviderToolCall::new(id, self.name, self.arguments))
    }
}

fn validate_openrouter_api_key(
    api_key: &str,
    request_current_key: impl FnOnce(&str) -> Result<u16, ProviderError>,
) -> Result<(), ProviderError> {
    let api_key = api_key.trim();
    if api_key.is_empty() {
        return Err(ProviderError::InvalidApiKey);
    }

    match request_current_key(api_key)? {
        200 => Ok(()),
        401 | 403 => Err(ProviderError::InvalidApiKey),
        _ => Err(ProviderError::ProviderUnavailable {
            provider_name: "OpenRouter".to_owned(),
        }),
    }
}

fn fetch_openrouter_models(
    api_key: &str,
    request_models: impl FnOnce(&str) -> Result<(u16, String), ProviderError>,
) -> Result<Vec<Model>, ProviderError> {
    let api_key = api_key.trim();
    if api_key.is_empty() {
        return Err(ProviderError::InvalidApiKey);
    }

    let (status, body) = request_models(api_key)?;
    match status {
        200 => parse_openrouter_models(&body),
        401 | 403 => Err(ProviderError::InvalidApiKey),
        _ => Err(ProviderError::ModelFetchFailed {
            provider_name: "OpenRouter".to_owned(),
        }),
    }
}

fn parse_openrouter_models(body: &str) -> Result<Vec<Model>, ProviderError> {
    let value: serde_json::Value =
        serde_json::from_str(body).map_err(|error| ProviderError::ResponseParsingFailed {
            provider_name: "OpenRouter".to_owned(),
            reason: error.to_string(),
        })?;
    let response: OpenRouterModelsResponse =
        serde_json::from_value(value).map_err(|error| ProviderError::MalformedResponse {
            provider_name: "OpenRouter".to_owned(),
            reason: error.to_string(),
        })?;
    let models = response
        .data
        .into_iter()
        .filter_map(|model| {
            let id = model.id.trim();
            if id.is_empty() {
                return None;
            }

            let display_name = model
                .name
                .as_deref()
                .map(str::trim)
                .filter(|name| !name.is_empty())
                .unwrap_or(id);

            Some(Model::new(id, display_name))
        })
        .collect::<Vec<_>>();

    if models.is_empty() {
        return Err(ProviderError::NoModelsReturned {
            provider_name: "OpenRouter".to_owned(),
        });
    }

    Ok(models)
}

#[derive(Deserialize)]
struct OpenRouterModelsResponse {
    data: Vec<OpenRouterModelResponse>,
}

#[derive(Deserialize)]
struct OpenRouterModelResponse {
    id: String,
    name: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn enabled_provider_is_openrouter() {
        assert_eq!(enabled_provider_name(), "OpenRouter");
    }

    #[test]
    fn registry_contains_enabled_openrouter_and_disabled_placeholders() {
        let providers = provider_registry();

        assert_eq!(providers[0].id(), OPENROUTER_PROVIDER_ID);
        assert!(providers[0].is_enabled());
        assert_eq!(providers[1].id(), OPENAI_PROVIDER_ID);
        assert!(!providers[1].is_enabled());
        assert_eq!(providers[2].id(), GOOGLE_GEMINI_PROVIDER_ID);
        assert!(!providers[2].is_enabled());
    }

    #[test]
    fn provider_lookup_uses_stable_ids() {
        let provider = provider_by_id(OPENROUTER_PROVIDER_ID).unwrap();

        assert_eq!(provider.display_name(), "OpenRouter");
    }

    #[test]
    fn openrouter_validation_rejects_blank_key_without_http_call() {
        let error =
            validate_openrouter_api_key("   ", |_| panic!("HTTP must not be called")).unwrap_err();

        assert!(matches!(error, ProviderError::InvalidApiKey));
    }

    #[test]
    fn openrouter_validation_accepts_success_status() {
        let result = validate_openrouter_api_key("sk-or-v1-valid", |_| Ok(200));

        assert!(result.is_ok());
    }

    #[test]
    fn openrouter_validation_rejects_unauthorized_status() {
        let error = validate_openrouter_api_key("sk-or-v1-invalid", |_| Ok(401)).unwrap_err();

        assert!(matches!(error, ProviderError::InvalidApiKey));
    }

    #[test]
    fn openrouter_validation_maps_other_statuses_to_unavailable() {
        let error = validate_openrouter_api_key("sk-or-v1-valid", |_| Ok(500)).unwrap_err();

        assert!(matches!(error, ProviderError::ProviderUnavailable { .. }));
    }

    #[test]
    fn provider_value_validation_rejects_unsupported_provider() {
        let error =
            validate_provider_value("openai", ValidationMode::ApiKey, "sk-test").unwrap_err();

        assert!(matches!(error, ProviderError::UnsupportedProvider { .. }));
    }

    #[test]
    fn openrouter_model_fetch_rejects_blank_key_without_http_call() {
        let error =
            fetch_openrouter_models("   ", |_| panic!("HTTP must not be called")).unwrap_err();

        assert!(matches!(error, ProviderError::InvalidApiKey));
    }

    #[test]
    fn openrouter_model_fetch_parses_model_ids_and_names() {
        let models = fetch_openrouter_models("sk-or-v1-valid", |_| {
            Ok((
                200,
                r#"{
                    "data": [
                        {"id": "openai/gpt-4o", "name": "GPT-4o"},
                        {"id": "anthropic/claude-sonnet-4.5", "name": null}
                    ]
                }"#
                .to_owned(),
            ))
        })
        .unwrap();

        assert_eq!(models[0], Model::new("openai/gpt-4o", "GPT-4o"));
        assert_eq!(
            models[1],
            Model::new("anthropic/claude-sonnet-4.5", "anthropic/claude-sonnet-4.5")
        );
    }

    #[test]
    fn openrouter_model_fetch_rejects_unauthorized_status() {
        let error = fetch_openrouter_models("sk-or-v1-invalid", |_| Ok((401, "{}".to_owned())))
            .unwrap_err();

        assert!(matches!(error, ProviderError::InvalidApiKey));
    }

    #[test]
    fn openrouter_model_fetch_rejects_empty_model_list() {
        let error =
            fetch_openrouter_models("sk-or-v1-valid", |_| Ok((200, r#"{"data":[]}"#.to_owned())))
                .unwrap_err();

        assert!(matches!(error, ProviderError::NoModelsReturned { .. }));
    }

    #[test]
    fn openrouter_model_fetch_maps_invalid_json_to_parse_error() {
        let error = fetch_openrouter_models("sk-or-v1-valid", |_| Ok((200, "not json".to_owned())))
            .unwrap_err();

        assert!(matches!(error, ProviderError::ResponseParsingFailed { .. }));
    }

    #[test]
    fn openrouter_model_fetch_maps_bad_shape_to_malformed_response() {
        let error = fetch_openrouter_models("sk-or-v1-valid", |_| {
            Ok((200, r#"{"data":[{"id":5,"name":"bad"}]}"#.to_owned()))
        })
        .unwrap_err();

        assert!(matches!(error, ProviderError::MalformedResponse { .. }));
    }

    #[test]
    fn provider_model_fetch_rejects_unsupported_provider() {
        let error = fetch_provider_models("openai", "sk-test").unwrap_err();

        assert!(matches!(error, ProviderError::UnsupportedProvider { .. }));
    }

    #[test]
    fn openrouter_exposes_async_stream_seam() {
        let provider = OpenRouterProvider::default();
        let request =
            ProviderRequest::new(vec![ProviderMessage::user("hello")]).with_model("test/model");
        let error = match futures::executor::block_on(
            provider.stream_completion(request, Cancellation::default()),
        ) {
            Ok(_) => panic!("stream should require an API key"),
            Err(error) => error,
        };

        assert!(matches!(error, ProviderError::InvalidApiKey));
    }

    #[test]
    fn openrouter_chat_chunk_parses_content_and_finish() {
        let events = parse_openrouter_chat_chunk(
            r#"{"choices":[{"delta":{"content":"hello"},"finish_reason":null},{"delta":{},"finish_reason":"stop"}]}"#,
        )
        .unwrap();

        assert!(matches!(
            &events[0],
            ProviderStreamEvent::MessageDelta(MessageDelta { content, .. }) if content == "hello"
        ));
        assert!(matches!(
            events[1],
            ProviderStreamEvent::Finished(ProviderFinished {
                finish_reason: FinishReason::Stop,
                ..
            })
        ));
    }

    #[test]
    fn openrouter_chat_request_maps_model_and_messages() {
        let request = OpenRouterChatRequest::from_provider_request(
            ProviderRequest::new(vec![
                ProviderMessage::system("system"),
                ProviderMessage::user("hello"),
                ProviderMessage::assistant("hi"),
                ProviderMessage::tool("tool output"),
            ])
            .with_model("openrouter/model"),
        )
        .unwrap();

        assert_eq!(request.model, "openrouter/model");
        assert!(request.stream);
        assert_eq!(request.messages[0].role, "system");
        assert_eq!(request.messages[1].role, "user");
        assert_eq!(request.messages[2].role, "assistant");
        assert_eq!(request.messages[3].role, "tool");
    }

    #[test]
    fn openrouter_chat_request_serializes_strict_tool_manifest() {
        let request = OpenRouterChatRequest::from_provider_request(
            ProviderRequest::new(vec![ProviderMessage::user("hello")])
                .with_model("openrouter/model")
                .with_tools(vec![ToolManifest::new(
                    "read_release",
                    "Read a release artifact.",
                    json!({
                        "type": "object",
                        "properties": {
                            "path": {"type": "string"}
                        },
                        "required": ["path"]
                    }),
                )]),
        )
        .unwrap();
        let value = serde_json::to_value(request).unwrap();

        assert_eq!(value["tools"][0]["type"], "function");
        assert_eq!(value["tools"][0]["function"]["name"], "read_release");
        assert_eq!(value["tools"][0]["function"]["strict"], true);
        assert_eq!(value["parallel_tool_calls"], false);
    }

    #[test]
    fn openrouter_chat_request_serializes_tool_call_and_tool_result_replay() {
        let request = OpenRouterChatRequest::from_provider_request(
            ProviderRequest::new(vec![
                ProviderMessage::assistant_tool_call(ProviderToolCall::new(
                    "call-1",
                    "read_release",
                    r#"{"path":"release/resume.yaml"}"#,
                )),
                ProviderMessage::tool_result("call-1", r#"{"ok":true}"#),
            ])
            .with_model("openrouter/model"),
        )
        .unwrap();
        let value = serde_json::to_value(request).unwrap();

        assert_eq!(value["messages"][0]["role"], "assistant");
        assert!(value["messages"][0].get("content").is_none());
        assert_eq!(value["messages"][0]["tool_calls"][0]["id"], "call-1");
        assert_eq!(value["messages"][0]["tool_calls"][0]["type"], "function");
        assert_eq!(
            value["messages"][0]["tool_calls"][0]["function"]["name"],
            "read_release"
        );
        assert_eq!(
            value["messages"][0]["tool_calls"][0]["function"]["arguments"],
            r#"{"path":"release/resume.yaml"}"#
        );
        assert_eq!(value["messages"][1]["role"], "tool");
        assert_eq!(value["messages"][1]["tool_call_id"], "call-1");
        assert_eq!(value["messages"][1]["content"], r#"{"ok":true}"#);
    }

    #[test]
    fn openrouter_chat_chunks_emit_tool_calls_only_on_tool_call_finish() {
        let mut accumulator = OpenRouterToolCallAccumulator::default();

        let events = parse_openrouter_chat_chunk_with_accumulator(
            r#"{"choices":[{"delta":{"tool_calls":[{"index":0,"id":"call-1","type":"function","function":{"name":"read_release","arguments":"{\"path\""}}]},"finish_reason":null}]}"#,
            &mut accumulator,
        )
        .unwrap();
        assert!(events.is_empty());

        let events = parse_openrouter_chat_chunk_with_accumulator(
            r#"{"choices":[{"delta":{"tool_calls":[{"index":0,"function":{"arguments":":\"release/resume.yaml\"}"}}]},"finish_reason":null}]}"#,
            &mut accumulator,
        )
        .unwrap();
        assert!(events.is_empty());

        let events = parse_openrouter_chat_chunk_with_accumulator(
            r#"{"choices":[{"delta":{},"finish_reason":"tool_calls"}]}"#,
            &mut accumulator,
        )
        .unwrap();

        assert_eq!(
            events,
            vec![ProviderStreamEvent::Finished(ProviderFinished {
                finish_reason: FinishReason::ToolCalls,
                tool_calls: vec![ProviderToolCall::new(
                    "call-1",
                    "read_release",
                    r#"{"path":"release/resume.yaml"}"#
                )],
                usage: None,
                reasoning: None,
            })]
        );
    }

    #[test]
    fn openrouter_stream_stops_after_valid_tool_call_finish_before_duplicate_empty_finish() {
        let mut accumulator = OpenRouterToolCallAccumulator::default();
        let (sender, mut receiver) = mpsc::channel(8);

        let should_stop = send_openrouter_payload_events(
            r#"{"choices":[{"delta":{"tool_calls":[{"index":0,"id":"tool_terminal_TXIsWhOkok7u4ZqvpAeG","type":"function","function":{"name":"terminal","arguments":"{\"command\":\"pwd\"}"}}]},"finish_reason":"tool_calls"}]}"#,
            &mut accumulator,
            &sender,
        )
        .unwrap();

        assert!(should_stop);
        let event = receiver.try_recv().unwrap().unwrap();
        assert_eq!(
            event,
            ProviderStreamEvent::Finished(ProviderFinished {
                finish_reason: FinishReason::ToolCalls,
                tool_calls: vec![ProviderToolCall::new(
                    "tool_terminal_TXIsWhOkok7u4ZqvpAeG",
                    "terminal",
                    r#"{"command":"pwd"}"#
                )],
                usage: None,
                reasoning: None,
            })
        );
        assert!(receiver.try_recv().is_err());

        let duplicate_empty_finish = r#"{"choices":[{"delta":{"content":"","role":"assistant"},"finish_reason":"tool_calls","native_finish_reason":"STOP"}],"usage":{"prompt_tokens":814,"completion_tokens":130,"total_tokens":944}}"#;
        if !should_stop {
            send_openrouter_payload_events(duplicate_empty_finish, &mut accumulator, &sender)
                .unwrap();
        }
        assert!(receiver.try_recv().is_err());
    }

    #[test]
    fn openrouter_chat_chunk_accepts_complete_message_tool_calls_on_tool_call_finish() {
        let events = parse_openrouter_chat_chunk(
            r#"{"choices":[{"message":{"role":"assistant","content":null,"tool_calls":[{"id":"call-1","type":"function","function":{"name":"read_release","arguments":"{\"path\":\"release/resume.yaml\"}"}}]},"finish_reason":"tool_calls"}]}"#,
        )
        .unwrap();

        assert_eq!(
            events,
            vec![ProviderStreamEvent::Finished(ProviderFinished {
                finish_reason: FinishReason::ToolCalls,
                tool_calls: vec![ProviderToolCall::new(
                    "call-1",
                    "read_release",
                    r#"{"path":"release/resume.yaml"}"#
                )],
                usage: None,
                reasoning: None,
            })]
        );
    }

    #[test]
    fn openrouter_chat_chunk_accepts_delta_finish_reason_from_tool_call_guide_shape() {
        let events = parse_openrouter_chat_chunk(
            r#"{"choices":[{"delta":{"tool_calls":[{"index":0,"id":"call-1","type":"function","function":{"name":"read_release","arguments":"{\"path\":\"release/resume.yaml\"}"}}],"finish_reason":"tool_calls"}}]}"#,
        )
        .unwrap();

        assert_eq!(
            events,
            vec![ProviderStreamEvent::Finished(ProviderFinished {
                finish_reason: FinishReason::ToolCalls,
                tool_calls: vec![ProviderToolCall::new(
                    "call-1",
                    "read_release",
                    r#"{"path":"release/resume.yaml"}"#
                )],
                usage: None,
                reasoning: None,
            })]
        );
    }

    #[test]
    fn openrouter_tool_call_finish_without_tool_data_reports_raw_response_chunk() {
        let payload = r#"{"id":"gen-1777903368-VISjaqh4vj28SScWcgcH","object":"chat.completion.chunk","created":1777903368,"model":"google/gemini-3.1-pro-preview-20260219","provider":"Google","choices":[{"index":0,"delta":{"content":"","role":"assistant"},"finish_reason":"tool_calls","native_finish_reason":"STOP"}],"usage":{"prompt_tokens":818,"completion_tokens":260,"total_tokens":1078,"cost":0.004756,"is_byok":false,"prompt_tokens_details":{"cached_tokens":0,"cache_write_tokens":0,"audio_tokens":0,"video_tokens":0},"cost_details":{"upstream_inference_cost":0.004756,"upstream_inference_prompt_cost":0.001636,"upstream_inference_completions_cost":0.00312},"completion_tokens_details":{"reasoning_tokens":228,"image_tokens":0,"audio_tokens":0}}}"#;

        let error = parse_openrouter_chat_chunk(payload).unwrap_err();

        assert!(matches!(error, ProviderError::MalformedResponse { .. }));
        let message = error.to_string();
        assert!(message.contains("finish_reason=tool_calls without any tool call data"));
        assert!(message.contains("native_finish_reason=STOP"));
        assert!(message.contains(
            "selected model/provider route stopped without emitting a native function call"
        ));
        assert!(message.contains(payload));
    }

    #[test]
    fn openrouter_chat_chunk_parses_openrouter_usage_token_names() {
        let events = parse_openrouter_chat_chunk(
            r#"{"choices":[{"delta":{},"finish_reason":"stop"}],"usage":{"prompt_tokens":818,"completion_tokens":260,"total_tokens":1078}}"#,
        )
        .unwrap();

        assert_eq!(
            events,
            vec![ProviderStreamEvent::Finished(ProviderFinished {
                finish_reason: FinishReason::Stop,
                tool_calls: Vec::new(),
                usage: Some(UsageMetadata {
                    input_tokens: Some(818),
                    output_tokens: Some(260),
                    total_tokens: Some(1078),
                }),
                reasoning: None,
            })]
        );
    }

    #[test]
    fn malformed_openrouter_chat_chunk_returns_provider_error() {
        let error = parse_openrouter_chat_chunk("{not json").unwrap_err();

        assert!(matches!(error, ProviderError::ResponseParsingFailed { .. }));
    }

    #[test]
    fn provider_request_defaults_to_no_tool_streaming_call() {
        let request = ProviderRequest::new(vec![ProviderMessage::user("hello")]);

        assert_eq!(request.messages[0].role, ProviderMessageRole::User);
        assert!(request.tools.is_empty());
        assert!(request.flags.stream);
        assert!(!request.flags.allow_tools);
        assert!(!request.flags.include_reasoning);
    }

    #[test]
    fn provider_messages_support_tool_role() {
        let message = ProviderMessage::tool("tool output");

        assert_eq!(message.role, ProviderMessageRole::Tool);
        assert_eq!(message.content, "tool output");
        assert!(message.tool_calls.is_empty());
        assert_eq!(message.tool_call_id, None);
    }

    #[test]
    fn provider_capabilities_default_to_no_optional_features_or_limits() {
        let capabilities = ProviderCapabilities::default();

        assert!(!capabilities.streaming);
        assert!(!capabilities.tool_calls);
        assert!(!capabilities.structured_output);
        assert!(!capabilities.usage_metadata);
        assert!(!capabilities.reasoning_metadata);
        assert_eq!(
            capabilities.context_limits,
            ProviderContextLimits::default()
        );
    }

    #[test]
    fn enabled_provider_capability_report_is_no_network_openrouter_metadata() {
        let report = enabled_provider_capability_report().unwrap();

        assert_eq!(report.metadata.id(), OPENROUTER_PROVIDER_ID);
        assert_eq!(report.metadata.display_name(), "OpenRouter");
        assert_eq!(
            report.capabilities,
            OpenRouterProvider::default().capabilities()
        );
    }

    #[test]
    fn provider_capability_report_rejects_unsupported_provider() {
        let error = provider_capability_report(OPENAI_PROVIDER_ID).unwrap_err();

        assert!(matches!(error, ProviderError::UnsupportedProvider { .. }));
    }
}
