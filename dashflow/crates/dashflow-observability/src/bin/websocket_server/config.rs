//! Configuration constants and environment variable parsing for the WebSocket server.
//!
//! This module centralizes all server configuration to make tuning and deployment
//! easier. All values can be overridden via environment variables.

use base64::{engine::general_purpose::URL_SAFE_NO_PAD as BASE64_URL_SAFE_NO_PAD, Engine};
use std::sync::OnceLock;

// =============================================================================
// ENVIRONMENT VARIABLE NAME CONSTANTS
// =============================================================================
// These mirror dashflow::core::config_loader::env_vars but are defined locally
// to avoid cyclic dependency (dashflow-observability cannot depend on dashflow).

/// Kafka broker addresses (default: 127.0.0.1:9092)
pub const KAFKA_BROKERS: &str = "KAFKA_BROKERS";
/// Kafka topic to consume (default: dashstream-quality)
pub const KAFKA_TOPIC: &str = "KAFKA_TOPIC";
/// Kafka Dead Letter Queue topic (default: {KAFKA_TOPIC}-dlq)
pub const KAFKA_DLQ_TOPIC: &str = "KAFKA_DLQ_TOPIC";
/// Kafka consumer group ID (default: websocket-server-v4)
pub const KAFKA_GROUP_ID: &str = "KAFKA_GROUP_ID";
/// Optional Kafka cluster ID for namespace disambiguation (M-747)
pub const KAFKA_CLUSTER_ID: &str = "KAFKA_CLUSTER_ID";
/// Kafka auto offset reset strategy (default: earliest)
pub const KAFKA_AUTO_OFFSET_RESET: &str = "KAFKA_AUTO_OFFSET_RESET";
/// Whether to use timestamp-based seeking for old data (default: false)
pub const KAFKA_OLD_DATA_USE_TIMESTAMP: &str = "KAFKA_OLD_DATA_USE_TIMESTAMP";

/// Redis URL for replay buffer persistence (default: redis://127.0.0.1:6379)
pub const REDIS_URL: &str = "REDIS_URL";
/// Redis clear timeout in seconds for replay buffer cursor_reset (default: 5)
/// M-153: Mirrors dashflow::core::config_loader::env_vars::REDIS_CLEAR_TIMEOUT_SECS
pub const REDIS_CLEAR_TIMEOUT_SECS: &str = "REDIS_CLEAR_TIMEOUT_SECS";

/// WebSocket server host binding (default: 127.0.0.1)
pub const WEBSOCKET_HOST: &str = "WEBSOCKET_HOST";
/// Maximum WebSocket connections per IP (default: 10)
pub const WEBSOCKET_MAX_CONNECTIONS_PER_IP: &str = "WEBSOCKET_MAX_CONNECTIONS_PER_IP";

/// OpenTelemetry OTLP exporter endpoint
pub const OTEL_EXPORTER_OTLP_ENDPOINT: &str = "OTEL_EXPORTER_OTLP_ENDPOINT";

/// Path to expected schemas for validation
pub const EXPECTED_SCHEMAS_PATH: &str = "EXPECTED_SCHEMAS_PATH";

/// Include full payload in dead letter queue messages (default: false)
/// M-153: Mirrors dashflow::core::config_loader::env_vars::DLQ_INCLUDE_FULL_PAYLOAD
pub const DLQ_INCLUDE_FULL_PAYLOAD: &str = "DLQ_INCLUDE_FULL_PAYLOAD";

/// Trusted proxy IPs for WebSocket x-forwarded-for parsing (comma-separated)
/// M-153: Mirrors dashflow::core::config_loader::env_vars::WEBSOCKET_TRUSTED_PROXY_IPS
pub const WEBSOCKET_TRUSTED_PROXY_IPS: &str = "WEBSOCKET_TRUSTED_PROXY_IPS";

/// Kafka decode error handling strategy: "skip" (default) or "pause"
/// M-153: Mirrors dashflow::core::config_loader::env_vars::KAFKA_ON_DECODE_ERROR
pub const KAFKA_ON_DECODE_ERROR: &str = "KAFKA_ON_DECODE_ERROR";

// =============================================================================
// DEFAULT CONSTANTS
// =============================================================================

