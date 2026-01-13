//! DynamoDB-based chat message history storage
//!
//! Stores chat message history in AWS DynamoDB. Messages are serialized to JSON
//! and stored in a DynamoDB table with configurable primary key and TTL support.
//!
//! # Overview
//!
//! The DynamoDB backend uses:
//! - **DynamoDB table** for message storage
//! - **JSON serialization** for message persistence
//! - **Session-based keys** (configurable primary key name)
//! - **Optional TTL** for automatic item expiration
//! - **History size limit** for trimming old messages
//!
//! # Usage
//!
//! ```rust,ignore
//! use dashflow_memory::DynamoDBChatMessageHistory;
//! use dashflow::core::chat_history::BaseChatMessageHistory;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
//! // Create DynamoDB history with default settings
//! let history = DynamoDBChatMessageHistory::builder()
//!     .table_name("chat_history")
//!     .session_id("session-123")
//!     .build()
//!     .await?;
//!
//! // Add messages
//! history.add_user_message("Hello!").await?;
//! history.add_ai_message("Hi there!").await?;
//!
//! // Retrieve messages
//! let messages = history.get_messages().await?;
//! assert_eq!(messages.len(), 2);
//!
//! // Clear history
//! history.clear().await?;
//! # Ok(())
//! # }
//! ```
//!
//! # Python Baseline Compatibility
//!
//! Matches `DynamoDBChatMessageHistory` from
//! `dashflow_community.chat_message_histories.dynamodb:27-179`.
//!
//! Key features:
//! - Stores messages in DynamoDB table with configurable primary key
//! - Supports optional TTL for automatic expiration
//! - Supports history size limit (keeps only latest N messages)
//! - Uses update_item with SET expression for atomic updates
//! - Supports custom key schemas (composite keys, GSI/LSI keys)

use async_trait::async_trait;
use aws_sdk_dynamodb::types::AttributeValue;
use aws_sdk_dynamodb::Client;
use dashflow::core::chat_history::BaseChatMessageHistory;
use dashflow::core::messages::Message;
use serde_json;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

/// DynamoDB-based chat message history implementation.
///
/// Stores chat messages in a DynamoDB table, with each session identified by
/// a configurable primary key. Messages are serialized as JSON and stored in
/// a configurable attribute (default: "History").
///
/// # Storage Format
///
/// - **Primary Key**: Configurable (default: "SessionId" = session_id)
/// - **Message Attribute**: Configurable (default: "History")
/// - **Message Format**: JSON-serialized list of Message objects
/// - **TTL Attribute**: Configurable (default: "expireAt")
///
/// # Features
///
/// - **Session-based**: Each session_id gets its own DynamoDB item
/// - **TTL support**: Optional expiration time for automatic cleanup
/// - **History size limit**: Optional limit to keep only latest N messages
/// - **Custom keys**: Support for composite keys and secondary indexes
/// - **JSON serialization**: Uses serde_json for Message serialization
/// - **Thread-safe**: Uses Arc for safe concurrent access
#[derive(Clone)]
pub struct DynamoDBChatMessageHistory {
    client: Arc<Client>,
    table_name: String,
    key: HashMap<String, AttributeValue>,
    ttl: Option<u64>,
    ttl_key_name: String,
    history_size: Option<usize>,
    history_messages_key: String,
}

impl DynamoDBChatMessageHistory {
    /// Creates a new builder for DynamoDBChatMessageHistory
    pub fn builder() -> DynamoDBChatMessageHistoryBuilder {
        DynamoDBChatMessageHistoryBuilder::default()
    }

    /// Get current Unix timestamp in seconds
    fn current_timestamp() -> u64 {
        match SystemTime::now().duration_since(UNIX_EPOCH) {
            Ok(duration) => duration.as_secs(),
            Err(_) => 0,
        }
    }

