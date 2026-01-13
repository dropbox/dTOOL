// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

//! Quality Aggregator - Kafka Consumer for Quality Metrics
//!
//! Consumes quality metrics from Kafka and aggregates them for analysis.
//! Exposes an HTTP health endpoint on port 3003.
//!
//! # Usage
//!
//! ```bash
//! # Set environment variables
//! export KAFKA_BROKERS="localhost:9092"
//! export KAFKA_TOPIC="dashstream-quality"
//! export KAFKA_GROUP="quality-aggregator"
//! export HEALTH_PORT="3003"  # Optional, defaults to 3003
//!
//! # Run aggregator
//! cargo run --bin quality_aggregator
//! ```
//!
//! # Docker Usage
//!
//! ```bash
//! docker-compose up -d quality-aggregator
//! docker-compose logs -f quality-aggregator
//! ```
//!
//! # Health Endpoint
//!
//! ```bash
//! curl http://localhost:3003/health
//! ```

use axum::{extract::State, response::IntoResponse, routing::get, Json, Router};
use dashflow_streaming::consumer::DashStreamConsumer;
use dashflow_streaming::env_vars;
use dashflow_streaming::kafka::get_partition_count;
use dashflow_streaming::DashStreamMessage;
use serde::Serialize;
use std::collections::HashMap;
use std::env;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::net::TcpListener;
use tokio::signal;
use tokio::sync::{mpsc, RwLock};
// S-21: Use structured tracing instead of println!/eprintln!
use tracing::{error, info, warn};

/// Quality statistics aggregator
#[derive(Default, Clone)]
struct QualityStats {
    total_evaluations: u64,
    accuracy_sum: f64,
    relevance_sum: f64,
    completeness_sum: f64,
    low_quality_count: u64,
    issue_counts: HashMap<String, u64>,
    kafka_messages_received: u64,
    kafka_decode_errors: u64,
}

/// Shared state for the HTTP server
#[derive(Clone)]
struct AppState {
    stats: Arc<RwLock<QualityStats>>,
    kafka_connected: Arc<RwLock<bool>>,
    start_time: Instant,
}

/// Health response structure
#[derive(Serialize)]
struct HealthResponse {
    status: String,
    uptime_seconds: u64,
    kafka_connected: bool,
    stats: StatsSnapshot,
}

/// Snapshot of quality stats for the health response
#[derive(Serialize)]
struct StatsSnapshot {
    total_evaluations: u64,
    kafka_messages_received: u64,
    kafka_decode_errors: u64,
    average_accuracy: Option<f64>,
    average_relevance: Option<f64>,
    average_completeness: Option<f64>,
    average_overall: Option<f64>,
    low_quality_count: u64,
    low_quality_percentage: Option<f64>,
}

impl QualityStats {
    fn update_from_metrics(&mut self, metrics: &dashflow_streaming::Metrics) {
        self.total_evaluations += 1;

        // Extract quality scores from metrics
        if let Some(acc) = metrics.metrics.get("accuracy") {
            if let Some(dashflow_streaming::metric_value::Value::FloatValue(v)) = acc.value {
                self.accuracy_sum += v;
            }
        }

        if let Some(rel) = metrics.metrics.get("relevance") {
            if let Some(dashflow_streaming::metric_value::Value::FloatValue(v)) = rel.value {
                self.relevance_sum += v;
            }
        }

        if let Some(comp) = metrics.metrics.get("completeness") {
            if let Some(dashflow_streaming::metric_value::Value::FloatValue(v)) = comp.value {
                self.completeness_sum += v;
            }
        }

        // Check average quality
        let avg_quality = (self.accuracy_sum + self.relevance_sum + self.completeness_sum)
            / (self.total_evaluations as f64 * 3.0);
        if avg_quality < 0.7 {
            self.low_quality_count += 1;
        }

        // Count issues from tags
        for (tag, value) in &metrics.tags {
            if value == "true" {
                *self.issue_counts.entry(tag.clone()).or_insert(0) += 1;
            }
        }
    }

