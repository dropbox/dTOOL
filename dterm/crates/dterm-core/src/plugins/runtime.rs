//! WASM plugin runtime using wasmtime.
//!
//! This module provides the actual WASM execution environment for plugins,
//! including memory limits, fuel-based CPU budgets, and host function bindings.
//!
//! ## Permission Gating
//!
//! All host functions check permissions before executing. If a plugin
//! attempts to call a function it doesn't have permission for, the
//! call returns an error code.
//!
//! ## Storage API
//!
//! Plugins with the `Storage` permission can use key-value storage:
//! - `host_storage_get(key_ptr, key_len)` - Get a value
//! - `host_storage_set(key_ptr, key_len, val_ptr, val_len)` - Set a value
//! - `host_storage_delete(key_ptr, key_len)` - Delete a value

// WASM uses i32 for all pointers and lengths. These conversions are intentional
// and bounds-checked at the call site (negative values return errors).
#![allow(clippy::cast_sign_loss)]
#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::cast_possible_wrap)]

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

use parking_lot::Mutex;
use wasmtime::{Config, Engine, Linker, Module, Store, StoreLimits, StoreLimitsBuilder};

use super::manifest::{Permission, PluginManifest};
use super::permissions::PermissionChecker;
use super::storage::{PluginStorage, StorageConfig};
use super::types::{
    Plugin, PluginAction, PluginError, PluginEvent, PluginId, PluginMetrics, PluginResult,
    PluginState,
};

// Storage API return codes
/// Storage operation succeeded.
const STORAGE_OK: i32 = 0;
/// Storage operation denied (permission not granted).
const STORAGE_ERR_PERMISSION_DENIED: i32 = -1;
/// Key not found in storage.
const STORAGE_ERR_NOT_FOUND: i32 = -2;
/// Key is too long.
const STORAGE_ERR_KEY_TOO_LONG: i32 = -3;
/// Value is too large.
const STORAGE_ERR_VALUE_TOO_LARGE: i32 = -4;
/// Storage quota exceeded.
const STORAGE_ERR_QUOTA_EXCEEDED: i32 = -5;

/// Configuration for the WASM runtime.
#[derive(Debug, Clone)]
pub struct WasmConfig {
    /// Maximum memory per plugin instance (bytes).
    pub max_memory: usize,
    /// Fuel (CPU budget) per event.
    pub fuel_per_event: u64,
    /// Maximum event queue size per plugin.
    pub max_queue_size: usize,
    /// Timeout for plugin operations.
    pub timeout: Duration,
    /// Whether to enable WASM SIMD.
    pub enable_simd: bool,
    /// Whether to enable reference types.
    pub enable_reference_types: bool,
}

impl Default for WasmConfig {
    fn default() -> Self {
        Self {
            max_memory: 32 * 1024 * 1024, // 32 MiB
            fuel_per_event: 10_000_000,    // ~10ms of execution
            max_queue_size: 1000,
            timeout: Duration::from_millis(100),
            enable_simd: true,
            enable_reference_types: true,
        }
    }
}

/// Errors specific to WASM execution.
#[derive(Debug, Clone)]
pub enum WasmError {
    /// Failed to compile WASM module.
    Compilation(String),
    /// Failed to instantiate module.
    Instantiation(String),
    /// Module validation failed.
    Validation(String),
    /// Runtime trap occurred.
    Trap(String),
    /// Function not found in module.
    FunctionNotFound(String),
    /// Invalid return type from function.
    InvalidReturn(String),
    /// Fuel exhausted.
    FuelExhausted,
    /// Memory limit exceeded.
    MemoryExceeded,
}

impl std::fmt::Display for WasmError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Compilation(msg) => write!(f, "wasm compilation error: {msg}"),
            Self::Instantiation(msg) => write!(f, "wasm instantiation error: {msg}"),
            Self::Validation(msg) => write!(f, "wasm validation error: {msg}"),
            Self::Trap(msg) => write!(f, "wasm trap: {msg}"),
            Self::FunctionNotFound(name) => write!(f, "function not found: {name}"),
            Self::InvalidReturn(msg) => write!(f, "invalid return: {msg}"),
            Self::FuelExhausted => write!(f, "wasm fuel exhausted"),
            Self::MemoryExceeded => write!(f, "wasm memory limit exceeded"),
        }
    }
}

impl std::error::Error for WasmError {}

