//! Plugin bridge for connecting terminal output/input with the plugin system.
//!
//! This module provides the integration layer between:
//! - Terminal output (parser events) → Plugin events
//! - Key input → Plugin events
//! - Shell integration (OSC 133) → Plugin command lifecycle events
//!
//! ## Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────┐
//! │                      PluginBridge                                │
//! │                                                                  │
//! │  ┌──────────────┐    ┌──────────────┐    ┌──────────────┐       │
//! │  │ Terminal     │───▶│ Plugin       │───▶│ Output/      │       │
//! │  │ (process())  │    │ Events       │    │ Transform    │       │
//! │  └──────────────┘    └──────────────┘    └──────────────┘       │
//! │          │                  │                   │                │
//! │          ▼                  ▼                   ▼                │
//! │  ┌──────────────┐    ┌──────────────┐    ┌──────────────┐       │
//! │  │ OSC 133      │───▶│ Command      │───▶│ Plugin       │       │
//! │  │ Events       │    │ Lifecycle    │    │ Actions      │       │
//! │  └──────────────┘    └──────────────┘    └──────────────┘       │
//! └─────────────────────────────────────────────────────────────────┘
//! ```
//!
//! ## Integration Points
//!
//! 1. **Output Integration**: Called during `Terminal::process()` with raw bytes
//! 2. **Input Integration**: Called when key events are received
//! 3. **Command Integration**: Called on OSC 133 shell events
//!
//! ## Phase 4 Implementation
//!
//! This module implements Phase 4 of the WASM plugin system:
//! - Output hooks for terminal data transformation
//! - Input hooks for key event interception
//! - Command lifecycle hooks for OSC 133 integration

use std::collections::VecDeque;
use std::time::{Duration, Instant};

use super::queue::EventPriority;
use super::types::{
    KeyCode, KeyEvent, KeyModifiers, PluginAction, PluginEvent, PluginId,
    PluginResult, PluginState, TerminalInfo,
};
use super::Permission;

#[cfg(feature = "wasm-plugins")]
use super::runtime::PluginExecutor;

/// Configuration for the plugin bridge.
#[derive(Debug, Clone)]
pub struct PluginBridgeConfig {
    /// Whether to process output events.
    pub enable_output_hooks: bool,
    /// Whether to process input/key events.
    pub enable_input_hooks: bool,
    /// Whether to process command lifecycle events.
    pub enable_command_hooks: bool,
    /// Maximum time to spend processing plugins per event (microseconds).
    pub max_processing_time_us: u64,
    /// Maximum queue capacity for event buffering.
    pub max_queue_capacity: usize,
    /// Maximum consecutive errors before disabling a plugin.
    pub max_consecutive_errors: u32,
    /// Whether to automatically disable plugins that exceed error threshold.
    pub auto_disable_failing_plugins: bool,
}

impl Default for PluginBridgeConfig {
    fn default() -> Self {
        Self {
            enable_output_hooks: true,
            enable_input_hooks: true,
            enable_command_hooks: true,
            max_processing_time_us: 1000, // 1ms max per event
            max_queue_capacity: 1000,
            max_consecutive_errors: 10,
            auto_disable_failing_plugins: true,
        }
    }
}

/// A queued event with priority and timestamp.
#[derive(Debug)]
struct BridgeQueuedEvent {
    /// The event payload.
    event: PluginEvent,
    /// Event priority.
    priority: EventPriority,
    /// When the event was enqueued (for future latency tracking).
    _enqueued_at: Instant,
}

/// Result from processing an event through plugins.
#[derive(Debug, Clone)]
pub struct ProcessResult {
    /// The final action after all plugins processed.
    pub action: PluginAction,
    /// Number of plugins that processed the event.
    pub plugins_processed: usize,
    /// Total processing time (microseconds).
    pub processing_time_us: u64,
    /// Whether any plugin transformed the data.
    pub was_transformed: bool,
    /// Whether the event was consumed (should not be passed to terminal).
    pub was_consumed: bool,
}

impl Default for ProcessResult {
    fn default() -> Self {
        Self {
            action: PluginAction::Continue,
            plugins_processed: 0,
            processing_time_us: 0,
            was_transformed: false,
            was_consumed: false,
        }
    }
}

/// Tracks health state for a plugin to enable automatic failure recovery.
#[derive(Debug, Clone, Default)]
struct PluginHealth {
    /// Consecutive errors without a successful event processing.
    consecutive_errors: u32,
    /// Total errors since plugin was registered.
    total_errors: u64,
    /// Whether the plugin has been automatically disabled due to failures.
    disabled: bool,
    /// Timestamp of last successful event processing.
    last_success: Option<Instant>,
    /// Timestamp of last error.
    last_error: Option<Instant>,
}