    fn print_summary(&self) {
        if self.total_evaluations == 0 {
            info!("No evaluations received yet");
            return;
        }

        let avg_accuracy = self.accuracy_sum / self.total_evaluations as f64;
        let avg_relevance = self.relevance_sum / self.total_evaluations as f64;
        let avg_completeness = self.completeness_sum / self.total_evaluations as f64;
        let avg_overall = (avg_accuracy + avg_relevance + avg_completeness) / 3.0;
        let low_quality_pct = (self.low_quality_count as f64 / self.total_evaluations as f64) * 100.0;

        // S-21: Use structured tracing for quality summary
        info!(
            total_evaluations = self.total_evaluations,
            avg_accuracy = format!("{:.3}", avg_accuracy).as_str(),
            avg_relevance = format!("{:.3}", avg_relevance).as_str(),
            avg_completeness = format!("{:.3}", avg_completeness).as_str(),
            avg_overall = format!("{:.3}", avg_overall).as_str(),
            low_quality_count = self.low_quality_count,
            low_quality_pct = format!("{:.1}", low_quality_pct).as_str(),
            "Quality Aggregation Summary"
        );

        if !self.issue_counts.is_empty() {
            for (issue, count) in &self.issue_counts {
                let pct = (*count as f64 / self.total_evaluations as f64) * 100.0;
                info!(issue = %issue, count = count, pct = format!("{:.1}", pct).as_str(), "Issue breakdown");
            }
        }
    }

    /// Create a snapshot for the health endpoint
    fn to_snapshot(&self) -> StatsSnapshot {
        let (avg_accuracy, avg_relevance, avg_completeness, avg_overall, low_quality_pct) =
            if self.total_evaluations > 0 {
                let acc = self.accuracy_sum / self.total_evaluations as f64;
                let rel = self.relevance_sum / self.total_evaluations as f64;
                let comp = self.completeness_sum / self.total_evaluations as f64;
                let overall = (acc + rel + comp) / 3.0;
                let low_pct =
                    (self.low_quality_count as f64 / self.total_evaluations as f64) * 100.0;
                (
                    Some(acc),
                    Some(rel),
                    Some(comp),
                    Some(overall),
                    Some(low_pct),
                )
            } else {
                (None, None, None, None, None)
            };

        StatsSnapshot {
            total_evaluations: self.total_evaluations,
            kafka_messages_received: self.kafka_messages_received,
            kafka_decode_errors: self.kafka_decode_errors,
            average_accuracy: avg_accuracy,
            average_relevance: avg_relevance,
            average_completeness: avg_completeness,
            average_overall: avg_overall,
            low_quality_count: self.low_quality_count,
            low_quality_percentage: low_quality_pct,
        }
    }
}

