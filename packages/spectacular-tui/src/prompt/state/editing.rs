use crate::metadata::CommandDescriptor;
use crate::prompt::grapheme::{clamp_boundary, grapheme_at, next_boundary, previous_boundary};
use crate::prompt::state::navigation::{next_word_boundary, previous_word_boundary};
use crate::prompt::state::text::{
    current_line_indentation, line_end, line_start, normalize_paste, slash_token_range,
};
use crate::session::PromptState;
use std::ops::Range;
use unicode_segmentation::UnicodeSegmentation;

impl PromptState {
    /// Clears editable prompt content and resets cursor, selection, and paste metadata.
    pub fn clear(&mut self) {
        self.record_history_boundary();
        self.lines = vec![String::new()];
        self.text.clear();
        self.cursor = 0;
        self.preferred_column = None;
        self.selection_anchor = None;
        self.selected_completion = 0;
        self.dismissed_completion = None;
        self.paste_burst.buffer.clear();
        self.scroll_top_row = 0;
        self.redo_stack.clear();
    }

    /// Inserts normalized text at the cursor after replacing any active selection.
    pub fn insert_text(&mut self, value: &str) {
        if value.is_empty() {
            return;
        }

        let normalized = normalize_paste(value);
        let grouped = is_groupable_insert(&normalized);
        self.prepare_edit(grouped);
        self.replace_selection_or_insert(&normalized);
        self.finish_edit();
        if should_close_group_after_insert(&normalized) {
            self.record_history_boundary();
        }
    }

    /// Inserts one line break plus the current line indentation at the cursor.
    pub fn insert_newline(&mut self) {
        let indentation = current_line_indentation(&self.text(), self.cursor);
        self.insert_text(&format!("\n{indentation}"));
        self.record_history_boundary();
    }

    /// Inserts pasted text with CRLF/CR normalization tracked in prompt paste metadata.
    pub fn insert_paste(&mut self, value: &str) {
        let normalized = normalize_paste(value);
        if normalized.is_empty() {
            return;
        }

        self.paste_burst.buffer = normalized.clone();
        self.prepare_edit(false);
        self.replace_selection_or_insert(&normalized);
        self.finish_edit();
        self.record_history_boundary();
    }

    /// Inserts a character, applying auto-close and type-over pair behavior.
    pub fn insert_character(&mut self, character: char) {
        if self.selection_range().is_none() && is_closing_pair(character) {
            let text = self.text();
            if grapheme_at(&text, self.cursor) == Some(character.to_string().as_str()) {
                self.move_right(false);
                return;
            }
        }

        if let Some(closing) = closing_pair(character) {
            self.prepare_edit(false);
            self.replace_selection_or_insert(&format!("{character}{closing}"));
            self.cursor = previous_boundary(&self.text(), self.cursor);
            self.finish_edit();
            self.record_history_boundary();
            return;
        }

        self.insert_text(&character.to_string());
    }

    /// Selects the complete prompt buffer when text is present.
    pub fn select_all(&mut self) {
        let text = self.text();
        if text.is_empty() {
            return;
        }

        self.selection_anchor = Some(0);
        self.cursor = text.len();
        self.preferred_column = None;
        self.selected_completion = 0;
        self.record_history_boundary();
    }

    /// Clears selection first and clears the prompt on a second escape press.
    pub fn escape(&mut self) {
        if self.selection_anchor.is_some() {
            self.selection_anchor = None;
            self.record_history_boundary();
            return;
        }

        self.clear();
    }

    /// Deletes one original word before the cursor or the active selected range.
    pub fn delete_previous_word(&mut self) {
        if self.delete_selection_as_edit() {
            return;
        }

        let text = self.text();
        let previous = previous_word_boundary(&text, self.cursor);
        if previous == self.cursor {
            return;
        }

        self.prepare_edit(false);
        self.replace_range(previous..self.cursor, "");
        self.cursor = previous;
        self.finish_edit();
        self.record_history_boundary();
    }

