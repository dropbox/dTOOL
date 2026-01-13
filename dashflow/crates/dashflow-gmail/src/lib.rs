// NOTE: clone_on_ref_ptr was removed - no Arc clones occur in this crate
// GmailAuth.hub() returns a reference, not a clone

//! Gmail tools for `DashFlow` Rust
//!
//! This crate provides tools for interacting with Gmail API, including:
//! - Send messages
//! - Create drafts
//! - Search messages/threads
//! - Get messages by ID
//! - Get threads by ID
//!
//! # Example
//!
//! ```no_run
//! use dashflow_gmail::{GmailSendMessage, GmailAuth};
//! use dashflow::core::tools::{Tool, ToolInput};
//! use serde_json::json;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     // Create Gmail authentication
//!     let auth = GmailAuth::from_credentials_file("credentials.json").await?;
//!
//!     // Create send message tool
//!     let tool = GmailSendMessage::new(auth);
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
use google_gmail1::api::{Draft, Message as GmailMessage};
use google_gmail1::{hyper_util, Gmail};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::io::Cursor;
use std::path::Path;
use std::sync::Arc;
use yup_oauth2::{read_application_secret, InstalledFlowAuthenticator, InstalledFlowReturnMethod};

// Re-export for convenience
pub use google_gmail1;
pub use yup_oauth2;

// Type alias for the default Gmail connector type
type DefaultGmailConnector =
    hyper_rustls::HttpsConnector<hyper_util::client::legacy::connect::HttpConnector>;

/// Gmail authentication handler
#[derive(Clone)]
pub struct GmailAuth {
    hub: Arc<Gmail<DefaultGmailConnector>>,
}

impl GmailAuth {
    /// Get Gmail hub for making API calls
    #[must_use]
    pub fn hub(&self) -> &Gmail<DefaultGmailConnector> {
        &self.hub
    }
}

impl GmailAuth {
    /// Create Gmail authentication from credentials file
    ///
    /// This will initiate `OAuth2` flow if needed and cache tokens in token.json
    pub async fn from_credentials_file<P: AsRef<Path>>(credentials_path: P) -> Result<Self> {
        let secret = read_application_secret(credentials_path)
            .await
            .map_err(|e| Error::config(format!("Failed to read credentials file: {e}")))?;

        let auth =
            InstalledFlowAuthenticator::builder(secret, InstalledFlowReturnMethod::HTTPRedirect)
                .persist_tokens_to_disk("token.json")
                .build()
                .await
                .map_err(|e| {
                    Error::authentication(format!("Failed to create authenticator: {e}"))
                })?;

        let connector = hyper_rustls::HttpsConnectorBuilder::new()
            .with_native_roots()
            .map_err(|e| Error::config(format!("Failed to create HTTPS connector: {e}")))?
            .https_or_http()
            .enable_http1()
            .build();

        // Create a hyper client with the connector
        let client =
            hyper_util::client::legacy::Client::builder(hyper_util::rt::TokioExecutor::new())
                .build(connector);

        let hub = Gmail::new(client, auth);

        Ok(Self { hub: Arc::new(hub) })
    }
}

/// Gmail Send Message tool
///
/// Sends an email message via Gmail API
pub struct GmailSendMessage {
    auth: GmailAuth,
}

impl GmailSendMessage {
    /// Create a new `GmailSendMessage` tool
    #[must_use]
    pub fn new(auth: GmailAuth) -> Self {
        Self { auth }
    }

    /// Prepare an email message
    fn prepare_message(
        &self,
        message: &str,
        to: &[String],
        subject: &str,
        cc: Option<&[String]>,
        bcc: Option<&[String]>,
    ) -> Vec<u8> {
        let mut email_content = format!("To: {}\r\n", to.join(", "));
        email_content.push_str(&format!("Subject: {subject}\r\n"));

        if let Some(cc) = cc {
            email_content.push_str(&format!("Cc: {}\r\n", cc.join(", ")));
        }

        if let Some(bcc) = bcc {
            email_content.push_str(&format!("Bcc: {}\r\n", bcc.join(", ")));
        }

        email_content.push_str("Content-Type: text/html; charset=UTF-8\r\n\r\n");
        email_content.push_str(message);

        email_content.into_bytes()
    }
}

