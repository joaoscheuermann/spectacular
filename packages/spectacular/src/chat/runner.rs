use crate::chat::provider::{provider_for_parts, provider_for_runtime};
use crate::chat::renderer::Renderer;
use crate::chat::session::{
    agent_events_from_records, records_before_latest_user_prompt, SessionManager,
};
use crate::chat::{ChatError, RuntimeSelection};
use spectacular_agent::{Agent, AgentConfig, AgentEvent, Store};
use spectacular_config::{ConfigError, TaskModelConfig, TaskModelSlot};
use spectacular_llms::ProviderMessageRole;
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
}

impl<'a> ChatRunner<'a> {
    pub fn new(session: &'a SessionManager, renderer: &'a Renderer) -> Self {
        Self { session, renderer }
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
        let agent = Arc::new(Agent::with_config_and_store(
            provider_for_runtime(&request.runtime)?,
            AgentConfig {
                model: Some(request.runtime.model.clone()),
                require_usage_metadata: false,
                ..AgentConfig::default()
            },
            store,
        ));
        let mut stream = agent.run_stream(request.prompt.clone());
        let mut title_text = String::new();
        let mut response_open = false;
        let mut title_spawned = self.session.has_title()?;
        let mut spinner_visible = true;
        let mut spinner_frame = 0usize;
        let mut spinner = tokio::time::interval(Duration::from_millis(90));
        let mut skip_retry_user = request.retry_existing_prompt;
        spinner.set_missed_tick_behavior(MissedTickBehavior::Delay);
        self.renderer.working();

        loop {
            tokio::select! {
                _ = spinner.tick(), if spinner_visible => {
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

                    if spinner_visible {
                        self.renderer.clear_working();
                        spinner_visible = false;
                    }
                    let is_assistant_delta = matches!(
                        &event,
                        AgentEvent::MessageDelta(delta) if delta.role == ProviderMessageRole::Assistant
                    );
                    if response_open && !is_assistant_delta {
                        println!("\n");
                        response_open = false;
                    }
                    render_agent_event(self.renderer, &event).await?;
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

pub async fn render_agent_event(renderer: &Renderer, event: &AgentEvent) -> Result<(), ChatError> {
    match event {
        AgentEvent::UserPrompt { content } => renderer.user_prompt(content),
        AgentEvent::MessageDelta(delta) if delta.role == ProviderMessageRole::Assistant => {
            renderer.assistant_delta(&delta.content).await?;
        }
        AgentEvent::ReasoningDelta(_) => {}
        AgentEvent::AssistantToolCallRequest { content } => renderer.tool_call(content),
        AgentEvent::ToolResult { content } => renderer.tool_result(content),
        AgentEvent::ValidationError { message } | AgentEvent::Error { message } => {
            renderer.error(message)
        }
        AgentEvent::Cancelled { reason } => renderer.cancelled(reason),
        AgentEvent::Finished { .. }
        | AgentEvent::UsageMetadata(_)
        | AgentEvent::ReasoningMetadata(_)
        | AgentEvent::Internal { .. } => {}
        AgentEvent::MessageDelta(_) => {}
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
        let title_prompt = format!(
            "Create a concise title for this chat session. Return only the title. Max 6 words.\n\nUser: {prompt}\nAssistant: {response}"
        );
        let Ok(provider) = provider_for_parts(&provider, api_key) else {
            return;
        };
        let store = Store::default();
        let agent = Arc::new(Agent::with_config_and_store(
            provider,
            AgentConfig {
                model: Some(model.model.clone()),
                require_usage_metadata: false,
                ..AgentConfig::default()
            },
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
