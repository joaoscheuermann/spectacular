use crate::metadata::CommandDescriptor;
use crate::prompt::grapheme::{clamp_boundary, grapheme_at, next_boundary, previous_boundary};
use crate::prompt::layout::{cursor_position, offset_for_column, visual_rows};
use crate::session::{PromptHistoryEntry, PromptState};
use std::ops::Range;
use unicode_segmentation::UnicodeSegmentation;

const DEFAULT_VERTICAL_WIDTH: usize = 120;

/// Editable prompt behavior owned by the reducer rather than terminal input code.
impl PromptState {
    /// Creates an empty prompt with the cursor at the start of the buffer.
    pub fn empty() -> Self {
        Self::default()
    }

    /// Creates a prompt from text with the cursor placed at the end.
    pub fn from_text(value: impl Into<String>) -> Self {
        let text = normalize_paste(&value.into());
        let cursor = text.len();
        Self {
            lines: split_logical_lines(&text),
            text,
            cursor,
            ..Self::default()
        }
    }

    /// Returns the complete prompt buffer by joining logical lines with line feeds.
    pub fn text(&self) -> String {
        self.lines.join("\n")
    }

    /// Returns whether the prompt has no user-authored content.
    pub fn is_empty(&self) -> bool {
        self.lines.len() == 1 && self.lines.first().is_none_or(String::is_empty)
    }