// ============================================================================
// WASM Memory Access Helpers
// ============================================================================

/// Read a slice from WASM linear memory.
///
/// # Arguments
/// * `caller` - The WASM caller context (mutable for wasmtime API)
/// * `ptr` - Pointer offset in WASM memory
/// * `len` - Length of data to read
///
/// # Returns
/// The bytes read from WASM memory, or None if out of bounds.
fn read_wasm_memory_mut(
    caller: &mut wasmtime::Caller<'_, HostState>,
    ptr: i32,
    len: i32,
) -> Option<Vec<u8>> {
    if ptr < 0 || len < 0 {
        return None;
    }
    let ptr = ptr as usize;
    let len = len as usize;

    // Get the "memory" export from the WASM module
    let memory = caller.get_export("memory")?.into_memory()?;
    let data = memory.data(caller);

    // Bounds check
    if ptr.saturating_add(len) > data.len() {
        return None;
    }

    Some(data[ptr..ptr + len].to_vec())
}

/// Alias for read_wasm_memory_mut for code compatibility.
/// In wasmtime, all memory access through Caller requires mutable access.
#[inline]
fn read_wasm_memory(
    caller: &mut wasmtime::Caller<'_, HostState>,
    ptr: i32,
    len: i32,
) -> Option<Vec<u8>> {
    read_wasm_memory_mut(caller, ptr, len)
}

/// Write a slice to WASM linear memory.
///
/// # Arguments
/// * `caller` - The WASM caller context
/// * `ptr` - Pointer offset in WASM memory
/// * `data` - Data to write
///
/// # Returns
/// Number of bytes written, or -1 if out of bounds.
fn write_wasm_memory(
    caller: &mut wasmtime::Caller<'_, HostState>,
    ptr: i32,
    data: &[u8],
) -> i32 {
    if ptr < 0 {
        return -1;
    }
    let ptr = ptr as usize;
    let len = data.len();

    // Get the "memory" export from the WASM module
    let memory = match caller.get_export("memory").and_then(|e| e.into_memory()) {
        Some(m) => m,
        None => return -1,
    };

    let mem_data = memory.data_mut(caller);

    // Bounds check
    if ptr.saturating_add(len) > mem_data.len() {
        return -1;
    }

    mem_data[ptr..ptr + len].copy_from_slice(data);
    len as i32
}

/// Read a string from WASM linear memory.
///
/// # Arguments
/// * `caller` - The WASM caller context (mutable for wasmtime API)
/// * `ptr` - Pointer offset in WASM memory
/// * `len` - Length of string data
///
/// # Returns
/// The UTF-8 string, or None if invalid.
fn read_wasm_string(
    caller: &mut wasmtime::Caller<'_, HostState>,
    ptr: i32,
    len: i32,
) -> Option<String> {
    let bytes = read_wasm_memory(caller, ptr, len)?;
    String::from_utf8(bytes).ok()
}

/// Host state accessible to WASM plugins.
struct HostState {
    /// Plugin ID for logging.
    plugin_id: PluginId,
    /// Permission checker for this plugin.
    permissions: PermissionChecker,
    /// Key-value storage for the plugin.
    storage: PluginStorage,
    /// Resource limits.
    limits: StoreLimits,
    /// Last action returned by plugin.
    last_action: Option<PluginAction>,
    /// Metrics.
    metrics: PluginMetrics,
    /// Temporary buffer for returning data to WASM (for storage_get).
    return_buffer: Vec<u8>,
}

impl HostState {
    fn new(plugin_id: PluginId, permissions: std::collections::HashSet<Permission>, config: &WasmConfig) -> Self {
        let permission_checker = PermissionChecker::new(plugin_id, permissions);
        let storage_config = StorageConfig::default();

        Self {
            plugin_id,
            permissions: permission_checker,
            storage: PluginStorage::with_config(plugin_id, storage_config),
            limits: StoreLimitsBuilder::new()
                .memory_size(config.max_memory)
                .build(),
            last_action: None,
            metrics: PluginMetrics::default(),
            return_buffer: Vec::new(),
        }
    }

    /// Check if a permission is granted.
    fn has_permission(&self, perm: Permission) -> bool {
        self.permissions.has_permission(perm)
    }

    /// Get storage reference.
    fn storage(&self) -> &PluginStorage {
        &self.storage
    }

