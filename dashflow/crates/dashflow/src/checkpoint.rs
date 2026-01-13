// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

//! Checkpointing system for graph state persistence
//!
//! Checkpointing enables:
//! - Resume execution from failures
//! - Pause/resume workflows (human-in-the-loop)
//! - State snapshots for debugging
//! - Audit trails
//!
//! # Example
//!
//! ```rust,ignore
//! use dashflow::{StateGraph, GraphState, MemoryCheckpointer};
//! use serde::{Deserialize, Serialize};
//!
//! #[derive(Clone, Debug, Serialize, Deserialize)]
//! struct MyState {
//!     value: i32,
//! }
//!
//! async fn example() -> Result<(), Box<dyn std::error::Error>> {
//!     let checkpointer = MemoryCheckpointer::new();
//!     let mut graph = StateGraph::new();
//!     graph.add_node("step1", |state: MyState| async move {
//!         Ok(MyState { value: state.value + 1 })
//!     });
//!     graph.add_edge("step1", "__end__")?;
//!     graph.set_entry_point("step1")?;
//!     let app = graph.compile()?.with_checkpointer(checkpointer);
//!
//!     let initial_state = MyState { value: 0 };
//!     let result = app.invoke(initial_state).await?;
//!     Ok(())
//! }
//! ```

pub mod compression;
pub mod differential;
pub mod distributed;
#[cfg(feature = "encryption")]
pub mod encryption;
pub mod replicated;
pub mod resume;
pub mod sqlite;
pub mod tiered;
pub mod versioned;

pub use compression::{CompressedFileCheckpointer, CompressionAlgorithm};
pub use differential::{CheckpointDiff, DifferentialCheckpointer, DifferentialConfig};
pub use distributed::DistributedCheckpointCoordinator;
#[cfg(feature = "encryption")]
pub use encryption::{decrypt_bytes, encrypt_bytes, EncryptionKey};
pub use replicated::{ReplicatedCheckpointer, ReplicatedCheckpointerConfig, ReplicationMode};
pub use resume::{ResumeEnvironment, ResumeError, ResumeOutcome, ResumeRunner, ResumeValidator};
pub use sqlite::SqliteCheckpointer;
pub use tiered::{MultiTierCheckpointer, WritePolicy};
pub use versioned::{
    MigrationChain, StateMigration, Version, VersionedCheckpoint, VersionedFileCheckpointer,
};

use crate::{GraphState, Result};
use serde::{Deserialize, Serialize};
use std::cell::Cell;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::SystemTime;
use thiserror::Error;

// ============================================================================
// Checkpoint File Integrity System
// ============================================================================
// Provides version and checksum verification for checkpoint files.
// Detects corruption from bit flips, partial writes, and format changes.

/// Magic bytes identifying a dashflow checkpoint file: "DCHK"
const CHECKPOINT_MAGIC: &[u8; 4] = b"DCHK";

/// Current format version for checkpoint files
/// Increment this when changing the header structure or serialization format
const CHECKPOINT_FORMAT_VERSION: u32 = 1;

/// Header size: magic(4) + version(4) + crc32(4) + length(8) = 20 bytes
const CHECKPOINT_HEADER_SIZE: usize = 20;

/// Error types for checkpoint integrity failures
#[derive(Debug, Clone, PartialEq, Error)]
#[non_exhaustive]
pub enum CheckpointIntegrityError {
    /// File is too small to contain a valid header
    #[error("Checkpoint file too small: {size} bytes (minimum {minimum} bytes)")]
    FileTooSmall {
        /// The actual file size in bytes.
        size: usize,
        /// The minimum required size in bytes.
        minimum: usize,
    },
    /// Magic bytes don't match expected value
    #[error("Invalid checkpoint magic bytes: expected {expected:?}, found {found:?}")]
    InvalidMagic {
        /// The expected magic bytes.
        expected: [u8; 4],
        /// The actual magic bytes found.
        found: [u8; 4],
    },
    /// Format version is not supported
    #[error("Unsupported checkpoint format version: found {found}, supported up to {supported}")]
    UnsupportedVersion {
        /// The version found in the checkpoint file.
        found: u32,
        /// The maximum supported version.
        supported: u32,
    },
    /// CRC32 checksum mismatch (data corruption detected)
    #[error("Checkpoint checksum mismatch (data corruption): expected 0x{expected:08X}, computed 0x{computed:08X}")]
    ChecksumMismatch {
        /// The checksum stored in the file.
        expected: u32,
        /// The checksum computed from the payload.
        computed: u32,
    },
    /// Declared data length doesn't match actual data
    #[error("Checkpoint length mismatch: declared {declared} bytes, actual {actual} bytes")]
    LengthMismatch {
        /// The length declared in the header.
        declared: u64,
        /// The actual length of the payload.
        actual: u64,
    },
}

/// Wraps checkpoint data with integrity header for corruption detection
///
/// File format (20-byte header + payload):
/// - Bytes 0-3:   Magic "DCHK" (identifies dashflow checkpoint)
/// - Bytes 4-7:   Format version (u32 little-endian)
/// - Bytes 8-11:  CRC32 checksum of payload (u32 little-endian)
/// - Bytes 12-19: Payload length (u64 little-endian)
/// - Bytes 20+:   Payload (bincode-serialized checkpoint)
#[derive(Debug, Clone, Copy, Default)]
pub struct CheckpointWithIntegrity;

impl CheckpointWithIntegrity {
    /// Serialize checkpoint data with integrity header
    ///
    /// # Arguments
    /// * `data` - The raw bincode-serialized checkpoint bytes
    ///
    /// # Returns
    /// * Bytes with integrity header prepended
    pub fn wrap(data: &[u8]) -> Vec<u8> {
        let checksum = crc32fast::hash(data);
        let length = data.len() as u64;

        let mut result = Vec::with_capacity(CHECKPOINT_HEADER_SIZE + data.len());

        // Write header
        result.extend_from_slice(CHECKPOINT_MAGIC);
        result.extend_from_slice(&CHECKPOINT_FORMAT_VERSION.to_le_bytes());
        result.extend_from_slice(&checksum.to_le_bytes());
        result.extend_from_slice(&length.to_le_bytes());

        // Write payload
        result.extend_from_slice(data);

        result
    }

