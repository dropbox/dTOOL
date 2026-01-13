// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

//! Error types for DashFlow
//!
//! This module provides actionable error messages for AI agents. All errors
//! include:
//! 1. What went wrong
//! 2. Why it's a problem
//! 3. How to fix it (with code snippets when applicable)

use std::fmt;
use thiserror::Error;

/// An actionable suggestion for fixing an error, including optional code snippets.
///
/// AI agents can use this to understand how to fix issues without searching documentation.
///
/// # Example
///
/// ```rust
/// use dashflow::error::ActionableSuggestion;
///
/// let suggestion = ActionableSuggestion::new(
///     "Implement MergeableState for your state type"
/// ).with_code_snippet(r#"
/// impl MergeableState for YourState {
///     fn merge(&mut self, other: &Self) {
///         // Merge logic here
///     }
/// }
/// "#);
///
/// println!("{}", suggestion);
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActionableSuggestion {
    /// Human-readable explanation of the fix
    pub description: String,
    /// Optional code snippet showing the fix
    pub code_snippet: Option<String>,
    /// Related documentation URL (if any)
    pub doc_url: Option<String>,
}

impl ActionableSuggestion {
    /// Create a new suggestion with just a description
    pub fn new(description: impl Into<String>) -> Self {
        Self {
            description: description.into(),
            code_snippet: None,
            doc_url: None,
        }
    }

    /// Add a code snippet to the suggestion
    #[must_use]
    pub fn with_code_snippet(mut self, snippet: impl Into<String>) -> Self {
        self.code_snippet = Some(snippet.into());
        self
    }

    /// Add a documentation URL to the suggestion
    #[must_use]
    pub fn with_doc_url(mut self, url: impl Into<String>) -> Self {
        self.doc_url = Some(url.into());
        self
    }
}

impl fmt::Display for ActionableSuggestion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.description)?;
        if let Some(snippet) = &self.code_snippet {
            write!(f, "\n\n```rust{}\n```", snippet)?;
        }
        if let Some(url) = &self.doc_url {
            write!(f, "\n\nSee: {}", url)?;
        }
        Ok(())
    }
}

/// Trait for errors that provide actionable suggestions with code snippets.
///
/// AI agents can call `suggestion()` to get detailed fix instructions.
///
/// # Example
///
/// ```rust
/// use dashflow::error::{Error, ActionableError};
///
/// fn handle_error(err: &Error) {
///     eprintln!("Error: {}", err);
///     if let Some(suggestion) = err.suggestion() {
///         eprintln!("\nHow to fix:\n{}", suggestion);
///     }
/// }
/// ```
pub trait ActionableError {
    /// Returns an actionable suggestion for fixing this error, if available.
    fn suggestion(&self) -> Option<ActionableSuggestion>;

    /// Returns true if this error has an actionable suggestion.
    fn has_suggestion(&self) -> bool {
        self.suggestion().is_some()
    }

    /// Formats the error with its suggestion for display.
    fn format_with_suggestion(&self) -> String
    where
        Self: fmt::Display,
    {
        let base = self.to_string();
        match self.suggestion() {
            Some(suggestion) => format!("{}\n\nHow to fix:\n{}", base, suggestion),
            None => base,
        }
    }
}

/// Checkpoint-specific error types for explicit failure handling
///
/// Provides typed errors for checkpoint operations, enabling pattern matching
/// and specific handling of different failure modes.
///
/// # Example
///
/// ```rust
/// use dashflow::error::CheckpointError;
///
/// fn handle_checkpoint_error(err: CheckpointError) {
///     match err {
///         CheckpointError::StorageFull { path, available, required } => {
///             eprintln!("Storage full at {}: need {} bytes, only {} available", path, required, available);
///         }
///         CheckpointError::ConnectionLost { backend, reason } => {
///             eprintln!("Connection to {} lost: {} - will retry", backend, reason);
///         }
///         CheckpointError::SerializationFailed { reason } => {
///             eprintln!("Bug: serialization failed: {}", reason);
///         }
///         _ => eprintln!("Checkpoint error: {}", err),
///     }
/// }
/// ```
#[non_exhaustive]
#[derive(Error, Debug)]
pub enum CheckpointError {
    /// Storage is full - cannot write checkpoint
    #[error("Storage full at '{path}': need {required} bytes, only {available} bytes available")]
    StorageFull {
        /// Path or identifier of the storage location
        path: String,
        /// Bytes available in storage
        available: u64,
        /// Bytes required for the checkpoint
        required: u64,
    },

    /// Connection to storage backend was lost
    #[error("Connection to checkpoint backend '{backend}' lost: {reason}")]
    ConnectionLost {
        /// Name of the backend (e.g., "postgres", "s3", "redis")
        backend: String,
        /// Reason for connection loss
        reason: String,
    },

    /// Serialization of checkpoint state failed
    #[error("Checkpoint serialization failed: {reason}")]
    SerializationFailed {
        /// Detailed reason for serialization failure
        reason: String,
    },

    /// Deserialization of checkpoint state failed
    #[error("Checkpoint deserialization failed: {reason}")]
    DeserializationFailed {
        /// Detailed reason for deserialization failure
        reason: String,
    },

    /// Checkpoint integrity check failed (corruption detected)
    #[error("Checkpoint integrity check failed for '{checkpoint_id}': {reason}")]
    IntegrityCheckFailed {
        /// ID of the checkpoint that failed integrity check
        checkpoint_id: String,
        /// Reason for integrity failure
        reason: String,
    },

    /// Checkpoint not found
    #[error("Checkpoint '{checkpoint_id}' not found")]
    NotFound {
        /// ID of the missing checkpoint
        checkpoint_id: String,
    },

    /// Permission denied accessing checkpoint storage
    #[error("Permission denied accessing checkpoint storage at '{path}': {reason}")]
    PermissionDenied {
        /// Path that was inaccessible
        path: String,
        /// Reason for permission denial
        reason: String,
    },

    /// I/O error during checkpoint operation
    #[error("Checkpoint I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// Timeout during checkpoint operation
    #[error("Checkpoint operation timed out after {duration:?}")]
    Timeout {
        /// Duration waited before timeout
        duration: std::time::Duration,
    },

    /// Index corruption detected
    #[error("Checkpoint index corrupted at '{path}': {reason}")]
    IndexCorrupted {
        /// Path to corrupted index
        path: String,
        /// Reason for corruption detection
        reason: String,
    },

    /// Schema version mismatch during migration
    #[error("Checkpoint schema version mismatch: found version {found}, expected {expected}")]
    SchemaMismatch {
        /// Version found in checkpoint
        found: u32,
        /// Version expected by current code
        expected: u32,
    },

