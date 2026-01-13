//! HTTP and WebSocket handlers for the WebSocket server.
//!
//! This module contains all the route handlers for the WebSocket server,
//! including health checks, metrics, expected schema API, and WebSocket handling.

use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        ConnectInfo, Path, State,
    },
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use serde::Serialize;
use serde_json::json;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::time::{Duration, Instant};
use tokio::sync::broadcast;

use crate::{
    extract_client_ip, get_replay_max_total, get_replay_timeout_secs, get_send_timeout_secs,
    ServerMetrics, ServerMetricsSnapshot, ServerState, SetExpectedSchemaRequest,
};
use crate::replay_buffer::ReplayBufferMetricsSnapshot;
use std::sync::atomic::Ordering as AtomicOrdering;

// M-1008: Resume parsing bounds/validation constants.
// These prevent DoS attacks via oversized resume payloads.
/// Maximum number of partitions allowed in a resume request.
const MAX_RESUME_PARTITIONS: usize = 1024;
/// Maximum number of threads allowed in a resume request.
const MAX_RESUME_THREADS: usize = 10_000;

/// M-1061: Maximum size of a WebSocket control frame (Text/JSON messages) in bytes.
/// Messages exceeding this limit are rejected before JSON parsing to prevent DoS.
/// Default: 1MB (sufficient for any legitimate control message).
const WEBSOCKET_MAX_CONTROL_BYTES: usize = 1024 * 1024; // 1MB

/// M-1117: Maximum length for user-provided strings in parse_errors to prevent log/heap DoS.
const MAX_PARSE_ERROR_VALUE_LEN: usize = 128;

/// M-1117: Truncate a string for inclusion in parse_errors to prevent unbounded memory/logs.
fn truncate_for_error(s: &str) -> String {
    if s.len() <= MAX_PARSE_ERROR_VALUE_LEN {
        s.to_string()
    } else {
        format!("{}...({}b)", &s[..MAX_PARSE_ERROR_VALUE_LEN], s.len())
    }
}

/// M-1009/M-1033: Send a WebSocket message with a timeout.
/// Returns Ok(()) on success, Err with reason on failure or timeout.
/// M-1033: Increments send_failed/send_timeout counters on error.
/// M-1033: Timeout is configurable via WEBSOCKET_SEND_TIMEOUT_SECS env var.
async fn send_with_timeout(
    socket: &mut WebSocket,
    message: Message,
    metrics: &ServerMetrics,
) -> Result<(), &'static str> {
    let timeout_secs = get_send_timeout_secs();
    match tokio::time::timeout(Duration::from_secs(timeout_secs), socket.send(message)).await {
        Ok(Ok(())) => Ok(()),
        Ok(Err(_)) => {
            metrics.send_failed.fetch_add(1, AtomicOrdering::Relaxed);
            // M-1095: Record in sliding window for windowed /health metrics
            metrics.record_send_failed();
            Err("send_failed")
        }
        Err(_) => {
            metrics.send_timeout.fetch_add(1, AtomicOrdering::Relaxed);
            // M-1095: Record in sliding window for windowed /health metrics
            metrics.record_send_timeout();
            tracing::warn!(timeout_secs, "WebSocket send timed out");
            Err("send_timeout")
        }
    }
}

/// Health check response
#[derive(Serialize)]
pub(crate) struct HealthResponse {
    pub status: String,
    pub metrics: ServerMetricsSnapshot,
    pub replay_buffer: ReplayBufferMetricsSnapshot,
    pub kafka_status: String,
    pub websocket_status: String,
    /// Alert if dropped messages exceed threshold (indicates data loss)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub alert: Option<String>,
    /// Circuit breaker status (Issue #16: auto-recovery tracking)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub circuit_breaker: Option<CircuitBreakerStatus>,
}

/// Circuit breaker status for auto-recovery monitoring (Issue #16)
#[derive(Debug, Clone, Serialize)]
pub(crate) struct CircuitBreakerStatus {
    pub state: String, // "healthy", "degraded", "will_restart_soon"
    pub degraded_duration_seconds: Option<u64>,
    pub time_until_restart_seconds: Option<u64>,
}

/// Version info for deployment tracking
#[derive(Serialize)]
pub(crate) struct VersionInfo {
    /// Git commit SHA (short form)
    pub git_sha: &'static str,
    /// Build timestamp in ISO 8601 format
    pub build_date: &'static str,
    /// DashStream protobuf schema version
    pub schema_version: u32,
    /// Component name
    pub component: &'static str,
    /// M-691: Namespace to scope persisted resume cursors (UI localStorage, etc.).
    pub resume_namespace: String,
    pub kafka_topic: String,
    pub kafka_group_id: String,
    /// M-1019: Max payload bytes this server accepts. UI should compare against its own
    /// decompression limit and warn operators if server_max > ui_max (config drift).
    pub max_payload_bytes: usize,
    /// M-1020: How the server handles decode errors: "skip" (advance offset) or "pause" (stop consuming).
    pub decode_error_policy: &'static str,
}