    /// Verify integrity and extract checkpoint data
    ///
    /// # Arguments
    /// * `data` - The raw file bytes including integrity header
    ///
    /// # Returns
    /// * `Ok(payload)` - The verified checkpoint bytes (without header)
    /// * `Err(CheckpointIntegrityError)` - Integrity check failed
    // SAFETY: try_into().unwrap() is safe - we verify data.len() >= CHECKPOINT_HEADER_SIZE (20 bytes)
    // before slicing, so [0..4], [4..8], [8..12], [12..20] are always valid sizes
    #[allow(clippy::unwrap_used)]
    pub fn unwrap(data: &[u8]) -> std::result::Result<&[u8], CheckpointIntegrityError> {
        // Check minimum size
        if data.len() < CHECKPOINT_HEADER_SIZE {
            return Err(CheckpointIntegrityError::FileTooSmall {
                size: data.len(),
                minimum: CHECKPOINT_HEADER_SIZE,
            });
        }

        // Verify magic bytes
        let magic: [u8; 4] = data[0..4].try_into().unwrap();
        if &magic != CHECKPOINT_MAGIC {
            return Err(CheckpointIntegrityError::InvalidMagic {
                expected: *CHECKPOINT_MAGIC,
                found: magic,
            });
        }

        // Verify version
        let version = u32::from_le_bytes(data[4..8].try_into().unwrap());
        if version > CHECKPOINT_FORMAT_VERSION {
            return Err(CheckpointIntegrityError::UnsupportedVersion {
                found: version,
                supported: CHECKPOINT_FORMAT_VERSION,
            });
        }

        // Read stored checksum and length
        let stored_checksum = u32::from_le_bytes(data[8..12].try_into().unwrap());
        let declared_length = u64::from_le_bytes(data[12..20].try_into().unwrap());

        // Extract payload
        let payload = &data[CHECKPOINT_HEADER_SIZE..];

        // Verify length
        let actual_length = payload.len() as u64;
        if declared_length != actual_length {
            return Err(CheckpointIntegrityError::LengthMismatch {
                declared: declared_length,
                actual: actual_length,
            });
        }

        // Verify checksum
        let computed_checksum = crc32fast::hash(payload);
        if stored_checksum != computed_checksum {
            return Err(CheckpointIntegrityError::ChecksumMismatch {
                expected: stored_checksum,
                computed: computed_checksum,
            });
        }

        Ok(payload)
    }

    /// Check if data appears to be a wrapped checkpoint (has magic header)
    ///
    /// Used for backward compatibility with old checkpoint files
    pub fn is_wrapped(data: &[u8]) -> bool {
        data.len() >= 4 && &data[0..4] == CHECKPOINT_MAGIC
    }
}

// Thread-local counter for checkpoint IDs (replaces expensive UUID generation)
// Each thread maintains its own counter, eliminating getentropy() overhead (20% of checkpoint time)
thread_local! {
    static CHECKPOINT_COUNTER: Cell<u64> = const { Cell::new(0) };
}

// Process-unique identifier to prevent checkpoint ID collisions across restarts
// Uses process start time (nanoseconds since epoch) + PID for uniqueness
// This is computed once at process start and used in all checkpoint IDs
static PROCESS_UNIQUE_ID: std::sync::OnceLock<String> = std::sync::OnceLock::new();

/// Get the process-unique identifier for checkpoint IDs
fn get_process_unique_id() -> &'static str {
    PROCESS_UNIQUE_ID.get_or_init(|| {
        let start_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let pid = std::process::id();
        // Use hex for compactness: timestamp (truncated) + pid
        format!("{:x}{:04x}", start_time % 0xFFFFFFFFFFFF, pid % 0xFFFF)
    })
}

/// Cross-process lock file path for a checkpoint directory.
/// Uses a `.checkpoint.lock` file in the directory to coordinate writes.
fn lock_file_path(directory: &std::path::Path) -> std::path::PathBuf {
    directory.join(".checkpoint.lock")
}

/// Acquire an exclusive (write) lock on the checkpoint directory.
///
/// This prevents concurrent writes from multiple processes that could
/// corrupt the index file. The lock is released when the returned File is dropped.
///
/// # Returns
/// A `File` handle that holds the lock - dropping it releases the lock.
fn acquire_exclusive_lock(directory: &std::path::Path) -> std::io::Result<std::fs::File> {
    let lock_path = lock_file_path(directory);
    let file = std::fs::OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .truncate(false)
        .open(&lock_path)?;

    // Use blocking exclusive lock (wait for other processes to release)
    // Use fs2::FileExt trait method explicitly to avoid MSRV issues with std::fs::File::lock_exclusive
    fs2::FileExt::lock_exclusive(&file)?;
    Ok(file)
}

/// Acquire a shared (read) lock on the checkpoint directory.
///
/// Multiple readers can hold shared locks simultaneously, but writers
/// are blocked. Used when loading the index.
///
/// # Returns
/// A `File` handle that holds the lock - dropping it releases the lock.
#[allow(dead_code)] // Architectural: API for future distributed index loading
fn acquire_shared_lock(directory: &std::path::Path) -> std::io::Result<std::fs::File> {
    let lock_path = lock_file_path(directory);
    // If lock file doesn't exist, there's nothing to read-protect
    if !lock_path.exists() {
        // Create it so future writers can coordinate
        return std::fs::OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .truncate(false)
            .open(&lock_path);
    }
    let file = std::fs::File::open(&lock_path)?;
    // Use fs2::FileExt trait method explicitly to avoid MSRV issues with std::fs::File::lock_shared
    fs2::FileExt::lock_shared(&file)?;
    Ok(file)
}

/// Atomic file write helper: writes to temp file, fsyncs, then atomic renames.
/// This prevents index corruption on crash or power loss.
async fn atomic_write_file(path: &std::path::Path, data: &[u8]) -> std::io::Result<()> {
    use tokio::io::AsyncWriteExt;

    // Generate a unique temp file name to avoid races with concurrent writes
    // Uses UUID v4 (122 bits of cryptographic randomness) for unpredictability
    // This prevents attackers from predicting temp file names (symlink attacks, etc.)
    let temp_name = format!(
        ".{}.{}.tmp",
        path.file_name().and_then(|n| n.to_str()).unwrap_or("file"),
        uuid::Uuid::new_v4()
    );
    let temp_path = path.with_file_name(&temp_name);

    // Write to temporary file first
    let mut file = tokio::fs::File::create(&temp_path).await?;
    file.write_all(data).await?;

    // Fsync to ensure data is on disk before rename
    file.sync_all().await?;

    // Atomic rename (on POSIX systems, rename is atomic)
    tokio::fs::rename(&temp_path, path).await?;

    // Optionally fsync the directory to ensure rename is durable
    // This is platform-dependent; on some systems the rename durability
    // requires fsyncing the parent directory
    #[cfg(unix)]
    {
        if let Some(parent) = path.parent() {
            if let Ok(dir) = tokio::fs::File::open(parent).await {
                // Best effort - ignore errors on directory fsync
                let _ = dir.sync_all().await;
            }
        }
    }

    Ok(())
}

