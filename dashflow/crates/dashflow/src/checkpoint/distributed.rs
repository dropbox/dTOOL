// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

// Allow clone_on_ref_ptr: DistributedCheckpointer uses Arc for shared inner storage and trackers
#![allow(clippy::clone_on_ref_ptr)]

//! Distributed checkpoint coordination for multi-node execution
//!
//! This module provides checkpoint coordination when graph nodes execute across
//! multiple remote workers. It ensures consistency and handles concurrent access.
//!
//! # Design
//!
//! When using distributed execution with work-stealing schedulers:
//! - Remote workers may create checkpoints during node execution
//! - Multiple parallel nodes may checkpoint concurrently
//! - The coordinator ensures checkpoints are consistent and properly ordered
//!
//! # Algorithm
//!
//! 1. **Local Checkpoint**: Each worker/executor saves checkpoints to its local storage
//! 2. **Coordination**: Coordinator tracks checkpoint sequence and validates consistency
//! 3. **Merge**: For parallel execution, coordinator selects canonical checkpoint
//!
//! # Example
//!
//! ```rust,ignore
//! use dashflow::checkpoint::{DistributedCheckpointCoordinator, MemoryCheckpointer};
//!
//! let local_checkpointer = MemoryCheckpointer::new();
//! let coordinator = DistributedCheckpointCoordinator::new(local_checkpointer);
//!
//! let app = graph.compile()?
//!     .with_checkpointer(coordinator)
//!     .with_scheduler(work_stealing_scheduler);
//! ```

use super::{Checkpoint, CheckpointId, CheckpointMetadata, Checkpointer, ThreadId, ThreadInfo};
use crate::{GraphState, Result};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Distributed checkpoint coordinator
///
/// Coordinates checkpoints across distributed node execution. Ensures consistency
/// when multiple workers create checkpoints concurrently.
///
/// # Type Parameters
///
/// - `S`: Graph state type (must be `GraphState`)
pub struct DistributedCheckpointCoordinator<S>
where
    S: GraphState,
{
    /// Underlying checkpointer for storage
    inner: Arc<dyn Checkpointer<S>>,
    /// Checkpoint sequence tracking (`thread_id` -> `next_sequence_number`)
    sequence_tracker: Arc<RwLock<HashMap<ThreadId, u64>>>,
    /// Pending checkpoints awaiting coordination (for parallel execution)
    pending: Arc<RwLock<HashMap<CheckpointId, Checkpoint<S>>>>,
}

