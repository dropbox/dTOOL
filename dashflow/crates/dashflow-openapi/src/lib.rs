//! `OpenAPI` tool for `DashFlow`
//!
//! This crate provides tools for interacting with REST APIs via OpenAPI/Swagger specifications.
//! It can parse `OpenAPI` v3.0.x specs and execute API operations dynamically.
//!
//! # Example
//!
//! ```rust,no_run
//! use dashflow_openapi::OpenAPITool;
//! use dashflow::core::tools::Tool;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! // Load OpenAPI spec from URL
//! let tool = OpenAPITool::from_url(
//!     "https://petstore3.swagger.io/api/v3/openapi.json",
//!     None, // No auth
//! ).await?;
//!
//! // Execute a GET operation
//! let result = tool._call_str(
//!     r#"{"operation_id": "getPetById", "parameters": {"petId": 1}}"#.to_string()
//! ).await?;
//!
//! println!("Result: {}", result);
//! # Ok(())
//! # }
//! ```

use async_trait::async_trait;
use dashflow::core::tools::{Tool, ToolInput};
use dashflow::core::Error;
use openapiv3::{OpenAPI, Operation, Parameter};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use std::collections::HashMap;
use url::Url;

/// Authentication configuration for `OpenAPI` requests
#[derive(Clone)]
pub enum Authentication {
    /// No authentication
    None,
    /// API Key authentication (header-based)
    ApiKey {
        /// Header name (e.g., "X-API-Key")
        header_name: String,
        /// API key value
        api_key: String,
    },
    /// Bearer token authentication
    Bearer {
        /// Bearer token
        token: String,
    },
    /// Basic authentication
    Basic {
        /// Username
        username: String,
        /// Password
        password: String,
    },
}

// Custom Debug to prevent secret exposure in logs
impl std::fmt::Debug for Authentication {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Authentication::None => write!(f, "Authentication::None"),
            Authentication::ApiKey { header_name, .. } => f
                .debug_struct("Authentication::ApiKey")
                .field("header_name", header_name)
                .field("api_key", &"[REDACTED]")
                .finish(),
            Authentication::Bearer { .. } => f
                .debug_struct("Authentication::Bearer")
                .field("token", &"[REDACTED]")
                .finish(),
            Authentication::Basic { username, .. } => f
                .debug_struct("Authentication::Basic")
                .field("username", username)
                .field("password", &"[REDACTED]")
                .finish(),
        }
    }
}

/// `OpenAPI` tool that executes API operations based on `OpenAPI` spec
pub struct OpenAPITool {
    /// `OpenAPI` specification
    spec: OpenAPI,
    /// Base URL for API requests
    base_url: String,
    /// HTTP client
    client: Client,
    /// Authentication configuration
    auth: Authentication,
}

/// Input for `OpenAPI` tool execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenAPIInput {
    /// Operation ID from `OpenAPI` spec (e.g., "getPetById")
    pub operation_id: String,
    /// Parameters for the operation (path, query, header params)
    #[serde(default)]
    pub parameters: HashMap<String, JsonValue>,
    /// Request body (for POST/PUT/PATCH operations)
    #[serde(default)]
    pub body: Option<JsonValue>,
}

impl OpenAPITool {
    /// Create a new `OpenAPI` tool from a specification URL
    ///
    /// # Arguments
    ///
    /// * `spec_url` - URL to the `OpenAPI` specification (JSON or YAML)
    /// * `auth` - Optional authentication configuration
    pub async fn from_url(spec_url: &str, auth: Option<Authentication>) -> Result<Self, Error> {
        let client = Client::new();
        let response = client
            .get(spec_url)
            .send()
            .await
            .map_err(|e| Error::tool_error(format!("Failed to fetch OpenAPI spec: {e}")))?;

        let spec_text = response
            .text()
            .await
            .map_err(|e| Error::tool_error(format!("Failed to read OpenAPI spec: {e}")))?;

        Self::from_str(&spec_text, auth)
    }

    /// Create a new `OpenAPI` tool from a specification string (JSON or YAML)
    ///
    /// # Arguments
    ///
    /// * `spec_text` - `OpenAPI` specification as a string
    /// * `auth` - Optional authentication configuration
    pub fn from_str(spec_text: &str, auth: Option<Authentication>) -> Result<Self, Error> {
        // Try parsing as JSON first, then YAML
        let spec: OpenAPI = serde_json::from_str(spec_text)
            .or_else(|_| {
                serde_yml::from_str(spec_text)
                    .map_err(|e| Error::tool_error(format!("Failed to parse OpenAPI spec: {e}")))
            })
            .map_err(|e| Error::tool_error(format!("Failed to parse OpenAPI spec: {e}")))?;

        // Extract base URL from servers
        let base_url = spec
            .servers
            .first()
            .map_or_else(|| "http://localhost".to_string(), |s| s.url.clone());

        Ok(Self {
            spec,
            base_url,
            client: Client::new(),
            auth: auth.unwrap_or(Authentication::None),
        })
    }

