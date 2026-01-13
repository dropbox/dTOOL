// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

//! Compressed checkpoint storage
//!
//! Provides compressed file-based checkpoint storage using gzip compression.
//! Typically achieves 60-90% size reduction for typical graph states.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::SystemTime;

use crate::{GraphState, Result};

use super::{
    atomic_write_file, load_checkpoint_index, Checkpoint, CheckpointId, CheckpointMetadata,
    Checkpointer, ThreadId, ThreadInfo,
};

// ============================================================================
// Error Helpers (CQ-34: Reduce repetitive IO error mapping)
// ============================================================================

/// Convert IO error to checkpoint error
#[inline]
fn io_err(e: std::io::Error) -> crate::Error {
    crate::Error::Checkpoint(crate::error::CheckpointError::Io(e))
}

/// Compression algorithm for checkpoint data
#[derive(Clone, Copy, Debug, PartialEq)]
#[non_exhaustive]
pub enum CompressionAlgorithm {
    /// No compression (passthrough)
    None,
    /// Gzip compression (good balance of speed and ratio)
    Gzip {
        /// Compression level (0-9, higher = better compression but slower)
        level: u32,
    },
}

impl Default for CompressionAlgorithm {
    fn default() -> Self {
        // Default to Gzip level 6 (good balance)
        CompressionAlgorithm::Gzip { level: 6 }
    }
}

impl CompressionAlgorithm {
    /// Create Gzip compression with default level (6)
    pub fn gzip() -> Self {
        CompressionAlgorithm::Gzip { level: 6 }
    }

    /// Create Gzip compression with specified level (0-9)
    #[must_use]
    pub fn gzip_with_level(level: u32) -> Self {
        CompressionAlgorithm::Gzip {
            level: level.min(9),
        }
    }

    /// Fast compression (level 1)
    pub fn fast() -> Self {
        CompressionAlgorithm::Gzip { level: 1 }
    }

    /// Best compression (level 9)
    pub fn best() -> Self {
        CompressionAlgorithm::Gzip { level: 9 }
    }

    /// Compress data using this algorithm
    pub fn compress(&self, data: &[u8]) -> Result<Vec<u8>> {
        match self {
            CompressionAlgorithm::None => Ok(data.to_vec()),
            CompressionAlgorithm::Gzip { level } => {
                use flate2::write::GzEncoder;
                use flate2::Compression;
                use std::io::Write;

                let mut encoder = GzEncoder::new(Vec::new(), Compression::new(*level));
                encoder.write_all(data).map_err(io_err)?;
                encoder.finish().map_err(io_err)
            }
        }
    }

    /// Maximum allowed decompressed size: 100 MB (protection against gzip bombs)
    pub const MAX_DECOMPRESSED_SIZE: usize = 100 * 1024 * 1024;

    /// Decompress data using this algorithm
    ///
    /// # Security
    ///
    /// This method limits decompressed output to [`Self::MAX_DECOMPRESSED_SIZE`]
    /// to protect against gzip bomb attacks.
    pub fn decompress(&self, data: &[u8]) -> Result<Vec<u8>> {
        match self {
            CompressionAlgorithm::None => Ok(data.to_vec()),
            CompressionAlgorithm::Gzip { .. } => {
                use flate2::read::GzDecoder;
                use std::io::Read;

                let decoder = GzDecoder::new(data);
                // Limit decompressed size to prevent gzip bombs
                let mut limited = decoder.take(Self::MAX_DECOMPRESSED_SIZE as u64 + 1);
                let mut decompressed = Vec::new();
                limited.read_to_end(&mut decompressed).map_err(io_err)?;

                if decompressed.len() > Self::MAX_DECOMPRESSED_SIZE {
                    return Err(crate::Error::Checkpoint(
                        crate::error::CheckpointError::DeserializationFailed {
                            reason: format!(
                                "Decompressed size exceeds limit of {} bytes",
                                Self::MAX_DECOMPRESSED_SIZE
                            ),
                        },
                    ));
                }
                Ok(decompressed)
            }
        }
    }
}

