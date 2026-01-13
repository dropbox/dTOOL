//! `OpenAPI` toolkit for interacting with OpenAPI-compliant APIs
//!
//! This module provides the `OpenAPIToolkit` which combines HTTP request tools
//! with JSON navigation capabilities to enable agents to interact with OpenAPI/Swagger APIs.

use crate::RequestsToolkit;
use dashflow::core::tools::{BaseToolkit, Tool};
use dashflow_json::{JsonSpec, JsonToolkit};
use std::sync::Arc;

/// Toolkit for interacting with an `OpenAPI` API.
///
/// `OpenAPIToolkit` combines HTTP request capabilities with JSON navigation tools
/// to enable agents to interact with OpenAPI-compliant REST APIs. It bundles:
///
/// 1. **`RequestsToolkit`** - All HTTP methods (GET, POST, PUT, PATCH, DELETE)
/// 2. **JSON Explorer Tool** - Navigate and query JSON responses
///
/// # Security Note
///
/// This toolkit contains tools that can read and modify the state of a service;
/// e.g., by creating, deleting, or updating, reading underlying data.
///
/// For example, this toolkit can be used to delete data exposed via an `OpenAPI`
/// compliant API.
///
/// Exercise care in who is allowed to use this toolkit. If exposing to end users,
/// consider that users will be able to make arbitrary requests on behalf of the server
/// hosting the code.
///
/// Control access to who can submit requests using this toolkit and what network access it has.
///
/// See <https://python.dashflow.com/docs/security> for more information.
///
/// # Example
///
/// ```rust
/// use dashflow_http_requests::OpenAPIToolkit;
/// use dashflow_json::JsonSpec;
/// use dashflow::core::tools::BaseToolkit;
/// use serde_json::json;
///
/// # #[tokio::main]
/// # async fn main() {
/// // Create JSON spec for OpenAPI schema
/// let api_spec = json!({
///     "openapi": "3.0.0",
///     "info": {
///         "title": "Example API",
///         "version": "1.0.0"
///     },
///     "paths": {
///         "/users": {
///             "get": {
///                 "summary": "Get users",
///                 "responses": {
///                     "200": {
///                         "description": "Success"
///                     }
///                 }
///             }
///         }
///     }
/// });
///
/// let json_spec = JsonSpec::new(api_spec);
/// let toolkit = OpenAPIToolkit::new(json_spec);
///
/// // Get all tools (5 HTTP tools + 2 JSON tools = 7 total)
/// let tools = toolkit.get_tools();
/// assert_eq!(tools.len(), 7);
/// # }
/// ```
///
/// # Example with Agent
///
/// ```rust,no_run
/// use dashflow_http_requests::OpenAPIToolkit;
/// use dashflow_json::JsonSpec;
/// use dashflow::core::tools::BaseToolkit;
/// use serde_json::json;
///
/// #[tokio::main]
/// async fn main() {
///     // Load OpenAPI spec
///     let api_spec = json!({
///         "openapi": "3.0.0",
///         "paths": {
///             "/posts": {
///                 "get": {"summary": "Get posts"}
///             }
///         }
///     });
///
///     let json_spec = JsonSpec::new(api_spec);
///     let toolkit = OpenAPIToolkit::new(json_spec);
///     let tools = toolkit.get_tools();
///
///     // Use with agent (pseudo-code)
///     // let system_message = format!("API Spec: {}", serde_json::to_string_pretty(&api_spec)?);
///     // let agent = create_react_agent(llm, tools, &system_message);
///     // let result = agent.run("Fetch the first 5 posts from the API").await?;
/// }
/// ```
#[derive(Clone)]
pub struct OpenAPIToolkit {
    json_spec: JsonSpec,
    requests_toolkit: RequestsToolkit,
    json_toolkit: JsonToolkit,
}

impl OpenAPIToolkit {
    /// Creates a new `OpenAPIToolkit` with the given `OpenAPI` specification.
    ///
    /// The toolkit will provide both HTTP request tools and JSON navigation tools
    /// to interact with the API defined in the spec.
    ///
    /// # Arguments
    ///
    /// * `json_spec` - The `OpenAPI` specification as a `JsonSpec`
    ///
    /// # Example
    ///
    /// ```rust
    /// use dashflow_http_requests::OpenAPIToolkit;
    /// use dashflow_json::JsonSpec;
    /// use serde_json::json;
    ///
    /// let spec = json!({
    ///     "openapi": "3.0.0",
    ///     "info": {"title": "My API", "version": "1.0.0"}
    /// });
    ///
    /// let json_spec = JsonSpec::new(spec);
    /// let toolkit = OpenAPIToolkit::new(json_spec);
    /// ```
    #[must_use]
    pub fn new(json_spec: JsonSpec) -> Self {
        let json_toolkit = JsonToolkit::new(json_spec.clone());
        Self {
            json_spec,
            requests_toolkit: RequestsToolkit::new(),
            json_toolkit,
        }
    }

