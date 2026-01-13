// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

//! Regression Detection
//!
//! Detects regressions by comparing current metrics to baseline.
//!
//! Regressions are classified by severity:
//! - **Critical**: Quality degradation (fail test)
//! - **Warning**: Performance/cost increase (warn but pass)
//! - **Info**: Minor deviation (informational)

use super::metrics::EvalMetrics;
use serde::{Deserialize, Serialize};

/// Thresholds for regression detection
///
/// # Examples
///
/// ```
/// use dashflow_streaming::evals::RegressionThresholds;
///
/// let thresholds = RegressionThresholds::default();
/// assert_eq!(thresholds.correctness_threshold, 0.05);
/// assert_eq!(thresholds.p95_latency_threshold, 1.2);
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegressionThresholds {
    // Quality thresholds (absolute difference, 0.0-1.0)
    /// Acceptable correctness drop (default: 0.05 = 5%)
    pub correctness_threshold: f64,

    /// Acceptable relevance drop (default: 0.10 = 10%)
    pub relevance_threshold: f64,

    /// Acceptable safety drop (default: 0.05 = 5%)
    pub safety_threshold: f64,

    /// Acceptable hallucination rate increase (default: 0.05 = 5%)
    pub hallucination_threshold: f64,

    // Performance thresholds (relative multiplier)
    /// Acceptable P95 latency increase (default: 1.2 = 20% slower)
    pub p95_latency_threshold: f64,

    /// Acceptable average latency increase (default: 1.2 = 20% slower)
    pub avg_latency_threshold: f64,

    /// Acceptable success rate drop (default: 0.05 = 5% drop)
    pub success_rate_threshold: f64,

    /// Acceptable error rate increase (default: 0.05 = 5% increase)
    pub error_rate_threshold: f64,

    // Cost thresholds (relative multiplier)
    /// Acceptable token usage increase (default: 1.5 = 50% more tokens)
    pub token_usage_threshold: f64,

    /// Acceptable cost increase (default: 1.5 = 50% more cost)
    pub cost_threshold: f64,

    /// Acceptable tool calls increase (default: 1.5 = 50% more calls)
    pub tool_calls_threshold: f64,
}

impl Default for RegressionThresholds {
    fn default() -> Self {
        Self {
            // Quality: strict thresholds (5-10% drop)
            correctness_threshold: 0.05,
            relevance_threshold: 0.10,
            safety_threshold: 0.05,
            hallucination_threshold: 0.05,
            // Performance: moderate thresholds (20% slower)
            p95_latency_threshold: 1.2,
            avg_latency_threshold: 1.2,
            success_rate_threshold: 0.05,
            error_rate_threshold: 0.05,
            // Cost: loose thresholds (50% increase)
            token_usage_threshold: 1.5,
            cost_threshold: 1.5,
            tool_calls_threshold: 1.5,
        }
    }
}

impl RegressionThresholds {
    /// Create strict thresholds (tighter than default)
    #[must_use]
    pub fn strict() -> Self {
        Self {
            correctness_threshold: 0.02,
            relevance_threshold: 0.05,
            safety_threshold: 0.02,
            hallucination_threshold: 0.02,
            p95_latency_threshold: 1.1,
            avg_latency_threshold: 1.1,
            success_rate_threshold: 0.02,
            error_rate_threshold: 0.02,
            token_usage_threshold: 1.2,
            cost_threshold: 1.2,
            tool_calls_threshold: 1.2,
        }
    }

    /// Create lenient thresholds (looser than default)
    #[must_use]
    pub fn lenient() -> Self {
        Self {
            correctness_threshold: 0.10,
            relevance_threshold: 0.15,
            safety_threshold: 0.10,
            hallucination_threshold: 0.10,
            p95_latency_threshold: 1.5,
            avg_latency_threshold: 1.5,
            success_rate_threshold: 0.10,
            error_rate_threshold: 0.10,
            token_usage_threshold: 2.0,
            cost_threshold: 2.0,
            tool_calls_threshold: 2.0,
        }
    }
}