    /// Deletes one original word after the cursor or the active selected range.
    pub fn delete_next_word(&mut self) {
        if self.delete_selection_as_edit() {
            return;
        }

        let text = self.text();
        let next = next_word_boundary(&text, self.cursor);
        if next == self.cursor {
            return;
        }

        self.prepare_edit(false);
        self.replace_range(self.cursor..next, "");
        self.finish_edit();
        self.record_history_boundary();
    }

    /// Kills text from the cursor to the current line start into the yank buffer.
    pub fn kill_to_line_start(&mut self) {
        if self.delete_selection_to_kill_buffer() {
            self.record_history_boundary();
            return;
        }

        let text = self.text();
        let start = line_start(&text, self.cursor);
        if start == self.cursor {
            return;
        }

        self.prepare_edit(false);
        self.kill_buffer = text[start..self.cursor].to_owned();
        self.replace_range(start..self.cursor, "");
        self.cursor = start;
        self.finish_edit();
        self.record_history_boundary();
    }

    /// Kills text from the cursor to the current line end into the yank buffer.
    pub fn kill_to_line_end(&mut self) {
        if self.delete_selection_to_kill_buffer() {
            self.record_history_boundary();
            return;
        }

        let text = self.text();
        let end = line_end(&text, self.cursor);
        if end == self.cursor {
            return;
        }

        self.prepare_edit(false);
        self.kill_buffer = text[self.cursor..end].to_owned();
        self.replace_range(self.cursor..end, "");
        self.finish_edit();
        self.record_history_boundary();
    }

    /// Inserts the current yank buffer at the cursor after replacing any selection.
    pub fn yank(&mut self) {
        if self.kill_buffer.is_empty() {
            return;
        }

        let value = self.kill_buffer.clone();
        self.insert_paste(&value);
    }

    /// Deletes the active selection and moves the cursor to the selection start.
    pub fn delete_selection(&mut self) -> bool {
        self.delete_selection_as_edit()
    }

    /// Returns selected text without changing prompt-owned kill buffer state.
    pub fn selected_text(&self) -> Option<String> {
        let text = self.text();
        let range = self.selection_range()?;
        Some(text[range].to_owned())
    }

    /// Copies selected text into the prompt kill buffer and returns it.
    pub fn copy_selection(&mut self) -> Option<String> {
        let value = self.selected_text()?;
        self.kill_buffer = value.clone();
        Some(value)
    }

    /// Cuts selected text into the prompt kill buffer and returns it.
    pub fn cut_selection(&mut self) -> Option<String> {
        let text = self.text();
        let range = self.selection_range()?;
        let value = text[range.clone()].to_owned();
        self.prepare_edit(false);
        self.replace_range(range.clone(), "");
        self.cursor = range.start;
        self.kill_buffer = value.clone();
        self.finish_edit();
        self.record_history_boundary();
        Some(value)
    }

    /// Deletes one grapheme before the cursor or the active selected range.
    pub fn backspace(&mut self) {
        if self.delete_selection_as_edit() {
            return;
        }

        let text = self.text();
        let previous = previous_boundary(&text, self.cursor);
        if previous == self.cursor {
            return;
        }

        self.prepare_edit(false);
        self.replace_range(previous..self.cursor, "");
        self.cursor = previous;
        self.finish_edit();
        self.record_history_boundary();
    }

    /// Deletes one grapheme after the cursor or the active selected range.
    pub fn delete_forward(&mut self) {
        if self.delete_selection_as_edit() {
            return;
        }

        let text = self.text();
        let next = next_boundary(&text, self.cursor);
        if next == self.cursor {
            return;
        }

        self.prepare_edit(false);
        self.replace_range(self.cursor..next, "");
        self.finish_edit();
        self.record_history_boundary();
    }

    /// Returns the active selection range in byte offsets when text is selected.
    pub fn selection_range(&self) -> Option<Range<usize>> {
        let anchor = self.selection_anchor?;
        if anchor == self.cursor {
            return None;
        }

        Some(anchor.min(self.cursor)..anchor.max(self.cursor))
    }

