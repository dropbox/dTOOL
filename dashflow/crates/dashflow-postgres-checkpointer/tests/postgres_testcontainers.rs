// PostgreSQL Integration Tests with Testcontainers
// Author: Andrew Yates (ayates@dropbox.com) - 2025 Dropbox
//
//! Integration tests for PostgresCheckpointer using testcontainers.
//! These tests automatically start PostgreSQL in Docker and clean up afterward.
//!
//! Run these tests with:
//! ```bash
//! # On macOS with Colima, set DOCKER_HOST:
//! export DOCKER_HOST=unix://$HOME/.colima/default/docker.sock
//! cargo test -p dashflow-postgres-checkpointer --test postgres_testcontainers
//!
//! # Or on systems with standard Docker socket:
//! cargo test -p dashflow-postgres-checkpointer --test postgres_testcontainers
//! ```

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use dashflow::{Checkpoint, Checkpointer};
use dashflow_postgres_checkpointer::PostgresCheckpointer;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use testcontainers::runners::AsyncRunner;
use testcontainers_modules::postgres::Postgres;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
struct TestState {
    value: i32,
    message: String,
}

/// Build connection string from container
fn build_connection_string(host: &str, port: u16) -> String {
    format!(
        "host={} port={} user=postgres password=postgres dbname=postgres",
        host, port
    )
}