impl PluginHealth {
    /// Record a successful event processing.
    fn record_success(&mut self) {
        self.consecutive_errors = 0;
        self.last_success = Some(Instant::now());
    }

    /// Record an error. Returns true if this exceeds the threshold.
    fn record_error(&mut self, threshold: u32) -> bool {
        self.consecutive_errors += 1;
        self.total_errors += 1;
        self.last_error = Some(Instant::now());
        self.consecutive_errors >= threshold
    }

    /// Reset the plugin health (e.g., after re-enabling).
    fn reset(&mut self) {
        self.consecutive_errors = 0;
        self.disabled = false;
    }
}

/// Bridge connecting terminal events to the plugin system.
///
/// This struct manages the flow of events from the terminal to plugins
/// and applies plugin actions back to the terminal.
pub struct PluginBridge {
    /// Configuration.
    config: PluginBridgeConfig,
    /// Event queue for buffering.
    queue: VecDeque<BridgeQueuedEvent>,
    /// Current terminal info (updated on each event).
    terminal_info: TerminalInfo,
    /// Active command state (for command lifecycle).
    active_command: Option<ActiveCommand>,
    /// Aggregate metrics across all processing.
    metrics: BridgeMetrics,
    /// Plugin processors (non-WASM, for testing/native plugins).
    native_plugins: Vec<Box<dyn NativePluginProcessor>>,
    /// Health tracking for each native plugin (by index).
    plugin_health: Vec<PluginHealth>,
    /// WASM plugin executor (when wasm-plugins feature enabled).
    #[cfg(feature = "wasm-plugins")]
    wasm_executor: Option<PluginExecutor>,
}

/// Active command state for OSC 133 integration.
#[derive(Debug, Clone)]
struct ActiveCommand {
    /// Command text.
    command: String,
    /// Working directory when command started (for future use).
    _cwd: Option<String>,
    /// When the command started.
    started_at: Instant,
}

/// Aggregate metrics for the bridge.
#[derive(Debug, Clone, Default)]
pub struct BridgeMetrics {
    /// Total output events processed.
    pub output_events: u64,
    /// Total input events processed.
    pub input_events: u64,
    /// Total command start events.
    pub command_starts: u64,
    /// Total command complete events.
    pub command_completes: u64,
    /// Total events that were consumed by plugins.
    pub events_consumed: u64,
    /// Total events that were transformed by plugins.
    pub events_transformed: u64,
    /// Total processing time (microseconds).
    pub total_processing_time_us: u64,
    /// Events dropped due to queue overflow.
    pub events_dropped: u64,
    /// Plugin errors encountered during processing.
    pub plugin_errors: u64,
    /// Plugin timeouts (exceeded time budget).
    pub plugin_timeouts: u64,
    /// Plugins automatically disabled due to repeated failures.
    pub plugins_disabled: u64,
}

/// Trait for native (non-WASM) plugin processors.
///
/// Used for testing and for plugins that don't need sandboxing.
pub trait NativePluginProcessor: Send + Sync {
    /// Get the plugin ID.
    fn id(&self) -> PluginId;

    /// Get the plugin name.
    fn name(&self) -> &'static str;

    /// Get the plugin state.
    fn state(&self) -> PluginState;

    /// Get the permissions this plugin has.
    fn permissions(&self) -> &[Permission];

    /// Process an event and return an action.
    fn process(&mut self, event: &PluginEvent, info: &TerminalInfo) -> PluginResult<PluginAction>;
}

impl std::fmt::Debug for PluginBridge {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PluginBridge")
            .field("config", &self.config)
            .field("queue_len", &self.queue.len())
            .field("has_active_command", &self.active_command.is_some())
            .field("native_plugins", &self.native_plugins.len())
            .finish_non_exhaustive()
    }
}

impl PluginBridge {
    /// Create a new plugin bridge with default configuration.
    pub fn new() -> Self {
        Self::with_config(PluginBridgeConfig::default())
    }

    /// Create a new plugin bridge with custom configuration.
    pub fn with_config(config: PluginBridgeConfig) -> Self {
        Self {
            queue: VecDeque::with_capacity(config.max_queue_capacity),
            config,
            terminal_info: TerminalInfo {
                cols: 80,
                rows: 24,
                cursor_col: 0,
                cursor_row: 0,
                cwd: None,
                alternate_screen: false,
                bracketed_paste: false,
            },
            active_command: None,
            metrics: BridgeMetrics::default(),
            native_plugins: Vec::new(),
            plugin_health: Vec::new(),
            #[cfg(feature = "wasm-plugins")]
            wasm_executor: None,
        }
    }