    /// Read messages from DynamoDB
    async fn read_messages(
        &self,
    ) -> Result<Vec<Message>, Box<dyn std::error::Error + Send + Sync>> {
        let response = self
            .client
            .get_item()
            .table_name(&self.table_name)
            .set_key(Some(self.key.clone()))
            .send()
            .await?;

        if let Some(item) = response.item {
            if let Some(AttributeValue::L(messages_list)) = item.get(&self.history_messages_key) {
                let mut messages = Vec::new();
                for attr_val in messages_list {
                    if let AttributeValue::S(json_str) = attr_val {
                        let message: Message = serde_json::from_str(json_str)?;
                        messages.push(message);
                    }
                }
                return Ok(messages);
            }
        }

        Ok(Vec::new())
    }

    /// Write messages to DynamoDB
    async fn write_messages(
        &self,
        messages: &[Message],
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Serialize messages to AttributeValue list
        let mut messages_list = Vec::with_capacity(messages.len());
        for msg in messages {
            let json_str = serde_json::to_string(msg)?;
            messages_list.push(AttributeValue::S(json_str));
        }

        let messages_attr = AttributeValue::L(messages_list);

        // Build update expression based on TTL configuration
        if let Some(ttl_seconds) = self.ttl {
            let expire_at = Self::current_timestamp() + ttl_seconds;

            self.client
                .update_item()
                .table_name(&self.table_name)
                .set_key(Some(self.key.clone()))
                .update_expression(format!(
                    "SET {} = :h, {} = :t",
                    self.history_messages_key, self.ttl_key_name
                ))
                .expression_attribute_values(":h", messages_attr)
                .expression_attribute_values(":t", AttributeValue::N(expire_at.to_string()))
                .send()
                .await?;
        } else {
            self.client
                .update_item()
                .table_name(&self.table_name)
                .set_key(Some(self.key.clone()))
                .update_expression(format!("SET {} = :h", self.history_messages_key))
                .expression_attribute_values(":h", messages_attr)
                .send()
                .await?;
        }

        Ok(())
    }
}

#[async_trait]
impl BaseChatMessageHistory for DynamoDBChatMessageHistory {
    async fn add_messages(
        &self,
        messages: &[Message],
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Read existing messages
        let mut existing_messages = self.read_messages().await?;

        // Append new messages
        existing_messages.extend_from_slice(messages);

        // Apply history size limit if configured
        let final_messages = if let Some(limit) = self.history_size {
            if existing_messages.len() > limit {
                existing_messages.split_off(existing_messages.len() - limit)
            } else {
                existing_messages
            }
        } else {
            existing_messages
        };

        // Write back to DynamoDB
        self.write_messages(&final_messages).await
    }

    async fn get_messages(&self) -> Result<Vec<Message>, Box<dyn std::error::Error + Send + Sync>> {
        self.read_messages().await
    }

    async fn clear(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.client
            .delete_item()
            .table_name(&self.table_name)
            .set_key(Some(self.key.clone()))
            .send()
            .await?;

        Ok(())
    }
}

/// Builder for DynamoDBChatMessageHistory
#[derive(Default)]
pub struct DynamoDBChatMessageHistoryBuilder {
    table_name: Option<String>,
    session_id: Option<String>,
    endpoint_url: Option<String>,
    primary_key_name: Option<String>,
    custom_key: Option<HashMap<String, String>>,
    ttl: Option<u64>,
    ttl_key_name: Option<String>,
    history_size: Option<usize>,
    history_messages_key: Option<String>,
}

impl DynamoDBChatMessageHistoryBuilder {
    /// Set the DynamoDB table name (required)
    pub fn table_name(mut self, table_name: impl Into<String>) -> Self {
        self.table_name = Some(table_name.into());
        self
    }

    /// Set the session ID (required)
    pub fn session_id(mut self, session_id: impl Into<String>) -> Self {
        self.session_id = Some(session_id.into());
        self
    }

    /// Set a custom endpoint URL (useful for testing with LocalStack)
    pub fn endpoint_url(mut self, endpoint_url: impl Into<String>) -> Self {
        self.endpoint_url = Some(endpoint_url.into());
        self
    }

    /// Set the primary key name (default: "SessionId")
    pub fn primary_key_name(mut self, primary_key_name: impl Into<String>) -> Self {
        self.primary_key_name = Some(primary_key_name.into());
        self
    }

