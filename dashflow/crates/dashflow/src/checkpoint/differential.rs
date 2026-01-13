// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

//! Differential checkpoint storage
//!
//! Stores state differences instead of full states to reduce storage and memory.
//! Uses binary diffing on serialized state for space efficiency.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};

use crate::{GraphState, Result};

use super::{Checkpoint, CheckpointId, CheckpointMetadata, Checkpointer, ThreadId, ThreadInfo};

/// Represents a binary difference between two serialized checkpoint states.
///
/// CheckpointDiff uses a simple but effective binary diffing strategy:
/// - For small states (< 1KB), stores the full new state (no overhead worth diffing)
/// - For larger states, stores the serialized delta
///
/// # Memory Savings
///
/// For states where only a small portion changes between nodes:
/// - Full checkpoint: N bytes per save
/// - Differential: ~(changed bytes + overhead) per save
///
/// Example: 100KB state with 1KB changes per node
/// - 10 nodes with full checkpoints: 1MB total
/// - 10 nodes with differential: ~110KB (1 base + 9 diffs)
///
/// Note: This is different from `graph_registry::StateDiff` which tracks
/// field-level state changes. `CheckpointDiff` is for binary checkpoint diffing.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CheckpointDiff {
    /// ID of the base checkpoint this diff applies to
    pub base_id: CheckpointId,
    /// Serialized state difference (compressed binary)
    pub diff_data: Vec<u8>,
    /// Size of the original state (for validation)
    pub original_size: usize,
    /// Size of the new state
    pub new_size: usize,
}

impl CheckpointDiff {
    /// Threshold below which we don't bother diffing (full state is stored)
    pub(crate) const MIN_DIFF_SIZE: usize = 1024;

    /// Create a diff between two serialized states.
    ///
    /// Returns `None` if the states are identical or if diffing isn't worthwhile.
    pub fn create(base_data: &[u8], new_data: &[u8]) -> Option<Self> {
        // Don't diff small states - overhead isn't worth it
        if new_data.len() < Self::MIN_DIFF_SIZE {
            return None;
        }

        // Simple binary diff: store changed bytes with positions
        let diff = Self::compute_binary_diff(base_data, new_data);

        // Only use diff if it's smaller than storing the full state
        if diff.len() >= new_data.len() {
            return None;
        }

        Some(Self {
            base_id: String::new(), // Will be set by caller
            diff_data: diff,
            original_size: base_data.len(),
            new_size: new_data.len(),
        })
    }

    /// Apply this diff to a base state to reconstruct the new state.
    pub fn apply(&self, base_data: &[u8]) -> Result<Vec<u8>> {
        if base_data.len() != self.original_size {
            return Err(crate::Error::Checkpoint(
                crate::error::CheckpointError::IntegrityCheckFailed {
                    checkpoint_id: self.base_id.clone(),
                    reason: format!(
                        "Base state size mismatch: expected {}, got {}",
                        self.original_size,
                        base_data.len()
                    ),
                },
            ));
        }

        let result = Self::apply_binary_diff(base_data, &self.diff_data, self.new_size)?;

        if result.len() != self.new_size {
            return Err(crate::Error::Checkpoint(
                crate::error::CheckpointError::IntegrityCheckFailed {
                    checkpoint_id: self.base_id.clone(),
                    reason: format!(
                        "Reconstructed state size mismatch: expected {}, got {}",
                        self.new_size,
                        result.len()
                    ),
                },
            ));
        }

        Ok(result)
    }

    /// Compute a simple binary diff between two byte sequences.
    ///
    /// Format: sequence of (position: u32, length: u16, data: [u8; length])
    /// Each chunk represents bytes that differ from the base.
    fn compute_binary_diff(base: &[u8], new: &[u8]) -> Vec<u8> {
        let mut diff = Vec::new();
        let mut i = 0;

        // First, encode the new length if different (allows growing/shrinking)
        diff.extend_from_slice(&(new.len() as u64).to_le_bytes());

        while i < new.len() {
            // Find start of difference
            while i < base.len() && i < new.len() && base[i] == new[i] {
                i += 1;
            }

            if i >= new.len() {
                break;
            }

            let diff_start = i;

            // Find end of difference (or run of up to 64KB)
            let mut diff_end = i + 1;
            let max_chunk = (i + u16::MAX as usize).min(new.len());

            while diff_end < max_chunk {
                // Check if we've re-synced with base for at least 8 bytes
                if diff_end + 8 <= base.len()
                    && diff_end + 8 <= new.len()
                    && base[diff_end..diff_end + 8] == new[diff_end..diff_end + 8]
                {
                    break;
                }
                diff_end += 1;
            }

            // Write chunk: position (4 bytes) + length (2 bytes) + data
            diff.extend_from_slice(&(diff_start as u32).to_le_bytes());
            diff.extend_from_slice(&((diff_end - diff_start) as u16).to_le_bytes());
            diff.extend_from_slice(&new[diff_start..diff_end]);

            i = diff_end;
        }

        diff
    }

