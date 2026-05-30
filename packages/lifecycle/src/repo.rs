use std::fmt::{self, Display};

use crate::redaction::{redact_repo_url, RedactionError};

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