/// Max number of concurrent background DLQ sends (best-effort).
/// Configurable via MAX_CONCURRENT_DLQ_SENDS env var.
/// See M-429 documentation in dlq.rs for durability semantics.
///
/// M-731: Lower this value if DLQ timeouts cause sustained semaphore exhaustion.
/// With timeout=5s and concurrency=100, if DLQ is broken, all 100 slots fill
/// with timing-out requests and new messages are dropped for up to 5 seconds.
/// Recommended: Set MAX_CONCURRENT_DLQ_SENDS=20 for production if DLQ latency
/// is a concern, or reduce DLQ_SEND_TIMEOUT_SECS to 1-2s.
pub const DEFAULT_MAX_CONCURRENT_DLQ_SENDS: usize = 100;

/// Timeout used for DLQ producer delivery (`message.timeout.ms`), in seconds.
/// Configurable via `DLQ_SEND_TIMEOUT_SECS` env var.
/// See M-429 documentation in dlq.rs for durability semantics.
///
/// M-731: Lower this for faster failure detection. 5s timeout with 100 concurrent
/// slots means worst-case 5 seconds of backpressure-induced drops when DLQ is broken.
/// Consider DLQ_SEND_TIMEOUT_SECS=2 for production if fast failure is preferred.
pub const DEFAULT_DLQ_SEND_TIMEOUT_SECS: u64 = 5;

/// M-1064: Whether to include full base64-encoded payloads in DLQ messages.
/// Default: false. When false, DLQ messages include only:
///   - SHA256 hash of the payload (for content verification)
///   - first/last 16 bytes (hex) for forensics
///
/// This prevents secret leakage and avoids exceeding Kafka message size limits.
/// Set DLQ_INCLUDE_FULL_PAYLOAD=true to enable legacy behavior (full base64 payloads).
pub const DEFAULT_DLQ_INCLUDE_FULL_PAYLOAD: bool = false;

/// M-682: Default threshold for cumulative lag before disconnecting slow clients.
/// If a client accumulates more than this many dropped messages during a session,
/// it will be disconnected and can reconnect using the resume protocol.
/// Configurable via `SLOW_CLIENT_DISCONNECT_THRESHOLD` env var.
/// Set to 0 to disable (never disconnect slow clients).
pub const DEFAULT_SLOW_CLIENT_DISCONNECT_THRESHOLD: u64 = 1000;

/// M-773: Default window duration (seconds) for slow client lag measurement.
/// The lag counter resets after this period, implementing "leaky bucket" semantics.
/// This prevents long-lived clients with occasional lag from eventually being disconnected
/// solely due to longevity. Set to 0 to use lifetime cumulative lag (legacy behavior).
/// Configurable via `SLOW_CLIENT_LAG_WINDOW_SECS` env var.
pub const DEFAULT_SLOW_CLIENT_LAG_WINDOW_SECS: u64 = 60;

/// M-743: Default timeout for replay operations (seconds).
/// If replay takes longer than this, the client is disconnected to prevent blocking.
/// Configurable via `REPLAY_TIMEOUT_SECS` env var.
pub const DEFAULT_REPLAY_TIMEOUT_SECS: u64 = 30;

/// M-743: Default maximum total messages to replay across all partitions.
/// This is a safety cap to prevent slow clients from blocking handlers indefinitely.
/// Configurable via `REPLAY_MAX_TOTAL` env var.
pub const DEFAULT_REPLAY_MAX_TOTAL: usize = 10_000;

/// M-1033: Default timeout for WebSocket send operations (seconds).
/// If a send takes longer than this, it is aborted and the client may be disconnected.
/// Configurable via `WEBSOCKET_SEND_TIMEOUT_SECS` env var.
pub const DEFAULT_SEND_TIMEOUT_SECS: u64 = 5;

// =============================================================================
// ENVIRONMENT VARIABLE PARSING
// =============================================================================

/// M-496: Helper to parse env vars with warning on parse failure.
///
/// When an env var is set but cannot be parsed, logs a warning and uses the default.
/// This prevents silent fallback that could confuse operators.
pub fn parse_env_var_with_warning<T: std::str::FromStr>(var_name: &str, default: T) -> T {
    match std::env::var(var_name) {
        Ok(value) => match value.parse() {
            Ok(parsed) => parsed,
            Err(_) => {
                eprintln!(
                    "Warning: Invalid value for {}: '{}'. Using default.",
                    var_name, value
                );
                default
            }
        },
        Err(_) => default,
    }
}

