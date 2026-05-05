use crate::chat::provider::{provider_for_parts, provider_for_runtime};
use crate::chat::renderer::Renderer;
use crate::chat::session::{
    agent_events_from_records, records_before_latest_user_prompt, SessionManager,
};
use crate::chat::{ChatError, RuntimeSelection};
use spectacular_agent::{
    Agent, AgentConfig, AgentEvent, Store, ToolRegistrationError, ToolStorage,
};
use spectacular_config::{ConfigError, ReasoningLevel, TaskModelConfig, TaskModelSlot};
use spectacular_llms::{LlmProvider, ProviderMessageRole};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::MissedTickBehavior;

pub struct ChatRunRequest {
    pub prompt: String,
    pub render_user_prompt: bool,
    pub retry_existing_prompt: bool,
    pub runtime: RuntimeSelection,
}

pub struct ChatRunner<'a> {
    session: &'a SessionManager,
    renderer: &'a Renderer,
    tools: ToolStorage,
}

impl<'a> ChatRunner<'a> {
    pub fn new(session: &'a SessionManager, renderer: &'a Renderer, tools: ToolStorage) -> Self {
        Self {
            session,
            renderer,
            tools,
        }
    }

    pub async fn run(&self, request: ChatRunRequest) -> Result<(), ChatError> {
        if request.render_user_prompt {
            self.renderer.user_prompt(&request.prompt);
        }

        let records = self.session.records()?;
        let context_records = if request.retry_existing_prompt {
            records_before_latest_user_prompt(&records)
        } else {
            records.as_slice()
        };
        let store = Store::from(agent_events_from_records(context_records));
        let agent = Arc::new(main_chat_agent(
            provider_for_runtime(&request.runtime)?,
            &request.runtime,
            store,
            self.tools.clone(),
        ));
        let mut stream = agent.run_stream(request.prompt.clone());
        let mut title_text = String::new();
        let mut response_open = false;
        let mut title_spawned = self.session.has_title()?;
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
                    if skip_retry_user && matches!(event, AgentEvent::UserPrompt { .. }) {
                        skip_retry_user = false;
                        continue;
                    }
                    if matches!(event, AgentEvent::UserPrompt { .. }) {
                        self.session.append_agent_event(&event)?;
                        continue;
                    }

                    let is_assistant_delta = matches!(
                        &event,
                        AgentEvent::MessageDelta(delta) if delta.role == ProviderMessageRole::Assistant
                    );
                    if is_assistant_delta && !is_streaming {
                        self.renderer.clear_working();
                        is_streaming = true;
                    } else if is_assistant_delta {
                        // still streaming, spinner stays hidden
                    } else if is_streaming {
                        // transition away from streaming
                        is_streaming = false;
                    }
                    if response_open && !is_assistant_delta {
                        println!("\n");
                        response_open = false;
                    }
                    render_agent_event(self.renderer, &self.tools, &event).await?;
                    self.session.append_agent_event(&event)?;
                    if let AgentEvent::MessageDelta(delta) = &event {
                        if delta.role == ProviderMessageRole::Assistant {
                            response_open = true;
                            title_text.push_str(&delta.content);
                            if !title_spawned && !title_text.trim().is_empty() {
                                spawn_title_task(
                                    self.session.clone(),
                                    request.prompt.clone(),
                                    title_text.clone(),
                                    &request.runtime,
                                    self.renderer,
                                )?;
                                title_spawned = true;
                            }
                        }
                    }

                    if matches!(
                        event,
                        AgentEvent::Finished { .. } | AgentEvent::Error { .. } | AgentEvent::Cancelled { .. }
                    ) {
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
        if response_open {
            println!("\n");
        }

        Ok(())
    }
}

pub fn main_chat_tool_storage(
    workspace_root: impl Into<PathBuf>,
) -> Result<ToolStorage, ToolRegistrationError> {
    spectacular_tools::built_in_tools(workspace_root)
}

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
            ..AgentConfig::default()
        },
        store,
    )
    .with_tools(tools)
}

fn runtime_reasoning_effort(reasoning: ReasoningLevel) -> Option<String> {
    match reasoning {
        ReasoningLevel::None => None,
        level => Some(level.as_str().to_owned()),
    }
}

