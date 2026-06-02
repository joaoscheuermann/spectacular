use std::error::Error;
use std::fmt::{self, Display};
use std::str::FromStr;

use crate::redaction::{redact_repo_url, RedactionError};

const ALLOWED_SCHEMES: &[&str] = &["https", "http", "ssh", "git"];

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct RepoIdentity(String);

impl RepoIdentity {
    pub fn from_raw_url(raw: &str) -> Result<Self, RedactionError> {
        redact_repo_url(raw).map(Self)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Display for RepoIdentity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct RepoUrl {
    clone_input: String,
    identity: RepoIdentity,
}

impl RepoUrl {
    pub fn as_clone_input(&self) -> &str {
        &self.clone_input
    }

    pub fn identity(&self) -> &RepoIdentity {
        &self.identity
    }

    fn parse(raw: &str) -> Result<Self, RepoUrlError> {
        validate(raw)?;

        Ok(Self {
            clone_input: raw.to_owned(),
            identity: RepoIdentity::from_raw_url(raw).map_err(RepoUrlError::from)?,
        })
    }
}

impl Display for RepoUrl {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.identity.fmt(f)
    }
}

impl FromStr for RepoUrl {
    type Err = RepoUrlError;

    fn from_str(raw: &str) -> Result<Self, Self::Err> {
        Self::parse(raw)
    }
}

impl TryFrom<&str> for RepoUrl {
    type Error = RepoUrlError;

    fn try_from(raw: &str) -> Result<Self, Self::Error> {
        Self::parse(raw)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RepoUrlError {
    Blank,
    LocalOrPathLike,
    MissingHost,
    MissingPath,
    UnsupportedScheme,
}

impl Display for RepoUrlError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Blank => f.write_str("repo URL must not be blank"),
            Self::LocalOrPathLike => f.write_str("repo URL must be a remote URL, not a local path"),
            Self::MissingHost => f.write_str("repo URL host must not be blank"),
            Self::MissingPath => f.write_str("repo URL path must not be blank"),
            Self::UnsupportedScheme => {
                f.write_str("repo URL scheme must be one of https, http, ssh, or git")
            }
        }
    }
}

impl Error for RepoUrlError {}

impl From<RedactionError> for RepoUrlError {
    fn from(error: RedactionError) -> Self {
        match error {
            RedactionError::BlankRepoUrl => Self::Blank,
            RedactionError::MissingHost => Self::MissingHost,
        }
    }
}

fn validate(raw: &str) -> Result<(), RepoUrlError> {
    if raw.trim().is_empty() {
        return Err(RepoUrlError::Blank);
    }

    if raw.trim() != raw || is_local_or_path_like(raw) {
        return Err(RepoUrlError::LocalOrPathLike);
    }

    if let Some(rest) = raw.strip_prefix("git@") {
        return validate_scp_like(rest);
    }

    validate_scheme_remote(raw)
}

fn validate_scheme_remote(raw: &str) -> Result<(), RepoUrlError> {
    let Some(scheme_end) = raw.find("://") else {
        return Err(RepoUrlError::LocalOrPathLike);
    };

    let scheme = &raw[..scheme_end];

    if scheme.eq_ignore_ascii_case("file") {
        return Err(RepoUrlError::LocalOrPathLike);
    }

    if !ALLOWED_SCHEMES
        .iter()
        .any(|allowed| scheme.eq_ignore_ascii_case(allowed))
    {
        return Err(RepoUrlError::UnsupportedScheme);
    }

    let authority_start = scheme_end + 3;
    let after_scheme = &raw[authority_start..];
    let authority_end = after_scheme
        .find(['/', '?', '#'])
        .map_or(raw.len(), |offset| authority_start + offset);
    let authority = &raw[authority_start..authority_end];
    let host = authority
        .rsplit_once('@')
        .map_or(authority, |(_, host)| host);

    validate_host(host)?;
    validate_path(&raw[authority_end..])
}

fn validate_scp_like(rest: &str) -> Result<(), RepoUrlError> {
    let Some((host, path)) = rest.split_once(':') else {
        return Err(RepoUrlError::MissingPath);
    };

    validate_host(host)?;

    if path.is_empty() {
        return Err(RepoUrlError::MissingPath);
    }

    Ok(())
}

fn validate_host(host: &str) -> Result<(), RepoUrlError> {
    let host = if let Some(bracketed) = host.strip_prefix('[') {
        bracketed.split_once(']').map_or("", |(value, _)| value)
    } else {
        host.split_once(':').map_or(host, |(value, _)| value)
    };

    if host.is_empty() || host.starts_with(':') {
        return Err(RepoUrlError::MissingHost);
    }

    Ok(())
}

fn validate_path(rest: &str) -> Result<(), RepoUrlError> {
    if !rest.starts_with('/') {
        return Err(RepoUrlError::MissingPath);
    }

    let path_end = rest[1..]
        .find(['?', '#'])
        .map_or(rest.len(), |offset| offset + 1);
    let path = &rest[1..path_end];

    if path.is_empty() {
        return Err(RepoUrlError::MissingPath);
    }

    Ok(())
}

fn is_local_or_path_like(raw: &str) -> bool {
    raw.starts_with('/')
        || raw.starts_with("\\\\")
        || raw.starts_with("//")
        || raw.starts_with("./")
        || raw.starts_with("../")
        || raw.starts_with(".\\")
        || raw.starts_with("..\\")
        || is_windows_drive_path(raw)
}

fn is_windows_drive_path(raw: &str) -> bool {
    let bytes = raw.as_bytes();

    matches!(
        bytes,
        [letter, b':', ..] if letter.is_ascii_alphabetic()
    )
}