    /// Get mutable storage reference.
    fn storage_mut(&mut self) -> &mut PluginStorage {
        &mut self.storage
    }
}

/// A WASM plugin runtime managing multiple plugin instances.
pub struct WasmRuntime {
    /// Wasmtime engine (shared across instances).
    engine: Engine,
    /// Runtime configuration.
    config: WasmConfig,
    /// Active plugin instances.
    instances: HashMap<PluginId, Arc<Mutex<PluginInstance>>>,
    /// Next plugin ID.
    next_id: u32,
}

impl WasmRuntime {
    /// Create a new WASM runtime with default configuration.
    pub fn new() -> Result<Self, WasmError> {
        Self::with_config(WasmConfig::default())
    }

    /// Create a new WASM runtime with custom configuration.
    pub fn with_config(config: WasmConfig) -> Result<Self, WasmError> {
        let mut wasm_config = Config::new();

        // Enable fuel consumption for CPU limiting
        wasm_config.consume_fuel(true);

        // Configure WASM features
        // Note: In wasmtime 26+, many features are enabled by default
        // and the configuration API has changed. We configure what's available.
        let _ = config.enable_simd; // SIMD enabled by default in wasmtime 26+
        let _ = config.enable_reference_types; // Reference types enabled by default

        // Use Cranelift for compilation
        wasm_config.strategy(wasmtime::Strategy::Cranelift);

        let engine = Engine::new(&wasm_config)
            .map_err(|e| WasmError::Compilation(e.to_string()))?;

        Ok(Self {
            engine,
            config,
            instances: HashMap::new(),
            next_id: 0,
        })
    }

    /// Load a plugin from a directory.
    pub fn load_plugin(&mut self, plugin_dir: &Path) -> Result<PluginId, WasmError> {
        // Read manifest
        let manifest_path = plugin_dir.join("plugin.toml");
        let manifest_content = std::fs::read_to_string(&manifest_path)
            .map_err(|e| WasmError::Validation(format!("failed to read manifest: {e}")))?;

        let manifest = super::manifest::parse_manifest(&manifest_content)
            .map_err(|e| WasmError::Validation(e.to_string()))?;

        // Validate manifest
        super::manifest::validate_manifest(&manifest, plugin_dir)
            .map_err(|e| WasmError::Validation(e.to_string()))?;

        // Load WASM module
        let wasm_path = plugin_dir.join(&manifest.entry);
        let wasm_bytes = std::fs::read(&wasm_path)
            .map_err(|e| WasmError::Compilation(format!("failed to read wasm: {e}")))?;

        self.load_plugin_from_bytes(&manifest, &wasm_bytes)
    }

    /// Load a plugin from manifest and WASM bytes.
    pub fn load_plugin_from_bytes(
        &mut self,
        manifest: &PluginManifest,
        wasm_bytes: &[u8],
    ) -> Result<PluginId, WasmError> {
        // Compile module
        let module = Module::new(&self.engine, wasm_bytes)
            .map_err(|e| WasmError::Compilation(e.to_string()))?;

        // Allocate plugin ID
        let plugin_id = PluginId(self.next_id);
        self.next_id += 1;

        // Create instance
        let instance = PluginInstance::new(
            plugin_id,
            manifest.clone(),
            &self.engine,
            module,
            &self.config,
        )?;

        self.instances.insert(plugin_id, Arc::new(Mutex::new(instance)));

        Ok(plugin_id)
    }

    /// Get a reference to a plugin instance.
    pub fn get(&self, id: PluginId) -> Option<Arc<Mutex<PluginInstance>>> {
        self.instances.get(&id).cloned()
    }

    /// Unload a plugin.
    pub fn unload(&mut self, id: PluginId) -> PluginResult<()> {
        if let Some(instance) = self.instances.remove(&id) {
            let mut inst = instance.lock();
            inst.state = PluginState::Unloaded;
            Ok(())
        } else {
            Err(PluginError::NotFound(id))
        }
    }

    /// Get all loaded plugin IDs.
    pub fn plugins(&self) -> Vec<PluginId> {
        self.instances.keys().copied().collect()
    }

    /// Get the runtime configuration.
    pub fn config(&self) -> &WasmConfig {
        &self.config
    }
}

impl Default for WasmRuntime {
    fn default() -> Self {
        Self::new().expect("failed to create default WasmRuntime")
    }
}

