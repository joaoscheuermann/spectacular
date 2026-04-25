use spectacular_agent::{Agent, AgentConfig, AgentEvent, OutputSchema};
use spectacular_llms::{
    provider_by_id, Cancellation, FinishReason, LlmProvider, MessageDelta, Model, ProviderCall,
    ProviderCapabilities, ProviderContextLimits, ProviderError, ProviderFinished, ProviderMetadata,
    ProviderRequest, ProviderStream, ProviderStreamEvent, UsageMetadata, ValidationMode,
    OPENROUTER_PROVIDER_ID,
};

#[derive(Clone, Debug)]
struct FakeStructuredProvider {
    response: &'static str,
}

impl LlmProvider for FakeStructuredProvider {
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
            structured_output: true,
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
        Box::pin(async move {
            let stream: ProviderStream = Box::new(
                vec![
                    Ok(ProviderStreamEvent::MessageDelta(MessageDelta::assistant(
                        self.response,
                    ))),
                    Ok(ProviderStreamEvent::Finished(ProviderFinished {
                        finish_reason: FinishReason::Stop,
                        tool_calls: Vec::new(),
                        usage: Some(UsageMetadata {
                            input_tokens: Some(5),
                            output_tokens: Some(7),
                            total_tokens: Some(12),
                        }),
                        reasoning: None,
                    })),
                ]
                .into_iter(),
            );
            Ok(stream)
        })
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    run_case("valid", r#"{"summary":"Release ready","status":"ready"}"#)?;
    run_case("invalid", r#"{"summary":"Release ready","status":"draft"}"#)?;
    Ok(())
}

fn run_case(label: &str, response: &'static str) -> Result<(), Box<dyn std::error::Error>> {
    let schema = OutputSchema::from_json_str(
        r#"{
            "type": "object",
            "required": ["summary", "status"],
            "properties": {
                "summary": { "type": "string" },
                "status": { "const": "ready" }
            },
            "additionalProperties": false
        }"#,
    )?;
    let mut agent = Agent::with_config(
        FakeStructuredProvider { response },
        AgentConfig {
            output_schema: Some(schema),
            ..AgentConfig::default()
        },
    );
    agent.enqueue_prompt("Return structured release status");

    println!("== {label} structured output ==");
    match futures::executor::block_on(agent.run_next()) {
        Ok(run_id) => println!("run {} finished successfully", run_id.value()),
        Err(error) => println!("run failed: {error}"),
    }

    for event in agent.events() {
        match event {
            AgentEvent::MessageDelta(_)
            | AgentEvent::UsageMetadata(_)
            | AgentEvent::Finished { .. }
            | AgentEvent::ValidationError { .. }
            | AgentEvent::Error { .. } => println!("{event}"),
            _ => {}
        }
    }

    Ok(())
}
