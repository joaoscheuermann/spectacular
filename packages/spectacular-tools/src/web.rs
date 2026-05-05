use crate::display::paint;
use anstyle::AnsiColor;
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use spectacular_agent::{Cancellation, Tool, ToolDisplay, ToolExecution, ToolManifest};
use std::time::Duration;

pub const WEB_SEARCH_TOOL_NAME: &str = "web";

const WEB_SEARCH_TOOL_DESCRIPTION: &str =
    "Searches the web, opens a page, or finds text within a page. Actions mirror Codex web navigation: search, open_page, and find_in_page.";
const USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36";
const DEFAULT_SEARCH_LIMIT: usize = 5;
const MAX_SEARCH_LIMIT: usize = 10;
const DEFAULT_FIND_LIMIT: usize = 20;
const MAX_FIND_LIMIT: usize = 100;
const DEFAULT_MAX_CHARS: usize = 12_000;
const MAX_PAGE_CHARS: usize = 50_000;
const REQUEST_TIMEOUT_SECS: u64 = 20;
const CANCELLATION_POLL_MS: u64 = 50;

#[derive(Clone, Debug, Default)]
pub struct WebSearchTool;

impl Tool for WebSearchTool {
    fn name(&self) -> &str {
        WEB_SEARCH_TOOL_NAME
    }

    fn manifest(&self) -> ToolManifest {
        ToolManifest::new(
            WEB_SEARCH_TOOL_NAME,
            WEB_SEARCH_TOOL_DESCRIPTION,
            json!({
                "type": "object",
                "properties": {
                    "action": {
                        "type": "string",
                        "enum": ["search", "open_page", "find_in_page"],
                        "description": "Navigation action to perform. Use search for a query, open_page for a URL, and find_in_page for a URL plus pattern."
                    },
                    "query": {
                        "type": "string",
                        "description": "Search query. Required when action is search."
                    },
                    "url": {
                        "type": "string",
                        "description": "Page URL. Required when action is open_page or find_in_page."
                    },
                    "pattern": {
                        "type": "string",
                        "description": "Literal text pattern to find in page text. Required when action is find_in_page."
                    },
                    "ignoreCase": {
                        "type": "boolean",
                        "description": "Case-insensitive find_in_page matching (default: true)."
                    },
                    "limit": {
                        "type": "integer",
                        "description": "Maximum number of search results or page matches to return."
                    },
                    "maxChars": {
                        "type": "integer",
                        "description": "Maximum extracted page text characters to return for open_page (default: 12000, maximum: 50000)."
                    }
                },
                "required": ["action"],
                "additionalProperties": false
            }),
        )
    }

    fn format_input(&self, arguments: &Value) -> ToolDisplay {
        web_action_detail(arguments).unwrap_or_else(|| "<missing web action>".to_owned())
    }

    fn format_output(&self, raw_output: &str, parsed_output: Option<&Value>) -> ToolDisplay {
        let Some(output) = parsed_output else {
            return raw_output.to_string();
        };

        if let Some(error) = output.get("error").and_then(Value::as_str) {
            let status = paint(AnsiColor::BrightRed.on_default().bold(), "failed");
            return format!("{status}: {error}");
        }

        let action = output
            .get("action")
            .and_then(Value::as_str)
            .unwrap_or("web");
        let total = output.get("total").and_then(Value::as_u64).unwrap_or(0);
        let truncated = output
            .get("truncated")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let suffix = if truncated { " (truncated)" } else { "" };

        match action {
            "search" => format!("{total} result(s){suffix}"),
            "open_page" => {
                let title = output
                    .get("page")
                    .and_then(|page| page.get("title"))
                    .and_then(Value::as_str)
                    .filter(|title| !title.is_empty())
                    .unwrap_or("page");
                format!("opened {title}{suffix}")
            }
            "find_in_page" => format!("{total} match(es){suffix}"),
            _ => raw_output.to_string(),
        }
    }

    fn execute<'a>(&'a self, arguments: Value, cancellation: Cancellation) -> ToolExecution<'a> {
        Box::pin(async move {
            let input = match serde_json::from_value::<WebInput>(arguments) {
                Ok(input) => input,
                Err(error) => {
                    return Ok(serialize_output(&web_error(
                        "unknown",
                        format!("Invalid input JSON: {error}"),
                    )));
                }
            };

            Ok(serialize_output(&execute_web(input, cancellation).await))
        })
    }
}

