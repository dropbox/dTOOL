// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

// Allow clone_on_ref_ptr: ReplicatedCheckpointer uses Arc for shared replica backends
#![allow(clippy::clone_on_ref_ptr)]

//! Cross-region replicated checkpoint storage
//!
//! Replicates checkpoints across multiple backend checkpointers for disaster recovery
//! and high availability. Supports async, sync, and quorum-based replication modes.

use std::sync::Arc;

use crate::constants::DEFAULT_MAX_RETRIES;
use crate::{GraphState, Result};

use super::{Checkpoint, CheckpointMetadata, Checkpointer, ThreadInfo};

/// Replication mode for cross-region checkpoint synchronization
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
pub enum ReplicationMode {
    /// Write to primary, replicate to replicas asynchronously (eventual consistency)
    /// Saves complete immediately after primary write; replica failures are logged but don't fail the operation
    #[default]
    Async,
    /// Write to primary and all replicas synchronously (strong consistency)
    /// Save only succeeds if all replicas succeed; slower but guarantees consistency
    Sync,
    /// Write to primary and a quorum of replicas (majority must succeed)
    /// Provides a balance between availability and consistency
    Quorum,
}

/// Configuration for cross-region replicated checkpointing
#[derive(Debug, Clone)]
pub struct ReplicatedCheckpointerConfig {
    /// Replication mode
    pub mode: ReplicationMode,
    /// Timeout for replica operations (only applies to Sync and Quorum modes)
    pub replica_timeout: std::time::Duration,
    /// Maximum number of retry attempts for failed replica writes
    pub max_retries: u32,
    /// Whether to read from replicas if primary fails (for load balancing and failover)
    pub read_from_replicas: bool,
}

impl Default for ReplicatedCheckpointerConfig {
    fn default() -> Self {
        Self {
            mode: ReplicationMode::Async,
            replica_timeout: std::time::Duration::from_secs(5),
            max_retries: DEFAULT_MAX_RETRIES,
            read_from_replicas: true,
        }
    }
}

impl ReplicatedCheckpointerConfig {
    /// Create a new configuration with default values
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the replication mode
    #[must_use]
    pub fn with_mode(mut self, mode: ReplicationMode) -> Self {
        self.mode = mode;
        self
    }

    /// Set the replica timeout
    #[must_use]
    pub fn with_replica_timeout(mut self, timeout: std::time::Duration) -> Self {
        self.replica_timeout = timeout;
        self
    }

    /// Set the maximum retry attempts
    #[must_use]
    pub fn with_max_retries(mut self, retries: u32) -> Self {
        self.max_retries = retries;
        self
    }

    /// Enable or disable reading from replicas
    #[must_use]
    pub fn with_read_from_replicas(mut self, enabled: bool) -> Self {
        self.read_from_replicas = enabled;
        self
    }
}

/// Cross-region replicated checkpoint storage
///
/// Replicates checkpoints across multiple backend checkpointers for disaster recovery
/// and high availability. Supports async, sync, and quorum-based replication modes.
///
/// # Example
///
/// ```rust,ignore
/// use dashflow::checkpoint::{ReplicatedCheckpointer, ReplicationMode, MemoryCheckpointer};
///
/// // Create checkpointers for different regions
/// let us_east = MemoryCheckpointer::new();
/// let us_west = MemoryCheckpointer::new();
/// let eu_west = MemoryCheckpointer::new();
///
/// // Create a replicated checkpointer with async replication
/// let replicated = ReplicatedCheckpointer::new(us_east)
///     .add_replica(us_west)
///     .add_replica(eu_west)
///     .with_mode(ReplicationMode::Async);
/// ```
pub struct ReplicatedCheckpointer<S: GraphState> {
    /// Primary checkpointer (writes go here first)
    primary: Arc<dyn Checkpointer<S>>,
    /// Replica checkpointers (for redundancy)
    replicas: Vec<Arc<dyn Checkpointer<S>>>,
    /// Configuration
    config: ReplicatedCheckpointerConfig,
}

impl<S: GraphState> ReplicatedCheckpointer<S> {
    /// Create a new replicated checkpointer with a primary backend
    ///
    /// # Arguments
    ///
    /// * `primary` - The primary checkpointer that receives all writes first
    pub fn new<C>(primary: C) -> Self
    where
        C: Checkpointer<S> + 'static,
    {
        Self {
            primary: Arc::new(primary),
            replicas: Vec::new(),
            config: ReplicatedCheckpointerConfig::default(),
        }
    }

    /// Add a replica checkpointer
    ///
    /// Replicas receive copies of all checkpoints for redundancy.
    /// The order replicas are added determines their priority for quorum voting.
    #[must_use]
    pub fn add_replica<C>(mut self, replica: C) -> Self
    where
        C: Checkpointer<S> + 'static,
    {
        self.replicas.push(Arc::new(replica));
        self
    }

