//! GitLab integration tools for `DashFlow` Rust.
//!
//! This crate provides a comprehensive set of tools for interacting with GitLab repositories,
//! enabling AI agents to manage issues, merge requests, and projects.
//!
//! # Tools
//!
//! ## Issue Management
//! - **`GetIssueTool`**: Get issue details by ID
//! - **`CreateIssueTool`**: Create new issues
//! - **`UpdateIssueTool`**: Update existing issues
//! - **`ListIssuesTool`**: List issues in a project
//!
//! ## Merge Request Management
//! - **`GetMergeRequestTool`**: Get merge request details by ID
//! - **`CreateMergeRequestTool`**: Create new merge requests
//! - **`ListMergeRequestsTool`**: List merge requests in a project
//!
//! ## Project Management
//! - **`ListProjectsTool`**: List accessible projects
//! - **`GetProjectTool`**: Get project details
//!
//! # Authentication
//!
//! All tools require a GitLab personal access token with appropriate permissions.
//! Set the token when creating the GitLab client instance.
//!
//! # Example
//!
//! ```no_run
//! use dashflow_gitlab::GetIssueTool;
//! use dashflow::core::tools::{Tool, ToolInput};
//! use serde_json::json;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! // Create tool (requires GitLab token in production)
//! let tool = GetIssueTool::new("https://gitlab.com", "project_id", "token")?;
//!
//! // Get issue #42
//! let input = json!({"issue_iid": 42});
//! let result = tool._call(ToolInput::Structured(input)).await?;
//! # Ok(())
//! # }
//! ```

use async_trait::async_trait;
use dashflow::core::tools::{Tool, ToolInput};
use dashflow::core::Error;
use gitlab::api::{projects, Query};
use gitlab::{Gitlab, GitlabBuilder};

// ============================================================================
// Helper Functions
// ============================================================================

/// Extract string field from `ToolInput`
fn extract_string_field(input: &ToolInput, field: &str) -> Result<String, Error> {
    match input {
        ToolInput::String(s) => Ok(s.clone()),
        ToolInput::Structured(v) => v
            .get(field)
            .and_then(|v| v.as_str())
            .map(std::string::ToString::to_string)
            .ok_or_else(|| Error::tool_error(format!("Missing '{field}' field in input"))),
    }
}

/// Extract optional string field from `ToolInput`
fn extract_optional_string(input: &ToolInput, field: &str) -> Option<String> {
    match input {
        ToolInput::Structured(v) => v
            .get(field)
            .and_then(|v| v.as_str())
            .map(std::string::ToString::to_string),
        _ => None,
    }
}

/// Extract u64 field from `ToolInput`
fn extract_u64_field(input: &ToolInput, field: &str) -> Result<u64, Error> {
    match input {
        ToolInput::Structured(v) => v
            .get(field)
            .and_then(serde_json::Value::as_u64)
            .ok_or_else(|| Error::tool_error(format!("Missing or invalid '{field}' field"))),
        _ => Err(Error::tool_error(format!(
            "Expected structured input with '{field}' field"
        ))),
    }
}

// ============================================================================
// GetIssueTool
// ============================================================================

/// Tool for getting GitLab issue details.
///
/// Retrieves information about a specific issue by IID (internal ID within project).
///
/// # Input Format
///
/// - **Structured**: `{"issue_iid": 42}`
///
/// # Example
///
/// ```rust,ignore
/// use dashflow_gitlab::GetIssueTool;
/// use dashflow::core::tools::Tool;
///
/// let tool = GetIssueTool::new("https://gitlab.com", "group/project", "token").unwrap();
/// assert_eq!(tool.name(), "get_issue");
/// ```
#[derive(Clone)]
pub struct GetIssueTool {
    project: String,
    client: Gitlab,
}

impl GetIssueTool {
    pub fn new(
        gitlab_url: impl Into<String>,
        project: impl Into<String>,
        token: impl Into<String>,
    ) -> Result<Self, Error> {
        let client = GitlabBuilder::new(gitlab_url.into().trim_end_matches('/'), token.into())
            .build()
            .map_err(|e| Error::tool_error(format!("Failed to build GitLab client: {e}")))?;

        Ok(Self {
            project: project.into(),
            client,
        })
    }
}

#[async_trait]
impl Tool for GetIssueTool {
    fn name(&self) -> &'static str {
        "get_issue"
    }

    fn description(&self) -> &'static str {
        "Get details of a GitLab issue by IID. Input: {\"issue_iid\": <number>}"
    }

    async fn _call(&self, input: ToolInput) -> Result<String, Error> {
        let issue_iid = extract_u64_field(&input, "issue_iid")?;

        let endpoint = projects::issues::Issue::builder()
            .project(self.project.clone())
            .issue(issue_iid)
            .build()
            .map_err(|e| Error::tool_error(format!("Failed to build endpoint: {e}")))?;

        let issue: serde_json::Value = endpoint
            .query(&self.client)
            .map_err(|e| Error::tool_error(format!("Failed to get issue: {e}")))?;

        Ok(serde_json::to_string_pretty(&issue).unwrap_or_else(|_| issue.to_string()))
    }
}