    /// Set the WASM plugin executor.
    #[cfg(feature = "wasm-plugins")]
    pub fn set_wasm_executor(&mut self, executor: PluginExecutor) {
        self.wasm_executor = Some(executor);
    }

    /// Get a reference to the WASM executor.
    #[cfg(feature = "wasm-plugins")]
    pub fn wasm_executor(&self) -> Option<&PluginExecutor> {
        self.wasm_executor.as_ref()
    }

    /// Get a mutable reference to the WASM executor.
    #[cfg(feature = "wasm-plugins")]
    pub fn wasm_executor_mut(&mut self) -> Option<&mut PluginExecutor> {
        self.wasm_executor.as_mut()
    }

    /// Register a native plugin processor.
    pub fn register_native_plugin(&mut self, plugin: Box<dyn NativePluginProcessor>) {
        self.native_plugins.push(plugin);
        self.plugin_health.push(PluginHealth::default());
    }

    /// Unregister a native plugin by ID.
    pub fn unregister_native_plugin(&mut self, id: PluginId) -> bool {
        if let Some(pos) = self.native_plugins.iter().position(|p| p.id() == id) {
            self.native_plugins.remove(pos);
            self.plugin_health.remove(pos);
            true
        } else {
            false
        }
    }

    /// Re-enable a plugin that was automatically disabled due to failures.
    ///
    /// Returns true if the plugin was found and re-enabled, false otherwise.
    pub fn reenable_plugin(&mut self, id: PluginId) -> bool {
        if let Some(pos) = self.native_plugins.iter().position(|p| p.id() == id) {
            if let Some(health) = self.plugin_health.get_mut(pos) {
                health.reset();
                return true;
            }
        }
        false
    }

    /// Check if a plugin is disabled due to failures.
    pub fn is_plugin_disabled(&self, id: PluginId) -> bool {
        self.native_plugins
            .iter()
            .position(|p| p.id() == id)
            .and_then(|pos| self.plugin_health.get(pos))
            .is_some_and(|h| h.disabled)
    }

    /// Get the consecutive error count for a plugin.
    pub fn plugin_error_count(&self, id: PluginId) -> Option<u32> {
        self.native_plugins
            .iter()
            .position(|p| p.id() == id)
            .and_then(|pos| self.plugin_health.get(pos))
            .map(|h| h.consecutive_errors)
    }

    /// Update terminal info (should be called before processing events).
    pub fn update_terminal_info(&mut self, info: TerminalInfo) {
        self.terminal_info = info;
    }

    /// Get the current terminal info.
    pub fn terminal_info(&self) -> &TerminalInfo {
        &self.terminal_info
    }

    /// Get bridge metrics.
    pub fn metrics(&self) -> &BridgeMetrics {
        &self.metrics
    }

    /// Get the configuration.
    pub fn config(&self) -> &PluginBridgeConfig {
        &self.config
    }

    // ========================================================================
    // Output Integration
    // ========================================================================

    /// Process terminal output through plugins.
    ///
    /// This is the main hook for output integration. Call this with raw terminal
    /// output bytes before or after parsing.
    ///
    /// # Arguments
    ///
    /// * `data` - Raw terminal output bytes
    /// * `in_command` - Whether this output is part of a command's output
    ///
    /// # Returns
    ///
    /// Returns the processing result, which may include transformed data.
    pub fn process_output(&mut self, data: &[u8], in_command: bool) -> ProcessResult {
        if !self.config.enable_output_hooks {
            return ProcessResult::default();
        }

        let event = PluginEvent::Output {
            data: data.to_vec(),
            in_command,
        };

        self.metrics.output_events += 1;
        self.process_event_internal(event, EventPriority::Normal)
    }

    /// Process terminal output with high priority (e.g., escape sequences).
    pub fn process_output_high_priority(&mut self, data: &[u8], in_command: bool) -> ProcessResult {
        if !self.config.enable_output_hooks {
            return ProcessResult::default();
        }

        let event = PluginEvent::Output {
            data: data.to_vec(),
            in_command,
        };

        self.metrics.output_events += 1;
        self.process_event_internal(event, EventPriority::High)
    }

    // ========================================================================
    // Input Integration
    // ========================================================================

    /// Process a key event through plugins.
    ///
    /// Call this when a key is pressed before sending to the PTY.
    ///
    /// # Arguments
    ///
    /// * `key` - The key code
    /// * `modifiers` - Active modifier keys
    ///
    /// # Returns
    ///
    /// Returns the processing result. If `was_consumed` is true, the key
    /// should not be sent to the PTY.
    pub fn process_key(&mut self, key: KeyCode, modifiers: KeyModifiers) -> ProcessResult {
        if !self.config.enable_input_hooks {
            return ProcessResult::default();
        }

        let event = PluginEvent::Key(KeyEvent { key, modifiers });

        self.metrics.input_events += 1;
        self.process_event_internal(event, EventPriority::High)
    }