/// A single plugin instance with its WASM store and state.
pub struct PluginInstance {
    /// Plugin ID.
    id: PluginId,
    /// Plugin manifest.
    manifest: PluginManifest,
    /// Current state.
    state: PluginState,
    /// Wasmtime store with host state.
    store: Store<HostState>,
    /// Compiled module.
    #[allow(dead_code)]
    module: Module,
    /// Linker for host functions.
    #[allow(dead_code)]
    linker: Linker<HostState>,
    /// Fuel budget per event.
    fuel_per_event: u64,
}

impl PluginInstance {
    /// Create a new plugin instance.
    fn new(
        id: PluginId,
        manifest: PluginManifest,
        engine: &Engine,
        module: Module,
        config: &WasmConfig,
    ) -> Result<Self, WasmError> {
        let host_state = HostState::new(id, manifest.permissions.clone(), config);
        let mut store = Store::new(engine, host_state);

        // Set resource limits
        store.limiter(|state| &mut state.limits);

        // Add initial fuel
        store
            .set_fuel(config.fuel_per_event)
            .map_err(|e| WasmError::Instantiation(e.to_string()))?;

        // Create linker and add host functions
        let mut linker = Linker::new(engine);
        Self::add_host_functions(&mut linker)?;

        Ok(Self {
            id,
            manifest,
            state: PluginState::Ready,
            store,
            module,
            linker,
            fuel_per_event: config.fuel_per_event,
        })
    }

    /// Add host functions to the linker.
    fn add_host_functions(linker: &mut Linker<HostState>) -> Result<(), WasmError> {
        // Host function: log(level: i32, msg_ptr: i32, msg_len: i32)
        // No permission required - logging is always allowed
        // Level: 0=error, 1=warn, 2=info, 3+=debug
        linker
            .func_wrap("env", "host_log", |mut caller: wasmtime::Caller<'_, HostState>, level: i32, ptr: i32, len: i32| {
                // Read the log message from WASM memory
                if let Some(msg) = read_wasm_string(&mut caller, ptr, len) {
                    let plugin_id = caller.data().plugin_id;
                    let level_str = match level {
                        0 => "ERROR",
                        1 => "WARN",
                        2 => "INFO",
                        _ => "DEBUG",
                    };
                    // Log to stderr (in production, this would integrate with a logging framework)
                    #[cfg(debug_assertions)]
                    eprintln!("[plugin {}] {}: {}", plugin_id, level_str, msg);
                    // In release, suppress unless level is error/warn
                    #[cfg(not(debug_assertions))]
                    if level <= 1 {
                        eprintln!("[plugin {}] {}: {}", plugin_id, level_str, msg);
                    }
                }
            })
            .map_err(|e| WasmError::Instantiation(e.to_string()))?;

