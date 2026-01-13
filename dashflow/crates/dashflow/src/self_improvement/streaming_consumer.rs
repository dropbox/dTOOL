// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

//! Streaming Consumer for Self-Improvement Daemon.
//!
//! This module connects the DashFlow streaming system to self-improvement,
//! allowing real-time analysis of metrics and events from Kafka.
//!
//! ## Status: Library-Only
//!
//! **This module is NOT currently wired to the CLI.** The streaming consumer
//! is available as a library for programmatic use, but the CLI daemon command
//! does not yet support `--streaming` or `--kafka` flags.
//!
//! ## Design
//!
//! Instead of watching `.dashflow/traces/` for files, the daemon can consume
//! `DashStreamMessage` from Kafka for real-time analysis:
//!
//! - **Metrics messages**: Extract quality scores, latencies, error rates
//! - **Event messages**: Track node execution times, errors, retries
//! - **Traces**: Convert streaming events into execution traces
//!
//! ## Programmatic Usage
//!
//! ```rust,ignore
//! use dashflow::self_improvement::streaming_consumer::{
//!     StreamingConsumerConfig, StreamingConsumer,
//! };
//!
//! // Using builder pattern (recommended)
//! let config = StreamingConsumerConfig::new()
//!     .with_bootstrap_servers("localhost:9092")
//!     .with_topic("dashstream-metrics")
//!     .with_consumer_group("self-improvement")
//!     .with_slow_node_threshold_ms(5000)
//!     .with_error_rate_threshold(0.10);
//!
//! // Validate configuration
//! let errors = config.validate();
//! if !errors.is_empty() {
//!     panic!("Invalid config: {:?}", errors);
//! }
//!
//! let consumer = StreamingConsumer::new(config);
//! // Use consumer.start() to begin consuming messages
//! ```

use crate::constants::{DEFAULT_MAX_NODE_DURATION_SAMPLES, DEFAULT_MAX_QUALITY_SCORE_SAMPLES};
// SHORT_TIMEOUT is only used when dashstream feature is enabled
#[cfg(feature = "dashstream")]
use crate::constants::SHORT_TIMEOUT;
use crate::self_improvement::daemon::{AnalysisTriggerType, FiredTrigger};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tokio::sync::mpsc;
use uuid::Uuid;

// These are only used when dashstream feature is enabled
#[cfg(feature = "dashstream")]
use std::sync::Arc;
#[cfg(feature = "dashstream")]
use std::time::Duration;

/// Configuration for streaming-based self-improvement analysis.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamingConsumerConfig {
    /// Kafka bootstrap servers (e.g., "localhost:9092")
    pub bootstrap_servers: String,

    /// Kafka topic to consume from
    pub topic: String,

    /// Consumer group ID
    pub consumer_group: String,

    /// Slow node threshold in milliseconds
    pub slow_node_threshold_ms: u64,

    /// Error rate threshold (0.0 - 1.0)
    pub error_rate_threshold: f64,

    /// Retry threshold (number of retries before triggering)
    pub retry_threshold: usize,

    /// Window size for aggregating metrics (seconds)
    pub aggregation_window_secs: u64,
}

impl Default for StreamingConsumerConfig {
    fn default() -> Self {
        Self {
            bootstrap_servers: "localhost:9092".to_string(),
            topic: "dashstream-metrics".to_string(),
            consumer_group: "dashflow-self-improvement".to_string(),
            slow_node_threshold_ms: 10_000, // 10 seconds
            error_rate_threshold: 0.05,     // 5%
            retry_threshold: 3,
            aggregation_window_secs: 60, // 1 minute window
        }
    }
}

impl StreamingConsumerConfig {
    /// Create a new config with default values.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Builder: set Kafka bootstrap servers.
    #[must_use]
    pub fn with_bootstrap_servers(mut self, servers: impl Into<String>) -> Self {
        self.bootstrap_servers = servers.into();
        self
    }

    /// Builder: set Kafka topic.
    #[must_use]
    pub fn with_topic(mut self, topic: impl Into<String>) -> Self {
        self.topic = topic.into();
        self
    }

    /// Builder: set consumer group ID.
    #[must_use]
    pub fn with_consumer_group(mut self, group: impl Into<String>) -> Self {
        self.consumer_group = group.into();
        self
    }