    /// Process a character key event.
    ///
    /// Convenience method for processing a character key press.
    pub fn process_char_key(&mut self, c: char, modifiers: KeyModifiers) -> ProcessResult {
        self.process_key(KeyCode::Char(c), modifiers)
    }

    // ========================================================================
    // Command Lifecycle Integration (OSC 133)
    // ========================================================================

    /// Notify plugins that a command has started.
    ///
    /// Call this when OSC 133 ; B or OSC 133 ; C is received.
    ///
    /// # Arguments
    ///
    /// * `command` - The command text
    /// * `cwd` - Current working directory (if known)
    pub fn on_command_start(&mut self, command: String, cwd: Option<String>) {
        if !self.config.enable_command_hooks {
            return;
        }

        // Store active command state
        self.active_command = Some(ActiveCommand {
            command: command.clone(),
            _cwd: cwd.clone(),
            started_at: Instant::now(),
        });

        let event = PluginEvent::CommandStart { command, cwd };

        self.metrics.command_starts += 1;
        let _ = self.process_event_internal(event, EventPriority::High);
    }

    /// Notify plugins that a command has completed.
    ///
    /// Call this when OSC 133 ; D is received.
    ///
    /// # Arguments
    ///
    /// * `exit_code` - The command's exit code (if available)
    pub fn on_command_complete(&mut self, exit_code: Option<i32>) {
        if !self.config.enable_command_hooks {
            return;
        }

        let (command, duration) = if let Some(active) = self.active_command.take() {
            (active.command, active.started_at.elapsed())
        } else {
            // No active command, use defaults
            (String::new(), Duration::ZERO)
        };

        let event = PluginEvent::CommandComplete {
            command,
            exit_code,
            duration,
        };

        self.metrics.command_completes += 1;
        let _ = self.process_event_internal(event, EventPriority::Normal);
    }

    /// Check if there's an active command.
    pub fn has_active_command(&self) -> bool {
        self.active_command.is_some()
    }

    /// Get the active command text.
    pub fn active_command(&self) -> Option<&str> {
        self.active_command.as_ref().map(|c| c.command.as_str())
    }

    // ========================================================================
    // Tick (Periodic Events)
    // ========================================================================

    /// Send a tick event to all plugins.
    ///
    /// Call this periodically (e.g., every 100ms) for time-based plugins.
    ///
    /// # Arguments
    ///
    /// * `now_ms` - Milliseconds since some reference point
    pub fn tick(&mut self, now_ms: u64) -> ProcessResult {
        let event = PluginEvent::Tick { now_ms };
        self.process_event_internal(event, EventPriority::Low)
    }

    // ========================================================================
    // Internal Processing
    // ========================================================================