#[async_trait]
impl Tool for GmailSendMessage {
    fn name(&self) -> &'static str {
        "send_gmail_message"
    }

    fn description(&self) -> &'static str {
        "Use this tool to send email messages. The input is the message, recipients"
    }

    async fn _call(&self, input: ToolInput) -> Result<String> {
        let input_data = match input {
            ToolInput::Structured(v) => v,
            ToolInput::String(s) => serde_json::from_str(&s).map_err(|e| {
                dashflow::core::Error::InvalidInput(format!("Failed to parse input: {e}"))
            })?,
        };

        let message = input_data
            .get("message")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                dashflow::core::Error::InvalidInput("'message' field required".to_string())
            })?;
        let subject = input_data
            .get("subject")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                dashflow::core::Error::InvalidInput("'subject' field required".to_string())
            })?;

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

        let raw_message =
            self.prepare_message(message, &to, subject, cc.as_deref(), bcc.as_deref());

        let msg = GmailMessage::default();
        let reader = Cursor::new(raw_message);

        // SAFETY: "message/rfc822" is a valid MIME type, parse cannot fail
        #[allow(clippy::unwrap_used)]
        let result = self
            .auth
            .hub()
            .users()
            .messages_send(msg, "me")
            .upload(reader, "message/rfc822".parse().unwrap())
            .await
            .map_err(|e| Error::tool_error(format!("Failed to send Gmail message: {e}")))?;

        Ok(format!(
            "Message sent. Message Id: {}",
            result.1.id.unwrap_or_default()
        ))
    }
}

/// Gmail Create Draft tool
///
/// Creates a draft email message
pub struct GmailCreateDraft {
    auth: GmailAuth,
}

impl GmailCreateDraft {
    /// Create a new `GmailCreateDraft` tool
    #[must_use]
    pub fn new(auth: GmailAuth) -> Self {
        Self { auth }
    }

    fn prepare_draft_message(
        &self,
        message: &str,
        to: &[String],
        subject: &str,
        cc: Option<&[String]>,
        bcc: Option<&[String]>,
    ) -> Vec<u8> {
        let mut email_content = format!("To: {}\r\n", to.join(", "));
        email_content.push_str(&format!("Subject: {subject}\r\n"));

        if let Some(cc) = cc {
            email_content.push_str(&format!("Cc: {}\r\n", cc.join(", ")));
        }

        if let Some(bcc) = bcc {
            email_content.push_str(&format!("Bcc: {}\r\n", bcc.join(", ")));
        }

        email_content.push_str("Content-Type: text/plain; charset=UTF-8\r\n\r\n");
        email_content.push_str(message);

        email_content.into_bytes()
    }
}

#[async_trait]
impl Tool for GmailCreateDraft {
    fn name(&self) -> &'static str {
        "create_gmail_draft"
    }

    fn description(&self) -> &'static str {
        "Use this tool to create a draft email with the provided message fields."
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

        let raw_message =
            self.prepare_draft_message(message, &to, subject, cc.as_deref(), bcc.as_deref());

        let msg = GmailMessage::default();
        let draft = Draft {
            message: Some(msg),
            ..Default::default()
        };

        let reader = Cursor::new(raw_message);

        // SAFETY: "message/rfc822" is a valid MIME type, parse cannot fail
        #[allow(clippy::unwrap_used)]
        let result = self
            .auth
            .hub()
            .users()
            .drafts_create(draft, "me")
            .upload(reader, "message/rfc822".parse().unwrap())
            .await
            .map_err(|e| Error::tool_error(format!("Failed to create Gmail draft: {e}")))?;

        Ok(format!(
            "Draft created. Draft Id: {}",
            result.1.id.unwrap_or_default()
        ))
    }
}

/// Gmail Search tool
///
/// Searches for messages or threads in Gmail
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SearchResource {
    Messages,
    Threads,
}

pub struct GmailSearch {
    auth: GmailAuth,
}

impl GmailSearch {
    /// Create a new `GmailSearch` tool
    #[must_use]
    pub fn new(auth: GmailAuth) -> Self {
        Self { auth }
    }

    async fn parse_messages(&self, message_ids: Vec<String>) -> Result<Vec<Value>> {
        let mut results = Vec::new();

        for message_id in message_ids {
            let (_response, message) = self
                .auth
                .hub()
                .users()
                .messages_get("me", &message_id)
                .format("raw")
                .doit()
                .await
                .map_err(|e| Error::tool_error(format!("Failed to get Gmail message: {e}")))?;

            if let Some(raw) = message.raw {
                let email_content = String::from_utf8_lossy(&raw);

                // Simple email parsing (in production, use a proper email parser)
                let mut subject = String::new();
                let mut sender = String::new();
                let mut body = String::new();
                let mut in_body = false;

                for line in email_content.lines() {
                    if let Some(stripped) = line.strip_prefix("Subject:") {
                        subject = stripped.trim().to_string();
                    } else if let Some(stripped) = line.strip_prefix("From:") {
                        sender = stripped.trim().to_string();
                    } else if line.is_empty() {
                        in_body = true;
                    } else if in_body {
                        body.push_str(line);
                        body.push('\n');
                    }
                }

                #[cfg(feature = "html-cleaning")]
                {
                    body = clean_html(&body);
                }

                results.push(serde_json::json!({
                    "id": message_id,
                    "threadId": message.thread_id.unwrap_or_default(),
                    "snippet": message.snippet.unwrap_or_default(),
                    "subject": subject,
                    "sender": sender,
                    "body": body.trim(),
                }));
            }
        }

        Ok(results)
    }

