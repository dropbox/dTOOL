// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

//! @dashflow-module
//! @name ab_testing
//! @category optimize
//! @status stable
//!
//! A/B testing framework for DashFlow optimization
//!
//! This module provides production-ready A/B testing capabilities for comparing
//! different optimizer configurations and module versions, including:
//! - Deterministic traffic splitting with hash-based routing
//! - Statistical significance testing (t-tests, confidence intervals)
//! - Result reporting in markdown and HTML formats
//! - Support for custom metrics and evaluation functions
//!
//! # Example
//!
//! ```rust,ignore
//! use dashflow::optimize::ab_testing::{ABTest, Variant};
//!
//! let mut ab_test = ABTest::new("optimizer_comparison")
//!     .with_minimum_sample_size(100)
//!     .with_significance_level(0.05);
//!
//! ab_test.add_variant("baseline", 0.5);
//! ab_test.add_variant("optimized", 0.5);
//!
//! // Run test
//! for (input, expected) in test_data {
//!     let variant_name = ab_test.assign_variant(&input.id);
//!     let result = match variant_name {
//!         "baseline" => baseline_module.call(&input).await?,
//!         "optimized" => optimized_module.call(&input).await?,
//!         _ => unreachable!(),
//!     };
//!     let score = evaluate(&result, &expected);
//!     ab_test.record_result(variant_name, score)?;
//! }
//!
//! // Analyze
//! let report = ab_test.analyze()?;
//! println!("{}", report.summary());
//! report.save_html("results.html")?;
//! ```

mod ab_test;
mod analysis;
mod report;
mod traffic;
mod variant;

pub use ab_test::ABTest;
pub use analysis::{ConfidenceInterval, StatisticalAnalysis, TTestResult};
pub use report::{ResultsReport, VariantReport};
pub use traffic::TrafficSplitter;
pub use variant::Variant;

/// Result type for A/B testing operations
pub type Result<T> = std::result::Result<T, Error>;

/// Error types for A/B testing operations
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    /// The specified test variant was not found.
    ///
    /// Check that the variant name matches one defined in the test configuration.
    #[error("Variant not found: {0}")]
    VariantNotFound(String),

    /// Traffic allocation percentages must sum to 1.0.
    ///
    /// The total of all variant weights should equal 1.0 (100% of traffic).
    #[error("Invalid traffic allocation: sum must equal 1.0, got {0}")]
    InvalidTrafficAllocation(f64),

    /// Not enough samples were collected for statistical significance.
    ///
    /// Continue the test until the minimum sample size is reached.
    #[error("Insufficient sample size: need at least {need}, got {got}")]
    InsufficientSampleSize {
        /// Minimum required samples
        need: usize,
        /// Actual samples collected
        got: usize,
    },

    /// The statistical significance test failed.
    ///
    /// This can occur if the test assumptions are violated or data is invalid.
    #[error("Statistical test failed: {0}")]
    StatisticalTestFailed(String),

    /// An I/O operation failed.
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// JSON serialization or deserialization failed.
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
}

#[cfg(test)]
mod tests {
    use super::*;

    // ===========================================
    // Error Display tests
    // ===========================================

    #[test]
    fn test_error_variant_not_found_display() {
        let error = Error::VariantNotFound("test_variant".to_string());
        assert_eq!(error.to_string(), "Variant not found: test_variant");
    }

    #[test]
    fn test_error_invalid_traffic_allocation_display() {
        let error = Error::InvalidTrafficAllocation(1.5);
        assert_eq!(
            error.to_string(),
            "Invalid traffic allocation: sum must equal 1.0, got 1.5"
        );
    }

    #[test]
    fn test_error_insufficient_sample_size_display() {
        let error = Error::InsufficientSampleSize { need: 100, got: 50 };
        assert_eq!(
            error.to_string(),
            "Insufficient sample size: need at least 100, got 50"
        );
    }

    #[test]
    fn test_error_statistical_test_failed_display() {
        let error = Error::StatisticalTestFailed("variance too high".to_string());
        assert_eq!(
            error.to_string(),
            "Statistical test failed: variance too high"
        );
    }

    #[test]
    fn test_error_io_display() {
        let io_error = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
        let error: Error = io_error.into();
        assert!(error.to_string().contains("file not found"));
    }

