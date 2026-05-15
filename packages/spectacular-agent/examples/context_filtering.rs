use spectacular_agent::{provider_messages_from_store, AgentEvent, Store};
use spectacular_llms::{FinishReason, ProviderMessageRole, UsageMetadata};

fn main() {
    let mut store = Store::default();
    store.append(AgentEvent::user_prompt("Summarize the release"));
    store.append(AgentEvent::message_start("message-1"));
    store.append(AgentEvent::message_delta(
        "message-1",
        "Release notes draft.",
    ));
    store.append(AgentEvent::message_finish("message-1"));
    store.append(AgentEvent::reasoning_start("reasoning-1"));
    store.append(AgentEvent::reasoning_delta(
        "reasoning-1",
        "private thought",
    ));
    store.append(AgentEvent::reasoning_finish("reasoning-1"));
    store.append(AgentEvent::UsageMetadata(UsageMetadata {
        input_tokens: Some(1),
        output_tokens: Some(2),
        total_tokens: Some(3),
    }));
    store.append(AgentEvent::Finished {
        finish_reason: FinishReason::Stop,
    });
    store.append(AgentEvent::tool_call_start("call-1", "read", "{}"));
    store.append(AgentEvent::tool_call_finish(
        "call-1",
        "read",
        r#"{"ok":true}"#,
    ));
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
