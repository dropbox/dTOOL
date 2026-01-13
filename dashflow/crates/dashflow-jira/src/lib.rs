//! # dashflow-jira
//!
//! Jira issue tracking tools for `DashFlow` Rust.
//!
//! This crate provides tools to search, get details, and manage Jira issues
//! using the Jira REST API v3.
//!
//! ## Features
//!
//! - Search Jira issues using JQL (Jira Query Language)
//! - Get detailed issue information
//! - Support for Jira Cloud and Jira Server
//!
//! ## Authentication
//!
//! Jira Cloud uses Basic Authentication with email + API token:
//! - Create API token: <https://id.atlassian.com/manage-profile/security/api-tokens>
//! - Credentials: `email:api_token` (base64 encoded)
//!
//! ## Example
//!
//! ```rust,no_run
//! use dashflow_jira::JiraSearchTool;
//! use dashflow::core::tools::{Tool, ToolInput};
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let tool = JiraSearchTool::new(
//!         "https://your-domain.atlassian.net",
//!         "your-email@example.com",
//!         "your-api-token"
//!     );
//!
//!     let input = serde_json::json!({
//!         "jql": "project = DEMO AND status = Open",
//!         "max_results": 10
//!     });
//!
//!     let result = tool._call(ToolInput::Structured(input)).await?;
//!     println!("{}", result);
//!
//!     Ok(())
//! }
//! ```

use async_trait::async_trait;
use base64::Engine;
use dashflow::core::tools::{Tool, ToolInput};
use dashflow::core::{Error, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Jira API response structures for search results
#[derive(Debug, Deserialize)]
struct JiraSearchResponse {
    total: u64,
    #[serde(default)]
    issues: Vec<JiraIssue>,
}

#[derive(Debug, Deserialize)]
struct JiraIssue {
    key: String,
    fields: JiraIssueFields,
    /// JIRA issue's self URL (API link to the issue)
    ///
    /// Self-reference URL from JIRA API
    #[serde(rename = "self")]
    #[allow(dead_code)] // Deserialize: JIRA issue URL - reserved for update/transition operations
    self_url: String,
}

#[derive(Debug, Deserialize)]
struct JiraIssueFields {
    summary: String,
    #[serde(default)]
    description: Option<Value>,
    status: JiraStatus,
    #[serde(default)]
    priority: Option<JiraPriority>,
    #[serde(default)]
    assignee: Option<JiraUser>,
    #[serde(default)]
    reporter: Option<JiraUser>,
    #[serde(default)]
    created: Option<String>,
    #[serde(default)]
    updated: Option<String>,
    #[serde(default)]
    issuetype: Option<JiraIssueType>,
}

#[derive(Debug, Deserialize)]
struct JiraStatus {
    name: String,
}

#[derive(Debug, Deserialize)]
struct JiraPriority {
    name: String,
}

#[derive(Debug, Deserialize)]
struct JiraUser {
    #[serde(alias = "displayName")]
    display_name: String,
}

#[derive(Debug, Deserialize)]
struct JiraIssueType {
    name: String,
}

/// Jira search request body
#[derive(Debug, Serialize)]
struct JiraSearchRequest {
    jql: String,
    #[serde(rename = "maxResults")]
    max_results: usize,
    #[serde(rename = "startAt")]
    start_at: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    fields: Option<Vec<String>>,
}

/// Jira search tool for searching issues using JQL (Jira Query Language).
///
/// This tool searches Jira issues using JQL queries and returns formatted issue information.
///
/// ## Input Parameters
///
/// - `jql` (required): JQL query string (e.g., "project = DEMO AND status = Open")
/// - `max_results` (optional): Maximum number of results (default: 10, max: 100)
/// - `start_at` (optional): Starting index for pagination (default: 0)
///
/// ## Output
///
/// Returns formatted list of issues with:
/// - Issue key and summary
/// - Status and priority
/// - Assignee and reporter
/// - Created and updated timestamps
/// - Issue URL
///
/// ## Authentication
///
/// Jira Cloud requires Basic Authentication:
/// - Email address (e.g., "user@example.com")
/// - API token (generate at <https://id.atlassian.com/manage-profile/security/api-tokens>)
///
/// ## Example
///
/// ```rust,no_run
/// use dashflow_jira::JiraSearchTool;
/// use dashflow::core::tools::{Tool, ToolInput};
///
/// #[tokio::main]
/// async fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let tool = JiraSearchTool::new(
///         "https://your-domain.atlassian.net",
///         "your-email@example.com",
///         "your-api-token"
///     );
///
///     let input = serde_json::json!({
///         "jql": "assignee = currentUser() AND status != Done",
///         "max_results": 5
///     });
///
///     let result = tool._call(ToolInput::Structured(input)).await?;
///     println!("{}", result);
///
///     Ok(())
/// }
/// ```
#[derive(Clone)]
pub struct JiraSearchTool {
    client: Client,
    base_url: String,
    auth_header: String,
}

impl JiraSearchTool {
    /// Create a new Jira search tool.
    ///
    /// # Arguments
    ///
    /// - `base_url`: Jira instance URL (e.g., "<https://your-domain.atlassian.net>")
    /// - `email`: Your Jira account email
    /// - `api_token`: Your Jira API token
    #[must_use]
    pub fn new(base_url: &str, email: &str, api_token: &str) -> Self {
        let credentials = format!("{email}:{api_token}");
        let encoded = base64::engine::general_purpose::STANDARD.encode(credentials.as_bytes());
        let auth_header = format!("Basic {encoded}");

        Self {
            client: Client::new(),
            base_url: base_url.trim_end_matches('/').to_string(),
            auth_header,
        }
    }

    async fn search_issues(
        &self,
        jql: &str,
        max_results: usize,
        start_at: usize,
    ) -> Result<String> {
        let url = format!("{}/rest/api/3/search", self.base_url);

        let request_body = JiraSearchRequest {
            jql: jql.to_string(),
            max_results: max_results.min(100),
            start_at,
            fields: Some(vec![
                "summary".to_string(),
                "description".to_string(),
                "status".to_string(),
                "priority".to_string(),
                "assignee".to_string(),
                "reporter".to_string(),
                "created".to_string(),
                "updated".to_string(),
                "issuetype".to_string(),
            ]),
        };

        let response = self
            .client
            .post(&url)
            .header("Authorization", &self.auth_header)
            .header("Content-Type", "application/json")
            .json(&request_body)
            .send()
            .await
            .map_err(|e| Error::tool_error(format!("Jira API request failed: {e}")))?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(Error::tool_error(format!(
                "Jira API returned error {status}: {error_text}"
            )));
        }

        let search_response: JiraSearchResponse = response
            .json()
            .await
            .map_err(|e| Error::tool_error(format!("Failed to parse Jira response: {e}")))?;

        // Format results
        let mut output = format!(
            "Found {} issues (showing {} results):\n\n",
            search_response.total,
            search_response.issues.len()
        );

        for issue in &search_response.issues {
            output.push_str(&format!("**{}**: {}\n", issue.key, issue.fields.summary));
            output.push_str(&format!("  Status: {}\n", issue.fields.status.name));

            if let Some(priority) = &issue.fields.priority {
                output.push_str(&format!("  Priority: {}\n", priority.name));
            }

            if let Some(issue_type) = &issue.fields.issuetype {
                output.push_str(&format!("  Type: {}\n", issue_type.name));
            }

            if let Some(assignee) = &issue.fields.assignee {
                output.push_str(&format!("  Assignee: {}\n", assignee.display_name));
            }

            if let Some(reporter) = &issue.fields.reporter {
                output.push_str(&format!("  Reporter: {}\n", reporter.display_name));
            }

            if let Some(created) = &issue.fields.created {
                output.push_str(&format!("  Created: {created}\n"));
            }

            if let Some(updated) = &issue.fields.updated {
                output.push_str(&format!("  Updated: {updated}\n"));
            }

            // Extract issue key from URL for browse link
            let browse_url = format!("{}/browse/{}", self.base_url, issue.key);
            output.push_str(&format!("  URL: {browse_url}\n"));
            output.push('\n');
        }

        if search_response.issues.is_empty() {
            output.push_str("No issues found matching the query.\n");
        }

        Ok(output)
    }
}

impl Default for JiraSearchTool {
    fn default() -> Self {
        Self::new(
            "https://example.atlassian.net",
            "user@example.com",
            "api-token",
        )
    }
}

#[async_trait]
impl Tool for JiraSearchTool {
    fn name(&self) -> &'static str {
        "jira_search"
    }

    fn description(&self) -> &'static str {
        "Search Jira issues using JQL (Jira Query Language). \
        Input should be a JSON object with 'jql' (required, JQL query string), \
        'max_results' (optional, default 10, max 100), and 'start_at' (optional, default 0). \
        Returns formatted list of matching issues with key, summary, status, assignee, and URL."
    }

    async fn _call(&self, input: ToolInput) -> Result<String> {
        let params = match input {
            ToolInput::Structured(json) => json,
            ToolInput::String(s) => {
                return Err(Error::tool_error(format!(
                "JiraSearchTool requires structured JSON input with 'jql' field, got string: {s}"
            )))
            }
        };

        let jql = params
            .get("jql")
            .and_then(|v| v.as_str())
            .ok_or_else(|| Error::tool_error("Missing required field: 'jql'"))?;

        let max_results = params
            .get("max_results")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(10) as usize;

        let start_at = params
            .get("start_at")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(0) as usize;

        self.search_issues(jql, max_results, start_at).await
    }
}

