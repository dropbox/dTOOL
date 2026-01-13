// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

// Allow clone_on_ref_ptr: TieredCheckpointer uses Arc for shared L1/L2 storage backends
#![allow(clippy::clone_on_ref_ptr)]

//! Multi-tier checkpointing with layered caching
//!
//! Provides a two-tier checkpointing strategy:
//! - L1 cache (fast): Redis, in-memory, etc.
//! - L2 storage (durable): S3, DynamoDB, PostgreSQL, etc.

use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc,
};

use tokio::sync::Semaphore;
use tracing::debug;

use crate::{GraphState, Result};

use super::{Checkpoint, CheckpointMetadata, Checkpointer, ThreadInfo};

/// Write policy for multi-tier checkpointing
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum WritePolicy {
    /// Write to both L1 and L2 simultaneously (default)
    /// - Pros: Simple, L1 always warm, durable immediately
    /// - Cons: Higher write latency (slowest of both tiers)
    WriteThrough,

    /// Write to L1 immediately, write to L2 asynchronously in background
    /// - Pros: Low write latency, L1 always warm
    /// - Cons: Risk of data loss if process crashes before L2 write
    WriteBehind,

    /// Write only to L2, skip L1 cache
    /// - Pros: Useful for large states that don't fit in cache
    /// - Cons: L1 cache not populated, slower reads
    WriteAround,
}

/// Default max concurrent background L2 writes for WriteBehind policy
const DEFAULT_MAX_CONCURRENT_L2_WRITES: usize = 100;

/// Multi-tier checkpointer with layered caching.
///
/// Provides a two-tier checkpointing strategy with configurable write policies:
/// - **L1 cache** (fast): Redis, in-memory, etc.
/// - **L2 storage** (durable): S3, DynamoDB, PostgreSQL, etc.
///
/// # Write Policies
///
/// - [`WritePolicy::WriteThrough`]: Write to both tiers simultaneously (default, safest)
/// - [`WritePolicy::WriteBehind`]: Write L1 immediately, L2 asynchronously (fastest)
/// - [`WritePolicy::WriteAround`]: Skip L1, write only to L2 (for large states)
///
/// # Example
///
/// ```rust,ignore
/// use dashflow::checkpoint::{MultiTierCheckpointer, WritePolicy};
///
/// let checkpointer = MultiTierCheckpointer::new(
///     Arc::new(redis_checkpointer),
///     Arc::new(s3_checkpointer),
/// ).with_write_policy(WritePolicy::WriteThrough);
/// ```
pub struct MultiTierCheckpointer<S: GraphState> {
    /// Fast tier (L1 cache): Redis, in-memory, etc.
    l1_cache: Arc<dyn Checkpointer<S>>,

    /// Slow tier (L2 storage): S3, DynamoDB, PostgreSQL, etc.
    l2_storage: Arc<dyn Checkpointer<S>>,

    /// Write policy: write-through, write-behind, or write-around
    write_policy: WritePolicy,

    /// Whether to warm L1 cache on L2 reads (default: true)
    warm_l1_on_read: bool,

    /// Semaphore to bound concurrent background L2 writes in WriteBehind mode.
    /// Prevents unbounded task spawning under high write load.
    l2_write_semaphore: Arc<Semaphore>,

    /// Counter for dropped L2 writes when semaphore is full
    l2_writes_dropped: Arc<AtomicU64>,
}

impl<S: GraphState> MultiTierCheckpointer<S> {
    /// Create a new multi-tier checkpointer
    ///
    /// By default:
    /// - Write policy: `WriteThrough` (write to both tiers)
    /// - Warm L1 on read: true (populate L1 cache when reading from L2)
    /// - Max concurrent L2 writes: 100 (for WriteBehind mode)
    pub fn new(l1_cache: Arc<dyn Checkpointer<S>>, l2_storage: Arc<dyn Checkpointer<S>>) -> Self {
        Self {
            l1_cache,
            l2_storage,
            write_policy: WritePolicy::WriteThrough,
            warm_l1_on_read: true,
            l2_write_semaphore: Arc::new(Semaphore::new(DEFAULT_MAX_CONCURRENT_L2_WRITES)),
            l2_writes_dropped: Arc::new(AtomicU64::new(0)),
        }
    }

