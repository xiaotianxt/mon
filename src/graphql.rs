use std::error::Error;
use std::fmt;

use anyhow::Context;
use anyhow::Result;
use serde::Deserialize;
use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GraphqlErrorClass {
    SchemaIncompatible,
    DefinitiveRejection,
    Ambiguous,
}

#[derive(Debug, Clone, Deserialize)]
struct GraphqlErrorItem {
    message: String,
    #[serde(default)]
    extensions: Option<GraphqlErrorExtensions>,
}

#[derive(Debug, Clone, Deserialize)]
struct GraphqlErrorExtensions {
    #[serde(default)]
    code: Option<String>,
}

#[derive(Debug)]
pub struct GraphqlResponseError {
    operation: String,
    http_status: Option<u16>,
    errors: Vec<GraphqlErrorItem>,
}

impl GraphqlResponseError {
    pub fn class(&self) -> GraphqlErrorClass {
        let codes = self
            .errors
            .iter()
            .filter_map(|error| {
                error
                    .extensions
                    .as_ref()
                    .and_then(|extensions| extensions.code.as_deref())
            })
            .collect::<Vec<_>>();

        if !codes.is_empty()
            && codes
                .iter()
                .all(|code| matches!(*code, "GRAPHQL_VALIDATION_FAILED" | "GRAPHQL_PARSE_FAILED"))
        {
            return GraphqlErrorClass::SchemaIncompatible;
        }

        if !codes.is_empty() && codes.iter().all(|code| *code == "BAD_USER_INPUT") {
            return GraphqlErrorClass::DefinitiveRejection;
        }

        // Monarch currently omits extension codes for document validation
        // failures and returns HTTP 400 with a generic GraphQL error. Treat a
        // GraphQL envelope at that status as an incompatible operation.
        if self.http_status == Some(400) {
            return GraphqlErrorClass::SchemaIncompatible;
        }

        GraphqlErrorClass::Ambiguous
    }
}

impl fmt::Display for GraphqlResponseError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let descriptions = self
            .errors
            .iter()
            .map(|error| {
                let code = error
                    .extensions
                    .as_ref()
                    .and_then(|extensions| extensions.code.as_deref());

                match code {
                    Some(code) => format!("[{code}] {}", error.message),
                    None => error.message.clone(),
                }
            })
            .collect::<Vec<_>>()
            .join("; ");

        match self.class() {
            GraphqlErrorClass::SchemaIncompatible => write!(
                formatter,
                "Monarch schema is incompatible with GraphQL operation {}: {}",
                self.operation, descriptions
            ),
            _ => write!(
                formatter,
                "GraphQL {} returned errors: {}",
                self.operation, descriptions
            ),
        }
    }
}

impl Error for GraphqlResponseError {}

pub fn response_error(
    operation: &str,
    http_status: Option<u16>,
    response: &Value,
) -> Result<Option<GraphqlResponseError>> {
    let Some(raw_errors) = response.get("errors") else {
        return Ok(None);
    };

    if raw_errors.is_null() {
        anyhow::bail!("GraphQL {operation} response contained null errors");
    }

    let errors: Vec<GraphqlErrorItem> = serde_json::from_value(raw_errors.clone())
        .with_context(|| format!("failed to parse GraphQL errors for {operation}"))?;

    if errors.is_empty() {
        return Ok(None);
    }

    Ok(Some(GraphqlResponseError {
        operation: operation.to_owned(),
        http_status,
        errors,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_schema_validation_errors_by_extension_code() {
        let response = serde_json::json!({
            "errors": [{
                "message": "Cannot query field isDisabled.",
                "extensions": { "code": "GRAPHQL_VALIDATION_FAILED" }
            }]
        });

        let error = response_error("GetCategories", Some(200), &response)
            .unwrap()
            .unwrap();

        assert_eq!(error.class(), GraphqlErrorClass::SchemaIncompatible);
        assert!(error.to_string().contains("schema is incompatible"));
    }

    #[test]
    fn classifies_http_400_graphql_errors_as_schema_incompatible() {
        let response = serde_json::json!({
            "errors": [{
                "message": "Something went wrong while processing.",
                "locations": [{ "line": 1, "column": 34 }]
            }]
        });

        let error = response_error("GetCategories", Some(400), &response)
            .unwrap()
            .unwrap();

        assert_eq!(error.class(), GraphqlErrorClass::SchemaIncompatible);
    }

    #[test]
    fn classifies_bad_user_input_as_definitive_rejection() {
        let response = serde_json::json!({
            "errors": [{
                "message": "Invalid category",
                "extensions": { "code": "BAD_USER_INPUT" }
            }]
        });

        let error = response_error(
            "Web_TransactionDrawerUpdateTransaction",
            Some(200),
            &response,
        )
        .unwrap()
        .unwrap();

        assert_eq!(error.class(), GraphqlErrorClass::DefinitiveRejection);
    }

    #[test]
    fn unknown_top_level_error_codes_remain_ambiguous() {
        let response = serde_json::json!({
            "errors": [{
                "message": "Resolver stopped unexpectedly",
                "extensions": { "code": "INTERNAL_ERROR" }
            }]
        });

        let error = response_error(
            "Web_TransactionDrawerUpdateTransaction",
            Some(200),
            &response,
        )
        .unwrap()
        .unwrap();

        assert_eq!(error.class(), GraphqlErrorClass::Ambiguous);
    }

    #[test]
    fn accepts_an_empty_errors_array() {
        let response = serde_json::json!({ "data": {}, "errors": [] });
        assert!(response_error("Example", Some(200), &response)
            .unwrap()
            .is_none());
    }
}