/// Health check endpoint handler
async fn health_handler(State(state): State<AppState>) -> impl IntoResponse {
    let stats = state.stats.read().await;
    let kafka_connected = *state.kafka_connected.read().await;
    let uptime = state.start_time.elapsed().as_secs();

    // Determine health status
    let status = if kafka_connected {
        "healthy".to_string()
    } else {
        "degraded".to_string()
    };

    let response = HealthResponse {
        status,
        uptime_seconds: uptime,
        kafka_connected,
        stats: stats.to_snapshot(),
    };

    Json(response)
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // S-21: Initialize tracing subscriber
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive(tracing::Level::INFO.into()),
        )
        .init();

    info!("Quality Aggregator Starting...");
    info!("==================================");

    // Read configuration from environment
    // S-14: Log warning when using defaults (may not work in Docker)
    let kafka_brokers = match env::var(env_vars::KAFKA_BROKERS) {
        Ok(brokers) => brokers,
        Err(_) => {
            let default = "localhost:9092";
            warn!(
                default = default,
                "KAFKA_BROKERS not set, using default. In Docker/production, set KAFKA_BROKERS \
                 to the correct broker address (e.g., 'kafka:29092' for Docker Compose)"
            );
            default.to_string()
        }
    };
    let kafka_topic =
        env::var(env_vars::KAFKA_TOPIC).unwrap_or_else(|_| "dashstream-quality".to_string());
    // Note: Consumer groups are NOT supported by rskafka. This aggregator uses partition-based
    // consumption where each partition gets its own consumer task. See consumer.rs:412.
    // KAFKA_GROUP env var is intentionally NOT read - it would be misleading.
    let health_port: u16 = env_vars::env_u16_or_default(env_vars::HEALTH_PORT, 3003);

    info!(
        kafka_brokers = %kafka_brokers,
        kafka_topic = %kafka_topic,
        health_port = health_port,
        "Configuration loaded (partition-based consumption, no consumer groups)"
    );

    // Create shared state for health endpoint
    let app_state = AppState {
        stats: Arc::new(RwLock::new(QualityStats::default())),
        kafka_connected: Arc::new(RwLock::new(false)),
        start_time: Instant::now(),
    };

    // Start HTTP server for health endpoint
    let app = Router::new()
        .route("/health", get(health_handler))
        .with_state(app_state.clone());

    let addr = format!("0.0.0.0:{}", health_port);
    let listener = TcpListener::bind(&addr).await?;
    info!(health_port = health_port, "Health endpoint started");

    tokio::spawn(async move {
        if let Err(e) = axum::serve(listener, app).await {
            error!(error = %e, "HTTP server error");
        }
    });

    // S-2: Multi-partition consumption - get partition count and spawn consumer per partition
    info!("Connecting to Kafka...");
    let partition_count = match get_partition_count(&kafka_brokers, &kafka_topic).await {
        Ok(count) => {
            info!(topic = %kafka_topic, partitions = count, "Got partition count");
            count
        }
        Err(e) => {
            warn!(error = %e, "Failed to get partition count, falling back to partition 0 only");
            1 // Fall back to single partition
        }
    };

    // Create a channel to merge messages from all partition consumers
    let (msg_tx, mut msg_rx) = mpsc::channel::<Result<DashStreamMessage, String>>(1000);

    // Spawn a consumer task for each partition
    let mut consumer_handles = Vec::new();
    for partition in 0..partition_count {
        let brokers = kafka_brokers.clone();
        let topic = kafka_topic.clone();
        let tx = msg_tx.clone();
        let app_state_clone = app_state.clone();

        let handle = tokio::spawn(async move {
            match DashStreamConsumer::new_for_partition(&brokers, &topic, partition).await {
                Ok(mut consumer) => {
                    // Mark kafka as connected on first successful consumer
                    *app_state_clone.kafka_connected.write().await = true;
                    info!(partition = partition, "Consumer connected");

                    loop {
                        match consumer.next_timeout(Duration::from_secs(60)).await {
                            Some(Ok(msg)) => {
                                if tx.send(Ok(msg)).await.is_err() {
                                    // Channel closed, receiver dropped
                                    break;
                                }
                            }
                            Some(Err(e)) => {
                                // Send error as string for logging
                                let _ = tx.send(Err(format!("Partition {}: {}", partition, e))).await;
                            }
                            None => {
                                // Timeout - continue waiting
                            }
                        }
                    }
                }
                Err(e) => {
                    error!(partition = partition, error = %e, "Failed to create consumer");
                }
            }
        });
        consumer_handles.push(handle);
    }
    // Drop the original sender so the channel closes when all consumers are done
    drop(msg_tx);

    // S-22: Return error instead of abrupt exit
    if partition_count == 0 {
        error!(topic = %kafka_topic, "No partitions found for topic");
        return Err(format!("No partitions found for topic '{}'", kafka_topic).into());
    }

    info!(
        partition_count = partition_count,
        topic = %kafka_topic,
        "Connected to Kafka, listening for quality metrics"
    );
    info!("Press Ctrl+C to stop and show final summary");

    let mut message_count = 0;
    let mut last_summary_time = std::time::Instant::now();
    let shutdown = signal::ctrl_c();
    tokio::pin!(shutdown);

    // Consume messages from merged channel
    loop {
        tokio::select! {
            _ = &mut shutdown => {
                info!("Ctrl+C received, shutting down...");
                // Abort consumer tasks
                for handle in &consumer_handles {
                    handle.abort();
                }
                break;
            }
            received = msg_rx.recv() => {
                match received {
                    Some(Ok(msg)) => {
                        message_count += 1;

                        // Update shared stats for kafka message count
                        {
                            let mut stats = app_state.stats.write().await;
                            stats.kafka_messages_received += 1;
                        }

                        match msg.message {
                            Some(dashflow_streaming::dash_stream_message::Message::Metrics(metrics)) => {
                                // Only process quality metrics
                                if metrics.scope == "quality" {
                                    let header = metrics.header.as_ref();
                                    let thread_id =
                                        header.map_or("unknown", |h| h.thread_id.as_str());

                                    // Extract quality scores
                                    let mut acc = None;
                                    let mut rel = None;
                                    let mut comp = None;

                                    if let Some(v) = metrics.metrics.get("accuracy") {
                                        if let Some(dashflow_streaming::metric_value::Value::FloatValue(
                                            val,
                                        )) = v.value
                                        {
                                            acc = Some(val);
                                        }
                                    }
                                    if let Some(v) = metrics.metrics.get("relevance") {
                                        if let Some(dashflow_streaming::metric_value::Value::FloatValue(
                                            val,
                                        )) = v.value
                                        {
                                            rel = Some(val);
                                        }
                                    }
                                    if let Some(v) = metrics.metrics.get("completeness") {
                                        if let Some(dashflow_streaming::metric_value::Value::FloatValue(
                                            val,
                                        )) = v.value
                                        {
                                            comp = Some(val);
                                        }
                                    }

                                    if let (Some(a), Some(r), Some(c)) = (acc, rel, comp) {
                                        let avg = (a + r + c) / 3.0;
                                        info!(
                                            thread_id = %thread_id,
                                            accuracy = format!("{:.3}", a).as_str(),
                                            relevance = format!("{:.3}", r).as_str(),
                                            completeness = format!("{:.3}", c).as_str(),
                                            average = format!("{:.3}", avg).as_str(),
                                            "Quality metrics received"
                                        );

                                        // Update shared stats
                                        let mut stats = app_state.stats.write().await;
                                        stats.update_from_metrics(&metrics);
                                    }
                                }
                            }
                            Some(dashflow_streaming::dash_stream_message::Message::Event(event)) => {
                                // Check for quality alerts
                                if let Some(alert_type) = event.attributes.get("alert_type") {
                                    if let Some(dashflow_streaming::attribute_value::Value::StringValue(
                                        val,
                                    )) = &alert_type.value
                                    {
                                        if val == "LOW_QUALITY" {
                                            let header = event.header.as_ref();
                                            let thread_id =
                                                header.map_or("unknown", |h| h.thread_id.as_str());
                                            warn!(thread_id = %thread_id, "LOW QUALITY ALERT");
                                        }
                                    }
                                }
                            }
                            _ => {
                                // Ignore other message types
                            }
                        }

                        // Print summary every 60 seconds
                        if last_summary_time.elapsed() > Duration::from_secs(60) {
                            let stats = app_state.stats.read().await;
                            stats.print_summary();
                            last_summary_time = std::time::Instant::now();
                        }
                    }
                    Some(Err(e)) => {
                        error!(error = %e, "Error decoding message");
                        // Update shared stats for decode errors
                        let mut stats = app_state.stats.write().await;
                        stats.kafka_decode_errors += 1;
                    }
                    None => {
                        // Channel closed - all consumers have finished
                        info!("All partition consumers have stopped");
                        if message_count > 0 {
                            let stats = app_state.stats.read().await;
                            stats.print_summary();
                        }
                        break;
                    }
                }
            }
        }
    }

    // Wait for consumer tasks to finish (they should already be done or aborted)
    for handle in consumer_handles {
        let _ = handle.await;
    }

    info!("Final summary");
    let stats = app_state.stats.read().await;
    stats.print_summary();
    Ok(())
}
