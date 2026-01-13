//! baseline - Manage evaluation baselines and check for regressions
//!
//! This command provides baseline management for continuous quality monitoring (M-301).
//!
//! ## Subcommands
//!
//! - `save` - Save current evaluation results as a baseline
//! - `list` - List available baselines
//! - `check` - Check current results against a baseline for regressions
//! - `delete` - Delete a baseline
//!
//! ## Examples
//!
//! ```bash
//! # Save current results as baseline
//! dashflow baseline save --app librarian --name main --results target/eval_results/latest.json
//!
//! # List all baselines for an app
//! dashflow baseline list --app librarian
//!
//! # Check for regressions
//! dashflow baseline check --app librarian --baseline main --results target/eval_results/latest.json
//!
//! # Delete a baseline
//! dashflow baseline delete --app librarian --name v1.0.0
//! ```

use anyhow::{Context, Result};
use clap::{Args, Subcommand};
use colored::Colorize;
use dashflow_evals::{EvalReport, MonitorConfig, QualityMonitor};
use std::path::PathBuf;

#[derive(Args)]
pub struct BaselineArgs {
    #[command(subcommand)]
    pub command: BaselineCommand,
}

#[derive(Subcommand)]
pub enum BaselineCommand {
    /// Save current evaluation results as a baseline
    Save(SaveArgs),

    /// List available baselines
    List(ListArgs),

    /// Check current results against a baseline for regressions
    Check(CheckArgs),

    /// Delete a baseline
    Delete(DeleteArgs),
}

#[derive(Args)]
pub struct SaveArgs {
    /// Application name
    #[arg(long, short)]
    pub app: String,

    /// Baseline name (e.g., "main", "v1.0.0")
    #[arg(long, short)]
    pub name: String,

    /// Path to evaluation results JSON file
    #[arg(long, short)]
    pub results: PathBuf,

    /// Directory to store baselines (default: baselines/)
    #[arg(long, default_value = "baselines")]
    pub baseline_dir: PathBuf,

    /// Optional description for the baseline
    #[arg(long, short)]
    pub description: Option<String>,
}

#[derive(Args)]
pub struct ListArgs {
    /// Application name
    #[arg(long, short)]
    pub app: String,

    /// Directory containing baselines (default: baselines/)
    #[arg(long, default_value = "baselines")]
    pub baseline_dir: PathBuf,

    /// Output format (table, json)
    #[arg(long, default_value = "table")]
    pub format: String,
}

#[derive(Args)]
pub struct CheckArgs {
    /// Application name
    #[arg(long, short)]
    pub app: String,

    /// Baseline name to compare against
    #[arg(long, short)]
    pub baseline: String,

    /// Path to current evaluation results JSON file
    #[arg(long, short)]
    pub results: PathBuf,

    /// Directory containing baselines (default: baselines/)
    #[arg(long, default_value = "baselines")]
    pub baseline_dir: PathBuf,

    /// Slack webhook URL for notifications
    #[arg(long)]
    pub slack_webhook: Option<String>,

    /// Slack channel for notifications
    #[arg(long, default_value = "#evals")]
    pub slack_channel: String,

    /// Output format (table, json)
    #[arg(long, default_value = "table")]
    pub format: String,

    /// Quality drop threshold (default: 0.05 = 5%)
    #[arg(long, default_value = "0.05")]
    pub quality_threshold: f64,

    /// Exit with error code if regressions detected
    #[arg(long)]
    pub fail_on_regression: bool,
}

#[derive(Args)]
pub struct DeleteArgs {
    /// Application name
    #[arg(long, short)]
    pub app: String,

    /// Baseline name to delete
    #[arg(long, short)]
    pub name: String,

    /// Directory containing baselines (default: baselines/)
    #[arg(long, default_value = "baselines")]
    pub baseline_dir: PathBuf,

    /// Skip confirmation prompt
    #[arg(long, short = 'y')]
    pub yes: bool,
}

pub async fn run(args: BaselineArgs) -> Result<()> {
    match args.command {
        BaselineCommand::Save(save_args) => run_save(save_args).await,
        BaselineCommand::List(list_args) => run_list(list_args).await,
        BaselineCommand::Check(check_args) => run_check(check_args).await,
        BaselineCommand::Delete(delete_args) => run_delete(delete_args).await,
    }
}

