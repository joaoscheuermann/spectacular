use crate::prompt::grapheme::{clamp_boundary, grapheme_at, next_boundary, previous_boundary};
use crate::prompt::layout::{cursor_position, offset_for_column, visual_rows};
use crate::prompt::state::text::{line_end, line_start};
use crate::session::PromptState;

const DEFAULT_VERTICAL_WIDTH: usize = 120;

impl PromptState {
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
    pub(super) fn move_to(&mut self, cursor: usize, selecting: bool) {
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
}

/// Returns the previous navigation word boundary, skipping separators before words.
pub(super) fn previous_navigation_word_boundary(value: &str, cursor: usize) -> usize {
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
pub(super) fn previous_word_boundary(value: &str, cursor: usize) -> usize {
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
pub(super) fn next_word_boundary(value: &str, cursor: usize) -> usize {
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
