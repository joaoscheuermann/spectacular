use std::time::Duration;

use spectacular_agent::Cancellation;

const USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36";
const REQUEST_TIMEOUT_SECS: u64 = 20;
const CANCELLATION_POLL_MS: u64 = 50;

/// HTTP dependency for web actions with shared client setup and cancellation polling.
#[derive(Clone, Debug)]
pub(crate) struct WebHttpClient {
    client: Result<reqwest::Client, String>,
    cancellation_poll: Duration,
}

impl WebHttpClient {
    /// Builds the default reqwest client used by web tool actions.
    pub(crate) fn new() -> Self {
        Self {
            client: build_client(),
            cancellation_poll: Duration::from_millis(CANCELLATION_POLL_MS),
        }
    }

    /// Fetches a URL into text with web-tool request settings.
    pub(crate) async fn fetch_url(
        &self,
        url: &str,
        cancellation: Cancellation,
    ) -> Result<String, String> {
        if cancellation.is_cancelled() {
            return Err("Request cancelled".to_owned());
        }

        let client = self.client.as_ref().map_err(Clone::clone)?;
        let response = self
            .send_with_cancellation(client.get(url).send(), cancellation.clone())
            .await?;
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

    /// Waits for a reqwest request while polling cancellation.
    async fn send_with_cancellation(
        &self,
        request: impl std::future::Future<Output = reqwest::Result<reqwest::Response>>,
        cancellation: Cancellation,
    ) -> Result<reqwest::Response, String> {
        tokio::pin!(request);

        loop {
            if cancellation.is_cancelled() {
                return Err("Request cancelled".to_owned());
            }

            tokio::select! {
                result = &mut request => return result.map_err(|error| format!("Request failed: {error}")),
                _ = tokio::time::sleep(self.cancellation_poll) => {}
            }
        }
    }
}

/// Builds the configured HTTP client and preserves construction errors for tool output.
fn build_client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(REQUEST_TIMEOUT_SECS))
        .user_agent(USER_AGENT)
        .build()
        .map_err(|error| format!("Failed to build web client: {error}"))
}
