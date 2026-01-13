// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

//! # DashFlow Constants
//!
//! Centralized constants for time values, retry parameters, and size limits.
//! Using named constants instead of magic numbers improves code clarity and maintainability.
//!
//! ## Categories
//!
//! - **Time Constants**: Timeouts, intervals, and durations
//! - **Duration Constants**: Pre-built `Duration` values for HTTP clients and polling
//! - **Retry Constants**: Retry limits, delays, and backoff parameters
//! - **Size Constants**: Capacity limits and size thresholds
//!
//! ## Usage
//!
//! ```rust,ignore
//! use dashflow::constants::{SECONDS_PER_DAY, DEFAULT_TIMEOUT_MS, DEFAULT_HTTP_REQUEST_TIMEOUT};
//!
//! let retention = Duration::from_secs(30 * SECONDS_PER_DAY);
//! let timeout = Duration::from_millis(DEFAULT_TIMEOUT_MS);
//! // Or use the pre-built Duration constant directly:
//! let http_timeout = DEFAULT_HTTP_REQUEST_TIMEOUT;
//! ```

use std::time::Duration;

// ============================================================================
// Time Constants (Phases 597-602)
// ============================================================================

/// Seconds in a day (86400 = 24 * 60 * 60)
///
/// Used for retention policies, cache expiration, and scheduling.
pub const SECONDS_PER_DAY: u64 = 86400;

/// Seconds in a week (604800 = 7 * 86400)
pub const SECONDS_PER_WEEK: u64 = 7 * SECONDS_PER_DAY;

/// Default timeout in milliseconds (30 seconds)
///
/// Used for MCP operations, API calls, and general request timeouts.
pub const DEFAULT_TIMEOUT_MS: u64 = 30_000;

/// Long timeout in milliseconds (60 seconds)
///
/// Used for operations that may take longer, such as LLM calls with retries.
pub const LONG_TIMEOUT_MS: u64 = 60_000;

/// Slow threshold in milliseconds (10 seconds)
///
/// Operations exceeding this duration are considered slow and may trigger alerts.
pub const SLOW_THRESHOLD_MS: u64 = 10_000;

/// Very long timeout in milliseconds (5 minutes)
///
/// Used for batch operations, large document processing, or complex agent workflows.
pub const VERY_LONG_TIMEOUT_MS: u64 = 300_000;

// ============================================================================
// HTTP Client Duration Constants (M-146)
// ============================================================================

/// Default HTTP request timeout (30 seconds)
///
/// Standard timeout for API requests, search queries, and general HTTP calls.
/// Matches `DEFAULT_TIMEOUT_MS` as a pre-built Duration.
pub const DEFAULT_HTTP_REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

/// Default HTTP connect timeout (10 seconds)
///
/// Time allowed to establish a TCP connection. Shorter than request timeout
/// to fail fast on network issues.
pub const DEFAULT_HTTP_CONNECT_TIMEOUT: Duration = Duration::from_secs(10);

/// Default connection pool idle timeout (90 seconds)
///
/// How long idle connections are kept in the pool before being closed.
/// Balances resource usage with connection reuse benefits.
pub const DEFAULT_POOL_IDLE_TIMEOUT: Duration = Duration::from_secs(90);

/// Default TCP keepalive interval (60 seconds)
///
/// Interval for TCP keepalive probes to detect dead connections.
pub const DEFAULT_TCP_KEEPALIVE: Duration = Duration::from_secs(60);

/// Short timeout for health checks and drain operations (5 seconds)
///
/// Used for operations that should complete quickly or fail fast.
pub const SHORT_TIMEOUT: Duration = Duration::from_secs(5);

/// Short polling interval (100ms)
///
/// Used for tight polling loops where responsiveness is critical.
pub const SHORT_POLL_INTERVAL: Duration = Duration::from_millis(100);

/// Medium polling interval (500ms)
///
/// Used for polling operations where some latency is acceptable.
pub const MEDIUM_POLL_INTERVAL: Duration = Duration::from_millis(500);

/// Very short poll interval for tight lock/coordination loops (10ms)
///
/// Used when waiting for locks, database availability, or tight coordination.
/// Shorter than SHORT_POLL_INTERVAL to minimize latency in critical sections.
pub const LOCK_RETRY_INTERVAL: Duration = Duration::from_millis(10);

