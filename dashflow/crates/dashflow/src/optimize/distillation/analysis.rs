// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

//! Analysis and reporting for distillation results

use serde::{Deserialize, Serialize};

/// Report for distillation process
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DistillationReport {
    /// Quality analysis
    pub quality: QualityGap,

    /// Cost analysis
    pub cost: CostAnalysis,

    /// ROI metrics
    pub roi: Option<ROIMetrics>,
}

/// Quality gap analysis between teacher and student
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QualityGap {
    /// Teacher model quality (accuracy/F1)
    pub teacher_quality: f64,

    /// Student baseline quality (before distillation)
    pub student_baseline_quality: f64,

    /// Distilled student quality (after training on teacher data)
    pub distilled_quality: f64,

    /// Absolute gap between teacher and distilled student
    pub absolute_gap: f64,

    /// Relative gap as percentage
    pub relative_gap_percent: f64,

    /// Whether gap is within acceptable threshold
    pub acceptable: bool,

    /// Quality improvement from baseline
    pub improvement: f64,
}

impl QualityGap {
    /// Calculate quality gap metrics between teacher and distilled student.
    pub fn new(
        teacher_quality: f64,
        student_baseline_quality: f64,
        distilled_quality: f64,
        max_gap_threshold: f64,
    ) -> Self {
        let absolute_gap = teacher_quality - distilled_quality;
        let relative_gap_percent = (absolute_gap / teacher_quality) * 100.0;
        let acceptable = absolute_gap <= max_gap_threshold;
        let improvement = distilled_quality - student_baseline_quality;

        Self {
            teacher_quality,
            student_baseline_quality,
            distilled_quality,
            absolute_gap,
            relative_gap_percent,
            acceptable,
            improvement,
        }
    }
}

/// Cost analysis for distillation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CostAnalysis {
    /// Cost per request for teacher model
    pub teacher_cost_per_request: f64,

    /// Cost per request for student model
    pub student_cost_per_request: f64,

    /// Cost reduction factor (teacher / student)
    pub cost_reduction_factor: f64,

    /// One-time cost to generate synthetic data
    pub synthetic_data_generation_cost: f64,

    /// Number of synthetic examples generated
    pub num_synthetic_examples: usize,
}

impl CostAnalysis {
    /// Create a new cost analysis with the given parameters.
    pub fn new(
        teacher_cost_per_request: f64,
        student_cost_per_request: f64,
        synthetic_data_generation_cost: f64,
        num_synthetic_examples: usize,
    ) -> Self {
        let cost_reduction_factor = if student_cost_per_request > 0.0 {
            teacher_cost_per_request / student_cost_per_request
        } else {
            0.0
        };

        Self {
            teacher_cost_per_request,
            student_cost_per_request,
            cost_reduction_factor,
            synthetic_data_generation_cost,
            num_synthetic_examples,
        }
    }
}

/// ROI metrics for distillation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ROIMetrics {
    /// Expected daily request volume
    pub requests_per_day: usize,

    /// Daily cost with teacher model
    pub daily_cost_teacher: f64,

    /// Daily cost with distilled student model
    pub daily_cost_student: f64,

    /// Daily savings
    pub daily_savings: f64,

    /// Monthly savings (30 days)
    pub monthly_savings: f64,

    /// Annual savings (365 days)
    pub annual_savings: f64,

    /// Payback period in hours (time to recoup synthetic data cost)
    pub payback_hours: f64,

    /// Payback period in days
    pub payback_days: f64,
}

impl ROIMetrics {
    /// Calculate ROI metrics for distillation.
    pub fn calculate(
        requests_per_day: usize,
        teacher_cost_per_request: f64,
        student_cost_per_request: f64,
        synthetic_data_cost: f64,
    ) -> Self {
        let daily_cost_teacher = teacher_cost_per_request * requests_per_day as f64;
        let daily_cost_student = student_cost_per_request * requests_per_day as f64;
        let daily_savings = daily_cost_teacher - daily_cost_student;

        let monthly_savings = daily_savings * 30.0;
        let annual_savings = daily_savings * 365.0;

        let payback_days = if daily_savings > 0.0 {
            synthetic_data_cost / daily_savings
        } else {
            f64::INFINITY
        };
        let payback_hours = payback_days * 24.0;

        Self {
            requests_per_day,
            daily_cost_teacher,
            daily_cost_student,
            daily_savings,
            monthly_savings,
            annual_savings,
            payback_hours,
            payback_days,
        }
    }

