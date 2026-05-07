#[derive(Clone, Debug, Eq, PartialEq)]
struct CompletionContext<'a> {
    token_start: usize,
    token_end: usize,
    query: &'a str,
    target: CompletionTarget,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum CompletionTarget {
    Command,
    Subcommand {
        command: String,
    },
    Field {
        command: String,
        subcommand: String,
        used_fields: Vec<String>,
    },
    Value {
        command: String,
        subcommand: String,
        field: String,
        field_query: String,
        value_query: String,
        args: Vec<(String, String)>,
    },
}

fn completion_context(buffer: &str, cursor: usize) -> Option<CompletionContext<'_>> {
    if !buffer.starts_with('/') || cursor == 0 || cursor > buffer.len() {
        return None;
    }

    if line_start(buffer, cursor) != 0 {
        return None;
    }

    let token_start = token_start(buffer, cursor);
    let token_end = token_end(buffer, cursor);
    let query = &buffer[token_start..cursor];
    let tokens = command_tokens_before(buffer, token_start);

    if tokens.is_empty() {
        return Some(CompletionContext {
            token_start,
            token_end,
            query,
            target: CompletionTarget::Command,
        });
    }

    if tokens.len() == 1 {
        return Some(CompletionContext {
            token_start,
            token_end,
            query,
            target: CompletionTarget::Subcommand {
                command: tokens[0].clone(),
            },
        });
    }

    let command = tokens[0].clone();
    let subcommand = tokens[1].clone();
    if let Some((field, value_query)) = query.split_once(':') {
        return Some(CompletionContext {
            token_start,
            token_end,
            query,
            target: CompletionTarget::Value {
                command,
                subcommand,
                field: field.to_owned(),
                field_query: field.to_owned(),
                value_query: value_query.to_owned(),
                args: named_pairs(&tokens[2..]),
            },
        });
    }

    Some(CompletionContext {
        token_start,
        token_end,
        query,
        target: CompletionTarget::Field {
            command,
            subcommand,
            used_fields: used_fields(&tokens[2..]),
        },
    })
}

/// Builds a stable key for suppressing the currently visible completion picker.
fn completion_context_key(buffer: &str, cursor: usize) -> Option<String> {
    let context = completion_context(buffer, cursor)?;
    Some(format!(
        "{}:{}:{}",
        context.token_start, context.token_end, context.query
    ))
}

fn token_start(buffer: &str, cursor: usize) -> usize {
    let line_start = line_start(buffer, cursor);
    buffer[line_start..cursor]
        .char_indices()
        .rev()
        .find(|(_, character)| character.is_whitespace())
        .map(|(index, character)| line_start + index + character.len_utf8())
        .unwrap_or(usize::from(buffer.starts_with('/')))
}

fn token_end(buffer: &str, cursor: usize) -> usize {
    let line_end = line_end(buffer, cursor);
    buffer[cursor..line_end]
        .char_indices()
        .find(|(_, character)| character.is_whitespace())
        .map(|(index, _)| cursor + index)
        .unwrap_or(line_end)
}

fn command_tokens_before(buffer: &str, token_start: usize) -> Vec<String> {
    buffer[1..token_start]
        .split_whitespace()
        .map(str::to_owned)
        .collect()
}

fn used_fields(tokens: &[String]) -> Vec<String> {
    tokens
        .iter()
        .filter_map(|token| token.split_once(':').map(|(field, _)| field.to_owned()))
        .collect()
}

fn named_pairs(tokens: &[String]) -> Vec<(String, String)> {
    tokens
        .iter()
        .filter_map(|token| {
            let (field, value) = token.split_once(':')?;
            Some((field.to_owned(), value.to_owned()))
        })
        .collect()
}

fn find_subcommand(
    specs: &[CompletionSubcommandSpec],
    name: &str,
) -> Option<CompletionSubcommandSpec> {
    specs.iter().copied().find(|spec| spec.name == name)
}

fn find_field(specs: &[CompletionFieldSpec], name: &str) -> Option<CompletionFieldSpec> {
    specs.iter().copied().find(|spec| spec.name == name)
}

fn line_start(buffer: &str, cursor: usize) -> usize {
    buffer[..cursor]
        .rfind('\n')
        .map(|index| index + 1)
        .unwrap_or(0)
}

fn line_end(buffer: &str, cursor: usize) -> usize {
    buffer[cursor..]
        .find('\n')
        .map(|index| cursor + index)
        .unwrap_or(buffer.len())
}

fn previous_word_boundary(buffer: &str, cursor: usize) -> usize {
    let mut cursor = cursor;
    cursor = move_while_previous(buffer, cursor, |character| character.is_whitespace());
    let Some(category) = previous_character(buffer, cursor).map(word_category) else {
        return cursor;
    };

    move_while_previous(buffer, cursor, |character| {
        word_category(character) == category
    })
}