    /// Builder: set slow node threshold in milliseconds.
    #[must_use]
    pub fn with_slow_node_threshold_ms(mut self, threshold: u64) -> Self {
        self.slow_node_threshold_ms = threshold;
        self
    }

    /// Builder: set error rate threshold (0.0 - 1.0).
    ///
    /// Values outside this range will be clamped.
    #[must_use]
    pub fn with_error_rate_threshold(mut self, threshold: f64) -> Self {
        self.error_rate_threshold = threshold.clamp(0.0, 1.0);
        self
    }

    /// Builder: set retry threshold.
    #[must_use]
    pub fn with_retry_threshold(mut self, threshold: usize) -> Self {
        self.retry_threshold = threshold;
        self
    }

    /// Builder: set aggregation window in seconds.
    #[must_use]
    pub fn with_aggregation_window_secs(mut self, secs: u64) -> Self {
        self.aggregation_window_secs = secs;
        self
    }

    /// Validate the configuration.
    ///
    /// Returns a list of validation errors, or empty if valid.
    #[must_use]
    pub fn validate(&self) -> Vec<String> {
        let mut errors = Vec::new();

        if self.bootstrap_servers.is_empty() {
            errors.push("bootstrap_servers cannot be empty".to_string());
        }

        if self.topic.is_empty() {
            errors.push("topic cannot be empty".to_string());
        }

        if self.consumer_group.is_empty() {
            errors.push("consumer_group cannot be empty".to_string());
        }

        if !(0.0..=1.0).contains(&self.error_rate_threshold) {
            errors.push(format!(
                "error_rate_threshold must be between 0.0 and 1.0, got {}",
                self.error_rate_threshold
            ));
        }

        if self.aggregation_window_secs == 0 {
            errors.push("aggregation_window_secs must be > 0".to_string());
        }

        errors
    }
}

/// Aggregated metrics from streaming messages within a time window.
#[derive(Debug, Clone, Default)]
pub struct StreamingMetricsWindow {
    /// Node execution durations (node_id -> durations in ms)
    pub node_durations: HashMap<String, Vec<u64>>,

    /// Error counts per node/operation
    pub error_counts: HashMap<String, usize>,

    /// Success counts per node/operation
    pub success_counts: HashMap<String, usize>,

    /// Retry counts per operation
    pub retry_counts: HashMap<String, usize>,

    /// Quality scores received
    pub quality_scores: Vec<f64>,

    /// Total messages processed in this window
    pub message_count: usize,

    /// Window start timestamp
    pub window_start: Option<chrono::DateTime<Utc>>,
}

impl StreamingMetricsWindow {
    /// Create a new empty metrics window.
    #[must_use]
    pub fn new() -> Self {
        Self {
            window_start: Some(Utc::now()),
            ..Default::default()
        }
    }

    /// Record a node execution duration.
    pub fn record_node_duration(&mut self, node_id: &str, duration_ms: u64) {
        let durations = self.node_durations.entry(node_id.to_string()).or_default();
        durations.push(duration_ms);
        if durations.len() > DEFAULT_MAX_NODE_DURATION_SAMPLES {
            // Keep the largest samples to preserve slow-node signal while bounding memory.
            // Order isn't meaningful for analysis, so `swap_remove` is fine.
            if let Some((min_index, _)) = durations.iter().enumerate().min_by_key(|(_, v)| *v) {
                durations.swap_remove(min_index);
            }
        }
        self.message_count += 1;
    }

    /// Record an error for a node/operation.
    pub fn record_error(&mut self, operation: &str) {
        *self.error_counts.entry(operation.to_string()).or_default() += 1;
        self.message_count += 1;
    }

    /// Record a success for a node/operation.
    pub fn record_success(&mut self, operation: &str) {
        *self
            .success_counts
            .entry(operation.to_string())
            .or_default() += 1;
        self.message_count += 1;
    }

    /// Record a retry for an operation.
    pub fn record_retry(&mut self, operation: &str) {
        *self.retry_counts.entry(operation.to_string()).or_default() += 1;
        self.message_count += 1;
    }

    /// Record a quality score.
    pub fn record_quality_score(&mut self, score: f64) {
        self.quality_scores.push(score);
        if self.quality_scores.len() > DEFAULT_MAX_QUALITY_SCORE_SAMPLES {
            let excess = self.quality_scores.len() - DEFAULT_MAX_QUALITY_SCORE_SAMPLES;
            self.quality_scores.drain(0..excess);
        }
        self.message_count += 1;
    }

