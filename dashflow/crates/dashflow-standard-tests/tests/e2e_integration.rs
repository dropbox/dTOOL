//! End-to-end integration tests with real APIs
//!
//! DashFlow features work correctly end-to-end
//! with real external services (OpenAI, etc.). They are marked with #[ignore]
//! so they don't run in CI by default.
//!
//! Run with: cargo test --test integration -- --ignored --nocapture
//! Or specific test: cargo test --test integration test_name -- --ignored --nocapture

mod common;

#[path = "integration/tool_calling_e2e.rs"]
mod tool_calling_e2e;

#[path = "integration/react_agent_e2e.rs"]
mod react_agent_e2e;

#[path = "integration/failure_modes.rs"]
mod failure_modes;

#[path = "integration/output_quality.rs"]
mod output_quality;