    /// Set the write policy
    #[must_use]
    pub fn with_write_policy(mut self, policy: WritePolicy) -> Self {
        self.write_policy = policy;
        self
    }

    /// Set whether to warm L1 cache on L2 reads
    #[must_use]
    pub fn with_warm_l1_on_read(mut self, warm: bool) -> Self {
        self.warm_l1_on_read = warm;
        self
    }

    /// Set max concurrent background L2 writes for WriteBehind mode
    ///
    /// When the limit is reached, additional L2 writes are dropped (L1 still succeeds).
    /// Use `l2_writes_dropped()` to monitor for backpressure.
    ///
    /// Default: 100
    #[must_use]
    pub fn with_max_concurrent_l2_writes(mut self, max: usize) -> Self {
        self.l2_write_semaphore = Arc::new(Semaphore::new(max));
        self
    }

    /// Get the count of L2 writes dropped due to backpressure in WriteBehind mode
    ///
    /// This counter increments when the max concurrent L2 writes limit is reached.
    /// Monitor this value to detect if your L2 storage is becoming a bottleneck.
    pub fn l2_writes_dropped(&self) -> u64 {
        self.l2_writes_dropped.load(Ordering::Relaxed)
    }
}

#[async_trait::async_trait]
impl<S: GraphState> Checkpointer<S> for MultiTierCheckpointer<S> {
    /// Save a checkpoint according to write policy
    async fn save(&self, checkpoint: Checkpoint<S>) -> Result<()> {
        match self.write_policy {
            WritePolicy::WriteThrough => {
                // Write to both tiers simultaneously
                let l1_result = self.l1_cache.save(checkpoint.clone()).await;
                let l2_result = self.l2_storage.save(checkpoint).await;

                // Return first error, or success if both succeed
                l1_result.and(l2_result)
            }
            WritePolicy::WriteBehind => {
                // Write to L1 immediately
                self.l1_cache.save(checkpoint.clone()).await?;

                // Write to L2 asynchronously in background with bounded concurrency.
                // Uses try_acquire to avoid blocking the caller - if at capacity,
                // the L2 write is dropped (L1 still succeeds).
                let permit = match Arc::clone(&self.l2_write_semaphore).try_acquire_owned() {
                    Ok(permit) => permit,
                    Err(_) => {
                        // Backpressure: too many concurrent L2 writes
                        let dropped = self.l2_writes_dropped.fetch_add(1, Ordering::Relaxed) + 1;
                        if dropped % 100 == 1 {
                            tracing::warn!(
                                checkpoint_id = %checkpoint.id,
                                dropped_count = dropped,
                                "L2 write dropped due to backpressure (WriteBehind mode)"
                            );
                        }
                        return Ok(());
                    }
                };

                let l2_storage = self.l2_storage.clone();
                let checkpoint_id = checkpoint.id.clone();
                tokio::spawn(async move {
                    let _permit = permit; // Keep permit alive until write completes
                    if let Err(e) = l2_storage.save(checkpoint).await {
                        tracing::warn!(
                            checkpoint_id = %checkpoint_id,
                            error = %e,
                            "Failed to save checkpoint to L2 storage in background"
                        );
                    }
                });

                Ok(())
            }
            WritePolicy::WriteAround => {
                // Write only to L2, skip L1 cache
                self.l2_storage.save(checkpoint).await
            }
        }
    }

