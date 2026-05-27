use super::ChatEvent;
use crate::chat::command_event::{
    CommandDelta, CommandEvent, CommandFinished, CommandStart, CommandStatus,
};

impl ChatEvent {
    /// Converts an app-owned command lifecycle event into a persisted chat event.
    pub fn from_command_event(event: &CommandEvent, created_at: String) -> Self {
        match event {
            CommandEvent::Start(start) => from_start(start, created_at),
            CommandEvent::Delta(delta) => from_delta(delta, created_at),
            CommandEvent::Finished(finished) => from_finished(finished, created_at),
        }
    }

    /// Converts a persisted chat event back into an app-owned command lifecycle event.
    pub fn to_command_event(&self) -> Option<CommandEvent> {
        to_start(self)
            .or_else(|| to_delta(self))
            .or_else(|| to_finished(self))
    }
}

fn from_start(start: &CommandStart, created_at: String) -> ChatEvent {
    ChatEvent::CommandStart {
        command_id: start.command_id.clone(),
        source: start.source.clone(),
        name: start.name.clone(),
        title: start.title.clone(),
        command: start.command.clone(),
        working_directory: start.working_directory.clone(),
        created_at,
    }
}

fn from_delta(delta: &CommandDelta, created_at: String) -> ChatEvent {
    ChatEvent::CommandDelta {
        command_id: delta.command_id.clone(),
        channel: delta.channel.clone(),
        content: delta.content.clone(),
        sequence: delta.sequence,
        created_at,
    }
}

fn from_finished(finished: &CommandFinished, created_at: String) -> ChatEvent {
    ChatEvent::CommandFinished {
        command_id: finished.command_id.clone(),
        status: finished.status.as_str().to_owned(),
        summary: finished.summary.clone(),
        created_at,
    }
}

fn to_start(event: &ChatEvent) -> Option<CommandEvent> {
    match event {
        ChatEvent::CommandStart {
            command_id,
            source,
            name,
            title,
            command,
            working_directory,
            ..
        } => Some(CommandEvent::Start(CommandStart {
            command_id: command_id.clone(),
            source: source.clone(),
            name: name.clone(),
            title: title.clone(),
            command: command.clone(),
            working_directory: working_directory.clone(),
        })),
        _ => None,
    }
}

fn to_delta(event: &ChatEvent) -> Option<CommandEvent> {
    match event {
        ChatEvent::CommandDelta {
            command_id,
            channel,
            content,
            sequence,
            ..
        } => Some(CommandEvent::Delta(CommandDelta {
            command_id: command_id.clone(),
            channel: channel.clone(),
            content: content.clone(),
            sequence: *sequence,
        })),
        _ => None,
    }
}

fn to_finished(event: &ChatEvent) -> Option<CommandEvent> {
    match event {
        ChatEvent::CommandFinished {
            command_id,
            status,
            summary,
            ..
        } => Some(CommandEvent::Finished(CommandFinished {
            command_id: command_id.clone(),
            status: CommandStatus::from_str(status),
            summary: summary.clone(),
        })),
        _ => None,
    }
}
