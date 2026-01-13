// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

//! CLI commands for DashFlow self-improvement.
//!
//! Provides CLI access to the self-improvement system for AI agents.
//! Storage auto-creates on first use (no init command needed).
//!
//! # Output Formats
//!
//! Most self-improve subcommands support `--format` for output format selection:
//! - `--format table` (default): Human-readable colored output
//! - `--format json`: Machine-readable JSON output for automation
//!
//! # Examples
//!
//! ```bash
//! # Run analysis to generate improvement plans
//! dashflow self-improve analyze
//! dashflow self-improve analyze --format json
//!
//! # List pending improvement plans
//! dashflow self-improve plans
//! dashflow self-improve plans --format json
//!
//! # Approve a plan for implementation
//! dashflow self-improve approve <plan-id>
//!
//! # Start daemon with JSON output
//! dashflow self-improve daemon --once --format json
//!
//! # Generate tests in JSON format
//! dashflow self-improve generate-tests --format json
//! ```

use crate::output::{create_table, print_error, print_info, OutputFormat};
use anyhow::Result;
use clap::{Args, Subcommand};
use colored::Colorize;
use dashflow::self_improvement::{
    approve_plan_cli, list_plans_cli, run_cli_introspection, run_daemon_cli,
    run_test_generation_cli, Alert, AlertDispatcher, ConsoleAlertHandler, FileAlertHandler,
    WebhookAlertHandler,
};
use std::path::PathBuf;

/// Self-improvement commands for AI agents
#[derive(Args)]
pub struct SelfImproveArgs {
    #[command(subcommand)]
    pub command: SelfImproveCommand,
}

#[derive(Subcommand)]
pub enum SelfImproveCommand {
    /// Run introspection analysis to generate improvement plans
    Analyze(AnalyzeArgs),

    /// List improvement plans
    Plans(PlansArgs),

    /// Approve a plan for implementation
    Approve(ApproveArgs),

    /// Start background analysis daemon
    Daemon(DaemonArgs),

    /// Generate regression tests from execution traces
    GenerateTests(GenerateTestsArgs),
}

/// Run analysis to generate improvement plans
#[derive(Args)]
pub struct AnalyzeArgs {
    /// Analysis depth: metrics, local, deep (or full)
    #[arg(long, default_value = "local")]
    depth: String,

    /// Reason for triggering analysis
    #[arg(long)]
    reason: Option<String>,

    /// Custom storage path (default: .dashflow/introspection - auto-created)
    #[arg(long)]
    storage: Option<PathBuf>,

    /// Output format (table or json)
    #[arg(long, value_enum, default_value_t = OutputFormat::Table)]
    format: OutputFormat,
}

/// List improvement plans
#[derive(Args)]
pub struct PlansArgs {
    /// Filter by status: pending, approved, implemented
    #[arg(long)]
    status: Option<String>,

    /// Custom storage path
    #[arg(long)]
    storage: Option<PathBuf>,

    /// Output format (table or json)
    #[arg(long, value_enum, default_value_t = OutputFormat::Table)]
    format: OutputFormat,
}

/// Approve a plan for implementation
#[derive(Args)]
pub struct ApproveArgs {
    /// Plan ID to approve (UUID format)
    pub plan_id: String,

    /// Assignee for implementation (default: ai-worker)
    #[arg(long, default_value = "ai-worker")]
    assignee: String,

    /// Custom storage path
    #[arg(long)]
    storage: Option<PathBuf>,
}

/// Start background analysis daemon
#[derive(Args)]
pub struct DaemonArgs {
    /// Analysis interval in seconds (default: 60)
    #[arg(long, default_value = "60")]
    interval: u64,

    /// Custom storage path (default: .dashflow/introspection)
    #[arg(long)]
    storage: Option<PathBuf>,

    /// Run a single analysis cycle and exit
    #[arg(long)]
    once: bool,

    /// Output format (table or json)
    #[arg(long, value_enum, default_value_t = OutputFormat::Table)]
    format: OutputFormat,

    /// File path to log alerts (JSON format)
    #[arg(long)]
    alert_file: Option<PathBuf>,

    /// Webhook URL to send alerts to
    #[arg(long)]
    alert_webhook: Option<String>,

    /// Disable console alerts (only use file/webhook)
    #[arg(long)]
    no_console_alerts: bool,
}