    /// Set the replication mode
    #[must_use]
    pub fn with_mode(mut self, mode: ReplicationMode) -> Self {
        self.config.mode = mode;
        self
    }

    /// Set the replica timeout
    #[must_use]
    pub fn with_replica_timeout(mut self, timeout: std::time::Duration) -> Self {
        self.config.replica_timeout = timeout;
        self
    }

    /// Set the configuration
    #[must_use]
    pub fn with_config(mut self, config: ReplicatedCheckpointerConfig) -> Self {
        self.config = config;
        self
    }

    /// Get the number of replicas
    pub fn replica_count(&self) -> usize {
        self.replicas.len()
    }

    #[cfg(test)]
    pub(crate) fn config(&self) -> &ReplicatedCheckpointerConfig {
        &self.config
    }

    /// Calculate quorum size (majority of total nodes including primary)
    pub(crate) fn quorum_size(&self) -> usize {
        let total = 1 + self.replicas.len(); // primary + replicas
        (total / 2) + 1 // majority
    }

    /// Replicate a checkpoint to all replicas asynchronously
    async fn replicate_async(&self, checkpoint: &Checkpoint<S>) {
        for (idx, replica) in self.replicas.iter().enumerate() {
            let replica = replica.clone();
            let checkpoint = checkpoint.clone();
            let max_retries = self.config.max_retries;

            tokio::spawn(async move {
                let mut attempts = 0;
                loop {
                    match replica.save(checkpoint.clone()).await {
                        Ok(()) => {
                            tracing::debug!(
                                replica_idx = idx,
                                checkpoint_id = %checkpoint.id,
                                "Checkpoint replicated successfully"
                            );
                            break;
                        }
                        Err(e) => {
                            attempts += 1;
                            if attempts >= max_retries {
                                tracing::warn!(
                                    replica_idx = idx,
                                    checkpoint_id = %checkpoint.id,
                                    attempts = attempts,
                                    error = %e,
                                    "Failed to replicate checkpoint after max retries"
                                );
                                break;
                            }
                            tracing::debug!(
                                replica_idx = idx,
                                checkpoint_id = %checkpoint.id,
                                attempt = attempts,
                                error = %e,
                                "Replica write failed, retrying"
                            );
                            // Exponential backoff
                            tokio::time::sleep(std::time::Duration::from_millis(
                                100 * (1 << attempts.min(5)),
                            ))
                            .await;
                        }
                    }
                }
            });
        }
    }

    /// Replicate a checkpoint to all replicas synchronously
    async fn replicate_sync(&self, checkpoint: &Checkpoint<S>) -> Result<()> {
        let timeout = self.config.replica_timeout;

        for (idx, replica) in self.replicas.iter().enumerate() {
            let result = tokio::time::timeout(timeout, replica.save(checkpoint.clone())).await;

            match result {
                Ok(Ok(())) => {
                    tracing::debug!(
                        replica_idx = idx,
                        checkpoint_id = %checkpoint.id,
                        "Checkpoint replicated successfully"
                    );
                }
                Ok(Err(e)) => {
                    return Err(crate::Error::Checkpoint(
                        crate::error::CheckpointError::ConnectionLost {
                            backend: format!("replica {}", idx),
                            reason: e.to_string(),
                        },
                    ));
                }
                Err(_) => {
                    return Err(crate::Error::Checkpoint(
                        crate::error::CheckpointError::Timeout {
                            duration: self.config.replica_timeout,
                        },
                    ));
                }
            }
        }

        Ok(())
    }

    /// Replicate a checkpoint with quorum-based consistency
    async fn replicate_quorum(&self, checkpoint: &Checkpoint<S>) -> Result<()> {
        let quorum = self.quorum_size();
        let timeout = self.config.replica_timeout;

        // Primary already succeeded, so we start with 1
        let mut successes = 1usize;

        // Try all replicas concurrently
        let mut futures = Vec::with_capacity(self.replicas.len());
        for (idx, replica) in self.replicas.iter().enumerate() {
            let replica = replica.clone();
            let checkpoint = checkpoint.clone();
            futures.push(async move {
                let result = tokio::time::timeout(timeout, replica.save(checkpoint)).await;
                (idx, result)
            });
        }

        // Wait for all and count successes
        let results = futures::future::join_all(futures).await;
        for (idx, result) in results {
            match result {
                Ok(Ok(())) => {
                    successes += 1;
                    tracing::debug!(
                        replica_idx = idx,
                        checkpoint_id = %checkpoint.id,
                        "Checkpoint replicated successfully"
                    );
                }
                Ok(Err(e)) => {
                    tracing::warn!(
                        replica_idx = idx,
                        checkpoint_id = %checkpoint.id,
                        error = %e,
                        "Failed to replicate checkpoint"
                    );
                }
                Err(_) => {
                    tracing::warn!(
                        replica_idx = idx,
                        checkpoint_id = %checkpoint.id,
                        "Timeout replicating checkpoint"
                    );
                }
            }
        }

        if successes >= quorum {
            tracing::debug!(
                checkpoint_id = %checkpoint.id,
                successes = successes,
                quorum = quorum,
                "Quorum achieved for checkpoint"
            );
            Ok(())
        } else {
            Err(crate::Error::Checkpoint(
                crate::error::CheckpointError::QuorumNotAchieved {
                    successes,
                    total: 1 + self.replicas.len(),
                    required: quorum,
                },
            ))
        }
    }

