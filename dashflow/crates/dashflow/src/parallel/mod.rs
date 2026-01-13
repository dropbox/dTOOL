// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

//! @dashflow-module
//! @name parallel
//! @category runtime
//! @status stable
//!
//! # Parallel AI Development Coordination
//!
//! This module provides coordination primitives for parallel AI development,
//! enabling multiple AI workers to work on different parts of the codebase
//! simultaneously without conflicts.
//!
//! ## Overview
//!
//! When multiple AI workers are operating on a codebase:
//! - They need to avoid editing the same files simultaneously
//! - They need to compile without blocking each other
//! - They need to commit without merge conflicts
//! - They need to not break each other's work through API changes
//!
//! This module provides a soft lock system to coordinate this work.
//!
//! ## Lock Granularity
//!
//! Locks can be acquired at different granularities:
//! - **Crate-level**: `dashflow-openai` - Working on an entire crate
//! - **Module-level**: `dashflow.introspection` - Working on a module in the core crate
//! - **File-level**: `dashflow.src.lib.rs` - Working on a specific file
//!
//! ## Example
//!
//! ```rust,ignore
//! use dashflow::parallel::{Lock, LockManager};
//! use std::path::Path;
//!
//! // Create a lock manager pointing to the locks directory
//! let manager = LockManager::new(Path::new(".dashflow/locks"));
//!
//! // Check if a scope is locked
//! if manager.is_locked("dashflow.optimize")? {
//!     println!("Another AI is working on the optimize module");
//!     return Ok(());
//! }
//!
//! // Acquire a lock
//! let lock = manager.acquire(
//!     "dashflow.optimize",
//!     "claude-abc123",
//!     "Implementing telemetry unification",
//! )?;
//!
//! // Do work...
//!
//! // Release the lock
//! manager.release("dashflow.optimize")?;
//! ```
//!
//! ## Isolated Builds
//!
//! To prevent Cargo lock conflicts during parallel builds, use separate target directories:
//!
//! ```bash
//! CARGO_TARGET_DIR=/tmp/dashflow-worker-1 cargo check -p dashflow-openai
//! CARGO_TARGET_DIR=/tmp/dashflow-worker-2 cargo check -p dashflow-anthropic
//! ```

mod locks;

pub use locks::{
    Lock, LockError, LockManager, LockResult, LockScope, LockStatus, DEFAULT_LOCKS_DIR,
    DEFAULT_LOCK_DURATION_SECS,
};