    /// Creates a new `OpenAPIToolkit` from an `OpenAPI` spec string (YAML or JSON).
    ///
    /// This is a convenience constructor that parses the `OpenAPI` spec and creates
    /// the toolkit.
    ///
    /// # Arguments
    ///
    /// * `spec_str` - The `OpenAPI` specification as a JSON or YAML string
    ///
    /// # Returns
    ///
    /// Returns `Ok(OpenAPIToolkit)` if parsing succeeds, or an error if the spec is invalid.
    ///
    /// # Example
    ///
    /// ```rust
    /// use dashflow_http_requests::OpenAPIToolkit;
    ///
    /// let spec_json = r#"{
    ///     "openapi": "3.0.0",
    ///     "info": {"title": "API", "version": "1.0.0"}
    /// }"#;
    ///
    /// let toolkit = OpenAPIToolkit::from_spec_str(spec_json).unwrap();
    /// ```
    pub fn from_spec_str(spec_str: &str) -> Result<Self, serde_json::Error> {
        let spec_value: serde_json::Value = serde_json::from_str(spec_str)?;
        let json_spec = JsonSpec::new(spec_value);
        Ok(Self::new(json_spec))
    }

    /// Returns a reference to the `OpenAPI` JSON specification.
    #[must_use]
    pub fn spec(&self) -> &JsonSpec {
        &self.json_spec
    }

    /// Returns a reference to the underlying `RequestsToolkit`.
    #[must_use]
    pub fn requests_toolkit(&self) -> &RequestsToolkit {
        &self.requests_toolkit
    }

    /// Returns a reference to the underlying `JsonToolkit`.
    #[must_use]
    pub fn json_toolkit(&self) -> &JsonToolkit {
        &self.json_toolkit
    }
}

impl BaseToolkit for OpenAPIToolkit {
    fn get_tools(&self) -> Vec<Arc<dyn Tool>> {
        let mut tools = Vec::new();

        // Add HTTP request tools (5 tools)
        tools.extend(self.requests_toolkit.get_tools());

        // Add JSON navigation tools (2 tools)
        tools.extend(self.json_toolkit.get_tools());

        tools
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn create_test_spec() -> JsonSpec {
        let spec = json!({
            "openapi": "3.0.0",
            "info": {
                "title": "Test API",
                "version": "1.0.0"
            },
            "servers": [
                {"url": "https://api.example.com"}
            ],
            "paths": {
                "/users": {
                    "get": {
                        "summary": "Get users",
                        "responses": {
                            "200": {
                                "description": "Success"
                            }
                        }
                    },
                    "post": {
                        "summary": "Create user",
                        "responses": {
                            "201": {
                                "description": "Created"
                            }
                        }
                    }
                },
                "/users/{id}": {
                    "get": {
                        "summary": "Get user by ID",
                        "parameters": [
                            {
                                "name": "id",
                                "in": "path",
                                "required": true,
                                "schema": {"type": "integer"}
                            }
                        ]
                    },
                    "delete": {
                        "summary": "Delete user"
                    }
                }
            }
        });
        JsonSpec::new(spec)
    }

    #[test]
    fn test_openapi_toolkit_construction() {
        let json_spec = create_test_spec();
        let toolkit = OpenAPIToolkit::new(json_spec);

        // Verify we can get tools
        let tools = toolkit.get_tools();

        // Should have 5 HTTP tools + 2 JSON tools = 7 total
        assert_eq!(tools.len(), 7);
    }

    #[test]
    fn test_openapi_toolkit_default() {
        let json_spec = create_test_spec();
        let toolkit1 = OpenAPIToolkit::new(json_spec.clone());
        let toolkit2 = OpenAPIToolkit::new(json_spec);

        // Both toolkits should provide the same number of tools
        assert_eq!(toolkit1.get_tools().len(), toolkit2.get_tools().len());
    }

    #[test]
    fn test_openapi_toolkit_tool_count() {
        let json_spec = create_test_spec();
        let toolkit = OpenAPIToolkit::new(json_spec);
        let tools = toolkit.get_tools();

        // Verify we have exactly 7 tools
        assert_eq!(tools.len(), 7, "Expected 7 tools (5 HTTP + 2 JSON)");
    }

    #[test]
    fn test_openapi_toolkit_tool_names() {
        let json_spec = create_test_spec();
        let toolkit = OpenAPIToolkit::new(json_spec);
        let tools = toolkit.get_tools();

        let tool_names: Vec<String> = tools.iter().map(|t| t.name().to_string()).collect();

        // Check for HTTP tools
        assert!(tool_names.contains(&"http_get".to_string()));
        assert!(tool_names.contains(&"http_post".to_string()));
        assert!(tool_names.contains(&"http_put".to_string()));
        assert!(tool_names.contains(&"http_patch".to_string()));
        assert!(tool_names.contains(&"http_delete".to_string()));

        // Check for JSON tools
        assert!(tool_names.contains(&"json_spec_list_keys".to_string()));
        assert!(tool_names.contains(&"json_spec_get_value".to_string()));
    }

    #[test]
    fn test_openapi_toolkit_has_http_get_tool() -> Result<(), &'static str> {
        let json_spec = create_test_spec();
        let toolkit = OpenAPIToolkit::new(json_spec);
        let tools = toolkit.get_tools();

        let Some(tool) = tools.iter().find(|t| t.name() == "http_get") else {
            return Err("HTTP GET tool should be present");
        };
        assert!(
            tool.description().contains("GET"),
            "Tool description should mention GET method"
        );
        Ok(())
    }

