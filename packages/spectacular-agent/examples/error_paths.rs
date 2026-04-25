use spectacular_agent::Agent;
use spectacular_llms::{
    provider_by_id, Cancellation, LlmProvider, Model, ProviderCall, ProviderCapabilities,
    ProviderContextLimits, ProviderError, ProviderMetadata, ProviderRequest, ValidationMode,
    OPENROUTER_PROVIDER_ID,
};

#[derive(Clone, Debug)]
struct ErrorProvider {
    error: ProviderError,
}

impl LlmProvider for ErrorProvider {
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
        _cancellation: Cancellation,
    ) -> ProviderCall<'a> {
        let error = self.error.clone();
        Box::pin(async move { Err(error) })
    }
}

fn main() {
    run_case(
        "provider network disconnect",
        ProviderError::NetworkError {
            provider_name: "FakeProvider".to_owned(),
            reason: "disconnect".to_owned(),
        },
    );
    run_case(
        "provider capability mismatch",
        ProviderError::CapabilityMismatch {
            provider_name: "FakeProvider".to_owned(),
            capability: "json_schema".to_owned(),
        },
    );
    run_case("provider cancellation", ProviderError::CancellationError);
}

fn run_case(label: &str, error: ProviderError) {
    let mut agent = Agent::new(ErrorProvider { error });
    agent.enqueue_prompt("hello");

    println!("== {label} ==");
    println!(
        "result: {:?}",
        futures::executor::block_on(agent.run_next())
    );
    for event in agent.events() {
        println!("{event}");
    }
}