impl<S> DistributedCheckpointCoordinator<S>
where
    S: GraphState,
{
    /// Create a new distributed checkpoint coordinator
    ///
    /// # Arguments
    ///
    /// * `checkpointer` - Underlying checkpointer for storage (e.g., `PostgreSQL`, S3)
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use dashflow::checkpoint::{DistributedCheckpointCoordinator, MemoryCheckpointer};
    ///
    /// let coordinator = DistributedCheckpointCoordinator::new(MemoryCheckpointer::new());
    /// ```
    pub fn new<C>(checkpointer: C) -> Self
    where
        C: Checkpointer<S> + 'static,
    {
        Self {
            inner: Arc::new(checkpointer),
            sequence_tracker: Arc::new(RwLock::new(HashMap::new())),
            pending: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Assign sequence number to checkpoint
    ///
    /// Ensures checkpoints have monotonically increasing sequence numbers
    /// for consistent ordering across distributed workers.
    async fn assign_sequence(&self, checkpoint: &mut Checkpoint<S>) -> Result<()> {
        let mut tracker = self.sequence_tracker.write().await;
        let seq = tracker.entry(checkpoint.thread_id.clone()).or_insert(0);

        // Add sequence number to checkpoint metadata
        checkpoint
            .metadata
            .insert("sequence".to_string(), seq.to_string());
        *seq += 1;

        Ok(())
    }

    /// Coordinate parallel checkpoints
    ///
    /// When multiple nodes execute in parallel, they may create checkpoints
    /// concurrently. This method selects the canonical checkpoint based on:
    /// - Latest timestamp
    /// - Highest sequence number (for ties)
    ///
    /// # Algorithm
    ///
    /// 1. Add checkpoint to pending set
    /// 2. Wait for all parallel checkpoints to arrive (same `parent_id`)
    /// 3. Select canonical checkpoint (latest timestamp)
    /// 4. Save canonical checkpoint, discard others
    async fn coordinate_parallel_checkpoint(&self, checkpoint: Checkpoint<S>) -> Result<()> {
        // For now, we use a simple strategy:
        // - If checkpoint has siblings (same parent_id), use timestamp ordering
        // - Always save to inner checkpointer (storage is cheap, consistency is expensive)

        // Check if there are other checkpoints with the same parent
        let mut pending = self.pending.write().await;

        // Add to pending
        let checkpoint_id = checkpoint.id.clone();
        pending.insert(checkpoint_id.clone(), checkpoint.clone());

        // For simplicity in this implementation, we immediately save
        // In a more sophisticated implementation, we could batch and coordinate
        self.inner.save(checkpoint.clone()).await?;

        // Remove from pending after successful save
        pending.remove(&checkpoint_id);

        Ok(())
    }

    /// Get checkpoint count for a thread
    ///
    /// Useful for testing and debugging
    pub async fn checkpoint_count(&self, thread_id: &str) -> Result<usize> {
        let checkpoints = self.inner.list(thread_id).await?;
        Ok(checkpoints.len())
    }
}

impl<S> Clone for DistributedCheckpointCoordinator<S>
where
    S: GraphState,
{
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
            sequence_tracker: self.sequence_tracker.clone(),
            pending: self.pending.clone(),
        }
    }
}

