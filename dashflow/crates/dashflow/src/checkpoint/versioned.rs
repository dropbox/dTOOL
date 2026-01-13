// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

//! Versioned checkpoint storage with schema evolution
//!
//! Provides checkpointing with version metadata and automatic migration
//! support for handling schema changes over time.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::SystemTime;

use serde::{Deserialize, Serialize};

use crate::{GraphState, Result};

use super::compression::CompressionAlgorithm;
use super::{
    atomic_write_file, load_checkpoint_index, systemtime_serde, Checkpoint, CheckpointId,
    CheckpointMetadata, Checkpointer, ThreadId, ThreadInfo,
};

// ============================================================================
// Error Helpers (CQ-34: Reduce repetitive IO error mapping)
// ============================================================================

/// Convert IO error to checkpoint error
#[inline]
fn io_err(e: std::io::Error) -> crate::Error {
    crate::Error::Checkpoint(crate::error::CheckpointError::Io(e))
}

/// Version number for checkpoint schema
pub type Version = u32;

/// Trait for migrating checkpoint state between versions
///
/// Implement this trait to handle state evolution when your GraphState
/// structure changes over time. The versioned checkpointer will automatically
/// apply migrations when loading old checkpoints.
///
/// # Example
///
/// ```rust,ignore
/// use dashflow::checkpoint::{StateMigration, Version};
/// use serde::{Deserialize, Serialize};
///
/// // Old state (version 1)
/// #[derive(Serialize, Deserialize)]
/// struct StateV1 {
///     name: String,
/// }
///
/// // New state (version 2) with additional field
/// #[derive(Clone, Serialize, Deserialize)]
/// struct StateV2 {
///     name: String,
///     email: Option<String>, // New field
/// }
///
/// struct MyMigration;
///
/// impl StateMigration<StateV2> for MyMigration {
///     fn source_version(&self) -> Version { 1 }
///     fn target_version(&self) -> Version { 2 }
///
///     fn migrate(&self, data: serde_json::Value) -> Result<StateV2, String> {
///         let old: StateV1 = serde_json::from_value(data)
///             .map_err(|e| e.to_string())?;
///         Ok(StateV2 {
///             name: old.name,
///             email: None, // Default for migrated data
///         })
///     }
/// }
/// ```
pub trait StateMigration<S: GraphState>: Send + Sync {
    /// Source version this migration upgrades from
    fn source_version(&self) -> Version;

    /// Target version this migration upgrades to
    fn target_version(&self) -> Version;

    /// Migrate state data from source version to target version
    ///
    /// # Arguments
    /// * `data` - The raw JSON representation of the old state
    ///
    /// # Returns
    /// The migrated state or an error message
    fn migrate(&self, data: serde_json::Value) -> std::result::Result<S, String>;
}

/// Versioned checkpoint wrapper
///
/// Wraps a checkpoint with version metadata for schema evolution.
/// State is stored as a JSON string to allow bincode serialization of the wrapper
/// while preserving the ability to deserialize/migrate the state later.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct VersionedCheckpoint {
    /// Schema version of this checkpoint
    pub version: Version,

    /// The raw checkpoint data (serialized state as JSON string)
    /// Stored as String to support bincode serialization of the wrapper
    pub data: String,

    /// Unique identifier for this checkpoint.
    pub id: CheckpointId,
    /// Thread ID that this checkpoint belongs to.
    pub thread_id: ThreadId,
    /// Node name where the checkpoint was created.
    pub node: String,
    /// When the checkpoint was created.
    #[serde(with = "systemtime_serde")]
    pub timestamp: SystemTime,
    /// Parent checkpoint ID for checkpoint lineage.
    pub parent_id: Option<CheckpointId>,
    /// User-defined key-value metadata.
    pub metadata: HashMap<String, String>,
}

impl VersionedCheckpoint {
    /// Create a versioned checkpoint from a regular checkpoint
    pub fn from_checkpoint<S: GraphState>(
        checkpoint: &Checkpoint<S>,
        version: Version,
    ) -> Result<Self> {
        let data = serde_json::to_string(&checkpoint.state).map_err(|e| {
            crate::Error::Checkpoint(crate::error::CheckpointError::SerializationFailed {
                reason: format!("Failed to serialize state: {e}"),
            })
        })?;

        Ok(Self {
            version,
            data,
            id: checkpoint.id.clone(),
            thread_id: checkpoint.thread_id.clone(),
            node: checkpoint.node.clone(),
            timestamp: checkpoint.timestamp,
            parent_id: checkpoint.parent_id.clone(),
            metadata: checkpoint.metadata.clone(),
        })
    }

    /// Convert to a regular checkpoint
    pub fn to_checkpoint<S: GraphState>(self) -> Result<Checkpoint<S>> {
        let state: S = serde_json::from_str(&self.data).map_err(|e| {
            crate::Error::Checkpoint(crate::error::CheckpointError::DeserializationFailed {
                reason: format!("Failed to deserialize state: {e}"),
            })
        })?;

        Ok(Checkpoint {
            id: self.id,
            thread_id: self.thread_id,
            state,
            node: self.node,
            timestamp: self.timestamp,
            parent_id: self.parent_id,
            metadata: self.metadata,
        })
    }

    /// Get the data as a JSON Value for migration
    pub fn data_as_value(&self) -> Result<serde_json::Value> {
        serde_json::from_str(&self.data).map_err(|e| {
            crate::Error::Checkpoint(crate::error::CheckpointError::DeserializationFailed {
                reason: format!("Failed to parse data as JSON: {e}"),
            })
        })
    }

