//! Test modules for dterm-core.
//!
//! This module contains:
//! - Property-based tests (proptest)
//! - Integration tests for Terminal
//! - VTTEST conformance tests
//! - CVE regression tests
//! - Visual regression tests (GPU)
//! - Additional unit tests

mod cve_regression;
mod proptest;
mod terminal_integration;
mod vttest_conformance;

#[cfg(feature = "visual-testing")]
mod visual_regression;
