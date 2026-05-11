use std::time::Duration;

use spectacular_agent::Cancellation;

const USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36";
const REQUEST_TIMEOUT_SECS: u64 = 20;
const CANCELLATION_POLL_MS: u64 = 50;

/// Fetches a URL into text with the default web-tool client settings and cancellation polling.
pub(crate) async fn fetch_url(url: &str, cancellation: Cancellation) -> Result<String, String> {
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