    /// Set a custom key (for composite keys or secondary indexes)
    pub fn custom_key(mut self, key: HashMap<String, String>) -> Self {
        self.custom_key = Some(key);
        self
    }

    /// Set TTL in seconds (items will expire after this duration)
    pub fn ttl(mut self, ttl: u64) -> Self {
        self.ttl = Some(ttl);
        self
    }

    /// Set the TTL attribute name (default: "expireAt")
    pub fn ttl_key_name(mut self, ttl_key_name: impl Into<String>) -> Self {
        self.ttl_key_name = Some(ttl_key_name.into());
        self
    }

    /// Set the maximum number of messages to keep (oldest are trimmed)
    pub fn history_size(mut self, history_size: usize) -> Self {
        self.history_size = Some(history_size);
        self
    }

    /// Set the attribute name for message history (default: "History")
    pub fn history_messages_key(mut self, history_messages_key: impl Into<String>) -> Self {
        self.history_messages_key = Some(history_messages_key.into());
        self
    }

    /// Build the DynamoDBChatMessageHistory
    pub async fn build(
        self,
    ) -> Result<DynamoDBChatMessageHistory, Box<dyn std::error::Error + Send + Sync>> {
        let table_name = self.table_name.ok_or("table_name is required")?;
        let session_id = self.session_id.ok_or("session_id is required")?;

        // Load AWS config
        let config = if let Some(endpoint) = self.endpoint_url {
            aws_config::defaults(aws_config::BehaviorVersion::latest())
                .endpoint_url(endpoint)
                .load()
                .await
        } else {
            aws_config::load_defaults(aws_config::BehaviorVersion::latest()).await
        };

        let client = Client::new(&config);

        // Build the key
        let key = if let Some(custom_key) = self.custom_key {
            custom_key
                .into_iter()
                .map(|(k, v)| (k, AttributeValue::S(v)))
                .collect()
        } else {
            let primary_key_name = self
                .primary_key_name
                .unwrap_or_else(|| "SessionId".to_string());
            let mut key = HashMap::new();
            key.insert(primary_key_name, AttributeValue::S(session_id.clone()));
            key
        };

        Ok(DynamoDBChatMessageHistory {
            client: Arc::new(client),
            table_name,
            key,
            ttl: self.ttl,
            ttl_key_name: self.ttl_key_name.unwrap_or_else(|| "expireAt".to_string()),
            history_size: self.history_size,
            history_messages_key: self
                .history_messages_key
                .unwrap_or_else(|| "History".to_string()),
        })
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use dashflow::core::messages::Message;

    // Note: These tests require a DynamoDB instance (LocalStack or AWS)
    // To run: docker run -p 4566:4566 localstack/localstack
    // Then: AWS_ACCESS_KEY_ID=test AWS_SECRET_ACCESS_KEY=test cargo test --features dynamodb-backend

    async fn create_test_table(client: &Client, table_name: &str) {
        use aws_sdk_dynamodb::types::{
            AttributeDefinition, KeySchemaElement, KeyType, ProvisionedThroughput,
            ScalarAttributeType,
        };

        // Try to delete table if it exists
        let _ = client.delete_table().table_name(table_name).send().await;

        // Wait a bit for deletion
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

        // Create table
        client
            .create_table()
            .table_name(table_name)
            .key_schema(
                KeySchemaElement::builder()
                    .attribute_name("SessionId")
                    .key_type(KeyType::Hash)
                    .build()
                    .unwrap(),
            )
            .attribute_definitions(
                AttributeDefinition::builder()
                    .attribute_name("SessionId")
                    .attribute_type(ScalarAttributeType::S)
                    .build()
                    .unwrap(),
            )
            .provisioned_throughput(
                ProvisionedThroughput::builder()
                    .read_capacity_units(5)
                    .write_capacity_units(5)
                    .build()
                    .unwrap(),
            )
            .send()
            .await
            .expect("Failed to create test table");

        // Wait for table to be active
        tokio::time::sleep(tokio::time::Duration::from_millis(1000)).await;
    }

    #[tokio::test]
    #[ignore = "requires DynamoDB"]
    async fn test_dynamodb_history_basic() {
        let table_name = "test_chat_history_basic";
        let session_id = "test-session-1";

        let history = DynamoDBChatMessageHistory::builder()
            .table_name(table_name)
            .session_id(session_id)
            .endpoint_url("http://localhost:4566")
            .build()
            .await
            .expect("Failed to create history");

        // Create test table
        create_test_table(&history.client, table_name).await;

        // Initially empty
        let messages = history.get_messages().await.unwrap();
        assert_eq!(messages.len(), 0);

        // Add a message
        let msg = Message::human("Hello!");
        history
            .add_messages(std::slice::from_ref(&msg))
            .await
            .unwrap();

        // Verify it was added
        let messages = history.get_messages().await.unwrap();
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].content().as_text(), "Hello!");

