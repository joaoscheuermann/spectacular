/// Persisted configuration for one named provider.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ProviderConfig {
    pub provider_type: String,
    pub credentials: Option<ProviderCredentials>,
}

/// Focused credential payload for provider authentication.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProviderCredentials {
    ApiKey { apikey: String },
    ChatGptOauth(ChatGptAuthConfig),
}

impl ProviderConfig {
    /// Creates a provider backed by API-key credentials.
    pub fn new(provider_type: impl Into<String>, apikey: impl Into<String>) -> Self {
        Self {
            provider_type: provider_type.into(),
            credentials: Some(ProviderCredentials::ApiKey {
                apikey: apikey.into(),
            }),
        }
    }

    /// Creates an OpenAI provider backed by ChatGPT OAuth credentials.
    pub fn oauth(provider_type: impl Into<String>, auth: ChatGptAuthConfig) -> Self {
        Self {
            provider_type: provider_type.into(),
            credentials: Some(ProviderCredentials::ChatGptOauth(auth)),
        }
    }

    /// Returns whether the provider has either API-key or OAuth credentials.
    pub fn has_credentials(&self) -> bool {
        match self.credentials.as_ref() {
            Some(ProviderCredentials::ApiKey { apikey }) => {
                required_text(Some(apikey.as_str())).is_some()
            }
            Some(ProviderCredentials::ChatGptOauth(auth)) => has_oauth_credentials(auth),
            None => false,
        }
    }

    /// Returns the effective auth mode for persisted credentials.
    pub fn auth_mode(&self) -> Option<ProviderAuthMode> {
        self.credentials.as_ref().map(|credentials| match credentials {
            ProviderCredentials::ApiKey { .. } => ProviderAuthMode::ApiKey,
            ProviderCredentials::ChatGptOauth(_) => ProviderAuthMode::Oauth,
        })
    }

    /// Returns the configured API key, or an empty string for non-API-key auth.
    pub fn api_key(&self) -> &str {
        match self.credentials.as_ref() {
            Some(ProviderCredentials::ApiKey { apikey }) => apikey,
            _ => "",
        }
    }

    /// Returns OAuth credentials when this provider is configured for OAuth.
    pub fn oauth_config(&self) -> Option<ChatGptAuthConfig> {
        match self.credentials.as_ref() {
            Some(ProviderCredentials::ChatGptOauth(auth)) => Some(auth.clone()),
            _ => None,
        }
    }

    /// Replaces any current credentials with API-key credentials.
    pub fn replace_with_api_key(
        &mut self,
        provider_type: impl Into<String>,
        apikey: impl Into<String>,
    ) {
        *self = Self::new(provider_type, apikey);
    }

    /// Replaces any current credentials with OAuth credentials.
    pub fn replace_with_oauth(
        &mut self,
        provider_type: impl Into<String>,
        auth: ChatGptAuthConfig,
    ) {
        *self = Self::oauth(provider_type, auth);
    }
}

