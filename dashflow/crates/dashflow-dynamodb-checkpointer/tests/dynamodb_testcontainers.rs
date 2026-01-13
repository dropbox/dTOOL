// DynamoDB Integration Tests with Testcontainers (LocalStack)
// Author: Andrew Yates (ayates@dropbox.com) - 2025 Dropbox
//
//! Integration tests for DynamoDBCheckpointer using testcontainers with LocalStack.
//! These tests automatically start LocalStack in Docker and clean up afterward.
//!
//! Run these tests with:
//! ```bash
//! # On macOS with Colima, set DOCKER_HOST:
//! export DOCKER_HOST=unix://$HOME/.colima/default/docker.sock
//! cargo test -p dashflow-dynamodb-checkpointer --test dynamodb_testcontainers
//!
//! # Or on systems with standard Docker socket:
//! cargo test -p dashflow-dynamodb-checkpointer --test dynamodb_testcontainers
//! ```

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use aws_sdk_dynamodb::{
    config::{Credentials, Region},
    types::{AttributeDefinition, KeySchemaElement, KeyType, ScalarAttributeType},
    Client as DynamoDBClient,
};
use dashflow::{Checkpoint, Checkpointer};
use dashflow_dynamodb_checkpointer::DynamoDBCheckpointer;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use testcontainers::runners::AsyncRunner;
use testcontainers_modules::localstack::LocalStack;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
struct TestState {
    value: i32,
    message: String,
}

/// Create DynamoDB client configured for LocalStack
async fn create_localstack_client(endpoint_url: &str) -> DynamoDBClient {
    let credentials = Credentials::new("test", "test", None, None, "static");
    let config = aws_sdk_dynamodb::Config::builder()
        .region(Region::new("us-east-1"))
        .endpoint_url(endpoint_url)
        .credentials_provider(credentials)
        .behavior_version_latest()
        .build();

    DynamoDBClient::from_conf(config)
}

/// Create the test table in LocalStack DynamoDB
async fn create_test_table(client: &DynamoDBClient, table_name: &str) {
    let result = client
        .create_table()
        .table_name(table_name)
        .attribute_definitions(
            AttributeDefinition::builder()
                .attribute_name("thread_id")
                .attribute_type(ScalarAttributeType::S)
                .build()
                .unwrap(),
        )
        .attribute_definitions(
            AttributeDefinition::builder()
                .attribute_name("checkpoint_id")
                .attribute_type(ScalarAttributeType::S)
                .build()
                .unwrap(),
        )
        .key_schema(
            KeySchemaElement::builder()
                .attribute_name("thread_id")
                .key_type(KeyType::Hash)
                .build()
                .unwrap(),
        )
        .key_schema(
            KeySchemaElement::builder()
                .attribute_name("checkpoint_id")
                .key_type(KeyType::Range)
                .build()
                .unwrap(),
        )
        .billing_mode(aws_sdk_dynamodb::types::BillingMode::PayPerRequest)
        .send()
        .await;

    match result {
        Ok(_) => {}
        Err(e) => {
            // Table might already exist, which is fine
            if !e.to_string().contains("ResourceInUseException") {
                panic!("Failed to create table: {:?}", e);
            }
        }
    }

    // Wait for table to be active
    tokio::time::sleep(Duration::from_secs(1)).await;
}

/// Start LocalStack container and return endpoint URL
async fn start_localstack() -> (testcontainers::ContainerAsync<LocalStack>, String) {
    let container = LocalStack::default()
        .start()
        .await
        .expect("Failed to start LocalStack container");

    let host = container.get_host().await.unwrap();
    let port = container.get_host_port_ipv4(4566).await.unwrap();
    let endpoint_url = format!("http://{}:{}", host, port);

    // Wait for LocalStack to be ready
    tokio::time::sleep(Duration::from_secs(5)).await;

    (container, endpoint_url)
}

#[tokio::test]
async fn test_dynamodb_checkpointer_save_load_with_testcontainers() {
    let (_container, endpoint_url) = start_localstack().await;
    let client = create_localstack_client(&endpoint_url).await;
    let table_name = "test_save_load";

    create_test_table(&client, table_name).await;

    let checkpointer = DynamoDBCheckpointer::<TestState>::new()
        .with_table_name(table_name)
        .with_dynamodb_client(client);

    let state = TestState {
        value: 42,
        message: "Hello DynamoDB with testcontainers!".to_string(),
    };
    let checkpoint = Checkpoint::new(
        "thread1".to_string(),
        state.clone(),
        "node1".to_string(),
        None,
    );
    let checkpoint_id = checkpoint.id.clone();

    // Save checkpoint
    checkpointer
        .save(checkpoint)
        .await
        .expect("Failed to save checkpoint");

    // Load checkpoint
    let loaded = checkpointer
        .load(&checkpoint_id)
        .await
        .expect("Failed to load checkpoint");
    assert!(loaded.is_some());
    let loaded = loaded.unwrap();
    assert_eq!(loaded.state.value, 42);
    assert_eq!(loaded.state.message, "Hello DynamoDB with testcontainers!");
}

