//! Plugin system fuzzer - tests WASM plugin infrastructure.
//!
//! This fuzzer exercises the plugin subsystem:
//! 1. Manifest parsing with arbitrary TOML-like input
//! 2. Storage operations with various key/value sizes
//! 3. Permission checking and gating
//! 4. Bridge event processing
//! 5. Queue overflow and recovery
//!
//! ## Running
//!
//! ```bash
//! cd crates/dterm-core
//! cargo +nightly fuzz run plugins -- -max_total_time=600
//! ```

#![no_main]

use arbitrary::{Arbitrary, Unstructured};
use libfuzzer_sys::fuzz_target;

use std::collections::HashSet;

use dterm_core::plugins::{
    parse_manifest, Permission, PermissionChecker, PluginBridge,
    PluginBridgeConfig, PluginStorage, StorageConfig, StorageManager,
};
use dterm_core::plugins::{
    KeyCode, KeyModifiers, PluginAction, PluginEvent, PluginId,
    PluginResult, PluginState, TerminalInfo, NativePluginProcessor,
};

// ============================================================================
// Phase 1: Manifest Parsing Fuzzing
// ============================================================================

/// Fuzz manifest parsing with arbitrary input.
fn fuzz_manifest_parsing(data: &[u8]) {
    // Try to parse as UTF-8 TOML
    if let Ok(content) = std::str::from_utf8(data) {
        // Parse should never panic, only return errors
        let _ = parse_manifest(content);
    }

    // Also try with various malformed inputs
    let variants = [
        // Empty
        "",
        // Missing fields
        "name = \"test\"",
        "version = \"1.0.0\"",
        "entry = \"plugin.wasm\"",
        // Invalid permission arrays
        "name = \"test\"\nversion = \"1.0\"\nentry = \"p.wasm\"\npermissions = []",
        "name = \"test\"\nversion = \"1.0\"\nentry = \"p.wasm\"\npermissions = [",
        "name = \"test\"\nversion = \"1.0\"\nentry = \"p.wasm\"\npermissions = ]",
        // Very long name
        &format!("name = \"{}\"\nversion = \"1.0\"\nentry = \"p.wasm\"", "a".repeat(1000)),
        // Invalid characters in name
        "name = \"test plugin!\"\nversion = \"1.0\"\nentry = \"p.wasm\"",
        "name = \"test/plugin\"\nversion = \"1.0\"\nentry = \"p.wasm\"",
        // Unknown permissions
        "name = \"test\"\nversion = \"1.0\"\nentry = \"p.wasm\"\npermissions = [\"evil.hack\"]",
        // Nested brackets
        "name = \"test\"\nversion = \"1.0\"\nentry = \"p.wasm\"\npermissions = [[\"nested\"]]",
        // Unclosed quotes
        "name = \"test",
        "name = \"test\nversion = \"1.0\"",
        // Binary data in string
        "name = \"test\0null\"\nversion = \"1.0\"\nentry = \"p.wasm\"",
    ];

    for variant in variants {
        let _ = parse_manifest(variant);
    }
}

// ============================================================================
// Phase 2: Storage Operations Fuzzing
// ============================================================================

/// Storage operations to fuzz.
#[derive(Debug, Arbitrary)]
enum StorageOperation {
    /// Set a key-value pair
    Set { key: Vec<u8>, value: Vec<u8> },

    /// Get a key
    Get { key: Vec<u8> },

    /// Delete a key
    Delete { key: Vec<u8> },

    /// Check if key exists
    Contains { key: Vec<u8> },

    /// Clear all storage
    Clear,

    /// Query usage
    QueryUsage,

    /// Query remaining
    QueryRemaining,

    /// List all keys
    ListKeys,
}

