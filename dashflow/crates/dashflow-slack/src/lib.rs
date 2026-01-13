//! # `DashFlow` Slack Tools
//!
//! This crate provides Slack integration tools for `DashFlow` Rust, enabling agents to interact
//! with Slack workspaces for team communication and collaboration.
//!
//! ## Features
//!
//! - **Send Messages**: Post messages to Slack channels or direct messages
//! - **Get Messages**: Retrieve conversation history from channels
//! - **Get Channels**: List and retrieve information about workspace channels
//! - **Schedule Messages**: Schedule messages for future delivery
//!
//! ## Authentication
//!
//! All tools require a Slack Bot Token with appropriate OAuth scopes:
//! - `chat:write` - Send messages
//! - `channels:history` - Read channel messages
//! - `channels:read` - List and get channel info
//! - `chat:write.public` - Send messages to channels bot isn't member of
//!
//! ## Example
//!
//! ```ignore
//! use dashflow_slack::SlackSendMessage;
//! use dashflow::core::tools::Tool;
//!
//! #[tokio::main]
//! async fn main() {
//!     let token = std::env::var("SLACK_BOT_TOKEN").unwrap();
//!     let tool = SlackSendMessage::new(token);
//!
//!     let result = tool._call_str("{\"channel\": \"#general\", \"text\": \"Hello from DashFlow!\"}").await;
//!     println!("Result: {:?}", result);
//! }
//! ```

use async_trait::async_trait;
use dashflow::core::error::{Error, Result};
use dashflow::core::tools::{Tool, ToolInput};
use serde::Deserialize;
use slack_morphism::prelude::*;

// Re-export main types
pub use slack_morphism;

/// Slack tool for sending messages to channels or users
///
/// Sends text messages to Slack channels, direct messages, or other conversation types.
/// Requires `chat:write` OAuth scope.
///
/// # Input Format
///
/// ```json
/// {
///   "channel": "#general",
///   "text": "Hello, world!"
/// }
/// ```
///
/// The channel can be:
/// - Channel name: `#general`, `#random`
/// - Channel ID: `C1234567890`
/// - User ID (for DMs): `U1234567890`
///
/// # Example
///
/// ```ignore
/// use dashflow_slack::SlackSendMessage;
/// use dashflow::core::tools::Tool;
///
/// #[tokio::main]
/// async fn main() {
///     let token = std::env::var("SLACK_BOT_TOKEN").unwrap();
///     let tool = SlackSendMessage::new(token);
///
///     let result = tool._call_str(r#"{"channel": "#general", "text": "Hello!"}"#.to_string()).await;
///     println!("Message sent: {:?}", result);
/// }
/// ```
pub struct SlackSendMessage {
    token: String,
}

#[derive(Debug, Deserialize)]
struct SendMessageInput {
    channel: String,
    text: String,
}

impl SlackSendMessage {
    /// Create a new `SlackSendMessage` tool
    ///
    /// # Arguments
    /// * `token` - Slack Bot Token (xoxb-...)
    pub fn new(token: impl Into<String>) -> Self {
        Self {
            token: token.into(),
        }
    }
}

#[async_trait]
impl Tool for SlackSendMessage {
    fn name(&self) -> &'static str {
        "slack_send_message"
    }

    fn description(&self) -> &'static str {
        "Send a message to a Slack channel or direct message. \
         Input should be a JSON object with 'channel' (channel name like '#general' or channel ID) \
         and 'text' (message text) fields."
    }

    async fn _call(&self, input: ToolInput) -> Result<String> {
        let input_str = match input {
            ToolInput::String(s) => s,
            ToolInput::Structured(v) => serde_json::to_string(&v)
                .map_err(|e| Error::tool_error(format!("Failed to serialize input: {e}")))?,
        };

        let parsed: SendMessageInput = serde_json::from_str(&input_str)
            .map_err(|e| Error::tool_error(format!("Invalid input JSON: {e}. Expected {{\"channel\": \"#general\", \"text\": \"message\"}}")))?;

        let connector = SlackClientHyperConnector::new()
            .map_err(|e| Error::tool_error(format!("Failed to create Slack HTTP client: {e}")))?;
        let client = SlackClient::new(connector);
        let token = SlackApiToken::new(self.token.clone().into());
        let session = client.open_session(&token);

        // Parse channel identifier
        let channel_id = if parsed.channel.starts_with('#') {
            // Channel name - need to resolve to ID
            // For simplicity, we'll try to use it directly and let Slack API handle it
            SlackChannelId::new(parsed.channel.trim_start_matches('#').to_string())
        } else {
            SlackChannelId::new(parsed.channel.clone())
        };

        let post_chat_req = SlackApiChatPostMessageRequest::new(
            channel_id,
            SlackMessageContent::new().with_text(parsed.text),
        );

        session
            .chat_post_message(&post_chat_req)
            .await
            .map_err(|e| Error::tool_error(format!("Failed to send message: {e}")))?;

        Ok(format!("Message sent successfully to {}", parsed.channel))
    }
}

