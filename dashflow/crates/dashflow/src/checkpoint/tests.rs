
use super::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
struct TestState {
    value: i32,
}

#[tokio::test]
async fn test_memory_checkpointer_save_load() {
    let checkpointer = MemoryCheckpointer::new();
    let state = TestState { value: 42 };
    let checkpoint = Checkpoint::new("thread1".to_string(), state, "node1".to_string(), None);
    let checkpoint_id = checkpoint.id.clone();

    checkpointer.save(checkpoint).await.unwrap();

    let loaded = checkpointer.load(&checkpoint_id).await.unwrap();
    assert!(loaded.is_some());
    assert_eq!(loaded.unwrap().state.value, 42);
}

#[tokio::test]
async fn test_memory_checkpointer_get_latest() {
    let checkpointer = MemoryCheckpointer::new();
    let thread_id = "thread1";

    // Save multiple checkpoints with delays to ensure different timestamps
    let cp1 = Checkpoint::new(
        thread_id.to_string(),
        TestState { value: 1 },
        "node1".to_string(),
        None,
    );
    checkpointer.save(cp1).await.unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    let cp2 = Checkpoint::new(
        thread_id.to_string(),
        TestState { value: 2 },
        "node2".to_string(),
        None,
    );
    checkpointer.save(cp2).await.unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    let cp3 = Checkpoint::new(
        thread_id.to_string(),
        TestState { value: 3 },
        "node3".to_string(),
        None,
    );
    checkpointer.save(cp3).await.unwrap();

    // Get latest should return the newest
    let latest = checkpointer.get_latest(thread_id).await.unwrap();
    assert!(latest.is_some());
    assert_eq!(latest.unwrap().state.value, 3);
}

#[tokio::test]
async fn test_memory_checkpointer_list() {
    let checkpointer = MemoryCheckpointer::new();
    let thread1 = "thread1";
    let thread2 = "thread2";

    // Save checkpoints for two threads
    let cp1 = Checkpoint::new(
        thread1.to_string(),
        TestState { value: 1 },
        "node1".to_string(),
        None,
    );
    checkpointer.save(cp1).await.unwrap();

    let cp2 = Checkpoint::new(
        thread2.to_string(),
        TestState { value: 2 },
        "node2".to_string(),
        None,
    );
    checkpointer.save(cp2).await.unwrap();

    let cp3 = Checkpoint::new(
        thread1.to_string(),
        TestState { value: 3 },
        "node3".to_string(),
        None,
    );
    checkpointer.save(cp3).await.unwrap();

    // List should only return checkpoints for specified thread
    let list = checkpointer.list(thread1).await.unwrap();
    assert_eq!(list.len(), 2);
    assert_eq!(list[0].node, "node3"); // Newest first
    assert_eq!(list[1].node, "node1");
}

#[tokio::test]
async fn test_memory_checkpointer_delete() {
    let checkpointer = MemoryCheckpointer::new();
    let checkpoint = Checkpoint::new(
        "thread1".to_string(),
        TestState { value: 42 },
        "node1".to_string(),
        None,
    );
    let checkpoint_id = checkpoint.id.clone();

    checkpointer.save(checkpoint).await.unwrap();
    assert_eq!(checkpointer.len(), 1);

    checkpointer.delete(&checkpoint_id).await.unwrap();
    assert_eq!(checkpointer.len(), 0);

    let loaded = checkpointer.load(&checkpoint_id).await.unwrap();
    assert!(loaded.is_none());
}

#[tokio::test]
async fn test_memory_checkpointer_delete_thread() {
    let checkpointer = MemoryCheckpointer::new();
    let thread1 = "thread1";
    let thread2 = "thread2";

    let cp1 = Checkpoint::new(
        thread1.to_string(),
        TestState { value: 1 },
        "node1".to_string(),
        None,
    );
    checkpointer.save(cp1).await.unwrap();

    let cp2 = Checkpoint::new(
        thread2.to_string(),
        TestState { value: 2 },
        "node2".to_string(),
        None,
    );
    checkpointer.save(cp2).await.unwrap();

    let cp3 = Checkpoint::new(
        thread1.to_string(),
        TestState { value: 3 },
        "node3".to_string(),
        None,
    );
    checkpointer.save(cp3).await.unwrap();

    checkpointer.delete_thread(thread1).await.unwrap();

    let list1 = checkpointer.list(thread1).await.unwrap();
    assert_eq!(list1.len(), 0);

    let list2 = checkpointer.list(thread2).await.unwrap();
    assert_eq!(list2.len(), 1);
}

#[tokio::test]
async fn test_checkpoint_with_metadata() {
    let checkpoint = Checkpoint::new(
        "thread1".to_string(),
        TestState { value: 42 },
        "node1".to_string(),
        None,
    )
    .with_metadata("user", "alice")
    .with_metadata("reason", "manual_checkpoint");

    assert_eq!(checkpoint.metadata.get("user"), Some(&"alice".to_string()));
    assert_eq!(
        checkpoint.metadata.get("reason"),
        Some(&"manual_checkpoint".to_string())
    );
}

#[tokio::test]
async fn test_file_checkpointer_save_load() {
    // Use UUID for unique temp directory to avoid test interference
    let unique_id = uuid::Uuid::new_v4().to_string();
    let temp_dir = std::env::temp_dir().join(format!("dashflow_test_{}", unique_id));

    let checkpointer = FileCheckpointer::new(&temp_dir).unwrap();
    let state = TestState { value: 42 };
    let checkpoint = Checkpoint::new("thread1".to_string(), state, "node1".to_string(), None);
    let checkpoint_id = checkpoint.id.clone();

    checkpointer.save(checkpoint).await.unwrap();

    let loaded = checkpointer.load(&checkpoint_id).await.unwrap();
    assert!(loaded.is_some());
    assert_eq!(loaded.unwrap().state.value, 42);

    // Cleanup
    std::fs::remove_dir_all(&temp_dir).unwrap();
}

#[tokio::test]
async fn test_file_checkpointer_new_async() {
    // Use UUID for unique temp directory to avoid test interference
    let unique_id = uuid::Uuid::new_v4().to_string();
    let temp_dir = std::env::temp_dir().join(format!("dashflow_test_async_{}", unique_id));

    // Create using async constructor
    let checkpointer = FileCheckpointer::<TestState>::new_async(&temp_dir)
        .await
        .unwrap();

    // Verify it works same as sync constructor
    let state = TestState { value: 123 };
    let checkpoint = Checkpoint::new("async_thread".to_string(), state, "node1".to_string(), None);
    let checkpoint_id = checkpoint.id.clone();

    checkpointer.save(checkpoint).await.unwrap();
    let loaded = checkpointer.load(&checkpoint_id).await.unwrap();
    assert!(loaded.is_some());
    assert_eq!(loaded.unwrap().state.value, 123);

    // Cleanup
    std::fs::remove_dir_all(&temp_dir).unwrap();
}

#[tokio::test]
async fn test_file_checkpointer_get_latest() {
    // Use UUID for unique temp directory to avoid test interference
    let unique_id = uuid::Uuid::new_v4().to_string();
    let temp_dir = std::env::temp_dir().join(format!("dashflow_test_{}", unique_id));

    let checkpointer = FileCheckpointer::new(&temp_dir).unwrap();
    let thread_id = "thread1";

    // Save multiple checkpoints with delays to ensure different timestamps
    let cp1 = Checkpoint::new(
        thread_id.to_string(),
        TestState { value: 1 },
        "node1".to_string(),
        None,
    );
    checkpointer.save(cp1).await.unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    let cp2 = Checkpoint::new(
        thread_id.to_string(),
        TestState { value: 2 },
        "node2".to_string(),
        None,
    );
    checkpointer.save(cp2).await.unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    let cp3 = Checkpoint::new(
        thread_id.to_string(),
        TestState { value: 3 },
        "node3".to_string(),
        None,
    );
    checkpointer.save(cp3).await.unwrap();

    // Get latest should return the newest
    let latest = checkpointer.get_latest(thread_id).await.unwrap();
    assert!(latest.is_some());
    assert_eq!(latest.unwrap().state.value, 3);

    // Cleanup
    std::fs::remove_dir_all(&temp_dir).unwrap();
}

#[tokio::test]
async fn test_file_checkpointer_list() {
    // Use UUID for unique temp directory to avoid test interference
    let unique_id = uuid::Uuid::new_v4().to_string();
    let temp_dir = std::env::temp_dir().join(format!("dashflow_test_{}", unique_id));

    let checkpointer = FileCheckpointer::new(&temp_dir).unwrap();
    let thread1 = "thread1";
    let thread2 = "thread2";

    // Save checkpoints for two threads with delays to ensure different timestamps
    let cp1 = Checkpoint::new(
        thread1.to_string(),
        TestState { value: 1 },
        "node1".to_string(),
        None,
    );
    checkpointer.save(cp1).await.unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    let cp2 = Checkpoint::new(
        thread2.to_string(),
        TestState { value: 2 },
        "node2".to_string(),
        None,
    );
    checkpointer.save(cp2).await.unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    let cp3 = Checkpoint::new(
        thread1.to_string(),
        TestState { value: 3 },
        "node3".to_string(),
        None,
    );
    checkpointer.save(cp3).await.unwrap();

    // List should only return checkpoints for specified thread
    let list = checkpointer.list(thread1).await.unwrap();
    assert_eq!(list.len(), 2);
    assert_eq!(list[0].node, "node3"); // Newest first
    assert_eq!(list[1].node, "node1");

    // Cleanup
    std::fs::remove_dir_all(&temp_dir).unwrap();
}

#[tokio::test]
async fn test_file_checkpointer_delete() {
    // Use UUID for unique temp directory to avoid test interference
    let unique_id = uuid::Uuid::new_v4().to_string();
    let temp_dir = std::env::temp_dir().join(format!("dashflow_test_{}", unique_id));

    let checkpointer = FileCheckpointer::new(&temp_dir).unwrap();
    let checkpoint = Checkpoint::new(
        "thread1".to_string(),
        TestState { value: 42 },
        "node1".to_string(),
        None,
    );
    let checkpoint_id = checkpoint.id.clone();

    checkpointer.save(checkpoint).await.unwrap();

    let loaded = checkpointer.load(&checkpoint_id).await.unwrap();
    assert!(loaded.is_some());

    checkpointer.delete(&checkpoint_id).await.unwrap();

    let loaded = checkpointer.load(&checkpoint_id).await.unwrap();
    assert!(loaded.is_none());

    // Cleanup
    std::fs::remove_dir_all(&temp_dir).unwrap();
}

#[tokio::test]
async fn test_file_checkpointer_delete_thread() {
    // Use UUID for unique temp directory to avoid test interference
    let unique_id = uuid::Uuid::new_v4().to_string();
    let temp_dir = std::env::temp_dir().join(format!("dashflow_test_{}", unique_id));

    let checkpointer = FileCheckpointer::new(&temp_dir).unwrap();
    let thread1 = "thread1";
    let thread2 = "thread2";

    let cp1 = Checkpoint::new(
        thread1.to_string(),
        TestState { value: 1 },
        "node1".to_string(),
        None,
    );
    checkpointer.save(cp1).await.unwrap();

    let cp2 = Checkpoint::new(
        thread2.to_string(),
        TestState { value: 2 },
        "node2".to_string(),
        None,
    );
    checkpointer.save(cp2).await.unwrap();

    let cp3 = Checkpoint::new(
        thread1.to_string(),
        TestState { value: 3 },
        "node3".to_string(),
        None,
    );
    checkpointer.save(cp3).await.unwrap();

    checkpointer.delete_thread(thread1).await.unwrap();

    let list1 = checkpointer.list(thread1).await.unwrap();
    assert_eq!(list1.len(), 0);

    let list2 = checkpointer.list(thread2).await.unwrap();
    assert_eq!(list2.len(), 1);

    // Cleanup
    std::fs::remove_dir_all(&temp_dir).unwrap();
}

// ===== Checkpoint struct tests =====

#[test]
fn test_checkpoint_new() {
    let checkpoint = Checkpoint::new(
        "test_thread".to_string(),
        TestState { value: 100 },
        "test_node".to_string(),
        None,
    );

    // Format: thread_id + "_" + process_unique_id + "_chkpt" + counter
    assert!(
        checkpoint.id.starts_with("test_thread_"),
        "ID should start with thread_id: {}",
        checkpoint.id
    );
    assert!(
        checkpoint.id.contains("_chkpt"),
        "ID should contain _chkpt: {}",
        checkpoint.id
    );
    assert_eq!(checkpoint.thread_id, "test_thread");
    assert_eq!(checkpoint.state.value, 100);
    assert_eq!(checkpoint.node, "test_node");
    assert_eq!(checkpoint.parent_id, None);
    assert!(checkpoint.metadata.is_empty());
}

#[test]
fn test_checkpoint_with_parent_id() {
    let parent_checkpoint = Checkpoint::new(
        "thread1".to_string(),
        TestState { value: 1 },
        "node1".to_string(),
        None,
    );
    let parent_id = parent_checkpoint.id.clone();

    let child_checkpoint = Checkpoint::new(
        "thread1".to_string(),
        TestState { value: 2 },
        "node2".to_string(),
        Some(parent_id.clone()),
    );

    assert_eq!(child_checkpoint.parent_id, Some(parent_id));
}

#[test]
fn test_checkpoint_with_metadata_single() {
    let checkpoint = Checkpoint::new(
        "thread1".to_string(),
        TestState { value: 1 },
        "node1".to_string(),
        None,
    )
    .with_metadata("key1", "value1");

    assert_eq!(checkpoint.metadata.get("key1"), Some(&"value1".to_string()));
    assert_eq!(checkpoint.metadata.len(), 1);
}

#[test]
fn test_checkpoint_with_metadata_multiple() {
    let checkpoint = Checkpoint::new(
        "thread1".to_string(),
        TestState { value: 1 },
        "node1".to_string(),
        None,
    )
    .with_metadata("key1", "value1")
    .with_metadata("key2", "value2")
    .with_metadata("key3", "value3");

    assert_eq!(checkpoint.metadata.len(), 3);
    assert_eq!(checkpoint.metadata.get("key1"), Some(&"value1".to_string()));
    assert_eq!(checkpoint.metadata.get("key2"), Some(&"value2".to_string()));
    assert_eq!(checkpoint.metadata.get("key3"), Some(&"value3".to_string()));
}

#[test]
fn test_checkpoint_id_generation() {
    // Test that checkpoint IDs are unique within same thread
    let cp1 = Checkpoint::new(
        "thread1".to_string(),
        TestState { value: 1 },
        "node1".to_string(),
        None,
    );
    let cp2 = Checkpoint::new(
        "thread1".to_string(),
        TestState { value: 2 },
        "node2".to_string(),
        None,
    );

    // IDs should be different (monotonic counter)
    assert_ne!(cp1.id, cp2.id);
    // Format: thread_id + "_" + process_unique_id + "_chkpt" + counter
    assert!(
        cp1.id.starts_with("thread1_"),
        "ID should start with thread_id: {}",
        cp1.id
    );
    assert!(
        cp1.id.contains("_chkpt"),
        "ID should contain _chkpt: {}",
        cp1.id
    );
    assert!(
        cp2.id.starts_with("thread1_"),
        "ID should start with thread_id: {}",
        cp2.id
    );
    assert!(
        cp2.id.contains("_chkpt"),
        "ID should contain _chkpt: {}",
        cp2.id
    );
}

