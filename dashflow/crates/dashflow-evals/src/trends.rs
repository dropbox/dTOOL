//! Trend analysis for tracking evaluation metrics over time.
//!
//! This module provides tools to analyze trends in evaluation results across
//! multiple runs, identify patterns, detect anomalies, and forecast future quality.

use crate::eval_runner::EvalReport;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Historical data point for trend analysis.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataPoint {
    /// Timestamp of this evaluation
    pub timestamp: DateTime<Utc>,

    /// Git commit hash (if available)
    pub git_commit: Option<String>,

    /// Average quality score
    pub quality: f64,

    /// Average latency in milliseconds
    pub latency_ms: u64,

    /// Pass rate (0.0-1.0)
    pub pass_rate: f64,

    /// Number of scenarios evaluated
    pub scenario_count: usize,
}

impl DataPoint {
    /// Create a data point from an evaluation report.
    #[must_use]
    pub fn from_report(report: &EvalReport, git_commit: Option<String>) -> Self {
        Self {
            timestamp: report.metadata.completed_at,
            git_commit,
            quality: report.avg_quality(),
            latency_ms: report.avg_latency_ms(),
            pass_rate: report.pass_rate(),
            scenario_count: report.total,
        }
    }
}

/// Direction of a trend (improving, degrading, or stable).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TrendDirection {
    /// Metric is improving over time
    Improving,

    /// Metric is degrading over time
    Degrading,

    /// Metric is stable (no significant trend)
    Stable,
}

/// Trend information for a specific metric.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrendInfo {
    /// Metric name
    pub metric: String,

    /// Direction of the trend
    pub direction: TrendDirection,

    /// Slope of the trend line (per day)
    pub slope: f64,

    /// Statistical confidence in the trend (0.0-1.0)
    pub confidence: f64,

    /// Description of the trend
    pub description: String,
}

/// Detected anomaly in evaluation history.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Anomaly {
    /// Data point where anomaly was detected
    pub data_point: DataPoint,

    /// Metric that showed anomalous behavior
    pub metric: String,

    /// Expected value based on trend
    pub expected_value: f64,

    /// Actual observed value
    pub actual_value: f64,

    /// Deviation from expected (in standard deviations)
    pub deviation: f64,

    /// Description of the anomaly
    pub description: String,
}

/// Quality forecast for future evaluations.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QualityForecast {
    /// Predicted quality score
    pub predicted_quality: f64,

    /// Confidence interval lower bound
    pub confidence_lower: f64,

    /// Confidence interval upper bound
    pub confidence_upper: f64,

    /// Number of data points ahead this forecast is for
    pub n_ahead: usize,

    /// Warning if quality is predicted to drop below threshold
    pub warning: Option<String>,
}

/// Complete trend analysis report.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrendReport {
    /// Trends for each metric
    pub trends: Vec<TrendInfo>,

    /// Detected anomalies
    pub anomalies: Vec<Anomaly>,

    /// Overall quality trend direction
    pub overall_direction: TrendDirection,

    /// Summary statistics
    pub summary: TrendSummary,
}

/// Summary statistics for trend analysis.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrendSummary {
    /// Number of historical data points
    pub data_point_count: usize,

    /// First data point timestamp
    pub first_timestamp: DateTime<Utc>,

    /// Last data point timestamp
    pub last_timestamp: DateTime<Utc>,

    /// Time span in days
    pub time_span_days: f64,

    /// Quality at start of period
    pub quality_start: f64,

    /// Quality at end of period
    pub quality_end: f64,

    /// Overall quality change
    pub quality_change: f64,

    /// Average quality across all data points
    pub quality_mean: f64,
}

/// Analyzer for trends in evaluation results over time.
pub struct TrendAnalyzer {
    /// Historical data points
    data_points: Vec<DataPoint>,

    /// Minimum number of data points needed for trend analysis
    min_data_points: usize,