/// Sync version of atomic file write for use in spawn_blocking contexts.
/// Writes to temp file, fsyncs, then atomic renames.
/// This prevents file corruption on crash or power loss.
fn atomic_write_file_sync(path: &std::path::Path, data: &[u8]) -> std::io::Result<()> {
    use std::io::Write;

    // Generate a unique temp file name to avoid races with concurrent writes
    // Uses UUID v4 (122 bits of cryptographic randomness) for unpredictability
    // This prevents attackers from predicting temp file names (symlink attacks, etc.)
    let temp_name = format!(
        ".{}.{}.tmp",
        path.file_name().and_then(|n| n.to_str()).unwrap_or("file"),
        uuid::Uuid::new_v4()
    );
    let temp_path = path.with_file_name(&temp_name);

    // Write to temporary file first
    let mut file = std::fs::File::create(&temp_path)?;
    file.write_all(data)?;

    // Fsync to ensure data is on disk before rename
    file.sync_all()?;

    // Atomic rename (on POSIX systems, rename is atomic)
    std::fs::rename(&temp_path, path)?;

    // Optionally fsync the directory to ensure rename is durable
    // This is platform-dependent; on some systems the rename durability
    // requires fsyncing the parent directory
    #[cfg(unix)]
    {
        if let Some(parent) = path.parent() {
            if let Ok(dir) = std::fs::File::open(parent) {
                // Best effort - ignore errors on directory fsync
                let _ = dir.sync_all();
            }
        }
    }

    Ok(())
}

/// Helper to load checkpoint index with proper error logging
///
/// Returns empty HashMap on corruption but logs a warning instead of silently failing.
fn load_checkpoint_index(
    index_path: &std::path::Path,
) -> HashMap<ThreadId, (CheckpointId, SystemTime)> {
    if !index_path.exists() {
        return HashMap::new();
    }

    match std::fs::read(index_path) {
        Ok(data) => match bincode::deserialize(&data) {
            Ok(idx) => idx,
            Err(e) => {
                // Log warning but recover - corrupted index means we lose O(1) lookup
                // but can still function by scanning files in list()
                tracing::warn!(
                    index_path = %index_path.display(),
                    error = %e,
                    "Checkpoint index is corrupted. Starting with empty index. Performance may be degraded until index is rebuilt."
                );
                HashMap::new()
            }
        },
        Err(e) => {
            tracing::warn!(
                index_path = %index_path.display(),
                error = %e,
                "Failed to read checkpoint index. Starting with empty index."
            );
            HashMap::new()
        }
    }
}

/// Unique identifier for a checkpoint
pub type CheckpointId = String;

/// Unique identifier for a graph execution thread
pub type ThreadId = String;

// ============================================================================
// Checkpoint Policy - Controls when checkpoints are taken
// ============================================================================

/// Policy controlling when checkpoints are saved during graph execution.
///
/// By default, checkpoints are saved after every node execution when a
/// checkpointer and thread_id are configured. For graphs with many nodes
/// or expensive state serialization, this can impact performance.
///
/// # Example
///
/// ```rust,ignore
/// use dashflow::{StateGraph, CheckpointPolicy, MemoryCheckpointer};
///
/// let app = graph.compile()?
///     .with_checkpointer(MemoryCheckpointer::new())
///     .with_thread_id("session-1")
///     // Only checkpoint every 5 nodes instead of every node
///     .with_checkpoint_policy(CheckpointPolicy::EveryN(5));
///
/// // Or use the convenience helper:
/// let app = graph.compile()?
///     .with_checkpointer(MemoryCheckpointer::new())
///     .with_thread_id("session-1")
///     .with_checkpoint_every(5);
/// ```
#[derive(Clone, Debug, Default, PartialEq)]
pub enum CheckpointPolicy {
    /// Checkpoint after every node execution (default).
    /// Best for: Critical workflows where no state loss is acceptable.
    #[default]
    Every,

    /// Checkpoint every N node executions.
    /// Best for: Balancing reliability with performance.
    EveryN(usize),

    /// Only checkpoint at explicitly marked nodes.
    /// Use `state.checkpoint_here()` or configure marker nodes.
    /// Best for: Maximum control over checkpoint locations.
    OnMarkers(std::collections::HashSet<String>),

    /// Checkpoint when state size changes by at least `min_delta` bytes.
    /// Best for: States that grow incrementally (append-only logs, etc.).
    OnStateChange {
        /// Minimum state size change in bytes to trigger checkpoint.
        min_delta: usize,
    },

    /// Never checkpoint (equivalent to `without_checkpointing()`).
    /// Best for: Testing or pure stateless graphs.
    Never,
}

/// Error type for checkpoint policy configuration validation.
#[derive(Debug, Clone, PartialEq, Error)]
#[non_exhaustive]
pub enum CheckpointPolicyError {
    /// N must be greater than 0 for every_n policy.
    #[error("CheckpointPolicy::every_n requires n > 0, got {n}")]
    InvalidN {
        /// The invalid value that was provided.
        n: usize,
    },
}

impl CheckpointPolicy {
    /// Create a policy that checkpoints every N nodes.
    ///
    /// # Panics
    ///
    /// Panics if `n` is 0.
    // ALLOW: Intentional panic for invalid argument (documented in # Panics); use try_every_n for fallible version
    #[allow(clippy::expect_used)]
    pub fn every_n(n: usize) -> Self {
        Self::try_every_n(n).expect("CheckpointPolicy::every_n requires n > 0")
    }

    /// Create a policy that checkpoints every N nodes, returning an error if `n` is 0.
    ///
    /// # Errors
    ///
    /// Returns `CheckpointPolicyError::InvalidN` if `n` is 0.
    pub fn try_every_n(n: usize) -> std::result::Result<Self, CheckpointPolicyError> {
        if n == 0 {
            return Err(CheckpointPolicyError::InvalidN { n });
        }
        Ok(Self::EveryN(n))
    }

    /// Create a policy that only checkpoints at marked nodes.
    pub fn on_markers<I, S>(markers: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        Self::OnMarkers(markers.into_iter().map(|s| s.into()).collect())
    }

    /// Create a policy that checkpoints on significant state changes.
    pub fn on_state_change(min_delta: usize) -> Self {
        Self::OnStateChange { min_delta }
    }

    /// Check if a checkpoint should be taken given the current context.
    ///
    /// # Arguments
    ///
    /// * `node` - Name of the node that just executed
    /// * `node_count` - Total number of nodes executed so far (1-indexed)
    /// * `state_size` - Current serialized state size in bytes
    /// * `last_checkpoint_size` - State size at last checkpoint (for delta calculation)
    pub fn should_checkpoint(
        &self,
        node: &str,
        node_count: usize,
        state_size: usize,
        last_checkpoint_size: usize,
    ) -> bool {
        match self {
            Self::Every => true,
            Self::EveryN(n) => node_count % n == 0,
            Self::OnMarkers(markers) => markers.contains(node),
            Self::OnStateChange { min_delta } => {
                state_size.abs_diff(last_checkpoint_size) >= *min_delta
            }
            Self::Never => false,
        }
    }
}