/// Fuzz storage operations with arbitrary sequences.
fn fuzz_storage_operations(data: &[u8]) {
    if data.len() < 4 {
        return;
    }

    let mut u = Unstructured::new(data);

    // Create storage with various configurations
    let config_variant = u.arbitrary::<u8>().unwrap_or(0) % 4;
    let config = match config_variant {
        0 => StorageConfig::default(),
        1 => StorageConfig {
            quota: 100, // Very small quota
            max_key_length: 10,
            max_value_length: 20,
            max_keys: 5,
        },
        2 => StorageConfig {
            quota: 1024 * 1024, // 1 MiB
            max_key_length: 1,   // Single char keys
            max_value_length: 1024,
            max_keys: 1000,
        },
        _ => StorageConfig {
            quota: 10,
            max_key_length: 256,
            max_value_length: 64 * 1024,
            max_keys: 2,
        },
    };

    let mut storage = PluginStorage::with_config(PluginId(1), config);

    // Execute operations - limit to prevent infinite loops
    let max_ops = u.arbitrary::<u8>().unwrap_or(10).min(100) as usize;
    for _ in 0..max_ops {
        let Ok(op) = u.arbitrary::<StorageOperation>() else { break };
        match op {
            StorageOperation::Set { key, value } => {
                if let Ok(key_str) = std::str::from_utf8(&key) {
                    let _ = storage.set(key_str, &value);
                }
            }
            StorageOperation::Get { key } => {
                if let Ok(key_str) = std::str::from_utf8(&key) {
                    let _ = storage.get(key_str);
                }
            }
            StorageOperation::Delete { key } => {
                if let Ok(key_str) = std::str::from_utf8(&key) {
                    let _ = storage.delete(key_str);
                }
            }
            StorageOperation::Contains { key } => {
                if let Ok(key_str) = std::str::from_utf8(&key) {
                    let _ = storage.contains(key_str);
                }
            }
            StorageOperation::Clear => {
                storage.clear();
            }
            StorageOperation::QueryUsage => {
                let _ = storage.usage();
                let _ = storage.quota();
            }
            StorageOperation::QueryRemaining => {
                let _ = storage.remaining();
            }
            StorageOperation::ListKeys => {
                let _: Vec<_> = storage.keys().collect();
            }
        }

        // Invariant: usage should never exceed quota
        assert!(
            storage.usage() <= storage.quota(),
            "Storage usage {} exceeded quota {}",
            storage.usage(),
            storage.quota()
        );
    }
}

/// Fuzz storage manager with multiple plugins.
fn fuzz_storage_manager(data: &[u8]) {
    if data.len() < 4 {
        return;
    }

    let mut u = Unstructured::new(data);
    let mut manager = StorageManager::new();

    // Execute operations on multiple plugins
    for _ in 0..u.arbitrary::<u8>().unwrap_or(10).min(50) {
        let plugin_id = PluginId(u.arbitrary::<u32>().unwrap_or(0) % 10);
        let storage = manager.get_or_create(plugin_id);

        // Do some operations
        if let Ok(key) = u.arbitrary::<[u8; 8]>() {
            if let Ok(key_str) = std::str::from_utf8(&key) {
                if let Ok(value) = u.arbitrary::<[u8; 16]>() {
                    let _ = storage.set(key_str, &value);
                }
            }
        }
    }

    // Total usage should be sum of individual usages
    let calculated_total: usize = (0..10)
        .filter_map(|i| manager.get(PluginId(i)))
        .map(|s| s.usage())
        .sum();
    assert_eq!(manager.total_usage(), calculated_total);
}

// ============================================================================
// Phase 3: Permission Checking Fuzzing
// ============================================================================

/// Fuzz permission checking.
fn fuzz_permission_checking(data: &[u8]) {
    if data.len() < 2 {
        return;
    }

    let mut u = Unstructured::new(data);

    // Create permission set
    let mut permissions_vec = Vec::new();
    for _ in 0..u.arbitrary::<u8>().unwrap_or(3) % 8 {
        let perm = match u.arbitrary::<u8>().unwrap_or(0) % 6 {
            0 => Permission::TerminalRead,
            1 => Permission::TerminalWrite,
            2 => Permission::TerminalCommand,
            3 => Permission::Storage,
            4 => Permission::ClipboardRead,
            _ => Permission::ClipboardWrite,
        };
        permissions_vec.push(perm);
    }

    let permissions: HashSet<Permission> = permissions_vec.iter().copied().collect();
    let checker = PermissionChecker::new(PluginId(1), permissions.clone());

    // Check various permissions
    for perm in [
        Permission::TerminalRead,
        Permission::TerminalWrite,
        Permission::TerminalCommand,
        Permission::Storage,
        Permission::ClipboardRead,
        Permission::ClipboardWrite,
    ] {
        let has = checker.has_permission(perm);
        assert_eq!(has, permissions.contains(&perm));
    }

    // Check dangerous permissions
    for perm in &permissions {
        let _ = perm.is_dangerous();
    }

    // Test permission string parsing
    let perm_strings = [
        "terminal.read",
        "terminal.write",
        "terminal.command",
        "storage",
        "clipboard.read",
        "clipboard.write",
        "invalid",
        "",
        "terminal",
        "read",
    ];
    for s in perm_strings {
        let _ = Permission::from_str(s);
    }
}

// ============================================================================
// Phase 4: Bridge Event Processing Fuzzing
// ============================================================================