    /// Try to load from replicas in order (for failover)
    async fn load_from_replicas(&self, checkpoint_id: &str) -> Result<Option<Checkpoint<S>>> {
        for (idx, replica) in self.replicas.iter().enumerate() {
            match replica.load(checkpoint_id).await {
                Ok(Some(checkpoint)) => {
                    tracing::debug!(
                        replica_idx = idx,
                        checkpoint_id = checkpoint_id,
                        "Loaded checkpoint from replica"
                    );
                    return Ok(Some(checkpoint));
                }
                Ok(None) => continue,
                Err(e) => {
                    tracing::debug!(
                        replica_idx = idx,
                        checkpoint_id = checkpoint_id,
                        error = %e,
                        "Failed to load from replica, trying next"
                    );
                    continue;
                }
            }
        }
        Ok(None)
    }

    /// Try to get latest from replicas (for failover)
    async fn get_latest_from_replicas(&self, thread_id: &str) -> Result<Option<Checkpoint<S>>> {
        let mut latest: Option<Checkpoint<S>> = None;

        for (idx, replica) in self.replicas.iter().enumerate() {
            match replica.get_latest(thread_id).await {
                Ok(Some(checkpoint)) => {
                    tracing::debug!(
                        replica_idx = idx,
                        thread_id = thread_id,
                        "Got latest checkpoint from replica"
                    );
                    match &latest {
                        None => latest = Some(checkpoint),
                        Some(current) => {
                            if checkpoint.timestamp > current.timestamp {
                                latest = Some(checkpoint);
                            }
                        }
                    }
                }
                Ok(None) => continue,
                Err(e) => {
                    tracing::debug!(
                        replica_idx = idx,
                        thread_id = thread_id,
                        error = %e,
                        "Failed to get latest from replica, trying next"
                    );
                    continue;
                }
            }
        }
        Ok(latest)
    }
}

impl<S: GraphState> Clone for ReplicatedCheckpointer<S> {
    fn clone(&self) -> Self {
        Self {
            primary: self.primary.clone(),
            replicas: self.replicas.clone(),
            config: self.config.clone(),
        }
    }
}

#[async_trait::async_trait]
impl<S: GraphState> Checkpointer<S> for ReplicatedCheckpointer<S> {
    async fn save(&self, checkpoint: Checkpoint<S>) -> Result<()> {
        // Always write to primary first
        self.primary.save(checkpoint.clone()).await?;

        // Then replicate according to mode
        match self.config.mode {
            ReplicationMode::Async => {
                self.replicate_async(&checkpoint).await;
                Ok(())
            }
            ReplicationMode::Sync => self.replicate_sync(&checkpoint).await,
            ReplicationMode::Quorum => self.replicate_quorum(&checkpoint).await,
        }
    }

    async fn load(&self, checkpoint_id: &str) -> Result<Option<Checkpoint<S>>> {
        // Try primary first
        match self.primary.load(checkpoint_id).await {
            Ok(Some(checkpoint)) => return Ok(Some(checkpoint)),
            Ok(None) => {
                // Not found in primary, try replicas if configured
                if self.config.read_from_replicas {
                    return self.load_from_replicas(checkpoint_id).await;
                }
                return Ok(None);
            }
            Err(e) => {
                // Primary failed, try replicas if configured
                if self.config.read_from_replicas {
                    tracing::debug!(
                        checkpoint_id = checkpoint_id,
                        error = %e,
                        "Primary load failed, trying replicas"
                    );
                    return self.load_from_replicas(checkpoint_id).await;
                }
                return Err(e);
            }
        }
    }