/// A checkpoint representing graph state at a specific point in execution.
///
/// Checkpoints enable:
/// - **Resume from failures** - Restart graph execution from last checkpoint
/// - **Human-in-the-loop** - Pause, inspect state, then continue
/// - **Debugging** - Examine state at any point in execution history
/// - **Audit trails** - Track all state transitions
///
/// Each checkpoint captures the full state, the node being executed,
/// and links to parent checkpoints for history traversal.
///
/// # Example
///
/// ```rust,ignore
/// use dashflow::checkpoint::{Checkpoint, MemoryCheckpointer, Checkpointer};
/// use dashflow::GraphState;
///
/// #[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
/// struct MyState {
///     step: u32,
///     data: String,
/// }
///
/// // Create a checkpoint manually
/// let checkpoint = Checkpoint::new(
///     "thread-123".to_string(),
///     MyState { step: 1, data: "processing".to_string() },
///     "process_node".to_string(),
///     None, // No parent (first checkpoint)
/// );
///
/// // Add metadata for debugging
/// let checkpoint = checkpoint
///     .with_metadata("user_id", "alice")
///     .with_metadata("request_id", "req-456");
///
/// // Save to checkpointer
/// let checkpointer = MemoryCheckpointer::new();
/// checkpointer.save(checkpoint).await?;
/// ```
///
/// # See Also
///
/// - [`Checkpointer`] - Trait for checkpoint storage backends
/// - [`CheckpointMetadata`] - Lightweight checkpoint info without state
/// - [`MemoryCheckpointer`] - In-memory checkpointer for testing
/// - [`SqliteCheckpointer`] - Persistent SQLite-based checkpointer
/// - [`CompiledGraph::with_checkpointer`](crate::CompiledGraph::with_checkpointer) - Enable checkpointing on graphs
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(bound(
    serialize = "S: Serialize",
    deserialize = "S: for<'de2> Deserialize<'de2>"
))]
pub struct Checkpoint<S: GraphState> {
    /// Unique identifier for this checkpoint (auto-generated)
    pub id: CheckpointId,

    /// Thread/execution ID this checkpoint belongs to
    pub thread_id: ThreadId,

    /// The complete graph state at this point
    pub state: S,

    /// Node that was just executed (or about to be executed)
    pub node: String,

    /// Timestamp when checkpoint was created
    #[serde(with = "systemtime_serde")]
    pub timestamp: SystemTime,

    /// Parent checkpoint ID (for tracking execution history)
    pub parent_id: Option<CheckpointId>,

    /// User-defined metadata about this checkpoint
    pub metadata: HashMap<String, String>,
}

impl<S: GraphState> Checkpoint<S> {
    /// Create a new checkpoint
    pub fn new(
        thread_id: ThreadId,
        state: S,
        node: String,
        parent_id: Option<CheckpointId>,
    ) -> Self {
        // Generate checkpoint ID using thread-local counter instead of UUID
        // This eliminates getentropy() overhead (20% of checkpoint time per N=900 flamegraph)
        // Includes process-unique ID to prevent collisions across process restarts
        let counter = CHECKPOINT_COUNTER.with(|c| {
            let current = c.get();
            c.set(current.wrapping_add(1));
            current
        });
        let process_id = get_process_unique_id();
        let id = format!("{thread_id}_{process_id}_chkpt{counter}");

        Self {
            id,
            thread_id,
            state,
            node,
            timestamp: SystemTime::now(),
            parent_id,
            metadata: HashMap::new(),
        }
    }

    /// Add metadata to this checkpoint
    #[must_use]
    pub fn with_metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }
}

/// Metadata about a checkpoint (without the full state).
///
/// A lightweight view of checkpoint information useful for listing and
/// browsing checkpoints without loading the full (potentially large) state.
/// Returned by [`Checkpointer::list`] for efficient checkpoint enumeration.
///
/// # Example
///
/// ```rust,ignore
/// use dashflow::checkpoint::{Checkpointer, MemoryCheckpointer};
///
/// let checkpointer = MemoryCheckpointer::new();
///
/// // List checkpoints for a thread (returns metadata only)
/// let checkpoints = checkpointer.list("thread-123").await?;
/// for meta in &checkpoints {
///     println!(
///         "Checkpoint {} at node '{}' ({})",
///         meta.id, meta.node, meta.timestamp.elapsed().unwrap().as_secs()
///     );
/// }
///
/// // Load full checkpoint when needed
/// if let Some(meta) = checkpoints.first() {
///     let full_checkpoint = checkpointer.load(&meta.id).await?;
/// }
/// ```
///
/// # See Also
///
/// - [`Checkpoint`] - Full checkpoint with state
/// - [`Checkpointer::list`] - Returns metadata for checkpoint browsing
/// - [`ThreadInfo`] - Summary info about a thread/session
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CheckpointMetadata {
    /// Unique checkpoint identifier
    pub id: CheckpointId,
    /// Thread/session this checkpoint belongs to
    pub thread_id: ThreadId,
    /// Node that was executed at this checkpoint
    pub node: String,
    /// When the checkpoint was created
    #[serde(with = "systemtime_serde")]
    pub timestamp: SystemTime,
    /// Parent checkpoint for history traversal
    pub parent_id: Option<CheckpointId>,
    /// User-defined key-value metadata
    pub metadata: HashMap<String, String>,
}

/// Information about a thread (session) stored in the checkpointer
///
/// Used by `list_threads()` to enumerate available sessions without
/// loading full checkpoint data.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ThreadInfo {
    /// The thread/session identifier
    pub thread_id: ThreadId,
    /// ID of the most recent checkpoint for this thread
    pub latest_checkpoint_id: CheckpointId,
    /// Timestamp of the most recent checkpoint
    #[serde(with = "systemtime_serde")]
    pub updated_at: SystemTime,
    /// Total number of checkpoints for this thread (if known)
    pub checkpoint_count: Option<usize>,
}

impl<S: GraphState> From<&Checkpoint<S>> for CheckpointMetadata {
    fn from(checkpoint: &Checkpoint<S>) -> Self {
        Self {
            id: checkpoint.id.clone(),
            thread_id: checkpoint.thread_id.clone(),
            node: checkpoint.node.clone(),
            timestamp: checkpoint.timestamp,
            parent_id: checkpoint.parent_id.clone(),
            metadata: checkpoint.metadata.clone(),
        }
    }
}

/// Trait for checkpoint persistence strategies
///
/// Implementations can store checkpoints in memory, on disk, in databases, etc.
///
/// # Required Methods
///
/// - `save` - Persist a checkpoint
/// - `load` - Load a specific checkpoint by ID
/// - `list` - List checkpoint metadata for a thread
/// - `delete` - Delete a specific checkpoint
///
/// # Methods with Default Implementations
///
/// - `get_latest` - Returns the most recent checkpoint (defaults to `list` + `load`)
/// - `delete_thread` - Deletes all checkpoints for a thread (defaults to `list` + `delete` loop)
/// - `list_threads` - Lists all threads (defaults to `NotImplemented` error)
///
/// Override the default implementations for better performance in your storage backend.
#[async_trait::async_trait]
pub trait Checkpointer<S: GraphState>: Send + Sync {
    /// Save a checkpoint
    async fn save(&self, checkpoint: Checkpoint<S>) -> Result<()>;