/// Slack tool for retrieving messages from a channel
///
/// Fetches conversation history from Slack channels. Requires `channels:history` OAuth scope.
///
/// # Input Format
///
/// ```json
/// {
///   "channel": "#general",
///   "limit": 10
/// }
/// ```
///
/// - `channel`: Channel name (e.g., `#general`) or ID (e.g., `C1234567890`)
/// - `limit`: Maximum number of messages to retrieve (default: 10, max: 100)
///
/// # Example
///
/// ```ignore
/// use dashflow_slack::SlackGetMessage;
/// use dashflow::core::tools::Tool;
///
/// #[tokio::main]
/// async fn main() {
///     let token = std::env::var("SLACK_BOT_TOKEN").unwrap();
///     let tool = SlackGetMessage::new(token);
///
///     let result = tool._call_str(r#"{"channel": "#general", "limit": 5}"#.to_string()).await;
///     println!("Messages: {:?}", result);
/// }
/// ```
pub struct SlackGetMessage {
    token: String,
}

#[derive(Debug, Deserialize)]
struct GetMessageInput {
    channel: String,
    #[serde(default = "default_limit")]
    limit: u16,
}

fn default_limit() -> u16 {
    10
}

impl SlackGetMessage {
    /// Create a new `SlackGetMessage` tool
    ///
    /// # Arguments
    /// * `token` - Slack Bot Token (xoxb-...)
    pub fn new(token: impl Into<String>) -> Self {
        Self {
            token: token.into(),
        }
    }
}

#[async_trait]
impl Tool for SlackGetMessage {
    fn name(&self) -> &'static str {
        "slack_get_message"
    }

    fn description(&self) -> &'static str {
        "Get messages from a Slack channel. \
         Input should be a JSON object with 'channel' (channel name or ID) \
         and optional 'limit' (number of messages, default 10, max 100)."
    }

    async fn _call(&self, input: ToolInput) -> Result<String> {
        let input_str = match input {
            ToolInput::String(s) => s,
            ToolInput::Structured(v) => serde_json::to_string(&v)
                .map_err(|e| Error::tool_error(format!("Failed to serialize input: {e}")))?,
        };

        let parsed: GetMessageInput = serde_json::from_str(&input_str).map_err(|e| {
            Error::tool_error(format!(
                "Invalid input JSON: {e}. Expected {{\"channel\": \"#general\", \"limit\": 10}}"
            ))
        })?;

        let connector = SlackClientHyperConnector::new()
            .map_err(|e| Error::tool_error(format!("Failed to create Slack HTTP client: {e}")))?;
        let client = SlackClient::new(connector);
        let token = SlackApiToken::new(self.token.clone().into());
        let session = client.open_session(&token);

        let channel_id = if parsed.channel.starts_with('#') {
            SlackChannelId::new(parsed.channel.trim_start_matches('#').to_string())
        } else {
            SlackChannelId::new(parsed.channel.clone())
        };

        let history_req = SlackApiConversationsHistoryRequest::new()
            .with_channel(channel_id)
            .with_limit(parsed.limit.min(100));

        let response = session
            .conversations_history(&history_req)
            .await
            .map_err(|e| Error::tool_error(format!("Failed to fetch messages: {e}")))?;

        let messages: Vec<String> = response
            .messages
            .iter()
            .filter_map(|msg| {
                msg.content.text.as_ref().map(|text| {
                    // Use channel as identifier since user field doesn't exist on origin
                    let channel_id = msg
                        .origin
                        .channel
                        .as_ref()
                        .map_or_else(|| "Unknown".to_string(), std::string::ToString::to_string);
                    format!("[{channel_id}]: {text}")
                })
            })
            .collect();

        if messages.is_empty() {
            Ok("No messages found".to_string())
        } else {
            Ok(messages.join("\n"))
        }
    }
}

/// Slack tool for listing and getting channel information
///
/// Retrieves information about workspace channels. Requires `channels:read` OAuth scope.
///
/// # Input Format
///
/// ```json
/// {
///   "limit": 20
/// }
/// ```
///
/// Or to get info about a specific channel:
///
/// ```json
/// {
///   "channel": "#general"
/// }
/// ```
///
/// # Example
///
/// ```ignore
/// use dashflow_slack::SlackGetChannel;
/// use dashflow::core::tools::Tool;
///
/// #[tokio::main]
/// async fn main() {
///     let token = std::env::var("SLACK_BOT_TOKEN").unwrap();
///     let tool = SlackGetChannel::new(token);
///
///     // List channels
///     let result = tool._call_str(r#"{"limit": 10}"#.to_string()).await;
///     println!("Channels: {:?}", result);
/// }
/// ```
pub struct SlackGetChannel {
    token: String,
}

#[derive(Debug, Deserialize)]
struct GetChannelInput {
    #[serde(default)]
    channel: Option<String>,
    #[serde(default = "default_channel_limit")]
    limit: u16,
}

fn default_channel_limit() -> u16 {
    20
}

impl SlackGetChannel {
    /// Create a new `SlackGetChannel` tool
    ///
    /// # Arguments
    /// * `token` - Slack Bot Token (xoxb-...)
    pub fn new(token: impl Into<String>) -> Self {
        Self {
            token: token.into(),
        }
    }
}

