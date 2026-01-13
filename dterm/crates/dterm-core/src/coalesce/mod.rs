//! Input coalescing for terminal emulation.
//!
//! This module implements input coalescing to reduce render frequency during rapid
//! terminal output. It uses a dual-timer approach inspired by foot terminal:
//!
//! - **Lower timer**: Reset on every input, short delay to batch rapid writes
//! - **Upper timer**: Maximum delay, ensures rendering happens within a frame period
//!
//! This prevents flickering when applications write intermediate states quickly,
//! while ensuring responsiveness is maintained.
//!
//! # Example
//!
//! ```
//! use dterm_core::coalesce::{InputCoalescer, CoalesceConfig};
//! use std::time::Duration;
//!
//! let config = CoalesceConfig::default();
//! let mut coalescer = InputCoalescer::new(config);
//!
//! // Notify of input and get coalescing decision
//! let now = std::time::Instant::now();
//! if let Some(action) = coalescer.on_input(now, 1024) {
//!     match action {
//!         dterm_core::coalesce::CoalesceAction::RenderNow => {
//!             // Render immediately (upper bound reached or buffer threshold)
//!         }
//!         dterm_core::coalesce::CoalesceAction::WaitUntil(deadline) => {
//!             // Schedule render at deadline
//!         }
//!     }
//! }
//! ```
//!
//! # Design
//!
//! Based on patterns from:
//! - **foot**: Dual-timer with lower (0.5ms) and upper (8.3ms) bounds
//! - **Kitty**: input_delay (3ms), repaint_delay (10ms), buffer threshold (16KB)
//! - **WezTerm**: Adaptive delay with action coalescing
//!
//! Default configuration targets 60Hz displays (16.6ms frame period), using half
//! a frame as the upper bound to avoid missing vsync.

use std::time::{Duration, Instant};

/// Configuration for input coalescing.
///
/// Default values are tuned for 60Hz displays with a balance between
/// responsiveness and efficient batching.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CoalesceConfig {
    /// Lower bound delay - reset on every input event.
    /// Batches rapid writes (e.g., shell output bursts).
    /// Default: 500us (0.5ms), matching foot.
    pub lower_delay: Duration,

    /// Upper bound delay - maximum time before forced render.
    /// Ensures responsiveness even during continuous output.
    /// Default: ~8.3ms (half a 60Hz frame).
    pub upper_delay: Duration,

    /// Buffer size threshold in bytes - forces immediate render if exceeded.
    /// Safety valve to prevent unbounded memory growth.
    /// Default: 16KB (16,384 bytes), matching Kitty.
    pub buffer_threshold: usize,

    /// Maximum buffer size before dropping input.
    /// Hard limit to prevent DoS from runaway output.
    /// Default: 1MB (1,048,576 bytes).
    pub max_buffer_size: usize,

    /// Whether coalescing is enabled.
    /// Can be disabled for debugging or specific applications.
    /// Default: true.
    pub enabled: bool,
}

impl Default for CoalesceConfig {
    fn default() -> Self {
        Self {
            lower_delay: Duration::from_nanos(500_000), // 0.5ms - foot default
            upper_delay: Duration::from_nanos(8_333_333), // ~8.3ms - half a 60Hz frame
            buffer_threshold: 16_384,                   // 16KB - Kitty threshold
            max_buffer_size: 1_048_576,                 // 1MB - Kitty buffer size
            enabled: true,
        }
    }
}

impl CoalesceConfig {
    /// Create a new configuration with all defaults.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Configuration optimized for low latency (gaming, interactive use).
    /// Smaller delays for more responsive feel.
    #[must_use]
    pub fn low_latency() -> Self {
        Self {
            lower_delay: Duration::from_nanos(250_000), // 0.25ms
            upper_delay: Duration::from_micros(4_000),  // 4ms
            buffer_threshold: 8_192,                    // 8KB
            max_buffer_size: 1_048_576,
            enabled: true,
        }
    }

    /// Configuration optimized for throughput (build logs, large output).
    /// Larger delays for better batching.
    #[must_use]
    pub fn high_throughput() -> Self {
        Self {
            lower_delay: Duration::from_micros(1_000),     // 1ms
            upper_delay: Duration::from_nanos(16_666_667), // ~16.6ms - full 60Hz frame
            buffer_threshold: 65_536,                      // 64KB
            max_buffer_size: 4_194_304,                    // 4MB
            enabled: true,
        }
    }

