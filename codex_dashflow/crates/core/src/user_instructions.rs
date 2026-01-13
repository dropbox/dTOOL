//! User instructions handling for LLM context
//!
//! This module provides types for representing user instructions (from AGENTS.md
//! or other sources) that are included in the conversation context for the LLM.

use serde::{Deserialize, Serialize};

/// Legacy XML tag for user instructions (deprecated format)
pub const USER_INSTRUCTIONS_OPEN_TAG_LEGACY: &str = "<user_instructions>";

/// Prefix for user instructions in the modern format
pub const USER_INSTRUCTIONS_PREFIX: &str = "# AGENTS.md instructions for ";

/// User instructions loaded from AGENTS.md or similar files.
///
/// These instructions are formatted and sent to the LLM as part of the conversation
/// context to provide project-specific guidance and constraints.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename = "user_instructions", rename_all = "snake_case")]
pub struct UserInstructions {
    /// Directory the instructions are associated with
    pub directory: String,
    /// The instruction text content
    pub text: String,
}

impl UserInstructions {
    /// Create new user instructions for a directory.
    pub fn new(directory: impl Into<String>, text: impl Into<String>) -> Self {
        Self {
            directory: directory.into(),
            text: text.into(),
        }
    }

    /// Check if a message content represents user instructions.
    ///
    /// This detects both the modern format (prefix) and legacy XML format.
    pub fn is_user_instructions(content: &str) -> bool {
        content.starts_with(USER_INSTRUCTIONS_PREFIX)
            || content.starts_with(USER_INSTRUCTIONS_OPEN_TAG_LEGACY)
    }

    /// Format the instructions for inclusion in conversation history.
    pub fn to_formatted_string(&self) -> String {
        format!(
            "{USER_INSTRUCTIONS_PREFIX}{directory}\n\n<INSTRUCTIONS>\n{contents}\n</INSTRUCTIONS>",
            directory = self.directory,
            contents = self.text
        )
    }
}

/// Developer instructions that are provided as system context.
///
/// These are distinct from user instructions and are typically set by
/// the system or application rather than loaded from project files.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename = "developer_instructions", rename_all = "snake_case")]
pub struct DeveloperInstructions {
    text: String,
}

impl DeveloperInstructions {
    /// Create new developer instructions.
    pub fn new(text: impl Into<String>) -> Self {
        Self { text: text.into() }
    }

    /// Get the instruction text.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Consume and return the instruction text.
    pub fn into_text(self) -> String {
        self.text
    }
}

/// Formatted message for user instructions in conversation history.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserInstructionsMessage {
    /// Role is always "user" for user instructions
    pub role: String,
    /// Formatted instruction content
    pub content: String,
}

impl From<UserInstructions> for UserInstructionsMessage {
    fn from(ui: UserInstructions) -> Self {
        Self {
            role: "user".to_string(),
            content: ui.to_formatted_string(),
        }
    }
}

/// Formatted message for developer instructions in conversation history.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeveloperInstructionsMessage {
    /// Role is "developer" for developer instructions
    pub role: String,
    /// Instruction content
    pub content: String,
}

impl From<DeveloperInstructions> for DeveloperInstructionsMessage {
    fn from(di: DeveloperInstructions) -> Self {
        Self {
            role: "developer".to_string(),
            content: di.into_text(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_user_instructions_new() {
        let ui = UserInstructions::new("test_directory", "test_text");
        assert_eq!(ui.directory, "test_directory");
        assert_eq!(ui.text, "test_text");
    }

    #[test]
    fn test_user_instructions_to_formatted_string() {
        let ui = UserInstructions::new("test_directory", "test_text");
        let formatted = ui.to_formatted_string();

        assert_eq!(
            formatted,
            "# AGENTS.md instructions for test_directory\n\n<INSTRUCTIONS>\ntest_text\n</INSTRUCTIONS>"
        );
    }

    #[test]
    fn test_user_instructions_message_conversion() {
        let ui = UserInstructions::new("test_directory", "test_text");
        let message: UserInstructionsMessage = ui.into();

        assert_eq!(message.role, "user");
        assert_eq!(
            message.content,
            "# AGENTS.md instructions for test_directory\n\n<INSTRUCTIONS>\ntest_text\n</INSTRUCTIONS>"
        );
    }

    #[test]
    fn test_is_user_instructions_modern_format() {
        assert!(UserInstructions::is_user_instructions(
            "# AGENTS.md instructions for test_directory\n\n<INSTRUCTIONS>\ntest_text\n</INSTRUCTIONS>"
        ));
    }

    #[test]
    fn test_is_user_instructions_legacy_format() {
        assert!(UserInstructions::is_user_instructions(
            "<user_instructions>test_text</user_instructions>"
        ));
    }

    #[test]
    fn test_is_user_instructions_false() {
        assert!(!UserInstructions::is_user_instructions("test_text"));
        assert!(!UserInstructions::is_user_instructions(""));
        assert!(!UserInstructions::is_user_instructions("Hello world"));
    }

    #[test]
    fn test_developer_instructions_new() {
        let di = DeveloperInstructions::new("test developer text");
        assert_eq!(di.text(), "test developer text");
    }

    #[test]
    fn test_developer_instructions_into_text() {
        let di = DeveloperInstructions::new("test developer text");
        assert_eq!(di.into_text(), "test developer text");
    }

    #[test]
    fn test_developer_instructions_message_conversion() {
        let di = DeveloperInstructions::new("test developer text");
        let message: DeveloperInstructionsMessage = di.into();

        assert_eq!(message.role, "developer");
        assert_eq!(message.content, "test developer text");
    }

    #[test]
    fn test_user_instructions_serialize() {
        let ui = UserInstructions::new("my/project", "Follow the code style guide.");
        let json = serde_json::to_string(&ui).unwrap();
        assert!(json.contains("my/project"));
        assert!(json.contains("Follow the code style guide."));
    }

    #[test]
    fn test_user_instructions_deserialize() {
        let json = r#"{"directory":"my/project","text":"Follow the code style guide."}"#;
        let ui: UserInstructions = serde_json::from_str(json).unwrap();
        assert_eq!(ui.directory, "my/project");
        assert_eq!(ui.text, "Follow the code style guide.");
    }

    #[test]
    fn test_constants() {
        assert_eq!(USER_INSTRUCTIONS_OPEN_TAG_LEGACY, "<user_instructions>");
        assert_eq!(USER_INSTRUCTIONS_PREFIX, "# AGENTS.md instructions for ");
    }
}