    /// Anomaly detection threshold (in standard deviations)
    anomaly_threshold: f64,
}

impl TrendAnalyzer {
    /// Create a new trend analyzer.
    #[must_use]
    pub fn new() -> Self {
        Self {
            data_points: Vec::new(),
            min_data_points: 3,
            anomaly_threshold: 2.5, // 2.5 standard deviations
        }
    }

    /// Create a trend analyzer with custom configuration.
    #[must_use]
    pub fn with_config(min_data_points: usize, anomaly_threshold: f64) -> Self {
        Self {
            data_points: Vec::new(),
            min_data_points,
            anomaly_threshold,
        }
    }

    /// Add a data point from an evaluation report.
    pub fn add_report(&mut self, report: &EvalReport, git_commit: Option<String>) {
        let data_point = DataPoint::from_report(report, git_commit);
        self.data_points.push(data_point);
        self.sort_data_points();
    }

    /// Add a data point directly.
    pub fn add_data_point(&mut self, data_point: DataPoint) {
        self.data_points.push(data_point);
        self.sort_data_points();
    }

    /// Load historical data points from evaluation reports.
    pub fn load_reports(&mut self, reports: Vec<(EvalReport, Option<String>)>) {
        for (report, git_commit) in reports {
            self.add_report(&report, git_commit);
        }
    }

    /// Sort data points by timestamp (oldest first).
    fn sort_data_points(&mut self) {
        self.data_points.sort_by_key(|dp| dp.timestamp);
    }

    /// Analyze trends across all metrics.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use dashflow_evals::{TrendAnalyzer, EvalReport, EvalMetadata};
    /// # use chrono::Utc;
    /// let mut analyzer = TrendAnalyzer::new();
    ///
    /// // Add multiple evaluation results over time
    /// # let report = EvalReport {
    /// #     total: 50, passed: 48, failed: 2, results: vec![],
    /// #     metadata: EvalMetadata {
    /// #         started_at: Utc::now(), completed_at: Utc::now(),
    /// #         duration_secs: 120.5, config: "{}".to_string(),
    /// #     },
    /// # };
    /// analyzer.add_report(&report, Some("abc123".to_string()));
    ///
    /// // Analyze trends
    /// let trend_report = analyzer.analyze();
    /// println!("Overall trend: {:?}", trend_report.overall_direction);
    /// ```
    #[must_use]
    pub fn analyze(&self) -> TrendReport {
        if self.data_points.len() < self.min_data_points {
            return self.empty_report();
        }

        let mut trends = Vec::new();

        // Analyze quality trend
        let quality_values: Vec<f64> = self.data_points.iter().map(|dp| dp.quality).collect();
        let quality_trend = self.analyze_metric_trend("quality", &quality_values);
        trends.push(quality_trend);

        // Analyze latency trend
        let latency_values: Vec<f64> = self
            .data_points
            .iter()
            .map(|dp| dp.latency_ms as f64)
            .collect();
        let latency_trend = self.analyze_metric_trend("latency", &latency_values);
        trends.push(latency_trend);

        // Analyze pass rate trend
        let pass_rate_values: Vec<f64> = self.data_points.iter().map(|dp| dp.pass_rate).collect();
        let pass_rate_trend = self.analyze_metric_trend("pass_rate", &pass_rate_values);
        trends.push(pass_rate_trend);

        // Detect anomalies
        let anomalies = self.detect_anomalies();

        // Determine overall direction
        let overall_direction = self.determine_overall_direction(&trends);

        // Calculate summary statistics
        let summary = self.calculate_summary();

        TrendReport {
            trends,
            anomalies,
            overall_direction,
            summary,
        }
    }