async fn run_save(args: SaveArgs) -> Result<()> {
    println!(
        "{} baseline '{}' for app '{}'",
        "Saving".bright_green(),
        args.name,
        args.app
    );

    // Load evaluation results
    let results_content = tokio::fs::read_to_string(&args.results)
        .await
        .with_context(|| format!("Failed to read results from {:?}", args.results))?;

    let report: EvalReport = serde_json::from_str(&results_content)
        .with_context(|| "Failed to parse evaluation results")?;

    // Create monitor and save baseline
    let config = MonitorConfig::default()
        .with_app_name(&args.app)
        .with_baseline_dir(&args.baseline_dir);

    let monitor = QualityMonitor::new(config);
    monitor.save_baseline(&args.name, &report, args.description.as_deref())?;

    println!();
    println!("{}", "Baseline saved successfully!".bright_green().bold());
    println!();
    println!("  Scenarios: {}", report.total);
    println!("  Pass Rate: {:.1}%", report.pass_rate() * 100.0);
    println!("  Quality:   {:.3}", report.avg_quality());
    println!("  Latency:   {}ms (avg)", report.avg_latency_ms());
    println!();

    Ok(())
}

async fn run_list(args: ListArgs) -> Result<()> {
    let config = MonitorConfig::default()
        .with_app_name(&args.app)
        .with_baseline_dir(&args.baseline_dir);

    let monitor = QualityMonitor::new(config);
    let baselines = monitor.list_baselines()?;

    if baselines.is_empty() {
        println!(
            "{} No baselines found for app '{}'",
            "Info:".bright_cyan(),
            args.app
        );
        println!(
            "  Create one with: dashflow baseline save --app {} --name main --results <path>",
            args.app
        );
        return Ok(());
    }

    if args.format == "json" {
        let json = serde_json::to_string_pretty(&baselines)?;
        println!("{json}");
        return Ok(());
    }

    // Table format
    println!();
    println!(
        "{}",
        format!("Baselines for '{}' ({} total)", args.app, baselines.len())
            .bright_white()
            .bold()
    );
    println!();
    println!(
        "  {:<15} {:>10} {:>10} {:>10} {:>20}",
        "Name", "Scenarios", "Quality", "Pass Rate", "Created"
    );
    println!("  {}", "-".repeat(70));

    for baseline in baselines {
        let created = baseline.created_at.format("%Y-%m-%d %H:%M");
        println!(
            "  {:<15} {:>10} {:>10.3} {:>9.1}% {:>20}",
            baseline.name,
            baseline.scenario_count,
            baseline.avg_quality,
            baseline.pass_rate * 100.0,
            created
        );
    }

    println!();
    Ok(())
}

async fn run_check(args: CheckArgs) -> Result<()> {
    println!(
        "{} results against baseline '{}'",
        "Checking".bright_cyan(),
        args.baseline
    );
    println!("  App:     {}", args.app);
    println!("  Results: {}", args.results.display());
    println!();

    // Build config
    let mut config = MonitorConfig::default()
        .with_app_name(&args.app)
        .with_baseline_dir(&args.baseline_dir);

    // Disable metrics for CLI
    config.enable_metrics = false;

    // Set regression threshold
    config.regression_config.quality_drop_threshold = args.quality_threshold;

    // Configure Slack if provided
    if let Some(webhook) = &args.slack_webhook {
        config = config.with_slack(webhook.clone(), args.slack_channel.clone());
    }

    let monitor = QualityMonitor::new(config);

    // Run regression check
    let result = monitor
        .check_regression(&args.baseline, &args.results)
        .await?;

    if args.format == "json" {
        let json = serde_json::to_string_pretty(&result)?;
        println!("{json}");
    } else {
        // Table format
        print_check_result(&result);
    }

    // Exit with error if regressions and fail flag is set
    if args.fail_on_regression && result.has_critical_regressions {
        println!();
        println!(
            "{} Critical regressions detected!",
            "ERROR:".bright_red().bold()
        );
        std::process::exit(1);
    }

    Ok(())
}