impl Serialize for ProviderConfig {
    /// Serializes focused credentials into the flat config file format.
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        provider_config_serialize_wire(self).serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for ProviderConfig {
    /// Deserializes the current flat auth shape while accepting the previous nested OAuth shape.
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wire = ProviderConfigWire::deserialize(deserializer)?;
        Ok(provider_config_from_wire(wire))
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ProviderAuthMode {
    ApiKey,
    Oauth,
}

#[derive(Default, Serialize)]
#[serde(default, deny_unknown_fields)]
struct ProviderConfigSerializeWire {
    #[serde(rename = "type")]
    provider_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    auth: Option<ProviderAuthMode>,
    #[serde(skip_serializing_if = "String::is_empty")]
    apikey: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    id_token: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    access_token: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    refresh_token: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    account_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    email: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    plan_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    user_id: Option<String>,
    #[serde(skip_serializing_if = "is_false")]
    fedramp: bool,
    #[serde(skip_serializing_if = "is_zero")]
    last_refresh_epoch_seconds: u64,
}

#[derive(Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct ProviderConfigWire {
    #[serde(rename = "type")]
    provider_type: String,
    auth: Option<ProviderAuthWire>,
    apikey: String,
    id_token: String,
    access_token: String,
    refresh_token: String,
    account_id: Option<String>,
    email: Option<String>,
    plan_type: Option<String>,
    user_id: Option<String>,
    fedramp: bool,
    last_refresh_epoch_seconds: u64,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum ProviderAuthWire {
    Mode(ProviderAuthMode),
    Legacy(LegacyProviderAuthConfig),
}

/// Previous inline provider authentication shape accepted only for reading old config files.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(tag = "mode", rename_all = "lowercase", deny_unknown_fields)]
enum LegacyProviderAuthConfig {
    Chatgpt(ChatGptAuthConfig),
}

/// ChatGPT OAuth credentials produced by the browser login flow.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct ChatGptAuthConfig {
    pub id_token: String,
    pub access_token: String,
    pub refresh_token: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub account_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plan_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_id: Option<String>,
    #[serde(skip_serializing_if = "is_false")]
    pub fedramp: bool,
    #[serde(skip_serializing_if = "is_zero")]
    pub last_refresh_epoch_seconds: u64,
}

impl Default for ChatGptAuthConfig {
    /// Creates an empty auth record for deserialization defaults.
    fn default() -> Self {
        Self {
            id_token: String::new(),
            access_token: String::new(),
            refresh_token: String::new(),
            account_id: None,
            email: None,
            plan_type: None,
            user_id: None,
            fedramp: false,
            last_refresh_epoch_seconds: 0,
        }
    }
}

/// Builds the flat serialization wire shape from focused provider credentials.
fn provider_config_serialize_wire(provider: &ProviderConfig) -> ProviderConfigSerializeWire {
    let mut wire = ProviderConfigSerializeWire {
        provider_type: provider.provider_type.clone(),
        ..ProviderConfigSerializeWire::default()
    };

    match provider.credentials.as_ref() {
        Some(ProviderCredentials::ApiKey { apikey }) => {
            wire.auth = Some(ProviderAuthMode::ApiKey);
            wire.apikey = apikey.clone();
        }
        Some(ProviderCredentials::ChatGptOauth(auth)) => {
            wire.auth = Some(ProviderAuthMode::Oauth);
            apply_oauth_to_wire(&mut wire, auth);
        }
        None => {}
    }

    wire
}

/// Builds a focused provider config from the flat or legacy wire shape.
fn provider_config_from_wire(wire: ProviderConfigWire) -> ProviderConfig {
    let ProviderConfigWire {
        provider_type,
        auth,
        apikey,
        id_token,
        access_token,
        refresh_token,
        account_id,
        email,
        plan_type,
        user_id,
        fedramp,
        last_refresh_epoch_seconds,
    } = wire;
    let oauth = ChatGptAuthConfig {
        id_token,
        access_token,
        refresh_token,
        account_id,
        email,
        plan_type,
        user_id,
        fedramp,
        last_refresh_epoch_seconds,
    };
    let credentials = match auth {
        Some(ProviderAuthWire::Mode(ProviderAuthMode::ApiKey)) => {
            Some(ProviderCredentials::ApiKey { apikey })
        }
        Some(ProviderAuthWire::Mode(ProviderAuthMode::Oauth)) => {
            Some(ProviderCredentials::ChatGptOauth(oauth))
        }
        Some(ProviderAuthWire::Legacy(LegacyProviderAuthConfig::Chatgpt(auth))) => {
            Some(ProviderCredentials::ChatGptOauth(auth))
        }
        None => infer_legacy_credentials(apikey, oauth),
    };

    ProviderConfig {
        provider_type,
        credentials,
    }
}

/// Infers credentials from legacy config files that did not persist an auth mode.
fn infer_legacy_credentials(
    apikey: String,
    oauth: ChatGptAuthConfig,
) -> Option<ProviderCredentials> {
    if required_text(Some(apikey.as_str())).is_some() {
        return Some(ProviderCredentials::ApiKey { apikey });
    }
    if has_oauth_credentials(&oauth) {
        return Some(ProviderCredentials::ChatGptOauth(oauth));
    }

    None
}

/// Copies OAuth fields into the flat serialization wire shape.
fn apply_oauth_to_wire(wire: &mut ProviderConfigSerializeWire, auth: &ChatGptAuthConfig) {
    wire.id_token = auth.id_token.clone();
    wire.access_token = auth.access_token.clone();
    wire.refresh_token = auth.refresh_token.clone();
    wire.account_id = auth.account_id.clone();
    wire.email = auth.email.clone();
    wire.plan_type = auth.plan_type.clone();
    wire.user_id = auth.user_id.clone();
    wire.fedramp = auth.fedramp;
    wire.last_refresh_epoch_seconds = auth.last_refresh_epoch_seconds;
}

/// Returns whether OAuth access and refresh tokens are both present.
fn has_oauth_credentials(auth: &ChatGptAuthConfig) -> bool {
    required_text(Some(auth.access_token.as_str())).is_some()
        && required_text(Some(auth.refresh_token.as_str())).is_some()
}

/// Returns true when a boolean is false so serde can skip default fields.
fn is_false(value: &bool) -> bool {
    !*value
}

/// Returns true when a numeric value is zero so serde can skip default fields.
fn is_zero(value: &u64) -> bool {
    *value == 0
}