/// Default health check interval (30 seconds)
///
/// How frequently health checks are performed on remote workers or services.
/// Balances responsiveness (detecting failures quickly) with resource usage
/// (avoiding excessive network traffic and CPU for health probes).
pub const DEFAULT_HEALTH_CHECK_INTERVAL: Duration = Duration::from_secs(30);

/// Long timeout for operations that may take longer (60 seconds)
///
/// Used for agent execution, LLM calls with retries, or operations that need
/// more time than the standard request timeout. Matches `LONG_TIMEOUT_MS`.
pub const LONG_TIMEOUT: Duration = Duration::from_secs(60);

// ============================================================================
// Retry Constants (Phases 603-608)
// ============================================================================

/// Default maximum retries for transient failures
///
/// Applies to API calls, database operations, and network requests.
pub const DEFAULT_MAX_RETRIES: u32 = 3;

/// Maximum allowed retries (hard limit to prevent infinite loops)
pub const MAX_RETRIES_LIMIT: u32 = 1000;

/// Default initial delay for exponential backoff (1 second)
pub const DEFAULT_INITIAL_DELAY_MS: u64 = 1_000;

/// Default maximum delay for exponential backoff (10 seconds)
///
/// Caps the delay between retries to prevent excessively long waits.
pub const DEFAULT_MAX_DELAY_MS: u64 = 10_000;

/// Default jitter range for retry delays (1 second)
///
/// Random jitter prevents thundering herd problems in distributed systems.
pub const DEFAULT_JITTER_MS: u64 = 1_000;

/// Default backoff multiplier for exponential retry
pub const DEFAULT_BACKOFF_MULTIPLIER: f64 = 2.0;

// ============================================================================
// Trace & Monitoring Constants
// ============================================================================

/// Maximum trace count for health monitoring
///
/// Limits the number of traces kept in memory for analysis.
pub const MAX_TRACE_COUNT: usize = 100;

/// High token threshold for complexity detection
///
/// Traces with tokens exceeding this are flagged as high-complexity.
pub const HIGH_TOKEN_THRESHOLD: u64 = 10_000;

/// Maximum concurrent executions for live introspection
pub const MAX_CONCURRENT_EXECUTIONS: usize = 100;

// ============================================================================
// Size Constants (Phases 609-614)
// ============================================================================

/// Maximum bytes for error context
///
/// Limits the size of error messages to prevent memory issues.
pub const MAX_BYTES_ERROR: usize = 10 * 1024; // 10KB

/// Default embedding cache size
pub const DEFAULT_CACHE_SIZE: usize = 10_000;

/// One million (for number formatting)
///
/// Used in formatting functions for human-readable output.
pub const MILLION: f64 = 1_000_000.0;

/// One thousand (for number formatting and common limits)
pub const THOUSAND: f64 = 1_000.0;

// ============================================================================
// Channel & Concurrency Constants
// ============================================================================

/// Default batch size for telemetry
pub const DEFAULT_BATCH_SIZE: usize = 100;

/// Maximum telemetry batch size (hard limit)
pub const MAX_TELEMETRY_BATCH_SIZE: usize = 1_000;

/// Default queue capacity for async processing
pub const DEFAULT_QUEUE_CAPACITY: usize = 1_000;

/// Default broadcast channel capacity
///
/// Used for event broadcasting (discovery, notifications).
/// Larger capacity reduces dropped messages but uses more memory.
pub const DEFAULT_BROADCAST_CHANNEL_CAPACITY: usize = 64;

/// Default MPSC channel capacity
///
/// Used for request/response channels (approval, commands).
/// Smaller than broadcast since these are typically more targeted.
pub const DEFAULT_MPSC_CHANNEL_CAPACITY: usize = 32;

/// Default WebSocket channel capacity
///
/// Used for WebSocket server connections.
/// Larger capacity to handle burst traffic from multiple clients.
pub const DEFAULT_WS_CHANNEL_CAPACITY: usize = 256;

// ============================================================================
// HTTP Pool Constants (M-147)
// ============================================================================

/// Default maximum idle connections per host in the connection pool
///
/// Balances memory usage with connection reuse benefits.
/// Higher values improve throughput for repeated requests to the same host.
pub const DEFAULT_POOL_MAX_IDLE_PER_HOST: usize = 32;

