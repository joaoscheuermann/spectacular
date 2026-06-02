use std::error::Error;
use std::fmt::{self, Display};

const REDACTED: &str = "[REDACTED]";

pub fn redact_repo_url(raw: &str) -> Result<String, RedactionError> {
    let input = raw.trim();

    if input.is_empty() {
        return Err(RedactionError::BlankRepoUrl);
    }

    let Some(scheme_end) = input.find("://") else {
        return Ok(input.to_owned());
    };

    let authority_start = scheme_end + 3;
    let after_scheme = &input[authority_start..];
    let authority_end = after_scheme
        .find(['/', '?', '#'])
        .map_or(input.len(), |offset| authority_start + offset);
    let authority = &input[authority_start..authority_end];

    let Some(userinfo_end) = authority.rfind('@') else {
        return Ok(input.to_owned());
    };

    let host = &authority[userinfo_end + 1..];

    if host.is_empty() {
        return Err(RedactionError::MissingHost);
    }

    Ok(format!(
        "{}{}{}",
        &input[..authority_start],
        host,
        &input[authority_end..]
    ))
}

pub fn redact_failure_text(text: &str) -> String {
    let mut redacted = String::with_capacity(text.len());
    let mut cursor = 0;

    while let Some(relative_start) = text[cursor..].find("sk-") {
        let start = cursor + relative_start;

        if !is_token_boundary(text[..start].chars().next_back()) {
            redacted.push_str(&text[cursor..start + 3]);
            cursor = start + 3;
            continue;
        }

        let end = token_end(text, start);
        let token = &text[start..end];

        if is_api_key_like(token) {
            redacted.push_str(&text[cursor..start]);
            redacted.push_str(REDACTED);
            cursor = end;
        } else {
            redacted.push_str(&text[cursor..end]);
            cursor = end;
        }
    }

    redacted.push_str(&text[cursor..]);
    redacted
}

pub(crate) fn redact_credential_urls(text: &str) -> String {
    let mut redacted = String::with_capacity(text.len());
    let mut cursor = 0;

    while let Some(relative_scheme_end) = text[cursor..].find("://") {
        let scheme_end = cursor + relative_scheme_end;
        let Some(url_start) = scheme_start(text, scheme_end) else {
            let after_marker = scheme_end + 3;
            redacted.push_str(&text[cursor..after_marker]);
            cursor = after_marker;
            continue;
        };
        let url_end = url_end(text, scheme_end + 3);
        let candidate = &text[url_start..url_end];
        let replacement =
            redact_repo_url(candidate).unwrap_or_else(|_| redact_malformed_url(candidate));

        redacted.push_str(&text[cursor..url_start]);
        redacted.push_str(&replacement);
        cursor = url_end;
    }

    redacted.push_str(&text[cursor..]);
    redacted
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RedactionError {
    BlankRepoUrl,
    MissingHost,
}

impl Display for RedactionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BlankRepoUrl => f.write_str("repo URL must not be blank"),
            Self::MissingHost => f.write_str("repo URL host must not be blank"),
        }
    }
}

impl Error for RedactionError {}

fn token_end(text: &str, start: usize) -> usize {
    text[start..]
        .char_indices()
        .find(|(_, ch)| !is_token_char(*ch))
        .map_or(text.len(), |(offset, _)| start + offset)
}

fn is_token_boundary(ch: Option<char>) -> bool {
    ch.is_none_or(|ch| !is_token_char(ch))
}

fn is_token_char(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_')
}

fn is_api_key_like(value: &str) -> bool {
    value.starts_with("sk-") && value.len() >= 12
}

fn redact_malformed_url(candidate: &str) -> String {
    let Some(scheme_end) = candidate.find("://") else {
        return REDACTED.to_owned();
    };

    let authority_start = scheme_end + 3;
    let after_scheme = &candidate[authority_start..];
    let authority_end = after_scheme
        .find(['/', '?', '#'])
        .map_or(candidate.len(), |offset| authority_start + offset);

    format!(
        "{}{}{}",
        &candidate[..authority_start],
        REDACTED,
        &candidate[authority_end..]
    )
}

fn scheme_start(text: &str, scheme_end: usize) -> Option<usize> {
    let mut start = scheme_end;

    for (index, ch) in text[..scheme_end].char_indices().rev() {
        if !is_scheme_char(ch) {
            break;
        }

        start = index;
    }

    let scheme = &text[start..scheme_end];
    scheme
        .chars()
        .next()
        .filter(|ch| ch.is_ascii_alphabetic())
        .map(|_| start)
}

fn url_end(text: &str, start: usize) -> usize {
    text[start..]
        .char_indices()
        .find(|(_, ch)| is_url_delimiter(*ch))
        .map_or(text.len(), |(offset, _)| start + offset)
}

fn is_scheme_char(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || matches!(ch, '+' | '-' | '.')
}

fn is_url_delimiter(ch: char) -> bool {
    ch.is_whitespace() || ch.is_control() || matches!(ch, '<' | '>' | '"' | '\'' | '`')
}
