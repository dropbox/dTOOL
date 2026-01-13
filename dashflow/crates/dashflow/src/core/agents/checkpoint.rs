//! Checkpoint system for agent state persistence and recovery.
//!
//! This module provides checkpointing capabilities for agents, enabling:
//! - State persistence during long-running agent executions
//! - Recovery from failures mid-execution
//! - Inspection of intermediate states for debugging
//!
//! # Example
//!
//! ```rust
//! use dashflow::core::agents::{AgentCheckpointState, AgentContext, Checkpoint, MemoryCheckpoint};
//!
//! # async fn example() -> dashflow::core::error::Result<()> {
//! let mut checkpoint = MemoryCheckpoint::new();
//!
//! // Create a context and save it
//! let context = AgentContext::new("my input");
//! let state = AgentCheckpointState::from_context(&context);
//! checkpoint.save_state("my-checkpoint", &state).await?;
//!
//! // Later, restore it
//! let restored = checkpoint.load_state("my-checkpoint").await?;
//! let _context = restored.to_context();
//! # Ok(())
//! # }
//! ```

use super::{AgentContext, AgentStep};
use crate::core::error::Result;

/// Serializable snapshot of agent execution state.
///
/// Contains all information needed to resume agent execution from a saved point,
/// including input, intermediate steps, iteration count, and metadata.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AgentCheckpointState {
    /// The original input to the agent.
    pub input: String,
    /// Actions taken and their observations so far.
    pub intermediate_steps: Vec<AgentStep>,
    /// Current iteration count (for max_iterations enforcement).
    pub iteration: usize,
    /// Custom metadata for application-specific state.
    pub metadata: std::collections::HashMap<String, String>,
    /// When this checkpoint was created.
    pub timestamp: std::time::SystemTime,
}

impl AgentCheckpointState {
    /// Creates a checkpoint state from an agent context.
    ///
    /// Captures the current state including input, steps, iteration count,
    /// and metadata. The timestamp is set to the current system time.
    #[must_use]
    pub fn from_context(context: &AgentContext) -> Self {
        Self {
            input: context.input.clone(),
            intermediate_steps: context.intermediate_steps.clone(),
            iteration: context.iteration,
            metadata: context.metadata.clone(),
            timestamp: std::time::SystemTime::now(),
        }
    }

    /// Converts this checkpoint state back to an agent context.
    ///
    /// Used to restore agent execution from a saved checkpoint.
    /// The timestamp is not included in the context as it's checkpoint-specific.
    #[must_use]
    pub fn to_context(&self) -> AgentContext {
        AgentContext {
            input: self.input.clone(),
            intermediate_steps: self.intermediate_steps.clone(),
            iteration: self.iteration,
            metadata: self.metadata.clone(),
        }
    }
}

/// Default maximum number of checkpoints returned by paginated listing.
pub const DEFAULT_CHECKPOINTS_LIMIT: usize = 1000;

/// Trait for checkpoint storage backends.
///
/// Implementations must be thread-safe (`Send + Sync`) to support concurrent access.
/// Two implementations are provided:
/// - [`MemoryCheckpoint`]: In-memory storage for testing and short-lived agents
/// - [`FileCheckpoint`]: File-based storage for persistence across restarts
#[async_trait::async_trait]
pub trait Checkpoint: Send + Sync {
    /// Saves agent state with the given checkpoint ID.
    ///
    /// If a checkpoint with this ID already exists, it is overwritten.
    async fn save_state(&mut self, checkpoint_id: &str, state: &AgentCheckpointState)
        -> Result<()>;

    /// Loads agent state by checkpoint ID.
    ///
    /// # Errors
    ///
    /// Returns an error if the checkpoint does not exist.
    async fn load_state(&self, checkpoint_id: &str) -> Result<AgentCheckpointState>;

    /// Lists all checkpoint IDs, sorted by timestamp (oldest first).
    async fn list_checkpoints(&self) -> Result<Vec<String>>;

