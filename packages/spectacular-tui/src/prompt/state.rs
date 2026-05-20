use crate::metadata::CommandDescriptor;
use crate::session::{PromptPasteBurstState, PromptState};
use std::ops::Range;

/// Editable prompt behavior owned by the reducer rather than terminal input code.
impl PromptState {
    /// Creates an empty prompt with the cursor at the start of the buffer.
    pub fn empty() -> Self {
        Self::default()
    }

    /// Creates a prompt from text with the cursor placed at the end.
    pub fn from_text(value: impl Into<String>) -> Self {
        let text = value.into();
        Self {
            cursor: text.len(),
            text,
            preferred_column: None,
            selection_anchor: None,
            selected_completion: 0,
            kill_buffer: String::new(),
            paste_burst: PromptPasteBurstState::default(),
        }
    }

    /// Clears editable prompt content and resets cursor, selection, and paste metadata.
    pub fn clear(&mut self) {
        self.text.clear();
        self.cursor = 0;
        self.preferred_column = None;
        self.selection_anchor = None;
        self.selected_completion = 0;
        self.paste_burst.buffer.clear();
    }

    /// Inserts normalized text at the cursor after replacing any active selection.
    pub fn insert_text(&mut self, value: &str) {
        if value.is_empty() {
            return;
        }

        self.delete_selection();
        self.cursor = clamp_boundary(&self.text, self.cursor);
        self.text.insert_str(self.cursor, value);
        self.cursor += value.len();
        self.after_edit();
    }

    /// Inserts one line break at the cursor after replacing any active selection.
    pub fn insert_newline(&mut self) {
        self.insert_text("\n");
    }

    /// Inserts pasted text with CRLF/CR normalization tracked in prompt paste metadata.
    pub fn insert_paste(&mut self, value: &str) {
        let normalized = normalize_paste(value);
        self.paste_burst.buffer = normalized.clone();
        self.insert_text(&normalized);
    }

    /// Moves the cursor one character left and optionally extends selection state.
    pub fn move_left(&mut self, selecting: bool) {
        let cursor = previous_boundary(&self.text, self.cursor);
        self.move_to(cursor, selecting);
    }

    /// Moves the cursor one character right and optionally extends selection state.
    pub fn move_right(&mut self, selecting: bool) {
        let cursor = next_boundary(&self.text, self.cursor);
        self.move_to(cursor, selecting);
    }

    /// Moves the cursor one original word boundary left and optionally selects text.
    pub fn move_word_left(&mut self, selecting: bool) {
        let cursor = previous_navigation_word_boundary(&self.text, self.cursor);
        self.move_to(cursor, selecting);
    }

    /// Moves the cursor one original word boundary right and optionally selects text.
    pub fn move_word_right(&mut self, selecting: bool) {
        let cursor = next_word_boundary(&self.text, self.cursor);
        self.move_to(cursor, selecting);
    }

    /// Moves the cursor to the current visual line start and optionally selects text.
    pub fn move_line_start(&mut self, selecting: bool) {
        self.move_to(line_start(&self.text, self.cursor), selecting);
    }

    /// Moves the cursor to the current visual line end and optionally selects text.
    pub fn move_line_end(&mut self, selecting: bool) {
        self.move_to(line_end(&self.text, self.cursor), selecting);
    }

    /// Moves the cursor to the prompt start and optionally extends selection state.
    pub fn move_to_start(&mut self, selecting: bool) {
        self.move_to(0, selecting);
    }

    /// Moves the cursor to the prompt end and optionally extends selection state.
    pub fn move_to_end(&mut self, selecting: bool) {
        self.move_to(self.text.len(), selecting);
    }

    /// Moves the cursor to the same character column on the previous prompt line.
    pub fn move_up(&mut self, selecting: bool) {
        self.move_vertical(-1, selecting);
    }

    /// Moves the cursor to the same character column on the next prompt line.
    pub fn move_down(&mut self, selecting: bool) {
        self.move_vertical(1, selecting);
    }

    /// Selects the complete prompt buffer when text is present.
    pub fn select_all(&mut self) {
        if self.text.is_empty() {
            return;
        }

        self.selection_anchor = Some(0);
        self.cursor = self.text.len();
        self.preferred_column = None;
        self.selected_completion = 0;
    }