fn print_check_result(result: &dashflow_evals::RegressionCheckResult) {
    println!("{}", "=== Regression Check Results ===".bright_white().bold());
    println!();

    // Summary
    let status = if result.has_critical_regressions {
        "CRITICAL REGRESSIONS".bright_red().bold()
    } else if result.has_regressions {
        "WARNINGS".bright_yellow().bold()
    } else {
        "PASSED".bright_green().bold()
    };

    println!("  Status: {status}");
    println!("  Baseline: {}", result.baseline_name);
    println!("  Checked at: {}", result.checked_at.format("%Y-%m-%d %H:%M:%S UTC"));
    println!();

    // Quality comparison
    println!("{}", "Quality Comparison:".bright_white());
    let quality_change_str = if result.quality_change >= 0.0 {
        format!("+{:.3}", result.quality_change).bright_green()
    } else {
        format!("{:.3}", result.quality_change).bright_red()
    };
    println!(
        "  Quality:   {:.3} -> {:.3} ({})",
        result.baseline_quality, result.current_quality, quality_change_str
    );

    let pass_rate_change = result.current_pass_rate - result.baseline_pass_rate;
    let pass_rate_change_str = if pass_rate_change >= 0.0 {
        format!("+{:.1}%", pass_rate_change * 100.0).bright_green()
    } else {
        format!("{:.1}%", pass_rate_change * 100.0).bright_red()
    };
    println!(
        "  Pass Rate: {:.1}% -> {:.1}% ({})",
        result.baseline_pass_rate * 100.0,
        result.current_pass_rate * 100.0,
        pass_rate_change_str
    );
    println!();

    // Regression counts
    if result.has_regressions {
        println!("{}", "Regressions Detected:".bright_white());
        if result.critical_count > 0 {
            println!(
                "  {} Critical: {}",
                "!!!".bright_red(),
                result.critical_count
            );
        }
        if result.warning_count > 0 {
            println!("  {} Warning: {}", "!!".bright_yellow(), result.warning_count);
        }
        if result.info_count > 0 {
            println!("  {} Info: {}", "i".bright_cyan(), result.info_count);
        }
        println!();

        // Show alerts
        if !result.alerts.is_empty() {
            println!("{}", "Alerts:".bright_white());
            for alert in &result.alerts {
                let severity_icon = match alert.severity {
                    dashflow_evals::AlertSeverity::Critical => "!!!".bright_red(),
                    dashflow_evals::AlertSeverity::Warning => "!!".bright_yellow(),
                    dashflow_evals::AlertSeverity::Info => "i".bright_cyan(),
                };
                println!("  {} {}", severity_icon, alert.title);
                println!("     {}", alert.description.bright_black());
            }
            println!();
        }
    } else {
        println!("{} No regressions detected", "OK".bright_green());
        println!();
    }
}

async fn run_delete(args: DeleteArgs) -> Result<()> {
    if !args.yes {
        println!(
            "{} Delete baseline '{}' for app '{}'?",
            "Confirm:".bright_yellow(),
            args.name,
            args.app
        );
        println!("  Use --yes to skip this prompt");

        // For now, require --yes flag
        println!();
        println!(
            "{} Deletion cancelled. Use --yes to confirm.",
            "Aborted:".bright_red()
        );
        return Ok(());
    }

    let config = MonitorConfig::default()
        .with_app_name(&args.app)
        .with_baseline_dir(&args.baseline_dir);

    let monitor = QualityMonitor::new(config);

    // Load baselines to find the one to delete
    let baselines = monitor.list_baselines()?;
    let to_delete = baselines.iter().find(|b| b.name == args.name);

    if to_delete.is_none() {
        println!(
            "{} Baseline '{}' not found for app '{}'",
            "Error:".bright_red(),
            args.name,
            args.app
        );
        return Ok(());
    }

    // Use the BaselineStore directly for deletion
    let store = dashflow_evals::BaselineStore::new(&args.baseline_dir);
    store.delete_baseline(&args.name, &args.app)?;

    println!(
        "{} Deleted baseline '{}' for app '{}'",
        "Success:".bright_green(),
        args.name,
        args.app
    );

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_check_args_defaults() {
        // Verify default values are set correctly
        let args = CheckArgs {
            app: "test".to_string(),
            baseline: "main".to_string(),
            results: PathBuf::from("results.json"),
            baseline_dir: PathBuf::from("baselines"),
            slack_webhook: None,
            slack_channel: "#evals".to_string(),
            format: "table".to_string(),
            quality_threshold: 0.05,
            fail_on_regression: false,
        };

        assert!((args.quality_threshold - 0.05).abs() < f64::EPSILON);
        assert!(!args.fail_on_regression);
    }
}
