//! # GraphQL Tool
//!
//! A GraphQL client tool for executing queries and mutations against GraphQL endpoints.
//! This tool allows agents to interact with any GraphQL API.
//!
//! ## Features
//!
//! - Execute GraphQL queries and mutations
//! - Variable support for parameterized queries
//! - Custom headers for authentication (API keys, Bearer tokens, etc.)
//! - Timeout configuration
//! - Error handling with detailed GraphQL errors
//!
//! ## Usage
//!
//! ```rust,no_run
//! use dashflow_graphql::GraphQLTool;
//! use dashflow::core::tools::Tool;
//! use serde_json::json;
//!
//! #[tokio::main]
//! async fn main() {
//!     let tool = GraphQLTool::new("https://api.spacex.land/graphql".to_string());
//!
//!     // Execute a query
//!     let response = tool._call_str(
//!         json!({
//!             "query": "query { company { name founder } }"
//!         }).to_string()
//!     ).await.unwrap();
//!
//!     println!("Response: {}", response);
//!
//!     // Query with variables
//!     let response_with_vars = tool._call_str(
//!         json!({
//!             "query": "query GetCompany($id: String!) { company(id: $id) { name } }",
//!             "variables": {
//!                 "id": "spacex"
//!             }
//!         }).to_string()
//!     ).await.unwrap();
//! }
//! ```

use async_trait::async_trait;
use dashflow::constants::{DEFAULT_HTTP_CONNECT_TIMEOUT, DEFAULT_HTTP_REQUEST_TIMEOUT};
use dashflow::core::error::Error;
use dashflow::core::http_client;
use dashflow::core::tools::{Tool, ToolInput};
use dashflow::core::Result;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::time::Duration;

/// GraphQL request configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphQLRequest {
    /// GraphQL query or mutation
    pub query: String,

    /// Optional variables for the query
    #[serde(default)]
    pub variables: Option<HashMap<String, Value>>,

    /// Optional operation name (for queries with multiple operations)
    #[serde(default)]
    pub operation_name: Option<String>,

    /// Optional custom headers
    #[serde(default)]
    pub headers: HashMap<String, String>,
}

/// GraphQL response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphQLResponse {
    /// Response data (if successful)
    pub data: Option<Value>,

    /// GraphQL errors (if any)
    pub errors: Option<Vec<GraphQLError>>,
}

/// GraphQL error details
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphQLError {
    /// Error message
    pub message: String,

    /// Error locations in the query
    #[serde(default)]
    pub locations: Option<Vec<GraphQLErrorLocation>>,

    /// Error path
    #[serde(default)]
    pub path: Option<Vec<Value>>,

    /// Additional error extensions
    #[serde(default)]
    pub extensions: Option<HashMap<String, Value>>,
}

/// Location of an error in the GraphQL query
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphQLErrorLocation {
    /// Line number
    pub line: u32,

    /// Column number
    pub column: u32,
}

/// GraphQL query and mutation tool
///
/// This tool allows executing GraphQL queries and mutations against any GraphQL endpoint.
/// It supports variables, custom headers for authentication, and provides detailed error information.
pub struct GraphQLTool {
    /// GraphQL endpoint URL
    endpoint: String,

    /// HTTP client
    client: Client,

    /// Default headers to include with every request
    default_headers: HashMap<String, String>,

    /// Request timeout in seconds
    timeout_secs: u64,
}

impl GraphQLTool {
    /// Create a new GraphQL tool
    ///
    /// # Arguments
    ///
    /// * `endpoint` - The GraphQL endpoint URL
    ///
    /// # Example
    ///
    /// ```rust
    /// use dashflow_graphql::GraphQLTool;
    ///
    /// let tool = GraphQLTool::new("https://api.example.com/graphql".to_string());
    /// ```
    #[must_use]
    pub fn new(endpoint: String) -> Self {
        let client = Client::builder()
            .timeout(DEFAULT_HTTP_REQUEST_TIMEOUT)
            .connect_timeout(DEFAULT_HTTP_CONNECT_TIMEOUT)
            .build()
            .unwrap_or_else(|_| Client::new());

        Self {
            endpoint,
            client,
            default_headers: HashMap::new(),
            timeout_secs: 30,
        }
    }