    /// Load a specific checkpoint by ID
    async fn load(&self, checkpoint_id: &str) -> Result<Option<Checkpoint<S>>>;

    /// Get the latest checkpoint for a thread
    ///
    /// Default implementation calls `list()` to get metadata, then `load()` with the
    /// first (most recent) checkpoint's ID. Override for better performance if your
    /// storage backend supports direct latest-checkpoint queries.
    async fn get_latest(&self, thread_id: &str) -> Result<Option<Checkpoint<S>>> {
        let metadata_list = self.list(thread_id).await?;
        match metadata_list.first() {
            Some(metadata) => self.load(&metadata.id).await,
            None => Ok(None),
        }
    }

    /// List all checkpoints for a thread (ordered by timestamp, newest first)
    async fn list(&self, thread_id: &str) -> Result<Vec<CheckpointMetadata>>;

    /// Delete a checkpoint
    async fn delete(&self, checkpoint_id: &str) -> Result<()>;

    /// Delete all checkpoints for a thread
    ///
    /// Default implementation calls `list()` then `delete()` for each checkpoint.
    /// Override for better performance if your storage backend supports batch deletes
    /// or prefix-based deletion.
    async fn delete_thread(&self, thread_id: &str) -> Result<()> {
        let metadata_list = self.list(thread_id).await?;
        for metadata in metadata_list {
            self.delete(&metadata.id).await?;
        }
        Ok(())
    }

    /// List all threads (sessions) that have checkpoints
    ///
    /// Returns a list of thread information sorted by most recently updated first.
    /// This is useful for implementing session pickers and management UIs.
    ///
    /// Default implementation returns an error. Override this method
    /// if your storage backend can efficiently enumerate threads.
    async fn list_threads(&self) -> Result<Vec<ThreadInfo>> {
        Err(crate::Error::Generic(
            "list_threads not implemented for this checkpointer".to_string(),
        ))
    }
}

/// In-memory checkpoint storage
///
/// Useful for testing and short-lived workflows.
/// Does not persist across process restarts.
#[derive(Clone)]
pub struct MemoryCheckpointer<S: GraphState> {
    checkpoints: Arc<Mutex<HashMap<CheckpointId, Checkpoint<S>>>>,
}

impl<S: GraphState> MemoryCheckpointer<S> {
    /// Create a new memory checkpointer
    #[must_use]
    pub fn new() -> Self {
        Self {
            checkpoints: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Get the number of stored checkpoints
    #[must_use]
    pub fn len(&self) -> usize {
        self.checkpoints
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .len()
    }

    /// Check if the checkpointer is empty
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.checkpoints
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .is_empty()
    }

    /// Clear all checkpoints
    pub fn clear(&self) {
        self.checkpoints
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clear();
    }
}

impl<S: GraphState> Default for MemoryCheckpointer<S> {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl<S: GraphState> Checkpointer<S> for MemoryCheckpointer<S> {
    async fn save(&self, checkpoint: Checkpoint<S>) -> Result<()> {
        let mut checkpoints = self.checkpoints.lock().unwrap_or_else(|e| e.into_inner());
        checkpoints.insert(checkpoint.id.clone(), checkpoint);
        Ok(())
    }

    async fn load(&self, checkpoint_id: &str) -> Result<Option<Checkpoint<S>>> {
        let checkpoints = self.checkpoints.lock().unwrap_or_else(|e| e.into_inner());
        Ok(checkpoints.get(checkpoint_id).cloned())
    }

    async fn get_latest(&self, thread_id: &str) -> Result<Option<Checkpoint<S>>> {
        let checkpoints = self.checkpoints.lock().unwrap_or_else(|e| e.into_inner());
        let mut thread_checkpoints: Vec<_> = checkpoints
            .values()
            .filter(|cp| cp.thread_id == thread_id)
            .collect();

        // Sort by timestamp DESC, then by ID DESC for stable ordering
        // This ensures that if two checkpoints have the same timestamp (can happen
        // when saved within same millisecond), we use the checkpoint ID as tiebreaker.
        // Checkpoint IDs contain a monotonic counter, so higher ID = more recent.
        thread_checkpoints
            .sort_by(|a, b| b.timestamp.cmp(&a.timestamp).then_with(|| b.id.cmp(&a.id)));
        Ok(thread_checkpoints.first().map(|cp| (*cp).clone()))
    }

    async fn list(&self, thread_id: &str) -> Result<Vec<CheckpointMetadata>> {
        let checkpoints = self.checkpoints.lock().unwrap_or_else(|e| e.into_inner());
        let mut thread_checkpoints: Vec<_> = checkpoints
            .values()
            .filter(|cp| cp.thread_id == thread_id)
            .map(CheckpointMetadata::from)
            .collect();

        // Sort by timestamp DESC, then by ID DESC for stable ordering
        thread_checkpoints
            .sort_by(|a, b| b.timestamp.cmp(&a.timestamp).then_with(|| b.id.cmp(&a.id)));
        Ok(thread_checkpoints)
    }

    async fn delete(&self, checkpoint_id: &str) -> Result<()> {
        let mut checkpoints = self.checkpoints.lock().unwrap_or_else(|e| e.into_inner());
        checkpoints.remove(checkpoint_id);
        Ok(())
    }

    async fn delete_thread(&self, thread_id: &str) -> Result<()> {
        let mut checkpoints = self.checkpoints.lock().unwrap_or_else(|e| e.into_inner());
        checkpoints.retain(|_, cp| cp.thread_id != thread_id);
        Ok(())
    }

