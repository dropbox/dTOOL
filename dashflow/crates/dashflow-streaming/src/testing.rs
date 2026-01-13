//! Test utilities for dashflow-streaming
//!
//! This module provides helper functions for integration tests that need to
//! send test metrics to Kafka without spawning external processes.
//!
//! # Why This Exists (M-110)
//!
//! Integration tests previously used `cargo run --example send_test_metrics` which:
//! - Could trigger recompilation during tests
//! - Had no timeout protection (could hang indefinitely)
//! - Was slower than direct library calls
//!
//! This module provides the same functionality as a library function that tests
//! can call directly.
//!
//! # Example
//!
//! ```rust,no_run
//! use dashflow_streaming::testing::send_test_quality_metrics;
//!
//! #[tokio::test]
//! async fn test_pipeline() {
//!     send_test_quality_metrics("localhost:9092", "dashstream-quality")
//!         .await
//!         .expect("Failed to send test metrics");
//! }
//! ```

use crate::producer::DashStreamProducer;
use crate::{Header, MessageType, MetricValue, Metrics};
use std::collections::HashMap;
use std::time::Duration;

/// Configuration for test quality metrics
#[derive(Debug, Clone)]
pub struct TestQualityConfig {
    /// Kafka bootstrap servers (e.g., "localhost:9092")
    pub bootstrap_servers: String,
    /// Kafka topic to send to (e.g., "dashstream-quality")
    pub topic: String,
    /// Tenant ID for test metrics
    pub tenant_id: String,
    /// Whether to print progress messages
    pub verbose: bool,
}

impl Default for TestQualityConfig {
    fn default() -> Self {
        Self {
            bootstrap_servers: "localhost:9092".to_string(),
            topic: "dashstream-quality".to_string(),
            tenant_id: "test-tenant".to_string(),
            verbose: false,
        }
    }
}

/// Test quality score data
#[derive(Debug, Clone)]
pub struct TestQualityScore {
    /// Thread ID for the test sample.
    pub thread_id: String,
    /// Accuracy score (0.0-1.0).
    pub accuracy: f64,
    /// Relevance score (0.0-1.0).
    pub relevance: f64,
    /// Completeness score (0.0-1.0).
    pub completeness: f64,
    /// Overall quality score (typically an aggregate of sub-scores).
    pub quality_score: f64,
    /// Whether this sample is considered passing for the test.
    pub passed: bool,
}

impl TestQualityScore {
    /// Create default test scores (3 samples with varying quality)
    pub fn default_test_scores() -> Vec<Self> {
        vec![
            Self {
                thread_id: "test-thread-001".to_string(),
                accuracy: 0.95,
                relevance: 0.87,
                completeness: 0.92,
                quality_score: 0.91,
                passed: true,
            },
            Self {
                thread_id: "test-thread-002".to_string(),
                accuracy: 0.88,
                relevance: 0.91,
                completeness: 0.85,
                quality_score: 0.88,
                passed: true,
            },
            Self {
                thread_id: "test-thread-003".to_string(),
                accuracy: 0.72,
                relevance: 0.68,
                completeness: 0.79,
                quality_score: 0.73,
                passed: false,
            },
        ]
    }
}

fn create_quality_metrics(score: &TestQualityScore, tenant_id: &str) -> Metrics {
    let mut metrics_map: HashMap<String, MetricValue> = HashMap::new();

    metrics_map.insert(
        "accuracy".to_string(),
        MetricValue {
            value: Some(crate::metric_value::Value::FloatValue(score.accuracy)),
            unit: "ratio".to_string(),
            r#type: crate::metric_value::MetricType::Gauge as i32,
        },
    );

    metrics_map.insert(
        "relevance".to_string(),
        MetricValue {
            value: Some(crate::metric_value::Value::FloatValue(score.relevance)),
            unit: "ratio".to_string(),
            r#type: crate::metric_value::MetricType::Gauge as i32,
        },
    );

    metrics_map.insert(
        "completeness".to_string(),
        MetricValue {
            value: Some(crate::metric_value::Value::FloatValue(score.completeness)),
            unit: "ratio".to_string(),
            r#type: crate::metric_value::MetricType::Gauge as i32,
        },
    );

    metrics_map.insert(
        "quality_score".to_string(),
        MetricValue {
            value: Some(crate::metric_value::Value::FloatValue(score.quality_score)),
            unit: "ratio".to_string(),
            r#type: crate::metric_value::MetricType::Gauge as i32,
        },
    );

    metrics_map.insert(
        "passed".to_string(),
        MetricValue {
            value: Some(crate::metric_value::Value::BoolValue(score.passed)),
            unit: "boolean".to_string(),
            r#type: crate::metric_value::MetricType::Gauge as i32,
        },
    );

    let mut tags: HashMap<String, String> = HashMap::new();
    tags.insert("thread_id".to_string(), score.thread_id.clone());
    tags.insert("tenant_id".to_string(), tenant_id.to_string());

    Metrics {
        header: Some(Header {
            message_id: uuid::Uuid::new_v4().as_bytes().to_vec(),
            timestamp_us: chrono::Utc::now().timestamp_micros(),
            tenant_id: tenant_id.to_string(),
            thread_id: score.thread_id.clone(),
            sequence: 1,
            r#type: MessageType::Metrics as i32,
            parent_id: vec![],
            compression: 0,
            schema_version: 1,
        }),
        scope: "quality".to_string(),
        scope_id: score.thread_id.clone(),
        metrics: metrics_map,
        tags,
    }
}

