use crate::chat::runner::render_agent_event;
use crate::chat::session::ChatRecord;
use crate::chat::ChatContext;
use spectacular_agent::AgentEvent;
use spectacular_commands::{Command, CommandControl, CommandError, CommandFuture};

pub fn command() -> Command<ChatContext> {
    Command {
        name: "resume",
        usage: "/resume <session-id>",
        summary: "Resume a saved session",
        execute,
    }
}

fn execute<'a>(context: &'a mut ChatContext, args: Vec<String>) -> CommandFuture<'a> {
    Box::pin(async move {
        let [prefix] = args.as_slice() else {
            return Err(CommandError::usage("/resume <session-id>"));
        };
        let records = context
            .session
            .resume(prefix)
            .map_err(|error| CommandError::message(error.to_string()))?;
        context
            .restore_runtime_from_records(&records)
            .map_err(|error| CommandError::message(error.to_string()))?;
        context.renderer.clear_screen();
        context.renderer.resumed(context.session.current_id());
        let mut assistant_buffer = String::new();
        for record in &records {
            if matches!(record, ChatRecord::Corrupt { .. }) {
                flush_assistant(context, &mut assistant_buffer);
                context.renderer.warning(&format!(
                    "unreadable session event at line {}",
                    record.line()
                ));
                continue;
            }
            let Some(event) = record.event() else {
                flush_assistant(context, &mut assistant_buffer);
                let event_type = match record {
                    ChatRecord::Unknown { value, .. } => value
                        .get("type")
                        .and_then(|value| value.as_str())
                        .unwrap_or("unknown"),
                    ChatRecord::Corrupt { .. } | ChatRecord::Known { .. } => "unknown",
                };
                context.renderer.warning(&format!(
                    "unknown session event `{event_type}` at line {}",
                    record.line()
                ));
                continue;
            };
            let Some(event) = event.to_agent_event() else {
                continue;
            };

            if let AgentEvent::MessageDelta(delta) = &event {
                assistant_buffer.push_str(&delta.content);
                continue;
            }

            flush_assistant(context, &mut assistant_buffer);
            render_replay_event(context, &event).await?;
        }
        flush_assistant(context, &mut assistant_buffer);
        Ok(CommandControl::Continue)
    })
}

async fn render_replay_event(
    context: &mut ChatContext,
    event: &AgentEvent,
) -> Result<(), CommandError> {
    render_agent_event(&context.renderer, &context.tools, event)
        .await
        .map_err(|error| CommandError::message(error.to_string()))
}

fn flush_assistant(context: &ChatContext, buffer: &mut String) {
    if buffer.is_empty() {
        return;
    }

    context.renderer.assistant_text(buffer);
    buffer.clear();
}
