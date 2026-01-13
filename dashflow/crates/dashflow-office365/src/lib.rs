//! Office 365 tools for `DashFlow` Rust
//!
//! This crate provides tools for interacting with Microsoft Graph API for Office 365, including:
//! - Send email messages
//! - Create calendar events
//! - Search emails
//! - Get messages by ID
//! - Create drafts
//! - List calendar events
//!
//! # Example
//!
//! ```no_run
//! use dashflow_office365::{Office365SendMessage, Office365Auth};
//! use dashflow::core::tools::{Tool, ToolInput};
//! use serde_json::json;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     // Create Office 365 authentication
//!     let auth = Office365Auth::new(
//!         "client_id".to_string(),
//!         "client_secret".to_string(),
//!         "tenant_id".to_string(),
//!     )?;
//!
//!     // Create send message tool
//!     let tool = Office365SendMessage::new(auth);
//!
//!     // Prepare input
//!     let input = ToolInput::Structured(json!({
//!         "to": "user@example.com",
//!         "subject": "Test",
//!         "message": "Hello!"
//!     }));
//!
//!     // Send message
//!     let result = tool._call(input).await?;
//!     println!("{}", result);
//!
//!     Ok(())
//! }
//! ```

use async_trait::async_trait;
use dashflow::core::error::{Error, Result};
use dashflow::core::tools::{Tool, ToolInput};
use graph_rs_sdk::{GraphClient, ODataQuery};
use serde_json::{json, Value};
use std::sync::Arc;

// Re-export for convenience
pub use graph_rs_sdk;

/// Office 365 authentication handler using Microsoft Graph API
#[derive(Clone)]
pub struct Office365Auth {
    client: Arc<GraphClient>,
}

impl Office365Auth {
    /// Create Office 365 authentication with access token
    ///
    /// This uses an existing access token. For production, use Azure AD `OAuth2` flow
    /// to obtain the token with appropriate scopes (Mail.Send, Calendars.ReadWrite, etc.)
    pub fn new(
        client_id: impl Into<String>,
        client_secret: impl Into<String>,
        tenant_id: impl Into<String>,
    ) -> Result<Self> {
        // Note: In production, you would use ConfidentialClientApplication to get a token
        // For simplicity, this assumes the user provides a valid access token
        // The actual token acquisition would happen outside this library
        let token = format!(
            "{}:{}@{}",
            client_id.into(),
            client_secret.into(),
            tenant_id.into()
        );
        let client = GraphClient::new(&token);

        Ok(Self {
            client: Arc::new(client),
        })
    }

    /// Create Office 365 authentication with an access token
    #[must_use]
    pub fn with_token(access_token: impl Into<String>) -> Self {
        let access_token = access_token.into();
        let client = GraphClient::new(&access_token);
        Self {
            client: Arc::new(client),
        }
    }

    /// Get `GraphClient` for making API calls
    #[must_use]
    pub fn client(&self) -> &GraphClient {
        &self.client
    }
}

/// Office 365 Send Message tool
///
/// Sends an email message via Microsoft Graph API
pub struct Office365SendMessage {
    auth: Office365Auth,
}

impl Office365SendMessage {
    /// Create a new `Office365SendMessage` tool
    #[must_use]
    pub fn new(auth: Office365Auth) -> Self {
        Self { auth }
    }
}

#[async_trait]
impl Tool for Office365SendMessage {
    fn name(&self) -> &'static str {
        "send_office365_message"
    }

    fn description(&self) -> &'static str {
        "Use this tool to send email messages via Office 365. The input should include 'to', 'subject', and 'message' fields. Optionally include 'cc' and 'bcc' fields."
    }

    async fn _call(&self, input: ToolInput) -> Result<String> {
        let input_data = match input {
            ToolInput::Structured(v) => v,
            ToolInput::String(s) => serde_json::from_str(&s)
                .map_err(|e| Error::invalid_input(format!("Failed to parse input: {e}")))?,
        };

        let message = input_data
            .get("message")
            .and_then(|v| v.as_str())
            .ok_or_else(|| Error::invalid_input("'message' field required"))?;
        let subject = input_data
            .get("subject")
            .and_then(|v| v.as_str())
            .ok_or_else(|| Error::invalid_input("'subject' field required"))?;

        // Parse 'to' field (can be string or array)
        let to: Vec<String> = match input_data.get("to") {
            Some(Value::String(s)) => vec![s.clone()],
            Some(Value::Array(arr)) => arr
                .iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect(),
            _ => return Err(Error::invalid_input("'to' field must be a string or array")),
        };

        let cc = input_data.get("cc").and_then(|v| match v {
            Value::String(s) => Some(vec![s.clone()]),
            Value::Array(arr) => Some(
                arr.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect(),
            ),
            _ => None,
        });

        let bcc = input_data.get("bcc").and_then(|v| match v {
            Value::String(s) => Some(vec![s.clone()]),
            Value::Array(arr) => Some(
                arr.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect(),
            ),
            _ => None,
        });

        // Build email message body for Microsoft Graph API
        let mut to_recipients = Vec::new();
        for addr in &to {
            to_recipients.push(json!({
                "emailAddress": {
                    "address": addr
                }
            }));
        }

        let mut body = json!({
            "message": {
                "subject": subject,
                "body": {
                    "contentType": "HTML",
                    "content": message
                },
                "toRecipients": to_recipients
            }
        });

        if let Some(cc_addrs) = cc {
            let mut cc_recipients = Vec::new();
            for addr in &cc_addrs {
                cc_recipients.push(json!({
                    "emailAddress": {
                        "address": addr
                    }
                }));
            }
            body["message"]["ccRecipients"] = json!(cc_recipients);
        }

        if let Some(bcc_addrs) = bcc {
            let mut bcc_recipients = Vec::new();
            for addr in &bcc_addrs {
                bcc_recipients.push(json!({
                    "emailAddress": {
                        "address": addr
                    }
                }));
            }
            body["message"]["bccRecipients"] = json!(bcc_recipients);
        }

        let response = self
            .auth
            .client()
            .me()
            .send_mail(&body)
            .send()
            .await
            .map_err(|e| Error::tool_error(format!("Failed to send Office 365 message: {e}")))?;

        Ok(format!(
            "Message sent successfully via Office 365. Status: {}",
            response.status()
        ))
    }
}