    /// Analyze trend for a specific metric.
    fn analyze_metric_trend(&self, metric: &str, values: &[f64]) -> TrendInfo {
        if values.len() < 2 {
            return TrendInfo {
                metric: metric.to_string(),
                direction: TrendDirection::Stable,
                slope: 0.0,
                confidence: 0.0,
                description: "Insufficient data for trend analysis".to_string(),
            };
        }

        // Calculate time points in days since first data point
        let time_points: Vec<f64> = self
            .data_points
            .iter()
            .map(|dp| (dp.timestamp - self.data_points[0].timestamp).num_seconds() as f64 / 86400.0)
            .collect();

        // Perform linear regression
        let (slope, _intercept, r_squared) = self.linear_regression(&time_points, values);

        // Determine direction
        let direction = if slope.abs() < 0.001 {
            TrendDirection::Stable
        } else if slope > 0.0 {
            if metric == "latency" {
                TrendDirection::Degrading // Higher latency is bad
            } else {
                TrendDirection::Improving // Higher quality/pass rate is good
            }
        } else if metric == "latency" {
            TrendDirection::Improving // Lower latency is good
        } else {
            TrendDirection::Degrading // Lower quality/pass rate is bad
        };

        // Generate description
        let description = match direction {
            TrendDirection::Improving => {
                format!("{metric} is improving over time (slope: {slope:.4} per day)")
            }
            TrendDirection::Degrading => {
                format!("{metric} is degrading over time (slope: {slope:.4} per day)")
            }
            TrendDirection::Stable => {
                format!("{metric} is stable (slope: {slope:.4} per day)")
            }
        };

        TrendInfo {
            metric: metric.to_string(),
            direction,
            slope,
            confidence: r_squared, // R² as confidence measure
            description,
        }
    }

    /// Perform linear regression on (x, y) data.
    ///
    /// Returns (slope, intercept, `r_squared`).
    fn linear_regression(&self, x: &[f64], y: &[f64]) -> (f64, f64, f64) {
        let n = x.len() as f64;

        if n < 2.0 {
            return (0.0, 0.0, 0.0);
        }

        // Calculate means
        let x_mean: f64 = x.iter().sum::<f64>() / n;
        let y_mean: f64 = y.iter().sum::<f64>() / n;

        // Calculate slope and intercept
        let mut numerator = 0.0;
        let mut denominator = 0.0;

        for i in 0..x.len() {
            let x_diff = x[i] - x_mean;
            let y_diff = y[i] - y_mean;
            numerator += x_diff * y_diff;
            denominator += x_diff * x_diff;
        }

        let slope = if denominator == 0.0 {
            0.0
        } else {
            numerator / denominator
        };

        let intercept = y_mean - slope * x_mean;

        // Calculate R²
        let mut ss_tot = 0.0;
        let mut ss_res = 0.0;

        for i in 0..x.len() {
            let y_pred = slope * x[i] + intercept;
            ss_tot += (y[i] - y_mean).powi(2);
            ss_res += (y[i] - y_pred).powi(2);
        }

        let r_squared = if ss_tot == 0.0 {
            0.0
        } else {
            1.0 - (ss_res / ss_tot)
        };

        (slope, intercept, r_squared)
    }

    /// Detect anomalies in the data.
    fn detect_anomalies(&self) -> Vec<Anomaly> {
        if self.data_points.len() < self.min_data_points {
            return Vec::new();
        }

        let mut anomalies = Vec::new();

        // Check quality anomalies
        let quality_values: Vec<f64> = self.data_points.iter().map(|dp| dp.quality).collect();
        anomalies.extend(self.detect_metric_anomalies("quality", &quality_values));

        // Check latency anomalies
        let latency_values: Vec<f64> = self
            .data_points
            .iter()
            .map(|dp| dp.latency_ms as f64)
            .collect();
        anomalies.extend(self.detect_metric_anomalies("latency", &latency_values));

        anomalies
    }