#[derive(Debug, Deserialize)]
struct WebInput {
    action: WebAction,
    #[serde(default)]
    query: Option<String>,
    #[serde(default)]
    url: Option<String>,
    #[serde(default)]
    pattern: Option<String>,
    #[serde(default, rename = "ignoreCase")]
    ignore_case: Option<bool>,
    #[serde(default)]
    limit: Option<usize>,
    #[serde(default, rename = "maxChars")]
    max_chars: Option<usize>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
enum WebAction {
    Search,
    OpenPage,
    FindInPage,
}

impl WebAction {
    fn as_str(self) -> &'static str {
        match self {
            Self::Search => "search",
            Self::OpenPage => "open_page",
            Self::FindInPage => "find_in_page",
        }
    }
}

#[derive(Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct WebSearchResult {
    pub title: String,
    pub url: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub snippet: String,
}

#[derive(Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct WebPageOutput {
    pub url: String,
    pub title: String,
    pub text: String,
}

#[derive(Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct WebFindMatch {
    pub line: usize,
    pub text: String,
}

#[derive(Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct WebOutput {
    pub action: String,
    pub detail: String,
    pub results: Vec<WebSearchResult>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page: Option<WebPageOutput>,
    pub matches: Vec<WebFindMatch>,
    pub total: usize,
    pub truncated: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

async fn execute_web(input: WebInput, cancellation: Cancellation) -> WebOutput {
    match input.action {
        WebAction::Search => execute_search(input, cancellation).await,
        WebAction::OpenPage => execute_open_page(input, cancellation).await,
        WebAction::FindInPage => execute_find_in_page(input, cancellation).await,
    }
}

async fn execute_search(input: WebInput, cancellation: Cancellation) -> WebOutput {
    let query = match input
        .query
        .as_deref()
        .map(str::trim)
        .filter(|query| !query.is_empty())
    {
        Some(query) => query.to_owned(),
        None => return web_error(WebAction::Search.as_str(), "Missing query for search"),
    };
    let limit = input
        .limit
        .unwrap_or(DEFAULT_SEARCH_LIMIT)
        .clamp(1, MAX_SEARCH_LIMIT);
    let url = match reqwest::Url::parse_with_params(
        "https://duckduckgo.com/html/",
        [("q", query.as_str())],
    ) {
        Ok(url) => url,
        Err(error) => {
            return web_error(
                WebAction::Search.as_str(),
                format!("Failed to build search URL: {error}"),
            );
        }
    };

    let html = match fetch_url(url.as_str(), cancellation).await {
        Ok(html) => html,
        Err(error) => return web_error(WebAction::Search.as_str(), error),
    };
    let mut results = parse_duckduckgo_results(&html);
    let total = results.len();
    let truncated = total > limit;
    if truncated {
        results.truncate(limit);
    }

    WebOutput {
        action: WebAction::Search.as_str().to_owned(),
        detail: query,
        results,
        page: None,
        matches: Vec::new(),
        total,
        truncated,
        error: None,
    }
}

async fn execute_open_page(input: WebInput, cancellation: Cancellation) -> WebOutput {
    let Some(url) = normalized_url(input.url.as_deref()) else {
        return web_error(
            WebAction::OpenPage.as_str(),
            "Missing or invalid URL for open_page",
        );
    };
    let max_chars = input
        .max_chars
        .unwrap_or(DEFAULT_MAX_CHARS)
        .clamp(1, MAX_PAGE_CHARS);

    let html = match fetch_url(&url, cancellation).await {
        Ok(html) => html,
        Err(error) => return web_error(WebAction::OpenPage.as_str(), error),
    };
    let title = extract_title(&html);
    let text = extract_page_text(&html);
    let (text, truncated) = truncate_text(&text, max_chars);

    WebOutput {
        action: WebAction::OpenPage.as_str().to_owned(),
        detail: url.clone(),
        results: Vec::new(),
        page: Some(WebPageOutput { url, title, text }),
        matches: Vec::new(),
        total: 1,
        truncated,
        error: None,
    }
}

async fn execute_find_in_page(input: WebInput, cancellation: Cancellation) -> WebOutput {
    let Some(url) = normalized_url(input.url.as_deref()) else {
        return web_error(
            WebAction::FindInPage.as_str(),
            "Missing or invalid URL for find_in_page",
        );
    };
    let pattern = match input
        .pattern
        .as_deref()
        .map(str::trim)
        .filter(|pattern| !pattern.is_empty())
    {
        Some(pattern) => pattern.to_owned(),
        None => {
            return web_error(
                WebAction::FindInPage.as_str(),
                "Missing pattern for find_in_page",
            );
        }
    };
    let limit = input
        .limit
        .unwrap_or(DEFAULT_FIND_LIMIT)
        .clamp(1, MAX_FIND_LIMIT);

    let html = match fetch_url(&url, cancellation).await {
        Ok(html) => html,
        Err(error) => return web_error(WebAction::FindInPage.as_str(), error),
    };
    let text = extract_page_text(&html);
    let (matches, total, truncated) =
        find_in_text(&text, &pattern, input.ignore_case.unwrap_or(true), limit);

    WebOutput {
        action: WebAction::FindInPage.as_str().to_owned(),
        detail: format!("'{pattern}' in {url}"),
        results: Vec::new(),
        page: None,
        matches,
        total,
        truncated,
        error: None,
    }
}

