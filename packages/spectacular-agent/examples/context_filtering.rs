use spectacular_agent::{provider_messages_from_store, AgentEvent, Store};
use spectacular_llms::{
    FinishReason, MessageDelta, ProviderMessageRole, ReasoningDelta, ReasoningMetadata,
    UsageMetadata,
};

fn main() {
    let mut store = Store::default();
    store.append(AgentEvent::user_prompt("Summarize the release"));
    store.append(AgentEvent::MessageDelta(MessageDelta::assistant(
        "Release notes draft.",
    )));
    store.append(AgentEvent::ReasoningDelta(ReasoningDelta {
        content: "private thought".to_owned(),
        metadata: Some(ReasoningMetadata {
            effort: Some("low".to_owned()),
            summary: Some("private reasoning summary".to_owned()),
        }),
    }));
    store.append(AgentEvent::UsageMetadata(UsageMetadata {
        input_tokens: Some(1),
        output_tokens: Some(2),
        total_tokens: Some(3),
    }));
    store.append(AgentEvent::Finished {
        finish_reason: FinishReason::Stop,
    });
    store.append(AgentEvent::assistant_tool_call_request(
        "call-1", "read", "{}",
    ));
    store.append(AgentEvent::tool_result("call-1", "read", r#"{"ok":true}"#));
    store.append(AgentEvent::cancelled("stale queued run"));
    store.append(AgentEvent::error("provider timeout stored for diagnostics"));
    store.append(AgentEvent::internal("queue bookkeeping"));

    let messages =
        provider_messages_from_store("You are Spectacular, a focused coding assistant.", &store);

    for message in messages {
        let role = match message.role {
            ProviderMessageRole::System => "system",
            ProviderMessageRole::User => "user",
            ProviderMessageRole::Assistant => "assistant",
            ProviderMessageRole::Tool => "tool",
        };
        println!("{role}: {}", message.content);
    }
}