    #[test]
    fn test_error_json_display() {
        let json_error = serde_json::from_str::<serde_json::Value>("invalid").unwrap_err();
        let error: Error = json_error.into();
        assert!(error.to_string().contains("JSON error"));
    }

    // ===========================================
    // Error Debug tests
    // ===========================================

    #[test]
    fn test_error_variant_not_found_debug() {
        let error = Error::VariantNotFound("test".to_string());
        let debug_str = format!("{:?}", error);
        assert!(debug_str.contains("VariantNotFound"));
        assert!(debug_str.contains("test"));
    }

    #[test]
    fn test_error_invalid_traffic_allocation_debug() {
        let error = Error::InvalidTrafficAllocation(0.5);
        let debug_str = format!("{:?}", error);
        assert!(debug_str.contains("InvalidTrafficAllocation"));
    }

    #[test]
    fn test_error_insufficient_sample_size_debug() {
        let error = Error::InsufficientSampleSize { need: 50, got: 25 };
        let debug_str = format!("{:?}", error);
        assert!(debug_str.contains("InsufficientSampleSize"));
        assert!(debug_str.contains("50"));
        assert!(debug_str.contains("25"));
    }

    #[test]
    fn test_error_statistical_test_failed_debug() {
        let error = Error::StatisticalTestFailed("test failed".to_string());
        let debug_str = format!("{:?}", error);
        assert!(debug_str.contains("StatisticalTestFailed"));
    }

    // ===========================================
    // From trait tests
    // ===========================================

    #[test]
    fn test_error_from_io_error() {
        let io_error = std::io::Error::new(std::io::ErrorKind::PermissionDenied, "access denied");
        let error: Error = io_error.into();
        match error {
            Error::Io(_) => {}
            _ => panic!("Expected Error::Io variant"),
        }
    }

    #[test]
    fn test_error_from_json_error() {
        let json_error = serde_json::from_str::<serde_json::Value>("{invalid}").unwrap_err();
        let error: Error = json_error.into();
        match error {
            Error::Json(_) => {}
            _ => panic!("Expected Error::Json variant"),
        }
    }

    // ===========================================
    // Module re-export tests
    // ===========================================

    #[test]
    fn test_ab_test_export() {
        // Verify ABTest is accessible through module exports
        let test = ABTest::new("test_experiment");
        // Just verify it was created - the name is stored internally
        assert!(test.variant_names().is_empty());
        assert_eq!(test.name(), "test_experiment");
    }

    #[test]
    fn test_variant_export() {
        // Verify Variant is accessible through module exports
        let variant = Variant::new("test", 0.5);
        assert_eq!(variant.name, "test");
        assert_eq!(variant.traffic, 0.5);
    }

    #[test]
    fn test_traffic_splitter_export() {
        // Verify TrafficSplitter is accessible through module exports
        let _splitter = TrafficSplitter::new(vec![("a".to_string(), 0.5), ("b".to_string(), 0.5)]);
    }

    #[test]
    fn test_statistical_analysis_export() {
        // Verify StatisticalAnalysis is accessible
        // Just test that the type exists and can be used
        fn _accepts_analysis(_: StatisticalAnalysis) {}
    }

    #[test]
    fn test_t_test_result_export() {
        // Verify TTestResult is accessible
        fn _accepts_t_test(_: TTestResult) {}
    }

    #[test]
    fn test_confidence_interval_export() {
        // Verify ConfidenceInterval is accessible
        fn _accepts_ci(_: ConfidenceInterval) {}
    }

    #[test]
    fn test_results_report_export() {
        // Verify ResultsReport is accessible
        fn _accepts_report(_: ResultsReport) {}
    }

    #[test]
    fn test_variant_report_export() {
        // Verify VariantReport is accessible
        fn _accepts_variant_report(_: VariantReport) {}
    }

    // ===========================================
    // Result type alias test
    // ===========================================

    #[test]
    fn test_result_type_alias() {
        let ok_result: Result<i32> = Ok(42);
        assert_eq!(ok_result.unwrap(), 42);

        let err_result: Result<i32> = Err(Error::VariantNotFound("missing".to_string()));
        assert!(err_result.is_err());
    }
}
