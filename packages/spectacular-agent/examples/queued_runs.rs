use spectacular_agent::Agent;
use spectacular_llms::{
    provider_by_id, Cancellation, FinishReason, LlmProvider, MessageDelta, Model, ProviderCall,
    ProviderCapabilities, ProviderContextLimits, ProviderError, ProviderFinished,
    ProviderMessageRole, ProviderMetadata, ProviderRequest, ProviderStream, ProviderStreamEvent,
    UsageMetadata, ValidationMode, OPENROUTER_PROVIDER_ID,
};
use std::sync::Arc;
use std::time::Duration;

#[derive(Clone, Debug)]
struct DelayedProvider;

impl LlmProvider for DelayedProvider {
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
            cancellation: false,
            usage_metadata: true,
            reasoning_metadata: false,
            context_limits: ProviderContextLimits::default(),
        }
    }

    fn stream_completion<'a>(
        &'a self,
        request: ProviderRequest,
        _cancellation: Cancellation,
    ) -> ProviderCall<'a> {
        Box::pin(async move {
            let prompt = request
                .messages
                .iter()
                .rev()
                .find(|message| message.role == ProviderMessageRole::User)
                .map(|message| message.content.clone())
                .unwrap_or_default();
            tokio::time::sleep(Duration::from_millis(match prompt.as_str() {
                "first" => 30,
                "second" => 1,
                _ => 10,
            }))
            .await;

            let stream = ProviderStream::from_events(vec![
                Ok(ProviderStreamEvent::MessageDelta(MessageDelta::assistant(
                    format!("answer {prompt}"),
                ))),
                Ok(ProviderStreamEvent::Finished(ProviderFinished {
                    finish_reason: FinishReason::Stop,
                    tool_calls: Vec::new(),
                    usage: Some(UsageMetadata {
                        input_tokens: Some(1),
                        output_tokens: Some(1),
                        total_tokens: Some(2),
                    }),
                    reasoning: None,
                })),
            ]);
            Ok(stream)
        })
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let agent = Arc::new(Agent::new(DelayedProvider));
    let (first, second, third) =
        tokio::join!(agent.run("first"), agent.run("second"), agent.run("third"));

    println!("first: {:?}", first.map(|id| id.value()));
    println!("second: {:?}", second.map(|id| id.value()));
    println!("third: {:?}", third.map(|id| id.value()));

    for (index, event) in agent.events().iter().enumerate() {
        println!("{index}: {event}");
    }

    Ok(())
}