    async fn list_threads(&self) -> Result<Vec<ThreadInfo>> {
        let checkpoints = self.checkpoints.lock().unwrap_or_else(|e| e.into_inner());

        // Group checkpoints by thread_id and find latest for each
        let mut threads: HashMap<ThreadId, (CheckpointId, SystemTime, usize)> = HashMap::new();
        for cp in checkpoints.values() {
            let entry =
                threads
                    .entry(cp.thread_id.clone())
                    .or_insert((cp.id.clone(), cp.timestamp, 0));
            entry.2 += 1; // Increment count

            // Update if this checkpoint is newer
            let is_newer = cp.timestamp > entry.1 || (cp.timestamp == entry.1 && cp.id > entry.0);
            if is_newer {
                entry.0 = cp.id.clone();
                entry.1 = cp.timestamp;
            }
        }

        // Convert to ThreadInfo and sort by updated_at DESC
        let mut thread_infos: Vec<ThreadInfo> = threads
            .into_iter()
            .map(
                |(thread_id, (checkpoint_id, updated_at, count))| ThreadInfo {
                    thread_id,
                    latest_checkpoint_id: checkpoint_id,
                    updated_at,
                    checkpoint_count: Some(count),
                },
            )
            .collect();

        thread_infos.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
        Ok(thread_infos)
    }
}

/// File-based checkpoint storage
///
/// Stores checkpoints as bincode-encoded files in a directory with buffered I/O.
/// Uses an index file for O(1) latest checkpoint lookup.
/// Thread-safe and persists across process restarts.
pub struct FileCheckpointer<S: GraphState> {
    directory: std::path::PathBuf,
    // Index: thread_id -> (checkpoint_id, timestamp)
    index: Arc<Mutex<HashMap<ThreadId, (CheckpointId, SystemTime)>>>,
    _phantom: std::marker::PhantomData<S>,
}

impl<S: GraphState> FileCheckpointer<S> {
    /// Create a new file checkpointer (synchronous)
    ///
    /// Creates the directory if it doesn't exist.
    /// Loads the index file if it exists.
    ///
    /// # Note
    ///
    /// This constructor performs blocking filesystem operations. If called from an
    /// async context, consider using [`Self::new_async`] instead to avoid blocking the
    /// executor.
    pub fn new(directory: impl Into<std::path::PathBuf>) -> Result<Self> {
        let directory = directory.into();
        std::fs::create_dir_all(&directory)
            .map_err(|e| crate::Error::Checkpoint(crate::error::CheckpointError::Io(e)))?;

        // Load index from file if it exists (with proper error logging)
        let index_path = directory.join("index.bin");
        let index = load_checkpoint_index(&index_path);

        Ok(Self {
            directory,
            index: Arc::new(Mutex::new(index)),
            _phantom: std::marker::PhantomData,
        })
    }

    /// Create a new file checkpointer (async)
    ///
    /// Creates the directory if it doesn't exist.
    /// Loads the index file if it exists.
    ///
    /// This is the preferred constructor when called from an async context,
    /// as it avoids blocking the executor on filesystem operations.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use dashflow::checkpoint::FileCheckpointer;
    ///
    /// // MyState automatically implements GraphState via blanket impl
    /// // because it meets the required bounds (Clone + Send + Sync + Serialize + Deserialize)
    /// #[derive(Default, Clone, Debug, serde::Serialize, serde::Deserialize, PartialEq)]
    /// struct MyState { count: i32 }
    ///
    /// # async fn example() -> dashflow::Result<()> {
    /// let checkpointer = FileCheckpointer::<MyState>::new_async("./checkpoints").await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn new_async(directory: impl Into<std::path::PathBuf>) -> Result<Self> {
        let directory = directory.into();

        // Use tokio::fs for non-blocking directory creation
        tokio::fs::create_dir_all(&directory)
            .await
            .map_err(|e| crate::Error::Checkpoint(crate::error::CheckpointError::Io(e)))?;

        // Load index in a blocking task to avoid blocking the executor
        let index_path = directory.join("index.bin");
        let index = tokio::task::spawn_blocking(move || load_checkpoint_index(&index_path))
            .await
            .map_err(|e| {
                crate::Error::Checkpoint(crate::error::CheckpointError::DeserializationFailed {
                    reason: format!("Failed to load checkpoint index: {e}"),
                })
            })?;

        Ok(Self {
            directory,
            index: Arc::new(Mutex::new(index)),
            _phantom: std::marker::PhantomData,
        })
    }

    /// Get the file path for a checkpoint ID
    fn checkpoint_path(&self, checkpoint_id: &str) -> std::path::PathBuf {
        self.directory.join(format!("{checkpoint_id}.bin"))
    }

    /// Get the index file path
    fn index_path(&self) -> std::path::PathBuf {
        self.directory.join("index.bin")
    }

    /// Save the index to disk (async to avoid blocking)
    ///
    /// Uses cross-process file locking to prevent concurrent index corruption
    /// when multiple processes share the same checkpoint directory .
    async fn save_index(&self) -> Result<()> {
        // Serialize while holding the in-process lock, then drop guard before await
        let data = {
            let index = self.index.lock().unwrap_or_else(|e| e.into_inner());
            bincode::serialize(&*index).map_err(|e| {
                crate::Error::Checkpoint(crate::error::CheckpointError::SerializationFailed {
                    reason: format!("Failed to serialize checkpoint index: {e}"),
                })
            })?
        };
        let index_path = self.index_path();
        let directory = self.directory.clone();

        // Acquire cross-process lock and write atomically in a blocking task
        tokio::task::spawn_blocking(move || {
            // Acquire exclusive lock for cross-process safety
            let _lock = acquire_exclusive_lock(&directory).map_err(|e| {
                crate::Error::Checkpoint(crate::error::CheckpointError::LockFailed {
                    path: directory.display().to_string(),
                    reason: e.to_string(),
                })
            })?;

            // Write atomically (blocking version for spawn_blocking)
            use std::io::Write;
            // Use UUID v4 for unpredictable temp file names (security hardening)
            let temp_name = format!(".index.{}.tmp", uuid::Uuid::new_v4());
            let temp_path = index_path.with_file_name(&temp_name);

            // Write to temp file
            let mut file = std::fs::File::create(&temp_path)
                .map_err(|e| crate::Error::Checkpoint(crate::error::CheckpointError::Io(e)))?;
            file.write_all(&data)
                .map_err(|e| crate::Error::Checkpoint(crate::error::CheckpointError::Io(e)))?;
            file.sync_all()
                .map_err(|e| crate::Error::Checkpoint(crate::error::CheckpointError::Io(e)))?;

            // Atomic rename
            std::fs::rename(&temp_path, &index_path)
                .map_err(|e| crate::Error::Checkpoint(crate::error::CheckpointError::Io(e)))?;

            Ok::<_, crate::Error>(())
            // _lock dropped here, releasing the cross-process lock
        })
        .await
        .map_err(|e| {
            crate::Error::Checkpoint(crate::error::CheckpointError::Other(format!(
                "Task join error saving index: {e}"
            )))
        })??;

        Ok(())
    }

    /// List all checkpoint files (async to avoid blocking)
    async fn list_files(&self) -> Result<Vec<std::path::PathBuf>> {
        let directory = self.directory.clone();
        tokio::task::spawn_blocking(move || {
            let entries = std::fs::read_dir(&directory)
                .map_err(|e| crate::Error::Checkpoint(crate::error::CheckpointError::Io(e)))?;

            let mut files = Vec::new();
            for entry in entries {
                let entry = entry
                    .map_err(|e| crate::Error::Checkpoint(crate::error::CheckpointError::Io(e)))?;
                let path = entry.path();
                let ext = path.extension().and_then(|s| s.to_str());
                // Include both .bin (new format) and .json (legacy format for backward compatibility)
                if ext == Some("bin") && path.file_name() != Some(std::ffi::OsStr::new("index.bin"))
                {
                    files.push(path);
                }
            }
            Ok(files)
        })
        .await
        .map_err(|e| {
            crate::Error::Checkpoint(crate::error::CheckpointError::Other(format!(
                "Task join error listing checkpoint files: {e}"
            )))
        })?
    }

