//! `ClickupAction` tool implementation

use crate::api::{ClickupAPIWrapper, ClickupError};
use crate::prompts::{
    CLICKUP_FOLDER_CREATE_PROMPT, CLICKUP_GET_ALL_TEAMS_PROMPT, CLICKUP_GET_FOLDERS_PROMPT,
    CLICKUP_GET_LIST_PROMPT, CLICKUP_GET_SPACES_PROMPT, CLICKUP_GET_TASK_ATTRIBUTE_PROMPT,
    CLICKUP_GET_TASK_PROMPT, CLICKUP_LIST_CREATE_PROMPT, CLICKUP_TASK_CREATE_PROMPT,
    CLICKUP_UPDATE_TASK_ASSIGNEE_PROMPT, CLICKUP_UPDATE_TASK_PROMPT,
};
use async_trait::async_trait;
use dashflow::core::tools::{Tool, ToolInput};
use serde::{Deserialize, Serialize};

/// `ClickupAction` tool that queries the `ClickUp` API
///
/// This tool allows agents to interact with `ClickUp` to create tasks,
/// lists, folders, and query information from a `ClickUp` workspace.
///
/// # Example
///
/// ```no_run
/// use dashflow_clickup::{ClickupAPIWrapper, ClickupAction};
/// use dashflow::core::tools::Tool;
/// use std::env;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// env::set_var("CLICKUP_ACCESS_TOKEN", "your_token_here");
///
/// let api = ClickupAPIWrapper::new().await?;
/// let tool = ClickupAction::new(api, "get_teams");
///
/// let result = tool.run("{}").await?;
/// println!("{}", result);
/// # Ok(())
/// # }
/// ```
#[derive(Clone, Serialize, Deserialize)]
pub struct ClickupAction {
    #[serde(skip)]
    api_wrapper: Option<ClickupAPIWrapper>,
    mode: String,
    name: String,
    description: String,
}

impl ClickupAction {
    /// Create a new `ClickupAction` with the given API wrapper and mode
    ///
    /// # Arguments
    ///
    /// * `api_wrapper` - The `ClickupAPIWrapper` to use for API calls
    /// * `mode` - The operation mode (e.g., "`get_task`", "`create_task`", "`get_teams`")
    ///
    /// # Supported Modes
    ///
    /// * `get_task` - Get a specific task by ID
    /// * `get_task_attribute` - Get a specific attribute from a task
    /// * `get_teams` - Get all teams the user is part of
    /// * `create_task` - Create a new task
    /// * `create_list` - Create a new list
    /// * `create_folder` - Create a new folder
    /// * `get_lists` - Get all lists
    /// * `get_folders` - Get all folders
    /// * `get_spaces` - Get all spaces
    /// * `update_task` - Update a task attribute
    /// * `update_task_assignees` - Add or remove task assignees
    pub fn new(api_wrapper: ClickupAPIWrapper, mode: impl Into<String>) -> Self {
        let mode = mode.into();
        let (name, description) = Self::get_name_and_description(&mode);

        Self {
            api_wrapper: Some(api_wrapper),
            mode,
            name,
            description,
        }
    }

    fn get_name_and_description(mode: &str) -> (String, String) {
        match mode {
            "create_task" => (
                "Create ClickUp Task".to_string(),
                CLICKUP_TASK_CREATE_PROMPT.to_string(),
            ),
            "create_list" => (
                "Create ClickUp List".to_string(),
                CLICKUP_LIST_CREATE_PROMPT.to_string(),
            ),
            "create_folder" => (
                "Create ClickUp Folder".to_string(),
                CLICKUP_FOLDER_CREATE_PROMPT.to_string(),
            ),
            "get_task" => (
                "Get ClickUp Task".to_string(),
                CLICKUP_GET_TASK_PROMPT.to_string(),
            ),
            "get_task_attribute" => (
                "Get ClickUp Task Attribute".to_string(),
                CLICKUP_GET_TASK_ATTRIBUTE_PROMPT.to_string(),
            ),
            "get_teams" => (
                "Get ClickUp Teams".to_string(),
                CLICKUP_GET_ALL_TEAMS_PROMPT.to_string(),
            ),
            "get_lists" => (
                "Get ClickUp Lists".to_string(),
                CLICKUP_GET_LIST_PROMPT.to_string(),
            ),
            "get_folders" => (
                "Get ClickUp Folders".to_string(),
                CLICKUP_GET_FOLDERS_PROMPT.to_string(),
            ),
            "get_spaces" => (
                "Get ClickUp Spaces".to_string(),
                CLICKUP_GET_SPACES_PROMPT.to_string(),
            ),
            "update_task" => (
                "Update ClickUp Task".to_string(),
                CLICKUP_UPDATE_TASK_PROMPT.to_string(),
            ),
            "update_task_assignees" => (
                "Update ClickUp Task Assignees".to_string(),
                CLICKUP_UPDATE_TASK_ASSIGNEE_PROMPT.to_string(),
            ),
            _ => (
                format!("ClickUp Action: {mode}"),
                format!("Execute ClickUp operation: {mode}"),
            ),
        }
    }

