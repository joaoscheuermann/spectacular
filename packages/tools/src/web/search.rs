use regex::Regex;

use super::web_html::clean_html_fragment;
use super::web_model::WebSearchResult;

/// Parses DuckDuckGo HTML result links into normalized web search results.
pub(crate) fn parse_duckduckgo_results(html: &str) -> Vec<WebSearchResult> {
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

/// Converts DuckDuckGo redirect links into direct target URLs when possible.
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
