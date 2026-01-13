// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

//! @status deprecated
//!
//! Cost monitoring and budget enforcement for optimized LLM applications
//!
//! **DEPRECATED**: This module is deprecated in favor of `dashflow_observability::cost`.
//! The `dashflow-observability` crate provides a consolidated, more feature-rich
//! implementation with unified metrics support.
//!
//! # Migration Guide
//!
//! Instead of:
//! ```ignore
//! use dashflow::optimize::cost_monitoring::{CostMonitor, BudgetConfig, BudgetEnforcer};
//! ```
//!
//! Use:
//! ```ignore
//! use dashflow_observability::cost::{CostTracker, BudgetConfig, BudgetEnforcer};
//! ```
//!
//! Key changes:
//! - `CostMonitor` is now `CostTracker` in `dashflow-observability`
//! - `UsageRecord` is now `CostRecord`
//! - Pricing now uses per-million token format internally
//! - Added `ModelPrice` struct with provider metadata
//! - Added comprehensive multi-provider model database
//! - Added `spent_today`/`spent_month` to `CostReport`
//!
//! # Legacy Example (Deprecated)
//!
//! ```rust,no_run
//! use dashflow::optimize::cost_monitoring::{CostMonitor, BudgetConfig, BudgetEnforcer};
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! // Basic monitoring
//! let monitor = CostMonitor::new()
//!     .with_daily_budget(100.0)  // $100/day
//!     .with_alert_threshold(0.9); // Alert at 90%
//!
//! monitor.record_usage("gpt-4o-mini", 1500, 500)?;
//!
//! let report = monitor.report();
//! println!("Spent today: ${:.2}", report.spent_today);
//! # Ok(())
//! # }
//! ```

mod budget;
mod error;
mod monitor;
mod pricing;

// Allow internal re-exports of deprecated types; external users will still see warnings
#[allow(deprecated)]
pub use budget::{AlertLevel, BudgetConfig, BudgetEnforcer};
#[allow(deprecated)]
pub use error::{CostMonitorError, Result};
#[allow(deprecated)]
pub use monitor::{CostMonitor, CostReport, UsageRecord};
#[allow(deprecated)]
pub use pricing::{ModelPrice, ModelPricing, TokenUsage};