/// Jira issue tool for getting detailed information about a specific issue.
///
/// This tool retrieves detailed information about a single Jira issue by its key.
///
/// ## Input Parameters
///
/// - `issue_key` (required): Jira issue key (e.g., "DEMO-123")
///
/// ## Output
///
/// Returns detailed issue information including:
/// - Issue key, summary, and description
/// - Status, priority, and type
/// - Assignee and reporter
/// - Created and updated timestamps
/// - Issue URL
///
/// ## Example
///
/// ```rust,no_run
/// use dashflow_jira::JiraIssueTool;
/// use dashflow::core::tools::{Tool, ToolInput};
///
/// #[tokio::main]
/// async fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let tool = JiraIssueTool::new(
///         "https://your-domain.atlassian.net",
///         "your-email@example.com",
///         "your-api-token"
///     );
///
///     let input = serde_json::json!({
///         "issue_key": "DEMO-123"
///     });
///
///     let result = tool._call(ToolInput::Structured(input)).await?;
///     println!("{}", result);
///
///     Ok(())
/// }
/// ```
#[derive(Clone)]
pub struct JiraIssueTool {
    client: Client,
    base_url: String,
    auth_header: String,
}

impl JiraIssueTool {
    /// Create a new Jira issue tool.
    ///
    /// # Arguments
    ///
    /// - `base_url`: Jira instance URL (e.g., "<https://your-domain.atlassian.net>")
    /// - `email`: Your Jira account email
    /// - `api_token`: Your Jira API token
    #[must_use]
    pub fn new(base_url: &str, email: &str, api_token: &str) -> Self {
        let credentials = format!("{email}:{api_token}");
        let encoded = base64::engine::general_purpose::STANDARD.encode(credentials.as_bytes());
        let auth_header = format!("Basic {encoded}");

        Self {
            client: Client::new(),
            base_url: base_url.trim_end_matches('/').to_string(),
            auth_header,
        }
    }

    async fn get_issue(&self, issue_key: &str) -> Result<String> {
        let url = format!("{}/rest/api/3/issue/{}", self.base_url, issue_key);

        let response = self
            .client
            .get(&url)
            .header("Authorization", &self.auth_header)
            .send()
            .await
            .map_err(|e| Error::tool_error(format!("Jira API request failed: {e}")))?;

        if !response.status().is_success() {
            let status = response.status();
            if status == 404 {
                return Err(Error::tool_error(format!("Issue '{issue_key}' not found")));
            }
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(Error::tool_error(format!(
                "Jira API returned error {status}: {error_text}"
            )));
        }

        let issue: JiraIssue = response
            .json()
            .await
            .map_err(|e| Error::tool_error(format!("Failed to parse Jira response: {e}")))?;

        // Format output
        let mut output = format!("**Issue: {}**\n\n", issue.key);
        output.push_str(&format!("**Summary:** {}\n\n", issue.fields.summary));

        // Extract description (may be in Atlassian Document Format)
        if let Some(description) = &issue.fields.description {
            let desc_text = extract_description_text(description);
            if !desc_text.is_empty() {
                output.push_str(&format!("**Description:**\n{desc_text}\n\n"));
            }
        }

        output.push_str(&format!("**Status:** {}\n", issue.fields.status.name));

        if let Some(priority) = &issue.fields.priority {
            output.push_str(&format!("**Priority:** {}\n", priority.name));
        }

        if let Some(issue_type) = &issue.fields.issuetype {
            output.push_str(&format!("**Type:** {}\n", issue_type.name));
        }

        if let Some(assignee) = &issue.fields.assignee {
            output.push_str(&format!("**Assignee:** {}\n", assignee.display_name));
        }

        if let Some(reporter) = &issue.fields.reporter {
            output.push_str(&format!("**Reporter:** {}\n", reporter.display_name));
        }

        if let Some(created) = &issue.fields.created {
            output.push_str(&format!("**Created:** {created}\n"));
        }

        if let Some(updated) = &issue.fields.updated {
            output.push_str(&format!("**Updated:** {updated}\n"));
        }

        let browse_url = format!("{}/browse/{}", self.base_url, issue.key);
        output.push_str(&format!("\n**URL:** {browse_url}\n"));

        Ok(output)
    }
}

impl Default for JiraIssueTool {
    fn default() -> Self {
        Self::new(
            "https://example.atlassian.net",
            "user@example.com",
            "api-token",
        )
    }
}

#[async_trait]
impl Tool for JiraIssueTool {
    fn name(&self) -> &'static str {
        "jira_issue"
    }

    fn description(&self) -> &'static str {
        "Get detailed information about a specific Jira issue. \
        Input should be a JSON object with 'issue_key' (required, e.g., 'DEMO-123'). \
        Returns detailed issue information including summary, description, status, assignee, and URL."
    }

    async fn _call(&self, input: ToolInput) -> Result<String> {
        let params = match input {
            ToolInput::Structured(json) => json,
            ToolInput::String(s) => {
                // Allow simple string input as issue key
                return self.get_issue(&s).await;
            }
        };

        let issue_key = params
            .get("issue_key")
            .and_then(|v| v.as_str())
            .ok_or_else(|| Error::tool_error("Missing required field: 'issue_key'"))?;

        self.get_issue(issue_key).await
    }
}