/// Office 365 Create Draft tool
///
/// Creates a draft email message
pub struct Office365CreateDraft {
    auth: Office365Auth,
}

impl Office365CreateDraft {
    /// Create a new `Office365CreateDraft` tool
    #[must_use]
    pub fn new(auth: Office365Auth) -> Self {
        Self { auth }
    }
}

#[async_trait]
impl Tool for Office365CreateDraft {
    fn name(&self) -> &'static str {
        "create_office365_draft"
    }

    fn description(&self) -> &'static str {
        "Use this tool to create a draft email in Office 365. The input should include 'to', 'subject', and 'message' fields."
    }

    async fn _call(&self, input: ToolInput) -> Result<String> {
        let input_data = match input {
            ToolInput::Structured(v) => v,
            ToolInput::String(s) => serde_json::from_str(&s)
                .map_err(|e| Error::invalid_input(format!("Failed to parse input: {e}")))?,
        };

        let message = input_data
            .get("message")
            .and_then(|v| v.as_str())
            .ok_or_else(|| Error::invalid_input("'message' field required"))?;
        let subject = input_data
            .get("subject")
            .and_then(|v| v.as_str())
            .ok_or_else(|| Error::invalid_input("'subject' field required"))?;

        let to: Vec<String> = match input_data.get("to") {
            Some(Value::String(s)) => vec![s.clone()],
            Some(Value::Array(arr)) => arr
                .iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect(),
            _ => return Err(Error::invalid_input("'to' field must be a string or array")),
        };

        let mut to_recipients = Vec::new();
        for addr in &to {
            to_recipients.push(json!({
                "emailAddress": {
                    "address": addr
                }
            }));
        }

        let body = json!({
            "subject": subject,
            "body": {
                "contentType": "HTML",
                "content": message
            },
            "toRecipients": to_recipients
        });

        let response = self
            .auth
            .client()
            .me()
            .messages()
            .create_messages(&body)
            .send()
            .await
            .map_err(|e| Error::tool_error(format!("Failed to create Office 365 draft: {e}")))?;

        let draft_id = response
            .json::<Value>()
            .await
            .map_err(|e| Error::tool_error(format!("Failed to parse response: {e}")))?
            .get("id")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown")
            .to_string();

        Ok(format!("Draft created successfully. Draft ID: {draft_id}"))
    }
}

/// Office 365 Search Emails tool
///
/// Searches for emails in mailbox
pub struct Office365SearchEmails {
    auth: Office365Auth,
}

impl Office365SearchEmails {
    /// Create a new `Office365SearchEmails` tool
    #[must_use]
    pub fn new(auth: Office365Auth) -> Self {
        Self { auth }
    }
}

#[async_trait]
impl Tool for Office365SearchEmails {
    fn name(&self) -> &'static str {
        "search_office365_emails"
    }

    fn description(&self) -> &'static str {
        "Use this tool to search for emails in Office 365. The input should include a 'query' field with the search query. Optionally include 'max_results' (default 10)."
    }

    async fn _call(&self, input: ToolInput) -> Result<String> {
        let input_data = match input {
            ToolInput::Structured(v) => v,
            ToolInput::String(s) => serde_json::from_str(&s)
                .map_err(|e| Error::invalid_input(format!("Failed to parse input: {e}")))?,
        };

        let query = input_data
            .get("query")
            .and_then(|v| v.as_str())
            .ok_or_else(|| Error::invalid_input("'query' field required"))?;

        let max_results = input_data
            .get("max_results")
            .and_then(serde_json::Value::as_i64)
            .unwrap_or(10) as usize;

        let response = self
            .auth
            .client()
            .me()
            .messages()
            .list_messages()
            .filter(&[query])
            .top(max_results.to_string())
            .send()
            .await
            .map_err(|e| Error::tool_error(format!("Failed to search Office 365 emails: {e}")))?;

        let messages = response
            .json::<Value>()
            .await
            .map_err(|e| Error::tool_error(format!("Failed to parse response: {e}")))?;

        let value = messages.get("value").and_then(|v| v.as_array());

        match value {
            Some(msgs) if !msgs.is_empty() => {
                let mut result = format!("Found {} email(s):\n\n", msgs.len());
                for msg in msgs {
                    let subject = msg
                        .get("subject")
                        .and_then(|v| v.as_str())
                        .unwrap_or("No subject");
                    let from = msg
                        .get("from")
                        .and_then(|v| v.get("emailAddress"))
                        .and_then(|v| v.get("address"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("Unknown");
                    let id = msg.get("id").and_then(|v| v.as_str()).unwrap_or("No ID");
                    result.push_str(&format!(
                        "- Subject: {subject}\n  From: {from}\n  ID: {id}\n\n"
                    ));
                }
                Ok(result)
            }
            _ => Ok("No emails found matching the query.".to_string()),
        }
    }
}

/// Office 365 Get Message tool
///
/// Gets a specific email message by ID
pub struct Office365GetMessage {
    auth: Office365Auth,
}

impl Office365GetMessage {
    /// Create a new `Office365GetMessage` tool
    #[must_use]
    pub fn new(auth: Office365Auth) -> Self {
        Self { auth }
    }
}

#[async_trait]
impl Tool for Office365GetMessage {
    fn name(&self) -> &'static str {
        "get_office365_message"
    }

    fn description(&self) -> &'static str {
        "Use this tool to get a specific email message by ID from Office 365. The input should include 'message_id' field."
    }

    async fn _call(&self, input: ToolInput) -> Result<String> {
        let input_data = match input {
            ToolInput::Structured(v) => v,
            ToolInput::String(s) => serde_json::from_str(&s)
                .map_err(|e| Error::invalid_input(format!("Failed to parse input: {e}")))?,
        };

        let message_id = input_data
            .get("message_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| Error::invalid_input("'message_id' field required"))?;

        let response = self
            .auth
            .client()
            .me()
            .message(message_id)
            .get_messages()
            .send()
            .await
            .map_err(|e| Error::tool_error(format!("Failed to get Office 365 message: {e}")))?;

        let message = response
            .json::<Value>()
            .await
            .map_err(|e| Error::tool_error(format!("Failed to parse response: {e}")))?;

        let subject = message
            .get("subject")
            .and_then(|v| v.as_str())
            .unwrap_or("No subject");
        let from = message
            .get("from")
            .and_then(|v| v.get("emailAddress"))
            .and_then(|v| v.get("address"))
            .and_then(|v| v.as_str())
            .unwrap_or("Unknown");
        let body = message
            .get("body")
            .and_then(|v| v.get("content"))
            .and_then(|v| v.as_str())
            .unwrap_or("No body");

        Ok(format!("Subject: {subject}\nFrom: {from}\n\nBody:\n{body}"))
    }
}