    /// Read and deserialize a checkpoint from a file
    ///
    /// This helper method encapsulates the common logic for reading checkpoint files
    /// with buffered I/O, integrity verification, and deserializing from bincode format.
    ///
    /// # Parameters
    /// - `file`: Path to the checkpoint file to read
    ///
    /// # Returns
    /// - `Ok(Checkpoint<S>)`: Successfully deserialized checkpoint
    /// - `Err(...)`: I/O, integrity, or deserialization error
    async fn read_checkpoint_from_file(file: std::path::PathBuf) -> Result<Checkpoint<S>> {
        let file_display = file.display().to_string();
        tokio::task::spawn_blocking(move || {
            use std::io::Read;
            let f = std::fs::File::open(&file)
                .map_err(|e| crate::Error::Checkpoint(crate::error::CheckpointError::Io(e)))?;
            let mut reader = std::io::BufReader::new(f);
            let mut data = Vec::new();
            reader
                .read_to_end(&mut data)
                .map_err(|e| crate::Error::Checkpoint(crate::error::CheckpointError::Io(e)))?;

            // Verify integrity if checkpoint has integrity header
            // Backward compatibility: old files without header are deserialized directly
            let payload: &[u8] = if CheckpointWithIntegrity::is_wrapped(&data) {
                CheckpointWithIntegrity::unwrap(&data).map_err(|e| {
                    crate::Error::Checkpoint(crate::error::CheckpointError::IntegrityCheckFailed {
                        checkpoint_id: file.display().to_string(),
                        reason: e.to_string(),
                    })
                })?
            } else {
                &data
            };

            bincode::deserialize(payload).map_err(|e| {
                crate::Error::Checkpoint(crate::error::CheckpointError::DeserializationFailed {
                    reason: format!(
                        "Failed to deserialize checkpoint from '{}': {e}",
                        file.display()
                    ),
                })
            })
        })
        .await
        .map_err(|e| {
            crate::Error::Checkpoint(crate::error::CheckpointError::Other(format!(
                "Task join error reading checkpoint '{}': {e}",
                file_display
            )))
        })?
    }

    /// Fallback: Find the latest checkpoint by scanning files and sorting by timestamp.
    /// Used when index is corrupted, reset, or points to a missing file .
    async fn get_latest_by_file_scan(&self, thread_id: &str) -> Result<Option<Checkpoint<S>>> {
        let files = self.list_files().await?;
        let mut latest: Option<Checkpoint<S>> = None;

        for file in files {
            match Self::read_checkpoint_from_file(file.clone()).await {
                Ok(checkpoint) => {
                    if checkpoint.thread_id == thread_id {
                        match &latest {
                            None => latest = Some(checkpoint),
                            Some(current) => {
                                // Pick newer by timestamp, then by ID for tie-breaking
                                if checkpoint.timestamp > current.timestamp
                                    || (checkpoint.timestamp == current.timestamp
                                        && checkpoint.id > current.id)
                                {
                                    latest = Some(checkpoint);
                                }
                            }
                        }
                    }
                }
                Err(e) => {
                    tracing::warn!(file = %file.display(), "Skipping corrupt file in recovery scan: {e}");
                }
            }
        }

        // If we found a latest checkpoint via file scan, update the index for future lookups
        if let Some(ref checkpoint) = latest {
            let mut index = self.index.lock().unwrap_or_else(|e| e.into_inner());
            index.insert(
                thread_id.to_string(),
                (checkpoint.id.clone(), checkpoint.timestamp),
            );
            // Note: Index is saved to disk on next save() call
        }

        Ok(latest)
    }
}

#[async_trait::async_trait]
impl<S: GraphState> Checkpointer<S> for FileCheckpointer<S> {
    async fn save(&self, checkpoint: Checkpoint<S>) -> Result<()> {
        let path = self.checkpoint_path(&checkpoint.id);
        let path_display = path.display().to_string();
        let thread_id = checkpoint.thread_id.clone();
        let checkpoint_id = checkpoint.id.clone();
        let timestamp = checkpoint.timestamp;

        // Clone checkpoint_id for use inside closure (original needed for index update)
        let checkpoint_id_for_err = checkpoint_id.clone();

        // Serialize + wrap + write atomically in spawn_blocking (M-635: CPU-intensive bincode)
        tokio::task::spawn_blocking(move || {
            // Serialize with bincode (2-5x faster than JSON)
            let serialized = bincode::serialize(&checkpoint).map_err(|e| {
                crate::Error::Checkpoint(crate::error::CheckpointError::SerializationFailed {
                    reason: format!("Failed to serialize checkpoint '{}': {e}", checkpoint_id_for_err),
                })
            })?;

            // Wrap with integrity header (magic + version + CRC32 + length)
            let data = CheckpointWithIntegrity::wrap(&serialized);

            // Write atomically to prevent corruption on crash/power loss (M-239)
            atomic_write_file_sync(&path, &data)
                .map_err(|e| crate::Error::Checkpoint(crate::error::CheckpointError::Io(e)))?;
            Ok::<_, crate::Error>(())
        })
        .await
        .map_err(|e| {
            crate::Error::Checkpoint(crate::error::CheckpointError::Other(format!(
                "Task join error saving checkpoint '{}': {e}",
                path_display
            )))
        })??;

        // Update index for O(1) get_latest()
        {
            let mut index = self.index.lock().unwrap_or_else(|e| e.into_inner());
            let entry = index
                .entry(thread_id)
                .or_insert((checkpoint_id.clone(), timestamp));
            // Only update if this checkpoint is newer (by timestamp, then by ID for tie-breaking)
            // Checkpoint IDs contain a monotonic counter, so higher ID = more recent
            let is_newer = timestamp > entry.1 || (timestamp == entry.1 && checkpoint_id > entry.0);
            if is_newer {
                *entry = (checkpoint_id, timestamp);
            }
        }

        // Save index to disk
        self.save_index().await?;

        Ok(())
    }

