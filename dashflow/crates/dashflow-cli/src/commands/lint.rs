// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)
//! Platform usage linter CLI command.
//!
//! Detects potential reimplementations of DashFlow platform features in app code.
//! Helps developers (human or AI) discover existing platform functionality.
//!
//! # Usage
//!
//! ```bash
//! dashflow lint examples/apps/librarian
//! dashflow lint --explain src/
//! dashflow lint --format json .
//! dashflow lint --use-yaml src/            # Use static YAML patterns instead of registry
//! dashflow lint --feedback "Reason not using platform feature" src/
//! dashflow lint feedback list
//! dashflow lint feedback export
//! ```
//!
//! By default, patterns are loaded from the introspection-based registry
//! populated via `#[dashflow::capability(...)]` proc macro attributes.
//! Use `--use-yaml` for stable, reproducible CI runs with static patterns.

use crate::output::{print_error, print_info, print_success, print_warning};
use anyhow::Result;
use clap::{Args, Subcommand};
use dashflow::core::config_loader::env_vars::{env_is_set, env_string, CI, CLAUDE_CODE, DASHFLOW_WORKER_ID};
use dashflow::lint::{
    lint_path, send_report, FeedbackCollector, FeedbackEntry, LintConfig, OutputFormat,
    ReportDestination, Severity, TelemetryCollector, TelemetryConfig,
};
use std::path::PathBuf;

/// Lint for potential platform feature reimplementations
#[derive(Args)]
pub struct LintArgs {
    /// Subcommand for feedback operations
    #[command(subcommand)]
    pub command: Option<LintCommand>,

    /// File or directory to scan for potential reimplementations
    #[arg(default_value = ".")]
    pub path: PathBuf,

    /// Show detailed explanations with example usage
    #[arg(long, short)]
    pub explain: bool,

    /// Output format (text or json)
    #[arg(long, value_enum, default_value = "text")]
    pub format: LintOutputFormat,

    /// Minimum severity level to report (info, warn, error)
    #[arg(long, value_enum, default_value = "warn")]
    pub severity: SeverityArg,

    /// Additional paths to exclude (can be specified multiple times)
    #[arg(long, short = 'x')]
    pub exclude: Vec<String>,

    /// Follow symbolic links
    #[arg(long)]
    pub follow_symlinks: bool,

    /// Provide feedback on why platform feature wasn't used (submitted with lint results)
    #[arg(long, short = 'F')]
    pub feedback: Option<String>,

    /// Enable anonymous telemetry collection for this lint run
    #[arg(long)]
    pub enable_telemetry: bool,

    /// Use YAML patterns instead of registry (default uses registry)
    ///
    /// By default, patterns are loaded from the introspection-based registry
    /// populated via `#[dashflow::capability(...)]` proc macro attributes.
    /// Use this flag to load patterns from static YAML files instead, which
    /// is useful for stable, reproducible CI runs.
    #[arg(long)]
    pub use_yaml: bool,
}

/// Lint subcommands
#[derive(Subcommand)]
pub enum LintCommand {
    /// Manage lint feedback
    Feedback(FeedbackArgs),
    /// Manage telemetry for gap reporting
    Telemetry(TelemetryArgs),
}

/// Feedback management arguments
#[derive(Args)]
pub struct FeedbackArgs {
    #[command(subcommand)]
    pub command: FeedbackCommand,
}

/// Feedback subcommands
#[derive(Subcommand)]
pub enum FeedbackCommand {
    /// List all collected feedback
    List {
        /// Filter by pattern name
        #[arg(long)]
        pattern: Option<String>,

        /// Filter by category
        #[arg(long)]
        category: Option<String>,

        /// Show only unreviewed feedback
        #[arg(long)]
        unreviewed: bool,

        /// Output format (text or json)
        #[arg(long, value_enum, default_value = "text")]
        format: LintOutputFormat,
    },

    /// Show feedback summary statistics
    Summary,

    /// Export feedback to a file
    Export {
        /// Output file path (defaults to stdout)
        #[arg(long, short)]
        output: Option<PathBuf>,

        /// Output format (json)
        #[arg(long, value_enum, default_value = "json")]
        format: LintOutputFormat,
    },

    /// Mark feedback as reviewed
    Review {
        /// Feedback ID to mark as reviewed
        id: String,
    },

    /// Submit feedback manually
    Submit {
        /// Pattern name the feedback is about
        #[arg(long)]
        pattern: String,

        /// The reason for not using the platform feature
        reason: String,

        /// Optional suggested enhancement
        #[arg(long)]
        enhancement: Option<String>,
    },
}