    /// Migration failed
    #[error("Checkpoint migration from version {from} to {to} failed: {reason}")]
    MigrationFailed {
        /// Source version
        from: u32,
        /// Target version
        to: u32,
        /// Reason for failure
        reason: String,
    },

    /// Replication quorum not achieved
    #[error("Checkpoint replication quorum not achieved: got {successes}/{total} successes, needed {required}")]
    QuorumNotAchieved {
        /// Number of successful replications
        successes: usize,
        /// Total number of replicas
        total: usize,
        /// Number required for quorum
        required: usize,
    },

    /// Lock acquisition failed
    #[error("Failed to acquire checkpoint lock at '{path}': {reason}")]
    LockFailed {
        /// Path where lock was attempted
        path: String,
        /// Reason for lock failure
        reason: String,
    },

    /// Encryption of checkpoint data failed
    #[error("Checkpoint encryption failed: {reason}")]
    EncryptionFailed {
        /// Reason for encryption failure
        reason: String,
    },

    /// Decryption of checkpoint data failed
    #[error("Checkpoint decryption failed: {reason}")]
    DecryptionFailed {
        /// Reason for decryption failure (likely wrong key or corrupted data)
        reason: String,
    },

    /// Other checkpoint error
    #[error("Checkpoint error: {0}")]
    Other(String),
}

impl CheckpointError {
    /// Returns true if this error is likely recoverable (e.g., retry may succeed)
    pub fn is_recoverable(&self) -> bool {
        matches!(
            self,
            CheckpointError::ConnectionLost { .. }
                | CheckpointError::Timeout { .. }
                | CheckpointError::LockFailed { .. }
                | CheckpointError::QuorumNotAchieved { .. }
        )
    }

    /// Returns true if this error indicates data corruption
    pub fn is_corruption(&self) -> bool {
        matches!(
            self,
            CheckpointError::IntegrityCheckFailed { .. }
                | CheckpointError::IndexCorrupted { .. }
                | CheckpointError::DeserializationFailed { .. }
        )
    }

    /// Returns true if this error is a configuration/permission issue
    pub fn is_configuration_issue(&self) -> bool {
        matches!(
            self,
            CheckpointError::PermissionDenied { .. }
                | CheckpointError::StorageFull { .. }
                | CheckpointError::SchemaMismatch { .. }
        )
    }
}

/// DashFlow error types
#[non_exhaustive]
#[derive(Error, Debug)]
pub enum Error {
    /// Graph validation error
    #[error("Graph validation error: {0}")]
    Validation(String),

    /// Node execution error
    #[error("Node execution error in '{node}': {source}")]
    NodeExecution {
        /// Name of the node that failed.
        node: String,
        /// The underlying error that occurred.
        source: Box<dyn std::error::Error + Send + Sync>,
    },

    /// Graph has no entry point
    #[error("Graph has no entry point defined")]
    NoEntryPoint,

    /// Node not found
    #[error("Node '{0}' not found in graph")]
    NodeNotFound(String),

