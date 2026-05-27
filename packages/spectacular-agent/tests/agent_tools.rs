mod support;

use spectacular_agent::{Agent, AgentEvent};
use spectacular_llms::{MessageDelta, ProviderFinished, ProviderStreamEvent, ProviderToolCall};
use std::sync::{atomic::AtomicUsize, Arc};
use support::{
    capabilities, finished_stop_with_usage, BuiltInStyleWriteTool, EchoTool, FakeProvider,
    RecordingProvider,
};
#[test]
/// Verifies a tool-call finish executes the tool and records its result.
fn tool_call_loop_stores_tool_result_then_finishes() {
    let provider = FakeProvider {
        calls: Arc::new(AtomicUsize::new(0)),
        capabilities: capabilities(),
        events: vec![
            ProviderStreamEvent::Finished(ProviderFinished::tool_calls(vec![
                ProviderToolCall::new("call-1", "echo", r#"{"ok":true}"#),
            ])),
            ProviderStreamEvent::MessageDelta(MessageDelta::assistant("done")),
            ProviderStreamEvent::Finished(finished_stop_with_usage()),
        ],
    };
    let mut agent = Agent::new(provider);
    agent.register_tool(EchoTool).unwrap();
    agent.enqueue_prompt("prompt");

    futures::executor::block_on(agent.run_next()).unwrap();

    assert!(agent
        .events()
        .iter()
        .any(|event| matches!(event, AgentEvent::ToolCallFinish { .. })));
}

#[test]
/// Verifies tool manifests and concise tool summaries are sent in provider requests.
fn provider_request_includes_tool_manifest_and_tool_summary_system_prompt() {
    let provider = RecordingProvider::new(vec![vec![
        ProviderStreamEvent::MessageDelta(MessageDelta::assistant("done")),
        ProviderStreamEvent::Finished(finished_stop_with_usage()),
    ]]);
    let requests = Arc::clone(&provider.requests);
    let mut agent = Agent::new(provider);
    agent.register_tool(EchoTool).unwrap();
    agent.enqueue_prompt("prompt");

    futures::executor::block_on(agent.run_next()).unwrap();

    let requests = requests.lock().unwrap();
    let request = requests.first().unwrap();
    assert_eq!(request.tools.len(), 1);
    assert_eq!(request.tools[0].name, "echo");
    assert_eq!(
        request.tools[0].description,
        "Echo parsed arguments as provider-visible JSON."
    );
    assert_eq!(
            request.messages[0].content,
            "You have access to the following tools:\n* echo - Echo parsed arguments as provider-visible JSON."
        );
    assert!(!request.messages[0].content.contains("Parameters:"));
    assert!(!request.messages[0].content.contains("additionalProperties"));
}

#[test]
/// Verifies tool-call request and result events preserve provider tool-call identity.
fn tool_call_loop_emits_structured_tool_events_with_matching_id() {
    let provider = RecordingProvider::new(vec![
        vec![ProviderStreamEvent::Finished(ProviderFinished::tool_calls(
            vec![ProviderToolCall::new("call-1", "echo", r#"{"ok":true}"#)],
        ))],
        vec![
            ProviderStreamEvent::MessageDelta(MessageDelta::assistant("done")),
            ProviderStreamEvent::Finished(finished_stop_with_usage()),
        ],
    ]);
    let mut agent = Agent::new(provider);
    agent.register_tool(EchoTool).unwrap();
    agent.enqueue_prompt("prompt");

    futures::executor::block_on(agent.run_next()).unwrap();

    let tool_call = agent.events().into_iter().find_map(|event| match event {
        AgentEvent::ToolCallStart {
            tool_call_id,
            name,
            arguments,
        } => Some((tool_call_id, name, arguments)),
        _ => None,
    });
    let tool_result = agent.events().into_iter().find_map(|event| match event {
        AgentEvent::ToolCallFinish {
            tool_call_id,
            name,
            output,
        } => Some((tool_call_id, name, output)),
        _ => None,
    });

    assert_eq!(
        tool_call,
        Some((
            "call-1".to_owned(),
            "echo".to_owned(),
            r#"{"ok":true}"#.to_owned()
        ))
    );
    assert_eq!(
        tool_result,
        Some((
            "call-1".to_owned(),
            "echo".to_owned(),
            r#"{"ok":true}"#.to_owned()
        ))
    );
}

#[test]
/// Verifies follow-up provider requests replay assistant tool calls and tool results.
fn follow_up_provider_request_replays_assistant_tool_call_and_tool_result() {
    let provider = RecordingProvider::new(vec![
        vec![ProviderStreamEvent::Finished(ProviderFinished::tool_calls(
            vec![ProviderToolCall::new("call-1", "echo", r#"{"ok":true}"#)],
        ))],
        vec![
            ProviderStreamEvent::MessageDelta(MessageDelta::assistant("done")),
            ProviderStreamEvent::Finished(finished_stop_with_usage()),
        ],
    ]);
    let requests = Arc::clone(&provider.requests);
    let mut agent = Agent::new(provider);
    agent.register_tool(EchoTool).unwrap();
    agent.enqueue_prompt("prompt");

    futures::executor::block_on(agent.run_next()).unwrap();

    let requests = requests.lock().unwrap();
    assert_eq!(requests.len(), 2);
    let initial = &requests[0];
    assert_eq!(initial.tools.len(), 1);
    assert_eq!(initial.tools[0].name, "echo");
    let follow_up = &requests[1];
    assert_eq!(follow_up.tools.len(), 1);
    assert_eq!(follow_up.tools[0].name, "echo");

    let assistant_tool_call = follow_up
        .messages
        .iter()
        .find(|message| !message.tool_calls.is_empty())
        .unwrap();
    let tool_result = follow_up
        .messages
        .iter()
        .find(|message| message.tool_call_id.as_deref() == Some("call-1"))
        .unwrap();

    assert_eq!(assistant_tool_call.tool_calls[0].id, "call-1");
    assert_eq!(assistant_tool_call.tool_calls[0].name, "echo");
    assert_eq!(
        assistant_tool_call.tool_calls[0].arguments,
        r#"{"ok":true}"#
    );
    assert_eq!(tool_result.content, r#"{"ok":true}"#);
    assert_eq!(tool_result.tool_call_id.as_deref(), Some("call-1"));
}

#[test]
/// Verifies built-in style tools feed matching tool-result messages back to providers.
fn fake_provider_receives_built_in_style_tool_result_with_matching_id() {
    let provider = RecordingProvider::new(vec![
        vec![ProviderStreamEvent::Finished(ProviderFinished::tool_calls(
            vec![ProviderToolCall::new(
                "call-write-1",
                "write",
                r#"{"path":"foo.txt","content":"hello"}"#,
            )],
        ))],
        vec![
            ProviderStreamEvent::MessageDelta(MessageDelta::assistant("done")),
            ProviderStreamEvent::Finished(finished_stop_with_usage()),
        ],
    ]);
    let requests = Arc::clone(&provider.requests);
    let mut agent = Agent::new(provider);
    agent.register_tool(BuiltInStyleWriteTool).unwrap();
    agent.enqueue_prompt("prompt");

    futures::executor::block_on(agent.run_next()).unwrap();

    let tool_result_event = agent.events().into_iter().find_map(|event| match event {
        AgentEvent::ToolCallFinish {
            tool_call_id,
            name,
            output,
        } => Some((tool_call_id, name, output)),
        _ => None,
    });
    let requests = requests.lock().unwrap();
    let follow_up_tool_result = requests[1]
        .messages
        .iter()
        .find(|message| message.tool_call_id.as_deref() == Some("call-write-1"))
        .unwrap();

    assert_eq!(
        tool_result_event,
        Some((
            "call-write-1".to_owned(),
            "write".to_owned(),
            r#"{"path":"foo.txt","success":true}"#.to_owned()
        ))
    );
    assert_eq!(
        follow_up_tool_result.tool_call_id.as_deref(),
        Some("call-write-1")
    );
    assert_eq!(
        follow_up_tool_result.content,
        r#"{"path":"foo.txt","success":true}"#
    );
}