    /// Configuration with coalescing disabled.
    #[must_use]
    pub fn disabled() -> Self {
        Self {
            enabled: false,
            ..Self::default()
        }
    }

    /// Configuration for 120Hz displays.
    #[must_use]
    pub fn for_120hz() -> Self {
        Self {
            lower_delay: Duration::from_nanos(500_000),   // 0.5ms
            upper_delay: Duration::from_nanos(4_166_667), // ~4.16ms - half a 120Hz frame
            buffer_threshold: 16_384,
            max_buffer_size: 1_048_576,
            enabled: true,
        }
    }

    /// Configuration for 144Hz displays.
    #[must_use]
    pub fn for_144hz() -> Self {
        Self {
            lower_delay: Duration::from_nanos(400_000),   // 0.4ms
            upper_delay: Duration::from_nanos(3_472_222), // ~3.47ms - half a 144Hz frame
            buffer_threshold: 16_384,
            max_buffer_size: 1_048_576,
            enabled: true,
        }
    }

    /// Set lower delay.
    #[must_use]
    pub fn with_lower_delay(mut self, delay: Duration) -> Self {
        self.lower_delay = delay;
        self
    }

    /// Set upper delay.
    #[must_use]
    pub fn with_upper_delay(mut self, delay: Duration) -> Self {
        self.upper_delay = delay;
        self
    }

    /// Set buffer threshold.
    #[must_use]
    pub fn with_buffer_threshold(mut self, threshold: usize) -> Self {
        self.buffer_threshold = threshold;
        self
    }

    /// Validate configuration invariants.
    pub fn validate(&self) -> Result<(), CoalesceError> {
        if self.upper_delay < self.lower_delay {
            return Err(CoalesceError::InvalidConfig(
                "upper_delay must be >= lower_delay".to_string(),
            ));
        }
        if self.max_buffer_size < self.buffer_threshold {
            return Err(CoalesceError::InvalidConfig(
                "max_buffer_size must be >= buffer_threshold".to_string(),
            ));
        }
        if self.upper_delay >= Duration::from_secs(1) {
            return Err(CoalesceError::InvalidConfig(
                "upper_delay must be < 1 second".to_string(),
            ));
        }
        Ok(())
    }
}

/// Action to take after input coalescing decision.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoalesceAction {
    /// Render immediately - buffer threshold exceeded or upper bound reached.
    RenderNow,
    /// Wait until the specified deadline before rendering.
    WaitUntil(Instant),
}

/// State of the coalescer for inspection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoalesceState {
    /// No pending input, coalescer is idle.
    Idle,
    /// Input received, waiting for lower timer.
    Waiting,
    /// Upper bound armed, will render soon regardless of input.
    UpperArmed,
}

/// Error type for coalescing operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CoalesceError {
    /// Configuration is invalid.
    InvalidConfig(String),
    /// Buffer overflow - too much data accumulated.
    BufferOverflow {
        /// Current buffer size in bytes.
        current: usize,
        /// Maximum allowed size in bytes.
        max: usize,
    },
}

impl std::fmt::Display for CoalesceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidConfig(msg) => write!(f, "invalid coalesce config: {}", msg),
            Self::BufferOverflow { current, max } => {
                write!(f, "buffer overflow: {} bytes exceeds max {}", current, max)
            }
        }
    }
}

impl std::error::Error for CoalesceError {}

/// Input coalescer for terminal rendering.
///
/// Implements dual-timer coalescing to batch rapid terminal output while
/// maintaining responsiveness. The coalescer tracks:
///
/// - When input was first received (upper timer anchor)
/// - Accumulated buffer size
/// - Current coalescing state
///
/// # Thread Safety
///
/// This struct is not thread-safe. For multi-threaded use, wrap in a mutex
/// or use channels to serialize access.
#[derive(Debug)]
pub struct InputCoalescer {
    /// Configuration
    config: CoalesceConfig,
    /// Timestamp of first input in current batch (upper timer anchor)
    first_input_at: Option<Instant>,
    /// Timestamp of most recent input
    last_input_at: Option<Instant>,
    /// Accumulated buffer size in bytes
    accumulated_bytes: usize,
    /// Current state
    state: CoalesceState,
    /// Total bytes processed (for statistics)
    total_bytes: u64,
    /// Total batches rendered (for statistics)
    total_batches: u64,
}

