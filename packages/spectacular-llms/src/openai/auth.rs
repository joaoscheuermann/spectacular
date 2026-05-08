use super::client::{OpenAiHttpClient, OpenAiTokenResponse};
use super::constants::CLIENT_ID;
use crate::ProviderError;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use rand::RngCore;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::mpsc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const AUTHORIZE_URL: &str = "https://auth.openai.com/oauth/authorize";
const CALLBACK_PATH: &str = "/auth/callback";
const CALLBACK_PORTS: [u16; 2] = [1455, 1457];
const LOGIN_TIMEOUT: Duration = Duration::from_secs(15 * 60);
const REFRESH_INTERVAL_SECONDS: u64 = 8 * 24 * 60 * 60;
const EXPIRY_SKEW_SECONDS: u64 = 60;

/// Stored ChatGPT OAuth token data used by the OpenAI Codex backend.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OpenAiAuthRecord {
    pub id_token: String,
    pub access_token: String,
    pub refresh_token: String,
    pub account_id: Option<String>,
    pub email: Option<String>,
    pub plan_type: Option<String>,
    pub user_id: Option<String>,
    pub fedramp: bool,
    pub last_refresh_epoch_seconds: u64,
}

impl OpenAiAuthRecord {
    /// Builds an auth record from token exchange data and parsed id-token claims.
    pub fn from_token_response(
        response: OpenAiTokenResponse,
        now: u64,
    ) -> Result<Self, ProviderError> {
        let claims = parse_openai_id_token(&response.id_token)?;
        Ok(Self {
            id_token: response.id_token,
            access_token: response.access_token,
            refresh_token: response.refresh_token,
            account_id: claims.account_id,
            email: claims.email,
            plan_type: claims.plan_type,
            user_id: claims.user_id,
            fedramp: claims.fedramp,
            last_refresh_epoch_seconds: now,
        })
    }

    /// Returns whether the access token should be refreshed before use.
    pub fn should_refresh(&self, now: u64) -> bool {
        if access_token_expired(&self.access_token, now) {
            return true;
        }

        now.saturating_sub(self.last_refresh_epoch_seconds) >= REFRESH_INTERVAL_SECONDS
    }

    /// Merges replacement token fields from a refresh response into this record.
    pub fn refreshed(
        mut self,
        response: OpenAiTokenResponse,
        now: u64,
    ) -> Result<Self, ProviderError> {
        if !response.id_token.trim().is_empty() {
            let claims = parse_openai_id_token(&response.id_token)?;
            self.id_token = response.id_token;
            self.account_id = claims.account_id;
            self.email = claims.email;
            self.plan_type = claims.plan_type;
            self.user_id = claims.user_id;
            self.fedramp = claims.fedramp;
        }
        if !response.access_token.trim().is_empty() {
            self.access_token = response.access_token;
        }
        if !response.refresh_token.trim().is_empty() {
            self.refresh_token = response.refresh_token;
        }
        self.last_refresh_epoch_seconds = now;
        Ok(self)
    }
}

/// Persistence contract used by the OpenAI provider to load and save auth.
pub trait OpenAiAuthStore: Send + Sync {
    /// Loads persisted ChatGPT OAuth credentials for an OpenAI provider call.
    fn load_openai_auth(&self) -> Result<OpenAiAuthRecord, ProviderError>;

    /// Saves refreshed ChatGPT OAuth credentials after token renewal.
    fn save_openai_auth(&self, auth: OpenAiAuthRecord) -> Result<(), ProviderError>;
}

/// Browser OAuth flow state after the callback server has started.
pub struct OpenAiBrowserAuthFlow {
    authorize_url: String,
    redirect_uri: String,
    code_verifier: String,
    receiver: mpsc::Receiver<Result<String, ProviderError>>,
    client: OpenAiHttpClient,
}

impl OpenAiBrowserAuthFlow {
    /// Starts the localhost callback server and returns the authorization URL.
    pub(crate) fn start(client: OpenAiHttpClient) -> Result<Self, ProviderError> {
        let callback = bind_callback_listener()?;
        let state = random_urlsafe_token(32);
        let code_verifier = random_urlsafe_token(64);
        let code_challenge = pkce_challenge(&code_verifier);
        let redirect_uri = format!("http://localhost:{}{CALLBACK_PATH}", callback.port);
        let authorize_url = authorize_url(&redirect_uri, &state, &code_challenge);
        let receiver = spawn_callback_handler(callback.listener, state);

        Ok(Self {
            authorize_url,
            redirect_uri,
            code_verifier,
            receiver,
            client,
        })
    }

    /// Returns the browser URL the user must open to approve ChatGPT auth.
    pub fn authorize_url(&self) -> &str {
        &self.authorize_url
    }

