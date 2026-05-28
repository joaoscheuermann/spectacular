use crate::selection::{display_width, SelectableSource};
use std::ops::Range;
use unicode_width::UnicodeWidthChar;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct SourceProjectionRow {
    pub(super) source: SelectableSource,
    pub(super) text: String,
    pub(super) source_columns: Range<usize>,
    pub(super) source_width: usize,
}

pub(super) fn wrapped_text_rows(
    text: &str,
    width: usize,
    source: SelectableSource,
) -> Vec<SourceProjectionRow> {
    let width = width.max(1);
    if text.is_empty() {
        return vec![SourceProjectionRow {
            source,
            text: String::new(),
            source_columns: 0..0,
            source_width: 0,
        }];
    }

    let source_width = display_width(text);
    let mut builder = WrappedRows::new(source.clone(), source_width);
    for token in wrapping_tokens(text) {
        if builder.fits(&token, width) {
            builder.push_text(&token.text);
            continue;
        }

        if builder.has_current_row() {
            builder.finish_row();
        }

        if token.non_trailing_width > width {
            builder.push_forced_wrapped(&token.text, width);
            continue;
        }

        builder.push_text(&token.text);
    }

    builder.finish()
}

fn wrapping_tokens(text: &str) -> Vec<WrappingToken> {
    let mut tokens = Vec::new();
    let mut text_start = 0usize;
    let mut text_end = 0usize;
    let mut non_trailing_width = 0usize;
    let mut trailing_width = 0usize;
    let mut in_token = false;

    for (offset, character) in text.char_indices() {
        if !in_token {
            text_start = offset;
            in_token = true;
        }
        text_end = offset + character.len_utf8();
        let width = character.width().unwrap_or(0);
        if character.is_whitespace() {
            trailing_width = trailing_width.saturating_add(width);
            continue;
        }

        if trailing_width > 0 && non_trailing_width > 0 {
            tokens.push(WrappingToken {
                text: text[text_start..offset].to_owned(),
                non_trailing_width,
            });
            text_start = offset;
            non_trailing_width = 0;
            trailing_width = 0;
        }

        non_trailing_width = non_trailing_width
            .saturating_add(trailing_width)
            .saturating_add(width);
        trailing_width = 0;
    }

    if in_token {
        tokens.push(WrappingToken {
            text: text[text_start..text_end].to_owned(),
            non_trailing_width,
        });
    }

    tokens
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct WrappedRows {
    source: SelectableSource,
    source_width: usize,
    rows: Vec<SourceProjectionRow>,
    current: String,
    current_width: usize,
    row_start_column: usize,
    row_end_column: usize,
}

impl WrappedRows {
    fn new(source: SelectableSource, source_width: usize) -> Self {
        Self {
            source,
            source_width,
            rows: Vec::new(),
            current: String::new(),
            current_width: 0,
            row_start_column: 0,
            row_end_column: 0,
        }
    }

    fn fits(&self, token: &WrappingToken, width: usize) -> bool {
        self.current_width.saturating_add(token.non_trailing_width) <= width
    }

    fn has_current_row(&self) -> bool {
        self.current_width > 0
    }

    fn push_text(&mut self, text: &str) {
        self.current.push_str(text);
        let width = display_width(text);
        self.current_width = self.current_width.saturating_add(width);
        self.row_end_column = self.row_end_column.saturating_add(width);
    }

    fn push_forced_wrapped(&mut self, text: &str, width: usize) {
        for character in text.chars() {
            let char_width = character.width().unwrap_or(0);
            if self.current_width > 0 && self.current_width.saturating_add(char_width) > width {
                self.finish_row();
            }
            self.current.push(character);
            self.current_width = self.current_width.saturating_add(char_width);
            self.row_end_column = self.row_end_column.saturating_add(char_width);
        }
    }

    fn finish_row(&mut self) {
        self.rows.push(SourceProjectionRow {
            source: self.source.clone(),
            text: std::mem::take(&mut self.current),
            source_columns: self.row_start_column..self.row_end_column,
            source_width: self.source_width,
        });
        self.row_start_column = self.row_end_column;
        self.current_width = 0;
    }

    fn finish(mut self) -> Vec<SourceProjectionRow> {
        self.finish_row();
        self.rows
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct WrappingToken {
    text: String,
    non_trailing_width: usize,
}