fn next_word_boundary(buffer: &str, cursor: usize) -> usize {
    let mut cursor = cursor;
    cursor = move_while_next(buffer, cursor, |character| character.is_whitespace());
    let Some(category) = next_character(buffer, cursor).map(word_category) else {
        return cursor;
    };

    move_while_next(buffer, cursor, |character| {
        word_category(character) == category
    })
}

fn move_while_previous(buffer: &str, mut cursor: usize, predicate: impl Fn(char) -> bool) -> usize {
    while let Some(character) = previous_character(buffer, cursor) {
        if !predicate(character) {
            break;
        }
        cursor = previous_boundary(buffer, cursor);
    }
    cursor
}

fn move_while_next(buffer: &str, mut cursor: usize, predicate: impl Fn(char) -> bool) -> usize {
    while let Some(character) = next_character(buffer, cursor) {
        if !predicate(character) {
            break;
        }
        cursor = next_boundary(buffer, cursor);
    }
    cursor
}

fn previous_character(buffer: &str, cursor: usize) -> Option<char> {
    if cursor == 0 {
        return None;
    }

    buffer[..cursor].chars().next_back()
}

fn next_character(buffer: &str, cursor: usize) -> Option<char> {
    if cursor >= buffer.len() {
        return None;
    }

    buffer[cursor..].chars().next()
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum WordCategory {
    Word,
    Separator,
}

fn word_category(character: char) -> WordCategory {
    if character.is_alphanumeric() || character == '_' {
        return WordCategory::Word;
    }

    WordCategory::Separator
}

fn previous_boundary(buffer: &str, cursor: usize) -> usize {
    if cursor == 0 {
        return 0;
    }

    buffer[..cursor]
        .char_indices()
        .last()
        .map(|(index, _)| index)
        .unwrap_or(0)
}

fn next_boundary(buffer: &str, cursor: usize) -> usize {
    if cursor >= buffer.len() {
        return buffer.len();
    }

    buffer[cursor..]
        .char_indices()
        .nth(1)
        .map(|(index, _)| cursor + index)
        .unwrap_or(buffer.len())
}

fn clamp_to_boundary(buffer: &str, cursor: usize) -> usize {
    let mut cursor = cursor.min(buffer.len());
    while !buffer.is_char_boundary(cursor) {
        cursor = cursor.saturating_sub(1);
    }
    cursor
}

fn normalize_paste(value: &str) -> String {
    value.replace("\r\n", "\n").replace('\r', "\n")
}

fn is_key_edit_event(key: KeyEvent) -> bool {
    matches!(key.kind, KeyEventKind::Press | KeyEventKind::Repeat)
}

fn is_submit_key(key: KeyEvent) -> bool {
    key.code == KeyCode::Enter && key.modifiers == KeyModifiers::NONE
}

fn is_unmodified_line_break_char(key: KeyEvent) -> bool {
    matches!(key.code, KeyCode::Char('\n' | '\r')) && key.modifiers == KeyModifiers::NONE
}

fn is_newline_key(key: KeyEvent) -> bool {
    key.code == KeyCode::Enter
        && (key.modifiers.contains(KeyModifiers::ALT)
            || key.modifiers.contains(KeyModifiers::CONTROL)
            || key.modifiers.contains(KeyModifiers::SHIFT))
        || is_ctrl_char(key, 'j')
}

fn moves_by_word(key: KeyEvent) -> bool {
    key.modifiers.contains(KeyModifiers::ALT) || key.modifiers.contains(KeyModifiers::CONTROL)
}

fn selects_text(key: KeyEvent) -> bool {
    key.modifiers.contains(KeyModifiers::SHIFT)
}

fn is_ctrl_char(key: KeyEvent, target: char) -> bool {
    if !key.modifiers.contains(KeyModifiers::CONTROL) {
        return false;
    }

    matches!(key.code, KeyCode::Char(character) if character.eq_ignore_ascii_case(&target))
}

fn should_insert_char(key: KeyEvent) -> bool {
    if key.modifiers.contains(KeyModifiers::CONTROL) && !key.modifiers.contains(KeyModifiers::ALT) {
        return false;
    }

    if key.modifiers.contains(KeyModifiers::ALT) && !key.modifiers.contains(KeyModifiers::CONTROL) {
        return false;
    }

    matches!(key.code, KeyCode::Char(character) if !character.is_control())
}

fn is_plain_paste_candidate_key(key: KeyEvent) -> bool {
    key.modifiers == KeyModifiers::NONE
}

fn saturating_u16(value: usize) -> u16 {
    value.min(usize::from(u16::MAX)) as u16
}