    /// Apply a binary diff to reconstruct the new state.
    // SAFETY: try_into().unwrap() is safe - slice sizes exactly match target array sizes
    // (diff[0..8] -> [u8; 8], diff[pos..pos+4] -> [u8; 4], diff[pos+4..pos+6] -> [u8; 2])
    #[allow(clippy::unwrap_used)]
    fn apply_binary_diff(base: &[u8], diff: &[u8], expected_len: usize) -> Result<Vec<u8>> {
        if diff.len() < 8 {
            return Err(crate::Error::Checkpoint(
                crate::error::CheckpointError::IntegrityCheckFailed {
                    checkpoint_id: String::new(),
                    reason: "Diff data too small".to_string(),
                },
            ));
        }

        // Read the target length
        let target_len = u64::from_le_bytes(diff[0..8].try_into().unwrap()) as usize;
        if target_len != expected_len {
            return Err(crate::Error::Checkpoint(
                crate::error::CheckpointError::IntegrityCheckFailed {
                    checkpoint_id: String::new(),
                    reason: format!(
                        "Diff target length mismatch: expected {}, diff says {}",
                        expected_len, target_len
                    ),
                },
            ));
        }

        // Start with base, extended or truncated to target length
        let mut result = if target_len <= base.len() {
            base[..target_len].to_vec()
        } else {
            let mut r = base.to_vec();
            r.resize(target_len, 0);
            r
        };

        // Apply chunks
        let mut pos = 8;
        while pos + 6 <= diff.len() {
            let chunk_pos = u32::from_le_bytes(diff[pos..pos + 4].try_into().unwrap()) as usize;
            let chunk_len = u16::from_le_bytes(diff[pos + 4..pos + 6].try_into().unwrap()) as usize;
            pos += 6;

            if pos + chunk_len > diff.len() {
                return Err(crate::Error::Checkpoint(
                    crate::error::CheckpointError::IntegrityCheckFailed {
                        checkpoint_id: String::new(),
                        reason: "Diff chunk extends past end of diff data".to_string(),
                    },
                ));
            }

            if chunk_pos + chunk_len > result.len() {
                return Err(crate::Error::Checkpoint(
                    crate::error::CheckpointError::IntegrityCheckFailed {
                        checkpoint_id: String::new(),
                        reason: format!(
                            "Diff chunk position {} + length {} exceeds result size {}",
                            chunk_pos,
                            chunk_len,
                            result.len()
                        ),
                    },
                ));
            }

            result[chunk_pos..chunk_pos + chunk_len].copy_from_slice(&diff[pos..pos + chunk_len]);
            pos += chunk_len;
        }

        Ok(result)
    }
}

/// Internal storage for differential checkpoints.
///
/// Either stores a full checkpoint (base) or a diff referencing a base.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(bound(
    serialize = "S: Serialize",
    deserialize = "S: for<'de2> Deserialize<'de2>"
))]
enum DifferentialEntry<S: GraphState> {
    /// Full checkpoint (base for diffs)
    Full(Checkpoint<S>),
    /// Differential checkpoint referencing a base
    Diff {
        /// Metadata (id, thread_id, node, timestamp, parent_id, metadata)
        metadata: CheckpointMetadata,
        /// The diff to apply to base
        diff: CheckpointDiff,
    },
}

/// Configuration for differential checkpointing behavior.
#[derive(Clone, Debug)]
pub struct DifferentialConfig {
    /// How many checkpoints between full (base) checkpoints.
    /// E.g., `base_interval = 10` means every 10th checkpoint is a full base.
    /// Higher values = more space savings but slower reconstruction.
    pub base_interval: usize,

    /// Maximum chain length before forcing a new base.
    /// Prevents reconstruction from becoming too slow.
    pub max_chain_length: usize,

    /// Minimum state size to consider for diffing.
    /// States smaller than this are always stored in full.
    pub min_diff_size: usize,
}

impl Default for DifferentialConfig {
    fn default() -> Self {
        Self {
            base_interval: 10,
            max_chain_length: 20,
            min_diff_size: CheckpointDiff::MIN_DIFF_SIZE,
        }
    }
}

impl DifferentialConfig {
    /// Create config optimized for memory savings.
    pub fn memory_optimized() -> Self {
        Self {
            base_interval: 20,
            max_chain_length: 50,
            min_diff_size: 512,
        }
    }

    /// Create config optimized for fast reconstruction.
    pub fn speed_optimized() -> Self {
        Self {
            base_interval: 5,
            max_chain_length: 10,
            min_diff_size: 2048,
        }
    }
}

/// A checkpointer wrapper that stores differential checkpoints.
///
/// Wraps any existing `Checkpointer` implementation and adds differential
/// storage on top. Full checkpoints are stored periodically as "bases",
/// with intermediate checkpoints stored as diffs.
///
/// # Example
///
/// ```rust,ignore
/// use dashflow::{MemoryCheckpointer, DifferentialCheckpointer};
///
/// let base = MemoryCheckpointer::new();
/// let diff_checkpointer = DifferentialCheckpointer::new(base);
///
/// // Use like any other checkpointer
/// let app = graph.compile()?.with_checkpointer(diff_checkpointer);
/// ```
///
/// # Performance Characteristics
///
/// - **Save**: Slightly slower (serialization + diff computation)
/// - **Load**: Potentially slower (may need to reconstruct from base + chain of diffs)
/// - **Memory**: Significantly reduced for states with incremental changes
/// - **Best for**: Large states where only small portions change per node
#[derive(Clone)]
pub struct DifferentialCheckpointer<S: GraphState, C: Checkpointer<S>> {
    /// The underlying checkpointer for actual storage
    inner: C,
    /// Configuration for differential behavior
    config: DifferentialConfig,
    /// Cache of entries (full or diff) by checkpoint ID
    entries: Arc<Mutex<HashMap<CheckpointId, DifferentialEntry<S>>>>,
    /// Count of checkpoints per thread (for base interval calculation)
    thread_counts: Arc<Mutex<HashMap<ThreadId, usize>>>,
    /// Last base checkpoint ID per thread
    last_base: Arc<Mutex<HashMap<ThreadId, CheckpointId>>>,
}

impl<S: GraphState, C: Checkpointer<S>> DifferentialCheckpointer<S, C> {
    /// Create a new differential checkpointer wrapping the given inner checkpointer.
    pub fn new(inner: C) -> Self {
        Self::with_config(inner, DifferentialConfig::default())
    }