        // Host function: storage_get(key_ptr: i32, key_len: i32) -> i32
        // Returns: -1 = permission denied, -2 = not found, >= 0 = value length
        // Requires: Storage permission
        linker
            .func_wrap("env", "host_storage_get", |mut caller: wasmtime::Caller<'_, HostState>, key_ptr: i32, key_len: i32| -> i32 {
                // Check permission
                if !caller.data().has_permission(Permission::Storage) {
                    return STORAGE_ERR_PERMISSION_DENIED;
                }

                // Read key from WASM memory
                let key = match read_wasm_string(&mut caller, key_ptr, key_len) {
                    Some(k) => k,
                    None => return STORAGE_ERR_KEY_TOO_LONG, // Invalid memory access
                };

                // Look up value
                match caller.data().storage().get(&key) {
                    Ok(value) => {
                        // Store value in return buffer for subsequent read
                        let len = value.len();
                        caller.data_mut().return_buffer = value.to_vec();
                        // Return value length as i32, saturating at i32::MAX if necessary
                        len.min(i32::MAX as usize) as i32
                    }
                    Err(_) => STORAGE_ERR_NOT_FOUND,
                }
            })
            .map_err(|e| WasmError::Instantiation(e.to_string()))?;

        // Host function: storage_get_value(buf_ptr: i32, buf_len: i32) -> i32
        // Copies the last retrieved value into the provided buffer.
        // Returns: number of bytes copied, or -1 if buffer too small
        linker
            .func_wrap("env", "host_storage_get_value", |mut caller: wasmtime::Caller<'_, HostState>, buf_ptr: i32, buf_len: i32| -> i32 {
                let value = caller.data().return_buffer.clone();
                let value_len = value.len();

                if buf_len < 0 || (buf_len as usize) < value_len {
                    return -1; // Buffer too small
                }

                // Write value to WASM memory
                let written = write_wasm_memory(&mut caller, buf_ptr, &value);
                if written < 0 {
                    return -1;
                }

                // Clear the return buffer after read
                caller.data_mut().return_buffer.clear();

                i32::try_from(value_len).unwrap_or(i32::MAX)
            })
            .map_err(|e| WasmError::Instantiation(e.to_string()))?;

        // Host function: storage_set(key_ptr: i32, key_len: i32, val_ptr: i32, val_len: i32) -> i32
        // Returns: 0 = success, -1 = permission denied, -2 = quota exceeded, -3 = key too long, -4 = value too large
        // Requires: Storage permission
        linker
            .func_wrap("env", "host_storage_set", |mut caller: wasmtime::Caller<'_, HostState>, key_ptr: i32, key_len: i32, val_ptr: i32, val_len: i32| -> i32 {
                // Check permission
                if !caller.data().has_permission(Permission::Storage) {
                    return STORAGE_ERR_PERMISSION_DENIED;
                }

                // Validate lengths
                if key_len < 0 || val_len < 0 {
                    return STORAGE_ERR_KEY_TOO_LONG;
                }

                // Read key from WASM memory
                let key = match read_wasm_string(&mut caller, key_ptr, key_len) {
                    Some(k) => k,
                    None => return STORAGE_ERR_KEY_TOO_LONG,
                };

                // Read value from WASM memory
                let value = match read_wasm_memory(&mut caller, val_ptr, val_len) {
                    Some(v) => v,
                    None => return STORAGE_ERR_VALUE_TOO_LARGE,
                };

                // Attempt to store
                match caller.data_mut().storage_mut().set(&key, &value) {
                    Ok(()) => STORAGE_OK,
                    Err(super::storage::StorageError::KeyTooLong { .. }) => STORAGE_ERR_KEY_TOO_LONG,
                    Err(super::storage::StorageError::ValueTooLarge { .. }) => STORAGE_ERR_VALUE_TOO_LARGE,
                    Err(super::storage::StorageError::QuotaExceeded { .. }) => STORAGE_ERR_QUOTA_EXCEEDED,
                    Err(super::storage::StorageError::TooManyKeys { .. }) => STORAGE_ERR_QUOTA_EXCEEDED,
                    Err(_) => STORAGE_ERR_PERMISSION_DENIED, // Catch-all
                }
            })
            .map_err(|e| WasmError::Instantiation(e.to_string()))?;

        // Host function: storage_delete(key_ptr: i32, key_len: i32) -> i32
        // Returns: 0 = success, -1 = permission denied, -2 = not found
        // Requires: Storage permission
        linker
            .func_wrap("env", "host_storage_delete", |mut caller: wasmtime::Caller<'_, HostState>, key_ptr: i32, key_len: i32| -> i32 {
                // Check permission
                if !caller.data().has_permission(Permission::Storage) {
                    return STORAGE_ERR_PERMISSION_DENIED;
                }

                // Read key from WASM memory
                let key = match read_wasm_string(&mut caller, key_ptr, key_len) {
                    Some(k) => k,
                    None => return STORAGE_ERR_KEY_TOO_LONG,
                };

                match caller.data_mut().storage_mut().delete(&key) {
                    Ok(()) => STORAGE_OK,
                    Err(_) => STORAGE_ERR_NOT_FOUND,
                }
            })
            .map_err(|e| WasmError::Instantiation(e.to_string()))?;

        // Host function: storage_contains(key_ptr: i32, key_len: i32) -> i32
        // Returns: 1 = exists, 0 = not exists, -1 = permission denied
        // Requires: Storage permission
        linker
            .func_wrap("env", "host_storage_contains", |mut caller: wasmtime::Caller<'_, HostState>, key_ptr: i32, key_len: i32| -> i32 {
                if !caller.data().has_permission(Permission::Storage) {
                    return STORAGE_ERR_PERMISSION_DENIED;
                }

                // Read key from WASM memory
                let key = match read_wasm_string(&mut caller, key_ptr, key_len) {
                    Some(k) => k,
                    None => return STORAGE_ERR_PERMISSION_DENIED,
                };

                i32::from(caller.data().storage().contains(&key))
            })
            .map_err(|e| WasmError::Instantiation(e.to_string()))?;

        // Host function: get_terminal_cols() -> i32
        // Requires: TerminalRead permission
        linker
            .func_wrap("env", "host_get_terminal_cols", |caller: wasmtime::Caller<'_, HostState>| -> i32 {
                if !caller.data().has_permission(Permission::TerminalRead) {
                    return -1; // Permission denied
                }
                80 // Default width (would come from actual terminal state)
            })
            .map_err(|e| WasmError::Instantiation(e.to_string()))?;

        // Host function: get_terminal_rows() -> i32
        // Requires: TerminalRead permission
        linker
            .func_wrap("env", "host_get_terminal_rows", |caller: wasmtime::Caller<'_, HostState>| -> i32 {
                if !caller.data().has_permission(Permission::TerminalRead) {
                    return -1; // Permission denied
                }
                24 // Default height (would come from actual terminal state)
            })
            .map_err(|e| WasmError::Instantiation(e.to_string()))?;

        // Host function: set_action(action_type: i32, data_ptr: i32, data_len: i32) -> i32
        // Returns: 0 = success, -1 = permission denied for action type
        linker
            .func_wrap("env", "host_set_action", |mut caller: wasmtime::Caller<'_, HostState>, action_type: i32, _data_ptr: i32, _data_len: i32| -> i32 {
                // Map action type to PluginAction and check permissions
                let (action, requires_write) = match action_type {
                    0 => (PluginAction::Continue, false),
                    1 => (PluginAction::Consume, false),
                    2 => (PluginAction::Transform(vec![]), true), // Would read data from memory
                    3 => (PluginAction::EmitInput(vec![]), true), // Would read data from memory
                    _ => (PluginAction::Continue, false),
                };

                // Check write permission if needed
                if requires_write && !caller.data().has_permission(Permission::TerminalWrite) {
                    return -1; // Permission denied
                }

                caller.data_mut().last_action = Some(action);
                0 // Success
            })
            .map_err(|e| WasmError::Instantiation(e.to_string()))?;

        Ok(())
    }

    /// Refuel the store before processing an event.
    fn refuel(&mut self) -> Result<(), WasmError> {
        self.store
            .set_fuel(self.fuel_per_event)
            .map_err(|e| WasmError::Trap(e.to_string()))
    }

    /// Get remaining fuel.
    #[allow(dead_code)]
    fn remaining_fuel(&self) -> u64 {
        self.store.get_fuel().unwrap_or(0)
    }
}