    /// Clears selection first and clears the prompt on a second escape press.
    pub fn escape(&mut self) {
        if self.selection_anchor.is_some() {
            self.selection_anchor = None;
            return;
        }

        self.clear();
    }

    /// Deletes one original word before the cursor or the active selected range.
    pub fn delete_previous_word(&mut self) {
        if self.delete_selection() {
            self.after_edit();
            return;
        }

        let previous = previous_word_boundary(&self.text, self.cursor);
        if previous == self.cursor {
            return;
        }

        self.text.replace_range(previous..self.cursor, "");
        self.cursor = previous;
        self.after_edit();
    }

    /// Deletes one original word after the cursor or the active selected range.
    pub fn delete_next_word(&mut self) {
        if self.delete_selection() {
            self.after_edit();
            return;
        }

        let next = next_word_boundary(&self.text, self.cursor);
        if next == self.cursor {
            return;
        }

        self.text.replace_range(self.cursor..next, "");
        self.after_edit();
    }

    /// Kills text from the cursor to the current line start into the yank buffer.
    pub fn kill_to_line_start(&mut self) {
        if self.delete_selection_to_kill_buffer() {
            self.after_edit();
            return;
        }

        let start = line_start(&self.text, self.cursor);
        if start == self.cursor {
            return;
        }

        self.kill_buffer = self.text[start..self.cursor].to_owned();
        self.text.replace_range(start..self.cursor, "");
        self.cursor = start;
        self.after_edit();
    }

    /// Kills text from the cursor to the current line end into the yank buffer.
    pub fn kill_to_line_end(&mut self) {
        if self.delete_selection_to_kill_buffer() {
            self.after_edit();
            return;
        }

        let end = line_end(&self.text, self.cursor);
        if end == self.cursor {
            return;
        }

        self.kill_buffer = self.text[self.cursor..end].to_owned();
        self.text.replace_range(self.cursor..end, "");
        self.after_edit();
    }

    /// Inserts the current yank buffer at the cursor after replacing any selection.
    pub fn yank(&mut self) {
        if self.kill_buffer.is_empty() {
            return;
        }

        let value = self.kill_buffer.clone();
        self.insert_text(&value);
    }

    /// Deletes the active selection and moves the cursor to the selection start.
    pub fn delete_selection(&mut self) -> bool {
        let Some(range) = self.selection_range() else {
            self.selection_anchor = None;
            return false;
        };

        self.text.replace_range(range.clone(), "");
        self.cursor = range.start;
        self.selection_anchor = None;
        true
    }

    /// Deletes the active selection into the yank buffer and moves the cursor to the start.
    fn delete_selection_to_kill_buffer(&mut self) -> bool {
        let Some(range) = self.selection_range() else {
            self.selection_anchor = None;
            return false;
        };

        self.kill_buffer = self.text[range.clone()].to_owned();
        self.text.replace_range(range.clone(), "");
        self.cursor = range.start;
        self.selection_anchor = None;
        true
    }

    /// Deletes one character before the cursor or the active selected range.
    pub fn backspace(&mut self) {
        if self.delete_selection() {
            self.after_edit();
            return;
        }

        let previous = previous_boundary(&self.text, self.cursor);
        if previous == self.cursor {
            return;
        }

        self.text.replace_range(previous..self.cursor, "");
        self.cursor = previous;
        self.after_edit();
    }