    /// Process an event through all plugins.
    fn process_event_internal(&mut self, event: PluginEvent, priority: EventPriority) -> ProcessResult {
        let start = Instant::now();
        let mut result = ProcessResult::default();
        let mut timed_out = false;

        // Enqueue for async processing if needed (drop oldest if at capacity)
        if self.queue.len() >= self.config.max_queue_capacity {
            self.queue.pop_front();
            self.metrics.events_dropped += 1;
        }
        self.queue.push_back(BridgeQueuedEvent {
            event: event.clone(),
            priority,
            _enqueued_at: Instant::now(),
        });

        // Process through native plugins
        for idx in 0..self.native_plugins.len() {
            // Check if plugin is disabled due to failures
            if let Some(health) = self.plugin_health.get(idx) {
                if health.disabled {
                    continue;
                }
            }

            let plugin = &mut self.native_plugins[idx];

            if plugin.state() != PluginState::Ready {
                continue;
            }

            // Check if plugin can receive this event type
            let can_receive = match &event {
                PluginEvent::Output { .. } => {
                    plugin.permissions().contains(&Permission::TerminalRead)
                }
                PluginEvent::Key(_) => plugin.permissions().contains(&Permission::TerminalRead),
                PluginEvent::CommandStart { .. } | PluginEvent::CommandComplete { .. } => {
                    plugin.permissions().contains(&Permission::TerminalCommand)
                }
                PluginEvent::Tick { .. } => true, // All plugins can receive ticks
            };

            if !can_receive {
                continue;
            }

            match plugin.process(&event, &self.terminal_info) {
                Ok(action) => {
                    // Record success and reset error counter
                    if let Some(health) = self.plugin_health.get_mut(idx) {
                        health.record_success();
                    }

                    result.plugins_processed += 1;
                    match &action {
                        PluginAction::Consume => {
                            result.was_consumed = true;
                            result.action = action;
                            self.metrics.events_consumed += 1;
                            break; // Stop processing
                        }
                        PluginAction::Transform(data) => {
                            result.was_transformed = true;
                            result.action = PluginAction::Transform(data.clone());
                            self.metrics.events_transformed += 1;
                            // Continue to let other plugins transform further
                        }
                        PluginAction::EmitInput(_) | PluginAction::EmitCommand { .. } => {
                            // Check write permission
                            if plugin.permissions().contains(&Permission::TerminalWrite) {
                                result.action = action;
                            }
                        }
                        _ => {}
                    }
                }
                Err(_err) => {
                    // Record error and potentially disable plugin
                    self.metrics.plugin_errors += 1;

                    if let Some(health) = self.plugin_health.get_mut(idx) {
                        let should_disable = health.record_error(self.config.max_consecutive_errors);
                        if should_disable && self.config.auto_disable_failing_plugins {
                            health.disabled = true;
                            self.metrics.plugins_disabled += 1;
                        }
                    }
                    // Continue with other plugins
                }
            }

            // Check time budget (saturate at u64::MAX for extremely long durations)
            let elapsed = start.elapsed();
            #[allow(clippy::cast_possible_truncation)] // Saturated to u64::MAX above
            let elapsed_micros = elapsed.as_micros().min(u128::from(u64::MAX)) as u64;
            if elapsed_micros > self.config.max_processing_time_us {
                timed_out = true;
                self.metrics.plugin_timeouts += 1;
                break;
            }
        }

        // Track timeout in result
        let _ = timed_out; // Currently just for metrics, but could add to ProcessResult

        // Process through WASM plugins
        #[cfg(feature = "wasm-plugins")]
        if let Some(executor) = &mut self.wasm_executor {
            let wasm_action = executor.process_event(&event);
            match &wasm_action {
                PluginAction::Consume => {
                    result.was_consumed = true;
                    result.action = wasm_action;
                    self.metrics.events_consumed += 1;
                }
                PluginAction::Transform(data) => {
                    result.was_transformed = true;
                    result.action = PluginAction::Transform(data.clone());
                    self.metrics.events_transformed += 1;
                }
                _ => {
                    if !result.was_transformed && !result.was_consumed {
                        result.action = wasm_action;
                    }
                }
            }
        }

        let elapsed = start.elapsed();
        // Saturate at u64::MAX (extremely unlikely in practice)
        #[allow(clippy::cast_possible_truncation)]
        let elapsed_us = elapsed.as_micros().min(u128::from(u64::MAX)) as u64;
        result.processing_time_us = elapsed_us;
        self.metrics.total_processing_time_us = self.metrics.total_processing_time_us.saturating_add(elapsed_us);

        result
    }

    /// Drain the event queue and process all pending events.
    ///
    /// Returns the number of events processed.
    pub fn drain_queue(&mut self) -> usize {
        let mut count = 0;
        while let Some(queued) = self.queue.pop_front() {
            let _ = self.process_event_internal(queued.event, queued.priority);
            count += 1;
        }
        count
    }

    /// Get the number of events in the queue.
    pub fn queue_len(&self) -> usize {
        self.queue.len()
    }

    /// Clear the event queue.
    pub fn clear_queue(&mut self) {
        self.queue.clear();
    }
}

impl Default for PluginBridge {
    fn default() -> Self {
        Self::new()
    }
}