/// Default timeout for LLM requests (5 minutes = 300 seconds)
///
/// Longer timeout for LLM API calls which may take several minutes
/// for complex prompts, large context windows, or batch operations.
pub const DEFAULT_LLM_REQUEST_TIMEOUT: Duration = Duration::from_secs(300);

// ============================================================================
// Regex Engine Constants (M-147)
// ============================================================================

/// Maximum regex size limit (256KB)
///
/// Limits the compiled regex size to prevent memory exhaustion
/// from maliciously crafted patterns or overly complex regexes.
pub const REGEX_SIZE_LIMIT: usize = 256 * 1024;

/// Maximum regex DFA size limit (256KB)
///
/// Limits the DFA (Deterministic Finite Automaton) cache size
/// for the regex engine. Prevents unbounded memory growth during matching.
pub const REGEX_DFA_SIZE_LIMIT: usize = 256 * 1024;

// ============================================================================
// Streaming & Network Constants (M-147)
// ============================================================================

/// Default capacity for stream channels (10,000 messages)
///
/// Used for graph execution streaming where high throughput is expected.
/// Larger than typical async channels to handle burst traffic during
/// node execution without backpressure.
pub const DEFAULT_STREAM_CHANNEL_CAPACITY: usize = 10_000;

/// Default mDNS TTL (time-to-live) in seconds (120s = 2 minutes)
///
/// Controls how long mDNS/DNS-SD records are valid before re-announcement.
/// Also used as the package broadcast interval since both serve similar
/// network discovery purposes.
pub const DEFAULT_MDNS_TTL_SECS: u32 = 120;

/// Default broadcast interval for resource advertisements (60 seconds)
///
/// How frequently nodes broadcast their available resources on the network.
/// Half of the mDNS TTL to ensure resources are re-advertised before expiry.
pub const DEFAULT_RESOURCE_BROADCAST_INTERVAL_SECS: u32 = 60;

// ============================================================================
// Live Introspection Constants (M-147)
// ============================================================================

/// Default TTL for completed executions before cleanup (300 seconds = 5 minutes)
///
/// How long completed/failed/cancelled executions are retained before auto-cleanup.
/// Matches `DEFAULT_LLM_REQUEST_TIMEOUT` duration as a reasonable retention period.
pub const DEFAULT_COMPLETED_TTL_SECS: u64 = 300;

/// Default maximum history steps per execution (100 steps)
///
/// Limits the number of execution steps recorded per execution to prevent
/// unbounded memory growth. Larger values provide better debuggability but
/// increase memory usage proportionally.
pub const DEFAULT_MAX_HISTORY_STEPS: usize = 100;

/// Default telemetry flush timeout in seconds (5 seconds)
///
/// Maximum time to wait when flushing telemetry on shutdown.
/// Matches `SHORT_TIMEOUT` as both represent "fast operation" timeouts.
pub const DEFAULT_FLUSH_TIMEOUT_SECS: u64 = 5;

/// Default telemetry batch timeout in milliseconds (100ms)
///
/// Events are flushed after this timeout even if batch is not full.
/// Matches `SHORT_POLL_INTERVAL` as both represent responsive intervals.
pub const DEFAULT_TELEMETRY_BATCH_TIMEOUT_MS: u64 = 100;

// ============================================================================
// Self-Improvement Daemon Constants (M-147)
// ============================================================================

/// Default capacity for file watcher event channels (100 events)
///
/// Used by the self-improvement daemon to buffer filesystem notification events
/// (file creates, modifies, deletes). Size balances memory with burst handling
/// for rapid file changes during development or continuous runs.
pub const DEFAULT_FILE_WATCHER_CHANNEL_CAPACITY: usize = 100;

/// Default capacity for trigger event channels (100 events)
///
/// Used by the self-improvement daemon for streaming trigger events.
/// Size matches file watcher capacity for consistency in daemon event handling.
pub const DEFAULT_TRIGGER_CHANNEL_CAPACITY: usize = 100;

/// Maximum channel capacity for validation (4096 messages)
///
/// Used as an upper bound when configuring channel capacities.
/// Prevents unbounded memory growth from excessive channel buffering.
/// The actual capacity is typically clamped between a minimum (e.g., 64)
/// and this maximum.
pub const DEFAULT_MAX_CHANNEL_CAPACITY: usize = 4096;

// ============================================================================
// Self-Improvement Metrics Buffer Constants (M-147)
// ============================================================================