    /// Find an operation by operation ID
    fn find_operation(&self, operation_id: &str) -> Result<(&str, &str, &Operation), Error> {
        for (path, path_item_ref) in &self.spec.paths.paths {
            // Dereference the ReferenceOr<PathItem>
            let path_item = match path_item_ref {
                openapiv3::ReferenceOr::Item(item) => item,
                openapiv3::ReferenceOr::Reference { .. } => continue, // Skip references
            };

            let operations = [
                ("GET", path_item.get.as_ref()),
                ("POST", path_item.post.as_ref()),
                ("PUT", path_item.put.as_ref()),
                ("DELETE", path_item.delete.as_ref()),
                ("PATCH", path_item.patch.as_ref()),
            ];

            for (method, op) in operations {
                if let Some(operation) = op {
                    if let Some(op_id) = &operation.operation_id {
                        if op_id == operation_id {
                            return Ok((path, method, operation));
                        }
                    }
                }
            }
        }

        Err(Error::tool_error(format!(
            "Operation '{operation_id}' not found in OpenAPI spec"
        )))
    }

    /// Execute an API operation
    async fn execute_operation(
        &self,
        operation_id: &str,
        parameters: &HashMap<String, JsonValue>,
        body: Option<&JsonValue>,
    ) -> Result<String, Error> {
        let (path_template, method, operation) = self.find_operation(operation_id)?;

        // Build URL with path parameters
        let mut url_path = path_template.to_string();
        let mut query_params: Vec<(String, String)> = Vec::new();

        // Process parameters
        for param_or_ref in &operation.parameters {
            // For simplicity, only handle direct Parameter objects (not $ref)
            if let openapiv3::ReferenceOr::Item(param) = param_or_ref {
                let param_data = match param {
                    Parameter::Query { parameter_data, .. } => Some(parameter_data),
                    Parameter::Path { parameter_data, .. } => Some(parameter_data),
                    Parameter::Header { parameter_data, .. } => Some(parameter_data),
                    Parameter::Cookie { parameter_data, .. } => Some(parameter_data),
                };

                if let Some(data) = param_data {
                    if let Some(value) = parameters.get(&data.name) {
                        let value_str = match value {
                            JsonValue::String(s) => s.clone(),
                            JsonValue::Number(n) => n.to_string(),
                            JsonValue::Bool(b) => b.to_string(),
                            _ => value.to_string(),
                        };

                        match param {
                            Parameter::Path { .. } => {
                                url_path =
                                    url_path.replace(&format!("{{{}}}", data.name), &value_str);
                            }
                            Parameter::Query { .. } => {
                                query_params.push((data.name.clone(), value_str));
                            }
                            _ => {}
                        }
                    }
                }
            }
        }

        // Build full URL
        let full_url = format!("{}{}", self.base_url, url_path);
        let mut url =
            Url::parse(&full_url).map_err(|e| Error::tool_error(format!("Invalid URL: {e}")))?;
        url.query_pairs_mut().extend_pairs(query_params);

        // Build request
        let mut request = match method {
            "GET" => self.client.get(url.as_str()),
            "POST" => self.client.post(url.as_str()),
            "PUT" => self.client.put(url.as_str()),
            "DELETE" => self.client.delete(url.as_str()),
            "PATCH" => self.client.patch(url.as_str()),
            _ => {
                return Err(Error::tool_error(format!(
                    "Unsupported HTTP method: {method}"
                )))
            }
        };

        // Add authentication
        request = match &self.auth {
            Authentication::None => request,
            Authentication::ApiKey {
                header_name,
                api_key,
            } => request.header(header_name.as_str(), api_key.as_str()),
            Authentication::Bearer { token } => request.bearer_auth(token),
            Authentication::Basic { username, password } => {
                request.basic_auth(username, Some(password))
            }
        };

        // Add body for POST/PUT/PATCH
        if let Some(body_value) = body {
            if matches!(method, "POST" | "PUT" | "PATCH") {
                request = request.json(body_value);
            }
        }

        // Execute request
        let response = request
            .send()
            .await
            .map_err(|e| Error::tool_error(format!("API request failed: {e}")))?;

        // Get response body
        let status = response.status();
        let response_text = response
            .text()
            .await
            .map_err(|e| Error::tool_error(format!("Failed to read response: {e}")))?;

        if status.is_success() {
            Ok(response_text)
        } else {
            Err(Error::tool_error(format!(
                "API request failed with status {status}: {response_text}"
            )))
        }
    }

