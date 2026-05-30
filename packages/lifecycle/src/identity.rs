use std::error::Error;
use std::fmt::{self, Display};
use std::str::FromStr;

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct WorkerId(String);

impl WorkerId {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Display for WorkerId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for WorkerId {
    type Err = IdParseError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        parse_non_blank(value, "worker id").map(Self)
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct RequestId(String);

impl RequestId {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Display for RequestId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for RequestId {
    type Err = IdParseError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        parse_non_blank(value, "request id").map(Self)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IdParseError {
    kind: &'static str,
}

impl Display for IdParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} must not be blank", self.kind)
    }
}

impl Error for IdParseError {}

fn parse_non_blank(value: &str, kind: &'static str) -> Result<String, IdParseError> {
    let trimmed = value.trim();

    if trimmed.is_empty() {
        Err(IdParseError { kind })
    } else {
        Ok(trimmed.to_owned())
    }
}
