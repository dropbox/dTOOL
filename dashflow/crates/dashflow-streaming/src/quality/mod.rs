// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

//! Quality Monitoring for DashFlow Streaming
//!
//! This module integrates LLM-as-judge quality evaluation directly into DashFlow Streaming telemetry.
//! Quality scores are automatically calculated and emitted as events for every agent response.
//!
//! # Architecture
//!
//! ```text
//! Graph Execution → QualityMonitor → Judge LLM → Emit Metrics → Kafka
//!                       ↓
//!                   (async, non-blocking)
//! ```
//!
//! # Features
//!
//! - **Automatic Quality Scoring**: Every response judged by LLM-as-judge
//! - **Non-Blocking**: Quality evaluation runs asynchronously
//! - **Telemetry Integration**: Quality scores emitted as DashFlow Streaming metrics
//! - **Alerting**: Low-quality responses trigger alerts
//! - **Production Ready**: Built-in quality gates for production deployment
//!
//! # Example
//!
//! ```rust,no_run
//! use dashflow_streaming::quality::QualityMonitor;
//! use dashflow_streaming::producer::DashStreamProducer;
//! use std::sync::Arc;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let producer = Arc::new(
//!     DashStreamProducer::new("localhost:9092", "dashstream-events").await?
//! );
//! // Create a judge implementation (e.g., using dashflow's OpenAIJudge)
//! // let judge = Arc::new(OpenAIJudge::new());
//! // let monitor = QualityMonitor::with_judge(producer, 0.95, judge);
//!
//! // After agent execution:
//! // monitor.evaluate_and_emit(
//! //     "thread-123",
//! //     "What is machine learning?",
//! //     "Machine learning is...",
//! //     &["AI", "algorithms", "data"],
//! //     None,
//! //     None,
//! // ).await?;
//! # Ok(())
//! # }
//! ```

use crate::errors::Error as DashStreamError;
use crate::producer::DashStreamProducer;
use crate::{AttributeValue, MessageType, MetricValue, Metrics};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tracing::debug;

/// Quality evaluation scores from LLM judge
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QualityScore {
    /// Accuracy: 0.0-1.0, is information factually correct?
    pub accuracy: f32,
    /// Relevance: 0.0-1.0, does it address the query?
    pub relevance: f32,
    /// Completeness: 0.0-1.0, covers all important aspects?
    pub completeness: f32,
    /// LLM's reasoning for the scores
    pub reasoning: String,
}

impl QualityScore {
    /// Calculate average quality score
    #[must_use]
    pub fn average(&self) -> f32 {
        (self.accuracy + self.relevance + self.completeness) / 3.0
    }

    /// Check if quality meets threshold
    #[must_use]
    pub fn meets_threshold(&self, threshold: f32) -> bool {
        self.accuracy >= threshold && self.relevance >= threshold && self.completeness >= threshold
    }
}

/// Quality issues detected in responses
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QualityIssue {
    /// Tool results were ignored ("couldn't find" when data exists)
    ToolResultsIgnored,
    /// Incomplete coverage of expected topics
    IncompleteCoverage,
    /// Low accuracy score
    LowAccuracy,
    /// Low relevance score
    LowRelevance,
    /// Low completeness score
    LowCompleteness,
}

impl QualityIssue {
    /// Returns a stable, machine-friendly identifier for this issue.
    #[must_use]
    pub fn as_str(&self) -> &str {
        match self {
            QualityIssue::ToolResultsIgnored => "tool_results_ignored",
            QualityIssue::IncompleteCoverage => "incomplete_coverage",
            QualityIssue::LowAccuracy => "low_accuracy",
            QualityIssue::LowRelevance => "low_relevance",
            QualityIssue::LowCompleteness => "low_completeness",
        }
    }
}

/// Quality monitor with integrated LLM-as-judge evaluation
///
/// This component:
/// 1. Evaluates responses using LLM-as-judge
/// 2. Emits quality scores to DashFlow Streaming telemetry
/// 3. Detects quality issues
/// 4. Triggers alerts for low-quality responses
pub struct QualityMonitor {
    /// DashFlow Streaming producer for emitting quality metrics
    producer: Arc<DashStreamProducer>,
    /// Quality threshold (0.0-1.0) for alerting
    threshold: f32,
    /// Judge model client (uses OpenAI-compatible API)
    judge_client: Option<Arc<dyn QualityJudge>>,
}

/// Trait for quality judge implementations
///
/// This allows different judge models (GPT-4, Claude, custom) to be used
#[async_trait::async_trait]
pub trait QualityJudge: Send + Sync {
    /// Judge response quality
    async fn judge_response(
        &self,
        query: &str,
        response: &str,
        expected_topics: &[&str],
        context: Option<&str>,
        tool_results: Option<&str>,
    ) -> Result<QualityScore, Box<dyn std::error::Error>>;