    /// Create a new GraphQL tool with custom headers
    ///
    /// # Arguments
    ///
    /// * `endpoint` - The GraphQL endpoint URL
    /// * `headers` - Default headers to include with every request (e.g., authentication)
    ///
    /// # Example
    ///
    /// ```rust
    /// use dashflow_graphql::GraphQLTool;
    /// use std::collections::HashMap;
    ///
    /// let mut headers = HashMap::new();
    /// headers.insert("Authorization".to_string(), "Bearer token123".to_string());
    /// let tool = GraphQLTool::with_headers("https://api.example.com/graphql".to_string(), headers);
    /// ```
    #[must_use]
    pub fn with_headers(endpoint: String, default_headers: HashMap<String, String>) -> Self {
        let client = Client::builder()
            .timeout(DEFAULT_HTTP_REQUEST_TIMEOUT)
            .connect_timeout(DEFAULT_HTTP_CONNECT_TIMEOUT)
            .build()
            .unwrap_or_else(|_| Client::new());

        Self {
            endpoint,
            client,
            default_headers,
            timeout_secs: 30,
        }
    }

    /// Set the request timeout
    ///
    /// # Arguments
    ///
    /// * `timeout_secs` - Timeout in seconds
    ///
    /// # Example
    ///
    /// ```rust
    /// use dashflow_graphql::GraphQLTool;
    ///
    /// let tool = GraphQLTool::new("https://api.example.com/graphql".to_string())
    ///     .with_timeout(60);
    /// ```
    #[must_use]
    pub fn with_timeout(mut self, timeout_secs: u64) -> Self {
        self.timeout_secs = timeout_secs;
        self.client = Client::builder()
            .timeout(Duration::from_secs(timeout_secs))
            .connect_timeout(DEFAULT_HTTP_CONNECT_TIMEOUT)
            .build()
            .unwrap_or_else(|_| Client::new());
        self
    }

    /// Execute a GraphQL request
    async fn execute(&self, request: GraphQLRequest) -> Result<GraphQLResponse> {
        // Prepare request body
        let mut body = serde_json::json!({
            "query": request.query,
        });

        if let Some(variables) = request.variables {
            body["variables"] = serde_json::to_value(variables)?;
        }

        if let Some(operation_name) = request.operation_name {
            body["operationName"] = serde_json::to_value(operation_name)?;
        }

        // Build HTTP request
        let mut req_builder = self.client.post(&self.endpoint).json(&body);

        // Add default headers
        for (key, value) in &self.default_headers {
            req_builder = req_builder.header(key, value);
        }

        // Add request-specific headers
        for (key, value) in &request.headers {
            req_builder = req_builder.header(key, value);
        }

        // Execute request
        let response = req_builder.send().await?;
        let status = response.status();

        // Parse response with size limit to prevent memory exhaustion
        let response_text =
            http_client::read_text_with_limit(response, http_client::DEFAULT_RESPONSE_SIZE_LIMIT)
                .await?;

        if !status.is_success() {
            return Err(Error::Http(format!(
                "GraphQL request failed with status {status}: {response_text}"
            )));
        }

        let graphql_response: GraphQLResponse = serde_json::from_str(&response_text)?;

        Ok(graphql_response)
    }
}

