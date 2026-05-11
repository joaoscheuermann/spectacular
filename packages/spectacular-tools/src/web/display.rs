use crate::display::{tool_arg_line, tool_arg_tool_arg_line};
use crate::output_preview::{preview_lines, preview_text};
use serde_json::Value;
use spectacular_agent::{ToolDisplay, ToolManifest};

use super::WEB_SEARCH_TOOL_NAME;

const WEB_SEARCH_TOOL_DESCRIPTION: &str =
    "Searches the web, opens a page, or finds text within a page. Actions mirror Codex web navigation: search, open_page, and find_in_page.";

/// Builds the web tool manifest and external JSON schema.
pub(crate) fn web_manifest() -> ToolManifest {
    ToolManifest::new(
        WEB_SEARCH_TOOL_NAME,
        WEB_SEARCH_TOOL_DESCRIPTION,
        serde_json::json!({
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

/// Extracts the human-readable action detail from raw web tool arguments.
pub(crate) fn web_action_detail(arguments: &Value) -> Option<String> {
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
        "find_in_page" => web_find_parts(arguments).map(|parts| match parts {
            WebFindParts::PatternAndUrl { pattern, url } => format!("'{pattern}' in {url}"),
            WebFindParts::Pattern(pattern) => format!("'{pattern}'"),
            WebFindParts::Url(url) => url,
        }),
        _ => None,
    }
}

/// Formats returned web data as visible line-limited command output.
pub(crate) fn web_output_display(output: &Value) -> Option<String> {
    if let Some(error) = output.get("error").and_then(Value::as_str) {
        return Some(error.to_owned());
    }

    match output.get("action").and_then(Value::as_str)? {
        "search" => Some(preview_lines(web_search_result_lines(output))),
        "open_page" => output
            .get("page")
            .and_then(|page| page.get("text"))
            .and_then(Value::as_str)
            .map(preview_text),
        "find_in_page" => Some(preview_lines(web_find_match_lines(output))),
        _ => None,
    }
}

/// Extracts search result titles, URLs, and snippets as compact preview lines.
fn web_search_result_lines(output: &Value) -> Vec<String> {
    output
        .get("results")
        .and_then(Value::as_array)
        .map(|results| results.iter().map(web_search_result_line).collect())
        .unwrap_or_default()
}

/// Formats one search result into a readable single-line summary.
fn web_search_result_line(result: &Value) -> String {
    let title = result.get("title").and_then(Value::as_str).unwrap_or("");
    let url = result.get("url").and_then(Value::as_str).unwrap_or("");
    let snippet = result.get("snippet").and_then(Value::as_str).unwrap_or("");
    [title, url, snippet]
        .into_iter()
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join(" - ")
}

/// Extracts find-in-page matches as `line: text` preview lines.
fn web_find_match_lines(output: &Value) -> Vec<String> {
    output
        .get("matches")
        .and_then(Value::as_array)
        .map(|matches| matches.iter().map(web_find_match_line).collect())
        .unwrap_or_default()
}

/// Formats a single find-in-page match.
fn web_find_match_line(matched: &Value) -> String {
    let line = matched.get("line").and_then(Value::as_u64).unwrap_or(0);
    let text = matched.get("text").and_then(Value::as_str).unwrap_or("");
    format!("{line}: {text}")
}

/// Formats web navigation arguments into a fully styled tool-call line.
pub(crate) fn web_action_call_display(arguments: &Value) -> ToolDisplay {
    match arguments.get("action").and_then(Value::as_str) {
        Some("search") => tool_arg_line(
            "Search web",
            &non_empty_string(arguments, "query").unwrap_or_else(|| "<missing query>".to_owned()),
        ),
        Some("open_page") => tool_arg_line(
            "Open",
            &non_empty_string(arguments, "url").unwrap_or_else(|| "<missing url>".to_owned()),
        ),
        Some("find_in_page") => web_find_call_display(arguments),
        _ => tool_arg_line("web", "<missing web action>"),
    }
}

/// Formats find-in-page arguments with the page URL as a secondary styled argument when present.
fn web_find_call_display(arguments: &Value) -> ToolDisplay {
    match web_find_parts(arguments) {
        Some(WebFindParts::PatternAndUrl { pattern, url }) => {
            tool_arg_tool_arg_line("Find", &format!("'{pattern}'"), "in", &url)
        }
        Some(WebFindParts::Pattern(pattern)) => tool_arg_line("Find", &format!("'{pattern}'")),
        Some(WebFindParts::Url(url)) => tool_arg_line("Find", &url),
        None => tool_arg_line("Find", "<missing pattern>"),
    }
}

/// Extracts non-empty pattern and URL combinations for find-in-page displays.
fn web_find_parts(arguments: &Value) -> Option<WebFindParts> {
    let pattern = non_empty_string(arguments, "pattern");
    let url = non_empty_string(arguments, "url");
    match (pattern, url) {
        (Some(pattern), Some(url)) => Some(WebFindParts::PatternAndUrl { pattern, url }),
        (Some(pattern), None) => Some(WebFindParts::Pattern(pattern)),
        (None, Some(url)) => Some(WebFindParts::Url(url)),
        (None, None) => None,
    }
}

/// Returns a non-empty string field from a JSON object.
fn non_empty_string(arguments: &Value, field: &str) -> Option<String> {
    arguments
        .get(field)
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
}

/// Captures the available fields for a find-in-page web action.
enum WebFindParts {
    PatternAndUrl { pattern: String, url: String },
    Pattern(String),
    Url(String),
}
