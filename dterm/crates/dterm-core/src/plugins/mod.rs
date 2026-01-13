//! # WASM Plugin System
//!
//! Sandboxed user-scriptable extensions via WebAssembly.
//!
//! ## Design
//!
//! The plugin system allows users to extend terminal functionality with
//! sandboxed WASM modules. Plugins can:
//!
//! - React to terminal output (syntax highlighting, auto-suggest)
//! - React to key events (custom keybindings, input transformation)
//! - React to command completion (notifications, logging)
//! - Transform terminal content (filter, annotate, colorize)
//!
//! ## Security Model
//!
//! Plugins run in sandboxed WASM environments with:
//! - Memory limits (configurable, default 32 MiB)
//! - CPU limits (fuel-based, per-event budgets)
//! - Explicit permission grants for each capability
//! - No direct filesystem or network access (v1)
//!
//! ## Permission Gating
//!
//! All plugin actions are gated by permissions declared in the manifest:
//! - `terminal.read` - Read terminal output and state
//! - `terminal.write` - Transform input or emit commands
//! - `terminal.command` - Observe command lifecycle events
//! - `storage` - Use persistent key-value storage
//! - `clipboard.read` / `clipboard.write` - Clipboard access (off by default)
//!
//! ## Storage API
//!
//! Plugins with the `storage` permission can use key-value storage:
//! - Isolated per-plugin namespace
//! - Configurable quota (default 1 MiB)
//! - Key/value size limits enforced
//! - In-memory only for v1 (persistence planned for v2)
//!
//! ## Package Format
//!
//! Plugins are directories containing:
//! - `plugin.toml` - Manifest with name, version, permissions
//! - `plugin.wasm` - Compiled WebAssembly module
//! - `assets/` - Optional read-only resources
//!
//! ## Phases
//!
//! - Phase 1: Loader + runtime scaffolding (complete)
//! - Phase 2: Event queue + lifecycle control with budgets and metrics (complete)
//! - Phase 3: Permission gating and storage API (complete)
//! - Phase 4: Integrations: output, input, command blocks (complete)
//! - Phase 5: Hardening: fuzzing, metrics, failure recovery
//!
//! See `docs/WASM_PLUGIN_SYSTEM.md` for full design documentation.

mod bridge;
mod lifecycle;
mod manifest;
mod permissions;
mod queue;
mod storage;
mod types;

#[cfg(feature = "wasm-plugins")]
mod runtime;

pub use lifecycle::{
    BackoffPolicy, BudgetViolation, LifecycleAction, LifecycleManager, PluginBudget,
    PluginLifecycle,
};
pub use manifest::{
    parse_manifest, validate_manifest, ManifestError, Permission, PluginManifest,
};
pub use permissions::{require_permission, PermissionCheckResult, PermissionChecker, PermissionDenied};
pub use queue::{
    EnqueueResult, EventPriority, PluginEventQueue, QueueConfig, QueueStats, QueuedEvent,
};
pub use storage::{
    PluginStorage, StorageConfig, StorageError, StorageManager, StorageResult,
    PLUGIN_STORAGE_DEFAULT_QUOTA, PLUGIN_STORAGE_MAX_KEY_LENGTH, PLUGIN_STORAGE_MAX_VALUE_LENGTH,
};
pub use types::{
    KeyCode, KeyEvent, KeyModifiers, Plugin, PluginAction, PluginError, PluginEvent, PluginId,
    PluginMetrics, PluginResult, PluginState, TerminalInfo,
};
pub use bridge::{
    BridgeMetrics, NativePluginProcessor, PluginBridge, PluginBridgeConfig, ProcessResult,
};

#[cfg(feature = "wasm-plugins")]
pub use runtime::{PluginExecutor, PluginInstance, WasmConfig, WasmError, WasmRuntime};
