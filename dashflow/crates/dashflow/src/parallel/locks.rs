// Lock management clippy exceptions:
// - clone_on_ref_ptr: Arc::clone() is idiomatic for shared lock state
// - needless_pass_by_value: API ergonomics - String parameters are cheap to clone
// - redundant_clone: Clone for ownership clarity in complex lock operations
#![allow(clippy::clone_on_ref_ptr)]
#![allow(clippy::needless_pass_by_value, clippy::redundant_clone)]
// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

//! Lock file system for parallel AI development coordination.
//!
//! Provides soft locks to coordinate multiple AI workers operating on
//! the same codebase without conflicts.

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use thiserror::Error;

/// Default directory for lock files relative to repository root.
pub const DEFAULT_LOCKS_DIR: &str = ".dashflow/locks";

/// Default lock duration in seconds (1 hour).
pub const DEFAULT_LOCK_DURATION_SECS: i64 = 3600;

/// Lock operation result type.
pub type LockResult<T> = Result<T, LockError>;

/// Errors that can occur during lock operations.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum LockError {
    /// Lock file could not be read or written.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// Lock file contains invalid JSON.
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    /// The requested scope is already locked by another worker.
    #[error("Scope '{scope}' is locked by '{worker_id}' until {expires_at}")]
    AlreadyLocked {
        /// The scope that is locked.
        scope: String,
        /// The worker that holds the lock.
        worker_id: String,
        /// When the lock expires.
        expires_at: DateTime<Utc>,
    },

    /// The lock does not exist.
    #[error("No lock exists for scope '{0}'")]
    NotLocked(String),

    /// The lock is owned by a different worker.
    #[error("Lock for scope '{scope}' is owned by '{owner}', not '{requestor}'")]
    NotOwner {
        /// The scope.
        scope: String,
        /// The current owner.
        owner: String,
        /// Who tried to release.
        requestor: String,
    },

    /// Invalid scope name.
    #[error("Invalid scope name: {0}")]
    InvalidScope(String),
}

/// Represents a lock scope - the resource being locked.
///
/// Scopes follow a naming convention:
/// - `dashflow-openai` - A whole crate
/// - `dashflow.introspection` - A module in the core crate
/// - `dashflow.src.lib.rs` - A specific file
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct LockScope {
    /// The scope name (e.g., "dashflow.optimize" or "dashflow-openai").
    name: String,
}

impl LockScope {
    /// Create a new lock scope from a name.
    ///
    /// Valid scope names:
    /// - Contain only alphanumeric characters, dots, dashes, and underscores
    /// - Are not empty
    /// - Do not start or end with dots
    pub fn new(name: impl Into<String>) -> LockResult<Self> {
        let name = name.into();
        Self::validate(&name)?;
        Ok(Self { name })
    }

    /// Create a scope for an entire crate.
    pub fn crate_scope(crate_name: &str) -> LockResult<Self> {
        Self::new(crate_name)
    }

    /// Create a scope for a module in the dashflow core crate.
    pub fn module_scope(module_name: &str) -> LockResult<Self> {
        Self::new(format!("dashflow.{}", module_name))
    }

    /// Create a scope for a specific file.
    pub fn file_scope(file_path: &str) -> LockResult<Self> {
        // Convert path separators to dots
        let scope = file_path.replace(['/', '\\'], ".");
        Self::new(scope)
    }

    /// Get the scope name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Get the lock file name for this scope.
    pub fn lock_file_name(&self) -> String {
        format!("{}.lock", self.name)
    }

    fn validate(name: &str) -> LockResult<()> {
        if name.is_empty() {
            return Err(LockError::InvalidScope("Scope name cannot be empty".into()));
        }

        if name.starts_with('.') || name.ends_with('.') {
            return Err(LockError::InvalidScope(
                "Scope name cannot start or end with a dot".into(),
            ));
        }

        // Allow alphanumeric, dots, dashes, underscores
        for ch in name.chars() {
            if !ch.is_alphanumeric() && ch != '.' && ch != '-' && ch != '_' {
                return Err(LockError::InvalidScope(format!(
                    "Invalid character '{}' in scope name",
                    ch
                )));
            }
        }

        Ok(())
    }
}