#[async_trait]
impl Tool for SlackGetChannel {
    fn name(&self) -> &'static str {
        "slack_get_channel"
    }

    fn description(&self) -> &'static str {
        "Get information about Slack channels. \
         Input can be a JSON object with optional 'channel' (specific channel name or ID) \
         or 'limit' (number of channels to list, default 20)."
    }

    async fn _call(&self, input: ToolInput) -> Result<String> {
        let input_str = match input {
            ToolInput::String(s) => s,
            ToolInput::Structured(v) => serde_json::to_string(&v)
                .map_err(|e| Error::tool_error(format!("Failed to serialize input: {e}")))?,
        };

        let parsed: GetChannelInput = serde_json::from_str(&input_str)
            .map_err(|e| Error::tool_error(format!("Invalid input JSON: {e}. Expected {{\"limit\": 20}} or {{\"channel\": \"#general\"}}")))?;

        let connector = SlackClientHyperConnector::new()
            .map_err(|e| Error::tool_error(format!("Failed to create Slack HTTP client: {e}")))?;
        let client = SlackClient::new(connector);
        let token = SlackApiToken::new(self.token.clone().into());
        let session = client.open_session(&token);

        if let Some(channel_name) = parsed.channel {
            // Get specific channel info
            let channel_id = if channel_name.starts_with('#') {
                SlackChannelId::new(channel_name.trim_start_matches('#').to_string())
            } else {
                SlackChannelId::new(channel_name.clone())
            };

            let info_req = SlackApiConversationsInfoRequest::new(channel_id);
            let response = session
                .conversations_info(&info_req)
                .await
                .map_err(|e| Error::tool_error(format!("Failed to get channel info: {e}")))?;

            let channel = response.channel;
            let name = channel
                .name
                .clone()
                .unwrap_or_else(|| "Unnamed".to_string());
            let id = channel.id.to_string();
            let members = channel.num_members.unwrap_or(0);

            Ok(format!("Channel: {name} (ID: {id}, Members: {members})"))
        } else {
            // List channels
            let list_req =
                SlackApiConversationsListRequest::new().with_limit(parsed.limit.min(200));

            let response = session
                .conversations_list(&list_req)
                .await
                .map_err(|e| Error::tool_error(format!("Failed to list channels: {e}")))?;

            let channels: Vec<String> = response
                .channels
                .iter()
                .filter_map(|channel| {
                    channel.name.as_ref().map(|name| {
                        let id = channel.id.to_string();
                        let members = channel.num_members.unwrap_or(0);
                        format!("#{name} (ID: {id}, Members: {members})")
                    })
                })
                .collect();

            if channels.is_empty() {
                Ok("No channels found".to_string())
            } else {
                Ok(channels.join("\n"))
            }
        }
    }
}

/// Slack tool for scheduling messages for future delivery
///
/// Schedules messages to be sent at a specific time in the future.
/// Requires `chat:write` OAuth scope.
///
/// # Input Format
///
/// ```json
/// {
///   "channel": "#general",
///   "text": "Scheduled message",
///   "post_at": 1609459200
/// }
/// ```
///
/// - `channel`: Channel name or ID
/// - `text`: Message text
/// - `post_at`: Unix timestamp (seconds since epoch) when message should be posted
///
/// # Example
///
/// ```ignore
/// use dashflow_slack::SlackScheduleMessage;
/// use dashflow::core::tools::Tool;
/// use chrono::{Utc, Duration};
///
/// #[tokio::main]
/// async fn main() {
///     let token = std::env::var("SLACK_BOT_TOKEN").unwrap();
///     let tool = SlackScheduleMessage::new(token);
///
///     // Schedule message for 1 hour from now
///     let post_at = (Utc::now() + Duration::hours(1)).timestamp();
///     let input = format!(r#"{{"channel": "#general", "text": "Reminder!", "post_at": {}}}"#, post_at);
///
///     let result = tool._call_str(input).await;
///     println!("Message scheduled: {:?}", result);
/// }
/// ```
pub struct SlackScheduleMessage {
    token: String,
}

#[derive(Debug, Deserialize)]
struct ScheduleMessageInput {
    channel: String,
    text: String,
    post_at: i64, // Unix timestamp
}

impl SlackScheduleMessage {
    /// Create a new `SlackScheduleMessage` tool
    ///
    /// # Arguments
    /// * `token` - Slack Bot Token (xoxb-...)
    pub fn new(token: impl Into<String>) -> Self {
        Self {
            token: token.into(),
        }
    }
}

#[async_trait]
impl Tool for SlackScheduleMessage {
    fn name(&self) -> &'static str {
        "slack_schedule_message"
    }

    fn description(&self) -> &'static str {
        "Schedule a message to be sent to a Slack channel at a future time. \
         Input should be a JSON object with 'channel', 'text', and 'post_at' (Unix timestamp) fields."
    }

    async fn _call(&self, input: ToolInput) -> Result<String> {
        let input_str = match input {
            ToolInput::String(s) => s,
            ToolInput::Structured(v) => serde_json::to_string(&v)
                .map_err(|e| Error::tool_error(format!("Failed to serialize input: {e}")))?,
        };

        let parsed: ScheduleMessageInput = serde_json::from_str(&input_str)
            .map_err(|e| Error::tool_error(format!("Invalid input JSON: {e}. Expected {{\"channel\": \"#general\", \"text\": \"message\", \"post_at\": 1609459200}}")))?;

        let connector = SlackClientHyperConnector::new()
            .map_err(|e| Error::tool_error(format!("Failed to create Slack HTTP client: {e}")))?;
        let client = SlackClient::new(connector);
        let token = SlackApiToken::new(self.token.clone().into());
        let session = client.open_session(&token);

        let channel_id = if parsed.channel.starts_with('#') {
            SlackChannelId::new(parsed.channel.trim_start_matches('#').to_string())
        } else {
            SlackChannelId::new(parsed.channel.clone())
        };

        // Convert Unix timestamp to DateTime<Utc>
        let post_at_datetime = chrono::DateTime::from_timestamp(parsed.post_at, 0)
            .ok_or_else(|| Error::tool_error("Invalid timestamp"))?;
        let post_at_slack = SlackDateTime::from(post_at_datetime);

        let schedule_req = SlackApiChatScheduleMessageRequest::new(
            channel_id,
            SlackMessageContent::new().with_text(parsed.text),
            post_at_slack,
        );

        let response = session
            .chat_schedule_message(&schedule_req)
            .await
            .map_err(|e| Error::tool_error(format!("Failed to schedule message: {e}")))?;

        let scheduled_message_id = response.scheduled_message_id.to_string();

        Ok(format!(
            "Message scheduled successfully for {} (ID: {})",
            parsed.channel, scheduled_message_id
        ))
    }
}