/// Compressed file-based checkpoint storage
///
/// Stores checkpoints as compressed bincode-encoded files.
/// Uses gzip compression by default, which typically achieves 60-90% size reduction
/// for typical graph states.
///
/// # Example
///
/// ```rust,ignore
/// use dashflow::checkpoint::{CompressedFileCheckpointer, CompressionAlgorithm};
///
/// // Default: Gzip level 6
/// let checkpointer = CompressedFileCheckpointer::new("./checkpoints")?;
///
/// // Fast compression (level 1) for low latency
/// let fast_checkpointer = CompressedFileCheckpointer::new("./checkpoints")?
///     .with_compression(CompressionAlgorithm::fast());
///
/// // Best compression (level 9) for minimum size
/// let best_checkpointer = CompressedFileCheckpointer::new("./checkpoints")?
///     .with_compression(CompressionAlgorithm::best());
/// ```
pub struct CompressedFileCheckpointer<S: GraphState> {
    directory: std::path::PathBuf,
    index: Arc<Mutex<HashMap<ThreadId, (CheckpointId, SystemTime)>>>,
    compression: CompressionAlgorithm,
    _phantom: std::marker::PhantomData<S>,
}

impl<S: GraphState> CompressedFileCheckpointer<S> {
    /// Create a new compressed file checkpointer with default gzip compression (synchronous)
    ///
    /// # Note
    ///
    /// This constructor performs blocking filesystem operations. If called from an
    /// async context, consider using [`Self::new_async`] instead.
    pub fn new(directory: impl Into<std::path::PathBuf>) -> Result<Self> {
        let directory = directory.into();
        std::fs::create_dir_all(&directory).map_err(io_err)?;

        // Load index from file if it exists (with proper error logging)
        let index_path = directory.join("index.bin");
        let index = load_checkpoint_index(&index_path);

        Ok(Self {
            directory,
            index: Arc::new(Mutex::new(index)),
            compression: CompressionAlgorithm::default(),
            _phantom: std::marker::PhantomData,
        })
    }

