use serde::{Deserialize, Serialize};

/// Supported web navigation actions accepted by the web tool.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum WebAction {
    Search,
    OpenPage,
    FindInPage,
}

impl WebAction {
    /// Returns the external action name used in JSON output payloads.
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Search => "search",
            Self::OpenPage => "open_page",
            Self::FindInPage => "find_in_page",
        }
    }
}

/// Input payload accepted by the external web tool JSON schema.
#[derive(Debug, Deserialize)]
pub(crate) struct WebInput {
    pub(crate) action: WebAction,
    #[serde(default)]
    pub(crate) query: Option<String>,
    #[serde(default)]
    pub(crate) url: Option<String>,
    #[serde(default)]
    pub(crate) pattern: Option<String>,
    #[serde(default, rename = "ignoreCase")]
    pub(crate) ignore_case: Option<bool>,
    #[serde(default)]
    pub(crate) limit: Option<usize>,
    #[serde(default, rename = "maxChars")]
    pub(crate) max_chars: Option<usize>,
}

/// One search result returned by the web search action.
#[derive(Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct WebSearchResult {
    /// Human-readable result title.
    pub title: String,
    /// Normalized result URL.
    pub url: String,
    /// Short text snippet returned by search parsing.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub snippet: String,
}

/// Extracted page content returned by the open-page action.
#[derive(Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct WebPageOutput {
    /// Normalized page URL.
    pub url: String,
    /// Extracted document title.
    pub title: String,
    /// Visible page text after script/style removal and truncation.
    pub text: String,
}

/// One literal line match returned by the find-in-page action.
#[derive(Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct WebFindMatch {
    /// One-based page-text line number.
    pub line: usize,
    /// Matching line text.
    pub text: String,
}

/// Common web tool output envelope for search, page-open, find, and error payloads.
#[derive(Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct WebOutput {
    /// Action name that produced this payload.
    pub action: String,
    /// Human-readable query, URL, or find target.
    pub detail: String,
    /// Search results for search actions.
    pub results: Vec<WebSearchResult>,
    /// Extracted page data for open-page actions.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page: Option<WebPageOutput>,
    /// Literal line matches for find-in-page actions.
    pub matches: Vec<WebFindMatch>,
    /// Total results or matches found before truncation.
    pub total: usize,
    /// Whether limits omitted additional result data.
    pub truncated: bool,
    /// User-facing failure message for invalid input or fetch errors.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Builds a consistent error payload for any web action.
pub(crate) fn web_error(action: impl Into<String>, message: impl Into<String>) -> WebOutput {
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

/// Serializes a web output payload to JSON for tool responses.
pub(crate) fn serialize_output(output: &WebOutput) -> String {
    serde_json::to_string(output).expect("web output should serialize")
}
