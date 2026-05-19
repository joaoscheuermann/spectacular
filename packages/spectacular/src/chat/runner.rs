use crate::chat::model::ChatRunRequestModel;
use crate::chat::provider::provider_for_runtime;
use crate::chat::renderer::Renderer;
use crate::chat::session::{agent_events_from_records, records_before_latest_user_prompt};
use crate::chat::title::spawn_title_task;
use crate::chat::{ChatError, ChatModel, RuntimeSelection};

const CODING_AGENT_SYSTEM_PROMPT: &str = include_str!("prompt/coding-agent.md");
use spectacular_agent::{
    Agent, AgentConfig, AgentEvent, ContextPolicy, Store, ToolRegistrationError, ToolStorage,
};
use spectacular_config::ReasoningLevel;
use spectacular_llms::LlmProvider;
use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::time::MissedTickBehavior;

mod event_rendering;
mod render_state;
mod token_usage;

pub use event_rendering::render_agent_event;
use render_state::{AssistantResponseRenderState, ReasoningResponseRenderState};
#[cfg(test)]
use token_usage::is_visible_assistant_delta;
use token_usage::{record_generated_tokens, AssistantTurnTokenCounter};

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
    pub prompt_event_id: Option<String>,
    pub render_user_prompt: bool,
    pub retry_existing_prompt: bool,
    pub runtime: RuntimeSelection,
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
                prompt_event_id: request.prompt_event_id,
                render_user_prompt: request.render_user_prompt,
                retry_existing_prompt: request.retry_existing_prompt,
                runtime: request.runtime,
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
            self.renderer.user_prompt(&request.prompt);
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
        let mut stream = Arc::clone(&agent).run_stream_with_prompt_event_id(
            request.prompt.clone(),
            request.prompt_event_id.clone(),
        );
        let mut title_text = String::new();
        let mut assistant_output = AssistantResponseRenderState::default();
        let mut reasoning_output = ReasoningResponseRenderState::default();
        let mut title_spawned = self.model.session_manager().has_title()?;
        let mut spinner_visible = true;
        let mut spinner_frame = 0usize;
        let mut turn_token_counter = AssistantTurnTokenCounter::for_model(&request.runtime.model);
        let mut is_streaming = false;
        let mut spinner = tokio::time::interval(Duration::from_millis(90));
        let mut skip_retry_user = request.retry_existing_prompt;
        let started_at = Instant::now();
        spinner.set_missed_tick_behavior(MissedTickBehavior::Delay);
        self.renderer.working();

        loop {
            tokio::select! {
                _ = spinner.tick(), if spinner_visible => {
                    spinner_frame = spinner_frame.wrapping_add(1);
                    self.renderer
                        .working_frame(spinner_frame, turn_token_counter.current_tokens());
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
                    if let AgentEvent::ContextTokenUsage(usage) = event {
                        self.model.set_context_token_usage(usage);
                        continue;
                    }
                    record_generated_tokens(&event, &mut turn_token_counter);
                    if let AgentEvent::UsageMetadata(_) = &event {
                        if spinner_visible {
                            self.renderer.working_frame(
                                spinner_frame,
                                turn_token_counter.current_tokens(),
                            );
                        }
                    }

                    if let AgentEvent::MessageDelta { content, .. } = &event {
                        if reasoning_output.close_visible_response() {
                            self.renderer.response_spacer();
                            if is_streaming {
                                self.renderer.resume_working_line();
                                is_streaming = false;
                            }
                        }
                        if let Some(render) = assistant_output.delta(content) {
                            if render.started && !is_streaming {
                                self.renderer.pause_working_line();
                                is_streaming = true;
                            }
                            self.renderer.assistant_delta(&render.content)?;
                        }
                        self.model.append_agent_event(&event)?;
                        title_text.push_str(content);
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

                    if let AgentEvent::ReasoningDelta { content, .. } = &event {
                        if spinner_visible {
                            self.renderer.working_frame(
                                spinner_frame,
                                turn_token_counter.current_tokens(),
                            );
                        }
                        if assistant_output.close_visible_response() {
                            self.renderer.response_spacer();
                            if is_streaming {
                                self.renderer.resume_working_line();
                                is_streaming = false;
                            }
                        }
                        if let Some(render) = reasoning_output.delta(content) {
                            if render.started && !is_streaming {
                                self.renderer.pause_working_line();
                                is_streaming = true;
                            }
                            self.renderer.reasoning_delta(&render.content)?;
                        }
                        self.model.append_agent_event(&event)?;
                        continue;
                    }

                    if assistant_output.close_visible_response() {
                        self.renderer.response_spacer();
                    }
                    if reasoning_output.close_visible_response() {
                        self.renderer.response_spacer();
                    }
                    if is_streaming {
                        self.renderer.resume_working_line();
                        is_streaming = false;
                    }
                    let refresh_after_render = should_refresh_working_tokens(spinner_visible, is_streaming, &event);
                    render_agent_event(self.renderer, &self.tools, &event).await?;
                    if refresh_after_render {
                        self.renderer
                            .working_frame(spinner_frame, turn_token_counter.current_tokens());
                    }
                    self.model.append_agent_event(&event)?;

                    if is_terminal_agent_event(&event) {
                        if spinner_visible && !matches!(event, AgentEvent::Finished { .. }) {
                            self.renderer.clear_working();
                            spinner_visible = false;
                        }
                        break;
                    }
                }
            }
        }

        let should_render_worked = spinner_visible;
        if spinner_visible {
            self.renderer.clear_working();
        }
        if assistant_output.close_visible_response() {
            self.renderer.response_spacer();
        }
        if reasoning_output.close_visible_response() {
            self.renderer.response_spacer();
        }
        if is_streaming {
            self.renderer.resume_working_line();
        }
        if should_render_worked {
            self.renderer
                .worked(started_at.elapsed(), turn_token_counter.current_tokens());
        }

        Ok(())
    }
}

