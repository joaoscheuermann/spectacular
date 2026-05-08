use crate::chat::model::{ChatPromptFooterModel, ChatRunRequestModel};
use crate::chat::provider::provider_for_runtime;
use crate::chat::renderer::{has_visible_assistant_text, Renderer};
use crate::chat::session::{agent_events_from_records, records_before_latest_user_prompt};
use crate::chat::title::spawn_title_task;
use crate::chat::{ChatError, ChatModel, RuntimeSelection};

const CODING_AGENT_SYSTEM_PROMPT: &str = include_str!("prompt/coding-agent.md");
use spectacular_agent::{
    Agent, AgentConfig, AgentEvent, Store, ToolRegistrationError, ToolStorage,
};
use spectacular_config::ReasoningLevel;
use spectacular_llms::{LlmProvider, ProviderMessageRole};
use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::MissedTickBehavior;

pub type ChatTurnFuture<'a> = Pin<Box<dyn Future<Output = Result<(), ChatError>> + Send + 'a>>;

/// Service contract for executing one chat turn from controllers and commands.
pub trait ChatTurnRunner {
    /// Runs one chat turn for the supplied model, renderer, tools, and request DTO.
    fn run<'a>(
        &'a self,
        model: &'a mut ChatModel,
        renderer: &'a Renderer,
        tools: &'a ToolStorage,
        request: ChatRunRequestModel,
    ) -> ChatTurnFuture<'a>;
}

/// Runtime request for one chat turn after controller-level context has been resolved.
pub struct ChatRunRequest {
    pub prompt: String,
    pub render_user_prompt: bool,
    pub retry_existing_prompt: bool,
    pub runtime: RuntimeSelection,
    pub prompt_footer: Option<ChatPromptFooterModel>,
}

/// Executes chat turns by wiring session context, provider runtime, tools, and rendering.
pub struct ChatRunner<'a> {
    model: &'a ChatModel,
    renderer: &'a Renderer,
    tools: ToolStorage,
}

/// Default service adapter that constructs a `ChatRunner` for each prompt turn.
#[derive(Clone, Copy, Debug, Default)]
pub struct ChatRunnerService;

impl ChatTurnRunner for ChatRunnerService {
    /// Delegates trait-based prompt execution to the inherent service runner.
    fn run<'a>(
        &'a self,
        model: &'a mut ChatModel,
        renderer: &'a Renderer,
        tools: &'a ToolStorage,
        request: ChatRunRequestModel,
    ) -> ChatTurnFuture<'a> {
        Box::pin(async move { ChatRunnerService::run(self, model, renderer, tools, request).await })
    }
}

impl ChatRunnerService {
    /// Runs one prompt through the concrete chat runner using request DTO data.
    pub async fn run(
        &self,
        model: &mut ChatModel,
        renderer: &Renderer,
        tools: &ToolStorage,
        request: ChatRunRequestModel,
    ) -> Result<(), ChatError> {
        ChatRunner::new(model, renderer, tools.clone())
            .run(ChatRunRequest {
                prompt: request.prompt,
                render_user_prompt: request.render_user_prompt,
                retry_existing_prompt: request.retry_existing_prompt,
                runtime: request.runtime,
                prompt_footer: request.prompt_footer,
            })
            .await
    }
}

impl<'a> ChatRunner<'a> {
    /// Creates a chat runner with explicit model, renderer, and tool dependencies.
    pub fn new(model: &'a ChatModel, renderer: &'a Renderer, tools: ToolStorage) -> Self {
        Self {
            model,
            renderer,
            tools,
        }
    }

