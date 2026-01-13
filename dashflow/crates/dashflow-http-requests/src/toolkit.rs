use crate::{HttpDeleteTool, HttpGetTool, HttpPatchTool, HttpPostTool, HttpPutTool};
use dashflow::core::tools::{BaseToolkit, Tool};
use std::sync::Arc;

/// Toolkit for making HTTP REST requests
///
/// Provides tools for all HTTP methods: GET, POST, PUT, PATCH, DELETE.
/// Enables agents to interact with REST APIs and web services.
///
/// # Security Note
///
/// This toolkit contains tools to make GET, POST, PATCH, PUT, and DELETE requests to APIs.
///
/// Exercise care in who is allowed to use this toolkit. If exposing to end users,
/// consider that users will be able to make arbitrary requests on behalf of the server
/// hosting the code. For example, users could ask the server to make a request to a
/// private API that is only accessible from the server.
///
/// Control access to who can submit requests using this toolkit and what network access it has.
///
/// # Example
///
/// ```rust
/// use dashflow_http_requests::RequestsToolkit;
/// use dashflow::core::tools::BaseToolkit;
///
/// let toolkit = RequestsToolkit::new();
/// let tools = toolkit.get_tools();
///
/// // tools contains: HttpGetTool, HttpPostTool, HttpPutTool, HttpPatchTool, HttpDeleteTool
/// assert_eq!(tools.len(), 5);
/// ```
///
/// # Example with Agent
///
/// ```rust,no_run
/// use dashflow_http_requests::RequestsToolkit;
/// use dashflow::core::tools::BaseToolkit;
/// use serde_json::json;
///
/// #[tokio::main]
/// async fn main() {
///     let toolkit = RequestsToolkit::new();
///     let tools = toolkit.get_tools();
///
///     // Pass tools to agent
///     // let agent = create_react_agent(llm, tools, system_message);
///
///     // Agent can now make HTTP requests
///     // Example: "Fetch the data from https://api.example.com/users"
/// }
/// ```
#[derive(Debug, Clone)]
pub struct RequestsToolkit {}

impl RequestsToolkit {
    /// Create a new `RequestsToolkit`
    ///
    /// # Example
    ///
    /// ```rust
    /// use dashflow_http_requests::RequestsToolkit;
    ///
    /// let toolkit = RequestsToolkit::new();
    /// ```
    #[must_use]
    pub fn new() -> Self {
        Self {}
    }
}

impl Default for RequestsToolkit {
    fn default() -> Self {
        Self::new()
    }
}

impl BaseToolkit for RequestsToolkit {
    fn get_tools(&self) -> Vec<Arc<dyn Tool>> {
        vec![
            Arc::new(HttpGetTool::new()),
            Arc::new(HttpPostTool::new()),
            Arc::new(HttpPutTool::new()),
            Arc::new(HttpPatchTool::new()),
            Arc::new(HttpDeleteTool::new()),
        ]
    }
}

#[cfg(test)]
mod tests {
    // `cargo verify` runs clippy with `-D warnings` for all targets, including unit tests.
    #![allow(clippy::expect_used)]

    use super::*;

    #[test]
    fn test_requests_toolkit_construction() {
        let toolkit = RequestsToolkit::new();
        let tools = toolkit.get_tools();

        assert_eq!(tools.len(), 5);

        // Verify tool names
        let tool_names: Vec<&str> = tools.iter().map(|t| t.name()).collect();
        assert!(tool_names.contains(&"http_get"));
        assert!(tool_names.contains(&"http_post"));
        assert!(tool_names.contains(&"http_put"));
        assert!(tool_names.contains(&"http_patch"));
        assert!(tool_names.contains(&"http_delete"));
    }

    #[test]
    fn test_requests_toolkit_default() {
        let toolkit = RequestsToolkit::default();
        let tools = toolkit.get_tools();
        assert_eq!(tools.len(), 5);
    }

    #[test]
    fn test_requests_toolkit_clone() {
        let toolkit1 = RequestsToolkit::new();
        let toolkit2 = toolkit1.clone();

        assert_eq!(toolkit1.get_tools().len(), toolkit2.get_tools().len());
    }

    #[test]
    fn test_requests_toolkit_tools_descriptions() {
        let toolkit = RequestsToolkit::new();
        let tools = toolkit.get_tools();

        // All tools should have descriptions
        for tool in tools {
            assert!(!tool.description().is_empty());
            assert!(tool.description().len() > 20); // Reasonable description length
        }
    }

    #[test]
    fn test_requests_toolkit_get_tool() {
        let toolkit = RequestsToolkit::new();
        let tools = toolkit.get_tools();

        // Verify HTTP GET tool
        let get_tool = tools
            .iter()
            .find(|t| t.name() == "http_get")
            .expect("http_get tool should exist");
        assert!(get_tool.description().contains("GET"));
    }

    #[test]
    fn test_requests_toolkit_post_tool() {
        let toolkit = RequestsToolkit::new();
        let tools = toolkit.get_tools();

        // Verify HTTP POST tool
        let post_tool = tools
            .iter()
            .find(|t| t.name() == "http_post")
            .expect("http_post tool should exist");
        assert!(post_tool.description().contains("POST"));
    }

    #[test]
    fn test_requests_toolkit_put_tool() {
        let toolkit = RequestsToolkit::new();
        let tools = toolkit.get_tools();

        // Verify HTTP PUT tool
        let put_tool = tools
            .iter()
            .find(|t| t.name() == "http_put")
            .expect("http_put tool should exist");
        assert!(put_tool.description().contains("PUT"));
    }

    #[test]
    fn test_requests_toolkit_patch_tool() {
        let toolkit = RequestsToolkit::new();
        let tools = toolkit.get_tools();

        // Verify HTTP PATCH tool
        let patch_tool = tools
            .iter()
            .find(|t| t.name() == "http_patch")
            .expect("http_patch tool should exist");
        assert!(patch_tool.description().contains("PATCH"));
    }

    #[test]
    fn test_requests_toolkit_delete_tool() {
        let toolkit = RequestsToolkit::new();
        let tools = toolkit.get_tools();

        // Verify HTTP DELETE tool
        let delete_tool = tools
            .iter()
            .find(|t| t.name() == "http_delete")
            .expect("http_delete tool should exist");
        assert!(delete_tool.description().contains("DELETE"));
    }
}
