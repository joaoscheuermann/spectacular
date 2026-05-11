use super::*;
use serde_json::json;

/// Verifies that display input text mirrors the Codex-style web action detail.
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

/// Verifies DuckDuckGo HTML result parsing and redirect URL normalization.
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

/// Verifies that page extraction returns title text and visible body text without scripts.
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

/// Verifies literal page-text matching with case-insensitive search and result truncation.
#[test]
fn finds_literal_text_in_extracted_page_lines() {
    let (matches, total, truncated) = find_in_text("Alpha\nBeta Needle\nneedle two", "needle", true, 1);

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

/// Verifies URL normalization rejects invalid or non-HTTP schemes and accepts HTTPS URLs.
#[test]
fn rejects_invalid_or_non_http_urls() {
    assert_eq!(normalized_url(Some("file:///tmp/a")), None);
    assert_eq!(normalized_url(Some("not a url")), None);
    assert_eq!(
        normalized_url(Some("https://example.com/path")).as_deref(),
        Some("https://example.com/path")
    );
}
