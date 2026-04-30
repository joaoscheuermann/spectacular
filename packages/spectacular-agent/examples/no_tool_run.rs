use spectacular_agent::{Agent, AgentEvent};
use spectacular_llms::{
    provider_by_id, Cancellation, FinishReason, LlmProvider, MessageDelta, Model, ProviderCall,
    ProviderCapabilities, ProviderContextLimits, ProviderError, ProviderFinished, ProviderMetadata,
    ProviderRequest, ProviderStream, ProviderStreamEvent, UsageMetadata, ValidationMode,
    OPENROUTER_PROVIDER_ID,
};

#[derive(Clone, Debug)]
struct FakeProvider;

impl LlmProvider for FakeProvider {
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
        _request: ProviderRequest,
        _cancellation: Cancellation,
    ) -> ProviderCall<'a> {
        Box::pin(async {
            let stream = ProviderStream::from_events(vec![
                Ok(ProviderStreamEvent::MessageDelta(MessageDelta::assistant(
                    "fake assistant response",
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

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut agent = Agent::new(FakeProvider);
    agent.enqueue_prompt("hello");
    futures::executor::block_on(agent.run_next())?;

    for (index, event) in agent.events().iter().enumerate() {
        print_event(index, event);
    }

    Ok(())
}

fn print_event(index: usize, event: &AgentEvent) {
    println!("{index}: {event}");
}