// ============================================================================
// CreateIssueTool
// ============================================================================

/// Tool for creating GitLab issues.
///
/// Creates a new issue in the specified project.
///
/// # Input Format
///
/// - **Structured**: `{"title": "Bug report", "description": "Description here"}`
///
/// # Example
///
/// ```rust,ignore
/// use dashflow_gitlab::CreateIssueTool;
/// use dashflow::core::tools::Tool;
///
/// # let tool = CreateIssueTool::new("https://gitlab.com", "group/project", "token").unwrap();
/// assert_eq!(tool.name(), "create_issue");
/// ```
#[derive(Clone)]
pub struct CreateIssueTool {
    project: String,
    client: Gitlab,
}

impl CreateIssueTool {
    pub fn new(
        gitlab_url: impl Into<String>,
        project: impl Into<String>,
        token: impl Into<String>,
    ) -> Result<Self, Error> {
        let client = GitlabBuilder::new(gitlab_url.into().trim_end_matches('/'), token.into())
            .build()
            .map_err(|e| Error::tool_error(format!("Failed to build GitLab client: {e}")))?;

        Ok(Self {
            project: project.into(),
            client,
        })
    }
}

#[async_trait]
impl Tool for CreateIssueTool {
    fn name(&self) -> &'static str {
        "create_issue"
    }

    fn description(&self) -> &'static str {
        "Create a new GitLab issue. Input: {\"title\": \"title\", \"description\": \"description\"}"
    }

    async fn _call(&self, input: ToolInput) -> Result<String, Error> {
        let title = extract_string_field(&input, "title")?;
        let description = extract_optional_string(&input, "description");

        let endpoint = if let Some(desc) = description {
            projects::issues::CreateIssue::builder()
                .project(self.project.clone())
                .title(title)
                .description(desc)
                .build()
                .map_err(|e| Error::tool_error(format!("Failed to build endpoint: {e}")))?
        } else {
            projects::issues::CreateIssue::builder()
                .project(self.project.clone())
                .title(title)
                .build()
                .map_err(|e| Error::tool_error(format!("Failed to build endpoint: {e}")))?
        };

        let issue: serde_json::Value = endpoint
            .query(&self.client)
            .map_err(|e| Error::tool_error(format!("Failed to create issue: {e}")))?;

        Ok(format!(
            "Issue created successfully:\n{}",
            serde_json::to_string_pretty(&issue).unwrap_or_else(|_| issue.to_string())
        ))
    }
}

// ============================================================================
// UpdateIssueTool
// ============================================================================

/// Tool for updating GitLab issues.
///
/// Updates an existing issue in the specified project.
///
/// # Input Format
///
/// - **Structured**: `{"issue_iid": 42, "title": "Updated title", "description": "Updated description", "state_event": "close"}`
///
/// # Example
///
/// ```rust,ignore
/// use dashflow_gitlab::UpdateIssueTool;
/// use dashflow::core::tools::Tool;
///
/// # let tool = UpdateIssueTool::new("https://gitlab.com", "group/project", "token").unwrap();
/// assert_eq!(tool.name(), "update_issue");
/// ```
#[derive(Clone)]
pub struct UpdateIssueTool {
    project: String,
    client: Gitlab,
}

impl UpdateIssueTool {
    pub fn new(
        gitlab_url: impl Into<String>,
        project: impl Into<String>,
        token: impl Into<String>,
    ) -> Result<Self, Error> {
        let client = GitlabBuilder::new(gitlab_url.into().trim_end_matches('/'), token.into())
            .build()
            .map_err(|e| Error::tool_error(format!("Failed to build GitLab client: {e}")))?;

        Ok(Self {
            project: project.into(),
            client,
        })
    }
}

#[async_trait]
impl Tool for UpdateIssueTool {
    fn name(&self) -> &'static str {
        "update_issue"
    }

    fn description(&self) -> &'static str {
        "Update a GitLab issue. Input: {\"issue_iid\": <number>, \"title\": \"new title\", \"description\": \"new desc\", \"state_event\": \"close\" or \"reopen\"}"
    }

    async fn _call(&self, input: ToolInput) -> Result<String, Error> {
        let issue_iid = extract_u64_field(&input, "issue_iid")?;
        let title = extract_optional_string(&input, "title");
        let description = extract_optional_string(&input, "description");
        let state_event = extract_optional_string(&input, "state_event");

        let mut builder = projects::issues::EditIssue::builder();
        builder.project(self.project.clone()).issue(issue_iid);

        if let Some(t) = title {
            builder.title(t);
        }

        if let Some(d) = description {
            builder.description(d);
        }

        if let Some(state) = state_event {
            let state_enum = match state.as_str() {
                "close" => gitlab::api::projects::issues::IssueStateEvent::Close,
                "reopen" => gitlab::api::projects::issues::IssueStateEvent::Reopen,
                _ => {
                    return Err(Error::tool_error(format!(
                        "Invalid state_event: {state}. Must be 'close' or 'reopen'"
                    )))
                }
            };
            builder.state_event(state_enum);
        }

        let endpoint = builder
            .build()
            .map_err(|e| Error::tool_error(format!("Failed to build endpoint: {e}")))?;

        let issue: serde_json::Value = endpoint
            .query(&self.client)
            .map_err(|e| Error::tool_error(format!("Failed to update issue: {e}")))?;

        Ok(format!(
            "Issue updated successfully:\n{}",
            serde_json::to_string_pretty(&issue).unwrap_or_else(|_| issue.to_string())
        ))
    }
}