    /// Calculate error rate across all operations.
    #[must_use]
    pub fn calculate_error_rate(&self) -> f64 {
        let total_errors: usize = self.error_counts.values().sum();
        let total_successes: usize = self.success_counts.values().sum();
        let total = total_errors + total_successes;

        if total == 0 {
            0.0
        } else {
            total_errors as f64 / total as f64
        }
    }

    /// Get average quality score.
    #[must_use]
    pub fn average_quality_score(&self) -> Option<f64> {
        if self.quality_scores.is_empty() {
            None
        } else {
            Some(self.quality_scores.iter().sum::<f64>() / self.quality_scores.len() as f64)
        }
    }

    /// Analyze the window and generate triggers.
    #[must_use]
    pub fn analyze(&self, config: &StreamingConsumerConfig) -> Vec<FiredTrigger> {
        let mut triggers = Vec::new();

        // Check for slow nodes
        for (node_id, durations) in &self.node_durations {
            for &duration_ms in durations {
                if duration_ms > config.slow_node_threshold_ms {
                    triggers.push(FiredTrigger {
                        id: Uuid::new_v4(),
                        trigger_type: AnalysisTriggerType::SlowNode {
                            node_name: node_id.clone(),
                            duration_ms,
                            threshold_ms: config.slow_node_threshold_ms,
                        },
                        fired_at: Utc::now(),
                        trace_ids: Vec::new(),
                        processed: false,
                    });
                }
            }
        }

        // Check error rate
        let error_rate = self.calculate_error_rate();
        let total_samples =
            self.error_counts.values().sum::<usize>() + self.success_counts.values().sum::<usize>();

        if error_rate > config.error_rate_threshold && total_samples >= 10 {
            triggers.push(FiredTrigger {
                id: Uuid::new_v4(),
                trigger_type: AnalysisTriggerType::HighErrorRate {
                    error_rate,
                    threshold: config.error_rate_threshold,
                    sample_count: total_samples,
                },
                fired_at: Utc::now(),
                trace_ids: Vec::new(),
                processed: false,
            });
        }

        // Check retry counts
        for (operation, &count) in &self.retry_counts {
            if count > config.retry_threshold {
                triggers.push(FiredTrigger {
                    id: Uuid::new_v4(),
                    trigger_type: AnalysisTriggerType::RepeatedRetry {
                        operation: operation.clone(),
                        retry_count: count,
                        threshold: config.retry_threshold,
                    },
                    fired_at: Utc::now(),
                    trace_ids: Vec::new(),
                    processed: false,
                });
            }
        }

        triggers
    }

    /// Reset the window for a new aggregation period.
    pub fn reset(&mut self) {
        self.node_durations.clear();
        self.error_counts.clear();
        self.success_counts.clear();
        self.retry_counts.clear();
        self.quality_scores.clear();
        self.message_count = 0;
        self.window_start = Some(Utc::now());
    }
}

/// Message types that can be received from streaming.
/// This mirrors the DashStreamMessage variants for processing without requiring
/// the full dashflow-streaming dependency at compile time.
#[derive(Debug, Clone)]
pub enum StreamingMessage {
    /// Node execution completed.
    NodeExecution {
        /// ID of the node that was executed.
        node_id: String,
        /// Duration of the execution in milliseconds.
        duration_ms: u64,
        /// Whether the execution was successful.
        success: bool,
    },

    /// Metrics message containing aggregated data.
    Metrics {
        /// Scope type (e.g., "graph", "node", "session").
        scope: String,
        /// ID of the scoped entity.
        scope_id: String,
        /// Map of metric names to their values.
        metrics: HashMap<String, f64>,
    },

    /// Error occurred during execution.
    Error {
        /// Operation that failed.
        operation: String,
        /// Error message describing the failure.
        message: String,
    },

    /// Retry occurred for an operation.
    Retry {
        /// Operation that was retried.
        operation: String,
        /// Retry attempt number.
        attempt: usize,
    },
}

/// Consumer that processes streaming messages for self-improvement analysis.
///
/// This is the bridge between `dashflow-streaming` and the self-improvement daemon.
pub struct SelfImprovementConsumer {
    config: StreamingConsumerConfig,
    metrics_window: StreamingMetricsWindow,
    trigger_sender: mpsc::Sender<Vec<FiredTrigger>>,
}

