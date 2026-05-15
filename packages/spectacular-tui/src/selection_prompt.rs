use crate::action::{SelectionPromptAnswer, SelectionPromptChoice};
use crate::session::SelectionPromptState;
use serde::{Deserialize, Serialize};

/// Input mode for interactive selection prompts.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub enum SelectionInputMode {
    #[default]
    Options,
    Comment,
}

impl SelectionPromptState {
    /// Creates a selection prompt with a title, description, and first option selected.
    pub fn new(
        title: impl Into<String>,
        description: impl Into<String>,
        options: Vec<String>,
    ) -> Self {
        Self {
            title: title.into(),
            description: description.into(),
            options,
            selected: 0,
            allow_custom: false,
            allow_comment: false,
            custom_input: String::new(),
            custom_cursor: 0,
            comment: String::new(),
            comment_cursor: 0,
            input_mode: SelectionInputMode::Options,
        }
    }

    /// Configures whether custom option and comment input rows are available.
    pub fn with_inputs(mut self, allow_custom: bool, allow_comment: bool) -> Self {
        self.allow_custom = allow_custom;
        self.allow_comment = allow_comment;
        self
    }

    /// Returns whether option navigation mode is currently active.
    pub fn is_options_mode(&self) -> bool {
        self.input_mode == SelectionInputMode::Options
    }

    /// Moves selection to the previous option with original wrapping behavior.
    pub fn select_previous(&mut self) {
        let count = self.choice_count();
        if count == 0 {
            return;
        }

        self.selected = if self.selected == 0 {
            count.saturating_sub(1)
        } else {
            self.selected - 1
        };
    }

    /// Moves selection to the next option with original wrapping behavior.
    pub fn select_next(&mut self) {
        let count = self.choice_count();
        if count == 0 {
            return;
        }

        self.selected = (self.selected + 1) % count;
    }

    /// Toggles between option navigation and comment editing when comments are allowed.
    pub fn toggle_comment_mode(&mut self) {
        if !self.allow_comment {
            return;
        }

        self.input_mode = match self.input_mode {
            SelectionInputMode::Options => SelectionInputMode::Comment,
            SelectionInputMode::Comment => SelectionInputMode::Options,
        };
    }

    /// Handles Escape by leaving comment mode first and indicating cancellation otherwise.
    pub fn escape(&mut self) -> bool {
        if self.input_mode == SelectionInputMode::Comment {
            self.input_mode = SelectionInputMode::Options;
            return false;
        }

        true
    }

    /// Inserts editable text into custom input or comment input based on mode.
    pub fn insert_text(&mut self, text: &str) {
        if text.is_empty() {
            return;
        }

        if self.input_mode == SelectionInputMode::Comment {
            self.comment.insert_str(self.comment_cursor, text);
            self.comment_cursor += text.len();
            return;
        }

        if !self.allow_custom {
            return;
        }

        self.selected = self.options.len();
        self.custom_input.insert_str(self.custom_cursor, text);
        self.custom_cursor += text.len();
    }

    /// Deletes one character before the active editable cursor.
    pub fn backspace(&mut self) {
        if self.input_mode == SelectionInputMode::Comment {
            delete_previous_character(&mut self.comment, &mut self.comment_cursor);
            return;
        }

        if self.is_custom_selected() {
            delete_previous_character(&mut self.custom_input, &mut self.custom_cursor);
        }
    }

    /// Deletes one character after the active editable cursor.
    pub fn delete_forward(&mut self) {
        if self.input_mode == SelectionInputMode::Comment {
            delete_next_character(&mut self.comment, self.comment_cursor);
            return;
        }

        if self.is_custom_selected() {
            delete_next_character(&mut self.custom_input, self.custom_cursor);
        }
    }

    /// Builds the current submitted answer, validating non-empty custom text.
    pub fn answer(&self) -> Option<SelectionPromptAnswer> {
        let choice = if self.is_custom_selected() {
            let value = self.custom_input.trim().to_owned();
            if value.is_empty() {
                return None;
            }
            SelectionPromptChoice::Custom(value)
        } else {
            SelectionPromptChoice::Option {
                index: self.selected,
                label: self.options.get(self.selected)?.clone(),
            }
        };

        Some(SelectionPromptAnswer {
            choice,
            comment: non_empty_trimmed(&self.comment),
        })
    }

    /// Returns whether the optional custom free-text row is selected.
    pub fn is_custom_selected(&self) -> bool {
        self.allow_custom && self.selected == self.options.len()
    }

    /// Returns the number of selectable rows including optional custom input.
    fn choice_count(&self) -> usize {
        self.options.len() + usize::from(self.allow_custom)
    }
}

/// Returns trimmed text only when it contains visible content.
fn non_empty_trimmed(value: &str) -> Option<String> {
    let value = value.trim();
    if value.is_empty() {
        return None;
    }

    Some(value.to_owned())
}

/// Deletes one Unicode scalar before a mutable cursor.
fn delete_previous_character(value: &mut String, cursor: &mut usize) {
    let previous = previous_boundary(value, *cursor);
    if previous == *cursor {
        return;
    }

    value.replace_range(previous..*cursor, "");
    *cursor = previous;
}

/// Deletes one Unicode scalar after a cursor.
fn delete_next_character(value: &mut String, cursor: usize) {
    let next = next_boundary(value, cursor);
    if next == cursor {
        return;
    }

    value.replace_range(cursor..next, "");
}

/// Returns the previous character boundary from the supplied cursor.
fn previous_boundary(value: &str, cursor: usize) -> usize {
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
    if cursor >= value.len() {
        return value.len();
    }

    value[cursor..]
        .char_indices()
        .nth(1)
        .map(|(index, _)| cursor + index)
        .unwrap_or(value.len())
}