// ============================================================================
// ListIssuesTool
// ============================================================================

/// Tool for listing GitLab issues.
///
/// Lists issues in the specified project with optional filtering.
///
/// # Input Format
///
/// - **Structured**: `{"state": "opened", "labels": "bug,critical"}` (all fields optional)
///
/// # Example
///
/// ```rust,ignore
/// use dashflow_gitlab::ListIssuesTool;
/// use dashflow::core::tools::Tool;
///
/// # let tool = ListIssuesTool::new("https://gitlab.com", "group/project", "token").unwrap();
/// assert_eq!(tool.name(), "list_issues");
/// ```
#[derive(Clone)]
pub struct ListIssuesTool {
    project: String,
    client: Gitlab,
}

impl ListIssuesTool {
    pub fn new(
        gitlab_url: impl Into<String>,
        project: impl Into<String>,
        token: impl Into<String>,
    ) -> Result<Self, Error> {
        let client = GitlabBuilder::new(gitlab_url.into().trim_end_matches('/'), token.into())
            .build()
            .map_err(|e| Error::tool_error(format!("Failed to build GitLab client: {e}")))?;

        Ok(Self {
            project: project.into(),
            client,
        })
    }
}

#[async_trait]
impl Tool for ListIssuesTool {
    fn name(&self) -> &'static str {
        "list_issues"
    }

    fn description(&self) -> &'static str {
        "List GitLab issues. Input: {\"state\": \"opened\"|\"closed\"|\"all\", \"labels\": \"label1,label2\"} (all optional)"
    }

    async fn _call(&self, input: ToolInput) -> Result<String, Error> {
        let state = extract_optional_string(&input, "state");
        let labels = extract_optional_string(&input, "labels");

        let mut builder = projects::issues::Issues::builder();
        builder.project(self.project.clone());

        if let Some(s) = state {
            if s != "all" {
                let state_enum = match s.as_str() {
                    "opened" => gitlab::api::projects::issues::IssueState::Opened,
                    "closed" => gitlab::api::projects::issues::IssueState::Closed,
                    _ => {
                        return Err(Error::tool_error(format!(
                            "Invalid state: {s}. Must be 'opened', 'closed', or 'all'"
                        )))
                    }
                };
                builder.state(state_enum);
            }
        }

        if let Some(l) = labels {
            let label_vec: Vec<String> = l.split(',').map(|s| s.trim().to_string()).collect();
            builder.labels(label_vec);
        }

        let endpoint = builder
            .build()
            .map_err(|e| Error::tool_error(format!("Failed to build endpoint: {e}")))?;

        let issues: Vec<serde_json::Value> = endpoint
            .query(&self.client)
            .map_err(|e| Error::tool_error(format!("Failed to list issues: {e}")))?;

        Ok(format!(
            "Found {} issues:\n{}",
            issues.len(),
            serde_json::to_string_pretty(&issues).unwrap_or_else(|_| format!("{issues:?}"))
        ))
    }
}

// ============================================================================
// GetMergeRequestTool
// ============================================================================

/// Tool for getting GitLab merge request details.
///
/// Retrieves information about a specific merge request by IID.
///
/// # Input Format
///
/// - **Structured**: `{"merge_request_iid": 42}`
///
/// # Example
///
/// ```rust,ignore
/// use dashflow_gitlab::GetMergeRequestTool;
/// use dashflow::core::tools::Tool;
///
/// # let tool = GetMergeRequestTool::new("https://gitlab.com", "group/project", "token").unwrap();
/// assert_eq!(tool.name(), "get_merge_request");
/// ```
#[derive(Clone)]
pub struct GetMergeRequestTool {
    project: String,
    client: Gitlab,
}

impl GetMergeRequestTool {
    pub fn new(
        gitlab_url: impl Into<String>,
        project: impl Into<String>,
        token: impl Into<String>,
    ) -> Result<Self, Error> {
        let client = GitlabBuilder::new(gitlab_url.into().trim_end_matches('/'), token.into())
            .build()
            .map_err(|e| Error::tool_error(format!("Failed to build GitLab client: {e}")))?;

        Ok(Self {
            project: project.into(),
            client,
        })
    }
}

#[async_trait]
impl Tool for GetMergeRequestTool {
    fn name(&self) -> &'static str {
        "get_merge_request"
    }

    fn description(&self) -> &'static str {
        "Get details of a GitLab merge request by IID. Input: {\"merge_request_iid\": <number>}"
    }

    async fn _call(&self, input: ToolInput) -> Result<String, Error> {
        let mr_iid = extract_u64_field(&input, "merge_request_iid")?;

        let endpoint = projects::merge_requests::MergeRequest::builder()
            .project(self.project.clone())
            .merge_request(mr_iid)
            .build()
            .map_err(|e| Error::tool_error(format!("Failed to build endpoint: {e}")))?;

        let mr: serde_json::Value = endpoint
            .query(&self.client)
            .map_err(|e| Error::tool_error(format!("Failed to get merge request: {e}")))?;

        Ok(serde_json::to_string_pretty(&mr).unwrap_or_else(|_| mr.to_string()))
    }
}