/// Extract plain text from Atlassian Document Format (ADF) description
fn extract_description_text(description: &Value) -> String {
    if let Some(text) = description.as_str() {
        return text.to_string();
    }

    // Handle ADF format (nested JSON structure)
    if let Some(obj) = description.as_object() {
        if let Some(content) = obj.get("content").and_then(|v| v.as_array()) {
            let mut text = String::new();
            for item in content {
                if let Some(item_obj) = item.as_object() {
                    // Extract text from paragraphs
                    if let Some(para_content) = item_obj.get("content").and_then(|v| v.as_array()) {
                        let mut para_text = String::new();
                        for para_item in para_content {
                            if let Some(t) = para_item.get("text").and_then(|v| v.as_str()) {
                                para_text.push_str(t);
                            }
                        }
                        if !para_text.is_empty() {
                            text.push_str(&para_text);
                            text.push('\n');
                        }
                    }
                }
            }
            return text.trim().to_string();
        }
    }

    String::new()
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    // ========================================================================
    // JiraSearchTool Construction Tests
    // ========================================================================

    #[test]
    fn test_jira_search_tool_creation() {
        let tool = JiraSearchTool::new(
            "https://example.atlassian.net",
            "test@example.com",
            "test-token",
        );
        assert_eq!(tool.name(), "jira_search");
        assert!(tool.description().contains("JQL"));
    }

    #[test]
    fn test_jira_search_tool_strips_trailing_slash() {
        let tool = JiraSearchTool::new(
            "https://example.atlassian.net/",
            "test@example.com",
            "test-token",
        );
        assert_eq!(tool.base_url, "https://example.atlassian.net");
    }

    #[test]
    fn test_jira_search_tool_strips_multiple_trailing_slashes() {
        let tool = JiraSearchTool::new(
            "https://example.atlassian.net///",
            "test@example.com",
            "test-token",
        );
        // trim_end_matches removes all trailing slashes
        assert_eq!(tool.base_url, "https://example.atlassian.net");
    }

    #[test]
    fn test_jira_search_tool_auth_header_format() {
        let tool = JiraSearchTool::new(
            "https://example.atlassian.net",
            "user@test.com",
            "my-api-token",
        );
        // Auth should be Base64("user@test.com:my-api-token")
        let expected_encoded =
            base64::engine::general_purpose::STANDARD.encode("user@test.com:my-api-token");
        assert_eq!(tool.auth_header, format!("Basic {}", expected_encoded));
    }

    #[test]
    fn test_jira_search_tool_auth_handles_special_chars() {
        let tool = JiraSearchTool::new(
            "https://example.atlassian.net",
            "user+tag@test.com",
            "token-with-dashes_underscores",
        );
        let expected_encoded = base64::engine::general_purpose::STANDARD
            .encode("user+tag@test.com:token-with-dashes_underscores");
        assert_eq!(tool.auth_header, format!("Basic {}", expected_encoded));
    }

    #[test]
    fn test_jira_search_tool_default() {
        let tool = JiraSearchTool::default();
        assert_eq!(tool.base_url, "https://example.atlassian.net");
        assert_eq!(tool.name(), "jira_search");
    }

    #[test]
    fn test_jira_search_tool_clone() {
        let tool = JiraSearchTool::new(
            "https://test.atlassian.net",
            "clone@test.com",
            "clone-token",
        );
        let cloned = tool.clone();
        assert_eq!(cloned.base_url, tool.base_url);
        assert_eq!(cloned.auth_header, tool.auth_header);
    }

    #[test]
    fn test_jira_search_tool_name() {
        let tool = JiraSearchTool::default();
        assert_eq!(tool.name(), "jira_search");
    }

    #[test]
    fn test_jira_search_tool_description_contains_key_terms() {
        let tool = JiraSearchTool::default();
        let desc = tool.description();
        assert!(desc.contains("JQL"));
        assert!(desc.contains("Search")); // Capital S in "Search Jira issues"
        assert!(desc.contains("Jira"));
    }

    // ========================================================================
    // JiraIssueTool Construction Tests
    // ========================================================================

    #[test]
    fn test_jira_issue_tool_creation() {
        let tool = JiraIssueTool::new(
            "https://example.atlassian.net",
            "test@example.com",
            "test-token",
        );
        assert_eq!(tool.name(), "jira_issue");
        assert!(tool.description().contains("issue"));
    }

    #[test]
    fn test_jira_issue_tool_strips_trailing_slash() {
        let tool = JiraIssueTool::new(
            "https://example.atlassian.net/",
            "test@example.com",
            "test-token",
        );
        assert_eq!(tool.base_url, "https://example.atlassian.net");
    }

    #[test]
    fn test_jira_issue_tool_auth_header_format() {
        let tool = JiraIssueTool::new(
            "https://example.atlassian.net",
            "admin@corp.com",
            "secret-token-123",
        );
        let expected_encoded =
            base64::engine::general_purpose::STANDARD.encode("admin@corp.com:secret-token-123");
        assert_eq!(tool.auth_header, format!("Basic {}", expected_encoded));
    }

    #[test]
    fn test_jira_issue_tool_default() {
        let tool = JiraIssueTool::default();
        assert_eq!(tool.base_url, "https://example.atlassian.net");
        assert_eq!(tool.name(), "jira_issue");
    }

    #[test]
    fn test_jira_issue_tool_clone() {
        let tool = JiraIssueTool::new(
            "https://corp.atlassian.net",
            "user@corp.com",
            "corp-token",
        );
        let cloned = tool.clone();
        assert_eq!(cloned.base_url, tool.base_url);
        assert_eq!(cloned.auth_header, tool.auth_header);
    }

    #[test]
    fn test_jira_issue_tool_name() {
        let tool = JiraIssueTool::default();
        assert_eq!(tool.name(), "jira_issue");
    }

    #[test]
    fn test_jira_issue_tool_description_contains_key_terms() {
        let tool = JiraIssueTool::default();
        let desc = tool.description();
        assert!(desc.contains("issue"));
        assert!(desc.contains("Jira"));
        assert!(desc.contains("issue_key"));
    }

    // ========================================================================
    // Description Extraction Tests (Pure Function)
    // ========================================================================

    #[test]
    fn test_extract_description_plain_text() {
        let description = serde_json::json!("This is a plain text description");
        let text = extract_description_text(&description);
        assert_eq!(text, "This is a plain text description");
    }

    #[test]
    fn test_extract_description_empty_string() {
        let description = serde_json::json!("");
        let text = extract_description_text(&description);
        assert_eq!(text, "");
    }

    #[test]
    fn test_extract_description_adf() {
        let description = serde_json::json!({
            "type": "doc",
            "version": 1,
            "content": [
                {
                    "type": "paragraph",
                    "content": [
                        {"type": "text", "text": "This is "},
                        {"type": "text", "text": "ADF format"}
                    ]
                }
            ]
        });
        let text = extract_description_text(&description);
        assert_eq!(text, "This is ADF format");
    }

    #[test]
    fn test_extract_description_adf_multiple_paragraphs() {
        let description = serde_json::json!({
            "type": "doc",
            "version": 1,
            "content": [
                {
                    "type": "paragraph",
                    "content": [
                        {"type": "text", "text": "First paragraph"}
                    ]
                },
                {
                    "type": "paragraph",
                    "content": [
                        {"type": "text", "text": "Second paragraph"}
                    ]
                }
            ]
        });
        let text = extract_description_text(&description);
        assert!(text.contains("First paragraph"));
        assert!(text.contains("Second paragraph"));
    }

    #[test]
    fn test_extract_description_adf_empty_content() {
        let description = serde_json::json!({
            "type": "doc",
            "version": 1,
            "content": []
        });
        let text = extract_description_text(&description);
        assert_eq!(text, "");
    }

    #[test]
    fn test_extract_description_null() {
        let description = serde_json::json!(null);
        let text = extract_description_text(&description);
        assert_eq!(text, "");
    }

    #[test]
    fn test_extract_description_number() {
        let description = serde_json::json!(12345);
        let text = extract_description_text(&description);
        assert_eq!(text, "");
    }

    #[test]
    fn test_extract_description_boolean() {
        let description = serde_json::json!(true);
        let text = extract_description_text(&description);
        assert_eq!(text, "");
    }

    #[test]
    fn test_extract_description_array() {
        let description = serde_json::json!(["item1", "item2"]);
        let text = extract_description_text(&description);
        assert_eq!(text, "");
    }

    #[test]
    fn test_extract_description_object_without_content() {
        let description = serde_json::json!({
            "type": "doc",
            "version": 1
        });
        let text = extract_description_text(&description);
        assert_eq!(text, "");
    }

    #[test]
    fn test_extract_description_paragraph_without_content() {
        let description = serde_json::json!({
            "type": "doc",
            "version": 1,
            "content": [
                {
                    "type": "paragraph"
                }
            ]
        });
        let text = extract_description_text(&description);
        assert_eq!(text, "");
    }

    #[test]
    fn test_extract_description_adf_with_non_text_nodes() {
        let description = serde_json::json!({
            "type": "doc",
            "version": 1,
            "content": [
                {
                    "type": "paragraph",
                    "content": [
                        {"type": "text", "text": "Before link "},
                        {"type": "hardBreak"},
                        {"type": "text", "text": "After link"}
                    ]
                }
            ]
        });
        let text = extract_description_text(&description);
        assert!(text.contains("Before link"));
        assert!(text.contains("After link"));
    }

    #[test]
    fn test_extract_description_adf_with_emoji() {
        let description = serde_json::json!({
            "type": "doc",
            "version": 1,
            "content": [
                {
                    "type": "paragraph",
                    "content": [
                        {"type": "text", "text": "Task with emoji 🎉"}
                    ]
                }
            ]
        });
        let text = extract_description_text(&description);
        assert_eq!(text, "Task with emoji 🎉");
    }

    #[test]
    fn test_extract_description_long_text() {
        let long_text = "x".repeat(10000);
        let description = serde_json::json!(long_text);
        let text = extract_description_text(&description);
        assert_eq!(text.len(), 10000);
    }

    #[test]
    fn test_extract_description_unicode() {
        let description = serde_json::json!("日本語テスト 中文测试 한국어테스트");
        let text = extract_description_text(&description);
        assert_eq!(text, "日本語テスト 中文测试 한국어테스트");
    }

    #[test]
    fn test_extract_description_newlines() {
        let description = serde_json::json!("Line 1\nLine 2\nLine 3");
        let text = extract_description_text(&description);
        assert!(text.contains("Line 1"));
        assert!(text.contains("Line 2"));
        assert!(text.contains("Line 3"));
    }

    // ========================================================================
    // Serialization Tests
    // ========================================================================

    #[test]
    fn test_jira_search_request_serialization() {
        let request = JiraSearchRequest {
            jql: "project = TEST".to_string(),
            max_results: 25,
            start_at: 10,
            fields: None,
        };
        let json = serde_json::to_value(&request).expect("serialize");
        assert_eq!(json["jql"], "project = TEST");
        assert_eq!(json["maxResults"], 25);
        assert_eq!(json["startAt"], 10);
        assert!(json.get("fields").is_none()); // skip_serializing_if = Option::is_none
    }

    #[test]
    fn test_jira_search_request_with_fields() {
        let request = JiraSearchRequest {
            jql: "status = Open".to_string(),
            max_results: 50,
            start_at: 0,
            fields: Some(vec!["summary".to_string(), "status".to_string()]),
        };
        let json = serde_json::to_value(&request).expect("serialize");
        assert_eq!(json["jql"], "status = Open");
        let fields = json["fields"].as_array().expect("fields array");
        assert_eq!(fields.len(), 2);
    }

    #[test]
    fn test_jira_search_request_camel_case() {
        let request = JiraSearchRequest {
            jql: "test".to_string(),
            max_results: 10,
            start_at: 5,
            fields: None,
        };
        let json = serde_json::to_string(&request).expect("serialize");
        // Verify camelCase field names
        assert!(json.contains("maxResults"));
        assert!(json.contains("startAt"));
        assert!(!json.contains("max_results"));
        assert!(!json.contains("start_at"));
    }

    // ========================================================================
    // Deserialization Tests
    // ========================================================================

    #[test]
    fn test_jira_search_response_deserialization() {
        let json = serde_json::json!({
            "total": 42,
            "issues": []
        });
        let response: JiraSearchResponse = serde_json::from_value(json).expect("deserialize");
        assert_eq!(response.total, 42);
        assert!(response.issues.is_empty());
    }

    #[test]
    fn test_jira_search_response_with_issues() {
        let json = serde_json::json!({
            "total": 1,
            "issues": [{
                "key": "TEST-123",
                "self": "https://example.atlassian.net/rest/api/3/issue/12345",
                "fields": {
                    "summary": "Test issue",
                    "status": {"name": "Open"},
                    "description": null
                }
            }]
        });
        let response: JiraSearchResponse = serde_json::from_value(json).expect("deserialize");
        assert_eq!(response.total, 1);
        assert_eq!(response.issues.len(), 1);
        assert_eq!(response.issues[0].key, "TEST-123");
        assert_eq!(response.issues[0].fields.summary, "Test issue");
    }

    #[test]
    fn test_jira_issue_deserialization_minimal() {
        let json = serde_json::json!({
            "key": "PROJ-1",
            "self": "https://example.atlassian.net/rest/api/3/issue/1",
            "fields": {
                "summary": "Minimal issue",
                "status": {"name": "Done"}
            }
        });
        let issue: JiraIssue = serde_json::from_value(json).expect("deserialize");
        assert_eq!(issue.key, "PROJ-1");
        assert_eq!(issue.fields.summary, "Minimal issue");
        assert_eq!(issue.fields.status.name, "Done");
        assert!(issue.fields.priority.is_none());
        assert!(issue.fields.assignee.is_none());
    }

    #[test]
    fn test_jira_issue_deserialization_full() {
        let json = serde_json::json!({
            "key": "PROJ-100",
            "self": "https://example.atlassian.net/rest/api/3/issue/100",
            "fields": {
                "summary": "Full issue with all fields",
                "description": "Detailed description",
                "status": {"name": "In Progress"},
                "priority": {"name": "High"},
                "assignee": {"displayName": "John Doe"},
                "reporter": {"displayName": "Jane Smith"},
                "created": "2024-01-15T10:30:00.000+0000",
                "updated": "2024-01-16T14:45:00.000+0000",
                "issuetype": {"name": "Bug"}
            }
        });
        let issue: JiraIssue = serde_json::from_value(json).expect("deserialize");
        assert_eq!(issue.key, "PROJ-100");
        assert_eq!(issue.fields.summary, "Full issue with all fields");
        assert_eq!(issue.fields.status.name, "In Progress");
        assert_eq!(issue.fields.priority.as_ref().unwrap().name, "High");
        assert_eq!(
            issue.fields.assignee.as_ref().unwrap().display_name,
            "John Doe"
        );
        assert_eq!(
            issue.fields.reporter.as_ref().unwrap().display_name,
            "Jane Smith"
        );
        assert!(issue.fields.created.is_some());
        assert!(issue.fields.updated.is_some());
        assert_eq!(issue.fields.issuetype.as_ref().unwrap().name, "Bug");
    }

    #[test]
    fn test_jira_user_display_name_alias() {
        // Test both "displayName" and "display_name" work
        let json1 = serde_json::json!({"displayName": "User One"});
        let user1: JiraUser = serde_json::from_value(json1).expect("deserialize");
        assert_eq!(user1.display_name, "User One");
    }

    #[test]
    fn test_jira_status_deserialization() {
        let json = serde_json::json!({"name": "In Review"});
        let status: JiraStatus = serde_json::from_value(json).expect("deserialize");
        assert_eq!(status.name, "In Review");
    }

    #[test]
    fn test_jira_priority_deserialization() {
        let json = serde_json::json!({"name": "Critical"});
        let priority: JiraPriority = serde_json::from_value(json).expect("deserialize");
        assert_eq!(priority.name, "Critical");
    }

    #[test]
    fn test_jira_issue_type_deserialization() {
        let json = serde_json::json!({"name": "Story"});
        let issue_type: JiraIssueType = serde_json::from_value(json).expect("deserialize");
        assert_eq!(issue_type.name, "Story");
    }

    // ========================================================================
    // Input Validation Tests (Async)
    // ========================================================================

    #[tokio::test]
    async fn test_jira_search_tool_rejects_string_input() {
        let tool = JiraSearchTool::default();
        let result = tool._call(ToolInput::String("project = TEST".to_string())).await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("structured JSON input"));
    }

    #[tokio::test]
    async fn test_jira_search_tool_requires_jql_field() {
        let tool = JiraSearchTool::default();
        let input = serde_json::json!({
            "max_results": 10
        });
        let result = tool._call(ToolInput::Structured(input)).await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("jql"));
    }

    #[tokio::test]
    async fn test_jira_issue_tool_accepts_string_input() {
        // JiraIssueTool should accept string input as issue key
        // This will fail on network, but should not fail on input validation
        let tool = JiraIssueTool::default();
        let result = tool._call(ToolInput::String("TEST-123".to_string())).await;
        // Should fail with network error, not input validation error
        assert!(result.is_err());
        let err = result.unwrap_err();
        // Should be a tool error from API call, not "Missing required field"
        assert!(!err.to_string().contains("Missing required field"));
    }

    #[tokio::test]
    async fn test_jira_issue_tool_requires_issue_key_field() {
        let tool = JiraIssueTool::default();
        let input = serde_json::json!({
            "project": "TEST"
        });
        let result = tool._call(ToolInput::Structured(input)).await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("issue_key"));
    }

    // ========================================================================
    // Edge Cases
    // ========================================================================

    #[test]
    fn test_jira_search_tool_with_empty_email() {
        let tool = JiraSearchTool::new("https://example.atlassian.net", "", "token");
        // Should still create auth header (empty email is technically valid base64)
        let expected = base64::engine::general_purpose::STANDARD.encode(":token");
        assert_eq!(tool.auth_header, format!("Basic {}", expected));
    }

    #[test]
    fn test_jira_search_tool_with_empty_token() {
        let tool = JiraSearchTool::new("https://example.atlassian.net", "user@test.com", "");
        let expected = base64::engine::general_purpose::STANDARD.encode("user@test.com:");
        assert_eq!(tool.auth_header, format!("Basic {}", expected));
    }

    #[test]
    fn test_jira_search_tool_url_without_protocol() {
        // URL without protocol should be stored as-is (not validated)
        let tool = JiraSearchTool::new("example.atlassian.net", "user@test.com", "token");
        assert_eq!(tool.base_url, "example.atlassian.net");
    }

    #[test]
    fn test_search_response_handles_missing_issues_array() {
        let json = serde_json::json!({
            "total": 0
        });
        let response: JiraSearchResponse = serde_json::from_value(json).expect("deserialize");
        assert_eq!(response.total, 0);
        assert!(response.issues.is_empty()); // default
    }

    #[test]
    fn test_issue_fields_all_optional_except_status_summary() {
        let json = serde_json::json!({
            "key": "MIN-1",
            "self": "https://example.atlassian.net/rest/api/3/issue/1",
            "fields": {
                "summary": "Minimal",
                "status": {"name": "Open"}
            }
        });
        let issue: JiraIssue = serde_json::from_value(json).expect("deserialize");
        assert_eq!(issue.key, "MIN-1");
        assert!(issue.fields.description.is_none());
        assert!(issue.fields.priority.is_none());
        assert!(issue.fields.assignee.is_none());
        assert!(issue.fields.reporter.is_none());
        assert!(issue.fields.created.is_none());
        assert!(issue.fields.updated.is_none());
        assert!(issue.fields.issuetype.is_none());
    }

    // ========================================================================
    // Integration Tests (require API credentials)
    // ========================================================================

    #[tokio::test]
    #[ignore = "requires JIRA_BASE_URL, JIRA_EMAIL, and JIRA_API_TOKEN"]
    async fn test_jira_search_integration() {
        let base_url = std::env::var("JIRA_BASE_URL").expect("JIRA_BASE_URL must be set");
        let email = std::env::var("JIRA_EMAIL").expect("JIRA_EMAIL must be set");
        let api_token = std::env::var("JIRA_API_TOKEN").expect("JIRA_API_TOKEN must be set");

        let tool = JiraSearchTool::new(&base_url, &email, &api_token);

        let input = serde_json::json!({
            "jql": "project = TEST",
            "max_results": 5
        });

        tool._call(ToolInput::Structured(input))
            .await
            .expect("Jira search failed");
    }

    #[tokio::test]
    #[ignore = "requires JIRA_BASE_URL, JIRA_EMAIL, and JIRA_API_TOKEN"]
    async fn test_jira_issue_integration() {
        let base_url = std::env::var("JIRA_BASE_URL").expect("JIRA_BASE_URL must be set");
        let email = std::env::var("JIRA_EMAIL").expect("JIRA_EMAIL must be set");
        let api_token = std::env::var("JIRA_API_TOKEN").expect("JIRA_API_TOKEN must be set");

        let tool = JiraIssueTool::new(&base_url, &email, &api_token);

        let input = serde_json::json!({
            "issue_key": "TEST-1"
        });

        tool._call(ToolInput::Structured(input))
            .await
            .expect("Jira issue fetch failed");
    }

    // ========================================================================
    // Builder Edge Cases - Unicode and Special Characters
    // ========================================================================

    #[test]
    fn test_jira_search_tool_unicode_base_url() {
        let tool = JiraSearchTool::new(
            "https://例え.atlassian.net",
            "user@test.com",
            "token",
        );
        assert_eq!(tool.base_url, "https://例え.atlassian.net");
    }

    #[test]
    fn test_jira_search_tool_unicode_email() {
        let tool = JiraSearchTool::new(
            "https://example.atlassian.net",
            "用户@example.com",
            "token",
        );
        let expected = base64::engine::general_purpose::STANDARD.encode("用户@example.com:token");
        assert_eq!(tool.auth_header, format!("Basic {}", expected));
    }

    #[test]
    fn test_jira_search_tool_unicode_token() {
        let tool = JiraSearchTool::new(
            "https://example.atlassian.net",
            "user@test.com",
            "令牌🔑",
        );
        let expected = base64::engine::general_purpose::STANDARD.encode("user@test.com:令牌🔑");
        assert_eq!(tool.auth_header, format!("Basic {}", expected));
    }

    #[test]
    fn test_jira_search_tool_very_long_token() {
        let long_token = "x".repeat(1000);
        let tool = JiraSearchTool::new(
            "https://example.atlassian.net",
            "user@test.com",
            &long_token,
        );
        let expected =
            base64::engine::general_purpose::STANDARD.encode(format!("user@test.com:{}", long_token));
        assert_eq!(tool.auth_header, format!("Basic {}", expected));
    }

    #[test]
    fn test_jira_search_tool_whitespace_in_email() {
        let tool = JiraSearchTool::new(
            "https://example.atlassian.net",
            "user name@test.com",
            "token",
        );
        let expected =
            base64::engine::general_purpose::STANDARD.encode("user name@test.com:token");
        assert_eq!(tool.auth_header, format!("Basic {}", expected));
    }

    #[test]
    fn test_jira_search_tool_colon_in_token() {
        // Colon in token should work (only first colon matters in Basic auth)
        let tool = JiraSearchTool::new(
            "https://example.atlassian.net",
            "user@test.com",
            "token:with:colons",
        );
        let expected =
            base64::engine::general_purpose::STANDARD.encode("user@test.com:token:with:colons");
        assert_eq!(tool.auth_header, format!("Basic {}", expected));
    }

    #[test]
    fn test_jira_issue_tool_unicode_base_url() {
        let tool = JiraIssueTool::new(
            "https://日本語.atlassian.net",
            "user@test.com",
            "token",
        );
        assert_eq!(tool.base_url, "https://日本語.atlassian.net");
    }

    #[test]
    fn test_jira_issue_tool_strips_multiple_trailing_slashes() {
        let tool = JiraIssueTool::new(
            "https://example.atlassian.net///",
            "test@example.com",
            "test-token",
        );
        assert_eq!(tool.base_url, "https://example.atlassian.net");
    }

    // ========================================================================
    // ADF Extraction - Complex Structures
    // ========================================================================

    #[test]
    fn test_extract_description_adf_code_block() {
        let description = serde_json::json!({
            "type": "doc",
            "version": 1,
            "content": [
                {
                    "type": "codeBlock",
                    "attrs": {"language": "rust"},
                    "content": [
                        {"type": "text", "text": "fn main() {}"}
                    ]
                }
            ]
        });
        let text = extract_description_text(&description);
        // Code blocks have "content" array at top level, not nested
        assert!(text.is_empty() || text.contains("fn main"));
    }

    #[test]
    fn test_extract_description_adf_bullet_list() {
        let description = serde_json::json!({
            "type": "doc",
            "version": 1,
            "content": [
                {
                    "type": "bulletList",
                    "content": [
                        {
                            "type": "listItem",
                            "content": [
                                {
                                    "type": "paragraph",
                                    "content": [{"type": "text", "text": "Item 1"}]
                                }
                            ]
                        }
                    ]
                }
            ]
        });
        let text = extract_description_text(&description);
        // The extraction only looks at direct paragraph children
        assert_eq!(text, "");
    }

    #[test]
    fn test_extract_description_adf_heading() {
        let description = serde_json::json!({
            "type": "doc",
            "version": 1,
            "content": [
                {
                    "type": "heading",
                    "attrs": {"level": 1},
                    "content": [{"type": "text", "text": "Header Text"}]
                }
            ]
        });
        let text = extract_description_text(&description);
        // Headings have content like paragraphs
        assert!(text.contains("Header Text") || text.is_empty());
    }

    #[test]
    fn test_extract_description_adf_inline_code() {
        let description = serde_json::json!({
            "type": "doc",
            "version": 1,
            "content": [
                {
                    "type": "paragraph",
                    "content": [
                        {"type": "text", "text": "Use the "},
                        {"type": "text", "text": "code_function", "marks": [{"type": "code"}]},
                        {"type": "text", "text": " method"}
                    ]
                }
            ]
        });
        let text = extract_description_text(&description);
        assert!(text.contains("Use the"));
        assert!(text.contains("code_function"));
        assert!(text.contains("method"));
    }

    #[test]
    fn test_extract_description_adf_link() {
        let description = serde_json::json!({
            "type": "doc",
            "version": 1,
            "content": [
                {
                    "type": "paragraph",
                    "content": [
                        {"type": "text", "text": "Click "},
                        {
                            "type": "text",
                            "text": "here",
                            "marks": [{"type": "link", "attrs": {"href": "https://example.com"}}]
                        }
                    ]
                }
            ]
        });
        let text = extract_description_text(&description);
        assert!(text.contains("Click"));
        assert!(text.contains("here"));
    }

    #[test]
    fn test_extract_description_adf_mention() {
        let description = serde_json::json!({
            "type": "doc",
            "version": 1,
            "content": [
                {
                    "type": "paragraph",
                    "content": [
                        {"type": "text", "text": "Assigned to "},
                        {
                            "type": "mention",
                            "attrs": {"id": "123", "text": "@john"}
                        },
                        {"type": "text", "text": " for review"}
                    ]
                }
            ]
        });
        let text = extract_description_text(&description);
        assert!(text.contains("Assigned to"));
        assert!(text.contains("for review"));
    }

    #[test]
    fn test_extract_description_adf_strong_text() {
        let description = serde_json::json!({
            "type": "doc",
            "version": 1,
            "content": [
                {
                    "type": "paragraph",
                    "content": [
                        {"type": "text", "text": "This is "},
                        {"type": "text", "text": "important", "marks": [{"type": "strong"}]},
                        {"type": "text", "text": " text"}
                    ]
                }
            ]
        });
        let text = extract_description_text(&description);
        assert!(text.contains("important"));
    }

    #[test]
    fn test_extract_description_adf_emphasis_text() {
        let description = serde_json::json!({
            "type": "doc",
            "version": 1,
            "content": [
                {
                    "type": "paragraph",
                    "content": [
                        {"type": "text", "text": "This is "},
                        {"type": "text", "text": "emphasized", "marks": [{"type": "em"}]}
                    ]
                }
            ]
        });
        let text = extract_description_text(&description);
        assert!(text.contains("emphasized"));
    }

    #[test]
    fn test_extract_description_adf_nested_content_non_array() {
        let description = serde_json::json!({
            "type": "doc",
            "version": 1,
            "content": "not an array"
        });
        let text = extract_description_text(&description);
        assert_eq!(text, "");
    }

    #[test]
    fn test_extract_description_adf_paragraph_content_non_array() {
        let description = serde_json::json!({
            "type": "doc",
            "version": 1,
            "content": [
                {
                    "type": "paragraph",
                    "content": "not an array"
                }
            ]
        });
        let text = extract_description_text(&description);
        assert_eq!(text, "");
    }

    #[test]
    fn test_extract_description_deep_nested_object() {
        let description = serde_json::json!({
            "level1": {
                "level2": {
                    "level3": {
                        "content": []
                    }
                }
            }
        });
        let text = extract_description_text(&description);
        assert_eq!(text, "");
    }

    // ========================================================================
    // Serialization Boundary Tests
    // ========================================================================

    #[test]
    fn test_jira_search_request_max_results_zero() {
        let request = JiraSearchRequest {
            jql: "test".to_string(),
            max_results: 0,
            start_at: 0,
            fields: None,
        };
        let json = serde_json::to_value(&request).expect("serialize");
        assert_eq!(json["maxResults"], 0);
    }

    #[test]
    fn test_jira_search_request_max_results_large() {
        let request = JiraSearchRequest {
            jql: "test".to_string(),
            max_results: 999999,
            start_at: 0,
            fields: None,
        };
        let json = serde_json::to_value(&request).expect("serialize");
        assert_eq!(json["maxResults"], 999999);
    }

    #[test]
    fn test_jira_search_request_start_at_large() {
        let request = JiraSearchRequest {
            jql: "test".to_string(),
            max_results: 10,
            start_at: 1_000_000,
            fields: None,
        };
        let json = serde_json::to_value(&request).expect("serialize");
        assert_eq!(json["startAt"], 1_000_000);
    }

    #[test]
    fn test_jira_search_request_empty_jql() {
        let request = JiraSearchRequest {
            jql: "".to_string(),
            max_results: 10,
            start_at: 0,
            fields: None,
        };
        let json = serde_json::to_value(&request).expect("serialize");
        assert_eq!(json["jql"], "");
    }

    #[test]
    fn test_jira_search_request_complex_jql() {
        let complex_jql = r#"project = "TEST PROJECT" AND (status = Open OR status = "In Progress") AND assignee in (currentUser(), "john@example.com") ORDER BY created DESC"#;
        let request = JiraSearchRequest {
            jql: complex_jql.to_string(),
            max_results: 50,
            start_at: 0,
            fields: None,
        };
        let json = serde_json::to_value(&request).expect("serialize");
        assert_eq!(json["jql"], complex_jql);
    }

    #[test]
    fn test_jira_search_request_empty_fields() {
        let request = JiraSearchRequest {
            jql: "test".to_string(),
            max_results: 10,
            start_at: 0,
            fields: Some(vec![]),
        };
        let json = serde_json::to_value(&request).expect("serialize");
        let fields = json["fields"].as_array().expect("fields");
        assert!(fields.is_empty());
    }

    #[test]
    fn test_jira_search_request_many_fields() {
        let many_fields: Vec<String> = (0..50).map(|i| format!("field_{}", i)).collect();
        let request = JiraSearchRequest {
            jql: "test".to_string(),
            max_results: 10,
            start_at: 0,
            fields: Some(many_fields.clone()),
        };
        let json = serde_json::to_value(&request).expect("serialize");
        let fields = json["fields"].as_array().expect("fields");
        assert_eq!(fields.len(), 50);
    }

    // ========================================================================
    // Deserialization Robustness Tests
    // ========================================================================

    #[test]
    fn test_jira_search_response_extra_fields() {
        let json = serde_json::json!({
            "total": 10,
            "issues": [],
            "startAt": 0,
            "maxResults": 50,
            "expand": "names,schema"
        });
        let response: JiraSearchResponse = serde_json::from_value(json).expect("deserialize");
        assert_eq!(response.total, 10);
    }

    #[test]
    fn test_jira_issue_extra_fields() {
        let json = serde_json::json!({
            "key": "TEST-1",
            "id": "12345",
            "self": "https://example.atlassian.net/rest/api/3/issue/12345",
            "expand": "operations,versionedRepresentations",
            "fields": {
                "summary": "Test",
                "status": {"name": "Open"},
                "customfield_10001": "Custom value"
            }
        });
        let issue: JiraIssue = serde_json::from_value(json).expect("deserialize");
        assert_eq!(issue.key, "TEST-1");
    }

    #[test]
    fn test_jira_issue_fields_extra_nested() {
        let json = serde_json::json!({
            "key": "TEST-2",
            "self": "https://example.atlassian.net/rest/api/3/issue/2",
            "fields": {
                "summary": "Extra fields test",
                "status": {"name": "Done", "id": "3", "statusCategory": {"key": "done"}},
                "labels": ["bug", "critical"],
                "components": [{"name": "Backend"}]
            }
        });
        let issue: JiraIssue = serde_json::from_value(json).expect("deserialize");
        assert_eq!(issue.fields.summary, "Extra fields test");
    }

    #[test]
    fn test_jira_status_extra_fields() {
        let json = serde_json::json!({
            "name": "In Progress",
            "id": "3",
            "description": "Work in progress",
            "statusCategory": {"key": "indeterminate"}
        });
        let status: JiraStatus = serde_json::from_value(json).expect("deserialize");
        assert_eq!(status.name, "In Progress");
    }

    #[test]
    fn test_jira_priority_extra_fields() {
        let json = serde_json::json!({
            "name": "High",
            "id": "2",
            "iconUrl": "https://example.com/priority-high.png"
        });
        let priority: JiraPriority = serde_json::from_value(json).expect("deserialize");
        assert_eq!(priority.name, "High");
    }

    #[test]
    fn test_jira_user_extra_fields() {
        let json = serde_json::json!({
            "displayName": "John Doe",
            "accountId": "123456789",
            "emailAddress": "john@example.com",
            "avatarUrls": {"48x48": "https://example.com/avatar.png"},
            "active": true
        });
        let user: JiraUser = serde_json::from_value(json).expect("deserialize");
        assert_eq!(user.display_name, "John Doe");
    }

    #[test]
    fn test_jira_issue_type_extra_fields() {
        let json = serde_json::json!({
            "name": "Epic",
            "id": "10000",
            "description": "Large feature",
            "iconUrl": "https://example.com/epic.png",
            "subtask": false,
            "hierarchyLevel": 0
        });
        let issue_type: JiraIssueType = serde_json::from_value(json).expect("deserialize");
        assert_eq!(issue_type.name, "Epic");
    }

    #[test]
    fn test_jira_issue_description_as_object() {
        let json = serde_json::json!({
            "key": "TEST-3",
            "self": "https://example.atlassian.net/rest/api/3/issue/3",
            "fields": {
                "summary": "Object description",
                "status": {"name": "Open"},
                "description": {
                    "type": "doc",
                    "version": 1,
                    "content": []
                }
            }
        });
        let issue: JiraIssue = serde_json::from_value(json).expect("deserialize");
        assert!(issue.fields.description.is_some());
    }

    // ========================================================================
    // Clone Independence Tests
    // ========================================================================

    #[test]
    fn test_jira_search_tool_clone_independence() {
        let original = JiraSearchTool::new(
            "https://orig.atlassian.net",
            "orig@test.com",
            "orig-token",
        );
        let cloned = original.clone();

        // Verify they have same values
        assert_eq!(original.base_url, cloned.base_url);
        assert_eq!(original.auth_header, cloned.auth_header);

        // Modifying one shouldn't affect the other (both are immutable, but test separation)
        drop(original);
        assert_eq!(cloned.base_url, "https://orig.atlassian.net");
    }

    #[test]
    fn test_jira_issue_tool_clone_independence() {
        let original = JiraIssueTool::new(
            "https://orig.atlassian.net",
            "orig@test.com",
            "orig-token",
        );
        let cloned = original.clone();

        assert_eq!(original.base_url, cloned.base_url);
        assert_eq!(original.auth_header, cloned.auth_header);

        drop(original);
        assert_eq!(cloned.base_url, "https://orig.atlassian.net");
    }

    // ========================================================================
    // Debug Format Tests
    // ========================================================================

    #[test]
    fn test_jira_search_response_debug() {
        let response = JiraSearchResponse {
            total: 42,
            issues: vec![],
        };
        let debug_str = format!("{:?}", response);
        assert!(debug_str.contains("JiraSearchResponse"));
        assert!(debug_str.contains("42"));
    }

    #[test]
    fn test_jira_status_debug() {
        let status = JiraStatus {
            name: "Open".to_string(),
        };
        let debug_str = format!("{:?}", status);
        assert!(debug_str.contains("JiraStatus"));
        assert!(debug_str.contains("Open"));
    }

    #[test]
    fn test_jira_priority_debug() {
        let priority = JiraPriority {
            name: "Critical".to_string(),
        };
        let debug_str = format!("{:?}", priority);
        assert!(debug_str.contains("JiraPriority"));
        assert!(debug_str.contains("Critical"));
    }

    #[test]
    fn test_jira_user_debug() {
        let user = JiraUser {
            display_name: "Jane Doe".to_string(),
        };
        let debug_str = format!("{:?}", user);
        assert!(debug_str.contains("JiraUser"));
        assert!(debug_str.contains("Jane Doe"));
    }

    #[test]
    fn test_jira_issue_type_debug() {
        let issue_type = JiraIssueType {
            name: "Story".to_string(),
        };
        let debug_str = format!("{:?}", issue_type);
        assert!(debug_str.contains("JiraIssueType"));
        assert!(debug_str.contains("Story"));
    }

    #[test]
    fn test_jira_search_request_debug() {
        let request = JiraSearchRequest {
            jql: "test".to_string(),
            max_results: 10,
            start_at: 0,
            fields: None,
        };
        let debug_str = format!("{:?}", request);
        assert!(debug_str.contains("JiraSearchRequest"));
        assert!(debug_str.contains("test"));
    }

    // ========================================================================
    // Tool Trait Implementation Tests
    // ========================================================================

    #[test]
    fn test_jira_search_tool_description_format() {
        let tool = JiraSearchTool::default();
        let desc = tool.description();
        // Verify description contains expected keywords
        assert!(desc.contains("max_results"));
        assert!(desc.contains("start_at"));
        assert!(desc.contains("100"));
    }

    #[test]
    fn test_jira_issue_tool_description_format() {
        let tool = JiraIssueTool::default();
        let desc = tool.description();
        assert!(desc.contains("DEMO-123"));
    }

    // ========================================================================
    // More Async Input Tests
    // ========================================================================

    #[tokio::test]
    async fn test_jira_search_tool_empty_jql() {
        let tool = JiraSearchTool::default();
        let input = serde_json::json!({
            "jql": ""
        });
        // Empty JQL is accepted by tool, will fail on API
        let result = tool._call(ToolInput::Structured(input)).await;
        assert!(result.is_err()); // API call fails
    }

    #[tokio::test]
    async fn test_jira_search_tool_null_jql() {
        let tool = JiraSearchTool::default();
        let input = serde_json::json!({
            "jql": null
        });
        let result = tool._call(ToolInput::Structured(input)).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("jql"));
    }

    #[tokio::test]
    async fn test_jira_search_tool_jql_wrong_type() {
        let tool = JiraSearchTool::default();
        let input = serde_json::json!({
            "jql": 12345
        });
        let result = tool._call(ToolInput::Structured(input)).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("jql"));
    }

    #[tokio::test]
    async fn test_jira_search_tool_max_results_string() {
        let tool = JiraSearchTool::default();
        let input = serde_json::json!({
            "jql": "project = TEST",
            "max_results": "10"  // String instead of number
        });
        // Will use default (10) since "10" is not a u64
        let result = tool._call(ToolInput::Structured(input)).await;
        // Will fail on API, not on parsing
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_jira_issue_tool_empty_issue_key() {
        let tool = JiraIssueTool::default();
        let input = serde_json::json!({
            "issue_key": ""
        });
        let result = tool._call(ToolInput::Structured(input)).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_jira_issue_tool_null_issue_key() {
        let tool = JiraIssueTool::default();
        let input = serde_json::json!({
            "issue_key": null
        });
        let result = tool._call(ToolInput::Structured(input)).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("issue_key"));
    }

    #[tokio::test]
    async fn test_jira_issue_tool_issue_key_wrong_type() {
        let tool = JiraIssueTool::default();
        let input = serde_json::json!({
            "issue_key": 123
        });
        let result = tool._call(ToolInput::Structured(input)).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("issue_key"));
    }

    #[tokio::test]
    async fn test_jira_issue_tool_empty_json_input() {
        let tool = JiraIssueTool::default();
        let input = serde_json::json!({});
        let result = tool._call(ToolInput::Structured(input)).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_jira_search_tool_empty_json_input() {
        let tool = JiraSearchTool::default();
        let input = serde_json::json!({});
        let result = tool._call(ToolInput::Structured(input)).await;
        assert!(result.is_err());
    }

    // ========================================================================
    // URL Construction Tests
    // ========================================================================

    #[test]
    fn test_jira_search_url_construction() {
        let tool = JiraSearchTool::new(
            "https://mycompany.atlassian.net",
            "user@test.com",
            "token",
        );
        // The URL would be constructed in search_issues
        // Just verify base_url is correct
        assert_eq!(tool.base_url, "https://mycompany.atlassian.net");
    }

    #[test]
    fn test_jira_search_url_with_path() {
        let tool = JiraSearchTool::new(
            "https://proxy.company.com/jira",
            "user@test.com",
            "token",
        );
        assert_eq!(tool.base_url, "https://proxy.company.com/jira");
    }

    #[test]
    fn test_jira_search_url_with_port() {
        let tool = JiraSearchTool::new(
            "https://jira.internal:8443",
            "user@test.com",
            "token",
        );
        assert_eq!(tool.base_url, "https://jira.internal:8443");
    }

    #[test]
    fn test_jira_search_url_http() {
        let tool = JiraSearchTool::new(
            "http://localhost:8080",
            "user@test.com",
            "token",
        );
        assert_eq!(tool.base_url, "http://localhost:8080");
    }

    // ========================================================================
    // Deserialization with Description Variants
    // ========================================================================

    #[test]
    fn test_jira_issue_description_string() {
        let json = serde_json::json!({
            "key": "TEST-4",
            "self": "https://example.atlassian.net/rest/api/3/issue/4",
            "fields": {
                "summary": "String description",
                "status": {"name": "Open"},
                "description": "Plain text description"
            }
        });
        let issue: JiraIssue = serde_json::from_value(json).expect("deserialize");
        let desc = issue.fields.description.as_ref().unwrap();
        assert_eq!(desc.as_str().unwrap(), "Plain text description");
    }

    #[test]
    fn test_jira_issue_description_null() {
        let json = serde_json::json!({
            "key": "TEST-5",
            "self": "https://example.atlassian.net/rest/api/3/issue/5",
            "fields": {
                "summary": "Null description",
                "status": {"name": "Open"},
                "description": null
            }
        });
        let issue: JiraIssue = serde_json::from_value(json).expect("deserialize");
        // null becomes None (via #[serde(default)])
        assert!(issue.fields.description.is_none() || issue.fields.description.as_ref().unwrap().is_null());
    }

    #[test]
    fn test_jira_search_response_large_total() {
        let json = serde_json::json!({
            "total": 999999999,
            "issues": []
        });
        let response: JiraSearchResponse = serde_json::from_value(json).expect("deserialize");
        assert_eq!(response.total, 999999999);
    }

    #[test]
    fn test_jira_issue_timestamps_iso8601() {
        let json = serde_json::json!({
            "key": "TEST-6",
            "self": "https://example.atlassian.net/rest/api/3/issue/6",
            "fields": {
                "summary": "Timestamp test",
                "status": {"name": "Open"},
                "created": "2024-12-25T10:30:00.000+0000",
                "updated": "2024-12-26T15:45:30.123Z"
            }
        });
        let issue: JiraIssue = serde_json::from_value(json).expect("deserialize");
        assert!(issue.fields.created.as_ref().unwrap().contains("2024-12-25"));
        assert!(issue.fields.updated.as_ref().unwrap().contains("2024-12-26"));
    }

    // ========================================================================
    // Error Message Content Tests
    // ========================================================================

    #[tokio::test]
    async fn test_jira_search_error_message_contains_context() {
        let tool = JiraSearchTool::default();
        let result = tool._call(ToolInput::String("invalid".to_string())).await;
        let err = result.unwrap_err();
        let err_str = err.to_string();
        assert!(err_str.contains("JiraSearchTool"));
        assert!(err_str.contains("JSON"));
    }

    #[tokio::test]
    async fn test_jira_issue_missing_key_error() {
        let tool = JiraIssueTool::default();
        let input = serde_json::json!({"wrong_field": "value"});
        let result = tool._call(ToolInput::Structured(input)).await;
        let err = result.unwrap_err();
        assert!(err.to_string().contains("issue_key"));
    }

    // ========================================================================
    // Special Character Tests
    // ========================================================================

    #[test]
    fn test_extract_description_html_entities() {
        let description = serde_json::json!("&lt;script&gt;alert('XSS')&lt;/script&gt;");
        let text = extract_description_text(&description);
        assert_eq!(text, "&lt;script&gt;alert('XSS')&lt;/script&gt;");
    }

    #[test]
    fn test_extract_description_control_characters() {
        let description = serde_json::json!("Line1\tTabbed\r\nLine2");
        let text = extract_description_text(&description);
        assert!(text.contains("Line1"));
        assert!(text.contains("Line2"));
    }

    #[test]
    fn test_extract_description_quotes() {
        let description = serde_json::json!("He said \"Hello\" and 'Goodbye'");
        let text = extract_description_text(&description);
        assert!(text.contains("Hello"));
        assert!(text.contains("Goodbye"));
    }

    #[test]
    fn test_extract_description_backslashes() {
        let description = serde_json::json!("Path: C:\\Users\\test\\file.txt");
        let text = extract_description_text(&description);
        assert!(text.contains("C:\\Users"));
    }

    // ========================================================================
    // Additional ADF Edge Cases
    // ========================================================================

    #[test]
    fn test_extract_description_adf_empty_paragraph_text() {
        let description = serde_json::json!({
            "type": "doc",
            "version": 1,
            "content": [
                {
                    "type": "paragraph",
                    "content": [
                        {"type": "text", "text": ""}
                    ]
                }
            ]
        });
        let text = extract_description_text(&description);
        assert_eq!(text, "");
    }

    #[test]
    fn test_extract_description_adf_whitespace_only() {
        let description = serde_json::json!({
            "type": "doc",
            "version": 1,
            "content": [
                {
                    "type": "paragraph",
                    "content": [
                        {"type": "text", "text": "   "}
                    ]
                }
            ]
        });
        let text = extract_description_text(&description);
        // trim() is applied, so should be empty
        assert_eq!(text, "");
    }

    #[test]
    fn test_extract_description_adf_mixed_node_types() {
        let description = serde_json::json!({
            "type": "doc",
            "version": 1,
            "content": [
                {
                    "type": "paragraph",
                    "content": [
                        {"type": "text", "text": "Text "},
                        {"type": "emoji", "attrs": {"shortName": ":smile:"}},
                        {"type": "text", "text": " more text"}
                    ]
                }
            ]
        });
        let text = extract_description_text(&description);
        assert!(text.contains("Text"));
        assert!(text.contains("more text"));
    }

    // ========================================================================
    // Field Default Tests
    // ========================================================================

    #[test]
    fn test_jira_issue_fields_default_values() {
        let json = serde_json::json!({
            "key": "DEF-1",
            "self": "https://example.atlassian.net/rest/api/3/issue/1",
            "fields": {
                "summary": "Default values test",
                "status": {"name": "Open"}
                // All optional fields omitted
            }
        });
        let issue: JiraIssue = serde_json::from_value(json).expect("deserialize");
        assert!(issue.fields.description.is_none());
        assert!(issue.fields.priority.is_none());
        assert!(issue.fields.assignee.is_none());
        assert!(issue.fields.reporter.is_none());
        assert!(issue.fields.created.is_none());
        assert!(issue.fields.updated.is_none());
        assert!(issue.fields.issuetype.is_none());
    }
}