impl SelfImprovementConsumer {
    /// Create a new consumer with the given configuration.
    pub fn new(
        config: StreamingConsumerConfig,
        trigger_sender: mpsc::Sender<Vec<FiredTrigger>>,
    ) -> Self {
        Self {
            config,
            metrics_window: StreamingMetricsWindow::new(),
            trigger_sender,
        }
    }

    /// Process a streaming message and update the metrics window.
    pub fn process_message(&mut self, msg: StreamingMessage) {
        match msg {
            StreamingMessage::NodeExecution {
                node_id,
                duration_ms,
                success,
            } => {
                self.metrics_window
                    .record_node_duration(&node_id, duration_ms);
                if success {
                    self.metrics_window.record_success(&node_id);
                } else {
                    self.metrics_window.record_error(&node_id);
                }
            }

            StreamingMessage::Metrics {
                scope,
                scope_id,
                metrics,
            } => {
                // Extract relevant metrics
                if let Some(&duration) = metrics.get("duration_ms") {
                    // M-959: Safe conversion from f64 to u64 with bounds checking
                    let duration_u64 = if duration < 0.0 {
                        0
                    } else if duration > u64::MAX as f64 {
                        u64::MAX
                    } else {
                        duration as u64
                    };
                    self.metrics_window
                        .record_node_duration(&scope_id, duration_u64);
                }
                if let Some(&quality_score) = metrics.get("quality_score") {
                    self.metrics_window.record_quality_score(quality_score);
                }
                if let Some(&error_rate) = metrics.get("error_rate") {
                    // M-961: Note that this binary recording loses rate granularity.
                    // A 1% and 99% error rate both count as "1 error" in aggregation.
                    // For accurate rate tracking, use the quality_scores mechanism
                    // or process error counts directly from Metrics messages.
                    let op_key = format!("{}:{}", scope, scope_id);
                    if error_rate > 0.0 {
                        if error_rate > 0.5 {
                            // High error rate: log for visibility
                            tracing::debug!(
                                scope = %scope,
                                scope_id = %scope_id,
                                error_rate = %error_rate,
                                "High error rate metric received"
                            );
                        }
                        self.metrics_window.record_error(&op_key);
                    } else {
                        self.metrics_window.record_success(&op_key);
                    }
                }
            }

            StreamingMessage::Error { operation, message } => {
                // M-960: Log error message for debugging context before recording
                tracing::debug!(
                    operation = %operation,
                    message = %message,
                    "Streaming error recorded in metrics window"
                );
                self.metrics_window.record_error(&operation);
            }

            StreamingMessage::Retry { operation, .. } => {
                self.metrics_window.record_retry(&operation);
            }
        }
    }

    /// Analyze the current metrics window and send triggers if any.
    pub async fn analyze_and_send(
        &mut self,
    ) -> Result<usize, mpsc::error::SendError<Vec<FiredTrigger>>> {
        let triggers = self.metrics_window.analyze(&self.config);
        let count = triggers.len();

        if !triggers.is_empty() {
            self.trigger_sender.send(triggers).await?;
        }

        Ok(count)
    }

    /// Reset the metrics window for a new aggregation period.
    pub fn reset_window(&mut self) {
        self.metrics_window.reset();
    }

    /// Get the current metrics window statistics.
    #[must_use]
    pub fn window_stats(&self) -> &StreamingMetricsWindow {
        &self.metrics_window
    }
}