/// Severity level for regressions
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RegressionSeverity {
    /// Critical regression (quality degradation, test should fail)
    Critical,

    /// Warning (performance/cost increase, test passes but warns)
    Warning,

    /// Informational (minor deviation)
    Info,
}

impl RegressionSeverity {
    /// Get emoji icon for severity
    #[must_use]
    pub fn icon(&self) -> &str {
        match self {
            Self::Critical => "✗",
            Self::Warning => "⚠",
            Self::Info => "ℹ",
        }
    }

    /// Get color for severity (ANSI color codes)
    #[must_use]
    pub fn color(&self) -> &str {
        match self {
            Self::Critical => "\x1b[31m", // Red
            Self::Warning => "\x1b[33m",  // Yellow
            Self::Info => "\x1b[36m",     // Cyan
        }
    }
}

/// A detected regression
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Regression {
    /// Metric name (e.g., "correctness", "`p95_latency`")
    pub metric: String,

    /// Baseline value
    pub baseline_value: f64,

    /// Current value
    pub current_value: f64,

    /// Threshold that was exceeded
    pub threshold: f64,

    /// Severity level
    pub severity: RegressionSeverity,

    /// Human-readable description
    pub description: String,
}

impl Regression {
    /// Format regression as colored text
    #[must_use]
    pub fn format_colored(&self) -> String {
        let icon = self.severity.icon();
        let color = self.severity.color();
        let reset = "\x1b[0m";

        format!(
            "{color}{icon} {metric}: {current:.4} (baseline: {baseline:.4}, {desc}){reset}",
            color = color,
            icon = icon,
            metric = self.metric,
            current = self.current_value,
            baseline = self.baseline_value,
            desc = self.description,
            reset = reset
        )
    }

    /// Format regression as plain text (no colors)
    #[must_use]
    pub fn format_plain(&self) -> String {
        let icon = self.severity.icon();
        format!(
            "{} {}: {:.4} (baseline: {:.4}, {})",
            icon, self.metric, self.current_value, self.baseline_value, self.description
        )
    }
}

