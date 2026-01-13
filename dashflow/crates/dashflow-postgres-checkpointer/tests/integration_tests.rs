//! Integration tests for PostgresCheckpointer
//!
//! These tests require a running PostgreSQL instance.
//! Use Docker Compose to start PostgreSQL:
//!
//! ```bash
//! docker-compose -f docker-compose.postgres.yml up -d
//! cargo test --package dashflow-postgres-checkpointer
//! docker-compose -f docker-compose.postgres.yml down
//! ```

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use dashflow::{Checkpoint, Checkpointer};
use dashflow_postgres_checkpointer::PostgresCheckpointer;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
struct TestState {
    value: i32,
    message: String,
}

/// Get PostgreSQL connection string from environment or use default
fn get_connection_string() -> String {
    std::env::var("POSTGRES_CONNECTION_STRING").unwrap_or_else(|_| {
        "host=localhost port=5432 user=postgres password=postgres dbname=dashflow".to_string()
    })
}

#[tokio::test]
#[ignore = "requires running PostgreSQL (run with --ignored)"]
async fn test_postgres_checkpointer_save_load() {
    let connection_string = get_connection_string();
    let checkpointer = PostgresCheckpointer::new(&connection_string)
        .await
        .expect("Failed to create PostgresCheckpointer");

    let state = TestState {
        value: 42,
        message: "Hello, PostgreSQL!".to_string(),
    };
    let checkpoint = Checkpoint::new("thread1".to_string(), state, "node1".to_string(), None);
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
    assert_eq!(loaded.state.message, "Hello, PostgreSQL!");

    // Cleanup
    checkpointer
        .delete(&checkpoint_id)
        .await
        .expect("Failed to delete checkpoint");
}

#[tokio::test]
#[ignore = "requires running PostgreSQL (run with --ignored)"]
async fn test_postgres_checkpointer_get_latest() {
    let connection_string = get_connection_string();
    let checkpointer = PostgresCheckpointer::new(&connection_string)
        .await
        .expect("Failed to create PostgresCheckpointer");

    let thread_id = "thread_latest_test";

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
    let id1 = cp1.id.clone();
    checkpointer
        .save(cp1)
        .await
        .expect("Failed to save checkpoint 1");
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    let cp2 = Checkpoint::new(
        thread_id.to_string(),
        TestState {
            value: 2,
            message: "Second".to_string(),
        },
        "node2".to_string(),
        None,
    );
    let id2 = cp2.id.clone();
    checkpointer
        .save(cp2)
        .await
        .expect("Failed to save checkpoint 2");
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    let cp3 = Checkpoint::new(
        thread_id.to_string(),
        TestState {
            value: 3,
            message: "Third".to_string(),
        },
        "node3".to_string(),
        None,
    );
    let id3 = cp3.id.clone();
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

    // Cleanup
    checkpointer
        .delete(&id1)
        .await
        .expect("Failed to delete checkpoint 1");
    checkpointer
        .delete(&id2)
        .await
        .expect("Failed to delete checkpoint 2");
    checkpointer
        .delete(&id3)
        .await
        .expect("Failed to delete checkpoint 3");
}

#[tokio::test]
#[ignore = "requires running PostgreSQL (run with --ignored)"]
async fn test_postgres_checkpointer_list() {
    let connection_string = get_connection_string();
    let checkpointer = PostgresCheckpointer::new(&connection_string)
        .await
        .expect("Failed to create PostgresCheckpointer");

    let thread1 = "thread_list_test_1";
    let thread2 = "thread_list_test_2";

    // Save checkpoints for two threads with delays to ensure different timestamps
    let cp1 = Checkpoint::new(
        thread1.to_string(),
        TestState {
            value: 1,
            message: "T1_First".to_string(),
        },
        "node1".to_string(),
        None,
    );
    let id1 = cp1.id.clone();
    checkpointer
        .save(cp1)
        .await
        .expect("Failed to save checkpoint 1");
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    let cp2 = Checkpoint::new(
        thread2.to_string(),
        TestState {
            value: 2,
            message: "T2_First".to_string(),
        },
        "node2".to_string(),
        None,
    );
    let id2 = cp2.id.clone();
    checkpointer
        .save(cp2)
        .await
        .expect("Failed to save checkpoint 2");
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    let cp3 = Checkpoint::new(
        thread1.to_string(),
        TestState {
            value: 3,
            message: "T1_Second".to_string(),
        },
        "node3".to_string(),
        None,
    );
    let id3 = cp3.id.clone();
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

    // Cleanup
    checkpointer
        .delete(&id1)
        .await
        .expect("Failed to delete checkpoint 1");
    checkpointer
        .delete(&id2)
        .await
        .expect("Failed to delete checkpoint 2");
    checkpointer
        .delete(&id3)
        .await
        .expect("Failed to delete checkpoint 3");
}

#[tokio::test]
#[ignore = "requires running PostgreSQL (run with --ignored)"]
async fn test_postgres_checkpointer_delete() {
    let connection_string = get_connection_string();
    let checkpointer = PostgresCheckpointer::new(&connection_string)
        .await
        .expect("Failed to create PostgresCheckpointer");

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

    checkpointer
        .save(checkpoint)
        .await
        .expect("Failed to save checkpoint");

    let loaded = checkpointer
        .load(&checkpoint_id)
        .await
        .expect("Failed to load checkpoint");
    let loaded = loaded.expect("Checkpoint should exist before delete");
    assert_eq!(loaded.state.value, 42);
    assert_eq!(loaded.state.message, "To be deleted");

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
#[ignore = "requires running PostgreSQL (run with --ignored)"]
async fn test_postgres_checkpointer_delete_thread() {
    let connection_string = get_connection_string();
    let checkpointer = PostgresCheckpointer::new(&connection_string)
        .await
        .expect("Failed to create PostgresCheckpointer");

    let thread1 = "thread_delete_all_test_1";
    let thread2 = "thread_delete_all_test_2";

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
    let id2 = cp2.id.clone();
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

    // Cleanup
    checkpointer
        .delete(&id2)
        .await
        .expect("Failed to delete checkpoint 2");
}

#[tokio::test]
#[ignore = "requires running PostgreSQL (run with --ignored)"]
async fn test_postgres_checkpointer_with_metadata() {
    let connection_string = get_connection_string();
    let checkpointer = PostgresCheckpointer::new(&connection_string)
        .await
        .expect("Failed to create PostgresCheckpointer");

    let checkpoint = Checkpoint::new(
        "thread_metadata_test".to_string(),
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

    // Cleanup
    checkpointer
        .delete(&checkpoint_id)
        .await
        .expect("Failed to delete checkpoint");
}