async fn fetch_url(url: &str, cancellation: Cancellation) -> Result<String, String> {
    if cancellation.is_cancelled() {
        return Err("Request cancelled".to_owned());
    }

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(REQUEST_TIMEOUT_SECS))
        .user_agent(USER_AGENT)
        .build()
        .map_err(|error| format!("Failed to build web client: {error}"))?;
    let request = client.get(url).send();
    tokio::pin!(request);

    let response = loop {
        if cancellation.is_cancelled() {
            return Err("Request cancelled".to_owned());
        }

        tokio::select! {
            result = &mut request => break result.map_err(|error| format!("Request failed: {error}"))?,
            _ = tokio::time::sleep(Duration::from_millis(CANCELLATION_POLL_MS)) => {}
        }
    };

    let status = response.status();
    if !status.is_success() {
        return Err(format!("Request returned HTTP {status}"));
    }

    if cancellation.is_cancelled() {
        return Err("Request cancelled".to_owned());
    }

    response
        .text()
        .await
        .map_err(|error| format!("Failed to read response body: {error}"))
}

fn normalized_url(url: Option<&str>) -> Option<String> {
    let url = url?.trim();
    if url.is_empty() {
        return None;
    }

    let parsed = reqwest::Url::parse(url).ok()?;
    matches!(parsed.scheme(), "http" | "https").then(|| parsed.to_string())
}