#[tokio::test]
async fn test_postgres_checkpointer_save_load_with_testcontainers() {
    // Start PostgreSQL in Docker (automatically cleaned up when test ends)
    let container = Postgres::default().start().await.unwrap();

    // Get connection details
    let host = container.get_host().await.unwrap();
    let port = container.get_host_port_ipv4(5432).await.unwrap();
    let connection_string = build_connection_string(&host.to_string(), port);

    // Wait for PostgreSQL to be ready
    tokio::time::sleep(Duration::from_secs(2)).await;

    let checkpointer = PostgresCheckpointer::new(&connection_string)
        .await
        .expect("Failed to create PostgresCheckpointer");

    let state = TestState {
        value: 42,
        message: "Hello, PostgreSQL with testcontainers!".to_string(),
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
    assert_eq!(
        loaded.state.message,
        "Hello, PostgreSQL with testcontainers!"
    );

    // Container is automatically cleaned up when dropped
}

#[tokio::test]
async fn test_postgres_checkpointer_get_latest_with_testcontainers() {
    let container = Postgres::default().start().await.unwrap();
    let host = container.get_host().await.unwrap();
    let port = container.get_host_port_ipv4(5432).await.unwrap();
    let connection_string = build_connection_string(&host.to_string(), port);

    tokio::time::sleep(Duration::from_secs(2)).await;

    let checkpointer = PostgresCheckpointer::new(&connection_string)
        .await
        .expect("Failed to create PostgresCheckpointer");

    let thread_id = "thread_latest_test_tc";

    // Save multiple checkpoints with delays to ensure different timestamps
    let cp1 = Checkpoint::new(
        thread_id.to_string(),
        TestState {
            value: 1,
            message: "First".to_string(),
        },
        "node1".to_string(),
        None,
    );
    checkpointer
        .save(cp1)
        .await
        .expect("Failed to save checkpoint 1");
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
    checkpointer
        .save(cp2)
        .await
        .expect("Failed to save checkpoint 2");
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
    checkpointer
        .save(cp3)
        .await
        .expect("Failed to save checkpoint 3");

    // Get latest should return the newest
    let latest = checkpointer
        .get_latest(thread_id)
        .await
        .expect("Failed to get latest checkpoint");
    assert!(latest.is_some());
    let latest = latest.unwrap();
    assert_eq!(latest.state.value, 3);
    assert_eq!(latest.state.message, "Third");
}

#[tokio::test]
async fn test_postgres_checkpointer_list_with_testcontainers() {
    let container = Postgres::default().start().await.unwrap();
    let host = container.get_host().await.unwrap();
    let port = container.get_host_port_ipv4(5432).await.unwrap();
    let connection_string = build_connection_string(&host.to_string(), port);

    tokio::time::sleep(Duration::from_secs(2)).await;

    let checkpointer = PostgresCheckpointer::new(&connection_string)
        .await
        .expect("Failed to create PostgresCheckpointer");

    let thread1 = "thread_list_test_1_tc";
    let thread2 = "thread_list_test_2_tc";

    // Save checkpoints for two threads with delays
    let cp1 = Checkpoint::new(
        thread1.to_string(),
        TestState {
            value: 1,
            message: "T1_First".to_string(),
        },
        "node1".to_string(),
        None,
    );
    checkpointer
        .save(cp1)
        .await
        .expect("Failed to save checkpoint 1");
    tokio::time::sleep(Duration::from_millis(100)).await;

    let cp2 = Checkpoint::new(
        thread2.to_string(),
        TestState {
            value: 2,
            message: "T2_First".to_string(),
        },
        "node2".to_string(),
        None,
    );
    checkpointer
        .save(cp2)
        .await
        .expect("Failed to save checkpoint 2");
    tokio::time::sleep(Duration::from_millis(100)).await;

    let cp3 = Checkpoint::new(
        thread1.to_string(),
        TestState {
            value: 3,
            message: "T1_Second".to_string(),
        },
        "node3".to_string(),
        None,
    );
    checkpointer
        .save(cp3)
        .await
        .expect("Failed to save checkpoint 3");

    // List should only return checkpoints for specified thread
    let list = checkpointer
        .list(thread1)
        .await
        .expect("Failed to list checkpoints");
    assert_eq!(list.len(), 2);
    assert_eq!(list[0].node, "node3"); // Newest first
    assert_eq!(list[1].node, "node1");
}

#[tokio::test]
async fn test_postgres_checkpointer_delete_with_testcontainers() {
    let container = Postgres::default().start().await.unwrap();
    let host = container.get_host().await.unwrap();
    let port = container.get_host_port_ipv4(5432).await.unwrap();
    let connection_string = build_connection_string(&host.to_string(), port);

    tokio::time::sleep(Duration::from_secs(2)).await;

    let checkpointer = PostgresCheckpointer::new(&connection_string)
        .await
        .expect("Failed to create PostgresCheckpointer");

    let checkpoint = Checkpoint::new(
        "thread_delete_test_tc".to_string(),
        TestState {
            value: 42,
            message: "To be deleted".to_string(),
        },
        "node1".to_string(),
        None,
    );
    let checkpoint_id = checkpoint.id.clone();

    checkpointer
        .save(checkpoint)
        .await
        .expect("Failed to save checkpoint");

    let loaded = checkpointer
        .load(&checkpoint_id)
        .await
        .expect("Failed to load checkpoint");
    assert!(loaded.is_some());

    checkpointer
        .delete(&checkpoint_id)
        .await
        .expect("Failed to delete checkpoint");

    let loaded = checkpointer
        .load(&checkpoint_id)
        .await
        .expect("Failed to load checkpoint after delete");
    assert!(loaded.is_none());
}

#[tokio::test]
async fn test_postgres_checkpointer_delete_thread_with_testcontainers() {
    let container = Postgres::default().start().await.unwrap();
    let host = container.get_host().await.unwrap();
    let port = container.get_host_port_ipv4(5432).await.unwrap();
    let connection_string = build_connection_string(&host.to_string(), port);

    tokio::time::sleep(Duration::from_secs(2)).await;

    let checkpointer = PostgresCheckpointer::new(&connection_string)
        .await
        .expect("Failed to create PostgresCheckpointer");

    let thread1 = "thread_delete_all_test_1_tc";
    let thread2 = "thread_delete_all_test_2_tc";

    let cp1 = Checkpoint::new(
        thread1.to_string(),
        TestState {
            value: 1,
            message: "T1_First".to_string(),
        },
        "node1".to_string(),
        None,
    );
    checkpointer
        .save(cp1)
        .await
        .expect("Failed to save checkpoint 1");

    let cp2 = Checkpoint::new(
        thread2.to_string(),
        TestState {
            value: 2,
            message: "T2_First".to_string(),
        },
        "node2".to_string(),
        None,
    );
    checkpointer
        .save(cp2)
        .await
        .expect("Failed to save checkpoint 2");

    let cp3 = Checkpoint::new(
        thread1.to_string(),
        TestState {
            value: 3,
            message: "T1_Second".to_string(),
        },
        "node3".to_string(),
        None,
    );
    checkpointer
        .save(cp3)
        .await
        .expect("Failed to save checkpoint 3");

    checkpointer
        .delete_thread(thread1)
        .await
        .expect("Failed to delete thread");

    let list1 = checkpointer
        .list(thread1)
        .await
        .expect("Failed to list thread1 checkpoints");
    assert_eq!(list1.len(), 0);

    let list2 = checkpointer
        .list(thread2)
        .await
        .expect("Failed to list thread2 checkpoints");
    assert_eq!(list2.len(), 1);
}

#[tokio::test]
async fn test_postgres_checkpointer_with_metadata_testcontainers() {
    let container = Postgres::default().start().await.unwrap();
    let host = container.get_host().await.unwrap();
    let port = container.get_host_port_ipv4(5432).await.unwrap();
    let connection_string = build_connection_string(&host.to_string(), port);

    tokio::time::sleep(Duration::from_secs(2)).await;

    let checkpointer = PostgresCheckpointer::new(&connection_string)
        .await
        .expect("Failed to create PostgresCheckpointer");

    let checkpoint = Checkpoint::new(
        "thread_metadata_test_tc".to_string(),
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

    checkpointer
        .save(checkpoint)
        .await
        .expect("Failed to save checkpoint");

    let loaded = checkpointer
        .load(&checkpoint_id)
        .await
        .expect("Failed to load checkpoint");
    assert!(loaded.is_some());
    let loaded = loaded.unwrap();
    assert_eq!(loaded.metadata.get("user"), Some(&"alice".to_string()));
    assert_eq!(
        loaded.metadata.get("reason"),
        Some(&"manual_checkpoint".to_string())
    );
}