// ============================================================================
// CreateMergeRequestTool
// ============================================================================

/// Tool for creating GitLab merge requests.
///
/// Creates a new merge request in the specified project.
///
/// # Input Format
///
/// - **Structured**: `{"title": "Fix bug", "source_branch": "feature", "target_branch": "main", "description": "Description"}`
///
/// # Example
///
/// ```rust,ignore
/// use dashflow_gitlab::CreateMergeRequestTool;
/// use dashflow::core::tools::Tool;
///
/// # let tool = CreateMergeRequestTool::new("https://gitlab.com", "group/project", "token").unwrap();
/// assert_eq!(tool.name(), "create_merge_request");
/// ```
#[derive(Clone)]
pub struct CreateMergeRequestTool {
    project: String,
    client: Gitlab,
}

impl CreateMergeRequestTool {
    pub fn new(
        gitlab_url: impl Into<String>,
        project: impl Into<String>,
        token: impl Into<String>,
    ) -> Result<Self, Error> {
        let client = GitlabBuilder::new(gitlab_url.into().trim_end_matches('/'), token.into())
            .build()
            .map_err(|e| Error::tool_error(format!("Failed to build GitLab client: {e}")))?;

        Ok(Self {
            project: project.into(),
            client,
        })
    }
}

#[async_trait]
impl Tool for CreateMergeRequestTool {
    fn name(&self) -> &'static str {
        "create_merge_request"
    }

    fn description(&self) -> &'static str {
        "Create a new GitLab merge request. Input: {\"title\": \"title\", \"source_branch\": \"feature\", \"target_branch\": \"main\", \"description\": \"description\"}"
    }

    async fn _call(&self, input: ToolInput) -> Result<String, Error> {
        let title = extract_string_field(&input, "title")?;
        let source_branch = extract_string_field(&input, "source_branch")?;
        let target_branch = extract_string_field(&input, "target_branch")?;
        let description = extract_optional_string(&input, "description");

        let mut builder = projects::merge_requests::CreateMergeRequest::builder();
        builder
            .project(self.project.clone())
            .title(title)
            .source_branch(source_branch)
            .target_branch(target_branch);

        if let Some(desc) = description {
            builder.description(desc);
        }

        let endpoint = builder
            .build()
            .map_err(|e| Error::tool_error(format!("Failed to build endpoint: {e}")))?;

        let mr: serde_json::Value = endpoint
            .query(&self.client)
            .map_err(|e| Error::tool_error(format!("Failed to create merge request: {e}")))?;

        Ok(format!(
            "Merge request created successfully:\n{}",
            serde_json::to_string_pretty(&mr).unwrap_or_else(|_| mr.to_string())
        ))
    }
}

// ============================================================================
// ListMergeRequestsTool
// ============================================================================

/// Tool for listing GitLab merge requests.
///
/// Lists merge requests in the specified project with optional filtering.
///
/// # Input Format
///
/// - **Structured**: `{"state": "opened"}` (optional)
///
/// # Example
///
/// ```rust,ignore
/// use dashflow_gitlab::ListMergeRequestsTool;
/// use dashflow::core::tools::Tool;
///
/// # let tool = ListMergeRequestsTool::new("https://gitlab.com", "group/project", "token").unwrap();
/// assert_eq!(tool.name(), "list_merge_requests");
/// ```
#[derive(Clone)]
pub struct ListMergeRequestsTool {
    project: String,
    client: Gitlab,
}

impl ListMergeRequestsTool {
    pub fn new(
        gitlab_url: impl Into<String>,
        project: impl Into<String>,
        token: impl Into<String>,
    ) -> Result<Self, Error> {
        let client = GitlabBuilder::new(gitlab_url.into().trim_end_matches('/'), token.into())
            .build()
            .map_err(|e| Error::tool_error(format!("Failed to build GitLab client: {e}")))?;

        Ok(Self {
            project: project.into(),
            client,
        })
    }
}