/// M-496: Helper to parse optional env vars with warning on parse failure.
///
/// Returns Some(value) if env var is set and parses successfully, None if unset.
/// Logs a warning and returns None if env var is set but cannot be parsed.
pub fn parse_optional_env_var_with_warning<T: std::str::FromStr>(var_name: &str) -> Option<T> {
    match std::env::var(var_name) {
        Ok(value) => match value.parse() {
            Ok(parsed) => Some(parsed),
            Err(_) => {
                eprintln!(
                    "Warning: Invalid value for {}: '{}'. Treating as unset.",
                    var_name, value
                );
                None
            }
        },
        Err(_) => None,
    }
}

/// M-691: Normalize `KAFKA_BROKERS` into a stable string for namespacing.
pub fn normalize_kafka_brokers(kafka_brokers: &str) -> String {
    let mut brokers: Vec<&str> = kafka_brokers
        .split(',')
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .collect();
    brokers.sort_unstable();
    brokers.join(",")
}

/// M-691: Stable namespace for resume cursors and replay buffers.
///
/// This is intentionally opaque and URL-safe so it can be embedded into Redis keys and localStorage keys.
///
/// M-747: Optional `cluster_id` parameter prevents collisions when multiple Kafka clusters
/// happen to have the same broker hostnames (e.g., different environments behind the same DNS).
/// Set via `KAFKA_CLUSTER_ID` env var. If not set, namespace uses only topic/group/brokers.
pub fn compute_resume_namespace(
    kafka_brokers: &str,
    kafka_topic: &str,
    group_id: &str,
    cluster_id: Option<&str>,
) -> String {
    let normalized_brokers = normalize_kafka_brokers(kafka_brokers);
    let raw = match cluster_id {
        Some(id) if !id.is_empty() => format!(
            "cluster={};topic={};group={};brokers={}",
            id, kafka_topic, group_id, normalized_brokers
        ),
        _ => format!(
            "topic={};group={};brokers={}",
            kafka_topic, group_id, normalized_brokers
        ),
    };
    // M-747: Use v2 prefix when cluster_id is present to avoid mixing with v1 namespaces
    let version = if cluster_id.is_some_and(|id| !id.is_empty()) {
        "v2"
    } else {
        "v1"
    };
    format!("{}_{}", version, BASE64_URL_SAFE_NO_PAD.encode(raw.as_bytes()))
}

// =============================================================================
// CONFIGURATION GETTERS (with env var overrides)
// =============================================================================

/// Get max concurrent DLQ sends from env var or use default (S-19)
pub fn get_max_concurrent_dlq_sends() -> usize {
    parse_env_var_with_warning("MAX_CONCURRENT_DLQ_SENDS", DEFAULT_MAX_CONCURRENT_DLQ_SENDS)
}

/// M-491: Get DLQ send timeout (ms) from env var or use default.
pub fn get_dlq_send_timeout_ms() -> u64 {
    let secs = parse_env_var_with_warning("DLQ_SEND_TIMEOUT_SECS", DEFAULT_DLQ_SEND_TIMEOUT_SECS);
    if secs == 0 {
        eprintln!(
            "Warning: DLQ_SEND_TIMEOUT_SECS must be >= 1; using default {}",
            DEFAULT_DLQ_SEND_TIMEOUT_SECS
        );
        return DEFAULT_DLQ_SEND_TIMEOUT_SECS.saturating_mul(1000);
    }
    let ms = secs.saturating_mul(1000);
    let max_ms = i32::MAX as u64;
    if ms > max_ms {
        eprintln!(
            "Warning: DLQ_SEND_TIMEOUT_SECS too large ({}s); clamping message.timeout.ms to {}ms",
            secs, max_ms
        );
        return max_ms;
    }
    ms
}

/// M-1064: Check if full base64 payload should be included in DLQ messages.
/// Returns true only if DLQ_INCLUDE_FULL_PAYLOAD env var is set to "true" or "1".
pub fn get_dlq_include_full_payload() -> bool {
    match std::env::var(DLQ_INCLUDE_FULL_PAYLOAD) {
        Ok(value) => {
            let v = value.to_lowercase();
            v == "true" || v == "1"
        }
        Err(_) => DEFAULT_DLQ_INCLUDE_FULL_PAYLOAD,
    }
}

