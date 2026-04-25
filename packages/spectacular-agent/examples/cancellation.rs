use spectacular_agent::{Agent, AgentError};
use spectacular_llms::{
    provider_by_id, Cancellation, LlmProvider, Model, ProviderCall, ProviderCapabilities,
    ProviderContextLimits, ProviderError, ProviderMetadata, ProviderRequest, ValidationMode,
    OPENROUTER_PROVIDER_ID,
};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::oneshot;

#[derive(Clone, Debug)]
struct SlowProvider {
    started: Arc<std::sync::Mutex<Option<oneshot::Sender<()>>>>,
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
        ProviderCapabilities {
            streaming: true,
            tool_calls: false,
            structured_output: false,
            reasoning: false,
            cancellation: true,
            usage_metadata: true,
            reasoning_metadata: false,
            context_limits: ProviderContextLimits::default(),
        }
    }

    fn stream_completion<'a>(
        &'a self,
        _request: ProviderRequest,
        cancellation: Cancellation,
    ) -> ProviderCall<'a> {
        let started = Arc::clone(&self.started);
        Box::pin(async move {
            if let Some(started) = started.lock().unwrap().take() {
                let _ = started.send(());
            }
            loop {
                if cancellation.is_cancelled() {
                    return Err(ProviderError::CancellationError);
                }
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        })
    }
}

#[tokio::main]
async fn main() {
    let (sender, receiver) = oneshot::channel();
    let agent = Arc::new(Agent::new(SlowProvider {
        started: Arc::new(std::sync::Mutex::new(Some(sender))),
    }));

    let active = tokio::spawn({
        let agent = Arc::clone(&agent);
        async move { agent.run("active prompt").await }
    });
    receiver.await.expect("provider should start");

    let queued = tokio::spawn({
        let agent = Arc::clone(&agent);
        async move { agent.run("queued prompt").await }
    });
    tokio::task::yield_now().await;

    println!("cancel requested: {}", agent.cancel_active().await);
    println!("active: {}", status(active.await.unwrap()));
    println!("queued: {}", status(queued.await.unwrap()));

    for (index, event) in agent.events().iter().enumerate() {
        println!("{index}: {event}");
    }
}

fn status(result: Result<spectacular_agent::RunId, AgentError>) -> &'static str {
    match result {
        Ok(_) => "Ok",
        Err(AgentError::CancellationError) => "Err(AgentError::CancellationError)",
        Err(_) => "Err(other)",
    }
}