    /// Clears editable prompt content and resets cursor, selection, and paste metadata.
    pub fn clear(&mut self) {
        self.record_history_boundary();
        self.lines = vec![String::new()];
        self.text.clear();
        self.cursor = 0;
        self.preferred_column = None;
        self.selection_anchor = None;
        self.selected_completion = 0;
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

    /// Moves the cursor one grapheme left and optionally extends selection state.
    pub fn move_left(&mut self, selecting: bool) {
        let cursor = previous_boundary(&self.text(), self.cursor);
        self.move_to(cursor, selecting);
    }

    /// Moves the cursor one grapheme right and optionally extends selection state.
    pub fn move_right(&mut self, selecting: bool) {
        let cursor = next_boundary(&self.text(), self.cursor);
        self.move_to(cursor, selecting);
    }

    /// Moves the cursor one original word boundary left and optionally selects text.
    pub fn move_word_left(&mut self, selecting: bool) {
        let text = self.text();
        let cursor = previous_navigation_word_boundary(&text, self.cursor);
        self.move_to(cursor, selecting);
    }

    /// Moves the cursor one original word boundary right and optionally selects text.
    pub fn move_word_right(&mut self, selecting: bool) {
        let text = self.text();
        let cursor = next_word_boundary(&text, self.cursor);
        self.move_to(cursor, selecting);
    }

    /// Moves the cursor to the current logical line start and optionally selects text.
    pub fn move_line_start(&mut self, selecting: bool) {
        let text = self.text();
        self.move_to(line_start(&text, self.cursor), selecting);
    }

    /// Moves the cursor to the current logical line end and optionally selects text.
    pub fn move_line_end(&mut self, selecting: bool) {
        let text = self.text();
        self.move_to(line_end(&text, self.cursor), selecting);
    }

    /// Moves the cursor to the prompt start and optionally extends selection state.
    pub fn move_to_start(&mut self, selecting: bool) {
        self.move_to(0, selecting);
    }

    /// Moves the cursor to the prompt end and optionally extends selection state.
    pub fn move_to_end(&mut self, selecting: bool) {
        self.move_to(self.text().len(), selecting);
    }

    /// Moves the cursor to the same visual display column on the previous wrapped row.
    pub fn move_up(&mut self, selecting: bool) {
        self.move_vertical(-1, selecting, DEFAULT_VERTICAL_WIDTH);
    }

    /// Moves the cursor to the same visual display column on the next wrapped row.
    pub fn move_down(&mut self, selecting: bool) {
        self.move_vertical(1, selecting, DEFAULT_VERTICAL_WIDTH);
    }

    /// Moves the cursor vertically using the caller's current prompt content width.
    pub fn move_vertical_with_width(&mut self, delta: i32, selecting: bool, content_width: usize) {
        self.move_vertical(delta, selecting, content_width);
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

    /// Restores the previous prompt content snapshot when available.
    pub fn undo(&mut self) {
        self.record_history_boundary();
        let Some(entry) = self.undo_stack.pop() else {
            return;
        };

        self.redo_stack.push(self.history_entry());
        self.restore_history_entry(entry);
    }

    /// Restores the next prompt content snapshot after an undo.
    pub fn redo(&mut self) {
        self.record_history_boundary();
        let Some(entry) = self.redo_stack.pop() else {
            return;
        };

        self.undo_stack.push(self.history_entry());
        self.restore_history_entry(entry);
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

    /// Keeps the cursor visible inside a vertical prompt viewport.
    pub fn ensure_cursor_visible(&mut self, content_width: usize, viewport_height: usize) {
        let text = self.text();
        let rows = visual_rows(&text, content_width);
        let cursor_row = cursor_position(&text, self.cursor, content_width, &rows).row;
        let viewport_height = viewport_height.max(1);
        if cursor_row < self.scroll_top_row {
            self.scroll_top_row = cursor_row;
            return;
        }

        if cursor_row >= self.scroll_top_row.saturating_add(viewport_height) {
            self.scroll_top_row = cursor_row.saturating_add(1).saturating_sub(viewport_height);
        }
    }

    /// Moves the cursor to a byte offset while preserving grapheme-boundary safety.
    fn move_to(&mut self, cursor: usize, selecting: bool) {
        self.record_history_boundary();
        self.preferred_column = None;
        self.move_to_preserving_preferred_column(cursor, selecting);
    }

    /// Moves the cursor vertically while keeping the original visual display column when possible.
    fn move_vertical(&mut self, delta: i32, selecting: bool, content_width: usize) {
        self.record_history_boundary();
        let text = self.text();
        let rows = visual_rows(&text, content_width);
        let position = cursor_position(&text, self.cursor, content_width, &rows);
        let column = self.preferred_column.unwrap_or(position.column);
        let target_row = if delta < 0 {
            position.row.saturating_sub(1)
        } else {
            (position.row + 1).min(rows.len().saturating_sub(1))
        };
        let target = rows
            .get(target_row)
            .map(|row| offset_for_column(&text, row, column))
            .unwrap_or(self.cursor);

        self.preferred_column = Some(column);
        self.move_to_preserving_preferred_column(target, selecting);
    }

    /// Moves the cursor without clearing vertical movement column tracking.
    fn move_to_preserving_preferred_column(&mut self, cursor: usize, selecting: bool) {
        let text = self.text();
        let previous_cursor = self.cursor;
        self.cursor = clamp_boundary(&text, cursor);
        if selecting {
            self.selection_anchor.get_or_insert(previous_cursor);
            if self.selection_anchor == Some(self.cursor) {
                self.selection_anchor = None;
            }
            return;
        }

        self.selection_anchor = None;
    }

    /// Captures undo state before an edit, grouping adjacent plain character insertions.
    fn prepare_edit(&mut self, grouped: bool) {
        if grouped {
            if self.history_group.is_none() {
                self.history_group = Some(self.history_entry());
            }
        } else {
            self.record_history_boundary();
            self.undo_stack.push(self.history_entry());
        }
        self.redo_stack.clear();
    }

    /// Commits active grouped insertion history to the undo stack.
    fn record_history_boundary(&mut self) {
        if let Some(entry) = self.history_group.take() {
            self.undo_stack.push(entry);
        }
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
        self.scroll_top_row = self
            .scroll_top_row
            .min(self.text().lines().count().saturating_sub(1));
    }

    /// Captures the content and cursor state used by history stacks.
    fn history_entry(&self) -> PromptHistoryEntry {
        PromptHistoryEntry {
            lines: self.lines.clone(),
            cursor: self.cursor,
        }
    }

    /// Restores a history snapshot and clears transient selection/navigation state.
    fn restore_history_entry(&mut self, entry: PromptHistoryEntry) {
        let lines = if entry.lines.is_empty() {
            vec![String::new()]
        } else {
            entry.lines
        };
        self.set_lines(lines);
        self.cursor = clamp_boundary(&self.text(), entry.cursor);
        self.preferred_column = None;
        self.selection_anchor = None;
        self.selected_completion = 0;
        self.scroll_top_row = 0;
    }

    /// Replaces prompt text and keeps legacy public text storage synchronized.
    fn set_text(&mut self, text: String) {
        self.lines = split_logical_lines(&text);
        self.text = text;
    }

    /// Replaces prompt logical lines and keeps legacy public text storage synchronized.
    fn set_lines(&mut self, lines: Vec<String>) {
        self.lines = if lines.is_empty() {
            vec![String::new()]
        } else {
            lines
        };
        self.text = self.lines.join("\n");
    }
}

/// Returns the display-ready slash command suggestions for the current prompt state.
pub fn slash_suggestions<'a>(
    prompt: &PromptState,
    commands: &'a [CommandDescriptor],
) -> Vec<&'a CommandDescriptor> {
    let text = prompt.text();
    let Some(query) = slash_command_query(&text, prompt.cursor) else {
        return Vec::new();
    };

    commands
        .iter()
        .filter(|command| command.name.starts_with(query))
        .collect()
}

/// Returns the current slash command query when the cursor is in the leading token.
pub fn slash_command_query(text: &str, cursor: usize) -> Option<&str> {
    let range = slash_token_range(text, cursor)?;
    text.get(range)?.strip_prefix('/')
}

/// Normalizes pasted text to the prompt's internal newline representation.
pub fn normalize_paste(value: &str) -> String {
    value.replace("\r\n", "\n").replace('\r', "\n")
}

/// Splits text into logical lines while retaining a trailing empty line.
fn split_logical_lines(text: &str) -> Vec<String> {
    let lines: Vec<String> = text.split('\n').map(ToOwned::to_owned).collect();
    if lines.is_empty() {
        return vec![String::new()];
    }

    lines
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

/// Returns leading spaces and tabs from the current logical line.
fn current_line_indentation(text: &str, cursor: usize) -> String {
    let start = line_start(text, cursor);
    text[start..]
        .chars()
        .take_while(|character| matches!(character, ' ' | '\t'))
        .collect()
}

/// Returns the byte range for the leading slash token when cursor is inside it.
fn slash_token_range(text: &str, cursor: usize) -> Option<Range<usize>> {
    let trimmed_start = text.len() - text.trim_start().len();
    if !text[trimmed_start..].starts_with('/') {
        return None;
    }

    let token_start = trimmed_start;
    let token_end = text[token_start..]
        .find(char::is_whitespace)
        .map(|index| token_start + index)
        .unwrap_or(text.len());
    if cursor < token_start || cursor > token_end {
        return None;
    }

    Some(token_start..token_end)
}

/// Returns the start offset for the line containing cursor.
fn line_start(value: &str, cursor: usize) -> usize {
    let cursor = clamp_boundary(value, cursor);
    value[..cursor]
        .rfind('\n')
        .map(|index| index + 1)
        .unwrap_or(0)
}

/// Returns the end offset for the line containing cursor.
fn line_end(value: &str, cursor: usize) -> usize {
    let cursor = clamp_boundary(value, cursor);
    value[cursor..]
        .find('\n')
        .map(|index| cursor + index)
        .unwrap_or(value.len())
}

/// Returns the previous navigation word boundary, skipping separators before words.
fn previous_navigation_word_boundary(value: &str, cursor: usize) -> usize {
    let mut cursor = clamp_boundary(value, cursor);
    while cursor > 0 && previous_character(value, cursor).is_some_and(|c| !is_word_character(c)) {
        cursor = previous_boundary(value, cursor);
    }

    while cursor > 0 && previous_character(value, cursor).is_some_and(is_word_character) {
        cursor = previous_boundary(value, cursor);
    }

    cursor
}

/// Returns the previous word boundary for deletion/navigation.
fn previous_word_boundary(value: &str, cursor: usize) -> usize {
    let mut cursor = clamp_boundary(value, cursor);
    let Some(category) = previous_character(value, cursor).map(word_category) else {
        return cursor;
    };

    while cursor > 0
        && previous_character(value, cursor).is_some_and(|c| word_category(c) == category)
    {
        cursor = previous_boundary(value, cursor);
    }

    cursor
}

/// Returns the next word boundary for deletion/navigation.
fn next_word_boundary(value: &str, cursor: usize) -> usize {
    let mut cursor = clamp_boundary(value, cursor);
    let Some(category) = next_character(value, cursor).map(word_category) else {
        return cursor;
    };

    while cursor < value.len()
        && next_character(value, cursor).is_some_and(|c| word_category(c) == category)
    {
        cursor = next_boundary(value, cursor);
    }

    cursor
}

/// Returns the last character from the previous grapheme cluster.
fn previous_character(value: &str, cursor: usize) -> Option<char> {
    let previous = previous_boundary(value, cursor);
    if previous == cursor {
        return None;
    }

    value[previous..cursor].chars().last()
}

/// Returns the first character from the next grapheme cluster.
fn next_character(value: &str, cursor: usize) -> Option<char> {
    grapheme_at(value, cursor)?.chars().next()
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum WordCategory {
    Word,
    Separator,
}

/// Classifies a character using the original editor word grouping.
fn word_category(character: char) -> WordCategory {
    if is_word_character(character) {
        return WordCategory::Word;
    }

    WordCategory::Separator
}

/// Returns true for characters treated as word constituents by prompt shortcuts.
fn is_word_character(character: char) -> bool {
    character.is_alphanumeric() || character == '_'
}
