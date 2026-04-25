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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn release_schema() -> OutputSchema {
        OutputSchema::new(json!({
            "type": "object",
            "required": ["summary", "status"],
            "properties": {
                "summary": {"type": "string"},
                "status": {"const": "ready"}
            },
            "additionalProperties": false
        }))
        .unwrap()
    }

    #[test]
    fn schema_from_json_str_accepts_valid_schema() {
        let schema = OutputSchema::from_json_str(r#"{"type":"object"}"#);

        assert!(schema.is_ok());
    }

    #[test]
    fn schema_from_json_str_rejects_invalid_json() {
        let error = OutputSchema::from_json_str("{").unwrap_err();

        assert!(error.to_string().contains("EOF"));
    }

    #[test]
    fn schema_new_rejects_invalid_schema_shape() {
        let error = OutputSchema::new(json!({"type": 5})).unwrap_err();

        assert!(error.to_string().contains("not valid"));
    }

    #[test]
    fn validate_response_accepts_matching_json() {
        let schema = release_schema();

        assert!(schema
            .validate_response(r#"{"summary":"done","status":"ready"}"#)
            .is_ok());
    }

    #[test]
    fn validate_response_rejects_invalid_json() {
        let schema = release_schema();

        let error = schema.validate_response("not json").unwrap_err();

        assert!(error.to_string().contains("expected ident"));
    }

    #[test]
    fn validate_response_rejects_schema_mismatch() {
        let schema = release_schema();

        let error = schema
            .validate_response(r#"{"summary":"done","status":"draft"}"#)
            .unwrap_err();

        assert!(error.to_string().contains("ready"));
    }
}