#[async_trait]
impl Tool for ListMergeRequestsTool {
    fn name(&self) -> &'static str {
        "list_merge_requests"
    }

    fn description(&self) -> &'static str {
        "List GitLab merge requests. Input: {\"state\": \"opened\"|\"closed\"|\"merged\"|\"all\"} (optional)"
    }

    async fn _call(&self, input: ToolInput) -> Result<String, Error> {
        let state = extract_optional_string(&input, "state");

        let mut builder = projects::merge_requests::MergeRequests::builder();
        builder.project(self.project.clone());

        if let Some(s) = state {
            if s != "all" {
                let state_enum = match s.as_str() {
                    "opened" => gitlab::api::projects::merge_requests::MergeRequestState::Opened,
                    "closed" => gitlab::api::projects::merge_requests::MergeRequestState::Closed,
                    "merged" => gitlab::api::projects::merge_requests::MergeRequestState::Merged,
                    _ => {
                        return Err(Error::tool_error(format!(
                            "Invalid state: {s}. Must be 'opened', 'closed', 'merged', or 'all'"
                        )))
                    }
                };
                builder.state(state_enum);
            }
        }

        let endpoint = builder
            .build()
            .map_err(|e| Error::tool_error(format!("Failed to build endpoint: {e}")))?;

        let mrs: Vec<serde_json::Value> = endpoint
            .query(&self.client)
            .map_err(|e| Error::tool_error(format!("Failed to list merge requests: {e}")))?;

        Ok(format!(
            "Found {} merge requests:\n{}",
            mrs.len(),
            serde_json::to_string_pretty(&mrs).unwrap_or_else(|_| format!("{mrs:?}"))
        ))
    }
}

// ============================================================================
// GetProjectTool
// ============================================================================

/// Tool for getting GitLab project details.
///
/// Retrieves information about a specific project.
///
/// # Input Format
///
/// - **Structured**: `{"project": "group/project"}` or just use the configured project
///
/// # Example
///
/// ```rust,ignore
/// use dashflow_gitlab::GetProjectTool;
/// use dashflow::core::tools::Tool;
///
/// # let tool = GetProjectTool::new("https://gitlab.com", "group/project", "token").unwrap();
/// assert_eq!(tool.name(), "get_project");
/// ```
#[derive(Clone)]
pub struct GetProjectTool {
    project: String,
    client: Gitlab,
}

impl GetProjectTool {
    pub fn new(
        gitlab_url: impl Into<String>,
        project: impl Into<String>,
        token: impl Into<String>,
    ) -> Result<Self, Error> {
        let client = GitlabBuilder::new(gitlab_url.into().trim_end_matches('/'), token.into())
            .build()
            .map_err(|e| Error::tool_error(format!("Failed to build GitLab client: {e}")))?;

        Ok(Self {
            project: project.into(),
            client,
        })
    }
}

#[async_trait]
impl Tool for GetProjectTool {
    fn name(&self) -> &'static str {
        "get_project"
    }

    fn description(&self) -> &'static str {
        "Get details of a GitLab project. Input: {} (uses configured project)"
    }

    async fn _call(&self, _input: ToolInput) -> Result<String, Error> {
        let endpoint = projects::Project::builder()
            .project(self.project.clone())
            .build()
            .map_err(|e| Error::tool_error(format!("Failed to build endpoint: {e}")))?;

        let project: serde_json::Value = endpoint
            .query(&self.client)
            .map_err(|e| Error::tool_error(format!("Failed to get project: {e}")))?;

        Ok(serde_json::to_string_pretty(&project).unwrap_or_else(|_| project.to_string()))
    }
}

