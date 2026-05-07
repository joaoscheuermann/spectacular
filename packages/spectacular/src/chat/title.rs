use crate::chat::provider::provider_for_parts;
use crate::chat::renderer::Renderer;
use crate::chat::session::SessionManager;
use crate::chat::{ChatError, RuntimeSelection};
use spectacular_agent::{Agent, AgentEvent, Store};
use spectacular_config::{ConfigError, TaskModelConfig, TaskModelSlot};
use spectacular_llms::{LlmDebugLogger, LlmProvider};
use std::sync::Arc;

pub(super) fn spawn_title_task(
    session: SessionManager,
    prompt: String,
    response: String,
    fallback_runtime: &RuntimeSelection,
    renderer: &Renderer,
    debug_logger: LlmDebugLogger,
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
        let Ok(provider) = provider_for_parts(&provider, api_key, debug_logger) else {
            return;
        };
        let agent = Arc::new(title_generation_agent(
            provider,
            model.model.clone(),
            system_prompt,
            Store::default(),
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

pub(super) fn title_generation_agent<P>(
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
        spectacular_agent::AgentConfig {
            system_prompt,
            model: Some(model),
            require_usage_metadata: false,
            ..spectacular_agent::AgentConfig::default()
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