    /// Detect anomalies for a specific metric.
    fn detect_metric_anomalies(&self, metric: &str, values: &[f64]) -> Vec<Anomaly> {
        if values.is_empty() {
            return Vec::new();
        }

        let mut anomalies = Vec::new();

        // Calculate mean and standard deviation
        let mean = values.iter().sum::<f64>() / values.len() as f64;
        let variance = values.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / values.len() as f64;
        let std_dev = variance.sqrt();

        if std_dev == 0.0 {
            return anomalies; // No variation - can't detect anomalies
        }

        // Check each data point for anomalies
        for (i, &value) in values.iter().enumerate() {
            let deviation = (value - mean).abs() / std_dev;

            if deviation > self.anomaly_threshold {
                anomalies.push(Anomaly {
                    data_point: self.data_points[i].clone(),
                    metric: metric.to_string(),
                    expected_value: mean,
                    actual_value: value,
                    deviation,
                    description: format!(
                        "{metric} anomaly detected: value={value:.3}, expected={mean:.3} (±{std_dev:.3}), deviation={deviation:.1}σ"
                    ),
                });
            }
        }

        anomalies
    }

    /// Determine overall trend direction from individual metric trends.
    fn determine_overall_direction(&self, trends: &[TrendInfo]) -> TrendDirection {
        // Focus on quality trend as the primary indicator
        if let Some(quality_trend) = trends.iter().find(|t| t.metric == "quality") {
            return quality_trend.direction.clone();
        }

        TrendDirection::Stable
    }

    /// Calculate summary statistics.
    fn calculate_summary(&self) -> TrendSummary {
        if self.data_points.is_empty() {
            return TrendSummary {
                data_point_count: 0,
                first_timestamp: Utc::now(),
                last_timestamp: Utc::now(),
                time_span_days: 0.0,
                quality_start: 0.0,
                quality_end: 0.0,
                quality_change: 0.0,
                quality_mean: 0.0,
            };
        }

        let first = &self.data_points[0];
        let last = &self.data_points[self.data_points.len() - 1];

        let time_span_days = (last.timestamp - first.timestamp).num_seconds() as f64 / 86400.0;

        let quality_values: Vec<f64> = self.data_points.iter().map(|dp| dp.quality).collect();
        let quality_mean = quality_values.iter().sum::<f64>() / quality_values.len() as f64;

        TrendSummary {
            data_point_count: self.data_points.len(),
            first_timestamp: first.timestamp,
            last_timestamp: last.timestamp,
            time_span_days,
            quality_start: first.quality,
            quality_end: last.quality,
            quality_change: last.quality - first.quality,
            quality_mean,
        }
    }

    /// Create an empty report when insufficient data is available.
    fn empty_report(&self) -> TrendReport {
        TrendReport {
            trends: Vec::new(),
            anomalies: Vec::new(),
            overall_direction: TrendDirection::Stable,
            summary: self.calculate_summary(),
        }
    }

    /// Forecast future quality based on historical trends.
    ///
    /// # Arguments
    ///
    /// * `n_ahead` - Number of data points ahead to forecast
    /// * `quality_threshold` - Threshold to warn if quality is predicted to drop below
    #[must_use]
    pub fn forecast(&self, n_ahead: usize, quality_threshold: f64) -> QualityForecast {
        if self.data_points.len() < self.min_data_points {
            return QualityForecast {
                predicted_quality: 0.0,
                confidence_lower: 0.0,
                confidence_upper: 0.0,
                n_ahead,
                warning: Some("Insufficient data for forecasting".to_string()),
            };
        }

        // Get quality values and time points
        let quality_values: Vec<f64> = self.data_points.iter().map(|dp| dp.quality).collect();
        let time_points: Vec<f64> = self
            .data_points
            .iter()
            .map(|dp| (dp.timestamp - self.data_points[0].timestamp).num_seconds() as f64 / 86400.0)
            .collect();

        // Perform linear regression
        let (slope, intercept, _r_squared) = self.linear_regression(&time_points, &quality_values);

        // Calculate prediction
        let last_time = time_points.last().unwrap_or(&0.0);
        let predicted_time = last_time + (n_ahead as f64 * 1.0); // Assuming 1 day between data points
        let predicted_quality = slope * predicted_time + intercept;

        // Calculate standard error for confidence interval
        let residuals: Vec<f64> = time_points
            .iter()
            .zip(quality_values.iter())
            .map(|(t, q)| q - (slope * t + intercept))
            .collect();

        let mse = residuals.iter().map(|r| r.powi(2)).sum::<f64>() / residuals.len() as f64;
        let std_error = mse.sqrt();

        // 95% confidence interval (approximately ±1.96 * std_error)
        let confidence_lower = predicted_quality - 1.96 * std_error;
        let confidence_upper = predicted_quality + 1.96 * std_error;

        // Check for warning
        let warning = if predicted_quality < quality_threshold {
            Some(format!(
                "Warning: Quality predicted to drop below threshold ({predicted_quality:.3} < {quality_threshold:.3})"
            ))
        } else {
            None
        };

        QualityForecast {
            predicted_quality,
            confidence_lower,
            confidence_upper,
            n_ahead,
            warning,
        }
    }