/// M-743: Get replay timeout (seconds) from env var or use default.
pub fn get_replay_timeout_secs() -> u64 {
    let secs = parse_env_var_with_warning("REPLAY_TIMEOUT_SECS", DEFAULT_REPLAY_TIMEOUT_SECS);
    if secs == 0 {
        eprintln!(
            "Warning: REPLAY_TIMEOUT_SECS must be >= 1; using default {}",
            DEFAULT_REPLAY_TIMEOUT_SECS
        );
        return DEFAULT_REPLAY_TIMEOUT_SECS;
    }
    secs
}

/// M-743: Get replay max total messages from env var or use default.
pub fn get_replay_max_total() -> usize {
    let max = parse_env_var_with_warning("REPLAY_MAX_TOTAL", DEFAULT_REPLAY_MAX_TOTAL);
    if max == 0 {
        eprintln!(
            "Warning: REPLAY_MAX_TOTAL must be >= 1; using default {}",
            DEFAULT_REPLAY_MAX_TOTAL
        );
        return DEFAULT_REPLAY_MAX_TOTAL;
    }
    max
}

/// M-1033: Get send timeout (seconds) from env var or use default.
/// M-1071 FIX: Use OnceLock to cache the value at first call, avoiding repeated
/// env var parsing on every WebSocket send (hot path).
pub fn get_send_timeout_secs() -> u64 {
    static SEND_TIMEOUT_SECS: OnceLock<u64> = OnceLock::new();
    *SEND_TIMEOUT_SECS.get_or_init(|| {
        let secs = parse_env_var_with_warning("WEBSOCKET_SEND_TIMEOUT_SECS", DEFAULT_SEND_TIMEOUT_SECS);
        if secs == 0 {
            eprintln!(
                "Warning: WEBSOCKET_SEND_TIMEOUT_SECS must be >= 1; using default {}",
                DEFAULT_SEND_TIMEOUT_SECS
            );
            return DEFAULT_SEND_TIMEOUT_SECS;
        }
        secs
    })
}

/// Get slow client disconnect threshold from env var or use default (M-682)
///
/// Reserved for M-682 (slow client handling) - not yet wired to connection logic.
#[allow(dead_code)]
pub fn get_slow_client_disconnect_threshold() -> u64 {
    parse_env_var_with_warning(
        "SLOW_CLIENT_DISCONNECT_THRESHOLD",
        DEFAULT_SLOW_CLIENT_DISCONNECT_THRESHOLD,
    )
}

/// Get slow client lag window duration from env var or use default (M-773)
///
/// Reserved for M-773 (slow client lag detection) - not yet wired to connection logic.
#[allow(dead_code)]
pub fn get_slow_client_lag_window_secs() -> u64 {
    parse_env_var_with_warning(
        "SLOW_CLIENT_LAG_WINDOW_SECS",
        DEFAULT_SLOW_CLIENT_LAG_WINDOW_SECS,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_kafka_brokers_sorts_and_dedupes() {
        assert_eq!(
            normalize_kafka_brokers("b:9092, a:9092, c:9092"),
            "a:9092,b:9092,c:9092"
        );
        assert_eq!(
            normalize_kafka_brokers("  b:9092 ,  a:9092  "),
            "a:9092,b:9092"
        );
    }

    #[test]
    fn test_compute_resume_namespace_without_cluster_id() {
        let ns = compute_resume_namespace("localhost:9092", "test-topic", "test-group", None);
        assert!(ns.starts_with("v1_"));
    }

    #[test]
    fn test_compute_resume_namespace_with_cluster_id() {
        let ns = compute_resume_namespace(
            "localhost:9092",
            "test-topic",
            "test-group",
            Some("prod-cluster"),
        );
        assert!(ns.starts_with("v2_"));
    }

    #[test]
    fn test_compute_resume_namespace_empty_cluster_id_uses_v1() {
        let ns = compute_resume_namespace("localhost:9092", "test-topic", "test-group", Some(""));
        assert!(ns.starts_with("v1_"));
    }
}