#[async_trait]
impl Tool for GraphQLTool {
    fn name(&self) -> &'static str {
        "graphql"
    }

    fn description(&self) -> &'static str {
        "Execute GraphQL queries and mutations. \
        Input should be a JSON object with required 'query' field (GraphQL query string), \
        optional 'variables' object (query variables), optional 'operation_name' string \
        (for queries with multiple operations), and optional 'headers' object \
        (custom request headers like Authorization). \
        Returns the GraphQL response data or errors."
    }

    async fn _call(&self, input: ToolInput) -> Result<String> {
        // Parse input based on ToolInput variant
        let request: GraphQLRequest = match input {
            ToolInput::String(s) => {
                // Try to parse as JSON first
                if let Ok(value) = serde_json::from_str::<Value>(&s) {
                    serde_json::from_value(value)?
                } else {
                    // If not JSON, treat as raw query string
                    GraphQLRequest {
                        query: s,
                        variables: None,
                        operation_name: None,
                        headers: HashMap::new(),
                    }
                }
            }
            ToolInput::Structured(v) => serde_json::from_value(v)?,
        };

        // Execute the GraphQL request
        let response = self.execute(request).await?;

        // Format response
        if let Some(errors) = response.errors {
            // If there are errors, format them nicely
            let error_messages: Vec<String> = errors
                .iter()
                .map(|e| {
                    let mut msg = format!("GraphQL Error: {}", e.message);
                    if let Some(locations) = &e.locations {
                        let locs: Vec<String> = locations
                            .iter()
                            .map(|l| format!("line {}, column {}", l.line, l.column))
                            .collect();
                        msg.push_str(&format!(" at {}", locs.join(", ")));
                    }
                    msg
                })
                .collect();

            // If we have data despite errors, include both
            if let Some(data) = response.data {
                Ok(format!(
                    "Partial Success:\nData: {}\n\nErrors:\n{}",
                    serde_json::to_string_pretty(&data)?,
                    error_messages.join("\n")
                ))
            } else {
                Ok(format!("Errors:\n{}", error_messages.join("\n")))
            }
        } else if let Some(data) = response.data {
            // Success case - return formatted data
            Ok(serde_json::to_string_pretty(&data)?)
        } else {
            Ok("No data returned from GraphQL query".to_string())
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;
    use serde_json::json;

    // ========================================================================
    // Tool Trait Implementation Tests
    // ========================================================================

    #[test]
    fn test_graphql_tool_creation() {
        let tool = GraphQLTool::new("https://api.example.com/graphql".to_string());
        assert_eq!(tool.name(), "graphql");
        assert!(tool.description().contains("GraphQL"));
        assert_eq!(tool.endpoint, "https://api.example.com/graphql");
        assert_eq!(tool.timeout_secs, 30);
    }

    #[test]
    fn test_tool_name_is_graphql() {
        let tool = GraphQLTool::new("http://localhost/graphql".to_string());
        assert_eq!(tool.name(), "graphql");
    }

    #[test]
    fn test_tool_description_contains_required_info() {
        let tool = GraphQLTool::new("http://localhost/graphql".to_string());
        let desc = tool.description();
        assert!(desc.contains("query"), "Description should mention query");
        assert!(
            desc.contains("mutation"),
            "Description should mention mutation"
        );
        assert!(
            desc.contains("variables"),
            "Description should mention variables"
        );
    }

    #[test]
    fn test_args_schema_structure() {
        let tool = GraphQLTool::new("http://localhost/graphql".to_string());
        let schema = tool.args_schema();

        // Should be an object type
        assert_eq!(schema["type"], "object");

        // Should have properties
        assert!(schema.get("properties").is_some());
    }

    // ========================================================================
    // Builder Pattern Tests
    // ========================================================================

    #[test]
    fn test_graphql_tool_with_headers() {
        let mut headers = HashMap::new();
        headers.insert("Authorization".to_string(), "Bearer token123".to_string());
        headers.insert("X-Api-Key".to_string(), "key456".to_string());

        let tool = GraphQLTool::with_headers(
            "https://api.example.com/graphql".to_string(),
            headers.clone(),
        );

        assert_eq!(tool.default_headers, headers);
    }

    #[test]
    fn test_graphql_tool_with_timeout() {
        let tool = GraphQLTool::new("https://api.example.com/graphql".to_string()).with_timeout(60);

        assert_eq!(tool.timeout_secs, 60);
    }

    #[test]
    fn test_with_timeout_zero() {
        let tool =
            GraphQLTool::new("https://api.example.com/graphql".to_string()).with_timeout(0);
        assert_eq!(tool.timeout_secs, 0);
    }

    #[test]
    fn test_with_timeout_very_large() {
        let tool =
            GraphQLTool::new("https://api.example.com/graphql".to_string()).with_timeout(3600);
        assert_eq!(tool.timeout_secs, 3600);
    }

    #[test]
    fn test_with_empty_headers() {
        let tool = GraphQLTool::with_headers(
            "https://api.example.com/graphql".to_string(),
            HashMap::new(),
        );
        assert!(tool.default_headers.is_empty());
    }

    #[test]
    fn test_with_single_header() {
        let mut headers = HashMap::new();
        headers.insert("Authorization".to_string(), "Bearer xyz".to_string());

        let tool = GraphQLTool::with_headers(
            "https://api.example.com/graphql".to_string(),
            headers,
        );
        assert_eq!(tool.default_headers.len(), 1);
        assert_eq!(
            tool.default_headers.get("Authorization"),
            Some(&"Bearer xyz".to_string())
        );
    }

    #[test]
    fn test_with_multiple_headers() {
        let mut headers = HashMap::new();
        headers.insert("Authorization".to_string(), "Bearer token".to_string());
        headers.insert("X-Request-Id".to_string(), "12345".to_string());
        headers.insert("Accept-Language".to_string(), "en-US".to_string());

        let tool = GraphQLTool::with_headers(
            "https://api.example.com/graphql".to_string(),
            headers.clone(),
        );
        assert_eq!(tool.default_headers.len(), 3);
    }

    #[test]
    fn test_builder_chaining() {
        let mut headers = HashMap::new();
        headers.insert("Authorization".to_string(), "Bearer token".to_string());

        let tool = GraphQLTool::with_headers(
            "https://api.example.com/graphql".to_string(),
            headers,
        )
        .with_timeout(120);

        assert_eq!(tool.timeout_secs, 120);
        assert!(!tool.default_headers.is_empty());
    }

    // ========================================================================
    // GraphQLRequest Deserialization Tests
    // ========================================================================

    #[test]
    fn test_graphql_request_parsing() {
        let json_str = json!({
            "query": "{ user(id: 1) { name } }",
            "variables": {
                "id": 1
            },
            "operation_name": "GetUser"
        })
        .to_string();

        let request: GraphQLRequest = serde_json::from_str(&json_str).unwrap();
        assert_eq!(request.query, "{ user(id: 1) { name } }");
        assert!(request.variables.is_some());
        assert_eq!(request.operation_name, Some("GetUser".to_string()));
    }

    #[test]
    fn test_graphql_request_simple() {
        let json_str = json!({
            "query": "{ hello }"
        })
        .to_string();

        let request: GraphQLRequest = serde_json::from_str(&json_str).unwrap();
        assert_eq!(request.query, "{ hello }");
        assert!(request.variables.is_none());
        assert!(request.operation_name.is_none());
    }

    #[test]
    fn test_graphql_request_with_empty_variables() {
        let json_str = json!({
            "query": "{ hello }",
            "variables": {}
        })
        .to_string();

        let request: GraphQLRequest = serde_json::from_str(&json_str).unwrap();
        assert!(request.variables.is_some());
        assert!(request.variables.unwrap().is_empty());
    }

    #[test]
    fn test_graphql_request_with_complex_variables() {
        let json_str = json!({
            "query": "mutation CreateUser($input: UserInput!) { createUser(input: $input) { id } }",
            "variables": {
                "input": {
                    "name": "John",
                    "email": "john@example.com",
                    "roles": ["admin", "user"],
                    "metadata": {
                        "createdAt": "2024-01-01"
                    }
                }
            }
        })
        .to_string();

        let request: GraphQLRequest = serde_json::from_str(&json_str).unwrap();
        let vars = request.variables.unwrap();
        assert!(vars.contains_key("input"));
    }

    #[test]
    fn test_graphql_request_with_custom_headers() {
        let json_str = json!({
            "query": "{ me { name } }",
            "headers": {
                "Authorization": "Bearer custom-token",
                "X-Custom-Header": "custom-value"
            }
        })
        .to_string();

        let request: GraphQLRequest = serde_json::from_str(&json_str).unwrap();
        assert_eq!(request.headers.len(), 2);
        assert_eq!(
            request.headers.get("Authorization"),
            Some(&"Bearer custom-token".to_string())
        );
    }

    #[test]
    fn test_graphql_request_with_operation_name_only() {
        let json_str = json!({
            "query": "query GetUser { user { name } } query GetPost { post { title } }",
            "operation_name": "GetUser"
        })
        .to_string();

        let request: GraphQLRequest = serde_json::from_str(&json_str).unwrap();
        assert_eq!(request.operation_name, Some("GetUser".to_string()));
    }

    #[test]
    fn test_graphql_request_multiline_query() {
        let json_str = json!({
            "query": r#"
                query GetUsers {
                    users {
                        id
                        name
                        email
                    }
                }
            "#
        })
        .to_string();

        let request: GraphQLRequest = serde_json::from_str(&json_str).unwrap();
        assert!(request.query.contains("GetUsers"));
        assert!(request.query.contains("users"));
    }

    #[test]
    fn test_graphql_request_mutation() {
        let json_str = json!({
            "query": "mutation { deleteUser(id: 1) }",
            "variables": {}
        })
        .to_string();

        let request: GraphQLRequest = serde_json::from_str(&json_str).unwrap();
        assert!(request.query.contains("mutation"));
    }

    #[test]
    fn test_graphql_request_subscription() {
        let json_str = json!({
            "query": "subscription { messageAdded { content sender } }"
        })
        .to_string();

        let request: GraphQLRequest = serde_json::from_str(&json_str).unwrap();
        assert!(request.query.contains("subscription"));
    }

    #[test]
    fn test_graphql_request_with_null_values() {
        let json_str = json!({
            "query": "{ user { name } }",
            "variables": null,
            "operation_name": null
        })
        .to_string();

        let request: GraphQLRequest = serde_json::from_str(&json_str).unwrap();
        assert!(request.variables.is_none());
        assert!(request.operation_name.is_none());
    }

    #[test]
    fn test_graphql_request_unicode_query() {
        let json_str = json!({
            "query": "{ user(name: \"日本語\") { displayName } }"
        })
        .to_string();

        let request: GraphQLRequest = serde_json::from_str(&json_str).unwrap();
        assert!(request.query.contains("日本語"));
    }

    #[test]
    fn test_graphql_request_special_chars_in_query() {
        let json_str = json!({
            "query": "{ search(term: \"foo & bar <> \\\"quoted\\\"\") { results } }"
        })
        .to_string();

        let request: GraphQLRequest = serde_json::from_str(&json_str).unwrap();
        assert!(!request.query.is_empty());
    }

    #[test]
    fn test_graphql_request_fragments() {
        let json_str = json!({
            "query": r#"
                query GetUser {
                    user(id: 1) {
                        ...UserFields
                    }
                }
                fragment UserFields on User {
                    id
                    name
                    email
                }
            "#
        })
        .to_string();

        let request: GraphQLRequest = serde_json::from_str(&json_str).unwrap();
        assert!(request.query.contains("fragment"));
        assert!(request.query.contains("UserFields"));
    }

    // ========================================================================
    // GraphQLResponse Deserialization Tests
    // ========================================================================

    #[test]
    fn test_graphql_response_success_only() {
        let json_str = json!({
            "data": {
                "user": {
                    "name": "John"
                }
            }
        })
        .to_string();

        let response: GraphQLResponse = serde_json::from_str(&json_str).unwrap();
        assert!(response.data.is_some());
        assert!(response.errors.is_none());
    }

    #[test]
    fn test_graphql_response_errors_only() {
        let json_str = json!({
            "errors": [
                {
                    "message": "Not found"
                }
            ]
        })
        .to_string();

        let response: GraphQLResponse = serde_json::from_str(&json_str).unwrap();
        assert!(response.data.is_none());
        assert!(response.errors.is_some());
        assert_eq!(response.errors.as_ref().unwrap().len(), 1);
    }

    #[test]
    fn test_graphql_response_partial_data_with_errors() {
        let json_str = json!({
            "data": {
                "user": {
                    "name": "John"
                },
                "posts": null
            },
            "errors": [
                {
                    "message": "Failed to fetch posts",
                    "path": ["posts"]
                }
            ]
        })
        .to_string();

        let response: GraphQLResponse = serde_json::from_str(&json_str).unwrap();
        assert!(response.data.is_some());
        assert!(response.errors.is_some());
    }

    #[test]
    fn test_graphql_response_empty() {
        let json_str = json!({}).to_string();

        let response: GraphQLResponse = serde_json::from_str(&json_str).unwrap();
        assert!(response.data.is_none());
        assert!(response.errors.is_none());
    }

    #[test]
    fn test_graphql_response_null_data() {
        let json_str = json!({
            "data": null
        })
        .to_string();

        let response: GraphQLResponse = serde_json::from_str(&json_str).unwrap();
        assert!(response.data.is_none());
    }

    #[test]
    fn test_graphql_response_complex_data() {
        let json_str = json!({
            "data": {
                "users": [
                    {"id": 1, "name": "Alice"},
                    {"id": 2, "name": "Bob"}
                ],
                "total": 2,
                "metadata": {
                    "page": 1,
                    "perPage": 10
                }
            }
        })
        .to_string();

        let response: GraphQLResponse = serde_json::from_str(&json_str).unwrap();
        assert!(response.data.is_some());
        let data = response.data.unwrap();
        assert!(data.get("users").is_some());
    }

    // ========================================================================
    // GraphQLError Deserialization Tests
    // ========================================================================

    #[test]
    fn test_graphql_error_minimal() {
        let json_str = json!({
            "message": "Something went wrong"
        })
        .to_string();

        let error: GraphQLError = serde_json::from_str(&json_str).unwrap();
        assert_eq!(error.message, "Something went wrong");
        assert!(error.locations.is_none());
        assert!(error.path.is_none());
        assert!(error.extensions.is_none());
    }

    #[test]
    fn test_graphql_error_with_locations() {
        let json_str = json!({
            "message": "Syntax error",
            "locations": [
                {"line": 1, "column": 5},
                {"line": 2, "column": 10}
            ]
        })
        .to_string();

        let error: GraphQLError = serde_json::from_str(&json_str).unwrap();
        let locations = error.locations.unwrap();
        assert_eq!(locations.len(), 2);
        assert_eq!(locations[0].line, 1);
        assert_eq!(locations[0].column, 5);
    }

    #[test]
    fn test_graphql_error_with_path() {
        let json_str = json!({
            "message": "Field error",
            "path": ["user", "profile", "avatar"]
        })
        .to_string();

        let error: GraphQLError = serde_json::from_str(&json_str).unwrap();
        let path = error.path.unwrap();
        assert_eq!(path.len(), 3);
    }

    #[test]
    fn test_graphql_error_with_mixed_path() {
        let json_str = json!({
            "message": "Array index error",
            "path": ["users", 0, "name"]
        })
        .to_string();

        let error: GraphQLError = serde_json::from_str(&json_str).unwrap();
        let path = error.path.unwrap();
        assert_eq!(path.len(), 3);
        assert_eq!(path[1], 0);
    }

    #[test]
    fn test_graphql_error_with_extensions() {
        let json_str = json!({
            "message": "Validation failed",
            "extensions": {
                "code": "GRAPHQL_VALIDATION_FAILED",
                "timestamp": "2024-01-01T00:00:00Z",
                "details": {
                    "field": "email",
                    "constraint": "format"
                }
            }
        })
        .to_string();

        let error: GraphQLError = serde_json::from_str(&json_str).unwrap();
        let extensions = error.extensions.unwrap();
        assert_eq!(
            extensions.get("code"),
            Some(&Value::String("GRAPHQL_VALIDATION_FAILED".to_string()))
        );
    }

    #[test]
    fn test_graphql_error_full() {
        let json_str = json!({
            "message": "Cannot query field \"foo\" on type \"Query\"",
            "locations": [{"line": 1, "column": 3}],
            "path": ["query"],
            "extensions": {
                "code": "FIELD_NOT_FOUND"
            }
        })
        .to_string();

        let error: GraphQLError = serde_json::from_str(&json_str).unwrap();
        assert!(error.locations.is_some());
        assert!(error.path.is_some());
        assert!(error.extensions.is_some());
    }

    // ========================================================================
    // GraphQLErrorLocation Tests
    // ========================================================================

    #[test]
    fn test_graphql_error_location_basic() {
        let json_str = json!({"line": 5, "column": 10}).to_string();

        let location: GraphQLErrorLocation = serde_json::from_str(&json_str).unwrap();
        assert_eq!(location.line, 5);
        assert_eq!(location.column, 10);
    }

    #[test]
    fn test_graphql_error_location_zero() {
        let json_str = json!({"line": 0, "column": 0}).to_string();

        let location: GraphQLErrorLocation = serde_json::from_str(&json_str).unwrap();
        assert_eq!(location.line, 0);
        assert_eq!(location.column, 0);
    }

    #[test]
    fn test_graphql_error_location_large_values() {
        let json_str = json!({"line": 999999, "column": 999999}).to_string();

        let location: GraphQLErrorLocation = serde_json::from_str(&json_str).unwrap();
        assert_eq!(location.line, 999999);
        assert_eq!(location.column, 999999);
    }

    // ========================================================================
    // Network Tests (ignored - require real endpoints)
    // ========================================================================

    #[tokio::test]
    #[ignore = "requires network access"]
    async fn test_graphql_query_execution() {
        // SpaceX GraphQL API (public endpoint)
        let tool = GraphQLTool::new("https://api.spacex.land/graphql".to_string());

        let input = json!({
            "query": "query { company { name founder } }"
        })
        .to_string();

        let response = tool._call_str(input).await.expect("GraphQL call failed");
        assert!(response.contains("name"));
    }

    #[tokio::test]
    #[ignore = "requires network access"]
    async fn test_graphql_query_with_variables() {
        // SpaceX GraphQL API (public endpoint)
        let tool = GraphQLTool::new("https://api.spacex.land/graphql".to_string());

        let input = json!({
            "query": "query Launches($limit: Int!) { launchesPast(limit: $limit) { mission_name } }",
            "variables": {
                "limit": 5
            }
        })
        .to_string();

        let response = tool._call_str(input).await.expect("GraphQL call failed");
        assert!(response.contains("mission_name"));
    }

    // ========================================================================
    // Mock Server Tests
    // ========================================================================

    #[tokio::test]
    async fn test_graphql_error_handling() {
        use wiremock::matchers::{method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let mock_server = MockServer::start().await;

        // Mock GraphQL error response
        Mock::given(method("POST"))
            .and(path("/graphql"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "errors": [
                    {
                        "message": "Field 'invalidField' doesn't exist on type 'Query'",
                        "locations": [{"line": 1, "column": 3}],
                        "extensions": {"code": "GRAPHQL_VALIDATION_FAILED"}
                    }
                ]
            })))
            .mount(&mock_server)
            .await;

        let tool = GraphQLTool::new(format!("{}/graphql", mock_server.uri()));
        let input = json!({
            "query": "{ invalidField }"
        })
        .to_string();

        let result = tool._call_str(input).await;
        assert!(result.is_ok());

        let response = result.unwrap();
        assert!(response.contains("GraphQL Error"));
        assert!(response.contains("invalidField"));
    }

    #[tokio::test]
    async fn test_graphql_partial_success() {
        use wiremock::matchers::{method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let mock_server = MockServer::start().await;

        // Mock partial success (data + errors)
        Mock::given(method("POST"))
            .and(path("/graphql"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "data": {
                    "user": {
                        "name": "John Doe"
                    }
                },
                "errors": [
                    {
                        "message": "Field 'email' is deprecated",
                        "path": ["user", "email"]
                    }
                ]
            })))
            .mount(&mock_server)
            .await;

        let tool = GraphQLTool::new(format!("{}/graphql", mock_server.uri()));
        let input = json!({
            "query": "{ user { name email } }"
        })
        .to_string();

        let result = tool._call_str(input).await;
        assert!(result.is_ok());

        let response = result.unwrap();
        assert!(response.contains("Partial Success"));
        assert!(response.contains("John Doe"));
        assert!(response.contains("deprecated"));
    }

    #[tokio::test]
    async fn test_graphql_success_response() {
        use wiremock::matchers::{method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let mock_server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/graphql"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "data": {
                    "user": {
                        "id": 1,
                        "name": "Alice",
                        "email": "alice@example.com"
                    }
                }
            })))
            .mount(&mock_server)
            .await;

        let tool = GraphQLTool::new(format!("{}/graphql", mock_server.uri()));
        let input = json!({
            "query": "{ user { id name email } }"
        })
        .to_string();

        let result = tool._call_str(input).await;
        assert!(result.is_ok());

        let response = result.unwrap();
        assert!(response.contains("Alice"));
        assert!(response.contains("alice@example.com"));
    }

    #[tokio::test]
    async fn test_graphql_empty_data_response() {
        use wiremock::matchers::{method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let mock_server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/graphql"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "data": null
            })))
            .mount(&mock_server)
            .await;

        let tool = GraphQLTool::new(format!("{}/graphql", mock_server.uri()));
        let input = json!({
            "query": "{ user { name } }"
        })
        .to_string();

        let result = tool._call_str(input).await;
        assert!(result.is_ok());

        let response = result.unwrap();
        assert!(response.contains("No data returned"));
    }

    #[tokio::test]
    async fn test_graphql_multiple_errors() {
        use wiremock::matchers::{method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let mock_server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/graphql"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "errors": [
                    {
                        "message": "First error",
                        "locations": [{"line": 1, "column": 1}]
                    },
                    {
                        "message": "Second error",
                        "locations": [{"line": 2, "column": 5}]
                    },
                    {
                        "message": "Third error"
                    }
                ]
            })))
            .mount(&mock_server)
            .await;

        let tool = GraphQLTool::new(format!("{}/graphql", mock_server.uri()));
        let input = json!({
            "query": "{ invalid }"
        })
        .to_string();

        let result = tool._call_str(input).await;
        assert!(result.is_ok());

        let response = result.unwrap();
        assert!(response.contains("First error"));
        assert!(response.contains("Second error"));
        assert!(response.contains("Third error"));
    }

    #[tokio::test]
    async fn test_graphql_http_error() {
        use wiremock::matchers::{method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let mock_server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/graphql"))
            .respond_with(ResponseTemplate::new(500).set_body_string("Internal Server Error"))
            .mount(&mock_server)
            .await;

        let tool = GraphQLTool::new(format!("{}/graphql", mock_server.uri()));
        let input = json!({
            "query": "{ user { name } }"
        })
        .to_string();

        let result = tool._call_str(input).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_graphql_with_variables_mock() {
        use wiremock::matchers::{method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let mock_server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/graphql"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "data": {
                    "user": {
                        "id": 42,
                        "name": "Test User"
                    }
                }
            })))
            .mount(&mock_server)
            .await;

        let tool = GraphQLTool::new(format!("{}/graphql", mock_server.uri()));
        let input = json!({
            "query": "query GetUser($id: Int!) { user(id: $id) { id name } }",
            "variables": {
                "id": 42
            }
        })
        .to_string();

        let result = tool._call_str(input).await;
        assert!(result.is_ok());
        assert!(result.unwrap().contains("Test User"));
    }

    #[tokio::test]
    async fn test_graphql_array_response() {
        use wiremock::matchers::{method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let mock_server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/graphql"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "data": {
                    "users": [
                        {"id": 1, "name": "Alice"},
                        {"id": 2, "name": "Bob"},
                        {"id": 3, "name": "Charlie"}
                    ]
                }
            })))
            .mount(&mock_server)
            .await;

        let tool = GraphQLTool::new(format!("{}/graphql", mock_server.uri()));
        let input = json!({
            "query": "{ users { id name } }"
        })
        .to_string();

        let result = tool._call_str(input).await;
        assert!(result.is_ok());

        let response = result.unwrap();
        assert!(response.contains("Alice"));
        assert!(response.contains("Bob"));
        assert!(response.contains("Charlie"));
    }

    #[tokio::test]
    async fn test_graphql_nested_data() {
        use wiremock::matchers::{method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let mock_server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/graphql"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "data": {
                    "company": {
                        "name": "Acme Corp",
                        "employees": {
                            "total": 100,
                            "items": [
                                {
                                    "name": "John",
                                    "department": {
                                        "name": "Engineering"
                                    }
                                }
                            ]
                        }
                    }
                }
            })))
            .mount(&mock_server)
            .await;

        let tool = GraphQLTool::new(format!("{}/graphql", mock_server.uri()));
        let input = json!({
            "query": "{ company { name employees { total items { name department { name } } } } }"
        })
        .to_string();

        let result = tool._call_str(input).await;
        assert!(result.is_ok());

        let response = result.unwrap();
        assert!(response.contains("Acme Corp"));
        assert!(response.contains("Engineering"));
    }

    // ========================================================================
    // Input Parsing Tests
    // ========================================================================

    #[tokio::test]
    async fn test_input_raw_query_string() {
        use wiremock::matchers::{method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let mock_server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/graphql"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "data": {"hello": "world"}
            })))
            .mount(&mock_server)
            .await;

        let tool = GraphQLTool::new(format!("{}/graphql", mock_server.uri()));

        // Input as raw GraphQL query string (not JSON)
        let result = tool._call_str("{ hello }".to_string()).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_input_structured_json() {
        use wiremock::matchers::{method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let mock_server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/graphql"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "data": {"user": {"name": "Test"}}
            })))
            .mount(&mock_server)
            .await;

        let tool = GraphQLTool::new(format!("{}/graphql", mock_server.uri()));

        // Input as ToolInput::Structured
        let input = ToolInput::Structured(json!({
            "query": "{ user { name } }"
        }));
        let result = tool._call(input).await;
        assert!(result.is_ok());
    }

    // ========================================================================
    // Edge Case Tests
    // ========================================================================

    #[test]
    fn test_empty_endpoint() {
        let tool = GraphQLTool::new(String::new());
        assert_eq!(tool.endpoint, "");
    }

    #[test]
    fn test_unicode_endpoint() {
        let tool = GraphQLTool::new("https://例え.jp/graphql".to_string());
        assert_eq!(tool.endpoint, "https://例え.jp/graphql");
    }

    #[test]
    fn test_endpoint_with_path() {
        let tool = GraphQLTool::new("https://api.example.com/v1/graphql".to_string());
        assert_eq!(tool.endpoint, "https://api.example.com/v1/graphql");
    }

    #[test]
    fn test_endpoint_with_port() {
        let tool = GraphQLTool::new("http://localhost:4000/graphql".to_string());
        assert_eq!(tool.endpoint, "http://localhost:4000/graphql");
    }

    #[test]
    fn test_default_timeout() {
        let tool = GraphQLTool::new("http://localhost/graphql".to_string());
        assert_eq!(tool.timeout_secs, 30);
    }

    #[test]
    fn test_header_with_special_chars() {
        let mut headers = HashMap::new();
        headers.insert(
            "Authorization".to_string(),
            "Bearer abc123+/=".to_string(),
        );

        let tool = GraphQLTool::with_headers("http://localhost/graphql".to_string(), headers);
        assert_eq!(
            tool.default_headers.get("Authorization"),
            Some(&"Bearer abc123+/=".to_string())
        );
    }

    #[tokio::test]
    async fn test_401_unauthorized() {
        use wiremock::matchers::{method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let mock_server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/graphql"))
            .respond_with(ResponseTemplate::new(401).set_body_string("Unauthorized"))
            .mount(&mock_server)
            .await;

        let tool = GraphQLTool::new(format!("{}/graphql", mock_server.uri()));
        let input = json!({"query": "{ me { name } }"}).to_string();

        let result = tool._call_str(input).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_403_forbidden() {
        use wiremock::matchers::{method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let mock_server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/graphql"))
            .respond_with(ResponseTemplate::new(403).set_body_string("Forbidden"))
            .mount(&mock_server)
            .await;

        let tool = GraphQLTool::new(format!("{}/graphql", mock_server.uri()));
        let input = json!({"query": "{ admin { secrets } }"}).to_string();

        let result = tool._call_str(input).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_malformed_json_response() {
        use wiremock::matchers::{method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let mock_server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/graphql"))
            .respond_with(ResponseTemplate::new(200).set_body_string("not valid json"))
            .mount(&mock_server)
            .await;

        let tool = GraphQLTool::new(format!("{}/graphql", mock_server.uri()));
        let input = json!({"query": "{ user }"}).to_string();

        let result = tool._call_str(input).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_empty_response_body() {
        use wiremock::matchers::{method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let mock_server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/graphql"))
            .respond_with(ResponseTemplate::new(200).set_body_string(""))
            .mount(&mock_server)
            .await;

        let tool = GraphQLTool::new(format!("{}/graphql", mock_server.uri()));
        let input = json!({"query": "{ user }"}).to_string();

        let result = tool._call_str(input).await;
        assert!(result.is_err());
    }
}