    /// Get checkpoint metadata without deserializing state
    pub fn to_metadata(&self) -> CheckpointMetadata {
        CheckpointMetadata {
            id: self.id.clone(),
            thread_id: self.thread_id.clone(),
            node: self.node.clone(),
            timestamp: self.timestamp,
            parent_id: self.parent_id.clone(),
            metadata: self.metadata.clone(),
        }
    }
}

/// Migration chain for upgrading checkpoints across multiple versions
pub struct MigrationChain<S: GraphState> {
    migrations: Vec<Box<dyn StateMigration<S>>>,
    current_version: Version,
}

impl<S: GraphState> MigrationChain<S> {
    /// Create a new migration chain with the current schema version
    pub fn new(current_version: Version) -> Self {
        Self {
            migrations: Vec::new(),
            current_version,
        }
    }

    /// Add a migration to the chain
    #[must_use]
    pub fn add_migration(mut self, migration: impl StateMigration<S> + 'static) -> Self {
        self.migrations.push(Box::new(migration));
        self
    }

    /// Get the current schema version
    pub fn current_version(&self) -> Version {
        self.current_version
    }

    /// Migrate data from source version to current version
    ///
    /// Finds and applies the necessary migration chain to upgrade
    /// the data through intermediate versions if needed.
    pub fn migrate_to_current(
        &self,
        mut data: serde_json::Value,
        from_version: Version,
    ) -> Result<S> {
        if from_version == self.current_version {
            // No migration needed
            return serde_json::from_value(data).map_err(|e| {
                crate::Error::Checkpoint(crate::error::CheckpointError::DeserializationFailed {
                    reason: format!("Failed to deserialize current version: {e}"),
                })
            });
        }

        let mut current = from_version;

        // Find path from source to target
        while current < self.current_version {
            // Find migration for this version
            let migration = self
                .migrations
                .iter()
                .find(|m| m.source_version() == current)
                .ok_or_else(|| {
                    crate::Error::Checkpoint(crate::error::CheckpointError::MigrationFailed {
                        from: current,
                        to: current + 1,
                        reason: "No migration found for version".to_string(),
                    })
                })?;

            // Apply migration
            let migrated = migration.migrate(data).map_err(|e| {
                crate::Error::Checkpoint(crate::error::CheckpointError::MigrationFailed {
                    from: current,
                    to: migration.target_version(),
                    reason: e, // e is already a String
                })
            })?;

            // Convert back to JSON for next migration
            data = serde_json::to_value(&migrated).map_err(|e| {
                crate::Error::Checkpoint(crate::error::CheckpointError::SerializationFailed {
                    reason: format!("Failed to serialize migrated state: {e}"),
                })
            })?;

            current = migration.target_version();
        }

        // Final deserialization
        serde_json::from_value(data).map_err(|e| {
            crate::Error::Checkpoint(crate::error::CheckpointError::DeserializationFailed {
                reason: format!("Failed to deserialize final state: {e}"),
            })
        })
    }
}

/// Versioned file-based checkpoint storage
///
/// Stores checkpoints with version metadata, enabling automatic migration
/// when loading checkpoints created with older schema versions.
///
/// # Example
///
/// ```rust,ignore
/// use dashflow::checkpoint::{VersionedFileCheckpointer, MigrationChain};
///
/// // Create migration chain
/// let migrations = MigrationChain::<MyState>::new(2)
///     .add_migration(V1ToV2Migration);
///
/// // Create versioned checkpointer
/// let checkpointer = VersionedFileCheckpointer::new("./checkpoints", migrations)?;
///
/// // Load will automatically migrate old checkpoints
/// let checkpoint = checkpointer.load("my-checkpoint").await?;
/// ```
pub struct VersionedFileCheckpointer<S: GraphState> {
    directory: std::path::PathBuf,
    index: Arc<Mutex<HashMap<ThreadId, (CheckpointId, SystemTime)>>>,
    migrations: MigrationChain<S>,
    compression: CompressionAlgorithm,
}

impl<S: GraphState> VersionedFileCheckpointer<S> {
    /// Create a new versioned file checkpointer (synchronous)
    ///
    /// # Note
    ///
    /// This constructor performs blocking filesystem operations. If called from an
    /// async context, consider using [`Self::new_async`] instead.
    pub fn new(
        directory: impl Into<std::path::PathBuf>,
        migrations: MigrationChain<S>,
    ) -> Result<Self> {
        let directory = directory.into();
        std::fs::create_dir_all(&directory).map_err(io_err)?;

        // Load index from file if it exists (with proper error logging)
        let index_path = directory.join("index.bin");
        let index = load_checkpoint_index(&index_path);

        Ok(Self {
            directory,
            index: Arc::new(Mutex::new(index)),
            migrations,
            compression: CompressionAlgorithm::default(),
        })
    }

