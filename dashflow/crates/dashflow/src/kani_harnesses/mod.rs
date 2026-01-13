//! Kani proof harnesses for the dashflow crate.
//!
//! These harnesses verify critical properties using the Kani model checker.
//! Run with: `cargo kani -p dashflow`
//!
//! # Adding New Harnesses
//!
//! 1. Add functions with `#[kani::proof]` attribute
//! 2. Use `kani::any()` for symbolic inputs
//! 3. Keep state spaces small (bounded types)
//! 4. Test with: `cargo kani --harness <harness_name>`

// Only compile when running under Kani
#![cfg(kani)]

mod state_graph;