impl Plugin for PluginInstance {
    fn id(&self) -> PluginId {
        self.id
    }

    fn name(&self) -> &str {
        &self.manifest.name
    }

    fn state(&self) -> PluginState {
        self.state
    }

    fn process(&mut self, event: &PluginEvent) -> PluginResult<PluginAction> {
        if self.state != PluginState::Ready {
            return Err(PluginError::InvalidState {
                plugin: self.id,
                expected: PluginState::Ready,
                actual: self.state,
            });
        }

        self.state = PluginState::Processing;
        let start = Instant::now();

        // Refuel for this event
        if let Err(e) = self.refuel() {
            self.state = PluginState::Error;
            return Err(PluginError::Trap {
                plugin: self.id,
                message: e.to_string(),
            });
        }

        // Clear last action
        self.store.data_mut().last_action = None;

        // In a full implementation, we would:
        // 1. Serialize the event to memory
        // 2. Call the appropriate WASM export (on_output, on_key, etc.)
        // 3. Read the result

        // For now, return Continue as a stub
        let action = match event {
            PluginEvent::Output { .. } => {
                // Would call wasm export "on_output"
                PluginAction::Continue
            }
            PluginEvent::Key(_) => {
                // Would call wasm export "on_key"
                PluginAction::Continue
            }
            PluginEvent::CommandStart { .. } => {
                // Would call wasm export "on_command_start"
                PluginAction::Continue
            }
            PluginEvent::CommandComplete { .. } => {
                // Would call wasm export "on_command_complete"
                PluginAction::Continue
            }
            PluginEvent::Tick { .. } => {
                // Would call wasm export "on_tick"
                PluginAction::Continue
            }
        };

        // Update metrics
        let elapsed = start.elapsed();
        let metrics = &mut self.store.data_mut().metrics;
        metrics.events_processed += 1;
        // Saturate at u64::MAX for very long executions (unlikely in practice)
        // The min() ensures the value fits in u64, making the truncation safe.
        #[allow(clippy::cast_possible_truncation)]
        let elapsed_us = elapsed.as_micros().min(u128::from(u64::MAX)) as u64;
        metrics.total_processing_us = metrics.total_processing_us.saturating_add(elapsed_us);

        self.state = PluginState::Ready;
        Ok(action)
    }