    /// Executes the requested prompt and renders streaming agent events.
    pub async fn run(&self, request: ChatRunRequest) -> Result<(), ChatError> {
        if request.render_user_prompt {
            match request.prompt_footer.as_ref() {
                Some(footer) => self
                    .renderer
                    .user_prompt_with_footer(&request.prompt, footer),
                None => self.renderer.user_prompt(&request.prompt),
            }
        }

        let records = self.model.records()?;
        let context_records = if request.retry_existing_prompt {
            records_before_latest_user_prompt(&records)
        } else {
            records.as_slice()
        };
        let store = Store::from(agent_events_from_records(context_records));
        let agent = Arc::new(main_chat_agent(
            provider_for_runtime(
                &request.runtime,
                self.model.debug_logger().clone(),
                self.model.config_io(),
            )?,
            &request.runtime,
            store,
            self.tools.clone(),
        ));
        let mut stream = agent.run_stream(request.prompt.clone());
        let mut title_text = String::new();
        let mut assistant_output = AssistantResponseRenderState::default();
        let mut title_spawned = self.model.session_manager().has_title()?;
        let mut spinner_visible = true;
        let mut spinner_frame = 0usize;
        let mut is_streaming = false;
        let mut spinner = tokio::time::interval(Duration::from_millis(90));
        let mut skip_retry_user = request.retry_existing_prompt;
        spinner.set_missed_tick_behavior(MissedTickBehavior::Delay);
        self.renderer.working();

        loop {
            tokio::select! {
                _ = spinner.tick(), if spinner_visible && !is_streaming => {
                    spinner_frame = spinner_frame.wrapping_add(1);
                    self.renderer.working_frame(spinner_frame);
                }
                _ = tokio::signal::ctrl_c() => {
                    stream.cancel();
                }
                event = stream.next() => {
                    let Some(event) = event else {
                        break;
                    };
                    if should_skip_retry_user_prompt(&mut skip_retry_user, &event) {
                        continue;
                    }
                    if should_append_without_render(&event) {
                        self.model.append_agent_event(&event)?;
                        continue;
                    }

                    if let AgentEvent::MessageDelta(delta) = &event {
                        if delta.role == ProviderMessageRole::Assistant {
                            if let Some(render) = assistant_output.delta(&delta.content) {
                                if render.started && !is_streaming {
                                    self.renderer.clear_working();
                                    is_streaming = true;
                                }
                                self.renderer.assistant_delta(&render.content).await?;
                            }
                            self.model.append_agent_event(&event)?;
                            title_text.push_str(&delta.content);
                            if should_spawn_title_task(title_spawned, &title_text) {
                                spawn_title_task(
                                    self.model.session_manager().clone(),
                                    request.prompt.clone(),
                                    title_text.clone(),
                                    &request.runtime,
                                    self.renderer,
                                    self.model.debug_logger().clone(),
                                )?;
                                title_spawned = true;
                            }
                            continue;
                        }
                    }

                    if assistant_output.close_visible_response() {
                        println!("\n");
                    }
                    if is_streaming {
                        is_streaming = false;
                    }
                    render_agent_event(self.renderer, &self.tools, &event).await?;
                    self.model.append_agent_event(&event)?;

                    if is_terminal_agent_event(&event) {
                        if spinner_visible {
                            self.renderer.clear_working();
                            spinner_visible = false;
                        }
                        break;
                    }
                }
            }
        }

        if spinner_visible {
            self.renderer.clear_working();
        }
        if assistant_output.close_visible_response() {
            println!("\n");
        }

        Ok(())
    }
}

/// Builds the built-in tool storage scoped to the injected workspace root.
pub fn main_chat_tool_storage(
    workspace_root: impl Into<PathBuf>,
) -> Result<ToolStorage, ToolRegistrationError> {
    spectacular_tools::built_in_tools(workspace_root)
}

/// Creates the main coding agent configured with runtime model, reasoning, and tools.
fn main_chat_agent<P>(
    provider: P,
    runtime: &RuntimeSelection,
    store: Store,
    tools: ToolStorage,
) -> Agent<P>
where
    P: LlmProvider,
{
    let reasoning_effort = runtime_reasoning_effort(runtime.reasoning);
    Agent::with_config_and_store(
        provider,
        AgentConfig {
            model: Some(runtime.model.clone()),
            require_usage_metadata: false,
            include_reasoning: reasoning_effort.is_some(),
            reasoning_effort,
            system_prompt: CODING_AGENT_SYSTEM_PROMPT.to_string(),
            ..AgentConfig::default()
        },
        store,
    )
    .with_tools(tools)
}