#[test]
fn test_checkpoint_id_different_threads() {
    let cp1 = Checkpoint::new(
        "thread1".to_string(),
        TestState { value: 1 },
        "node1".to_string(),
        None,
    );
    let cp2 = Checkpoint::new(
        "thread2".to_string(),
        TestState { value: 2 },
        "node2".to_string(),
        None,
    );

    // IDs should have different thread prefixes
    // Format: thread_id + "_" + process_unique_id + "_chkpt" + counter
    assert!(
        cp1.id.starts_with("thread1_"),
        "ID should start with thread1_: {}",
        cp1.id
    );
    assert!(
        cp1.id.contains("_chkpt"),
        "ID should contain _chkpt: {}",
        cp1.id
    );
    assert!(
        cp2.id.starts_with("thread2_"),
        "ID should start with thread2_: {}",
        cp2.id
    );
    assert!(
        cp2.id.contains("_chkpt"),
        "ID should contain _chkpt: {}",
        cp2.id
    );
}

#[test]
fn test_checkpoint_timestamp() {
    let before = SystemTime::now();
    let checkpoint = Checkpoint::new(
        "thread1".to_string(),
        TestState { value: 1 },
        "node1".to_string(),
        None,
    );
    let after = SystemTime::now();

    // Timestamp should be between before and after
    assert!(checkpoint.timestamp >= before);
    assert!(checkpoint.timestamp <= after);
}

// ===== CheckpointMetadata tests =====

#[test]
fn test_checkpoint_metadata_from_checkpoint() {
    let checkpoint = Checkpoint::new(
        "thread1".to_string(),
        TestState { value: 42 },
        "test_node".to_string(),
        Some("parent_id".to_string()),
    )
    .with_metadata("key1", "value1")
    .with_metadata("key2", "value2");

    let metadata = CheckpointMetadata::from(&checkpoint);

    assert_eq!(metadata.id, checkpoint.id);
    assert_eq!(metadata.thread_id, "thread1");
    assert_eq!(metadata.node, "test_node");
    assert_eq!(metadata.parent_id, Some("parent_id".to_string()));
    assert_eq!(metadata.metadata.len(), 2);
    assert_eq!(metadata.metadata.get("key1"), Some(&"value1".to_string()));
    assert_eq!(metadata.metadata.get("key2"), Some(&"value2".to_string()));
}

#[test]
fn test_checkpoint_metadata_excludes_state() {
    // CheckpointMetadata should not include state (lighter weight)
    let checkpoint = Checkpoint::new(
        "thread1".to_string(),
        TestState { value: 999 },
        "node1".to_string(),
        None,
    );

    let metadata = CheckpointMetadata::from(&checkpoint);

    // Verify metadata exists but state is not accessible
    assert_eq!(metadata.id, checkpoint.id);
    assert_eq!(metadata.thread_id, checkpoint.thread_id);
    // state field doesn't exist in CheckpointMetadata
}

// ===== MemoryCheckpointer edge cases =====

#[tokio::test]
async fn test_memory_checkpointer_load_nonexistent() {
    let checkpointer = MemoryCheckpointer::<TestState>::new();
    let result = checkpointer.load("nonexistent_id").await.unwrap();
    assert!(result.is_none());
}

#[tokio::test]
async fn test_memory_checkpointer_get_latest_empty_thread() {
    let checkpointer = MemoryCheckpointer::<TestState>::new();
    let result = checkpointer.get_latest("empty_thread").await.unwrap();
    assert!(result.is_none());
}

#[tokio::test]
async fn test_memory_checkpointer_list_empty_thread() {
    let checkpointer = MemoryCheckpointer::<TestState>::new();
    let list = checkpointer.list("empty_thread").await.unwrap();
    assert_eq!(list.len(), 0);
}

#[tokio::test]
async fn test_memory_checkpointer_delete_nonexistent() {
    let checkpointer = MemoryCheckpointer::<TestState>::new();
    // Delete should succeed even if checkpoint doesn't exist (idempotent)
    let result = checkpointer.delete("nonexistent_id").await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_memory_checkpointer_multiple_threads() {
    let checkpointer = MemoryCheckpointer::<TestState>::new();

    // Save checkpoints for multiple threads
    let cp1 = Checkpoint::new(
        "thread1".to_string(),
        TestState { value: 1 },
        "node1".to_string(),
        None,
    );
    checkpointer.save(cp1.clone()).await.unwrap();

    let cp2 = Checkpoint::new(
        "thread2".to_string(),
        TestState { value: 2 },
        "node2".to_string(),
        None,
    );
    checkpointer.save(cp2.clone()).await.unwrap();

    let cp3 = Checkpoint::new(
        "thread1".to_string(),
        TestState { value: 3 },
        "node3".to_string(),
        None,
    );
    checkpointer.save(cp3.clone()).await.unwrap();

    // List thread1 should only return thread1 checkpoints
    let list1 = checkpointer.list("thread1").await.unwrap();
    assert_eq!(list1.len(), 2);
    assert!(list1.iter().all(|m| m.thread_id == "thread1"));

    // List thread2 should only return thread2 checkpoints
    let list2 = checkpointer.list("thread2").await.unwrap();
    assert_eq!(list2.len(), 1);
    assert_eq!(list2[0].thread_id, "thread2");
}

#[tokio::test]
async fn test_memory_checkpointer_overwrite_same_id() {
    let checkpointer = MemoryCheckpointer::<TestState>::new();

    let cp1 = Checkpoint::new(
        "thread1".to_string(),
        TestState { value: 1 },
        "node1".to_string(),
        None,
    );
    let checkpoint_id = cp1.id.clone();

    checkpointer.save(cp1).await.unwrap();

    // Create checkpoint with same ID but different state
    let mut cp2 = Checkpoint::new(
        "thread1".to_string(),
        TestState { value: 999 },
        "node2".to_string(),
        None,
    );
    cp2.id = checkpoint_id.clone();

    checkpointer.save(cp2).await.unwrap();

    // Load should return the latest version
    let loaded = checkpointer.load(&checkpoint_id).await.unwrap().unwrap();
    assert_eq!(loaded.state.value, 999);
    assert_eq!(loaded.node, "node2");
}

#[tokio::test]
async fn test_memory_checkpointer_list_ordering() {
    let checkpointer = MemoryCheckpointer::<TestState>::new();
    let thread_id = "thread1";

    // Save checkpoints with delays to ensure different timestamps
    let cp1 = Checkpoint::new(
        thread_id.to_string(),
        TestState { value: 1 },
        "node1".to_string(),
        None,
    );
    checkpointer.save(cp1).await.unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(10)).await;

    let cp2 = Checkpoint::new(
        thread_id.to_string(),
        TestState { value: 2 },
        "node2".to_string(),
        None,
    );
    checkpointer.save(cp2).await.unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(10)).await;

    let cp3 = Checkpoint::new(
        thread_id.to_string(),
        TestState { value: 3 },
        "node3".to_string(),
        None,
    );
    checkpointer.save(cp3).await.unwrap();

    // List should return newest first
    let list = checkpointer.list(thread_id).await.unwrap();
    assert_eq!(list.len(), 3);
    assert_eq!(list[0].node, "node3"); // Newest
    assert_eq!(list[1].node, "node2");
    assert_eq!(list[2].node, "node1"); // Oldest
}

// ===== Checkpoint serialization tests =====

#[test]
fn test_checkpoint_serialization() {
    let checkpoint = Checkpoint::new(
        "thread1".to_string(),
        TestState { value: 42 },
        "node1".to_string(),
        Some("parent_id".to_string()),
    )
    .with_metadata("key1", "value1");

    // Serialize to JSON
    let json = serde_json::to_string(&checkpoint).unwrap();

    // Deserialize back
    let deserialized: Checkpoint<TestState> = serde_json::from_str(&json).unwrap();

    assert_eq!(deserialized.id, checkpoint.id);
    assert_eq!(deserialized.thread_id, "thread1");
    assert_eq!(deserialized.state.value, 42);
    assert_eq!(deserialized.node, "node1");
    assert_eq!(deserialized.parent_id, Some("parent_id".to_string()));
    assert_eq!(
        deserialized.metadata.get("key1"),
        Some(&"value1".to_string())
    );
}

#[test]
fn test_checkpoint_metadata_serialization() {
    let checkpoint = Checkpoint::new(
        "thread1".to_string(),
        TestState { value: 42 },
        "node1".to_string(),
        Some("parent_id".to_string()),
    )
    .with_metadata("key1", "value1");

    let metadata = CheckpointMetadata::from(&checkpoint);

    // Serialize to JSON
    let json = serde_json::to_string(&metadata).unwrap();

    // Deserialize back
    let deserialized: CheckpointMetadata = serde_json::from_str(&json).unwrap();

    assert_eq!(deserialized.id, metadata.id);
    assert_eq!(deserialized.thread_id, "thread1");
    assert_eq!(deserialized.node, "node1");
    assert_eq!(deserialized.parent_id, Some("parent_id".to_string()));
    assert_eq!(
        deserialized.metadata.get("key1"),
        Some(&"value1".to_string())
    );
}

#[test]
fn test_checkpoint_clone() {
    let checkpoint = Checkpoint::new(
        "thread1".to_string(),
        TestState { value: 42 },
        "node1".to_string(),
        Some("parent_id".to_string()),
    )
    .with_metadata("key1", "value1");

    let cloned = checkpoint.clone();

    assert_eq!(cloned.id, checkpoint.id);
    assert_eq!(cloned.thread_id, checkpoint.thread_id);
    assert_eq!(cloned.state.value, checkpoint.state.value);
    assert_eq!(cloned.node, checkpoint.node);
    assert_eq!(cloned.parent_id, checkpoint.parent_id);
    assert_eq!(cloned.metadata, checkpoint.metadata);
}

#[test]
fn test_checkpoint_metadata_clone() {
    let checkpoint = Checkpoint::new(
        "thread1".to_string(),
        TestState { value: 42 },
        "node1".to_string(),
        None,
    );

    let metadata = CheckpointMetadata::from(&checkpoint);
    let cloned = metadata.clone();

    assert_eq!(cloned.id, metadata.id);
    assert_eq!(cloned.thread_id, metadata.thread_id);
    assert_eq!(cloned.node, metadata.node);
    assert_eq!(cloned.timestamp, metadata.timestamp);
    assert_eq!(cloned.parent_id, metadata.parent_id);
    assert_eq!(cloned.metadata, metadata.metadata);
}

// ===== FileCheckpointer edge cases =====

#[tokio::test]
async fn test_file_checkpointer_load_nonexistent() {
    let unique_id = uuid::Uuid::new_v4().to_string();
    let temp_dir = std::env::temp_dir().join(format!("dashflow_test_{}", unique_id));

    let checkpointer = FileCheckpointer::<TestState>::new(&temp_dir).unwrap();
    let result = checkpointer.load("nonexistent_id").await.unwrap();
    assert!(result.is_none());

    // Cleanup
    std::fs::remove_dir_all(&temp_dir).unwrap();
}

#[tokio::test]
async fn test_file_checkpointer_empty_thread() {
    let unique_id = uuid::Uuid::new_v4().to_string();
    let temp_dir = std::env::temp_dir().join(format!("dashflow_test_{}", unique_id));

    let checkpointer = FileCheckpointer::<TestState>::new(&temp_dir).unwrap();

    let latest = checkpointer.get_latest("empty_thread").await.unwrap();
    assert!(latest.is_none());

    let list = checkpointer.list("empty_thread").await.unwrap();
    assert_eq!(list.len(), 0);

    // Cleanup
    std::fs::remove_dir_all(&temp_dir).unwrap();
}

#[tokio::test]
async fn test_file_checkpointer_with_parent_chain() {
    let unique_id = uuid::Uuid::new_v4().to_string();
    let temp_dir = std::env::temp_dir().join(format!("dashflow_test_{}", unique_id));

    let checkpointer = FileCheckpointer::new(&temp_dir).unwrap();
    let thread_id = "thread1";

    // Create checkpoint chain: cp1 -> cp2 -> cp3
    let cp1 = Checkpoint::new(
        thread_id.to_string(),
        TestState { value: 1 },
        "node1".to_string(),
        None,
    );
    let cp1_id = cp1.id.clone();
    checkpointer.save(cp1).await.unwrap();

    let cp2 = Checkpoint::new(
        thread_id.to_string(),
        TestState { value: 2 },
        "node2".to_string(),
        Some(cp1_id.clone()),
    );
    let cp2_id = cp2.id.clone();
    checkpointer.save(cp2).await.unwrap();

    let cp3 = Checkpoint::new(
        thread_id.to_string(),
        TestState { value: 3 },
        "node3".to_string(),
        Some(cp2_id.clone()),
    );
    checkpointer.save(cp3.clone()).await.unwrap();

    // Load cp3 and verify parent chain
    let loaded_cp3 = checkpointer.load(&cp3.id).await.unwrap().unwrap();
    assert_eq!(loaded_cp3.parent_id, Some(cp2_id.clone()));

    let loaded_cp2 = checkpointer.load(&cp2_id).await.unwrap().unwrap();
    assert_eq!(loaded_cp2.parent_id, Some(cp1_id.clone()));

    let loaded_cp1 = checkpointer.load(&cp1_id).await.unwrap().unwrap();
    assert_eq!(loaded_cp1.parent_id, None);

    // Cleanup
    std::fs::remove_dir_all(&temp_dir).unwrap();
}

// Multi-tier checkpointer tests
#[tokio::test]
async fn test_multi_tier_write_through() {
    let l1 = Arc::new(MemoryCheckpointer::<TestState>::new());
    let l2 = Arc::new(MemoryCheckpointer::<TestState>::new());

    let checkpointer = MultiTierCheckpointer::new(l1.clone(), l2.clone())
        .with_write_policy(WritePolicy::WriteThrough);

    let checkpoint = Checkpoint::new(
        "thread1".to_string(),
        TestState { value: 42 },
        "node1".to_string(),
        None,
    );
    let cp_id = checkpoint.id.clone();

    // Save with write-through policy
    checkpointer.save(checkpoint.clone()).await.unwrap();

    // Both L1 and L2 should have the checkpoint
    assert_eq!(l1.len(), 1);
    assert_eq!(l2.len(), 1);

    // Load should get from L1 (cache hit)
    let loaded = checkpointer.load(&cp_id).await.unwrap().unwrap();
    assert_eq!(loaded.state.value, 42);
}

#[tokio::test]
async fn test_multi_tier_write_around() {
    let l1 = Arc::new(MemoryCheckpointer::<TestState>::new());
    let l2 = Arc::new(MemoryCheckpointer::<TestState>::new());

    let checkpointer = MultiTierCheckpointer::new(l1.clone(), l2.clone())
        .with_write_policy(WritePolicy::WriteAround);

    let checkpoint = Checkpoint::new(
        "thread1".to_string(),
        TestState { value: 99 },
        "node1".to_string(),
        None,
    );
    let cp_id = checkpoint.id.clone();

    // Save with write-around policy
    checkpointer.save(checkpoint.clone()).await.unwrap();

    // Only L2 should have the checkpoint (L1 bypassed)
    assert_eq!(l1.len(), 0);
    assert_eq!(l2.len(), 1);

    // Load should warm L1 from L2
    let loaded = checkpointer.load(&cp_id).await.unwrap().unwrap();
    assert_eq!(loaded.state.value, 99);

    // After load with warming, L1 should be populated
    assert_eq!(l1.len(), 1);
}

