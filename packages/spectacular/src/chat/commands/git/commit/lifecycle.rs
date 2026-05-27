use crate::chat::command_event::{
    CommandDelta, CommandEvent, CommandFinished, CommandStart, CommandStatus,
};
use crate::chat::commands::ChatCommandContext;

use std::time::{SystemTime, UNIX_EPOCH};

use super::{
    bounded_text, COMMAND_DELTA_TRUNCATED_NOTICE, MAX_COMMAND_DELTA_BYTES,
    MAX_COMMAND_DELTA_CONTENT_CHARS, MAX_COMMAND_DELTA_EVENTS, MAX_COMMAND_SUMMARY_CHARS,
    MAX_COMMAND_TEXT_CHARS,
};

pub(super) struct CommitLifecycle<'a, 'context> {
    context: &'a ChatCommandContext<'context>,
    command_id: String,
    sequence: u64,
    persisted_delta_bytes: usize,
    persisted_delta_events: usize,
    delta_truncated: bool,
}

impl<'a, 'context> CommitLifecycle<'a, 'context> {
    pub(super) fn new(context: &'a ChatCommandContext<'context>) -> Self {
        Self {
            context,
            command_id: command_id(),
            sequence: 0,
            persisted_delta_bytes: 0,
            persisted_delta_events: 0,
            delta_truncated: false,
        }
    }

    pub(super) fn start(&self) -> Result<(), String> {
        let command = bounded_text("/git commit", MAX_COMMAND_TEXT_CHARS);
        self.context
            .append_command_event(&CommandEvent::Start(CommandStart {
                command_id: self.command_id.clone(),
                source: "slash_command".to_owned(),
                name: "/git commit".to_owned(),
                title: "Git commit".to_owned(),
                command: command.clone(),
                working_directory: working_directory(),
            }))
            .map_err(|error| error.to_string())?;
        self.context.command_start("Git commit", &command);
        Ok(())
    }

    pub(super) fn delta(&mut self, content: &str) -> Result<(), String> {
        if self.delta_truncated {
            return Ok(());
        }

        let content = bounded_text(content, MAX_COMMAND_DELTA_CONTENT_CHARS);
        if self.should_append_truncation_notice(content.len()) {
            return self.append_delta_truncation_notice();
        }

        self.append_delta_record(content)
    }

    pub(super) fn finish(
        &self,
        status: CommandStatus,
        summary: impl AsRef<str>,
    ) -> Result<(), String> {
        let summary = bounded_text(summary.as_ref(), MAX_COMMAND_SUMMARY_CHARS);
        self.context
            .append_command_event(&CommandEvent::Finished(CommandFinished {
                command_id: self.command_id.clone(),
                status,
                summary: summary.clone(),
            }))
            .map_err(|error| error.to_string())?;
        self.context.command_finished(status, &summary);
        Ok(())
    }

    fn should_append_truncation_notice(&self, next_delta_bytes: usize) -> bool {
        if self.persisted_delta_events.saturating_add(1) >= MAX_COMMAND_DELTA_EVENTS {
            return true;
        }

        self.persisted_delta_bytes
            .saturating_add(next_delta_bytes)
            .saturating_add(COMMAND_DELTA_TRUNCATED_NOTICE.len())
            > MAX_COMMAND_DELTA_BYTES
    }

    fn append_delta_truncation_notice(&mut self) -> Result<(), String> {
        self.delta_truncated = true;
        if self.persisted_delta_events >= MAX_COMMAND_DELTA_EVENTS {
            return Ok(());
        }

        let remaining_bytes = MAX_COMMAND_DELTA_BYTES.saturating_sub(self.persisted_delta_bytes);
        if remaining_bytes == 0 {
            return Ok(());
        }

        self.append_delta_record(bounded_text(
            COMMAND_DELTA_TRUNCATED_NOTICE,
            remaining_bytes.min(MAX_COMMAND_DELTA_CONTENT_CHARS),
        ))
    }

    fn append_delta_record(&mut self, content: String) -> Result<(), String> {
        let bytes = content.len();
        self.sequence += 1;
        self.context
            .append_command_event(&CommandEvent::Delta(CommandDelta {
                command_id: self.command_id.clone(),
                channel: "status".to_owned(),
                content: content.clone(),
                sequence: self.sequence,
            }))
            .map_err(|error| error.to_string())?;
        self.context.command_delta(&content);
        self.persisted_delta_bytes += bytes;
        self.persisted_delta_events += 1;
        Ok(())
    }
}

fn command_id() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();
    format!("git-commit-{nanos}")
}

fn working_directory() -> Option<String> {
    std::env::current_dir()
        .ok()
        .map(|path| path.display().to_string())
}
