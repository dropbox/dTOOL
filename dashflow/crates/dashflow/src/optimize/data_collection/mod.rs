// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

//! @dashflow-module
//! @name data_collection
//! @category optimize
//! @status stable
//!
//! Data Collection and Class Balancing Framework
//!
//! This module provides tools for collecting training data from production systems,
//! analyzing label distributions, detecting class imbalance, and managing balanced datasets.
//!
//! # Core Components
//!
//! - [`DataCollector`] - Captures input/output pairs from production execution
//! - [`DistributionAnalyzer`] - Analyzes label distributions and detects imbalance
//! - [`TrainingExample`] - Represents a single training data point
//!
//! # Usage Example
//!
//! ```rust,no_run
//! use dashflow::optimize::data_collection::*;
//! use std::collections::HashMap;
//!
//! # #[tokio::main]
//! # async fn main() -> Result<(), Box<dyn std::error::Error>> {
//! // 1. Create collector
//! let format = DataFormat::classification("query", "category");
//! let store = DataStore::memory();
//! let mut collector = DataCollector::new(format, store);
//!
//! // 2. Collect production data
//! let mut data = HashMap::new();
//! data.insert("query".to_string(), serde_json::json!("What is my balance?"));
//! data.insert("category".to_string(), serde_json::json!("billing"));
//! collector.collect(data).await?;
//!
//! // 3. Analyze distribution
//! let examples = collector.load_dataset().await?;
//! let analyzer = DistributionAnalyzer::new()
//!     .with_min_examples_per_class(100);
//! let analysis = analyzer.analyze(&examples);
//!
//! println!("Total examples: {}", analysis.total_examples);
//! println!("Imbalance ratio: {:.2}", analysis.imbalance_ratio);
//! # Ok(())
//! # }
//! ```

pub mod analyzer;
pub mod collector;
pub mod types;

pub use analyzer::{DistributionAnalysis, DistributionAnalyzer};
pub use collector::{DataCollector, DataFormat, DataStore};
pub use types::{DataSource, TrainingExample};