/// Builds the built-in tool storage scoped to the injected workspace root.
pub fn main_chat_tool_storage(
    workspace_root: impl Into<PathBuf>,
    trace_dir: impl Into<PathBuf>,
) -> Result<ToolStorage, ToolRegistrationError> {
    spectacular_tools::built_in_tools_with_trace_dir(workspace_root, trace_dir)
}

/// Creates the main coding agent configured with runtime model, reasoning, and tools.
pub(crate) fn main_chat_agent<P>(
    provider: P,
    runtime: &RuntimeSelection,
    store: Store,
    tools: ToolStorage,
) -> Agent<P>
where
    P: LlmProvider,
{
    let reasoning_effort = runtime_reasoning_effort(runtime.reasoning);
    let context_window_tokens = runtime_context_window_tokens(&provider, runtime);
    let config = AgentConfig {
        model: Some(runtime.model.clone()),
        require_usage_metadata: false,
        include_reasoning: reasoning_effort.is_some(),
        reasoning_effort,
        system_prompt: CODING_AGENT_SYSTEM_PROMPT.to_string(),
        context_policy: context_policy_for_runtime(runtime, context_window_tokens),
        ..AgentConfig::default()
    };

    Agent::with_config_and_store(provider, config, store).with_tools(tools)
}

/// Resolves the context window from cached runtime metadata before consulting the provider.
fn runtime_context_window_tokens<P>(provider: &P, runtime: &RuntimeSelection) -> Option<usize>
where
    P: LlmProvider,
{
    runtime
        .context_window_tokens
        .or_else(|| provider.context_window_tokens(&runtime.model))
}

/// Builds the context policy used by normal chat runs for the selected runtime model.
fn context_policy_for_runtime(
    runtime: &RuntimeSelection,
    context_window_tokens: Option<usize>,
) -> ContextPolicy {
    let mut policy = ContextPolicy::default();
    policy.model_context_window_tokens = context_window_tokens;
    policy.reasoning_reserve_tokens = reasoning_reserve_tokens(runtime.reasoning);
    policy.max_summary_passes_per_request = 4;
    policy
}

/// Reserves additional input budget for models configured to spend reasoning tokens.
fn reasoning_reserve_tokens(reasoning: ReasoningLevel) -> usize {
    if reasoning.non_none() {
        return 8_192;
    }

    0
}

/// Converts a configured reasoning level into an optional provider effort string.
fn runtime_reasoning_effort(reasoning: ReasoningLevel) -> Option<String> {
    match reasoning {
        ReasoningLevel::None => None,
        level => Some(level.as_str().to_owned()),
    }
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

/// Returns whether the runner should repaint the working line with current turn tokens.
fn should_refresh_working_tokens(
    spinner_visible: bool,
    is_streaming: bool,
    event: &AgentEvent,
) -> bool {
    spinner_visible && !is_streaming && !is_terminal_agent_event(event)
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