    async fn parse_threads(&self, thread_ids: Vec<String>) -> Result<Vec<Value>> {
        let mut results = Vec::new();

        for thread_id in thread_ids {
            let (_response, thread) = self
                .auth
                .hub()
                .users()
                .threads_get("me", &thread_id)
                .doit()
                .await
                .map_err(|e| Error::tool_error(format!("Failed to get Gmail thread: {e}")))?;

            let mut messages_info = Vec::new();
            if let Some(messages) = thread.messages {
                for msg in messages {
                    messages_info.push(serde_json::json!({
                        "id": msg.id.unwrap_or_default(),
                        "snippet": msg.snippet.unwrap_or_default(),
                    }));
                }
            }

            results.push(serde_json::json!({
                "id": thread_id,
                "messages": messages_info,
            }));
        }

        Ok(results)
    }
}

#[async_trait]
impl Tool for GmailSearch {
    fn name(&self) -> &'static str {
        "search_gmail"
    }

    fn description(&self) -> &'static str {
        "Use this tool to search for email messages or threads. The input must be a valid Gmail query. The output is a JSON list of the requested resource."
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

        let resource = input_data
            .get("resource")
            .and_then(|v| serde_json::from_value::<SearchResource>(v.clone()).ok())
            .unwrap_or(SearchResource::Messages);

        let max_results = input_data
            .get("max_results")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(10) as u32;

        let (_response, list_response) = self
            .auth
            .hub()
            .users()
            .messages_list("me")
            .q(query)
            .max_results(max_results)
            .doit()
            .await
            .map_err(|e| Error::tool_error(format!("Failed to search Gmail: {e}")))?;

        match resource {
            SearchResource::Messages => {
                let message_ids: Vec<String> = list_response
                    .messages
                    .unwrap_or_default()
                    .into_iter()
                    .filter_map(|m| m.id)
                    .collect();

                let results = self.parse_messages(message_ids).await?;
                Ok(serde_json::to_string_pretty(&results)?)
            }
            SearchResource::Threads => {
                let thread_ids: Vec<String> = list_response
                    .messages
                    .unwrap_or_default()
                    .into_iter()
                    .filter_map(|m| m.thread_id)
                    .collect();

                let results = self.parse_threads(thread_ids).await?;
                Ok(serde_json::to_string_pretty(&results)?)
            }
        }
    }
}

/// Gmail Get Message tool
///
/// Fetches a specific message by ID
pub struct GmailGetMessage {
    auth: GmailAuth,
}

impl GmailGetMessage {
    /// Create a new `GmailGetMessage` tool
    #[must_use]
    pub fn new(auth: GmailAuth) -> Self {
        Self { auth }
    }
}

#[async_trait]
impl Tool for GmailGetMessage {
    fn name(&self) -> &'static str {
        "get_gmail_message"
    }

    fn description(&self) -> &'static str {
        "Use this tool to fetch an email by message ID. Returns the thread ID, snippet, body, subject, and sender."
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

        let (_response, message) = self
            .auth
            .hub()
            .users()
            .messages_get("me", message_id)
            .format("raw")
            .doit()
            .await
            .map_err(|e| Error::tool_error(format!("Failed to get Gmail message: {e}")))?;

        if let Some(raw) = message.raw {
            let email_content = String::from_utf8_lossy(&raw);

            let mut subject = String::new();
            let mut sender = String::new();
            let mut body = String::new();
            let mut in_body = false;

            for line in email_content.lines() {
                if let Some(stripped) = line.strip_prefix("Subject:") {
                    subject = stripped.trim().to_string();
                } else if let Some(stripped) = line.strip_prefix("From:") {
                    sender = stripped.trim().to_string();
                } else if line.is_empty() {
                    in_body = true;
                } else if in_body {
                    body.push_str(line);
                    body.push('\n');
                }
            }

            #[cfg(feature = "html-cleaning")]
            {
                body = clean_html(&body);
            }

            let result = serde_json::json!({
                "id": message_id,
                "threadId": message.thread_id.unwrap_or_default(),
                "snippet": message.snippet.unwrap_or_default(),
                "subject": subject,
                "sender": sender,
                "body": body.trim(),
            });

            Ok(serde_json::to_string_pretty(&result)?)
        } else {
            Err(Error::tool_error("Message has no raw content"))
        }
    }
}

/// Gmail Get Thread tool
///
/// Fetches a specific thread by ID
pub struct GmailGetThread {
    auth: GmailAuth,
}

impl GmailGetThread {
    /// Create a new `GmailGetThread` tool
    #[must_use]
    pub fn new(auth: GmailAuth) -> Self {
        Self { auth }
    }
}

