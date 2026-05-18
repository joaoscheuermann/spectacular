use crate::chat::renderer::{has_visible_assistant_text, Renderer};
use crate::chat::ChatError;
use spectacular_agent::{AgentEvent, ToolStorage};

/// Renders a persisted or streamed agent event without appending it to session storage.
pub async fn render_agent_event(
    renderer: &Renderer,
    tools: &ToolStorage,
    event: &AgentEvent,
) -> Result<(), ChatError> {
    match event {
        AgentEvent::UserPrompt { content, .. } => renderer.user_prompt(content),
        AgentEvent::MessageDelta { content, .. } => {
            if !has_visible_assistant_text(content) {
                return Ok(());
            }

            renderer.assistant_delta(content).await?;
        }
        AgentEvent::ReasoningDelta { content, .. } => renderer.reasoning_text(content),
        AgentEvent::ToolCallStart {
            tool_call_id,
            name,
            arguments,
        } => {
            renderer.clear_working();
            renderer.tool_call(tool_call_id, name, arguments, tools);
            renderer.working();
        }
        AgentEvent::ToolCallFinish {
            tool_call_id,
            name,
            output,
        } => {
            renderer.clear_working();
            renderer.tool_result(tool_call_id, name, output, tools);
            renderer.working();
        }

        AgentEvent::ValidationError { message } | AgentEvent::Error { message } => {
            renderer.clear_working();
            renderer.error(message);
            renderer.working();
        }
        AgentEvent::Cancelled { reason } => renderer.cancelled(reason),
        AgentEvent::Finished { .. }
        | AgentEvent::UsageMetadata(_)
        | AgentEvent::ContextTokenUsage(_)
        | AgentEvent::ReasoningMetadata(_)
        | AgentEvent::Internal { .. }
        | AgentEvent::MessageStart { .. }
        | AgentEvent::MessageFinish { .. }
        | AgentEvent::ReasoningStart { .. }
        | AgentEvent::ReasoningFinish { .. }
        | AgentEvent::ToolCallDelta { .. } => {}
        _ => {}
    }

    Ok(())
}