/// Enhanced health check endpoint with detailed metrics
pub(crate) async fn health_handler(State(state): State<ServerState>) -> impl IntoResponse {
    // Create snapshot from atomic counters
    let mut metrics = state.metrics.snapshot();
    let replay_buffer = state.replay_buffer.snapshot_metrics();

    // Update connected clients from broadcast channel (can't be tracked atomically)
    metrics.connected_clients = state.tx.receiver_count();

    // Determine overall status - Issue #1: Separate infrastructure errors from data quality errors
    // M-1069: Use windowed decode error rate (last 120s) instead of lifetime rate.
    // This ensures recent spikes trigger alerts, and old spikes don't haunt healthy servers.
    // M-1094: Minimum sample size to prevent false positives on tiny denominators.
    // With < 100 messages, even 1 error would be 1% - statistically meaningless.
    const MIN_SAMPLE_SIZE_FOR_DEGRADED: u64 = 100;
    let decode_error_rate_120s = if metrics.messages_last_120s > 0 {
        (metrics.decode_errors_last_120s as f64) / (metrics.messages_last_120s as f64)
    } else {
        0.0
    };

    // Also compute lifetime rate for comparison/logging (not used for status determination)
    let decode_error_rate_lifetime = if metrics.kafka_messages_received > 0 {
        (metrics.decode_errors as f64) / (metrics.kafka_messages_received as f64)
    } else {
        0.0
    };

    // S-25 fix: Check if infrastructure errors are RECENT using actual timestamp
    // Previous logic was broken: it checked "any infra errors ever AND messages flowing"
    // which remained true forever. Now we check actual recency of the last infra error.
    let recent_infrastructure_errors = metrics
        .last_infrastructure_error_ago_seconds
        .map(|secs| secs < 120) // Within last 2 minutes
        .unwrap_or(false); // No infra errors recorded = not recent

    // M-1069: Use windowed rate for degraded status (reflects current reality)
    // M-1094: Only consider degraded if we have enough samples for statistical significance.
    // This prevents false positives during startup, low-traffic periods, or catch-up phases.
    let status = if metrics.messages_last_120s >= MIN_SAMPLE_SIZE_FOR_DEGRADED
        && decode_error_rate_120s > 0.01
    {
        // Data quality issue - CRITICAL
        // More than 1% decode errors in last 2 minutes with sufficient sample size
        "degraded".to_string()
    } else if metrics.kafka_messages_received == 0
        || metrics.last_kafka_message_ago_seconds.unwrap_or(999) > 60
    {
        // No messages or stale messages - WAITING/RECONNECTING
        if recent_infrastructure_errors {
            "reconnecting".to_string() // Infrastructure errors, actively reconnecting
        } else {
            "waiting".to_string() // Waiting for initial connection or messages
        }
    } else if recent_infrastructure_errors {
        // Messages flowing but with recent infrastructure errors (transient issues during recovery)
        "reconnecting".to_string()
    } else {
        // All good - messages flowing, no data errors, no recent infrastructure errors
        "healthy".to_string()
    };

    // Kafka status
    let kafka_status = if metrics.kafka_messages_received > 0
        && metrics.last_kafka_message_ago_seconds.unwrap_or(999) < 60
    {
        "connected".to_string()
    } else if metrics.kafka_messages_received == 0 {
        "waiting_for_messages".to_string()
    } else {
        "no_recent_messages".to_string()
    };

    // WebSocket status
    let websocket_status = if metrics.connected_clients > 0 {
        format!("{}_clients_connected", metrics.connected_clients)
    } else {
        "no_clients".to_string()
    };

    let mut alerts = Vec::new();
    // M-1041: Use a recency window (not lifetime totals) so long-lived servers don't warn forever.
    if metrics.dropped_messages_last_120s > 10 {
        alerts.push(format!(
            "WARNING: {} messages dropped due to slow clients in last 2m ({} total since boot). Consider increasing WEBSOCKET_BUFFER_SIZE or improving client performance.",
            metrics.dropped_messages_last_120s, metrics.dropped_messages
        ));
    }
    // M-1069/M-1094: Report error rate if above threshold AND sample size sufficient.
    // Also warn if rate is high but sample is too small to trigger degraded status.
    if decode_error_rate_120s > 0.01 {
        if metrics.messages_last_120s >= MIN_SAMPLE_SIZE_FOR_DEGRADED {
            alerts.push(format!(
                "CRITICAL: High decode error rate in last 2m ({:.2}%). {} errors / {} messages in window. Lifetime: {:.2}% ({}/{} total). Check logs for diagnostic details.",
                decode_error_rate_120s * 100.0,
                metrics.decode_errors_last_120s,
                metrics.messages_last_120s,
                decode_error_rate_lifetime * 100.0,
                metrics.decode_errors,
                metrics.kafka_messages_received
            ));
        } else {
            // High rate but insufficient sample - informational only
            alerts.push(format!(
                "INFO: Elevated decode error rate ({:.2}%) but sample size ({}) < minimum ({}). Not triggering degraded status.",
                decode_error_rate_120s * 100.0,
                metrics.messages_last_120s,
                MIN_SAMPLE_SIZE_FOR_DEGRADED
            ));
        }
    }
    if replay_buffer.redis_enabled && replay_buffer.redis_write_dropped > 0 {
        alerts.push(format!(
            "WARNING: Replay buffer dropped {} Redis writes (concurrency limit). Reconnect replays may be incomplete under load.",
            replay_buffer.redis_write_dropped
        ));
    }
    if replay_buffer.redis_enabled && replay_buffer.redis_write_failures > 0 {
        alerts.push(format!(
            "WARNING: Replay buffer had {} Redis write failures. Check Redis health/latency; reconnect replays may be incomplete.",
            replay_buffer.redis_write_failures
        ));
    }
    // M-1107 FIX: Surface payload_missing in health alerts (indicates data loss/corruption)
    if metrics.payload_missing > 0 {
        alerts.push(format!(
            "CRITICAL: {} Kafka messages had missing payloads (data loss). Indicates tombstone/corruption/compaction.",
            metrics.payload_missing
        ));
    }

    let alert = if alerts.is_empty() {
        None
    } else {
        Some(alerts.join(" | "))
    };

    // Issue #16: Circuit breaker status for auto-recovery monitoring
    // Issue #3: Adaptive circuit breaker with variable thresholds
    let circuit_breaker = {
        let degraded_since_lock = state.degraded_since.read().await;
        if let Some(degraded_start) = *degraded_since_lock {
            let degraded_duration = degraded_start.elapsed();
            let degraded_duration_secs = degraded_duration.as_secs();

            // Adaptive thresholds (Issue #3)
            // - Quick restart (30s) for transient issues
            // - Recovery period (5min) if improving
            // - Stuck threshold (10min) maximum
            let quick_restart_threshold = 30u64;
            let recovery_threshold = 300u64;
            let stuck_threshold = 600u64;

            // Estimate restart threshold (conservative: assume stuck state since we can't check improvement here)
            let restart_threshold = if degraded_duration_secs < quick_restart_threshold {
                quick_restart_threshold
            } else if degraded_duration_secs < recovery_threshold {
                // Could be recovery phase or stuck, report recovery threshold
                recovery_threshold
            } else {
                // Long degradation, definitely using stuck threshold
                stuck_threshold
            };

            let time_until_restart = if degraded_duration_secs < restart_threshold {
                Some(restart_threshold - degraded_duration_secs)
            } else {
                Some(0) // Will restart imminently
            };

            let state = if time_until_restart.unwrap_or(0) < 60 {
                "will_restart_soon"
            } else {
                "degraded"
            };

            Some(CircuitBreakerStatus {
                state: state.to_string(),
                degraded_duration_seconds: Some(degraded_duration_secs),
                time_until_restart_seconds: time_until_restart,
            })
        } else {
            // Server is healthy or just started
            if status == "healthy" {
                Some(CircuitBreakerStatus {
                    state: "healthy".to_string(),
                    degraded_duration_seconds: None,
                    time_until_restart_seconds: None,
                })
            } else {
                // Just became degraded, circuit breaker hasn't tracked it yet
                None
            }
        }
    };

    Json(HealthResponse {
        status,
        metrics,
        replay_buffer,
        kafka_status,
        websocket_status,
        alert,
        circuit_breaker,
    })
}

/// Version endpoint for deployment tracking (Issue #9: CI/CD validation)
/// Returns build metadata to verify which code version is running
pub(crate) async fn version_handler(State(state): State<ServerState>) -> impl IntoResponse {
    // Get build metadata from compile-time environment variables
    // These are set during Docker build via --build-arg
    let version = VersionInfo {
        git_sha: option_env!("GIT_COMMIT_SHA").unwrap_or("unknown"),
        build_date: option_env!("BUILD_DATE").unwrap_or("unknown"),
        schema_version: dashflow_streaming::CURRENT_SCHEMA_VERSION,
        component: "websocket-server",
        resume_namespace: state.resume_namespace.clone(),
        kafka_topic: state.kafka_topic.clone(),
        kafka_group_id: state.kafka_group_id.clone(),
        // M-1019: Expose max payload config so UI can detect mismatch
        max_payload_bytes: state.max_payload_bytes,
        // M-1020: Expose decode error policy so operators know the behavior
        decode_error_policy: state.decode_error_policy.as_str(),
    };

    Json(version)
}