impl std::fmt::Display for LockScope {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name)
    }
}

/// Information about a lock.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Lock {
    /// The scope being locked.
    pub scope: LockScope,

    /// Unique identifier for the worker holding the lock.
    pub worker_id: String,

    /// When the lock was acquired.
    pub acquired_at: DateTime<Utc>,

    /// When the lock expires.
    pub expires_at: DateTime<Utc>,

    /// Description of what work is being done.
    pub purpose: String,

    /// List of files being touched by this lock.
    #[serde(default)]
    pub files_touched: Vec<String>,
}

impl Lock {
    /// Create a new lock.
    pub fn new(
        scope: LockScope,
        worker_id: impl Into<String>,
        purpose: impl Into<String>,
        duration_secs: Option<i64>,
    ) -> Self {
        let now = Utc::now();
        let duration_secs = duration_secs.unwrap_or(DEFAULT_LOCK_DURATION_SECS);

        Self {
            scope,
            worker_id: worker_id.into(),
            acquired_at: now,
            expires_at: now + Duration::seconds(duration_secs),
            purpose: purpose.into(),
            files_touched: Vec::new(),
        }
    }

    /// Check if the lock has expired.
    pub fn is_expired(&self) -> bool {
        Utc::now() > self.expires_at
    }

    /// Get time remaining until expiration.
    pub fn time_remaining(&self) -> Option<chrono::Duration> {
        let remaining = self.expires_at - Utc::now();
        if remaining.num_seconds() > 0 {
            Some(remaining)
        } else {
            None
        }
    }

    /// Add a file to the list of files being touched.
    pub fn add_file(&mut self, file_path: impl Into<String>) {
        self.files_touched.push(file_path.into());
    }

    /// Extend the lock expiration by the given duration.
    pub fn extend(&mut self, duration_secs: i64) {
        self.expires_at += Duration::seconds(duration_secs);
    }
}

/// Status of a lock check operation.
#[derive(Debug, Clone)]
pub enum LockStatus {
    /// The scope is not locked.
    Unlocked,

    /// The scope is locked by the specified worker.
    Locked(Lock),

    /// The scope was locked but the lock has expired.
    Expired(Lock),
}

impl LockStatus {
    /// Returns true if the scope is currently locked (not expired).
    pub fn is_locked(&self) -> bool {
        matches!(self, LockStatus::Locked(_))
    }

    /// Returns true if the scope is available (unlocked or expired).
    pub fn is_available(&self) -> bool {
        matches!(self, LockStatus::Unlocked | LockStatus::Expired(_))
    }
}

/// Manager for coordinating locks across AI workers.
///
/// The lock manager provides operations for:
/// - Checking lock status
/// - Acquiring locks
/// - Releasing locks
/// - Listing all locks
/// - Force-releasing stale locks
#[derive(Debug, Clone)]
pub struct LockManager {
    /// Directory where lock files are stored.
    locks_dir: PathBuf,
}

impl LockManager {
    /// Create a new lock manager.
    ///
    /// The directory will be created if it doesn't exist.
    pub fn new(locks_dir: impl AsRef<Path>) -> LockResult<Self> {
        let locks_dir = locks_dir.as_ref().to_path_buf();
        fs::create_dir_all(&locks_dir)?;
        Ok(Self { locks_dir })
    }

    /// Create a lock manager using the default locks directory.
    ///
    /// Uses `.dashflow/locks` relative to the current directory.
    pub fn default_location() -> LockResult<Self> {
        Self::new(DEFAULT_LOCKS_DIR)
    }

    /// Get the path to a lock file for a given scope.
    fn lock_path(&self, scope: &LockScope) -> PathBuf {
        self.locks_dir.join(scope.lock_file_name())
    }

    /// Check the status of a lock.
    pub fn status(&self, scope: &str) -> LockResult<LockStatus> {
        let scope = LockScope::new(scope)?;
        let lock_path = self.lock_path(&scope);

        if !lock_path.exists() {
            return Ok(LockStatus::Unlocked);
        }

        let content = fs::read_to_string(&lock_path)?;
        let lock: Lock = serde_json::from_str(&content)?;

        if lock.is_expired() {
            Ok(LockStatus::Expired(lock))
        } else {
            Ok(LockStatus::Locked(lock))
        }
    }