    /// Lists checkpoints with pagination.
    ///
    /// The limit is capped at [`DEFAULT_CHECKPOINTS_LIMIT`].
    async fn list_checkpoints_paginated(&self, limit: usize, offset: usize) -> Result<Vec<String>> {
        let limit = limit.min(DEFAULT_CHECKPOINTS_LIMIT);
        let all = self.list_checkpoints().await?;
        Ok(all.into_iter().skip(offset).take(limit).collect())
    }

    /// Deletes a checkpoint by ID.
    ///
    /// No error is returned if the checkpoint does not exist.
    async fn delete_checkpoint(&mut self, checkpoint_id: &str) -> Result<()>;

    /// Removes all checkpoints.
    async fn clear(&mut self) -> Result<()>;
}

/// In-memory checkpoint storage.
///
/// Stores checkpoints in a HashMap. Suitable for testing and short-lived agents
/// where persistence is not required. All data is lost when the process exits.
///
/// # Example
///
/// ```rust
/// use dashflow::core::agents::{Checkpoint, MemoryCheckpoint};
///
/// let checkpoint = MemoryCheckpoint::new();
/// // Use checkpoint.save_state(), load_state(), etc.
/// ```
#[derive(Debug, Clone)]
pub struct MemoryCheckpoint {
    checkpoints: std::collections::HashMap<String, AgentCheckpointState>,
}

impl MemoryCheckpoint {
    /// Creates a new empty in-memory checkpoint store.
    #[must_use]
    pub fn new() -> Self {
        Self {
            checkpoints: std::collections::HashMap::new(),
        }
    }
}

impl Default for MemoryCheckpoint {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl Checkpoint for MemoryCheckpoint {
    async fn save_state(
        &mut self,
        checkpoint_id: &str,
        state: &AgentCheckpointState,
    ) -> Result<()> {
        self.checkpoints
            .insert(checkpoint_id.to_string(), state.clone());
        Ok(())
    }
    async fn load_state(&self, checkpoint_id: &str) -> Result<AgentCheckpointState> {
        self.checkpoints.get(checkpoint_id).cloned().ok_or_else(|| {
            crate::core::Error::invalid_input(format!("Checkpoint '{checkpoint_id}' not found"))
        })
    }
    async fn list_checkpoints(&self) -> Result<Vec<String>> {
        let mut checkpoints: Vec<_> = self.checkpoints.iter().collect();
        checkpoints.sort_by_key(|(_, state)| state.timestamp);
        Ok(checkpoints.into_iter().map(|(id, _)| id.clone()).collect())
    }
    async fn delete_checkpoint(&mut self, checkpoint_id: &str) -> Result<()> {
        self.checkpoints.remove(checkpoint_id);
        Ok(())
    }
    async fn clear(&mut self) -> Result<()> {
        self.checkpoints.clear();
        Ok(())
    }
}

/// File-based checkpoint storage.
///
/// Stores each checkpoint as a JSON file in a directory. Suitable for agents that
/// need persistence across process restarts.
///
/// # Example
///
/// ```rust,no_run
/// use dashflow::core::agents::{Checkpoint, FileCheckpoint};
///
/// # async fn example() -> dashflow::core::error::Result<()> {
/// let _checkpoint = FileCheckpoint::new("./checkpoints").await?;
/// // Checkpoints are stored as ./checkpoints/{id}.json
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone)]
pub struct FileCheckpoint {
    directory: std::path::PathBuf,
}

impl FileCheckpoint {
    /// Creates a new file-based checkpoint store.
    ///
    /// Creates the directory if it does not exist.
    ///
    /// # Errors
    ///
    /// Returns an error if the directory cannot be created.
    pub async fn new(directory: impl AsRef<std::path::Path>) -> Result<Self> {
        let directory = directory.as_ref().to_path_buf();
        if !directory.exists() {
            tokio::fs::create_dir_all(&directory).await?;
        }
        Ok(Self { directory })
    }

    fn checkpoint_path(&self, checkpoint_id: &str) -> std::path::PathBuf {
        self.directory.join(format!("{checkpoint_id}.json"))
    }
}