#[tokio::test]
async fn test_dynamodb_checkpointer_get_latest_with_testcontainers() {
    let (_container, endpoint_url) = start_localstack().await;
    let client = create_localstack_client(&endpoint_url).await;
    let table_name = "test_get_latest";

    create_test_table(&client, table_name).await;

    let checkpointer = DynamoDBCheckpointer::<TestState>::new()
        .with_table_name(table_name)
        .with_dynamodb_client(client);

    let thread_id = "thread_latest_test";

    // Save multiple checkpoints with delays
    let cp1 = Checkpoint::new(
        thread_id.to_string(),
        TestState {
            value: 1,
            message: "First".to_string(),
        },
        "node1".to_string(),
        None,
    );
    checkpointer.save(cp1).await.expect("Failed to save cp1");
    tokio::time::sleep(Duration::from_millis(100)).await;

    let cp2 = Checkpoint::new(
        thread_id.to_string(),
        TestState {
            value: 2,
            message: "Second".to_string(),
        },
        "node2".to_string(),
        None,
    );
    checkpointer.save(cp2).await.expect("Failed to save cp2");
    tokio::time::sleep(Duration::from_millis(100)).await;

    let cp3 = Checkpoint::new(
        thread_id.to_string(),
        TestState {
            value: 3,
            message: "Third".to_string(),
        },
        "node3".to_string(),
        None,
    );
    checkpointer.save(cp3).await.expect("Failed to save cp3");

    // Get latest should return the newest
    let latest = checkpointer
        .get_latest(thread_id)
        .await
        .expect("Failed to get latest");
    assert!(latest.is_some());
    let latest = latest.unwrap();
    assert_eq!(latest.state.value, 3);
    assert_eq!(latest.state.message, "Third");
}

#[tokio::test]
async fn test_dynamodb_checkpointer_list_with_testcontainers() {
    let (_container, endpoint_url) = start_localstack().await;
    let client = create_localstack_client(&endpoint_url).await;
    let table_name = "test_list";

    create_test_table(&client, table_name).await;

    let checkpointer = DynamoDBCheckpointer::<TestState>::new()
        .with_table_name(table_name)
        .with_dynamodb_client(client);

    let thread1 = "thread_list_1";
    let thread2 = "thread_list_2";

    // Save checkpoints for two threads
    let cp1 = Checkpoint::new(
        thread1.to_string(),
        TestState {
            value: 1,
            message: "T1_First".to_string(),
        },
        "node1".to_string(),
        None,
    );
    checkpointer.save(cp1).await.expect("Failed to save cp1");

    let cp2 = Checkpoint::new(
        thread2.to_string(),
        TestState {
            value: 2,
            message: "T2_First".to_string(),
        },
        "node2".to_string(),
        None,
    );
    checkpointer.save(cp2).await.expect("Failed to save cp2");

    let cp3 = Checkpoint::new(
        thread1.to_string(),
        TestState {
            value: 3,
            message: "T1_Second".to_string(),
        },
        "node3".to_string(),
        None,
    );
    checkpointer.save(cp3).await.expect("Failed to save cp3");

    // List should return checkpoints for specific thread
    let list = checkpointer
        .list(thread1)
        .await
        .expect("Failed to list checkpoints");
    assert_eq!(list.len(), 2);
}

#[tokio::test]
async fn test_dynamodb_checkpointer_delete_with_testcontainers() {
    let (_container, endpoint_url) = start_localstack().await;
    let client = create_localstack_client(&endpoint_url).await;
    let table_name = "test_delete";

    create_test_table(&client, table_name).await;

    let checkpointer = DynamoDBCheckpointer::<TestState>::new()
        .with_table_name(table_name)
        .with_dynamodb_client(client);

    let checkpoint = Checkpoint::new(
        "thread_delete_test".to_string(),
        TestState {
            value: 42,
            message: "To be deleted".to_string(),
        },
        "node1".to_string(),
        None,
    );
    let checkpoint_id = checkpoint.id.clone();

    checkpointer.save(checkpoint).await.expect("Failed to save");

    let loaded = checkpointer
        .load(&checkpoint_id)
        .await
        .expect("Failed to load");
    assert!(loaded.is_some());

    checkpointer
        .delete(&checkpoint_id)
        .await
        .expect("Failed to delete");

    let loaded = checkpointer
        .load(&checkpoint_id)
        .await
        .expect("Failed to load after delete");
    assert!(loaded.is_none());
}