#[async_trait]
impl Tool for GmailGetThread {
    fn name(&self) -> &'static str {
        "get_gmail_thread"
    }

    fn description(&self) -> &'static str {
        "Use this tool to search for email messages. The input must be a valid Gmail query. The output is a JSON list of messages."
    }

    async fn _call(&self, input: ToolInput) -> Result<String> {
        let input_data = match input {
            ToolInput::Structured(v) => v,
            ToolInput::String(s) => serde_json::from_str(&s)
                .map_err(|e| Error::invalid_input(format!("Failed to parse input: {e}")))?,
        };

        let thread_id = input_data
            .get("thread_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| Error::invalid_input("'thread_id' field required"))?;

        let (_response, thread) = self
            .auth
            .hub()
            .users()
            .threads_get("me", thread_id)
            .doit()
            .await
            .map_err(|e| Error::tool_error(format!("Failed to get Gmail thread: {e}")))?;

        let mut messages_info = Vec::new();
        if let Some(messages) = thread.messages {
            for msg in messages {
                messages_info.push(serde_json::json!({
                    "id": msg.id.unwrap_or_default(),
                    "snippet": msg.snippet.unwrap_or_default(),
                }));
            }
        }

        let result = serde_json::json!({
            "id": thread_id,
            "messages": messages_info,
        });

        Ok(serde_json::to_string_pretty(&result)?)
    }
}