/// Prometheus /metrics endpoint (Issue #11)
/// Returns metrics in Prometheus text format for scraping
pub(crate) async fn metrics_handler(State(state): State<ServerState>) -> impl IntoResponse {
    let metric_families = state.prometheus_registry.gather();
    let encoder = prometheus::TextEncoder::new();
    match encoder.encode_to_string(&metric_families) {
        Ok(text) => (StatusCode::OK, text),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to encode Prometheus metrics: {}", e),
        ),
    }
}

// ============================================================================
// Expected Schema API Handlers
// ============================================================================

/// GET /api/expected-schema - List all expected schemas
pub(crate) async fn list_expected_schemas(State(state): State<ServerState>) -> impl IntoResponse {
    let schemas = state.expected_schemas.list().await;
    Json(schemas)
}

/// GET /api/expected-schema/:graph_name - Get expected schema for a specific graph
///
/// M-487: Fixed to handle serialization errors gracefully instead of panicking.
pub(crate) async fn get_expected_schema(
    State(state): State<ServerState>,
    Path(graph_name): Path<String>,
) -> impl IntoResponse {
    match state.expected_schemas.get(&graph_name).await {
        Some(entry) => {
            // M-487: Handle serialization error instead of unwrap()
            match serde_json::to_value(entry) {
                Ok(value) => (StatusCode::OK, Json(value)),
                Err(e) => (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(
                        json!({ "error": "Failed to serialize schema", "details": e.to_string() }),
                    ),
                ),
            }
        }
        None => (
            StatusCode::NOT_FOUND,
            Json(
                json!({ "error": "No expected schema set for this graph", "graph_name": graph_name }),
            ),
        ),
    }
}

/// PUT /api/expected-schema/:graph_name - Set expected schema for a graph
pub(crate) async fn set_expected_schema(
    State(state): State<ServerState>,
    Path(graph_name): Path<String>,
    Json(request): Json<SetExpectedSchemaRequest>,
) -> impl IntoResponse {
    let entry = state.expected_schemas.set(graph_name, request).await;
    (StatusCode::OK, Json(entry))
}

/// DELETE /api/expected-schema/:graph_name - Remove expected schema for a graph
///
/// M-487: Fixed to handle serialization errors gracefully instead of panicking.
pub(crate) async fn delete_expected_schema(
    State(state): State<ServerState>,
    Path(graph_name): Path<String>,
) -> impl IntoResponse {
    match state.expected_schemas.remove(&graph_name).await {
        Some(entry) => {
            // M-487: Handle serialization error instead of unwrap()
            match serde_json::to_value(entry) {
                Ok(value) => (StatusCode::OK, Json(value)),
                Err(e) => (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(
                        json!({ "error": "Failed to serialize schema", "details": e.to_string() }),
                    ),
                ),
            }
        }
        None => (
            StatusCode::NOT_FOUND,
            Json(json!({ "error": "No expected schema to delete", "graph_name": graph_name })),
        ),
    }
}

// ============================================================================
// WebSocket Handlers
// ============================================================================

pub(crate) async fn websocket_handler(
    ws: WebSocketUpgrade,
    State(state): State<ServerState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
) -> Response {
    let client_ip = extract_client_ip(&headers, addr, state.trusted_proxy_ips.as_ref());

    // M-488: Check rate limit before accepting upgrade
    if !state.connection_rate_limiter.try_acquire(&client_ip).await {
        return (StatusCode::TOO_MANY_REQUESTS, "Too many connections from this IP").into_response();
    }

    ws.on_upgrade(move |socket| handle_socket(socket, state, client_ip))
}

