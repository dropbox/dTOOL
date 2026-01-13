// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

//! Result reporting for A/B tests

use crate::optimize::ab_testing::analysis::{ConfidenceInterval, TTestResult};
use std::fs::File;
use std::io::Write;

/// Report for a single variant's results
#[derive(Debug, Clone)]
pub struct VariantReport {
    /// Variant name
    pub name: String,
    /// Sample size
    pub sample_size: usize,
    /// Mean metric value
    pub mean: f64,
    /// Standard deviation
    pub std_dev: f64,
    /// Confidence interval for mean
    pub confidence_interval: ConfidenceInterval,
}

impl VariantReport {
    /// Create a new variant report
    pub fn new(
        name: String,
        sample_size: usize,
        mean: f64,
        std_dev: f64,
        confidence_interval: ConfidenceInterval,
    ) -> Self {
        Self {
            name,
            sample_size,
            mean,
            std_dev,
            confidence_interval,
        }
    }

    /// Format as markdown
    pub fn to_markdown(&self) -> String {
        format!(
            "**{}** (n={})\n  Mean: {:.3} (CI: [{:.3}, {:.3}])\n  Std Dev: {:.3}",
            self.name,
            self.sample_size,
            self.mean,
            self.confidence_interval.lower,
            self.confidence_interval.upper,
            self.std_dev
        )
    }
}

/// Complete results report for an A/B test
#[derive(Debug, Clone)]
pub struct ResultsReport {
    /// Test name
    pub test_name: String,
    /// Reports for each variant
    pub variants: Vec<VariantReport>,
    /// T-test results (if applicable)
    pub t_test: Option<TTestResult>,
    /// Winner name (if significant)
    pub winner: Option<String>,
    /// Recommendation text
    pub recommendation: String,
}

impl ResultsReport {
    /// Create a new results report
    pub fn new(test_name: String) -> Self {
        Self {
            test_name,
            variants: Vec::new(),
            t_test: None,
            winner: None,
            recommendation: String::new(),
        }
    }

    /// Add a variant report
    pub fn add_variant(&mut self, report: VariantReport) {
        self.variants.push(report);
    }

    /// Set t-test results
    pub fn set_t_test(&mut self, t_test: TTestResult) {
        self.t_test = Some(t_test);
    }

    /// Set winner and recommendation
    pub fn set_winner(&mut self, winner: String, recommendation: String) {
        self.winner = Some(winner);
        self.recommendation = recommendation;
    }

    /// Generate summary text
    pub fn summary(&self) -> String {
        let mut output = String::new();

        output.push_str(&format!("# A/B Test Results: {}\n\n", self.test_name));

        output.push_str("## Variant Performance\n\n");
        for variant in &self.variants {
            output.push_str(&variant.to_markdown());
            output.push('\n');
        }

        if let Some(t_test) = &self.t_test {
            output.push_str(&format!(
                "\n## Statistical Analysis\n\n\
                 Mean Difference: {:.3}\n\
                 T-statistic: {:.3}\n\
                 P-value: {:.4}\n\
                 Significant: {}\n\n",
                t_test.mean_difference,
                t_test.t_statistic,
                t_test.p_value,
                if t_test.is_significant { "YES" } else { "NO" }
            ));
        }

        if let Some(winner) = &self.winner {
            output.push_str(&format!("## Winner: {}\n\n", winner));
        }

        if !self.recommendation.is_empty() {
            output.push_str(&format!("## Recommendation\n\n{}\n", self.recommendation));
        }

        output
    }

    /// Save report as markdown file
    pub fn save_markdown(&self, path: &str) -> crate::optimize::ab_testing::Result<()> {
        let mut file = File::create(path)?;
        file.write_all(self.summary().as_bytes())?;
        Ok(())
    }