/// Telemetry management arguments
#[derive(Args)]
pub struct TelemetryArgs {
    #[command(subcommand)]
    pub command: TelemetryCommand,
}

/// Telemetry subcommands
#[derive(Subcommand)]
pub enum TelemetryCommand {
    /// Show current telemetry status
    Status,

    /// Enable telemetry collection
    Enable,

    /// Disable telemetry collection
    Disable,

    /// Preview what data would be sent (without sending)
    Preview,

    /// Send accumulated telemetry report
    Send {
        /// Output file instead of remote endpoint
        #[arg(long)]
        output: Option<PathBuf>,
    },

    /// Clear accumulated telemetry data
    Clear,
}

/// Output format for lint results
#[derive(clap::ValueEnum, Clone, Copy, Debug)]
pub enum LintOutputFormat {
    /// Human-readable text output (default)
    Text,
    /// JSON output for automation
    Json,
    /// SARIF format for IDE integration (VS Code, IntelliJ, GitHub)
    Sarif,
}

impl From<LintOutputFormat> for OutputFormat {
    fn from(f: LintOutputFormat) -> Self {
        match f {
            LintOutputFormat::Text => OutputFormat::Text,
            LintOutputFormat::Json => OutputFormat::Json,
            LintOutputFormat::Sarif => OutputFormat::Sarif,
        }
    }
}

/// Severity level for filtering warnings
#[derive(clap::ValueEnum, Clone, Copy, Debug)]
pub enum SeverityArg {
    /// Show all messages including informational
    Info,
    /// Show warnings and errors (default)
    Warn,
    /// Show only errors
    Error,
}

impl From<SeverityArg> for Severity {
    fn from(s: SeverityArg) -> Self {
        match s {
            SeverityArg::Info => Severity::Info,
            SeverityArg::Warn => Severity::Warn,
            SeverityArg::Error => Severity::Error,
        }
    }
}

/// Find the workspace root directory (async to avoid blocking I/O)
async fn find_workspace_root() -> Option<PathBuf> {
    // Try common locations
    let candidates = vec![
        PathBuf::from("."),
        PathBuf::from(".."),
        PathBuf::from("../.."),
    ];

    for candidate in candidates {
        let cargo_toml = candidate.join("Cargo.toml");
        if tokio::fs::try_exists(&cargo_toml).await.unwrap_or(false) {
            if let Ok(content) = tokio::fs::read_to_string(&cargo_toml).await {
                if content.contains("[workspace]") {
                    // Wrap blocking canonicalize in spawn_blocking
                    let candidate_clone = candidate.clone();
                    return tokio::task::spawn_blocking(move || candidate_clone.canonicalize().ok())
                        .await
                        .ok()
                        .flatten();
                }
            }
        }
    }

    None
}

