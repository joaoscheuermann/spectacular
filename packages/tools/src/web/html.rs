use regex::Regex;

/// Normalizes an optional URL string to an HTTP(S) URL accepted by the web tool.
pub(crate) fn normalized_url(url: Option<&str>) -> Option<String> {
    let url = url?.trim();
    if url.is_empty() {
        return None;
    }

    let parsed = reqwest::Url::parse(url).ok()?;
    matches!(parsed.scheme(), "http" | "https").then(|| parsed.to_string())
}

/// Extracts the page title from HTML and decodes common entities.
pub(crate) fn extract_title(html: &str) -> String {
    let title_re =
        Regex::new(r"(?is)<title[^>]*>(.*?)</title>").expect("title regex should compile");
    title_re
        .captures(html)
        .and_then(|capture| capture.get(1))
        .map(|title| clean_html_fragment(title.as_str()))
        .unwrap_or_default()
}

/// Extracts visible text from HTML by removing head, scripts, tags, and redundant whitespace.
pub(crate) fn extract_page_text(html: &str) -> String {
    let script_re = Regex::new(
        r"(?is)<script[^>]*>.*?</script>|<style[^>]*>.*?</style>|<noscript[^>]*>.*?</noscript>",
    )
    .expect("script regex should compile");
    let head_re = Regex::new(r"(?is)<head[^>]*>.*?</head>").expect("head regex should compile");
    let break_re = Regex::new(r"(?i)<\s*(br|/p|/div|/li|/h[1-6]|/tr)\s*/?\s*>")
        .expect("break regex should compile");
    let tag_re = Regex::new(r"(?is)<[^>]+>").expect("tag regex should compile");
    let blank_lines_re = Regex::new(r"\n{3,}").expect("blank line regex should compile");
    let horizontal_space_re = Regex::new(r"[ \t]{2,}").expect("space regex should compile");
    let punctuation_space_re =
        Regex::new(r"\s+([.,;:!?])").expect("punctuation regex should compile");

    let without_head = head_re.replace_all(html, " ");
    let without_scripts = script_re.replace_all(&without_head, " ");
    let with_breaks = break_re.replace_all(&without_scripts, "\n");
    let without_tags = tag_re.replace_all(&with_breaks, " ");
    let decoded = decode_html_entities(&without_tags);
    let normalized_lines = decoded
        .lines()
        .map(|line| {
            horizontal_space_re
                .replace_all(line.trim(), " ")
                .to_string()
        })
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join("\n");

    let compact_punctuation = punctuation_space_re.replace_all(&normalized_lines, "$1");
    blank_lines_re
        .replace_all(&compact_punctuation, "\n\n")
        .trim()
        .to_owned()
}

/// Removes tags from a small HTML fragment and normalizes whitespace.
pub(crate) fn clean_html_fragment(fragment: &str) -> String {
    let tag_re = Regex::new(r"(?is)<[^>]+>").expect("fragment tag regex should compile");
    let space_re = Regex::new(r"\s+").expect("fragment space regex should compile");
    let without_tags = tag_re.replace_all(fragment, " ");
    let decoded = decode_html_entities(&without_tags);
    space_re.replace_all(decoded.trim(), " ").to_string()
}

/// Decodes numeric and common named HTML entities used in page and search result text.
fn decode_html_entities(value: &str) -> String {
    let entity_re = Regex::new(r"&#(x[0-9a-fA-F]+|\d+);|&(amp|lt|gt|quot|apos|#39|nbsp);")
        .expect("entity regex should compile");
    entity_re
        .replace_all(value, |captures: &regex::Captures<'_>| {
            if let Some(numeric) = captures.get(1) {
                return decode_numeric_entity(numeric.as_str()).unwrap_or_else(|| {
                    captures
                        .get(0)
                        .map(|matched| matched.as_str().to_owned())
                        .unwrap_or_default()
                });
            }

            match captures.get(2).map(|matched| matched.as_str()) {
                Some("amp") => "&".to_owned(),
                Some("lt") => "<".to_owned(),
                Some("gt") => ">".to_owned(),
                Some("quot") => "\"".to_owned(),
                Some("apos") | Some("#39") => "'".to_owned(),
                Some("nbsp") => " ".to_owned(),
                _ => captures
                    .get(0)
                    .map(|matched| matched.as_str().to_owned())
                    .unwrap_or_default(),
            }
        })
        .to_string()
}

/// Decodes decimal or hexadecimal numeric HTML entity content into a one-character string.
fn decode_numeric_entity(value: &str) -> Option<String> {
    let codepoint = if let Some(hex) = value.strip_prefix('x').or_else(|| value.strip_prefix('X')) {
        u32::from_str_radix(hex, 16).ok()?
    } else {
        value.parse::<u32>().ok()?
    };

    char::from_u32(codepoint).map(|character| character.to_string())
}

/// Truncates text to a maximum character count while preserving UTF-8 boundaries.
pub(crate) fn truncate_text(text: &str, max_chars: usize) -> (String, bool) {
    if text.chars().count() <= max_chars {
        return (text.to_owned(), false);
    }

    let truncated = text.chars().take(max_chars).collect::<String>();
    (truncated, true)
}