pub(crate) async fn handle_socket(mut socket: WebSocket, state: ServerState, client_ip: String) {
    use std::sync::atomic::Ordering;

    println!("New WebSocket client connected from {}", client_ip);
    state
        .metrics
        .connected_clients
        .fetch_add(1, Ordering::Relaxed);
    let mut rx = state.tx.subscribe();
    let mut shutdown_rx = state.shutdown_tx.subscribe();
    let mut msg_count = 0;
    // M-682/M-773: Track lag for backpressure disconnects.
    // M-773: Use windowed semantics when slow_client_lag_window_secs > 0.
    let mut lag_in_window: u64 = 0;
    let mut window_start = Instant::now();

    loop {
        tokio::select! {
            // Listen for shutdown signal
            _ = shutdown_rx.recv() => {
                println!("   WebSocket handler received shutdown signal, closing connection gracefully...");
                // Send Close frame to client
                if let Err(e) = socket.send(Message::Close(None)).await {
                    println!("   Failed to send Close frame (client may have already disconnected): {}", e);
                } else {
                    println!("   Sent Close frame to client");
                }
                break;
            }
            // Check if socket is still connected by trying to receive control messages
            msg_result = socket.recv() => {
                match msg_result {
                    Some(Ok(Message::Close(_))) | None => {
                        println!("   Client disconnected gracefully (sent {} messages)", msg_count);
                        break;
                    }
                    Some(Ok(Message::Ping(data))) => {
                        // Respond to WebSocket native ping to keep connection alive
                        // M-1043: Use send_with_timeout to prevent wedged handler if client stops reading
                        if let Err(reason) = send_with_timeout(&mut socket, Message::Pong(data), &state.metrics).await {
                            println!("   Failed to send pong ({})", reason);
                            break;
                        }
                    }
                    Some(Ok(Message::Text(text))) => {
                        // M-1061: Reject oversized control frames before JSON parsing (DoS prevention)
                        if text.len() > WEBSOCKET_MAX_CONTROL_BYTES {
                            // Track oversized control frame rejections
                            if let Some(ref counter) = state.control_oversized_total {
                                counter.inc();
                            }
                            tracing::warn!(
                                control_frame_bytes = text.len(),
                                max_bytes = WEBSOCKET_MAX_CONTROL_BYTES,
                                "Rejecting oversized control frame (M-1061)"
                            );
                            let error_msg = serde_json::json!({
                                "type": "error",
                                "code": "CONTROL_FRAME_TOO_LARGE",
                                "message": format!(
                                    "Control message ({} bytes) exceeds maximum ({} bytes)",
                                    text.len(), WEBSOCKET_MAX_CONTROL_BYTES
                                )
                            });
                            let _ = send_with_timeout(&mut socket, Message::Text(error_msg.to_string()), &state.metrics).await;
                            break; // Disconnect potentially malicious client
                        }

                        // Handle JSON messages from client
                        if let Ok(msg) = serde_json::from_str::<serde_json::Value>(&text) {
                            match msg.get("type").and_then(|v| v.as_str()) {
                                Some("ping") => {
                                    // Issue #4: Health check ping/pong
                                    // M-1034: Use send_with_timeout to prevent wedged handler tasks
                                    println!("   Received JSON ping from client, sending pong...");
                                    let pong = r#"{"type":"pong"}"#;
                                    if let Err(reason) = send_with_timeout(&mut socket, Message::Text(pong.to_string()), &state.metrics).await {
                                        println!("   Failed to send JSON pong ({}), disconnecting", reason);
                                        break;
                                    }
                                }
                                Some("resume") => {
                                    // Issue #3 Reconnection recovery - replay missed messages
                                    // M-486: Wrap entire replay operation in timeout to prevent slow clients blocking handler
                                    // M-743: Configurable via REPLAY_TIMEOUT_SECS env var
                                    let replay_timeout_secs = get_replay_timeout_secs();
                                    // M-684: Start timer for latency tracking
                                    let replay_start = Instant::now();

                                    // M-809: Fixed - the future must NOT be .await'd before wrapping in timeout!
                                    // Previously: `handle_resume_message(...).await` was completed BEFORE timeout,
                                    // making the timeout wrap an immediate value (no effect).
                                    // Now: Pass the un-awaited future directly to timeout().
                                    match tokio::time::timeout(
                                        Duration::from_secs(replay_timeout_secs),
                                        handle_resume_message(&msg, &state, &mut socket, replay_start)
                                    ).await {
                                        Ok(Ok(())) => { /* replay completed successfully */ }
                                        Ok(Err(reason)) => {
                                            // Send error occurred, disconnect client
                                            println!("   Replay failed ({}), disconnecting client", reason);
                                            break;
                                        }
                                        Err(_timeout) => {
                                            // Timeout: slow client, disconnect to free resources
                                            // M-1034: Use send_with_timeout to prevent wedged handler tasks
                                            println!("   Replay timeout ({}s) - slow client, disconnecting", replay_timeout_secs);
                                            let timeout_msg = serde_json::json!({
                                                "type": "error",
                                                "code": "REPLAY_TIMEOUT",
                                                "message": "Replay operation timed out due to slow connection"
                                            });
                                            // Best effort: try to notify client but don't block forever
                                            let _ = send_with_timeout(&mut socket, Message::Text(timeout_msg.to_string()), &state.metrics).await;
                                            break;
                                        }
                                    }
                                }
                                Some("cursor_reset") => {
                                    // M-706: Explicit cursor reset protocol
                                    // Client requests to clear stored cursors and get current offsets
                                    // This is useful when:
                                    // - Client knows their state is corrupt
                                    // - Topic was recreated and old offsets are invalid
                                    // - Admin action to force a clean state
                                    println!("   Client requesting cursor reset");

                                    // Get latest offsets for all known partitions (before clearing)
                                    let latest_offsets = state.replay_buffer.get_latest_offsets_for_all_partitions().await;

                                    // M-746: Clear the replay buffer to prevent stale history from
                                    // reappearing when the client resumes after cursor reset.
                                    // This must happen AFTER getting latest offsets so the client
                                    // has valid offsets to resume from.
                                    let deleted_keys = state.replay_buffer.clear().await;
                                    println!("   Cleared replay buffer ({} Redis keys deleted)", deleted_keys);

                                    // M-1063: Cap response size to prevent huge JSON payloads
                                    // On high-partition topics, returning all offsets can exceed message limits
                                    const MAX_PARTITIONS_IN_RESPONSE: usize = 100;
                                    let total_partitions = latest_offsets.len();
                                    let truncated = total_partitions > MAX_PARTITIONS_IN_RESPONSE;

                                    // M-690: Encode offsets as strings for JSON
                                    // M-1063: Only include up to MAX_PARTITIONS_IN_RESPONSE partitions
                                    let offsets_json: HashMap<String, String> = latest_offsets
                                        .iter()
                                        .take(MAX_PARTITIONS_IN_RESPONSE)
                                        .map(|(p, o)| (p.to_string(), o.to_string()))
                                        .collect();

                                    // M-746: Include bufferCleared field to confirm replay buffer was invalidated
                                    // M-1063: Include truncated field and total count for large topics
                                    let reset_complete_msg = serde_json::json!({
                                        "type": "cursor_reset_complete",
                                        "latestOffsetsByPartition": offsets_json,
                                        "bufferCleared": true,
                                        "truncated": truncated,
                                        "totalPartitions": total_partitions,
                                        "message": format!(
                                            "Cursor reset complete. {} partition(s) available{}. Replay buffer cleared ({} keys). Use these offsets for 'from:cursor' resume.",
                                            total_partitions,
                                            if truncated { format!(" (showing first {}, truncated)", MAX_PARTITIONS_IN_RESPONSE) } else { String::new() },
                                            deleted_keys
                                        )
                                    });

                                    // M-1029: Use send_with_timeout to prevent hung control sends
                                    if let Err(reason) = send_with_timeout(
                                        &mut socket,
                                        Message::Text(reset_complete_msg.to_string()),
                                        &state.metrics,
                                    ).await {
                                        println!("   Failed to send cursor_reset_complete ({})", reason);
                                        break;
                                    }

                                    println!(
                                        "   Cursor reset complete: {} partition(s) with latest offsets, buffer cleared",
                                        latest_offsets.len()
                                    );
                                }
                                _ => {
                                    // Ignore other message types
                                }
                            }
                        } else {
                            // M-1062: Invalid JSON - track metric and warn.
                            // Repeated failures indicate protocol drift or malicious input.
                            if let Some(ref counter) = state.control_parse_failures_total {
                                counter.inc();
                            }
                            tracing::warn!(
                                text_len = text.len(),
                                text_preview = &text[..text.len().min(100)],
                                "Invalid JSON in control message (M-1062)"
                            );
                            // Note: We don't disconnect on single parse failures.
                            // Future enhancement: disconnect after N failures in a window.
                        }
                    }
                    Some(Err(e)) => {
                        println!("   WebSocket receive error: {}", e);
                        break;
                    }
                    _ => {
                        // Other message types from client - ignore for now
                    }
                }
            }
            // Receive broadcast messages and send to client
            broadcast_result = rx.recv() => {
                match broadcast_result {
                    Ok(outbound) => {
                        msg_count += 1;
                        // M-1002: Reduce hot-path logging from 10% to 0.1% frequency.
                        // Original: log first 3 + every 10th message (10% overhead with many clients).
                        // Now: log first 3 + every 1000th message (0.1% overhead).
                        // Use tracing::debug! so it can be filtered out entirely in production.
                        if msg_count <= 3 || msg_count % 1000 == 0 {
                            tracing::debug!(
                                msg_count = msg_count,
                                bytes = outbound.data.len(),
                                "Sending message to client"
                            );
                        }

                        // M-674: Send cursor metadata so the UI can resume by Kafka partition+offset.
                        // M-690: Encode offset as string to avoid JS precision loss for values > 2^53
                        let cursor_message = serde_json::json!({
                            "type": "cursor",
                            "partition": outbound.cursor.partition,
                            "offset": outbound.cursor.offset.to_string(),
                        });
                        // M-1015: Use send_with_timeout for broadcast sends to prevent wedged clients
                        // from blocking the handler task indefinitely.
                        if let Err(reason) = send_with_timeout(
                            &mut socket,
                            Message::Text(cursor_message.to_string()),
                            &state.metrics,
                        ).await {
                            // Send failed or timed out - client is gone or wedged
                            println!("   Broadcast cursor send failed ({})", reason);
                            break;
                        }

                        // M-995: Convert Bytes to Vec<u8> for axum 0.7 WebSocket API.
                        // Note: In axum 0.8+, Message::Binary accepts Bytes directly.
                        // M-1015: Use send_with_timeout for broadcast sends.
                        if let Err(reason) = send_with_timeout(
                            &mut socket,
                            Message::Binary(outbound.data.into()),
                            &state.metrics,
                        ).await {
                            // Send failed or timed out - client is gone or wedged
                            println!("   Broadcast binary send failed ({})", reason);
                            break;
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(n)) => {
                        // Issue #6: Enhanced client lag monitoring with severity levels
                        let severity = if n >= 100 {
                            "critical"
                        } else if n >= 10 {
                            "warning"
                        } else {
                            "info"
                        };

                        // Log with severity-based emoji
                        let emoji = match severity {
                            "critical" => "[CRITICAL]",
                            "warning" => "[WARNING]",
                            _ => "[INFO]",
                        };
                        println!("   {} Client lagged by {} messages (severity: {}), continuing...", emoji, n, severity);

                        // Track dropped messages in metrics for monitoring and alerting (atomic)
                        // M-1026: Note this counts per-client drops (N clients × M dropped = N×M counted)
                        state.metrics.record_drop(n);
                        // M-1027: Track lag events (1 per event, regardless of message count)
                        // Use this for backpressure decisions - stable regardless of client count
                        state.metrics.lag_events.fetch_add(1, Ordering::Relaxed);

                        // Issue #6: Track lag events and messages in Prometheus
                        if let Some(ref lag_events) = state.client_lag_events {
                            lag_events.with_label_values(&[severity]).inc();
                        }
                        if let Some(ref lag_messages) = state.client_lag_messages {
                            lag_messages.with_label_values(&[severity]).inc_by(n);
                        }

                        // Ordering Bug Fix (Issue #2): Send gap indicator to client
                        // This notifies the UI that messages were dropped due to slow consumption
                        let gap_message = serde_json::json!({
                            "type": "gap",
                            "count": n,
                            "severity": severity,
                            "message": format!("{} {} messages missed due to slow connection", emoji, n)
                        });

                        // M-1015: Use send_with_timeout for gap indicator to prevent wedged clients
                        // from blocking the handler task indefinitely.
                        if let Err(reason) = send_with_timeout(
                            &mut socket,
                            Message::Text(gap_message.to_string()),
                            &state.metrics,
                        ).await {
                            eprintln!("   Failed to send gap indicator ({})", reason);
                            break; // Client disconnected or wedged during gap notification
                        }

                        // M-682/M-773: Track lag for backpressure disconnect.
                        // M-773: Reset window if it has expired (leaky bucket semantics).
                        if state.slow_client_lag_window_secs > 0 {
                            let window_duration = Duration::from_secs(state.slow_client_lag_window_secs);
                            if window_start.elapsed() > window_duration {
                                // Window expired - reset the lag counter
                                lag_in_window = 0;
                                window_start = Instant::now();
                            }
                        }
                        lag_in_window += n;

                        // M-682: Check if lag exceeds threshold (backpressure)
                        // Threshold of 0 disables this feature
                        if state.slow_client_disconnect_threshold > 0
                            && lag_in_window >= state.slow_client_disconnect_threshold
                        {
                            let mode = if state.slow_client_lag_window_secs > 0 {
                                format!("within {}s window", state.slow_client_lag_window_secs)
                            } else {
                                "lifetime".to_string()
                            };
                            println!(
                                "   Client exceeded lag threshold ({} >= {}, {}), disconnecting for backpressure",
                                lag_in_window, state.slow_client_disconnect_threshold, mode
                            );

                            // Track metric
                            if let Some(ref m) = state.slow_client_disconnects {
                                m.inc();
                            }

                            // Send disconnect notification to client before closing
                            // M-1034: Use send_with_timeout to prevent wedged handler tasks
                            let disconnect_msg = serde_json::json!({
                                "type": "disconnect",
                                "reason": "backpressure",
                                "lag_in_window": lag_in_window,
                                "threshold": state.slow_client_disconnect_threshold,
                                "window_secs": state.slow_client_lag_window_secs,
                                "message": format!(
                                    "Disconnected due to slow consumption ({} messages dropped {}). Please reconnect using the resume protocol.",
                                    lag_in_window, mode
                                )
                            });
                            // Best effort: try to notify client but don't block forever
                            let _ = send_with_timeout(&mut socket, Message::Text(disconnect_msg.to_string()), &state.metrics).await;
                            break;
                        }

                        // Client is slow but still connected, keep trying
                    }
                    Err(e) => {
                        println!("   Broadcast receive error: {}", e);
                        break;
                    }
                }
            }
        }
    }

    // M-488: Release rate limit slot on disconnect
    state.connection_rate_limiter.release(&client_ip).await;

    state
        .metrics
        .connected_clients
        .fetch_sub(1, Ordering::Relaxed);
    println!(
        "WebSocket client from {} disconnected (sent {} messages)",
        client_ip, msg_count
    );
}

/// Handle the "resume" message from client to replay missed messages.
/// Returns Ok(()) on success, Err(reason) if send failed.
async fn handle_resume_message(
    msg: &serde_json::Value,
    state: &ServerState,
    socket: &mut WebSocket,
    replay_start: Instant,
) -> Result<(), &'static str> {
    // M-674: Prefer Kafka partition+offset cursors for resume.
    // This supports catching up threads that started while the UI was offline.
    let mut last_offsets_by_partition: HashMap<i32, i64> = HashMap::new();
    // M-1012: Track parsing errors for telemetry
    let mut parse_errors: Vec<String> = Vec::new();

    if let Some(obj) = msg
        .get("lastOffsetsByPartition")
        .and_then(|v| v.as_object())
    {
        // M-1008: Validate map size to prevent DoS
        if obj.len() > MAX_RESUME_PARTITIONS {
            tracing::warn!(
                count = obj.len(),
                max = MAX_RESUME_PARTITIONS,
                "Resume request exceeded max partitions, rejecting"
            );
            let error_msg = serde_json::json!({
                "type": "error",
                "code": "RESUME_TOO_MANY_PARTITIONS",
                "message": format!("Too many partitions in resume request: {} > {}", obj.len(), MAX_RESUME_PARTITIONS)
            });
            // M-1044: Use send_with_timeout for consistency with M-1034 (all control-plane sends)
            let _ = send_with_timeout(socket, Message::Text(error_msg.to_string()), &state.metrics).await;
            return Err("too_many_partitions");
        }

        for (partition_str, v) in obj {
            // M-1008: Validate partition is a non-negative integer
            let partition = match partition_str.parse::<i32>() {
                Ok(p) if p >= 0 => p,
                Ok(p) => {
                    // M-1012: Log negative partition
                    tracing::warn!(partition = p, "Resume request has negative partition, skipping");
                    parse_errors.push(format!("negative_partition:{}", p));
                    continue;
                }
                Err(_) => {
                    // M-1012: Log invalid partition key
                    tracing::warn!(partition_str = %partition_str, "Resume request has non-numeric partition key, skipping");
                    // M-1117: Truncate user-provided string to prevent unbounded memory
                    parse_errors.push(format!("invalid_partition:{}", truncate_for_error(partition_str)));
                    continue;
                }
            };

            // M-690: Accept offsets as either JSON numbers or strings.
            let offset = if let Some(offset) = v.as_i64() {
                offset
            } else if let Some(offset_str) = v.as_str() {
                match offset_str.parse::<i64>() {
                    Ok(o) => o,
                    Err(_) => {
                        // M-1012: Log invalid offset
                        tracing::warn!(
                            partition = partition,
                            offset_str = %offset_str,
                            "Resume request has non-numeric offset, skipping"
                        );
                        // M-1117: Truncate user-provided string to prevent unbounded memory
                        parse_errors.push(format!("invalid_offset:p{}:{}", partition, truncate_for_error(offset_str)));
                        continue;
                    }
                }
            } else {
                // M-1012: Log unexpected offset type
                tracing::warn!(
                    partition = partition,
                    "Resume request has non-string/non-numeric offset type, skipping"
                );
                parse_errors.push(format!("invalid_offset_type:p{}", partition));
                continue;
            };

            // M-1008: Validate offset is non-negative
            if offset < 0 {
                tracing::warn!(
                    partition = partition,
                    offset = offset,
                    "Resume request has negative offset, skipping"
                );
                parse_errors.push(format!("negative_offset:p{}:{}", partition, offset));
                continue;
            }
            last_offsets_by_partition.insert(partition, offset);
        }
    }

    // M-1012: Log parse error summary if any
    if !parse_errors.is_empty() {
        tracing::info!(
            error_count = parse_errors.len(),
            errors = ?parse_errors,
            "Resume request had parsing errors (fields skipped)"
        );
    }

    // M-765: Support explicit mode selection via "mode" field.
    // This allows clients to explicitly request thread mode even when they
    // have partition offsets (e.g., during migration or when partition mode
    // failed). Modes:
    // - "partition": use lastOffsetsByPartition (explicit)
    // - "thread": use lastSequencesByThread (explicit)
    // - "auto" or absent: implicit selection based on field presence (backwards compatible)
    //
    // M-676: Support resume even if client has no offsets yet (first connect),
    // as long as client indicates partition mode (by sending empty lastOffsetsByPartition)
    let explicit_mode = msg.get("mode").and_then(|v| v.as_str());
    let use_partition_mode = match explicit_mode {
        Some("partition") => true,
        Some("thread") => false,
        _ => msg.get("lastOffsetsByPartition").is_some(), // Backwards compatible
    };

    // M-703: Parse explicit resume strategy.
    // - "latest": start from latest offsets, no replay (ideal for first connect)
    // - "earliest": replay from earliest retained offsets
    // - "cursor" (default): use provided offsets, discover missing partitions
    // M-1008: Validate `from` parameter explicitly
    let resume_from_raw = msg.get("from").and_then(|v| v.as_str());
    let resume_from = match resume_from_raw {
        Some("latest") => "latest",
        Some("earliest") => "earliest",
        Some("cursor") => "cursor",
        None => "cursor", // Default
        Some(unknown) => {
            // M-1012: Log invalid `from` value
            tracing::warn!(
                from_value = %unknown,
                "Resume request has invalid 'from' value, defaulting to 'cursor'"
            );
            "cursor"
        }
    };

    if use_partition_mode {
        handle_partition_mode_resume(msg, state, socket, replay_start, &last_offsets_by_partition, resume_from).await
    } else {
        handle_thread_mode_resume(msg, state, socket, replay_start).await
    }
}