/// Detect regressions by comparing current metrics to baseline
///
/// # Arguments
///
/// * `baseline` - Baseline metrics
/// * `current` - Current run metrics
/// * `thresholds` - Regression thresholds
///
/// # Returns
///
/// List of detected regressions, sorted by severity (Critical first)
///
/// # Examples
///
/// ```
/// use dashflow_streaming::evals::{EvalMetrics, RegressionThresholds, detect_regressions};
///
/// let baseline = EvalMetrics {
///     correctness: Some(0.95),
///     p95_latency: 1000.0,
///     total_tokens: 100,
///     ..Default::default()
/// };
///
/// let current = EvalMetrics {
///     correctness: Some(0.85),  // 10% drop (regression!)
///     p95_latency: 1500.0,       // 50% slower (regression!)
///     total_tokens: 120,         // 20% more (acceptable)
///     ..Default::default()
/// };
///
/// let thresholds = RegressionThresholds::default();
/// let regressions = detect_regressions(&baseline, &current, &thresholds);
///
/// assert_eq!(regressions.len(), 2);
/// assert_eq!(regressions[0].metric, "correctness");
/// ```
#[must_use]
pub fn detect_regressions(
    baseline: &EvalMetrics,
    current: &EvalMetrics,
    thresholds: &RegressionThresholds,
) -> Vec<Regression> {
    let mut regressions = Vec::new();

    // Check quality metrics (Critical severity)
    if let (Some(base), Some(curr)) = (baseline.correctness, current.correctness) {
        if curr < base - thresholds.correctness_threshold {
            let drop = base - curr;
            regressions.push(Regression {
                metric: "correctness".to_string(),
                baseline_value: base,
                current_value: curr,
                threshold: thresholds.correctness_threshold,
                severity: RegressionSeverity::Critical,
                description: format!("dropped {:.1}%", drop * 100.0),
            });
        }
    }

    if let (Some(base), Some(curr)) = (baseline.relevance, current.relevance) {
        if curr < base - thresholds.relevance_threshold {
            let drop = base - curr;
            regressions.push(Regression {
                metric: "relevance".to_string(),
                baseline_value: base,
                current_value: curr,
                threshold: thresholds.relevance_threshold,
                severity: RegressionSeverity::Critical,
                description: format!("dropped {:.1}%", drop * 100.0),
            });
        }
    }

    if let (Some(base), Some(curr)) = (baseline.safety, current.safety) {
        if curr < base - thresholds.safety_threshold {
            let drop = base - curr;
            regressions.push(Regression {
                metric: "safety".to_string(),
                baseline_value: base,
                current_value: curr,
                threshold: thresholds.safety_threshold,
                severity: RegressionSeverity::Critical,
                description: format!("dropped {:.1}%", drop * 100.0),
            });
        }
    }

    if let (Some(base), Some(curr)) = (baseline.hallucination_rate, current.hallucination_rate) {
        if curr > base + thresholds.hallucination_threshold {
            let increase = curr - base;
            regressions.push(Regression {
                metric: "hallucination_rate".to_string(),
                baseline_value: base,
                current_value: curr,
                threshold: thresholds.hallucination_threshold,
                severity: RegressionSeverity::Critical,
                description: format!("increased {:.1}%", increase * 100.0),
            });
        }
    }

    // Check performance metrics (Warning severity)
    if current.p95_latency > baseline.p95_latency * thresholds.p95_latency_threshold {
        let multiplier = current.p95_latency / baseline.p95_latency;
        regressions.push(Regression {
            metric: "p95_latency".to_string(),
            baseline_value: baseline.p95_latency,
            current_value: current.p95_latency,
            threshold: thresholds.p95_latency_threshold,
            severity: RegressionSeverity::Warning,
            description: format!("{multiplier:.1}x slower"),
        });
    }

    if current.avg_latency > baseline.avg_latency * thresholds.avg_latency_threshold {
        let multiplier = current.avg_latency / baseline.avg_latency;
        regressions.push(Regression {
            metric: "avg_latency".to_string(),
            baseline_value: baseline.avg_latency,
            current_value: current.avg_latency,
            threshold: thresholds.avg_latency_threshold,
            severity: RegressionSeverity::Warning,
            description: format!("{multiplier:.1}x slower"),
        });
    }

    if current.success_rate < baseline.success_rate - thresholds.success_rate_threshold {
        let drop = baseline.success_rate - current.success_rate;
        regressions.push(Regression {
            metric: "success_rate".to_string(),
            baseline_value: baseline.success_rate,
            current_value: current.success_rate,
            threshold: thresholds.success_rate_threshold,
            severity: RegressionSeverity::Warning,
            description: format!("dropped {:.1}%", drop * 100.0),
        });
    }

    if current.error_rate > baseline.error_rate + thresholds.error_rate_threshold {
        let increase = current.error_rate - baseline.error_rate;
        regressions.push(Regression {
            metric: "error_rate".to_string(),
            baseline_value: baseline.error_rate,
            current_value: current.error_rate,
            threshold: thresholds.error_rate_threshold,
            severity: RegressionSeverity::Warning,
            description: format!("increased {:.1}%", increase * 100.0),
        });
    }

    // Check cost metrics (Warning severity)
    if current.total_tokens as f64 > baseline.total_tokens as f64 * thresholds.token_usage_threshold
    {
        let multiplier = current.total_tokens as f64 / baseline.total_tokens as f64;
        regressions.push(Regression {
            metric: "total_tokens".to_string(),
            baseline_value: baseline.total_tokens as f64,
            current_value: current.total_tokens as f64,
            threshold: thresholds.token_usage_threshold,
            severity: RegressionSeverity::Warning,
            description: format!("{multiplier:.1}x more tokens"),
        });
    }

    if current.cost_per_run > baseline.cost_per_run * thresholds.cost_threshold {
        let multiplier = current.cost_per_run / baseline.cost_per_run;
        regressions.push(Regression {
            metric: "cost_per_run".to_string(),
            baseline_value: baseline.cost_per_run,
            current_value: current.cost_per_run,
            threshold: thresholds.cost_threshold,
            severity: RegressionSeverity::Warning,
            description: format!("{multiplier:.1}x more expensive"),
        });
    }

    if current.tool_calls as f64 > baseline.tool_calls as f64 * thresholds.tool_calls_threshold {
        let multiplier = current.tool_calls as f64 / baseline.tool_calls as f64;
        regressions.push(Regression {
            metric: "tool_calls".to_string(),
            baseline_value: baseline.tool_calls as f64,
            current_value: current.tool_calls as f64,
            threshold: thresholds.tool_calls_threshold,
            severity: RegressionSeverity::Warning,
            description: format!("{multiplier:.1}x more calls"),
        });
    }

    // Sort by severity (Critical first, then Warning, then Info)
    regressions.sort_by_key(|r| match r.severity {
        RegressionSeverity::Critical => 0,
        RegressionSeverity::Warning => 1,
        RegressionSeverity::Info => 2,
    });

    regressions
}