/// Converts a configured reasoning level into an optional provider effort string.
fn runtime_reasoning_effort(reasoning: ReasoningLevel) -> Option<String> {
    match reasoning {
        ReasoningLevel::None => None,
        level => Some(level.as_str().to_owned()),
    }
}

#[derive(Default)]
struct AssistantResponseRenderState {
    pending: String,
    visible: bool,
}

struct AssistantDeltaRender {
    content: String,
    started: bool,
}

impl AssistantResponseRenderState {
    /// Returns newly visible assistant text once accumulated deltas contain nonblank content.
    fn delta(&mut self, content: &str) -> Option<AssistantDeltaRender> {
        if self.visible {
            return Some(AssistantDeltaRender {
                content: content.to_owned(),
                started: false,
            });
        }

        self.pending.push_str(content);
        if !has_visible_assistant_text(&self.pending) {
            return None;
        }

        self.visible = true;
        Some(AssistantDeltaRender {
            content: std::mem::take(&mut self.pending),
            started: true,
        })
    }

    /// Closes any visible response and reports whether a spacer line should be emitted.
    fn close_visible_response(&mut self) -> bool {
        self.pending.clear();
        if !self.visible {
            return false;
        }

        self.visible = false;
        true
    }
}

/// Renders a persisted or streamed agent event without appending it to session storage.
pub async fn render_agent_event(
    renderer: &Renderer,
    tools: &ToolStorage,
    event: &AgentEvent,
) -> Result<(), ChatError> {
    match event {
        AgentEvent::UserPrompt { content } => renderer.user_prompt(content),
        AgentEvent::MessageDelta(delta) if delta.role == ProviderMessageRole::Assistant => {
            if !has_visible_assistant_text(&delta.content) {
                return Ok(());
            }

            renderer.assistant_delta(&delta.content).await?;
        }
        AgentEvent::ReasoningDelta(_) => {}
        AgentEvent::AssistantToolCallRequest {
            tool_call_id,
            name,
            arguments,
        } => {
            renderer.clear_working();
            renderer.tool_call(tool_call_id, name, arguments, tools);
            renderer.working();
        }
        AgentEvent::ToolResult { name, content, .. } => {
            renderer.clear_working();
            renderer.tool_result(name, content, tools);
            renderer.working();
        }
        AgentEvent::ValidationError { message } | AgentEvent::Error { message } => {
            renderer.clear_working();
            renderer.error(message);
            renderer.working();
        }
        AgentEvent::Cancelled { reason } => renderer.cancelled(reason),
        AgentEvent::Finished { .. }
        | AgentEvent::UsageMetadata(_)
        | AgentEvent::ReasoningMetadata(_)
        | AgentEvent::Internal { .. } => {}
        AgentEvent::MessageDelta(_) => {}
        _ => {}
    }

    Ok(())
}

/// Consumes the replayed retry user prompt so it is not rendered or stored twice.
fn should_skip_retry_user_prompt(skip_retry_user: &mut bool, event: &AgentEvent) -> bool {
    if *skip_retry_user && matches!(event, AgentEvent::UserPrompt { .. }) {
        *skip_retry_user = false;
        return true;
    }

    false
}

/// Returns whether an event should be persisted without immediate terminal rendering.
fn should_append_without_render(event: &AgentEvent) -> bool {
    matches!(event, AgentEvent::UserPrompt { .. })
}

/// Returns whether enough assistant text exists to launch title generation once.
fn should_spawn_title_task(title_spawned: bool, title_text: &str) -> bool {
    !title_spawned && !title_text.trim().is_empty()
}

/// Returns whether an event ends the active agent stream for the current prompt.
fn is_terminal_agent_event(event: &AgentEvent) -> bool {
    matches!(
        event,
        AgentEvent::Finished { .. } | AgentEvent::Error { .. } | AgentEvent::Cancelled { .. }
    )
}

#[cfg(test)]
mod tests {
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/unit/chat/runner.rs"
    ));
}