    /// Check if a scope is currently locked (not expired).
    pub fn is_locked(&self, scope: &str) -> LockResult<bool> {
        Ok(self.status(scope)?.is_locked())
    }

    /// Acquire a lock on a scope.
    ///
    /// Returns an error if the scope is already locked by another worker.
    pub fn acquire(&self, scope: &str, worker_id: &str, purpose: &str) -> LockResult<Lock> {
        self.acquire_with_duration(scope, worker_id, purpose, None)
    }

    /// Acquire a lock with a custom duration.
    pub fn acquire_with_duration(
        &self,
        scope: &str,
        worker_id: &str,
        purpose: &str,
        duration_secs: Option<i64>,
    ) -> LockResult<Lock> {
        let scope = LockScope::new(scope)?;
        let lock_path = self.lock_path(&scope);

        // Check existing lock
        if lock_path.exists() {
            let content = fs::read_to_string(&lock_path)?;
            let existing: Lock = serde_json::from_str(&content)?;

            if !existing.is_expired() && existing.worker_id != worker_id {
                return Err(LockError::AlreadyLocked {
                    scope: scope.to_string(),
                    worker_id: existing.worker_id,
                    expires_at: existing.expires_at,
                });
            }
        }

        // Create new lock
        let lock = Lock::new(scope.clone(), worker_id, purpose, duration_secs);
        let content = serde_json::to_string_pretty(&lock)?;
        fs::write(&lock_path, content)?;

        Ok(lock)
    }

    /// Release a lock.
    ///
    /// Only the worker that acquired the lock can release it (unless force is true).
    pub fn release(&self, scope: &str, worker_id: &str) -> LockResult<()> {
        let scope = LockScope::new(scope)?;
        let lock_path = self.lock_path(&scope);

        if !lock_path.exists() {
            return Err(LockError::NotLocked(scope.to_string()));
        }

        // Verify ownership
        let content = fs::read_to_string(&lock_path)?;
        let lock: Lock = serde_json::from_str(&content)?;

        if lock.worker_id != worker_id {
            return Err(LockError::NotOwner {
                scope: scope.to_string(),
                owner: lock.worker_id,
                requestor: worker_id.to_string(),
            });
        }

        fs::remove_file(&lock_path)?;
        Ok(())
    }

    /// Force release a lock (for stale lock cleanup).
    ///
    /// This bypasses ownership checks and should only be used for
    /// cleaning up locks from crashed workers.
    pub fn force_release(&self, scope: &str) -> LockResult<()> {
        let scope = LockScope::new(scope)?;
        let lock_path = self.lock_path(&scope);

        if !lock_path.exists() {
            return Err(LockError::NotLocked(scope.to_string()));
        }

        fs::remove_file(&lock_path)?;
        Ok(())
    }

    /// Extend a lock's expiration.
    pub fn extend(&self, scope: &str, worker_id: &str, additional_secs: i64) -> LockResult<Lock> {
        let scope = LockScope::new(scope)?;
        let lock_path = self.lock_path(&scope);

        if !lock_path.exists() {
            return Err(LockError::NotLocked(scope.to_string()));
        }

        let content = fs::read_to_string(&lock_path)?;
        let mut lock: Lock = serde_json::from_str(&content)?;

        if lock.worker_id != worker_id {
            return Err(LockError::NotOwner {
                scope: scope.to_string(),
                owner: lock.worker_id,
                requestor: worker_id.to_string(),
            });
        }

        lock.extend(additional_secs);
        let content = serde_json::to_string_pretty(&lock)?;
        fs::write(&lock_path, content)?;

        Ok(lock)
    }

    /// Add a file to a lock's touched files list.
    pub fn add_touched_file(
        &self,
        scope: &str,
        worker_id: &str,
        file_path: &str,
    ) -> LockResult<Lock> {
        let scope = LockScope::new(scope)?;
        let lock_path = self.lock_path(&scope);

        if !lock_path.exists() {
            return Err(LockError::NotLocked(scope.to_string()));
        }

        let content = fs::read_to_string(&lock_path)?;
        let mut lock: Lock = serde_json::from_str(&content)?;

        if lock.worker_id != worker_id {
            return Err(LockError::NotOwner {
                scope: scope.to_string(),
                owner: lock.worker_id,
                requestor: worker_id.to_string(),
            });
        }

        lock.add_file(file_path);
        let content = serde_json::to_string_pretty(&lock)?;
        fs::write(&lock_path, content)?;

        Ok(lock)
    }