    /// Waits for the OAuth callback and exchanges its code for tokens.
    pub async fn finish(self) -> Result<OpenAiAuthRecord, ProviderError> {
        let code = tokio::task::spawn_blocking(move || {
            self.receiver
                .recv_timeout(LOGIN_TIMEOUT)
                .map_err(|_| openai_auth_error("timed out waiting for browser callback"))?
        })
        .await
        .map_err(|error| openai_auth_error(format!("callback worker failed: {error}")))??;
        let response = self
            .client
            .exchange_authorization_code(&code, &self.redirect_uri, &self.code_verifier)
            .await?;
        OpenAiAuthRecord::from_token_response(response, unix_timestamp())
    }
}

struct CallbackListener {
    listener: TcpListener,
    port: u16,
}

struct IdTokenClaims {
    account_id: Option<String>,
    email: Option<String>,
    plan_type: Option<String>,
    user_id: Option<String>,
    fedramp: bool,
}

/// Refreshes an auth record and returns the updated value.
pub async fn refresh_openai_auth(
    client: &OpenAiHttpClient,
    auth: OpenAiAuthRecord,
) -> Result<OpenAiAuthRecord, ProviderError> {
    let response = client.refresh_tokens(&auth.refresh_token).await?;
    auth.refreshed(response, unix_timestamp())
}

/// Returns the current Unix timestamp in seconds.
pub fn unix_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

/// Creates an OpenAI authentication error.
pub fn openai_auth_error(reason: impl Into<String>) -> ProviderError {
    ProviderError::AuthenticationFailed {
        provider_name: "OpenAI".to_owned(),
        reason: reason.into(),
    }
}

/// Attempts to open the authorization URL in the platform browser.
pub fn open_browser(url: &str) -> Result<(), ProviderError> {
    #[cfg(windows)]
    let status = std::process::Command::new("rundll32")
        .args(["url.dll,FileProtocolHandler", url])
        .status();

    #[cfg(target_os = "macos")]
    let status = std::process::Command::new("open").arg(url).status();

    #[cfg(all(unix, not(target_os = "macos")))]
    let status = std::process::Command::new("xdg-open").arg(url).status();

    status
        .map_err(|error| openai_auth_error(format!("could not open browser: {error}")))
        .and_then(|status| {
            if status.success() {
                return Ok(());
            }

            Err(openai_auth_error("browser command exited unsuccessfully"))
        })
}

/// Parses the ChatGPT account metadata from an id-token JWT without validating its signature.
fn parse_openai_id_token(id_token: &str) -> Result<IdTokenClaims, ProviderError> {
    let payload = id_token
        .split('.')
        .nth(1)
        .ok_or_else(|| openai_auth_error("id_token is not a JWT"))?;
    let payload = URL_SAFE_NO_PAD.decode(payload).map_err(|error| {
        openai_auth_error(format!("id_token payload could not be decoded: {error}"))
    })?;
    let value = serde_json::from_slice::<Value>(&payload)
        .map_err(|error| openai_auth_error(format!("id_token payload is not JSON: {error}")))?;

    Ok(IdTokenClaims {
        account_id: claim_string(&value, "chatgpt_account_id"),
        email: claim_string(&value, "email"),
        plan_type: claim_string(&value, "chatgpt_plan_type"),
        user_id: claim_string(&value, "chatgpt_user_id"),
        fedramp: value
            .get("chatgpt_account_is_fedramp")
            .and_then(Value::as_bool)
            .unwrap_or(false),
    })
}

/// Returns whether a JWT-style access token is already expired.
fn access_token_expired(access_token: &str, now: u64) -> bool {
    let Some(payload) = access_token.split('.').nth(1) else {
        return false;
    };
    let Ok(payload) = URL_SAFE_NO_PAD.decode(payload) else {
        return false;
    };
    let Ok(value) = serde_json::from_slice::<Value>(&payload) else {
        return false;
    };
    let Some(exp) = value.get("exp").and_then(Value::as_u64) else {
        return false;
    };

    exp <= now.saturating_add(EXPIRY_SKEW_SECONDS)
}

/// Reads an optional string claim from an id-token payload.
fn claim_string(value: &Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .map(str::to_owned)
}

/// Binds the first available registered callback port.
fn bind_callback_listener() -> Result<CallbackListener, ProviderError> {
    for port in CALLBACK_PORTS {
        match TcpListener::bind(("127.0.0.1", port)) {
            Ok(listener) => return Ok(CallbackListener { listener, port }),
            Err(_) => continue,
        }
    }

    Err(openai_auth_error(
        "could not bind localhost OAuth callback port",
    ))
}

/// Spawns a blocking localhost callback handler.
fn spawn_callback_handler(
    listener: TcpListener,
    expected_state: String,
) -> mpsc::Receiver<Result<String, ProviderError>> {
    let (sender, receiver) = mpsc::channel();
    std::thread::spawn(move || {
        let result = handle_callback(listener, &expected_state);
        let _ = sender.send(result);
    });
    receiver
}