    /// Deletes one character after the cursor or the active selected range.
    pub fn delete_forward(&mut self) {
        if self.delete_selection() {
            self.after_edit();
            return;
        }

        let next = next_boundary(&self.text, self.cursor);
        if next == self.cursor {
            return;
        }

        self.text.replace_range(self.cursor..next, "");
        self.after_edit();
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

    /// Accepts a display-ready slash command suggestion into the leading command token.
    pub fn accept_command_completion(&mut self, command: &CommandDescriptor) {
        let Some(range) = slash_token_range(&self.text, self.cursor) else {
            return;
        };

        let replacement = format!("{} ", command.name);
        self.text.replace_range(range, &replacement);
        self.cursor = 1 + replacement.len();
        self.preferred_column = None;
        self.selection_anchor = None;
    }

    /// Moves the cursor to a byte offset while preserving character-boundary safety.
    fn move_to(&mut self, cursor: usize, selecting: bool) {
        self.preferred_column = None;
        self.move_to_preserving_preferred_column(cursor, selecting);
    }

    /// Moves the cursor vertically while keeping the original character column when possible.
    fn move_vertical(&mut self, delta: i32, selecting: bool) {
        let cursor = clamp_boundary(&self.text, self.cursor);
        let current_start = line_start(&self.text, cursor);
        let current_end = line_end(&self.text, cursor);
        let column = self
            .preferred_column
            .unwrap_or_else(|| character_column(&self.text, current_start, cursor));
        let target = if delta < 0 {
            previous_line_target(&self.text, current_start, column)
        } else {
            next_line_target(&self.text, current_end, column)
        };

        self.preferred_column = Some(column);
        self.move_to_preserving_preferred_column(target, selecting);
    }

    /// Moves the cursor without clearing vertical movement column tracking.
    fn move_to_preserving_preferred_column(&mut self, cursor: usize, selecting: bool) {
        let previous_cursor = self.cursor;
        self.cursor = clamp_boundary(&self.text, cursor);
        if selecting {
            self.selection_anchor.get_or_insert(previous_cursor);
            if self.selection_anchor == Some(self.cursor) {
                self.selection_anchor = None;
            }
            return;
        }

        self.selection_anchor = None;
    }

    /// Resets transient editing metadata after prompt content changes.
    fn after_edit(&mut self) {
        self.preferred_column = None;
        self.selection_anchor = None;
        self.selected_completion = 0;
    }
}

/// Returns the display-ready slash command suggestions for the current prompt state.
pub fn slash_suggestions<'a>(
    prompt: &PromptState,
    commands: &'a [CommandDescriptor],
) -> Vec<&'a CommandDescriptor> {
    let Some(query) = slash_command_query(&prompt.text, prompt.cursor) else {
        return Vec::new();
    };

    commands
        .iter()
        .filter(|command| command.name.starts_with(query))
        .collect()
}

/// Extracts a slash-command query from the first prompt token at the cursor.
pub fn slash_command_query(text: &str, cursor: usize) -> Option<&str> {
    if !text.starts_with('/') || line_start(text, clamp_boundary(text, cursor)) != 0 {
        return None;
    }

    let cursor = clamp_boundary(text, cursor);
    let token_end = text[cursor..]
        .find(char::is_whitespace)
        .map(|index| cursor + index)
        .unwrap_or(text.len());
    if text[..token_end].contains(char::is_whitespace) {
        return None;
    }

    Some(&text[1..cursor])
}

/// Normalizes terminal paste content to LF-only line breaks.
fn normalize_paste(value: &str) -> String {
    value.replace("\r\n", "\n").replace('\r', "\n")
}

/// Returns the leading slash-token range excluding the slash marker.
fn slash_token_range(text: &str, cursor: usize) -> Option<Range<usize>> {
    slash_command_query(text, cursor)?;
    let cursor = clamp_boundary(text, cursor);
    let end = text[cursor..]
        .find(char::is_whitespace)
        .map(|index| cursor + index)
        .unwrap_or(text.len());
    Some(1..end)
}

/// Returns the byte offset for the beginning of the line containing the cursor.
fn line_start(value: &str, cursor: usize) -> usize {
    value[..clamp_boundary(value, cursor)]
        .rfind('\n')
        .map(|index| index + 1)
        .unwrap_or(0)
}

/// Returns the byte offset for the end of the line containing the cursor.
fn line_end(value: &str, cursor: usize) -> usize {
    let cursor = clamp_boundary(value, cursor);
    value[cursor..]
        .find('\n')
        .map(|index| cursor + index)
        .unwrap_or(value.len())
}

/// Returns the character column between the line start and cursor byte offsets.
fn character_column(value: &str, line_start: usize, cursor: usize) -> usize {
    value[line_start..cursor].chars().count()
}

/// Returns the cursor byte offset for the previous line at the requested character column.
fn previous_line_target(value: &str, current_start: usize, column: usize) -> usize {
    if current_start == 0 {
        return 0;
    }

    let previous_end = current_start.saturating_sub(1);
    let previous_start = line_start(value, previous_end);
    offset_for_column(value, previous_start, previous_end, column)
}

