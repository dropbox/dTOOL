//! `ClickUp` API wrapper implementation

use dashflow::core::config_loader::env_vars::{env_string, CLICKUP_ACCESS_TOKEN};
use dashflow::{DEFAULT_HTTP_CONNECT_TIMEOUT, DEFAULT_HTTP_REQUEST_TIMEOUT};
use reqwest::{header, Client};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::env;
use thiserror::Error;

const DEFAULT_URL: &str = "https://api.clickup.com/api/v2";

#[derive(Error, Debug)]
#[non_exhaustive]
pub enum ClickupError {
    #[error("HTTP request failed: {0}")]
    RequestError(#[from] reqwest::Error),

    #[error("JSON parsing error: {0}")]
    JsonError(#[from] serde_json::Error),

    #[error("Missing access token. Set CLICKUP_ACCESS_TOKEN environment variable")]
    MissingToken,

    #[error("Invalid mode: {0}")]
    InvalidMode(String),

    #[error("API error: {0}")]
    ApiError(String),

    #[error("Missing required field: {0}")]
    MissingField(String),

    #[error("API wrapper not initialized. This can happen if ClickupAction was deserialized without re-initializing the API wrapper.")]
    NotInitialized,
}

pub type Result<T> = std::result::Result<T, ClickupError>;

/// `ClickUp` API wrapper
#[derive(Clone)]
pub struct ClickupAPIWrapper {
    client: Client,
    pub team_id: Option<String>,
    pub space_id: Option<String>,
    pub folder_id: Option<String>,
    pub list_id: Option<String>,
}

impl ClickupAPIWrapper {
    /// Create a new `ClickupAPIWrapper` from environment variables
    pub async fn new() -> Result<Self> {
        let access_token = env_string(CLICKUP_ACCESS_TOKEN).ok_or_else(|| {
            ClickupError::ApiError(format!(
                "{CLICKUP_ACCESS_TOKEN} environment variable not set"
            ))
        })?;

        let mut headers = header::HeaderMap::new();
        headers.insert(
            header::AUTHORIZATION,
            header::HeaderValue::from_str(&access_token)
                .map_err(|e| ClickupError::ApiError(format!("Invalid token format: {e}")))?,
        );
        headers.insert(
            header::CONTENT_TYPE,
            header::HeaderValue::from_static("application/json"),
        );

        let client = Client::builder()
            .default_headers(headers)
            .timeout(DEFAULT_HTTP_REQUEST_TIMEOUT)
            .connect_timeout(DEFAULT_HTTP_CONNECT_TIMEOUT)
            .build()?;

        let mut wrapper = Self {
            client,
            team_id: None,
            space_id: None,
            folder_id: None,
            list_id: None,
        };

        // Fetch default IDs
        wrapper.team_id = wrapper.fetch_team_id().await.ok();
        if let Some(team_id) = &wrapper.team_id {
            wrapper.space_id = wrapper.fetch_space_id(team_id).await.ok();
            if let Some(space_id) = &wrapper.space_id {
                wrapper.folder_id = wrapper.fetch_folder_id(space_id).await.ok();
                if let (Some(space_id), folder_id) = (&wrapper.space_id, &wrapper.folder_id) {
                    wrapper.list_id = wrapper
                        .fetch_list_id(space_id, folder_id.as_deref())
                        .await
                        .ok();
                }
            }
        }

        Ok(wrapper)
    }

    /// Create a new `ClickupAPIWrapper` with explicit access token
    pub async fn with_token(access_token: String) -> Result<Self> {
        env::set_var("CLICKUP_ACCESS_TOKEN", &access_token);
        Self::new().await
    }

    async fn fetch_team_id(&self) -> Result<String> {
        let url = format!("{DEFAULT_URL}/team");
        let response: Value = self.client.get(&url).send().await?.json().await?;

        if let Some(teams) = response["teams"].as_array() {
            if let Some(first_team) = teams.first() {
                if let Some(id) = first_team["id"].as_str() {
                    return Ok(id.to_string());
                }
            }
        }
        Err(ClickupError::ApiError("No teams found".to_string()))
    }

    async fn fetch_space_id(&self, team_id: &str) -> Result<String> {
        let url = format!("{DEFAULT_URL}/team/{team_id}/space");
        let response: Value = self
            .client
            .get(&url)
            .query(&[("archived", "false")])
            .send()
            .await?
            .json()
            .await?;

        if let Some(spaces) = response["spaces"].as_array() {
            if let Some(first_space) = spaces.first() {
                if let Some(id) = first_space["id"].as_str() {
                    return Ok(id.to_string());
                }
            }
        }
        Err(ClickupError::ApiError("No spaces found".to_string()))
    }

    async fn fetch_folder_id(&self, space_id: &str) -> Result<String> {
        let url = format!("{DEFAULT_URL}/space/{space_id}/folder");
        let response: Value = self
            .client
            .get(&url)
            .query(&[("archived", "false")])
            .send()
            .await?
            .json()
            .await?;

        if let Some(folders) = response["folders"].as_array() {
            if let Some(first_folder) = folders.first() {
                if let Some(id) = first_folder["id"].as_str() {
                    return Ok(id.to_string());
                }
            }
        }
        // No folders is OK - return empty to signal folderless lists
        Ok(String::new())
    }

    async fn fetch_list_id(&self, space_id: &str, folder_id: Option<&str>) -> Result<String> {
        let url = if let Some(fid) = folder_id.filter(|s| !s.is_empty()) {
            format!("{DEFAULT_URL}/folder/{fid}/list")
        } else {
            format!("{DEFAULT_URL}/space/{space_id}/list")
        };

        let response: Value = self
            .client
            .get(&url)
            .query(&[("archived", "false")])
            .send()
            .await?
            .json()
            .await?;

        // For folder-based lists
        if folder_id.is_some() && response["id"].is_string() {
            if let Some(id) = response["id"].as_str() {
                return Ok(id.to_string());
            }
        }

        // For folderless lists
        if let Some(lists) = response["lists"].as_array() {
            if let Some(first_list) = lists.first() {
                if let Some(id) = first_list["id"].as_str() {
                    return Ok(id.to_string());
                }
            }
        }

        Err(ClickupError::ApiError("No lists found".to_string()))
    }

    /// Get all teams the user is authorized for
    pub async fn get_authorized_teams(&self) -> Result<Value> {
        let url = format!("{DEFAULT_URL}/team");
        let response: Value = self.client.get(&url).send().await?.json().await?;
        Ok(response)
    }

    /// Get all spaces for the team
    pub async fn get_spaces(&self) -> Result<Value> {
        let team_id = self
            .team_id
            .as_ref()
            .ok_or_else(|| ClickupError::MissingField("team_id".to_string()))?;

        let url = format!("{DEFAULT_URL}/team/{team_id}/space");
        let response: Value = self
            .client
            .get(&url)
            .query(&[("archived", "false")])
            .send()
            .await?
            .json()
            .await?;
        Ok(response)
    }

    /// Get all folders
    pub async fn get_folders(&self) -> Result<Value> {
        let team_id = self
            .team_id
            .as_ref()
            .ok_or_else(|| ClickupError::MissingField("team_id".to_string()))?;

        let url = format!("{DEFAULT_URL}/team/{team_id}/space");
        let response: Value = self
            .client
            .get(&url)
            .query(&[("archived", "false")])
            .send()
            .await?
            .json()
            .await?;
        Ok(response)
    }

    /// Get all lists
    pub async fn get_lists(&self) -> Result<Value> {
        let folder_id = self
            .folder_id
            .as_ref()
            .ok_or_else(|| ClickupError::MissingField("folder_id".to_string()))?;

        let url = format!("{DEFAULT_URL}/folder/{folder_id}/list");
        let response: Value = self
            .client
            .get(&url)
            .query(&[("archived", "false")])
            .send()
            .await?
            .json()
            .await?;
        Ok(response)
    }

    /// Get a specific task
    pub async fn get_task(&self, query: &str) -> Result<Value> {
        let params: HashMap<String, Value> = serde_json::from_str(query)?;
        let task_id = params
            .get("task_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ClickupError::MissingField("task_id".to_string()))?;

        let url = format!("{DEFAULT_URL}/task/{task_id}");
        let response: Value = self
            .client
            .get(&url)
            .query(&[
                ("custom_task_ids", "true"),
                ("team_id", self.team_id.as_deref().unwrap_or("")),
                ("include_subtasks", "true"),
            ])
            .send()
            .await?
            .json()
            .await?;

        Ok(response)
    }

    /// Get a specific attribute from a task
    pub async fn get_task_attribute(&self, query: &str) -> Result<Value> {
        let task_data = self.get_task(query).await?;
        let params: HashMap<String, Value> = serde_json::from_str(query)?;

        let attribute_name = params
            .get("attribute_name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ClickupError::MissingField("attribute_name".to_string()))?;

        if let Some(value) = task_data.get(attribute_name) {
            Ok(json!({ attribute_name: value }))
        } else {
            Err(ClickupError::ApiError(format!(
                "Attribute '{attribute_name}' not found in task"
            )))
        }
    }

    /// Create a new task
    pub async fn create_task(&self, query: &str) -> Result<Value> {
        let list_id = self
            .list_id
            .as_ref()
            .ok_or_else(|| ClickupError::MissingField("list_id".to_string()))?;

        let payload: Value = serde_json::from_str(query)?;

        let url = format!("{DEFAULT_URL}/list/{list_id}/task");
        let response: Value = self
            .client
            .post(&url)
            .query(&[
                ("custom_task_ids", "true"),
                ("team_id", self.team_id.as_deref().unwrap_or("")),
            ])
            .json(&payload)
            .send()
            .await?
            .json()
            .await?;

        Ok(response)
    }

    /// Create a new list
    pub async fn create_list(&self, query: &str) -> Result<Value> {
        let location = if let Some(folder_id) = self.folder_id.as_ref().filter(|s| !s.is_empty()) {
            folder_id.clone()
        } else {
            self.space_id
                .as_ref()
                .ok_or_else(|| ClickupError::MissingField("space_id".to_string()))?
                .clone()
        };

        let payload: Value = serde_json::from_str(query)?;

        let url = format!("{DEFAULT_URL}/folder/{location}/list");
        let response: Value = self
            .client
            .post(&url)
            .json(&payload)
            .send()
            .await?
            .json()
            .await?;

        Ok(response)
    }

    /// Create a new folder
    pub async fn create_folder(&self, query: &str) -> Result<Value> {
        let space_id = self
            .space_id
            .as_ref()
            .ok_or_else(|| ClickupError::MissingField("space_id".to_string()))?;

        let params: HashMap<String, Value> = serde_json::from_str(query)?;
        let name = params
            .get("name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ClickupError::MissingField("name".to_string()))?;

        let payload = json!({ "name": name });

        let url = format!("{DEFAULT_URL}/space/{space_id}/folder");
        let response: Value = self
            .client
            .post(&url)
            .json(&payload)
            .send()
            .await?
            .json()
            .await?;

        Ok(response)
    }

    /// Update a task
    pub async fn update_task(&self, query: &str) -> Result<Value> {
        let params: HashMap<String, Value> = serde_json::from_str(query)?;

        let task_id = params
            .get("task_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ClickupError::MissingField("task_id".to_string()))?;

        let attribute_name = params
            .get("attribute_name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ClickupError::MissingField("attribute_name".to_string()))?;

        let value = params
            .get("value")
            .ok_or_else(|| ClickupError::MissingField("value".to_string()))?;

        let payload = json!({ attribute_name: value });

        let url = format!("{DEFAULT_URL}/task/{task_id}");
        let response: Value = self
            .client
            .put(&url)
            .query(&[
                ("custom_task_ids", "true"),
                ("team_id", self.team_id.as_deref().unwrap_or("")),
                ("include_subtasks", "true"),
            ])
            .json(&payload)
            .send()
            .await?
            .json()
            .await?;

        Ok(response)
    }

    /// Update task assignees
    pub async fn update_task_assignees(&self, query: &str) -> Result<Value> {
        let params: HashMap<String, Value> = serde_json::from_str(query)?;

        let task_id = params
            .get("task_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ClickupError::MissingField("task_id".to_string()))?;

        let operation = params
            .get("operation")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ClickupError::MissingField("operation".to_string()))?;

        let users = params
            .get("users")
            .and_then(|v| v.as_array())
            .ok_or_else(|| ClickupError::MissingField("users".to_string()))?;

        let assignee_payload = match operation {
            "add" => json!({ "add": users, "rem": [] }),
            "rem" => json!({ "add": [], "rem": users }),
            _ => {
                return Err(ClickupError::InvalidMode(format!(
                    "Invalid operation '{operation}'. Valid options: 'add', 'rem'"
                )))
            }
        };

        let payload = json!({ "assignees": assignee_payload });

        let url = format!("{DEFAULT_URL}/task/{task_id}");
        let response: Value = self
            .client
            .put(&url)
            .query(&[
                ("custom_task_ids", "true"),
                ("team_id", self.team_id.as_deref().unwrap_or("")),
                ("include_subtasks", "true"),
            ])
            .json(&payload)
            .send()
            .await?
            .json()
            .await?;

        Ok(response)
    }

    /// Run an operation based on mode
    pub async fn run(&self, mode: &str, query: &str) -> Result<String> {
        let output = match mode {
            "get_task" => self.get_task(query).await?,
            "get_task_attribute" => self.get_task_attribute(query).await?,
            "get_teams" => self.get_authorized_teams().await?,
            "create_task" => self.create_task(query).await?,
            "create_list" => self.create_list(query).await?,
            "create_folder" => self.create_folder(query).await?,
            "get_lists" => self.get_lists().await?,
            "get_folders" => self.get_folders().await?,
            "get_spaces" => self.get_spaces().await?,
            "update_task" => self.update_task(query).await?,
            "update_task_assignees" => self.update_task_assignees(query).await?,
            _ => return Err(ClickupError::InvalidMode(mode.to_string())),
        };

        Ok(serde_json::to_string(&output)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::error::Error as StdError;

    // ========================================================================
    // Error Display Tests
    // ========================================================================

    #[test]
    fn test_error_display_missing_token() {
        let err = ClickupError::MissingToken;
        assert_eq!(
            err.to_string(),
            "Missing access token. Set CLICKUP_ACCESS_TOKEN environment variable"
        );
    }

    #[test]
    fn test_error_display_invalid_mode() {
        let err = ClickupError::InvalidMode("bad_mode".to_string());
        assert_eq!(err.to_string(), "Invalid mode: bad_mode");
    }

    #[test]
    fn test_error_display_api_error() {
        let err = ClickupError::ApiError("Server returned 500".to_string());
        assert_eq!(err.to_string(), "API error: Server returned 500");
    }

    #[test]
    fn test_error_display_missing_field() {
        let err = ClickupError::MissingField("task_id".to_string());
        assert_eq!(err.to_string(), "Missing required field: task_id");
    }

    #[test]
    fn test_error_display_not_initialized() {
        let err = ClickupError::NotInitialized;
        assert!(err.to_string().contains("not initialized"));
        assert!(err.to_string().contains("ClickupAction"));
    }

    #[test]
    fn test_error_display_json_error() {
        let json_err = serde_json::from_str::<Value>("invalid").unwrap_err();
        let err = ClickupError::JsonError(json_err);
        assert!(err.to_string().contains("JSON parsing error:"));
    }

    // ========================================================================
    // Error Debug Tests
    // ========================================================================

    #[test]
    fn test_error_debug_format() {
        let err = ClickupError::InvalidMode("test".to_string());
        let debug_str = format!("{:?}", err);
        assert!(debug_str.contains("InvalidMode"));
        assert!(debug_str.contains("test"));
    }

    #[test]
    fn test_error_debug_missing_token() {
        let err = ClickupError::MissingToken;
        let debug_str = format!("{:?}", err);
        assert!(debug_str.contains("MissingToken"));
    }

    // ========================================================================
    // Error From Implementations
    // ========================================================================

    #[test]
    fn test_error_from_json() {
        let json_err = serde_json::from_str::<Value>("{bad json}").unwrap_err();
        let err: ClickupError = json_err.into();
        assert!(matches!(err, ClickupError::JsonError(_)));
    }

    // Note: reqwest::Error is harder to construct in tests without network

    // ========================================================================
    // Error Source Tests
    // ========================================================================

    #[test]
    fn test_error_source_json() {
        let json_err = serde_json::from_str::<Value>("!").unwrap_err();
        let err = ClickupError::JsonError(json_err);
        assert!(err.source().is_some());
    }

    #[test]
    fn test_error_source_missing_token() {
        let err = ClickupError::MissingToken;
        assert!(err.source().is_none());
    }

    #[test]
    fn test_error_source_invalid_mode() {
        let err = ClickupError::InvalidMode("x".to_string());
        assert!(err.source().is_none());
    }

    #[test]
    fn test_error_source_api_error() {
        let err = ClickupError::ApiError("x".to_string());
        assert!(err.source().is_none());
    }

    #[test]
    fn test_error_source_missing_field() {
        let err = ClickupError::MissingField("x".to_string());
        assert!(err.source().is_none());
    }

    #[test]
    fn test_error_source_not_initialized() {
        let err = ClickupError::NotInitialized;
        assert!(err.source().is_none());
    }

    // ========================================================================
    // Result Type Tests
    // ========================================================================

    #[test]
    fn test_result_type_ok() {
        fn success() -> Result<i32> {
            Ok(42)
        }
        assert_eq!(success().unwrap(), 42);
    }

    #[test]
    fn test_result_type_err() {
        fn failure() -> Result<i32> {
            Err(ClickupError::MissingToken)
        }
        assert!(failure().is_err());
    }

    // ========================================================================
    // Error Type Property Tests
    // ========================================================================

    #[test]
    fn test_error_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<ClickupError>();
    }

    #[test]
    fn test_error_non_exhaustive() {
        // Test that all variants can be matched (non_exhaustive still allows internal matching)
        let err = ClickupError::MissingToken;
        let _ = match err {
            ClickupError::RequestError(_) => "request",
            ClickupError::JsonError(_) => "json",
            ClickupError::MissingToken => "missing_token",
            ClickupError::InvalidMode(_) => "invalid_mode",
            ClickupError::ApiError(_) => "api",
            ClickupError::MissingField(_) => "missing_field",
            ClickupError::NotInitialized => "not_init",
            // Note: #[non_exhaustive] enables forward compatibility for external crates,
            // but all current variants are covered above.
        };
    }

    // ========================================================================
    // Constants Tests
    // ========================================================================

    #[test]
    fn test_default_url_format() {
        assert!(DEFAULT_URL.starts_with("https://"));
        assert!(DEFAULT_URL.contains("clickup.com"));
        assert!(DEFAULT_URL.contains("api"));
        assert!(DEFAULT_URL.ends_with("v2"));
    }

    // ========================================================================
    // ClickupAPIWrapper Field Tests
    // ========================================================================

    #[test]
    fn test_wrapper_clone_trait() {
        // Verify Clone is derived properly by checking the trait bounds compile
        fn assert_clone<T: Clone>() {}
        assert_clone::<ClickupAPIWrapper>();
    }

    // ========================================================================
    // JSON Payload Edge Cases
    // ========================================================================

    #[test]
    fn test_json_error_empty_string() {
        let result: std::result::Result<Value, _> = serde_json::from_str("");
        assert!(result.is_err());
        let err: ClickupError = result.unwrap_err().into();
        assert!(matches!(err, ClickupError::JsonError(_)));
    }

    #[test]
    fn test_json_error_incomplete_object() {
        let result: std::result::Result<Value, _> = serde_json::from_str("{\"key\":");
        assert!(result.is_err());
        let err: ClickupError = result.unwrap_err().into();
        assert!(matches!(err, ClickupError::JsonError(_)));
    }

    #[test]
    fn test_json_error_invalid_unicode() {
        let result: std::result::Result<Value, _> = serde_json::from_str("\"\\uXXXX\"");
        assert!(result.is_err());
        let err: ClickupError = result.unwrap_err().into();
        assert!(matches!(err, ClickupError::JsonError(_)));
    }

    // ========================================================================
    // Error Message Content Tests
    // ========================================================================

    #[test]
    fn test_invalid_mode_contains_mode_name() {
        let mode = "some_unknown_mode";
        let err = ClickupError::InvalidMode(mode.to_string());
        assert!(err.to_string().contains(mode));
    }

    #[test]
    fn test_missing_field_contains_field_name() {
        let field = "assignees";
        let err = ClickupError::MissingField(field.to_string());
        assert!(err.to_string().contains(field));
    }

    #[test]
    fn test_api_error_contains_message() {
        let msg = "Rate limit exceeded";
        let err = ClickupError::ApiError(msg.to_string());
        assert!(err.to_string().contains(msg));
    }
}
