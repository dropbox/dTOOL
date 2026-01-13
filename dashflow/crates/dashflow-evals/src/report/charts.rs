//! Chart generation for visual analysis of evaluation results.
//!
//! Generates SVG charts for:
//! - Quality score distribution (histogram)
//! - Latency distribution across scenarios
//! - Quality trends over time
//! - Pass/fail breakdown by category

use crate::eval_runner::EvalReport;
use anyhow::{Context, Result};
use plotters::prelude::*;
use std::path::Path;

/// Chart generator for evaluation visualizations
pub struct ChartGenerator;

impl ChartGenerator {
    /// Generate quality distribution histogram
    ///
    /// Creates a histogram showing the distribution of quality scores across all scenarios.
    /// Helps identify if scores cluster around high quality or are more varied.
    ///
    /// # Arguments
    ///
    /// * `report` - Evaluation report
    /// * `output_path` - Where to save the SVG file
    ///
    /// # Example
    ///
    /// ```no_run
    /// use dashflow_evals::report::charts::ChartGenerator;
    /// # use dashflow_evals::eval_runner::EvalReport;
    /// # fn example(report: EvalReport) -> anyhow::Result<()> {
    /// ChartGenerator::quality_histogram(&report, "quality_dist.svg")?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn quality_histogram(report: &EvalReport, output_path: impl AsRef<Path>) -> Result<()> {
        let output_path = output_path.as_ref();

        // Extract quality scores
        let scores: Vec<f64> = report
            .results
            .iter()
            .map(|r| r.quality_score.overall)
            .collect();

        if scores.is_empty() {
            anyhow::bail!("No scores to plot");
        }

        // Create bins: 0.0-0.1, 0.1-0.2, ..., 0.9-1.0
        let bins = 10;
        let mut histogram = vec![0u32; bins];

        for &score in &scores {
            let bin = ((score * bins as f64).floor() as usize).min(bins - 1);
            histogram[bin] += 1;
        }

        // Find max count for y-axis scaling
        let max_count = *histogram.iter().max().unwrap_or(&1);

        // Create SVG backend
        let root = SVGBackend::new(output_path, (800, 600)).into_drawing_area();
        root.fill(&WHITE)?;

        let mut chart = ChartBuilder::on(&root)
            .caption("Quality Score Distribution", ("sans-serif", 40))
            .margin(20)
            .x_label_area_size(50)
            .y_label_area_size(60)
            .build_cartesian_2d(0.0..1.0, 0u32..(max_count + max_count / 10))?;

        chart
            .configure_mesh()
            .x_desc("Quality Score")
            .y_desc("Count")
            .x_label_formatter(&|x| format!("{x:.1}"))
            .draw()?;

        // Draw histogram bars
        let bar_width = 1.0 / bins as f64;
        for (i, &count) in histogram.iter().enumerate() {
            let x_start = i as f64 * bar_width;
            let x_end = (i + 1) as f64 * bar_width;

            // Color based on quality range (high-contrast colors)
            let color = if x_start >= 0.95 {
                RGBColor(5, 150, 105) // Dark green - Excellent (#059669)
            } else if x_start >= 0.90 {
                RGBColor(37, 99, 235) // Dark blue - Good (#2563eb)
            } else if x_start >= 0.80 {
                RGBColor(217, 119, 6) // Dark orange - Fair (#d97706)
            } else {
                RGBColor(220, 38, 38) // Dark red - Poor (#dc2626)
            };

            chart.draw_series(std::iter::once(Rectangle::new(
                [(x_start, 0), (x_end, count)],
                color.filled(),
            )))?;
        }

        root.present().context("Failed to save quality histogram")?;