/// Returns the cursor byte offset for the next line at the requested character column.
fn next_line_target(value: &str, current_end: usize, column: usize) -> usize {
    if current_end >= value.len() {
        return value.len();
    }

    let next_start = current_end + 1;
    let next_end = line_end(value, next_start);
    offset_for_column(value, next_start, next_end, column)
}

/// Returns the byte offset at a character column within a line range.
fn offset_for_column(value: &str, start: usize, end: usize, column: usize) -> usize {
    value[start..end]
        .char_indices()
        .nth(column)
        .map(|(index, _)| start + index)
        .unwrap_or(end)
}

/// Returns the previous word-navigation boundary from the supplied cursor.
fn previous_navigation_word_boundary(value: &str, cursor: usize) -> usize {
    let mut cursor = clamp_boundary(value, cursor);
    cursor = move_while_previous(value, cursor, char::is_whitespace);
    cursor = move_while_previous(value, cursor, |character| {
        word_category(character) == WordCategory::Separator
    });
    move_while_previous(value, cursor, |character| {
        word_category(character) == WordCategory::Word
    })
}

/// Returns the previous original deletion word boundary from the supplied cursor.
fn previous_word_boundary(value: &str, cursor: usize) -> usize {
    let mut cursor = clamp_boundary(value, cursor);
    cursor = move_while_previous(value, cursor, char::is_whitespace);
    let Some(category) = previous_character(value, cursor).map(word_category) else {
        return cursor;
    };

    move_while_previous(value, cursor, |character| {
        word_category(character) == category
    })
}

/// Returns the next original word boundary from the supplied cursor.
fn next_word_boundary(value: &str, cursor: usize) -> usize {
    let mut cursor = clamp_boundary(value, cursor);
    cursor = move_while_next(value, cursor, char::is_whitespace);
    let Some(category) = next_character(value, cursor).map(word_category) else {
        return cursor;
    };

    move_while_next(value, cursor, |character| {
        word_category(character) == category
    })
}

/// Moves left while the previous character matches a predicate.
fn move_while_previous(value: &str, mut cursor: usize, predicate: impl Fn(char) -> bool) -> usize {
    while let Some(character) = previous_character(value, cursor) {
        if !predicate(character) {
            break;
        }
        cursor = previous_boundary(value, cursor);
    }
    cursor
}

/// Moves right while the next character matches a predicate.
fn move_while_next(value: &str, mut cursor: usize, predicate: impl Fn(char) -> bool) -> usize {
    while let Some(character) = next_character(value, cursor) {
        if !predicate(character) {
            break;
        }
        cursor = next_boundary(value, cursor);
    }
    cursor
}

/// Returns the previous Unicode scalar value before the cursor.
fn previous_character(value: &str, cursor: usize) -> Option<char> {
    if cursor == 0 {
        return None;
    }

    value[..cursor].chars().next_back()
}

/// Returns the next Unicode scalar value at the cursor.
fn next_character(value: &str, cursor: usize) -> Option<char> {
    if cursor >= value.len() {
        return None;
    }

    value[cursor..].chars().next()
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum WordCategory {
    Word,
    Separator,
}

/// Classifies a character using the original editor word grouping.
fn word_category(character: char) -> WordCategory {
    if character.is_alphanumeric() || character == '_' {
        return WordCategory::Word;
    }

    WordCategory::Separator
}

/// Returns the nearest valid character boundary at or before an offset.
fn clamp_boundary(value: &str, offset: usize) -> usize {
    let mut cursor = offset.min(value.len());
    while cursor > 0 && !value.is_char_boundary(cursor) {
        cursor -= 1;
    }
    cursor
}

/// Returns the previous character boundary from the supplied cursor.
fn previous_boundary(value: &str, cursor: usize) -> usize {
    let cursor = clamp_boundary(value, cursor);
    if cursor == 0 {
        return 0;
    }

    value[..cursor]
        .char_indices()
        .last()
        .map(|(index, _)| index)
        .unwrap_or(0)
}

/// Returns the next character boundary from the supplied cursor.
fn next_boundary(value: &str, cursor: usize) -> usize {
    let cursor = clamp_boundary(value, cursor);
    if cursor >= value.len() {
        return value.len();
    }

    value[cursor..]
        .char_indices()
        .nth(1)
        .map(|(index, _)| cursor + index)
        .unwrap_or(value.len())
}