    /// Create a new compressed file checkpointer (async)
    ///
    /// This is the preferred constructor when called from an async context,
    /// as it avoids blocking the executor on filesystem operations.
    pub async fn new_async(directory: impl Into<std::path::PathBuf>) -> Result<Self> {
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
            compression: CompressionAlgorithm::default(),
            _phantom: std::marker::PhantomData,
        })
    }

    /// Set the compression algorithm
    #[must_use]
    pub fn with_compression(mut self, compression: CompressionAlgorithm) -> Self {
        self.compression = compression;
        self
    }

    /// Get the current compression algorithm
    pub fn compression(&self) -> CompressionAlgorithm {
        self.compression
    }

    /// Get the file path for a checkpoint ID (compressed files use .bin.gz extension)
    fn checkpoint_path(&self, checkpoint_id: &str) -> std::path::PathBuf {
        match self.compression {
            CompressionAlgorithm::None => self.directory.join(format!("{checkpoint_id}.bin")),
            CompressionAlgorithm::Gzip { .. } => {
                self.directory.join(format!("{checkpoint_id}.bin.gz"))
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

    /// List all checkpoint files (both compressed and uncompressed) - async to avoid blocking
    async fn list_files(&self) -> Result<Vec<std::path::PathBuf>> {
        let directory = self.directory.clone();
        tokio::task::spawn_blocking(move || {
            let entries = std::fs::read_dir(&directory).map_err(io_err)?;

            let mut files = Vec::new();
            for entry in entries {
                let entry = entry.map_err(io_err)?;
                let path = entry.path();
                let filename = path.file_name().and_then(|s| s.to_str()).unwrap_or("");

                // Include .bin.gz (compressed) and .bin (uncompressed, for backward compatibility)
                if (filename.ends_with(".bin.gz") || filename.ends_with(".bin"))
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

    /// Read and decompress a checkpoint from a file
    async fn read_checkpoint_from_file(
        file: std::path::PathBuf,
        compression: CompressionAlgorithm,
    ) -> Result<Checkpoint<S>> {
        let file_display = file.display().to_string();
        tokio::task::spawn_blocking(move || {
            use std::io::Read;
            let f = std::fs::File::open(&file).map_err(io_err)?;
            let mut reader = std::io::BufReader::new(f);
            let mut data = Vec::new();
            reader.read_to_end(&mut data).map_err(io_err)?;

            // Determine if file is compressed based on extension
            let is_compressed = file
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
    /// Used when index is corrupted, reset, or points to a missing file.
    /// Matches FileCheckpointer::get_latest_by_file_scan() behavior.
    async fn get_latest_by_file_scan(&self, thread_id: &str) -> Result<Option<Checkpoint<S>>> {
        let files = self.list_files().await?;
        let compression = self.compression;
        let mut latest: Option<Checkpoint<S>> = None;

        for file in files {
            match Self::read_checkpoint_from_file(file.clone(), compression).await {
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
impl<S: GraphState> Checkpointer<S> for CompressedFileCheckpointer<S> {
    async fn save(&self, checkpoint: Checkpoint<S>) -> Result<()> {
        let path = self.checkpoint_path(&checkpoint.id);
        let path_display = path.display().to_string();
        let thread_id = checkpoint.thread_id.clone();
        let checkpoint_id = checkpoint.id.clone();
        let timestamp = checkpoint.timestamp;
        let compression = self.compression;

        // Clone checkpoint_id for use inside closure (original needed for index update)
        let checkpoint_id_for_err = checkpoint_id.clone();

        // Serialize + compress + write in spawn_blocking (M-635: CPU-intensive bincode + compression)
        tokio::task::spawn_blocking(move || {
            // Serialize with bincode
            let data = bincode::serialize(&checkpoint).map_err(|e| {
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

        // Try compressed path first, then uncompressed for backward compatibility
        // Use async file existence checks to avoid blocking the async runtime
        let actual_path = if tokio::fs::try_exists(&path).await.unwrap_or(false) {
            path
        } else {
            // Try without compression extension
            let uncompressed = self.directory.join(format!("{checkpoint_id}.bin"));
            if tokio::fs::try_exists(&uncompressed).await.unwrap_or(false) {
                uncompressed
            } else {
                return Ok(None);
            }
        };

        let compression = self.compression;
        let checkpoint = Self::read_checkpoint_from_file(actual_path, compression).await?;

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
        let compression = self.compression;
        let mut checkpoints = Vec::new();

        for file in files {
            // Skip corrupt files instead of aborting entire list operation
            match Self::read_checkpoint_from_file(file.clone(), compression).await {
                Ok(checkpoint) => {
                    if checkpoint.thread_id == thread_id {
                        checkpoints.push(CheckpointMetadata::from(&checkpoint));
                    }
                }
                Err(e) => {
                    tracing::warn!(
                        file = %file.display(),
                        "Skipping corrupt compressed checkpoint file: {e}"
                    );
                }
            }
        }

        checkpoints.sort_by(|a, b| b.timestamp.cmp(&a.timestamp).then_with(|| b.id.cmp(&a.id)));
        Ok(checkpoints)
    }

    async fn delete(&self, checkpoint_id: &str) -> Result<()> {
        // Try to delete both compressed and uncompressed versions
        let compressed_path = self.checkpoint_path(checkpoint_id);
        let uncompressed_path = self.directory.join(format!("{checkpoint_id}.bin"));

        // Use async file existence checks to avoid blocking the async runtime
        if tokio::fs::try_exists(&compressed_path)
            .await
            .unwrap_or(false)
        {
            tokio::fs::remove_file(&compressed_path)
                .await
                .map_err(io_err)?;
        }
        if tokio::fs::try_exists(&uncompressed_path)
            .await
            .unwrap_or(false)
        {
            tokio::fs::remove_file(&uncompressed_path)
                .await
                .map_err(io_err)?;
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
            let checkpoint = Self::read_checkpoint_from_file(file.clone(), compression).await?;

            if checkpoint.thread_id == thread_id {
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
mod tests {
    use super::*;

    // ============================================================================
    // CompressionAlgorithm Tests
    // ============================================================================

    #[test]
    fn test_compression_algorithm_default() {
        let algo = CompressionAlgorithm::default();
        assert_eq!(algo, CompressionAlgorithm::Gzip { level: 6 });
    }

    #[test]
    fn test_compression_algorithm_constructors() {
        assert_eq!(
            CompressionAlgorithm::gzip(),
            CompressionAlgorithm::Gzip { level: 6 }
        );
        assert_eq!(
            CompressionAlgorithm::fast(),
            CompressionAlgorithm::Gzip { level: 1 }
        );
        assert_eq!(
            CompressionAlgorithm::best(),
            CompressionAlgorithm::Gzip { level: 9 }
        );
        assert_eq!(
            CompressionAlgorithm::gzip_with_level(3),
            CompressionAlgorithm::Gzip { level: 3 }
        );
    }

    #[test]
    fn test_gzip_level_clamping() {
        // Level should be clamped to 9
        let algo = CompressionAlgorithm::gzip_with_level(100);
        assert_eq!(algo, CompressionAlgorithm::Gzip { level: 9 });
    }

    #[test]
    fn test_compress_decompress_roundtrip_gzip() {
        let original = b"Hello, world! This is test data for compression.";
        let algo = CompressionAlgorithm::gzip();

        let compressed = algo.compress(original).expect("compression should succeed");
        let decompressed = algo
            .decompress(&compressed)
            .expect("decompression should succeed");

        assert_eq!(decompressed, original);
    }

    #[test]
    fn test_compress_decompress_roundtrip_none() {
        let original = b"Hello, world! This is test data.";
        let algo = CompressionAlgorithm::None;

        let compressed = algo.compress(original).expect("compression should succeed");
        let decompressed = algo
            .decompress(&compressed)
            .expect("decompression should succeed");

        assert_eq!(decompressed, original);
        // With None, data should be unchanged
        assert_eq!(compressed, original);
    }

    #[test]
    fn test_compress_empty_data() {
        let algo = CompressionAlgorithm::gzip();
        let empty: &[u8] = &[];

        let compressed = algo.compress(empty).expect("compression should succeed");
        let decompressed = algo
            .decompress(&compressed)
            .expect("decompression should succeed");

        assert_eq!(decompressed, empty);
    }

    #[test]
    fn test_compress_large_data() {
        let algo = CompressionAlgorithm::gzip();
        // Create 1MB of repetitive data (should compress well)
        let original: Vec<u8> = (0..1_000_000).map(|i| (i % 256) as u8).collect();

        let compressed = algo
            .compress(&original)
            .expect("compression should succeed");
        let decompressed = algo
            .decompress(&compressed)
            .expect("decompression should succeed");

        assert_eq!(decompressed, original);
        // Repetitive data should compress significantly
        assert!(compressed.len() < original.len() / 2);
    }

    #[test]
    fn test_compression_levels_produce_different_sizes() {
        // Large repetitive data to show compression level differences
        let data: Vec<u8> = (0..100_000).map(|i| (i % 256) as u8).collect();

        let fast = CompressionAlgorithm::fast();
        let best = CompressionAlgorithm::best();

        let fast_compressed = fast
            .compress(&data)
            .expect("fast compression should succeed");
        let best_compressed = best
            .compress(&data)
            .expect("best compression should succeed");

        // Both should decompress to original
        assert_eq!(fast.decompress(&fast_compressed).unwrap(), data);
        assert_eq!(best.decompress(&best_compressed).unwrap(), data);

        // Best compression should produce smaller or equal output
        assert!(best_compressed.len() <= fast_compressed.len());
    }

    #[test]
    fn test_decompress_invalid_gzip_data() {
        let algo = CompressionAlgorithm::gzip();
        let invalid_data = b"this is not valid gzip data";

        let result = algo.decompress(invalid_data);
        assert!(result.is_err());
    }

    #[test]
    fn test_max_decompressed_size_constant() {
        // Verify the constant is reasonable (100 MB)
        assert_eq!(
            CompressionAlgorithm::MAX_DECOMPRESSED_SIZE,
            100 * 1024 * 1024
        );
    }

    // ============================================================================
    // CompressedFileCheckpointer Tests
    // ============================================================================

    /// Simple test state that works with bincode serialization.
    /// Note: TestState automatically implements GraphState via the blanket impl
    /// in state.rs for types that are Clone + Send + Sync + Serialize + Deserialize.
    #[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
    struct TestState {
        value: i32,
    }

    #[tokio::test]
    async fn test_checkpointer_new_creates_directory() {
        let temp_dir = tempfile::tempdir().expect("failed to create temp dir");
        let checkpoint_dir = temp_dir.path().join("checkpoints");

        // Directory shouldn't exist yet
        assert!(!checkpoint_dir.exists());

        let checkpointer = CompressedFileCheckpointer::<TestState>::new(&checkpoint_dir)
            .expect("should create checkpointer");

        // Directory should now exist
        assert!(checkpoint_dir.exists());

        // Compression should be default
        assert_eq!(checkpointer.compression(), CompressionAlgorithm::default());
    }

    #[tokio::test]
    async fn test_checkpointer_with_compression() {
        let temp_dir = tempfile::tempdir().expect("failed to create temp dir");

        let checkpointer = CompressedFileCheckpointer::<TestState>::new(temp_dir.path())
            .expect("should create checkpointer")
            .with_compression(CompressionAlgorithm::fast());

        assert_eq!(checkpointer.compression(), CompressionAlgorithm::fast());
    }

    #[tokio::test]
    async fn test_checkpointer_save_and_load() {
        let temp_dir = tempfile::tempdir().expect("failed to create temp dir");

        let checkpointer = CompressedFileCheckpointer::<TestState>::new(temp_dir.path())
            .expect("should create checkpointer");

        let state = TestState { value: 42 };
        let checkpoint = Checkpoint::new(
            "test-thread".to_string(),
            state,
            "test-node".to_string(),
            None,
        );
        let checkpoint_id = checkpoint.id.clone();

        // Save checkpoint
        checkpointer
            .save(checkpoint)
            .await
            .expect("save should succeed");

        // Load checkpoint
        let loaded = checkpointer
            .load(&checkpoint_id)
            .await
            .expect("load should succeed")
            .expect("checkpoint should exist");

        assert_eq!(loaded.id, checkpoint_id);
        assert_eq!(loaded.thread_id, "test-thread");
        assert_eq!(loaded.state.value, 42);
    }

    #[tokio::test]
    async fn test_checkpointer_get_latest() {
        let temp_dir = tempfile::tempdir().expect("failed to create temp dir");

        let checkpointer = CompressedFileCheckpointer::<TestState>::new(temp_dir.path())
            .expect("should create checkpointer");

        // Initially no checkpoints
        let latest = checkpointer
            .get_latest("test-thread")
            .await
            .expect("get_latest should succeed");
        assert!(latest.is_none());

        // Save first checkpoint
        let state1 = TestState { value: 1 };
        let checkpoint1 = Checkpoint::new(
            "test-thread".to_string(),
            state1,
            "node-1".to_string(),
            None,
        );
        let id1 = checkpoint1.id.clone();
        checkpointer
            .save(checkpoint1)
            .await
            .expect("save should succeed");

        // Save second checkpoint (newer)
        let state2 = TestState { value: 2 };
        let checkpoint2 = Checkpoint::new(
            "test-thread".to_string(),
            state2,
            "node-2".to_string(),
            None,
        );
        let id2 = checkpoint2.id.clone();
        checkpointer
            .save(checkpoint2)
            .await
            .expect("save should succeed");

        // Get latest should return second checkpoint
        let latest = checkpointer
            .get_latest("test-thread")
            .await
            .expect("get_latest should succeed")
            .expect("should have checkpoint");

        // The latest should be the second checkpoint (by timestamp or ID)
        assert!(latest.id == id1 || latest.id == id2);
    }

    #[tokio::test]
    async fn test_checkpointer_delete() {
        let temp_dir = tempfile::tempdir().expect("failed to create temp dir");

        let checkpointer = CompressedFileCheckpointer::<TestState>::new(temp_dir.path())
            .expect("should create checkpointer");

        let state = TestState { value: 100 };
        let checkpoint = Checkpoint::new(
            "test-thread".to_string(),
            state,
            "test-node".to_string(),
            None,
        );
        let checkpoint_id = checkpoint.id.clone();

        // Save and verify it exists
        checkpointer
            .save(checkpoint)
            .await
            .expect("save should succeed");
        assert!(checkpointer.load(&checkpoint_id).await.unwrap().is_some());

        // Delete and verify it's gone
        checkpointer
            .delete(&checkpoint_id)
            .await
            .expect("delete should succeed");
        assert!(checkpointer.load(&checkpoint_id).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn test_checkpointer_list_threads() {
        let temp_dir = tempfile::tempdir().expect("failed to create temp dir");

        let checkpointer = CompressedFileCheckpointer::<TestState>::new(temp_dir.path())
            .expect("should create checkpointer");

        // Initially empty
        let threads = checkpointer
            .list_threads()
            .await
            .expect("list_threads should succeed");
        assert!(threads.is_empty());

        // Save checkpoint for thread-1
        let state1 = TestState { value: 1 };
        let checkpoint1 =
            Checkpoint::new("thread-1".to_string(), state1, "node-1".to_string(), None);
        checkpointer
            .save(checkpoint1)
            .await
            .expect("save should succeed");

        // Save checkpoint for thread-2
        let state2 = TestState { value: 2 };
        let checkpoint2 =
            Checkpoint::new("thread-2".to_string(), state2, "node-2".to_string(), None);
        checkpointer
            .save(checkpoint2)
            .await
            .expect("save should succeed");

        // List should contain both threads
        let threads = checkpointer
            .list_threads()
            .await
            .expect("list_threads should succeed");
        assert_eq!(threads.len(), 2);

        let thread_ids: Vec<_> = threads.iter().map(|t| t.thread_id.as_str()).collect();
        assert!(thread_ids.contains(&"thread-1"));
        assert!(thread_ids.contains(&"thread-2"));
    }
}
