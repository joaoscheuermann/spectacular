#![allow(dead_code)]

use crate::prompt::grapheme::graphemes_in;
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

/// One visual prompt row represented as byte offsets into the prompt buffer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct VisualRow {
    pub start: usize,
    pub end: usize,
}

/// Cursor location in visual row and display-column coordinates.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct CursorPosition {
    pub row: usize,
    pub column: usize,
}

/// Splits prompt text into explicit and wrapped visual rows.
pub(crate) fn visual_rows(text: &str, content_width: usize) -> Vec<VisualRow> {
    let content_width = content_width.max(1);
    if text.is_empty() {
        return vec![VisualRow { start: 0, end: 0 }];
    }

    let mut rows = Vec::new();
    let mut line_start = 0;
    loop {
        let line_end = text[line_start..]
            .find('\n')
            .map(|index| line_start + index)
            .unwrap_or(text.len());
        push_wrapped_line(text, line_start, line_end, content_width, &mut rows);

        if line_end == text.len() {
            break;
        }

        line_start = line_end + 1;
        if line_start == text.len() {
            rows.push(VisualRow {
                start: text.len(),
                end: text.len(),
            });
            break;
        }
    }

    rows
}

/// Returns the visual row index containing or immediately preceding the cursor.
pub(crate) fn row_for_cursor(rows: &[VisualRow], cursor: usize) -> usize {
    rows.iter()
        .enumerate()
        .take_while(|(_, row)| row.start <= cursor)
        .map(|(index, _)| index)
        .last()
        .unwrap_or(0)
}

/// Returns row and display-column coordinates for a cursor byte offset.
pub(crate) fn cursor_position(
    text: &str,
    cursor: usize,
    _content_width: usize,
    rows: &[VisualRow],
) -> CursorPosition {
    let row_index = row_for_cursor(rows, cursor);
    let Some(row) = rows.get(row_index) else {
        return CursorPosition { row: 0, column: 0 };
    };

    let column_end = cursor.clamp(row.start, row.end);
    CursorPosition {
        row: row_index,
        column: display_width(&text[row.start..column_end]),
    }
}

/// Returns the byte offset in a visual row nearest to a target display column.
pub(crate) fn offset_for_column(text: &str, row: &VisualRow, target_column: usize) -> usize {
    let mut column = 0usize;
    for (offset, grapheme) in graphemes_in(&text[row.start..row.end]) {
        let width = grapheme_width(grapheme);
        if column + width > target_column {
            return row.start + offset;
        }
        column += width;
    }

    row.end
}

/// Returns the terminal display width of prompt text.
pub(crate) fn display_width(value: &str) -> usize {
    value
        .split('\t')
        .enumerate()
        .map(|(index, segment)| UnicodeWidthStr::width(segment) + usize::from(index > 0) * 4)
        .sum()
}

/// Returns a stable terminal cell width for one prompt character.
pub(crate) fn char_width(character: char) -> usize {
    if character == '\t' {
        return 4;
    }

    UnicodeWidthChar::width(character).unwrap_or(0)
}

/// Returns a stable terminal cell width for one grapheme cluster.
fn grapheme_width(grapheme: &str) -> usize {
    if grapheme == "\t" {
        return 4;
    }

    UnicodeWidthStr::width(grapheme)
}

/// Adds wrapped rows for one explicit prompt line.
fn push_wrapped_line(
    text: &str,
    line_start: usize,
    line_end: usize,
    content_width: usize,
    rows: &mut Vec<VisualRow>,
) {
    if line_start == line_end {
        rows.push(VisualRow {
            start: line_start,
            end: line_end,
        });
        return;
    }

    let mut row_start = line_start;
    let mut row_width = 0usize;
    for (offset, grapheme) in graphemes_in(&text[line_start..line_end]) {
        let index = line_start + offset;
        let width = grapheme_width(grapheme);
        if row_width > 0 && row_width + width > content_width {
            rows.push(VisualRow {
                start: row_start,
                end: index,
            });
            row_start = index;
            row_width = 0;
        }
        row_width += width;
    }

    rows.push(VisualRow {
        start: row_start,
        end: line_end,
    });
}