        // Clean up
        history.clear().await.unwrap();
        let messages = history.get_messages().await.unwrap();
        assert_eq!(messages.len(), 0);
    }

    #[tokio::test]
    #[ignore = "requires DynamoDB"]
    async fn test_dynamodb_history_multiple_messages() {
        let table_name = "test_chat_history_multiple";
        let session_id = "test-session-2";

        let history = DynamoDBChatMessageHistory::builder()
            .table_name(table_name)
            .session_id(session_id)
            .endpoint_url("http://localhost:4566")
            .build()
            .await
            .expect("Failed to create history");

        // Create test table
        create_test_table(&history.client, table_name).await;

        // Add multiple messages
        let messages = vec![
            Message::human("Hello!"),
            Message::ai("Hi there!"),
            Message::human("How are you?"),
        ];
        history.add_messages(&messages).await.unwrap();

        // Verify all messages
        let retrieved = history.get_messages().await.unwrap();
        assert_eq!(retrieved.len(), 3);
        assert_eq!(retrieved[0].content().as_text(), "Hello!");
        assert_eq!(retrieved[1].content().as_text(), "Hi there!");
        assert_eq!(retrieved[2].content().as_text(), "How are you?");

        // Clean up
        history.clear().await.unwrap();
    }

    #[tokio::test]
    #[ignore = "requires DynamoDB"]
    async fn test_dynamodb_history_with_size_limit() {
        let table_name = "test_chat_history_size_limit";
        let session_id = "test-session-3";

        let history = DynamoDBChatMessageHistory::builder()
            .table_name(table_name)
            .session_id(session_id)
            .endpoint_url("http://localhost:4566")
            .history_size(2) // Only keep 2 messages
            .build()
            .await
            .expect("Failed to create history");

        // Create test table
        create_test_table(&history.client, table_name).await;

        // Add 4 messages
        let messages = vec![
            Message::human("Message 1"),
            Message::ai("Message 2"),
            Message::human("Message 3"),
            Message::ai("Message 4"),
        ];
        history.add_messages(&messages).await.unwrap();

        // Should only keep last 2
        let retrieved = history.get_messages().await.unwrap();
        assert_eq!(retrieved.len(), 2);
        assert_eq!(retrieved[0].content().as_text(), "Message 3");
        assert_eq!(retrieved[1].content().as_text(), "Message 4");

        // Clean up
        history.clear().await.unwrap();
    }

    #[tokio::test]
    #[ignore = "requires DynamoDB"]
    async fn test_dynamodb_history_persistence() {
        let table_name = "test_chat_history_persistence";
        let session_id = "test-session-4";

        // Create first instance and add messages
        {
            let history = DynamoDBChatMessageHistory::builder()
                .table_name(table_name)
                .session_id(session_id)
                .endpoint_url("http://localhost:4566")
                .build()
                .await
                .expect("Failed to create history");

            // Create test table
            create_test_table(&history.client, table_name).await;

            let msg = Message::human("Persistent message");
            history.add_messages(&[msg]).await.unwrap();
        }

        // Create second instance with same session_id
        {
            let history = DynamoDBChatMessageHistory::builder()
                .table_name(table_name)
                .session_id(session_id)
                .endpoint_url("http://localhost:4566")
                .build()
                .await
                .expect("Failed to create history");

            // Should retrieve the message from first instance
            let messages = history.get_messages().await.unwrap();
            assert_eq!(messages.len(), 1);
            assert_eq!(messages[0].content().as_text(), "Persistent message");

            // Clean up
            history.clear().await.unwrap();
        }
    }

    #[tokio::test]
    #[ignore = "requires DynamoDB"]
    async fn test_dynamodb_history_custom_key() {
        let table_name = "test_chat_history_custom_key";
        let session_id = "test-session-5";

        let history = DynamoDBChatMessageHistory::builder()
            .table_name(table_name)
            .session_id(session_id)
            .endpoint_url("http://localhost:4566")
            .primary_key_name("CustomSessionId")
            .build()
            .await
            .expect("Failed to create history");

        // Note: In a real scenario, you'd need to create a table with CustomSessionId as primary key
        // For this test, we're just verifying the builder works correctly

        // Verify the key was set correctly
        assert!(history.key.contains_key("CustomSessionId"));
        assert_eq!(
            history.key.get("CustomSessionId"),
            Some(&AttributeValue::S(session_id.to_string()))
        );
    }

    #[tokio::test]
    #[ignore = "requires DynamoDB"]
    async fn test_dynamodb_history_append_messages() {
        let table_name = "test_chat_history_append";
        let session_id = "test-session-6";

        let history = DynamoDBChatMessageHistory::builder()
            .table_name(table_name)
            .session_id(session_id)
            .endpoint_url("http://localhost:4566")
            .build()
            .await
            .expect("Failed to create history");

        // Create test table
        create_test_table(&history.client, table_name).await;

        // Add first message
        history
            .add_messages(&[Message::human("First")])
            .await
            .unwrap();

        // Add second message
        history
            .add_messages(&[Message::ai("Second")])
            .await
            .unwrap();

        // Both should be present
        let messages = history.get_messages().await.unwrap();
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].content().as_text(), "First");
        assert_eq!(messages[1].content().as_text(), "Second");

        // Clean up
        history.clear().await.unwrap();
    }

    #[tokio::test]
    async fn test_dynamodb_builder_validation() {
        // Should fail without table_name
        let result = DynamoDBChatMessageHistory::builder()
            .session_id("test")
            .build()
            .await;
        assert!(result.is_err());

        // Should fail without session_id
        let result = DynamoDBChatMessageHistory::builder()
            .table_name("test")
            .build()
            .await;
        assert!(result.is_err());
    }

    #[test]
    fn test_current_timestamp() {
        let ts = DynamoDBChatMessageHistory::current_timestamp();
        assert!(ts > 0);
        // Should be roughly current (after year 2020)
        assert!(ts > 1577836800); // Jan 1, 2020
    }

    #[tokio::test]
    #[ignore = "requires DynamoDB"]
    async fn test_dynamodb_chat_history_with_unicode() {
        let table_name = "test_chat_history_unicode";
        let session_id = "test-session-unicode";

        let history = DynamoDBChatMessageHistory::builder()
            .table_name(table_name)
            .session_id(session_id)
            .endpoint_url("http://localhost:4566")
            .build()
            .await
            .expect("Failed to create history");

        // Create test table
        create_test_table(&history.client, table_name).await;

        // Test unicode messages: Chinese, Russian, Arabic, emojis, mathematical symbols
        let unicode_messages = vec![
            "你好世界",       // Chinese
            "Здравствуй мир", // Russian
            "مرحبا بالعالم",  // Arabic
            "🌍👋🎉💬",       // Emojis
            "∑∫∂√∞≈≠",        // Mathematical symbols
        ];

        for msg in &unicode_messages {
            history.add_messages(&[Message::human(*msg)]).await.unwrap();
        }

        // Verify all unicode messages stored correctly
        let messages = history.get_messages().await.unwrap();
        assert_eq!(messages.len(), 5);

        for (i, expected) in unicode_messages.iter().enumerate() {
            assert_eq!(messages[i].content().as_text(), *expected);
        }

        // Clean up
        history.clear().await.unwrap();
    }
}