/// Run the lint command
pub async fn run(args: LintArgs) -> Result<()> {
    // Handle subcommands first
    if let Some(command) = args.command {
        return match command {
            LintCommand::Feedback(feedback_args) => run_feedback_command(feedback_args).await,
            LintCommand::Telemetry(telemetry_args) => run_telemetry_command(telemetry_args).await,
        };
    }

    let mut config = LintConfig::new()
        .with_explain(args.explain)
        .with_format(args.format.into())
        .with_min_severity(args.severity.into())
        .with_excludes(args.exclude)
        .with_telemetry(args.enable_telemetry)
        .with_use_registry(!args.use_yaml); // Registry is default; --use-yaml disables it

    // Auto-detect workspace root for introspection
    if let Some(workspace_root) = find_workspace_root().await {
        config = config.with_workspace_root(workspace_root);
    }

    // Wrap blocking canonicalize in spawn_blocking
    let path_to_resolve = args.path.clone();
    let path = tokio::task::spawn_blocking(move || {
        path_to_resolve.canonicalize().unwrap_or(path_to_resolve)
    })
    .await
    .unwrap_or(args.path.clone());

    print_info(&format!(
        "Scanning {} for platform feature reimplementations...",
        path.display()
    ));

    // Initialize telemetry collector if enabled
    let mut telemetry = TelemetryCollector::new();
    if args.enable_telemetry || telemetry.is_enabled() {
        telemetry.record_lint_run();
    }

    let result = lint_path(&path, config).await?;

    // Record telemetry for warnings found
    if args.enable_telemetry || telemetry.is_enabled() {
        for warning in &result.warnings {
            telemetry.record_pattern_match(&warning.pattern, &warning.category);
        }
        if let Err(e) = telemetry.save() {
            eprintln!("Warning: Failed to save telemetry: {}", e);
        }
    }

    // If feedback was provided and there are warnings, submit feedback for each
    if let Some(ref feedback_reason) = args.feedback {
        if !result.warnings.is_empty() {
            let mut collector = FeedbackCollector::new();
            let reporter = detect_reporter();

            for warning in &result.warnings {
                let entry = FeedbackEntry::new(
                    warning.pattern.clone(),
                    warning.category.clone(),
                    warning.file.clone(),
                    warning.line,
                    feedback_reason.clone(),
                    warning.platform_module.clone(),
                    reporter.clone(),
                );
                collector.add_feedback(entry)?;
            }

            print_info(&format!(
                "Submitted feedback for {} warnings to {}",
                result.warnings.len(),
                collector.store_path().display()
            ));
        }
    }

    // Output results based on format
    match args.format {
        LintOutputFormat::Text => {
            let output = result.to_text(args.explain);
            println!("{}", output);

            // Print summary with colors
            if result.warnings.is_empty() {
                print_success("No potential reimplementations detected.");
            } else if result.has_errors() {
                print_warning(&format!(
                    "Found {} potential reimplementations that should use DashFlow platform features.",
                    result.warning_count()
                ));
            } else {
                print_info(&format!(
                    "Found {} potential reimplementations. Consider using DashFlow platform features.",
                    result.warning_count()
                ));
            }
        }
        LintOutputFormat::Json => {
            let json = result.to_json()?;
            println!("{}", json);
        }
        LintOutputFormat::Sarif => {
            let sarif = result.to_sarif()?;
            println!("{}", sarif);
        }
    }

    // Exit with error code if there are errors
    if result.has_errors() {
        std::process::exit(1);
    }

    Ok(())
}

/// Run feedback subcommand
async fn run_feedback_command(args: FeedbackArgs) -> Result<()> {
    match args.command {
        FeedbackCommand::List {
            pattern,
            category,
            unreviewed,
            format,
        } => {
            let collector = FeedbackCollector::new();
            let store = collector.store();

            let entries: Vec<_> = store
                .entries
                .iter()
                .filter(|e| {
                    if let Some(ref p) = pattern {
                        if &e.pattern != p {
                            return false;
                        }
                    }
                    if let Some(ref c) = category {
                        if &e.category != c {
                            return false;
                        }
                    }
                    if unreviewed && e.reviewed {
                        return false;
                    }
                    true
                })
                .collect();

            match format {
                LintOutputFormat::Json | LintOutputFormat::Sarif => {
                    let json = serde_json::to_string_pretty(&entries)?;
                    println!("{}", json);
                }
                LintOutputFormat::Text => {
                    if entries.is_empty() {
                        print_info("No feedback entries found.");
                    } else {
                        println!("=== Lint Feedback ({} entries) ===\n", entries.len());
                        for entry in entries {
                            println!("ID: {}", entry.id);
                            println!("  Pattern: {}", entry.pattern);
                            println!("  File: {}:{}", entry.file.display(), entry.line);
                            println!("  Reason: {}", entry.reason);
                            if let Some(ref enh) = entry.suggested_enhancement {
                                println!("  Enhancement: {}", enh);
                            }
                            println!("  Reporter: {}", entry.reporter);
                            println!("  Reviewed: {}", if entry.reviewed { "yes" } else { "no" });
                            println!();
                        }
                    }
                }
            }
        }

        FeedbackCommand::Summary => {
            let collector = FeedbackCollector::new();
            let report = collector.store().summary_report();
            println!("{}", report);
        }

        FeedbackCommand::Export { output, format: _ } => {
            let collector = FeedbackCollector::new();
            let json = collector.export_json()?;

            if let Some(output_path) = output {
                // Use tokio::fs to avoid blocking the async runtime
                tokio::fs::write(&output_path, &json).await?;
                print_success(&format!("Exported feedback to {}", output_path.display()));
            } else {
                println!("{}", json);
            }
        }

        FeedbackCommand::Review { id } => {
            let mut collector = FeedbackCollector::new();
            if collector.store_mut().mark_reviewed(&id) {
                collector.save()?;
                print_success(&format!("Marked feedback {} as reviewed", id));
            } else {
                print_error(&format!("Feedback with ID {} not found", id));
            }
        }

        FeedbackCommand::Submit {
            pattern,
            reason,
            enhancement,
        } => {
            let mut collector = FeedbackCollector::new();
            let reporter = detect_reporter();

            let mut entry = FeedbackEntry::new(
                pattern.clone(),
                "manual".to_string(),
                PathBuf::from("manual-submission"),
                0,
                reason,
                "unknown".to_string(),
                reporter,
            );

            if let Some(enh) = enhancement {
                entry = entry.with_suggested_enhancement(enh);
            }

            let id = entry.id.clone();
            collector.add_feedback(entry)?;
            print_success(&format!("Submitted feedback with ID: {}", id));
        }
    }

    Ok(())
}