    /// Create a new versioned file checkpointer (async)
    ///
    /// This is the preferred constructor when called from an async context,
    /// as it avoids blocking the executor on filesystem operations.
    pub async fn new_async(
        directory: impl Into<std::path::PathBuf>,
        migrations: MigrationChain<S>,
    ) -> Result<Self> {
        let directory = directory.into();

        tokio::fs::create_dir_all(&directory)
            .await
            .map_err(io_err)?;

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
            migrations,
            compression: CompressionAlgorithm::default(),
        })
    }

    /// Set the compression algorithm
    #[must_use]
    pub fn with_compression(mut self, compression: CompressionAlgorithm) -> Self {
        self.compression = compression;
        self
    }

    /// Get the current schema version
    pub fn current_version(&self) -> Version {
        self.migrations.current_version()
    }

    /// Get the file path for a checkpoint ID
    fn checkpoint_path(&self, checkpoint_id: &str) -> std::path::PathBuf {
        match self.compression {
            CompressionAlgorithm::None => self.directory.join(format!("{checkpoint_id}.v.bin")),
            CompressionAlgorithm::Gzip { .. } => {
                self.directory.join(format!("{checkpoint_id}.v.bin.gz"))
            }
        }
    }

    /// Get the index file path
    fn index_path(&self) -> std::path::PathBuf {
        self.directory.join("index.bin")
    }

    /// Save the index to disk (async to avoid blocking)
    async fn save_index(&self) -> Result<()> {
        // Serialize while holding the lock, then drop guard before await
        let data = {
            let index = self.index.lock().unwrap_or_else(|e| e.into_inner());
            bincode::serialize(&*index).map_err(|e| {
                crate::Error::Checkpoint(crate::error::CheckpointError::SerializationFailed {
                    reason: format!("Failed to serialize checkpoint index: {e}"),
                })
            })?
        };
        let index_path = self.index_path();
        atomic_write_file(&index_path, &data)
            .await
            .map_err(io_err)?;
        Ok(())
    }

    /// List all versioned checkpoint files (async to avoid blocking)
    async fn list_files(&self) -> Result<Vec<std::path::PathBuf>> {
        let directory = self.directory.clone();
        tokio::task::spawn_blocking(move || {
            let entries = std::fs::read_dir(&directory).map_err(io_err)?;

            let mut files = Vec::new();
            for entry in entries {
                let entry = entry.map_err(io_err)?;
                let path = entry.path();
                let filename = path.file_name().and_then(|s| s.to_str()).unwrap_or("");

                // Versioned files have .v.bin or .v.bin.gz extension
                if (filename.contains(".v.bin.gz") || filename.contains(".v.bin"))
                    && filename != "index.bin"
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

    /// Fallback: Find the latest checkpoint by scanning files and sorting by timestamp.
    /// Used when index is corrupted, reset, or points to a missing file.
    /// Matches FileCheckpointer::get_latest_by_file_scan() behavior.
    async fn get_latest_by_file_scan(&self, thread_id: &str) -> Result<Option<Checkpoint<S>>> {
        let files = self.list_files().await?;
        let compression = self.compression;
        let mut latest: Option<(VersionedCheckpoint, SystemTime)> = None;

        for file in files {
            let file_clone = file.clone();
            let file_display = file.display().to_string();
            let result: Result<VersionedCheckpoint> = tokio::task::spawn_blocking(move || {
                use std::io::Read;
                let f = std::fs::File::open(&file_clone).map_err(io_err)?;
                let mut reader = std::io::BufReader::new(f);
                let mut data = Vec::new();
                reader.read_to_end(&mut data).map_err(io_err)?;

                let is_compressed = file_clone
                    .extension()
                    .and_then(|s| s.to_str())
                    .map(|s| s == "gz")
                    .unwrap_or(false);

                let decompressed = if is_compressed {
                    compression.decompress(&data)?
                } else {
                    data
                };

                bincode::deserialize(&decompressed).map_err(|e| {
                    crate::Error::Checkpoint(crate::error::CheckpointError::DeserializationFailed {
                        reason: format!(
                            "Failed to deserialize checkpoint from '{}': {e}",
                            file_clone.display()
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
            })?;

            match result {
                Ok(versioned) => {
                    if versioned.thread_id == thread_id {
                        match &latest {
                            None => latest = Some((versioned.clone(), versioned.timestamp)),
                            Some((current_checkpoint, current_ts)) => {
                                // Pick newer by timestamp, then by ID for tie-breaking
                                if versioned.timestamp > *current_ts
                                    || (versioned.timestamp == *current_ts
                                        && versioned.id > current_checkpoint.id)
                                {
                                    latest = Some((versioned.clone(), versioned.timestamp));
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

        // If we found a latest checkpoint via file scan, update the index and convert to Checkpoint
        if let Some((versioned, _)) = latest {
            {
                let mut index = self.index.lock().unwrap_or_else(|e| e.into_inner());
                index.insert(
                    thread_id.to_string(),
                    (versioned.id.clone(), versioned.timestamp),
                );
            }
            // Note: Index is saved to disk on next save() call

            // Migrate if necessary
            let data_value = versioned.data_as_value()?;
            let state = self
                .migrations
                .migrate_to_current(data_value, versioned.version)?;

            return Ok(Some(Checkpoint {
                id: versioned.id,
                thread_id: versioned.thread_id,
                state,
                node: versioned.node,
                timestamp: versioned.timestamp,
                parent_id: versioned.parent_id,
                metadata: versioned.metadata,
            }));
        }

        Ok(None)
    }
}

#[async_trait::async_trait]
impl<S: GraphState> Checkpointer<S> for VersionedFileCheckpointer<S> {
    async fn save(&self, checkpoint: Checkpoint<S>) -> Result<()> {
        let path = self.checkpoint_path(&checkpoint.id);
        let path_display = path.display().to_string();
        let thread_id = checkpoint.thread_id.clone();
        let checkpoint_id = checkpoint.id.clone();
        let timestamp = checkpoint.timestamp;
        let compression = self.compression;
        let version = self.migrations.current_version();

        // Create versioned checkpoint
        let versioned = VersionedCheckpoint::from_checkpoint(&checkpoint, version)?;

        // Clone checkpoint_id for use inside closure (original needed for index update)
        let checkpoint_id_for_err = checkpoint_id.clone();

        // Serialize + compress + write in spawn_blocking (M-635: CPU-intensive bincode + compression)
        tokio::task::spawn_blocking(move || {
            // Serialize with bincode
            let data = bincode::serialize(&versioned).map_err(|e| {
                crate::Error::Checkpoint(crate::error::CheckpointError::SerializationFailed {
                    reason: format!("Failed to serialize checkpoint '{}': {e}", checkpoint_id_for_err),
                })
            })?;

            // Compress
            let compressed = compression.compress(&data)?;

            // Write with buffered I/O
            use std::io::Write;
            let file = std::fs::File::create(&path).map_err(io_err)?;
            let mut writer = std::io::BufWriter::new(file);
            writer.write_all(&compressed).map_err(io_err)?;
            writer.flush().map_err(io_err)?;
            Ok::<_, crate::Error>(())
        })
        .await
        .map_err(|e| {
            crate::Error::Checkpoint(crate::error::CheckpointError::Other(format!(
                "Task join error saving checkpoint '{}': {e}",
                path_display
            )))
        })??;

        // Update index
        {
            let mut index = self.index.lock().unwrap_or_else(|e| e.into_inner());
            let entry = index
                .entry(thread_id)
                .or_insert((checkpoint_id.clone(), timestamp));
            let is_newer = timestamp > entry.1 || (timestamp == entry.1 && checkpoint_id > entry.0);
            if is_newer {
                *entry = (checkpoint_id, timestamp);
            }
        }

        self.save_index().await?;
        Ok(())
    }

    async fn load(&self, checkpoint_id: &str) -> Result<Option<Checkpoint<S>>> {
        let path = self.checkpoint_path(checkpoint_id);
        // Use async file existence check to avoid blocking the async runtime
        if !tokio::fs::try_exists(&path).await.unwrap_or(false) {
            return Ok(None);
        }

        let path_display = path.display().to_string();
        let compression = self.compression;

        // Read and decompress
        let versioned: VersionedCheckpoint = tokio::task::spawn_blocking(move || {
            use std::io::Read;
            let f = std::fs::File::open(&path).map_err(io_err)?;
            let mut reader = std::io::BufReader::new(f);
            let mut data = Vec::new();
            reader.read_to_end(&mut data).map_err(io_err)?;

            // Determine if file is compressed
            let is_compressed = path
                .extension()
                .and_then(|s| s.to_str())
                .map(|s| s == "gz")
                .unwrap_or(false);

            let decompressed = if is_compressed {
                compression.decompress(&data)?
            } else {
                data
            };

            bincode::deserialize(&decompressed).map_err(|e| {
                crate::Error::Checkpoint(crate::error::CheckpointError::DeserializationFailed {
                    reason: format!(
                        "Failed to deserialize checkpoint from '{}': {e}",
                        path.display()
                    ),
                })
            })
        })
        .await
        .map_err(|e| {
            crate::Error::Checkpoint(crate::error::CheckpointError::Other(format!(
                "Task join error loading checkpoint '{}': {e}",
                path_display
            )))
        })??;

        // Migrate if necessary
        let data_value = versioned.data_as_value()?;
        let state = self
            .migrations
            .migrate_to_current(data_value, versioned.version)?;

        Ok(Some(Checkpoint {
            id: versioned.id,
            thread_id: versioned.thread_id,
            state,
            node: versioned.node,
            timestamp: versioned.timestamp,
            parent_id: versioned.parent_id,
            metadata: versioned.metadata,
        }))
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
        let compression = self.compression;
        let mut checkpoints = Vec::new();

        for file in files {
            // Read versioned checkpoint for metadata
            let file_clone = file.clone();
            let file_display = file.display().to_string();
            let result: Result<VersionedCheckpoint> = tokio::task::spawn_blocking(move || {
                use std::io::Read;
                let f = std::fs::File::open(&file_clone).map_err(io_err)?;
                let mut reader = std::io::BufReader::new(f);
                let mut data = Vec::new();
                reader.read_to_end(&mut data).map_err(io_err)?;

                let is_compressed = file_clone
                    .extension()
                    .and_then(|s| s.to_str())
                    .map(|s| s == "gz")
                    .unwrap_or(false);

                let decompressed = if is_compressed {
                    compression.decompress(&data)?
                } else {
                    data
                };

                bincode::deserialize(&decompressed).map_err(|e| {
                    crate::Error::Checkpoint(crate::error::CheckpointError::DeserializationFailed {
                        reason: format!(
                            "Failed to deserialize checkpoint from '{}': {e}",
                            file_clone.display()
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
            })?;

            // Skip corrupt files instead of aborting entire list operation
            match result {
                Ok(versioned) => {
                    if versioned.thread_id == thread_id {
                        checkpoints.push(versioned.to_metadata());
                    }
                }
                Err(e) => {
                    tracing::warn!(
                        file = %file.display(),
                        "Skipping corrupt versioned checkpoint file: {e}"
                    );
                }
            }
        }

        checkpoints.sort_by(|a, b| b.timestamp.cmp(&a.timestamp).then_with(|| b.id.cmp(&a.id)));
        Ok(checkpoints)
    }

    async fn delete(&self, checkpoint_id: &str) -> Result<()> {
        let path = self.checkpoint_path(checkpoint_id);
        // Use async file existence check to avoid blocking the async runtime
        if tokio::fs::try_exists(&path).await.unwrap_or(false) {
            tokio::fs::remove_file(&path).await.map_err(io_err)?;
        }

        // Update index
        {
            let mut index = self.index.lock().unwrap_or_else(|e| e.into_inner());
            index.retain(|_, (id, _)| id != checkpoint_id);
        }
        self.save_index().await?;

        Ok(())
    }

    async fn delete_thread(&self, thread_id: &str) -> Result<()> {
        let files = self.list_files().await?;
        let compression = self.compression;

        for file in files {
            let file_clone = file.clone();
            let file_display = file.display().to_string();
            let versioned: VersionedCheckpoint = tokio::task::spawn_blocking(move || {
                use std::io::Read;
                let f = std::fs::File::open(&file_clone).map_err(io_err)?;
                let mut reader = std::io::BufReader::new(f);
                let mut data = Vec::new();
                reader.read_to_end(&mut data).map_err(io_err)?;

                let is_compressed = file_clone
                    .extension()
                    .and_then(|s| s.to_str())
                    .map(|s| s == "gz")
                    .unwrap_or(false);

                let decompressed = if is_compressed {
                    compression.decompress(&data)?
                } else {
                    data
                };

                bincode::deserialize(&decompressed).map_err(|e| {
                    crate::Error::Checkpoint(crate::error::CheckpointError::DeserializationFailed {
                        reason: format!(
                            "Failed to deserialize checkpoint from '{}': {e}",
                            file_clone.display()
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
            })??;

            if versioned.thread_id == thread_id {
                tokio::fs::remove_file(&file).await.map_err(io_err)?;
            }
        }

        // Update index
        {
            let mut index = self.index.lock().unwrap_or_else(|e| e.into_inner());
            index.remove(thread_id);
        }
        self.save_index().await?;

        Ok(())
    }

    async fn list_threads(&self) -> Result<Vec<ThreadInfo>> {
        // Use the index for O(1) thread listing
        let index = self.index.lock().unwrap_or_else(|e| e.into_inner());

        let mut thread_infos: Vec<ThreadInfo> = index
            .iter()
            .map(|(thread_id, (checkpoint_id, timestamp))| ThreadInfo {
                thread_id: thread_id.clone(),
                latest_checkpoint_id: checkpoint_id.clone(),
                updated_at: *timestamp,
                checkpoint_count: None,
            })
            .collect();

        thread_infos.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
        Ok(thread_infos)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use serde::{Deserialize, Serialize};

    /// Test state for checkpoint tests
    /// Note: GraphState is auto-implemented via blanket impl for Clone+Send+Sync+Serialize+Deserialize
    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    struct TestState {
        counter: i32,
        message: String,
    }

    // ========================================================================
    // VersionedCheckpoint Tests
    // ========================================================================

    #[test]
    fn test_versioned_checkpoint_from_to_checkpoint() {
        let state = TestState {
            counter: 42,
            message: "test".to_string(),
        };
        let checkpoint = Checkpoint {
            id: "cp-001".into(),
            thread_id: "thread-1".into(),
            state: state.clone(),
            node: "node-1".to_string(),
            timestamp: SystemTime::now(),
            parent_id: None,
            metadata: HashMap::new(),
        };

        // Convert to versioned
        let versioned = VersionedCheckpoint::from_checkpoint(&checkpoint, 1).unwrap();
        assert_eq!(versioned.version, 1);
        assert_eq!(versioned.id, "cp-001");
        assert_eq!(versioned.thread_id, "thread-1");
        assert_eq!(versioned.node, "node-1");
        assert!(versioned.parent_id.is_none());

        // Convert back
        let recovered: Checkpoint<TestState> = versioned.to_checkpoint().unwrap();
        assert_eq!(recovered.state, state);
        assert_eq!(recovered.id, "cp-001");
    }

    #[test]
    fn test_versioned_checkpoint_with_parent() {
        let state = TestState {
            counter: 1,
            message: "".to_string(),
        };
        let checkpoint = Checkpoint {
            id: "cp-002".into(),
            thread_id: "t".into(),
            state,
            node: "n".to_string(),
            timestamp: SystemTime::now(),
            parent_id: Some("cp-001".into()),
            metadata: HashMap::new(),
        };

        let versioned = VersionedCheckpoint::from_checkpoint(&checkpoint, 2).unwrap();
        assert_eq!(versioned.parent_id, Some("cp-001".into()));
    }

    #[test]
    fn test_versioned_checkpoint_with_metadata() {
        let state = TestState {
            counter: 0,
            message: "".to_string(),
        };
        let mut meta = HashMap::new();
        meta.insert("key1".to_string(), "value1".to_string());
        meta.insert("key2".to_string(), "value2".to_string());

        let checkpoint = Checkpoint {
            id: "cp-003".into(),
            thread_id: "t".into(),
            state,
            node: "n".to_string(),
            timestamp: SystemTime::now(),
            parent_id: None,
            metadata: meta.clone(),
        };

        let versioned = VersionedCheckpoint::from_checkpoint(&checkpoint, 1).unwrap();
        assert_eq!(versioned.metadata, meta);
    }

    #[test]
    fn test_versioned_checkpoint_data_as_value() {
        let state = TestState {
            counter: 100,
            message: "hello".to_string(),
        };
        let checkpoint = Checkpoint {
            id: "cp".into(),
            thread_id: "t".into(),
            state,
            node: "n".to_string(),
            timestamp: SystemTime::now(),
            parent_id: None,
            metadata: HashMap::new(),
        };

        let versioned = VersionedCheckpoint::from_checkpoint(&checkpoint, 1).unwrap();
        let value = versioned.data_as_value().unwrap();

        assert_eq!(value["counter"], 100);
        assert_eq!(value["message"], "hello");
    }

    #[test]
    fn test_versioned_checkpoint_to_metadata() {
        let state = TestState {
            counter: 0,
            message: "".to_string(),
        };
        let checkpoint = Checkpoint {
            id: "meta-test".into(),
            thread_id: "thread-meta".into(),
            state,
            node: "test-node".to_string(),
            timestamp: SystemTime::now(),
            parent_id: Some("parent-id".into()),
            metadata: HashMap::new(),
        };

        let versioned = VersionedCheckpoint::from_checkpoint(&checkpoint, 1).unwrap();
        let meta = versioned.to_metadata();

        assert_eq!(meta.id, "meta-test");
        assert_eq!(meta.thread_id, "thread-meta");
        assert_eq!(meta.node, "test-node");
        assert_eq!(meta.parent_id, Some("parent-id".into()));
    }

    // ========================================================================
    // MigrationChain Tests
    // ========================================================================

    #[test]
    fn test_migration_chain_new() {
        let chain = MigrationChain::<TestState>::new(5);
        assert_eq!(chain.current_version(), 5);
    }

    #[test]
    fn test_migration_chain_no_migration_needed() {
        let chain = MigrationChain::<TestState>::new(1);
        let data = serde_json::json!({
            "counter": 42,
            "message": "test"
        });

        let result: TestState = chain.migrate_to_current(data, 1).unwrap();
        assert_eq!(result.counter, 42);
        assert_eq!(result.message, "test");
    }

    /// Test state v2 with an extra field
    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    struct TestStateV2 {
        counter: i32,
        message: String,
        enabled: bool,
    }

    struct V1ToV2Migration;

    impl StateMigration<TestStateV2> for V1ToV2Migration {
        fn source_version(&self) -> Version {
            1
        }
        fn target_version(&self) -> Version {
            2
        }
        fn migrate(&self, data: serde_json::Value) -> std::result::Result<TestStateV2, String> {
            let counter = data["counter"].as_i64().ok_or("missing counter")? as i32;
            let message = data["message"]
                .as_str()
                .ok_or("missing message")?
                .to_string();
            Ok(TestStateV2 {
                counter,
                message,
                enabled: true, // Default for migrated data
            })
        }
    }

    #[test]
    fn test_migration_chain_single_migration() {
        let chain = MigrationChain::<TestStateV2>::new(2).add_migration(V1ToV2Migration);

        let data = serde_json::json!({
            "counter": 10,
            "message": "old"
        });

        let result = chain.migrate_to_current(data, 1).unwrap();
        assert_eq!(result.counter, 10);
        assert_eq!(result.message, "old");
        assert!(result.enabled);
    }

    #[test]
    fn test_migration_chain_missing_migration() {
        let chain = MigrationChain::<TestState>::new(3); // No migrations added

        let data = serde_json::json!({
            "counter": 1,
            "message": ""
        });

        let result = chain.migrate_to_current(data, 1);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(format!("{:?}", err).contains("MigrationFailed"));
    }

    /// Test state v3 with yet another field
    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    struct TestStateV3 {
        counter: i32,
        message: String,
        enabled: bool,
        tags: Vec<String>,
    }

    struct V2ToV3Migration;

    impl StateMigration<TestStateV3> for V2ToV3Migration {
        fn source_version(&self) -> Version {
            2
        }
        fn target_version(&self) -> Version {
            3
        }
        fn migrate(&self, data: serde_json::Value) -> std::result::Result<TestStateV3, String> {
            let counter = data["counter"].as_i64().ok_or("missing counter")? as i32;
            let message = data["message"]
                .as_str()
                .ok_or("missing message")?
                .to_string();
            let enabled = data["enabled"].as_bool().unwrap_or(false);
            Ok(TestStateV3 {
                counter,
                message,
                enabled,
                tags: vec![], // Default for migrated data
            })
        }
    }

    struct V1ToV3DirectMigration;

    impl StateMigration<TestStateV3> for V1ToV3DirectMigration {
        fn source_version(&self) -> Version {
            1
        }
        fn target_version(&self) -> Version {
            3
        }
        fn migrate(&self, data: serde_json::Value) -> std::result::Result<TestStateV3, String> {
            let counter = data["counter"].as_i64().ok_or("missing counter")? as i32;
            let message = data["message"]
                .as_str()
                .ok_or("missing message")?
                .to_string();
            Ok(TestStateV3 {
                counter,
                message,
                enabled: true,
                tags: vec!["migrated".to_string()],
            })
        }
    }

    #[test]
    fn test_migration_chain_multi_step() {
        // Note: Chain migrations require intermediate migrations
        // This test shows that we need a direct 1->2->3 path or 1->3
        let chain =
            MigrationChain::<TestStateV3>::new(3).add_migration(V1ToV3DirectMigration);

        let data = serde_json::json!({
            "counter": 5,
            "message": "v1 data"
        });

        let result = chain.migrate_to_current(data, 1).unwrap();
        assert_eq!(result.counter, 5);
        assert_eq!(result.message, "v1 data");
        assert!(result.enabled);
        assert_eq!(result.tags, vec!["migrated".to_string()]);
    }

    // ========================================================================
    // VersionedFileCheckpointer Tests
    // ========================================================================

    #[test]
    fn test_versioned_file_checkpointer_new() {
        let temp_dir = tempfile::tempdir().unwrap();
        let migrations = MigrationChain::<TestState>::new(1);

        let checkpointer =
            VersionedFileCheckpointer::new(temp_dir.path(), migrations).unwrap();
        assert_eq!(checkpointer.current_version(), 1);
    }

    #[test]
    fn test_versioned_file_checkpointer_with_compression() {
        let temp_dir = tempfile::tempdir().unwrap();
        let migrations = MigrationChain::<TestState>::new(1);

        let checkpointer = VersionedFileCheckpointer::new(temp_dir.path(), migrations)
            .unwrap()
            .with_compression(CompressionAlgorithm::Gzip { level: 6 });
        // Just verify it doesn't panic
        assert_eq!(checkpointer.current_version(), 1);
    }

    #[test]
    fn test_versioned_file_checkpointer_creates_directory() {
        let temp_dir = tempfile::tempdir().unwrap();
        let new_path = temp_dir.path().join("nested").join("checkpoints");
        let migrations = MigrationChain::<TestState>::new(1);

        let _checkpointer = VersionedFileCheckpointer::new(&new_path, migrations).unwrap();
        assert!(new_path.exists());
    }

    #[tokio::test]
    async fn test_versioned_file_checkpointer_save_and_load() {
        use crate::checkpoint::Checkpointer;

        let temp_dir = tempfile::tempdir().unwrap();
        let migrations = MigrationChain::<TestState>::new(1);
        let checkpointer =
            VersionedFileCheckpointer::new(temp_dir.path(), migrations).unwrap();

        let state = TestState {
            counter: 42,
            message: "hello world".to_string(),
        };
        let checkpoint = Checkpoint {
            id: "test-id".into(),
            thread_id: "test-thread".into(),
            state: state.clone(),
            node: "test-node".to_string(),
            timestamp: SystemTime::now(),
            parent_id: None,
            metadata: HashMap::new(),
        };

        // Save
        checkpointer.save(checkpoint).await.unwrap();

        // Load by checkpoint ID
        let loaded: Option<Checkpoint<TestState>> =
            checkpointer.load("test-id").await.unwrap();
        assert!(loaded.is_some());
        let loaded = loaded.unwrap();
        assert_eq!(loaded.state, state);
        assert_eq!(loaded.id, "test-id");

        // Also test get_latest by thread ID
        let latest: Option<Checkpoint<TestState>> =
            checkpointer.get_latest("test-thread").await.unwrap();
        assert!(latest.is_some());
        assert_eq!(latest.unwrap().state, state);
    }

    #[tokio::test]
    async fn test_versioned_file_checkpointer_load_nonexistent() {
        use crate::checkpoint::Checkpointer;

        let temp_dir = tempfile::tempdir().unwrap();
        let migrations = MigrationChain::<TestState>::new(1);
        let checkpointer =
            VersionedFileCheckpointer::new(temp_dir.path(), migrations).unwrap();

        // Load by checkpoint ID that doesn't exist
        let loaded: Option<Checkpoint<TestState>> =
            checkpointer.load("nonexistent").await.unwrap();
        assert!(loaded.is_none());

        // Get latest by thread ID that has no checkpoints
        let latest: Option<Checkpoint<TestState>> =
            checkpointer.get_latest("nonexistent-thread").await.unwrap();
        assert!(latest.is_none());
    }

    #[tokio::test]
    async fn test_versioned_file_checkpointer_list() {
        use crate::checkpoint::Checkpointer;

        let temp_dir = tempfile::tempdir().unwrap();
        let migrations = MigrationChain::<TestState>::new(1);
        let checkpointer =
            VersionedFileCheckpointer::new(temp_dir.path(), migrations).unwrap();

        let state = TestState {
            counter: 0,
            message: "".to_string(),
        };

        // Save multiple checkpoints for same thread
        for i in 0..3 {
            let checkpoint = Checkpoint {
                id: format!("cp-{i}"),
                thread_id: "thread-1".into(),
                state: state.clone(),
                node: "node".to_string(),
                timestamp: SystemTime::now(),
                parent_id: if i > 0 {
                    Some(format!("cp-{}", i - 1))
                } else {
                    None
                },
                metadata: HashMap::new(),
            };
            checkpointer.save(checkpoint).await.unwrap();
        }

        // List checkpoints
        let list = checkpointer.list("thread-1").await.unwrap();
        assert_eq!(list.len(), 3);
    }

    #[tokio::test]
    async fn test_versioned_file_checkpointer_delete() {
        use crate::checkpoint::Checkpointer;

        let temp_dir = tempfile::tempdir().unwrap();
        let migrations = MigrationChain::<TestState>::new(1);
        let checkpointer =
            VersionedFileCheckpointer::new(temp_dir.path(), migrations).unwrap();

        let state = TestState {
            counter: 1,
            message: "".to_string(),
        };
        let checkpoint = Checkpoint {
            id: "to-delete".into(),
            thread_id: "thread-del".into(),
            state,
            node: "node".to_string(),
            timestamp: SystemTime::now(),
            parent_id: None,
            metadata: HashMap::new(),
        };

        checkpointer.save(checkpoint).await.unwrap();

        // Verify exists by checkpoint ID
        let loaded: Option<Checkpoint<TestState>> =
            checkpointer.load("to-delete").await.unwrap();
        assert!(loaded.is_some());

        // Verify exists by thread ID
        let latest: Option<Checkpoint<TestState>> =
            checkpointer.get_latest("thread-del").await.unwrap();
        assert!(latest.is_some());

        // Delete
        checkpointer.delete("to-delete").await.unwrap();

        // Verify deleted by checkpoint ID
        let loaded: Option<Checkpoint<TestState>> =
            checkpointer.load("to-delete").await.unwrap();
        assert!(loaded.is_none());

        // Verify index is updated (get_latest should now return None)
        let latest: Option<Checkpoint<TestState>> =
            checkpointer.get_latest("thread-del").await.unwrap();
        assert!(latest.is_none());
    }

    #[tokio::test]
    async fn test_versioned_file_checkpointer_list_threads() {
        use crate::checkpoint::Checkpointer;

        let temp_dir = tempfile::tempdir().unwrap();
        let migrations = MigrationChain::<TestState>::new(1);
        let checkpointer =
            VersionedFileCheckpointer::new(temp_dir.path(), migrations).unwrap();

        let state = TestState {
            counter: 0,
            message: "".to_string(),
        };

        // Save checkpoints for different threads
        for thread_num in 0..3 {
            let checkpoint = Checkpoint {
                id: format!("cp-thread-{thread_num}"),
                thread_id: format!("thread-{thread_num}"),
                state: state.clone(),
                node: "node".to_string(),
                timestamp: SystemTime::now(),
                parent_id: None,
                metadata: HashMap::new(),
            };
            checkpointer.save(checkpoint).await.unwrap();
        }

        let threads = checkpointer.list_threads().await.unwrap();
        assert_eq!(threads.len(), 3);
    }

    // ========================================================================
    // Serialization Tests
    // ========================================================================

    #[test]
    fn test_versioned_checkpoint_serialization() {
        let state = TestState {
            counter: 99,
            message: "serialize me".to_string(),
        };
        let checkpoint = Checkpoint {
            id: "ser-test".into(),
            thread_id: "t".into(),
            state,
            node: "n".to_string(),
            timestamp: SystemTime::now(),
            parent_id: None,
            metadata: HashMap::new(),
        };

        let versioned = VersionedCheckpoint::from_checkpoint(&checkpoint, 3).unwrap();

        // Serialize to bincode (how it's stored on disk)
        let bytes = bincode::serialize(&versioned).unwrap();
        assert!(!bytes.is_empty());

        // Deserialize
        let recovered: VersionedCheckpoint = bincode::deserialize(&bytes).unwrap();
        assert_eq!(recovered.version, 3);
        assert_eq!(recovered.id, "ser-test");

        // Verify state round-trips
        let recovered_state: TestState = serde_json::from_str(&recovered.data).unwrap();
        assert_eq!(recovered_state.counter, 99);
        assert_eq!(recovered_state.message, "serialize me");
    }

    #[test]
    fn test_versioned_checkpoint_json_serialization() {
        let state = TestState {
            counter: 1,
            message: "json".to_string(),
        };
        let checkpoint = Checkpoint {
            id: "json-test".into(),
            thread_id: "t".into(),
            state,
            node: "n".to_string(),
            timestamp: SystemTime::now(),
            parent_id: None,
            metadata: HashMap::new(),
        };

        let versioned = VersionedCheckpoint::from_checkpoint(&checkpoint, 1).unwrap();

        // Serialize to JSON
        let json = serde_json::to_string(&versioned).unwrap();
        assert!(json.contains("version"));
        assert!(json.contains("json-test"));

        // Deserialize
        let recovered: VersionedCheckpoint = serde_json::from_str(&json).unwrap();
        assert_eq!(recovered.id, "json-test");
    }

    // ========================================================================
    // Edge Case Tests
    // ========================================================================

    #[test]
    fn test_versioned_checkpoint_empty_state() {
        #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
        struct EmptyState {}

        let checkpoint = Checkpoint {
            id: "empty".into(),
            thread_id: "t".into(),
            state: EmptyState {},
            node: "n".to_string(),
            timestamp: SystemTime::now(),
            parent_id: None,
            metadata: HashMap::new(),
        };

        let versioned = VersionedCheckpoint::from_checkpoint(&checkpoint, 1).unwrap();
        assert_eq!(versioned.data, "{}");

        let recovered: Checkpoint<EmptyState> = versioned.to_checkpoint().unwrap();
        assert_eq!(recovered.state, EmptyState {});
    }

    #[test]
    fn test_versioned_checkpoint_complex_state() {
        #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
        struct ComplexState {
            strings: Vec<String>,
            nested: HashMap<String, i32>,
            optional: Option<bool>,
        }

        let mut nested = HashMap::new();
        nested.insert("a".to_string(), 1);
        nested.insert("b".to_string(), 2);

        let state = ComplexState {
            strings: vec!["one".to_string(), "two".to_string()],
            nested,
            optional: Some(true),
        };

        let checkpoint = Checkpoint {
            id: "complex".into(),
            thread_id: "t".into(),
            state: state.clone(),
            node: "n".to_string(),
            timestamp: SystemTime::now(),
            parent_id: None,
            metadata: HashMap::new(),
        };

        let versioned = VersionedCheckpoint::from_checkpoint(&checkpoint, 1).unwrap();
        let recovered: Checkpoint<ComplexState> = versioned.to_checkpoint().unwrap();
        assert_eq!(recovered.state, state);
    }

    #[test]
    fn test_migration_chain_preserves_order() {
        // Migration should be applied by source version, not insertion order
        let chain = MigrationChain::<TestStateV3>::new(3)
            .add_migration(V2ToV3Migration)
            .add_migration(V1ToV3DirectMigration); // 1->3 direct

        let v1_data = serde_json::json!({
            "counter": 1,
            "message": "v1"
        });

        // Should use 1->3 migration (first matching source)
        let result = chain.migrate_to_current(v1_data, 1).unwrap();
        assert_eq!(result.tags, vec!["migrated".to_string()]);
    }
}