    /// Get the number of data points in the analyzer.
    #[must_use]
    pub fn data_point_count(&self) -> usize {
        self.data_points.len()
    }

    /// Clear all data points.
    pub fn clear(&mut self) {
        self.data_points.clear();
    }
}

impl Default for TrendAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
#[cfg(test)]
mod tests {
    use super::*;
    use crate::eval_runner::EvalMetadata;

    fn create_test_report_with_time(
        quality: f64,
        latency_ms: u64,
        pass_rate: f64,
        timestamp: DateTime<Utc>,
    ) -> EvalReport {
        use crate::eval_runner::{ScenarioResult, ValidationResult};
        use crate::quality_judge::QualityScore;

        let total = 50;
        let passed = (total as f64 * pass_rate) as usize;
        let failed = total - passed;

        // Create mock results with the specified quality and latency
        let mut results = Vec::new();
        for i in 0..total {
            let is_passed = i < passed;
            results.push(ScenarioResult {
                scenario_id: format!("s{}", i),
                passed: is_passed,
                output: "test output".to_string(),
                quality_score: QualityScore {
                    accuracy: quality,
                    relevance: quality,
                    completeness: quality,
                    safety: quality,
                    coherence: quality,
                    conciseness: quality,
                    overall: quality,
                    reasoning: "test".to_string(),
                    issues: vec![],
                    suggestions: vec![],
                },
                latency_ms,
                validation: ValidationResult {
                    passed: is_passed,
                    missing_contains: vec![],
                    forbidden_found: vec![],
                    failure_reason: None,
                },
                error: None,
                retry_attempts: 0,
                timestamp,
                input: None,
                tokens_used: None,
                cost_usd: None,
            });
        }

        EvalReport {
            total,
            passed,
            failed,
            results,
            metadata: EvalMetadata {
                started_at: timestamp,
                completed_at: timestamp,
                duration_secs: 120.0,
                config: "{}".to_string(),
            },
        }
    }

    #[test]
    fn test_trend_analyzer_insufficient_data() {
        let analyzer = TrendAnalyzer::new();
        let report = analyzer.analyze();

        assert_eq!(report.trends.len(), 0);
        assert_eq!(report.overall_direction, TrendDirection::Stable);
    }

    #[test]
    fn test_quality_improving_trend() {
        let mut analyzer = TrendAnalyzer::new();

        let base_time = Utc::now();

        // Add improving quality over time (one data point per day)
        for i in 0..5 {
            let quality = 0.80 + (i as f64 * 0.03); // Increasing
            let timestamp = base_time + chrono::Duration::days(i);
            let report = create_test_report_with_time(quality, 100, 0.95, timestamp);
            analyzer.add_report(&report, None);
        }

        let trend_report = analyzer.analyze();

        assert!(!trend_report.trends.is_empty());

        let quality_trend = trend_report
            .trends
            .iter()
            .find(|t| t.metric == "quality")
            .unwrap();
        assert_eq!(quality_trend.direction, TrendDirection::Improving);
        assert!(quality_trend.slope > 0.0);
    }