/// A test plugin that tracks events and can perform various actions.
struct FuzzPlugin {
    id: PluginId,
    state: PluginState,
    permissions: Vec<Permission>,
    action_mode: u8,
    events_seen: usize,
}

impl FuzzPlugin {
    fn new(id: u32, action_mode: u8) -> Self {
        Self {
            id: PluginId(id),
            state: PluginState::Ready,
            permissions: vec![
                Permission::TerminalRead,
                Permission::TerminalWrite,
                Permission::TerminalCommand,
            ],
            action_mode,
            events_seen: 0,
        }
    }
}

impl NativePluginProcessor for FuzzPlugin {
    fn id(&self) -> PluginId {
        self.id
    }

    fn name(&self) -> &str {
        "fuzz-plugin"
    }

    fn state(&self) -> PluginState {
        self.state
    }

    fn permissions(&self) -> &[Permission] {
        &self.permissions
    }

    fn process(&mut self, _event: &PluginEvent, _info: &TerminalInfo) -> PluginResult<PluginAction> {
        self.events_seen += 1;

        // Return different actions based on mode
        match self.action_mode % 5 {
            0 => Ok(PluginAction::Continue),
            1 => Ok(PluginAction::Consume),
            2 => Ok(PluginAction::Transform(vec![b'X'; 10])),
            3 => Ok(PluginAction::EmitInput(vec![b'Y'; 5])),
            _ => Ok(PluginAction::EmitCommand {
                command: "test".to_string(),
                args: vec!["arg".to_string()],
            }),
        }
    }
}

/// Bridge operations to fuzz.
#[derive(Debug, Arbitrary)]
enum BridgeOperation {
    /// Process terminal output
    ProcessOutput { data: Vec<u8>, in_command: bool },

    /// Process key event
    ProcessKey { key_code: u8, modifiers: u8 },

    /// Command lifecycle
    CommandStart { command: Vec<u8>, cwd: Option<Vec<u8>> },
    CommandComplete { exit_code: Option<i32> },

    /// Tick
    Tick { now_ms: u64 },

    /// Update terminal info
    UpdateInfo {
        cols: u16,
        rows: u16,
        cursor_col: u16,
        cursor_row: u16,
    },

    /// Queue operations
    DrainQueue,
    ClearQueue,

    /// Plugin management
    RegisterPlugin { action_mode: u8 },
    UnregisterPlugin { id: u32 },
}

/// Convert byte to KeyCode.
fn key_code_from_byte(b: u8) -> KeyCode {
    match b % 10 {
        0 => KeyCode::Char('a'),
        1 => KeyCode::Char('\n'),
        2 => KeyCode::Enter,
        3 => KeyCode::Backspace,
        4 => KeyCode::Tab,
        5 => KeyCode::Escape,
        6 => KeyCode::Left,
        7 => KeyCode::Right,
        8 => KeyCode::Up,
        _ => KeyCode::Down,
    }
}

/// Convert byte to KeyModifiers.
fn modifiers_from_byte(b: u8) -> KeyModifiers {
    let mut mods = KeyModifiers::empty();
    if b & 0x01 != 0 {
        mods |= KeyModifiers::SHIFT;
    }
    if b & 0x02 != 0 {
        mods |= KeyModifiers::CTRL;
    }
    if b & 0x04 != 0 {
        mods |= KeyModifiers::ALT;
    }
    mods
}