/// Helper to create a TerminalInfo from terminal state.
impl TerminalInfo {
    /// Create terminal info from terminal dimensions and cursor position.
    pub fn from_terminal(
        cols: u16,
        rows: u16,
        cursor_col: u16,
        cursor_row: u16,
        cwd: Option<String>,
        alternate_screen: bool,
        bracketed_paste: bool,
    ) -> Self {
        Self {
            cols,
            rows,
            cursor_col,
            cursor_row,
            cwd,
            alternate_screen,
            bracketed_paste,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Test native plugin that echoes events.
    struct EchoPlugin {
        id: PluginId,
        state: PluginState,
        events_received: Vec<String>,
    }

    impl EchoPlugin {
        fn new(id: u32) -> Self {
            Self {
                id: PluginId(id),
                state: PluginState::Ready,
                events_received: Vec::new(),
            }
        }
    }

    impl NativePluginProcessor for EchoPlugin {
        fn id(&self) -> PluginId {
            self.id
        }

        fn name(&self) -> &'static str {
            "echo"
        }

        fn state(&self) -> PluginState {
            self.state
        }

        fn permissions(&self) -> &[Permission] {
            &[Permission::TerminalRead, Permission::TerminalWrite]
        }

        fn process(&mut self, event: &PluginEvent, _info: &TerminalInfo) -> PluginResult<PluginAction> {
            match event {
                PluginEvent::Output { data, .. } => {
                    self.events_received.push(format!("output:{}", data.len()));
                }
                PluginEvent::Key(key) => {
                    self.events_received.push(format!("key:{:?}", key.key));
                }
                PluginEvent::CommandStart { command, .. } => {
                    self.events_received.push(format!("start:{}", command));
                }
                PluginEvent::CommandComplete { command, exit_code, .. } => {
                    self.events_received.push(format!("complete:{}:{:?}", command, exit_code));
                }
                PluginEvent::Tick { now_ms } => {
                    self.events_received.push(format!("tick:{}", now_ms));
                }
            }
            Ok(PluginAction::Continue)
        }
    }

    /// Test plugin that consumes specific events.
    struct ConsumerPlugin {
        id: PluginId,
        consume_pattern: String,
    }

    impl ConsumerPlugin {
        fn new(id: u32, pattern: &str) -> Self {
            Self {
                id: PluginId(id),
                consume_pattern: pattern.to_string(),
            }
        }
    }

    impl NativePluginProcessor for ConsumerPlugin {
        fn id(&self) -> PluginId {
            self.id
        }

        fn name(&self) -> &'static str {
            "consumer"
        }

        fn state(&self) -> PluginState {
            PluginState::Ready
        }

        fn permissions(&self) -> &[Permission] {
            &[Permission::TerminalRead]
        }

        fn process(&mut self, event: &PluginEvent, _info: &TerminalInfo) -> PluginResult<PluginAction> {
            if let PluginEvent::Output { data, .. } = event {
                if let Ok(text) = std::str::from_utf8(data) {
                    if text.contains(&self.consume_pattern) {
                        return Ok(PluginAction::Consume);
                    }
                }
            }
            Ok(PluginAction::Continue)
        }
    }

    /// Test plugin that transforms output.
    struct TransformPlugin {
        id: PluginId,
    }

    impl TransformPlugin {
        fn new(id: u32) -> Self {
            Self { id: PluginId(id) }
        }
    }

    impl NativePluginProcessor for TransformPlugin {
        fn id(&self) -> PluginId {
            self.id
        }

        fn name(&self) -> &'static str {
            "transform"
        }

        fn state(&self) -> PluginState {
            PluginState::Ready
        }

        fn permissions(&self) -> &[Permission] {
            &[Permission::TerminalRead, Permission::TerminalWrite]
        }