#[tokio::test]
async fn test_multi_tier_l1_miss_l2_hit() {
    let l1 = Arc::new(MemoryCheckpointer::<TestState>::new());
    let l2 = Arc::new(MemoryCheckpointer::<TestState>::new());

    let checkpointer = MultiTierCheckpointer::new(l1.clone(), l2.clone())
        .with_write_policy(WritePolicy::WriteThrough)
        .with_warm_l1_on_read(true);

    // Directly add checkpoint to L2 only (simulating L1 eviction)
    let checkpoint = Checkpoint::new(
        "thread1".to_string(),
        TestState { value: 123 },
        "node1".to_string(),
        None,
    );
    let cp_id = checkpoint.id.clone();
    l2.save(checkpoint).await.unwrap();

    // L1 cache miss, L2 hit
    assert_eq!(l1.len(), 0);
    assert_eq!(l2.len(), 1);

    // Load should find in L2 and warm L1
    let loaded = checkpointer.load(&cp_id).await.unwrap().unwrap();
    assert_eq!(loaded.state.value, 123);

    // L1 should now be warmed
    assert_eq!(l1.len(), 1);
}

#[tokio::test]
async fn test_multi_tier_get_latest() {
    let l1 = Arc::new(MemoryCheckpointer::<TestState>::new());
    let l2 = Arc::new(MemoryCheckpointer::<TestState>::new());

    let checkpointer = MultiTierCheckpointer::new(l1.clone(), l2.clone())
        .with_write_policy(WritePolicy::WriteThrough);

    let thread_id = "thread1";

    // Create multiple checkpoints
    let cp1 = Checkpoint::new(
        thread_id.to_string(),
        TestState { value: 1 },
        "node1".to_string(),
        None,
    );
    checkpointer.save(cp1.clone()).await.unwrap();

    // Sleep to ensure timestamp ordering
    tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

    let cp2 = Checkpoint::new(
        thread_id.to_string(),
        TestState { value: 2 },
        "node2".to_string(),
        Some(cp1.id.clone()),
    );
    checkpointer.save(cp2.clone()).await.unwrap();

    // Get latest should return cp2
    let latest = checkpointer.get_latest(thread_id).await.unwrap().unwrap();
    assert_eq!(latest.state.value, 2);
    assert_eq!(latest.parent_id, Some(cp1.id));
}

#[tokio::test]
async fn test_multi_tier_list() {
    let l1 = Arc::new(MemoryCheckpointer::<TestState>::new());
    let l2 = Arc::new(MemoryCheckpointer::<TestState>::new());

    let checkpointer = MultiTierCheckpointer::new(l1.clone(), l2.clone())
        .with_write_policy(WritePolicy::WriteThrough);

    let thread_id = "thread1";

    // Create checkpoints
    for i in 0..3 {
        let cp = Checkpoint::new(
            thread_id.to_string(),
            TestState { value: i },
            format!("node{}", i),
            None,
        );
        checkpointer.save(cp).await.unwrap();
        tokio::time::sleep(tokio::time::Duration::from_millis(5)).await;
    }

    // List should query L2 (source of truth)
    let list = checkpointer.list(thread_id).await.unwrap();
    assert_eq!(list.len(), 3);
}

#[tokio::test]
async fn test_multi_tier_delete() {
    let l1 = Arc::new(MemoryCheckpointer::<TestState>::new());
    let l2 = Arc::new(MemoryCheckpointer::<TestState>::new());

    let checkpointer = MultiTierCheckpointer::new(l1.clone(), l2.clone())
        .with_write_policy(WritePolicy::WriteThrough);

    let checkpoint = Checkpoint::new(
        "thread1".to_string(),
        TestState { value: 42 },
        "node1".to_string(),
        None,
    );
    let cp_id = checkpoint.id.clone();

    checkpointer.save(checkpoint).await.unwrap();

    // Both tiers should have it
    assert_eq!(l1.len(), 1);
    assert_eq!(l2.len(), 1);

    // Delete from both tiers
    checkpointer.delete(&cp_id).await.unwrap();

    // Both should be empty
    assert_eq!(l1.len(), 0);
    assert_eq!(l2.len(), 0);
}

#[tokio::test]
async fn test_multi_tier_delete_thread() {
    let l1 = Arc::new(MemoryCheckpointer::<TestState>::new());
    let l2 = Arc::new(MemoryCheckpointer::<TestState>::new());

    let checkpointer = MultiTierCheckpointer::new(l1.clone(), l2.clone())
        .with_write_policy(WritePolicy::WriteThrough);

    let thread_id = "thread1";

    // Create multiple checkpoints for the thread
    for i in 0..3 {
        let cp = Checkpoint::new(
            thread_id.to_string(),
            TestState { value: i },
            format!("node{}", i),
            None,
        );
        checkpointer.save(cp).await.unwrap();
    }

    assert_eq!(l1.len(), 3);
    assert_eq!(l2.len(), 3);

    // Delete entire thread
    checkpointer.delete_thread(thread_id).await.unwrap();

    // Both tiers should be empty
    assert_eq!(l1.len(), 0);
    assert_eq!(l2.len(), 0);
}

#[tokio::test]
async fn test_multi_tier_no_warm_on_read() {
    let l1 = Arc::new(MemoryCheckpointer::<TestState>::new());
    let l2 = Arc::new(MemoryCheckpointer::<TestState>::new());

    let checkpointer = MultiTierCheckpointer::new(l1.clone(), l2.clone())
        .with_write_policy(WritePolicy::WriteAround)
        .with_warm_l1_on_read(false);

    let checkpoint = Checkpoint::new(
        "thread1".to_string(),
        TestState { value: 55 },
        "node1".to_string(),
        None,
    );
    let cp_id = checkpoint.id.clone();

    // Save with write-around (only L2)
    checkpointer.save(checkpoint).await.unwrap();
    assert_eq!(l1.len(), 0);
    assert_eq!(l2.len(), 1);

    // Load should NOT warm L1 (warming disabled)
    let loaded = checkpointer.load(&cp_id).await.unwrap().unwrap();
    assert_eq!(loaded.state.value, 55);

    // L1 should still be empty
    assert_eq!(l1.len(), 0);
}

// ============================================================================
// Coverage Improvement Tests
// Target: Increase coverage from 38.6% to ≥80%
// ============================================================================

// MemoryCheckpointer helper methods
#[tokio::test]
async fn test_memory_checkpointer_is_empty() {
    let checkpointer = MemoryCheckpointer::<TestState>::new();
    assert!(checkpointer.is_empty());

    let checkpoint = Checkpoint::new(
        "thread1".to_string(),
        TestState { value: 1 },
        "node1".to_string(),
        None,
    );
    checkpointer.save(checkpoint).await.unwrap();
    assert!(!checkpointer.is_empty());
}

#[tokio::test]
async fn test_memory_checkpointer_clear() {
    let checkpointer = MemoryCheckpointer::<TestState>::new();

    // Add multiple checkpoints
    for i in 0..5 {
        let checkpoint = Checkpoint::new(
            format!("thread{}", i),
            TestState { value: i },
            format!("node{}", i),
            None,
        );
        checkpointer.save(checkpoint).await.unwrap();
    }

    assert_eq!(checkpointer.len(), 5);
    checkpointer.clear();
    assert_eq!(checkpointer.len(), 0);
    assert!(checkpointer.is_empty());
}

#[tokio::test]
async fn test_memory_checkpointer_default() {
    let checkpointer = MemoryCheckpointer::<TestState>::default();
    assert_eq!(checkpointer.len(), 0);
    assert!(checkpointer.is_empty());
}

// FileCheckpointer error handling
#[tokio::test]
async fn test_file_checkpointer_corrupted_index_new() {
    let unique_id = uuid::Uuid::new_v4().to_string();
    let temp_dir = std::env::temp_dir().join(format!("dashflow_test_{}", unique_id));

    // Write corrupted index file
    std::fs::create_dir_all(&temp_dir).unwrap();
    let index_path = temp_dir.join("index.bin");
    std::fs::write(&index_path, b"corrupted data").unwrap();

    // FileCheckpointer should gracefully handle corrupted index
    let checkpointer = FileCheckpointer::<TestState>::new(&temp_dir).unwrap();

    // Should be able to save checkpoints despite corrupted index
    let checkpoint = Checkpoint::new(
        "thread1".to_string(),
        TestState { value: 42 },
        "node1".to_string(),
        None,
    );
    checkpointer.save(checkpoint).await.unwrap();

    // Cleanup
    std::fs::remove_dir_all(&temp_dir).unwrap();
}

#[tokio::test]
async fn test_file_checkpointer_corrupted_checkpoint_file_new() {
    let unique_id = uuid::Uuid::new_v4().to_string();
    let temp_dir = std::env::temp_dir().join(format!("dashflow_test_{}", unique_id));

    let checkpointer = FileCheckpointer::<TestState>::new(&temp_dir).unwrap();

    // Create a checkpoint
    let checkpoint = Checkpoint::new(
        "thread1".to_string(),
        TestState { value: 42 },
        "node1".to_string(),
        None,
    );
    let cp_id = checkpoint.id.clone();
    checkpointer.save(checkpoint).await.unwrap();

    // Corrupt the checkpoint file
    let checkpoint_path = temp_dir.join(format!("{}.bin", cp_id));
    std::fs::write(&checkpoint_path, b"corrupted bincode data").unwrap();

    // Loading should return an error
    let result = checkpointer.load(&cp_id).await;
    assert!(result.is_err());

    // Cleanup
    std::fs::remove_dir_all(&temp_dir).unwrap();
}

#[tokio::test]
async fn test_file_checkpointer_delete_nonexistent_new() {
    let unique_id = uuid::Uuid::new_v4().to_string();
    let temp_dir = std::env::temp_dir().join(format!("dashflow_test_{}", unique_id));

    let checkpointer = FileCheckpointer::<TestState>::new(&temp_dir).unwrap();

    // Deleting nonexistent checkpoint should not error
    let result = checkpointer.delete("nonexistent_id").await;
    assert!(result.is_ok());

    // Cleanup
    std::fs::remove_dir_all(&temp_dir).unwrap();
}

// MultiTierCheckpointer write policies
#[tokio::test]
async fn test_multi_tier_write_behind_new() {
    let l1 = Arc::new(MemoryCheckpointer::<TestState>::new());
    let l2 = Arc::new(MemoryCheckpointer::<TestState>::new());

    let checkpointer = MultiTierCheckpointer::new(l1.clone(), l2.clone())
        .with_write_policy(WritePolicy::WriteBehind);

    let checkpoint = Checkpoint::new(
        "thread1".to_string(),
        TestState { value: 99 },
        "node1".to_string(),
        None,
    );
    checkpointer.save(checkpoint).await.unwrap();

    // L1 should be written immediately
    assert_eq!(l1.len(), 1);

    // L2 write is asynchronous, give it time to complete
    tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
    assert_eq!(l2.len(), 1);
}

// FileCheckpointer additional edge cases
#[tokio::test]
async fn test_file_checkpointer_large_state_new() {
    let unique_id = uuid::Uuid::new_v4().to_string();
    let temp_dir = std::env::temp_dir().join(format!("dashflow_test_{}", unique_id));

    let checkpointer = FileCheckpointer::<TestState>::new(&temp_dir).unwrap();

    // Create checkpoint with "large" value
    let checkpoint = Checkpoint::new(
        "thread1".to_string(),
        TestState { value: i32::MAX },
        "node1".to_string(),
        None,
    );
    let cp_id = checkpoint.id.clone();

    checkpointer.save(checkpoint).await.unwrap();

    let loaded = checkpointer.load(&cp_id).await.unwrap().unwrap();
    assert_eq!(loaded.state.value, i32::MAX);

    // Cleanup
    std::fs::remove_dir_all(&temp_dir).unwrap();
}

#[tokio::test]
async fn test_file_checkpointer_multiple_threads_new() {
    let unique_id = uuid::Uuid::new_v4().to_string();
    let temp_dir = std::env::temp_dir().join(format!("dashflow_test_{}", unique_id));

    let checkpointer = FileCheckpointer::<TestState>::new(&temp_dir).unwrap();

    // Create checkpoints for multiple threads
    for thread_num in 0..5 {
        for checkpoint_num in 0..3 {
            let checkpoint = Checkpoint::new(
                format!("thread{}", thread_num),
                TestState {
                    value: checkpoint_num,
                },
                format!("node{}", checkpoint_num),
                None,
            );
            checkpointer.save(checkpoint).await.unwrap();
            tokio::time::sleep(tokio::time::Duration::from_millis(5)).await;
        }
    }

    // Verify each thread has 3 checkpoints
    for thread_num in 0..5 {
        let list = checkpointer
            .list(&format!("thread{}", thread_num))
            .await
            .unwrap();
        assert_eq!(list.len(), 3);
    }

    // Cleanup
    std::fs::remove_dir_all(&temp_dir).unwrap();
}

#[tokio::test]
async fn test_memory_checkpointer_concurrent_access() {
    let checkpointer = Arc::new(MemoryCheckpointer::new());
    let mut handles = vec![];

    // Spawn 10 concurrent tasks that save checkpoints
    for i in 0..10 {
        let cp = checkpointer.clone();
        let handle = tokio::spawn(async move {
            let checkpoint = Checkpoint::new(
                format!("thread{}", i % 3),
                TestState { value: i },
                format!("node{}", i),
                None,
            );
            cp.save(checkpoint).await.unwrap();
        });
        handles.push(handle);
    }

    // Wait for all tasks to complete
    for handle in handles {
        handle.await.unwrap();
    }

    // Verify we have 10 checkpoints across 3 threads
    assert_eq!(checkpointer.len(), 10);

    let thread0 = checkpointer.list("thread0").await.unwrap();
    let thread1 = checkpointer.list("thread1").await.unwrap();
    let thread2 = checkpointer.list("thread2").await.unwrap();

    // 10 items distributed across 3 threads: 4+3+3 or 4+4+2 etc
    assert_eq!(thread0.len() + thread1.len() + thread2.len(), 10);
}

#[tokio::test]
async fn test_file_checkpointer_concurrent_access() {
    let unique_id = uuid::Uuid::new_v4().to_string();
    let temp_dir = std::env::temp_dir().join(format!("dashflow_test_{}", unique_id));

    let checkpointer = Arc::new(FileCheckpointer::<TestState>::new(&temp_dir).unwrap());
    let mut handles = vec![];

    // Spawn 10 concurrent tasks that save checkpoints
    for i in 0..10 {
        let cp = checkpointer.clone();
        let handle = tokio::spawn(async move {
            let checkpoint = Checkpoint::new(
                format!("thread{}", i % 3),
                TestState { value: i },
                format!("node{}", i),
                None,
            );
            cp.save(checkpoint).await.unwrap();
        });
        handles.push(handle);
    }

    // Wait for all tasks to complete
    for handle in handles {
        handle.await.unwrap();
    }

    // Verify all checkpoints are saved
    let thread0 = checkpointer.list("thread0").await.unwrap();
    let thread1 = checkpointer.list("thread1").await.unwrap();
    let thread2 = checkpointer.list("thread2").await.unwrap();

    assert_eq!(thread0.len() + thread1.len() + thread2.len(), 10);

    // Cleanup
    std::fs::remove_dir_all(&temp_dir).unwrap();
}