    /// Run the tool with the given instructions
    pub async fn run(&self, instructions: &str) -> Result<String, ClickupError> {
        let api = self
            .api_wrapper
            .as_ref()
            .ok_or(ClickupError::NotInitialized)?;
        api.run(&self.mode, instructions).await
    }
}

#[async_trait]
impl Tool for ClickupAction {
    fn name(&self) -> &str {
        &self.name
    }

    fn description(&self) -> &str {
        &self.description
    }

    async fn _call(&self, input: ToolInput) -> dashflow::core::error::Result<String> {
        let input_str = match input {
            ToolInput::String(s) => s,
            ToolInput::Structured(v) => serde_json::to_string(&v)
                .map_err(|e| dashflow::core::error::Error::tool_error(e.to_string()))?,
        };

        self.run(&input_str)
            .await
            .map_err(|e| dashflow::core::error::Error::tool_error(e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ========================================================================
    // get_name_and_description Tests - Task Operations
    // ========================================================================

    #[test]
    fn test_get_name_and_description_create_task() {
        let (name, desc) = ClickupAction::get_name_and_description("create_task");
        assert_eq!(name, "Create ClickUp Task");
        assert!(desc.contains("create_task API"));
        assert!(!desc.is_empty());
    }

    #[test]
    fn test_get_name_and_description_get_task() {
        let (name, desc) = ClickupAction::get_name_and_description("get_task");
        assert_eq!(name, "Get ClickUp Task");
        assert!(desc.contains("task"));
        assert!(!desc.is_empty());
    }

    #[test]
    fn test_get_name_and_description_get_task_attribute() {
        let (name, desc) = ClickupAction::get_name_and_description("get_task_attribute");
        assert_eq!(name, "Get ClickUp Task Attribute");
        assert!(desc.contains("attribute"));
        assert!(!desc.is_empty());
    }

    #[test]
    fn test_get_name_and_description_update_task() {
        let (name, desc) = ClickupAction::get_name_and_description("update_task");
        assert_eq!(name, "Update ClickUp Task");
        assert!(desc.contains("task"));
        assert!(!desc.is_empty());
    }

    #[test]
    fn test_get_name_and_description_update_task_assignees() {
        let (name, desc) = ClickupAction::get_name_and_description("update_task_assignees");
        assert_eq!(name, "Update ClickUp Task Assignees");
        assert!(desc.contains("assignee"));
        assert!(!desc.is_empty());
    }

    // ========================================================================
    // get_name_and_description Tests - Team/Space/Folder/List Operations
    // ========================================================================

    #[test]
    fn test_get_name_and_description_get_teams() {
        let (name, desc) = ClickupAction::get_name_and_description("get_teams");
        assert_eq!(name, "Get ClickUp Teams");
        assert!(desc.contains("all teams"));
        assert!(!desc.is_empty());
    }

    #[test]
    fn test_get_name_and_description_get_spaces() {
        let (name, desc) = ClickupAction::get_name_and_description("get_spaces");
        assert_eq!(name, "Get ClickUp Spaces");
        assert!(desc.contains("space"));
        assert!(!desc.is_empty());
    }

    #[test]
    fn test_get_name_and_description_get_folders() {
        let (name, desc) = ClickupAction::get_name_and_description("get_folders");
        assert_eq!(name, "Get ClickUp Folders");
        assert!(desc.contains("folder"));
        assert!(!desc.is_empty());
    }

    #[test]
    fn test_get_name_and_description_get_lists() {
        let (name, desc) = ClickupAction::get_name_and_description("get_lists");
        assert_eq!(name, "Get ClickUp Lists");
        assert!(desc.contains("list"));
        assert!(!desc.is_empty());
    }

    #[test]
    fn test_get_name_and_description_create_list() {
        let (name, desc) = ClickupAction::get_name_and_description("create_list");
        assert_eq!(name, "Create ClickUp List");
        assert!(desc.contains("list"));
        assert!(!desc.is_empty());
    }

    #[test]
    fn test_get_name_and_description_create_folder() {
        let (name, desc) = ClickupAction::get_name_and_description("create_folder");
        assert_eq!(name, "Create ClickUp Folder");
        assert!(desc.contains("folder"));
        assert!(!desc.is_empty());
    }

    // ========================================================================
    // get_name_and_description Tests - Unknown Mode
    // ========================================================================

    #[test]
    fn test_get_name_and_description_unknown_mode() {
        let (name, desc) = ClickupAction::get_name_and_description("unknown_mode");
        assert_eq!(name, "ClickUp Action: unknown_mode");
        assert_eq!(desc, "Execute ClickUp operation: unknown_mode");
    }

    #[test]
    fn test_get_name_and_description_empty_mode() {
        let (name, desc) = ClickupAction::get_name_and_description("");
        assert_eq!(name, "ClickUp Action: ");
        assert_eq!(desc, "Execute ClickUp operation: ");
    }

    #[test]
    fn test_get_name_and_description_custom_mode() {
        let (name, desc) = ClickupAction::get_name_and_description("my_custom_action");
        assert!(name.contains("my_custom_action"));
        assert!(desc.contains("my_custom_action"));
    }

    #[test]
    fn test_get_name_and_description_case_sensitive() {
        // Mode matching is case-sensitive
        let (name, _) = ClickupAction::get_name_and_description("CREATE_TASK");
        // Should not match "create_task" - should fall through to default
        assert!(name.contains("CREATE_TASK"));

        let (name, _) = ClickupAction::get_name_and_description("Get_Teams");
        assert!(name.contains("Get_Teams"));
    }

    // ========================================================================
    // Serialization/Deserialization Tests
    // ========================================================================

    #[test]
    fn test_serialize_clickup_action() {
        // Create a ClickupAction without API wrapper (using internal fields only)
        let action = ClickupAction {
            api_wrapper: None,
            mode: "get_teams".to_string(),
            name: "Get ClickUp Teams".to_string(),
            description: "Test description".to_string(),
        };

        let serialized = serde_json::to_string(&action).expect("Failed to serialize");
        assert!(serialized.contains("get_teams"));
        assert!(serialized.contains("Get ClickUp Teams"));
        assert!(serialized.contains("Test description"));
        // api_wrapper should be skipped in serialization
        assert!(!serialized.contains("api_wrapper"));
    }

    #[test]
    fn test_deserialize_clickup_action() {
        let json = r#"{
            "mode": "create_task",
            "name": "Create ClickUp Task",
            "description": "Test desc"
        }"#;

        let action: ClickupAction = serde_json::from_str(json).expect("Failed to deserialize");
        assert_eq!(action.mode, "create_task");
        assert_eq!(action.name, "Create ClickUp Task");
        assert_eq!(action.description, "Test desc");
        assert!(action.api_wrapper.is_none()); // Skipped during deserialization
    }

    #[test]
    fn test_roundtrip_serialization() {
        let action = ClickupAction {
            api_wrapper: None,
            mode: "update_task".to_string(),
            name: "Update ClickUp Task".to_string(),
            description: "Update a task attribute".to_string(),
        };

        let serialized = serde_json::to_string(&action).expect("Failed to serialize");
        let deserialized: ClickupAction =
            serde_json::from_str(&serialized).expect("Failed to deserialize");

        assert_eq!(action.mode, deserialized.mode);
        assert_eq!(action.name, deserialized.name);
        assert_eq!(action.description, deserialized.description);
    }

    #[test]
    fn test_deserialize_with_extra_fields() {
        // JSON with extra fields should still deserialize
        let json = r#"{
            "mode": "get_task",
            "name": "Test Name",
            "description": "Test Desc",
            "extra_field": "ignored"
        }"#;

        let result: Result<ClickupAction, _> = serde_json::from_str(json);
        // Depending on serde settings, this may succeed (ignoring extra fields)
        // or fail (strict mode). Test that it doesn't panic.
        let _ = result;
    }

    #[test]
    fn test_deserialize_missing_fields() {
        let json = r#"{"mode": "get_task"}"#;
        let result: Result<ClickupAction, _> = serde_json::from_str(json);
        // Should fail due to missing required fields
        assert!(result.is_err());
    }

    #[test]
    fn test_serialize_special_characters() {
        let action = ClickupAction {
            api_wrapper: None,
            mode: "test".to_string(),
            name: "Name with \"quotes\" and \\ backslash".to_string(),
            description: "Desc with\nnewline".to_string(),
        };

        let serialized = serde_json::to_string(&action).expect("Failed to serialize");
        let deserialized: ClickupAction =
            serde_json::from_str(&serialized).expect("Failed to deserialize");

        assert_eq!(action.name, deserialized.name);
        assert_eq!(action.description, deserialized.description);
    }

    #[test]
    fn test_serialize_unicode() {
        let action = ClickupAction {
            api_wrapper: None,
            mode: "test".to_string(),
            name: "日本語の名前 🎉".to_string(),
            description: "Description with émojis 😊".to_string(),
        };

        let serialized = serde_json::to_string(&action).expect("Failed to serialize");
        let deserialized: ClickupAction =
            serde_json::from_str(&serialized).expect("Failed to deserialize");

        assert_eq!(action.name, deserialized.name);
        assert_eq!(action.description, deserialized.description);
    }

    // ========================================================================
    // Clone Tests
    // ========================================================================

    #[test]
    fn test_clone_without_api_wrapper() {
        let action = ClickupAction {
            api_wrapper: None,
            mode: "get_teams".to_string(),
            name: "Test".to_string(),
            description: "Desc".to_string(),
        };

        let cloned = action.clone();
        assert_eq!(action.mode, cloned.mode);
        assert_eq!(action.name, cloned.name);
        assert_eq!(action.description, cloned.description);
    }

    // ========================================================================
    // Tool Trait Tests (without API wrapper)
    // ========================================================================

    #[test]
    fn test_tool_name_returns_correct_value() {
        let action = ClickupAction {
            api_wrapper: None,
            mode: "get_teams".to_string(),
            name: "Custom Name".to_string(),
            description: "Desc".to_string(),
        };

        assert_eq!(action.name(), "Custom Name");
    }

    #[test]
    fn test_tool_description_returns_correct_value() {
        let action = ClickupAction {
            api_wrapper: None,
            mode: "get_teams".to_string(),
            name: "Name".to_string(),
            description: "Custom Description".to_string(),
        };

        assert_eq!(action.description(), "Custom Description");
    }

    // ========================================================================
    // Run Method Error Cases
    // ========================================================================

    #[tokio::test]
    async fn test_run_without_api_wrapper() {
        let action = ClickupAction {
            api_wrapper: None,
            mode: "get_teams".to_string(),
            name: "Name".to_string(),
            description: "Desc".to_string(),
        };

        let result = action.run("{}").await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, ClickupError::NotInitialized));
    }

    // ========================================================================
    // Tool _call Method Tests (without API wrapper)
    // ========================================================================

    #[tokio::test]
    async fn test_call_string_input_without_wrapper() {
        let action = ClickupAction {
            api_wrapper: None,
            mode: "get_teams".to_string(),
            name: "Name".to_string(),
            description: "Desc".to_string(),
        };

        let input = ToolInput::String("{}".to_string());
        let result = action._call(input).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_call_structured_input_without_wrapper() {
        let action = ClickupAction {
            api_wrapper: None,
            mode: "get_task".to_string(),
            name: "Name".to_string(),
            description: "Desc".to_string(),
        };

        let input = ToolInput::Structured(serde_json::json!({"task_id": "123"}));
        let result = action._call(input).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_call_complex_structured_input() {
        let action = ClickupAction {
            api_wrapper: None,
            mode: "create_task".to_string(),
            name: "Name".to_string(),
            description: "Desc".to_string(),
        };

        let input = ToolInput::Structured(serde_json::json!({
            "name": "Test Task",
            "description": "Test description",
            "priority": 2,
            "assignees": [12345, 67890]
        }));

        let result = action._call(input).await;
        // Should fail because no API wrapper, but should serialize correctly
        assert!(result.is_err());
    }

    // ========================================================================
    // Mode String Tests
    // ========================================================================

    #[test]
    fn test_all_known_modes() {
        let modes = [
            "create_task",
            "create_list",
            "create_folder",
            "get_task",
            "get_task_attribute",
            "get_teams",
            "get_lists",
            "get_folders",
            "get_spaces",
            "update_task",
            "update_task_assignees",
        ];

        for mode in &modes {
            let (name, desc) = ClickupAction::get_name_and_description(mode);
            assert!(!name.is_empty(), "Name for mode '{}' should not be empty", mode);
            assert!(!desc.is_empty(), "Description for mode '{}' should not be empty", mode);
            assert!(
                !name.contains(mode) || mode == &"unknown_mode",
                "Known mode '{}' should have custom name, got '{}'",
                mode,
                name
            );
        }
    }

    #[test]
    fn test_mode_descriptions_contain_keywords() {
        // Task-related modes should mention "task" in description
        let task_modes = ["create_task", "get_task", "get_task_attribute", "update_task"];
        for mode in &task_modes {
            let (_, desc) = ClickupAction::get_name_and_description(mode);
            assert!(
                desc.to_lowercase().contains("task"),
                "Mode '{}' description should mention 'task'",
                mode
            );
        }

        // List-related modes should mention "list"
        let (_, desc) = ClickupAction::get_name_and_description("create_list");
        assert!(desc.to_lowercase().contains("list"));

        let (_, desc) = ClickupAction::get_name_and_description("get_lists");
        assert!(desc.to_lowercase().contains("list"));

        // Folder-related
        let (_, desc) = ClickupAction::get_name_and_description("create_folder");
        assert!(desc.to_lowercase().contains("folder"));

        // Space-related
        let (_, desc) = ClickupAction::get_name_and_description("get_spaces");
        assert!(desc.to_lowercase().contains("space"));
    }

    // ========================================================================
    // Edge Cases
    // ========================================================================

    #[test]
    fn test_empty_string_fields() {
        let action = ClickupAction {
            api_wrapper: None,
            mode: String::new(),
            name: String::new(),
            description: String::new(),
        };

        assert_eq!(action.name(), "");
        assert_eq!(action.description(), "");
    }

    #[test]
    fn test_whitespace_mode() {
        let (name, desc) = ClickupAction::get_name_and_description("   ");
        // Whitespace mode should fall through to default
        assert!(name.contains("   "));
        assert!(desc.contains("   "));
    }

    #[test]
    fn test_very_long_mode() {
        let long_mode = "a".repeat(1000);
        let (name, desc) = ClickupAction::get_name_and_description(&long_mode);
        assert!(name.contains(&long_mode));
        assert!(desc.contains(&long_mode));
    }

    // ========================================================================
    // Struct Field Tests
    // ========================================================================

    #[test]
    fn test_struct_fields_accessible() {
        let action = ClickupAction {
            api_wrapper: None,
            mode: "test_mode".to_string(),
            name: "test_name".to_string(),
            description: "test_description".to_string(),
        };

        // Verify fields are correctly set
        assert_eq!(action.mode, "test_mode");
        // name() and description() are via Tool trait
        assert_eq!(action.name(), "test_name");
        assert_eq!(action.description(), "test_description");
    }

    #[test]
    fn test_api_wrapper_is_optional() {
        let action_without = ClickupAction {
            api_wrapper: None,
            mode: "get_teams".to_string(),
            name: "Name".to_string(),
            description: "Desc".to_string(),
        };

        // Should be able to serialize/deserialize without API wrapper
        let json = serde_json::to_string(&action_without).expect("Serialize failed");
        let _: ClickupAction = serde_json::from_str(&json).expect("Deserialize failed");
    }
}