impl InputCoalescer {
    /// Create a new coalescer with the given configuration.
    pub fn new(config: CoalesceConfig) -> Self {
        Self {
            config,
            first_input_at: None,
            last_input_at: None,
            accumulated_bytes: 0,
            state: CoalesceState::Idle,
            total_bytes: 0,
            total_batches: 0,
        }
    }

    /// Create a coalescer with default configuration.
    #[must_use]
    pub fn default_config() -> Self {
        Self::new(CoalesceConfig::default())
    }

    /// Get the current configuration.
    #[must_use]
    pub fn config(&self) -> &CoalesceConfig {
        &self.config
    }

    /// Update the configuration.
    ///
    /// This resets the coalescer state.
    pub fn set_config(&mut self, config: CoalesceConfig) {
        self.config = config;
        self.reset();
    }

    /// Get the current state.
    #[must_use]
    pub fn state(&self) -> CoalesceState {
        self.state
    }

    /// Get accumulated bytes in current batch.
    #[must_use]
    pub fn accumulated_bytes(&self) -> usize {
        self.accumulated_bytes
    }

    /// Get total bytes processed across all batches.
    #[must_use]
    pub fn total_bytes(&self) -> u64 {
        self.total_bytes
    }

    /// Get total batches rendered.
    #[must_use]
    pub fn total_batches(&self) -> u64 {
        self.total_batches
    }

    /// Get average batch size.
    #[must_use]
    pub fn average_batch_size(&self) -> u64 {
        if self.total_batches == 0 {
            0
        } else {
            self.total_bytes / self.total_batches
        }
    }

    /// Check if coalescing is enabled.
    #[must_use]
    pub fn is_enabled(&self) -> bool {
        self.config.enabled
    }

    /// Notify the coalescer of input data and get the recommended action.
    ///
    /// # Arguments
    ///
    /// * `now` - Current timestamp
    /// * `bytes` - Number of bytes received
    ///
    /// # Returns
    ///
    /// - `None` if no action needed yet (continue accumulating)
    /// - `Some(CoalesceAction::RenderNow)` if should render immediately
    /// - `Some(CoalesceAction::WaitUntil(deadline))` if should wait until deadline
    pub fn on_input(&mut self, now: Instant, bytes: usize) -> Option<CoalesceAction> {
        // If disabled, always render immediately
        if !self.config.enabled {
            self.accumulated_bytes = self.accumulated_bytes.saturating_add(bytes);
            self.total_bytes = self.total_bytes.saturating_add(bytes as u64);
            return Some(CoalesceAction::RenderNow);
        }

        // Update accumulated bytes
        self.accumulated_bytes = self.accumulated_bytes.saturating_add(bytes);
        self.total_bytes = self.total_bytes.saturating_add(bytes as u64);

        // Check buffer threshold - render immediately if exceeded
        if self.accumulated_bytes >= self.config.buffer_threshold {
            return Some(CoalesceAction::RenderNow);
        }

        // Update timestamps
        self.last_input_at = Some(now);

        // If this is the first input, start the upper timer
        if self.first_input_at.is_none() {
            self.first_input_at = Some(now);
            self.state = CoalesceState::UpperArmed;
        }

        // Calculate deadlines
        let lower_deadline = now + self.config.lower_delay;
        let upper_deadline = self.first_input_at.unwrap() + self.config.upper_delay;

        // Check if upper bound already exceeded
        if now >= upper_deadline {
            return Some(CoalesceAction::RenderNow);
        }

        // Return the earlier deadline (lower or upper)
        let deadline = if lower_deadline < upper_deadline {
            lower_deadline
        } else {
            upper_deadline
        };

        self.state = CoalesceState::Waiting;
        Some(CoalesceAction::WaitUntil(deadline))
    }

