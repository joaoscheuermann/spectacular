use serde::{Deserialize, Serialize};
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
}

impl ProviderMessage {
    pub fn system(content: impl Into<String>) -> Self {
        Self {
            role: ProviderMessageRole::System,
            content: content.into(),
        }
    }

    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: ProviderMessageRole::User,
            content: content.into(),
        }
    }

    pub fn assistant(content: impl Into<String>) -> Self {
        Self {
            role: ProviderMessageRole::Assistant,
            content: content.into(),
        }
    }

    pub fn tool(content: impl Into<String>) -> Self {
        Self {
            role: ProviderMessageRole::Tool,
            content: content.into(),
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

/// Provider completion request consumed by async provider implementations.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProviderRequest {
    pub model: Option<String>,
    pub messages: Vec<ProviderMessage>,
    pub capabilities: ProviderCapabilities,
    pub flags: ProviderCallFlags,
}

impl ProviderRequest {
    pub fn new(messages: Vec<ProviderMessage>) -> Self {
        Self {
            model: None,
            messages,
            capabilities: ProviderCapabilities::default(),
            flags: ProviderCallFlags::default(),
        }
    }

    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.model = Some(model.into());
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
    pub input_tokens: Option<u64>,
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
            tool_calls: false,
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

        for event in parse_openrouter_chat_chunk(payload)? {
            saw_finished |= matches!(event, ProviderStreamEvent::Finished(_));
            if sender.blocking_send(Ok(event)).is_err() {
                return Err(ProviderError::CancellationError);
            }
        }
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

#[derive(Serialize)]
struct OpenRouterChatRequest {
    model: String,
    messages: Vec<OpenRouterChatMessage>,
    stream: bool,
}

impl OpenRouterChatRequest {
    fn from_provider_request(request: ProviderRequest) -> Result<Self, ProviderError> {
        let model = request
            .model
            .filter(|model| !model.trim().is_empty())
            .ok_or_else(|| ProviderError::MalformedResponse {
                provider_name: "OpenRouter".to_owned(),
                reason: "missing model for chat completion".to_owned(),
            })?;

        Ok(Self {
            model,
            messages: request
                .messages
                .into_iter()
                .map(OpenRouterChatMessage::from_provider_message)
                .collect(),
            stream: true,
        })
    }
}

#[derive(Serialize)]
struct OpenRouterChatMessage {
    role: &'static str,
    content: String,
}

impl OpenRouterChatMessage {
    fn from_provider_message(message: ProviderMessage) -> Self {
        let role = match message.role {
            ProviderMessageRole::System => "system",
            ProviderMessageRole::User => "user",
            ProviderMessageRole::Assistant => "assistant",
            ProviderMessageRole::Tool => "tool",
        };

        Self {
            role,
            content: message.content,
        }
    }
}

fn parse_openrouter_chat_chunk(payload: &str) -> Result<Vec<ProviderStreamEvent>, ProviderError> {
    let chunk: OpenRouterChatChunk =
        serde_json::from_str(payload).map_err(|error| ProviderError::ResponseParsingFailed {
            provider_name: "OpenRouter".to_owned(),
            reason: error.to_string(),
        })?;
    let mut events = Vec::new();

    for choice in chunk.choices {
        if let Some(content) = choice.delta.and_then(|delta| delta.content) {
            if !content.is_empty() {
                events.push(ProviderStreamEvent::MessageDelta(MessageDelta::assistant(
                    content,
                )));
            }
        }

        if let Some(finish_reason) = choice.finish_reason {
            events.push(ProviderStreamEvent::Finished(ProviderFinished {
                finish_reason: parse_openrouter_finish_reason(&finish_reason),
                tool_calls: Vec::new(),
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

#[derive(Deserialize)]
struct OpenRouterChatChunk {
    choices: Vec<OpenRouterChatChoice>,
    usage: Option<UsageMetadata>,
}

#[derive(Deserialize)]
struct OpenRouterChatChoice {
    delta: Option<OpenRouterChatDelta>,
    finish_reason: Option<String>,
}

#[derive(Deserialize)]
struct OpenRouterChatDelta {
    content: Option<String>,
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
    fn malformed_openrouter_chat_chunk_returns_provider_error() {
        let error = parse_openrouter_chat_chunk("{not json").unwrap_err();

        assert!(matches!(error, ProviderError::ResponseParsingFailed { .. }));
    }

    #[test]
    fn provider_request_defaults_to_no_tool_streaming_call() {
        let request = ProviderRequest::new(vec![ProviderMessage::user("hello")]);

        assert_eq!(request.messages[0].role, ProviderMessageRole::User);
        assert!(request.flags.stream);
        assert!(!request.flags.allow_tools);
        assert!(!request.flags.include_reasoning);
    }

    #[test]
    fn provider_messages_support_tool_role() {
        let message = ProviderMessage::tool("tool output");

        assert_eq!(message.role, ProviderMessageRole::Tool);
        assert_eq!(message.content, "tool output");
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
