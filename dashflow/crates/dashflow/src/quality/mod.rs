// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

//! @dashflow-module
//! @name quality
//! @category runtime
//! @status stable
//!
//! Quality assurance module for DashFlow.
//!
//! This module provides architectural solutions for ensuring high-quality responses
//! from LLM agents. Instead of relying solely on prompts, it uses DashFlow's
//! graph features (cycles, conditionals, validation nodes) to GUARANTEE quality.
//!
//! # Innovations
//!
//! ## 1. Self-Correcting Graph (INNOVATION 1)
//! Uses cycles and conditional edges to automatically retry until quality threshold is met.
//!
//! ## 2. Confidence Scoring (INNOVATION 2)
//! LLM rates its own confidence (0.0-1.0) to trigger searches when uncertain.
//!
//! ## 3. Response Validator (INNOVATION 10)
//! Detects when LLM ignores tool results and says "couldn't find" despite data being available.
//!
//! ## 4. Quality Gate (INNOVATION 5)
//! Judges every response and automatically retries until quality threshold is met.
//!
//! ## 5. Tool Result Validator (INNOVATION 6)
//! Validates tool results before passing to LLM (checks for empty, error, relevance).
//!
//! # Architecture
//!
//! ```text
//! Agent → Response → Validator → Quality Gate → Pass? → Done
//!             ↓           ↓           ↓          ↓
//!         Tool used   Check for   Judge score   Fail
//!             ↓       "couldn't"      ↓          ↓
//!         Results      find"      < 0.95?    ← Retry
//! ```
//!
//! # Usage
//!
//! ```rust,ignore
//! use dashflow::quality::{ResponseValidator, ValidationResult};
//!
//! let validator = ResponseValidator::default();
//!
//! // After agent generates response
//! match validator.validate(&response, tool_results.is_some()) {
//!     ValidationResult::Valid => {
//!         // Response is good
//!     }
//!     ValidationResult::ToolResultsIgnored { phrase } => {
//!         // LLM said "couldn't find" but we have results!
//!         // Automatically retry with stronger prompt
//!     }
//! }
//! ```

mod confidence_scorer;
mod quality_gate;
mod response_validator;
mod tool_result_validator;

pub use confidence_scorer::{ConfidenceScore, ConfidenceScorer};
pub use quality_gate::{
    QualityGate, QualityGateConfig, QualityGateResult, QualityScore, RetryStrategy,
};
pub use response_validator::{ResponseValidator, ValidationAction, ValidationResult};
pub use tool_result_validator::{
    ToolResultValidator, ToolValidationAction, ToolValidationResult, ToolValidatorConfig,
};