fn parse_duckduckgo_results(html: &str) -> Vec<WebSearchResult> {
    let result_link_re = Regex::new(
        r#"(?is)<a[^>]*class=["'][^"']*result__a[^"']*["'][^>]*href=["']([^"']+)["'][^>]*>(.*?)</a>"#,
    )
    .expect("result link regex should compile");
    let snippet_re =
        Regex::new(r#"(?is)<a[^>]*class=["'][^"']*result__snippet[^"']*["'][^>]*>(.*?)</a>"#)
            .expect("snippet regex should compile");
    let snippets = snippet_re
        .captures_iter(html)
        .map(|capture| {
            clean_html_fragment(capture.get(1).map(|value| value.as_str()).unwrap_or(""))
        })
        .collect::<Vec<_>>();

    result_link_re
        .captures_iter(html)
        .enumerate()
        .filter_map(|(index, capture)| {
            let raw_url = capture.get(1)?.as_str();
            let url = normalize_duckduckgo_result_url(raw_url);
            if url.is_empty() {
                return None;
            }

            Some(WebSearchResult {
                title: clean_html_fragment(
                    capture.get(2).map(|value| value.as_str()).unwrap_or(""),
                ),
                url,
                snippet: snippets.get(index).cloned().unwrap_or_default(),
            })
        })
        .collect()
}

fn normalize_duckduckgo_result_url(raw_url: &str) -> String {
    let url = if raw_url.starts_with("//") {
        format!("https:{raw_url}")
    } else {
        raw_url.to_owned()
    };

    let Ok(parsed) = reqwest::Url::parse(&url) else {
        return url;
    };

    parsed
        .query_pairs()
        .find_map(|(key, value)| (key == "uddg").then(|| value.into_owned()))
        .unwrap_or_else(|| parsed.to_string())
}

fn extract_title(html: &str) -> String {
    let title_re =
        Regex::new(r"(?is)<title[^>]*>(.*?)</title>").expect("title regex should compile");
    title_re
        .captures(html)
        .and_then(|capture| capture.get(1))
        .map(|title| clean_html_fragment(title.as_str()))
        .unwrap_or_default()
}

fn extract_page_text(html: &str) -> String {
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

fn clean_html_fragment(fragment: &str) -> String {
    let tag_re = Regex::new(r"(?is)<[^>]+>").expect("fragment tag regex should compile");
    let space_re = Regex::new(r"\s+").expect("fragment space regex should compile");
    let without_tags = tag_re.replace_all(fragment, " ");
    let decoded = decode_html_entities(&without_tags);
    space_re.replace_all(decoded.trim(), " ").to_string()
}

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

fn decode_numeric_entity(value: &str) -> Option<String> {
    let codepoint = if let Some(hex) = value.strip_prefix('x').or_else(|| value.strip_prefix('X')) {
        u32::from_str_radix(hex, 16).ok()?
    } else {
        value.parse::<u32>().ok()?
    };

    char::from_u32(codepoint).map(|character| character.to_string())
}

fn find_in_text(
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

fn truncate_text(text: &str, max_chars: usize) -> (String, bool) {
    if text.chars().count() <= max_chars {
        return (text.to_owned(), false);
    }

    let truncated = text.chars().take(max_chars).collect::<String>();
    (truncated, true)
}

fn web_action_detail(arguments: &Value) -> Option<String> {
    match arguments.get("action").and_then(Value::as_str)? {
        "search" => arguments
            .get("query")
            .and_then(Value::as_str)
            .filter(|query| !query.is_empty())
            .map(str::to_owned),
        "open_page" => arguments
            .get("url")
            .and_then(Value::as_str)
            .filter(|url| !url.is_empty())
            .map(str::to_owned),
        "find_in_page" => {
            let pattern = arguments.get("pattern").and_then(Value::as_str);
            let url = arguments.get("url").and_then(Value::as_str);
            match (pattern, url) {
                (Some(pattern), Some(url)) if !pattern.is_empty() && !url.is_empty() => {
                    Some(format!("'{pattern}' in {url}"))
                }
                (Some(pattern), _) if !pattern.is_empty() => Some(format!("'{pattern}'")),
                (_, Some(url)) if !url.is_empty() => Some(url.to_owned()),
                _ => None,
            }
        }
        _ => None,
    }
}

fn web_error(action: impl Into<String>, message: impl Into<String>) -> WebOutput {
    WebOutput {
        action: action.into(),
        detail: String::new(),
        results: Vec::new(),
        page: None,
        matches: Vec::new(),
        total: 0,
        truncated: false,
        error: Some(message.into()),
    }
}

fn serialize_output(output: &WebOutput) -> String {
    serde_json::to_string(output).expect("web output should serialize")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn format_input_mirrors_codex_web_action_details() {
        let tool = WebSearchTool;

        assert_eq!(
            tool.format_input(&json!({"action": "search", "query": "rust async"})),
            "rust async"
        );
        assert_eq!(
            tool.format_input(&json!({"action": "open_page", "url": "https://example.com"})),
            "https://example.com"
        );
        assert_eq!(
            tool.format_input(&json!({
                "action": "find_in_page",
                "url": "https://example.com",
                "pattern": "needle"
            })),
            "'needle' in https://example.com"
        );
    }

    #[test]
    fn parses_duckduckgo_result_links_and_redirect_urls() {
        let html = r#"
            <a class="result__a" href="//duckduckgo.com/l/?uddg=https%3A%2F%2Fexample.com%2Fdoc&amp;rut=abc">Example &amp; Docs</a>
            <a class="result__snippet">A <b>short</b> snippet &amp; context.</a>
        "#;

        assert_eq!(
            parse_duckduckgo_results(html),
            vec![WebSearchResult {
                title: "Example & Docs".to_owned(),
                url: "https://example.com/doc".to_owned(),
                snippet: "A short snippet & context.".to_owned(),
            }]
        );
    }

    #[test]
    fn extracts_page_title_and_visible_text() {
        let html = r#"
            <html>
                <head><title>Example &amp; Page</title><style>.x{display:none}</style></head>
                <body><h1>Hello&nbsp;World</h1><script>alert('x')</script><p>Visible <b>text</b>.</p></body>
            </html>
        "#;

        assert_eq!(extract_title(html), "Example & Page");
        assert_eq!(extract_page_text(html), "Hello World\nVisible text.");
    }

    #[test]
    fn finds_literal_text_in_extracted_page_lines() {
        let (matches, total, truncated) =
            find_in_text("Alpha\nBeta Needle\nneedle two", "needle", true, 1);

        assert!(truncated);
        assert_eq!(total, 2);
        assert_eq!(
            matches,
            vec![WebFindMatch {
                line: 2,
                text: "Beta Needle".to_owned(),
            }]
        );
    }

    #[test]
    fn rejects_invalid_or_non_http_urls() {
        assert_eq!(normalized_url(Some("file:///tmp/a")), None);
        assert_eq!(normalized_url(Some("not a url")), None);
        assert_eq!(
            normalized_url(Some("https://example.com/path")).as_deref(),
            Some("https://example.com/path")
        );
    }
}