/// Handle partition-mode resume (M-674)
async fn handle_partition_mode_resume(
    _msg: &serde_json::Value,
    state: &ServerState,
    socket: &mut WebSocket,
    replay_start: Instant,
    last_offsets_by_partition: &HashMap<i32, i64>,
    resume_from: &str,
) -> Result<(), &'static str> {
    // M-684: Record partition mode resume request
    if let Some(ref m) = state.resume_requests_total {
        m.with_label_values(&["partition"]).inc();
    }
    println!(
        "   Client requesting resume by partition offsets for {} known partition(s), from={}",
        last_offsets_by_partition.len(),
        resume_from
    );

    // M-679: Check for stale cursors before replay
    // If client's cursor is older than our oldest retained data, warn them
    let stale_cursors = state
        .replay_buffer
        .check_for_stale_cursors(last_offsets_by_partition)
        .await;

    for (partition, requested, oldest) in &stale_cursors {
        println!(
            "   Stale cursor detected for partition={}: client requested offset {}, oldest retained is {}",
            partition, requested, oldest
        );
        // M-690: Encode offsets as strings to avoid JS precision loss
        let stale_message = serde_json::json!({
            "type": "cursor_stale",
            "partition": partition,
            "requested": requested.to_string(),
            "oldest": oldest.to_string(),
            "message": "Cursor is stale; data may have been evicted. State may be incomplete."
        });
        // M-1009: Use timed send
        if let Err(reason) = send_with_timeout(
            socket,
            Message::Text(stale_message.to_string()),
            &state.metrics,
        )
        .await
        {
            eprintln!("   Failed to send cursor_stale indicator: {}", reason);
            return Err("cursor_stale_failed");
        }
    }

    // M-703: Handle "from:latest" mode - skip replay, start from current position
    if resume_from == "latest" {
        let latest_offsets = state
            .replay_buffer
            .get_latest_offsets_for_all_partitions()
            .await;

        println!(
            "   from=latest mode: skipping replay, starting from latest offsets ({} partition(s))",
            latest_offsets.len()
        );

        // M-690: Encode offsets as strings for JS precision safety
        let final_offsets_json: HashMap<String, String> = latest_offsets
            .iter()
            .map(|(p, o)| (p.to_string(), o.to_string()))
            .collect();

        // M-748: Use consistent "totalReplayed" field name (matches cursor-mode replay_complete)
        let replay_complete_message = serde_json::json!({
            "type": "replay_complete",
            "lastOffsetsByPartition": final_offsets_json,
            "capped": false,
            "totalReplayed": 0,
            "mode": "latest"
        });
        // M-1009: Use timed send
        if let Err(reason) = send_with_timeout(
            socket,
            Message::Text(replay_complete_message.to_string()),
            &state.metrics,
        )
        .await
        {
            eprintln!("   Failed to send replay_complete: {}", reason);
            return Err("replay_complete_failed");
        }

        // We already sent replay_complete for from=latest; skip replay loop.
        return Ok(());
    }

    // M-764: Handle "from:earliest" mode - replay from earliest retained offsets
    // Get all known partitions and set offsets to -1 to replay everything retained.
    let current_offsets = if resume_from == "earliest" {
        println!("   from=earliest mode: replaying from earliest retained offsets");
        let known_partitions = state
            .replay_buffer
            .get_known_partitions()
            .await;
        known_partitions.into_iter().map(|p| (p, -1i64)).collect()
    } else {
        // Default "cursor" mode: use client-provided offsets
        last_offsets_by_partition.clone()
    };

    // M-676: Replay loop with paging and partition discovery
    // M-698: REPLAY_MAX_TOTAL is a global safety cap across all partitions
    // The per-partition limit is defined in ReplayBuffer::REDIS_PARTITION_PAGE_LIMIT
    // M-743: Configurable via REPLAY_MAX_TOTAL env var
    let replay_max_total = get_replay_max_total();
    let mut total_replayed: usize = 0;
    let mut current_offsets = current_offsets;
    let mut final_offsets: HashMap<i32, i64> = HashMap::new();
    let mut replay_capped = false; // M-692: Track if we hit the safety cap

    loop {
        // M-698: Now returns truncated_partitions to track which partitions have more data
        let (replay_messages, gaps, new_partitions, truncated_partitions) = state
            .replay_buffer
            .get_messages_after_by_partition(&current_offsets)
            .await;

        // M-676 Issue A: Log newly discovered partitions
        if !new_partitions.is_empty() {
            println!(
                "   Discovered {} new partition(s) client hadn't seen: {:?}",
                new_partitions.len(),
                new_partitions
            );
        }

        for (partition, gap_size) in &gaps {
            // M-684: Record partition mode gaps
            if let Some(ref m) = state.replay_gaps_total {
                m.with_label_values(&["partition"]).inc();
            }
            // M-782: Record actual missing message count
            if let Some(ref m) = state.replay_gap_messages_total {
                m.with_label_values(&["partition"]).inc_by(*gap_size as u64);
            }
            println!(
                "   Resume gap detected for partition={} (missing {} message(s)), sending gap indicator",
                partition, gap_size
            );
            let gap_message = serde_json::json!({
                "type": "gap",
                "count": gap_size,
                "partition": partition,
                "severity": "warning",
                "message": "Replay gap detected; some messages may be lost"
            });
            // M-1009: Use timed send
            if let Err(reason) = send_with_timeout(
                socket,
                Message::Text(gap_message.to_string()),
                &state.metrics,
            )
            .await
            {
                eprintln!("   Failed to send gap indicator: {}", reason);
                return Err("gap_indicator_failed");
            }
        }

        if replay_messages.is_empty() {
            println!(
                "   No more messages to replay (partition cursor; client is up to date)"
            );
            break;
        }

        let page_count = replay_messages.len();
        println!(
            "   Replaying {} messages (total so far: {})",
            page_count, total_replayed
        );

        for (i, outbound) in replay_messages.iter().enumerate() {
            // M-690: Encode offset as string to avoid JS precision loss for values > 2^53
            let cursor_message = serde_json::json!({
                "type": "cursor",
                "partition": outbound.cursor.partition,
                "offset": outbound.cursor.offset.to_string(),
            });
            // M-1009: Use timed send for cursor
            if let Err(reason) = send_with_timeout(
                socket,
                Message::Text(cursor_message.to_string()),
                &state.metrics,
            )
            .await
            {
                println!(
                    "   Failed to replay cursor {}/{}: {}",
                    i + 1, page_count, reason
                );
                return Err("replay_send_failed");
            }

            // M-995: Clone Bytes (O(1)) then convert to Vec<u8> for axum 0.7.
            // The clone is needed because we still use outbound.cursor below.
            // M-1009: Use timed send for binary
            if let Err(reason) = send_with_timeout(
                socket,
                Message::Binary(outbound.data.clone().into()),
                &state.metrics,
            )
            .await
            {
                println!(
                    "   Failed to replay message {}/{}: {}",
                    i + 1, page_count, reason
                );
                return Err("replay_send_failed");
            }

            // Track final offset per partition for replay_complete
            let entry = final_offsets.entry(outbound.cursor.partition).or_insert(-1);
            if outbound.cursor.offset > *entry {
                *entry = outbound.cursor.offset;
            }

            // Update cursor for next page
            current_offsets.insert(outbound.cursor.partition, outbound.cursor.offset);
        }

        total_replayed += page_count;
        // M-684: Record partition mode replayed messages
        if let Some(ref m) = state.replay_messages_total {
            m.with_label_values(&["partition"]).inc_by(page_count as u64);
        }

        // M-698: Check if any partition hit the per-partition limit (has more data)
        // This is the correct check instead of comparing total page_count
        // against a single-partition limit (which was incorrect with multiple partitions)
        if truncated_partitions.is_empty() {
            // No partitions were truncated - we've sent all available data
            println!("   All partitions fully replayed");
            break;
        }

        // M-743: Check global safety cap
        if total_replayed >= replay_max_total {
            replay_capped = true;
            println!(
                "   Reached REPLAY_MAX_TOTAL={} safety cap; {} partition(s) may have more data",
                replay_max_total,
                truncated_partitions.len()
            );
            break;
        }

        // Some partitions have more data - continue to next page
        println!(
            "   {} partition(s) have more data, fetching next page...",
            truncated_partitions.len()
        );
    }

    println!(
        "   Replay complete: {} messages total",
        total_replayed
    );

    // M-692: Send replay_complete signal with final offsets
    let final_offsets_json: serde_json::Map<String, serde_json::Value> = final_offsets
        .iter()
        .map(|(k, v)| (k.to_string(), serde_json::json!(v.to_string())))
        .collect();
    let replay_complete = serde_json::json!({
        "type": "replay_complete",
        "totalReplayed": total_replayed,
        "lastOffsetsByPartition": final_offsets_json,
        "capped": replay_capped, // M-692: Explicitly signal if replay was truncated
        "mode": resume_from, // M-764: "cursor" or "earliest"
    });
    // M-1009: Use timed send
    if let Err(reason) = send_with_timeout(
        socket,
        Message::Text(replay_complete.to_string()),
        &state.metrics,
    )
    .await
    {
        eprintln!("   Failed to send replay_complete: {}", reason);
        return Err("replay_complete_failed");
    }
    println!(
        "   Sent replay_complete (total: {} messages)",
        total_replayed
    );

    // M-684: Record partition mode replay latency
    if let Some(ref h) = state.replay_latency_histogram {
        h.with_label_values(&["partition"]).observe(replay_start.elapsed().as_millis() as f64);
    }
    Ok(())
}

