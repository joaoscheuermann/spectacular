use super::model::TranscriptItem;
use super::rows::{transcript_item_rows, TranscriptRowWrap};
use unicode_width::UnicodeWidthChar;

/// Returns the width-aware row count used by live layout for one transcript item.
pub(crate) fn transcript_item_row_count_for_width(item: &TranscriptItem, width: usize) -> usize {
    transcript_item_rows(item)
        .iter()
        .map(|row| match row.wrap {
            TranscriptRowWrap::NoWrap => 1,
            TranscriptRowWrap::Wrap => wrapped_text_row_count(&row.text, width),
        })
        .sum()
}

/// Returns the row count from the local IOCraft-style text wrapping model.
pub(crate) fn wrapped_layout_text_rows(text: &str, width: usize) -> usize {
    wrapped_text_row_count(text, width)
}

fn wrapped_text_row_count(text: &str, width: usize) -> usize {
    if text.is_empty() || width == 0 {
        return 1;
    }

    let mut rows = 1usize;
    let mut current_width = 0usize;
    for token in wrapping_tokens(text) {
        if current_width + token.non_trailing_width <= width {
            current_width = current_width.saturating_add(token.total_width);
            continue;
        }

        if current_width > 0 {
            rows = rows.saturating_add(1);
        }

        let (token_rows, token_width) = forced_wrap_width(token.non_trailing_width, width);
        rows = rows.saturating_add(token_rows.saturating_sub(1));
        current_width = token_width.saturating_add(token.trailing_width);
    }

    rows
}

fn wrapping_tokens(text: &str) -> Vec<WrappingToken> {
    let mut tokens = Vec::new();
    let mut non_trailing_width = 0usize;
    let mut trailing_width = 0usize;

    for character in text.chars() {
        let width = character.width().unwrap_or_default();
        if character.is_whitespace() {
            trailing_width = trailing_width.saturating_add(width);
            continue;
        }

        if trailing_width > 0 && non_trailing_width > 0 {
            tokens.push(WrappingToken::new(non_trailing_width, trailing_width));
            non_trailing_width = 0;
            trailing_width = 0;
        }

        non_trailing_width = non_trailing_width.saturating_add(trailing_width);
        trailing_width = 0;
        non_trailing_width = non_trailing_width.saturating_add(width);
    }

    if non_trailing_width > 0 || trailing_width > 0 {
        tokens.push(WrappingToken::new(non_trailing_width, trailing_width));
    }

    tokens
}

fn forced_wrap_width(width: usize, row_width: usize) -> (usize, usize) {
    if width == 0 {
        return (1, 0);
    }

    let rows = width.saturating_add(row_width.saturating_sub(1)) / row_width;
    let remainder = width % row_width;
    (
        rows.max(1),
        if remainder == 0 { row_width } else { remainder },
    )
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct WrappingToken {
    non_trailing_width: usize,
    trailing_width: usize,
    total_width: usize,
}

impl WrappingToken {
    fn new(non_trailing_width: usize, trailing_width: usize) -> Self {
        Self {
            non_trailing_width,
            trailing_width,
            total_width: non_trailing_width.saturating_add(trailing_width),
        }
    }
}