/// Handles one OAuth callback request and returns its authorization code.
fn handle_callback(listener: TcpListener, expected_state: &str) -> Result<String, ProviderError> {
    let (mut stream, _) = listener
        .accept()
        .map_err(|error| openai_auth_error(format!("callback accept failed: {error}")))?;
    let mut buffer = [0u8; 8192];
    let read = stream
        .read(&mut buffer)
        .map_err(|error| openai_auth_error(format!("callback read failed: {error}")))?;
    let request = String::from_utf8_lossy(&buffer[..read]);
    let result = parse_callback_request(&request, expected_state);
    let _ = write_callback_response(&mut stream, result.is_ok());
    result
}

/// Parses the callback request line and validates state.
fn parse_callback_request(request: &str, expected_state: &str) -> Result<String, ProviderError> {
    let first_line = request
        .lines()
        .next()
        .ok_or_else(|| openai_auth_error("callback request was empty"))?;
    let path = first_line
        .split_whitespace()
        .nth(1)
        .ok_or_else(|| openai_auth_error("callback request omitted a path"))?;
    let (path, query) = path.split_once('?').unwrap_or((path, ""));
    if path != CALLBACK_PATH {
        return Err(openai_auth_error("callback path was not /auth/callback"));
    }
    let fields = parse_query(query);
    if let Some(error) = fields.get("error") {
        return Err(openai_auth_error(format!("OAuth returned error `{error}`")));
    }
    if fields.get("state").map(String::as_str) != Some(expected_state) {
        return Err(openai_auth_error("OAuth state did not match"));
    }

    fields
        .get("code")
        .filter(|code| !code.trim().is_empty())
        .cloned()
        .ok_or_else(|| openai_auth_error("OAuth callback omitted authorization code"))
}

/// Writes a small browser response after the local callback is received.
fn write_callback_response(stream: &mut impl Write, success: bool) -> std::io::Result<()> {
    let body = if success {
        "Spectacular is signed in. You can return to the terminal."
    } else {
        "Spectacular could not complete sign-in. Return to the terminal for details."
    };
    write!(
        stream,
        "HTTP/1.1 200 OK\r\ncontent-type: text/plain\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}",
        body.len(),
        body
    )
}

/// Parses URL query parameters from the local callback path.
fn parse_query(query: &str) -> std::collections::BTreeMap<String, String> {
    query
        .split('&')
        .filter(|part| !part.is_empty())
        .filter_map(|part| {
            let (key, value) = part.split_once('=').unwrap_or((part, ""));
            Some((percent_decode(key)?, percent_decode(value)?))
        })
        .collect()
}

/// Builds the OpenAI OAuth authorization URL.
fn authorize_url(redirect_uri: &str, state: &str, code_challenge: &str) -> String {
    let fields = [
        ("response_type", "code"),
        ("client_id", CLIENT_ID),
        ("redirect_uri", redirect_uri),
        (
            "scope",
            "openid profile email offline_access api.connectors.read api.connectors.invoke",
        ),
        ("code_challenge", code_challenge),
        ("code_challenge_method", "S256"),
        ("id_token_add_organizations", "true"),
        ("codex_cli_simplified_flow", "true"),
        ("state", state),
        ("originator", "codex_cli_rs"),
    ];
    let query = fields
        .into_iter()
        .map(|(key, value)| format!("{}={}", percent_encode(key), percent_encode(value)))
        .collect::<Vec<_>>()
        .join("&");
    format!("{AUTHORIZE_URL}?{query}")
}

/// Creates a PKCE code challenge from a verifier.
fn pkce_challenge(verifier: &str) -> String {
    let digest = Sha256::digest(verifier.as_bytes());
    URL_SAFE_NO_PAD.encode(digest)
}

/// Generates a random URL-safe OAuth token.
fn random_urlsafe_token(byte_len: usize) -> String {
    let mut bytes = vec![0u8; byte_len];
    rand::rng().fill_bytes(&mut bytes);
    URL_SAFE_NO_PAD.encode(bytes)
}

/// Percent-encodes a query parameter value.
fn percent_encode(value: &str) -> String {
    value
        .bytes()
        .flat_map(|byte| match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                vec![byte as char]
            }
            _ => format!("%{byte:02X}").chars().collect(),
        })
        .collect()
}

/// Percent-decodes a query parameter value.
fn percent_decode(value: &str) -> Option<String> {
    let mut bytes = Vec::new();
    let mut chars = value.as_bytes().iter().copied();
    while let Some(byte) = chars.next() {
        if byte == b'+' {
            bytes.push(b' ');
            continue;
        }
        if byte != b'%' {
            bytes.push(byte);
            continue;
        }
        let high = chars.next()?;
        let low = chars.next()?;
        let hex = [high, low];
        let text = std::str::from_utf8(&hex).ok()?;
        bytes.push(u8::from_str_radix(text, 16).ok()?);
    }

    String::from_utf8(bytes).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/unit/openai_auth.rs"
    ));
}