/// Run telemetry subcommand
async fn run_telemetry_command(args: TelemetryArgs) -> Result<()> {
    match args.command {
        TelemetryCommand::Status => {
            let config = TelemetryConfig::load();
            println!("=== Lint Telemetry Status ===\n");
            println!(
                "Enabled: {}",
                if config.is_enabled() { "yes" } else { "no" }
            );
            println!("Auto-send: {}", if config.auto_send { "yes" } else { "no" });
            println!("Min runs before send: {}", config.min_runs_before_send);

            if let Some(ref id) = config.installation_id {
                println!("Installation ID: {}", id);
            }

            if let Some(last_sent) = config.last_sent {
                println!("Last sent: {}", last_sent.format("%Y-%m-%d %H:%M:%S UTC"));
            }

            let collector = TelemetryCollector::new();
            if collector.is_ready_for_report() {
                print_info("Ready to send report (min runs threshold reached)");
            }

            println!("\nTo enable: dashflow lint telemetry enable");
            println!("To preview data: dashflow lint telemetry preview");
        }

        TelemetryCommand::Enable => {
            let mut config = TelemetryConfig::load();
            config.enabled = true;
            config.save()?;
            print_success("Telemetry enabled. Thank you for helping improve DashFlow!");
            print_info(
                "Data is anonymous and aggregated. Preview with: dashflow lint telemetry preview",
            );
        }

        TelemetryCommand::Disable => {
            let mut config = TelemetryConfig::load();
            config.enabled = false;
            config.save()?;
            print_success("Telemetry disabled.");
        }

        TelemetryCommand::Preview => {
            let mut collector = TelemetryCollector::new();
            let preview = collector.preview_report();
            println!("=== Telemetry Report Preview ===\n");
            println!("This is what would be sent (anonymized, aggregated data):\n");
            println!("{}", preview);
            println!("\nNote: No source code, file paths, or personal data is included.");
        }

        TelemetryCommand::Send { output } => {
            let mut collector = TelemetryCollector::new();

            if !collector.is_ready_for_report() {
                print_warning("Not enough data accumulated yet for a meaningful report.");
                print_info(&format!(
                    "Need at least {} lint runs. Run more lint commands first.",
                    TelemetryConfig::default().min_runs_before_send
                ));
                return Ok(());
            }

            let report = collector.generate_report();

            let destination = if let Some(path) = output {
                ReportDestination::LocalFile(path)
            } else {
                ReportDestination::default()
            };

            send_report(&report, destination)?;
            collector.clear_data()?;
            print_success("Telemetry report sent and local data cleared.");
        }

        TelemetryCommand::Clear => {
            let mut collector = TelemetryCollector::new();
            collector.clear_data()?;
            print_success("Telemetry data cleared.");
        }
    }

    Ok(())
}

/// Detect if we're running in an AI worker context
fn detect_reporter() -> String {
    // Check for AI worker environment variables
    if let Some(worker_id) = env_string(DASHFLOW_WORKER_ID) {
        return format!("ai-worker-{}", worker_id);
    }

    // Check for CI environment
    if env_is_set(CI) {
        return "ci-automated".to_string();
    }

    // Check for Claude Code
    if env_is_set(CLAUDE_CODE) {
        return "ai-claude-code".to_string();
    }

    // Default to manual user
    "user-manual".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_severity_conversion() {
        assert_eq!(Severity::from(SeverityArg::Info), Severity::Info);
        assert_eq!(Severity::from(SeverityArg::Warn), Severity::Warn);
        assert_eq!(Severity::from(SeverityArg::Error), Severity::Error);
    }

    #[test]
    fn test_format_conversion() {
        assert_eq!(
            OutputFormat::from(LintOutputFormat::Text),
            OutputFormat::Text
        );
        assert_eq!(
            OutputFormat::from(LintOutputFormat::Json),
            OutputFormat::Json
        );
        assert_eq!(
            OutputFormat::from(LintOutputFormat::Sarif),
            OutputFormat::Sarif
        );
    }
}