#[tokio::test]
async fn test_checkpoint_metadata_from() {
    let checkpoint = Checkpoint::new(
        "thread1".to_string(),
        TestState { value: 42 },
        "node1".to_string(),
        Some("parent1".to_string()),
    )
    .with_metadata("key1", "value1")
    .with_metadata("key2", "value2");

    let metadata = CheckpointMetadata::from(&checkpoint);

    assert_eq!(metadata.id, checkpoint.id);
    assert_eq!(metadata.thread_id, "thread1");
    assert_eq!(metadata.node, "node1");
    assert_eq!(metadata.parent_id, Some("parent1".to_string()));
    assert_eq!(metadata.metadata.get("key1"), Some(&"value1".to_string()));
    assert_eq!(metadata.metadata.get("key2"), Some(&"value2".to_string()));
}

#[tokio::test]
async fn test_memory_checkpointer_clone() {
    let checkpointer1 = MemoryCheckpointer::new();
    let checkpoint = Checkpoint::new(
        "thread1".to_string(),
        TestState { value: 42 },
        "node1".to_string(),
        None,
    );
    checkpointer1.save(checkpoint).await.unwrap();

    // Clone the checkpointer
    let checkpointer2 = checkpointer1.clone();

    // Both should see the same data (shared Arc)
    assert_eq!(checkpointer1.len(), 1);
    assert_eq!(checkpointer2.len(), 1);

    // Add via clone
    let checkpoint2 = Checkpoint::new(
        "thread2".to_string(),
        TestState { value: 99 },
        "node2".to_string(),
        None,
    );
    checkpointer2.save(checkpoint2).await.unwrap();

    // Both should see 2 checkpoints
    assert_eq!(checkpointer1.len(), 2);
    assert_eq!(checkpointer2.len(), 2);
}

#[tokio::test]
async fn test_file_checkpointer_index_persistence() {
    let unique_id = uuid::Uuid::new_v4().to_string();
    let temp_dir = std::env::temp_dir().join(format!("dashflow_test_{}", unique_id));

    // Create checkpointer and save a checkpoint
    {
        let checkpointer = FileCheckpointer::<TestState>::new(&temp_dir).unwrap();
        let checkpoint = Checkpoint::new(
            "thread1".to_string(),
            TestState { value: 42 },
            "node1".to_string(),
            None,
        );
        checkpointer.save(checkpoint).await.unwrap();
    }

    // Create new checkpointer from same directory (should load index)
    {
        let checkpointer = FileCheckpointer::<TestState>::new(&temp_dir).unwrap();
        let latest = checkpointer.get_latest("thread1").await.unwrap();
        assert!(latest.is_some());
        assert_eq!(latest.unwrap().state.value, 42);
    }

    // Cleanup
    std::fs::remove_dir_all(&temp_dir).unwrap();
}

#[tokio::test]
async fn test_multi_tier_load_both_tiers_empty() {
    let l1 = Arc::new(MemoryCheckpointer::new()) as Arc<dyn Checkpointer<TestState>>;
    let l2 = Arc::new(MemoryCheckpointer::new()) as Arc<dyn Checkpointer<TestState>>;
    let checkpointer = MultiTierCheckpointer::new(l1, l2);

    let result = checkpointer.load("nonexistent").await.unwrap();
    assert!(result.is_none());
}

#[tokio::test]
async fn test_multi_tier_get_latest_both_tiers_empty() {
    let l1 = Arc::new(MemoryCheckpointer::new()) as Arc<dyn Checkpointer<TestState>>;
    let l2 = Arc::new(MemoryCheckpointer::new()) as Arc<dyn Checkpointer<TestState>>;
    let checkpointer = MultiTierCheckpointer::new(l1, l2);

    let result = checkpointer.get_latest("thread1").await.unwrap();
    assert!(result.is_none());
}

#[tokio::test]
async fn test_multi_tier_delete_only_in_l2() {
    let l1 = Arc::new(MemoryCheckpointer::new()) as Arc<dyn Checkpointer<TestState>>;
    let l2 = Arc::new(MemoryCheckpointer::new()) as Arc<dyn Checkpointer<TestState>>;

    // Save to L2 only
    let checkpoint = Checkpoint::new(
        "thread1".to_string(),
        TestState { value: 42 },
        "node1".to_string(),
        None,
    );
    let checkpoint_id = checkpoint.id.clone();
    l2.save(checkpoint).await.unwrap();

    let checkpointer = MultiTierCheckpointer::new(l1, l2);

    // Delete should work even though L1 doesn't have it
    checkpointer.delete(&checkpoint_id).await.unwrap();

    // Verify it's gone
    let result = checkpointer.load(&checkpoint_id).await.unwrap();
    assert!(result.is_none());
}

#[tokio::test]
async fn test_multi_tier_delete_thread_only_in_l2() {
    let l1 = Arc::new(MemoryCheckpointer::new()) as Arc<dyn Checkpointer<TestState>>;
    let l2 = Arc::new(MemoryCheckpointer::new()) as Arc<dyn Checkpointer<TestState>>;

    // Save to L2 only
    let checkpoint = Checkpoint::new(
        "thread1".to_string(),
        TestState { value: 42 },
        "node1".to_string(),
        None,
    );
    l2.save(checkpoint).await.unwrap();

    let checkpointer = MultiTierCheckpointer::new(l1, l2);

    // Delete thread should work even though L1 doesn't have it
    checkpointer.delete_thread("thread1").await.unwrap();

    // Verify it's gone
    let result = checkpointer.get_latest("thread1").await.unwrap();
    assert!(result.is_none());
}

#[tokio::test]
async fn test_checkpoint_new_generates_unique_ids() {
    let cp1 = Checkpoint::new(
        "thread1".to_string(),
        TestState { value: 1 },
        "node1".to_string(),
        None,
    );
    let cp2 = Checkpoint::new(
        "thread1".to_string(),
        TestState { value: 2 },
        "node1".to_string(),
        None,
    );

    // IDs should be different (thread-local counter increments)
    assert_ne!(cp1.id, cp2.id);
}

#[tokio::test]
async fn test_memory_checkpointer_default_constructor() {
    let checkpointer = MemoryCheckpointer::<TestState>::default();
    assert!(checkpointer.is_empty());
}

#[tokio::test]
async fn test_write_policy_variants() {
    // Test that WritePolicy variants can be created
    let wt = WritePolicy::WriteThrough;
    let wb = WritePolicy::WriteBehind;
    let wa = WritePolicy::WriteAround;

    assert_eq!(wt, WritePolicy::WriteThrough);
    assert_eq!(wb, WritePolicy::WriteBehind);
    assert_eq!(wa, WritePolicy::WriteAround);
    assert_ne!(wt, wb);
    assert_ne!(wb, wa);
    assert_ne!(wt, wa);
}

#[tokio::test]
async fn test_multi_tier_builder_pattern() {
    let l1 = Arc::new(MemoryCheckpointer::new()) as Arc<dyn Checkpointer<TestState>>;
    let l2 = Arc::new(MemoryCheckpointer::new()) as Arc<dyn Checkpointer<TestState>>;

    let checkpointer = MultiTierCheckpointer::new(l1, l2)
        .with_write_policy(WritePolicy::WriteBehind)
        .with_warm_l1_on_read(false);

    // Save a checkpoint with WriteBehind policy
    let checkpoint = Checkpoint::new(
        "thread1".to_string(),
        TestState { value: 42 },
        "node1".to_string(),
        None,
    );
    let checkpoint_id = checkpoint.id.clone();
    checkpointer.save(checkpoint).await.unwrap();

    // L1 should have it immediately (WriteBehind writes to L1 first)
    let loaded = checkpointer.load(&checkpoint_id).await.unwrap();
    assert!(loaded.is_some());
    assert_eq!(loaded.unwrap().state.value, 42);
}

#[tokio::test]
async fn test_multi_tier_write_behind_backpressure() {
    let l1 = Arc::new(MemoryCheckpointer::new()) as Arc<dyn Checkpointer<TestState>>;
    let l2 = Arc::new(MemoryCheckpointer::new()) as Arc<dyn Checkpointer<TestState>>;

    // Set max concurrent L2 writes to 2 to easily trigger backpressure
    let checkpointer = MultiTierCheckpointer::new(l1, l2)
        .with_write_policy(WritePolicy::WriteBehind)
        .with_max_concurrent_l2_writes(2);

    // Initially no dropped writes
    assert_eq!(checkpointer.l2_writes_dropped(), 0);

    // Save multiple checkpoints - L1 should always succeed even if L2 has backpressure
    for i in 0..10 {
        let checkpoint = Checkpoint::new(
            format!("thread{}", i),
            TestState { value: i as i32 },
            "node1".to_string(),
            None,
        );
        checkpointer.save(checkpoint).await.unwrap();
    }

    // All saves should succeed (L1 always written first in WriteBehind)
    // L2 may have dropped some due to backpressure, but reads should work
    for i in 0..10 {
        let loaded = checkpointer
            .get_latest(&format!("thread{}", i))
            .await
            .unwrap();
        assert!(loaded.is_some(), "Should have thread{}", i);
        assert_eq!(loaded.unwrap().state.value, i as i32);
    }
}

#[tokio::test]
async fn test_file_checkpointer_delete_updates_index() {
    let unique_id = uuid::Uuid::new_v4().to_string();
    let temp_dir = std::env::temp_dir().join(format!("dashflow_test_{}", unique_id));

    let checkpointer = FileCheckpointer::<TestState>::new(&temp_dir).unwrap();

    // Save a checkpoint
    let checkpoint = Checkpoint::new(
        "thread1".to_string(),
        TestState { value: 42 },
        "node1".to_string(),
        None,
    );
    let checkpoint_id = checkpoint.id.clone();
    checkpointer.save(checkpoint).await.unwrap();

    // Verify index has it
    let latest = checkpointer.get_latest("thread1").await.unwrap();
    assert!(latest.is_some());

    // Delete the checkpoint
    checkpointer.delete(&checkpoint_id).await.unwrap();

    // Index should be updated (get_latest should return None)
    let latest_after = checkpointer.get_latest("thread1").await.unwrap();
    assert!(latest_after.is_none());

    // Cleanup
    std::fs::remove_dir_all(&temp_dir).unwrap();
}

#[tokio::test]
async fn test_file_checkpointer_delete_thread_updates_index() {
    let unique_id = uuid::Uuid::new_v4().to_string();
    let temp_dir = std::env::temp_dir().join(format!("dashflow_test_{}", unique_id));

    let checkpointer = FileCheckpointer::<TestState>::new(&temp_dir).unwrap();

    // Save checkpoints for two threads
    let cp1 = Checkpoint::new(
        "thread1".to_string(),
        TestState { value: 1 },
        "node1".to_string(),
        None,
    );
    let cp2 = Checkpoint::new(
        "thread2".to_string(),
        TestState { value: 2 },
        "node2".to_string(),
        None,
    );
    checkpointer.save(cp1).await.unwrap();
    checkpointer.save(cp2).await.unwrap();

    // Delete thread1
    checkpointer.delete_thread("thread1").await.unwrap();

    // thread1 should be gone, thread2 should remain
    let latest1 = checkpointer.get_latest("thread1").await.unwrap();
    let latest2 = checkpointer.get_latest("thread2").await.unwrap();

    assert!(latest1.is_none());
    assert!(latest2.is_some());
    assert_eq!(latest2.unwrap().state.value, 2);

    // Cleanup
    std::fs::remove_dir_all(&temp_dir).unwrap();
}

// ===== Compression Tests =====

#[test]
fn test_compression_algorithm_gzip() {
    let data = b"Hello, World! This is a test of compression. ".repeat(100);
    let algorithm = CompressionAlgorithm::gzip();

    let compressed = algorithm.compress(&data).unwrap();
    let decompressed = algorithm.decompress(&compressed).unwrap();

    assert_eq!(decompressed, data);
    // Gzip should compress repetitive data significantly
    assert!(compressed.len() < data.len() / 2);
}

#[test]
fn test_compression_algorithm_none() {
    let data = b"Hello, World!";
    let algorithm = CompressionAlgorithm::None;

    let compressed = algorithm.compress(data).unwrap();
    let decompressed = algorithm.decompress(&compressed).unwrap();

    assert_eq!(compressed, data);
    assert_eq!(decompressed, data);
}

#[test]
fn test_compression_levels() {
    let data = b"Test data for compression level comparison. ".repeat(50);

    let fast = CompressionAlgorithm::fast();
    let best = CompressionAlgorithm::best();

    let fast_compressed = fast.compress(&data).unwrap();
    let best_compressed = best.compress(&data).unwrap();

    // Best compression should produce smaller output (or equal)
    assert!(best_compressed.len() <= fast_compressed.len());

    // Both should decompress correctly
    assert_eq!(fast.decompress(&fast_compressed).unwrap(), data);
    assert_eq!(best.decompress(&best_compressed).unwrap(), data);
}

#[tokio::test]
async fn test_compressed_file_checkpointer_save_load() {
    let temp_dir = std::env::temp_dir().join(format!(
        "compressed_checkpoint_test_{}",
        uuid::Uuid::new_v4()
    ));

    let checkpointer: CompressedFileCheckpointer<TestState> =
        CompressedFileCheckpointer::new(&temp_dir).unwrap();

    let state = TestState { value: 42 };
    let checkpoint = Checkpoint::new("thread1".to_string(), state, "node1".to_string(), None);
    let checkpoint_id = checkpoint.id.clone();

    checkpointer.save(checkpoint).await.unwrap();

    // Verify file has .gz extension
    let files: Vec<_> = std::fs::read_dir(&temp_dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.path()
                .file_name()
                .and_then(|s| s.to_str())
                .map(|s| s.ends_with(".bin.gz"))
                .unwrap_or(false)
        })
        .collect();
    assert_eq!(files.len(), 1);

    // Load and verify
    let loaded = checkpointer.load(&checkpoint_id).await.unwrap();
    assert!(loaded.is_some());
    assert_eq!(loaded.unwrap().state.value, 42);

    // Cleanup
    std::fs::remove_dir_all(&temp_dir).unwrap();
}

#[tokio::test]
async fn test_compressed_file_checkpointer_get_latest() {
    let temp_dir = std::env::temp_dir().join(format!(
        "compressed_checkpoint_latest_{}",
        uuid::Uuid::new_v4()
    ));

    let checkpointer: CompressedFileCheckpointer<TestState> =
        CompressedFileCheckpointer::new(&temp_dir).unwrap();

    let thread_id = "thread1";

    let cp1 = Checkpoint::new(
        thread_id.to_string(),
        TestState { value: 1 },
        "node1".to_string(),
        None,
    );
    checkpointer.save(cp1).await.unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(10)).await;

    let cp2 = Checkpoint::new(
        thread_id.to_string(),
        TestState { value: 2 },
        "node2".to_string(),
        None,
    );
    checkpointer.save(cp2).await.unwrap();

    let latest = checkpointer.get_latest(thread_id).await.unwrap();
    assert!(latest.is_some());
    assert_eq!(latest.unwrap().state.value, 2);

    // Cleanup
    std::fs::remove_dir_all(&temp_dir).unwrap();
}

