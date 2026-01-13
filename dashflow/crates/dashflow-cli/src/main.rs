// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)
// Allow clippy warnings for CLI application
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::clone_on_ref_ptr)]
#![allow(clippy::needless_pass_by_value, clippy::redundant_clone)]

use anyhow::Result;
use clap::{Parser, Subcommand};

mod commands;
mod helpers;
mod output;

use commands::{
    analyze, baseline, costs, dataset, debug, diff, eval, evals, executions, export, flamegraph,
    inspect, introspect, lint, locks, mcp_server, new, optimize, patterns, pkg, profile, replay,
    self_improve, status, tail, timeline, train, visualize, watch,
};
use output::print_warning;

/// DashFlow CLI - Unified tooling for AI workflow optimization
///
/// Commands are organized into several categories:
///
/// **Timeline** (M-38 unified interface - RECOMMENDED):
///   timeline live, timeline replay, timeline view, timeline export
///
/// **Streaming Telemetry** (from DashFlow Streaming):
///   tail, inspect, replay, diff, export, flamegraph, costs, profile
///
/// **Prompt Optimization** (from DashOptimize):
///   baseline, optimize, eval, train, dataset
///
/// **Developer Tools**:
///   visualize, debug
///
/// **Pattern Detection**:
///   patterns
///
/// **Parallel AI Development**:
///   locks
///
/// **Infrastructure Health**:
///   status
///
/// **Project Scaffolding**:
///   new
#[derive(Parser)]
#[command(name = "dashflow")]
#[command(author = "Andrew Yates")]
#[command(version = env!("CARGO_PKG_VERSION"))]
#[command(about = "Unified DashFlow CLI - streaming telemetry and prompt optimization", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    // === Unified Timeline Interface (M-38) ===
    /// Unified timeline interface for graph execution observation (RECOMMENDED)
    ///
    /// Provides consistent subcommands: live, replay, view, export
    /// with harmonized flag naming (e.g., --thread instead of --thread-id)
    Timeline(timeline::TimelineArgs),

    // === Streaming Telemetry Commands ===
    /// Stream live events from Kafka
    Tail(tail::TailArgs),

    /// DEPRECATED: Watch live graph execution with TUI visualization (use `timeline live`)
    Watch(watch::WatchArgs),

    /// Show thread details and execution history
    Inspect(inspect::InspectArgs),

    /// DEPRECATED: Replay execution from a checkpoint (use `timeline replay`)
    Replay(replay::ReplayArgs),

    /// Compare two checkpoints
    Diff(diff::DiffArgs),

    /// Export thread data to JSON
    Export(export::ExportArgs),

    /// Generate flamegraph for performance visualization (Kafka)
    Flamegraph(flamegraph::FlamegraphArgs),

    /// Analyze token costs across executions (Kafka)
    Costs(costs::CostsArgs),

    /// Profile execution performance (Kafka)
    Profile(profile::ProfileArgs),

    /// Analyze exported JSON files offline (no Kafka required)
    Analyze(analyze::AnalyzeArgs),

    // === Optimization Commands ===
    /// Manage evaluation baselines and check for regressions (M-301)
    Baseline(baseline::BaselineArgs),

    /// Run prompt optimization on a graph
    Optimize(optimize::OptimizeArgs),

    /// Evaluate graph performance on a test dataset
    Eval(eval::EvalArgs),

    /// Train or fine-tune models (distillation, RL)
    Train(train::TrainArgs),

    /// Dataset utilities (generate, validate, inspect)
    Dataset(dataset::DatasetArgs),

    /// Manage evaluation test cases and golden datasets (M-2464)
    Evals(evals::EvalsArgs),

    // === Developer Tools ===
    /// DEPRECATED: Visualize DashFlow graphs (use `timeline view/export`)
    Visualize(visualize::VisualizeArgs),

    /// Interactive debugger for step-through graph execution
    Debug(debug::DebugArgs),

    /// Lint for platform feature reimplementations
    Lint(lint::LintArgs),

    // === Pattern Detection ===
    /// Detect patterns in execution traces (unified pattern engine)
    Patterns(patterns::PatternsArgs),

    // === Parallel AI Development ===
    /// Manage parallel AI development locks (list, acquire, release)
    Locks(locks::LocksArgs),

    // === Infrastructure Health ===
    /// Check DashFlow infrastructure health (Docker, Kafka, services)
    Status(status::StatusArgs),

    /// Query persisted execution data from the EventStore
    Executions(executions::ExecutionsArgs),

    // === Introspection ===
    /// Query DashFlow module information directly (CLI-first introspection)
    Introspect(introspect::IntrospectArgs),

    /// MCP server for AI introspection (HTTP API for external tools)
    McpServer(mcp_server::McpServerArgs),

    // === Self-Improvement ===
    /// Self-improvement commands for AI agents (analyze, plans, approve)
    SelfImprove(self_improve::SelfImproveArgs),

    // === Project Scaffolding ===
    /// Create a new DashFlow application with production defaults
    New(new::NewArgs),

    // === Package Registry ===
    /// Package registry operations (search, install, publish)
    Pkg(pkg::PkgArgs),
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        // Unified timeline interface (M-38)
        Commands::Timeline(args) => timeline::run(args).await,
        // Streaming telemetry
        Commands::Tail(args) => tail::run(args).await,
        Commands::Watch(args) => {
            print_warning("`dashflow watch` is deprecated; use `dashflow timeline live` instead.");
            watch::run(args).await
        }
        Commands::Inspect(args) => inspect::run(args).await,
        Commands::Replay(args) => {
            print_warning(
                "`dashflow replay` is deprecated; use `dashflow timeline replay` instead.",
            );
            replay::run(args).await
        }
        Commands::Diff(args) => diff::run(args).await,
        Commands::Export(args) => export::run(args).await,
        Commands::Flamegraph(args) => flamegraph::run(args).await,
        Commands::Costs(args) => costs::run(args).await,
        Commands::Profile(args) => profile::run(args).await,
        Commands::Analyze(args) => analyze::run(args).await,
        // Optimization
        Commands::Baseline(args) => baseline::run(args).await,
        Commands::Optimize(args) => optimize::run(args).await,
        Commands::Eval(args) => eval::run(args).await,
        Commands::Train(args) => train::run(args).await,
        Commands::Dataset(args) => dataset::run(args).await,
        Commands::Evals(args) => evals::run(args).await,
        // Developer tools
        Commands::Visualize(args) => {
            print_warning(
                "`dashflow visualize` is deprecated; use `dashflow timeline view/export` instead.",
            );
            visualize::run(args).await
        }
        Commands::Debug(args) => debug::run(args).await,
        Commands::Lint(args) => lint::run(args).await,
        // Pattern detection
        Commands::Patterns(args) => patterns::run(args).await,
        // Parallel AI development
        Commands::Locks(args) => locks::run(args).await,
        // Infrastructure health
        Commands::Status(args) => status::run(args).await,
        Commands::Executions(args) => executions::run(args).await,
        // Introspection
        Commands::Introspect(args) => introspect::run(args).await,
        Commands::McpServer(args) => mcp_server::run(args).await,
        // Self-improvement
        Commands::SelfImprove(args) => self_improve::run(args).await,
        // Project scaffolding
        Commands::New(args) => new::run(args).await,
        // Package registry
        Commands::Pkg(args) => pkg::run(args).await,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clap_parses_known_subcommands() {
        let cli = Cli::try_parse_from(["dashflow", "status"]).expect("parse status");
        assert!(matches!(cli.command, Commands::Status(_)));

        let cli =
            Cli::try_parse_from(["dashflow", "profile", "--thread", "abc"]).expect("profile");
        assert!(matches!(cli.command, Commands::Profile(_)));
    }

    #[test]
    fn clap_enforces_required_args() {
        assert!(Cli::try_parse_from(["dashflow", "profile"]).is_err());
        assert!(Cli::try_parse_from(["dashflow", "export"]).is_err());
    }
}