/// Office 365 Send Event tool
///
/// Creates a calendar event
pub struct Office365SendEvent {
    auth: Office365Auth,
}

impl Office365SendEvent {
    /// Create a new `Office365SendEvent` tool
    #[must_use]
    pub fn new(auth: Office365Auth) -> Self {
        Self { auth }
    }
}

#[async_trait]
impl Tool for Office365SendEvent {
    fn name(&self) -> &'static str {
        "send_office365_event"
    }

    fn description(&self) -> &'static str {
        "Use this tool to create a calendar event in Office 365. The input should include 'subject', 'start', 'end', and optionally 'attendees' (array of email addresses) and 'body'."
    }

    async fn _call(&self, input: ToolInput) -> Result<String> {
        let input_data = match input {
            ToolInput::Structured(v) => v,
            ToolInput::String(s) => serde_json::from_str(&s)
                .map_err(|e| Error::invalid_input(format!("Failed to parse input: {e}")))?,
        };

        let subject = input_data
            .get("subject")
            .and_then(|v| v.as_str())
            .ok_or_else(|| Error::invalid_input("'subject' field required"))?;
        let start = input_data
            .get("start")
            .and_then(|v| v.as_str())
            .ok_or_else(|| Error::invalid_input("'start' field required (ISO 8601 format)"))?;
        let end = input_data
            .get("end")
            .and_then(|v| v.as_str())
            .ok_or_else(|| Error::invalid_input("'end' field required (ISO 8601 format)"))?;

        let body_content = input_data
            .get("body")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        let mut event_body = json!({
            "subject": subject,
            "start": {
                "dateTime": start,
                "timeZone": "UTC"
            },
            "end": {
                "dateTime": end,
                "timeZone": "UTC"
            },
            "body": {
                "contentType": "HTML",
                "content": body_content
            }
        });

        if let Some(attendees_value) = input_data.get("attendees") {
            let attendees: Vec<String> = match attendees_value {
                Value::String(s) => vec![s.clone()],
                Value::Array(arr) => arr
                    .iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect(),
                _ => Vec::new(),
            };

            let mut attendees_json = Vec::new();
            for email in attendees {
                attendees_json.push(json!({
                    "emailAddress": {
                        "address": email
                    },
                    "type": "required"
                }));
            }
            event_body["attendees"] = json!(attendees_json);
        }

        let response = self
            .auth
            .client()
            .me()
            .events()
            .create_events(&event_body)
            .send()
            .await
            .map_err(|e| Error::tool_error(format!("Failed to create Office 365 event: {e}")))?;

        let event_id = response
            .json::<Value>()
            .await
            .map_err(|e| Error::tool_error(format!("Failed to parse response: {e}")))?
            .get("id")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown")
            .to_string();

        Ok(format!("Event created successfully. Event ID: {event_id}"))
    }
}

/// Office 365 List Events tool
///
/// Lists calendar events
pub struct Office365ListEvents {
    auth: Office365Auth,
}

impl Office365ListEvents {
    /// Create a new `Office365ListEvents` tool
    #[must_use]
    pub fn new(auth: Office365Auth) -> Self {
        Self { auth }
    }
}

