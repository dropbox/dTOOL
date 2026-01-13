// Cassandra/ScyllaDB Integration Tests with Testcontainers
// Author: Andrew Yates (ayates@dropbox.com) - 2025 Dropbox
//
//! Integration tests for CassandraChatMessageHistory using testcontainers.
//! These tests automatically start ScyllaDB in Docker and clean up afterward.
//!
//! Run these tests with:
//! ```bash
//! # On macOS with Colima, set DOCKER_HOST:
//! export DOCKER_HOST=unix://$HOME/.colima/default/docker.sock
//! cargo test -p dashflow-memory --features cassandra-backend --test cassandra_testcontainers
//!
//! # Or on systems with standard Docker socket:
//! cargo test -p dashflow-memory --features cassandra-backend --test cassandra_testcontainers
//! ```

#![cfg(feature = "cassandra-backend")]

use dashflow::core::chat_history::BaseChatMessageHistory;
use dashflow_memory::CassandraChatMessageHistory;
use scylla::SessionBuilder;
use std::sync::Arc;
use std::time::Duration;
use testcontainers::core::{ContainerRequest, IntoContainerPort, WaitFor};
use testcontainers::runners::AsyncRunner;
use testcontainers::{GenericImage, ImageExt};

const SCYLLA_IMAGE: &str = "scylladb/scylla";
const SCYLLA_TAG: &str = "5.4";
const CQL_PORT: u16 = 9042;
const TEST_KEYSPACE: &str = "test_keyspace";

/// Create a ScyllaDB container
fn scylla_image() -> ContainerRequest<GenericImage> {
    GenericImage::new(SCYLLA_IMAGE, SCYLLA_TAG)
        .with_exposed_port(CQL_PORT.tcp())
        .with_wait_for(WaitFor::message_on_stdout(
            "Starting listening for CQL clients",
        ))
        .with_env_var("SCYLLA_DEVELOPER_MODE", "1")
}

/// Wait for ScyllaDB to be fully ready and create test keyspace
async fn setup_keyspace(host: &str, port: u16) -> Arc<scylla::Session> {
    // Wait a bit more for ScyllaDB to be fully ready
    tokio::time::sleep(Duration::from_secs(5)).await;

    let session = SessionBuilder::new()
        .known_node(format!("{}:{}", host, port))
        .build()
        .await
        .expect("Failed to create session");

    // Create test keyspace
    session
        .query_unpaged(
            format!(
                "CREATE KEYSPACE IF NOT EXISTS {} WITH replication = {{'class': 'SimpleStrategy', 'replication_factor': 1}}",
                TEST_KEYSPACE
            ),
            &[],
        )
        .await
        .expect("Failed to create keyspace");

    // Wait for keyspace to be ready
    tokio::time::sleep(Duration::from_millis(500)).await;

    Arc::new(session)
}

#[tokio::test]
async fn test_cassandra_history_basic_with_testcontainers() {
    // Start ScyllaDB in Docker (automatically cleaned up when test ends)
    let container = scylla_image().start().await.unwrap();

    // Get connection details
    let host = container.get_host().await.unwrap();
    let port = container.get_host_port_ipv4(CQL_PORT).await.unwrap();

    let session = setup_keyspace(&host.to_string(), port).await;

    // Create message history
    let history = CassandraChatMessageHistory::builder()
        .shared_session(session)
        .keyspace(TEST_KEYSPACE)
        .session_id("session-tc-basic")
        .build()
        .await
        .expect("Failed to create CassandraChatMessageHistory");

    // Add messages
    history
        .add_user_message("Hello!")
        .await
        .expect("Failed to add user message");
    history
        .add_ai_message("Hi there!")
        .await
        .expect("Failed to add AI message");

    // Retrieve messages
    let messages = history
        .get_messages()
        .await
        .expect("Failed to get messages");
    assert_eq!(messages.len(), 2);

    // Clear history
    history.clear().await.expect("Failed to clear history");
    let messages = history
        .get_messages()
        .await
        .expect("Failed to get messages");
    assert_eq!(messages.len(), 0);
}