/// Generate regression tests from execution traces
#[derive(Args)]
pub struct GenerateTestsArgs {
    /// Maximum number of tests to generate
    #[arg(long, default_value = "10")]
    limit: usize,

    /// Output format (table for Rust code, json for test specs)
    #[arg(long, value_enum, default_value_t = OutputFormat::Table)]
    format: OutputFormat,

    /// Output file path (writes to stdout if not specified)
    #[arg(long, short)]
    output: Option<PathBuf>,

    /// Custom traces directory (default: .dashflow/traces)
    #[arg(long)]
    traces: Option<PathBuf>,

    /// Include timing assertions (soft bounds)
    #[arg(long)]
    include_timing: bool,
}

pub async fn run(args: SelfImproveArgs) -> Result<()> {
    match args.command {
        SelfImproveCommand::Analyze(analyze_args) => run_analyze(analyze_args).await,
        SelfImproveCommand::Plans(plans_args) => run_plans(plans_args).await,
        SelfImproveCommand::Approve(approve_args) => run_approve(approve_args).await,
        SelfImproveCommand::GenerateTests(gen_args) => run_generate_tests(gen_args).await,
        SelfImproveCommand::Daemon(daemon_args) => run_daemon(daemon_args).await,
    }
}

async fn run_analyze(args: AnalyzeArgs) -> Result<()> {
    let storage_path = args
        .storage
        .as_ref()
        .map(|p| p.to_string_lossy().to_string());

    let json_output = matches!(args.format, OutputFormat::Json);
    if !json_output {
        println!();
        println!("{}", "Self-Improvement Analysis".bright_cyan().bold());
        println!("{}", "═".repeat(60).bright_cyan());
        println!();
        println!("  {}: {}", "Depth".bright_cyan(), args.depth);
        if let Some(ref reason) = args.reason {
            println!("  {}: {}", "Reason".bright_cyan(), reason);
        }
        println!(
            "  {}: {}",
            "Storage".bright_cyan(),
            storage_path
                .as_deref()
                .unwrap_or(".dashflow/introspection (default)")
        );
        println!();
        println!("{}", "Running analysis...".dimmed());
    }

    match run_cli_introspection(
        storage_path.as_deref(),
        Some(&args.depth),
        args.reason.as_deref(),
    ) {
        Ok(result) => {
            if json_output {
                // Create serializable summary
                #[derive(serde::Serialize)]
                struct AnalysisSummary {
                    capability_gaps: usize,
                    plans_generated: usize,
                    hypotheses_tracked: usize,
                    warnings: Vec<String>,
                    saved_files: Vec<String>,
                    plan_ids: Vec<String>,
                }
                let summary = AnalysisSummary {
                    capability_gaps: result.report.capability_gaps.len(),
                    plans_generated: result.plans.len(),
                    hypotheses_tracked: result.hypotheses.len(),
                    warnings: result.warnings.clone(),
                    saved_files: result
                        .saved_files
                        .iter()
                        .map(|p| p.display().to_string())
                        .collect(),
                    plan_ids: result.plans.iter().map(|p| p.id.to_string()).collect(),
                };
                println!("{}", serde_json::to_string_pretty(&summary)?);
            } else {
                println!();
                println!("  {} Analysis complete!", "✓".bright_green());
                println!();

                // Show summary
                println!("  {}", "Summary:".bright_cyan());
                println!(
                    "    Capability gaps found: {}",
                    result.report.capability_gaps.len()
                );
                println!("    Execution plans generated: {}", result.plans.len());
                println!("    Hypotheses tracked: {}", result.hypotheses.len());

                if !result.plans.is_empty() {
                    println!();
                    println!("  {}", "New Plans:".bright_cyan());
                    for plan in &result.plans {
                        println!("    - {} ({})", plan.title, plan.id);
                    }
                    println!();
                    println!(
                        "{}",
                        "Run 'dashflow self-improve plans' to see all plans.".dimmed()
                    );
                }
            }
            Ok(())
        }
        Err(e) => {
            if json_output {
                println!(r#"{{"error": "{}"}}"#, e);
            } else {
                print_error(&format!("Analysis failed: {}", e));
            }
            anyhow::bail!("Analysis failed: {}", e)
        }
    }
}

async fn run_plans(args: PlansArgs) -> Result<()> {
    let storage_path = args
        .storage
        .as_ref()
        .map(|p| p.to_string_lossy().to_string());

    match list_plans_cli(args.status.as_deref(), storage_path.as_deref()) {
        Ok(plans) => {
            if matches!(args.format, OutputFormat::Json) {
                println!("{}", serde_json::to_string_pretty(&plans)?);
                return Ok(());
            }

            // Human-readable output
            println!();
            let status_str = args.status.as_deref().unwrap_or("pending");
            println!(
                "{} {} {} plans",
                "Improvement Plans".bright_cyan().bold(),
                format!("({})", status_str).dimmed(),
                plans.len().to_string().bright_green()
            );
            println!("{}", "═".repeat(80).bright_cyan());

            if plans.is_empty() {
                print_info(&format!("No {} plans found.", status_str));
                println!();
                println!(
                    "{}",
                    "Run 'dashflow self-improve analyze' to generate new plans.".dimmed()
                );
                return Ok(());
            }

            let mut table = create_table();
            table.set_header(vec!["ID", "Title", "Category", "Priority", "Commits"]);

            for plan in &plans {
                let id_short = plan
                    .id
                    .to_string()
                    .split('-')
                    .next()
                    .unwrap_or("?")
                    .to_string();
                let category = format!("{:?}", plan.category);
                let priority = format!("{}", plan.priority);
                let commits = format!("{}", plan.estimated_commits);

                table.add_row(vec![
                    id_short,
                    truncate_str(&plan.title, 40),
                    category,
                    priority,
                    commits,
                ]);
            }

            println!("{table}");
            println!();
            println!(
                "{}",
                "Run 'dashflow self-improve approve <ID>' to approve a plan.".dimmed()
            );
            Ok(())
        }
        Err(e) => {
            print_error(&format!("Failed to list plans: {}", e));
            anyhow::bail!("Failed to list plans: {}", e)
        }
    }
}

async fn run_approve(args: ApproveArgs) -> Result<()> {
    let storage_path = args
        .storage
        .as_ref()
        .map(|p| p.to_string_lossy().to_string());

    println!();
    println!("{}", "Approving Plan".bright_cyan().bold());
    println!("{}", "═".repeat(60).bright_cyan());
    println!();
    println!("  {}: {}", "Plan ID".bright_cyan(), args.plan_id);
    println!("  {}: {}", "Assignee".bright_cyan(), args.assignee);
    println!();

    match approve_plan_cli(&args.plan_id, &args.assignee, storage_path.as_deref()) {
        Ok(()) => {
            println!("  {} Plan approved successfully!", "✓".bright_green());
            println!();
            println!(
                "{}",
                format!("Assigned to '{}' for implementation.", args.assignee).dimmed()
            );
            Ok(())
        }
        Err(e) => {
            print_error(&format!("Failed to approve plan: {}", e));
            anyhow::bail!("Failed to approve plan: {}", e)
        }
    }
}

async fn run_generate_tests(args: GenerateTestsArgs) -> Result<()> {
    let traces_dir = args
        .traces
        .as_ref()
        .map(|p| p.to_string_lossy().to_string());

    let json_output = matches!(args.format, OutputFormat::Json);
    if !json_output {
        println!();
        println!("{}", "Test Generation".bright_cyan().bold());
        println!("{}", "═".repeat(60).bright_cyan());
        println!();
        println!("  {}: {}", "Limit".bright_cyan(), args.limit);
        println!(
            "  {}: {}",
            "Traces".bright_cyan(),
            traces_dir
                .as_deref()
                .unwrap_or(".dashflow/traces (default)")
        );
        println!(
            "  {}: {}",
            "Format".bright_cyan(),
            if json_output { "JSON" } else { "Rust" }
        );
        if args.include_timing {
            println!("  {}: enabled", "Timing bounds".bright_cyan());
        }
        println!();
        println!("{}", "Generating tests from execution traces...".dimmed());
    }

    let result = run_test_generation_cli(
        Some(args.limit),
        json_output,
        args.output.as_deref(),
        traces_dir.as_deref(),
    );

    // Check for errors
    if !result.errors.is_empty() {
        for error in &result.errors {
            print_error(error);
        }
        if result.tests.is_empty() {
            anyhow::bail!("Test generation failed with errors");
        }
    }

    if json_output {
        // JSON output mode - output the test specs
        println!("{}", serde_json::to_string_pretty(&result.tests)?);
    } else if let Some(ref output_path) = result.output_path {
        // File output mode - show summary
        println!();
        println!("  {} Tests generated successfully!", "✓".bright_green());
        println!();
        println!("  {}", "Summary:".bright_cyan());
        println!("    Tests generated: {}", result.tests.len());
        println!("    Traces processed: {}", result.traces_processed);
        let traces_omitted = result.traces_skipped;
        println!("    Traces omitted: {}", traces_omitted);
        println!("    Output file: {}", output_path.display());
    } else if result.tests.is_empty() {
        // No tests generated
        println!();
        print_info("No traces found in .dashflow/traces/");
        println!();
        println!(
            "{}",
            "Run your graphs to generate execution traces first.".dimmed()
        );
    } else {
        // Console output mode - show generated code
        println!();
        println!(
            "  {} Generated {} tests",
            "✓".bright_green(),
            result.tests.len()
        );
        println!();

        // Generate Rust code output
        let config = dashflow::self_improvement::TestGenerationConfig {
            include_timing_bounds: args.include_timing,
            ..Default::default()
        };
        let generator = dashflow::self_improvement::TestGenerator::with_config(config);
        let code = generator.generate_rust_module(&result.tests);

        // Output the generated code
        println!("{}", code);
    }

    Ok(())
}

fn truncate_str(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else if max_len <= 3 {
        "...".to_string()
    } else {
        format!("{}...", &s[..max_len - 3])
    }
}

#[cfg(test)]
#[allow(clippy::items_after_test_module)]
mod tests {
    use super::truncate_str;

    #[test]
    fn truncate_str_leaves_short_strings_untouched() {
        assert_eq!(truncate_str("hello", 10), "hello");
        assert_eq!(truncate_str("", 10), "");
    }

    #[test]
    fn truncate_str_adds_ellipsis_and_respects_max_len() {
        assert_eq!(truncate_str("hello world", 8), "hello...");
        assert_eq!(truncate_str("hello world", 5), "he...");
        assert_eq!(truncate_str("hello world", 3), "...");
        assert_eq!(truncate_str("hello world", 2), "...");
    }
}

async fn run_daemon(args: DaemonArgs) -> Result<()> {
    let storage_path = args
        .storage
        .as_ref()
        .map(|p| p.to_string_lossy().to_string());
    let has_alert_handlers =
        args.alert_file.is_some() || args.alert_webhook.is_some() || !args.no_console_alerts;
    let json_output = matches!(args.format, OutputFormat::Json);

    if !json_output && !args.once {
        println!();
        println!("{}", "Self-Improvement Daemon".bright_cyan().bold());
        println!("{}", "═".repeat(60).bright_cyan());
        println!();
        println!("  {}: {} seconds", "Interval".bright_cyan(), args.interval);
        println!(
            "  {}: {}",
            "Storage".bright_cyan(),
            storage_path
                .as_deref()
                .unwrap_or(".dashflow/introspection (default)")
        );

        // Show alert configuration
        if has_alert_handlers {
            println!();
            println!("  {}", "Alert Handlers:".bright_cyan());
            if !args.no_console_alerts {
                println!("    - Console (colored output)");
            }
            if let Some(ref path) = args.alert_file {
                println!("    - File: {}", path.display());
            }
            if let Some(ref url) = args.alert_webhook {
                println!("    - Webhook: {}", url);
            }
        }

        println!();
        println!(
            "{}",
            "Monitoring for new traces and generating improvement plans...".dimmed()
        );
        println!();
        println!(
            "{}",
            "Triggers: SlowNode (>10s), HighErrorRate (>5%), RepeatedRetry (>3), UnusedCapability"
                .dimmed()
        );
        println!("{}", "Press Ctrl+C to stop.".dimmed());
        println!();
    }

    // Create alert dispatcher if handlers are configured
    let dispatcher = if has_alert_handlers {
        let mut d = AlertDispatcher::new();

        // Console handler (unless disabled or JSON mode)
        if !args.no_console_alerts && !json_output {
            d.add_handler(Box::new(ConsoleAlertHandler::new()));
        }

        // File handler
        if let Some(ref path) = args.alert_file {
            d.add_handler(Box::new(FileAlertHandler::new(path)));
        }

        // Webhook handler
        if let Some(ref url) = args.alert_webhook {
            d.add_handler(Box::new(WebhookAlertHandler::new(url)));
        }

        Some(d)
    } else {
        None
    };

    // Run daemon loop
    if args.once {
        // Single cycle mode
        match run_daemon_cli(Some(args.interval), storage_path.as_deref(), true) {
            Ok(result) => {
                // Dispatch alerts for triggered issues
                if let Some(ref dispatcher) = dispatcher {
                    for trigger in &result.triggers_fired {
                        let alert = Alert::from_trigger(trigger);
                        if let Err(e) = dispatcher.dispatch(&alert).await {
                            eprintln!("Alert dispatch error: {}", e);
                        }
                    }
                }

                if json_output {
                    // Output JSON for single-run mode
                    #[derive(serde::Serialize)]
                    struct DaemonSummary {
                        traces_analyzed: usize,
                        triggers_fired: usize,
                        plans_generated: usize,
                        alerts_dispatched: usize,
                        errors: Vec<String>,
                    }
                    let summary = DaemonSummary {
                        traces_analyzed: result.traces_analyzed,
                        triggers_fired: result.triggers_fired.len(),
                        plans_generated: result.plans_generated.len(),
                        alerts_dispatched: result.triggers_fired.len(),
                        errors: result.errors,
                    };
                    println!("{}", serde_json::to_string_pretty(&summary)?);
                } else {
                    // Human-readable single-run output
                    println!();
                    println!("  {} Analysis cycle complete!", "✓".bright_green());
                    println!();
                    println!("  {}", "Summary:".bright_cyan());
                    println!("    Traces analyzed: {}", result.traces_analyzed);
                    println!("    Triggers fired: {}", result.triggers_fired.len());
                    println!("    Plans generated: {}", result.plans_generated.len());

                    if !result.triggers_fired.is_empty() && dispatcher.is_none() {
                        // Only show triggers if no alert dispatcher (dispatcher shows alerts)
                        println!();
                        println!("  {}", "Triggers:".bright_cyan());
                        for trigger in &result.triggers_fired {
                            println!("    - {}", trigger.trigger_type.description());
                        }
                    }

                    if !result.plans_generated.is_empty() {
                        println!();
                        println!("  {}", "New Plans:".bright_cyan());
                        for plan in &result.plans_generated {
                            println!("    - {} ({})", plan.title, plan.id);
                        }
                    }

                    if !result.errors.is_empty() {
                        println!();
                        println!("  {}", "Errors:".bright_red());
                        for error in &result.errors {
                            println!("    - {}", error);
                        }
                    }
                }
                Ok(())
            }
            Err(e) => {
                if json_output {
                    println!(r#"{{"error": "{}"}}"#, e);
                } else {
                    print_error(&format!("Daemon failed: {}", e));
                }
                anyhow::bail!("Daemon failed: {}", e)
            }
        }
    } else {
        // Continuous mode - run loop with alerts
        loop {
            match run_daemon_cli(Some(args.interval), storage_path.as_deref(), true) {
                Ok(result) => {
                    // Dispatch alerts for triggered issues
                    if let Some(ref dispatcher) = dispatcher {
                        for trigger in &result.triggers_fired {
                            let alert = Alert::from_trigger(trigger);
                            if let Err(e) = dispatcher.dispatch(&alert).await {
                                eprintln!("Alert dispatch error: {}", e);
                            }
                        }
                    }

                    // Print cycle summary (unless JSON mode)
                    if !json_output {
                        println!(
                            "[{}] Analyzed {} traces, {} triggers fired, {} plans generated",
                            chrono::Utc::now().format("%H:%M:%S"),
                            result.traces_analyzed,
                            result.triggers_fired.len(),
                            result.plans_generated.len()
                        );

                        if !result.errors.is_empty() {
                            for error in &result.errors {
                                eprintln!("  Error: {}", error);
                            }
                        }
                    }
                }
                Err(e) => {
                    eprintln!("Daemon cycle error: {}", e);
                }
            }

            // Sleep for interval
            tokio::time::sleep(std::time::Duration::from_secs(args.interval)).await;
        }
    }
}