    /// List all current locks.
    pub fn list(&self) -> LockResult<Vec<Lock>> {
        let mut locks = Vec::new();

        for entry in fs::read_dir(&self.locks_dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.extension().is_some_and(|ext| ext == "lock") {
                let content = fs::read_to_string(&path)?;
                if let Ok(lock) = serde_json::from_str::<Lock>(&content) {
                    locks.push(lock);
                }
            }
        }

        Ok(locks)
    }

    /// List all active (non-expired) locks.
    pub fn list_active(&self) -> LockResult<Vec<Lock>> {
        Ok(self
            .list()?
            .into_iter()
            .filter(|l| !l.is_expired())
            .collect())
    }

    /// List all expired locks.
    pub fn list_expired(&self) -> LockResult<Vec<Lock>> {
        Ok(self
            .list()?
            .into_iter()
            .filter(|l| l.is_expired())
            .collect())
    }

    /// List all locks held by a specific worker.
    pub fn list_by_worker(&self, worker_id: &str) -> LockResult<Vec<Lock>> {
        Ok(self
            .list()?
            .into_iter()
            .filter(|l| l.worker_id == worker_id)
            .collect())
    }

    /// Clean up all expired locks.
    pub fn cleanup_expired(&self) -> LockResult<usize> {
        let expired = self.list_expired()?;
        let count = expired.len();

        for lock in expired {
            let lock_path = self.lock_path(&lock.scope);
            if lock_path.exists() {
                fs::remove_file(&lock_path)?;
            }
        }

        Ok(count)
    }

    /// Get a summary of all locks grouped by status.
    pub fn summary(&self) -> LockResult<LockSummary> {
        let locks = self.list()?;
        // Capacity hint: total locks as upper bound (typically most are active)
        let mut active = Vec::with_capacity(locks.len());
        let mut expired = Vec::new();

        for lock in locks {
            if lock.is_expired() {
                expired.push(lock);
            } else {
                active.push(lock);
            }
        }

        Ok(LockSummary {
            active_count: active.len(),
            expired_count: expired.len(),
            active,
            expired,
        })
    }
}

/// Summary of all locks.
#[derive(Debug, Clone)]
pub struct LockSummary {
    /// Number of active (non-expired) locks.
    pub active_count: usize,

    /// Number of expired locks.
    pub expired_count: usize,

    /// All active locks.
    pub active: Vec<Lock>,

    /// All expired locks.
    pub expired: Vec<Lock>,
}