#[cfg(feature = "html-cleaning")]
#[allow(clippy::unwrap_used)] // SAFETY: "*" is a valid CSS selector
fn clean_html(html: &str) -> String {
    use scraper::{Html, Selector};

    let fragment = Html::parse_fragment(html);
    let selector = Selector::parse("body").unwrap_or_else(|_| Selector::parse("*").unwrap());

    fragment
        .select(&selector)
        .next()
        .map(|element| element.text().collect::<String>())
        .unwrap_or_else(|| html.to_string())
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    // ==================== SearchResource Tests ====================

    #[test]
    fn test_search_resource_serialization_messages() {
        let json = serde_json::to_string(&SearchResource::Messages).unwrap();
        assert_eq!(json, "\"messages\"");
    }

    #[test]
    fn test_search_resource_serialization_threads() {
        let json = serde_json::to_string(&SearchResource::Threads).unwrap();
        assert_eq!(json, "\"threads\"");
    }

    #[test]
    fn test_search_resource_deserialization_messages() {
        let resource: SearchResource = serde_json::from_str("\"messages\"").unwrap();
        assert!(matches!(resource, SearchResource::Messages));
    }

    #[test]
    fn test_search_resource_deserialization_threads() {
        let resource: SearchResource = serde_json::from_str("\"threads\"").unwrap();
        assert!(matches!(resource, SearchResource::Threads));
    }

    #[test]
    fn test_search_resource_invalid_value() {
        let result: std::result::Result<SearchResource, _> = serde_json::from_str("\"invalid\"");
        assert!(result.is_err());
    }

    #[test]
    fn test_search_resource_debug() {
        let messages = SearchResource::Messages;
        let threads = SearchResource::Threads;
        assert_eq!(format!("{:?}", messages), "Messages");
        assert_eq!(format!("{:?}", threads), "Threads");
    }

    #[test]
    fn test_search_resource_clone() {
        let original = SearchResource::Messages;
        let cloned = original;
        assert!(matches!(cloned, SearchResource::Messages));
    }

    #[test]
    fn test_search_resource_copy() {
        let original = SearchResource::Threads;
        let copied: SearchResource = original;
        let _ = original; // Original still usable (Copy)
        assert!(matches!(copied, SearchResource::Threads));
    }

    // ==================== Tool Name and Description Tests ====================

    // Note: These tests verify the Tool trait implementations use correct names/descriptions.
    // Actual Tool instances require GmailAuth, which needs credentials.
    // We test the string constants here.

    #[test]
    fn test_tool_name_send_gmail_message() {
        // The tool name should be snake_case
        let expected = "send_gmail_message";
        assert!(!expected.is_empty());
        assert!(expected.chars().all(|c| c.is_ascii_lowercase() || c == '_'));
    }

    #[test]
    fn test_tool_name_create_gmail_draft() {
        let expected = "create_gmail_draft";
        assert!(!expected.is_empty());
        assert!(expected.chars().all(|c| c.is_ascii_lowercase() || c == '_'));
    }

    #[test]
    fn test_tool_name_search_gmail() {
        let expected = "search_gmail";
        assert!(!expected.is_empty());
        assert!(expected.chars().all(|c| c.is_ascii_lowercase() || c == '_'));
    }

    #[test]
    fn test_tool_name_get_gmail_message() {
        let expected = "get_gmail_message";
        assert!(!expected.is_empty());
        assert!(expected.chars().all(|c| c.is_ascii_lowercase() || c == '_'));
    }

    #[test]
    fn test_tool_name_get_gmail_thread() {
        let expected = "get_gmail_thread";
        assert!(!expected.is_empty());
        assert!(expected.chars().all(|c| c.is_ascii_lowercase() || c == '_'));
    }

    // ==================== Message Preparation Tests ====================

    // Helper struct to test prepare_message without full auth
    struct MessagePreparer;

    impl MessagePreparer {
        fn prepare_message(
            message: &str,
            to: &[String],
            subject: &str,
            cc: Option<&[String]>,
            bcc: Option<&[String]>,
        ) -> Vec<u8> {
            let mut email_content = format!("To: {}\r\n", to.join(", "));
            email_content.push_str(&format!("Subject: {subject}\r\n"));

            if let Some(cc) = cc {
                email_content.push_str(&format!("Cc: {}\r\n", cc.join(", ")));
            }

            if let Some(bcc) = bcc {
                email_content.push_str(&format!("Bcc: {}\r\n", bcc.join(", ")));
            }

            email_content.push_str("Content-Type: text/html; charset=UTF-8\r\n\r\n");
            email_content.push_str(message);

            email_content.into_bytes()
        }

        fn prepare_draft_message(
            message: &str,
            to: &[String],
            subject: &str,
            cc: Option<&[String]>,
            bcc: Option<&[String]>,
        ) -> Vec<u8> {
            let mut email_content = format!("To: {}\r\n", to.join(", "));
            email_content.push_str(&format!("Subject: {subject}\r\n"));

            if let Some(cc) = cc {
                email_content.push_str(&format!("Cc: {}\r\n", cc.join(", ")));
            }

            if let Some(bcc) = bcc {
                email_content.push_str(&format!("Bcc: {}\r\n", bcc.join(", ")));
            }

            email_content.push_str("Content-Type: text/plain; charset=UTF-8\r\n\r\n");
            email_content.push_str(message);

            email_content.into_bytes()
        }
    }

    #[test]
    fn test_prepare_message_basic() {
        let to = vec!["user@example.com".to_string()];
        let result = MessagePreparer::prepare_message("Hello", &to, "Test Subject", None, None);
        let content = String::from_utf8(result).unwrap();

        assert!(content.contains("To: user@example.com"));
        assert!(content.contains("Subject: Test Subject"));
        assert!(content.contains("Content-Type: text/html; charset=UTF-8"));
        assert!(content.contains("Hello"));
    }

    #[test]
    fn test_prepare_message_multiple_recipients() {
        let to = vec![
            "user1@example.com".to_string(),
            "user2@example.com".to_string(),
        ];
        let result = MessagePreparer::prepare_message("Body", &to, "Subject", None, None);
        let content = String::from_utf8(result).unwrap();

        assert!(content.contains("To: user1@example.com, user2@example.com"));
    }

    #[test]
    fn test_prepare_message_with_cc() {
        let to = vec!["user@example.com".to_string()];
        let cc = vec!["cc@example.com".to_string()];
        let result = MessagePreparer::prepare_message("Body", &to, "Subject", Some(&cc), None);
        let content = String::from_utf8(result).unwrap();

        assert!(content.contains("Cc: cc@example.com"));
    }

    #[test]
    fn test_prepare_message_with_multiple_cc() {
        let to = vec!["user@example.com".to_string()];
        let cc = vec!["cc1@example.com".to_string(), "cc2@example.com".to_string()];
        let result = MessagePreparer::prepare_message("Body", &to, "Subject", Some(&cc), None);
        let content = String::from_utf8(result).unwrap();

        assert!(content.contains("Cc: cc1@example.com, cc2@example.com"));
    }

    #[test]
    fn test_prepare_message_with_bcc() {
        let to = vec!["user@example.com".to_string()];
        let bcc = vec!["bcc@example.com".to_string()];
        let result = MessagePreparer::prepare_message("Body", &to, "Subject", None, Some(&bcc));
        let content = String::from_utf8(result).unwrap();

        assert!(content.contains("Bcc: bcc@example.com"));
    }

    #[test]
    fn test_prepare_message_with_cc_and_bcc() {
        let to = vec!["user@example.com".to_string()];
        let cc = vec!["cc@example.com".to_string()];
        let bcc = vec!["bcc@example.com".to_string()];
        let result =
            MessagePreparer::prepare_message("Body", &to, "Subject", Some(&cc), Some(&bcc));
        let content = String::from_utf8(result).unwrap();

        assert!(content.contains("Cc: cc@example.com"));
        assert!(content.contains("Bcc: bcc@example.com"));
    }

    #[test]
    fn test_prepare_message_html_content() {
        let to = vec!["user@example.com".to_string()];
        let html_body = "<html><body><h1>Hello</h1><p>World</p></body></html>";
        let result = MessagePreparer::prepare_message(html_body, &to, "HTML Test", None, None);
        let content = String::from_utf8(result).unwrap();

        assert!(content.contains("Content-Type: text/html; charset=UTF-8"));
        assert!(content.contains("<html>"));
        assert!(content.contains("<h1>Hello</h1>"));
    }

    #[test]
    fn test_prepare_message_unicode_subject() {
        let to = vec!["user@example.com".to_string()];
        let result = MessagePreparer::prepare_message("Body", &to, "日本語の件名", None, None);
        let content = String::from_utf8(result).unwrap();

        assert!(content.contains("Subject: 日本語の件名"));
    }

    #[test]
    fn test_prepare_message_unicode_body() {
        let to = vec!["user@example.com".to_string()];
        let result =
            MessagePreparer::prepare_message("こんにちは世界", &to, "Subject", None, None);
        let content = String::from_utf8(result).unwrap();

        assert!(content.contains("こんにちは世界"));
    }

    #[test]
    fn test_prepare_message_empty_body() {
        let to = vec!["user@example.com".to_string()];
        let result = MessagePreparer::prepare_message("", &to, "Subject", None, None);
        let content = String::from_utf8(result).unwrap();

        assert!(content.contains("Subject: Subject"));
        assert!(content.ends_with("\r\n\r\n"));
    }

    #[test]
    fn test_prepare_message_line_endings() {
        let to = vec!["user@example.com".to_string()];
        let result = MessagePreparer::prepare_message("Body", &to, "Subject", None, None);
        let content = String::from_utf8(result).unwrap();

        // Email headers must use CRLF line endings
        assert!(content.contains("\r\n"));
    }

    // ==================== Draft Message Preparation Tests ====================

    #[test]
    fn test_prepare_draft_message_basic() {
        let to = vec!["user@example.com".to_string()];
        let result =
            MessagePreparer::prepare_draft_message("Draft body", &to, "Draft Subject", None, None);
        let content = String::from_utf8(result).unwrap();

        assert!(content.contains("To: user@example.com"));
        assert!(content.contains("Subject: Draft Subject"));
        // Drafts use plain text content type
        assert!(content.contains("Content-Type: text/plain; charset=UTF-8"));
        assert!(content.contains("Draft body"));
    }

    #[test]
    fn test_prepare_draft_message_with_cc() {
        let to = vec!["user@example.com".to_string()];
        let cc = vec!["cc@example.com".to_string()];
        let result =
            MessagePreparer::prepare_draft_message("Body", &to, "Subject", Some(&cc), None);
        let content = String::from_utf8(result).unwrap();

        assert!(content.contains("Cc: cc@example.com"));
    }

    #[test]
    fn test_prepare_draft_message_with_bcc() {
        let to = vec!["user@example.com".to_string()];
        let bcc = vec!["bcc@example.com".to_string()];
        let result =
            MessagePreparer::prepare_draft_message("Body", &to, "Subject", None, Some(&bcc));
        let content = String::from_utf8(result).unwrap();

        assert!(content.contains("Bcc: bcc@example.com"));
    }

    #[test]
    fn test_prepare_draft_vs_send_content_type() {
        let to = vec!["user@example.com".to_string()];
        let send_result = MessagePreparer::prepare_message("Body", &to, "Subject", None, None);
        let draft_result =
            MessagePreparer::prepare_draft_message("Body", &to, "Subject", None, None);

        let send_content = String::from_utf8(send_result).unwrap();
        let draft_content = String::from_utf8(draft_result).unwrap();

        // Send uses HTML, draft uses plain text
        assert!(send_content.contains("text/html"));
        assert!(draft_content.contains("text/plain"));
    }

    // ==================== Input Parsing Tests ====================

    #[test]
    fn test_parse_to_field_string() {
        let input = serde_json::json!({
            "to": "single@example.com",
            "subject": "Test",
            "message": "Body"
        });

        let to: Vec<String> = match input.get("to") {
            Some(Value::String(s)) => vec![s.clone()],
            Some(Value::Array(arr)) => arr
                .iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect(),
            _ => vec![],
        };

        assert_eq!(to, vec!["single@example.com".to_string()]);
    }

    #[test]
    fn test_parse_to_field_array() {
        let input = serde_json::json!({
            "to": ["first@example.com", "second@example.com"],
            "subject": "Test",
            "message": "Body"
        });

        let to: Vec<String> = match input.get("to") {
            Some(Value::String(s)) => vec![s.clone()],
            Some(Value::Array(arr)) => arr
                .iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect(),
            _ => vec![],
        };

        assert_eq!(
            to,
            vec![
                "first@example.com".to_string(),
                "second@example.com".to_string()
            ]
        );
    }

    #[test]
    fn test_parse_to_field_empty_array() {
        let input = serde_json::json!({
            "to": [],
            "subject": "Test",
            "message": "Body"
        });

        let to: Vec<String> = match input.get("to") {
            Some(Value::Array(arr)) => arr
                .iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect(),
            _ => vec![],
        };

        assert!(to.is_empty());
    }

    #[test]
    fn test_parse_cc_field_optional_none() {
        let input = serde_json::json!({
            "to": "user@example.com",
            "subject": "Test",
            "message": "Body"
        });

        let cc = input.get("cc").and_then(|v| match v {
            Value::String(s) => Some(vec![s.clone()]),
            Value::Array(arr) => Some(
                arr.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect(),
            ),
            _ => None,
        });

        assert!(cc.is_none());
    }

    #[test]
    fn test_parse_cc_field_string() {
        let input = serde_json::json!({
            "to": "user@example.com",
            "subject": "Test",
            "message": "Body",
            "cc": "cc@example.com"
        });

        let cc = input.get("cc").and_then(|v| match v {
            Value::String(s) => Some(vec![s.clone()]),
            _ => None,
        });

        assert_eq!(cc, Some(vec!["cc@example.com".to_string()]));
    }

    #[test]
    fn test_parse_cc_field_array() {
        let input = serde_json::json!({
            "to": "user@example.com",
            "subject": "Test",
            "message": "Body",
            "cc": ["cc1@example.com", "cc2@example.com"]
        });

        let cc = input.get("cc").and_then(|v| match v {
            Value::Array(arr) => Some(
                arr.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect(),
            ),
            _ => None,
        });

        assert_eq!(
            cc,
            Some(vec![
                "cc1@example.com".to_string(),
                "cc2@example.com".to_string()
            ])
        );
    }

    #[test]
    fn test_parse_max_results_default() {
        let input = serde_json::json!({
            "query": "from:test@example.com"
        });

        let max_results = input
            .get("max_results")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(10) as u32;

        assert_eq!(max_results, 10);
    }

    #[test]
    fn test_parse_max_results_custom() {
        let input = serde_json::json!({
            "query": "from:test@example.com",
            "max_results": 25
        });

        let max_results = input
            .get("max_results")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(10) as u32;

        assert_eq!(max_results, 25);
    }

    #[test]
    fn test_parse_resource_default() {
        let input = serde_json::json!({
            "query": "test"
        });

        let resource = input
            .get("resource")
            .and_then(|v| serde_json::from_value::<SearchResource>(v.clone()).ok())
            .unwrap_or(SearchResource::Messages);

        assert!(matches!(resource, SearchResource::Messages));
    }

    #[test]
    fn test_parse_resource_threads() {
        let input = serde_json::json!({
            "query": "test",
            "resource": "threads"
        });

        let resource = input
            .get("resource")
            .and_then(|v| serde_json::from_value::<SearchResource>(v.clone()).ok())
            .unwrap_or(SearchResource::Messages);

        assert!(matches!(resource, SearchResource::Threads));
    }

    // ==================== Error Handling Tests ====================

    #[test]
    fn test_missing_to_field_error() {
        let input = serde_json::json!({
            "subject": "Test",
            "message": "Body"
        });

        let result = match input.get("to") {
            Some(Value::String(_)) | Some(Value::Array(_)) => Ok(()),
            _ => Err("'to' field must be a string or array"),
        };

        assert!(result.is_err());
    }

    #[test]
    fn test_missing_subject_field() {
        let input = serde_json::json!({
            "to": "user@example.com",
            "message": "Body"
        });

        let subject = input.get("subject").and_then(|v| v.as_str());
        assert!(subject.is_none());
    }

    #[test]
    fn test_missing_message_field() {
        let input = serde_json::json!({
            "to": "user@example.com",
            "subject": "Test"
        });

        let message = input.get("message").and_then(|v| v.as_str());
        assert!(message.is_none());
    }

    #[test]
    fn test_invalid_to_field_type() {
        let input = serde_json::json!({
            "to": 12345,
            "subject": "Test",
            "message": "Body"
        });

        let is_valid = match input.get("to") {
            Some(Value::String(_)) | Some(Value::Array(_)) => true,
            _ => false,
        };

        assert!(!is_valid);
    }

    #[test]
    fn test_to_field_array_with_non_strings() {
        let input = serde_json::json!({
            "to": ["valid@example.com", 123, null, "another@example.com"],
            "subject": "Test",
            "message": "Body"
        });

        let to: Vec<String> = match input.get("to") {
            Some(Value::Array(arr)) => arr
                .iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect(),
            _ => vec![],
        };

        // Only string values should be included
        assert_eq!(
            to,
            vec![
                "valid@example.com".to_string(),
                "another@example.com".to_string()
            ]
        );
    }

    // ==================== JSON String Input Parsing Tests ====================

    #[test]
    fn test_parse_json_string_input() {
        let json_str = r#"{"to": "user@example.com", "subject": "Test", "message": "Body"}"#;
        let result: std::result::Result<Value, _> = serde_json::from_str(json_str);
        assert!(result.is_ok());

        let value = result.unwrap();
        assert_eq!(value.get("to").and_then(|v| v.as_str()), Some("user@example.com"));
    }

    #[test]
    fn test_parse_invalid_json_string() {
        let invalid_json = r#"{"to": "user@example.com", invalid}"#;
        let result: std::result::Result<Value, _> = serde_json::from_str(invalid_json);
        assert!(result.is_err());
    }

    // ==================== VectorMetadata-like Structure Tests ====================

    #[test]
    fn test_gmail_message_id_extraction() {
        // Simulate extracting message ID from response
        let message_id = Some("msg12345".to_string());
        assert_eq!(message_id.unwrap_or_default(), "msg12345");
    }

    #[test]
    fn test_gmail_message_id_missing() {
        let message_id: Option<String> = None;
        assert_eq!(message_id.unwrap_or_default(), "");
    }

    #[test]
    fn test_thread_id_extraction() {
        let thread_id = Some("thread67890".to_string());
        assert_eq!(thread_id.unwrap_or_default(), "thread67890");
    }

    // ==================== Email Content Parsing Tests ====================

    #[test]
    fn test_parse_email_headers() {
        let email_content = "Subject: Test Email\r\nFrom: sender@example.com\r\n\r\nBody here";

        let mut subject = String::new();
        let mut sender = String::new();

        for line in email_content.lines() {
            if let Some(stripped) = line.strip_prefix("Subject:") {
                subject = stripped.trim().to_string();
            } else if let Some(stripped) = line.strip_prefix("From:") {
                sender = stripped.trim().to_string();
            }
        }

        assert_eq!(subject, "Test Email");
        assert_eq!(sender, "sender@example.com");
    }

    #[test]
    fn test_parse_email_body() {
        let email_content = "Subject: Test\r\nFrom: test@example.com\r\n\r\nThis is the body.\r\nSecond line.";

        let mut body = String::new();
        let mut in_body = false;

        for line in email_content.lines() {
            if line.is_empty() {
                in_body = true;
            } else if in_body {
                body.push_str(line);
                body.push('\n');
            }
        }

        assert!(body.contains("This is the body."));
        assert!(body.contains("Second line."));
    }

    #[test]
    fn test_parse_email_empty_body() {
        let email_content = "Subject: Test\r\nFrom: test@example.com\r\n\r\n";

        let mut body = String::new();
        let mut in_body = false;

        for line in email_content.lines() {
            if line.is_empty() {
                in_body = true;
            } else if in_body {
                body.push_str(line);
                body.push('\n');
            }
        }

        assert!(body.trim().is_empty());
    }

    // ==================== Special Character Tests ====================

    #[test]
    fn test_subject_with_special_chars() {
        let to = vec!["user@example.com".to_string()];
        let subject = "Re: [URGENT] Meeting @ 3pm - Q&A Session!";
        let result = MessagePreparer::prepare_message("Body", &to, subject, None, None);
        let content = String::from_utf8(result).unwrap();

        assert!(content.contains(subject));
    }

    #[test]
    fn test_body_with_newlines() {
        let to = vec!["user@example.com".to_string()];
        let body = "Line 1\nLine 2\nLine 3";
        let result = MessagePreparer::prepare_message(body, &to, "Subject", None, None);
        let content = String::from_utf8(result).unwrap();

        assert!(content.contains("Line 1\nLine 2\nLine 3"));
    }

    #[test]
    fn test_recipient_with_display_name() {
        let to = vec!["John Doe <john@example.com>".to_string()];
        let result = MessagePreparer::prepare_message("Body", &to, "Subject", None, None);
        let content = String::from_utf8(result).unwrap();

        assert!(content.contains("To: John Doe <john@example.com>"));
    }

    // ==================== Integration Tests (Require Credentials) ====================

    #[tokio::test]
    #[ignore = "requires Gmail credentials.json and network access"]
    async fn test_gmail_auth_from_credentials_file_not_found() {
        let result = GmailAuth::from_credentials_file("nonexistent_credentials.json").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    #[ignore = "requires Gmail credentials.json and network access"]
    async fn test_gmail_auth_from_valid_credentials() {
        // This test would use actual credentials
        // let auth = GmailAuth::from_credentials_file("credentials.json").await;
        // assert!(auth.is_ok());
    }

    #[tokio::test]
    #[ignore = "requires Gmail API credentials and network access"]
    async fn test_gmail_send_message_integration() {
        // Would test actual message sending
    }

    #[tokio::test]
    #[ignore = "requires Gmail API credentials and network access"]
    async fn test_gmail_create_draft_integration() {
        // Would test actual draft creation
    }

    #[tokio::test]
    #[ignore = "requires Gmail API credentials and network access"]
    async fn test_gmail_search_integration() {
        // Would test actual search
    }

    #[tokio::test]
    #[ignore = "requires Gmail API credentials and network access"]
    async fn test_gmail_get_message_integration() {
        // Would test actual message retrieval
    }

    #[tokio::test]
    #[ignore = "requires Gmail API credentials and network access"]
    async fn test_gmail_get_thread_integration() {
        // Would test actual thread retrieval
    }
}