#[async_trait::async_trait]
impl Checkpoint for FileCheckpoint {
    async fn save_state(
        &mut self,
        checkpoint_id: &str,
        state: &AgentCheckpointState,
    ) -> Result<()> {
        let path = self.checkpoint_path(checkpoint_id);
        let json = serde_json::to_string_pretty(state)
            .map_err(|e| crate::core::Error::other(format!("Failed to serialize: {e}")))?;
        tokio::fs::write(&path, &json)
            .await
            .map_err(|e| crate::core::Error::other(format!("Failed to write: {e}")))?;
        Ok(())
    }
    async fn load_state(&self, checkpoint_id: &str) -> Result<AgentCheckpointState> {
        let path = self.checkpoint_path(checkpoint_id);
        if !path.exists() {
            return Err(crate::core::Error::invalid_input(
                "Checkpoint not found".to_string(),
            ));
        }
        let json = tokio::fs::read_to_string(&path)
            .await
            .map_err(|e| crate::core::Error::other(format!("Failed to read: {e}")))?;
        serde_json::from_str(&json)
            .map_err(|e| crate::core::Error::other(format!("Failed to parse: {e}")))
    }
    async fn list_checkpoints(&self) -> Result<Vec<String>> {
        let mut entries = tokio::fs::read_dir(&self.directory)
            .await
            .map_err(|e| crate::core::Error::other(format!("Failed to read dir: {e}")))?;
        let mut checkpoints = Vec::new();
        while let Some(entry) = entries
            .next_entry()
            .await
            .map_err(|e| crate::core::Error::other(format!("Failed to iterate: {e}")))?
        {
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("json") {
                if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                    checkpoints.push((stem.to_string(), path.clone()));
                }
            }
        }
        let mut checkpoint_times = Vec::new();
        for (id, path) in checkpoints {
            let metadata = tokio::fs::metadata(&path)
                .await
                .map_err(|e| crate::core::Error::other(format!("Failed metadata: {e}")))?;
            let modified = metadata
                .modified()
                .map_err(|e| crate::core::Error::other(format!("Failed modified: {e}")))?;
            checkpoint_times.push((id, modified));
        }
        checkpoint_times.sort_by_key(|(_, time)| *time);
        Ok(checkpoint_times.into_iter().map(|(id, _)| id).collect())
    }
    async fn delete_checkpoint(&mut self, checkpoint_id: &str) -> Result<()> {
        let path = self.checkpoint_path(checkpoint_id);
        if path.exists() {
            tokio::fs::remove_file(&path)
                .await
                .map_err(|e| crate::core::Error::other(format!("Failed to delete: {e}")))?;
        }
        Ok(())
    }
    async fn clear(&mut self) -> Result<()> {
        let checkpoints = self.list_checkpoints().await?;
        for checkpoint_id in checkpoints {
            self.delete_checkpoint(&checkpoint_id).await?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::agents::{AgentAction, AgentContext, AgentStep};
    use crate::core::tools::ToolInput;

    fn sample_context() -> AgentContext {
        AgentContext {
            input: "test input".to_string(),
            intermediate_steps: vec![AgentStep {
                action: AgentAction {
                    tool: "test_tool".to_string(),
                    tool_input: ToolInput::String("arg".to_string()),
                    log: "log".to_string(),
                },
                observation: "result".to_string(),
            }],
            iteration: 5,
            metadata: [("key".to_string(), "value".to_string())]
                .into_iter()
                .collect(),
        }
    }

    // ===================== AgentCheckpointState =====================

    #[test]
    fn checkpoint_state_from_context_preserves_input() {
        let ctx = sample_context();
        let state = AgentCheckpointState::from_context(&ctx);
        assert_eq!(state.input, "test input");
    }

    #[test]
    fn checkpoint_state_from_context_preserves_steps() {
        let ctx = sample_context();
        let state = AgentCheckpointState::from_context(&ctx);
        assert_eq!(state.intermediate_steps.len(), 1);
        assert_eq!(state.intermediate_steps[0].action.tool, "test_tool");
    }

    #[test]
    fn checkpoint_state_from_context_preserves_iteration() {
        let ctx = sample_context();
        let state = AgentCheckpointState::from_context(&ctx);
        assert_eq!(state.iteration, 5);
    }

    #[test]
    fn checkpoint_state_from_context_preserves_metadata() {
        let ctx = sample_context();
        let state = AgentCheckpointState::from_context(&ctx);
        assert_eq!(state.metadata.get("key"), Some(&"value".to_string()));
    }

    #[test]
    fn checkpoint_state_from_context_sets_timestamp() {
        let before = std::time::SystemTime::now();
        let ctx = sample_context();
        let state = AgentCheckpointState::from_context(&ctx);
        let after = std::time::SystemTime::now();
        assert!(state.timestamp >= before);
        assert!(state.timestamp <= after);
    }

    #[test]
    fn checkpoint_state_to_context_preserves_input() {
        let ctx = sample_context();
        let state = AgentCheckpointState::from_context(&ctx);
        let restored = state.to_context();
        assert_eq!(restored.input, ctx.input);
    }

    #[test]
    fn checkpoint_state_to_context_preserves_steps() {
        let ctx = sample_context();
        let state = AgentCheckpointState::from_context(&ctx);
        let restored = state.to_context();
        assert_eq!(restored.intermediate_steps.len(), ctx.intermediate_steps.len());
    }

    #[test]
    fn checkpoint_state_to_context_preserves_iteration() {
        let ctx = sample_context();
        let state = AgentCheckpointState::from_context(&ctx);
        let restored = state.to_context();
        assert_eq!(restored.iteration, ctx.iteration);
    }

    #[test]
    fn checkpoint_state_to_context_preserves_metadata() {
        let ctx = sample_context();
        let state = AgentCheckpointState::from_context(&ctx);
        let restored = state.to_context();
        assert_eq!(restored.metadata, ctx.metadata);
    }

    #[test]
    fn checkpoint_state_roundtrip() {
        let ctx = sample_context();
        let state = AgentCheckpointState::from_context(&ctx);
        let restored = state.to_context();
        assert_eq!(restored.input, ctx.input);
        assert_eq!(restored.iteration, ctx.iteration);
        assert_eq!(restored.metadata, ctx.metadata);
    }

    #[test]
    fn checkpoint_state_serialization_roundtrip() {
        let ctx = sample_context();
        let state = AgentCheckpointState::from_context(&ctx);
        let json = serde_json::to_string(&state).expect("serialize");
        let restored: AgentCheckpointState = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(restored.input, state.input);
        assert_eq!(restored.iteration, state.iteration);
    }

    // ===================== DEFAULT_CHECKPOINTS_LIMIT =====================

    #[test]
    fn default_checkpoints_limit_is_1000() {
        assert_eq!(DEFAULT_CHECKPOINTS_LIMIT, 1000);
    }

    // ===================== MemoryCheckpoint =====================

    #[test]
    fn memory_checkpoint_new_is_empty() {
        let cp = MemoryCheckpoint::new();
        assert!(cp.checkpoints.is_empty());
    }

    #[test]
    fn memory_checkpoint_default_is_empty() {
        let cp = MemoryCheckpoint::default();
        assert!(cp.checkpoints.is_empty());
    }

    #[tokio::test]
    async fn memory_checkpoint_save_and_load() {
        let mut cp = MemoryCheckpoint::new();
        let ctx = sample_context();
        let state = AgentCheckpointState::from_context(&ctx);

        cp.save_state("cp1", &state).await.expect("save");
        let loaded = cp.load_state("cp1").await.expect("load");
        assert_eq!(loaded.input, state.input);
    }

    #[tokio::test]
    async fn memory_checkpoint_load_nonexistent_returns_error() {
        let cp = MemoryCheckpoint::new();
        let result = cp.load_state("nonexistent").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn memory_checkpoint_list_returns_ids_sorted_by_timestamp() {
        let mut cp = MemoryCheckpoint::new();
        let ctx = sample_context();

        let state1 = AgentCheckpointState::from_context(&ctx);
        std::thread::sleep(std::time::Duration::from_millis(10));
        let state2 = AgentCheckpointState::from_context(&ctx);
        std::thread::sleep(std::time::Duration::from_millis(10));
        let state3 = AgentCheckpointState::from_context(&ctx);

        cp.save_state("cp3", &state3).await.expect("save");
        cp.save_state("cp1", &state1).await.expect("save");
        cp.save_state("cp2", &state2).await.expect("save");

        let list = cp.list_checkpoints().await.expect("list");
        assert_eq!(list, vec!["cp1", "cp2", "cp3"]);
    }

    #[tokio::test]
    async fn memory_checkpoint_delete_removes_checkpoint() {
        let mut cp = MemoryCheckpoint::new();
        let ctx = sample_context();
        let state = AgentCheckpointState::from_context(&ctx);

        cp.save_state("cp1", &state).await.expect("save");
        assert!(cp.load_state("cp1").await.is_ok());

        cp.delete_checkpoint("cp1").await.expect("delete");
        assert!(cp.load_state("cp1").await.is_err());
    }

    #[tokio::test]
    async fn memory_checkpoint_delete_nonexistent_is_ok() {
        let mut cp = MemoryCheckpoint::new();
        let result = cp.delete_checkpoint("nonexistent").await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn memory_checkpoint_clear_removes_all() {
        let mut cp = MemoryCheckpoint::new();
        let ctx = sample_context();
        let state = AgentCheckpointState::from_context(&ctx);

        cp.save_state("cp1", &state).await.expect("save");
        cp.save_state("cp2", &state).await.expect("save");
        cp.clear().await.expect("clear");

        let list = cp.list_checkpoints().await.expect("list");
        assert!(list.is_empty());
    }

    #[tokio::test]
    async fn memory_checkpoint_paginated_respects_limit() {
        let mut cp = MemoryCheckpoint::new();
        let ctx = sample_context();

        for i in 0..5 {
            let mut state = AgentCheckpointState::from_context(&ctx);
            state.timestamp = std::time::SystemTime::UNIX_EPOCH
                + std::time::Duration::from_secs(i as u64);
            cp.save_state(&format!("cp{i}"), &state).await.expect("save");
        }

        let page = cp.list_checkpoints_paginated(2, 0).await.expect("paginated");
        assert_eq!(page.len(), 2);
    }

    #[tokio::test]
    async fn memory_checkpoint_paginated_respects_offset() {
        let mut cp = MemoryCheckpoint::new();
        let ctx = sample_context();

        for i in 0..5 {
            let mut state = AgentCheckpointState::from_context(&ctx);
            state.timestamp = std::time::SystemTime::UNIX_EPOCH
                + std::time::Duration::from_secs(i as u64);
            cp.save_state(&format!("cp{i}"), &state).await.expect("save");
        }

        let page = cp.list_checkpoints_paginated(2, 2).await.expect("paginated");
        assert_eq!(page, vec!["cp2", "cp3"]);
    }

    #[tokio::test]
    async fn memory_checkpoint_paginated_caps_at_default_limit() {
        let mut cp = MemoryCheckpoint::new();
        let ctx = sample_context();
        let state = AgentCheckpointState::from_context(&ctx);
        cp.save_state("cp1", &state).await.expect("save");

        // Request more than DEFAULT_CHECKPOINTS_LIMIT
        let page = cp
            .list_checkpoints_paginated(DEFAULT_CHECKPOINTS_LIMIT + 100, 0)
            .await
            .expect("paginated");
        assert_eq!(page.len(), 1); // Only 1 checkpoint exists
    }

    // ===================== FileCheckpoint =====================

    #[tokio::test]
    async fn file_checkpoint_creates_directory() {
        let dir = tempfile::tempdir().expect("tempdir");
        let subdir = dir.path().join("checkpoints");
        assert!(!subdir.exists());

        let _cp = FileCheckpoint::new(&subdir).await.expect("new");
        assert!(subdir.exists());
    }

    #[tokio::test]
    async fn file_checkpoint_save_and_load() {
        let dir = tempfile::tempdir().expect("tempdir");
        let mut cp = FileCheckpoint::new(dir.path()).await.expect("new");

        let ctx = sample_context();
        let state = AgentCheckpointState::from_context(&ctx);

        cp.save_state("cp1", &state).await.expect("save");
        let loaded = cp.load_state("cp1").await.expect("load");
        assert_eq!(loaded.input, state.input);
    }

    #[tokio::test]
    async fn file_checkpoint_load_nonexistent_returns_error() {
        let dir = tempfile::tempdir().expect("tempdir");
        let cp = FileCheckpoint::new(dir.path()).await.expect("new");

        let result = cp.load_state("nonexistent").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn file_checkpoint_list_returns_ids() {
        let dir = tempfile::tempdir().expect("tempdir");
        let mut cp = FileCheckpoint::new(dir.path()).await.expect("new");

        let ctx = sample_context();
        let state = AgentCheckpointState::from_context(&ctx);

        cp.save_state("cp1", &state).await.expect("save");
        cp.save_state("cp2", &state).await.expect("save");

        let list = cp.list_checkpoints().await.expect("list");
        assert_eq!(list.len(), 2);
        assert!(list.contains(&"cp1".to_string()));
        assert!(list.contains(&"cp2".to_string()));
    }

    #[tokio::test]
    async fn file_checkpoint_delete_removes_file() {
        let dir = tempfile::tempdir().expect("tempdir");
        let mut cp = FileCheckpoint::new(dir.path()).await.expect("new");

        let ctx = sample_context();
        let state = AgentCheckpointState::from_context(&ctx);

        cp.save_state("cp1", &state).await.expect("save");
        assert!(dir.path().join("cp1.json").exists());

        cp.delete_checkpoint("cp1").await.expect("delete");
        assert!(!dir.path().join("cp1.json").exists());
    }

    #[tokio::test]
    async fn file_checkpoint_delete_nonexistent_is_ok() {
        let dir = tempfile::tempdir().expect("tempdir");
        let mut cp = FileCheckpoint::new(dir.path()).await.expect("new");

        let result = cp.delete_checkpoint("nonexistent").await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn file_checkpoint_clear_removes_all_files() {
        let dir = tempfile::tempdir().expect("tempdir");
        let mut cp = FileCheckpoint::new(dir.path()).await.expect("new");

        let ctx = sample_context();
        let state = AgentCheckpointState::from_context(&ctx);

        cp.save_state("cp1", &state).await.expect("save");
        cp.save_state("cp2", &state).await.expect("save");
        cp.clear().await.expect("clear");

        let list = cp.list_checkpoints().await.expect("list");
        assert!(list.is_empty());
    }

    #[tokio::test]
    async fn file_checkpoint_uses_json_extension() {
        let dir = tempfile::tempdir().expect("tempdir");
        let mut cp = FileCheckpoint::new(dir.path()).await.expect("new");

        let ctx = sample_context();
        let state = AgentCheckpointState::from_context(&ctx);

        cp.save_state("myid", &state).await.expect("save");
        assert!(dir.path().join("myid.json").exists());
    }

    #[tokio::test]
    async fn file_checkpoint_saved_json_is_valid() {
        let dir = tempfile::tempdir().expect("tempdir");
        let mut cp = FileCheckpoint::new(dir.path()).await.expect("new");

        let ctx = sample_context();
        let state = AgentCheckpointState::from_context(&ctx);

        cp.save_state("test", &state).await.expect("save");

        let json = std::fs::read_to_string(dir.path().join("test.json")).expect("read");
        let parsed: AgentCheckpointState = serde_json::from_str(&json).expect("parse");
        assert_eq!(parsed.input, state.input);
    }
}
