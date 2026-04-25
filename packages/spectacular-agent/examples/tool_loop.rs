use serde_json::{json, Value};
use spectacular_agent::{Agent, AgentEvent, Tool, ToolExecution};
use spectacular_llms::{
    provider_by_id, Cancellation, FinishReason, LlmProvider, MessageDelta, Model, ProviderCall,
    ProviderCapabilities, ProviderContextLimits, ProviderError, ProviderFinished, ProviderMetadata,
    ProviderRequest, ProviderStream, ProviderStreamEvent, ProviderToolCall, UsageMetadata,
    ValidationMode, OPENROUTER_PROVIDER_ID,
};
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};

#[derive(Clone, Debug)]
struct FakeTool {
    name: &'static str,
}

impl Tool for FakeTool {
    fn name(&self) -> &str {
        self.name
    }

    fn execute<'a>(&'a self, arguments: Value, _cancellation: Cancellation) -> ToolExecution<'a> {
        Box::pin(async move {
            Ok(json!({
                "tool": self.name,
                "arguments": arguments,
                "output": format!("{} result", self.name),
            }))
        })
    }
}

#[derive(Clone, Debug)]
struct FakeToolLoopProvider {
    calls: Arc<AtomicUsize>,
}

impl LlmProvider for FakeToolLoopProvider {
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
            tool_calls: true,
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
            let call_index = self.calls.fetch_add(1, Ordering::SeqCst);
            let events = if call_index == 0 {
                vec![ProviderStreamEvent::Finished(ProviderFinished::tool_calls(
                    vec![
                        ProviderToolCall::new(
                            "call-1",
                            "read_release",
                            r#"{"path":"CHANGELOG.md"}"#,
                        ),
                        ProviderToolCall::new("call-2", "summarize_diff", r#"{"limit":2}"#),
                    ],
                ))]
            } else {
                assert!(request
                    .messages
                    .iter()
                    .any(|message| message.role == spectacular_llms::ProviderMessageRole::Tool));
                vec![
                    ProviderStreamEvent::MessageDelta(MessageDelta::assistant(
                        "Final response after two tool results.",
                    )),
                    ProviderStreamEvent::Finished(ProviderFinished {
                        finish_reason: FinishReason::Stop,
                        tool_calls: Vec::new(),
                        usage: Some(UsageMetadata {
                            input_tokens: Some(1),
                            output_tokens: Some(1),
                            total_tokens: Some(2),
                        }),
                        reasoning: None,
                    }),
                ]
            };

            let stream: ProviderStream = Box::new(events.into_iter().map(Ok));
            Ok(stream)
        })
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut agent = Agent::new(FakeToolLoopProvider {
        calls: Arc::new(AtomicUsize::new(0)),
    });
    futures::executor::block_on(agent.register_tool(FakeTool {
        name: "read_release",
    }));
    futures::executor::block_on(agent.register_tool(FakeTool {
        name: "summarize_diff",
    }));

    agent.enqueue_prompt("Use tools before answering");
    futures::executor::block_on(agent.run_next())?;

    for (index, event) in agent.events().iter().enumerate() {
        println!("{index}: {event}");
    }

    let final_output = agent
        .events()
        .iter()
        .filter_map(|event| match event {
            AgentEvent::MessageDelta(delta) => Some(delta.content.as_str()),
            _ => None,
        })
        .collect::<String>();
    println!("final output: {final_output}");

    Ok(())
}