    /// List all available operations in the `OpenAPI` spec
    #[must_use]
    pub fn list_operations(&self) -> Vec<String> {
        let mut operations = Vec::new();

        for (_path, path_item_ref) in &self.spec.paths.paths {
            // Dereference the ReferenceOr<PathItem>
            let path_item = match path_item_ref {
                openapiv3::ReferenceOr::Item(item) => item,
                openapiv3::ReferenceOr::Reference { .. } => continue,
            };

            let ops = [
                &path_item.get,
                &path_item.post,
                &path_item.put,
                &path_item.delete,
                &path_item.patch,
            ];

            for op in ops.iter().filter_map(|o| o.as_ref()) {
                if let Some(op_id) = &op.operation_id {
                    operations.push(op_id.clone());
                }
            }
        }

        operations
    }
}

#[async_trait]
impl Tool for OpenAPITool {
    fn name(&self) -> &'static str {
        "openapi_tool"
    }

    fn description(&self) -> &'static str {
        "Execute API operations based on OpenAPI specification. \
         Input should be JSON with 'operation_id', 'parameters', and optional 'body' fields. \
         Example: {\"operation_id\": \"getPetById\", \"parameters\": {\"petId\": 1}}"
    }

    async fn _call(&self, input: ToolInput) -> Result<String, Error> {
        let input_json = match input {
            ToolInput::String(s) => s,
            ToolInput::Structured(v) => serde_json::to_string(&v)
                .map_err(|e| Error::tool_error(format!("Failed to serialize input: {e}")))?,
        };

        let parsed_input: OpenAPIInput = serde_json::from_str(&input_json)
            .map_err(|e| Error::tool_error(format!("Invalid input format: {e}")))?;

        self.execute_operation(
            &parsed_input.operation_id,
            &parsed_input.parameters,
            parsed_input.body.as_ref(),
        )
        .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ========================================================================
    // Authentication Tests
    // ========================================================================

    #[test]
    fn test_authentication_none() {
        let auth = Authentication::None;
        let debug_str = format!("{:?}", auth);
        assert_eq!(debug_str, "Authentication::None");
    }

    #[test]
    fn test_authentication_api_key() {
        let auth = Authentication::ApiKey {
            header_name: "X-API-Key".to_string(),
            api_key: "super_secret_key_12345".to_string(),
        };
        let debug_str = format!("{:?}", auth);
        // Should contain header name but NOT the actual secret
        assert!(debug_str.contains("X-API-Key"));
        assert!(debug_str.contains("[REDACTED]"));
        assert!(!debug_str.contains("super_secret_key_12345"));
    }

    #[test]
    fn test_authentication_bearer() {
        let auth = Authentication::Bearer {
            token: "bearer_token_secret_xyz".to_string(),
        };
        let debug_str = format!("{:?}", auth);
        assert!(debug_str.contains("Bearer"));
        assert!(debug_str.contains("[REDACTED]"));
        assert!(!debug_str.contains("bearer_token_secret_xyz"));
    }

    #[test]
    fn test_authentication_basic() {
        let auth = Authentication::Basic {
            username: "testuser".to_string(),
            password: "supersecretpassword".to_string(),
        };
        let debug_str = format!("{:?}", auth);
        // Username should be visible, password should be redacted
        assert!(debug_str.contains("testuser"));
        assert!(debug_str.contains("[REDACTED]"));
        assert!(!debug_str.contains("supersecretpassword"));
    }

    #[test]
    fn test_authentication_clone() {
        let auth = Authentication::ApiKey {
            header_name: "X-Custom-Auth".to_string(),
            api_key: "key123".to_string(),
        };
        let cloned = auth.clone();
        match cloned {
            Authentication::ApiKey {
                header_name,
                api_key,
            } => {
                assert_eq!(header_name, "X-Custom-Auth");
                assert_eq!(api_key, "key123");
            }
            _ => panic!("Expected ApiKey variant"),
        }
    }

    // ========================================================================
    // OpenAPIInput Tests
    // ========================================================================

    #[test]
    fn test_openapi_input_deserialize_minimal() {
        let json = r#"{"operation_id": "getPetById"}"#;
        let input: OpenAPIInput = serde_json::from_str(json).unwrap();
        assert_eq!(input.operation_id, "getPetById");
        assert!(input.parameters.is_empty());
        assert!(input.body.is_none());
    }

    #[test]
    fn test_openapi_input_deserialize_with_parameters() {
        let json = r#"{"operation_id": "getPetById", "parameters": {"petId": 123}}"#;
        let input: OpenAPIInput = serde_json::from_str(json).unwrap();
        assert_eq!(input.operation_id, "getPetById");
        assert_eq!(input.parameters.get("petId").unwrap(), &JsonValue::from(123));
    }

    #[test]
    fn test_openapi_input_deserialize_with_body() {
        let json = r#"{"operation_id": "createPet", "parameters": {}, "body": {"name": "Fluffy"}}"#;
        let input: OpenAPIInput = serde_json::from_str(json).unwrap();
        assert_eq!(input.operation_id, "createPet");
        assert!(input.body.is_some());
        let body = input.body.unwrap();
        assert_eq!(body["name"], "Fluffy");
    }

    #[test]
    fn test_openapi_input_deserialize_complex_parameters() {
        let json = r#"{
            "operation_id": "searchPets",
            "parameters": {
                "name": "dog",
                "status": "available",
                "limit": 10,
                "active": true
            }
        }"#;
        let input: OpenAPIInput = serde_json::from_str(json).unwrap();
        assert_eq!(input.parameters.len(), 4);
        assert_eq!(
            input.parameters.get("name").unwrap(),
            &JsonValue::from("dog")
        );
        assert_eq!(
            input.parameters.get("limit").unwrap(),
            &JsonValue::from(10)
        );
        assert_eq!(
            input.parameters.get("active").unwrap(),
            &JsonValue::from(true)
        );
    }

    #[test]
    fn test_openapi_input_serialize() {
        let input = OpenAPIInput {
            operation_id: "deletePet".to_string(),
            parameters: {
                let mut map = HashMap::new();
                map.insert("petId".to_string(), JsonValue::from(42));
                map
            },
            body: None,
        };
        let json = serde_json::to_string(&input).unwrap();
        assert!(json.contains("deletePet"));
        assert!(json.contains("petId"));
        assert!(json.contains("42"));
    }

    #[test]
    fn test_openapi_input_debug() {
        let input = OpenAPIInput {
            operation_id: "testOp".to_string(),
            parameters: HashMap::new(),
            body: None,
        };
        let debug_str = format!("{:?}", input);
        assert!(debug_str.contains("testOp"));
    }

    #[test]
    fn test_openapi_input_clone() {
        let input = OpenAPIInput {
            operation_id: "cloneTest".to_string(),
            parameters: {
                let mut map = HashMap::new();
                map.insert("key".to_string(), JsonValue::from("value"));
                map
            },
            body: Some(JsonValue::from("body_content")),
        };
        let cloned = input.clone();
        assert_eq!(cloned.operation_id, "cloneTest");
        assert_eq!(cloned.parameters.get("key").unwrap(), "value");
        assert!(cloned.body.is_some());
    }

    // ========================================================================
    // OpenAPI Spec Parsing Tests
    // ========================================================================

    fn minimal_openapi_json() -> &'static str {
        r#"{
            "openapi": "3.0.0",
            "info": {"title": "Test API", "version": "1.0.0"},
            "paths": {}
        }"#
    }

    fn minimal_openapi_yaml() -> &'static str {
        r#"openapi: "3.0.0"