    async fn get_latest(&self, thread_id: &str) -> Result<Option<Checkpoint<S>>> {
        // Try primary first
        match self.primary.get_latest(thread_id).await {
            Ok(Some(checkpoint)) => return Ok(Some(checkpoint)),
            Ok(None) => {
                // Not found in primary, try replicas if configured
                if self.config.read_from_replicas {
                    return self.get_latest_from_replicas(thread_id).await;
                }
                return Ok(None);
            }
            Err(e) => {
                // Primary failed, try replicas if configured
                if self.config.read_from_replicas {
                    tracing::debug!(
                        thread_id = thread_id,
                        error = %e,
                        "Primary get_latest failed, trying replicas"
                    );
                    return self.get_latest_from_replicas(thread_id).await;
                }
                return Err(e);
            }
        }
    }

    async fn list(&self, thread_id: &str) -> Result<Vec<CheckpointMetadata>> {
        // For list, we only query primary (listing from multiple sources would require deduplication)
        self.primary.list(thread_id).await
    }

    async fn delete(&self, checkpoint_id: &str) -> Result<()> {
        // Delete from primary first
        self.primary.delete(checkpoint_id).await?;

        // Then delete from replicas (best effort for async mode)
        match self.config.mode {
            ReplicationMode::Async => {
                for replica in &self.replicas {
                    let replica = replica.clone();
                    let checkpoint_id = checkpoint_id.to_string();
                    tokio::spawn(async move {
                        if let Err(e) = replica.delete(&checkpoint_id).await {
                            tracing::warn!(
                                checkpoint_id = checkpoint_id,
                                error = %e,
                                "Failed to delete checkpoint from replica"
                            );
                        }
                    });
                }
            }
            ReplicationMode::Sync | ReplicationMode::Quorum => {
                // For sync/quorum modes, wait for all deletes
                for (idx, replica) in self.replicas.iter().enumerate() {
                    if let Err(e) = replica.delete(checkpoint_id).await {
                        tracing::warn!(
                            replica_idx = idx,
                            checkpoint_id = checkpoint_id,
                            error = %e,
                            "Failed to delete checkpoint from replica"
                        );
                    }
                }
            }
        }

        Ok(())
    }

    async fn delete_thread(&self, thread_id: &str) -> Result<()> {
        // Delete from primary first
        self.primary.delete_thread(thread_id).await?;

        // Then delete from replicas (best effort for async mode)
        match self.config.mode {
            ReplicationMode::Async => {
                for replica in &self.replicas {
                    let replica = replica.clone();
                    let thread_id = thread_id.to_string();
                    tokio::spawn(async move {
                        if let Err(e) = replica.delete_thread(&thread_id).await {
                            tracing::warn!(
                                thread_id = thread_id,
                                error = %e,
                                "Failed to delete thread checkpoints from replica"
                            );
                        }
                    });
                }
            }
            ReplicationMode::Sync | ReplicationMode::Quorum => {
                // For sync/quorum modes, wait for all deletes
                for (idx, replica) in self.replicas.iter().enumerate() {
                    if let Err(e) = replica.delete_thread(thread_id).await {
                        tracing::warn!(
                            replica_idx = idx,
                            thread_id = thread_id,
                            error = %e,
                            "Failed to delete thread checkpoints from replica"
                        );
                    }
                }
            }
        }

        Ok(())
    }

