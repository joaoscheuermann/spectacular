use regex::Regex;

use super::web_model::WebFindMatch;

/// Finds literal pattern matches in extracted page text and returns line-based matches.
pub(crate) fn find_in_text(
    text: &str,
    pattern: &str,
    ignore_case: bool,
    limit: usize,
) -> (Vec<WebFindMatch>, usize, bool) {
    let regex_pattern = regex::escape(pattern);
    let regex = Regex::new(&if ignore_case {
        format!("(?i){regex_pattern}")
    } else {
        regex_pattern
    })
    .expect("escaped find pattern should compile");
    let mut matches = Vec::new();
    let mut truncated = false;
    let mut total = 0;

    for (index, line) in text.lines().enumerate() {
        if !regex.is_match(line) {
            continue;
        }

        total += 1;
        if matches.len() >= limit {
            truncated = true;
            break;
        }

        matches.push(WebFindMatch {
            line: index + 1,
            text: line.to_owned(),
        });
    }

    (matches, total, truncated)
}