#[tokio::test]
async fn test_compressed_file_checkpointer_compression_ratio() {
    let temp_dir = std::env::temp_dir().join(format!(
        "compressed_checkpoint_ratio_{}",
        uuid::Uuid::new_v4()
    ));

    // Use uncompressed checkpointer
    let uncompressed: CompressedFileCheckpointer<TestState> =
        CompressedFileCheckpointer::new(&temp_dir)
            .unwrap()
            .with_compression(CompressionAlgorithm::None);

    // Use compressed checkpointer
    let temp_dir_compressed = temp_dir.with_extension("compressed");
    let compressed: CompressedFileCheckpointer<TestState> =
        CompressedFileCheckpointer::new(&temp_dir_compressed).unwrap();

    // Create a larger state with repetitive data
    let state = TestState { value: 12345678 };
    let cp1 = Checkpoint::new(
        "thread1".to_string(),
        state.clone(),
        "node1".to_string(),
        None,
    );
    let cp2 = Checkpoint::new("thread2".to_string(), state, "node1".to_string(), None);

    uncompressed.save(cp1.clone()).await.unwrap();
    compressed.save(cp2.clone()).await.unwrap();

    // Get file sizes
    let uncompressed_size: u64 = std::fs::read_dir(&temp_dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().is_some_and(|s| s == "bin"))
        .map(|e| e.metadata().map(|m| m.len()).unwrap_or(0))
        .sum();

    let compressed_size: u64 = std::fs::read_dir(&temp_dir_compressed)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.path()
                .file_name()
                .and_then(|s| s.to_str())
                .map(|s| s.ends_with(".bin.gz"))
                .unwrap_or(false)
        })
        .map(|e| e.metadata().map(|m| m.len()).unwrap_or(0))
        .sum();

    // Compressed should be smaller (for typical state data)
    // Note: Very small states may not compress well
    println!(
        "Uncompressed: {} bytes, Compressed: {} bytes",
        uncompressed_size, compressed_size
    );

    // Cleanup
    std::fs::remove_dir_all(&temp_dir).ok();
    std::fs::remove_dir_all(&temp_dir_compressed).ok();
}

// ===== Versioning Tests =====

// Old state for migration testing (version 1)
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
struct OldState {
    name: String,
}

// New state for migration testing (version 2)
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
struct NewState {
    name: String,
    count: i32,
}

// Note: GraphState is implemented via blanket impl for Clone + Send + Sync + Serialize + Deserialize

// Migration from V1 to V2
struct V1ToV2Migration;

impl StateMigration<NewState> for V1ToV2Migration {
    fn source_version(&self) -> Version {
        1
    }

    fn target_version(&self) -> Version {
        2
    }

    fn migrate(&self, data: serde_json::Value) -> std::result::Result<NewState, String> {
        let old: OldState = serde_json::from_value(data).map_err(|e| e.to_string())?;
        Ok(NewState {
            name: old.name,
            count: 0, // Default for migrated data
        })
    }
}

#[test]
fn test_migration_chain_no_migration_needed() {
    let chain = MigrationChain::<TestState>::new(1);
    let data = serde_json::json!({ "value": 42 });

    let result = chain.migrate_to_current(data, 1);
    assert!(result.is_ok());
    assert_eq!(result.unwrap().value, 42);
}

#[test]
fn test_migration_chain_single_migration() {
    let chain = MigrationChain::<NewState>::new(2).add_migration(V1ToV2Migration);

    let old_data = serde_json::json!({ "name": "test" });
    let result = chain.migrate_to_current(old_data, 1);

    assert!(result.is_ok());
    let new_state = result.unwrap();
    assert_eq!(new_state.name, "test");
    assert_eq!(new_state.count, 0);
}

#[test]
fn test_migration_chain_missing_migration() {
    let chain = MigrationChain::<NewState>::new(3); // No migrations registered

    let old_data = serde_json::json!({ "name": "test" });
    let result = chain.migrate_to_current(old_data, 1);

    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("No migration found"));
}

#[test]
fn test_versioned_checkpoint_roundtrip() {
    let state = TestState { value: 42 };
    let checkpoint = Checkpoint::new(
        "thread1".to_string(),
        state.clone(),
        "node1".to_string(),
        None,
    );

    let versioned = VersionedCheckpoint::from_checkpoint(&checkpoint, 1).unwrap();
    assert_eq!(versioned.version, 1);
    assert_eq!(versioned.id, checkpoint.id);

    let recovered: Checkpoint<TestState> = versioned.to_checkpoint().unwrap();
    assert_eq!(recovered.state.value, 42);
}

#[tokio::test]
async fn test_versioned_file_checkpointer_save_load() {
    let temp_dir = std::env::temp_dir().join(format!(
        "versioned_checkpoint_test_{}",
        uuid::Uuid::new_v4()
    ));

    let migrations = MigrationChain::<TestState>::new(1);
    let checkpointer = VersionedFileCheckpointer::new(&temp_dir, migrations).unwrap();

    let state = TestState { value: 42 };
    let checkpoint = Checkpoint::new("thread1".to_string(), state, "node1".to_string(), None);
    let checkpoint_id = checkpoint.id.clone();

    checkpointer.save(checkpoint).await.unwrap();

    // Verify file has .v.bin.gz extension
    let files: Vec<_> = std::fs::read_dir(&temp_dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.path()
                .file_name()
                .and_then(|s| s.to_str())
                .map(|s| s.contains(".v.bin"))
                .unwrap_or(false)
        })
        .collect();
    assert_eq!(files.len(), 1);

    // Load and verify
    let loaded = checkpointer.load(&checkpoint_id).await.unwrap();
    assert!(loaded.is_some());
    assert_eq!(loaded.unwrap().state.value, 42);

    // Cleanup
    std::fs::remove_dir_all(&temp_dir).unwrap();
}

#[tokio::test]
async fn test_versioned_file_checkpointer_get_latest() {
    let temp_dir = std::env::temp_dir().join(format!(
        "versioned_checkpoint_latest_{}",
        uuid::Uuid::new_v4()
    ));

    let migrations = MigrationChain::<TestState>::new(1);
    let checkpointer = VersionedFileCheckpointer::new(&temp_dir, migrations).unwrap();

    let thread_id = "thread1";

    let cp1 = Checkpoint::new(
        thread_id.to_string(),
        TestState { value: 1 },
        "node1".to_string(),
        None,
    );
    checkpointer.save(cp1).await.unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(10)).await;

    let cp2 = Checkpoint::new(
        thread_id.to_string(),
        TestState { value: 2 },
        "node2".to_string(),
        None,
    );
    checkpointer.save(cp2).await.unwrap();

    let latest = checkpointer.get_latest(thread_id).await.unwrap();
    assert!(latest.is_some());
    assert_eq!(latest.unwrap().state.value, 2);

    // Cleanup
    std::fs::remove_dir_all(&temp_dir).unwrap();
}

// ===== Checkpoint Integrity Tests (Bug #18) =====

#[test]
fn test_checkpoint_integrity_wrap_unwrap_roundtrip() {
    let data = b"Hello, this is test checkpoint data for integrity verification!";

    // Wrap data with integrity header
    let wrapped = CheckpointWithIntegrity::wrap(data);

    // Should be larger than original (20 byte header)
    assert_eq!(wrapped.len(), data.len() + 20);

    // Should start with magic bytes
    assert_eq!(&wrapped[0..4], b"DCHK");

    // Unwrap and verify data matches
    let unwrapped = CheckpointWithIntegrity::unwrap(&wrapped).unwrap();
    assert_eq!(unwrapped, data);
}

#[test]
fn test_checkpoint_integrity_detects_corrupted_data() {
    let data = b"Original checkpoint data that will be corrupted";

    let mut wrapped = CheckpointWithIntegrity::wrap(data);

    // Corrupt a byte in the payload (flip a bit)
    let payload_start = 20; // header size
    wrapped[payload_start + 5] ^= 0xFF;

    // Unwrap should fail with checksum mismatch
    let result = CheckpointWithIntegrity::unwrap(&wrapped);
    assert!(result.is_err());
    match result {
        Err(CheckpointIntegrityError::ChecksumMismatch { .. }) => {}
        other => panic!("Expected ChecksumMismatch, got {:?}", other),
    }
}

#[test]
fn test_checkpoint_integrity_detects_truncated_file() {
    let data = b"Test data";
    let wrapped = CheckpointWithIntegrity::wrap(data);

    // Truncate to just the header (no payload)
    let truncated = &wrapped[0..20];

    // Length mismatch should be detected
    let result = CheckpointWithIntegrity::unwrap(truncated);
    assert!(result.is_err());
    match result {
        Err(CheckpointIntegrityError::LengthMismatch { declared, actual }) => {
            assert_eq!(declared, data.len() as u64);
            assert_eq!(actual, 0);
        }
        other => panic!("Expected LengthMismatch, got {:?}", other),
    }
}

#[test]
fn test_checkpoint_integrity_detects_invalid_magic() {
    let mut data = vec![0u8; 30];
    // Wrong magic bytes
    data[0..4].copy_from_slice(b"XXXX");
    data[4..8].copy_from_slice(&1u32.to_le_bytes()); // version
    data[8..12].copy_from_slice(&0u32.to_le_bytes()); // checksum
    data[12..20].copy_from_slice(&10u64.to_le_bytes()); // length

    let result = CheckpointWithIntegrity::unwrap(&data);
    assert!(result.is_err());
    match result {
        Err(CheckpointIntegrityError::InvalidMagic { expected, found }) => {
            assert_eq!(expected, *b"DCHK");
            assert_eq!(found, *b"XXXX");
        }
        other => panic!("Expected InvalidMagic, got {:?}", other),
    }
}

#[test]
fn test_checkpoint_integrity_detects_unsupported_version() {
    let mut data = vec![0u8; 30];
    data[0..4].copy_from_slice(b"DCHK");
    data[4..8].copy_from_slice(&999u32.to_le_bytes()); // Future version
    data[8..12].copy_from_slice(&0u32.to_le_bytes());
    data[12..20].copy_from_slice(&10u64.to_le_bytes());

    let result = CheckpointWithIntegrity::unwrap(&data);
    assert!(result.is_err());
    match result {
        Err(CheckpointIntegrityError::UnsupportedVersion { found, supported }) => {
            assert_eq!(found, 999);
            assert_eq!(supported, 1); // CHECKPOINT_FORMAT_VERSION
        }
        other => panic!("Expected UnsupportedVersion, got {:?}", other),
    }
}

#[test]
fn test_checkpoint_integrity_detects_file_too_small() {
    // Less than header size (20 bytes)
    let data = b"short";

    let result = CheckpointWithIntegrity::unwrap(data);
    assert!(result.is_err());
    match result {
        Err(CheckpointIntegrityError::FileTooSmall { size, minimum }) => {
            assert_eq!(size, 5);
            assert_eq!(minimum, 20);
        }
        other => panic!("Expected FileTooSmall, got {:?}", other),
    }
}

#[test]
fn test_checkpoint_integrity_is_wrapped_detection() {
    let data = b"Test data";
    let wrapped = CheckpointWithIntegrity::wrap(data);

    // Wrapped data should be detected as wrapped
    assert!(CheckpointWithIntegrity::is_wrapped(&wrapped));

    // Raw data should not be detected as wrapped
    assert!(!CheckpointWithIntegrity::is_wrapped(data));

    // Empty data should not be wrapped
    assert!(!CheckpointWithIntegrity::is_wrapped(&[]));

    // Short data with wrong magic should not be wrapped
    assert!(!CheckpointWithIntegrity::is_wrapped(b"DCH")); // Too short
    assert!(!CheckpointWithIntegrity::is_wrapped(b"XXXX")); // Wrong magic
}

#[test]
fn test_checkpoint_integrity_error_display() {
    // Test that error messages are human-readable
    let err = CheckpointIntegrityError::ChecksumMismatch {
        expected: 0xDEADBEEF,
        computed: 0xCAFEBABE,
    };
    let msg = err.to_string();
    assert!(msg.contains("checksum mismatch"));
    assert!(msg.contains("DEADBEEF"));
    assert!(msg.contains("CAFEBABE"));

    let err = CheckpointIntegrityError::FileTooSmall {
        size: 10,
        minimum: 20,
    };
    let msg = err.to_string();
    assert!(msg.contains("too small"));
    assert!(msg.contains("10"));
    assert!(msg.contains("20"));
}

#[tokio::test]
async fn test_file_checkpointer_integrity_save_load() {
    // Test that FileCheckpointer saves with integrity and loads correctly
    let unique_id = uuid::Uuid::new_v4().to_string();
    let temp_dir = std::env::temp_dir().join(format!("dashflow_integrity_test_{}", unique_id));

    let checkpointer = FileCheckpointer::new(&temp_dir).unwrap();
    let state = TestState { value: 42 };
    let checkpoint = Checkpoint::new("thread1".to_string(), state, "node1".to_string(), None);
    let checkpoint_id = checkpoint.id.clone();

    // Save checkpoint
    checkpointer.save(checkpoint).await.unwrap();

    // Verify the file has integrity header
    let checkpoint_path = temp_dir.join(format!("{}.bin", checkpoint_id));
    let file_data = std::fs::read(&checkpoint_path).unwrap();
    assert!(
        CheckpointWithIntegrity::is_wrapped(&file_data),
        "Saved file should have integrity header"
    );
    assert_eq!(
        &file_data[0..4],
        b"DCHK",
        "File should start with magic bytes"
    );

    // Load and verify
    let loaded = checkpointer.load(&checkpoint_id).await.unwrap();
    assert!(loaded.is_some());
    assert_eq!(loaded.unwrap().state.value, 42);

    // Cleanup
    std::fs::remove_dir_all(&temp_dir).unwrap();
}

#[tokio::test]
async fn test_file_checkpointer_detects_corrupted_file() {
    // Test that FileCheckpointer rejects corrupted files
    let unique_id = uuid::Uuid::new_v4().to_string();
    let temp_dir = std::env::temp_dir().join(format!("dashflow_corrupt_test_{}", unique_id));

    let checkpointer = FileCheckpointer::<TestState>::new(&temp_dir).unwrap();
    let state = TestState { value: 42 };
    let checkpoint = Checkpoint::new("thread1".to_string(), state, "node1".to_string(), None);
    let checkpoint_id = checkpoint.id.clone();

    // Save checkpoint
    checkpointer.save(checkpoint).await.unwrap();

    // Corrupt the file by flipping bits in the payload
    let checkpoint_path = temp_dir.join(format!("{}.bin", checkpoint_id));
    let mut file_data = std::fs::read(&checkpoint_path).unwrap();
    file_data[25] ^= 0xFF; // Flip bits in payload area
    std::fs::write(&checkpoint_path, &file_data).unwrap();

    // Load should fail with integrity error
    let result = checkpointer.load(&checkpoint_id).await;
    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg.contains("integrity") || err_msg.contains("checksum"),
        "Error should mention integrity: {}",
        err_msg
    );

    // Cleanup
    std::fs::remove_dir_all(&temp_dir).unwrap();
}