    /// Format payback period as human-readable string
    pub fn payback_string(&self) -> String {
        if self.payback_hours < 24.0 {
            format!("{:.1} hours", self.payback_hours)
        } else if self.payback_days < 7.0 {
            format!("{:.1} days", self.payback_days)
        } else if self.payback_days < 30.0 {
            format!("{:.1} weeks", self.payback_days / 7.0)
        } else {
            format!("{:.1} months", self.payback_days / 30.0)
        }
    }
}

impl DistillationReport {
    /// Create a new distillation report from quality, cost, and ROI data.
    pub fn new(quality: QualityGap, cost: CostAnalysis, roi: Option<ROIMetrics>) -> Self {
        Self { quality, cost, roi }
    }

    /// Generate a formatted text report
    pub fn format_report(&self) -> String {
        let mut report = String::new();

        report.push_str("═══════════════════════════════════════════════════\n");
        report.push_str("           MODEL DISTILLATION REPORT\n");
        report.push_str("═══════════════════════════════════════════════════\n\n");

        // Quality Metrics
        report.push_str("QUALITY METRICS\n");
        report.push_str("─────────────────────────────────────────────────\n");
        report.push_str(&format!(
            "Teacher Model:           {:.1}%\n",
            self.quality.teacher_quality * 100.0
        ));
        report.push_str(&format!(
            "Student Baseline:        {:.1}%\n",
            self.quality.student_baseline_quality * 100.0
        ));
        report.push_str(&format!(
            "Distilled Student:       {:.1}%\n",
            self.quality.distilled_quality * 100.0
        ));
        report.push_str(&format!(
            "\nQuality Gap:             {:.1}% ({:.1}% relative)\n",
            self.quality.absolute_gap * 100.0,
            self.quality.relative_gap_percent
        ));
        report.push_str(&format!(
            "Improvement over Baseline: {:.1}%\n",
            self.quality.improvement * 100.0
        ));
        report.push_str(&format!(
            "Status:                  {}\n\n",
            if self.quality.acceptable {
                "ACCEPTABLE"
            } else {
                "EXCEEDS THRESHOLD"
            }
        ));

        // Cost Metrics
        report.push_str("COST METRICS\n");
        report.push_str("─────────────────────────────────────────────────\n");
        report.push_str(&format!(
            "Teacher Cost:            ${:.6}/request\n",
            self.cost.teacher_cost_per_request
        ));
        report.push_str(&format!(
            "Student Cost:            ${:.6}/request\n",
            self.cost.student_cost_per_request
        ));
        report.push_str(&format!(
            "Cost Reduction:          {:.1}x\n\n",
            self.cost.cost_reduction_factor
        ));

        report.push_str("Synthetic Data Generation:\n");
        report.push_str(&format!(
            "  Examples:              {}\n",
            self.cost.num_synthetic_examples
        ));
        report.push_str(&format!(
            "  Cost:                  ${:.2}\n\n",
            self.cost.synthetic_data_generation_cost
        ));

        // ROI Metrics
        if let Some(roi) = &self.roi {
            report.push_str("ROI ANALYSIS\n");
            report.push_str("─────────────────────────────────────────────────\n");
            report.push_str(&format!(
                "Request Volume:          {}/day\n",
                roi.requests_per_day
            ));
            report.push_str(&format!(
                "\nDaily Savings:           ${:.2}\n",
                roi.daily_savings
            ));
            report.push_str(&format!(
                "Monthly Savings:         ${:.2}\n",
                roi.monthly_savings
            ));
            report.push_str(&format!(
                "Annual Savings:          ${:.2}\n\n",
                roi.annual_savings
            ));
            report.push_str(&format!(
                "Payback Period:          {}\n",
                roi.payback_string()
            ));
            report.push_str(&format!(
                "                         ({:.2} days)\n\n",
                roi.payback_days
            ));
        }

        report.push_str("═══════════════════════════════════════════════════\n");

        // Recommendation
        report.push_str("\nRECOMMENDATION\n");
        report.push_str("─────────────────────────────────────────────────\n");

        if self.quality.acceptable && self.cost.cost_reduction_factor > 2.0 {
            if let Some(roi) = &self.roi {
                if roi.payback_hours < 24.0 {
                    report.push_str("DEPLOY IMMEDIATELY\n\n");
                    report.push_str(&format!(
                        "The distilled model achieves {:.1}% of teacher quality\n",
                        (self.quality.distilled_quality / self.quality.teacher_quality) * 100.0
                    ));
                    report.push_str(&format!(
                        "at {:.1}x lower cost. Payback in just {}.\n",
                        self.cost.cost_reduction_factor,
                        roi.payback_string()
                    ));
                    report.push_str(&format!("Annual savings: ${:.2}\n", roi.annual_savings));
                } else {
                    report.push_str("DEPLOY FOR HIGH-VOLUME USE CASES\n\n");
                    report.push_str(&format!(
                        "Excellent quality ({:.1}%) with {:.1}x cost reduction.\n",
                        self.quality.distilled_quality * 100.0,
                        self.cost.cost_reduction_factor
                    ));
                    report.push_str(&format!(
                        "Best for applications with >{}K requests/day.\n",
                        roi.requests_per_day / 1000
                    ));
                }
            }
        } else if !self.quality.acceptable {
            report.push_str("QUALITY GAP TOO LARGE\n\n");
            report.push_str(&format!(
                "Quality gap of {:.1}% may be too large for production.\n",
                self.quality.absolute_gap * 100.0
            ));
            report.push_str("Consider:\n");
            report.push_str("  - Generating more synthetic examples\n");
            report.push_str("  - Using a stronger teacher model\n");
            report.push_str("  - Adding domain-specific training data\n");
        } else {
            report.push_str("LIMITED COST SAVINGS\n\n");
            report.push_str(&format!(
                "Cost reduction of {:.1}x may not justify distillation overhead.\n",
                self.cost.cost_reduction_factor
            ));
        }

        report.push_str("\n═══════════════════════════════════════════════════\n");

        report
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_quality_gap_calculation() {
        let gap = QualityGap::new(0.92, 0.78, 0.895, 0.05);

        assert_eq!(gap.teacher_quality, 0.92);
        assert_eq!(gap.distilled_quality, 0.895);
        assert!((gap.absolute_gap - 0.025).abs() < 0.001);
        assert!((gap.relative_gap_percent - 2.717).abs() < 0.01);
        assert!(gap.acceptable);
        assert!((gap.improvement - 0.115).abs() < 0.001);
    }

    #[test]
    fn test_cost_analysis() {
        let cost = CostAnalysis::new(0.0045, 0.00042, 2.25, 500);

        assert_eq!(cost.teacher_cost_per_request, 0.0045);
        assert_eq!(cost.student_cost_per_request, 0.00042);
        assert!((cost.cost_reduction_factor - 10.714).abs() < 0.01);
        assert_eq!(cost.synthetic_data_generation_cost, 2.25);
    }

    #[test]
    fn test_roi_metrics() {
        let roi = ROIMetrics::calculate(10_000, 0.0045, 0.00042, 2.25);

        assert_eq!(roi.requests_per_day, 10_000);
        assert!((roi.daily_cost_teacher - 45.0).abs() < 0.01);
        assert!((roi.daily_cost_student - 4.2).abs() < 0.01);
        assert!((roi.daily_savings - 40.8).abs() < 0.01);
        assert!((roi.monthly_savings - 1224.0).abs() < 1.0);
        assert!(roi.payback_hours > 1.0 && roi.payback_hours < 2.0);
    }

    #[test]
    fn test_payback_string_formatting() {
        let roi = ROIMetrics::calculate(10_000, 0.0045, 0.00042, 2.25);
        let payback = roi.payback_string();
        assert!(payback.contains("hours"));

        let roi2 = ROIMetrics::calculate(1_000, 0.0045, 0.00042, 2.25);
        let payback2 = roi2.payback_string();
        // With 1000 req/day: payback = 2.25 / ((0.0045-0.00042)*1000) = 0.55 days
        assert!(payback2.contains("day") || payback2.contains("hour"));
    }

    #[test]
    fn test_distillation_report_formatting() {
        let quality = QualityGap::new(0.92, 0.78, 0.895, 0.05);
        let cost = CostAnalysis::new(0.0045, 0.00042, 2.25, 500);
        let roi = Some(ROIMetrics::calculate(10_000, 0.0045, 0.00042, 2.25));

        let report = DistillationReport::new(quality, cost, roi);
        let formatted = report.format_report();

        assert!(formatted.contains("QUALITY METRICS"));
        assert!(formatted.contains("COST METRICS"));
        assert!(formatted.contains("ROI ANALYSIS"));
        assert!(formatted.contains("RECOMMENDATION"));
        assert!(formatted.contains("92.0%"));
        assert!(formatted.contains("89.5%"));
        assert!(formatted.contains("10.7x"));
        assert!(formatted.contains("1224"));
    }
}