    /// Save report as HTML file
    pub fn save_html(&self, path: &str) -> crate::optimize::ab_testing::Result<()> {
        let html = format!(
            r#"<!DOCTYPE html>
<html>
<head>
    <meta charset="UTF-8">
    <title>{}</title>
    <style>
        body {{
            font-family: Arial, sans-serif;
            max-width: 800px;
            margin: 40px auto;
            padding: 20px;
            line-height: 1.6;
        }}
        h1 {{ color: #333; }}
        h2 {{ color: #666; margin-top: 30px; }}
        .variant {{
            background: #f5f5f5;
            padding: 15px;
            margin: 10px 0;
            border-radius: 5px;
        }}
        .winner {{
            background: #e8f5e9;
            padding: 15px;
            border-left: 4px solid #4caf50;
            margin: 20px 0;
        }}
        .stats {{
            background: #f5f5f5;
            padding: 15px;
            font-family: monospace;
        }}
        .footer {{
            margin-top: 40px;
            padding-top: 20px;
            border-top: 1px solid #ccc;
            text-align: center;
            color: #666;
            font-size: 0.9em;
        }}
    </style>
</head>
<body>
"#,
            self.test_name
        );

        let mut body = html;
        body.push_str(&format!("<h1>A/B Test Results: {}</h1>\n", self.test_name));

        body.push_str("<h2>Variant Performance</h2>\n");
        for variant in &self.variants {
            body.push_str("<div class='variant'>\n");
            body.push_str(&format!(
                "<strong>{}</strong> (n={})<br>\n",
                variant.name, variant.sample_size
            ));
            body.push_str(&format!("Mean: {:.3}<br>\n", variant.mean));
            body.push_str(&format!(
                "95% CI: [{:.3}, {:.3}]<br>\n",
                variant.confidence_interval.lower, variant.confidence_interval.upper
            ));
            body.push_str(&format!("Std Dev: {:.3}\n", variant.std_dev));
            body.push_str("</div>\n");
        }

        if let Some(t_test) = &self.t_test {
            body.push_str("<h2>Statistical Analysis</h2>\n");
            body.push_str("<div class='stats'>\n");
            body.push_str(&format!(
                "Mean Difference: {:.3}<br>\n",
                t_test.mean_difference
            ));
            body.push_str(&format!("T-statistic: {:.3}<br>\n", t_test.t_statistic));
            body.push_str(&format!("P-value: {:.4}<br>\n", t_test.p_value));
            body.push_str(&format!(
                "Significant: {}\n",
                if t_test.is_significant { "YES" } else { "NO" }
            ));
            body.push_str("</div>\n");
        }

        if let Some(winner) = &self.winner {
            body.push_str("<div class='winner'>\n");
            body.push_str(&format!("<h2>Winner: {}</h2>\n", winner));
            if !self.recommendation.is_empty() {
                body.push_str(&format!("<p>{}</p>\n", self.recommendation));
            }
            body.push_str("</div>\n");
        }

        body.push_str("<div class='footer'>\n");
        body.push_str("&copy; 2025 Dropbox (created by Andrew Yates <ayates@dropbox.com>)\n");
        body.push_str("</div>\n");

        body.push_str("</body>\n</html>");

        let mut file = File::create(path)?;
        file.write_all(body.as_bytes())?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_variant_report_markdown() {
        let ci = ConfidenceInterval::new(0.7, 0.9, 0.95);
        let report = VariantReport::new("control".to_string(), 100, 0.8, 0.1, ci);

        let md = report.to_markdown();
        assert!(md.contains("control"));
        assert!(md.contains("n=100"));
        assert!(md.contains("0.800"));
    }

    #[test]
    fn test_results_report_summary() {
        let mut report = ResultsReport::new("Test 1".to_string());

        let ci1 = ConfidenceInterval::new(0.7, 0.9, 0.95);
        let v1 = VariantReport::new("control".to_string(), 100, 0.8, 0.1, ci1);

        let ci2 = ConfidenceInterval::new(0.8, 1.0, 0.95);
        let v2 = VariantReport::new("treatment".to_string(), 100, 0.9, 0.1, ci2);

        report.add_variant(v1);
        report.add_variant(v2);

        let summary = report.summary();
        assert!(summary.contains("Test 1"));
        assert!(summary.contains("control"));
        assert!(summary.contains("treatment"));
    }

    #[test]
    fn test_results_report_with_winner() {
        let mut report = ResultsReport::new("Test 1".to_string());

        report.set_winner(
            "treatment".to_string(),
            "Deploy treatment variant".to_string(),
        );

        let summary = report.summary();
        assert!(summary.contains("Winner: treatment"));
        assert!(summary.contains("Deploy treatment variant"));
    }

    // ============================================================================
    // Additional comprehensive tests for report.rs
    // ============================================================================

    // ------------------------------------------------------------------------
    // VariantReport Tests
    // ------------------------------------------------------------------------

    #[test]
    fn test_variant_report_new() {
        let ci = ConfidenceInterval::new(0.5, 0.9, 0.95);
        let report = VariantReport::new("test_variant".to_string(), 50, 0.7, 0.15, ci.clone());

        assert_eq!(report.name, "test_variant");
        assert_eq!(report.sample_size, 50);
        assert!((report.mean - 0.7).abs() < 1e-6);
        assert!((report.std_dev - 0.15).abs() < 1e-6);
        assert!((report.confidence_interval.lower - 0.5).abs() < 1e-6);
        assert!((report.confidence_interval.upper - 0.9).abs() < 1e-6);
    }

    #[test]
    fn test_variant_report_markdown_format() {
        let ci = ConfidenceInterval::new(0.75, 0.85, 0.95);
        let report = VariantReport::new("variant_a".to_string(), 200, 0.8, 0.05, ci);

        let md = report.to_markdown();

        // Check all expected components
        assert!(md.contains("**variant_a**"));
        assert!(md.contains("(n=200)"));
        assert!(md.contains("Mean: 0.800"));
        assert!(md.contains("CI: [0.750, 0.850]"));
        assert!(md.contains("Std Dev: 0.050"));
    }

    #[test]
    fn test_variant_report_zero_values() {
        let ci = ConfidenceInterval::new(0.0, 0.0, 0.95);
        let report = VariantReport::new("zero_variant".to_string(), 0, 0.0, 0.0, ci);

        let md = report.to_markdown();
        assert!(md.contains("(n=0)"));
        assert!(md.contains("Mean: 0.000"));
        assert!(md.contains("Std Dev: 0.000"));
    }

    #[test]
    fn test_variant_report_large_values() {
        let ci = ConfidenceInterval::new(999.0, 1001.0, 0.95);
        let report = VariantReport::new("large_variant".to_string(), 100000, 1000.0, 50.0, ci);

        let md = report.to_markdown();
        assert!(md.contains("(n=100000)"));
        assert!(md.contains("Mean: 1000.000"));
        assert!(md.contains("Std Dev: 50.000"));
    }

    #[test]
    fn test_variant_report_negative_values() {
        let ci = ConfidenceInterval::new(-1.5, -0.5, 0.95);
        let report = VariantReport::new("negative_variant".to_string(), 100, -1.0, 0.25, ci);

        let md = report.to_markdown();
        assert!(md.contains("Mean: -1.000"));
        assert!(md.contains("CI: [-1.500, -0.500]"));
    }

    #[test]
    fn test_variant_report_special_name() {
        let ci = ConfidenceInterval::new(0.0, 1.0, 0.95);
        let report = VariantReport::new(
            "variant with spaces & symbols!".to_string(),
            100,
            0.5,
            0.1,
            ci,
        );

        let md = report.to_markdown();
        assert!(md.contains("**variant with spaces & symbols!**"));
    }

    #[test]
    fn test_variant_report_clone() {
        let ci = ConfidenceInterval::new(0.7, 0.9, 0.95);
        let report = VariantReport::new("original".to_string(), 100, 0.8, 0.1, ci);
        let cloned = report.clone();

        assert_eq!(report.name, cloned.name);
        assert_eq!(report.sample_size, cloned.sample_size);
        assert!((report.mean - cloned.mean).abs() < 1e-6);
        assert!((report.std_dev - cloned.std_dev).abs() < 1e-6);
    }

    #[test]
    fn test_variant_report_debug() {
        let ci = ConfidenceInterval::new(0.7, 0.9, 0.95);
        let report = VariantReport::new("test".to_string(), 100, 0.8, 0.1, ci);
        let debug_str = format!("{:?}", report);

        assert!(debug_str.contains("VariantReport"));
        assert!(debug_str.contains("test"));
        assert!(debug_str.contains("100"));
    }

    // ------------------------------------------------------------------------
    // ResultsReport Tests
    // ------------------------------------------------------------------------

    #[test]
    fn test_results_report_new() {
        let report = ResultsReport::new("New Test".to_string());

        assert_eq!(report.test_name, "New Test");
        assert!(report.variants.is_empty());
        assert!(report.t_test.is_none());
        assert!(report.winner.is_none());
        assert!(report.recommendation.is_empty());
    }

    #[test]
    fn test_results_report_add_variant() {
        let mut report = ResultsReport::new("Test".to_string());

        let ci1 = ConfidenceInterval::new(0.7, 0.9, 0.95);
        let v1 = VariantReport::new("variant1".to_string(), 100, 0.8, 0.1, ci1);

        let ci2 = ConfidenceInterval::new(0.8, 1.0, 0.95);
        let v2 = VariantReport::new("variant2".to_string(), 100, 0.9, 0.1, ci2);

        report.add_variant(v1);
        assert_eq!(report.variants.len(), 1);

        report.add_variant(v2);
        assert_eq!(report.variants.len(), 2);

        assert_eq!(report.variants[0].name, "variant1");
        assert_eq!(report.variants[1].name, "variant2");
    }

    #[test]
    fn test_results_report_set_t_test() {
        let mut report = ResultsReport::new("Test".to_string());

        assert!(report.t_test.is_none());

        let t_test = TTestResult {
            t_statistic: 2.5,
            degrees_of_freedom: 100.0,
            p_value: 0.01,
            mean_difference: 0.1,
            is_significant: true,
            significance_level: 0.05,
        };

        report.set_t_test(t_test);
        assert!(report.t_test.is_some());

        let t = report.t_test.as_ref().unwrap();
        assert!((t.t_statistic - 2.5).abs() < 1e-6);
        assert!((t.p_value - 0.01).abs() < 1e-6);
        assert!(t.is_significant);
    }

    #[test]
    fn test_results_report_set_winner() {
        let mut report = ResultsReport::new("Test".to_string());

        assert!(report.winner.is_none());
        assert!(report.recommendation.is_empty());

        report.set_winner(
            "control".to_string(),
            "Keep current implementation".to_string(),
        );

        assert_eq!(report.winner, Some("control".to_string()));
        assert_eq!(report.recommendation, "Keep current implementation");
    }

    #[test]
    fn test_results_report_summary_empty() {
        let report = ResultsReport::new("Empty Test".to_string());
        let summary = report.summary();

        assert!(summary.contains("# A/B Test Results: Empty Test"));
        assert!(summary.contains("## Variant Performance"));
    }

    #[test]
    fn test_results_report_summary_with_t_test() {
        let mut report = ResultsReport::new("Statistical Test".to_string());

        let t_test = TTestResult {
            t_statistic: 3.0,
            degrees_of_freedom: 198.0,
            p_value: 0.003,
            mean_difference: 0.15,
            is_significant: true,
            significance_level: 0.05,
        };

        report.set_t_test(t_test);

        let summary = report.summary();

        assert!(summary.contains("## Statistical Analysis"));
        assert!(summary.contains("Mean Difference: 0.150"));
        assert!(summary.contains("T-statistic: 3.000"));
        assert!(summary.contains("P-value: 0.0030"));
        assert!(summary.contains("Significant: YES"));
    }

    #[test]
    fn test_results_report_summary_not_significant() {
        let mut report = ResultsReport::new("Not Significant Test".to_string());

        let t_test = TTestResult {
            t_statistic: 0.5,
            degrees_of_freedom: 50.0,
            p_value: 0.6,
            mean_difference: 0.02,
            is_significant: false,
            significance_level: 0.05,
        };

        report.set_t_test(t_test);

        let summary = report.summary();
        assert!(summary.contains("Significant: NO"));
    }

    #[test]
    fn test_results_report_summary_with_recommendation() {
        let mut report = ResultsReport::new("Recommendation Test".to_string());

        report.recommendation = "This is a detailed recommendation for the test.".to_string();

        let summary = report.summary();
        assert!(summary.contains("## Recommendation"));
        assert!(summary.contains("This is a detailed recommendation for the test."));
    }

    #[test]
    fn test_results_report_summary_no_recommendation() {
        let report = ResultsReport::new("No Recommendation".to_string());
        let summary = report.summary();

        // Should not contain recommendation section if recommendation is empty
        assert!(!summary.contains("## Recommendation\n\n\n"));
    }

    #[test]
    fn test_results_report_full_summary() {
        let mut report = ResultsReport::new("Full Test".to_string());

        let ci1 = ConfidenceInterval::new(0.70, 0.80, 0.95);
        let v1 = VariantReport::new("control".to_string(), 500, 0.75, 0.12, ci1);

        let ci2 = ConfidenceInterval::new(0.82, 0.92, 0.95);
        let v2 = VariantReport::new("treatment".to_string(), 500, 0.87, 0.10, ci2);

        report.add_variant(v1);
        report.add_variant(v2);

        let t_test = TTestResult {
            t_statistic: 5.5,
            degrees_of_freedom: 998.0,
            p_value: 0.0001,
            mean_difference: 0.12,
            is_significant: true,
            significance_level: 0.05,
        };
        report.set_t_test(t_test);

        report.set_winner(
            "treatment".to_string(),
            "Deploy the treatment variant for improved performance.".to_string(),
        );

        let summary = report.summary();

        // Check all sections
        assert!(summary.contains("# A/B Test Results: Full Test"));
        assert!(summary.contains("## Variant Performance"));
        assert!(summary.contains("control"));
        assert!(summary.contains("treatment"));
        assert!(summary.contains("n=500"));
        assert!(summary.contains("## Statistical Analysis"));
        assert!(summary.contains("## Winner: treatment"));
        assert!(summary.contains("## Recommendation"));
        assert!(summary.contains("Deploy the treatment variant"));
    }

    #[test]
    fn test_results_report_multiple_variants() {
        let mut report = ResultsReport::new("Multi-Variant Test".to_string());

        for i in 0..5 {
            let ci = ConfidenceInterval::new(0.6 + i as f64 * 0.05, 0.8 + i as f64 * 0.05, 0.95);
            let variant = VariantReport::new(
                format!("variant_{}", i),
                100 + i * 10,
                0.7 + i as f64 * 0.05,
                0.1,
                ci,
            );
            report.add_variant(variant);
        }

        assert_eq!(report.variants.len(), 5);

        let summary = report.summary();
        for i in 0..5 {
            assert!(summary.contains(&format!("variant_{}", i)));
        }
    }

    #[test]
    fn test_results_report_clone() {
        let mut report = ResultsReport::new("Clone Test".to_string());

        let ci = ConfidenceInterval::new(0.7, 0.9, 0.95);
        let variant = VariantReport::new("v1".to_string(), 100, 0.8, 0.1, ci);
        report.add_variant(variant);

        report.set_winner("v1".to_string(), "Winner recommendation".to_string());

        let cloned = report.clone();

        assert_eq!(report.test_name, cloned.test_name);
        assert_eq!(report.variants.len(), cloned.variants.len());
        assert_eq!(report.winner, cloned.winner);
        assert_eq!(report.recommendation, cloned.recommendation);
    }

    #[test]
    fn test_results_report_debug() {
        let report = ResultsReport::new("Debug Test".to_string());
        let debug_str = format!("{:?}", report);

        assert!(debug_str.contains("ResultsReport"));
        assert!(debug_str.contains("Debug Test"));
    }

    #[test]
    fn test_results_report_empty_test_name() {
        let report = ResultsReport::new(String::new());

        assert!(report.test_name.is_empty());
        let summary = report.summary();
        assert!(summary.contains("# A/B Test Results:"));
    }

    #[test]
    fn test_results_report_special_characters_in_name() {
        let report = ResultsReport::new("Test <with> \"special\" & characters".to_string());
        let summary = report.summary();

        assert!(summary.contains("Test <with> \"special\" & characters"));
    }

    #[test]
    fn test_variant_report_precision() {
        // Test that formatting preserves 3 decimal places
        let ci = ConfidenceInterval::new(0.123456789, 0.987654321, 0.95);
        let report = VariantReport::new("precision".to_string(), 100, 0.555555, 0.111111, ci);

        let md = report.to_markdown();
        assert!(md.contains("0.556")); // Mean rounded
        assert!(md.contains("0.111")); // Std dev rounded
        assert!(md.contains("0.123")); // CI lower rounded
        assert!(md.contains("0.988")); // CI upper rounded
    }

    #[test]
    fn test_results_report_t_test_precision() {
        let mut report = ResultsReport::new("Precision Test".to_string());

        let t_test = TTestResult {
            t_statistic: 1.234567,
            degrees_of_freedom: 100.0,
            p_value: 0.0567891,
            mean_difference: 0.123456,
            is_significant: false,
            significance_level: 0.05,
        };

        report.set_t_test(t_test);

        let summary = report.summary();
        assert!(summary.contains("T-statistic: 1.235")); // 3 decimal places
        assert!(summary.contains("P-value: 0.0568")); // 4 decimal places
        assert!(summary.contains("Mean Difference: 0.123")); // 3 decimal places
    }

    #[test]
    fn test_results_report_overwrite_winner() {
        let mut report = ResultsReport::new("Overwrite Test".to_string());

        report.set_winner(
            "first_winner".to_string(),
            "First recommendation".to_string(),
        );
        assert_eq!(report.winner, Some("first_winner".to_string()));

        report.set_winner(
            "second_winner".to_string(),
            "Second recommendation".to_string(),
        );
        assert_eq!(report.winner, Some("second_winner".to_string()));
        assert_eq!(report.recommendation, "Second recommendation");
    }

    #[test]
    fn test_results_report_overwrite_t_test() {
        let mut report = ResultsReport::new("Overwrite T-Test".to_string());

        let t_test1 = TTestResult {
            t_statistic: 1.0,
            degrees_of_freedom: 50.0,
            p_value: 0.5,
            mean_difference: 0.01,
            is_significant: false,
            significance_level: 0.05,
        };

        let t_test2 = TTestResult {
            t_statistic: 5.0,
            degrees_of_freedom: 100.0,
            p_value: 0.001,
            mean_difference: 0.2,
            is_significant: true,
            significance_level: 0.05,
        };

        report.set_t_test(t_test1);
        assert!((report.t_test.as_ref().unwrap().t_statistic - 1.0).abs() < 1e-6);

        report.set_t_test(t_test2);
        assert!((report.t_test.as_ref().unwrap().t_statistic - 5.0).abs() < 1e-6);
        assert!(report.t_test.as_ref().unwrap().is_significant);
    }

    #[test]
    fn test_results_report_summary_ordering() {
        let mut report = ResultsReport::new("Order Test".to_string());

        let ci = ConfidenceInterval::new(0.7, 0.9, 0.95);
        let variant = VariantReport::new("variant".to_string(), 100, 0.8, 0.1, ci);
        report.add_variant(variant);

        let t_test = TTestResult {
            t_statistic: 2.0,
            degrees_of_freedom: 198.0,
            p_value: 0.05,
            mean_difference: 0.1,
            is_significant: true,
            significance_level: 0.05,
        };
        report.set_t_test(t_test);

        report.set_winner("variant".to_string(), "Recommendation text".to_string());

        let summary = report.summary();

        // Verify sections appear in correct order
        let variant_pos = summary.find("## Variant Performance").unwrap();
        let stats_pos = summary.find("## Statistical Analysis").unwrap();
        let winner_pos = summary.find("## Winner:").unwrap();
        let rec_pos = summary.find("## Recommendation").unwrap();

        assert!(variant_pos < stats_pos);
        assert!(stats_pos < winner_pos);
        assert!(winner_pos < rec_pos);
    }
}