#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
#[cfg(test)]
mod tests {
    use super::*;

    // ============ Mock/Unit Tests (no Slack token required) ============

    #[test]
    fn test_send_message_tool_name() {
        let tool = SlackSendMessage::new("xoxb-test-token");
        assert_eq!(tool.name(), "slack_send_message");
    }

    #[test]
    fn test_send_message_description() {
        let tool = SlackSendMessage::new("xoxb-test-token");
        assert!(tool.description().contains("Send a message"));
    }

    #[test]
    fn test_get_message_tool_name() {
        let tool = SlackGetMessage::new("xoxb-test-token");
        assert_eq!(tool.name(), "slack_get_message");
    }

    #[test]
    fn test_get_message_description() {
        let tool = SlackGetMessage::new("xoxb-test-token");
        assert!(tool.description().contains("Get messages"));
    }

    #[test]
    fn test_get_channel_tool_name() {
        let tool = SlackGetChannel::new("xoxb-test-token");
        assert_eq!(tool.name(), "slack_get_channel");
    }

    #[test]
    fn test_get_channel_description() {
        let tool = SlackGetChannel::new("xoxb-test-token");
        assert!(tool
            .description()
            .contains("information about Slack channels"));
    }

    #[test]
    fn test_schedule_message_tool_name() {
        let tool = SlackScheduleMessage::new("xoxb-test-token");
        assert_eq!(tool.name(), "slack_schedule_message");
    }

    #[test]
    fn test_schedule_message_description() {
        let tool = SlackScheduleMessage::new("xoxb-test-token");
        assert!(tool.description().contains("Schedule a message"));
    }

    #[test]
    fn test_send_message_input_parsing() {
        // Test parsing SendMessageInput
        let json = r##"{"channel": "#general", "text": "Hello, world!"}"##;
        let parsed: SendMessageInput = serde_json::from_str(json).unwrap();
        assert_eq!(parsed.channel, "#general");
        assert_eq!(parsed.text, "Hello, world!");
    }

    #[test]
    fn test_send_message_input_with_channel_id() {
        // Test parsing with channel ID instead of name
        let json = r#"{"channel": "C1234567890", "text": "Test message"}"#;
        let parsed: SendMessageInput = serde_json::from_str(json).unwrap();
        assert_eq!(parsed.channel, "C1234567890");
        assert_eq!(parsed.text, "Test message");
    }

    #[test]
    fn test_get_message_input_parsing() {
        // Test parsing GetMessageInput with default limit
        let json = r##"{"channel": "#random"}"##;
        let parsed: GetMessageInput = serde_json::from_str(json).unwrap();
        assert_eq!(parsed.channel, "#random");
        assert_eq!(parsed.limit, 10); // default_limit()
    }

    #[test]
    fn test_get_message_input_with_limit() {
        // Test parsing GetMessageInput with explicit limit
        let json = r##"{"channel": "#general", "limit": 50}"##;
        let parsed: GetMessageInput = serde_json::from_str(json).unwrap();
        assert_eq!(parsed.channel, "#general");
        assert_eq!(parsed.limit, 50);
    }

    #[test]
    fn test_get_channel_input_list_mode() {
        // Test parsing GetChannelInput for listing channels
        let json = r#"{"limit": 25}"#;
        let parsed: GetChannelInput = serde_json::from_str(json).unwrap();
        assert!(parsed.channel.is_none());
        assert_eq!(parsed.limit, 25);
    }

    #[test]
    fn test_get_channel_input_specific_channel() {
        // Test parsing GetChannelInput for specific channel
        let json = r##"{"channel": "#engineering"}"##;
        let parsed: GetChannelInput = serde_json::from_str(json).unwrap();
        assert_eq!(parsed.channel, Some("#engineering".to_string()));
        assert_eq!(parsed.limit, 20); // default_channel_limit()
    }

    #[test]
    fn test_schedule_message_input_parsing() {
        // Test parsing ScheduleMessageInput
        let json = r##"{"channel": "#general", "text": "Reminder!", "post_at": 1700000000}"##;
        let parsed: ScheduleMessageInput = serde_json::from_str(json).unwrap();
        assert_eq!(parsed.channel, "#general");
        assert_eq!(parsed.text, "Reminder!");
        assert_eq!(parsed.post_at, 1700000000);
    }