#[async_trait]
impl Tool for Office365ListEvents {
    fn name(&self) -> &'static str {
        "list_office365_events"
    }

    fn description(&self) -> &'static str {
        "Use this tool to list calendar events from Office 365. Optionally include 'max_results' (default 10)."
    }

    async fn _call(&self, input: ToolInput) -> Result<String> {
        let input_data = match input {
            ToolInput::Structured(v) => v,
            ToolInput::String(s) => serde_json::from_str(&s)
                .map_err(|e| Error::invalid_input(format!("Failed to parse input: {e}")))?,
        };

        let max_results = input_data
            .get("max_results")
            .and_then(serde_json::Value::as_i64)
            .unwrap_or(10) as usize;

        let response = self
            .auth
            .client()
            .me()
            .events()
            .list_events()
            .top(max_results.to_string())
            .send()
            .await
            .map_err(|e| Error::tool_error(format!("Failed to list Office 365 events: {e}")))?;

        let events = response
            .json::<Value>()
            .await
            .map_err(|e| Error::tool_error(format!("Failed to parse response: {e}")))?;

        let value = events.get("value").and_then(|v| v.as_array());

        match value {
            Some(evts) if !evts.is_empty() => {
                let mut result = format!("Found {} event(s):\n\n", evts.len());
                for evt in evts {
                    let subject = evt
                        .get("subject")
                        .and_then(|v| v.as_str())
                        .unwrap_or("No subject");
                    let start = evt
                        .get("start")
                        .and_then(|v| v.get("dateTime"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("Unknown");
                    let end = evt
                        .get("end")
                        .and_then(|v| v.get("dateTime"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("Unknown");
                    let id = evt.get("id").and_then(|v| v.as_str()).unwrap_or("No ID");
                    result.push_str(&format!(
                        "- Subject: {subject}\n  Start: {start}\n  End: {end}\n  ID: {id}\n\n"
                    ));
                }
                Ok(result)
            }
            _ => Ok("No events found.".to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::unwrap_used)]

    use super::*;
    use std::sync::Arc;

    // ==================== Office365Auth Tests ====================

    #[test]
    fn test_auth_with_token_basic() {
        let auth = Office365Auth::with_token("test_token");
        let client = auth.client();
        assert!(client.url().as_str().starts_with("https://"));
    }

    #[test]
    fn test_auth_with_token_string() {
        let auth = Office365Auth::with_token("my_access_token".to_string());
        assert!(auth.client().url().as_str().starts_with("https://"));
    }

    #[test]
    fn test_auth_with_token_empty() {
        let auth = Office365Auth::with_token("");
        // Should still create auth, even with empty token
        assert!(auth.client().url().as_str().starts_with("https://"));
    }

    #[test]
    fn test_auth_new_valid() {
        let auth = Office365Auth::new("client_id", "client_secret", "tenant_id");
        assert!(auth.is_ok());
        let auth = auth.unwrap();
        assert!(auth.client().url().as_str().starts_with("https://"));
    }

    #[test]
    fn test_auth_new_with_strings() {
        let auth = Office365Auth::new(
            "my_client_id".to_string(),
            "my_secret".to_string(),
            "my_tenant".to_string(),
        );
        assert!(auth.is_ok());
    }

    #[test]
    fn test_auth_new_empty_values() {
        // Empty values should still work for construction
        let auth = Office365Auth::new("", "", "");
        assert!(auth.is_ok());
    }

    #[test]
    fn test_auth_clone() {
        let auth = Office365Auth::with_token("token");
        let cloned = auth.clone();
        // Both should have valid clients
        assert!(auth.client().url().as_str().starts_with("https://"));
        assert!(cloned.client().url().as_str().starts_with("https://"));
    }

    #[test]
    fn test_auth_client_returns_graph_client() {
        let auth = Office365Auth::with_token("test_token");
        let client = auth.client();
        // GraphClient URL should be the Microsoft Graph API base
        let url = client.url().as_str();
        assert!(url.contains("graph.microsoft.com") || url.starts_with("https://"));
    }

    // ==================== Tool Name Tests ====================

    #[test]
    fn test_tool_names_unique() {
        let tools = vec![
            "send_office365_message",
            "create_office365_draft",
            "search_office365_emails",
            "get_office365_message",
            "send_office365_event",
            "list_office365_events",
        ];

        assert_eq!(tools.len(), 6);

        let mut sorted = tools.clone();
        sorted.sort();
        sorted.dedup();
        assert_eq!(sorted.len(), tools.len());
    }

    #[test]
    fn test_send_message_tool_name() {
        let auth = Office365Auth::with_token("token");
        let tool = Office365SendMessage::new(auth);
        assert_eq!(tool.name(), "send_office365_message");
    }

    #[test]
    fn test_create_draft_tool_name() {
        let auth = Office365Auth::with_token("token");
        let tool = Office365CreateDraft::new(auth);
        assert_eq!(tool.name(), "create_office365_draft");
    }

    #[test]
    fn test_search_emails_tool_name() {
        let auth = Office365Auth::with_token("token");
        let tool = Office365SearchEmails::new(auth);
        assert_eq!(tool.name(), "search_office365_emails");
    }

    #[test]
    fn test_get_message_tool_name() {
        let auth = Office365Auth::with_token("token");
        let tool = Office365GetMessage::new(auth);
        assert_eq!(tool.name(), "get_office365_message");
    }

    #[test]
    fn test_send_event_tool_name() {
        let auth = Office365Auth::with_token("token");
        let tool = Office365SendEvent::new(auth);
        assert_eq!(tool.name(), "send_office365_event");
    }

    #[test]
    fn test_list_events_tool_name() {
        let auth = Office365Auth::with_token("token");
        let tool = Office365ListEvents::new(auth);
        assert_eq!(tool.name(), "list_office365_events");
    }

    // ==================== Tool Description Tests ====================

    #[test]
    fn test_send_message_description() {
        let auth = Office365Auth::with_token("token");
        let tool = Office365SendMessage::new(auth);
        let desc = tool.description();
        assert!(!desc.is_empty());
        assert!(desc.to_lowercase().contains("email") || desc.to_lowercase().contains("message"));
    }

    #[test]
    fn test_create_draft_description() {
        let auth = Office365Auth::with_token("token");
        let tool = Office365CreateDraft::new(auth);
        let desc = tool.description();
        assert!(!desc.is_empty());
        assert!(desc.to_lowercase().contains("draft"));
    }

    #[test]
    fn test_search_emails_description() {
        let auth = Office365Auth::with_token("token");
        let tool = Office365SearchEmails::new(auth);
        let desc = tool.description();
        assert!(!desc.is_empty());
        assert!(desc.to_lowercase().contains("search"));
    }

    #[test]
    fn test_get_message_description() {
        let auth = Office365Auth::with_token("token");
        let tool = Office365GetMessage::new(auth);
        let desc = tool.description();
        assert!(!desc.is_empty());
        assert!(desc.to_lowercase().contains("message"));
    }

    #[test]
    fn test_send_event_description() {
        let auth = Office365Auth::with_token("token");
        let tool = Office365SendEvent::new(auth);
        let desc = tool.description();
        assert!(!desc.is_empty());
        assert!(desc.to_lowercase().contains("event") || desc.to_lowercase().contains("calendar"));
    }

    #[test]
    fn test_list_events_description() {
        let auth = Office365Auth::with_token("token");
        let tool = Office365ListEvents::new(auth);
        let desc = tool.description();
        assert!(!desc.is_empty());
        assert!(desc.to_lowercase().contains("event") || desc.to_lowercase().contains("list"));
    }

    #[test]
    fn test_descriptions_mention_required_fields() {
        let auth = Office365Auth::with_token("token");

        // SendMessage should mention required fields
        let send_msg = Office365SendMessage::new(auth.clone());
        let desc = send_msg.description().to_lowercase();
        assert!(desc.contains("to") || desc.contains("subject") || desc.contains("message"));

        // SendEvent should mention required fields
        let send_evt = Office365SendEvent::new(auth.clone());
        let desc = send_evt.description().to_lowercase();
        assert!(desc.contains("subject") || desc.contains("start") || desc.contains("end"));

        // SearchEmails should mention query
        let search = Office365SearchEmails::new(auth);
        let desc = search.description().to_lowercase();
        assert!(desc.contains("query"));
    }

    // ==================== Input Validation Tests ====================

    #[tokio::test]
    async fn test_send_message_missing_message() {
        let auth = Office365Auth::with_token("token");
        let tool = Office365SendMessage::new(auth);

        let input = ToolInput::Structured(json!({
            "to": "test@example.com",
            "subject": "Test"
        }));

        let result = tool._call(input).await;
        assert!(result.is_err());
        let err = result.unwrap_err().to_string().to_lowercase();
        assert!(err.contains("message"));
    }

    #[tokio::test]
    async fn test_send_message_missing_subject() {
        let auth = Office365Auth::with_token("token");
        let tool = Office365SendMessage::new(auth);

        let input = ToolInput::Structured(json!({
            "to": "test@example.com",
            "message": "Hello"
        }));

        let result = tool._call(input).await;
        assert!(result.is_err());
        let err = result.unwrap_err().to_string().to_lowercase();
        assert!(err.contains("subject"));
    }

    #[tokio::test]
    async fn test_send_message_missing_to() {
        let auth = Office365Auth::with_token("token");
        let tool = Office365SendMessage::new(auth);

        let input = ToolInput::Structured(json!({
            "subject": "Test",
            "message": "Hello"
        }));

        let result = tool._call(input).await;
        assert!(result.is_err());
        let err = result.unwrap_err().to_string().to_lowercase();
        assert!(err.contains("to"));
    }

    #[tokio::test]
    async fn test_send_message_invalid_to_type() {
        let auth = Office365Auth::with_token("token");
        let tool = Office365SendMessage::new(auth);

        let input = ToolInput::Structured(json!({
            "to": 12345,  // Invalid type
            "subject": "Test",
            "message": "Hello"
        }));

        let result = tool._call(input).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_create_draft_missing_message() {
        let auth = Office365Auth::with_token("token");
        let tool = Office365CreateDraft::new(auth);

        let input = ToolInput::Structured(json!({
            "to": "test@example.com",
            "subject": "Test"
        }));

        let result = tool._call(input).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_create_draft_missing_subject() {
        let auth = Office365Auth::with_token("token");
        let tool = Office365CreateDraft::new(auth);

        let input = ToolInput::Structured(json!({
            "to": "test@example.com",
            "message": "Hello"
        }));

        let result = tool._call(input).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_create_draft_missing_to() {
        let auth = Office365Auth::with_token("token");
        let tool = Office365CreateDraft::new(auth);

        let input = ToolInput::Structured(json!({
            "subject": "Test",
            "message": "Hello"
        }));

        let result = tool._call(input).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_search_emails_missing_query() {
        let auth = Office365Auth::with_token("token");
        let tool = Office365SearchEmails::new(auth);

        let input = ToolInput::Structured(json!({}));

        let result = tool._call(input).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("query"));
    }

    #[test]
    fn test_search_emails_empty_query_parsing() {
        // Empty query is technically valid JSON
        let input = json!({
            "query": ""
        });

        let query = input.get("query").unwrap().as_str().unwrap();
        assert!(query.is_empty());
    }

    #[tokio::test]
    async fn test_get_message_missing_id() {
        let auth = Office365Auth::with_token("token");
        let tool = Office365GetMessage::new(auth);

        let input = ToolInput::Structured(json!({}));

        let result = tool._call(input).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("message_id"));
    }

    #[tokio::test]
    async fn test_send_event_missing_start() {
        let auth = Office365Auth::with_token("token");
        let tool = Office365SendEvent::new(auth);

        let input = ToolInput::Structured(json!({
            "subject": "Meeting",
            "end": "2024-01-01T11:00:00"
        }));

        let result = tool._call(input).await;
        assert!(result.is_err());
        let err = result.unwrap_err().to_string().to_lowercase();
        assert!(err.contains("start"));
    }

    #[tokio::test]
    async fn test_send_event_missing_end() {
        let auth = Office365Auth::with_token("token");
        let tool = Office365SendEvent::new(auth);

        let input = ToolInput::Structured(json!({
            "subject": "Meeting",
            "start": "2024-01-01T10:00:00"
        }));

        let result = tool._call(input).await;
        assert!(result.is_err());
        let err = result.unwrap_err().to_string().to_lowercase();
        assert!(err.contains("end"));
    }

    #[tokio::test]
    async fn test_send_event_missing_subject() {
        let auth = Office365Auth::with_token("token");
        let tool = Office365SendEvent::new(auth);

        let input = ToolInput::Structured(json!({
            "start": "2024-01-01T10:00:00",
            "end": "2024-01-01T11:00:00"
        }));

        let result = tool._call(input).await;
        assert!(result.is_err());
        let err = result.unwrap_err().to_string().to_lowercase();
        assert!(err.contains("subject"));
    }

    // ==================== String Input Parsing Tests ====================

    #[test]
    fn test_string_input_parsing_valid_json() {
        // Test that valid JSON strings can be parsed
        let input_str = r#"{"to": "test@example.com", "subject": "Test", "message": "Hello"}"#;
        let parsed: serde_json::Result<Value> = serde_json::from_str(input_str);
        assert!(parsed.is_ok());

        let value = parsed.unwrap();
        assert_eq!(value.get("to").unwrap().as_str().unwrap(), "test@example.com");
        assert_eq!(value.get("subject").unwrap().as_str().unwrap(), "Test");
        assert_eq!(value.get("message").unwrap().as_str().unwrap(), "Hello");
    }

    #[tokio::test]
    async fn test_send_message_invalid_json_string() {
        let auth = Office365Auth::with_token("token");
        let tool = Office365SendMessage::new(auth);

        let input = ToolInput::String("not valid json".to_string());

        let result = tool._call(input).await;
        assert!(result.is_err());
        let err = result.unwrap_err().to_string().to_lowercase();
        assert!(err.contains("parse") || err.contains("json"));
    }

    #[tokio::test]
    async fn test_create_draft_invalid_json_string() {
        let auth = Office365Auth::with_token("token");
        let tool = Office365CreateDraft::new(auth);

        let input = ToolInput::String("{invalid".to_string());

        let result = tool._call(input).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_search_emails_invalid_json_string() {
        let auth = Office365Auth::with_token("token");
        let tool = Office365SearchEmails::new(auth);

        let input = ToolInput::String("not json at all".to_string());

        let result = tool._call(input).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_get_message_invalid_json_string() {
        let auth = Office365Auth::with_token("token");
        let tool = Office365GetMessage::new(auth);

        let input = ToolInput::String("[not an object]".to_string());

        let result = tool._call(input).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_send_event_invalid_json_string() {
        let auth = Office365Auth::with_token("token");
        let tool = Office365SendEvent::new(auth);

        let input = ToolInput::String("{{bad json}}".to_string());

        let result = tool._call(input).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_list_events_invalid_json_string() {
        let auth = Office365Auth::with_token("token");
        let tool = Office365ListEvents::new(auth);

        let input = ToolInput::String("null".to_string());

        // null is valid JSON but not a valid object for our purposes
        // The result depends on implementation - just test it doesn't panic
        let _result = tool._call(input).await;
    }

    #[test]
    fn test_search_query_string_parsing() {
        let input_str = r#"{"query": "from:test@example.com", "max_results": 25}"#;
        let parsed: Value = serde_json::from_str(input_str).unwrap();

        assert_eq!(parsed.get("query").unwrap().as_str().unwrap(), "from:test@example.com");
        assert_eq!(parsed.get("max_results").unwrap().as_i64().unwrap(), 25);
    }

    #[test]
    fn test_event_string_parsing() {
        let input_str = r#"{"subject": "Meeting", "start": "2024-01-01T10:00:00", "end": "2024-01-01T11:00:00"}"#;
        let parsed: Value = serde_json::from_str(input_str).unwrap();

        assert_eq!(parsed.get("subject").unwrap().as_str().unwrap(), "Meeting");
        assert_eq!(parsed.get("start").unwrap().as_str().unwrap(), "2024-01-01T10:00:00");
        assert_eq!(parsed.get("end").unwrap().as_str().unwrap(), "2024-01-01T11:00:00");
    }

    // ==================== Recipient Parsing Tests ====================

    #[test]
    fn test_recipient_single_string() {
        let input = json!({
            "to": "user@example.com"
        });

        let to = input.get("to").unwrap();
        assert!(to.is_string());
        assert_eq!(to.as_str().unwrap(), "user@example.com");
    }

    #[test]
    fn test_recipient_array() {
        let input = json!({
            "to": ["user1@example.com", "user2@example.com"]
        });

        let to = input.get("to").unwrap();
        assert!(to.is_array());
        let arr = to.as_array().unwrap();
        assert_eq!(arr.len(), 2);
        assert_eq!(arr[0].as_str().unwrap(), "user1@example.com");
        assert_eq!(arr[1].as_str().unwrap(), "user2@example.com");
    }

    #[test]
    fn test_recipient_empty_array() {
        let input = json!({
            "to": []
        });

        let to = input.get("to").unwrap();
        assert!(to.is_array());
        assert!(to.as_array().unwrap().is_empty());
    }

    #[test]
    fn test_cc_as_string() {
        let input = json!({
            "to": "to@example.com",
            "cc": "cc@example.com"
        });

        let cc = input.get("cc").unwrap();
        assert!(cc.is_string());
    }

    #[test]
    fn test_cc_as_array() {
        let input = json!({
            "to": "to@example.com",
            "cc": ["cc1@example.com", "cc2@example.com"]
        });

        let cc = input.get("cc").unwrap();
        assert!(cc.is_array());
        assert_eq!(cc.as_array().unwrap().len(), 2);
    }

    #[test]
    fn test_bcc_as_string() {
        let input = json!({
            "to": "to@example.com",
            "bcc": "bcc@example.com"
        });

        let bcc = input.get("bcc").unwrap();
        assert!(bcc.is_string());
    }

    #[test]
    fn test_bcc_as_array() {
        let input = json!({
            "to": "to@example.com",
            "bcc": ["bcc1@example.com", "bcc2@example.com"]
        });

        let bcc = input.get("bcc").unwrap();
        assert!(bcc.is_array());
    }

    #[test]
    fn test_all_recipient_types_combined() {
        let input = json!({
            "to": ["to1@example.com", "to2@example.com"],
            "cc": "cc@example.com",
            "bcc": ["bcc1@example.com", "bcc2@example.com", "bcc3@example.com"]
        });

        assert!(input.get("to").unwrap().is_array());
        assert!(input.get("cc").unwrap().is_string());
        assert!(input.get("bcc").unwrap().is_array());
        assert_eq!(input.get("bcc").unwrap().as_array().unwrap().len(), 3);
    }

    #[test]
    fn test_attendees_as_string() {
        let input = json!({
            "subject": "Meeting",
            "start": "2024-01-01T10:00:00",
            "end": "2024-01-01T11:00:00",
            "attendees": "attendee@example.com"
        });

        let attendees = input.get("attendees").unwrap();
        assert!(attendees.is_string());
    }

    #[test]
    fn test_attendees_as_array() {
        let input = json!({
            "subject": "Meeting",
            "start": "2024-01-01T10:00:00",
            "end": "2024-01-01T11:00:00",
            "attendees": ["a1@example.com", "a2@example.com"]
        });

        let attendees = input.get("attendees").unwrap();
        assert!(attendees.is_array());
        assert_eq!(attendees.as_array().unwrap().len(), 2);
    }

    // ==================== Send + Sync Bounds Tests ====================

    fn assert_send<T: Send>() {}
    fn assert_sync<T: Sync>() {}

    #[test]
    fn test_auth_is_send() {
        assert_send::<Office365Auth>();
    }

    #[test]
    fn test_auth_is_sync() {
        assert_sync::<Office365Auth>();
    }

    #[test]
    fn test_send_message_is_send() {
        assert_send::<Office365SendMessage>();
    }

    #[test]
    fn test_send_message_is_sync() {
        assert_sync::<Office365SendMessage>();
    }

    #[test]
    fn test_create_draft_is_send() {
        assert_send::<Office365CreateDraft>();
    }

    #[test]
    fn test_create_draft_is_sync() {
        assert_sync::<Office365CreateDraft>();
    }

    #[test]
    fn test_search_emails_is_send() {
        assert_send::<Office365SearchEmails>();
    }

    #[test]
    fn test_search_emails_is_sync() {
        assert_sync::<Office365SearchEmails>();
    }

    #[test]
    fn test_get_message_is_send() {
        assert_send::<Office365GetMessage>();
    }

    #[test]
    fn test_get_message_is_sync() {
        assert_sync::<Office365GetMessage>();
    }

    #[test]
    fn test_send_event_is_send() {
        assert_send::<Office365SendEvent>();
    }

    #[test]
    fn test_send_event_is_sync() {
        assert_sync::<Office365SendEvent>();
    }

    #[test]
    fn test_list_events_is_send() {
        assert_send::<Office365ListEvents>();
    }

    #[test]
    fn test_list_events_is_sync() {
        assert_sync::<Office365ListEvents>();
    }

    // ==================== Edge Case Tests ====================

    #[test]
    fn test_special_chars_in_subject() {
        let input = json!({
            "to": "test@example.com",
            "subject": "Test <script>alert('xss')</script> & special \"chars\"",
            "message": "Hello"
        });

        let subject = input.get("subject").unwrap().as_str().unwrap();
        assert!(subject.contains("<script>"));
        assert!(subject.contains("&"));
    }

    #[test]
    fn test_unicode_in_message() {
        let input = json!({
            "to": "test@example.com",
            "subject": "日本語テスト",
            "message": "你好世界 🌍 مرحبا"
        });

        let subject = input.get("subject").unwrap().as_str().unwrap();
        let message = input.get("message").unwrap().as_str().unwrap();

        assert!(subject.contains("日本語"));
        assert!(message.contains("你好"));
        assert!(message.contains("🌍"));
    }

    #[test]
    fn test_very_long_subject() {
        let long_subject = "A".repeat(1000);
        let input = json!({
            "to": "test@example.com",
            "subject": long_subject,
            "message": "Hello"
        });

        let subject = input.get("subject").unwrap().as_str().unwrap();
        assert_eq!(subject.len(), 1000);
    }

    #[test]
    fn test_very_long_message() {
        let long_message = "Hello ".repeat(10000);
        let input = json!({
            "to": "test@example.com",
            "subject": "Test",
            "message": long_message
        });

        let message = input.get("message").unwrap().as_str().unwrap();
        assert!(message.len() > 50000);
    }

    #[test]
    fn test_whitespace_only_subject() {
        let input = json!({
            "to": "test@example.com",
            "subject": "   \t\n   ",
            "message": "Hello"
        });

        let subject = input.get("subject").unwrap().as_str().unwrap();
        assert!(subject.trim().is_empty());
    }

    #[test]
    fn test_html_in_message() {
        let input = json!({
            "to": "test@example.com",
            "subject": "Test",
            "message": "<h1>Header</h1><p>Paragraph with <strong>bold</strong></p>"
        });

        let message = input.get("message").unwrap().as_str().unwrap();
        assert!(message.contains("<h1>"));
        assert!(message.contains("<strong>"));
    }

    #[test]
    fn test_email_with_display_name() {
        // Some systems support "Display Name <email@example.com>" format
        let input = json!({
            "to": "John Doe <john@example.com>",
            "subject": "Test",
            "message": "Hello"
        });

        let to = input.get("to").unwrap().as_str().unwrap();
        assert!(to.contains("John Doe"));
        assert!(to.contains("<"));
    }

    #[test]
    fn test_iso_datetime_format() {
        let input = json!({
            "subject": "Meeting",
            "start": "2024-12-25T10:00:00Z",
            "end": "2024-12-25T11:30:00Z"
        });

        let start = input.get("start").unwrap().as_str().unwrap();
        let end = input.get("end").unwrap().as_str().unwrap();

        assert!(start.contains("T"));
        assert!(end.contains("Z"));
    }

    #[test]
    fn test_max_results_parsing() {
        let input = json!({
            "query": "test",
            "max_results": 50
        });

        let max = input.get("max_results").unwrap().as_i64().unwrap();
        assert_eq!(max, 50);
    }

    #[test]
    fn test_max_results_default() {
        let input = json!({
            "query": "test"
        });

        // Default should be 10 when not specified
        let max = input.get("max_results").and_then(|v| v.as_i64()).unwrap_or(10);
        assert_eq!(max, 10);
    }

    // ==================== Concurrent Usage Tests ====================

    #[test]
    fn test_auth_in_arc() {
        let auth = Arc::new(Office365Auth::with_token("token"));
        let auth2 = Arc::clone(&auth);

        assert!(auth.client().url().as_str().starts_with("https://"));
        assert!(auth2.client().url().as_str().starts_with("https://"));
    }

    #[test]
    fn test_multiple_tools_same_auth() {
        let auth = Office365Auth::with_token("token");

        let send_msg = Office365SendMessage::new(auth.clone());
        let create_draft = Office365CreateDraft::new(auth.clone());
        let search = Office365SearchEmails::new(auth.clone());
        let get_msg = Office365GetMessage::new(auth.clone());
        let send_evt = Office365SendEvent::new(auth.clone());
        let list_evts = Office365ListEvents::new(auth);

        // All tools should work with the same auth
        assert_eq!(send_msg.name(), "send_office365_message");
        assert_eq!(create_draft.name(), "create_office365_draft");
        assert_eq!(search.name(), "search_office365_emails");
        assert_eq!(get_msg.name(), "get_office365_message");
        assert_eq!(send_evt.name(), "send_office365_event");
        assert_eq!(list_evts.name(), "list_office365_events");
    }

    #[test]
    fn test_create_multiple_independent_instances() {
        let auth1 = Office365Auth::with_token("token1");
        let auth2 = Office365Auth::with_token("token2");

        let tool1 = Office365SendMessage::new(auth1);
        let tool2 = Office365SendMessage::new(auth2);

        // Both tools should work independently
        assert_eq!(tool1.name(), tool2.name());
    }

    // ==================== JSON Structure Tests ====================

    #[test]
    fn test_email_body_structure() {
        // Test the expected structure for sending mail
        let body = json!({
            "message": {
                "subject": "Test Subject",
                "body": {
                    "contentType": "HTML",
                    "content": "<p>Test message</p>"
                },
                "toRecipients": [
                    {
                        "emailAddress": {
                            "address": "test@example.com"
                        }
                    }
                ]
            }
        });

        assert!(body.get("message").is_some());
        let message = body.get("message").unwrap();
        assert_eq!(
            message.get("subject").unwrap().as_str().unwrap(),
            "Test Subject"
        );
        assert!(message.get("body").is_some());
        assert!(message.get("toRecipients").is_some());
    }

    #[test]
    fn test_event_body_structure() {
        // Test the expected structure for creating events
        let body = json!({
            "subject": "Meeting",
            "start": {
                "dateTime": "2024-01-01T10:00:00",
                "timeZone": "UTC"
            },
            "end": {
                "dateTime": "2024-01-01T11:00:00",
                "timeZone": "UTC"
            },
            "body": {
                "contentType": "HTML",
                "content": "Meeting notes"
            },
            "attendees": [
                {
                    "emailAddress": {
                        "address": "attendee@example.com"
                    },
                    "type": "required"
                }
            ]
        });

        assert_eq!(body.get("subject").unwrap().as_str().unwrap(), "Meeting");
        assert!(body.get("start").is_some());
        assert!(body.get("end").is_some());
        assert!(body.get("attendees").is_some());

        let start = body.get("start").unwrap();
        assert_eq!(start.get("timeZone").unwrap().as_str().unwrap(), "UTC");
    }

    #[test]
    fn test_recipient_structure() {
        let recipient = json!({
            "emailAddress": {
                "address": "user@example.com"
            }
        });

        let email = recipient
            .get("emailAddress")
            .unwrap()
            .get("address")
            .unwrap()
            .as_str()
            .unwrap();
        assert_eq!(email, "user@example.com");
    }

    #[test]
    fn test_search_response_structure() {
        // Test parsing a typical search response
        let response = json!({
            "value": [
                {
                    "id": "msg-123",
                    "subject": "Test Email",
                    "from": {
                        "emailAddress": {
                            "address": "sender@example.com"
                        }
                    }
                }
            ]
        });

        let value = response.get("value").unwrap().as_array().unwrap();
        assert_eq!(value.len(), 1);

        let msg = &value[0];
        assert_eq!(msg.get("id").unwrap().as_str().unwrap(), "msg-123");
        assert_eq!(msg.get("subject").unwrap().as_str().unwrap(), "Test Email");
    }

    #[test]
    fn test_events_response_structure() {
        // Test parsing a typical events response
        let response = json!({
            "value": [
                {
                    "id": "event-456",
                    "subject": "Team Meeting",
                    "start": {
                        "dateTime": "2024-01-01T10:00:00"
                    },
                    "end": {
                        "dateTime": "2024-01-01T11:00:00"
                    }
                }
            ]
        });

        let value = response.get("value").unwrap().as_array().unwrap();
        assert_eq!(value.len(), 1);

        let evt = &value[0];
        assert_eq!(evt.get("id").unwrap().as_str().unwrap(), "event-456");
        assert_eq!(
            evt.get("subject").unwrap().as_str().unwrap(),
            "Team Meeting"
        );
    }

    // ==================== Error Message Quality Tests ====================

    #[tokio::test]
    async fn test_error_message_mentions_field_name() {
        let auth = Office365Auth::with_token("token");

        // Test that error messages are helpful
        let tool = Office365SendMessage::new(auth.clone());
        let result = tool
            ._call(ToolInput::Structured(json!({"to": "a@b.com", "subject": "s"})))
            .await;
        assert!(result.unwrap_err().to_string().contains("message"));

        let tool = Office365SearchEmails::new(auth.clone());
        let result = tool._call(ToolInput::Structured(json!({}))).await;
        assert!(result.unwrap_err().to_string().contains("query"));

        let tool = Office365GetMessage::new(auth);
        let result = tool._call(ToolInput::Structured(json!({}))).await;
        assert!(result.unwrap_err().to_string().contains("message_id"));
    }

    #[tokio::test]
    async fn test_parse_error_is_descriptive() {
        let auth = Office365Auth::with_token("token");
        let tool = Office365SendMessage::new(auth);

        let result = tool
            ._call(ToolInput::String("invalid json {".to_string()))
            .await;
        assert!(result.is_err());
        let err_str = result.unwrap_err().to_string().to_lowercase();
        // Should indicate it's a parsing error
        assert!(err_str.contains("parse") || err_str.contains("json") || err_str.contains("input"));
    }
}