#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // ============================================================================
    // Helper Function Tests (no external dependencies)
    // ============================================================================

    mod extract_string_field_tests {
        use super::*;

        #[test]
        fn test_extracts_from_structured_input() {
            let input = ToolInput::Structured(json!({"field": "value"}));
            let result = extract_string_field(&input, "field");
            assert!(result.is_ok());
            assert_eq!(result.unwrap(), "value");
        }

        #[test]
        fn test_extracts_from_string_input_ignores_field() {
            // When input is String, it returns the whole string regardless of field name
            let input = ToolInput::String("whole_string".to_string());
            let result = extract_string_field(&input, "any_field");
            assert!(result.is_ok());
            assert_eq!(result.unwrap(), "whole_string");
        }

        #[test]
        fn test_missing_field_returns_error() {
            let input = ToolInput::Structured(json!({"other": "value"}));
            let result = extract_string_field(&input, "field");
            assert!(result.is_err());
            let err_msg = result.unwrap_err().to_string();
            assert!(err_msg.contains("Missing 'field' field"));
        }

        #[test]
        fn test_null_field_returns_error() {
            let input = ToolInput::Structured(json!({"field": null}));
            let result = extract_string_field(&input, "field");
            assert!(result.is_err());
        }

        #[test]
        fn test_non_string_field_returns_error() {
            let input = ToolInput::Structured(json!({"field": 123}));
            let result = extract_string_field(&input, "field");
            assert!(result.is_err());
        }

        #[test]
        fn test_array_field_returns_error() {
            let input = ToolInput::Structured(json!({"field": ["a", "b"]}));
            let result = extract_string_field(&input, "field");
            assert!(result.is_err());
        }

        #[test]
        fn test_boolean_field_returns_error() {
            let input = ToolInput::Structured(json!({"field": true}));
            let result = extract_string_field(&input, "field");
            assert!(result.is_err());
        }

        #[test]
        fn test_empty_string_is_valid() {
            let input = ToolInput::Structured(json!({"field": ""}));
            let result = extract_string_field(&input, "field");
            assert!(result.is_ok());
            assert_eq!(result.unwrap(), "");
        }

        #[test]
        fn test_string_with_special_chars() {
            let input = ToolInput::Structured(json!({"field": "hello\nworld\ttab"}));
            let result = extract_string_field(&input, "field");
            assert!(result.is_ok());
            assert_eq!(result.unwrap(), "hello\nworld\ttab");
        }

        #[test]
        fn test_unicode_string() {
            let input = ToolInput::Structured(json!({"field": "こんにちは🚀"}));
            let result = extract_string_field(&input, "field");
            assert!(result.is_ok());
            assert_eq!(result.unwrap(), "こんにちは🚀");
        }

        #[test]
        fn test_nested_object_field_returns_error() {
            let input = ToolInput::Structured(json!({"field": {"nested": "value"}}));
            let result = extract_string_field(&input, "field");
            assert!(result.is_err());
        }

        #[test]
        fn test_empty_object() {
            let input = ToolInput::Structured(json!({}));
            let result = extract_string_field(&input, "field");
            assert!(result.is_err());
        }
    }

    mod extract_optional_string_tests {
        use super::*;

        #[test]
        fn test_extracts_present_field() {
            let input = ToolInput::Structured(json!({"field": "value"}));
            let result = extract_optional_string(&input, "field");
            assert_eq!(result, Some("value".to_string()));
        }

        #[test]
        fn test_missing_field_returns_none() {
            let input = ToolInput::Structured(json!({"other": "value"}));
            let result = extract_optional_string(&input, "field");
            assert_eq!(result, None);
        }

        #[test]
        fn test_null_field_returns_none() {
            let input = ToolInput::Structured(json!({"field": null}));
            let result = extract_optional_string(&input, "field");
            assert_eq!(result, None);
        }

        #[test]
        fn test_non_string_field_returns_none() {
            let input = ToolInput::Structured(json!({"field": 123}));
            let result = extract_optional_string(&input, "field");
            assert_eq!(result, None);
        }

        #[test]
        fn test_string_input_returns_none() {
            // String input always returns None for optional extraction
            let input = ToolInput::String("value".to_string());
            let result = extract_optional_string(&input, "field");
            assert_eq!(result, None);
        }

        #[test]
        fn test_empty_string_field_is_some() {
            let input = ToolInput::Structured(json!({"field": ""}));
            let result = extract_optional_string(&input, "field");
            assert_eq!(result, Some("".to_string()));
        }

        #[test]
        fn test_whitespace_only_string() {
            let input = ToolInput::Structured(json!({"field": "   "}));
            let result = extract_optional_string(&input, "field");
            assert_eq!(result, Some("   ".to_string()));
        }

        #[test]
        fn test_array_field_returns_none() {
            let input = ToolInput::Structured(json!({"field": ["a", "b"]}));
            let result = extract_optional_string(&input, "field");
            assert_eq!(result, None);
        }

        #[test]
        fn test_boolean_field_returns_none() {
            let input = ToolInput::Structured(json!({"field": false}));
            let result = extract_optional_string(&input, "field");
            assert_eq!(result, None);
        }

        #[test]
        fn test_empty_structured_input() {
            let input = ToolInput::Structured(json!({}));
            let result = extract_optional_string(&input, "field");
            assert_eq!(result, None);
        }

        #[test]
        fn test_unicode_field_value() {
            let input = ToolInput::Structured(json!({"field": "日本語テスト"}));
            let result = extract_optional_string(&input, "field");
            assert_eq!(result, Some("日本語テスト".to_string()));
        }

        #[test]
        fn test_multiple_fields_extracts_correct_one() {
            let input = ToolInput::Structured(json!({
                "field1": "value1",
                "field2": "value2",
                "field3": "value3"
            }));
            assert_eq!(
                extract_optional_string(&input, "field2"),
                Some("value2".to_string())
            );
        }
    }

    mod extract_u64_field_tests {
        use super::*;

        #[test]
        fn test_extracts_valid_u64() {
            let input = ToolInput::Structured(json!({"num": 42}));
            let result = extract_u64_field(&input, "num");
            assert!(result.is_ok());
            assert_eq!(result.unwrap(), 42);
        }

        #[test]
        fn test_extracts_zero() {
            let input = ToolInput::Structured(json!({"num": 0}));
            let result = extract_u64_field(&input, "num");
            assert!(result.is_ok());
            assert_eq!(result.unwrap(), 0);
        }

        #[test]
        fn test_extracts_large_number() {
            let input = ToolInput::Structured(json!({"num": 9_007_199_254_740_991_u64})); // Max safe JS integer
            let result = extract_u64_field(&input, "num");
            assert!(result.is_ok());
            assert_eq!(result.unwrap(), 9_007_199_254_740_991);
        }

        #[test]
        fn test_missing_field_returns_error() {
            let input = ToolInput::Structured(json!({"other": 42}));
            let result = extract_u64_field(&input, "num");
            assert!(result.is_err());
            let err_msg = result.unwrap_err().to_string();
            assert!(err_msg.contains("Missing or invalid 'num' field"));
        }

        #[test]
        fn test_null_field_returns_error() {
            let input = ToolInput::Structured(json!({"num": null}));
            let result = extract_u64_field(&input, "num");
            assert!(result.is_err());
        }

        #[test]
        fn test_string_input_returns_error() {
            let input = ToolInput::String("42".to_string());
            let result = extract_u64_field(&input, "num");
            assert!(result.is_err());
            let err_msg = result.unwrap_err().to_string();
            assert!(err_msg.contains("Expected structured input"));
        }

        #[test]
        fn test_string_number_returns_error() {
            let input = ToolInput::Structured(json!({"num": "42"}));
            let result = extract_u64_field(&input, "num");
            assert!(result.is_err());
        }

        #[test]
        fn test_float_number_returns_error() {
            // JSON floats don't convert to u64
            let input = ToolInput::Structured(json!({"num": 42.5}));
            let result = extract_u64_field(&input, "num");
            assert!(result.is_err());
        }

        #[test]
        fn test_negative_number_returns_error() {
            let input = ToolInput::Structured(json!({"num": -1}));
            let result = extract_u64_field(&input, "num");
            assert!(result.is_err());
        }

        #[test]
        fn test_boolean_field_returns_error() {
            let input = ToolInput::Structured(json!({"num": true}));
            let result = extract_u64_field(&input, "num");
            assert!(result.is_err());
        }

        #[test]
        fn test_array_field_returns_error() {
            let input = ToolInput::Structured(json!({"num": [1, 2, 3]}));
            let result = extract_u64_field(&input, "num");
            assert!(result.is_err());
        }

        #[test]
        fn test_object_field_returns_error() {
            let input = ToolInput::Structured(json!({"num": {"value": 42}}));
            let result = extract_u64_field(&input, "num");
            assert!(result.is_err());
        }

        #[test]
        fn test_empty_structured_input() {
            let input = ToolInput::Structured(json!({}));
            let result = extract_u64_field(&input, "num");
            assert!(result.is_err());
        }

        #[test]
        fn test_multiple_fields_extracts_correct_one() {
            let input = ToolInput::Structured(json!({
                "num1": 10,
                "num2": 20,
                "num3": 30
            }));
            let result = extract_u64_field(&input, "num2");
            assert!(result.is_ok());
            assert_eq!(result.unwrap(), 20);
        }

        #[test]
        fn test_whole_float_value() {
            // 42.0 may or may not be interpreted as u64 depending on JSON library
            let input = ToolInput::Structured(json!({"num": 42.0}));
            let result = extract_u64_field(&input, "num");
            // serde_json::Value::as_u64() returns None for floats
            assert!(result.is_err());
        }
    }

    mod tool_input_edge_cases {
        use super::*;

        #[test]
        fn test_deeply_nested_structure() {
            let input = ToolInput::Structured(json!({
                "level1": {
                    "level2": {
                        "field": "deep_value"
                    }
                }
            }));
            // Top-level field extraction only
            let result = extract_string_field(&input, "level1");
            assert!(result.is_err()); // Object, not string
        }

        #[test]
        fn test_array_of_objects() {
            let input = ToolInput::Structured(json!({
                "items": [
                    {"name": "item1"},
                    {"name": "item2"}
                ]
            }));
            let result = extract_string_field(&input, "items");
            assert!(result.is_err()); // Array, not string
        }

        #[test]
        fn test_mixed_types_in_object() {
            let input = ToolInput::Structured(json!({
                "string_field": "text",
                "number_field": 42,
                "bool_field": true,
                "null_field": null,
                "array_field": [1, 2, 3],
                "object_field": {"nested": "value"}
            }));

            assert_eq!(
                extract_string_field(&input, "string_field").unwrap(),
                "text"
            );
            assert!(extract_string_field(&input, "number_field").is_err());
            assert_eq!(extract_u64_field(&input, "number_field").unwrap(), 42);
            assert_eq!(
                extract_optional_string(&input, "string_field"),
                Some("text".to_string())
            );
            assert_eq!(extract_optional_string(&input, "null_field"), None);
        }

        #[test]
        fn test_special_field_names() {
            let input = ToolInput::Structured(json!({
                "": "empty_key",
                "with spaces": "spaced",
                "with-dashes": "dashed",
                "with_underscores": "underscored",
                "CamelCase": "cameled",
                "123numeric": "numeric_start"
            }));

            assert_eq!(extract_string_field(&input, "").unwrap(), "empty_key");
            assert_eq!(extract_string_field(&input, "with spaces").unwrap(), "spaced");
            assert_eq!(
                extract_string_field(&input, "with-dashes").unwrap(),
                "dashed"
            );
            assert_eq!(
                extract_string_field(&input, "with_underscores").unwrap(),
                "underscored"
            );
            assert_eq!(extract_string_field(&input, "CamelCase").unwrap(), "cameled");
            assert_eq!(
                extract_string_field(&input, "123numeric").unwrap(),
                "numeric_start"
            );
        }

        #[test]
        fn test_very_long_string_value() {
            let long_string = "a".repeat(10_000);
            let input = ToolInput::Structured(json!({"field": long_string}));
            let result = extract_string_field(&input, "field");
            assert!(result.is_ok());
            assert_eq!(result.unwrap().len(), 10_000);
        }

        #[test]
        fn test_string_with_escape_sequences() {
            let input = ToolInput::Structured(
                json!({"field": "line1\nline2\ttab\r\nwindows\u{0000}null"}),
            );
            let result = extract_string_field(&input, "field");
            assert!(result.is_ok());
            assert!(result.unwrap().contains('\n'));
        }

        #[test]
        fn test_json_number_boundary_u64_max() {
            // u64::MAX as string because JSON can't represent it precisely
            let input = ToolInput::Structured(json!({"num": 18_446_744_073_709_551_615_u64}));
            let result = extract_u64_field(&input, "num");
            // This should work if the JSON was created with serde_json from u64
            assert!(result.is_ok());
            assert_eq!(result.unwrap(), u64::MAX);
        }
    }

    // Unit tests - GitLabBuilder validates credentials on construction, so these need real tokens
    #[test]
    #[ignore = "requires GITLAB_TOKEN"]
    fn test_get_issue_tool_name() {
        let token = std::env::var("GITLAB_TOKEN").expect("GITLAB_TOKEN must be set");
        let tool = GetIssueTool::new("https://gitlab.com", "test/project", token).unwrap();
        assert_eq!(tool.name(), "get_issue");
    }

    #[test]
    #[ignore = "requires GITLAB_TOKEN"]
    fn test_create_issue_tool_name() {
        let token = std::env::var("GITLAB_TOKEN").expect("GITLAB_TOKEN must be set");
        let tool = CreateIssueTool::new("https://gitlab.com", "test/project", token).unwrap();
        assert_eq!(tool.name(), "create_issue");
    }

    #[test]
    #[ignore = "requires GITLAB_TOKEN"]
    fn test_update_issue_tool_name() {
        let token = std::env::var("GITLAB_TOKEN").expect("GITLAB_TOKEN must be set");
        let tool = UpdateIssueTool::new("https://gitlab.com", "test/project", token).unwrap();
        assert_eq!(tool.name(), "update_issue");
    }

    #[test]
    #[ignore = "requires GITLAB_TOKEN"]
    fn test_list_issues_tool_name() {
        let token = std::env::var("GITLAB_TOKEN").expect("GITLAB_TOKEN must be set");
        let tool = ListIssuesTool::new("https://gitlab.com", "test/project", token).unwrap();
        assert_eq!(tool.name(), "list_issues");
    }

    #[test]
    #[ignore = "requires GITLAB_TOKEN"]
    fn test_get_merge_request_tool_name() {
        let token = std::env::var("GITLAB_TOKEN").expect("GITLAB_TOKEN must be set");
        let tool = GetMergeRequestTool::new("https://gitlab.com", "test/project", token).unwrap();
        assert_eq!(tool.name(), "get_merge_request");
    }

    #[test]
    #[ignore = "requires GITLAB_TOKEN"]
    fn test_create_merge_request_tool_name() {
        let token = std::env::var("GITLAB_TOKEN").expect("GITLAB_TOKEN must be set");
        let tool =
            CreateMergeRequestTool::new("https://gitlab.com", "test/project", token).unwrap();
        assert_eq!(tool.name(), "create_merge_request");
    }

    #[test]
    #[ignore = "requires GITLAB_TOKEN"]
    fn test_list_merge_requests_tool_name() {
        let token = std::env::var("GITLAB_TOKEN").expect("GITLAB_TOKEN must be set");
        let tool = ListMergeRequestsTool::new("https://gitlab.com", "test/project", token).unwrap();
        assert_eq!(tool.name(), "list_merge_requests");
    }

    #[test]
    #[ignore = "requires GITLAB_TOKEN"]
    fn test_get_project_tool_name() {
        let token = std::env::var("GITLAB_TOKEN").expect("GITLAB_TOKEN must be set");
        let tool = GetProjectTool::new("https://gitlab.com", "test/project", token).unwrap();
        assert_eq!(tool.name(), "get_project");
    }

    // Integration tests - require real GitLab token via GITLAB_TOKEN env var
    #[tokio::test]
    #[ignore = "requires GITLAB_TOKEN"]
    async fn test_get_issue_integration() {
        let token = std::env::var("GITLAB_TOKEN").expect("GITLAB_TOKEN must be set");
        let tool = GetIssueTool::new("https://gitlab.com", "test/project", token).unwrap();
        let input = ToolInput::Structured(json!({"issue_iid": 1}));
        let result = tool._call(input).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    #[ignore = "requires GITLAB_TOKEN"]
    async fn test_list_issues_integration() {
        let token = std::env::var("GITLAB_TOKEN").expect("GITLAB_TOKEN must be set");
        let tool = ListIssuesTool::new("https://gitlab.com", "test/project", token).unwrap();
        let input = ToolInput::Structured(json!({"state": "opened"}));
        let result = tool._call(input).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    #[ignore = "requires GITLAB_TOKEN"]
    async fn test_get_project_integration() {
        let token = std::env::var("GITLAB_TOKEN").expect("GITLAB_TOKEN must be set");
        let tool = GetProjectTool::new("https://gitlab.com", "test/project", token).unwrap();
        let input = ToolInput::Structured(json!({}));
        let result = tool._call(input).await;
        assert!(result.is_ok());
    }
}