    #[test]
    fn test_openapi_toolkit_has_json_tools() {
        let json_spec = create_test_spec();
        let toolkit = OpenAPIToolkit::new(json_spec);
        let tools = toolkit.get_tools();

        let json_list_keys = tools.iter().find(|t| t.name() == "json_spec_list_keys");
        assert!(
            json_list_keys.is_some(),
            "JSON list keys tool should be present"
        );

        let json_get_value = tools.iter().find(|t| t.name() == "json_spec_get_value");
        assert!(
            json_get_value.is_some(),
            "JSON get value tool should be present"
        );
    }

    #[test]
    fn test_openapi_toolkit_accessors() {
        let json_spec = create_test_spec();
        let toolkit = OpenAPIToolkit::new(json_spec);

        // Test accessors
        let _spec = toolkit.spec();
        let _requests = toolkit.requests_toolkit();
        let _json = toolkit.json_toolkit();

        // Verify requests toolkit works
        assert_eq!(toolkit.requests_toolkit().get_tools().len(), 5);

        // Verify json toolkit works
        assert_eq!(toolkit.json_toolkit().get_tools().len(), 2);
    }

    #[test]
    fn test_openapi_toolkit_clone() {
        let json_spec = create_test_spec();
        let toolkit1 = OpenAPIToolkit::new(json_spec);
        let toolkit2 = toolkit1.clone();

        // Both toolkits should provide the same tools
        assert_eq!(toolkit1.get_tools().len(), toolkit2.get_tools().len());
    }

    #[test]
    fn test_openapi_toolkit_from_spec_str_json() -> Result<(), serde_json::Error> {
        let spec_json = r#"{
            "openapi": "3.0.0",
            "info": {
                "title": "Test API",
                "version": "1.0.0"
            },
            "paths": {
                "/test": {
                    "get": {"summary": "Test endpoint"}
                }
            }
        }"#;

        let toolkit = OpenAPIToolkit::from_spec_str(spec_json)?;
        let tools = toolkit.get_tools();

        assert_eq!(tools.len(), 7);
        Ok(())
    }

    #[test]
    fn test_openapi_toolkit_from_spec_str_invalid() {
        let invalid_json = "not valid json {{{";

        let result = OpenAPIToolkit::from_spec_str(invalid_json);
        assert!(result.is_err(), "Should return error for invalid JSON");
    }

    #[test]
    fn test_openapi_toolkit_minimal_spec() {
        let minimal_spec = json!({
            "openapi": "3.0.0"
        });

        let json_spec = JsonSpec::new(minimal_spec);
        let toolkit = OpenAPIToolkit::new(json_spec);
        let tools = toolkit.get_tools();

        // Should still provide all tools even with minimal spec
        assert_eq!(tools.len(), 7);
    }

    #[test]
    fn test_openapi_toolkit_tool_descriptions_not_empty() {
        let json_spec = create_test_spec();
        let toolkit = OpenAPIToolkit::new(json_spec);
        let tools = toolkit.get_tools();

        for tool in tools {
            assert!(
                !tool.description().is_empty(),
                "Tool '{}' should have a non-empty description",
                tool.name()
            );
        }
    }
}