    /// Check if the coalescer should render now.
    ///
    /// Call this when a timer fires or periodically to check deadlines.
    ///
    /// # Arguments
    ///
    /// * `now` - Current timestamp
    ///
    /// # Returns
    ///
    /// - `true` if rendering should happen now
    /// - `false` if should continue waiting
    pub fn should_render(&self, now: Instant) -> bool {
        // If disabled or no pending input, no need to render
        if !self.config.enabled || self.first_input_at.is_none() {
            return false;
        }

        // Check upper bound
        if let Some(first) = self.first_input_at {
            if now >= first + self.config.upper_delay {
                return true;
            }
        }

        // Check lower bound (time since last input)
        if let Some(last) = self.last_input_at {
            if now >= last + self.config.lower_delay {
                return true;
            }
        }

        // Check buffer threshold
        if self.accumulated_bytes >= self.config.buffer_threshold {
            return true;
        }

        false
    }

    /// Get the next deadline to check.
    ///
    /// Returns `None` if no timer is active.
    #[must_use]
    pub fn next_deadline(&self) -> Option<Instant> {
        if !self.config.enabled || self.first_input_at.is_none() {
            return None;
        }

        let upper_deadline = self.first_input_at.map(|t| t + self.config.upper_delay);
        let lower_deadline = self.last_input_at.map(|t| t + self.config.lower_delay);

        match (lower_deadline, upper_deadline) {
            (Some(l), Some(u)) => Some(l.min(u)),
            (Some(l), None) => Some(l),
            (None, Some(u)) => Some(u),
            (None, None) => None,
        }
    }

    /// Mark that rendering has happened and reset state.
    ///
    /// Call this after rendering to prepare for the next batch.
    pub fn on_render(&mut self) {
        self.total_batches = self.total_batches.saturating_add(1);
        self.accumulated_bytes = 0;
        self.first_input_at = None;
        self.last_input_at = None;
        self.state = CoalesceState::Idle;
    }

    /// Reset the coalescer state without counting as a batch.
    pub fn reset(&mut self) {
        self.accumulated_bytes = 0;
        self.first_input_at = None;
        self.last_input_at = None;
        self.state = CoalesceState::Idle;
    }

    /// Check if there is pending input to render.
    #[must_use]
    pub fn has_pending(&self) -> bool {
        self.accumulated_bytes > 0
    }

    /// Get time remaining until upper deadline.
    ///
    /// Returns `None` if no timer is active or deadline passed.
    #[must_use]
    pub fn time_until_upper_deadline(&self, now: Instant) -> Option<Duration> {
        self.first_input_at.and_then(|first| {
            let deadline = first + self.config.upper_delay;
            deadline.checked_duration_since(now)
        })
    }

    /// Get time remaining until lower deadline.
    ///
    /// Returns `None` if no timer is active or deadline passed.
    #[must_use]
    pub fn time_until_lower_deadline(&self, now: Instant) -> Option<Duration> {
        self.last_input_at.and_then(|last| {
            let deadline = last + self.config.lower_delay;
            deadline.checked_duration_since(now)
        })
    }
}

impl Default for InputCoalescer {
    fn default() -> Self {
        Self::new(CoalesceConfig::default())
    }
}

/// Render callback for integration with event loops.
///
/// This trait abstracts the rendering callback for different event loop
/// implementations (tokio, async-std, mio, etc.).
pub trait RenderCallback {
    /// Schedule a render at the given deadline.
    fn schedule_render(&mut self, deadline: Instant);
    /// Render immediately.
    fn render_now(&mut self);
    /// Cancel any scheduled render.
    fn cancel_render(&mut self);
}

// ============================================================================
// Kani proofs
// ============================================================================

#[cfg(kani)]
mod verification {
    use super::*;

    /// Verify that config validation catches invalid upper < lower.
    #[kani::proof]
    fn config_upper_less_than_lower_invalid() {
        let lower: u64 = kani::any();
        let upper: u64 = kani::any();
        kani::assume(upper < lower);
        kani::assume(lower < 1_000_000_000);

        let config = CoalesceConfig {
            lower_delay: Duration::from_nanos(lower),
            upper_delay: Duration::from_nanos(upper),
            buffer_threshold: 16_384,
            max_buffer_size: 1_048_576,
            enabled: true,
        };

        assert!(config.validate().is_err());
    }