    /// Detect quality issues from score and response content
    fn detect_issues(&self, score: &QualityScore, response: &str) -> Vec<QualityIssue> {
        let mut issues = Vec::new();

        // Check for tool ignorance patterns
        let ignorance_patterns = [
            "couldn't find",
            "wasn't able to find",
            "no information available",
            "I don't have access",
        ];

        for pattern in ignorance_patterns {
            if response.to_lowercase().contains(pattern) {
                issues.push(QualityIssue::ToolResultsIgnored);
                break;
            }
        }

        // Check individual scores against threshold (0.7)
        if score.accuracy < 0.7 {
            issues.push(QualityIssue::LowAccuracy);
        }
        if score.relevance < 0.7 {
            issues.push(QualityIssue::LowRelevance);
        }
        if score.completeness < 0.7 {
            issues.push(QualityIssue::LowCompleteness);
            issues.push(QualityIssue::IncompleteCoverage);
        }

        issues
    }
}

impl QualityMonitor {
    /// Create new quality monitor
    ///
    /// # Arguments
    /// * `producer` - DashFlow Streaming producer for emitting metrics
    /// * `threshold` - Quality threshold for alerting (0.0-1.0)
    #[must_use]
    pub fn new(producer: Arc<DashStreamProducer>, threshold: f32) -> Self {
        Self {
            producer,
            threshold,
            judge_client: None,
        }
    }

    /// Create quality monitor with custom judge
    pub fn with_judge(
        producer: Arc<DashStreamProducer>,
        threshold: f32,
        judge: Arc<dyn QualityJudge>,
    ) -> Self {
        Self {
            producer,
            threshold,
            judge_client: Some(judge),
        }
    }

    /// Evaluate response quality and emit metrics to DashFlow Streaming
    ///
    /// This method:
    /// 1. Judges the response using LLM-as-judge
    /// 2. Detects quality issues
    /// 3. Emits quality metrics to Kafka
    /// 4. Triggers alerts if quality below threshold
    ///
    /// # Arguments
    /// * `thread_id` - Session/thread ID
    /// * `query` - User query
    /// * `response` - Agent response
    /// * `expected_topics` - Topics that should be covered
    /// * `context` - Optional conversation context
    ///
    /// # Returns
    /// Quality score and detected issues
    pub async fn evaluate_and_emit(
        &self,
        thread_id: &str,
        query: &str,
        response: &str,
        expected_topics: &[&str],
        context: Option<&str>,
        tool_results: Option<&str>,
    ) -> Result<(QualityScore, Vec<QualityIssue>), DashStreamError> {
        // If no judge configured, skip evaluation gracefully.
        let Some(ref judge) = self.judge_client else {
            debug!(thread_id = %thread_id, "No quality judge configured; skipping evaluation");
            let sentinel = QualityScore {
                accuracy: 1.0,
                relevance: 1.0,
                completeness: 1.0,
                reasoning: "no_judge_configured".to_string(),
            };
            return Ok((sentinel, Vec::new()));
        };

        // Judge the response (async) with a hard timeout.
        const JUDGE_TIMEOUT: Duration = Duration::from_secs(30);
        let score = match tokio::time::timeout(
            JUDGE_TIMEOUT,
            judge.judge_response(query, response, expected_topics, context, tool_results),
        )
        .await
        {
            Ok(res) => res.map_err(|e| {
                DashStreamError::Io(std::io::Error::other(format!("Judge failed: {e}")))
            })?,
            Err(_) => {
                return Err(DashStreamError::Io(std::io::Error::other(format!(
                    "Judge timed out after {:?}",
                    JUDGE_TIMEOUT
                ))))
            }
        };

        // Detect quality issues
        let issues = judge.detect_issues(&score, response);

        // Emit quality metrics to DashFlow Streaming
        self.emit_quality_metrics(thread_id, &score, &issues)
            .await?;

        // Check threshold and emit alert if needed
        if score.average() < self.threshold {
            self.emit_quality_alert(thread_id, &score, &issues).await?;
        }

        Ok((score, issues))
    }

    /// Emit quality metrics as DashFlow Streaming Metrics message
    async fn emit_quality_metrics(
        &self,
        thread_id: &str,
        score: &QualityScore,
        issues: &[QualityIssue],
    ) -> Result<(), DashStreamError> {
        let mut metrics_map = HashMap::new();

        // Add quality scores
        metrics_map.insert(
            "quality_accuracy".to_string(),
            MetricValue {
                value: Some(crate::metric_value::Value::FloatValue(f64::from(
                    score.accuracy,
                ))),
                unit: "score".to_string(),
                r#type: crate::metric_value::MetricType::Gauge as i32,
            },
        );

        metrics_map.insert(
            "quality_relevance".to_string(),
            MetricValue {
                value: Some(crate::metric_value::Value::FloatValue(f64::from(
                    score.relevance,
                ))),
                unit: "score".to_string(),
                r#type: crate::metric_value::MetricType::Gauge as i32,
            },
        );

        metrics_map.insert(
            "quality_completeness".to_string(),
            MetricValue {
                value: Some(crate::metric_value::Value::FloatValue(f64::from(
                    score.completeness,
                ))),
                unit: "score".to_string(),
                r#type: crate::metric_value::MetricType::Gauge as i32,
            },
        );

        metrics_map.insert(
            "quality_average".to_string(),
            MetricValue {
                value: Some(crate::metric_value::Value::FloatValue(f64::from(
                    score.average(),
                ))),
                unit: "score".to_string(),
                r#type: crate::metric_value::MetricType::Gauge as i32,
            },
        );

        // Add issue count
        metrics_map.insert(
            "quality_issues_count".to_string(),
            MetricValue {
                value: Some(crate::metric_value::Value::IntValue(issues.len() as i64)),
                unit: "count".to_string(),
                r#type: crate::metric_value::MetricType::Counter as i32,
            },
        );

        // Create tags for detected issues
        let mut tags = HashMap::new();
        for issue in issues {
            tags.insert(issue.as_str().to_string(), "true".to_string());
        }

        // Create Metrics message
        let metrics = Metrics {
            header: Some(self.producer.create_header(thread_id, MessageType::Metrics)),
            scope: "quality".to_string(),
            scope_id: "response_evaluation".to_string(),
            metrics: metrics_map,
            tags,
        };

        // Emit to Kafka
        self.producer.send_metrics(metrics).await?;

        Ok(())
    }

