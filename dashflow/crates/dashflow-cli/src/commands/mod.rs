//! # CLI Commands
//!
//! This module contains all CLI subcommands for the DashFlow CLI tool.
//!
//! ## Command Categories
//!
//! - **Timeline** (M-38 unified interface): `timeline` with subcommands `live`, `replay`, `view`, `export`
//! - **Streaming Telemetry**: `analyze`, `costs`, `diff`, `export`, `flamegraph`, `inspect`, `profile`, `replay`, `tail`, `watch`
//! - **Optimization**: `baseline`, `dataset`, `eval`, `optimize`, `train`
//! - **Developer Tools**: `debug`, `lint`, `visualize`
//! - **Pattern Detection**: `patterns`
//! - **Infrastructure**: `executions`, `locks`, `status`
//! - **Introspection**: `docs_index`, `introspect`, `mcp_server`
//! - **Self-Improvement**: `self_improve`
//! - **Project**: `new`, `pkg`

// Unified timeline interface (M-38)
pub mod timeline;

// Streaming telemetry commands
pub mod analyze;
pub mod costs;
pub mod diff;
pub mod export;
pub mod flamegraph;
pub mod inspect;
pub mod profile;
pub mod replay;
pub mod tail;
pub mod watch;

// Optimization commands
pub mod baseline;
pub mod dataset;
pub mod eval;
pub mod evals;
pub mod optimize;
pub mod train;

// Developer tools
pub mod debug;
pub mod lint;
pub mod visualize;

// Pattern detection
pub mod patterns;

// Parallel AI development coordination
pub mod locks;

// Infrastructure health
pub mod executions;
pub mod status;

// Introspection
pub mod docs_index;
pub mod introspect;
pub mod mcp_server;

// Self-improvement
pub mod self_improve;

// Project scaffolding
pub mod new;

// Package registry
pub mod pkg;

#[cfg(test)]
mod tests {
    #[test]
    fn command_modules_are_linked() {
        let _ = std::any::type_name::<super::status::StatusArgs>();
        let _ = std::any::type_name::<super::profile::ProfileArgs>();
    }
}