/// Convert a DashStreamMessage to our internal StreamingMessage type.
///
/// This function is only available when the `dashstream` feature is enabled.
#[cfg(feature = "dashstream")]
pub fn convert_dashstream_message(
    msg: &dashflow_streaming::DashStreamMessage,
) -> Option<StreamingMessage> {
    use dashflow_streaming::dash_stream_message::Message;
    use dashflow_streaming::EventType;

    match &msg.message {
        Some(Message::Event(event)) => {
            // Check if this is a node end event
            if event.event_type == EventType::NodeEnd as i32 {
                Some(StreamingMessage::NodeExecution {
                    node_id: event.node_id.clone(),
                    duration_ms: (event.duration_us / 1000) as u64,
                    success: true, // Assume success if we got a NodeEnd event
                })
            } else {
                None
            }
        }

        Some(Message::Metrics(metrics)) => {
            let mut metric_values = HashMap::new();
            for (name, value) in &metrics.metrics {
                if let Some(ref v) = value.value {
                    use dashflow_streaming::metric_value::Value;
                    match v {
                        Value::FloatValue(f) => {
                            metric_values.insert(name.clone(), *f);
                        }
                        Value::IntValue(i) => {
                            metric_values.insert(name.clone(), *i as f64);
                        }
                        Value::BoolValue(b) => {
                            metric_values.insert(name.clone(), if *b { 1.0 } else { 0.0 });
                        }
                        _ => {}
                    }
                }
            }

            Some(StreamingMessage::Metrics {
                scope: metrics.scope.clone(),
                scope_id: metrics.scope_id.clone(),
                metrics: metric_values,
            })
        }

        Some(Message::Error(error)) => {
            let header = error.header.as_ref()?;
            Some(StreamingMessage::Error {
                operation: header.thread_id.clone(),
                message: error.message.clone(),
            })
        }

        _ => None,
    }
}