    /// Duplicate node name
    #[error(
        "Node '{0}' already exists in graph. Use add_node_or_replace() for intentional overwrites."
    )]
    DuplicateNodeName(String),

    /// Cycle detected (when cycles are not allowed)
    #[error("Cycle detected in graph: {0}")]
    CycleDetected(String),

    /// Invalid edge
    #[error("Invalid edge: {0}")]
    InvalidEdge(String),

    /// Execution timeout
    #[error("Execution timeout after {0:?}")]
    Timeout(std::time::Duration),

    /// Serialization error
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    /// Interrupt without checkpointer
    #[error("Cannot interrupt at node '{0}' without a checkpointer configured. Use with_checkpointer() before compiling the graph.")]
    InterruptWithoutCheckpointer(String),

    /// Interrupt without thread ID
    #[error("Cannot interrupt at node '{0}' without a thread_id configured. Use with_thread_id() before invoking the graph.")]
    InterruptWithoutThreadId(String),

    /// Resume without checkpointer
    #[error("Cannot resume without a checkpointer configured. Use with_checkpointer() before compiling the graph.")]
    ResumeWithoutCheckpointer,

    /// Resume without thread ID
    #[error("Cannot resume without a thread_id configured. Use with_thread_id() before invoking the graph.")]
    ResumeWithoutThreadId,

    /// No checkpoint to resume from
    #[error("No checkpoint found to resume from for thread_id: {0}")]
    NoCheckpointToResume(String),

    /// Recursion limit exceeded
    #[error("Recursion limit of {limit} reached. Graph execution exceeded maximum number of steps. This may indicate an infinite loop. Use with_recursion_limit() to increase the limit if needed.")]
    RecursionLimit {
        /// The recursion limit that was exceeded.
        limit: u32,
    },

    /// State size exceeded maximum limit
    #[error("State size of {actual_bytes} bytes exceeds maximum limit of {max_bytes} bytes after node '{node}'. Use with_max_state_size() to increase the limit or without_limits() to disable.")]
    StateSizeExceeded {
        /// Name of the node that produced the oversized state.
        node: String,
        /// Actual size of the state in bytes.
        actual_bytes: u64,
        /// Maximum allowed state size in bytes.
        max_bytes: u64,
    },

    /// Get/update state without checkpointer
    #[error("Cannot {operation} without a checkpointer configured. Use with_checkpointer() before compiling the graph.")]
    StateOperationWithoutCheckpointer {
        /// The operation that was attempted (e.g., "get_state", "update_state").
        operation: &'static str,
    },

    /// Get/update state without thread ID
    #[error("Cannot {operation} without a thread_id configured. Use with_thread_id() before invoking the graph.")]
    StateOperationWithoutThreadId {
        /// The operation that was attempted (e.g., "get_state", "update_state").
        operation: &'static str,
    },

    /// Parallel execution failed (all nodes failed)
    #[error("Parallel execution failed: no successful node executions")]
    ParallelExecutionFailed,

    /// Internal execution error (should not happen in normal operation)
    #[error("Internal execution error: {0}")]
    InternalExecutionError(String),

    /// Generic error
    #[error("{0}")]
    Generic(String),

    /// DashFlow core error
    #[error("DashFlow error: {0}")]
    Core(#[from] crate::core::Error),

    /// Checkpoint error
    #[error("Checkpoint error: {0}")]
    Checkpoint(#[from] CheckpointError),
}

/// Result type for DashFlow operations
pub type Result<T> = std::result::Result<T, Error>;

impl ActionableError for Error {
    fn suggestion(&self) -> Option<ActionableSuggestion> {
        match self {
            Error::NoEntryPoint => Some(
                ActionableSuggestion::new(
                    "Set an entry point for your graph using set_entry_point()"
                ).with_code_snippet(r#"
let mut graph = StateGraph::<MyState>::new();
graph.add_node("start", start_node);
graph.add_node("end", end_node);
graph.set_entry_point("start");  // <-- Add this
graph.add_edge("start", "end");
"#)
            ),

            Error::NodeNotFound(name) => Some(
                ActionableSuggestion::new(
                    format!("Add the missing node '{}' to your graph before referencing it", name)
                ).with_code_snippet(format!(r#"
// Add the node before creating edges to it:
graph.add_node("{}", your_node_implementation);

// Or if this is a typo, check your node names:
// Available nodes: graph.node_names()
"#, name))
            ),

            Error::DuplicateNodeName(name) => Some(
                ActionableSuggestion::new(
                    "Use a unique name, or explicitly replace the node"
                ).with_code_snippet(format!(r#"
// Option 1: Use a different name
graph.add_node("{}_v2", new_node);

// Option 2: Explicitly replace the node
graph.add_node_or_replace("{}", new_node);

// Option 3: Use strict mode for early detection
let mut graph = StateGraph::new().strict();
// This will error immediately on duplicates
"#, name, name))
            ),

            Error::CycleDetected(path) => Some(
                ActionableSuggestion::new(
                    format!("Break the cycle by removing one of the edges in: {}", path)
                ).with_code_snippet(r#"
// Cycles are only allowed in graphs created with allow_cycles():
let mut graph = StateGraph::new().allow_cycles();

// Or use conditional routing to avoid the cycle:
graph.add_conditional_edges("node", |state| {
    if state.should_continue() {
        "continue"
    } else {
        "end"  // <-- Breaks the cycle
    }
}, vec![("continue", "process"), ("end", END)]);
"#)
            ),

            Error::InvalidEdge(reason) => Some(
                ActionableSuggestion::new(
                    "Check that both source and target nodes exist"
                ).with_code_snippet(format!(r#"
// Ensure nodes exist before adding edges:
graph.add_node("source", source_node);
graph.add_node("target", target_node);
graph.add_edge("source", "target");  // Now this works

// Error context: {}
"#, reason))
            ),

            Error::Timeout(duration) => Some(
                ActionableSuggestion::new(
                    format!("Increase the timeout (currently {:?}) or optimize your nodes", duration)
                ).with_code_snippet(r#"
// Option 1: Increase the timeout
let app = graph.compile()?
    .with_timeout(Duration::from_secs(600));  // 10 minutes

// Option 2: Disable timeouts entirely (for long-running tasks)
let app = graph.compile()?
    .without_timeouts();

// Option 3: Use per-node timeouts for fine control
let app = graph.compile()?
    .with_node_timeout(Duration::from_secs(120));  // 2 min per node
"#)
            ),

            Error::InterruptWithoutCheckpointer(node) => Some(
                ActionableSuggestion::new(
                    format!("Configure a checkpointer to enable interrupts at '{}'", node)
                ).with_code_snippet(r#"
use dashflow::checkpoint::MemoryCheckpointer;

let checkpointer = MemoryCheckpointer::<MyState>::new();
let app = graph.compile()?
    .with_checkpointer(checkpointer);  // <-- Add this

// For production, consider SqliteCheckpointer for persistence
"#)
            ),

            Error::InterruptWithoutThreadId(node) => Some(
                ActionableSuggestion::new(
                    format!("Set a thread_id to enable interrupts at '{}'", node)
                ).with_code_snippet(r#"
// Set thread_id when invoking:
let result = app
    .with_thread_id("conversation-123")  // <-- Add this
    .invoke(initial_state)
    .await?;

// Thread IDs identify resumable execution contexts
"#)
            ),

            Error::ResumeWithoutCheckpointer => Some(
                ActionableSuggestion::new(
                    "Configure a checkpointer to enable resume functionality"
                ).with_code_snippet(r#"
use dashflow::checkpoint::MemoryCheckpointer;

let checkpointer = MemoryCheckpointer::<MyState>::new();
let app = graph.compile()?
    .with_checkpointer(checkpointer);

// Then resume:
let result = app
    .with_thread_id("my-thread")
    .resume()
    .await?;
"#)
            ),

            Error::ResumeWithoutThreadId => Some(
                ActionableSuggestion::new(
                    "Set a thread_id to identify which execution to resume"
                ).with_code_snippet(r#"
// Resume requires knowing which thread to continue:
let result = app
    .with_thread_id("conversation-123")  // <-- Add this
    .resume()
    .await?;
"#)
            ),

            Error::NoCheckpointToResume(thread_id) => Some(
                ActionableSuggestion::new(
                    format!("No checkpoint exists for thread '{}'. Start a new execution or check the thread_id", thread_id)
                ).with_code_snippet(format!(r#"
// Option 1: Start fresh instead of resuming
let result = app
    .with_thread_id("{}")
    .invoke(initial_state)  // Use invoke, not resume
    .await?;

// Option 2: Check if a checkpoint exists first
if app.has_checkpoint("{}").await? {{
    app.resume().await?
}} else {{
    app.invoke(initial_state).await?
}}
"#, thread_id, thread_id))
            ),

            Error::RecursionLimit { limit } => Some(
                ActionableSuggestion::new(
                    format!("Recursion limit of {} reached. This may indicate an infinite loop.", limit)
                ).with_code_snippet(format!(r#"
// Option 1: Increase the limit if your graph legitimately needs more steps
let app = graph.compile()?
    .with_recursion_limit({});  // Increase from {}

// Option 2: Check for infinite loops in your conditional edges
graph.add_conditional_edges("router", |state| {{
    if state.iterations > 100 {{
        "done"  // <-- Add termination condition
    }} else {{
        "continue"
    }}
}}, routes);

// Option 3: Disable limits (use with caution!)
let app = graph.compile()?.without_limits();
"#, limit * 2, limit))
            ),

            Error::StateSizeExceeded { node, actual_bytes, max_bytes } => Some(
                ActionableSuggestion::new(
                    format!("State grew to {} bytes (limit: {}) after node '{}'", actual_bytes, max_bytes, node)
                ).with_code_snippet(format!(r#"
// Option 1: Increase the limit
let app = graph.compile()?
    .with_max_state_size({});  // Increase from {}

// Option 2: Reduce state size in your node
// Node '{}' is accumulating too much data. Consider:
// - Summarizing/compressing data
// - Moving large data to external storage
// - Clearing processed items from state

// Option 3: Disable limits (use with caution!)
let app = graph.compile()?.without_limits();
"#, actual_bytes * 2, max_bytes, node))
            ),

            Error::StateOperationWithoutCheckpointer { operation } => Some(
                ActionableSuggestion::new(
                    format!("Configure a checkpointer to {} state", operation)
                ).with_code_snippet(r#"
use dashflow::checkpoint::MemoryCheckpointer;

let checkpointer = MemoryCheckpointer::<MyState>::new();
let app = graph.compile()?
    .with_checkpointer(checkpointer);

// State operations (get/update) require persistence
"#)
            ),

            Error::StateOperationWithoutThreadId { operation } => Some(
                ActionableSuggestion::new(
                    format!("Set a thread_id to {} state", operation)
                ).with_code_snippet(r#"
let result = app
    .with_thread_id("my-thread")  // <-- Add this
    .invoke(state)
    .await?;

// Thread ID identifies which state to access
"#)
            ),

            Error::ParallelExecutionFailed => Some(
                ActionableSuggestion::new(
                    "All parallel nodes failed. Check individual node implementations."
                ).with_code_snippet(r#"
// Parallel execution fails if ALL branches fail.
// To make parallel execution more resilient:

// Option 1: Add error handling in your nodes
async fn my_node(state: MyState) -> Result<MyState, NodeError> {
    match do_work(&state).await {
        Ok(result) => Ok(result),
        Err(e) => {
            // Return partial result instead of failing
            Ok(state.with_error(e.to_string()))
        }
    }
}

// Option 2: Check node implementations for common failure causes
"#)
            ),

            Error::Serialization(_) => Some(
                ActionableSuggestion::new(
                    "Serialization failed. Check your state type's serde attributes."
                ).with_code_snippet(r#"
// Ensure your state type derives Serialize and Deserialize:
#[derive(Clone, Serialize, Deserialize)]
pub struct MyState {
    // All fields must be serializable
    pub data: String,

    // Skip non-serializable fields:
    #[serde(skip)]
    pub handle: SomeNonSerializableType,
}

// Common issues:
// - Circular references
// - Function pointers
// - Raw pointers
// - Types without serde support
"#)
            ),

            // Errors without specific suggestions
            Error::Validation(_) |
            Error::NodeExecution { .. } |
            Error::InternalExecutionError(_) |
            Error::Generic(_) |
            Error::Core(_) |
            Error::Checkpoint(_) => None,
        }
    }
}

impl ActionableError for CheckpointError {
    fn suggestion(&self) -> Option<ActionableSuggestion> {
        match self {
            CheckpointError::StorageFull { path, required, .. } => Some(
                ActionableSuggestion::new(format!(
                    "Free up space at '{}' or use a different storage location",
                    path
                ))
                .with_code_snippet(format!(
                    r#"
// Option 1: Use a different path with more space
let checkpointer = SqliteCheckpointer::new("/path/with/space/checkpoints.db")?;

// Option 2: Clean up old checkpoints
checkpointer.cleanup_older_than(Duration::from_days(7)).await?;

// Option 3: Use differential checkpoints to reduce size
let checkpointer = DifferentialCheckpointer::wrap(
    your_checkpointer,
    DifferentialConfig::memory_optimized()
);

// Required space: {} bytes
"#,
                    required
                )),
            ),

            CheckpointError::ConnectionLost { backend, reason } => Some(
                ActionableSuggestion::new(format!(
                    "Connection to '{}' was lost: {}. Check network/credentials.",
                    backend, reason
                ))
                .with_code_snippet(format!(
                    r#"
// Checkpoint backends should be resilient to transient failures.
// The error will auto-retry for recoverable issues.

// If using remote storage, check:
// 1. Network connectivity
// 2. Authentication credentials
// 3. Service availability

// For local development, use MemoryCheckpointer:
let checkpointer = MemoryCheckpointer::<MyState>::new();

// Backend: {}
"#,
                    backend
                )),
            ),

            CheckpointError::SerializationFailed { reason } => Some(
                ActionableSuggestion::new(format!("State serialization failed: {}", reason))
                    .with_code_snippet(
                        r#"
// Ensure your state type is fully serializable:
#[derive(Clone, Serialize, Deserialize)]
pub struct MyState {
    pub data: String,

    // Skip non-serializable fields:
    #[serde(skip)]
    pub temp_handle: Option<Handle>,
}

// Common issues:
// - Recursive types without Box
// - Generic types missing serde bounds
// - Fields with lifetime parameters
"#,
                    ),
            ),

            CheckpointError::NotFound { checkpoint_id } => Some(
                ActionableSuggestion::new(format!("Checkpoint '{}' does not exist", checkpoint_id))
                    .with_code_snippet(format!(
                        r#"
// Check if checkpoint exists before loading:
if let Some(checkpoint) = checkpointer.load("{}").await? {{
    // Use checkpoint
}} else {{
    // Start fresh
}}

// Or list available checkpoints:
let checkpoints = checkpointer.list_for_thread(thread_id).await?;
"#,
                        checkpoint_id
                    )),
            ),

            CheckpointError::SchemaMismatch { found, expected } => Some(
                ActionableSuggestion::new(format!(
                    "Checkpoint schema v{} doesn't match code v{}",
                    found, expected
                ))
                .with_code_snippet(format!(
                    r#"
// Your state struct has changed since the checkpoint was saved.
// Options:

// 1. Migrate the checkpoint (if migration exists):
checkpointer.migrate_from({}, {}).await?;

// 2. Start fresh (discard old checkpoints):
checkpointer.delete_all_for_thread(thread_id).await?;

// 3. Add backwards compatibility to your state:
#[derive(Serialize, Deserialize)]
pub struct MyState {{
    pub data: String,
    #[serde(default)]  // <-- Handle missing fields
    pub new_field: Option<String>,
}}
"#,
                    found, expected
                )),
            ),

            CheckpointError::PermissionDenied { path, .. } => Some(
                ActionableSuggestion::new(format!("Permission denied accessing '{}'", path))
                    .with_code_snippet(format!(
                        r#"
// Check file/directory permissions:
// chmod 755 {}

// Or use a path you have access to:
let checkpointer = SqliteCheckpointer::new("./local_checkpoints.db")?;

// For containers, ensure the volume is mounted with write permissions
"#,
                        path
                    )),
            ),

            CheckpointError::EncryptionFailed { .. } => Some(
                ActionableSuggestion::new(
                    "Checkpoint encryption failed. Check that the encryption key is valid."
                        .to_string(),
                )
                .with_code_snippet(
                    r#"
// Ensure key is exactly 32 bytes
let key = EncryptionKey::from_bytes(&valid_32_byte_key)?;

// Or use a passphrase (Argon2 derives the key)
let key = EncryptionKey::from_passphrase("secure-password", None)?;
"#
                    .to_string(),
                ),
            ),

            CheckpointError::DecryptionFailed { .. } => Some(
                ActionableSuggestion::new(
                    "Checkpoint decryption failed. Wrong key or corrupted data.".to_string(),
                )
                .with_code_snippet(
                    r#"
// Verify using the same key that was used for encryption
// Common causes:
// 1. Wrong encryption key
// 2. Data was modified/corrupted after encryption
// 3. Passphrase doesn't match
"#
                    .to_string(),
                ),
            ),

            // Errors without specific suggestions
            CheckpointError::DeserializationFailed { .. }
            | CheckpointError::IntegrityCheckFailed { .. }
            | CheckpointError::Io(_)
            | CheckpointError::Timeout { .. }
            | CheckpointError::IndexCorrupted { .. }
            | CheckpointError::MigrationFailed { .. }
            | CheckpointError::QuorumNotAchieved { .. }
            | CheckpointError::LockFailed { .. }
            | CheckpointError::Other(_) => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn test_validation_error() {
        let error = Error::Validation("invalid graph structure".to_string());
        assert_eq!(
            error.to_string(),
            "Graph validation error: invalid graph structure"
        );
    }

    #[test]
    fn test_node_execution_error() {
        let source_error = std::io::Error::other("node failed");
        let error = Error::NodeExecution {
            node: "test_node".to_string(),
            source: Box::new(source_error),
        };
        assert!(error.to_string().contains("test_node"));
        assert!(error.to_string().contains("Node execution error"));
    }

    #[test]
    fn test_no_entry_point_error() {
        let error = Error::NoEntryPoint;
        assert_eq!(error.to_string(), "Graph has no entry point defined");
    }

    #[test]
    fn test_node_not_found_error() {
        let error = Error::NodeNotFound("missing_node".to_string());
        assert_eq!(error.to_string(), "Node 'missing_node' not found in graph");
    }

    #[test]
    fn test_cycle_detected_error() {
        let error = Error::CycleDetected("A -> B -> C -> A".to_string());
        assert_eq!(
            error.to_string(),
            "Cycle detected in graph: A -> B -> C -> A"
        );
    }

    #[test]
    fn test_invalid_edge_error() {
        let error = Error::InvalidEdge("edge from nonexistent node".to_string());
        assert_eq!(
            error.to_string(),
            "Invalid edge: edge from nonexistent node"
        );
    }

    #[test]
    fn test_timeout_error() {
        let duration = Duration::from_secs(30);
        let error = Error::Timeout(duration);
        assert!(error.to_string().contains("Execution timeout"));
        assert!(error.to_string().contains("30s"));
    }

    #[test]
    fn test_serialization_error_from() {
        let json_error = serde_json::from_str::<i32>("invalid json").unwrap_err();
        let error = Error::from(json_error);
        assert!(matches!(error, Error::Serialization(_)));
        assert!(error.to_string().contains("Serialization error"));
    }

    #[test]
    fn test_generic_error() {
        let error = Error::Generic("something went wrong".to_string());
        assert_eq!(error.to_string(), "something went wrong");
    }

    #[test]
    fn test_error_is_send() {
        fn assert_send<T: Send>() {}
        assert_send::<Error>();
    }

    #[test]
    fn test_error_is_sync() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<Error>();
    }

    #[test]
    fn test_error_debug_format() {
        let error = Error::Validation("test".to_string());
        let debug_str = format!("{:?}", error);
        assert!(debug_str.contains("Validation"));
        assert!(debug_str.contains("test"));
    }

    #[test]
    fn test_result_type_ok() {
        let result: Result<i32> = Ok(42);
        assert!(result.is_ok());
        if let Ok(val) = result {
            assert_eq!(val, 42);
        }
    }

    #[test]
    fn test_result_type_err() {
        let result: Result<i32> = Err(Error::Generic("test error".to_string()));
        assert!(result.is_err());
        if let Err(e) = result {
            assert_eq!(e.to_string(), "test error");
        }
    }

    #[test]
    fn test_node_execution_error_preserves_source() {
        let source_error = std::io::Error::new(std::io::ErrorKind::NotFound, "file missing");
        let error = Error::NodeExecution {
            node: "reader".to_string(),
            source: Box::new(source_error),
        };

        let error_string = error.to_string();
        assert!(error_string.contains("reader"));
        assert!(error_string.contains("file missing"));
    }

    #[test]
    fn test_multiple_error_variants() {
        let errors = vec![
            Error::Validation("val".to_string()),
            Error::NoEntryPoint,
            Error::NodeNotFound("node".to_string()),
            Error::CycleDetected("cycle".to_string()),
            Error::InvalidEdge("edge".to_string()),
            Error::Timeout(Duration::from_secs(1)),
            Error::Generic("generic".to_string()),
        ];

        for error in errors {
            // All errors should produce non-empty strings
            assert!(!error.to_string().is_empty());
            // All errors should be Debug
            assert!(!format!("{:?}", error).is_empty());
        }
    }

    #[test]
    fn test_error_propagation() {
        fn might_fail() -> Result<i32> {
            Err(Error::Validation("test".to_string()))
        }

        fn calls_might_fail() -> Result<i32> {
            might_fail()?;
            Ok(42)
        }

        let result = calls_might_fail();
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), Error::Validation(_)));
    }

    #[test]
    fn test_timeout_duration_values() {
        let timeouts = vec![
            Duration::from_millis(100),
            Duration::from_secs(1),
            Duration::from_secs(60),
            Duration::from_secs(3600),
        ];

        for duration in timeouts {
            let error = Error::Timeout(duration);
            let error_str = error.to_string();
            assert!(error_str.contains("Execution timeout"));
        }
    }

    #[test]
    fn test_serialization_error_conversion() {
        // Create a JSON error
        let result: std::result::Result<i32, serde_json::Error> =
            serde_json::from_str("not a number");

        let json_error = result.unwrap_err();
        let graph_error: Error = json_error.into();

        assert!(matches!(graph_error, Error::Serialization(_)));
    }

    #[test]
    fn test_error_message_accuracy() {
        let test_cases = vec![
            (
                Error::Validation("missing nodes".to_string()),
                "Graph validation error",
            ),
            (Error::NoEntryPoint, "Graph has no entry point defined"),
            (
                Error::NodeNotFound("start".to_string()),
                "Node 'start' not found",
            ),
            (
                Error::CycleDetected("A->B->A".to_string()),
                "Cycle detected",
            ),
            (Error::InvalidEdge("bad edge".to_string()), "Invalid edge"),
            (
                Error::Generic("custom message".to_string()),
                "custom message",
            ),
        ];

        for (error, expected_substring) in test_cases {
            let error_string = error.to_string();
            assert!(
                error_string.contains(expected_substring),
                "Error '{}' should contain '{}'",
                error_string,
                expected_substring
            );
        }
    }

    #[test]
    fn test_node_execution_error_different_sources() {
        // Test with different error types as sources
        let io_error = Error::NodeExecution {
            node: "io_node".to_string(),
            source: Box::new(std::io::Error::other("io failed")),
        };
        assert!(io_error.to_string().contains("io_node"));

        let fmt_error = Error::NodeExecution {
            node: "fmt_node".to_string(),
            source: Box::new(std::fmt::Error),
        };
        assert!(fmt_error.to_string().contains("fmt_node"));
    }

    #[test]
    fn test_error_size() {
        // Ensure Error enum is not excessively large
        let size = std::mem::size_of::<Error>();
        // Should be reasonable (< 128 bytes for a boxed error)
        assert!(size < 128, "Error size {} is too large", size);
    }

    #[test]
    fn test_error_equality_semantics() {
        // While Error doesn't implement PartialEq (due to Box<dyn Error>),
        // we can test that error variants are distinct
        let err1 = Error::NoEntryPoint;
        let err2 = Error::Generic("test".to_string());

        // Different error messages
        assert_ne!(err1.to_string(), err2.to_string());
    }

    // ==================== CheckpointError Tests ====================

    #[test]
    fn test_checkpoint_error_storage_full() {
        let err = CheckpointError::StorageFull {
            path: "/tmp/checkpoints".to_string(),
            available: 1024,
            required: 4096,
        };
        let msg = err.to_string();
        assert!(msg.contains("Storage full"));
        assert!(msg.contains("/tmp/checkpoints"));
        assert!(msg.contains("1024"));
        assert!(msg.contains("4096"));
    }

    #[test]
    fn test_checkpoint_error_connection_lost() {
        let err = CheckpointError::ConnectionLost {
            backend: "postgres".to_string(),
            reason: "connection refused".to_string(),
        };
        let msg = err.to_string();
        assert!(msg.contains("postgres"));
        assert!(msg.contains("connection refused"));
    }

    #[test]
    fn test_checkpoint_error_serialization_failed() {
        let err = CheckpointError::SerializationFailed {
            reason: "recursive type not supported".to_string(),
        };
        let msg = err.to_string();
        assert!(msg.contains("serialization failed"));
        assert!(msg.contains("recursive type"));
    }

    #[test]
    fn test_checkpoint_error_deserialization_failed() {
        let err = CheckpointError::DeserializationFailed {
            reason: "invalid UTF-8".to_string(),
        };
        let msg = err.to_string();
        assert!(msg.contains("deserialization failed"));
        assert!(msg.contains("invalid UTF-8"));
    }

    #[test]
    fn test_checkpoint_error_integrity_check_failed() {
        let err = CheckpointError::IntegrityCheckFailed {
            checkpoint_id: "chkpt_123".to_string(),
            reason: "CRC mismatch".to_string(),
        };
        let msg = err.to_string();
        assert!(msg.contains("integrity check failed"));
        assert!(msg.contains("chkpt_123"));
        assert!(msg.contains("CRC mismatch"));
    }

    #[test]
    fn test_checkpoint_error_not_found() {
        let err = CheckpointError::NotFound {
            checkpoint_id: "missing_chkpt".to_string(),
        };
        let msg = err.to_string();
        assert!(msg.contains("not found"));
        assert!(msg.contains("missing_chkpt"));
    }

    #[test]
    fn test_checkpoint_error_permission_denied() {
        let err = CheckpointError::PermissionDenied {
            path: "/var/checkpoints".to_string(),
            reason: "read-only filesystem".to_string(),
        };
        let msg = err.to_string();
        assert!(msg.contains("Permission denied"));
        assert!(msg.contains("/var/checkpoints"));
    }

    #[test]
    fn test_checkpoint_error_io() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
        let err = CheckpointError::from(io_err);
        let msg = err.to_string();
        assert!(msg.contains("I/O error"));
    }

    #[test]
    fn test_checkpoint_error_timeout() {
        let err = CheckpointError::Timeout {
            duration: Duration::from_secs(30),
        };
        let msg = err.to_string();
        assert!(msg.contains("timed out"));
        assert!(msg.contains("30"));
    }

    #[test]
    fn test_checkpoint_error_index_corrupted() {
        let err = CheckpointError::IndexCorrupted {
            path: "/tmp/index.bin".to_string(),
            reason: "invalid header".to_string(),
        };
        let msg = err.to_string();
        assert!(msg.contains("index corrupted"));
        assert!(msg.contains("invalid header"));
    }

    #[test]
    fn test_checkpoint_error_schema_mismatch() {
        let err = CheckpointError::SchemaMismatch {
            found: 1,
            expected: 3,
        };
        let msg = err.to_string();
        assert!(msg.contains("version mismatch"));
        assert!(msg.contains("found version 1"));
        assert!(msg.contains("expected 3"));
    }

    #[test]
    fn test_checkpoint_error_migration_failed() {
        let err = CheckpointError::MigrationFailed {
            from: 1,
            to: 2,
            reason: "missing field 'count'".to_string(),
        };
        let msg = err.to_string();
        assert!(msg.contains("migration"));
        assert!(msg.contains("version 1"));
        assert!(msg.contains("to 2"));
    }

    #[test]
    fn test_checkpoint_error_quorum_not_achieved() {
        let err = CheckpointError::QuorumNotAchieved {
            successes: 1,
            total: 3,
            required: 2,
        };
        let msg = err.to_string();
        assert!(msg.contains("quorum not achieved"));
        assert!(msg.contains("1/3"));
        assert!(msg.contains("needed 2"));
    }

    #[test]
    fn test_checkpoint_error_lock_failed() {
        let err = CheckpointError::LockFailed {
            path: "/tmp/.lock".to_string(),
            reason: "already locked by another process".to_string(),
        };
        let msg = err.to_string();
        assert!(msg.contains("lock"));
        assert!(msg.contains("already locked"));
    }

    #[test]
    fn test_checkpoint_error_other() {
        let err = CheckpointError::Other("unexpected condition".to_string());
        let msg = err.to_string();
        assert!(msg.contains("unexpected condition"));
    }

    #[test]
    fn test_checkpoint_error_is_recoverable() {
        // Recoverable errors
        assert!(CheckpointError::ConnectionLost {
            backend: "test".to_string(),
            reason: "test".to_string()
        }
        .is_recoverable());

        assert!(CheckpointError::Timeout {
            duration: Duration::from_secs(1)
        }
        .is_recoverable());

        assert!(CheckpointError::LockFailed {
            path: "test".to_string(),
            reason: "test".to_string()
        }
        .is_recoverable());

        assert!(CheckpointError::QuorumNotAchieved {
            successes: 1,
            total: 3,
            required: 2
        }
        .is_recoverable());

        // Non-recoverable errors
        assert!(!CheckpointError::SerializationFailed {
            reason: "test".to_string()
        }
        .is_recoverable());

        assert!(!CheckpointError::NotFound {
            checkpoint_id: "test".to_string()
        }
        .is_recoverable());
    }

    #[test]
    fn test_checkpoint_error_is_corruption() {
        // Corruption errors
        assert!(CheckpointError::IntegrityCheckFailed {
            checkpoint_id: "test".to_string(),
            reason: "test".to_string()
        }
        .is_corruption());

        assert!(CheckpointError::IndexCorrupted {
            path: "test".to_string(),
            reason: "test".to_string()
        }
        .is_corruption());

        assert!(CheckpointError::DeserializationFailed {
            reason: "test".to_string()
        }
        .is_corruption());

        // Non-corruption errors
        assert!(!CheckpointError::ConnectionLost {
            backend: "test".to_string(),
            reason: "test".to_string()
        }
        .is_corruption());

        assert!(!CheckpointError::StorageFull {
            path: "test".to_string(),
            available: 0,
            required: 100
        }
        .is_corruption());
    }

    #[test]
    fn test_checkpoint_error_is_configuration_issue() {
        // Configuration issues
        assert!(CheckpointError::PermissionDenied {
            path: "test".to_string(),
            reason: "test".to_string()
        }
        .is_configuration_issue());

        assert!(CheckpointError::StorageFull {
            path: "test".to_string(),
            available: 0,
            required: 100
        }
        .is_configuration_issue());

        assert!(CheckpointError::SchemaMismatch {
            found: 1,
            expected: 2
        }
        .is_configuration_issue());

        // Non-configuration errors
        assert!(!CheckpointError::ConnectionLost {
            backend: "test".to_string(),
            reason: "test".to_string()
        }
        .is_configuration_issue());

        assert!(!CheckpointError::IntegrityCheckFailed {
            checkpoint_id: "test".to_string(),
            reason: "test".to_string()
        }
        .is_configuration_issue());
    }

    #[test]
    fn test_checkpoint_error_to_error_conversion() {
        let checkpoint_err = CheckpointError::NotFound {
            checkpoint_id: "test_chkpt".to_string(),
        };
        let error: Error = checkpoint_err.into();

        assert!(matches!(error, Error::Checkpoint(_)));
        let msg = error.to_string();
        assert!(msg.contains("Checkpoint error"));
        assert!(msg.contains("test_chkpt"));
    }

    #[test]
    fn test_checkpoint_error_debug_format() {
        let err = CheckpointError::StorageFull {
            path: "/tmp".to_string(),
            available: 100,
            required: 200,
        };
        let debug_str = format!("{:?}", err);
        assert!(debug_str.contains("StorageFull"));
        assert!(debug_str.contains("/tmp"));
    }

    #[test]
    fn test_checkpoint_error_is_send_sync() {
        fn assert_send<T: Send>() {}
        fn assert_sync<T: Sync>() {}
        assert_send::<CheckpointError>();
        assert_sync::<CheckpointError>();
    }

    #[test]
    fn test_checkpoint_error_all_variants_have_messages() {
        let errors: Vec<CheckpointError> = vec![
            CheckpointError::StorageFull {
                path: "p".to_string(),
                available: 0,
                required: 1,
            },
            CheckpointError::ConnectionLost {
                backend: "b".to_string(),
                reason: "r".to_string(),
            },
            CheckpointError::SerializationFailed {
                reason: "r".to_string(),
            },
            CheckpointError::DeserializationFailed {
                reason: "r".to_string(),
            },
            CheckpointError::IntegrityCheckFailed {
                checkpoint_id: "c".to_string(),
                reason: "r".to_string(),
            },
            CheckpointError::NotFound {
                checkpoint_id: "c".to_string(),
            },
            CheckpointError::PermissionDenied {
                path: "p".to_string(),
                reason: "r".to_string(),
            },
            CheckpointError::Io(std::io::Error::other("test")),
            CheckpointError::Timeout {
                duration: Duration::from_secs(1),
            },
            CheckpointError::IndexCorrupted {
                path: "p".to_string(),
                reason: "r".to_string(),
            },
            CheckpointError::SchemaMismatch {
                found: 1,
                expected: 2,
            },
            CheckpointError::MigrationFailed {
                from: 1,
                to: 2,
                reason: "r".to_string(),
            },
            CheckpointError::QuorumNotAchieved {
                successes: 1,
                total: 3,
                required: 2,
            },
            CheckpointError::LockFailed {
                path: "p".to_string(),
                reason: "r".to_string(),
            },
            CheckpointError::Other("o".to_string()),
        ];

        for err in errors {
            let msg = err.to_string();
            assert!(
                !msg.is_empty(),
                "Error message should not be empty: {:?}",
                err
            );
        }
    }

    // ==================== ActionableSuggestion Tests ====================

    #[test]
    fn test_actionable_suggestion_basic() {
        let suggestion = ActionableSuggestion::new("Use a different approach");
        assert_eq!(suggestion.description, "Use a different approach");
        assert!(suggestion.code_snippet.is_none());
        assert!(suggestion.doc_url.is_none());
    }

    #[test]
    fn test_actionable_suggestion_with_code() {
        let suggestion = ActionableSuggestion::new("Add the missing import")
            .with_code_snippet("use dashflow::prelude::*;");
        assert!(suggestion.code_snippet.is_some());
        assert!(suggestion
            .code_snippet
            .as_ref()
            .unwrap()
            .contains("use dashflow"));
    }

    #[test]
    fn test_actionable_suggestion_with_doc_url() {
        let suggestion = ActionableSuggestion::new("See documentation")
            .with_doc_url("https://docs.example.com/guide");
        assert!(suggestion.doc_url.is_some());
        assert!(suggestion.doc_url.as_ref().unwrap().contains("example.com"));
    }

    #[test]
    fn test_actionable_suggestion_display() {
        let suggestion =
            ActionableSuggestion::new("Fix the issue").with_code_snippet("\nlet x = 42;\n");
        let display = suggestion.to_string();
        assert!(display.contains("Fix the issue"));
        assert!(display.contains("```rust"));
        assert!(display.contains("let x = 42"));
    }

    #[test]
    fn test_actionable_suggestion_display_with_url() {
        let suggestion = ActionableSuggestion::new("Read more").with_doc_url("https://example.com");
        let display = suggestion.to_string();
        assert!(display.contains("See: https://example.com"));
    }

    #[test]
    fn test_actionable_suggestion_equality() {
        let s1 = ActionableSuggestion::new("Test");
        let s2 = ActionableSuggestion::new("Test");
        let s3 = ActionableSuggestion::new("Different");
        assert_eq!(s1, s2);
        assert_ne!(s1, s3);
    }

    #[test]
    fn test_actionable_suggestion_clone() {
        let original = ActionableSuggestion::new("Original")
            .with_code_snippet("code")
            .with_doc_url("url");
        let cloned = original.clone();
        assert_eq!(original, cloned);
    }

    // ==================== ActionableError Trait Tests ====================

    #[test]
    fn test_error_has_suggestion() {
        let with_suggestion = Error::NoEntryPoint;
        let without_suggestion = Error::Generic("test".to_string());

        assert!(with_suggestion.has_suggestion());
        assert!(!without_suggestion.has_suggestion());
    }

    #[test]
    fn test_error_suggestion_no_entry_point() {
        let error = Error::NoEntryPoint;
        let suggestion = error.suggestion().expect("Should have suggestion");
        assert!(suggestion.description.contains("entry point"));
        assert!(suggestion.code_snippet.is_some());
        assert!(suggestion
            .code_snippet
            .as_ref()
            .unwrap()
            .contains("set_entry_point"));
    }

    #[test]
    fn test_error_suggestion_node_not_found() {
        let error = Error::NodeNotFound("my_node".to_string());
        let suggestion = error.suggestion().expect("Should have suggestion");
        assert!(suggestion.description.contains("my_node"));
        assert!(suggestion
            .code_snippet
            .as_ref()
            .unwrap()
            .contains("my_node"));
    }

    #[test]
    fn test_error_suggestion_duplicate_node() {
        let error = Error::DuplicateNodeName("duplicate".to_string());
        let suggestion = error.suggestion().expect("Should have suggestion");
        assert!(suggestion
            .code_snippet
            .as_ref()
            .unwrap()
            .contains("duplicate_v2"));
        assert!(suggestion
            .code_snippet
            .as_ref()
            .unwrap()
            .contains("add_node_or_replace"));
    }

    #[test]
    fn test_error_suggestion_timeout() {
        let error = Error::Timeout(Duration::from_secs(30));
        let suggestion = error.suggestion().expect("Should have suggestion");
        assert!(suggestion.description.contains("30s") || suggestion.description.contains("30"));
        assert!(suggestion
            .code_snippet
            .as_ref()
            .unwrap()
            .contains("with_timeout"));
    }

    #[test]
    fn test_error_suggestion_recursion_limit() {
        let error = Error::RecursionLimit { limit: 100 };
        let suggestion = error.suggestion().expect("Should have suggestion");
        assert!(suggestion.description.contains("100"));
        assert!(suggestion.code_snippet.as_ref().unwrap().contains("200")); // Suggests doubling
    }

    #[test]
    fn test_error_suggestion_state_size_exceeded() {
        let error = Error::StateSizeExceeded {
            node: "big_node".to_string(),
            actual_bytes: 2000000,
            max_bytes: 1000000,
        };
        let suggestion = error.suggestion().expect("Should have suggestion");
        assert!(suggestion.description.contains("big_node"));
        assert!(suggestion
            .code_snippet
            .as_ref()
            .unwrap()
            .contains("with_max_state_size"));
    }

    #[test]
    fn test_error_format_with_suggestion() {
        let error = Error::NoEntryPoint;
        let formatted = error.format_with_suggestion();
        assert!(formatted.contains("Graph has no entry point"));
        assert!(formatted.contains("How to fix:"));
        assert!(formatted.contains("set_entry_point"));
    }

    #[test]
    fn test_error_format_without_suggestion() {
        let error = Error::Generic("Something went wrong".to_string());
        let formatted = error.format_with_suggestion();
        assert_eq!(formatted, "Something went wrong");
        assert!(!formatted.contains("How to fix:"));
    }

    #[test]
    fn test_checkpoint_error_has_suggestion() {
        let with_suggestion = CheckpointError::StorageFull {
            path: "/tmp".to_string(),
            available: 0,
            required: 1000,
        };
        let without_suggestion = CheckpointError::Other("misc".to_string());

        assert!(with_suggestion.has_suggestion());
        assert!(!without_suggestion.has_suggestion());
    }

    #[test]
    fn test_checkpoint_error_suggestion_storage_full() {
        let error = CheckpointError::StorageFull {
            path: "/var/data".to_string(),
            available: 100,
            required: 5000,
        };
        let suggestion = error.suggestion().expect("Should have suggestion");
        assert!(suggestion.description.contains("/var/data"));
        assert!(suggestion.code_snippet.as_ref().unwrap().contains("5000"));
    }

    #[test]
    fn test_checkpoint_error_suggestion_schema_mismatch() {
        let error = CheckpointError::SchemaMismatch {
            found: 1,
            expected: 3,
        };
        let suggestion = error.suggestion().expect("Should have suggestion");
        assert!(suggestion.description.contains("v1"));
        assert!(suggestion.description.contains("v3"));
        assert!(suggestion
            .code_snippet
            .as_ref()
            .unwrap()
            .contains("migrate"));
    }

    #[test]
    fn test_all_main_errors_have_suggestions() {
        // Verify that the important user-facing errors have suggestions
        let errors_with_suggestions = vec![
            Error::NoEntryPoint,
            Error::NodeNotFound("x".to_string()),
            Error::DuplicateNodeName("x".to_string()),
            Error::CycleDetected("A->B".to_string()),
            Error::InvalidEdge("bad".to_string()),
            Error::Timeout(Duration::from_secs(1)),
            Error::InterruptWithoutCheckpointer("x".to_string()),
            Error::InterruptWithoutThreadId("x".to_string()),
            Error::ResumeWithoutCheckpointer,
            Error::ResumeWithoutThreadId,
            Error::NoCheckpointToResume("x".to_string()),
            Error::RecursionLimit { limit: 100 },
            Error::StateSizeExceeded {
                node: "x".to_string(),
                actual_bytes: 100,
                max_bytes: 50,
            },
            Error::StateOperationWithoutCheckpointer { operation: "get" },
            Error::StateOperationWithoutThreadId {
                operation: "update",
            },
            Error::ParallelExecutionFailed,
        ];

        for error in errors_with_suggestions {
            assert!(
                error.has_suggestion(),
                "Error {:?} should have a suggestion",
                error
            );
        }
    }

    #[test]
    fn test_suggestion_code_snippets_are_valid_rust_syntax() {
        // Verify code snippets don't have obvious syntax errors
        let errors = vec![
            Error::NoEntryPoint,
            Error::NodeNotFound("node".to_string()),
            Error::DuplicateNodeName("node".to_string()),
            Error::Timeout(Duration::from_secs(30)),
            Error::RecursionLimit { limit: 100 },
        ];

        for error in errors {
            if let Some(suggestion) = error.suggestion() {
                if let Some(snippet) = &suggestion.code_snippet {
                    // Basic sanity checks
                    assert!(!snippet.is_empty(), "Code snippet should not be empty");
                    // Check balanced braces
                    let open_braces = snippet.matches('{').count();
                    let close_braces = snippet.matches('}').count();
                    assert_eq!(
                        open_braces, close_braces,
                        "Unbalanced braces in snippet for {:?}: {}",
                        error, snippet
                    );
                }
            }
        }
    }
}