    #[test]
    fn test_channel_name_stripping() {
        // Test that channel names with # prefix get stripped correctly
        let channel_with_hash = "#general";
        let channel_without_hash = "general";

        let stripped = channel_with_hash.trim_start_matches('#');
        assert_eq!(stripped, "general");

        let already_stripped = channel_without_hash.trim_start_matches('#');
        assert_eq!(already_stripped, "general");
    }

    #[test]
    fn test_channel_id_detection() {
        // Test channel ID vs name detection
        let channel_name = "#general";
        let channel_id = "C1234567890";

        assert!(channel_name.starts_with('#'));
        assert!(!channel_id.starts_with('#'));
    }

    #[test]
    fn test_default_limit() {
        assert_eq!(default_limit(), 10);
    }

    #[test]
    fn test_default_channel_limit() {
        assert_eq!(default_channel_limit(), 20);
    }

    #[test]
    fn test_tool_creation_with_different_token_types() {
        // Test tool creation with string literal
        let tool1 = SlackSendMessage::new("xoxb-token-123");
        assert_eq!(tool1.token, "xoxb-token-123");

        // Test tool creation with String
        let token = String::from("xoxb-another-token");
        let tool2 = SlackSendMessage::new(token);
        assert_eq!(tool2.token, "xoxb-another-token");
    }

    #[test]
    fn test_invalid_send_message_json() {
        // Test that invalid JSON fails to parse
        let invalid_json = r##"{"channel": "#general"}"##; // Missing text field
        let result: std::result::Result<SendMessageInput, _> = serde_json::from_str(invalid_json);
        assert!(result.is_err());
    }

    #[test]
    fn test_invalid_schedule_message_json() {
        // Test that invalid JSON fails to parse
        let invalid_json = r##"{"channel": "#general", "text": "Hello"}"##; // Missing post_at
        let result: std::result::Result<ScheduleMessageInput, _> =
            serde_json::from_str(invalid_json);
        assert!(result.is_err());
    }

    #[test]
    fn test_message_limit_clamping() {
        // Test that limits over 100 get clamped
        let limit: u16 = 150;
        let clamped = limit.min(100);
        assert_eq!(clamped, 100);

        let limit_within: u16 = 50;
        let not_clamped = limit_within.min(100);
        assert_eq!(not_clamped, 50);
    }

    #[test]
    fn test_channel_limit_clamping() {
        // Test that channel limits over 200 get clamped
        let limit: u16 = 300;
        let clamped = limit.min(200);
        assert_eq!(clamped, 200);

        let limit_within: u16 = 150;
        let not_clamped = limit_within.min(200);
        assert_eq!(not_clamped, 150);
    }

    // ============ Additional Unit Tests ============

    #[test]
    fn test_send_message_input_debug() {
        let input = SendMessageInput {
            channel: "#test".to_string(),
            text: "Hello".to_string(),
        };
        let debug_str = format!("{:?}", input);
        assert!(debug_str.contains("SendMessageInput"));
        assert!(debug_str.contains("#test"));
        assert!(debug_str.contains("Hello"));
    }

    #[test]
    fn test_get_message_input_debug() {
        let input = GetMessageInput {
            channel: "#random".to_string(),
            limit: 25,
        };
        let debug_str = format!("{:?}", input);
        assert!(debug_str.contains("GetMessageInput"));
        assert!(debug_str.contains("#random"));
        assert!(debug_str.contains("25"));
    }

    #[test]
    fn test_get_channel_input_debug() {
        let input = GetChannelInput {
            channel: Some("#engineering".to_string()),
            limit: 15,
        };
        let debug_str = format!("{:?}", input);
        assert!(debug_str.contains("GetChannelInput"));
        assert!(debug_str.contains("engineering"));
    }

    #[test]
    fn test_schedule_message_input_debug() {
        let input = ScheduleMessageInput {
            channel: "#general".to_string(),
            text: "Reminder".to_string(),
            post_at: 1700000000,
        };
        let debug_str = format!("{:?}", input);
        assert!(debug_str.contains("ScheduleMessageInput"));
        assert!(debug_str.contains("1700000000"));
    }

    #[test]
    fn test_send_message_input_unicode() {
        let json = "{\"channel\": \"#日本語\", \"text\": \"こんにちは世界\"}";
        let parsed: SendMessageInput = serde_json::from_str(json).unwrap();
        assert_eq!(parsed.channel, "#日本語");
        assert!(parsed.text.contains("こんにちは"));
    }

    #[test]
    fn test_send_message_input_empty_text() {
        let json = "{\"channel\": \"#general\", \"text\": \"\"}";
        let parsed: SendMessageInput = serde_json::from_str(json).unwrap();
        assert_eq!(parsed.channel, "#general");
        assert_eq!(parsed.text, "");
    }

    #[test]
    fn test_send_message_input_whitespace_text() {
        let json = "{\"channel\": \"#general\", \"text\": \"   \"}";
        let parsed: SendMessageInput = serde_json::from_str(json).unwrap();
        assert_eq!(parsed.text, "   ");
    }

    #[test]
    fn test_send_message_input_newlines() {
        let json = "{\"channel\": \"#general\", \"text\": \"Line 1\\nLine 2\\nLine 3\"}";
        let parsed: SendMessageInput = serde_json::from_str(json).unwrap();
        assert!(parsed.text.contains('\n'));
        assert_eq!(parsed.text.lines().count(), 3);
    }