/// Start a streaming consumer that connects to Kafka and feeds the daemon.
///
/// This function is only available when the `dashstream` feature is enabled.
///
/// # Errors
///
/// Returns an error if:
/// - The topic has more than one partition (S-24 enforcement)
/// - Kafka connection fails
/// - Message consumption fails
#[cfg(feature = "dashstream")]
pub async fn start_streaming_consumer(
    config: StreamingConsumerConfig,
    trigger_sender: mpsc::Sender<Vec<FiredTrigger>>,
    stop_signal: Arc<std::sync::atomic::AtomicBool>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    use dashflow_streaming::consumer::DashStreamConsumer;
    use dashflow_streaming::kafka::get_partition_count;
    use std::sync::atomic::Ordering;

    // S-24: Enforce single-partition topics at startup.
    // DashStreamConsumer uses rskafka PartitionClient which only reads one partition.
    // Rather than silently losing data from other partitions, we fail loudly.
    let partition_count = get_partition_count(&config.bootstrap_servers, &config.topic)
        .await
        .map_err(|e| {
            format!(
                "Failed to get partition count for topic '{}': {}",
                config.topic, e
            )
        })?;

    if partition_count != 1 {
        return Err(format!(
            "S-24: Topic '{}' has {} partitions, but this consumer only supports single-partition topics. \
             Either: (1) Use a single-partition topic, or (2) For multi-partition support, migrate to \
             rdkafka StreamConsumer with consumer groups (see quality_aggregator.rs for reference).",
            config.topic, partition_count
        )
        .into());
    }

    let mut consumer = DashStreamConsumer::new(
        &config.bootstrap_servers,
        &config.topic,
        &config.consumer_group,
    )
    .await?;

    let mut self_improve_consumer = SelfImprovementConsumer::new(config.clone(), trigger_sender);
    let window_duration = Duration::from_secs(config.aggregation_window_secs);
    let mut last_analysis = std::time::Instant::now();

    tracing::info!(
        "Starting streaming consumer on {}:{}",
        config.bootstrap_servers,
        config.topic
    );

    while !stop_signal.load(Ordering::SeqCst) {
        // Try to get next message with timeout - uses SHORT_TIMEOUT (5s) for responsive stop signal handling
        match consumer.next_timeout(SHORT_TIMEOUT).await {
            Some(Ok(msg)) => {
                if let Some(streaming_msg) = convert_dashstream_message(&msg) {
                    self_improve_consumer.process_message(streaming_msg);
                }
            }
            Some(Err(e)) => {
                tracing::warn!("Error consuming message: {}", e);
            }
            None => {
                // Timeout - continue
            }
        }

        // Check if we should analyze the window
        if last_analysis.elapsed() >= window_duration {
            match self_improve_consumer.analyze_and_send().await {
                Ok(count) => {
                    if count > 0 {
                        tracing::info!("Sent {} triggers from streaming analysis", count);
                    }
                }
                Err(e) => {
                    tracing::error!("Failed to send triggers: {}", e);
                }
            }
            self_improve_consumer.reset_window();
            last_analysis = std::time::Instant::now();
        }
    }

    tracing::info!("Streaming consumer stopped");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metrics_window_basic() {
        let mut window = StreamingMetricsWindow::new();

        // Record some node durations
        window.record_node_duration("node_a", 5000);
        window.record_node_duration("node_a", 15000);
        window.record_node_duration("node_b", 3000);

        assert_eq!(window.node_durations.get("node_a").unwrap().len(), 2);
        assert_eq!(window.node_durations.get("node_b").unwrap().len(), 1);
    }

    #[test]
    fn test_metrics_window_node_duration_cap() {
        let mut window = StreamingMetricsWindow::new();

        let extra = 10;
        for i in 0..(DEFAULT_MAX_NODE_DURATION_SAMPLES + extra) {
            window.record_node_duration("node", i as u64);
        }

        let durations = window.node_durations.get("node").unwrap();
        assert_eq!(durations.len(), DEFAULT_MAX_NODE_DURATION_SAMPLES);
        assert_eq!(*durations.iter().min().unwrap(), extra as u64);
        assert_eq!(
            *durations.iter().max().unwrap(),
            (DEFAULT_MAX_NODE_DURATION_SAMPLES + extra - 1) as u64
        );
    }

    #[test]
    fn test_metrics_window_node_duration_cap_preserves_slow_signal() {
        let mut window = StreamingMetricsWindow::new();
        let config = StreamingConsumerConfig {
            slow_node_threshold_ms: 5_000,
            ..Default::default()
        };

        // Record a slow sample, then flood the window with fast samples. Even with
        // capping enabled, the slow sample should remain because we keep the
        // largest per-node values.
        window.record_node_duration("node", 10_000);
        for _ in 0..(DEFAULT_MAX_NODE_DURATION_SAMPLES + 50) {
            window.record_node_duration("node", 1);
        }

        let triggers = window.analyze(&config);
        assert!(
            triggers
                .iter()
                .any(|t| matches!(t.trigger_type, AnalysisTriggerType::SlowNode { .. })),
            "expected SlowNode trigger"
        );
    }

    #[test]
    fn test_metrics_window_quality_score_cap() {
        let mut window = StreamingMetricsWindow::new();

        let extra = 10;
        for i in 0..(DEFAULT_MAX_QUALITY_SCORE_SAMPLES + extra) {
            window.record_quality_score(i as f64);
        }

        assert_eq!(window.quality_scores.len(), DEFAULT_MAX_QUALITY_SCORE_SAMPLES);
        assert!((window.quality_scores[0] - extra as f64).abs() < f64::EPSILON);
        assert!(
            (window.quality_scores[window.quality_scores.len() - 1]
                - (DEFAULT_MAX_QUALITY_SCORE_SAMPLES + extra - 1) as f64)
                .abs()
                < f64::EPSILON
        );
    }

    #[test]
    fn test_error_rate_calculation() {
        let mut window = StreamingMetricsWindow::new();

        // 2 errors, 8 successes = 20% error rate
        window.record_error("op1");
        window.record_error("op2");
        for _ in 0..8 {
            window.record_success("op1");
        }

        let error_rate = window.calculate_error_rate();
        assert!((error_rate - 0.2).abs() < 0.001);
    }

    #[test]
    fn test_trigger_generation() {
        let mut window = StreamingMetricsWindow::new();
        let config = StreamingConsumerConfig {
            slow_node_threshold_ms: 5000, // 5 seconds
            error_rate_threshold: 0.1,    // 10%
            retry_threshold: 2,
            ..Default::default()
        };

        // Add a slow node
        window.record_node_duration("slow_node", 10000);

        // Add retries
        window.record_retry("op1");
        window.record_retry("op1");
        window.record_retry("op1"); // Should trigger

        // Add errors (30% error rate with 10 samples)
        for _ in 0..3 {
            window.record_error("err_op");
        }
        for _ in 0..7 {
            window.record_success("err_op");
        }

        let triggers = window.analyze(&config);

        // Should have: 1 slow node, 1 retry, 1 error rate
        assert_eq!(triggers.len(), 3);

        let trigger_types: Vec<_> = triggers
            .iter()
            .map(|t| match &t.trigger_type {
                AnalysisTriggerType::SlowNode { .. } => "slow",
                AnalysisTriggerType::HighErrorRate { .. } => "error",
                AnalysisTriggerType::RepeatedRetry { .. } => "retry",
                AnalysisTriggerType::UnusedCapability { .. } => "unused",
            })
            .collect();

        assert!(trigger_types.contains(&"slow"));
        assert!(trigger_types.contains(&"error"));
        assert!(trigger_types.contains(&"retry"));
    }

    #[test]
    fn test_window_reset() {
        let mut window = StreamingMetricsWindow::new();

        window.record_node_duration("node", 1000);
        window.record_error("op");
        window.record_quality_score(0.85);

        assert_eq!(window.message_count, 3);

        window.reset();

        assert_eq!(window.message_count, 0);
        assert!(window.node_durations.is_empty());
        assert!(window.error_counts.is_empty());
        assert!(window.quality_scores.is_empty());
    }

    #[test]
    fn test_quality_score_tracking() {
        let mut window = StreamingMetricsWindow::new();

        window.record_quality_score(0.80);
        window.record_quality_score(0.90);
        window.record_quality_score(0.85);

        let avg = window.average_quality_score().unwrap();
        assert!((avg - 0.85).abs() < 0.001);
    }

    // Tests for StreamingConsumerConfig builder pattern

    #[test]
    fn test_config_builder_new() {
        let config = StreamingConsumerConfig::new();
        assert_eq!(config.bootstrap_servers, "localhost:9092");
        assert_eq!(config.topic, "dashstream-metrics");
        assert_eq!(config.consumer_group, "dashflow-self-improvement");
    }

    #[test]
    fn test_config_builder_chain() {
        let config = StreamingConsumerConfig::new()
            .with_bootstrap_servers("kafka:9093")
            .with_topic("custom-topic")
            .with_consumer_group("my-group")
            .with_slow_node_threshold_ms(5000)
            .with_error_rate_threshold(0.10)
            .with_retry_threshold(5)
            .with_aggregation_window_secs(120);

        assert_eq!(config.bootstrap_servers, "kafka:9093");
        assert_eq!(config.topic, "custom-topic");
        assert_eq!(config.consumer_group, "my-group");
        assert_eq!(config.slow_node_threshold_ms, 5000);
        assert!((config.error_rate_threshold - 0.10).abs() < f64::EPSILON);
        assert_eq!(config.retry_threshold, 5);
        assert_eq!(config.aggregation_window_secs, 120);
    }

    #[test]
    fn test_config_error_rate_clamped() {
        // Values > 1.0 should be clamped to 1.0
        let config = StreamingConsumerConfig::new().with_error_rate_threshold(1.5);
        assert!((config.error_rate_threshold - 1.0).abs() < f64::EPSILON);

        // Values < 0.0 should be clamped to 0.0
        let config = StreamingConsumerConfig::new().with_error_rate_threshold(-0.5);
        assert!(config.error_rate_threshold.abs() < f64::EPSILON);
    }

    #[test]
    fn test_config_validate_valid() {
        let config = StreamingConsumerConfig::new();
        let errors = config.validate();
        assert!(errors.is_empty(), "Default config should be valid");
    }

    #[test]
    fn test_config_validate_empty_bootstrap_servers() {
        let config = StreamingConsumerConfig::new().with_bootstrap_servers("");
        let errors = config.validate();
        assert!(errors.iter().any(|e| e.contains("bootstrap_servers")));
    }

    #[test]
    fn test_config_validate_empty_topic() {
        let config = StreamingConsumerConfig::new().with_topic("");
        let errors = config.validate();
        assert!(errors.iter().any(|e| e.contains("topic")));
    }

    #[test]
    fn test_config_validate_empty_consumer_group() {
        let config = StreamingConsumerConfig::new().with_consumer_group("");
        let errors = config.validate();
        assert!(errors.iter().any(|e| e.contains("consumer_group")));
    }

    #[test]
    fn test_config_validate_zero_window() {
        let config = StreamingConsumerConfig::new().with_aggregation_window_secs(0);
        let errors = config.validate();
        assert!(errors.iter().any(|e| e.contains("aggregation_window_secs")));
    }

    #[test]
    fn test_config_validate_multiple_errors() {
        let mut config = StreamingConsumerConfig::new()
            .with_bootstrap_servers("")
            .with_topic("")
            .with_aggregation_window_secs(0);

        // Force invalid error_rate by bypassing clamping (direct assignment)
        config.error_rate_threshold = 2.0;

        let errors = config.validate();
        assert_eq!(errors.len(), 4, "Expected 4 validation errors");
    }
}