pub async fn render_agent_event(
    renderer: &Renderer,
    tools: &ToolStorage,
    event: &AgentEvent,
) -> Result<(), ChatError> {
    match event {
        AgentEvent::UserPrompt { content } => renderer.user_prompt(content),
        AgentEvent::MessageDelta(delta) if delta.role == ProviderMessageRole::Assistant => {
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

fn spawn_title_task(
    session: SessionManager,
    prompt: String,
    response: String,
    fallback_runtime: &RuntimeSelection,
    renderer: &Renderer,
) -> Result<(), ChatError> {
    let config = spectacular_config::read_config_or_default()?;
    let (provider, slot, model, api_key) = title_model(&config, fallback_runtime)?;
    let fallback = slot == TaskModelSlot::Coding;
    if fallback {
        renderer.warning("labeling model is not configured; using coding model for session title");
    }

    tokio::spawn(async move {
        let system_prompt = "Generate a chat title with maximum of 6 words. You will get a User prompt and an Assistant response, use both to generate a title. Only return the title, no other data or text".to_owned();

        let title_prompt =
            format!("Return only the title. \n\nUser: {prompt}\nAssistant: {response}");

        let Ok(provider) = provider_for_parts(&provider, api_key) else {
            return;
        };
        let store = Store::default();
        let agent = Arc::new(title_generation_agent(
            provider,
            model.model.clone(),
            system_prompt,
            store,
        ));
        let mut stream = agent.run_stream(title_prompt);
        let mut title = String::new();
        while let Some(event) = stream.next().await {
            match event {
                AgentEvent::MessageDelta(delta) => title.push_str(&delta.content),
                AgentEvent::Finished { .. } => break,
                AgentEvent::Error { .. } | AgentEvent::Cancelled { .. } => return,
                _ => {}
            }
        }

        let title = sanitize_title(&title);
        if title.is_empty() {
            return;
        }

        let _ = session.append_title(&title, slot, &model.model, fallback);
    });

    Ok(())
}

fn title_generation_agent<P>(
    provider: P,
    model: String,
    system_prompt: String,
    store: Store,
) -> Agent<P>
where
    P: LlmProvider,
{
    Agent::with_config_and_store(
        provider,
        AgentConfig {
            system_prompt,
            model: Some(model),
            require_usage_metadata: false,
            ..AgentConfig::default()
        },
        store,
    )
}

fn title_model(
    config: &spectacular_config::SpectacularConfig,
    fallback: &RuntimeSelection,
) -> Result<(String, TaskModelSlot, TaskModelConfig, String), ChatError> {
    let provider = config
        .providers
        .selected
        .as_deref()
        .ok_or(ConfigError::NoSelectedProvider)?;
    let provider_config = config.providers.available.get(provider).ok_or_else(|| {
        ConfigError::ProviderNotConfigured {
            provider: provider.to_owned(),
        }
    })?;
    let api_key = provider_config
        .key
        .as_deref()
        .filter(|key| !key.trim().is_empty())
        .ok_or_else(|| ConfigError::MissingProviderApiKey {
            provider: provider.to_owned(),
        })?
        .to_owned();

    if let Some(labeling) = provider_config
        .tasks
        .labeling
        .as_ref()
        .filter(|task| !task.model.trim().is_empty())
    {
        return Ok((
            provider.to_owned(),
            TaskModelSlot::Labeling,
            labeling.clone(),
            api_key,
        ));
    }

    Ok((
        provider.to_owned(),
        TaskModelSlot::Coding,
        TaskModelConfig::new(fallback.model.clone(), fallback.reasoning),
        api_key,
    ))
}

fn sanitize_title(title: &str) -> String {
    title
        .trim()
        .trim_matches('"')
        .split_whitespace()
        .take(6)
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use spectacular_config::ReasoningLevel;
    use spectacular_llms::{OpenRouterProvider, OPENROUTER_PROVIDER_ID};
    use spectacular_tools::{
        EDIT_TOOL_NAME, FIND_TOOL_NAME, GREP_TOOL_NAME, TERMINAL_TOOL_NAME, TREE_TOOL_NAME,
        WEB_SEARCH_TOOL_NAME, WRITE_TOOL_NAME,
    };

    #[test]
    fn main_chat_agent_gets_built_in_tools_and_title_agent_stays_text_only() {
        let tools = main_chat_tool_storage(PathBuf::from("workspace")).unwrap();
        let runtime = RuntimeSelection {
            provider: OPENROUTER_PROVIDER_ID.to_owned(),
            api_key: "sk-or-v1-test".to_owned(),
            model: "test/model".to_owned(),
            reasoning: ReasoningLevel::Medium,
        };

        let main_agent = main_chat_agent(
            OpenRouterProvider::new(runtime.api_key.clone()),
            &runtime,
            Store::default(),
            tools,
        );
        let title_agent = title_generation_agent(
            OpenRouterProvider::new(runtime.api_key),
            "title/model".to_owned(),
            "Generate a title.".to_owned(),
            Store::default(),
        );

        assert_eq!(
            main_agent
                .tool_manifests()
                .into_iter()
                .map(|manifest| manifest.name)
                .collect::<Vec<_>>(),
            vec![
                EDIT_TOOL_NAME,
                FIND_TOOL_NAME,
                GREP_TOOL_NAME,
                TERMINAL_TOOL_NAME,
                TREE_TOOL_NAME,
                WEB_SEARCH_TOOL_NAME,
                WRITE_TOOL_NAME
            ]
        );
        assert!(title_agent.tool_manifests().is_empty());
    }
}
