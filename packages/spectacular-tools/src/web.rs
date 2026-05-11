#[path = "web/display.rs"]
mod web_display;
#[path = "web/find.rs"]
mod web_find;
#[path = "web/html.rs"]
mod web_html;
#[path = "web/http.rs"]
mod web_http;
#[path = "web/model.rs"]
mod web_model;
#[path = "web/search.rs"]
mod web_search;

use serde_json::Value;
use spectacular_agent::{Cancellation, Tool, ToolDisplay, ToolExecution, ToolManifest};
use web_display::{web_action_call_display, web_action_detail, web_manifest, web_output_display};
use web_find::find_in_text;
use web_html::{extract_page_text, extract_title, normalized_url, truncate_text};
use web_http::fetch_url;
use web_model::{serialize_output, web_error, WebAction, WebInput, WebOutput, WebPageOutput};
pub use web_model::{WebFindMatch, WebSearchResult};
use web_search::parse_duckduckgo_results;

pub const WEB_SEARCH_TOOL_NAME: &str = "web";

const DEFAULT_SEARCH_LIMIT: usize = 5;
const MAX_SEARCH_LIMIT: usize = 10;
const DEFAULT_FIND_LIMIT: usize = 20;
const MAX_FIND_LIMIT: usize = 100;
const DEFAULT_MAX_CHARS: usize = 12_000;
const MAX_PAGE_CHARS: usize = 50_000;

#[derive(Clone, Debug, Default)]
pub struct WebSearchTool;

impl Tool for WebSearchTool {
    /// Returns the stable tool name used for agent registration and dispatch.
    fn name(&self) -> &str {
        WEB_SEARCH_TOOL_NAME
    }

    /// Builds the web navigation manifest and external JSON parameter schema.
    fn manifest(&self) -> ToolManifest {
        web_manifest()
    }

    /// Formats web arguments as the concise action detail shown in tool summaries.
    fn format_input(&self, arguments: &Value) -> ToolDisplay {
        web_action_detail(arguments).unwrap_or_else(|| "<missing web action>".to_owned())
    }

    /// Formats web arguments as a styled renderer line.
    fn format_call(&self, arguments: &Value) -> ToolDisplay {
        web_action_call_display(arguments)
    }

    /// Formats structured web output as a bounded text preview.
    fn format_output(&self, raw_output: &str, parsed_output: Option<&Value>) -> ToolDisplay {
        let Some(output) = parsed_output else {
            return crate::output_preview::preview_text(raw_output);
        };

        web_output_display(output)
            .unwrap_or_else(|| crate::output_preview::preview_text(raw_output))
    }

    /// Executes the requested web action and returns a serialized web output payload.
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

/// Routes a validated web input to its action-specific executor.
async fn execute_web(input: WebInput, cancellation: Cancellation) -> WebOutput {
    match input.action {
        WebAction::Search => execute_search(input, cancellation).await,
        WebAction::OpenPage => execute_open_page(input, cancellation).await,
        WebAction::FindInPage => execute_find_in_page(input, cancellation).await,
    }
}

/// Executes a DuckDuckGo HTML search and returns normalized search result payloads.
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

/// Executes an open-page action and returns extracted page text bounded by the maxChars limit.
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

/// Executes a find-in-page action against extracted page text.
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

#[cfg(test)]
mod tests {
    include!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/unit/web.rs"));
}