/// Fuzz bridge event processing.
fn fuzz_bridge_processing(data: &[u8]) {
    if data.len() < 4 {
        return;
    }

    let mut u = Unstructured::new(data);

    // Create bridge with various configurations
    let config_variant = u.arbitrary::<u8>().unwrap_or(0) % 4;
    let config = match config_variant {
        0 => PluginBridgeConfig::default(),
        1 => PluginBridgeConfig {
            enable_output_hooks: false,
            enable_input_hooks: true,
            enable_command_hooks: true,
            max_processing_time_us: 100,
            max_queue_capacity: 10,
            max_consecutive_errors: 3,
            auto_disable_failing_plugins: true,
        },
        2 => PluginBridgeConfig {
            enable_output_hooks: true,
            enable_input_hooks: false,
            enable_command_hooks: false,
            max_processing_time_us: 10000,
            max_queue_capacity: 1000,
            max_consecutive_errors: 100,
            auto_disable_failing_plugins: false,
        },
        _ => PluginBridgeConfig {
            enable_output_hooks: true,
            enable_input_hooks: true,
            enable_command_hooks: true,
            max_processing_time_us: 1,
            max_queue_capacity: 1, // Minimal queue
            max_consecutive_errors: 1,
            auto_disable_failing_plugins: true,
        },
    };

    let mut bridge = PluginBridge::with_config(config);
    let mut next_plugin_id = 0u32;

    // Execute operations - limit to prevent infinite loops
    let max_ops = u.arbitrary::<u8>().unwrap_or(10).min(100) as usize;
    for _ in 0..max_ops {
        let Ok(op) = u.arbitrary::<BridgeOperation>() else { break };
        match op {
            BridgeOperation::ProcessOutput { data, in_command } => {
                let result = bridge.process_output(&data, in_command);
                // Verify result is valid
                let _ = result.action;
                let _ = result.plugins_processed;
                let _ = result.was_consumed;
                let _ = result.was_transformed;
            }

            BridgeOperation::ProcessKey { key_code, modifiers } => {
                let key = key_code_from_byte(key_code);
                let mods = modifiers_from_byte(modifiers);
                let result = bridge.process_key(key, mods);
                let _ = result.action;
            }

            BridgeOperation::CommandStart { command, cwd } => {
                let command_str = String::from_utf8_lossy(&command).to_string();
                let cwd_str = cwd.map(|c| String::from_utf8_lossy(&c).to_string());
                bridge.on_command_start(command_str, cwd_str);
            }

            BridgeOperation::CommandComplete { exit_code } => {
                bridge.on_command_complete(exit_code);
            }

            BridgeOperation::Tick { now_ms } => {
                let _ = bridge.tick(now_ms);
            }

            BridgeOperation::UpdateInfo {
                cols,
                rows,
                cursor_col,
                cursor_row,
            } => {
                let info = TerminalInfo::from_terminal(
                    cols.max(1),
                    rows.max(1),
                    cursor_col,
                    cursor_row,
                    None,
                    false,
                    false,
                );
                bridge.update_terminal_info(info);
            }

            BridgeOperation::DrainQueue => {
                let _ = bridge.drain_queue();
            }

            BridgeOperation::ClearQueue => {
                bridge.clear_queue();
            }

            BridgeOperation::RegisterPlugin { action_mode } => {
                let plugin = FuzzPlugin::new(next_plugin_id, action_mode);
                next_plugin_id = next_plugin_id.wrapping_add(1);
                bridge.register_native_plugin(Box::new(plugin));
            }

            BridgeOperation::UnregisterPlugin { id } => {
                let _ = bridge.unregister_native_plugin(PluginId(id));
            }
        }

        // Verify metrics are consistent
        let metrics = bridge.metrics();
        // Output + input events should be non-negative (they are u64, so always true)
        let _ = metrics.output_events;
        let _ = metrics.input_events;
        let _ = metrics.command_starts;
        let _ = metrics.command_completes;
    }
}

// ============================================================================
// Phase 5: Queue Overflow and Recovery
// ============================================================================

/// Fuzz queue overflow scenarios.
fn fuzz_queue_overflow(data: &[u8]) {
    if data.len() < 4 {
        return;
    }

    let mut u = Unstructured::new(data);

    // Create bridge with tiny queue
    let config = PluginBridgeConfig {
        enable_output_hooks: true,
        enable_input_hooks: true,
        enable_command_hooks: true,
        max_processing_time_us: 1000,
        max_queue_capacity: 5,
        max_consecutive_errors: 5,
        auto_disable_failing_plugins: true,
    };

    let mut bridge = PluginBridge::with_config(config);

    // Flood with events to trigger overflow
    let event_count = u.arbitrary::<u8>().unwrap_or(20) as usize;
    for i in 0..event_count {
        let data_size = u.arbitrary::<u8>().unwrap_or(10) as usize;
        let data = vec![b'x'; data_size.min(100)];
        let _ = bridge.process_output(&data, i % 2 == 0);
    }

    // Queue should never exceed capacity
    assert!(
        bridge.queue_len() <= 5,
        "Queue exceeded capacity: {}",
        bridge.queue_len()
    );

    // Drain and verify
    let drained = bridge.drain_queue();
    assert!(drained <= 5);
    assert_eq!(bridge.queue_len(), 0);
}

// ============================================================================
// Main Fuzz Target
// ============================================================================

fuzz_target!(|data: &[u8]| {
    if data.len() < 4 {
        return;
    }

    // Use first byte to select test phase
    let phase = data[0] % 6;
    let remaining = &data[1..];

    match phase {
        0 => fuzz_manifest_parsing(remaining),
        1 => fuzz_storage_operations(remaining),
        2 => fuzz_storage_manager(remaining),
        3 => fuzz_permission_checking(remaining),
        4 => fuzz_bridge_processing(remaining),
        _ => fuzz_queue_overflow(remaining),
    }
});