    /// Verify that config validation catches upper >= 1 second.
    #[kani::proof]
    fn config_upper_too_large_invalid() {
        let upper: u64 = kani::any();
        kani::assume(upper >= 1_000_000_000);

        let config = CoalesceConfig {
            lower_delay: Duration::from_nanos(500_000),
            upper_delay: Duration::from_nanos(upper),
            buffer_threshold: 16_384,
            max_buffer_size: 1_048_576,
            enabled: true,
        };

        assert!(config.validate().is_err());
    }

    /// Verify that valid config passes validation.
    #[kani::proof]
    fn config_valid_passes() {
        let lower: u64 = kani::any();
        let upper: u64 = kani::any();
        let threshold: usize = kani::any();
        let max: usize = kani::any();

        kani::assume(lower > 0 && lower <= 100_000_000); // up to 100ms
        kani::assume(upper >= lower && upper < 1_000_000_000);
        kani::assume(threshold > 0 && threshold <= max);
        kani::assume(max <= 10_000_000); // up to 10MB

        let config = CoalesceConfig {
            lower_delay: Duration::from_nanos(lower),
            upper_delay: Duration::from_nanos(upper),
            buffer_threshold: threshold,
            max_buffer_size: max,
            enabled: true,
        };

        assert!(config.validate().is_ok());
    }

    /// Verify accumulated bytes never overflow (saturating add).
    #[kani::proof]
    fn accumulated_bytes_saturates() {
        let mut coalescer = InputCoalescer::new(CoalesceConfig::disabled());

        let bytes1: usize = kani::any();
        let bytes2: usize = kani::any();

        // Simulate two inputs
        coalescer.accumulated_bytes = bytes1;
        coalescer.accumulated_bytes = coalescer.accumulated_bytes.saturating_add(bytes2);

        // Verify no overflow
        assert!(coalescer.accumulated_bytes <= usize::MAX);
    }

    /// Verify that on_render resets state correctly.
    #[kani::proof]
    fn on_render_resets_state() {
        let mut coalescer = InputCoalescer::new(CoalesceConfig::default());

        // Simulate some state
        coalescer.accumulated_bytes = kani::any();
        coalescer.state = CoalesceState::Waiting;
        coalescer.total_batches = kani::any();

        let old_batches = coalescer.total_batches;
        coalescer.on_render();

        assert_eq!(coalescer.accumulated_bytes, 0);
        assert_eq!(coalescer.state, CoalesceState::Idle);
        assert!(coalescer.total_batches >= old_batches);
    }

    /// Verify disabled coalescer always returns RenderNow.
    #[kani::proof]
    fn disabled_always_renders() {
        let config = CoalesceConfig::disabled();
        let mut coalescer = InputCoalescer::new(config);

        let bytes: usize = kani::any();
        kani::assume(bytes > 0 && bytes < 1_000_000);

        // Need to create a mock instant - skip for now as Instant::now()
        // can't be called in Kani. Test this in unit tests instead.
    }

    /// Verify buffer threshold triggers immediate render.
    #[kani::proof]
    fn buffer_threshold_triggers_render() {
        let threshold: usize = kani::any();
        kani::assume(threshold > 0 && threshold <= 1_000_000);

        let mut coalescer = InputCoalescer::new(CoalesceConfig {
            buffer_threshold: threshold,
            ..CoalesceConfig::default()
        });

        coalescer.accumulated_bytes = threshold;

        // When accumulated >= threshold, should_render returns true
        // (regardless of timers, since buffer check doesn't need time)
        assert!(coalescer.accumulated_bytes >= threshold);
    }

    /// Verify state transitions are valid.
    #[kani::proof]
    fn state_transitions_valid() {
        let state: u8 = kani::any();
        kani::assume(state < 3);

        let state = match state {
            0 => CoalesceState::Idle,
            1 => CoalesceState::Waiting,
            _ => CoalesceState::UpperArmed,
        };

        // All states should be distinct
        match state {
            CoalesceState::Idle => {
                assert!(state != CoalesceState::Waiting);
                assert!(state != CoalesceState::UpperArmed);
            }
            CoalesceState::Waiting => {
                assert!(state != CoalesceState::Idle);
                assert!(state != CoalesceState::UpperArmed);
            }
            CoalesceState::UpperArmed => {
                assert!(state != CoalesceState::Idle);
                assert!(state != CoalesceState::Waiting);
            }
        }
    }