    #[test]
    fn test_send_message_input_special_chars() {
        let json = "{\"channel\": \"#general\", \"text\": \"Hello & <script>test</script>\"}";
        let parsed: SendMessageInput = serde_json::from_str(json).unwrap();
        assert!(parsed.text.contains("<script>"));
    }

    #[test]
    fn test_send_message_input_long_text() {
        let long_text = "a".repeat(10000);
        let json = format!("{{\"channel\": \"#general\", \"text\": \"{}\"}}", long_text);
        let parsed: SendMessageInput = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.text.len(), 10000);
    }

    #[test]
    fn test_get_message_input_zero_limit() {
        let json = "{\"channel\": \"#general\", \"limit\": 0}";
        let parsed: GetMessageInput = serde_json::from_str(json).unwrap();
        assert_eq!(parsed.limit, 0);
    }

    #[test]
    fn test_get_message_input_max_limit() {
        let json = "{\"channel\": \"#general\", \"limit\": 65535}";
        let parsed: GetMessageInput = serde_json::from_str(json).unwrap();
        assert_eq!(parsed.limit, 65535);
    }

    #[test]
    fn test_get_channel_input_empty_defaults() {
        let json = "{}";
        let parsed: GetChannelInput = serde_json::from_str(json).unwrap();
        assert!(parsed.channel.is_none());
        assert_eq!(parsed.limit, 20);
    }

    #[test]
    fn test_get_channel_input_channel_only() {
        let json = r#"{"channel": "C1234567890"}"#;
        let parsed: GetChannelInput = serde_json::from_str(json).unwrap();
        assert_eq!(parsed.channel, Some("C1234567890".to_string()));
        assert_eq!(parsed.limit, 20);
    }

    #[test]
    fn test_get_channel_input_limit_only() {
        let json = r#"{"limit": 50}"#;
        let parsed: GetChannelInput = serde_json::from_str(json).unwrap();
        assert!(parsed.channel.is_none());
        assert_eq!(parsed.limit, 50);
    }

    #[test]
    fn test_schedule_message_input_negative_timestamp() {
        let json = "{\"channel\": \"#general\", \"text\": \"Historical\", \"post_at\": -86400}";
        let parsed: ScheduleMessageInput = serde_json::from_str(json).unwrap();
        assert_eq!(parsed.post_at, -86400);
    }

    #[test]
    fn test_schedule_message_input_far_future_timestamp() {
        let json = "{\"channel\": \"#general\", \"text\": \"Future\", \"post_at\": 4102444800}";
        let parsed: ScheduleMessageInput = serde_json::from_str(json).unwrap();
        assert_eq!(parsed.post_at, 4102444800);
    }

    #[test]
    fn test_schedule_message_input_zero_timestamp() {
        let json = "{\"channel\": \"#general\", \"text\": \"Epoch\", \"post_at\": 0}";
        let parsed: ScheduleMessageInput = serde_json::from_str(json).unwrap();
        assert_eq!(parsed.post_at, 0);
    }

    #[test]
    fn test_channel_stripping_multiple_hashes() {
        let channel = "##general";
        let stripped = channel.trim_start_matches('#');
        assert_eq!(stripped, "general");
    }

    #[test]
    fn test_channel_stripping_no_hash() {
        let channel = "general";
        let stripped = channel.trim_start_matches('#');
        assert_eq!(stripped, "general");
    }

    #[test]
    fn test_channel_stripping_empty_after_hash() {
        let channel = "#";
        let stripped = channel.trim_start_matches('#');
        assert_eq!(stripped, "");
    }

    #[test]
    fn test_user_id_detection() {
        let user_id = "U1234567890";
        assert!(user_id.starts_with('U'));
        assert!(!user_id.starts_with('#'));
        assert_eq!(user_id.len(), 11);
    }

    #[test]
    fn test_channel_id_format() {
        let channel_id = "C0123456789";
        assert!(channel_id.starts_with('C'));
        assert_eq!(channel_id.len(), 11);
    }

    #[test]
    fn test_group_id_format() {
        let group_id = "G0123456789";
        assert!(group_id.starts_with('G'));
    }

    #[test]
    fn test_dm_id_format() {
        let dm_id = "D0123456789";
        assert!(dm_id.starts_with('D'));
    }

    #[test]
    fn test_send_message_input_extra_fields_ignored() {
        let json = "{\"channel\": \"#general\", \"text\": \"Hello\", \"extra\": \"ignored\"}";
        let parsed: SendMessageInput = serde_json::from_str(json).unwrap();
        assert_eq!(parsed.channel, "#general");
        assert_eq!(parsed.text, "Hello");
    }

    #[test]
    fn test_get_message_input_extra_fields_ignored() {
        let json = "{\"channel\": \"#general\", \"limit\": 5, \"unused\": true}";
        let parsed: GetMessageInput = serde_json::from_str(json).unwrap();
        assert_eq!(parsed.channel, "#general");
        assert_eq!(parsed.limit, 5);
    }

    #[test]
    fn test_get_channel_input_extra_fields_ignored() {
        let json = "{\"channel\": \"#general\", \"limit\": 10, \"extra\": null}";
        let parsed: GetChannelInput = serde_json::from_str(json).unwrap();
        assert_eq!(parsed.channel, Some("#general".to_string()));
        assert_eq!(parsed.limit, 10);
    }

    #[test]
    fn test_schedule_message_input_extra_fields_ignored() {
        let json = "{\"channel\": \"#general\", \"text\": \"Hi\", \"post_at\": 1700000000, \"blocks\": []}";
        let parsed: ScheduleMessageInput = serde_json::from_str(json).unwrap();
        assert_eq!(parsed.channel, "#general");
        assert_eq!(parsed.post_at, 1700000000);
    }

    #[test]
    fn test_send_message_invalid_json_missing_channel() {
        let json = r#"{"text": "Hello"}"#;
        let result: std::result::Result<SendMessageInput, _> = serde_json::from_str(json);
        assert!(result.is_err());
    }

    #[test]
    fn test_send_message_invalid_json_wrong_type_channel() {
        let json = r#"{"channel": 123, "text": "Hello"}"#;
        let result: std::result::Result<SendMessageInput, _> = serde_json::from_str(json);
        assert!(result.is_err());
    }

    #[test]
    fn test_send_message_invalid_json_wrong_type_text() {
        let json = "{\"channel\": \"#general\", \"text\": true}";
        let result: std::result::Result<SendMessageInput, _> = serde_json::from_str(json);
        assert!(result.is_err());
    }

    #[test]
    fn test_get_message_invalid_json_wrong_type_limit() {
        let json = "{\"channel\": \"#general\", \"limit\": \"ten\"}";
        let result: std::result::Result<GetMessageInput, _> = serde_json::from_str(json);
        assert!(result.is_err());
    }

    #[test]
    fn test_get_message_invalid_json_negative_limit() {
        let json = "{\"channel\": \"#general\", \"limit\": -5}";
        let result: std::result::Result<GetMessageInput, _> = serde_json::from_str(json);
        assert!(result.is_err());
    }

    #[test]
    fn test_schedule_message_invalid_json_wrong_type_post_at() {
        let json = "{\"channel\": \"#general\", \"text\": \"Hi\", \"post_at\": \"tomorrow\"}";
        let result: std::result::Result<ScheduleMessageInput, _> = serde_json::from_str(json);
        assert!(result.is_err());
    }

    #[test]
    fn test_all_tools_different_names() {
        let send = SlackSendMessage::new("token");
        let get_msg = SlackGetMessage::new("token");
        let get_channel = SlackGetChannel::new("token");
        let schedule = SlackScheduleMessage::new("token");

        let names = vec![
            send.name(),
            get_msg.name(),
            get_channel.name(),
            schedule.name(),
        ];

        let unique_count = {
            let mut v = names.clone();
            v.sort();
            v.dedup();
            v.len()
        };
        assert_eq!(unique_count, 4);
    }

    #[test]
    fn test_all_tools_have_descriptions() {
        let send = SlackSendMessage::new("token");
        let get_msg = SlackGetMessage::new("token");
        let get_channel = SlackGetChannel::new("token");
        let schedule = SlackScheduleMessage::new("token");

        assert!(!send.description().is_empty());
        assert!(!get_msg.description().is_empty());
        assert!(!get_channel.description().is_empty());
        assert!(!schedule.description().is_empty());

        assert_ne!(send.description(), get_msg.description());
        assert_ne!(send.description(), get_channel.description());
        assert_ne!(send.description(), schedule.description());
    }

    #[test]
    fn test_tool_creation_empty_token() {
        let tool = SlackSendMessage::new("");
        assert_eq!(tool.token, "");
    }

    #[test]
    fn test_tool_creation_whitespace_token() {
        let tool = SlackSendMessage::new("   ");
        assert_eq!(tool.token, "   ");
    }

    #[test]
    fn test_tool_creation_from_string_ref() {
        let token_string = String::from("xoxb-test");
        let tool = SlackSendMessage::new(&token_string);
        assert_eq!(tool.token, "xoxb-test");
    }

    #[test]
    fn test_limit_clamping_boundary_100() {
        let exactly_100: u16 = 100;
        assert_eq!(exactly_100.min(100), 100);

        let just_over: u16 = 101;
        assert_eq!(just_over.min(100), 100);

        let just_under: u16 = 99;
        assert_eq!(just_under.min(100), 99);
    }

    #[test]
    fn test_limit_clamping_boundary_200() {
        let exactly_200: u16 = 200;
        assert_eq!(exactly_200.min(200), 200);

        let just_over: u16 = 201;
        assert_eq!(just_over.min(200), 200);

        let just_under: u16 = 199;
        assert_eq!(just_under.min(200), 199);
    }

    #[test]
    fn test_send_message_input_markdown_text() {
        let json = "{\"channel\": \"#general\", \"text\": \"*bold* _italic_ `code`\"}";
        let parsed: SendMessageInput = serde_json::from_str(json).unwrap();
        assert!(parsed.text.contains("*bold*"));
        assert!(parsed.text.contains("_italic_"));
        assert!(parsed.text.contains("`code`"));
    }

    #[test]
    fn test_send_message_input_mention_format() {
        let json = "{\"channel\": \"#general\", \"text\": \"Hey <@U12345> check this\"}";
        let parsed: SendMessageInput = serde_json::from_str(json).unwrap();
        assert!(parsed.text.contains("<@U12345>"));
    }

    #[test]
    fn test_send_message_input_channel_mention() {
        let json = "{\"channel\": \"#general\", \"text\": \"See <#C12345|other>\"}";
        let parsed: SendMessageInput = serde_json::from_str(json).unwrap();
        assert!(parsed.text.contains("<#C12345"));
    }

    #[test]
    fn test_send_message_input_link_format() {
        let json = "{\"channel\": \"#general\", \"text\": \"<https://example.com|link>\"}";
        let parsed: SendMessageInput = serde_json::from_str(json).unwrap();
        assert!(parsed.text.contains("https://example.com"));
    }

    #[test]
    fn test_send_message_input_emoji() {
        let json = "{\"channel\": \"#general\", \"text\": \":wave: Hello :smile:\"}";
        let parsed: SendMessageInput = serde_json::from_str(json).unwrap();
        assert!(parsed.text.contains(":wave:"));
        assert!(parsed.text.contains(":smile:"));
    }

    #[test]
    fn test_json_roundtrip_send_message() {
        let input = SendMessageInput {
            channel: "#test".to_string(),
            text: "Hello world".to_string(),
        };
        let json = "{\"channel\":\"#test\",\"text\":\"Hello world\"}";
        let parsed: SendMessageInput = serde_json::from_str(json).unwrap();
        assert_eq!(parsed.channel, input.channel);
        assert_eq!(parsed.text, input.text);
    }

    #[test]
    fn test_json_field_order_independent() {
        let json1 = "{\"channel\": \"#general\", \"text\": \"Hello\"}";
        let json2 = "{\"text\": \"Hello\", \"channel\": \"#general\"}";

        let parsed1: SendMessageInput = serde_json::from_str(json1).unwrap();
        let parsed2: SendMessageInput = serde_json::from_str(json2).unwrap();

        assert_eq!(parsed1.channel, parsed2.channel);
        assert_eq!(parsed1.text, parsed2.text);
    }

    #[test]
    fn test_json_whitespace_handling() {
        let json_compact = "{\"channel\":\"#general\",\"text\":\"Hello\"}";
        let json_pretty = "{\n  \"channel\": \"#general\",\n  \"text\": \"Hello\"\n}";

        let parsed1: SendMessageInput = serde_json::from_str(json_compact).unwrap();
        let parsed2: SendMessageInput = serde_json::from_str(json_pretty).unwrap();

        assert_eq!(parsed1.channel, parsed2.channel);
        assert_eq!(parsed1.text, parsed2.text);
    }

    #[test]
    fn test_get_message_input_default_vs_explicit() {
        let json_default = "{\"channel\": \"#general\"}";
        let json_explicit = "{\"channel\": \"#general\", \"limit\": 10}";

        let parsed1: GetMessageInput = serde_json::from_str(json_default).unwrap();
        let parsed2: GetMessageInput = serde_json::from_str(json_explicit).unwrap();

        assert_eq!(parsed1.limit, parsed2.limit);
    }

    #[test]
    fn test_get_channel_input_default_vs_explicit() {
        let json_default = "{}";
        let json_explicit = "{\"limit\": 20}";

        let parsed1: GetChannelInput = serde_json::from_str(json_default).unwrap();
        let parsed2: GetChannelInput = serde_json::from_str(json_explicit).unwrap();

        assert_eq!(parsed1.limit, parsed2.limit);
        assert_eq!(parsed1.channel, parsed2.channel);
    }

    // ============ Integration Tests (require real Slack token) ============

    #[tokio::test]
    #[ignore = "requires SLACK_BOT_TOKEN"]
    async fn test_send_message_integration() {
        let token = std::env::var("SLACK_BOT_TOKEN").expect("SLACK_BOT_TOKEN must be set");
        let channel = std::env::var("SLACK_TEST_CHANNEL").unwrap_or_else(|_| "#test".to_string());
        let tool = SlackSendMessage::new(token);

        let result = tool
            ._call_str(format!(
                r##"{{"channel": "{}", "text": "Test message from dashflow-slack"}}"##,
                channel
            ))
            .await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    #[ignore = "requires SLACK_BOT_TOKEN"]
    async fn test_get_message_integration() {
        let token = std::env::var("SLACK_BOT_TOKEN").expect("SLACK_BOT_TOKEN must be set");
        let channel = std::env::var("SLACK_TEST_CHANNEL").unwrap_or_else(|_| "#test".to_string());
        let tool = SlackGetMessage::new(token);

        let result = tool
            ._call_str(format!(r##"{{"channel": "{}", "limit": 5}}"##, channel))
            .await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    #[ignore = "requires SLACK_BOT_TOKEN"]
    async fn test_get_channel_integration() {
        let token = std::env::var("SLACK_BOT_TOKEN").expect("SLACK_BOT_TOKEN must be set");
        let tool = SlackGetChannel::new(token);

        let result = tool._call_str(r#"{"limit": 10}"#.to_string()).await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    #[ignore = "requires SLACK_BOT_TOKEN"]
    async fn test_schedule_message_integration() {
        use chrono::{Duration, Utc};

        let token = std::env::var("SLACK_BOT_TOKEN").expect("SLACK_BOT_TOKEN must be set");
        let channel = std::env::var("SLACK_TEST_CHANNEL").unwrap_or_else(|_| "#test".to_string());
        let tool = SlackScheduleMessage::new(token);

        let post_at = (Utc::now() + Duration::hours(1)).timestamp();
        let input = format!(
            r##"{{"channel": "{}", "text": "Scheduled test message", "post_at": {}}}"##,
            channel, post_at
        );

        let result = tool._call_str(input).await;
        assert!(result.is_ok());
    }
}