        Ok(())
    }

    /// Generate latency chart showing latency per scenario
    ///
    /// Creates a line chart showing how latency varies across different scenarios.
    /// Useful for identifying performance outliers.
    pub fn latency_chart(report: &EvalReport, output_path: impl AsRef<Path>) -> Result<()> {
        let output_path = output_path.as_ref();

        if report.results.is_empty() {
            anyhow::bail!("No results to plot");
        }

        let latencies: Vec<(usize, u64)> = report
            .results
            .iter()
            .enumerate()
            .map(|(i, r)| (i, r.latency_ms))
            .collect();

        let max_latency = latencies.iter().map(|(_, l)| l).max().unwrap_or(&1000);

        let root = SVGBackend::new(output_path, (1000, 600)).into_drawing_area();
        root.fill(&WHITE)?;

        let mut chart = ChartBuilder::on(&root)
            .caption("Latency by Scenario", ("sans-serif", 40))
            .margin(20)
            .x_label_area_size(50)
            .y_label_area_size(80)
            .build_cartesian_2d(
                0usize..report.results.len(),
                0u64..(*max_latency + *max_latency / 10),
            )?;

        chart
            .configure_mesh()
            .x_desc("Scenario Index")
            .y_desc("Latency (ms)")
            .draw()?;

        // Draw line
        chart
            .draw_series(plotters::series::LineSeries::new(latencies.clone(), &BLUE))?
            .label("Latency")
            .legend(|(x, y)| PathElement::new(vec![(x, y), (x + 20, y)], BLUE));

        // Draw points
        chart.draw_series(
            latencies
                .iter()
                .map(|(x, y)| Circle::new((*x, *y), 3, BLUE.filled())),
        )?;

        // Add average line
        let avg_latency = report.avg_latency_ms();
        chart
            .draw_series(plotters::series::LineSeries::new(
                vec![(0, avg_latency), (report.results.len(), avg_latency)],
                &RED.mix(0.5),
            ))?
            .label(format!("Average: {avg_latency}ms"))
            .legend(|(x, y)| PathElement::new(vec![(x, y), (x + 20, y)], RED.mix(0.5)));

        chart
            .configure_series_labels()
            .background_style(WHITE.mix(0.8))
            .border_style(BLACK)
            .draw()?;

        root.present().context("Failed to save latency chart")?;

        Ok(())
    }

    /// Generate pass/fail pie chart
    ///
    /// Simple pie chart showing the ratio of passed vs failed scenarios.
    pub fn pass_fail_chart(report: &EvalReport, output_path: impl AsRef<Path>) -> Result<()> {
        let output_path = output_path.as_ref();

        let passed = report.passed as f64;
        let failed = report.failed as f64;
        let total = passed + failed;

        if total == 0.0 {
            anyhow::bail!("No results to plot");
        }

        let root = SVGBackend::new(output_path, (600, 600)).into_drawing_area();
        root.fill(&WHITE)?;

        let mut chart = ChartBuilder::on(&root)
            .caption("Pass/Fail Distribution", ("sans-serif", 40))
            .margin(30)
            .build_cartesian_2d(-1.2..1.2, -1.2..1.2)?;

        // Draw pie slices
        let pass_angle = (passed / total) * 360.0;

        // Passed slice (dark green)
        if passed > 0.0 {
            let pass_points: Vec<(f64, f64)> = (0..=pass_angle as i32)
                .map(|deg| {
                    let rad = f64::from(deg).to_radians();
                    (rad.sin(), rad.cos())
                })
                .collect();

            chart.draw_series(std::iter::once(Polygon::new(
                std::iter::once((0.0, 0.0))
                    .chain(pass_points.into_iter())
                    .collect::<Vec<_>>(),
                RGBColor(5, 150, 105).filled(), // Dark green (#059669)
            )))?;
        }

        // Failed slice (dark red)
        if failed > 0.0 {
            let fail_points: Vec<(f64, f64)> = (pass_angle as i32..=360)
                .map(|deg| {
                    let rad = f64::from(deg).to_radians();
                    (rad.sin(), rad.cos())
                })
                .collect();

            chart.draw_series(std::iter::once(Polygon::new(
                std::iter::once((0.0, 0.0))
                    .chain(fail_points.into_iter())
                    .collect::<Vec<_>>(),
                RGBColor(220, 38, 38).filled(), // Dark red (#dc2626)
            )))?;
        }

        // Add labels
        root.draw_text(
            &format!(
                "Passed: {} ({:.1}%)",
                report.passed,
                (passed / total) * 100.0
            ),
            &TextStyle::from(("sans-serif", 20).into_font()).color(&BLACK),
            (20, 20),
        )?;
        root.draw_text(
            &format!(
                "Failed: {} ({:.1}%)",
                report.failed,
                (failed / total) * 100.0
            ),
            &TextStyle::from(("sans-serif", 20).into_font()).color(&BLACK),
            (20, 50),
        )?;

        root.present().context("Failed to save pass/fail chart")?;

        Ok(())
    }

    /// Generate all standard charts
    ///
    /// Convenience method that generates all available charts in one call.
    /// Creates files: {prefix}_quality.svg, {prefix}_latency.svg, {prefix}_`pass_fail.svg`
    pub fn generate_all(
        report: &EvalReport,
        output_dir: impl AsRef<Path>,
        prefix: &str,
    ) -> Result<()> {
        let output_dir = output_dir.as_ref();

        Self::quality_histogram(report, output_dir.join(format!("{prefix}_quality.svg")))?;
        Self::latency_chart(report, output_dir.join(format!("{prefix}_latency.svg")))?;
        Self::pass_fail_chart(report, output_dir.join(format!("{prefix}_pass_fail.svg")))?;

        Ok(())
    }

    /// Generate multi-dimensional quality breakdown chart
    ///
    /// Stacked bar chart showing average scores across all quality dimensions.
    pub fn quality_dimensions_chart(
        report: &EvalReport,
        output_path: impl AsRef<Path>,
    ) -> Result<()> {
        let output_path = output_path.as_ref();

        if report.results.is_empty() {
            anyhow::bail!("No results to plot");
        }

        // Calculate average for each dimension
        let mut accuracy_sum = 0.0;
        let mut relevance_sum = 0.0;
        let mut completeness_sum = 0.0;
        let mut safety_sum = 0.0;
        let mut coherence_sum = 0.0;
        let mut conciseness_sum = 0.0;

        for result in &report.results {
            accuracy_sum += result.quality_score.accuracy;
            relevance_sum += result.quality_score.relevance;
            completeness_sum += result.quality_score.completeness;
            safety_sum += result.quality_score.safety;
            coherence_sum += result.quality_score.coherence;
            conciseness_sum += result.quality_score.conciseness;
        }

        let count = report.results.len() as f64;
        let dimensions = [
            ("Accuracy", accuracy_sum / count),
            ("Relevance", relevance_sum / count),
            ("Completeness", completeness_sum / count),
            ("Safety", safety_sum / count),
            ("Coherence", coherence_sum / count),
            ("Conciseness", conciseness_sum / count),
        ];

        let root = SVGBackend::new(output_path, (800, 600)).into_drawing_area();
        root.fill(&WHITE)?;

        let mut chart = ChartBuilder::on(&root)
            .caption("Quality Dimensions Breakdown", ("sans-serif", 40))
            .margin(20)
            .x_label_area_size(100)
            .y_label_area_size(60)
            .build_cartesian_2d(0usize..dimensions.len(), 0.0..1.0)?;

        chart
            .configure_mesh()
            .y_desc("Average Score")
            .x_label_formatter(&|x| {
                dimensions
                    .get(*x)
                    .map(|(name, _)| (*name).to_string())
                    .unwrap_or_default()
            })
            .draw()?;

        // Draw bars
        chart.draw_series(dimensions.iter().enumerate().map(|(i, (_, score))| {
            let color = if *score >= 0.95 {
                RGBColor(34, 197, 94)
            } else if *score >= 0.90 {
                RGBColor(59, 130, 246)
            } else if *score >= 0.80 {
                RGBColor(245, 158, 11)
            } else {
                RGBColor(239, 68, 68)
            };

            Rectangle::new([(i, 0.0), (i + 1, *score)], color.filled())
        }))?;

        // Add threshold line at 0.90
        chart.draw_series(plotters::series::LineSeries::new(
            vec![(0, 0.90), (dimensions.len(), 0.90)],
            &RED.mix(0.5),
        ))?;

        root.present()
            .context("Failed to save quality dimensions chart")?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::eval_runner::{EvalMetadata, ScenarioResult, ValidationResult};
    use crate::quality_judge::QualityScore;
    use chrono::Utc;
    use tempfile::TempDir;

    fn create_mock_result(id: &str, quality: f64, latency_ms: u64) -> ScenarioResult {
        ScenarioResult {
            scenario_id: id.to_string(),
            passed: quality >= 0.90,
            output: "Test".to_string(),
            quality_score: QualityScore {
                accuracy: quality,
                relevance: quality,
                completeness: quality,
                safety: 1.0,
                coherence: quality,
                conciseness: quality,
                overall: quality,
                reasoning: "Test".to_string(),
                issues: vec![],
                suggestions: vec![],
            },
            latency_ms,
            validation: ValidationResult {
                passed: true,
                missing_contains: vec![],
                forbidden_found: vec![],
                failure_reason: None,
            },
            error: None,
            retry_attempts: 0,
            timestamp: Utc::now(),
            input: None,
            tokens_used: None,
            cost_usd: None,
        }
    }

    #[test]
    fn test_quality_histogram_generation() {
        let temp_dir = TempDir::new().unwrap();
        let output_path = temp_dir.path().join("quality.svg");

        let report = EvalReport {
            total: 5,
            passed: 4,
            failed: 1,
            results: vec![
                create_mock_result("s1", 0.97, 1000),
                create_mock_result("s2", 0.92, 1200),
                create_mock_result("s3", 0.88, 1100),
                create_mock_result("s4", 0.95, 900),
                create_mock_result("s5", 0.75, 1300),
            ],
            metadata: EvalMetadata {
                started_at: Utc::now(),
                completed_at: Utc::now(),
                duration_secs: 5.0,
                config: "{}".to_string(),
            },
        };

        ChartGenerator::quality_histogram(&report, &output_path).unwrap();
        assert!(output_path.exists());

        // Verify SVG file is not empty
        let content = std::fs::read_to_string(&output_path).unwrap();
        assert!(content.contains("<svg"));
        assert!(content.contains("Quality Score Distribution"));
    }

    #[test]
    fn test_latency_chart_generation() {
        let temp_dir = TempDir::new().unwrap();
        let output_path = temp_dir.path().join("latency.svg");

        let report = EvalReport {
            total: 3,
            passed: 3,
            failed: 0,
            results: vec![
                create_mock_result("s1", 0.95, 1000),
                create_mock_result("s2", 0.92, 2000),
                create_mock_result("s3", 0.94, 1500),
            ],
            metadata: EvalMetadata {
                started_at: Utc::now(),
                completed_at: Utc::now(),
                duration_secs: 4.5,
                config: "{}".to_string(),
            },
        };

        ChartGenerator::latency_chart(&report, &output_path).unwrap();
        assert!(output_path.exists());

        let content = std::fs::read_to_string(&output_path).unwrap();
        assert!(content.contains("<svg"));
        assert!(content.contains("Latency"));
    }

    #[test]
    fn test_pass_fail_chart_generation() {
        let temp_dir = TempDir::new().unwrap();
        let output_path = temp_dir.path().join("pass_fail.svg");

        let report = EvalReport {
            total: 4,
            passed: 3,
            failed: 1,
            results: vec![
                create_mock_result("s1", 0.95, 1000),
                create_mock_result("s2", 0.92, 1000),
                create_mock_result("s3", 0.94, 1000),
                create_mock_result("s4", 0.85, 1000),
            ],
            metadata: EvalMetadata {
                started_at: Utc::now(),
                completed_at: Utc::now(),
                duration_secs: 4.0,
                config: "{}".to_string(),
            },
        };

        ChartGenerator::pass_fail_chart(&report, &output_path).unwrap();
        assert!(output_path.exists());

        let content = std::fs::read_to_string(&output_path).unwrap();
        assert!(content.contains("Pass/Fail"));
        assert!(content.contains("Passed"));
        assert!(content.contains("Failed"));
    }

    #[test]
    fn test_generate_all_charts() {
        let temp_dir = TempDir::new().unwrap();

        let report = EvalReport {
            total: 2,
            passed: 2,
            failed: 0,
            results: vec![
                create_mock_result("s1", 0.95, 1000),
                create_mock_result("s2", 0.92, 1200),
            ],
            metadata: EvalMetadata {
                started_at: Utc::now(),
                completed_at: Utc::now(),
                duration_secs: 2.2,
                config: "{}".to_string(),
            },
        };

        ChartGenerator::generate_all(&report, temp_dir.path(), "eval").unwrap();

        assert!(temp_dir.path().join("eval_quality.svg").exists());
        assert!(temp_dir.path().join("eval_latency.svg").exists());
        assert!(temp_dir.path().join("eval_pass_fail.svg").exists());
    }

    #[test]
    fn test_quality_dimensions_chart() {
        let temp_dir = TempDir::new().unwrap();
        let output_path = temp_dir.path().join("dimensions.svg");

        let report = EvalReport {
            total: 2,
            passed: 2,
            failed: 0,
            results: vec![
                create_mock_result("s1", 0.95, 1000),
                create_mock_result("s2", 0.90, 1000),
            ],
            metadata: EvalMetadata {
                started_at: Utc::now(),
                completed_at: Utc::now(),
                duration_secs: 2.0,
                config: "{}".to_string(),
            },
        };

        ChartGenerator::quality_dimensions_chart(&report, &output_path).unwrap();
        assert!(output_path.exists());

        let content = std::fs::read_to_string(&output_path).unwrap();
        assert!(content.contains("Quality Dimensions"));
    }
}