    /// Emit quality alert for low-quality responses
    async fn emit_quality_alert(
        &self,
        thread_id: &str,
        score: &QualityScore,
        issues: &[QualityIssue],
    ) -> Result<(), DashStreamError> {
        // Create alert as Event with attributes
        let mut attributes = HashMap::new();

        attributes.insert(
            "alert_type".to_string(),
            AttributeValue {
                value: Some(crate::attribute_value::Value::StringValue(
                    "LOW_QUALITY".to_string(),
                )),
            },
        );

        attributes.insert(
            "quality_score".to_string(),
            AttributeValue {
                value: Some(crate::attribute_value::Value::FloatValue(f64::from(
                    score.average(),
                ))),
            },
        );

        attributes.insert(
            "threshold".to_string(),
            AttributeValue {
                value: Some(crate::attribute_value::Value::FloatValue(f64::from(
                    self.threshold,
                ))),
            },
        );

        // Add issues as array
        let issue_strs: Vec<_> = issues.iter().map(|i| i.as_str().to_string()).collect();
        attributes.insert(
            "issues".to_string(),
            AttributeValue {
                value: Some(crate::attribute_value::Value::StringValue(
                    issue_strs.join(", "),
                )),
            },
        );

        // Emit event
        let event = crate::Event {
            header: Some(self.producer.create_header(thread_id, MessageType::Event)),
            event_type: crate::EventType::GraphError as i32, // Use GRAPH_ERROR for alerts
            node_id: "quality_monitor".to_string(),
            attributes,
            duration_us: 0,
            llm_request_id: String::new(),
        };

        self.producer.send_event(event).await?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockJudge;

    #[async_trait::async_trait]
    impl QualityJudge for MockJudge {
        async fn judge_response(
            &self,
            _query: &str,
            _response: &str,
            _expected_topics: &[&str],
            _context: Option<&str>,
            _tool_results: Option<&str>,
        ) -> Result<QualityScore, Box<dyn std::error::Error>> {
            Ok(QualityScore {
                accuracy: 0.95,
                relevance: 0.92,
                completeness: 0.88,
                reasoning: "Good response".to_string(),
            })
        }
    }

    #[test]
    fn test_quality_score_average() {
        let score = QualityScore {
            accuracy: 0.9,
            relevance: 0.8,
            completeness: 0.7,
            reasoning: "test".to_string(),
        };

        assert_eq!(score.average(), 0.8);
    }

    #[test]
    fn test_quality_score_meets_threshold() {
        let score = QualityScore {
            accuracy: 0.9,
            relevance: 0.85,
            completeness: 0.8,
            reasoning: "test".to_string(),
        };

        assert!(score.meets_threshold(0.7));
        assert!(!score.meets_threshold(0.9));
    }

    #[test]
    fn test_detect_tool_ignorance() {
        let judge = MockJudge;
        let score = QualityScore {
            accuracy: 0.9,
            relevance: 0.9,
            completeness: 0.9,
            reasoning: "test".to_string(),
        };

        let response1 = "I couldn't find any information about that.";
        let issues1 = judge.detect_issues(&score, response1);
        assert!(issues1.contains(&QualityIssue::ToolResultsIgnored));

        let response2 = "Based on the documentation, the answer is...";
        let issues2 = judge.detect_issues(&score, response2);
        assert!(!issues2.contains(&QualityIssue::ToolResultsIgnored));
    }

    #[test]
    fn test_detect_low_scores() {
        let judge = MockJudge;
        let score = QualityScore {
            accuracy: 0.5,
            relevance: 0.6,
            completeness: 0.4,
            reasoning: "test".to_string(),
        };

        let issues = judge.detect_issues(&score, "Normal response");
        assert!(issues.contains(&QualityIssue::LowAccuracy));
        assert!(issues.contains(&QualityIssue::LowRelevance));
        assert!(issues.contains(&QualityIssue::LowCompleteness));
    }
}