/// Check if there are any critical regressions
#[must_use]
pub fn has_critical_regressions(regressions: &[Regression]) -> bool {
    regressions
        .iter()
        .any(|r| matches!(r.severity, RegressionSeverity::Critical))
}

/// Count regressions by severity
#[must_use]
pub fn count_by_severity(regressions: &[Regression]) -> (usize, usize, usize) {
    let critical = regressions
        .iter()
        .filter(|r| matches!(r.severity, RegressionSeverity::Critical))
        .count();
    let warning = regressions
        .iter()
        .filter(|r| matches!(r.severity, RegressionSeverity::Warning))
        .count();
    let info = regressions
        .iter()
        .filter(|r| matches!(r.severity, RegressionSeverity::Info))
        .count();

    (critical, warning, info)
}

#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_thresholds() {
        let thresholds = RegressionThresholds::default();
        assert_eq!(thresholds.correctness_threshold, 0.05);
        assert_eq!(thresholds.p95_latency_threshold, 1.2);
        assert_eq!(thresholds.token_usage_threshold, 1.5);
    }

    #[test]
    fn test_strict_thresholds() {
        let thresholds = RegressionThresholds::strict();
        assert_eq!(thresholds.correctness_threshold, 0.02);
        assert_eq!(thresholds.p95_latency_threshold, 1.1);
    }

    #[test]
    fn test_detect_quality_regression() {
        let baseline = EvalMetrics {
            correctness: Some(0.95),
            ..Default::default()
        };

        let current = EvalMetrics {
            correctness: Some(0.85), // 10% drop
            ..Default::default()
        };

        let thresholds = RegressionThresholds::default();
        let regressions = detect_regressions(&baseline, &current, &thresholds);

        assert_eq!(regressions.len(), 1);
        assert_eq!(regressions[0].metric, "correctness");
        assert_eq!(regressions[0].severity, RegressionSeverity::Critical);
    }

    #[test]
    fn test_detect_performance_regression() {
        let baseline = EvalMetrics {
            p95_latency: 1000.0,
            ..Default::default()
        };

        let current = EvalMetrics {
            p95_latency: 1500.0, // 50% slower
            ..Default::default()
        };

        let thresholds = RegressionThresholds::default();
        let regressions = detect_regressions(&baseline, &current, &thresholds);

        assert_eq!(regressions.len(), 1);
        assert_eq!(regressions[0].metric, "p95_latency");
        assert_eq!(regressions[0].severity, RegressionSeverity::Warning);
    }

    #[test]
    fn test_detect_cost_regression() {
        let baseline = EvalMetrics {
            total_tokens: 100,
            ..Default::default()
        };

        let current = EvalMetrics {
            total_tokens: 200, // 2x tokens
            ..Default::default()
        };

        let thresholds = RegressionThresholds::default();
        let regressions = detect_regressions(&baseline, &current, &thresholds);

        assert_eq!(regressions.len(), 1);
        assert_eq!(regressions[0].metric, "total_tokens");
        assert_eq!(regressions[0].severity, RegressionSeverity::Warning);
    }

    #[test]
    fn test_no_regression_when_within_threshold() {
        let baseline = EvalMetrics {
            correctness: Some(0.95),
            p95_latency: 1000.0,
            total_tokens: 100,
            ..Default::default()
        };

        let current = EvalMetrics {
            correctness: Some(0.92), // 3% drop (< 5% threshold)
            p95_latency: 1100.0,     // 10% slower (< 20% threshold)
            total_tokens: 120,       // 20% more (< 50% threshold)
            ..Default::default()
        };

        let thresholds = RegressionThresholds::default();
        let regressions = detect_regressions(&baseline, &current, &thresholds);

        assert_eq!(regressions.len(), 0);
    }

    #[test]
    fn test_multiple_regressions_sorted_by_severity() {
        let baseline = EvalMetrics {
            correctness: Some(0.95),
            p95_latency: 1000.0,
            total_tokens: 100,
            ..Default::default()
        };

        let current = EvalMetrics {
            correctness: Some(0.85), // Critical
            p95_latency: 1500.0,     // Warning
            total_tokens: 200,       // Warning
            ..Default::default()
        };

        let thresholds = RegressionThresholds::default();
        let regressions = detect_regressions(&baseline, &current, &thresholds);

        assert_eq!(regressions.len(), 3);
        // First should be Critical
        assert_eq!(regressions[0].severity, RegressionSeverity::Critical);
        // Rest should be Warning
        assert_eq!(regressions[1].severity, RegressionSeverity::Warning);
        assert_eq!(regressions[2].severity, RegressionSeverity::Warning);
    }

    #[test]
    fn test_has_critical_regressions() {
        let baseline = EvalMetrics {
            correctness: Some(0.95),
            ..Default::default()
        };

        let current = EvalMetrics {
            correctness: Some(0.85),
            ..Default::default()
        };

        let thresholds = RegressionThresholds::default();
        let regressions = detect_regressions(&baseline, &current, &thresholds);

        assert!(has_critical_regressions(&regressions));
    }

    #[test]
    fn test_count_by_severity() {
        let baseline = EvalMetrics {
            correctness: Some(0.95),
            p95_latency: 1000.0,
            total_tokens: 100,
            ..Default::default()
        };

        let current = EvalMetrics {
            correctness: Some(0.85),
            p95_latency: 1500.0,
            total_tokens: 200,
            ..Default::default()
        };

        let thresholds = RegressionThresholds::default();
        let regressions = detect_regressions(&baseline, &current, &thresholds);

        let (critical, warning, info) = count_by_severity(&regressions);
        assert_eq!(critical, 1);
        assert_eq!(warning, 2);
        assert_eq!(info, 0);
    }

    #[test]
    fn test_regression_format() {
        let regression = Regression {
            metric: "correctness".to_string(),
            baseline_value: 0.95,
            current_value: 0.85,
            threshold: 0.05,
            severity: RegressionSeverity::Critical,
            description: "dropped 10.0%".to_string(),
        };

        let plain = regression.format_plain();
        assert!(plain.contains("correctness"));
        assert!(plain.contains("0.8500"));
        assert!(plain.contains("0.9500"));
        assert!(plain.contains("dropped 10.0%"));
    }
}
