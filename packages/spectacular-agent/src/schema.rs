use serde_json::Value;
use std::error::Error;
use std::fmt::{self, Display};

#[derive(Clone, Debug, PartialEq)]
pub struct OutputSchema {
    schema: Value,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SchemaError {
    message: String,
}

impl OutputSchema {
    pub fn new(schema: Value) -> Result<Self, SchemaError> {
        jsonschema::validator_for(&schema).map_err(|error| SchemaError {
            message: error.to_string(),
        })?;
        Ok(Self { schema })
    }

    pub fn from_json_str(schema: &str) -> Result<Self, SchemaError> {
        let value = serde_json::from_str(schema).map_err(|error| SchemaError {
            message: error.to_string(),
        })?;
        Self::new(value)
    }

    pub fn validate_response(&self, response: &str) -> Result<(), SchemaError> {
        let value: Value = serde_json::from_str(response).map_err(|error| SchemaError {
            message: error.to_string(),
        })?;
        let validator = jsonschema::validator_for(&self.schema).map_err(|error| SchemaError {
            message: error.to_string(),
        })?;

        validator.validate(&value).map_err(|error| SchemaError {
            message: error.to_string(),
        })
    }
}

impl Display for SchemaError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl Error for SchemaError {}