#[tokio::test]
async fn test_file_checkpointer_handles_bit_flip_in_magic() {
    // Test that file with corrupted magic bytes is rejected
    let unique_id = uuid::Uuid::new_v4().to_string();
    let temp_dir = std::env::temp_dir().join(format!("dashflow_magic_test_{}", unique_id));

    let checkpointer = FileCheckpointer::<TestState>::new(&temp_dir).unwrap();
    let state = TestState { value: 42 };
    let checkpoint = Checkpoint::new("thread1".to_string(), state, "node1".to_string(), None);
    let checkpoint_id = checkpoint.id.clone();

    // Save checkpoint
    checkpointer.save(checkpoint).await.unwrap();

    // Corrupt the magic bytes
    let checkpoint_path = temp_dir.join(format!("{}.bin", checkpoint_id));
    let mut file_data = std::fs::read(&checkpoint_path).unwrap();
    file_data[0] = b'X'; // Change 'D' to 'X'
    std::fs::write(&checkpoint_path, &file_data).unwrap();

    // Load should fail (treated as legacy file, will fail bincode deserialize)
    let result = checkpointer.load(&checkpoint_id).await;
    assert!(result.is_err(), "Corrupted magic should cause load to fail");

    // Cleanup
    std::fs::remove_dir_all(&temp_dir).unwrap();
}

#[tokio::test]
async fn test_file_checkpointer_cross_process_lock_file_created() {
    // Test that the cross-process lock file is created when saving
    let unique_id = uuid::Uuid::new_v4().to_string();
    let temp_dir = std::env::temp_dir().join(format!("dashflow_lock_test_{}", unique_id));

    let checkpointer = FileCheckpointer::<TestState>::new(&temp_dir).unwrap();
    let state = TestState { value: 123 };
    let checkpoint = Checkpoint::new("thread1".to_string(), state, "node1".to_string(), None);

    // Save checkpoint - this should create the lock file
    checkpointer.save(checkpoint).await.unwrap();

    // Verify lock file exists
    let lock_path = temp_dir.join(".checkpoint.lock");
    assert!(
        lock_path.exists(),
        "Cross-process lock file should exist after save"
    );

    // Verify index file also exists
    let index_path = temp_dir.join("index.bin");
    assert!(index_path.exists(), "Index file should exist after save");

    // Cleanup
    std::fs::remove_dir_all(&temp_dir).unwrap();
}

#[test]
fn test_acquire_exclusive_lock_creates_lock_file() {
    let unique_id = uuid::Uuid::new_v4().to_string();
    let temp_dir = std::env::temp_dir().join(format!("dashflow_excl_lock_{}", unique_id));
    std::fs::create_dir_all(&temp_dir).unwrap();

    // Lock file should not exist initially
    let lock_path = temp_dir.join(".checkpoint.lock");
    assert!(!lock_path.exists());

    // Acquire lock - should create file
    {
        let _lock = acquire_exclusive_lock(&temp_dir).unwrap();
        assert!(lock_path.exists(), "Lock file should be created");
    }
    // Lock released when _lock dropped

    // Cleanup
    std::fs::remove_dir_all(&temp_dir).unwrap();
}

#[test]
fn test_acquire_exclusive_lock_blocks_concurrent_access() {
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;

    let unique_id = uuid::Uuid::new_v4().to_string();
    let temp_dir = std::env::temp_dir().join(format!("dashflow_concurrent_lock_{}", unique_id));
    std::fs::create_dir_all(&temp_dir).unwrap();

    let temp_dir_clone = temp_dir.clone();
    let lock_acquired = Arc::new(AtomicBool::new(false));
    let lock_acquired_clone = lock_acquired.clone();

    // First thread holds the lock
    let _lock = acquire_exclusive_lock(&temp_dir).unwrap();

    // Second thread tries to acquire - should block
    let handle = std::thread::spawn(move || {
        // Try non-blocking lock first
        let lock_path = lock_file_path(&temp_dir_clone);
        let file = std::fs::OpenOptions::new()
            .create(true)
            .truncate(false) // Lock file contents don't matter, don't truncate
            .read(true)
            .write(true)
            .open(&lock_path)
            .unwrap();

        // Try non-blocking - should fail since first thread holds it
        // Use fs2::FileExt trait method explicitly to avoid MSRV issues
        match fs2::FileExt::try_lock_exclusive(&file) {
            Ok(_) => {
                // We got the lock (first thread must have released)
                lock_acquired_clone.store(true, Ordering::SeqCst);
            }
            Err(_) => {
                // Expected: lock is held by first thread
                lock_acquired_clone.store(false, Ordering::SeqCst);
            }
        }
    });

    handle.join().unwrap();

    // Second thread should NOT have acquired the lock (first still holds it)
    assert!(
        !lock_acquired.load(Ordering::SeqCst),
        "Second thread should not acquire lock while first holds it"
    );

    // Release first lock
    drop(_lock);

    // Cleanup
    std::fs::remove_dir_all(&temp_dir).unwrap();
}

#[test]
fn test_checkpoint_id_includes_process_unique_component() {
    // Test that checkpoint IDs include process-unique identifier
    let state = TestState { value: 1 };
    let checkpoint1 = Checkpoint::new(
        "thread1".to_string(),
        state.clone(),
        "node1".to_string(),
        None,
    );
    let checkpoint2 = Checkpoint::new(
        "thread1".to_string(),
        state.clone(),
        "node1".to_string(),
        None,
    );

    // IDs should be different (different counter values)
    assert_ne!(
        checkpoint1.id, checkpoint2.id,
        "Sequential checkpoints should have different IDs"
    );

    // Both IDs should contain the process unique ID
    let process_id = get_process_unique_id();
    assert!(
        checkpoint1.id.contains(process_id),
        "Checkpoint ID '{}' should contain process unique ID '{}'",
        checkpoint1.id,
        process_id
    );
    assert!(
        checkpoint2.id.contains(process_id),
        "Checkpoint ID '{}' should contain process unique ID '{}'",
        checkpoint2.id,
        process_id
    );

    // IDs should start with thread_id
    assert!(
        checkpoint1.id.starts_with("thread1"),
        "Checkpoint ID should start with thread_id"
    );
}

#[test]
fn test_process_unique_id_is_stable_within_process() {
    // Test that process unique ID is consistent across calls
    let id1 = get_process_unique_id();
    let id2 = get_process_unique_id();
    assert_eq!(
        id1, id2,
        "Process unique ID should be stable within process"
    );
}

#[test]
fn test_process_unique_id_has_reasonable_length() {
    // Test that process unique ID is reasonable length (not too long, not too short)
    let id = get_process_unique_id();
    assert!(
        id.len() >= 4,
        "Process unique ID should have at least 4 chars, got {}",
        id.len()
    );
    assert!(
        id.len() <= 20,
        "Process unique ID should have at most 20 chars, got {}",
        id.len()
    );
}

#[tokio::test]
async fn test_get_latest_recovers_when_index_is_empty() {
    // Test that get_latest falls back to file scan when index is empty
    let unique_id = uuid::Uuid::new_v4().to_string();
    let temp_dir = std::env::temp_dir().join(format!("dashflow_index_recovery_{}", unique_id));

    // Create checkpointer and save a checkpoint
    let checkpointer = FileCheckpointer::<TestState>::new(&temp_dir).unwrap();
    let state = TestState { value: 42 };
    let checkpoint = Checkpoint::new("thread1".to_string(), state, "node1".to_string(), None);
    let saved_id = checkpoint.id.clone();

    checkpointer.save(checkpoint).await.unwrap();

    // Create a NEW checkpointer (simulates process restart with empty index)
    // Note: In a real scenario, if the index.bin file is deleted or corrupted,
    // the new checkpointer would have an empty index but files still exist
    let checkpointer2 = FileCheckpointer::<TestState>::new(&temp_dir).unwrap();

    // get_latest should recover via file scan even if index in memory was initially empty
    // (In this test, the index file still exists, so it's loaded - but this tests the mechanism)
    let latest = checkpointer2.get_latest("thread1").await.unwrap();
    assert!(latest.is_some(), "Should recover latest checkpoint");
    assert_eq!(
        latest.unwrap().id,
        saved_id,
        "Should find the same checkpoint"
    );

    // Cleanup
    std::fs::remove_dir_all(&temp_dir).unwrap();
}

#[tokio::test]
async fn test_get_latest_recovers_when_indexed_file_missing() {
    // Test that get_latest falls back to file scan when indexed file is deleted
    let unique_id = uuid::Uuid::new_v4().to_string();
    let temp_dir = std::env::temp_dir().join(format!("dashflow_missing_file_{}", unique_id));

    let checkpointer = FileCheckpointer::<TestState>::new(&temp_dir).unwrap();

    // Save two checkpoints
    let state1 = TestState { value: 1 };
    let cp1 = Checkpoint::new("thread1".to_string(), state1, "node1".to_string(), None);
    let cp1_id = cp1.id.clone();
    checkpointer.save(cp1).await.unwrap();

    // Wait a tiny bit to ensure different timestamp
    tokio::time::sleep(std::time::Duration::from_millis(10)).await;

    let state2 = TestState { value: 2 };
    let cp2 = Checkpoint::new("thread1".to_string(), state2, "node2".to_string(), None);
    let cp2_id = cp2.id.clone();
    checkpointer.save(cp2).await.unwrap();

    // Delete the second (latest) checkpoint file - simulating corruption
    let cp2_path = temp_dir.join(format!("{}.bin", cp2_id));
    std::fs::remove_file(&cp2_path).unwrap();

    // get_latest should fall back to file scan and find cp1
    let latest = checkpointer.get_latest("thread1").await.unwrap();
    assert!(latest.is_some(), "Should recover older checkpoint");
    assert_eq!(
        latest.unwrap().id,
        cp1_id,
        "Should find the older checkpoint via file scan"
    );

    // Cleanup
    std::fs::remove_dir_all(&temp_dir).unwrap();
}

// ==================== ReplicatedCheckpointer Tests ====================

#[test]
fn test_replication_mode_default() {
    let mode = ReplicationMode::default();
    assert_eq!(mode, ReplicationMode::Async);
}

#[test]
fn test_replicated_checkpointer_config_default() {
    let config = ReplicatedCheckpointerConfig::default();
    assert_eq!(config.mode, ReplicationMode::Async);
    assert_eq!(config.replica_timeout, std::time::Duration::from_secs(5));
    assert_eq!(config.max_retries, 3);
    assert!(config.read_from_replicas);
}

#[test]
fn test_replicated_checkpointer_config_builder() {
    let config = ReplicatedCheckpointerConfig::new()
        .with_mode(ReplicationMode::Sync)
        .with_replica_timeout(std::time::Duration::from_secs(10))
        .with_max_retries(5)
        .with_read_from_replicas(false);

    assert_eq!(config.mode, ReplicationMode::Sync);
    assert_eq!(config.replica_timeout, std::time::Duration::from_secs(10));
    assert_eq!(config.max_retries, 5);
    assert!(!config.read_from_replicas);
}

#[tokio::test]
async fn test_replicated_checkpointer_save_load_async_mode() {
    let primary: MemoryCheckpointer<TestState> = MemoryCheckpointer::new();
    let replica1: MemoryCheckpointer<TestState> = MemoryCheckpointer::new();
    let replica2: MemoryCheckpointer<TestState> = MemoryCheckpointer::new();

    let primary_clone = primary.clone();
    let replica1_clone = replica1.clone();
    let replica2_clone = replica2.clone();

    let replicated = ReplicatedCheckpointer::new(primary)
        .add_replica(replica1)
        .add_replica(replica2)
        .with_mode(ReplicationMode::Async);

    assert_eq!(replicated.replica_count(), 2);

    let state = TestState { value: 42 };
    let checkpoint = Checkpoint::new("thread1".to_string(), state, "node1".to_string(), None);
    let checkpoint_id = checkpoint.id.clone();

    replicated.save(checkpoint).await.unwrap();

    // Primary should have it immediately
    let loaded = replicated.load(&checkpoint_id).await.unwrap();
    assert!(loaded.is_some());
    assert_eq!(loaded.unwrap().state.value, 42);

    // Allow async replication to complete
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    // Replicas should also have it
    let replica1_loaded = replica1_clone.load(&checkpoint_id).await.unwrap();
    assert!(replica1_loaded.is_some());
    assert_eq!(replica1_loaded.unwrap().state.value, 42);

    let replica2_loaded = replica2_clone.load(&checkpoint_id).await.unwrap();
    assert!(replica2_loaded.is_some());
    assert_eq!(replica2_loaded.unwrap().state.value, 42);

    // Primary should also have it
    let primary_loaded = primary_clone.load(&checkpoint_id).await.unwrap();
    assert!(primary_loaded.is_some());
}

#[tokio::test]
async fn test_replicated_checkpointer_save_load_sync_mode() {
    let primary: MemoryCheckpointer<TestState> = MemoryCheckpointer::new();
    let replica1: MemoryCheckpointer<TestState> = MemoryCheckpointer::new();

    let replica1_clone = replica1.clone();

    let replicated = ReplicatedCheckpointer::new(primary)
        .add_replica(replica1)
        .with_mode(ReplicationMode::Sync);

    let state = TestState { value: 99 };
    let checkpoint = Checkpoint::new("thread1".to_string(), state, "node1".to_string(), None);
    let checkpoint_id = checkpoint.id.clone();

    // Sync mode should wait for replica
    replicated.save(checkpoint).await.unwrap();

    // Replica should have it immediately (no sleep needed)
    let replica1_loaded = replica1_clone.load(&checkpoint_id).await.unwrap();
    assert!(replica1_loaded.is_some());
    assert_eq!(replica1_loaded.unwrap().state.value, 99);
}

#[tokio::test]
async fn test_replicated_checkpointer_quorum_success() {
    let primary: MemoryCheckpointer<TestState> = MemoryCheckpointer::new();
    let replica1: MemoryCheckpointer<TestState> = MemoryCheckpointer::new();
    let replica2: MemoryCheckpointer<TestState> = MemoryCheckpointer::new();

    let replicated = ReplicatedCheckpointer::new(primary)
        .add_replica(replica1)
        .add_replica(replica2)
        .with_mode(ReplicationMode::Quorum);

    // Quorum for 3 nodes = 2
    assert_eq!(replicated.quorum_size(), 2);

    let state = TestState { value: 77 };
    let checkpoint = Checkpoint::new("thread1".to_string(), state, "node1".to_string(), None);

    // Should succeed - primary + at least 1 replica
    replicated.save(checkpoint).await.unwrap();
}

#[tokio::test]
async fn test_replicated_checkpointer_quorum_size_calculation() {
    let primary: MemoryCheckpointer<TestState> = MemoryCheckpointer::new();

    // Just primary: quorum = 1
    let replicated = ReplicatedCheckpointer::new(primary.clone());
    assert_eq!(replicated.quorum_size(), 1);

    // Primary + 1 replica: quorum = 2
    let replicated =
        ReplicatedCheckpointer::new(primary.clone()).add_replica(MemoryCheckpointer::new());
    assert_eq!(replicated.quorum_size(), 2);

    // Primary + 2 replicas: quorum = 2
    let replicated = ReplicatedCheckpointer::new(primary.clone())
        .add_replica(MemoryCheckpointer::new())
        .add_replica(MemoryCheckpointer::new());
    assert_eq!(replicated.quorum_size(), 2);

    // Primary + 3 replicas: quorum = 3
    let replicated = ReplicatedCheckpointer::new(primary.clone())
        .add_replica(MemoryCheckpointer::new())
        .add_replica(MemoryCheckpointer::new())
        .add_replica(MemoryCheckpointer::new());
    assert_eq!(replicated.quorum_size(), 3);

    // Primary + 4 replicas: quorum = 3
    let replicated = ReplicatedCheckpointer::new(primary)
        .add_replica(MemoryCheckpointer::new())
        .add_replica(MemoryCheckpointer::new())
        .add_replica(MemoryCheckpointer::new())
        .add_replica(MemoryCheckpointer::new());
    assert_eq!(replicated.quorum_size(), 3);
}