    async fn load(&self, checkpoint_id: &str) -> Result<Option<Checkpoint<S>>> {
        let path = self.checkpoint_path(checkpoint_id);
        // Use tokio::fs::try_exists for non-blocking path existence check (M-633)
        let exists = tokio::fs::try_exists(&path)
            .await
            .map_err(|e| crate::Error::Checkpoint(crate::error::CheckpointError::Io(e)))?;
        if !exists {
            return Ok(None);
        }

        let path_display = path.display().to_string();
        // Read with buffered I/O and verify integrity
        let checkpoint = tokio::task::spawn_blocking(move || {
            use std::io::Read;
            let file = std::fs::File::open(&path).map_err(|e| {
                crate::Error::Checkpoint(crate::error::CheckpointError::Io(e))
            })?;
            let mut reader = std::io::BufReader::new(file);
            let mut data = Vec::new();
            reader.read_to_end(&mut data).map_err(|e| {
                crate::Error::Checkpoint(crate::error::CheckpointError::Io(e))
            })?;

            // Verify integrity and unwrap if checkpoint has integrity header
            // Backward compatibility: old files without header are deserialized directly
            let payload = if CheckpointWithIntegrity::is_wrapped(&data) {
                CheckpointWithIntegrity::unwrap(&data).map_err(|e| {
                    crate::Error::Checkpoint(crate::error::CheckpointError::IntegrityCheckFailed {
                        checkpoint_id: path.display().to_string(),
                        reason: e.to_string(),
                    })
                })?
            } else {
                // Legacy file without integrity header - warn and proceed
                tracing::warn!(
                    path = %path.display(),
                    "Loading legacy checkpoint without integrity header. Consider re-saving to add integrity checks."
                );
                &data
            };

            // Deserialize with bincode
            bincode::deserialize(payload).map_err(|e| {
                crate::Error::Checkpoint(crate::error::CheckpointError::DeserializationFailed {
                    reason: format!("Failed to deserialize checkpoint from '{}': {e}", path.display()),
                })
            })
        })
        .await
        .map_err(|e| crate::Error::Checkpoint(crate::error::CheckpointError::Other(format!("Task join error loading checkpoint '{}': {e}", path_display))))??;

        Ok(Some(checkpoint))
    }

    async fn get_latest(&self, thread_id: &str) -> Result<Option<Checkpoint<S>>> {
        // O(1) lookup using index instead of O(n) file scanning
        let checkpoint_id = {
            let index = self.index.lock().unwrap_or_else(|e| e.into_inner());
            index.get(thread_id).map(|(id, _)| id.clone())
        };

        match checkpoint_id {
            Some(id) => {
                // Try to load the indexed checkpoint
                if let Some(checkpoint) = self.load(&id).await? {
                    return Ok(Some(checkpoint));
                }
                // Index pointed to missing/corrupt file - fall back to file scan
                tracing::warn!(
                    thread_id = thread_id,
                    indexed_checkpoint = %id,
                    "Index pointed to missing checkpoint, falling back to file scan"
                );
                self.get_latest_by_file_scan(thread_id).await
            }
            None => {
                // No index entry - fall back to file scan to recover from index reset
                self.get_latest_by_file_scan(thread_id).await
            }
        }
    }

    async fn list(&self, thread_id: &str) -> Result<Vec<CheckpointMetadata>> {
        let files = self.list_files().await?;
        let mut checkpoints = Vec::new();

        for file in files {
            // Skip corrupt files instead of aborting entire list operation
            // This ensures one bad file doesn't break checkpoint listing/recovery
            match Self::read_checkpoint_from_file(file.clone()).await {
                Ok(checkpoint) => {
                    if checkpoint.thread_id == thread_id {
                        checkpoints.push(CheckpointMetadata::from(&checkpoint));
                    }
                }
                Err(e) => {
                    tracing::warn!(
                        file = %file.display(),
                        "Skipping corrupt checkpoint file: {e}"
                    );
                    // Continue processing other files
                }
            }
        }

        // Sort by timestamp DESC, then by ID DESC for stable ordering
        checkpoints.sort_by(|a, b| b.timestamp.cmp(&a.timestamp).then_with(|| b.id.cmp(&a.id)));
        Ok(checkpoints)
    }

    async fn delete(&self, checkpoint_id: &str) -> Result<()> {
        let path = self.checkpoint_path(checkpoint_id);
        // Use tokio::fs::try_exists for non-blocking path existence check (M-633)
        let exists = tokio::fs::try_exists(&path)
            .await
            .map_err(|e| crate::Error::Checkpoint(crate::error::CheckpointError::Io(e)))?;
        if exists {
            tokio::fs::remove_file(&path)
                .await
                .map_err(|e| crate::Error::Checkpoint(crate::error::CheckpointError::Io(e)))?;
        }

        // Update index: remove entry if it points to this checkpoint
        {
            let mut index = self.index.lock().unwrap_or_else(|e| e.into_inner());
            index.retain(|_, (id, _)| id != checkpoint_id);
        }
        self.save_index().await?;

        Ok(())
    }

    async fn delete_thread(&self, thread_id: &str) -> Result<()> {
        let files = self.list_files().await?;

        for file in files {
            let checkpoint = Self::read_checkpoint_from_file(file.clone()).await?;

            if checkpoint.thread_id == thread_id {
                tokio::fs::remove_file(&file)
                    .await
                    .map_err(|e| crate::Error::Checkpoint(crate::error::CheckpointError::Io(e)))?;
            }
        }

        // Update index: remove entry for this thread
        {
            let mut index = self.index.lock().unwrap_or_else(|e| e.into_inner());
            index.remove(thread_id);
        }
        self.save_index().await?;

        Ok(())
    }

    async fn list_threads(&self) -> Result<Vec<ThreadInfo>> {
        // Use the index for O(1) thread listing - it already tracks thread_id -> (checkpoint_id, timestamp)
        let index = self.index.lock().unwrap_or_else(|e| e.into_inner());

        let mut thread_infos: Vec<ThreadInfo> = index
            .iter()
            .map(|(thread_id, (checkpoint_id, timestamp))| ThreadInfo {
                thread_id: thread_id.clone(),
                latest_checkpoint_id: checkpoint_id.clone(),
                updated_at: *timestamp,
                checkpoint_count: None, // Would require scanning files to get accurate count
            })
            .collect();

        // Sort by updated_at DESC (most recent first)
        thread_infos.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
        Ok(thread_infos)
    }
}

/// Serde support for `SystemTime`
mod systemtime_serde {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};
    use std::time::{SystemTime, UNIX_EPOCH};

    #[derive(Serialize, Deserialize)]
    struct SystemTimeRepr {
        secs: u64,
        nanos: u32,
    }

    pub fn serialize<S>(time: &SystemTime, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let duration = time
            .duration_since(UNIX_EPOCH)
            .map_err(serde::ser::Error::custom)?;
        let repr = SystemTimeRepr {
            secs: duration.as_secs(),
            nanos: duration.subsec_nanos(),
        };
        repr.serialize(serializer)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> std::result::Result<SystemTime, D::Error>
    where
        D: Deserializer<'de>,
    {
        let repr = SystemTimeRepr::deserialize(deserializer)?;
        Ok(UNIX_EPOCH + std::time::Duration::new(repr.secs, repr.nanos))
    }
}

#[cfg(test)]
mod tests;