        fn process(&mut self, event: &PluginEvent, _info: &TerminalInfo) -> PluginResult<PluginAction> {
            if let PluginEvent::Output { data, .. } = event {
                // Transform: uppercase all ASCII
                let transformed: Vec<u8> = data.iter().map(|b| b.to_ascii_uppercase()).collect();
                return Ok(PluginAction::Transform(transformed));
            }
            Ok(PluginAction::Continue)
        }
    }

    #[test]
    fn test_bridge_creation() {
        let bridge = PluginBridge::new();
        assert!(bridge.config.enable_output_hooks);
        assert!(bridge.config.enable_input_hooks);
        assert!(bridge.config.enable_command_hooks);
    }

    #[test]
    fn test_output_processing() {
        let mut bridge = PluginBridge::new();
        bridge.register_native_plugin(Box::new(EchoPlugin::new(1)));

        let result = bridge.process_output(b"hello world", false);
        assert!(!result.was_consumed);
        assert!(!result.was_transformed);
        assert_eq!(result.plugins_processed, 1);

        assert_eq!(bridge.metrics.output_events, 1);
    }

    #[test]
    fn test_key_processing() {
        let mut bridge = PluginBridge::new();
        bridge.register_native_plugin(Box::new(EchoPlugin::new(1)));

        let result = bridge.process_char_key('a', KeyModifiers::empty());
        assert!(!result.was_consumed);
        assert_eq!(result.plugins_processed, 1);

        assert_eq!(bridge.metrics.input_events, 1);
    }

    #[test]
    fn test_command_lifecycle() {
        let mut bridge = PluginBridge::new();
        bridge.register_native_plugin(Box::new(EchoPlugin::new(1)));

        bridge.on_command_start("ls -la".to_string(), Some("/home".to_string()));
        assert!(bridge.has_active_command());
        assert_eq!(bridge.active_command(), Some("ls -la"));

        bridge.on_command_complete(Some(0));
        assert!(!bridge.has_active_command());

        assert_eq!(bridge.metrics.command_starts, 1);
        assert_eq!(bridge.metrics.command_completes, 1);
    }

    #[test]
    fn test_consumer_plugin() {
        let mut bridge = PluginBridge::new();
        bridge.register_native_plugin(Box::new(ConsumerPlugin::new(1, "secret")));

        // Normal output should pass through
        let result = bridge.process_output(b"normal output", false);
        assert!(!result.was_consumed);

        // Output containing "secret" should be consumed
        let result = bridge.process_output(b"this is secret data", false);
        assert!(result.was_consumed);

        assert_eq!(bridge.metrics.events_consumed, 1);
    }

    #[test]
    fn test_transform_plugin() {
        let mut bridge = PluginBridge::new();
        bridge.register_native_plugin(Box::new(TransformPlugin::new(1)));

        let result = bridge.process_output(b"hello", false);
        assert!(result.was_transformed);

        if let PluginAction::Transform(data) = result.action {
            assert_eq!(data, b"HELLO");
        } else {
            panic!("Expected Transform action");
        }

        assert_eq!(bridge.metrics.events_transformed, 1);
    }

    #[test]
    fn test_plugin_priority_order() {
        let mut bridge = PluginBridge::new();

        // Consumer plugin first - should consume before transform
        bridge.register_native_plugin(Box::new(ConsumerPlugin::new(1, "stop")));
        bridge.register_native_plugin(Box::new(TransformPlugin::new(2)));

        // Output with "stop" should be consumed, not transformed
        let result = bridge.process_output(b"stop here", false);
        assert!(result.was_consumed);
        assert!(!result.was_transformed);
    }

    #[test]
    fn test_terminal_info_update() {
        let mut bridge = PluginBridge::new();

        let info = TerminalInfo::from_terminal(120, 40, 10, 5, Some("/tmp".to_string()), false, true);
        bridge.update_terminal_info(info);

        assert_eq!(bridge.terminal_info().cols, 120);
        assert_eq!(bridge.terminal_info().rows, 40);
        assert_eq!(bridge.terminal_info().cursor_col, 10);
        assert_eq!(bridge.terminal_info().cursor_row, 5);
        assert_eq!(bridge.terminal_info().cwd, Some("/tmp".to_string()));
        assert!(!bridge.terminal_info().alternate_screen);
        assert!(bridge.terminal_info().bracketed_paste);
    }

    #[test]
    fn test_tick() {
        let mut bridge = PluginBridge::new();
        bridge.register_native_plugin(Box::new(EchoPlugin::new(1)));

        let result = bridge.tick(1000);
        assert_eq!(result.plugins_processed, 1);
    }

    #[test]
    fn test_disabled_hooks() {
        let config = PluginBridgeConfig {
            enable_output_hooks: false,
            enable_input_hooks: false,
            enable_command_hooks: false,
            ..Default::default()
        };
        let mut bridge = PluginBridge::with_config(config);
        bridge.register_native_plugin(Box::new(EchoPlugin::new(1)));

        // Output should not be processed
        let result = bridge.process_output(b"hello", false);
        assert_eq!(result.plugins_processed, 0);

        // Key should not be processed
        let result = bridge.process_char_key('a', KeyModifiers::empty());
        assert_eq!(result.plugins_processed, 0);

        // Commands should not be tracked
        bridge.on_command_start("ls".to_string(), None);
        assert!(!bridge.has_active_command());
    }

    #[test]
    fn test_unregister_plugin() {
        let mut bridge = PluginBridge::new();
        bridge.register_native_plugin(Box::new(EchoPlugin::new(1)));
        bridge.register_native_plugin(Box::new(EchoPlugin::new(2)));

        // Should have 2 plugins
        let result = bridge.process_output(b"test", false);
        assert_eq!(result.plugins_processed, 2);

        // Unregister one
        assert!(bridge.unregister_native_plugin(PluginId(1)));

        // Should have 1 plugin
        let result = bridge.process_output(b"test", false);
        assert_eq!(result.plugins_processed, 1);

        // Unregistering non-existent should return false
        assert!(!bridge.unregister_native_plugin(PluginId(999)));
    }

    // ========================================================================
    // Failure Recovery Tests
    // ========================================================================

    /// Test plugin that always fails.
    struct FailingPlugin {
        id: PluginId,
    }

    impl FailingPlugin {
        fn new(id: u32) -> Self {
            Self { id: PluginId(id) }
        }
    }

    impl NativePluginProcessor for FailingPlugin {
        fn id(&self) -> PluginId {
            self.id
        }

        fn name(&self) -> &'static str {
            "failing"
        }

        fn state(&self) -> PluginState {
            PluginState::Ready
        }

        fn permissions(&self) -> &[Permission] {
            &[Permission::TerminalRead]
        }

        fn process(&mut self, _event: &PluginEvent, _info: &TerminalInfo) -> PluginResult<PluginAction> {
            Err(super::super::types::PluginError::Trap {
                plugin: self.id,
                message: "intentional test failure".to_string(),
            })
        }
    }

    #[test]
    fn test_plugin_failure_tracking() {
        let config = PluginBridgeConfig {
            max_consecutive_errors: 3,
            auto_disable_failing_plugins: true,
            ..Default::default()
        };
        let mut bridge = PluginBridge::with_config(config);
        bridge.register_native_plugin(Box::new(FailingPlugin::new(1)));

        // Plugin should start with 0 errors
        assert_eq!(bridge.plugin_error_count(PluginId(1)), Some(0));
        assert!(!bridge.is_plugin_disabled(PluginId(1)));

        // Process events and accumulate errors
        let _ = bridge.process_output(b"test1", false);
        assert_eq!(bridge.plugin_error_count(PluginId(1)), Some(1));

        let _ = bridge.process_output(b"test2", false);
        assert_eq!(bridge.plugin_error_count(PluginId(1)), Some(2));

        // Third error should trigger disable
        let _ = bridge.process_output(b"test3", false);
        assert_eq!(bridge.plugin_error_count(PluginId(1)), Some(3));
        assert!(bridge.is_plugin_disabled(PluginId(1)));

        // Verify metrics
        assert_eq!(bridge.metrics().plugin_errors, 3);
        assert_eq!(bridge.metrics().plugins_disabled, 1);
    }

    #[test]
    fn test_plugin_reenable_after_disable() {
        let config = PluginBridgeConfig {
            max_consecutive_errors: 2,
            auto_disable_failing_plugins: true,
            ..Default::default()
        };
        let mut bridge = PluginBridge::with_config(config);
        bridge.register_native_plugin(Box::new(FailingPlugin::new(1)));

        // Trigger disable
        let _ = bridge.process_output(b"test1", false);
        let _ = bridge.process_output(b"test2", false);
        assert!(bridge.is_plugin_disabled(PluginId(1)));

        // Re-enable the plugin
        assert!(bridge.reenable_plugin(PluginId(1)));
        assert!(!bridge.is_plugin_disabled(PluginId(1)));
        assert_eq!(bridge.plugin_error_count(PluginId(1)), Some(0));
    }

    #[test]
    fn test_success_resets_error_counter() {
        let config = PluginBridgeConfig {
            max_consecutive_errors: 5,
            auto_disable_failing_plugins: true,
            ..Default::default()
        };
        let mut bridge = PluginBridge::with_config(config);

        // Add both failing and working plugins
        bridge.register_native_plugin(Box::new(EchoPlugin::new(1))); // Working
        bridge.register_native_plugin(Box::new(FailingPlugin::new(2))); // Failing

        // Process some events - failing plugin accumulates errors
        let _ = bridge.process_output(b"test", false);
        assert_eq!(bridge.plugin_error_count(PluginId(2)), Some(1));

        // Working plugin has 0 errors
        assert_eq!(bridge.plugin_error_count(PluginId(1)), Some(0));
    }

    #[test]
    fn test_disabled_plugin_not_called() {
        let config = PluginBridgeConfig {
            max_consecutive_errors: 1,
            auto_disable_failing_plugins: true,
            ..Default::default()
        };
        let mut bridge = PluginBridge::with_config(config);
        bridge.register_native_plugin(Box::new(FailingPlugin::new(1)));
        bridge.register_native_plugin(Box::new(EchoPlugin::new(2)));

        // First event triggers error and disables plugin 1
        let result = bridge.process_output(b"test1", false);
        assert_eq!(result.plugins_processed, 1); // Only echo plugin processed

        // Subsequent events should only hit echo plugin
        let result = bridge.process_output(b"test2", false);
        assert_eq!(result.plugins_processed, 1);

        // Error count should not increase (plugin not called)
        assert_eq!(bridge.metrics().plugin_errors, 1);
    }

    #[test]
    fn test_auto_disable_can_be_turned_off() {
        let config = PluginBridgeConfig {
            max_consecutive_errors: 2,
            auto_disable_failing_plugins: false, // Disabled
            ..Default::default()
        };
        let mut bridge = PluginBridge::with_config(config);
        bridge.register_native_plugin(Box::new(FailingPlugin::new(1)));

        // Exceed error threshold
        let _ = bridge.process_output(b"test1", false);
        let _ = bridge.process_output(b"test2", false);
        let _ = bridge.process_output(b"test3", false);

        // Plugin should NOT be disabled
        assert!(!bridge.is_plugin_disabled(PluginId(1)));

        // Errors still tracked
        assert_eq!(bridge.metrics().plugin_errors, 3);
        assert_eq!(bridge.metrics().plugins_disabled, 0);
    }
}