    fn metrics(&self) -> &PluginMetrics {
        &self.store.data().metrics
    }

    fn pause(&mut self) -> PluginResult<()> {
        if self.state == PluginState::Ready {
            self.state = PluginState::Paused;
            Ok(())
        } else {
            Err(PluginError::InvalidState {
                plugin: self.id,
                expected: PluginState::Ready,
                actual: self.state,
            })
        }
    }

    fn resume(&mut self) -> PluginResult<()> {
        if self.state == PluginState::Paused {
            self.state = PluginState::Ready;
            Ok(())
        } else {
            Err(PluginError::InvalidState {
                plugin: self.id,
                expected: PluginState::Paused,
                actual: self.state,
            })
        }
    }

    fn unload(&mut self) -> PluginResult<()> {
        self.state = PluginState::Unloaded;
        Ok(())
    }
}

/// Plugin executor for processing events across multiple plugins.
pub struct PluginExecutor {
    /// Runtime reference.
    runtime: WasmRuntime,
    /// Plugin priority order.
    priority: Vec<PluginId>,
}

impl PluginExecutor {
    /// Create a new executor.
    pub fn new(runtime: WasmRuntime) -> Self {
        Self {
            runtime,
            priority: Vec::new(),
        }
    }

    /// Load a plugin and add it to the executor.
    pub fn load_plugin(&mut self, plugin_dir: &Path) -> Result<PluginId, WasmError> {
        let id = self.runtime.load_plugin(plugin_dir)?;
        self.priority.push(id);
        Ok(id)
    }

    /// Process an event through all plugins in priority order.
    pub fn process_event(&mut self, event: &PluginEvent) -> PluginAction {
        let mut final_action = PluginAction::Continue;

        for &id in &self.priority {
            if let Some(instance_arc) = self.runtime.get(id) {
                let mut instance = instance_arc.lock();
                if instance.state() == PluginState::Ready {
                    if let Ok(action) = instance.process(event) {
                        match &action {
                            PluginAction::Consume => {
                                // Stop processing, event is consumed
                                return action;
                            }
                            PluginAction::Transform(data) => {
                                // Later plugins get transformed data
                                final_action = PluginAction::Transform(data.clone());
                            }
                            _ => {}
                        }
                    }
                    // On error, continue with other plugins
                }
            }
        }

        final_action
    }

    /// Get the runtime.
    pub fn runtime(&self) -> &WasmRuntime {
        &self.runtime
    }

    /// Get mutable runtime reference.
    pub fn runtime_mut(&mut self) -> &mut WasmRuntime {
        &mut self.runtime
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wasm_config_default() {
        let config = WasmConfig::default();
        assert_eq!(config.max_memory, 32 * 1024 * 1024);
        assert_eq!(config.fuel_per_event, 10_000_000);
        assert!(config.enable_simd);
    }

    #[test]
    fn test_wasm_runtime_creation() {
        let runtime = WasmRuntime::new();
        assert!(runtime.is_ok());
    }

    #[test]
    fn test_wasm_runtime_with_custom_config() {
        let config = WasmConfig {
            max_memory: 16 * 1024 * 1024,
            fuel_per_event: 5_000_000,
            ..Default::default()
        };
        let runtime = WasmRuntime::with_config(config);
        assert!(runtime.is_ok());
    }

    #[test]
    fn test_wasm_error_display() {
        let err = WasmError::FuelExhausted;
        assert_eq!(format!("{err}"), "wasm fuel exhausted");

        let err = WasmError::FunctionNotFound("on_output".to_string());
        assert_eq!(format!("{err}"), "function not found: on_output");
    }

    #[test]
    fn test_plugin_executor_creation() {
        let runtime = WasmRuntime::new().unwrap();
        let executor = PluginExecutor::new(runtime);
        assert!(executor.runtime.plugins().is_empty());
    }
}