/// Maximum node duration samples to retain per node (256 samples)
///
/// Used by the streaming consumer to limit memory usage when collecting
/// node performance metrics. Older samples are evicted to keep the
/// largest (slowest) values which are most useful for detecting issues.
pub const DEFAULT_MAX_NODE_DURATION_SAMPLES: usize = 256;

/// Maximum quality score samples to retain (1024 samples)
///
/// Used by the streaming consumer for quality scoring metrics.
/// Larger than duration samples since quality scores are aggregated
/// globally rather than per-node.
pub const DEFAULT_MAX_QUALITY_SCORE_SAMPLES: usize = 1024;

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_time_constants_consistency() {
        // Verify derived constants match base values
        assert_eq!(SECONDS_PER_WEEK, 7 * SECONDS_PER_DAY);
        assert_eq!(SECONDS_PER_DAY, 24 * 60 * 60);
    }

    #[test]
    fn test_timeout_ordering() {
        // Timeouts should be in ascending order
        assert!(SLOW_THRESHOLD_MS < DEFAULT_TIMEOUT_MS);
        assert!(DEFAULT_TIMEOUT_MS < LONG_TIMEOUT_MS);
        assert!(LONG_TIMEOUT_MS < VERY_LONG_TIMEOUT_MS);
    }

    #[test]
    fn test_retry_constants_reasonable() {
        // Initial delay should not exceed max delay
        assert!(DEFAULT_INITIAL_DELAY_MS <= DEFAULT_MAX_DELAY_MS);

        // Max retries limit should be reasonable
        assert!(DEFAULT_MAX_RETRIES <= MAX_RETRIES_LIMIT);
    }

    #[test]
    fn test_formatting_constants() {
        assert_eq!(MILLION, 1_000_000.0);
        assert_eq!(THOUSAND, 1_000.0);
    }

    #[test]
    fn test_http_duration_constants_values() {
        // Verify Duration constants match expected values
        assert_eq!(DEFAULT_HTTP_REQUEST_TIMEOUT, Duration::from_secs(30));
        assert_eq!(DEFAULT_HTTP_CONNECT_TIMEOUT, Duration::from_secs(10));
        assert_eq!(DEFAULT_POOL_IDLE_TIMEOUT, Duration::from_secs(90));
        assert_eq!(DEFAULT_TCP_KEEPALIVE, Duration::from_secs(60));
        assert_eq!(SHORT_TIMEOUT, Duration::from_secs(5));
        assert_eq!(SHORT_POLL_INTERVAL, Duration::from_millis(100));
        assert_eq!(MEDIUM_POLL_INTERVAL, Duration::from_millis(500));
    }

    #[test]
    fn test_http_duration_ordering() {
        // Connect timeout should be shorter than request timeout
        assert!(DEFAULT_HTTP_CONNECT_TIMEOUT < DEFAULT_HTTP_REQUEST_TIMEOUT);
        // Short timeout should be shortest
        assert!(SHORT_TIMEOUT < DEFAULT_HTTP_CONNECT_TIMEOUT);
        // Polling intervals should be in order
        assert!(SHORT_POLL_INTERVAL < MEDIUM_POLL_INTERVAL);
    }

    #[test]
    fn test_duration_matches_ms_constants() {
        // Verify Duration constants match the u64 millisecond equivalents
        assert_eq!(
            DEFAULT_HTTP_REQUEST_TIMEOUT,
            Duration::from_millis(DEFAULT_TIMEOUT_MS)
        );
        assert_eq!(
            DEFAULT_HTTP_CONNECT_TIMEOUT,
            Duration::from_millis(SLOW_THRESHOLD_MS)
        );
    }

    #[test]
    fn test_lock_retry_interval_value() {
        assert_eq!(LOCK_RETRY_INTERVAL, Duration::from_millis(10));
        // Lock retry should be much shorter than short poll interval
        assert!(LOCK_RETRY_INTERVAL < SHORT_POLL_INTERVAL);
    }

    #[test]
    fn test_health_check_interval_value() {
        // Health check interval should be 30 seconds
        assert_eq!(DEFAULT_HEALTH_CHECK_INTERVAL, Duration::from_secs(30));
        // Should be longer than short timeout (health checks need time)
        assert!(DEFAULT_HEALTH_CHECK_INTERVAL > SHORT_TIMEOUT);
        // Should equal the HTTP request timeout (reasonable balance)
        assert_eq!(DEFAULT_HEALTH_CHECK_INTERVAL, DEFAULT_HTTP_REQUEST_TIMEOUT);
    }

    #[test]
    fn test_long_timeout_value() {
        // Long timeout should be 60 seconds
        assert_eq!(LONG_TIMEOUT, Duration::from_secs(60));
        // Should match LONG_TIMEOUT_MS in milliseconds
        assert_eq!(LONG_TIMEOUT, Duration::from_millis(LONG_TIMEOUT_MS));
        // Should be longer than standard request timeout
        assert!(LONG_TIMEOUT > DEFAULT_HTTP_REQUEST_TIMEOUT);
        // Should be shorter than LLM request timeout (5 minutes)
        assert!(LONG_TIMEOUT < DEFAULT_LLM_REQUEST_TIMEOUT);
    }

    #[test]
    fn test_channel_capacity_ordering() {
        // Channel capacities should be in logical order by typical message volume
        assert!(DEFAULT_MPSC_CHANNEL_CAPACITY < DEFAULT_BROADCAST_CHANNEL_CAPACITY);
        assert!(DEFAULT_BROADCAST_CHANNEL_CAPACITY < DEFAULT_WS_CHANNEL_CAPACITY);
    }

    #[test]
    fn test_channel_capacity_reasonable() {
        // All capacities should be power of 2 or close (typical allocator-friendly)
        assert!(DEFAULT_MPSC_CHANNEL_CAPACITY >= 16);
        assert!(DEFAULT_BROADCAST_CHANNEL_CAPACITY >= 32);
        assert!(DEFAULT_WS_CHANNEL_CAPACITY >= 128);
        assert!(DEFAULT_WS_CHANNEL_CAPACITY <= 1024);
    }

    #[test]
    fn test_pool_max_idle_per_host_value() {
        // Default pool size should be reasonable (power of 2)
        assert_eq!(DEFAULT_POOL_MAX_IDLE_PER_HOST, 32);
        // Should be between 1 and 256
        assert!(DEFAULT_POOL_MAX_IDLE_PER_HOST >= 1);
        assert!(DEFAULT_POOL_MAX_IDLE_PER_HOST <= 256);
    }

    #[test]
    fn test_llm_request_timeout_value() {
        // LLM timeout should be 5 minutes
        assert_eq!(DEFAULT_LLM_REQUEST_TIMEOUT, Duration::from_secs(300));
        // Should be longer than standard request timeout
        assert!(DEFAULT_LLM_REQUEST_TIMEOUT > DEFAULT_HTTP_REQUEST_TIMEOUT);
    }

    #[test]
    fn test_regex_limits_values() {
        // Both limits should be 256KB
        assert_eq!(REGEX_SIZE_LIMIT, 256 * 1024);
        assert_eq!(REGEX_DFA_SIZE_LIMIT, 256 * 1024);
        // Should be reasonable values (not too small, not too large)
        assert!(REGEX_SIZE_LIMIT >= 64 * 1024); // At least 64KB
        assert!(REGEX_SIZE_LIMIT <= 1024 * 1024); // At most 1MB
    }

    #[test]
    fn test_stream_channel_capacity_value() {
        // Stream channel should be 10,000 (high throughput streaming)
        assert_eq!(DEFAULT_STREAM_CHANNEL_CAPACITY, 10_000);
        // Should be larger than standard channel capacities
        assert!(DEFAULT_STREAM_CHANNEL_CAPACITY > DEFAULT_WS_CHANNEL_CAPACITY);
        assert!(DEFAULT_STREAM_CHANNEL_CAPACITY > DEFAULT_BROADCAST_CHANNEL_CAPACITY);
    }

    #[test]
    fn test_network_broadcast_constants() {
        // mDNS TTL should be 120 seconds (2 minutes)
        assert_eq!(DEFAULT_MDNS_TTL_SECS, 120);
        // Resource broadcast should be 60 seconds (1 minute)
        assert_eq!(DEFAULT_RESOURCE_BROADCAST_INTERVAL_SECS, 60);
        // Resource broadcast should be half of TTL for overlap safety
        assert_eq!(DEFAULT_RESOURCE_BROADCAST_INTERVAL_SECS * 2, DEFAULT_MDNS_TTL_SECS);
    }

    #[test]
    fn test_self_improvement_daemon_constants() {
        // File watcher and trigger channels should have same capacity (100)
        assert_eq!(DEFAULT_FILE_WATCHER_CHANNEL_CAPACITY, 100);
        assert_eq!(DEFAULT_TRIGGER_CHANNEL_CAPACITY, 100);
        // Should match for consistency in daemon event handling
        assert_eq!(DEFAULT_FILE_WATCHER_CHANNEL_CAPACITY, DEFAULT_TRIGGER_CHANNEL_CAPACITY);
        // Should be larger than standard MPSC capacity (32) for burst handling
        assert!(DEFAULT_FILE_WATCHER_CHANNEL_CAPACITY > DEFAULT_MPSC_CHANNEL_CAPACITY);
        assert!(DEFAULT_TRIGGER_CHANNEL_CAPACITY > DEFAULT_MPSC_CHANNEL_CAPACITY);
        // Should be smaller than stream channel (10K) since daemon events are lower volume
        assert!(DEFAULT_FILE_WATCHER_CHANNEL_CAPACITY < DEFAULT_STREAM_CHANNEL_CAPACITY);
    }

    #[test]
    fn test_live_introspection_constants() {
        // Completed TTL should be 300 seconds (5 minutes)
        assert_eq!(DEFAULT_COMPLETED_TTL_SECS, 300);
        // Should match LLM request timeout duration in seconds
        assert_eq!(DEFAULT_COMPLETED_TTL_SECS, DEFAULT_LLM_REQUEST_TIMEOUT.as_secs());

        // Max history steps should be 100 (reasonable for debugging)
        assert_eq!(DEFAULT_MAX_HISTORY_STEPS, 100);
        // Should match file watcher/trigger channel capacity (same semantic meaning)
        assert_eq!(DEFAULT_MAX_HISTORY_STEPS, DEFAULT_FILE_WATCHER_CHANNEL_CAPACITY);

        // Flush timeout should be 5 seconds (fast operation)
        assert_eq!(DEFAULT_FLUSH_TIMEOUT_SECS, 5);
        // Should match SHORT_TIMEOUT duration in seconds
        assert_eq!(DEFAULT_FLUSH_TIMEOUT_SECS, SHORT_TIMEOUT.as_secs());

        // Telemetry batch timeout should be 100ms (responsive interval)
        assert_eq!(DEFAULT_TELEMETRY_BATCH_TIMEOUT_MS, 100);
        // Should match SHORT_POLL_INTERVAL duration in milliseconds
        assert_eq!(DEFAULT_TELEMETRY_BATCH_TIMEOUT_MS, SHORT_POLL_INTERVAL.as_millis() as u64);
    }

    #[test]
    fn test_max_channel_capacity_value() {
        // Max channel capacity should be 4096 (validation upper bound)
        assert_eq!(DEFAULT_MAX_CHANNEL_CAPACITY, 4096);
        // Should be larger than standard non-streaming channel capacities
        assert!(DEFAULT_MAX_CHANNEL_CAPACITY > DEFAULT_WS_CHANNEL_CAPACITY);
        assert!(DEFAULT_MAX_CHANNEL_CAPACITY > DEFAULT_BROADCAST_CHANNEL_CAPACITY);
        // Stream channel capacity is larger (10K) for high-throughput streaming
        // Max channel capacity is for validation of telemetry channels, not streaming
        assert!(DEFAULT_STREAM_CHANNEL_CAPACITY > DEFAULT_MAX_CHANNEL_CAPACITY);
    }

    #[test]
    fn test_self_improvement_metrics_buffer_constants() {
        // Node duration samples should be 256 (per-node buffer)
        assert_eq!(DEFAULT_MAX_NODE_DURATION_SAMPLES, 256);
        // Quality score samples should be 1024 (global buffer)
        assert_eq!(DEFAULT_MAX_QUALITY_SCORE_SAMPLES, 1024);
        // Quality scores buffer should be larger than duration buffer
        // (quality is global aggregation, duration is per-node)
        assert!(DEFAULT_MAX_QUALITY_SCORE_SAMPLES > DEFAULT_MAX_NODE_DURATION_SAMPLES);
        // Both should be reasonable powers of 2 for memory alignment
        assert!(DEFAULT_MAX_NODE_DURATION_SAMPLES.is_power_of_two());
        assert!(DEFAULT_MAX_QUALITY_SCORE_SAMPLES.is_power_of_two());
    }
}