/// Send test quality metrics to Kafka
///
/// This is the library equivalent of `cargo run --example send_test_metrics`.
/// Use this in integration tests instead of spawning a subprocess.
///
/// # Arguments
///
/// * `bootstrap_servers` - Kafka bootstrap servers (e.g., "localhost:9092")
/// * `topic` - Kafka topic to send to (e.g., "dashstream-quality")
///
/// # Returns
///
/// Returns the number of messages sent, or an error if sending failed.
///
/// # Example
///
/// ```rust,no_run
/// use dashflow_streaming::testing::send_test_quality_metrics;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
/// let messages_sent = send_test_quality_metrics("localhost:9092", "dashstream-quality").await?;
/// assert_eq!(messages_sent, 3);
/// # Ok(())
/// # }
/// ```
pub async fn send_test_quality_metrics(
    bootstrap_servers: &str,
    topic: &str,
) -> Result<usize, Box<dyn std::error::Error + Send + Sync>> {
    send_test_quality_metrics_with_config(TestQualityConfig {
        bootstrap_servers: bootstrap_servers.to_string(),
        topic: topic.to_string(),
        ..Default::default()
    })
    .await
}

/// Send test quality metrics with custom configuration
///
/// More flexible version of [`send_test_quality_metrics`] that allows
/// customizing tenant ID and verbosity.
pub async fn send_test_quality_metrics_with_config(
    config: TestQualityConfig,
) -> Result<usize, Box<dyn std::error::Error + Send + Sync>> {
    send_quality_metrics(
        &config.bootstrap_servers,
        &config.topic,
        &config.tenant_id,
        &TestQualityScore::default_test_scores(),
        config.verbose,
    )
    .await
}

/// Send custom quality metrics to Kafka
///
/// Low-level function that allows full control over what metrics are sent.
pub async fn send_quality_metrics(
    bootstrap_servers: &str,
    topic: &str,
    tenant_id: &str,
    scores: &[TestQualityScore],
    verbose: bool,
) -> Result<usize, Box<dyn std::error::Error + Send + Sync>> {
    if verbose {
        println!("Connecting to Kafka at {}...", bootstrap_servers);
    }

    let producer = DashStreamProducer::new(bootstrap_servers, topic).await?;

    if verbose {
        println!("Producer connected to topic: {}", topic);
    }

    let mut sent_count = 0;
    for score in scores {
        if verbose {
            println!(
                "Sending metrics for {}: accuracy={}, relevance={}, completeness={}, quality_score={}, passed={}",
                score.thread_id, score.accuracy, score.relevance, score.completeness, score.quality_score, score.passed
            );
        }

        let metrics = create_quality_metrics(score, tenant_id);
        producer.send_metrics(metrics).await?;
        sent_count += 1;

        // Small delay between messages
        tokio::time::sleep(Duration::from_millis(100)).await;
    }

    // Flush to ensure all messages are sent
    producer.flush(Duration::from_secs(5)).await?;

    if verbose {
        println!("All {} test metrics sent successfully!", sent_count);
    }

    Ok(sent_count)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_test_scores() {
        let scores = TestQualityScore::default_test_scores();
        assert_eq!(scores.len(), 3);

        // First score should be high quality, passed
        assert!(scores[0].quality_score > 0.9);
        assert!(scores[0].passed);

        // Last score should be lower quality, failed
        assert!(scores[2].quality_score < 0.8);
        assert!(!scores[2].passed);
    }

    #[test]
    fn test_create_quality_metrics() {
        let score = TestQualityScore {
            thread_id: "test-123".to_string(),
            accuracy: 0.9,
            relevance: 0.8,
            completeness: 0.85,
            quality_score: 0.85,
            passed: true,
        };

        let metrics = create_quality_metrics(&score, "test-tenant");

        assert_eq!(metrics.scope, "quality");
        assert_eq!(metrics.scope_id, "test-123");
        assert!(metrics.header.is_some());
        assert_eq!(metrics.metrics.len(), 5);
        assert!(metrics.metrics.contains_key("accuracy"));
        assert!(metrics.metrics.contains_key("quality_score"));
    }
}
