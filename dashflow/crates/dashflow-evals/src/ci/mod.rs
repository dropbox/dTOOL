//! CI/CD Integration Module
//!
//! Provides tools for integrating evaluation results into CI/CD pipelines.

pub mod gates;

pub use gates::{GateResult, GateViolation, QualityGate, QualityGateConfig};