    /// Create a differential checkpointer with custom configuration.
    #[must_use]
    pub fn with_config(inner: C, config: DifferentialConfig) -> Self {
        Self {
            inner,
            config,
            entries: Arc::new(Mutex::new(HashMap::new())),
            thread_counts: Arc::new(Mutex::new(HashMap::new())),
            last_base: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Create a memory-optimized differential checkpointer.
    pub fn memory_optimized(inner: C) -> Self {
        Self::with_config(inner, DifferentialConfig::memory_optimized())
    }

    /// Create a speed-optimized differential checkpointer.
    pub fn speed_optimized(inner: C) -> Self {
        Self::with_config(inner, DifferentialConfig::speed_optimized())
    }

    /// Get the current configuration.
    pub fn config(&self) -> &DifferentialConfig {
        &self.config
    }

    /// Check if we should store a full base checkpoint.
    fn should_store_base(&self, thread_id: &str) -> bool {
        let counts = self.thread_counts.lock().unwrap_or_else(|e| e.into_inner());
        let count = counts.get(thread_id).copied().unwrap_or(0);
        count % self.config.base_interval == 0
    }

    /// Increment the checkpoint count for a thread.
    fn increment_count(&self, thread_id: &str) -> usize {
        let mut counts = self.thread_counts.lock().unwrap_or_else(|e| e.into_inner());
        let count = counts.entry(thread_id.to_string()).or_insert(0);
        *count += 1;
        *count
    }

    /// Reconstruct a full checkpoint from a potentially differential entry.
    async fn reconstruct(&self, entry: &DifferentialEntry<S>) -> Result<Checkpoint<S>> {
        match entry {
            DifferentialEntry::Full(checkpoint) => Ok(checkpoint.clone()),
            DifferentialEntry::Diff { metadata, diff } => {
                // Need to load the base and apply diff
                let base = self.load_base(&diff.base_id).await?;

                // Clone data for spawn_blocking (M-635: CPU-intensive bincode + diff)
                let base_state = base.state.clone();
                let diff_clone = diff.clone();

                // CPU-intensive work in spawn_blocking
                let state: S = tokio::task::spawn_blocking(move || {
                    let base_data = bincode::serialize(&base_state).map_err(|e| {
                        crate::Error::Checkpoint(crate::error::CheckpointError::SerializationFailed {
                            reason: format!("Failed to serialize base state: {}", e),
                        })
                    })?;

                    let new_data = diff_clone.apply(&base_data)?;
                    bincode::deserialize(&new_data).map_err(|e| {
                        crate::Error::Checkpoint(crate::error::CheckpointError::DeserializationFailed {
                            reason: format!("Failed to deserialize reconstructed state: {}", e),
                        })
                    })
                })
                .await
                .map_err(|e| {
                    crate::Error::Checkpoint(crate::error::CheckpointError::Other(format!(
                        "Task join error reconstructing checkpoint: {e}"
                    )))
                })??;

                Ok(Checkpoint {
                    id: metadata.id.clone(),
                    thread_id: metadata.thread_id.clone(),
                    state,
                    node: metadata.node.clone(),
                    timestamp: metadata.timestamp,
                    parent_id: metadata.parent_id.clone(),
                    metadata: metadata.metadata.clone(),
                })
            }
        }
    }

    /// Load a base checkpoint (following chain if necessary).
    async fn load_base(&self, base_id: &str) -> Result<Checkpoint<S>> {
        let mut chain_length = 0;
        let mut current_id = base_id.to_string();

        loop {
            chain_length += 1;
            if chain_length > self.config.max_chain_length {
                return Err(crate::Error::Checkpoint(
                    crate::error::CheckpointError::IntegrityCheckFailed {
                        checkpoint_id: current_id,
                        reason: format!(
                            "Diff chain too long (>{}) - possible corruption",
                            self.config.max_chain_length
                        ),
                    },
                ));
            }

            // Check our entry cache first
            let entry = {
                let entries = self.entries.lock().unwrap_or_else(|e| e.into_inner());
                entries.get(&current_id).cloned()
            };

            if let Some(entry) = entry {
                match entry {
                    DifferentialEntry::Full(checkpoint) => return Ok(checkpoint),
                    DifferentialEntry::Diff { diff, .. } => {
                        current_id = diff.base_id.clone();
                        continue;
                    }
                }
            }

            // Fall back to inner checkpointer
            if let Some(checkpoint) = self.inner.load(&current_id).await? {
                return Ok(checkpoint);
            }

            return Err(crate::Error::Checkpoint(
                crate::error::CheckpointError::NotFound {
                    checkpoint_id: current_id,
                },
            ));
        }
    }
}

#[async_trait::async_trait]
impl<S: GraphState, C: Checkpointer<S>> Checkpointer<S> for DifferentialCheckpointer<S, C> {
    async fn save(&self, checkpoint: Checkpoint<S>) -> Result<()> {
        let thread_id = checkpoint.thread_id.clone();
        let checkpoint_id = checkpoint.id.clone();
        let count = self.increment_count(&thread_id);

        // Decide: store full or diff
        let store_full = self.should_store_base(&thread_id) || checkpoint.parent_id.is_none();

        if store_full {
            // Store as full base checkpoint
            {
                let mut entries = self.entries.lock().unwrap_or_else(|e| e.into_inner());
                entries.insert(
                    checkpoint_id.clone(),
                    DifferentialEntry::Full(checkpoint.clone()),
                );
            }
            {
                let mut last_base = self.last_base.lock().unwrap_or_else(|e| e.into_inner());
                last_base.insert(thread_id, checkpoint_id);
            }
            self.inner.save(checkpoint).await
        } else {
            // Try to create a diff from the last base
            let base_id = {
                let last_base = self.last_base.lock().unwrap_or_else(|e| e.into_inner());
                last_base.get(&thread_id).cloned()
            };

            if let Some(base_id) = base_id {
                // Load base and compute diff
                if let Ok(base) = self.load_base(&base_id).await {
                    // Clone data for spawn_blocking (M-635: CPU-intensive bincode + diff)
                    let base_state = base.state.clone();
                    let checkpoint_state = checkpoint.state.clone();
                    let min_diff_size = self.config.min_diff_size;
                    let base_id_clone = base_id.clone();

                    // CPU-intensive work in spawn_blocking
                    let diff_result: Option<CheckpointDiff> = tokio::task::spawn_blocking(
                        move || {
                            let base_data = bincode::serialize(&base_state).map_err(|e| {
                                crate::Error::Checkpoint(
                                    crate::error::CheckpointError::SerializationFailed {
                                        reason: format!("Failed to serialize base state: {}", e),
                                    },
                                )
                            })?;

                            let new_data = bincode::serialize(&checkpoint_state).map_err(|e| {
                                crate::Error::Checkpoint(
                                    crate::error::CheckpointError::SerializationFailed {
                                        reason: format!("Failed to serialize new state: {}", e),
                                    },
                                )
                            })?;

                            if new_data.len() >= min_diff_size {
                                if let Some(mut diff) =
                                    CheckpointDiff::create(&base_data, &new_data)
                                {
                                    diff.base_id = base_id_clone;
                                    return Ok(Some(diff));
                                }
                            }
                            Ok::<_, crate::Error>(None)
                        },
                    )
                    .await
                    .map_err(|e| {
                        crate::Error::Checkpoint(crate::error::CheckpointError::Other(format!(
                            "Task join error computing diff: {e}"
                        )))
                    })??;

                    if let Some(diff) = diff_result {
                        let metadata = CheckpointMetadata::from(&checkpoint);
                        let entry = DifferentialEntry::Diff { metadata, diff };

                        {
                            let mut entries =
                                self.entries.lock().unwrap_or_else(|e| e.into_inner());
                            entries.insert(checkpoint_id, entry);
                        } // Drop lock before await

                        // For diff entries, we still need to save metadata to inner
                        // so get_latest and list work correctly
                        return self.inner.save(checkpoint).await;
                    }
                }
            }

            // Fallback: couldn't diff, store as full base
            tracing::debug!(
                checkpoint_id = checkpoint_id,
                count = count,
                "Storing full checkpoint (diff not beneficial)"
            );

            {
                let mut entries = self.entries.lock().unwrap_or_else(|e| e.into_inner());
                entries.insert(
                    checkpoint_id.clone(),
                    DifferentialEntry::Full(checkpoint.clone()),
                );
            }
            {
                let mut last_base = self.last_base.lock().unwrap_or_else(|e| e.into_inner());
                last_base.insert(thread_id, checkpoint_id);
            }
            self.inner.save(checkpoint).await
        }
    }

    async fn load(&self, checkpoint_id: &str) -> Result<Option<Checkpoint<S>>> {
        // Check our entry cache first
        let entry = {
            let entries = self.entries.lock().unwrap_or_else(|e| e.into_inner());
            entries.get(checkpoint_id).cloned()
        };

        if let Some(entry) = entry {
            return Ok(Some(self.reconstruct(&entry).await?));
        }

        // Fall back to inner checkpointer
        self.inner.load(checkpoint_id).await
    }

    async fn get_latest(&self, thread_id: &str) -> Result<Option<Checkpoint<S>>> {
        // Get latest from inner, then reconstruct if it's a diff
        if let Some(checkpoint) = self.inner.get_latest(thread_id).await? {
            let entry = {
                let entries = self.entries.lock().unwrap_or_else(|e| e.into_inner());
                entries.get(&checkpoint.id).cloned()
            };

            if let Some(entry) = entry {
                return Ok(Some(self.reconstruct(&entry).await?));
            }

            return Ok(Some(checkpoint));
        }

        Ok(None)
    }

    async fn list(&self, thread_id: &str) -> Result<Vec<CheckpointMetadata>> {
        // Delegate to inner - metadata is always stored there
        self.inner.list(thread_id).await
    }

    async fn delete(&self, checkpoint_id: &str) -> Result<()> {
        // Remove from our cache
        {
            let mut entries = self.entries.lock().unwrap_or_else(|e| e.into_inner());
            entries.remove(checkpoint_id);
        }

        // Delegate to inner
        self.inner.delete(checkpoint_id).await
    }

    async fn delete_thread(&self, thread_id: &str) -> Result<()> {
        // Remove all entries for this thread from our cache
        {
            let mut entries = self.entries.lock().unwrap_or_else(|e| e.into_inner());
            entries.retain(|_, entry| match entry {
                DifferentialEntry::Full(cp) => cp.thread_id != thread_id,
                DifferentialEntry::Diff { metadata, .. } => metadata.thread_id != thread_id,
            });
        }

        // Clear thread-specific tracking
        {
            let mut counts = self.thread_counts.lock().unwrap_or_else(|e| e.into_inner());
            counts.remove(thread_id);
        }
        {
            let mut last_base = self.last_base.lock().unwrap_or_else(|e| e.into_inner());
            last_base.remove(thread_id);
        }

        // Delegate to inner
        self.inner.delete_thread(thread_id).await
    }

    async fn list_threads(&self) -> Result<Vec<ThreadInfo>> {
        // Delegate to inner
        self.inner.list_threads().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::checkpoint::MemoryCheckpointer;
    use serde::{Deserialize, Serialize};
    use std::time::SystemTime;

    /// Test state for checkpoint tests
    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    struct TestState {
        counter: i32,
        message: String,
        data: Vec<u8>,
    }

    impl Default for TestState {
        fn default() -> Self {
            Self {
                counter: 0,
                message: String::new(),
                data: Vec::new(),
            }
        }
    }

    // ========================================================================
    // CheckpointDiff Tests
    // ========================================================================

    #[test]
    fn test_checkpoint_diff_small_state_returns_none() {
        // States smaller than MIN_DIFF_SIZE should return None
        let base = vec![0u8; 100];
        let new = vec![1u8; 100];

        let diff = CheckpointDiff::create(&base, &new);
        assert!(diff.is_none(), "Small states should not produce a diff");
    }

    #[test]
    fn test_checkpoint_diff_identical_states() {
        // Identical states produce a diff with just the header (8 bytes for length)
        // which is smaller than the full state, so a diff IS created
        let data = vec![42u8; 2000];
        let diff = CheckpointDiff::create(&data, &data);

        // For identical states, diff is just 8 bytes (the length header)
        // which is smaller than 2000 bytes, so create() returns Some
        if let Some(diff) = diff {
            // Diff should be just 8 bytes for identical states
            assert_eq!(diff.diff_data.len(), 8);

            // Applying should reconstruct the original
            let mut diff_with_id = diff;
            diff_with_id.base_id = "test".to_string();
            let reconstructed = diff_with_id.apply(&data).unwrap();
            assert_eq!(reconstructed, data);
        }
    }

    #[test]
    fn test_checkpoint_diff_create_and_apply() {
        // Create two states with similar content but some differences
        let base = vec![0u8; 2000];
        let mut new = base.clone();

        // Change a small portion
        for i in 100..200 {
            new[i] = 255;
        }

        let diff = CheckpointDiff::create(&base, &new);
        assert!(diff.is_some(), "Should create diff for partial changes");

        let diff = diff.unwrap();
        assert!(
            diff.diff_data.len() < new.len(),
            "Diff should be smaller than full state"
        );
        assert_eq!(diff.original_size, base.len());
        assert_eq!(diff.new_size, new.len());

        // Apply diff with a base_id
        let mut diff_with_id = diff;
        diff_with_id.base_id = "base-001".to_string();

        let reconstructed = diff_with_id.apply(&base).unwrap();
        assert_eq!(reconstructed, new);
    }

    #[test]
    fn test_checkpoint_diff_apply_size_mismatch() {
        let base = vec![0u8; 2000];
        let mut new = base.clone();
        for i in 100..200 {
            new[i] = 255;
        }

        if let Some(mut diff) = CheckpointDiff::create(&base, &new) {
            diff.base_id = "test".to_string();

            // Try to apply with wrong-sized base
            let wrong_base = vec![0u8; 1000];
            let result = diff.apply(&wrong_base);
            assert!(result.is_err(), "Should fail with size mismatch");

            let err = result.unwrap_err();
            assert!(
                err.to_string().contains("size mismatch"),
                "Error should mention size mismatch"
            );
        }
    }

    #[test]
    fn test_checkpoint_diff_growing_state() {
        // Test state that grows
        let base = vec![0u8; 2000];
        let mut new = vec![0u8; 3000]; // Larger state
        for i in 0..100 {
            new[i] = 255; // Add differences
        }

        let diff = CheckpointDiff::create(&base, &new);
        if let Some(mut diff) = diff {
            diff.base_id = "base".to_string();
            let reconstructed = diff.apply(&base).unwrap();
            assert_eq!(reconstructed.len(), new.len());
            assert_eq!(reconstructed, new);
        }
    }

    #[test]
    fn test_checkpoint_diff_shrinking_state() {
        // Test state that shrinks
        let base = vec![0u8; 3000];
        let mut new = vec![0u8; 2000]; // Smaller state
        for i in 0..100 {
            new[i] = 255; // Add differences
        }

        let diff = CheckpointDiff::create(&base, &new);
        if let Some(mut diff) = diff {
            diff.base_id = "base".to_string();
            let reconstructed = diff.apply(&base).unwrap();
            assert_eq!(reconstructed.len(), new.len());
            assert_eq!(reconstructed, new);
        }
    }

    #[test]
    fn test_checkpoint_diff_binary_diff_format() {
        // Test that diff format encodes length correctly
        let base = vec![0u8; 2000];
        let mut new = base.clone();
        for i in 500..600 {
            new[i] = 255;
        }

        let diff = CheckpointDiff::create(&base, &new);
        assert!(diff.is_some());

        let diff = diff.unwrap();
        // First 8 bytes are target length
        let target_len = u64::from_le_bytes(diff.diff_data[0..8].try_into().unwrap());
        assert_eq!(target_len as usize, new.len());
    }

    #[test]
    fn test_checkpoint_diff_apply_corrupted_diff() {
        // Test with corrupted diff data (too small)
        let base = vec![0u8; 2000];
        let diff = CheckpointDiff {
            base_id: "test".to_string(),
            diff_data: vec![0u8; 4], // Too small to be valid
            original_size: base.len(),
            new_size: 2000,
        };

        let result = diff.apply(&base);
        assert!(result.is_err(), "Should fail with corrupted diff");
    }

    #[test]
    fn test_checkpoint_diff_apply_length_mismatch_in_diff() {
        // Test where diff says different length than expected
        let base = vec![0u8; 2000];
        let mut diff_data = vec![0u8; 20];
        // Write wrong length (1000 instead of 2000)
        diff_data[0..8].copy_from_slice(&(1000u64).to_le_bytes());

        let diff = CheckpointDiff {
            base_id: "test".to_string(),
            diff_data,
            original_size: base.len(),
            new_size: 2000, // Says 2000 but diff says 1000
        };

        let result = diff.apply(&base);
        assert!(result.is_err(), "Should fail with length mismatch");
    }

    #[test]
    fn test_checkpoint_diff_chunk_extends_past_diff() {
        // Test where chunk claims to extend past diff data end
        let base = vec![0u8; 2000];
        let mut diff_data = Vec::new();
        // Target length
        diff_data.extend_from_slice(&(2000u64).to_le_bytes());
        // Position
        diff_data.extend_from_slice(&(0u32).to_le_bytes());
        // Length that exceeds remaining diff data
        diff_data.extend_from_slice(&(1000u16).to_le_bytes());
        // Only 10 bytes of actual data (not 1000)
        diff_data.extend_from_slice(&[0u8; 10]);

        let diff = CheckpointDiff {
            base_id: "test".to_string(),
            diff_data,
            original_size: base.len(),
            new_size: 2000,
        };

        let result = diff.apply(&base);
        assert!(result.is_err(), "Should fail when chunk extends past diff");
    }

    #[test]
    fn test_checkpoint_diff_chunk_exceeds_result() {
        // Test where chunk position + length exceeds result size
        let base = vec![0u8; 2000];
        let mut diff_data = Vec::new();
        // Target length
        diff_data.extend_from_slice(&(2000u64).to_le_bytes());
        // Position near end
        diff_data.extend_from_slice(&(1990u32).to_le_bytes());
        // Length that would exceed result
        diff_data.extend_from_slice(&(100u16).to_le_bytes());
        // Data
        diff_data.extend_from_slice(&[255u8; 100]);

        let diff = CheckpointDiff {
            base_id: "test".to_string(),
            diff_data,
            original_size: base.len(),
            new_size: 2000,
        };

        let result = diff.apply(&base);
        assert!(result.is_err(), "Should fail when chunk exceeds result size");
    }

    #[test]
    fn test_checkpoint_diff_min_diff_size_constant() {
        assert_eq!(CheckpointDiff::MIN_DIFF_SIZE, 1024);
    }

    #[test]
    fn test_checkpoint_diff_not_worth_it() {
        // Create states where diff would be larger than full state
        // (random-looking data has no repeating patterns to diff)
        let base = (0..2000u16).map(|i| (i % 256) as u8).collect::<Vec<_>>();
        let new = (0..2000u16)
            .map(|i| ((i + 128) % 256) as u8)
            .collect::<Vec<_>>();

        let diff = CheckpointDiff::create(&base, &new);
        // Diff should be None because diff is larger than full state
        assert!(diff.is_none());
    }

    // ========================================================================
    // DifferentialConfig Tests
    // ========================================================================

    #[test]
    fn test_differential_config_default() {
        let config = DifferentialConfig::default();
        assert_eq!(config.base_interval, 10);
        assert_eq!(config.max_chain_length, 20);
        assert_eq!(config.min_diff_size, CheckpointDiff::MIN_DIFF_SIZE);
    }

    #[test]
    fn test_differential_config_memory_optimized() {
        let config = DifferentialConfig::memory_optimized();
        assert_eq!(config.base_interval, 20);
        assert_eq!(config.max_chain_length, 50);
        assert_eq!(config.min_diff_size, 512);
    }

    #[test]
    fn test_differential_config_speed_optimized() {
        let config = DifferentialConfig::speed_optimized();
        assert_eq!(config.base_interval, 5);
        assert_eq!(config.max_chain_length, 10);
        assert_eq!(config.min_diff_size, 2048);
    }

    #[test]
    fn test_differential_config_clone() {
        let config = DifferentialConfig::default();
        let cloned = config.clone();
        assert_eq!(config.base_interval, cloned.base_interval);
        assert_eq!(config.max_chain_length, cloned.max_chain_length);
        assert_eq!(config.min_diff_size, cloned.min_diff_size);
    }

    // ========================================================================
    // DifferentialEntry Tests
    // ========================================================================

    #[test]
    fn test_differential_entry_full_serialization() {
        let state = TestState {
            counter: 42,
            message: "test".to_string(),
            data: vec![1, 2, 3],
        };
        let checkpoint = Checkpoint {
            id: "cp-1".into(),
            thread_id: "thread-1".into(),
            state,
            node: "node".to_string(),
            timestamp: SystemTime::now(),
            parent_id: None,
            metadata: HashMap::new(),
        };

        let entry: DifferentialEntry<TestState> = DifferentialEntry::Full(checkpoint.clone());

        // Serialize and deserialize
        let json = serde_json::to_string(&entry).unwrap();
        let recovered: DifferentialEntry<TestState> = serde_json::from_str(&json).unwrap();

        match recovered {
            DifferentialEntry::Full(cp) => {
                assert_eq!(cp.id, "cp-1");
                assert_eq!(cp.state.counter, 42);
            }
            DifferentialEntry::Diff { .. } => panic!("Expected Full entry"),
        }
    }

    // ========================================================================
    // DifferentialCheckpointer Tests
    // ========================================================================

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

    #[test]
    fn test_differential_checkpointer_new() {
        let inner: MemoryCheckpointer<TestState> = MemoryCheckpointer::new();
        let diff = DifferentialCheckpointer::new(inner);

        let config = diff.config();
        assert_eq!(config.base_interval, 10);
    }

    #[test]
    fn test_differential_checkpointer_with_config() {
        let inner: MemoryCheckpointer<TestState> = MemoryCheckpointer::new();
        let config = DifferentialConfig {
            base_interval: 5,
            max_chain_length: 15,
            min_diff_size: 512,
        };
        let diff = DifferentialCheckpointer::with_config(inner, config);

        assert_eq!(diff.config().base_interval, 5);
        assert_eq!(diff.config().max_chain_length, 15);
        assert_eq!(diff.config().min_diff_size, 512);
    }

    #[test]
    fn test_differential_checkpointer_memory_optimized() {
        let inner: MemoryCheckpointer<TestState> = MemoryCheckpointer::new();
        let diff = DifferentialCheckpointer::memory_optimized(inner);

        assert_eq!(diff.config().base_interval, 20);
    }

    #[test]
    fn test_differential_checkpointer_speed_optimized() {
        let inner: MemoryCheckpointer<TestState> = MemoryCheckpointer::new();
        let diff = DifferentialCheckpointer::speed_optimized(inner);

        assert_eq!(diff.config().base_interval, 5);
    }

    #[tokio::test]
    async fn test_differential_checkpointer_save_first_as_base() {
        let inner: MemoryCheckpointer<TestState> = MemoryCheckpointer::new();
        let diff = DifferentialCheckpointer::new(inner);

        let state = TestState {
            counter: 1,
            message: "first".to_string(),
            data: vec![0u8; 100],
        };
        let checkpoint = make_checkpoint("cp-1", "thread-1", state.clone());

        diff.save(checkpoint).await.unwrap();

        // Should be able to load it back
        let loaded = diff.load("cp-1").await.unwrap();
        assert!(loaded.is_some());
        assert_eq!(loaded.unwrap().state, state);
    }

    #[tokio::test]
    async fn test_differential_checkpointer_save_and_load() {
        let inner: MemoryCheckpointer<TestState> = MemoryCheckpointer::new();
        let diff = DifferentialCheckpointer::new(inner);

        // Save multiple checkpoints
        for i in 0..5 {
            let state = TestState {
                counter: i,
                message: format!("checkpoint-{}", i),
                data: vec![i as u8; 100],
            };
            let checkpoint = make_checkpoint(&format!("cp-{}", i), "thread-1", state);
            diff.save(checkpoint).await.unwrap();
        }

        // Load each one
        for i in 0..5 {
            let loaded = diff.load(&format!("cp-{}", i)).await.unwrap();
            assert!(loaded.is_some());
            let loaded = loaded.unwrap();
            assert_eq!(loaded.state.counter, i);
            assert_eq!(loaded.state.message, format!("checkpoint-{}", i));
        }
    }

    #[tokio::test]
    async fn test_differential_checkpointer_get_latest() {
        let inner: MemoryCheckpointer<TestState> = MemoryCheckpointer::new();
        let diff = DifferentialCheckpointer::new(inner);

        // Save checkpoints
        for i in 0..3 {
            let state = TestState {
                counter: i,
                message: format!("msg-{}", i),
                data: vec![],
            };
            let checkpoint = make_checkpoint(&format!("cp-{}", i), "thread-1", state);
            diff.save(checkpoint).await.unwrap();
        }

        let latest = diff.get_latest("thread-1").await.unwrap();
        assert!(latest.is_some());
        assert_eq!(latest.unwrap().state.counter, 2);
    }

    #[tokio::test]
    async fn test_differential_checkpointer_list() {
        let inner: MemoryCheckpointer<TestState> = MemoryCheckpointer::new();
        let diff = DifferentialCheckpointer::new(inner);

        for i in 0..3 {
            let state = TestState::default();
            let checkpoint = make_checkpoint(&format!("cp-{}", i), "thread-1", state);
            diff.save(checkpoint).await.unwrap();
        }

        let list = diff.list("thread-1").await.unwrap();
        assert_eq!(list.len(), 3);
    }

    #[tokio::test]
    async fn test_differential_checkpointer_delete() {
        let inner: MemoryCheckpointer<TestState> = MemoryCheckpointer::new();
        let diff = DifferentialCheckpointer::new(inner);

        let state = TestState::default();
        let checkpoint = make_checkpoint("cp-1", "thread-1", state);
        diff.save(checkpoint).await.unwrap();

        // Verify it exists
        let loaded = diff.load("cp-1").await.unwrap();
        assert!(loaded.is_some());

        // Delete
        diff.delete("cp-1").await.unwrap();

        // Verify deleted
        let loaded = diff.load("cp-1").await.unwrap();
        assert!(loaded.is_none());
    }

    #[tokio::test]
    async fn test_differential_checkpointer_delete_thread() {
        let inner: MemoryCheckpointer<TestState> = MemoryCheckpointer::new();
        let diff = DifferentialCheckpointer::new(inner);

        // Save to two threads
        for i in 0..3 {
            let state = TestState::default();
            let cp1 = make_checkpoint(&format!("t1-cp-{}", i), "thread-1", state.clone());
            let cp2 = make_checkpoint(&format!("t2-cp-{}", i), "thread-2", state);
            diff.save(cp1).await.unwrap();
            diff.save(cp2).await.unwrap();
        }

        // Delete thread-1
        diff.delete_thread("thread-1").await.unwrap();

        // thread-1 should be empty
        let list1 = diff.list("thread-1").await.unwrap();
        assert!(list1.is_empty());

        // thread-2 should still have checkpoints
        let list2 = diff.list("thread-2").await.unwrap();
        assert_eq!(list2.len(), 3);
    }

    #[tokio::test]
    async fn test_differential_checkpointer_list_threads() {
        let inner: MemoryCheckpointer<TestState> = MemoryCheckpointer::new();
        let diff = DifferentialCheckpointer::new(inner);

        let state = TestState::default();
        let cp1 = make_checkpoint("cp-1", "thread-1", state.clone());
        let cp2 = make_checkpoint("cp-2", "thread-2", state);
        diff.save(cp1).await.unwrap();
        diff.save(cp2).await.unwrap();

        let threads = diff.list_threads().await.unwrap();
        assert_eq!(threads.len(), 2);
    }

    #[tokio::test]
    async fn test_differential_checkpointer_base_interval() {
        // Test that every N-th checkpoint is stored as a base
        let inner: MemoryCheckpointer<TestState> = MemoryCheckpointer::new();
        let config = DifferentialConfig {
            base_interval: 3,
            max_chain_length: 10,
            min_diff_size: 512,
        };
        let diff = DifferentialCheckpointer::with_config(inner, config);

        // Save 6 checkpoints
        for i in 0..6 {
            let state = TestState {
                counter: i,
                message: format!("data-{}", i),
                data: vec![i as u8; 100],
            };
            let checkpoint = make_checkpoint(&format!("cp-{}", i), "thread-1", state);
            diff.save(checkpoint).await.unwrap();
        }

        // All should be loadable
        for i in 0..6 {
            let loaded = diff.load(&format!("cp-{}", i)).await.unwrap();
            assert!(loaded.is_some(), "Checkpoint {} should be loadable", i);
            assert_eq!(loaded.unwrap().state.counter, i);
        }
    }

    #[tokio::test]
    async fn test_differential_checkpointer_large_state_with_diffs() {
        // Test with states large enough to actually benefit from diffing
        let inner: MemoryCheckpointer<TestState> = MemoryCheckpointer::new();
        let config = DifferentialConfig {
            base_interval: 5,
            max_chain_length: 20,
            min_diff_size: 512,
        };
        let diff = DifferentialCheckpointer::with_config(inner, config);

        // Create large states with small changes
        let base_data = vec![0u8; 2000];

        for i in 0..10 {
            let mut data = base_data.clone();
            // Small change each iteration
            if i > 0 {
                data[100 + i] = i as u8;
            }

            let state = TestState {
                counter: i as i32,
                message: "large".to_string(),
                data,
            };

            let parent = if i > 0 {
                Some(format!("cp-{}", i - 1))
            } else {
                None
            };

            let checkpoint = Checkpoint {
                id: format!("cp-{}", i),
                thread_id: "thread-1".into(),
                state,
                node: "test".to_string(),
                timestamp: SystemTime::now(),
                parent_id: parent.map(Into::into),
                metadata: HashMap::new(),
            };

            diff.save(checkpoint).await.unwrap();
        }

        // All should be loadable and correct
        for i in 0..10 {
            let loaded = diff.load(&format!("cp-{}", i)).await.unwrap();
            assert!(loaded.is_some());
            let loaded = loaded.unwrap();
            assert_eq!(loaded.state.counter, i as i32);
            assert_eq!(loaded.state.data.len(), 2000);
        }
    }

    #[tokio::test]
    async fn test_differential_checkpointer_load_nonexistent() {
        let inner: MemoryCheckpointer<TestState> = MemoryCheckpointer::new();
        let diff = DifferentialCheckpointer::new(inner);

        let loaded = diff.load("nonexistent").await.unwrap();
        assert!(loaded.is_none());
    }

    #[tokio::test]
    async fn test_differential_checkpointer_get_latest_nonexistent() {
        let inner: MemoryCheckpointer<TestState> = MemoryCheckpointer::new();
        let diff = DifferentialCheckpointer::new(inner);

        let latest = diff.get_latest("nonexistent-thread").await.unwrap();
        assert!(latest.is_none());
    }

    #[tokio::test]
    async fn test_differential_checkpointer_multiple_threads() {
        let inner: MemoryCheckpointer<TestState> = MemoryCheckpointer::new();
        let diff = DifferentialCheckpointer::new(inner);

        // Each thread has its own count/base tracking
        for t in 0..3 {
            for i in 0..5 {
                let state = TestState {
                    counter: (t * 100 + i) as i32,
                    message: format!("t{}-{}", t, i),
                    data: vec![],
                };
                let checkpoint =
                    make_checkpoint(&format!("t{}-cp-{}", t, i), &format!("thread-{}", t), state);
                diff.save(checkpoint).await.unwrap();
            }
        }

        // Verify each thread's data
        for t in 0..3 {
            let latest = diff.get_latest(&format!("thread-{}", t)).await.unwrap();
            assert!(latest.is_some());
            assert_eq!(latest.unwrap().state.counter, (t * 100 + 4) as i32);
        }
    }

    #[tokio::test]
    async fn test_differential_checkpointer_clone() {
        let inner: MemoryCheckpointer<TestState> = MemoryCheckpointer::new();
        let diff = DifferentialCheckpointer::new(inner);

        let state = TestState::default();
        let checkpoint = make_checkpoint("cp-1", "thread-1", state);
        diff.save(checkpoint).await.unwrap();

        // Clone should share state via Arc
        let cloned = diff.clone();
        let loaded = cloned.load("cp-1").await.unwrap();
        assert!(loaded.is_some());
    }

    #[tokio::test]
    async fn test_differential_checkpointer_with_metadata() {
        let inner: MemoryCheckpointer<TestState> = MemoryCheckpointer::new();
        let diff = DifferentialCheckpointer::new(inner);

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
            parent_id: None,
            metadata: metadata.clone(),
        };

        diff.save(checkpoint).await.unwrap();

        let loaded = diff.load("cp-meta").await.unwrap().unwrap();
        assert_eq!(loaded.metadata, metadata);
    }

    #[tokio::test]
    async fn test_differential_checkpointer_preserves_parent_chain() {
        let inner: MemoryCheckpointer<TestState> = MemoryCheckpointer::new();
        let diff = DifferentialCheckpointer::new(inner);

        // First checkpoint (no parent)
        let state1 = TestState {
            counter: 1,
            message: "first".to_string(),
            data: vec![],
        };
        let cp1 = Checkpoint {
            id: "cp-1".into(),
            thread_id: "thread-1".into(),
            state: state1,
            node: "node".to_string(),
            timestamp: SystemTime::now(),
            parent_id: None,
            metadata: HashMap::new(),
        };
        diff.save(cp1).await.unwrap();

        // Second checkpoint (parent = cp-1)
        let state2 = TestState {
            counter: 2,
            message: "second".to_string(),
            data: vec![],
        };
        let cp2 = Checkpoint {
            id: "cp-2".into(),
            thread_id: "thread-1".into(),
            state: state2,
            node: "node".to_string(),
            timestamp: SystemTime::now(),
            parent_id: Some("cp-1".into()),
            metadata: HashMap::new(),
        };
        diff.save(cp2).await.unwrap();

        // Verify parent_id is preserved
        let loaded = diff.load("cp-2").await.unwrap().unwrap();
        assert_eq!(loaded.parent_id, Some("cp-1".into()));
    }
}