    #[test]
    fn test_quality_degrading_trend() {
        let mut analyzer = TrendAnalyzer::new();

        let base_time = Utc::now();

        // Add degrading quality over time
        for i in 0..5 {
            let quality = 0.95 - (i as f64 * 0.03); // Decreasing
            let timestamp = base_time + chrono::Duration::days(i);
            let report = create_test_report_with_time(quality, 100, 0.90, timestamp);
            analyzer.add_report(&report, None);
        }

        let trend_report = analyzer.analyze();

        let quality_trend = trend_report
            .trends
            .iter()
            .find(|t| t.metric == "quality")
            .unwrap();
        assert_eq!(quality_trend.direction, TrendDirection::Degrading);
        assert!(quality_trend.slope < 0.0);
    }

    #[test]
    fn test_anomaly_detection() {
        let mut analyzer = TrendAnalyzer::new();

        let base_time = Utc::now();

        // Add mostly consistent quality
        for i in 0..10 {
            let timestamp = base_time + chrono::Duration::days(i);
            let report = create_test_report_with_time(0.90, 100, 0.95, timestamp);
            analyzer.add_report(&report, None);
        }

        // Add anomalous data point
        let timestamp = base_time + chrono::Duration::days(10);
        let anomalous_report = create_test_report_with_time(0.50, 100, 0.95, timestamp); // Significantly lower
        analyzer.add_report(&anomalous_report, None);

        let trend_report = analyzer.analyze();

        // Should detect quality anomaly
        assert!(!trend_report.anomalies.is_empty());
        let quality_anomaly = trend_report
            .anomalies
            .iter()
            .find(|a| a.metric == "quality");
        assert!(quality_anomaly.is_some());
    }

    #[test]
    fn test_forecast() {
        let mut analyzer = TrendAnalyzer::new();

        let base_time = Utc::now();

        // Add improving quality trend
        for i in 0..5 {
            let quality = 0.80 + (i as f64 * 0.02);
            let timestamp = base_time + chrono::Duration::days(i);
            let report = create_test_report_with_time(quality, 100, 0.95, timestamp);
            analyzer.add_report(&report, None);
        }

        let forecast = analyzer.forecast(3, 0.85);

        // Should predict continued improvement
        assert!(forecast.predicted_quality > 0.85);
        assert!(forecast.confidence_lower < forecast.predicted_quality);
        assert!(forecast.confidence_upper > forecast.predicted_quality);
    }

    #[test]
    fn test_forecast_warning() {
        let mut analyzer = TrendAnalyzer::new();

        let base_time = Utc::now();

        // Add degrading quality trend
        for i in 0..5 {
            let quality = 0.95 - (i as f64 * 0.03);
            let timestamp = base_time + chrono::Duration::days(i);
            let report = create_test_report_with_time(quality, 100, 0.90, timestamp);
            analyzer.add_report(&report, None);
        }

        let forecast = analyzer.forecast(5, 0.85);

        // Should warn about predicted drop
        assert!(forecast.warning.is_some());
    }

    #[test]
    fn test_summary_statistics() {
        let mut analyzer = TrendAnalyzer::new();

        let base_time = Utc::now();

        for i in 0..10 {
            let quality = 0.90 + (i as f64 * 0.01);
            let timestamp = base_time + chrono::Duration::days(i);
            let report = create_test_report_with_time(quality, 100, 0.95, timestamp);
            analyzer.add_report(&report, None);
        }

        let trend_report = analyzer.analyze();

        assert_eq!(trend_report.summary.data_point_count, 10);
        assert!(trend_report.summary.quality_end > trend_report.summary.quality_start);
        assert!(trend_report.summary.quality_change > 0.0);
    }
}