#[tokio::test]
async fn test_dynamodb_checkpointer_delete_thread_with_testcontainers() {
    let (_container, endpoint_url) = start_localstack().await;
    let client = create_localstack_client(&endpoint_url).await;
    let table_name = "test_delete_thread";

    create_test_table(&client, table_name).await;

    let checkpointer = DynamoDBCheckpointer::<TestState>::new()
        .with_table_name(table_name)
        .with_dynamodb_client(client);

    let thread1 = "thread_to_delete";
    let thread2 = "thread_to_keep";

    // Save checkpoints for both threads
    let cp1 = Checkpoint::new(
        thread1.to_string(),
        TestState {
            value: 1,
            message: "T1".to_string(),
        },
        "node1".to_string(),
        None,
    );
    checkpointer.save(cp1).await.expect("Failed to save cp1");

    let cp2 = Checkpoint::new(
        thread2.to_string(),
        TestState {
            value: 2,
            message: "T2".to_string(),
        },
        "node2".to_string(),
        None,
    );
    checkpointer.save(cp2).await.expect("Failed to save cp2");

    let cp3 = Checkpoint::new(
        thread1.to_string(),
        TestState {
            value: 3,
            message: "T1_2".to_string(),
        },
        "node3".to_string(),
        None,
    );
    checkpointer.save(cp3).await.expect("Failed to save cp3");

    // Delete thread1
    checkpointer
        .delete_thread(thread1)
        .await
        .expect("Failed to delete thread");

    // Verify thread1 is empty
    let list1 = checkpointer.list(thread1).await.expect("Failed to list");
    assert_eq!(list1.len(), 0);

    // Verify thread2 still exists
    let list2 = checkpointer.list(thread2).await.expect("Failed to list");
    assert_eq!(list2.len(), 1);
}

#[tokio::test]
async fn test_dynamodb_checkpointer_with_metadata_testcontainers() {
    let (_container, endpoint_url) = start_localstack().await;
    let client = create_localstack_client(&endpoint_url).await;
    let table_name = "test_metadata";

    create_test_table(&client, table_name).await;

    let checkpointer = DynamoDBCheckpointer::<TestState>::new()
        .with_table_name(table_name)
        .with_dynamodb_client(client);

    let checkpoint = Checkpoint::new(
        "thread_metadata".to_string(),
        TestState {
            value: 42,
            message: "With metadata".to_string(),
        },
        "node1".to_string(),
        None,
    )
    .with_metadata("user", "alice")
    .with_metadata("reason", "manual_checkpoint");

    let checkpoint_id = checkpoint.id.clone();

    checkpointer.save(checkpoint).await.expect("Failed to save");

    let loaded = checkpointer
        .load(&checkpoint_id)
        .await
        .expect("Failed to load");
    assert!(loaded.is_some());
    let loaded = loaded.unwrap();
    assert_eq!(loaded.metadata.get("user"), Some(&"alice".to_string()));
    assert_eq!(
        loaded.metadata.get("reason"),
        Some(&"manual_checkpoint".to_string())
    );
}

#[tokio::test]
async fn test_dynamodb_checkpointer_list_threads_testcontainers() {
    let (_container, endpoint_url) = start_localstack().await;
    let client = create_localstack_client(&endpoint_url).await;
    let table_name = "test_list_threads";

    create_test_table(&client, table_name).await;

    let checkpointer = DynamoDBCheckpointer::<TestState>::new()
        .with_table_name(table_name)
        .with_dynamodb_client(client);

    // Create checkpoints for multiple threads
    let cp1 = Checkpoint::new(
        "thread_a".to_string(),
        TestState {
            value: 1,
            message: "A".to_string(),
        },
        "node1".to_string(),
        None,
    );
    checkpointer.save(cp1).await.expect("Failed to save cp1");

    let cp2 = Checkpoint::new(
        "thread_b".to_string(),
        TestState {
            value: 2,
            message: "B".to_string(),
        },
        "node2".to_string(),
        None,
    );
    checkpointer.save(cp2).await.expect("Failed to save cp2");

    let threads = checkpointer
        .list_threads()
        .await
        .expect("Failed to list threads");
    assert_eq!(threads.len(), 2);

    // Both threads should be present
    let thread_ids: Vec<_> = threads.iter().map(|t| &t.thread_id).collect();
    assert!(thread_ids.contains(&&"thread_a".to_string()));
    assert!(thread_ids.contains(&&"thread_b".to_string()));
}

#[tokio::test]
async fn test_dynamodb_checkpointer_missing_table_name() {
    let (_container, endpoint_url) = start_localstack().await;
    let client = create_localstack_client(&endpoint_url).await;

    // Create checkpointer without table name
    let checkpointer = DynamoDBCheckpointer::<TestState>::new().with_dynamodb_client(client);

    let checkpoint = Checkpoint::new(
        "thread1".to_string(),
        TestState {
            value: 1,
            message: "Test".to_string(),
        },
        "node1".to_string(),
        None,
    );

    // Should fail due to missing table name
    let result = checkpointer.save(checkpoint).await;
    assert!(result.is_err());
}
