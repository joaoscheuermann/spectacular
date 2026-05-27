use super::context::{line_end, line_start, next_boundary};
use super::CommandSuggestion;
use std::ops::Range;

pub(super) fn guide_next_field(text: &mut String, cursor: &mut usize, field: &str) {
    if let Some(range) = field_token_range(text, *cursor, field) {
        *cursor = range.end;
        return;
    }

    append_command_field(text, cursor, field);
}

pub(super) fn complete_suggestion(
    text: &mut String,
    cursor: &mut usize,
    range: Range<usize>,
    suggestion: &CommandSuggestion,
) {
    let token_start = range.start;
    text.replace_range(range, &suggestion.replacement);

    let insert_at = token_start + suggestion.replacement.len();
    if !suggestion.append_space {
        *cursor = insert_at;
        return;
    }

    if text[insert_at..]
        .chars()
        .next()
        .is_some_and(char::is_whitespace)
    {
        *cursor = next_boundary(text, insert_at);
        return;
    }

    text.insert(insert_at, ' ');
    *cursor = insert_at + 1;
}

fn field_token_range(text: &str, cursor: usize, field: &str) -> Option<Range<usize>> {
    let start = line_start(text, cursor);
    let end = line_end(text, cursor);
    let mut token_start = None;

    for (offset, character) in text[start..end].char_indices() {
        let index = start + offset;
        if character.is_whitespace() {
            if let Some(token_start) = token_start.take() {
                let token = &text[token_start..index];
                if token.starts_with(field) && token.as_bytes().get(field.len()) == Some(&b':') {
                    return Some(token_start..index);
                }
            }
            continue;
        }

        token_start.get_or_insert(index);
    }

    let token_start = token_start?;
    let token = &text[token_start..end];
    (token.starts_with(field) && token.as_bytes().get(field.len()) == Some(&b':'))
        .then_some(token_start..end)
}

fn append_command_field(text: &mut String, cursor: &mut usize, field: &str) {
    let end = line_end(text, *cursor);
    let needs_space = text[..end]
        .chars()
        .last()
        .is_some_and(|character| !character.is_whitespace());
    let insertion = if needs_space {
        format!(" {field}:")
    } else {
        format!("{field}:")
    };

    text.insert_str(end, &insertion);
    *cursor = end + insertion.len();
}