#[async_trait::async_trait]
impl<S> Checkpointer<S> for DistributedCheckpointCoordinator<S>
where
    S: GraphState,
{
    async fn save(&self, mut checkpoint: Checkpoint<S>) -> Result<()> {
        // Assign sequence number for ordering
        self.assign_sequence(&mut checkpoint).await?;

        // Add distributed coordination metadata
        checkpoint
            .metadata
            .insert("distributed".to_string(), "true".to_string());

        // Coordinate with other parallel checkpoints
        self.coordinate_parallel_checkpoint(checkpoint).await
    }

    async fn load(&self, checkpoint_id: &str) -> Result<Option<Checkpoint<S>>> {
        self.inner.load(checkpoint_id).await
    }

    async fn get_latest(&self, thread_id: &str) -> Result<Option<Checkpoint<S>>> {
        self.inner.get_latest(thread_id).await
    }

    async fn list(&self, thread_id: &str) -> Result<Vec<CheckpointMetadata>> {
        self.inner.list(thread_id).await
    }

    async fn delete(&self, checkpoint_id: &str) -> Result<()> {
        self.inner.delete(checkpoint_id).await
    }

    async fn delete_thread(&self, thread_id: &str) -> Result<()> {
        // Clear sequence tracker for this thread
        {
            let mut tracker = self.sequence_tracker.write().await;
            tracker.remove(thread_id);
        }

        // Clear any pending checkpoints for this thread
        {
            let mut pending = self.pending.write().await;
            pending.retain(|_, cp| cp.thread_id != thread_id);
        }

        // Delete from underlying storage
        self.inner.delete_thread(thread_id).await
    }

    async fn list_threads(&self) -> Result<Vec<ThreadInfo>> {
        // Delegate to underlying storage
        self.inner.list_threads().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::checkpoint::MemoryCheckpointer;
    use serde::{Deserialize, Serialize};

    #[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
    struct TestState {
        value: i32,
    }

    // GraphState is auto-implemented for types that meet the trait bounds

    #[tokio::test]
    async fn test_distributed_coordinator_basic() {
        let inner = MemoryCheckpointer::new();
        let coordinator = DistributedCheckpointCoordinator::new(inner);

        let checkpoint = Checkpoint::new(
            "thread-1".to_string(),
            TestState { value: 42 },
            "node-1".to_string(),
            None,
        );

        // Save checkpoint
        coordinator.save(checkpoint.clone()).await.unwrap();

        // Load checkpoint
        let loaded = coordinator.load(&checkpoint.id).await.unwrap();
        assert!(loaded.is_some());
        let loaded = loaded.unwrap();
        assert_eq!(loaded.state.value, 42);

        // Verify sequence number was added
        assert!(loaded.metadata.contains_key("sequence"));
        assert_eq!(loaded.metadata.get("sequence").unwrap(), "0");
    }

    #[tokio::test]
    async fn test_sequence_tracking() {
        let inner = MemoryCheckpointer::new();
        let coordinator = DistributedCheckpointCoordinator::new(inner);

        let thread_id = "thread-seq-test".to_string();

        // Save multiple checkpoints
        for i in 0..5 {
            let checkpoint = Checkpoint::new(
                thread_id.clone(),
                TestState { value: i },
                format!("node-{}", i),
                None,
            );
            coordinator.save(checkpoint).await.unwrap();
        }

        // List checkpoints
        let checkpoints = coordinator.list(&thread_id).await.unwrap();
        assert_eq!(checkpoints.len(), 5);

        // Verify sequence numbers are monotonically increasing
        let mut sequences: Vec<u64> = checkpoints
            .iter()
            .map(|cp| cp.metadata.get("sequence").unwrap().parse().unwrap())
            .collect();
        sequences.sort();
        assert_eq!(sequences, vec![0, 1, 2, 3, 4]);
    }

    #[tokio::test]
    async fn test_concurrent_checkpoints() {
        let inner = MemoryCheckpointer::new();
        let coordinator = Arc::new(DistributedCheckpointCoordinator::new(inner));

        let thread_id = "thread-concurrent".to_string();

        // Simulate concurrent checkpoint saves from multiple workers
        let mut handles = vec![];
        for i in 0..10 {
            let coordinator_clone = coordinator.clone();
            let thread_id_clone = thread_id.clone();

            let handle = tokio::spawn(async move {
                let checkpoint = Checkpoint::new(
                    thread_id_clone,
                    TestState { value: i },
                    format!("worker-{}", i),
                    None,
                );
                coordinator_clone.save(checkpoint).await
            });
            handles.push(handle);
        }

        // Wait for all to complete
        for handle in handles {
            handle.await.unwrap().unwrap();
        }

        // Verify all checkpoints were saved
        let count = coordinator.checkpoint_count(&thread_id).await.unwrap();
        assert_eq!(count, 10);

        // Verify each has a unique sequence number
        let checkpoints = coordinator.list(&thread_id).await.unwrap();
        let mut sequences: Vec<u64> = checkpoints
            .iter()
            .map(|cp| cp.metadata.get("sequence").unwrap().parse().unwrap())
            .collect();
        sequences.sort();
        sequences.dedup();
        assert_eq!(sequences.len(), 10); // All unique
    }

    #[tokio::test]
    async fn test_delete_thread_cleans_up() {
        let inner = MemoryCheckpointer::new();
        let coordinator = DistributedCheckpointCoordinator::new(inner);

        let thread_id = "thread-cleanup".to_string();

        // Save checkpoints
        for i in 0..3 {
            let checkpoint = Checkpoint::new(
                thread_id.clone(),
                TestState { value: i },
                format!("node-{}", i),
                None,
            );
            coordinator.save(checkpoint).await.unwrap();
        }

        // Verify checkpoints exist
        let count = coordinator.checkpoint_count(&thread_id).await.unwrap();
        assert_eq!(count, 3);

        // Delete thread
        coordinator.delete_thread(&thread_id).await.unwrap();

        // Verify cleanup
        let count = coordinator.checkpoint_count(&thread_id).await.unwrap();
        assert_eq!(count, 0);

        // Verify sequence tracker was cleared (new checkpoints start at 0)
        let checkpoint = Checkpoint::new(
            thread_id.clone(),
            TestState { value: 100 },
            "new-node".to_string(),
            None,
        );
        coordinator.save(checkpoint.clone()).await.unwrap();

        let loaded = coordinator.load(&checkpoint.id).await.unwrap().unwrap();
        assert_eq!(loaded.metadata.get("sequence").unwrap(), "0");
    }

    #[tokio::test]
    async fn test_parallel_checkpoint_consistency() {
        let inner = MemoryCheckpointer::new();
        let coordinator = Arc::new(DistributedCheckpointCoordinator::new(inner));

        let thread_id = "thread-parallel".to_string();
        let parent_id = Some("parent-checkpoint".to_string());

        // Simulate parallel nodes creating checkpoints with same parent
        let mut handles = vec![];
        for i in 0..5 {
            let coordinator_clone = coordinator.clone();
            let thread_id_clone = thread_id.clone();
            let parent_id_clone = parent_id.clone();

            let handle = tokio::spawn(async move {
                tokio::time::sleep(tokio::time::Duration::from_millis(i * 10)).await;
                let checkpoint = Checkpoint::new(
                    thread_id_clone,
                    TestState { value: i as i32 },
                    format!("parallel-node-{}", i),
                    parent_id_clone,
                );
                coordinator_clone.save(checkpoint).await
            });
            handles.push(handle);
        }

        // Wait for all to complete
        for handle in handles {
            handle.await.unwrap().unwrap();
        }

        // Verify all checkpoints were saved
        let count = coordinator.checkpoint_count(&thread_id).await.unwrap();
        assert_eq!(count, 5);

        // Verify checkpoints have different sequence numbers
        let checkpoints = coordinator.list(&thread_id).await.unwrap();
        let sequences: Vec<u64> = checkpoints
            .iter()
            .map(|cp| cp.metadata.get("sequence").unwrap().parse().unwrap())
            .collect();

        // Should have unique sequences even though they were parallel
        let mut unique_sequences = sequences.clone();
        unique_sequences.sort();
        unique_sequences.dedup();
        assert_eq!(unique_sequences.len(), 5);
    }

    #[tokio::test]
    async fn test_clone_preserves_state() {
        let inner = MemoryCheckpointer::new();
        let coordinator = DistributedCheckpointCoordinator::new(inner);

        let thread_id = "thread-clone".to_string();

        // Save checkpoint with original coordinator
        let checkpoint1 = Checkpoint::new(
            thread_id.clone(),
            TestState { value: 1 },
            "node-1".to_string(),
            None,
        );
        coordinator.save(checkpoint1.clone()).await.unwrap();

        // Clone coordinator
        let coordinator_clone = coordinator.clone();

        // Save checkpoint with cloned coordinator
        let checkpoint2 = Checkpoint::new(
            thread_id.clone(),
            TestState { value: 2 },
            "node-2".to_string(),
            None,
        );
        coordinator_clone.save(checkpoint2.clone()).await.unwrap();

        // Both coordinators should see both checkpoints (shared state)
        let count = coordinator.checkpoint_count(&thread_id).await.unwrap();
        assert_eq!(count, 2);

        let count_clone = coordinator_clone
            .checkpoint_count(&thread_id)
            .await
            .unwrap();
        assert_eq!(count_clone, 2);

        // Sequence numbers should be consistent (0, 1)
        let checkpoints = coordinator.list(&thread_id).await.unwrap();
        let mut sequences: Vec<u64> = checkpoints
            .iter()
            .map(|cp| cp.metadata.get("sequence").unwrap().parse().unwrap())
            .collect();
        sequences.sort();
        assert_eq!(sequences, vec![0, 1]);
    }

    #[tokio::test]
    async fn test_distributed_metadata_added() {
        let inner = MemoryCheckpointer::new();
        let coordinator = DistributedCheckpointCoordinator::new(inner);

        let checkpoint = Checkpoint::new(
            "thread-metadata".to_string(),
            TestState { value: 42 },
            "node-1".to_string(),
            None,
        );

        coordinator.save(checkpoint.clone()).await.unwrap();

        // Load and verify distributed metadata was added
        let loaded = coordinator.load(&checkpoint.id).await.unwrap().unwrap();
        assert_eq!(loaded.metadata.get("distributed").unwrap(), "true");
        assert!(loaded.metadata.contains_key("sequence"));
    }

    #[tokio::test]
    async fn test_get_latest() {
        let inner = MemoryCheckpointer::new();
        let coordinator = DistributedCheckpointCoordinator::new(inner);

        let thread_id = "thread-latest".to_string();

        // Save multiple checkpoints
        for i in 0..3 {
            let checkpoint = Checkpoint::new(
                thread_id.clone(),
                TestState { value: i },
                format!("node-{}", i),
                None,
            );
            coordinator.save(checkpoint).await.unwrap();
            tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
        }

        // Get latest should return the last checkpoint
        let latest = coordinator.get_latest(&thread_id).await.unwrap();
        assert!(latest.is_some());
        let latest = latest.unwrap();

        // Should have highest sequence number
        assert_eq!(latest.metadata.get("sequence").unwrap(), "2");
    }

    #[tokio::test]
    async fn test_delete_checkpoint() {
        let inner = MemoryCheckpointer::new();
        let coordinator = DistributedCheckpointCoordinator::new(inner);

        let thread_id = "thread-delete".to_string();

        // Save checkpoint
        let checkpoint = Checkpoint::new(
            thread_id.clone(),
            TestState { value: 42 },
            "node-1".to_string(),
            None,
        );
        coordinator.save(checkpoint.clone()).await.unwrap();

        // Verify it exists
        let loaded = coordinator.load(&checkpoint.id).await.unwrap();
        assert!(loaded.is_some());

        // Delete it
        coordinator.delete(&checkpoint.id).await.unwrap();

        // Verify it's gone
        let loaded = coordinator.load(&checkpoint.id).await.unwrap();
        assert!(loaded.is_none());
    }

    #[tokio::test]
    async fn test_load_nonexistent_checkpoint() {
        let inner = MemoryCheckpointer::<TestState>::new();
        let coordinator = DistributedCheckpointCoordinator::new(inner);

        let loaded = coordinator.load("nonexistent-id").await.unwrap();
        assert!(loaded.is_none());
    }

    #[tokio::test]
    async fn test_get_latest_empty_thread() {
        let inner = MemoryCheckpointer::<TestState>::new();
        let coordinator = DistributedCheckpointCoordinator::new(inner);

        let latest = coordinator.get_latest("empty-thread").await.unwrap();
        assert!(latest.is_none());
    }

    #[tokio::test]
    async fn test_checkpoint_count_empty_thread() {
        let inner = MemoryCheckpointer::<TestState>::new();
        let coordinator = DistributedCheckpointCoordinator::new(inner);

        let count = coordinator.checkpoint_count("empty-thread").await.unwrap();
        assert_eq!(count, 0);
    }

    #[tokio::test]
    async fn test_list_checkpoints() {
        let inner = MemoryCheckpointer::new();
        let coordinator = DistributedCheckpointCoordinator::new(inner);

        let thread_id = "thread-list".to_string();

        // Save checkpoints
        for i in 0..4 {
            let checkpoint = Checkpoint::new(
                thread_id.clone(),
                TestState { value: i },
                format!("node-{}", i),
                None,
            );
            coordinator.save(checkpoint).await.unwrap();
        }

        // List checkpoints
        let checkpoints = coordinator.list(&thread_id).await.unwrap();
        assert_eq!(checkpoints.len(), 4);

        // All should have sequence metadata
        for cp in checkpoints {
            assert!(cp.metadata.contains_key("sequence"));
            assert!(cp.metadata.contains_key("distributed"));
        }
    }

    #[tokio::test]
    async fn test_delete_thread_with_pending() {
        let inner = MemoryCheckpointer::new();
        let coordinator = Arc::new(DistributedCheckpointCoordinator::new(inner));

        let thread_id1 = "thread-1".to_string();
        let thread_id2 = "thread-2".to_string();

        // Save checkpoints for both threads concurrently
        let coordinator_clone = coordinator.clone();
        let thread_id1_clone = thread_id1.clone();
        let handle1 = tokio::spawn(async move {
            for i in 0..3 {
                let checkpoint = Checkpoint::new(
                    thread_id1_clone.clone(),
                    TestState { value: i },
                    format!("node-{}", i),
                    None,
                );
                coordinator_clone.save(checkpoint).await.unwrap();
            }
        });

        let coordinator_clone = coordinator.clone();
        let thread_id2_clone = thread_id2.clone();
        let handle2 = tokio::spawn(async move {
            for i in 0..3 {
                let checkpoint = Checkpoint::new(
                    thread_id2_clone.clone(),
                    TestState { value: i + 100 },
                    format!("node-{}", i),
                    None,
                );
                coordinator_clone.save(checkpoint).await.unwrap();
            }
        });

        handle1.await.unwrap();
        handle2.await.unwrap();

        // Verify both threads have checkpoints
        let count1 = coordinator.checkpoint_count(&thread_id1).await.unwrap();
        let count2 = coordinator.checkpoint_count(&thread_id2).await.unwrap();
        assert_eq!(count1, 3);
        assert_eq!(count2, 3);

        // Delete thread 1
        coordinator.delete_thread(&thread_id1).await.unwrap();

        // Verify thread 1 is deleted but thread 2 remains
        let count1 = coordinator.checkpoint_count(&thread_id1).await.unwrap();
        let count2 = coordinator.checkpoint_count(&thread_id2).await.unwrap();
        assert_eq!(count1, 0);
        assert_eq!(count2, 3);

        // Verify sequence tracker was cleared for thread 1 (new checkpoint starts at 0)
        let checkpoint = Checkpoint::new(
            thread_id1.clone(),
            TestState { value: 999 },
            "new-node".to_string(),
            None,
        );
        coordinator.save(checkpoint.clone()).await.unwrap();
        let loaded = coordinator.load(&checkpoint.id).await.unwrap().unwrap();
        assert_eq!(loaded.metadata.get("sequence").unwrap(), "0");

        // Thread 2 sequence should continue from 3
        let checkpoint2 = Checkpoint::new(
            thread_id2.clone(),
            TestState { value: 888 },
            "new-node".to_string(),
            None,
        );
        coordinator.save(checkpoint2.clone()).await.unwrap();
        let loaded2 = coordinator.load(&checkpoint2.id).await.unwrap().unwrap();
        assert_eq!(loaded2.metadata.get("sequence").unwrap(), "3");
    }

    #[tokio::test]
    async fn test_multiple_thread_independence() {
        let inner = MemoryCheckpointer::new();
        let coordinator = DistributedCheckpointCoordinator::new(inner);

        // Create checkpoints for multiple threads
        for thread_num in 0..3 {
            let thread_id = format!("thread-{}", thread_num);
            for i in 0..2 {
                let checkpoint = Checkpoint::new(
                    thread_id.clone(),
                    TestState { value: i },
                    format!("node-{}", i),
                    None,
                );
                coordinator.save(checkpoint).await.unwrap();
            }
        }

        // Each thread should have 2 checkpoints
        for thread_num in 0..3 {
            let thread_id = format!("thread-{}", thread_num);
            let count = coordinator.checkpoint_count(&thread_id).await.unwrap();
            assert_eq!(count, 2);

            // Each thread should have independent sequence numbers (0, 1)
            let checkpoints = coordinator.list(&thread_id).await.unwrap();
            let mut sequences: Vec<u64> = checkpoints
                .iter()
                .map(|cp| cp.metadata.get("sequence").unwrap().parse().unwrap())
                .collect();
            sequences.sort();
            assert_eq!(sequences, vec![0, 1]);
        }
    }

    #[tokio::test]
    async fn test_coordinator_checkpointer_trait_methods() {
        let inner = MemoryCheckpointer::new();
        let coordinator = DistributedCheckpointCoordinator::new(inner);

        let thread_id = "thread-trait".to_string();

        // Test save via Checkpointer trait
        let checkpoint1 = Checkpoint::new(
            thread_id.clone(),
            TestState { value: 100 },
            "node-1".to_string(),
            None,
        );

        // Use trait method explicitly
        use crate::checkpoint::Checkpointer as _;
        coordinator.save(checkpoint1.clone()).await.unwrap();

        // Test load via trait
        let loaded = coordinator.load(&checkpoint1.id).await.unwrap();
        assert!(loaded.is_some());
        assert_eq!(loaded.unwrap().state.value, 100);

        // Save another checkpoint
        let checkpoint2 = Checkpoint::new(
            thread_id.clone(),
            TestState { value: 200 },
            "node-2".to_string(),
            None,
        );
        coordinator.save(checkpoint2.clone()).await.unwrap();

        // Test get_latest via trait
        let latest = coordinator.get_latest(&thread_id).await.unwrap();
        assert!(latest.is_some());
        let latest = latest.unwrap();
        assert_eq!(latest.state.value, 200);

        // Test list via trait
        let list = coordinator.list(&thread_id).await.unwrap();
        assert_eq!(list.len(), 2);

        // Test delete via trait
        coordinator.delete(&checkpoint1.id).await.unwrap();
        let loaded = coordinator.load(&checkpoint1.id).await.unwrap();
        assert!(loaded.is_none());

        // Should only have 1 checkpoint left
        let list = coordinator.list(&thread_id).await.unwrap();
        assert_eq!(list.len(), 1);
    }

    #[tokio::test]
    async fn test_metadata_insertion_and_coordination() {
        let inner = MemoryCheckpointer::new();
        let coordinator = DistributedCheckpointCoordinator::new(inner);

        let checkpoint = Checkpoint::new(
            "thread-meta".to_string(),
            TestState { value: 42 },
            "node-meta".to_string(),
            None,
        );

        // Save and verify all metadata is added
        coordinator.save(checkpoint.clone()).await.unwrap();

        let loaded = coordinator.load(&checkpoint.id).await.unwrap().unwrap();

        // Verify distributed flag
        assert!(loaded.metadata.contains_key("distributed"));
        assert_eq!(loaded.metadata.get("distributed").unwrap(), "true");

        // Verify sequence
        assert!(loaded.metadata.contains_key("sequence"));
        assert_eq!(loaded.metadata.get("sequence").unwrap(), "0");

        // Save another from same thread
        let checkpoint2 = Checkpoint::new(
            "thread-meta".to_string(),
            TestState { value: 43 },
            "node-meta-2".to_string(),
            None,
        );
        coordinator.save(checkpoint2.clone()).await.unwrap();

        let loaded2 = coordinator.load(&checkpoint2.id).await.unwrap().unwrap();
        assert_eq!(loaded2.metadata.get("sequence").unwrap(), "1");
    }

    #[tokio::test]
    async fn test_sequence_assignment_across_operations() {
        let inner = MemoryCheckpointer::new();
        let coordinator = DistributedCheckpointCoordinator::new(inner);

        let thread_id = "thread-seq".to_string();

        // Save, delete, save again - sequence should continue
        let checkpoint1 = Checkpoint::new(
            thread_id.clone(),
            TestState { value: 1 },
            "node-1".to_string(),
            None,
        );
        coordinator.save(checkpoint1.clone()).await.unwrap();

        let checkpoint2 = Checkpoint::new(
            thread_id.clone(),
            TestState { value: 2 },
            "node-2".to_string(),
            None,
        );
        coordinator.save(checkpoint2.clone()).await.unwrap();

        // Delete first checkpoint
        coordinator.delete(&checkpoint1.id).await.unwrap();

        // Save another - sequence should be 2 (continuing from before)
        let checkpoint3 = Checkpoint::new(
            thread_id.clone(),
            TestState { value: 3 },
            "node-3".to_string(),
            None,
        );
        coordinator.save(checkpoint3.clone()).await.unwrap();

        let loaded3 = coordinator.load(&checkpoint3.id).await.unwrap().unwrap();
        assert_eq!(loaded3.metadata.get("sequence").unwrap(), "2");

        // Verify we have 2 checkpoints (deleted 1)
        let count = coordinator.checkpoint_count(&thread_id).await.unwrap();
        assert_eq!(count, 2);
    }
}