    async fn list_threads(&self) -> Result<Vec<ThreadInfo>> {
        // For list_threads, we only query primary (listing from multiple sources would require deduplication)
        self.primary.list_threads().await
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

    // ===========================================
    // ReplicationMode tests
    // ===========================================

    #[test]
    fn test_replication_mode_default() {
        let mode = ReplicationMode::default();
        assert_eq!(mode, ReplicationMode::Async);
    }

    #[test]
    #[allow(clippy::clone_on_copy)]
    fn test_replication_mode_clone() {
        let mode = ReplicationMode::Sync;
        let cloned = mode.clone();
        assert_eq!(mode, cloned);
    }

    #[test]
    fn test_replication_mode_debug() {
        let mode = ReplicationMode::Quorum;
        let debug_str = format!("{:?}", mode);
        assert!(debug_str.contains("Quorum"));
    }

    #[test]
    fn test_replication_mode_copy() {
        let mode = ReplicationMode::Async;
        let copied: ReplicationMode = mode; // Copy
        assert_eq!(mode, copied);
    }

    #[test]
    fn test_replication_mode_eq() {
        assert_eq!(ReplicationMode::Async, ReplicationMode::Async);
        assert_eq!(ReplicationMode::Sync, ReplicationMode::Sync);
        assert_eq!(ReplicationMode::Quorum, ReplicationMode::Quorum);
        assert_ne!(ReplicationMode::Async, ReplicationMode::Sync);
        assert_ne!(ReplicationMode::Sync, ReplicationMode::Quorum);
    }

    // ===========================================
    // ReplicatedCheckpointerConfig tests
    // ===========================================

    #[test]
    fn test_config_default() {
        let config = ReplicatedCheckpointerConfig::default();
        assert_eq!(config.mode, ReplicationMode::Async);
        assert_eq!(config.replica_timeout, std::time::Duration::from_secs(5));
        assert_eq!(config.max_retries, 3);
        assert!(config.read_from_replicas);
    }

    #[test]
    fn test_config_new() {
        let config = ReplicatedCheckpointerConfig::new();
        assert_eq!(config.mode, ReplicationMode::Async);
    }

    #[test]
    fn test_config_with_mode() {
        let config = ReplicatedCheckpointerConfig::new().with_mode(ReplicationMode::Sync);
        assert_eq!(config.mode, ReplicationMode::Sync);
    }

    #[test]
    fn test_config_with_replica_timeout() {
        let timeout = std::time::Duration::from_secs(10);
        let config = ReplicatedCheckpointerConfig::new().with_replica_timeout(timeout);
        assert_eq!(config.replica_timeout, timeout);
    }

    #[test]
    fn test_config_with_max_retries() {
        let config = ReplicatedCheckpointerConfig::new().with_max_retries(5);
        assert_eq!(config.max_retries, 5);
    }

    #[test]
    fn test_config_with_read_from_replicas() {
        let config = ReplicatedCheckpointerConfig::new().with_read_from_replicas(false);
        assert!(!config.read_from_replicas);
    }

    #[test]
    fn test_config_builder_chain() {
        let config = ReplicatedCheckpointerConfig::new()
            .with_mode(ReplicationMode::Quorum)
            .with_replica_timeout(std::time::Duration::from_secs(15))
            .with_max_retries(10)
            .with_read_from_replicas(false);

        assert_eq!(config.mode, ReplicationMode::Quorum);
        assert_eq!(config.replica_timeout, std::time::Duration::from_secs(15));
        assert_eq!(config.max_retries, 10);
        assert!(!config.read_from_replicas);
    }

    #[test]
    fn test_config_clone() {
        let config = ReplicatedCheckpointerConfig::new().with_mode(ReplicationMode::Sync);
        let cloned = config.clone();
        assert_eq!(config.mode, cloned.mode);
        assert_eq!(config.replica_timeout, cloned.replica_timeout);
    }

    #[test]
    fn test_config_debug() {
        let config = ReplicatedCheckpointerConfig::new();
        let debug_str = format!("{:?}", config);
        assert!(debug_str.contains("ReplicatedCheckpointerConfig"));
        assert!(debug_str.contains("Async"));
    }

    // ===========================================
    // ReplicatedCheckpointer construction tests
    // ===========================================

    #[test]
    fn test_checkpointer_new() {
        let primary: MemoryCheckpointer<TestState> = MemoryCheckpointer::new();
        let checkpointer = ReplicatedCheckpointer::new(primary);
        assert_eq!(checkpointer.replica_count(), 0);
    }

    #[test]
    fn test_checkpointer_add_replica() {
        let primary: MemoryCheckpointer<TestState> = MemoryCheckpointer::new();
        let replica1 = MemoryCheckpointer::new();
        let replica2 = MemoryCheckpointer::new();

        let checkpointer = ReplicatedCheckpointer::new(primary)
            .add_replica(replica1)
            .add_replica(replica2);

        assert_eq!(checkpointer.replica_count(), 2);
    }

    #[test]
    fn test_checkpointer_with_mode() {
        let primary: MemoryCheckpointer<TestState> = MemoryCheckpointer::new();
        let checkpointer = ReplicatedCheckpointer::new(primary).with_mode(ReplicationMode::Sync);
        assert_eq!(checkpointer.config().mode, ReplicationMode::Sync);
    }

    #[test]
    fn test_checkpointer_with_replica_timeout() {
        let primary: MemoryCheckpointer<TestState> = MemoryCheckpointer::new();
        let timeout = std::time::Duration::from_secs(20);
        let checkpointer = ReplicatedCheckpointer::new(primary).with_replica_timeout(timeout);
        assert_eq!(checkpointer.config().replica_timeout, timeout);
    }

    #[test]
    fn test_checkpointer_with_config() {
        let primary: MemoryCheckpointer<TestState> = MemoryCheckpointer::new();
        let config = ReplicatedCheckpointerConfig::new()
            .with_mode(ReplicationMode::Quorum)
            .with_max_retries(7);

        let checkpointer = ReplicatedCheckpointer::new(primary).with_config(config);
        assert_eq!(checkpointer.config().mode, ReplicationMode::Quorum);
        assert_eq!(checkpointer.config().max_retries, 7);
    }

    // ===========================================
    // Quorum size calculation tests
    // ===========================================

    #[test]
    fn test_quorum_size_no_replicas() {
        let primary: MemoryCheckpointer<TestState> = MemoryCheckpointer::new();
        let checkpointer = ReplicatedCheckpointer::new(primary);
        // 1 node: quorum = (1 / 2) + 1 = 1
        assert_eq!(checkpointer.quorum_size(), 1);
    }

    #[test]
    fn test_quorum_size_one_replica() {
        let primary: MemoryCheckpointer<TestState> = MemoryCheckpointer::new();
        let checkpointer = ReplicatedCheckpointer::new(primary).add_replica(MemoryCheckpointer::new());
        // 2 nodes: quorum = (2 / 2) + 1 = 2
        assert_eq!(checkpointer.quorum_size(), 2);
    }

    #[test]
    fn test_quorum_size_two_replicas() {
        let primary: MemoryCheckpointer<TestState> = MemoryCheckpointer::new();
        let checkpointer = ReplicatedCheckpointer::new(primary)
            .add_replica(MemoryCheckpointer::new())
            .add_replica(MemoryCheckpointer::new());
        // 3 nodes: quorum = (3 / 2) + 1 = 2
        assert_eq!(checkpointer.quorum_size(), 2);
    }

    #[test]
    fn test_quorum_size_three_replicas() {
        let primary: MemoryCheckpointer<TestState> = MemoryCheckpointer::new();
        let checkpointer = ReplicatedCheckpointer::new(primary)
            .add_replica(MemoryCheckpointer::new())
            .add_replica(MemoryCheckpointer::new())
            .add_replica(MemoryCheckpointer::new());
        // 4 nodes: quorum = (4 / 2) + 1 = 3
        assert_eq!(checkpointer.quorum_size(), 3);
    }

    #[test]
    fn test_quorum_size_four_replicas() {
        let primary: MemoryCheckpointer<TestState> = MemoryCheckpointer::new();
        let checkpointer = ReplicatedCheckpointer::new(primary)
            .add_replica(MemoryCheckpointer::new())
            .add_replica(MemoryCheckpointer::new())
            .add_replica(MemoryCheckpointer::new())
            .add_replica(MemoryCheckpointer::new());
        // 5 nodes: quorum = (5 / 2) + 1 = 3
        assert_eq!(checkpointer.quorum_size(), 3);
    }

    // ===========================================
    // Async replication tests
    // ===========================================

    #[tokio::test]
    async fn test_async_replication_basic() {
        let primary: MemoryCheckpointer<TestState> = MemoryCheckpointer::new();
        let replica = MemoryCheckpointer::new();

        let checkpointer = ReplicatedCheckpointer::new(primary.clone())
            .add_replica(replica.clone())
            .with_mode(ReplicationMode::Async);

        let checkpoint = Checkpoint::new(
            "thread-1".to_string(),
            TestState { value: 42 },
            "node-1".to_string(),
            None,
        );

        // Save should succeed immediately (async replication)
        checkpointer.save(checkpoint.clone()).await.unwrap();

        // Primary should have the checkpoint
        let loaded = primary.load(&checkpoint.id).await.unwrap();
        assert!(loaded.is_some());

        // Give async replication time to complete
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;

        // Replica should eventually have the checkpoint
        let replica_loaded = replica.load(&checkpoint.id).await.unwrap();
        assert!(replica_loaded.is_some());
    }

    #[tokio::test]
    async fn test_async_replication_primary_failure_doesnt_affect_save() {
        // Async mode should complete save after primary succeeds, even if replica fails
        let primary: MemoryCheckpointer<TestState> = MemoryCheckpointer::new();
        let replica = MemoryCheckpointer::new();

        let checkpointer = ReplicatedCheckpointer::new(primary.clone())
            .add_replica(replica)
            .with_mode(ReplicationMode::Async);

        let checkpoint = Checkpoint::new(
            "thread-1".to_string(),
            TestState { value: 42 },
            "node-1".to_string(),
            None,
        );

        // Save should succeed (primary succeeds, replica failures are logged but don't fail)
        checkpointer.save(checkpoint.clone()).await.unwrap();

        // Primary should have the checkpoint
        let loaded = primary.load(&checkpoint.id).await.unwrap();
        assert!(loaded.is_some());
    }

    // ===========================================
    // Sync replication tests
    // ===========================================

    #[tokio::test]
    async fn test_sync_replication_all_succeed() {
        let primary: MemoryCheckpointer<TestState> = MemoryCheckpointer::new();
        let replica1 = MemoryCheckpointer::new();
        let replica2 = MemoryCheckpointer::new();

        let checkpointer = ReplicatedCheckpointer::new(primary.clone())
            .add_replica(replica1.clone())
            .add_replica(replica2.clone())
            .with_mode(ReplicationMode::Sync);

        let checkpoint = Checkpoint::new(
            "thread-1".to_string(),
            TestState { value: 42 },
            "node-1".to_string(),
            None,
        );

        // Save should succeed
        checkpointer.save(checkpoint.clone()).await.unwrap();

        // All should have the checkpoint
        assert!(primary.load(&checkpoint.id).await.unwrap().is_some());
        assert!(replica1.load(&checkpoint.id).await.unwrap().is_some());
        assert!(replica2.load(&checkpoint.id).await.unwrap().is_some());
    }

    // ===========================================
    // Quorum replication tests
    // ===========================================

    #[tokio::test]
    async fn test_quorum_replication_all_succeed() {
        let primary: MemoryCheckpointer<TestState> = MemoryCheckpointer::new();
        let replica1 = MemoryCheckpointer::new();
        let replica2 = MemoryCheckpointer::new();

        let checkpointer = ReplicatedCheckpointer::new(primary.clone())
            .add_replica(replica1.clone())
            .add_replica(replica2.clone())
            .with_mode(ReplicationMode::Quorum);

        let checkpoint = Checkpoint::new(
            "thread-1".to_string(),
            TestState { value: 42 },
            "node-1".to_string(),
            None,
        );

        // Save should succeed (all replicas succeed, quorum easily achieved)
        checkpointer.save(checkpoint.clone()).await.unwrap();

        // All should have the checkpoint
        assert!(primary.load(&checkpoint.id).await.unwrap().is_some());
        assert!(replica1.load(&checkpoint.id).await.unwrap().is_some());
        assert!(replica2.load(&checkpoint.id).await.unwrap().is_some());
    }

    // ===========================================
    // Read failover tests
    // ===========================================

    #[tokio::test]
    async fn test_load_from_primary() {
        let primary: MemoryCheckpointer<TestState> = MemoryCheckpointer::new();
        let replica = MemoryCheckpointer::new();

        let checkpointer = ReplicatedCheckpointer::new(primary.clone())
            .add_replica(replica)
            .with_mode(ReplicationMode::Async);

        let checkpoint = Checkpoint::new(
            "thread-1".to_string(),
            TestState { value: 42 },
            "node-1".to_string(),
            None,
        );

        // Save directly to primary
        primary.save(checkpoint.clone()).await.unwrap();

        // Load should succeed from primary
        let loaded = checkpointer.load(&checkpoint.id).await.unwrap();
        assert!(loaded.is_some());
        assert_eq!(loaded.unwrap().state.value, 42);
    }

    #[tokio::test]
    async fn test_load_not_found_returns_none() {
        let primary: MemoryCheckpointer<TestState> = MemoryCheckpointer::new();
        let replica = MemoryCheckpointer::new();

        let checkpointer = ReplicatedCheckpointer::new(primary)
            .add_replica(replica)
            .with_mode(ReplicationMode::Async);

        // Load non-existent checkpoint
        let loaded = checkpointer.load("non-existent-id").await.unwrap();
        assert!(loaded.is_none());
    }

    #[tokio::test]
    async fn test_get_latest_from_primary() {
        let primary: MemoryCheckpointer<TestState> = MemoryCheckpointer::new();

        let checkpointer = ReplicatedCheckpointer::new(primary.clone())
            .with_mode(ReplicationMode::Async);

        let thread_id = "thread-latest";

        // Save multiple checkpoints
        for i in 0..3 {
            let checkpoint = Checkpoint::new(
                thread_id.to_string(),
                TestState { value: i },
                format!("node-{}", i),
                None,
            );
            primary.save(checkpoint).await.unwrap();
        }

        // Get latest should return the most recent
        let latest = checkpointer.get_latest(thread_id).await.unwrap();
        assert!(latest.is_some());
    }

    // ===========================================
    // Clone tests
    // ===========================================

    #[test]
    fn test_checkpointer_clone() {
        let primary: MemoryCheckpointer<TestState> = MemoryCheckpointer::new();
        let checkpointer = ReplicatedCheckpointer::new(primary)
            .add_replica(MemoryCheckpointer::new())
            .with_mode(ReplicationMode::Sync);

        let cloned = checkpointer.clone();

        assert_eq!(cloned.replica_count(), 1);
        assert_eq!(cloned.config().mode, ReplicationMode::Sync);
    }

    // ===========================================
    // List and delete tests
    // ===========================================

    #[tokio::test]
    async fn test_list_checkpoints() {
        let primary: MemoryCheckpointer<TestState> = MemoryCheckpointer::new();
        let checkpointer = ReplicatedCheckpointer::new(primary).with_mode(ReplicationMode::Async);

        let thread_id = "thread-list";

        // Save multiple checkpoints
        for i in 0..3 {
            let checkpoint = Checkpoint::new(
                thread_id.to_string(),
                TestState { value: i },
                format!("node-{}", i),
                None,
            );
            checkpointer.save(checkpoint).await.unwrap();
        }

        // List should return all
        let list = checkpointer.list(thread_id).await.unwrap();
        assert_eq!(list.len(), 3);
    }

    #[tokio::test]
    async fn test_delete_checkpoint() {
        let primary: MemoryCheckpointer<TestState> = MemoryCheckpointer::new();
        let replica = MemoryCheckpointer::new();

        let checkpointer = ReplicatedCheckpointer::new(primary.clone())
            .add_replica(replica.clone())
            .with_mode(ReplicationMode::Async);

        let checkpoint = Checkpoint::new(
            "thread-delete".to_string(),
            TestState { value: 42 },
            "node-1".to_string(),
            None,
        );

        // Save checkpoint
        checkpointer.save(checkpoint.clone()).await.unwrap();

        // Give async replication time
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;

        // Delete checkpoint
        checkpointer.delete(&checkpoint.id).await.unwrap();

        // Primary should not have the checkpoint
        let loaded = primary.load(&checkpoint.id).await.unwrap();
        assert!(loaded.is_none());
    }

    #[tokio::test]
    async fn test_delete_thread() {
        let primary: MemoryCheckpointer<TestState> = MemoryCheckpointer::new();
        let checkpointer = ReplicatedCheckpointer::new(primary).with_mode(ReplicationMode::Async);

        let thread_id = "thread-delete-all";

        // Save multiple checkpoints
        for i in 0..3 {
            let checkpoint = Checkpoint::new(
                thread_id.to_string(),
                TestState { value: i },
                format!("node-{}", i),
                None,
            );
            checkpointer.save(checkpoint).await.unwrap();
        }

        // Verify checkpoints exist
        let list = checkpointer.list(thread_id).await.unwrap();
        assert_eq!(list.len(), 3);

        // Delete thread
        checkpointer.delete_thread(thread_id).await.unwrap();

        // Should be empty
        let list = checkpointer.list(thread_id).await.unwrap();
        assert_eq!(list.len(), 0);
    }

    #[tokio::test]
    async fn test_list_threads() {
        let primary: MemoryCheckpointer<TestState> = MemoryCheckpointer::new();
        let checkpointer = ReplicatedCheckpointer::new(primary).with_mode(ReplicationMode::Async);

        // Save checkpoints to different threads
        for i in 0..3 {
            let checkpoint = Checkpoint::new(
                format!("thread-{}", i),
                TestState { value: i },
                "node-1".to_string(),
                None,
            );
            checkpointer.save(checkpoint).await.unwrap();
        }

        // List threads
        let threads = checkpointer.list_threads().await.unwrap();
        assert_eq!(threads.len(), 3);
    }

    // ===========================================
    // Sync delete tests
    // ===========================================

    #[tokio::test]
    async fn test_sync_delete_checkpoint() {
        let primary: MemoryCheckpointer<TestState> = MemoryCheckpointer::new();
        let replica = MemoryCheckpointer::new();

        let checkpointer = ReplicatedCheckpointer::new(primary.clone())
            .add_replica(replica.clone())
            .with_mode(ReplicationMode::Sync);

        let checkpoint = Checkpoint::new(
            "thread-sync-delete".to_string(),
            TestState { value: 42 },
            "node-1".to_string(),
            None,
        );

        // Save checkpoint
        checkpointer.save(checkpoint.clone()).await.unwrap();

        // Verify both have checkpoint
        assert!(primary.load(&checkpoint.id).await.unwrap().is_some());
        assert!(replica.load(&checkpoint.id).await.unwrap().is_some());

        // Delete checkpoint (sync mode)
        checkpointer.delete(&checkpoint.id).await.unwrap();

        // Both should not have the checkpoint
        assert!(primary.load(&checkpoint.id).await.unwrap().is_none());
        assert!(replica.load(&checkpoint.id).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn test_sync_delete_thread() {
        let primary: MemoryCheckpointer<TestState> = MemoryCheckpointer::new();
        let replica = MemoryCheckpointer::new();

        let checkpointer = ReplicatedCheckpointer::new(primary.clone())
            .add_replica(replica.clone())
            .with_mode(ReplicationMode::Sync);

        let thread_id = "thread-sync-delete-all";

        // Save checkpoints
        let checkpoint = Checkpoint::new(
            thread_id.to_string(),
            TestState { value: 42 },
            "node-1".to_string(),
            None,
        );
        checkpointer.save(checkpoint).await.unwrap();

        // Delete thread (sync mode)
        checkpointer.delete_thread(thread_id).await.unwrap();

        // Both should be empty
        assert_eq!(primary.list(thread_id).await.unwrap().len(), 0);
        assert_eq!(replica.list(thread_id).await.unwrap().len(), 0);
    }
}