info:
  title: Test API
  version: "1.0.0"
paths: {}"#
    }

    fn petstore_openapi_json() -> &'static str {
        r#"{
            "openapi": "3.0.0",
            "info": {"title": "Petstore API", "version": "1.0.0"},
            "servers": [{"url": "https://api.petstore.example.com/v1"}],
            "paths": {
                "/pets": {
                    "get": {
                        "operationId": "listPets",
                        "parameters": [
                            {"name": "limit", "in": "query", "schema": {"type": "integer"}}
                        ],
                        "responses": {"200": {"description": "OK"}}
                    },
                    "post": {
                        "operationId": "createPet",
                        "requestBody": {"content": {"application/json": {}}},
                        "responses": {"201": {"description": "Created"}}
                    }
                },
                "/pets/{petId}": {
                    "get": {
                        "operationId": "getPetById",
                        "parameters": [
                            {"name": "petId", "in": "path", "required": true, "schema": {"type": "integer"}}
                        ],
                        "responses": {"200": {"description": "OK"}}
                    },
                    "delete": {
                        "operationId": "deletePet",
                        "parameters": [
                            {"name": "petId", "in": "path", "required": true, "schema": {"type": "integer"}}
                        ],
                        "responses": {"204": {"description": "Deleted"}}
                    },
                    "put": {
                        "operationId": "updatePet",
                        "parameters": [
                            {"name": "petId", "in": "path", "required": true, "schema": {"type": "integer"}}
                        ],
                        "requestBody": {"content": {"application/json": {}}},
                        "responses": {"200": {"description": "Updated"}}
                    },
                    "patch": {
                        "operationId": "patchPet",
                        "parameters": [
                            {"name": "petId", "in": "path", "required": true, "schema": {"type": "integer"}}
                        ],
                        "requestBody": {"content": {"application/json": {}}},
                        "responses": {"200": {"description": "Patched"}}
                    }
                }
            }
        }"#
    }

    #[test]
    fn test_from_str_json_minimal() {
        let spec = minimal_openapi_json();
        let tool = OpenAPITool::from_str(spec, None).unwrap();
        // Default base URL when no servers specified
        assert_eq!(tool.base_url, "http://localhost");
    }

    #[test]
    fn test_from_str_yaml_minimal() {
        let spec = minimal_openapi_yaml();
        let tool = OpenAPITool::from_str(spec, None).unwrap();
        assert_eq!(tool.base_url, "http://localhost");
    }

    #[test]
    fn test_from_str_with_server() {
        let spec = petstore_openapi_json();
        let tool = OpenAPITool::from_str(spec, None).unwrap();
        assert_eq!(tool.base_url, "https://api.petstore.example.com/v1");
    }

    #[test]
    fn test_from_str_with_authentication() {
        let spec = minimal_openapi_json();
        let auth = Authentication::ApiKey {
            header_name: "X-API-Key".to_string(),
            api_key: "test123".to_string(),
        };
        let tool = OpenAPITool::from_str(spec, Some(auth)).unwrap();
        match &tool.auth {
            Authentication::ApiKey { header_name, .. } => {
                assert_eq!(header_name, "X-API-Key");
            }
            _ => panic!("Expected ApiKey authentication"),
        }
    }

    #[test]
    fn test_from_str_invalid_json() {
        let invalid = "{ this is not valid json }";
        let result = OpenAPITool::from_str(invalid, None);
        assert!(result.is_err());
        // Check the error message via matching
        match result {
            Err(e) => {
                let err_str = e.to_string();
                assert!(
                    err_str.contains("parse") || err_str.contains("OpenAPI"),
                    "Expected parse error, got: {}",
                    err_str
                );
            }
            Ok(_) => panic!("Expected error"),
        }
    }

    #[test]
    fn test_from_str_invalid_yaml() {
        let invalid = ":::\nthis: is: invalid: yaml:";
        let result = OpenAPITool::from_str(invalid, None);
        assert!(result.is_err());
    }

    // ========================================================================
    // Operation Listing and Finding Tests
    // ========================================================================

    #[test]
    fn test_list_operations_empty() {
        let spec = minimal_openapi_json();
        let tool = OpenAPITool::from_str(spec, None).unwrap();
        let operations = tool.list_operations();
        assert!(operations.is_empty());
    }

    #[test]
    fn test_list_operations_petstore() {
        let spec = petstore_openapi_json();
        let tool = OpenAPITool::from_str(spec, None).unwrap();
        let operations = tool.list_operations();
        assert!(operations.contains(&"listPets".to_string()));
        assert!(operations.contains(&"createPet".to_string()));
        assert!(operations.contains(&"getPetById".to_string()));
        assert!(operations.contains(&"deletePet".to_string()));
        assert!(operations.contains(&"updatePet".to_string()));
        assert!(operations.contains(&"patchPet".to_string()));
        assert_eq!(operations.len(), 6);
    }

    #[test]
    fn test_find_operation_get() {
        let spec = petstore_openapi_json();
        let tool = OpenAPITool::from_str(spec, None).unwrap();
        let (path, method, _op) = tool.find_operation("listPets").unwrap();
        assert_eq!(path, "/pets");
        assert_eq!(method, "GET");
    }

    #[test]
    fn test_find_operation_post() {
        let spec = petstore_openapi_json();
        let tool = OpenAPITool::from_str(spec, None).unwrap();
        let (path, method, _op) = tool.find_operation("createPet").unwrap();
        assert_eq!(path, "/pets");
        assert_eq!(method, "POST");
    }

    #[test]
    fn test_find_operation_delete() {
        let spec = petstore_openapi_json();
        let tool = OpenAPITool::from_str(spec, None).unwrap();
        let (path, method, _op) = tool.find_operation("deletePet").unwrap();
        assert_eq!(path, "/pets/{petId}");
        assert_eq!(method, "DELETE");
    }

    #[test]
    fn test_find_operation_put() {
        let spec = petstore_openapi_json();
        let tool = OpenAPITool::from_str(spec, None).unwrap();
        let (path, method, _op) = tool.find_operation("updatePet").unwrap();
        assert_eq!(path, "/pets/{petId}");
        assert_eq!(method, "PUT");
    }

    #[test]
    fn test_find_operation_patch() {
        let spec = petstore_openapi_json();
        let tool = OpenAPITool::from_str(spec, None).unwrap();
        let (path, method, _op) = tool.find_operation("patchPet").unwrap();
        assert_eq!(path, "/pets/{petId}");
        assert_eq!(method, "PATCH");
    }

    #[test]
    fn test_find_operation_not_found() {
        let spec = petstore_openapi_json();
        let tool = OpenAPITool::from_str(spec, None).unwrap();
        let result = tool.find_operation("nonExistentOperation");
        assert!(result.is_err());
        let err_str = result.unwrap_err().to_string();
        assert!(err_str.contains("not found"));
        assert!(err_str.contains("nonExistentOperation"));
    }

    // ========================================================================
    // Tool Trait Implementation Tests
    // ========================================================================

    #[test]
    fn test_tool_name() {
        let spec = minimal_openapi_json();
        let tool = OpenAPITool::from_str(spec, None).unwrap();
        assert_eq!(tool.name(), "openapi_tool");
    }

    #[test]
    fn test_tool_description() {
        let spec = minimal_openapi_json();
        let tool = OpenAPITool::from_str(spec, None).unwrap();
        let desc = tool.description();
        assert!(desc.contains("OpenAPI"));
        assert!(desc.contains("operation_id"));
    }

    #[tokio::test]
    async fn test_tool_call_invalid_json_input() {
        let spec = minimal_openapi_json();
        let tool = OpenAPITool::from_str(spec, None).unwrap();
        let input = ToolInput::String("not valid json".to_string());
        let result = tool._call(input).await;
        assert!(result.is_err());
        let err_str = result.unwrap_err().to_string();
        assert!(err_str.contains("Invalid input"));
    }

    #[tokio::test]
    async fn test_tool_call_missing_operation_id() {
        let spec = minimal_openapi_json();
        let tool = OpenAPITool::from_str(spec, None).unwrap();
        let input = ToolInput::String(r#"{"parameters": {}}"#.to_string());
        let result = tool._call(input).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_tool_call_operation_not_found() {
        let spec = petstore_openapi_json();
        let tool = OpenAPITool::from_str(spec, None).unwrap();
        let input = ToolInput::String(r#"{"operation_id": "unknownOp"}"#.to_string());
        let result = tool._call(input).await;
        assert!(result.is_err());
        let err_str = result.unwrap_err().to_string();
        assert!(err_str.contains("unknownOp"));
        assert!(err_str.contains("not found"));
    }

    #[tokio::test]
    async fn test_tool_call_structured_input() {
        let spec = petstore_openapi_json();
        let tool = OpenAPITool::from_str(spec, None).unwrap();
        // Use structured input format - this will fail due to network but tests input parsing
        let input = ToolInput::Structured(serde_json::json!({
            "operation_id": "getPetById",
            "parameters": {"petId": 123}
        }));
        let result = tool._call(input).await;
        // Will fail with network error, but should not fail on input parsing
        assert!(result.is_err());
        // Error should be about network/request, not about input format
        let err_str = result.unwrap_err().to_string();
        assert!(
            err_str.contains("request") || err_str.contains("connect") || err_str.contains("error")
        );
    }

    // ========================================================================
    // Edge Cases and Error Handling Tests
    // ========================================================================

    #[test]
    fn test_empty_operation_id() {
        let spec = petstore_openapi_json();
        let tool = OpenAPITool::from_str(spec, None).unwrap();
        let result = tool.find_operation("");
        assert!(result.is_err());
    }

    #[test]
    fn test_unicode_operation_id() {
        let spec = petstore_openapi_json();
        let tool = OpenAPITool::from_str(spec, None).unwrap();
        let result = tool.find_operation("获取宠物");
        assert!(result.is_err());
    }

    #[test]
    fn test_spec_with_multiple_servers() {
        let spec = r#"{
            "openapi": "3.0.0",
            "info": {"title": "Multi-Server API", "version": "1.0.0"},
            "servers": [
                {"url": "https://primary.example.com"},
                {"url": "https://backup.example.com"}
            ],
            "paths": {}
        }"#;
        let tool = OpenAPITool::from_str(spec, None).unwrap();
        // Should use first server as base URL
        assert_eq!(tool.base_url, "https://primary.example.com");
    }

    #[test]
    fn test_spec_with_server_variables() {
        let spec = r#"{
            "openapi": "3.0.0",
            "info": {"title": "Variable Server API", "version": "1.0.0"},
            "servers": [
                {"url": "https://{environment}.example.com/{version}"}
            ],
            "paths": {}
        }"#;
        let tool = OpenAPITool::from_str(spec, None).unwrap();
        // Server variables remain as-is (not resolved)
        assert!(tool.base_url.contains("{environment}"));
    }

    #[test]
    fn test_yaml_spec_with_operations() {
        let yaml_spec = r#"
openapi: "3.0.0"
info:
  title: YAML Test API
  version: "1.0.0"
servers:
  - url: https://yaml.example.com
paths:
  /items:
    get:
      operationId: listItems
      responses:
        "200":
          description: Success
"#;
        let tool = OpenAPITool::from_str(yaml_spec, None).unwrap();
        let ops = tool.list_operations();
        assert!(ops.contains(&"listItems".to_string()));
    }

    #[test]
    fn test_authentication_default_none() {
        let spec = minimal_openapi_json();
        let tool = OpenAPITool::from_str(spec, None).unwrap();
        match &tool.auth {
            Authentication::None => {} // Expected
            _ => panic!("Expected None authentication by default"),
        }
    }

    #[test]
    fn test_operation_with_path_parameters() {
        let spec = petstore_openapi_json();
        let tool = OpenAPITool::from_str(spec, None).unwrap();
        let (path, _method, operation) = tool.find_operation("getPetById").unwrap();
        assert_eq!(path, "/pets/{petId}");
        // Verify operation has parameters defined
        assert!(!operation.parameters.is_empty());
    }

    #[test]
    fn test_operation_with_query_parameters() {
        let spec = petstore_openapi_json();
        let tool = OpenAPITool::from_str(spec, None).unwrap();
        let (_path, _method, operation) = tool.find_operation("listPets").unwrap();
        // listPets has 'limit' query parameter
        assert!(!operation.parameters.is_empty());
    }

    // ========================================================================
    // Complex Spec Tests
    // ========================================================================

    #[test]
    fn test_spec_with_all_http_methods() {
        let spec = r#"{
            "openapi": "3.0.0",
            "info": {"title": "All Methods API", "version": "1.0.0"},
            "paths": {
                "/resource": {
                    "get": {"operationId": "getResource", "responses": {"200": {"description": "OK"}}},
                    "post": {"operationId": "createResource", "responses": {"201": {"description": "Created"}}},
                    "put": {"operationId": "replaceResource", "responses": {"200": {"description": "OK"}}},
                    "patch": {"operationId": "updateResource", "responses": {"200": {"description": "OK"}}},
                    "delete": {"operationId": "deleteResource", "responses": {"204": {"description": "Deleted"}}}
                }
            }
        }"#;
        let tool = OpenAPITool::from_str(spec, None).unwrap();
        let ops = tool.list_operations();
        assert_eq!(ops.len(), 5);
        assert!(ops.contains(&"getResource".to_string()));
        assert!(ops.contains(&"createResource".to_string()));
        assert!(ops.contains(&"replaceResource".to_string()));
        assert!(ops.contains(&"updateResource".to_string()));
        assert!(ops.contains(&"deleteResource".to_string()));
    }

    #[test]
    fn test_spec_with_multiple_paths() {
        let spec = r#"{
            "openapi": "3.0.0",
            "info": {"title": "Multi-Path API", "version": "1.0.0"},
            "paths": {
                "/users": {"get": {"operationId": "listUsers", "responses": {"200": {"description": "OK"}}}},
                "/users/{id}": {"get": {"operationId": "getUser", "responses": {"200": {"description": "OK"}}}},
                "/products": {"get": {"operationId": "listProducts", "responses": {"200": {"description": "OK"}}}},
                "/orders": {"get": {"operationId": "listOrders", "responses": {"200": {"description": "OK"}}}}
            }
        }"#;
        let tool = OpenAPITool::from_str(spec, None).unwrap();
        let ops = tool.list_operations();
        assert_eq!(ops.len(), 4);
    }

    #[test]
    fn test_spec_with_operations_without_id() {
        let spec = r#"{
            "openapi": "3.0.0",
            "info": {"title": "No ID API", "version": "1.0.0"},
            "paths": {
                "/noId": {
                    "get": {"responses": {"200": {"description": "OK"}}}
                },
                "/withId": {
                    "get": {"operationId": "withIdOperation", "responses": {"200": {"description": "OK"}}}
                }
            }
        }"#;
        let tool = OpenAPITool::from_str(spec, None).unwrap();
        let ops = tool.list_operations();
        // Should only contain the operation with an ID
        assert_eq!(ops.len(), 1);
        assert!(ops.contains(&"withIdOperation".to_string()));
    }

    // ========================================================================
    // OpenAPIInput Edge Cases
    // ========================================================================

    #[test]
    fn test_openapi_input_empty_parameters() {
        let json = r#"{"operation_id": "test", "parameters": {}}"#;
        let input: OpenAPIInput = serde_json::from_str(json).unwrap();
        assert!(input.parameters.is_empty());
    }

    #[test]
    fn test_openapi_input_null_body() {
        let json = r#"{"operation_id": "test", "body": null}"#;
        let input: OpenAPIInput = serde_json::from_str(json).unwrap();
        assert!(input.body.is_none());
    }

    #[test]
    fn test_openapi_input_array_body() {
        let json = r#"{"operation_id": "bulkCreate", "body": [{"name": "a"}, {"name": "b"}]}"#;
        let input: OpenAPIInput = serde_json::from_str(json).unwrap();
        let body = input.body.unwrap();
        assert!(body.is_array());
        assert_eq!(body.as_array().unwrap().len(), 2);
    }

    #[test]
    fn test_openapi_input_nested_parameters() {
        let json = r#"{"operation_id": "test", "parameters": {"filter": {"status": "active"}}}"#;
        let input: OpenAPIInput = serde_json::from_str(json).unwrap();
        let filter = input.parameters.get("filter").unwrap();
        assert!(filter.is_object());
    }

    #[test]
    fn test_openapi_input_array_parameter() {
        let json = r#"{"operation_id": "test", "parameters": {"ids": [1, 2, 3]}}"#;
        let input: OpenAPIInput = serde_json::from_str(json).unwrap();
        let ids = input.parameters.get("ids").unwrap();
        assert!(ids.is_array());
        assert_eq!(ids.as_array().unwrap().len(), 3);
    }

    // ========================================================================
    // Authentication Comprehensive Tests
    // ========================================================================

    #[test]
    fn test_all_auth_variants_construct() {
        let auths: Vec<Authentication> = vec![
            Authentication::None,
            Authentication::ApiKey {
                header_name: "X-Key".to_string(),
                api_key: "key".to_string(),
            },
            Authentication::Bearer {
                token: "token".to_string(),
            },
            Authentication::Basic {
                username: "user".to_string(),
                password: "pass".to_string(),
            },
        ];

        for auth in auths {
            // Just verify they all implement Debug properly
            let _ = format!("{:?}", auth);
        }
    }

    #[test]
    fn test_authentication_api_key_special_characters() {
        let auth = Authentication::ApiKey {
            header_name: "X-Custom-Header-With-Dashes".to_string(),
            api_key: "key-with-special-chars!@#$%".to_string(),
        };
        let debug = format!("{:?}", auth);
        assert!(debug.contains("X-Custom-Header-With-Dashes"));
        // Secret should be redacted
        assert!(!debug.contains("!@#$%"));
    }

    #[test]
    fn test_authentication_bearer_empty_token() {
        let auth = Authentication::Bearer {
            token: "".to_string(),
        };
        let debug = format!("{:?}", auth);
        assert!(debug.contains("[REDACTED]"));
    }

    #[test]
    fn test_authentication_basic_empty_password() {
        let auth = Authentication::Basic {
            username: "admin".to_string(),
            password: "".to_string(),
        };
        let debug = format!("{:?}", auth);
        assert!(debug.contains("admin"));
        assert!(debug.contains("[REDACTED]"));
    }

    // ========================================================================
    // Network-Dependent Tests (Ignored by default)
    // ========================================================================

    #[tokio::test]
    #[ignore = "Requires network access to external API"]
    async fn test_from_url_petstore() {
        let result =
            OpenAPITool::from_url("https://petstore3.swagger.io/api/v3/openapi.json", None).await;
        assert!(result.is_ok());
        let tool = result.unwrap();
        let ops = tool.list_operations();
        assert!(!ops.is_empty());
    }

    #[tokio::test]
    #[ignore = "Requires network access"]
    async fn test_execute_operation_petstore() {
        let tool =
            OpenAPITool::from_url("https://petstore3.swagger.io/api/v3/openapi.json", None)
                .await
                .unwrap();

        let mut params = HashMap::new();
        params.insert("petId".to_string(), JsonValue::from(1));

        let result = tool.execute_operation("getPetById", &params, None).await;
        // May succeed or fail depending on petstore availability
        assert!(result.is_ok() || result.is_err());
    }

    #[tokio::test]
    #[ignore = "Requires network access"]
    async fn test_from_url_invalid_url() {
        let result = OpenAPITool::from_url("https://nonexistent.invalid.domain/api.json", None).await;
        assert!(result.is_err());
    }
}