#[tokio::test]
async fn test_cassandra_history_multiple_messages_with_testcontainers() {
    let container = scylla_image().start().await.unwrap();
    let host = container.get_host().await.unwrap();
    let port = container.get_host_port_ipv4(CQL_PORT).await.unwrap();
    let session = setup_keyspace(&host.to_string(), port).await;

    let history = CassandraChatMessageHistory::builder()
        .shared_session(session)
        .keyspace(TEST_KEYSPACE)
        .session_id("session-tc-multiple")
        .build()
        .await
        .expect("Failed to create CassandraChatMessageHistory");

    // Add multiple messages
    for i in 0..5 {
        history
            .add_user_message(&format!("User message {}", i))
            .await
            .expect("Failed to add user message");
        history
            .add_ai_message(&format!("AI response {}", i))
            .await
            .expect("Failed to add AI message");
    }

    // Verify all messages
    let messages = history
        .get_messages()
        .await
        .expect("Failed to get messages");
    assert_eq!(messages.len(), 10);

    // Clear
    history.clear().await.expect("Failed to clear history");
}

#[tokio::test]
async fn test_cassandra_history_unicode_with_testcontainers() {
    let container = scylla_image().start().await.unwrap();
    let host = container.get_host().await.unwrap();
    let port = container.get_host_port_ipv4(CQL_PORT).await.unwrap();
    let session = setup_keyspace(&host.to_string(), port).await;

    let history = CassandraChatMessageHistory::builder()
        .shared_session(session)
        .keyspace(TEST_KEYSPACE)
        .session_id("session-tc-unicode")
        .build()
        .await
        .expect("Failed to create CassandraChatMessageHistory");

    // Test unicode messages
    history
        .add_user_message("Hello! I want to learn about languages.")
        .await
        .expect("Failed to add message");
    history
        .add_ai_message(
            "I can help with languages like Japanese (Japanese), Chinese (Chinese), etc.",
        )
        .await
        .expect("Failed to add message");

    let messages = history
        .get_messages()
        .await
        .expect("Failed to get messages");
    assert_eq!(messages.len(), 2);

    history.clear().await.expect("Failed to clear history");
}

#[tokio::test]
async fn test_cassandra_history_empty_state_with_testcontainers() {
    let container = scylla_image().start().await.unwrap();
    let host = container.get_host().await.unwrap();
    let port = container.get_host_port_ipv4(CQL_PORT).await.unwrap();
    let session = setup_keyspace(&host.to_string(), port).await;

    let history = CassandraChatMessageHistory::builder()
        .shared_session(session)
        .keyspace(TEST_KEYSPACE)
        .session_id("session-tc-empty")
        .build()
        .await
        .expect("Failed to create CassandraChatMessageHistory");

    // Empty history should return empty vec
    let messages = history
        .get_messages()
        .await
        .expect("Failed to get messages");
    assert!(messages.is_empty());

    // Clear on empty should be safe
    history.clear().await.expect("Failed to clear history");
}

#[tokio::test]
async fn test_cassandra_multiple_sessions_with_testcontainers() {
    let container = scylla_image().start().await.unwrap();
    let host = container.get_host().await.unwrap();
    let port = container.get_host_port_ipv4(CQL_PORT).await.unwrap();
    let session = setup_keyspace(&host.to_string(), port).await;

    // Create two histories for different session IDs
    let history1 = CassandraChatMessageHistory::builder()
        .shared_session(Arc::clone(&session))
        .keyspace(TEST_KEYSPACE)
        .session_id("session-tc-1")
        .build()
        .await
        .expect("Failed to create history 1");

    let history2 = CassandraChatMessageHistory::builder()
        .shared_session(session)
        .keyspace(TEST_KEYSPACE)
        .session_id("session-tc-2")
        .build()
        .await
        .expect("Failed to create history 2");

    // Add messages to each
    history1
        .add_user_message("Message for session 1")
        .await
        .expect("Failed to add message");
    history2
        .add_user_message("Message for session 2")
        .await
        .expect("Failed to add message");

    // Verify isolation
    let msgs1 = history1
        .get_messages()
        .await
        .expect("Failed to get messages");
    let msgs2 = history2
        .get_messages()
        .await
        .expect("Failed to get messages");

    assert_eq!(msgs1.len(), 1);
    assert_eq!(msgs2.len(), 1);

    // Cleanup
    history1.clear().await.expect("Failed to clear history 1");
    history2.clear().await.expect("Failed to clear history 2");
}