    /// Load a checkpoint from L1 cache, fallback to L2 storage
    async fn load(&self, checkpoint_id: &str) -> Result<Option<Checkpoint<S>>> {
        // Try L1 cache first
        match self.l1_cache.load(checkpoint_id).await? {
            Some(checkpoint) => Ok(Some(checkpoint)),
            None => {
                // L1 miss, try L2 storage
                match self.l2_storage.load(checkpoint_id).await? {
                    Some(checkpoint) => {
                        // L2 hit - warm L1 cache if enabled (best-effort)
                        if self.warm_l1_on_read {
                            if let Err(e) = self.l1_cache.save(checkpoint.clone()).await {
                                debug!(checkpoint_id = %checkpoint.id, "L1 cache warm failed (non-critical): {}", e);
                            }
                        }
                        Ok(Some(checkpoint))
                    }
                    None => Ok(None),
                }
            }
        }
    }

    /// Get latest checkpoint from L1 cache, fallback to L2 storage
    async fn get_latest(&self, thread_id: &str) -> Result<Option<Checkpoint<S>>> {
        // Try L1 cache first
        match self.l1_cache.get_latest(thread_id).await? {
            Some(checkpoint) => Ok(Some(checkpoint)),
            None => {
                // L1 miss, try L2 storage
                match self.l2_storage.get_latest(thread_id).await? {
                    Some(checkpoint) => {
                        // L2 hit - warm L1 cache if enabled (best-effort)
                        if self.warm_l1_on_read {
                            if let Err(e) = self.l1_cache.save(checkpoint.clone()).await {
                                debug!(checkpoint_id = %checkpoint.id, "L1 cache warm failed (non-critical): {}", e);
                            }
                        }
                        Ok(Some(checkpoint))
                    }
                    None => Ok(None),
                }
            }
        }
    }

    /// List checkpoints from L2 storage (canonical source)
    async fn list(&self, thread_id: &str) -> Result<Vec<CheckpointMetadata>> {
        // L2 is the source of truth for listing
        self.l2_storage.list(thread_id).await
    }

    /// Delete from both tiers
    async fn delete(&self, checkpoint_id: &str) -> Result<()> {
        // Delete from both tiers - L1 errors are non-critical (item might not be cached)
        if let Err(e) = self.l1_cache.delete(checkpoint_id).await {
            debug!(
                checkpoint_id,
                "L1 cache delete failed (non-critical): {}", e
            );
        }
        self.l2_storage.delete(checkpoint_id).await
    }

    /// Delete thread from both tiers
    async fn delete_thread(&self, thread_id: &str) -> Result<()> {
        // Delete from both tiers - L1 errors are non-critical (thread might not be cached)
        if let Err(e) = self.l1_cache.delete_thread(thread_id).await {
            debug!(
                thread_id,
                "L1 cache delete_thread failed (non-critical): {}", e
            );
        }
        self.l2_storage.delete_thread(thread_id).await
    }

