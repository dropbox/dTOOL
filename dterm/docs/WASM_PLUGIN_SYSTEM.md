# WASM Plugin System - Design

## Goals
- Enable user-scriptable extensions with strong sandboxing.
- Provide stable, minimal host APIs for terminal events and commands.
- Keep plugin execution isolated, bounded, and observable.
- Support future expansion without breaking v1 plugins.

## Non-Goals
- Full access to host filesystem, network, or process spawning.
- Real-time UI rendering inside plugins (host-controlled only).
- Zero-cost performance overhead (safety and isolation first).

## Threat Model
- Untrusted plugin code must not access host resources beyond granted permissions.
- Plugins must not block terminal rendering or input processing.
- Plugins must not exhaust memory or CPU resources.
- Plugins must not compromise user data without explicit permissions.

## Runtime Choice
- Default runtime: wasmtime (mature sandboxing, fuel, epoch interrupts).
- Optional runtimes can be added behind feature flags if needed.

## Plugin Package Format
- Directory layout:
  - plugin.toml
  - plugin.wasm
  - assets/ (optional, read-only)

Example `plugin.toml`:
```
name = "example-plugin"
version = "0.1.0"
entry = "plugin.wasm"
min_dterm_version = "0.9.0"
permissions = ["terminal.read", "terminal.write", "storage"]
```

## Permissions
- terminal.read: receive terminal output and state snapshots.
- terminal.write: emit input to terminal (guarded by policy).
- terminal.command: observe command blocks and metadata.
- storage: use host-provided key/value storage (size capped).
- clipboard.read / clipboard.write: optional, off by default.
- net: disabled for v1.
- fs: disabled for v1.

## Host API (v1)
Events to plugins:
- on_output(bytes, metadata)
- on_key(key_event)
- on_command_start(command_block)
- on_command_complete(command_block)
- on_tick(now_ms)

Actions from plugins:
- Continue
- Consume
- Transform(bytes)
- EmitInput(bytes)
- EmitCommand(command, args)
- Annotate(range, style)

Host calls exposed to plugins:
- log(level, message)
- get_terminal_info() -> size, cursor, modes
- request_snapshot(range) -> bytes
- storage_get(key) -> bytes
- storage_set(key, bytes)

## Lifecycle
1. Discover: load plugin manifest and validate permissions.
2. Validate: verify wasm module, enforce memory and feature limits.
3. Instantiate: create instance with host API bindings.
4. Configure: pass config and permission grants.
5. Run: process events from host queue with time budget.
6. Shutdown: release instance, flush metrics, persist storage.

## Execution Model
- Dedicated plugin executor thread per plugin.
- Host thread enqueues events; plugin thread processes with fuel limits.
- Timeouts enforced with wasmtime epoch interrupts.
- On repeated traps or budget overrun, plugin is disabled.

## Resource Limits
- Memory: fixed max (configurable, default 32 MiB).
- CPU: per-event fuel budget, soft limit with backoff.
- Event queue: bounded; overflow drops oldest non-critical events.

## Observability
- Metrics per plugin: events processed, traps, timeouts, bytes transformed.
- Structured logs tagged with plugin id and permission set.

## Integration Points
- Parser output path: raw bytes + decoded screen events (configurable).
- Input path: key events and paste events.
- Command blocks: OSC 133 integration for start/complete boundaries.
- Search and selection: optional read-only snapshots (future).

## Error Handling
- Trap: disable plugin and surface error in logs.
- Permission violation: reject action and emit warning.
- Malformed output: ignore action, continue host processing.

## Verification Plan
- TLA+ spec for plugin lifecycle and event queue state machine.
- Kani proofs for any unsafe FFI glue.
- MIRI for unsafe blocks.
- Fuzz plugin loader with malformed wasm modules and manifests.
- Proptest for action mapping and permission enforcement.

## Implementation Phases
1. Loader + runtime scaffolding (manifest validation, wasm instantiation). **COMPLETE** (iter 436)
2. Event queue + lifecycle control with budgets and metrics. **COMPLETE** (iter 437)
3. Permission gating and storage API. **COMPLETE** (iter 438)
4. Integrations: output, input, command blocks. **COMPLETE** (iter 443)
5. Hardening: fuzzing, metrics, failure recovery.

## Phase 3 Implementation Details (iter 438)

### Permission Gating
- `PermissionChecker`: Validates actions against granted permissions
- `can_receive_event()`: Checks if plugin can receive event types
- `can_perform_action()`: Validates action permissions (Transform/EmitInput require TerminalWrite)
- `require_permission()`: Helper that returns Result for error handling
- All host functions check permissions before executing