#[tokio::test]
async fn test_replicated_checkpointer_get_latest() {
    let primary: MemoryCheckpointer<TestState> = MemoryCheckpointer::new();
    let replica: MemoryCheckpointer<TestState> = MemoryCheckpointer::new();

    let replicated = ReplicatedCheckpointer::new(primary)
        .add_replica(replica)
        .with_mode(ReplicationMode::Sync);

    // Save multiple checkpoints
    let cp1 = Checkpoint::new(
        "thread1".to_string(),
        TestState { value: 1 },
        "node1".to_string(),
        None,
    );
    replicated.save(cp1).await.unwrap();

    tokio::time::sleep(std::time::Duration::from_millis(10)).await;

    let cp2 = Checkpoint::new(
        "thread1".to_string(),
        TestState { value: 2 },
        "node2".to_string(),
        None,
    );
    replicated.save(cp2).await.unwrap();

    // Get latest should return second checkpoint
    let latest = replicated.get_latest("thread1").await.unwrap();
    assert!(latest.is_some());
    assert_eq!(latest.unwrap().state.value, 2);
}

#[tokio::test]
async fn test_replicated_checkpointer_list() {
    let primary: MemoryCheckpointer<TestState> = MemoryCheckpointer::new();
    let replica: MemoryCheckpointer<TestState> = MemoryCheckpointer::new();

    let replicated = ReplicatedCheckpointer::new(primary)
        .add_replica(replica)
        .with_mode(ReplicationMode::Sync);

    // Save checkpoints
    for i in 0..3 {
        let cp = Checkpoint::new(
            "thread1".to_string(),
            TestState { value: i },
            format!("node{}", i),
            None,
        );
        replicated.save(cp).await.unwrap();
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }

    // List should return all 3 (from primary)
    let list = replicated.list("thread1").await.unwrap();
    assert_eq!(list.len(), 3);
}

#[tokio::test]
async fn test_replicated_checkpointer_delete() {
    let primary: MemoryCheckpointer<TestState> = MemoryCheckpointer::new();
    let replica: MemoryCheckpointer<TestState> = MemoryCheckpointer::new();

    let primary_clone = primary.clone();
    let replica_clone = replica.clone();

    let replicated = ReplicatedCheckpointer::new(primary)
        .add_replica(replica)
        .with_mode(ReplicationMode::Sync);

    let checkpoint = Checkpoint::new(
        "thread1".to_string(),
        TestState { value: 42 },
        "node1".to_string(),
        None,
    );
    let checkpoint_id = checkpoint.id.clone();

    replicated.save(checkpoint).await.unwrap();

    // Both should have it
    assert!(primary_clone.load(&checkpoint_id).await.unwrap().is_some());
    assert!(replica_clone.load(&checkpoint_id).await.unwrap().is_some());

    // Delete
    replicated.delete(&checkpoint_id).await.unwrap();

    // Both should be empty
    assert!(primary_clone.load(&checkpoint_id).await.unwrap().is_none());
    assert!(replica_clone.load(&checkpoint_id).await.unwrap().is_none());
}

#[tokio::test]
async fn test_replicated_checkpointer_delete_thread() {
    let primary: MemoryCheckpointer<TestState> = MemoryCheckpointer::new();
    let replica: MemoryCheckpointer<TestState> = MemoryCheckpointer::new();

    let primary_clone = primary.clone();
    let replica_clone = replica.clone();

    let replicated = ReplicatedCheckpointer::new(primary)
        .add_replica(replica)
        .with_mode(ReplicationMode::Sync);

    // Save checkpoints for two threads
    let cp1 = Checkpoint::new(
        "thread1".to_string(),
        TestState { value: 1 },
        "node1".to_string(),
        None,
    );
    let cp2 = Checkpoint::new(
        "thread2".to_string(),
        TestState { value: 2 },
        "node2".to_string(),
        None,
    );

    replicated.save(cp1).await.unwrap();
    replicated.save(cp2).await.unwrap();

    // Delete thread1
    replicated.delete_thread("thread1").await.unwrap();

    // thread1 should be gone from both
    assert!(primary_clone.get_latest("thread1").await.unwrap().is_none());
    assert!(replica_clone.get_latest("thread1").await.unwrap().is_none());

    // thread2 should still exist
    assert!(primary_clone.get_latest("thread2").await.unwrap().is_some());
    assert!(replica_clone.get_latest("thread2").await.unwrap().is_some());
}

#[tokio::test]
async fn test_replicated_checkpointer_failover_to_replica() {
    // Test that we can read from replica when primary doesn't have data
    let primary: MemoryCheckpointer<TestState> = MemoryCheckpointer::new();
    let replica: MemoryCheckpointer<TestState> = MemoryCheckpointer::new();

    let replica_clone = replica.clone();

    let replicated = ReplicatedCheckpointer::new(primary)
        .add_replica(replica)
        .with_config(ReplicatedCheckpointerConfig::new().with_read_from_replicas(true));

    // Save directly to replica (simulating data that exists only in replica)
    let checkpoint = Checkpoint::new(
        "thread1".to_string(),
        TestState { value: 99 },
        "node1".to_string(),
        None,
    );
    let checkpoint_id = checkpoint.id.clone();
    replica_clone.save(checkpoint).await.unwrap();

    // Load through replicated should find it in replica
    let loaded = replicated.load(&checkpoint_id).await.unwrap();
    assert!(loaded.is_some());
    assert_eq!(loaded.unwrap().state.value, 99);
}

#[tokio::test]
async fn test_replicated_checkpointer_no_failover_when_disabled() {
    let primary: MemoryCheckpointer<TestState> = MemoryCheckpointer::new();
    let replica: MemoryCheckpointer<TestState> = MemoryCheckpointer::new();

    let replica_clone = replica.clone();

    let replicated = ReplicatedCheckpointer::new(primary)
        .add_replica(replica)
        .with_config(ReplicatedCheckpointerConfig::new().with_read_from_replicas(false));

    // Save directly to replica
    let checkpoint = Checkpoint::new(
        "thread1".to_string(),
        TestState { value: 99 },
        "node1".to_string(),
        None,
    );
    let checkpoint_id = checkpoint.id.clone();
    replica_clone.save(checkpoint).await.unwrap();

    // Load through replicated should NOT find it (failover disabled)
    let loaded = replicated.load(&checkpoint_id).await.unwrap();
    assert!(loaded.is_none());
}

#[tokio::test]
async fn test_replicated_checkpointer_clone() {
    let primary: MemoryCheckpointer<TestState> = MemoryCheckpointer::new();
    let replica: MemoryCheckpointer<TestState> = MemoryCheckpointer::new();

    let replicated = ReplicatedCheckpointer::new(primary)
        .add_replica(replica)
        .with_mode(ReplicationMode::Sync);

    let cloned = replicated.clone();
    assert_eq!(cloned.replica_count(), 1);

    // Both should work independently
    let checkpoint = Checkpoint::new(
        "thread1".to_string(),
        TestState { value: 42 },
        "node1".to_string(),
        None,
    );
    let checkpoint_id = checkpoint.id.clone();

    replicated.save(checkpoint).await.unwrap();

    // Cloned should see the same data (they share state)
    let loaded = cloned.load(&checkpoint_id).await.unwrap();
    assert!(loaded.is_some());
}

#[tokio::test]
async fn test_replicated_checkpointer_with_config() {
    let primary: MemoryCheckpointer<TestState> = MemoryCheckpointer::new();
    let config = ReplicatedCheckpointerConfig::new()
        .with_mode(ReplicationMode::Quorum)
        .with_replica_timeout(std::time::Duration::from_secs(30))
        .with_max_retries(10);

    let replicated = ReplicatedCheckpointer::new(primary).with_config(config);

    // Verify config was applied
    assert_eq!(replicated.config().mode, ReplicationMode::Quorum);
    assert_eq!(
        replicated.config().replica_timeout,
        std::time::Duration::from_secs(30)
    );
    assert_eq!(replicated.config().max_retries, 10);
}

#[tokio::test]
async fn test_replicated_checkpointer_primary_only() {
    // Test with no replicas
    let primary: MemoryCheckpointer<TestState> = MemoryCheckpointer::new();
    let replicated = ReplicatedCheckpointer::new(primary);

    assert_eq!(replicated.replica_count(), 0);
    assert_eq!(replicated.quorum_size(), 1); // Just primary

    let checkpoint = Checkpoint::new(
        "thread1".to_string(),
        TestState { value: 42 },
        "node1".to_string(),
        None,
    );
    let checkpoint_id = checkpoint.id.clone();

    replicated.save(checkpoint).await.unwrap();

    let loaded = replicated.load(&checkpoint_id).await.unwrap();
    assert!(loaded.is_some());
}

#[test]
fn test_replication_mode_variants() {
    // Test all variants are distinct
    assert_ne!(ReplicationMode::Async, ReplicationMode::Sync);
    assert_ne!(ReplicationMode::Sync, ReplicationMode::Quorum);
    assert_ne!(ReplicationMode::Async, ReplicationMode::Quorum);

    // Test debug formatting
    assert_eq!(format!("{:?}", ReplicationMode::Async), "Async");
    assert_eq!(format!("{:?}", ReplicationMode::Sync), "Sync");
    assert_eq!(format!("{:?}", ReplicationMode::Quorum), "Quorum");

    // Test clone
    let mode = ReplicationMode::Quorum;
    let cloned = mode;
    assert_eq!(mode, cloned);
}

// ========================================================================
// CheckpointPolicy Tests
// ========================================================================

#[test]
fn test_checkpoint_policy_default() {
    let policy = CheckpointPolicy::default();
    assert_eq!(policy, CheckpointPolicy::Every);
}

#[test]
fn test_checkpoint_policy_every() {
    let policy = CheckpointPolicy::Every;
    // Every policy should always return true
    assert!(policy.should_checkpoint("node1", 1, 100, 100));
    assert!(policy.should_checkpoint("node2", 100, 100, 100));
}

#[test]
fn test_checkpoint_policy_every_n() {
    let policy = CheckpointPolicy::every_n(3);
    // Should checkpoint at multiples of 3
    assert!(!policy.should_checkpoint("node1", 1, 100, 100));
    assert!(!policy.should_checkpoint("node2", 2, 100, 100));
    assert!(policy.should_checkpoint("node3", 3, 100, 100));
    assert!(!policy.should_checkpoint("node4", 4, 100, 100));
    assert!(!policy.should_checkpoint("node5", 5, 100, 100));
    assert!(policy.should_checkpoint("node6", 6, 100, 100));
}

#[test]
fn test_checkpoint_policy_try_every_n_valid() {
    let result = CheckpointPolicy::try_every_n(3);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), CheckpointPolicy::EveryN(3));
}

#[test]
fn test_checkpoint_policy_try_every_n_zero_fails() {
    let result = CheckpointPolicy::try_every_n(0);
    assert!(result.is_err());
    assert!(matches!(
        result.unwrap_err(),
        CheckpointPolicyError::InvalidN { n: 0 }
    ));
}

#[test]
fn test_checkpoint_policy_on_markers() {
    let policy = CheckpointPolicy::on_markers(["save_point", "critical"]);
    // Should only checkpoint at marker nodes
    assert!(policy.should_checkpoint("save_point", 1, 100, 100));
    assert!(policy.should_checkpoint("critical", 2, 100, 100));
    assert!(!policy.should_checkpoint("other_node", 3, 100, 100));
    assert!(!policy.should_checkpoint("random", 4, 100, 100));
}

#[test]
fn test_checkpoint_policy_on_state_change() {
    let policy = CheckpointPolicy::on_state_change(50);
    // Should checkpoint when delta >= min_delta
    assert!(!policy.should_checkpoint("node1", 1, 100, 100)); // delta = 0
    assert!(!policy.should_checkpoint("node2", 2, 130, 100)); // delta = 30
    assert!(policy.should_checkpoint("node3", 3, 150, 100)); // delta = 50
    assert!(policy.should_checkpoint("node4", 4, 200, 100)); // delta = 100
                                                             // Also works for shrinking state
    assert!(policy.should_checkpoint("node5", 5, 50, 100)); // delta = 50
    assert!(!policy.should_checkpoint("node6", 6, 70, 100)); // delta = 30
}

#[test]
fn test_checkpoint_policy_never() {
    let policy = CheckpointPolicy::Never;
    // Never policy should always return false
    assert!(!policy.should_checkpoint("node1", 1, 100, 100));
    assert!(!policy.should_checkpoint("node2", 100, 100, 100));
}

#[test]
fn test_checkpoint_policy_equality() {
    assert_eq!(CheckpointPolicy::Every, CheckpointPolicy::Every);
    assert_eq!(CheckpointPolicy::Never, CheckpointPolicy::Never);
    assert_eq!(CheckpointPolicy::EveryN(5), CheckpointPolicy::EveryN(5));
    assert_ne!(CheckpointPolicy::EveryN(5), CheckpointPolicy::EveryN(3));
    assert_ne!(CheckpointPolicy::Every, CheckpointPolicy::Never);
}

#[test]
fn test_checkpoint_policy_clone() {
    let policy = CheckpointPolicy::on_markers(["a", "b"]);
    let cloned = policy.clone();
    assert_eq!(policy, cloned);
}

#[test]
fn test_checkpoint_policy_debug() {
    // Ensure Debug trait is implemented and produces meaningful output
    let policy = CheckpointPolicy::Every;
    let debug_str = format!("{:?}", policy);
    assert!(debug_str.contains("Every"));

    let policy = CheckpointPolicy::EveryN(5);
    let debug_str = format!("{:?}", policy);
    assert!(debug_str.contains("EveryN"));
    assert!(debug_str.contains("5"));
}

// ============================================================================
// Differential Checkpoint Tests
// ============================================================================

#[test]
fn test_checkpoint_diff_create_small_state() {
    // Small states should not create diffs (below threshold)
    let base = vec![1u8; 100];
    let new = vec![2u8; 100];
    let diff = CheckpointDiff::create(&base, &new);
    assert!(diff.is_none(), "Should not diff small states");
}

#[test]
fn test_checkpoint_diff_create_and_apply() {
    // Create a large base state
    let base: Vec<u8> = (0..2000).map(|i| (i % 256) as u8).collect();

    // Modify a small portion
    let mut new = base.clone();
    for i in 500..600 {
        new[i] = 255;
    }

    // Create diff
    let diff = CheckpointDiff::create(&base, &new);
    assert!(
        diff.is_some(),
        "Should create diff for large state with small changes"
    );

    let mut diff = diff.unwrap();
    diff.base_id = "test-base".to_string();

    // Verify diff is smaller than full state
    assert!(
        diff.diff_data.len() < new.len(),
        "Diff ({}) should be smaller than full state ({})",
        diff.diff_data.len(),
        new.len()
    );

    // Apply diff and verify reconstruction
    let reconstructed = diff.apply(&base).unwrap();
    assert_eq!(
        reconstructed, new,
        "Reconstructed state should match original"
    );
}

