//! Integration tests for RedisCheckpointer
//!
//! These tests require a running Redis instance.
//! Use Docker Compose to start Redis:
//!
//! ```bash
//! docker-compose -f docker-compose.test.yml up -d redis
//! cargo test --package dashflow-redis-checkpointer --test integration_tests -- --ignored
//! docker-compose -f docker-compose.test.yml down
//! ```

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use dashflow::{Checkpoint, Checkpointer};
use dashflow_redis_checkpointer::RedisCheckpointer;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
struct TestState {
    value: i32,
    message: String,
}

/// Get Redis connection string from environment or use default
fn get_connection_string() -> String {
    std::env::var("REDIS_CONNECTION_STRING")
        .unwrap_or_else(|_| "redis://localhost:6379".to_string())
}

#[tokio::test]
#[ignore = "requires running Redis (run with --ignored)"]
async fn test_redis_checkpointer_save_load() {
    let connection_string = get_connection_string();
    let checkpointer = RedisCheckpointer::<TestState>::new(&connection_string)
        .await
        .expect("Failed to create RedisCheckpointer");

    let state = TestState {
        value: 42,
        message: "Hello, Redis!".to_string(),
    };
    let checkpoint = Checkpoint::new("thread1".to_string(), state, "node1".to_string(), None);
    let checkpoint_id = checkpoint.id.clone();
    let thread_id = checkpoint.thread_id.clone();

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
    assert_eq!(loaded.state.message, "Hello, Redis!");

    // Cleanup
    checkpointer
        .delete_thread(&thread_id)
        .await
        .expect("Failed to delete thread");
}

#[tokio::test]
#[ignore = "requires running Redis (run with --ignored)"]
async fn test_redis_checkpointer_get_latest() {
    let connection_string = get_connection_string();
    let checkpointer = RedisCheckpointer::<TestState>::new(&connection_string)
        .await
        .expect("Failed to create RedisCheckpointer");

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
        .delete_thread(thread_id)
        .await
        .expect("Failed to delete thread");
}

#[tokio::test]
#[ignore = "requires running Redis (run with --ignored)"]
async fn test_redis_checkpointer_list() {
    let connection_string = get_connection_string();
    let checkpointer = RedisCheckpointer::<TestState>::new(&connection_string)
        .await
        .expect("Failed to create RedisCheckpointer");

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
        .delete_thread(thread1)
        .await
        .expect("Failed to delete thread 1");
    checkpointer
        .delete_thread(thread2)
        .await
        .expect("Failed to delete thread 2");
}

#[tokio::test]
#[ignore = "requires running Redis (run with --ignored)"]
async fn test_redis_checkpointer_delete() {
    let connection_string = get_connection_string();
    let checkpointer = RedisCheckpointer::<TestState>::new(&connection_string)
        .await
        .expect("Failed to create RedisCheckpointer");

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
    let thread_id = checkpoint.thread_id.clone();

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

    // Cleanup
    checkpointer
        .delete_thread(&thread_id)
        .await
        .expect("Failed to delete thread");
}

#[tokio::test]
#[ignore = "requires running Redis (run with --ignored)"]
async fn test_redis_checkpointer_delete_thread() {
    let connection_string = get_connection_string();
    let checkpointer = RedisCheckpointer::<TestState>::new(&connection_string)
        .await
        .expect("Failed to create RedisCheckpointer");

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
        .delete_thread(thread2)
        .await
        .expect("Failed to delete thread 2");
}

#[tokio::test]
#[ignore = "requires running Redis (run with --ignored)"]
async fn test_redis_checkpointer_with_metadata() {
    let connection_string = get_connection_string();
    let checkpointer = RedisCheckpointer::<TestState>::new(&connection_string)
        .await
        .expect("Failed to create RedisCheckpointer");

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
    let thread_id = checkpoint.thread_id.clone();

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
        .delete_thread(&thread_id)
        .await
        .expect("Failed to delete thread");
}

#[tokio::test]
#[ignore = "requires running Redis (run with --ignored)"]
async fn test_redis_checkpointer_concurrent_saves() {
    let connection_string = get_connection_string();
    let checkpointer = RedisCheckpointer::<TestState>::new(&connection_string)
        .await
        .expect("Failed to create RedisCheckpointer");

    let thread_id = "thread_concurrent_test";

    // Spawn multiple concurrent save tasks
    let mut handles = vec![];
    for i in 0..10 {
        let cp = Checkpoint::new(
            thread_id.to_string(),
            TestState {
                value: i,
                message: format!("Checkpoint {}", i),
            },
            format!("node{}", i),
            None,
        );
        let conn_str = connection_string.clone();
        handles.push(tokio::spawn(async move {
            let checkpointer = RedisCheckpointer::<TestState>::new(&conn_str)
                .await
                .expect("Failed to create checkpointer");
            checkpointer.save(cp).await
        }));
    }

    // Wait for all saves to complete
    for handle in handles {
        handle.await.expect("Task failed").expect("Save failed");
    }

    // Verify all checkpoints were saved
    let list = checkpointer
        .list(thread_id)
        .await
        .expect("Failed to list checkpoints");
    assert_eq!(list.len(), 10);

    // Cleanup
    checkpointer
        .delete_thread(thread_id)
        .await
        .expect("Failed to delete thread");
}