### Storage API
- `PluginStorage`: Per-plugin isolated key-value store
- `StorageManager`: Manages storage for multiple plugins
- Configurable limits:
  - `MAX_KEY_LENGTH`: 256 bytes
  - `MAX_VALUE_LENGTH`: 64 KiB
  - `DEFAULT_STORAGE_QUOTA`: 1 MiB per plugin
  - `max_keys`: 1000 per plugin
- Host functions:
  - `host_storage_get(key_ptr, key_len)` -> value length or error code
  - `host_storage_get_value(buf_ptr, buf_len)` -> copies value to buffer
  - `host_storage_set(key_ptr, key_len, val_ptr, val_len)` -> 0 or error
  - `host_storage_delete(key_ptr, key_len)` -> 0 or error
  - `host_storage_contains(key_ptr, key_len)` -> 1/0 or error
- Error codes: -1 (permission denied), -2 (not found), -3 (key too long), -4 (value too large), -5 (quota exceeded)

## Phase 4 Implementation Details (iter 443)

### PluginBridge
- `PluginBridge`: Central integration layer connecting terminal events to plugins
- Configuration via `PluginBridgeConfig`:
  - `enable_output_hooks`: Process terminal output through plugins (default: true)
  - `enable_input_hooks`: Process key events through plugins (default: true)
  - `enable_command_hooks`: Process OSC 133 shell events (default: true)
  - `max_processing_time_us`: Time budget per event (default: 1000µs)
  - `max_queue_capacity`: Event buffer size (default: 1000)

### Output Integration
- `process_output(data, in_command)`: Hook for terminal output bytes
- `process_output_high_priority(data, in_command)`: For escape sequences
- Plugins can Transform, Consume, or Continue the data flow
- Metrics tracked: `output_events`, `events_transformed`, `events_consumed`

### Input Integration
- `process_key(key, modifiers)`: Hook for keyboard events
- `process_char_key(c, modifiers)`: Convenience for character keys
- Plugins can intercept keys before they reach the PTY
- If `was_consumed` is true, key should not be sent to PTY

### Command Lifecycle Integration (OSC 133)
- `on_command_start(command, cwd)`: Called on OSC 133;B or OSC 133;C
- `on_command_complete(exit_code)`: Called on OSC 133;D
- Active command state tracked for duration calculation
- Plugins notified via `PluginEvent::CommandStart` and `PluginEvent::CommandComplete`

### WASM Memory Access
- `read_wasm_memory_mut(caller, ptr, len)`: Read bytes from WASM linear memory
- `write_wasm_memory(caller, ptr, data)`: Write bytes to WASM linear memory
- `read_wasm_string(caller, ptr, len)`: Read UTF-8 string from WASM memory
- All host functions now properly access WASM memory instead of using placeholders

### Native Plugin Support
- `NativePluginProcessor` trait for non-WASM plugins (testing, trusted code)
- `register_native_plugin()` / `unregister_native_plugin()`
- Native plugins must declare permissions like WASM plugins

### ProcessResult
- `action`: Final action after all plugins processed
- `plugins_processed`: Count of plugins that handled the event
- `processing_time_us`: Time spent in plugin processing
- `was_transformed`: Whether any plugin transformed the data
- `was_consumed`: Whether any plugin consumed the event

### BridgeMetrics
- `output_events`, `input_events`: Event counts by type
- `command_starts`, `command_completes`: Shell integration counts
- `events_consumed`, `events_transformed`: Plugin action counts
- `total_processing_time_us`: Aggregate processing time
- `events_dropped`: Queue overflow count
- `plugin_errors`: Total errors from plugin processing (Phase 5)
- `plugin_timeouts`: Count of time budget exceeded events (Phase 5)
- `plugins_disabled`: Plugins auto-disabled due to failures (Phase 5)

## Phase 5 Implementation Details (iter 465)

### Plugin Hardening & Failure Recovery
- Added comprehensive fuzz target `plugins.rs`:
  - Manifest parsing with malformed TOML/invalid permissions
  - Storage operations with boundary conditions and quota enforcement
  - Permission checking validation across all permission types
  - Bridge event processing with multiple plugins
  - Queue overflow and recovery scenarios
- Added `PluginHealth` struct for per-plugin error tracking:
  - `consecutive_errors`: Resets on successful processing
  - `total_errors`: Cumulative error count
  - `disabled`: Flag for auto-disabled plugins
  - `last_success`/`last_error`: Timestamps for debugging

### Failure Recovery Configuration
New `PluginBridgeConfig` fields:
- `max_consecutive_errors`: Threshold before auto-disable (default: 10)
- `auto_disable_failing_plugins`: Enable/disable auto-recovery (default: true)

### Recovery APIs
- `is_plugin_disabled(id)`: Check if plugin was auto-disabled
- `plugin_error_count(id)`: Get consecutive error count
- `reenable_plugin(id)`: Manually re-enable a disabled plugin (resets error counter)