    /// List threads from L2 storage (canonical source)
    async fn list_threads(&self) -> Result<Vec<ThreadInfo>> {
        // L2 is the source of truth for thread listing
        self.l2_storage.list_threads().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::checkpoint::MemoryCheckpointer;
    use serde::{Deserialize, Serialize};
    use std::collections::HashMap;
    use std::time::SystemTime;

    /// Test state for checkpoint tests
    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    struct TestState {
        counter: i32,
        message: String,
    }

    impl Default for TestState {
        fn default() -> Self {
            Self {
                counter: 0,
                message: String::new(),
            }
        }
    }

    fn make_checkpoint(id: &str, thread_id: &str, state: TestState) -> Checkpoint<TestState> {
        Checkpoint {
            id: id.into(),
            thread_id: thread_id.into(),
            state,
            node: "test-node".to_string(),
            timestamp: SystemTime::now(),
            parent_id: None,
            metadata: HashMap::new(),
        }
    }

    // ========================================================================
    // WritePolicy Tests
    // ========================================================================

    #[test]
    fn test_write_policy_clone() {
        let policy = WritePolicy::WriteThrough;
        let cloned = policy.clone();
        assert_eq!(policy, cloned);
    }

    #[test]
    fn test_write_policy_debug() {
        let policy = WritePolicy::WriteBehind;
        let debug = format!("{:?}", policy);
        assert!(debug.contains("WriteBehind"));
    }

    #[test]
    fn test_write_policy_eq() {
        assert_eq!(WritePolicy::WriteThrough, WritePolicy::WriteThrough);
        assert_eq!(WritePolicy::WriteBehind, WritePolicy::WriteBehind);
        assert_eq!(WritePolicy::WriteAround, WritePolicy::WriteAround);
        assert_ne!(WritePolicy::WriteThrough, WritePolicy::WriteBehind);
    }

    // ========================================================================
    // MultiTierCheckpointer Creation Tests
    // ========================================================================

    #[test]
    fn test_multi_tier_checkpointer_new() {
        let l1: Arc<dyn Checkpointer<TestState>> = Arc::new(MemoryCheckpointer::new());
        let l2: Arc<dyn Checkpointer<TestState>> = Arc::new(MemoryCheckpointer::new());
        let tier = MultiTierCheckpointer::new(l1, l2);

        // Default is WriteThrough
        assert_eq!(tier.write_policy, WritePolicy::WriteThrough);
        assert!(tier.warm_l1_on_read);
    }

    #[test]
    fn test_multi_tier_checkpointer_with_write_policy() {
        let l1: Arc<dyn Checkpointer<TestState>> = Arc::new(MemoryCheckpointer::new());
        let l2: Arc<dyn Checkpointer<TestState>> = Arc::new(MemoryCheckpointer::new());
        let tier = MultiTierCheckpointer::new(l1, l2).with_write_policy(WritePolicy::WriteBehind);

        assert_eq!(tier.write_policy, WritePolicy::WriteBehind);
    }

    #[test]
    fn test_multi_tier_checkpointer_with_warm_l1_on_read() {
        let l1: Arc<dyn Checkpointer<TestState>> = Arc::new(MemoryCheckpointer::new());
        let l2: Arc<dyn Checkpointer<TestState>> = Arc::new(MemoryCheckpointer::new());
        let tier = MultiTierCheckpointer::new(l1, l2).with_warm_l1_on_read(false);

        assert!(!tier.warm_l1_on_read);
    }

    #[test]
    fn test_multi_tier_checkpointer_with_max_concurrent_l2_writes() {
        let l1: Arc<dyn Checkpointer<TestState>> = Arc::new(MemoryCheckpointer::new());
        let l2: Arc<dyn Checkpointer<TestState>> = Arc::new(MemoryCheckpointer::new());
        let tier = MultiTierCheckpointer::new(l1, l2).with_max_concurrent_l2_writes(50);

        assert_eq!(tier.l2_write_semaphore.available_permits(), 50);
    }

    #[test]
    fn test_multi_tier_checkpointer_l2_writes_dropped_initial() {
        let l1: Arc<dyn Checkpointer<TestState>> = Arc::new(MemoryCheckpointer::new());
        let l2: Arc<dyn Checkpointer<TestState>> = Arc::new(MemoryCheckpointer::new());
        let tier = MultiTierCheckpointer::new(l1, l2);

        assert_eq!(tier.l2_writes_dropped(), 0);
    }

    #[test]
    fn test_multi_tier_checkpointer_builder_chaining() {
        let l1: Arc<dyn Checkpointer<TestState>> = Arc::new(MemoryCheckpointer::new());
        let l2: Arc<dyn Checkpointer<TestState>> = Arc::new(MemoryCheckpointer::new());
        let tier = MultiTierCheckpointer::new(l1, l2)
            .with_write_policy(WritePolicy::WriteAround)
            .with_warm_l1_on_read(false)
            .with_max_concurrent_l2_writes(25);

        assert_eq!(tier.write_policy, WritePolicy::WriteAround);
        assert!(!tier.warm_l1_on_read);
        assert_eq!(tier.l2_write_semaphore.available_permits(), 25);
    }

    // ========================================================================
    // WriteThrough Policy Tests
    // ========================================================================

    #[tokio::test]
    async fn test_write_through_save_writes_to_both() {
        let l1: Arc<MemoryCheckpointer<TestState>> = Arc::new(MemoryCheckpointer::new());
        let l2: Arc<MemoryCheckpointer<TestState>> = Arc::new(MemoryCheckpointer::new());
        let tier = MultiTierCheckpointer::new(
            l1.clone() as Arc<dyn Checkpointer<TestState>>,
            l2.clone() as Arc<dyn Checkpointer<TestState>>,
        )
        .with_write_policy(WritePolicy::WriteThrough);

        let state = TestState {
            counter: 42,
            message: "test".to_string(),
        };
        let checkpoint = make_checkpoint("cp-1", "thread-1", state.clone());

        tier.save(checkpoint).await.unwrap();

        // Both L1 and L2 should have the checkpoint
        let l1_loaded = l1.load("cp-1").await.unwrap();
        assert!(l1_loaded.is_some());
        assert_eq!(l1_loaded.unwrap().state, state);

        let l2_loaded = l2.load("cp-1").await.unwrap();
        assert!(l2_loaded.is_some());
        assert_eq!(l2_loaded.unwrap().state, state);
    }

    #[tokio::test]
    async fn test_write_through_load_from_l1() {
        let l1: Arc<MemoryCheckpointer<TestState>> = Arc::new(MemoryCheckpointer::new());
        let l2: Arc<MemoryCheckpointer<TestState>> = Arc::new(MemoryCheckpointer::new());
        let tier = MultiTierCheckpointer::new(
            l1.clone() as Arc<dyn Checkpointer<TestState>>,
            l2.clone() as Arc<dyn Checkpointer<TestState>>,
        )
        .with_write_policy(WritePolicy::WriteThrough);

        let state = TestState {
            counter: 1,
            message: "l1".to_string(),
        };
        let checkpoint = make_checkpoint("cp-1", "thread-1", state.clone());
        tier.save(checkpoint).await.unwrap();

        // Load should get from L1 first
        let loaded = tier.load("cp-1").await.unwrap();
        assert!(loaded.is_some());
        assert_eq!(loaded.unwrap().state, state);
    }

    #[tokio::test]
    async fn test_write_through_load_fallback_to_l2() {
        let l1: Arc<MemoryCheckpointer<TestState>> = Arc::new(MemoryCheckpointer::new());
        let l2: Arc<MemoryCheckpointer<TestState>> = Arc::new(MemoryCheckpointer::new());

        // Save directly to L2 only (simulating L1 cache miss)
        let state = TestState {
            counter: 99,
            message: "l2 only".to_string(),
        };
        let checkpoint = make_checkpoint("cp-cold", "thread-1", state.clone());
        l2.save(checkpoint).await.unwrap();

        let tier = MultiTierCheckpointer::new(
            l1.clone() as Arc<dyn Checkpointer<TestState>>,
            l2.clone() as Arc<dyn Checkpointer<TestState>>,
        )
        .with_write_policy(WritePolicy::WriteThrough)
        .with_warm_l1_on_read(true);

        // Load should fallback to L2
        let loaded = tier.load("cp-cold").await.unwrap();
        assert!(loaded.is_some());
        assert_eq!(loaded.unwrap().state, state);

        // L1 should now be warmed
        let l1_loaded = l1.load("cp-cold").await.unwrap();
        assert!(l1_loaded.is_some());
    }

    #[tokio::test]
    async fn test_write_through_load_no_warm() {
        let l1: Arc<MemoryCheckpointer<TestState>> = Arc::new(MemoryCheckpointer::new());
        let l2: Arc<MemoryCheckpointer<TestState>> = Arc::new(MemoryCheckpointer::new());

        // Save directly to L2 only
        let state = TestState::default();
        let checkpoint = make_checkpoint("cp-cold", "thread-1", state);
        l2.save(checkpoint).await.unwrap();

        let tier = MultiTierCheckpointer::new(
            l1.clone() as Arc<dyn Checkpointer<TestState>>,
            l2.clone() as Arc<dyn Checkpointer<TestState>>,
        )
        .with_warm_l1_on_read(false); // Don't warm L1

        // Load should fallback to L2
        let loaded = tier.load("cp-cold").await.unwrap();
        assert!(loaded.is_some());

        // L1 should NOT be warmed
        let l1_loaded = l1.load("cp-cold").await.unwrap();
        assert!(l1_loaded.is_none());
    }

    // ========================================================================
    // WriteAround Policy Tests
    // ========================================================================

    #[tokio::test]
    async fn test_write_around_only_writes_to_l2() {
        let l1: Arc<MemoryCheckpointer<TestState>> = Arc::new(MemoryCheckpointer::new());
        let l2: Arc<MemoryCheckpointer<TestState>> = Arc::new(MemoryCheckpointer::new());
        let tier = MultiTierCheckpointer::new(
            l1.clone() as Arc<dyn Checkpointer<TestState>>,
            l2.clone() as Arc<dyn Checkpointer<TestState>>,
        )
        .with_write_policy(WritePolicy::WriteAround);

        let state = TestState {
            counter: 42,
            message: "test".to_string(),
        };
        let checkpoint = make_checkpoint("cp-1", "thread-1", state.clone());

        tier.save(checkpoint).await.unwrap();

        // L1 should NOT have the checkpoint
        let l1_loaded = l1.load("cp-1").await.unwrap();
        assert!(l1_loaded.is_none());

        // L2 should have the checkpoint
        let l2_loaded = l2.load("cp-1").await.unwrap();
        assert!(l2_loaded.is_some());
        assert_eq!(l2_loaded.unwrap().state, state);
    }

    // ========================================================================
    // WriteBehind Policy Tests
    // ========================================================================

    #[tokio::test]
    async fn test_write_behind_immediate_l1_write() {
        let l1: Arc<MemoryCheckpointer<TestState>> = Arc::new(MemoryCheckpointer::new());
        let l2: Arc<MemoryCheckpointer<TestState>> = Arc::new(MemoryCheckpointer::new());
        let tier = MultiTierCheckpointer::new(
            l1.clone() as Arc<dyn Checkpointer<TestState>>,
            l2.clone() as Arc<dyn Checkpointer<TestState>>,
        )
        .with_write_policy(WritePolicy::WriteBehind);

        let state = TestState {
            counter: 100,
            message: "async".to_string(),
        };
        let checkpoint = make_checkpoint("cp-1", "thread-1", state.clone());

        tier.save(checkpoint).await.unwrap();

        // L1 should have the checkpoint immediately
        let l1_loaded = l1.load("cp-1").await.unwrap();
        assert!(l1_loaded.is_some());
        assert_eq!(l1_loaded.unwrap().state, state);

        // Give async L2 write time to complete
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        // L2 should eventually have it
        let l2_loaded = l2.load("cp-1").await.unwrap();
        assert!(l2_loaded.is_some());
    }

    // ========================================================================
    // get_latest Tests
    // ========================================================================

    #[tokio::test]
    async fn test_get_latest_from_l1() {
        let l1: Arc<MemoryCheckpointer<TestState>> = Arc::new(MemoryCheckpointer::new());
        let l2: Arc<MemoryCheckpointer<TestState>> = Arc::new(MemoryCheckpointer::new());
        let tier = MultiTierCheckpointer::new(
            l1.clone() as Arc<dyn Checkpointer<TestState>>,
            l2.clone() as Arc<dyn Checkpointer<TestState>>,
        );

        for i in 0..3 {
            let state = TestState {
                counter: i,
                message: format!("msg-{}", i),
            };
            let checkpoint = make_checkpoint(&format!("cp-{}", i), "thread-1", state);
            tier.save(checkpoint).await.unwrap();
        }

        let latest = tier.get_latest("thread-1").await.unwrap();
        assert!(latest.is_some());
        assert_eq!(latest.unwrap().state.counter, 2);
    }

    #[tokio::test]
    async fn test_get_latest_fallback_to_l2() {
        let l1: Arc<MemoryCheckpointer<TestState>> = Arc::new(MemoryCheckpointer::new());
        let l2: Arc<MemoryCheckpointer<TestState>> = Arc::new(MemoryCheckpointer::new());

        // Save directly to L2
        let state = TestState {
            counter: 50,
            message: "l2 latest".to_string(),
        };
        let checkpoint = make_checkpoint("cp-l2", "thread-1", state.clone());
        l2.save(checkpoint).await.unwrap();

        let tier = MultiTierCheckpointer::new(
            l1.clone() as Arc<dyn Checkpointer<TestState>>,
            l2.clone() as Arc<dyn Checkpointer<TestState>>,
        )
        .with_warm_l1_on_read(true);

        let latest = tier.get_latest("thread-1").await.unwrap();
        assert!(latest.is_some());
        assert_eq!(latest.unwrap().state, state);

        // Should have warmed L1
        let l1_latest = l1.get_latest("thread-1").await.unwrap();
        assert!(l1_latest.is_some());
    }

    #[tokio::test]
    async fn test_get_latest_nonexistent() {
        let l1: Arc<dyn Checkpointer<TestState>> = Arc::new(MemoryCheckpointer::new());
        let l2: Arc<dyn Checkpointer<TestState>> = Arc::new(MemoryCheckpointer::new());
        let tier = MultiTierCheckpointer::new(l1, l2);

        let latest = tier.get_latest("nonexistent-thread").await.unwrap();
        assert!(latest.is_none());
    }

    // ========================================================================
    // list Tests
    // ========================================================================

    #[tokio::test]
    async fn test_list_from_l2() {
        let l1: Arc<dyn Checkpointer<TestState>> = Arc::new(MemoryCheckpointer::new());
        let l2: Arc<MemoryCheckpointer<TestState>> = Arc::new(MemoryCheckpointer::new());

        // Save to L2 directly (L2 is canonical source)
        for i in 0..3 {
            let state = TestState::default();
            let checkpoint = make_checkpoint(&format!("cp-{}", i), "thread-1", state);
            l2.save(checkpoint).await.unwrap();
        }

        let tier = MultiTierCheckpointer::new(l1, l2 as Arc<dyn Checkpointer<TestState>>);

        let list = tier.list("thread-1").await.unwrap();
        assert_eq!(list.len(), 3);
    }

    // ========================================================================
    // delete Tests
    // ========================================================================

    #[tokio::test]
    async fn test_delete_from_both_tiers() {
        let l1: Arc<MemoryCheckpointer<TestState>> = Arc::new(MemoryCheckpointer::new());
        let l2: Arc<MemoryCheckpointer<TestState>> = Arc::new(MemoryCheckpointer::new());
        let tier = MultiTierCheckpointer::new(
            l1.clone() as Arc<dyn Checkpointer<TestState>>,
            l2.clone() as Arc<dyn Checkpointer<TestState>>,
        )
        .with_write_policy(WritePolicy::WriteThrough);

        let state = TestState::default();
        let checkpoint = make_checkpoint("cp-1", "thread-1", state);
        tier.save(checkpoint).await.unwrap();

        // Verify in both
        assert!(l1.load("cp-1").await.unwrap().is_some());
        assert!(l2.load("cp-1").await.unwrap().is_some());

        // Delete
        tier.delete("cp-1").await.unwrap();

        // Should be gone from both
        assert!(l1.load("cp-1").await.unwrap().is_none());
        assert!(l2.load("cp-1").await.unwrap().is_none());
    }

    // ========================================================================
    // delete_thread Tests
    // ========================================================================

    #[tokio::test]
    async fn test_delete_thread_from_both_tiers() {
        let l1: Arc<MemoryCheckpointer<TestState>> = Arc::new(MemoryCheckpointer::new());
        let l2: Arc<MemoryCheckpointer<TestState>> = Arc::new(MemoryCheckpointer::new());
        let tier = MultiTierCheckpointer::new(
            l1.clone() as Arc<dyn Checkpointer<TestState>>,
            l2.clone() as Arc<dyn Checkpointer<TestState>>,
        )
        .with_write_policy(WritePolicy::WriteThrough);

        for i in 0..3 {
            let state = TestState::default();
            let checkpoint = make_checkpoint(&format!("cp-{}", i), "thread-1", state);
            tier.save(checkpoint).await.unwrap();
        }

        // Delete thread
        tier.delete_thread("thread-1").await.unwrap();

        // Should be empty in both
        let l1_list = l1.list("thread-1").await.unwrap();
        assert!(l1_list.is_empty());

        let l2_list = l2.list("thread-1").await.unwrap();
        assert!(l2_list.is_empty());
    }

    // ========================================================================
    // list_threads Tests
    // ========================================================================

    #[tokio::test]
    async fn test_list_threads_from_l2() {
        let l1: Arc<dyn Checkpointer<TestState>> = Arc::new(MemoryCheckpointer::new());
        let l2: Arc<MemoryCheckpointer<TestState>> = Arc::new(MemoryCheckpointer::new());

        // Save threads to L2
        for t in 0..3 {
            let state = TestState::default();
            let checkpoint = make_checkpoint(&format!("cp-t{}", t), &format!("thread-{}", t), state);
            l2.save(checkpoint).await.unwrap();
        }

        let tier = MultiTierCheckpointer::new(l1, l2 as Arc<dyn Checkpointer<TestState>>);

        let threads = tier.list_threads().await.unwrap();
        assert_eq!(threads.len(), 3);
    }

    // ========================================================================
    // Load Nonexistent Tests
    // ========================================================================

    #[tokio::test]
    async fn test_load_nonexistent() {
        let l1: Arc<dyn Checkpointer<TestState>> = Arc::new(MemoryCheckpointer::new());
        let l2: Arc<dyn Checkpointer<TestState>> = Arc::new(MemoryCheckpointer::new());
        let tier = MultiTierCheckpointer::new(l1, l2);

        let loaded = tier.load("nonexistent").await.unwrap();
        assert!(loaded.is_none());
    }

    // ========================================================================
    // Metadata Preservation Tests
    // ========================================================================

    #[tokio::test]
    async fn test_metadata_preserved() {
        let l1: Arc<dyn Checkpointer<TestState>> = Arc::new(MemoryCheckpointer::new());
        let l2: Arc<dyn Checkpointer<TestState>> = Arc::new(MemoryCheckpointer::new());
        let tier = MultiTierCheckpointer::new(l1, l2);

        let mut metadata = HashMap::new();
        metadata.insert("key1".to_string(), "value1".to_string());
        metadata.insert("key2".to_string(), "value2".to_string());

        let state = TestState::default();
        let checkpoint = Checkpoint {
            id: "cp-meta".into(),
            thread_id: "thread-1".into(),
            state,
            node: "node".to_string(),
            timestamp: SystemTime::now(),
            parent_id: Some("parent-1".into()),
            metadata: metadata.clone(),
        };

        tier.save(checkpoint).await.unwrap();

        let loaded = tier.load("cp-meta").await.unwrap().unwrap();
        assert_eq!(loaded.metadata, metadata);
        assert_eq!(loaded.parent_id, Some("parent-1".into()));
        assert_eq!(loaded.node, "node");
    }

    // ========================================================================
    // Default Constants Test
    // ========================================================================

    #[test]
    fn test_default_max_concurrent_l2_writes() {
        assert_eq!(DEFAULT_MAX_CONCURRENT_L2_WRITES, 100);
    }
}