impl LockSummary {
    /// Get locks grouped by worker.
    pub fn by_worker(&self) -> HashMap<String, Vec<&Lock>> {
        let mut map: HashMap<String, Vec<&Lock>> = HashMap::new();

        for lock in &self.active {
            map.entry(lock.worker_id.clone()).or_default().push(lock);
        }

        map
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn setup() -> (TempDir, LockManager) {
        let temp_dir = TempDir::new().unwrap();
        let manager = LockManager::new(temp_dir.path()).unwrap();
        (temp_dir, manager)
    }

    #[test]
    fn test_lock_scope_validation() {
        // Valid scopes
        assert!(LockScope::new("dashflow-openai").is_ok());
        assert!(LockScope::new("dashflow.optimize").is_ok());
        assert!(LockScope::new("dashflow.src.lib.rs").is_ok());
        assert!(LockScope::new("my_scope_123").is_ok());

        // Invalid scopes
        assert!(LockScope::new("").is_err());
        assert!(LockScope::new(".starts-with-dot").is_err());
        assert!(LockScope::new("ends-with-dot.").is_err());
        assert!(LockScope::new("has spaces").is_err());
        assert!(LockScope::new("has/slashes").is_err());
    }

    #[test]
    fn test_lock_scope_helpers() {
        let crate_scope = LockScope::crate_scope("dashflow-openai").unwrap();
        assert_eq!(crate_scope.name(), "dashflow-openai");

        let module_scope = LockScope::module_scope("optimize").unwrap();
        assert_eq!(module_scope.name(), "dashflow.optimize");

        let file_scope = LockScope::file_scope("crates/dashflow/src/lib.rs").unwrap();
        assert_eq!(file_scope.name(), "crates.dashflow.src.lib.rs");
    }

    #[test]
    fn test_lock_creation() {
        let scope = LockScope::new("test-scope").unwrap();
        let lock = Lock::new(scope, "worker-1", "Testing", Some(3600));

        assert_eq!(lock.worker_id, "worker-1");
        assert_eq!(lock.purpose, "Testing");
        assert!(!lock.is_expired());
        assert!(lock.time_remaining().is_some());
    }

    #[test]
    fn test_lock_expiration() {
        let scope = LockScope::new("test-scope").unwrap();
        // Create a lock that expires in -1 seconds (already expired)
        let mut lock = Lock::new(scope, "worker-1", "Testing", Some(-1));

        // Manually set expired time
        lock.expires_at = Utc::now() - Duration::seconds(100);

        assert!(lock.is_expired());
        assert!(lock.time_remaining().is_none());
    }

    #[test]
    fn test_acquire_and_release() {
        let (_temp, manager) = setup();

        // Acquire lock
        let lock = manager
            .acquire("test-scope", "worker-1", "Testing")
            .unwrap();
        assert_eq!(lock.worker_id, "worker-1");

        // Verify locked
        assert!(manager.is_locked("test-scope").unwrap());

        // Release lock
        manager.release("test-scope", "worker-1").unwrap();

        // Verify unlocked
        assert!(!manager.is_locked("test-scope").unwrap());
    }

    #[test]
    fn test_acquire_already_locked() {
        let (_temp, manager) = setup();

        // First worker acquires lock
        manager.acquire("test-scope", "worker-1", "First").unwrap();

        // Second worker tries to acquire - should fail
        let result = manager.acquire("test-scope", "worker-2", "Second");
        assert!(matches!(result, Err(LockError::AlreadyLocked { .. })));
    }

    #[test]
    fn test_release_not_owner() {
        let (_temp, manager) = setup();

        // Worker 1 acquires lock
        manager
            .acquire("test-scope", "worker-1", "Testing")
            .unwrap();

        // Worker 2 tries to release - should fail
        let result = manager.release("test-scope", "worker-2");
        assert!(matches!(result, Err(LockError::NotOwner { .. })));
    }

    #[test]
    fn test_force_release() {
        let (_temp, manager) = setup();

        // Acquire lock
        manager
            .acquire("test-scope", "worker-1", "Testing")
            .unwrap();

        // Force release (bypasses ownership check)
        manager.force_release("test-scope").unwrap();

        // Verify unlocked
        assert!(!manager.is_locked("test-scope").unwrap());
    }

    #[test]
    fn test_extend_lock() {
        let (_temp, manager) = setup();

        // Acquire lock with short duration
        let lock = manager
            .acquire_with_duration("test-scope", "worker-1", "Testing", Some(60))
            .unwrap();
        let original_expires = lock.expires_at;

        // Extend by 60 more seconds
        let extended = manager.extend("test-scope", "worker-1", 60).unwrap();

        assert!(extended.expires_at > original_expires);
    }

    #[test]
    fn test_add_touched_file() {
        let (_temp, manager) = setup();

        // Acquire lock
        manager
            .acquire("test-scope", "worker-1", "Testing")
            .unwrap();

        // Add touched file
        let lock = manager
            .add_touched_file("test-scope", "worker-1", "src/lib.rs")
            .unwrap();

        assert_eq!(lock.files_touched, vec!["src/lib.rs"]);
    }

    #[test]
    fn test_list_locks() {
        let (_temp, manager) = setup();

        // Acquire multiple locks
        manager.acquire("scope-1", "worker-1", "First").unwrap();
        manager.acquire("scope-2", "worker-1", "Second").unwrap();
        manager.acquire("scope-3", "worker-2", "Third").unwrap();

        let all = manager.list().unwrap();
        assert_eq!(all.len(), 3);

        let by_worker_1 = manager.list_by_worker("worker-1").unwrap();
        assert_eq!(by_worker_1.len(), 2);
    }

    #[test]
    fn test_lock_status() {
        let (_temp, manager) = setup();

        // Initially unlocked
        let status = manager.status("test-scope").unwrap();
        assert!(matches!(status, LockStatus::Unlocked));

        // Acquire lock
        manager
            .acquire("test-scope", "worker-1", "Testing")
            .unwrap();

        // Now locked
        let status = manager.status("test-scope").unwrap();
        assert!(matches!(status, LockStatus::Locked(_)));
    }

    #[test]
    fn test_cleanup_expired() {
        let (_temp, manager) = setup();

        // Create an expired lock manually
        let scope = LockScope::new("expired-scope").unwrap();
        let mut lock = Lock::new(scope.clone(), "worker-1", "Expired", None);
        lock.expires_at = Utc::now() - Duration::seconds(100);

        let lock_path = manager.locks_dir.join(scope.lock_file_name());
        let content = serde_json::to_string_pretty(&lock).unwrap();
        fs::write(&lock_path, content).unwrap();

        // Verify it's expired
        let status = manager.status("expired-scope").unwrap();
        assert!(matches!(status, LockStatus::Expired(_)));

        // Cleanup
        let cleaned = manager.cleanup_expired().unwrap();
        assert_eq!(cleaned, 1);

        // Verify removed
        let status = manager.status("expired-scope").unwrap();
        assert!(matches!(status, LockStatus::Unlocked));
    }

    #[test]
    fn test_summary() {
        let (_temp, manager) = setup();

        // Acquire some locks
        manager.acquire("scope-1", "worker-1", "First").unwrap();
        manager.acquire("scope-2", "worker-2", "Second").unwrap();

        let summary = manager.summary().unwrap();
        assert_eq!(summary.active_count, 2);
        assert_eq!(summary.expired_count, 0);

        let by_worker = summary.by_worker();
        assert_eq!(by_worker.get("worker-1").map(|v| v.len()), Some(1));
        assert_eq!(by_worker.get("worker-2").map(|v| v.len()), Some(1));
    }

    #[test]
    fn test_same_worker_can_reacquire() {
        let (_temp, manager) = setup();

        // Worker acquires lock
        manager.acquire("test-scope", "worker-1", "First").unwrap();

        // Same worker can reacquire (update purpose)
        let lock = manager
            .acquire("test-scope", "worker-1", "Updated")
            .unwrap();
        assert_eq!(lock.purpose, "Updated");
    }

    #[test]
    fn test_can_acquire_expired_lock() {
        let (_temp, manager) = setup();

        // Create an expired lock manually
        let scope = LockScope::new("test-scope").unwrap();
        let mut lock = Lock::new(scope.clone(), "worker-1", "Original", None);
        lock.expires_at = Utc::now() - Duration::seconds(100);

        let lock_path = manager.locks_dir.join(scope.lock_file_name());
        let content = serde_json::to_string_pretty(&lock).unwrap();
        fs::write(&lock_path, content).unwrap();

        // Different worker can acquire expired lock
        let new_lock = manager.acquire("test-scope", "worker-2", "New").unwrap();
        assert_eq!(new_lock.worker_id, "worker-2");
    }

    #[test]
    fn test_release_not_locked() {
        let (_temp, manager) = setup();

        // Try to release non-existent lock
        let result = manager.release("nonexistent", "worker-1");
        assert!(matches!(result, Err(LockError::NotLocked(_))));
    }

    #[test]
    fn test_lock_serialization() {
        let scope = LockScope::new("test-scope").unwrap();
        let lock = Lock::new(scope, "worker-1", "Testing", Some(3600));

        // Serialize to JSON
        let json = serde_json::to_string_pretty(&lock).unwrap();

        // Deserialize back
        let restored: Lock = serde_json::from_str(&json).unwrap();

        assert_eq!(restored.worker_id, lock.worker_id);
        assert_eq!(restored.purpose, lock.purpose);
        assert_eq!(restored.scope.name(), lock.scope.name());
    }
}