/// Handle thread-mode resume (legacy)
async fn handle_thread_mode_resume(
    msg: &serde_json::Value,
    state: &ServerState,
    socket: &mut WebSocket,
    replay_start: Instant,
) -> Result<(), &'static str> {
    let mut last_sequences_by_thread: HashMap<String, u64> = HashMap::new();
    // M-1012: Track parsing errors for telemetry
    let mut parse_errors: Vec<String> = Vec::new();

    if let Some(obj) = msg
        .get("lastSequencesByThread")
        .and_then(|v| v.as_object())
    {
        // M-1008: Validate map size to prevent DoS
        if obj.len() > MAX_RESUME_THREADS {
            tracing::warn!(
                count = obj.len(),
                max = MAX_RESUME_THREADS,
                "Resume request exceeded max threads, rejecting"
            );
            let error_msg = serde_json::json!({
                "type": "error",
                "code": "RESUME_TOO_MANY_THREADS",
                "message": format!("Too many threads in resume request: {} > {}", obj.len(), MAX_RESUME_THREADS)
            });
            // M-1044: Use send_with_timeout for consistency with M-1034 (all control-plane sends)
            let _ = send_with_timeout(socket, Message::Text(error_msg.to_string()), &state.metrics).await;
            return Err("too_many_threads");
        }

        for (thread_id, v) in obj {
            // M-762: Accept both JSON numbers and numeric strings.
            // This mirrors offset parsing (M-690) and supports UI which
            // stores sequences as strings after M-693.
            // Also allow seq=0 as valid cursor (M-751).
            let seq = if let Some(seq) = v.as_u64() {
                seq
            } else if let Some(seq_str) = v.as_str() {
                match seq_str.parse::<u64>() {
                    Ok(s) => s,
                    Err(_) => {
                        // M-1012: Log invalid sequence
                        tracing::warn!(
                            thread_id = %thread_id,
                            seq_str = %seq_str,
                            "Resume request has non-numeric sequence, skipping"
                        );
                        // M-1117: Truncate user-provided strings to prevent unbounded memory
                        parse_errors.push(format!("invalid_seq:{}:{}", truncate_for_error(thread_id), truncate_for_error(seq_str)));
                        continue;
                    }
                }
            } else {
                // M-1012: Log unexpected sequence type
                tracing::warn!(
                    thread_id = %thread_id,
                    "Resume request has non-string/non-numeric sequence type, skipping"
                );
                // M-1117: Truncate user-provided string to prevent unbounded memory
                parse_errors.push(format!("invalid_seq_type:{}", truncate_for_error(thread_id)));
                continue;
            };
            // Note: seq=0 is now allowed (replay "after 0" is meaningful)
            last_sequences_by_thread.insert(thread_id.clone(), seq);
        }
    } else if let (Some(thread_id), Some(last_sequence)) = (
        msg.get("threadId")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
        // M-762: Also support string for legacy single-thread mode
        msg.get("lastSequence").and_then(|v| {
            v.as_u64().or_else(|| {
                v.as_str().and_then(|s| s.parse::<u64>().ok())
            })
        }),
    ) {
        // Note: lastSequence=0 is now allowed (M-762)
        last_sequences_by_thread.insert(thread_id, last_sequence);
    }

    // M-1012: Log parse error summary if any
    if !parse_errors.is_empty() {
        tracing::info!(
            error_count = parse_errors.len(),
            errors = ?parse_errors,
            "Thread-mode resume request had parsing errors (fields skipped)"
        );
    }

    if last_sequences_by_thread.is_empty() {
        println!(
            "   Resume request missing lastSequencesByThread (or empty); nothing to replay"
        );
        return Ok(());
    }

    // M-684: Record thread mode resume request
    if let Some(ref m) = state.resume_requests_total {
        m.with_label_values(&["thread"]).inc();
    }
    println!(
        "   Client requesting resume for {} thread(s)",
        last_sequences_by_thread.len()
    );

    let (replay_messages, gaps) = state
        .replay_buffer
        .get_messages_after_by_thread(&last_sequences_by_thread)
        .await;

    for (thread_id, gap_size) in gaps {
        // M-684: Record thread mode gaps
        if let Some(ref m) = state.replay_gaps_total {
            m.with_label_values(&["thread"]).inc();
        }
        // M-782: Record actual missing message count
        if let Some(ref m) = state.replay_gap_messages_total {
            m.with_label_values(&["thread"]).inc_by(gap_size);
        }
        println!(
            "   Resume gap detected for thread_id={} (missing {} message(s)), sending gap indicator",
            thread_id, gap_size
        );
        let gap_message = serde_json::json!({
            "type": "gap",
            "count": gap_size,
            "threadId": thread_id,
            "severity": "warning",
            "message": "Replay gap detected; some messages may be lost"
        });
        // M-1009: Use timed send
        if let Err(reason) = send_with_timeout(
            socket,
            Message::Text(gap_message.to_string()),
            &state.metrics,
        )
        .await
        {
            eprintln!("   Failed to send gap indicator: {}", reason);
            return Err("gap_indicator_failed");
        }
    }

    let total_replayed = replay_messages.len();
    if !replay_messages.is_empty() {
        println!(
            "   Replaying {} missed messages to client",
            total_replayed
        );
        // M-766: Thread-mode replay now sends cursor metadata before
        // each binary frame, matching partition-mode behavior.
        // This ensures cursor pairing (M-720) works correctly.
        for (i, outbound) in replay_messages.iter().enumerate() {
            // M-766: Send cursor JSON before binary (same as partition mode)
            // M-690: Encode offset as string to avoid JS precision loss
            let cursor_message = serde_json::json!({
                "type": "cursor",
                "partition": outbound.cursor.partition,
                "offset": outbound.cursor.offset.to_string(),
            });
            // M-1009: Use timed send for cursor
            if let Err(reason) = send_with_timeout(
                socket,
                Message::Text(cursor_message.to_string()),
                &state.metrics,
            )
            .await
            {
                println!(
                    "   Failed to replay cursor {}/{}: {}",
                    i + 1,
                    total_replayed,
                    reason
                );
                return Err("replay_send_failed");
            }

            // M-995: Clone Bytes (O(1)) then convert to Vec<u8> for axum 0.7.
            // M-1009: Use timed send for binary
            if let Err(reason) = send_with_timeout(
                socket,
                Message::Binary(outbound.data.clone().into()),
                &state.metrics,
            )
            .await
            {
                println!(
                    "   Failed to replay message {}/{}: {}",
                    i + 1,
                    total_replayed,
                    reason
                );
                return Err("replay_send_failed");
            }
        }
        // M-684: Record thread mode replayed messages
        if let Some(ref m) = state.replay_messages_total {
            m.with_label_values(&["thread"]).inc_by(total_replayed as u64);
        }
        println!(
            "   Successfully replayed {} messages",
            total_replayed
        );
    } else {
        println!("   No messages to replay (client is up to date)");
    }

    // M-810: Send replay_complete for thread-mode resume (consistent with partition mode).
    // This allows UI to know when replay is finished and switch to live processing.
    let replay_complete = serde_json::json!({
        "type": "replay_complete",
        "totalReplayed": total_replayed,
        "mode": "thread",
    });
    // M-1009: Use timed send
    if let Err(reason) = send_with_timeout(
        socket,
        Message::Text(replay_complete.to_string()),
        &state.metrics,
    )
    .await
    {
        eprintln!("   Failed to send thread-mode replay_complete: {}", reason);
        return Err("replay_complete_failed");
    }
    println!(
        "   Sent thread-mode replay_complete (total: {} messages)",
        total_replayed
    );

    // M-684: Record thread mode replay latency
    if let Some(ref h) = state.replay_latency_histogram {
        h.with_label_values(&["thread"]).observe(replay_start.elapsed().as_millis() as f64);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn truncate_for_error_returns_short_strings_unchanged() {
        assert_eq!(truncate_for_error("hello"), "hello");
        assert_eq!(truncate_for_error(""), "");
        // Exactly at boundary
        let at_limit = "a".repeat(MAX_PARSE_ERROR_VALUE_LEN);
        assert_eq!(truncate_for_error(&at_limit), at_limit);
    }

    #[test]
    fn truncate_for_error_truncates_long_strings_with_size_suffix() {
        let long_str = "x".repeat(200);
        let truncated = truncate_for_error(&long_str);

        // Should contain the first MAX_PARSE_ERROR_VALUE_LEN characters
        assert!(truncated.starts_with(&"x".repeat(MAX_PARSE_ERROR_VALUE_LEN)));
        // Should end with size indicator
        assert!(truncated.ends_with("(200b)"));
        assert!(truncated.contains("..."));
    }

    #[test]
    fn truncate_for_error_handles_unicode_boundary() {
        // String with multi-byte characters
        let unicode_str = "🔥".repeat(50); // Each emoji is 4 bytes
        // This should not panic on character boundary
        let result = truncate_for_error(&unicode_str);
        // Original is 200 bytes, which exceeds limit
        assert!(result.len() < unicode_str.len() + 20); // Some overhead for suffix
    }
}
