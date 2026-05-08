use super::auth::OpenAiAuthRecord;
use super::constants::CLIENT_ID;
use super::dto::OpenAiResponsesRequest;
use crate::ProviderError;
use serde::Deserialize;

const TOKEN_URL: &str = "https://auth.openai.com/oauth/token";
pub(crate) const API_RESPONSES_URL: &str = "https://api.openai.com/v1/responses";
pub(crate) const CHATGPT_RESPONSES_URL: &str = "https://chatgpt.com/backend-api/codex/responses";

#[derive(Clone)]
pub(crate) struct OpenAiHttpClient {
    http: reqwest::Client,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
pub struct OpenAiTokenResponse {
    #[serde(default)]
    pub id_token: String,
    #[serde(default)]
    pub access_token: String,
    #[serde(default)]
    pub refresh_token: String,
}

impl OpenAiHttpClient {
    /// Creates a new async HTTP client for OpenAI OAuth and Responses calls.
    pub(crate) fn new() -> Self {
        Self {
            http: reqwest::Client::new(),
        }
    }

    /// Exchanges an OAuth authorization code for ChatGPT tokens.
    pub(crate) async fn exchange_authorization_code(
        &self,
        code: &str,
        redirect_uri: &str,
        code_verifier: &str,
    ) -> Result<OpenAiTokenResponse, ProviderError> {
        let fields = [
            ("grant_type", "authorization_code"),
            ("code", code),
            ("redirect_uri", redirect_uri),
            ("client_id", CLIENT_ID),
            ("code_verifier", code_verifier),
        ];
        self.token_request(&fields).await
    }

    /// Refreshes ChatGPT OAuth tokens using a stored refresh token.
    pub(crate) async fn refresh_tokens(
        &self,
        refresh_token: &str,
    ) -> Result<OpenAiTokenResponse, ProviderError> {
        let fields = [
            ("client_id", CLIENT_ID),
            ("grant_type", "refresh_token"),
            ("refresh_token", refresh_token),
        ];
        self.token_request(&fields).await
    }

    /// Opens a streaming Responses request against the public OpenAI API backend.
    pub(crate) async fn stream_api_response(
        &self,
        api_key: &str,
        body: &OpenAiResponsesRequest,
    ) -> Result<reqwest::Response, ProviderError> {
        self.http
            .post(API_RESPONSES_URL)
            .bearer_auth(api_key)
            .header(reqwest::header::ACCEPT, "text/event-stream")
            .json(body)
            .send()
            .await
            .map_err(openai_network_error)
    }

    /// Opens a streaming Responses request against the ChatGPT Codex backend.
    pub(crate) async fn stream_chatgpt_response(
        &self,
        auth: &OpenAiAuthRecord,
        body: &OpenAiResponsesRequest,
    ) -> Result<reqwest::Response, ProviderError> {
        let mut request = self
            .http
            .post(CHATGPT_RESPONSES_URL)
            .bearer_auth(&auth.access_token)
            .header(reqwest::header::ACCEPT, "text/event-stream")
            .json(body);
        if let Some(account_id) = auth.account_id.as_ref() {
            request = request.header("ChatGPT-Account-ID", account_id);
        }
        if auth.fedramp {
            request = request.header("X-OpenAI-Fedramp", "true");
        }

        request.send().await.map_err(openai_network_error)
    }

    /// Sends a form-encoded request to the OpenAI OAuth token endpoint.
    async fn token_request(
        &self,
        fields: &[(&str, &str)],
    ) -> Result<OpenAiTokenResponse, ProviderError> {
        let response = self
            .http
            .post(TOKEN_URL)
            .form(fields)
            .send()
            .await
            .map_err(openai_network_error)?;
        let status = response.status().as_u16();
        let body = response.text().await.map_err(openai_network_error)?;
        if !(200..300).contains(&status) {
            return Err(ProviderError::AuthenticationFailed {
                provider_name: "OpenAI".to_owned(),
                reason: format!("token endpoint returned status {status}: {body}"),
            });
        }

        serde_json::from_str(&body).map_err(|error| ProviderError::ResponseParsingFailed {
            provider_name: "OpenAI".to_owned(),
            reason: format!("token endpoint returned invalid JSON: {error}; body: {body}"),
        })
    }
}

/// Converts reqwest transport errors into provider network errors.
fn openai_network_error(error: reqwest::Error) -> ProviderError {
    ProviderError::NetworkError {
        provider_name: "OpenAI".to_owned(),
        reason: error.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/unit/openai_client.rs"
    ));
}