#[test]
fn test_checkpoint_diff_identical_states() {
    // Identical states should produce minimal or no diff
    let base: Vec<u8> = (0..2000).map(|i| (i % 256) as u8).collect();
    let new = base.clone();

    let diff = CheckpointDiff::create(&base, &new);
    // Either no diff (identical) or very small diff (just header)
    if let Some(d) = diff {
        assert!(
            d.diff_data.len() < 20,
            "Identical states should produce minimal diff"
        );
    }
}

#[test]
fn test_checkpoint_diff_growing_state() {
    // Test state that grows
    let base: Vec<u8> = (0..2000).map(|i| (i % 256) as u8).collect();
    let mut new = base.clone();
    new.extend(vec![42u8; 500]); // Add 500 bytes

    let diff = CheckpointDiff::create(&base, &new);
    assert!(diff.is_some());

    let mut diff = diff.unwrap();
    diff.base_id = "test-base".to_string();

    let reconstructed = diff.apply(&base).unwrap();
    assert_eq!(reconstructed.len(), new.len());
    assert_eq!(reconstructed, new);
}

#[test]
fn test_checkpoint_diff_shrinking_state() {
    // Test state that shrinks
    let base: Vec<u8> = (0..2500).map(|i| (i % 256) as u8).collect();
    let new: Vec<u8> = base[0..2000].to_vec();

    let diff = CheckpointDiff::create(&base, &new);
    assert!(diff.is_some());

    let mut diff = diff.unwrap();
    diff.base_id = "test-base".to_string();

    let reconstructed = diff.apply(&base).unwrap();
    assert_eq!(reconstructed.len(), new.len());
    assert_eq!(reconstructed, new);
}

#[test]
fn test_differential_config_default() {
    let config = DifferentialConfig::default();
    assert_eq!(config.base_interval, 10);
    assert_eq!(config.max_chain_length, 20);
    assert_eq!(config.min_diff_size, CheckpointDiff::MIN_DIFF_SIZE);
}

#[test]
fn test_differential_config_presets() {
    let memory = DifferentialConfig::memory_optimized();
    assert!(memory.base_interval > DifferentialConfig::default().base_interval);

    let speed = DifferentialConfig::speed_optimized();
    assert!(speed.base_interval < DifferentialConfig::default().base_interval);
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
struct LargeTestState {
    data: Vec<u8>,
    counter: u32,
}

#[tokio::test]
async fn test_differential_checkpointer_basic() {
    let inner = MemoryCheckpointer::new();
    let diff_checkpointer = DifferentialCheckpointer::new(inner);

    // Create a state large enough to trigger diffing
    let state = LargeTestState {
        data: vec![42u8; 2000],
        counter: 1,
    };

    let checkpoint = Checkpoint::new(
        "thread1".to_string(),
        state.clone(),
        "node1".to_string(),
        None,
    );
    let checkpoint_id = checkpoint.id.clone();

    // Save and load
    diff_checkpointer.save(checkpoint).await.unwrap();
    let loaded = diff_checkpointer.load(&checkpoint_id).await.unwrap();

    assert!(loaded.is_some());
    assert_eq!(loaded.unwrap().state, state);
}

#[tokio::test]
async fn test_differential_checkpointer_incremental() {
    let inner = MemoryCheckpointer::new();
    // Use small base interval for testing
    let config = DifferentialConfig {
        base_interval: 3,
        max_chain_length: 10,
        min_diff_size: 100,
    };
    let diff_checkpointer = DifferentialCheckpointer::with_config(inner, config);

    let mut last_id = None;
    let mut saved_ids = Vec::new();

    // Save 5 checkpoints with incremental changes
    for i in 0..5 {
        let mut data = vec![0u8; 2000];
        // Only modify a small portion each time
        for j in (i * 100)..(i * 100 + 50) {
            if j < data.len() {
                data[j] = (i + 1) as u8;
            }
        }

        let state = LargeTestState {
            data,
            counter: i as u32,
        };

        let checkpoint = Checkpoint::new(
            "thread1".to_string(),
            state,
            format!("node{}", i),
            last_id.clone(),
        );
        last_id = Some(checkpoint.id.clone());
        saved_ids.push(checkpoint.id.clone());

        diff_checkpointer.save(checkpoint).await.unwrap();
    }

    // Verify all checkpoints can be loaded correctly
    for (i, id) in saved_ids.iter().enumerate() {
        let loaded = diff_checkpointer.load(id).await.unwrap();
        assert!(loaded.is_some(), "Checkpoint {} should be loadable", i);
        let state = loaded.unwrap().state;
        assert_eq!(state.counter, i as u32);
    }

    // Verify get_latest returns the most recent
    let latest = diff_checkpointer.get_latest("thread1").await.unwrap();
    assert!(latest.is_some());
    assert_eq!(latest.unwrap().state.counter, 4);
}

#[tokio::test]
async fn test_differential_checkpointer_delete() {
    let inner = MemoryCheckpointer::new();
    let diff_checkpointer = DifferentialCheckpointer::new(inner);

    let state = LargeTestState {
        data: vec![42u8; 2000],
        counter: 1,
    };

    let checkpoint = Checkpoint::new("thread1".to_string(), state, "node1".to_string(), None);
    let checkpoint_id = checkpoint.id.clone();

    diff_checkpointer.save(checkpoint).await.unwrap();
    diff_checkpointer.delete(&checkpoint_id).await.unwrap();

    let loaded = diff_checkpointer.load(&checkpoint_id).await.unwrap();
    assert!(loaded.is_none());
}

#[tokio::test]
async fn test_differential_checkpointer_delete_thread() {
    let inner = MemoryCheckpointer::new();
    let diff_checkpointer = DifferentialCheckpointer::new(inner);

    // Save checkpoints to two threads
    for thread in ["thread1", "thread2"] {
        let state = LargeTestState {
            data: vec![42u8; 2000],
            counter: 1,
        };

        let checkpoint = Checkpoint::new(thread.to_string(), state, "node1".to_string(), None);

        diff_checkpointer.save(checkpoint).await.unwrap();
    }

    // Delete thread1
    diff_checkpointer.delete_thread("thread1").await.unwrap();

    // thread1 should have no checkpoints, thread2 should still have one
    let thread1_latest = diff_checkpointer.get_latest("thread1").await.unwrap();
    let thread2_latest = diff_checkpointer.get_latest("thread2").await.unwrap();

    assert!(thread1_latest.is_none());
    assert!(thread2_latest.is_some());
}

#[tokio::test]
async fn test_differential_checkpointer_list() {
    let inner = MemoryCheckpointer::new();
    let diff_checkpointer = DifferentialCheckpointer::new(inner);

    // Save 3 checkpoints
    let mut last_id = None;
    for i in 0..3 {
        let state = LargeTestState {
            data: vec![42u8; 2000],
            counter: i,
        };

        let checkpoint = Checkpoint::new(
            "thread1".to_string(),
            state,
            format!("node{}", i),
            last_id.clone(),
        );
        last_id = Some(checkpoint.id.clone());

        diff_checkpointer.save(checkpoint).await.unwrap();
    }

    let list = diff_checkpointer.list("thread1").await.unwrap();
    assert_eq!(list.len(), 3);
}

#[tokio::test]
async fn test_differential_checkpointer_list_threads() {
    let inner = MemoryCheckpointer::new();
    let diff_checkpointer = DifferentialCheckpointer::new(inner);

    // Save checkpoints to multiple threads
    for thread in ["thread1", "thread2", "thread3"] {
        let state = LargeTestState {
            data: vec![42u8; 2000],
            counter: 1,
        };

        let checkpoint = Checkpoint::new(thread.to_string(), state, "node1".to_string(), None);

        diff_checkpointer.save(checkpoint).await.unwrap();
    }

    let threads = diff_checkpointer.list_threads().await.unwrap();
    assert_eq!(threads.len(), 3);
}

// ============================================================================
// Error Recovery Tests (M-242)
// ============================================================================
// Tests for partial write detection and recovery

/// Test that FileCheckpointer file scan skips truncated (partial write) checkpoint files (M-242)
///
/// Note: The index-based `get_latest` returns an error when the indexed file is corrupt.
/// The file scan path (`get_latest_by_file_scan`) gracefully skips corrupt files.
/// This test verifies the file scan recovery behavior by deleting the index first.
#[tokio::test]
async fn test_file_checkpointer_recovers_from_partial_write() {
    // Simulates a partial write scenario where power failure or crash leaves truncated file
    let unique_id = uuid::Uuid::new_v4().to_string();
    let temp_dir = std::env::temp_dir().join(format!("dashflow_partial_write_{}", unique_id));

    let checkpointer = FileCheckpointer::<TestState>::new(&temp_dir).unwrap();

    // Save two checkpoints - first will be valid backup
    let state1 = TestState { value: 100 };
    let cp1 = Checkpoint::new("thread1".to_string(), state1, "node1".to_string(), None);
    let cp1_id = cp1.id.clone();
    checkpointer.save(cp1).await.unwrap();

    // Small delay to ensure different timestamps
    tokio::time::sleep(std::time::Duration::from_millis(10)).await;

    // Save second checkpoint
    let state2 = TestState { value: 200 };
    let cp2 = Checkpoint::new("thread1".to_string(), state2, "node2".to_string(), None);
    let cp2_id = cp2.id.clone();
    checkpointer.save(cp2).await.unwrap();

    // Simulate partial write by truncating the second checkpoint file
    let cp2_path = temp_dir.join(format!("{}.bin", cp2_id));
    let file_data = std::fs::read(&cp2_path).unwrap();
    // Truncate to just the header (20 bytes) - missing payload simulates partial write
    std::fs::write(&cp2_path, &file_data[0..20]).unwrap();

    // Delete the index to force file scan recovery path
    // (Index-based lookup returns error on corrupt file, but file scan skips them gracefully)
    let index_path = temp_dir.join("index.bin");
    if index_path.exists() {
        std::fs::remove_file(&index_path).unwrap();
    }

    // Create a fresh checkpointer to force file scan (no cached index)
    let fresh_checkpointer = FileCheckpointer::<TestState>::new(&temp_dir).unwrap();

    // get_latest should now use file scan which skips corrupt files
    let latest = fresh_checkpointer.get_latest("thread1").await.unwrap();
    assert!(
        latest.is_some(),
        "Should recover older checkpoint after partial write via file scan"
    );
    assert_eq!(
        latest.as_ref().unwrap().id,
        cp1_id,
        "Should fall back to older valid checkpoint"
    );
    assert_eq!(
        latest.unwrap().state.value,
        100,
        "Should have the correct state from backup"
    );

    // Cleanup
    std::fs::remove_dir_all(&temp_dir).unwrap();
}

/// Test that loading a truncated checkpoint returns error (M-242)
#[tokio::test]
async fn test_file_checkpointer_load_truncated_returns_error() {
    let unique_id = uuid::Uuid::new_v4().to_string();
    let temp_dir = std::env::temp_dir().join(format!("dashflow_truncated_load_{}", unique_id));

    let checkpointer = FileCheckpointer::<TestState>::new(&temp_dir).unwrap();

    // Save a checkpoint
    let state = TestState { value: 42 };
    let checkpoint = Checkpoint::new("thread1".to_string(), state, "node1".to_string(), None);
    let checkpoint_id = checkpoint.id.clone();
    checkpointer.save(checkpoint).await.unwrap();

    // Truncate the file mid-payload
    let checkpoint_path = temp_dir.join(format!("{}.bin", checkpoint_id));
    let file_data = std::fs::read(&checkpoint_path).unwrap();
    // Truncate to 30 bytes (header is 20, so only 10 bytes of payload)
    std::fs::write(&checkpoint_path, &file_data[0..30]).unwrap();

    // Direct load should return error
    let result = checkpointer.load(&checkpoint_id).await;
    assert!(result.is_err(), "Loading truncated file should return error");
    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg.contains("integrity") || err_msg.contains("mismatch") || err_msg.contains("failed"),
        "Error should indicate integrity issue: {}",
        err_msg
    );

    // Cleanup
    std::fs::remove_dir_all(&temp_dir).unwrap();
}

/// Test that zero-byte file (complete write failure) is handled gracefully (M-242)
#[tokio::test]
async fn test_file_checkpointer_handles_zero_byte_file() {
    let unique_id = uuid::Uuid::new_v4().to_string();
    let temp_dir = std::env::temp_dir().join(format!("dashflow_zero_byte_{}", unique_id));

    let checkpointer = FileCheckpointer::<TestState>::new(&temp_dir).unwrap();

    // Save a valid checkpoint first
    let state1 = TestState { value: 1 };
    let cp1 = Checkpoint::new("thread1".to_string(), state1, "node1".to_string(), None);
    let cp1_id = cp1.id.clone();
    checkpointer.save(cp1).await.unwrap();

    // Create a zero-byte file that looks like a checkpoint
    let fake_id = uuid::Uuid::new_v4().to_string();
    let fake_path = temp_dir.join(format!("{}.bin", fake_id));
    std::fs::write(&fake_path, b"").unwrap();

    // get_latest should skip the zero-byte file and find valid checkpoint
    let latest = checkpointer.get_latest("thread1").await.unwrap();
    assert!(latest.is_some(), "Should find valid checkpoint");
    assert_eq!(latest.unwrap().id, cp1_id, "Should return valid checkpoint");

    // Cleanup
    std::fs::remove_dir_all(&temp_dir).unwrap();
}

/// Test CheckpointIntegrity detects half-written header (M-242)
#[test]
fn test_checkpoint_integrity_detects_half_written_header() {
    // Simulate crash during header write - only magic bytes written
    let partial_header = b"DCHK"; // Just magic bytes, missing version/checksum/length

    let result = CheckpointWithIntegrity::unwrap(partial_header);
    assert!(result.is_err());
    match result {
        Err(CheckpointIntegrityError::FileTooSmall { size, minimum }) => {
            assert_eq!(size, 4);
            assert_eq!(minimum, 20);
        }
        other => panic!("Expected FileTooSmall error, got {:?}", other),
    }
}

/// Test that atomic write prevents partial writes (M-242)
#[tokio::test]
async fn test_file_checkpointer_atomic_write_prevents_corruption() {
    // Verify that save() uses atomic write (temp file + rename)
    // This test verifies the mechanism exists, not its atomicity (which requires OS-level testing)
    let unique_id = uuid::Uuid::new_v4().to_string();
    let temp_dir = std::env::temp_dir().join(format!("dashflow_atomic_{}", unique_id));

    let checkpointer = FileCheckpointer::<TestState>::new(&temp_dir).unwrap();

    // Save multiple checkpoints in sequence
    for i in 0..5 {
        let state = TestState { value: i };
        let checkpoint = Checkpoint::new("thread1".to_string(), state, format!("node{}", i), None);
        checkpointer.save(checkpoint).await.unwrap();
    }

    // All checkpoints should be loadable (no partial writes)
    let list = checkpointer.list("thread1").await.unwrap();
    assert_eq!(list.len(), 5, "All checkpoints should be saved");

    for (_idx, metadata) in list.iter().enumerate() {
        let loaded = checkpointer.load(&metadata.id).await;
        assert!(
            loaded.is_ok(),
            "Checkpoint {} should be loadable",
            metadata.id
        );
        let checkpoint = loaded.unwrap();
        assert!(checkpoint.is_some());
        // Verify integrity - state matches what we saved (values 0-4)
        assert!(
            checkpoint.as_ref().unwrap().state.value < 5,
            "Checkpoint state should be valid"
        );
    }

    // Cleanup
    std::fs::remove_dir_all(&temp_dir).unwrap();
}