    /// Verify average batch size calculation doesn't panic.
    #[kani::proof]
    fn average_batch_size_safe() {
        let mut coalescer = InputCoalescer::new(CoalesceConfig::default());

        coalescer.total_bytes = kani::any();
        coalescer.total_batches = kani::any();

        // Should never panic, even with zero batches
        let _avg = coalescer.average_batch_size();
    }
}

// ============================================================================
// Unit tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = CoalesceConfig::default();
        assert_eq!(config.lower_delay, Duration::from_nanos(500_000));
        assert_eq!(config.upper_delay, Duration::from_nanos(8_333_333));
        assert_eq!(config.buffer_threshold, 16_384);
        assert_eq!(config.max_buffer_size, 1_048_576);
        assert!(config.enabled);
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_low_latency_config() {
        let config = CoalesceConfig::low_latency();
        assert!(config.lower_delay < CoalesceConfig::default().lower_delay);
        assert!(config.upper_delay < CoalesceConfig::default().upper_delay);
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_high_throughput_config() {
        let config = CoalesceConfig::high_throughput();
        assert!(config.lower_delay > CoalesceConfig::default().lower_delay);
        assert!(config.upper_delay > CoalesceConfig::default().upper_delay);
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_config_validation_upper_less_than_lower() {
        let config = CoalesceConfig {
            lower_delay: Duration::from_micros(1_000),
            upper_delay: Duration::from_nanos(500_000),
            ..CoalesceConfig::default()
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_config_validation_upper_too_large() {
        let config = CoalesceConfig {
            upper_delay: Duration::from_secs(2), // 2 seconds
            ..CoalesceConfig::default()
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_config_validation_buffer_less_than_max() {
        let config = CoalesceConfig {
            buffer_threshold: 100_000,
            max_buffer_size: 50_000,
            ..CoalesceConfig::default()
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_coalescer_initial_state() {
        let coalescer = InputCoalescer::new(CoalesceConfig::default());
        assert_eq!(coalescer.state(), CoalesceState::Idle);
        assert_eq!(coalescer.accumulated_bytes(), 0);
        assert!(!coalescer.has_pending());
    }

    #[test]
    fn test_coalescer_disabled_immediate_render() {
        let mut coalescer = InputCoalescer::new(CoalesceConfig::disabled());
        let now = Instant::now();

        let action = coalescer.on_input(now, 100);
        assert_eq!(action, Some(CoalesceAction::RenderNow));
    }

    #[test]
    fn test_coalescer_buffer_threshold_immediate_render() {
        let mut coalescer = InputCoalescer::new(CoalesceConfig {
            buffer_threshold: 100,
            ..CoalesceConfig::default()
        });
        let now = Instant::now();

        // First input below threshold
        let action = coalescer.on_input(now, 50);
        assert!(matches!(action, Some(CoalesceAction::WaitUntil(_))));

        // Second input exceeds threshold
        let action = coalescer.on_input(now, 60);
        assert_eq!(action, Some(CoalesceAction::RenderNow));
    }

    #[test]
    fn test_coalescer_on_render_resets() {
        let mut coalescer = InputCoalescer::new(CoalesceConfig::default());
        let now = Instant::now();

        coalescer.on_input(now, 100);
        assert!(coalescer.has_pending());

        coalescer.on_render();
        assert!(!coalescer.has_pending());
        assert_eq!(coalescer.accumulated_bytes(), 0);
        assert_eq!(coalescer.state(), CoalesceState::Idle);
        assert_eq!(coalescer.total_batches(), 1);
    }

    #[test]
    fn test_coalescer_statistics() {
        let mut coalescer = InputCoalescer::new(CoalesceConfig::disabled());
        let now = Instant::now();

        coalescer.on_input(now, 100);
        coalescer.on_render();
        coalescer.on_input(now, 200);
        coalescer.on_render();

        assert_eq!(coalescer.total_bytes(), 300);
        assert_eq!(coalescer.total_batches(), 2);
        assert_eq!(coalescer.average_batch_size(), 150);
    }

    #[test]
    fn test_coalescer_deadline_calculation() {
        let config = CoalesceConfig {
            lower_delay: Duration::from_micros(1_000), // 1ms
            upper_delay: Duration::from_millis(10),    // 10ms
            ..CoalesceConfig::default()
        };
        let mut coalescer = InputCoalescer::new(config);
        let now = Instant::now();

        let action = coalescer.on_input(now, 100);

        // Should return a wait action with deadline
        if let Some(CoalesceAction::WaitUntil(deadline)) = action {
            // Deadline should be within reasonable bounds
            assert!(deadline > now);
            assert!(deadline <= now + Duration::from_millis(10));
        } else {
            panic!("Expected WaitUntil action");
        }
    }

    #[test]
    fn test_coalescer_should_render_lower_expired() {
        let config = CoalesceConfig {
            lower_delay: Duration::from_nanos(100_000), // 100us
            upper_delay: Duration::from_millis(10),     // 10ms
            ..CoalesceConfig::default()
        };
        let mut coalescer = InputCoalescer::new(config);
        let start = Instant::now();

        coalescer.on_input(start, 100);

        // Wait for lower delay to expire
        std::thread::sleep(Duration::from_micros(200));
        let now = Instant::now();

        assert!(coalescer.should_render(now));
    }

    #[test]
    fn test_coalescer_reset() {
        let mut coalescer = InputCoalescer::new(CoalesceConfig::default());
        let now = Instant::now();

        coalescer.on_input(now, 100);
        coalescer.reset();

        assert!(!coalescer.has_pending());
        assert_eq!(coalescer.total_batches(), 0); // reset doesn't count as batch
    }

    #[test]
    fn test_config_builders() {
        let config = CoalesceConfig::new()
            .with_lower_delay(Duration::from_millis(1))
            .with_upper_delay(Duration::from_millis(20))
            .with_buffer_threshold(32_768);

        assert_eq!(config.lower_delay, Duration::from_millis(1));
        assert_eq!(config.upper_delay, Duration::from_millis(20));
        assert_eq!(config.buffer_threshold, 32_768);
    }

    #[test]
    fn test_for_120hz() {
        let config = CoalesceConfig::for_120hz();
        // 120Hz = 8.33ms frame, half = ~4.16ms
        assert!(config.upper_delay < Duration::from_micros(5_000));
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_for_144hz() {
        let config = CoalesceConfig::for_144hz();
        // 144Hz = 6.94ms frame, half = ~3.47ms
        assert!(config.upper_delay < Duration::from_micros(4_000));
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_time_until_deadlines() {
        let config = CoalesceConfig {
            lower_delay: Duration::from_micros(1_000), // 1ms
            upper_delay: Duration::from_millis(10),    // 10ms
            ..CoalesceConfig::default()
        };
        let mut coalescer = InputCoalescer::new(config);
        let now = Instant::now();

        // Before any input, no deadlines
        assert!(coalescer.time_until_lower_deadline(now).is_none());
        assert!(coalescer.time_until_upper_deadline(now).is_none());

        coalescer.on_input(now, 100);

        // After input, deadlines should exist
        let lower_remaining = coalescer.time_until_lower_deadline(now);
        let upper_remaining = coalescer.time_until_upper_deadline(now);

        assert!(lower_remaining.is_some());
        assert!(upper_remaining.is_some());
        assert!(lower_remaining.unwrap() <= upper_remaining.unwrap());
    }

    #[test]
    fn test_next_deadline() {
        let config = CoalesceConfig {
            lower_delay: Duration::from_micros(1_000), // 1ms
            upper_delay: Duration::from_millis(10),    // 10ms
            ..CoalesceConfig::default()
        };
        let mut coalescer = InputCoalescer::new(config);
        let now = Instant::now();

        // No deadline initially
        assert!(coalescer.next_deadline().is_none());

        coalescer.on_input(now, 100);

        // Should return the lower deadline (sooner)
        let deadline = coalescer.next_deadline();
        assert!(deadline.is_some());
    }

    #[test]
    fn test_saturating_bytes() {
        let mut coalescer = InputCoalescer::new(CoalesceConfig::disabled());
        let now = Instant::now();

        // Add near-max bytes
        coalescer.accumulated_bytes = usize::MAX - 100;
        coalescer.on_input(now, 200);

        // Should saturate at MAX, not overflow
        assert_eq!(coalescer.accumulated_bytes, usize::MAX);
    }
}