    /// Selects the previous visible slash completion with saturating original behavior.
    pub fn select_previous_completion(&mut self) {
        self.selected_completion = self.selected_completion.saturating_sub(1);
    }

    /// Selects the next visible slash completion with saturating original behavior.
    pub fn select_next_completion(&mut self, count: usize) {
        self.selected_completion = (self.selected_completion + 1).min(count.saturating_sub(1));
    }

    /// Replaces prompt text as one undoable edit while preserving the caller-updated cursor.
    pub(crate) fn replace_text_as_edit(&mut self, text: String) {
        self.prepare_edit(false);
        self.set_text(text);
        self.cursor = clamp_boundary(&self.text(), self.cursor);
        self.finish_edit();
        self.record_history_boundary();
    }

    /// Accepts a display-ready slash command suggestion into the leading command token.
    pub fn accept_command_completion(&mut self, command: &CommandDescriptor) {
        let text = self.text();
        let Some(range) = slash_token_range(&text, self.cursor) else {
            return;
        };

        let replacement = format!("/{} ", command.name);
        let cursor = range.start + replacement.len();
        self.prepare_edit(false);
        self.replace_range(range, &replacement);
        self.cursor = cursor;
        self.finish_edit();
        self.record_history_boundary();
    }

    /// Replaces a selection or inserts text at the cursor.
    fn replace_selection_or_insert(&mut self, value: &str) {
        let range = self.selection_range().unwrap_or(self.cursor..self.cursor);
        let start = range.start;
        self.replace_range(range, value);
        self.cursor = start + value.len();
    }

    /// Replaces a byte range in the joined buffer and rebuilds logical lines.
    fn replace_range(&mut self, range: Range<usize>, replacement: &str) {
        let mut text = self.text();
        text.replace_range(range, replacement);
        self.set_text(text);
    }

    /// Deletes the active selection as an undoable edit.
    fn delete_selection_as_edit(&mut self) -> bool {
        let Some(range) = self.selection_range() else {
            self.selection_anchor = None;
            return false;
        };

        self.prepare_edit(false);
        self.replace_range(range.clone(), "");
        self.cursor = range.start;
        self.finish_edit();
        self.record_history_boundary();
        true
    }

    /// Deletes the active selection into the yank buffer and moves the cursor to the start.
    fn delete_selection_to_kill_buffer(&mut self) -> bool {
        let text = self.text();
        let Some(range) = self.selection_range() else {
            self.selection_anchor = None;
            return false;
        };

        self.prepare_edit(false);
        self.kill_buffer = text[range.clone()].to_owned();
        self.replace_range(range.clone(), "");
        self.cursor = range.start;
        self.finish_edit();
        true
    }

    /// Resets transient editing metadata after prompt content changes.
    fn finish_edit(&mut self) {
        self.preferred_column = None;
        self.selection_anchor = None;
        self.selected_completion = 0;
        self.dismissed_completion = None;
        self.scroll_top_row = self
            .scroll_top_row
            .min(self.text().lines().count().saturating_sub(1));
    }
}

/// Returns true when text can continue a grouped character insertion history frame.
fn is_groupable_insert(value: &str) -> bool {
    let mut graphemes = value.graphemes(true);
    let Some(grapheme) = graphemes.next() else {
        return false;
    };
    graphemes.next().is_none() && grapheme.chars().all(|character| !character.is_whitespace())
}

/// Returns true when an insertion should stop grouped undo accumulation.
fn should_close_group_after_insert(value: &str) -> bool {
    value.chars().any(char::is_whitespace)
}

/// Returns the closing pair character for an opening pair.
fn closing_pair(character: char) -> Option<char> {
    match character {
        '(' => Some(')'),
        '{' => Some('}'),
        '[' => Some(']'),
        '"' => Some('"'),
        '\'' => Some('\''),
        '`' => Some('`'),
        _ => None,
    }
}

/// Returns true when the character is a supported auto-close closing delimiter.
fn is_closing_pair(character: char) -> bool {
    matches!(character, ')' | '}' | ']' | '"' | '\'' | '`')
}
