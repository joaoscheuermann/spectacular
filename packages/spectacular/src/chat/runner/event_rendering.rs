use crate::chat::renderer::{has_visible_assistant_text, Renderer};
use crate::chat::ChatError;
use spectacular_agent::{AgentEvent, ToolStorage};
use spectacular_llms::ProviderMessageRole;

/// Renders a persisted or streamed agent event without appending it to session storage.
pub async fn render_agent_event(
    renderer: &Renderer,
    tools: &ToolStorage,
    event: &AgentEvent,
) -> Result<(), ChatError> {
    match event {
        AgentEvent::UserPrompt { content } => renderer.user_prompt(content),
        AgentEvent::MessageDelta(delta) if delta.role == ProviderMessageRole::Assistant => {
            if !has_visible_assistant_text(&delta.content) {
                return Ok(());
            }

            renderer.assistant_delta(&delta.content).await?;
        }
        AgentEvent::ReasoningDelta(delta) => renderer.reasoning_text(&delta.content),
        AgentEvent::AssistantToolCallRequest {
            tool_call_id,
            name,
            arguments,
        } => {
            renderer.clear_working();
            renderer.tool_call(tool_call_id, name, arguments, tools);
            renderer.working();
        }
        AgentEvent::ToolResult {
            tool_call_id,
            name,
            content,
        } => {
            renderer.clear_working();
            renderer.tool_result(tool_call_id, name, content, tools);
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
        | AgentEvent::ReasoningMetadata(_)
        | AgentEvent::Internal { .. } => {}
        AgentEvent::MessageDelta(_) => {}
        _ => {}
    }

    Ok(())
}
