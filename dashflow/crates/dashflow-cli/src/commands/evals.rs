//! evals - Manage evaluation test cases and golden datasets
//!
//! This command provides utilities for the continuous learning workflow,
//! including listing pending tests, viewing test details, and promoting
//! generated test cases to golden scenarios (M-2464, M-2465).
//!
//! ## Subcommands
//!
//! - `list` - List pending tests awaiting review
//! - `show` - Show details of a specific pending test
//! - `promote` - Promote a pending test to a golden scenario
//!
//! ## Examples
//!
//! ```bash
//! # List all pending tests
//! dashflow evals list --pending-dir .dashflow/pending_tests
//!
//! # Show details of a specific test (JSON output)
//! dashflow evals show test_001_failure_variant --format json
//!
//! # Promote a test to golden (interactive mode)
//! dashflow evals promote test_001_failure_variant --golden-dir data/golden_dataset
//!
//! # Preview promotion without saving (dry-run)
//! dashflow evals promote test_001 --golden-dir data/golden --dry-run
//!
//! # Promote with JSON output
//! dashflow evals promote test_001 --golden-dir data/golden --format json
//!
//! # Promote with all options specified
//! dashflow evals promote test_001 \
//!     --golden-dir data/golden_dataset \
//!     --description "Validates async runtime usage" \
//!     --expected-contains tokio,async \
//!     --quality-threshold 0.90 \
//!     --difficulty medium
//! ```

use anyhow::{Context, Result};
use clap::{Args, Subcommand, ValueEnum};
use colored::Colorize;
use dashflow_evals::{
    ContinuousLearning, Difficulty, GoldenPromotionInput, GoldenScenario, LearningConfig,
};
use std::io::{self, Write};
use std::path::PathBuf;

#[derive(Args)]
pub struct EvalsArgs {
    #[command(subcommand)]
    pub command: EvalsCommand,
}

#[derive(Subcommand)]
pub enum EvalsCommand {
    /// List pending tests awaiting review
    List(ListArgs),

    /// Show details of a specific pending test
    Show(ShowArgs),

    /// Promote a pending test to a golden scenario
    Promote(PromoteArgs),
}

#[derive(Args)]
pub struct ListArgs {
    /// Directory containing pending tests
    #[arg(long, default_value = ".dashflow/pending_tests")]
    pub pending_dir: PathBuf,

    /// Output format
    #[arg(long, value_enum, default_value_t = OutputFormat::Table)]
    pub format: OutputFormat,

    /// Show only tests that need review
    #[arg(long)]
    pub needs_review: bool,
}

#[derive(Args)]
pub struct ShowArgs {
    /// Test ID to show
    pub test_id: String,

    /// Directory containing pending tests
    #[arg(long, default_value = ".dashflow/pending_tests")]
    pub pending_dir: PathBuf,

    /// Output format
    #[arg(long, value_enum, default_value_t = OutputFormat::Table)]
    pub format: OutputFormat,
}

#[derive(Args)]
pub struct PromoteArgs {
    /// Test ID to promote
    pub test_id: String,

    /// Directory containing pending tests
    #[arg(long, default_value = ".dashflow/pending_tests")]
    pub pending_dir: PathBuf,

    /// Directory for golden scenarios (required)
    #[arg(long)]
    pub golden_dir: PathBuf,

    /// Output format
    #[arg(long, value_enum, default_value_t = OutputFormat::Table)]
    pub format: OutputFormat,

    /// Preview the golden scenario without saving
    #[arg(long)]
    pub dry_run: bool,

    /// Human-readable description of what this scenario tests
    #[arg(long, short)]
    pub description: Option<String>,

    /// Strings that MUST appear in the output (comma-separated)
    #[arg(long, value_delimiter = ',')]
    pub expected_contains: Option<Vec<String>>,

    /// Strings that MUST NOT appear in the output (comma-separated)
    #[arg(long, value_delimiter = ',')]
    pub expected_not_contains: Option<Vec<String>>,

    /// Minimum quality score threshold (0.0-1.0)
    #[arg(long, default_value = "0.85")]
    pub quality_threshold: f64,

    /// Context for the scenario (e.g., "First turn", "Follow-up")
    #[arg(long)]
    pub context: Option<String>,

    /// Difficulty level
    #[arg(long, value_enum)]
    pub difficulty: Option<DifficultyArg>,

    /// Maximum allowed latency in milliseconds
    #[arg(long)]
    pub max_latency_ms: Option<u64>,

    /// Expected tool calls (comma-separated)
    #[arg(long, value_delimiter = ',')]
    pub expected_tool_calls: Option<Vec<String>>,

    /// Skip interactive prompts and use defaults for unspecified fields
    #[arg(long, short = 'y')]
    pub yes: bool,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum OutputFormat {
    /// Human-readable table
    Table,
    /// JSON output
    Json,
}

/// CLI-friendly difficulty enum that maps to dashflow_evals::Difficulty
#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum DifficultyArg {
    Simple,
    Medium,
    Complex,
    Adversarial,
}

impl From<DifficultyArg> for Difficulty {
    fn from(d: DifficultyArg) -> Self {
        match d {
            DifficultyArg::Simple => Difficulty::Simple,
            DifficultyArg::Medium => Difficulty::Medium,
            DifficultyArg::Complex => Difficulty::Complex,
            DifficultyArg::Adversarial => Difficulty::Adversarial,
        }
    }
}

pub async fn run(args: EvalsArgs) -> Result<()> {
    match args.command {
        EvalsCommand::List(list_args) => run_list(list_args).await,
        EvalsCommand::Show(show_args) => run_show(show_args).await,
        EvalsCommand::Promote(promote_args) => run_promote(promote_args).await,
    }
}

async fn run_list(args: ListArgs) -> Result<()> {
    // Validate directory exists
    if !args.pending_dir.exists() {
        println!(
            "{} No pending tests directory found at: {}",
            "Info:".bright_cyan(),
            args.pending_dir.display()
        );
        println!(
            "  {}",
            "Run evaluation with continuous learning enabled to generate pending tests."
                .bright_black()
        );
        return Ok(());
    }

    // Create a minimal config to use ContinuousLearning::load_pending_tests
    let config = LearningConfig {
        pending_tests_dir: args.pending_dir.clone(),
        ..Default::default()
    };
    let learning = ContinuousLearning::new(config);

    let tests = learning.load_pending_tests()?;

    if tests.is_empty() {
        println!(
            "{} No pending tests found in: {}",
            "Info:".bright_cyan(),
            args.pending_dir.display()
        );
        return Ok(());
    }

    // Filter if needed
    let filtered_tests: Vec<_> = if args.needs_review {
        tests.into_iter().filter(|t| t.needs_review).collect()
    } else {
        tests
    };

    match args.format {
        OutputFormat::Table => {
            println!();
            println!(
                "{} ({} tests)",
                "=== Pending Tests ===".bright_white().bold(),
                filtered_tests.len()
            );
            println!();
            println!(
                "  {:<35} {:<15} {:<10} {}",
                "ID".bright_white(),
                "Source".bright_white(),
                "Confidence".bright_white(),
                "Review".bright_white()
            );
            println!("  {}", "-".repeat(75));

            for test in &filtered_tests {
                let source = match &test.generation_source {
                    dashflow_evals::GenerationSource::Failure { .. } => "Failure",
                    dashflow_evals::GenerationSource::Uncertainty { .. } => "Uncertainty",
                    dashflow_evals::GenerationSource::HumanFeedback { .. } => "Feedback",
                    dashflow_evals::GenerationSource::Synthesis { .. } => "Synthesis",
                };
                let confidence = format!("{:.0}%", test.confidence * 100.0);
                let review = if test.needs_review {
                    "Yes".bright_yellow()
                } else {
                    "No".bright_green()
                };

                let id_display = if test.scenario_id.len() > 33 {
                    format!("{}...", &test.scenario_id[..30])
                } else {
                    test.scenario_id.clone()
                };

                println!(
                    "  {:<35} {:<15} {:<10} {}",
                    id_display, source, confidence, review
                );
            }
            println!();
            println!(
                "{} Use `dashflow evals show <test-id>` to view details",
                "Tip:".bright_cyan()
            );
            println!(
                "     Use `dashflow evals promote <test-id> --golden-dir <dir>` to promote to golden",
            );
        }
        OutputFormat::Json => {
            let json = serde_json::to_string_pretty(&filtered_tests)?;
            println!("{}", json);
        }
    }

    Ok(())
}

async fn run_show(args: ShowArgs) -> Result<()> {
    // Validate directory exists
    if !args.pending_dir.exists() {
        anyhow::bail!(
            "Pending tests directory not found: {}",
            args.pending_dir.display()
        );
    }

    // Load the specific test
    let test_path = args.pending_dir.join(format!("{}.json", args.test_id));
    if !test_path.exists() {
        anyhow::bail!(
            "Pending test not found: {} (looked in {})",
            args.test_id,
            test_path.display()
        );
    }

    let content = tokio::fs::read_to_string(&test_path)
        .await
        .with_context(|| format!("Failed to read test file: {}", test_path.display()))?;

    let test: dashflow_evals::GeneratedTestCase = serde_json::from_str(&content)
        .with_context(|| format!("Failed to parse test file: {}", test_path.display()))?;

    match args.format {
        OutputFormat::Table => {
            println!();
            println!(
                "{}",
                format!("=== Test: {} ===", test.scenario_id)
                    .bright_white()
                    .bold()
            );
            println!();
            println!("  {}: {}", "ID".bright_cyan(), test.scenario_id);
            println!("  {}: {}", "Query".bright_cyan(), test.query);
            println!(
                "  {}: {:.0}%",
                "Confidence".bright_cyan(),
                test.confidence * 100.0
            );
            println!(
                "  {}: {}",
                "Needs Review".bright_cyan(),
                if test.needs_review {
                    "Yes".bright_yellow()
                } else {
                    "No".bright_green()
                }
            );
            println!();

            let source_info = match &test.generation_source {
                dashflow_evals::GenerationSource::Failure {
                    original_scenario_id,
                } => format!("Failure (from {})", original_scenario_id),
                dashflow_evals::GenerationSource::Uncertainty {
                    original_scenario_id,
                } => format!("Uncertainty (from {})", original_scenario_id),
                dashflow_evals::GenerationSource::HumanFeedback { feedback_id } => {
                    format!("Human Feedback ({})", feedback_id)
                }
                dashflow_evals::GenerationSource::Synthesis {
                    source_scenario_ids,
                } => format!("Synthesis (from {} scenarios)", source_scenario_ids.len()),
            };
            println!("  {}: {}", "Source".bright_cyan(), source_info);
            println!(
                "  {}: {}",
                "Generation Reason".bright_cyan(),
                test.generation_reason
            );
            println!();

            if let Some(ref observed) = test.observed_output {
                println!("  {}:", "Observed Output".bright_cyan());
                // Truncate long output for display
                let display_output = if observed.len() > 500 {
                    format!("{}...", &observed[..500])
                } else {
                    observed.clone()
                };
                for line in display_output.lines() {
                    println!("    {}", line.bright_black());
                }
            }
            println!();
            println!(
                "{} To promote this test to golden, run:",
                "Tip:".bright_cyan()
            );
            println!(
                "     dashflow evals promote {} --golden-dir <path>",
                test.scenario_id
            );
        }
        OutputFormat::Json => {
            let json = serde_json::to_string_pretty(&test)?;
            println!("{}", json);
        }
    }

    Ok(())
}

async fn run_promote(args: PromoteArgs) -> Result<()> {
    // Validate directories
    if !args.pending_dir.exists() {
        anyhow::bail!(
            "Pending tests directory not found: {}",
            args.pending_dir.display()
        );
    }

    // Verify the test exists before prompting
    let test_path = args.pending_dir.join(format!("{}.json", args.test_id));
    if !test_path.exists() {
        anyhow::bail!(
            "Pending test not found: {} (looked in {})",
            args.test_id,
            test_path.display()
        );
    }

    // Load test to show context
    let content = tokio::fs::read_to_string(&test_path)
        .await
        .with_context(|| format!("Failed to read test file: {}", test_path.display()))?;
    let test: dashflow_evals::GeneratedTestCase = serde_json::from_str(&content)?;

    // Build the promotion input
    let description = if let Some(ref desc) = args.description {
        desc.clone()
    } else if args.yes {
        anyhow::bail!("Description is required. Use --description or remove --yes for interactive mode.");
    } else {
        // Interactive prompt
        println!();
        println!(
            "{}",
            format!("=== Promoting: {} ===", test.scenario_id)
                .bright_white()
                .bold()
        );
        println!();
        println!("  {}: {}", "Query".bright_cyan(), test.query);
        if let Some(ref obs) = test.observed_output {
            let truncated = if obs.len() > 200 {
                format!("{}...", &obs[..200])
            } else {
                obs.clone()
            };
            println!("  {}: {}", "Observed".bright_cyan(), truncated);
        }
        println!();

        print!("{}", "Enter description (required): ".bright_yellow());
        io::stdout().flush()?;
        let mut desc = String::new();
        io::stdin().read_line(&mut desc)?;
        desc.trim().to_string()
    };

    if description.trim().is_empty() {
        anyhow::bail!("Description cannot be empty");
    }

    let expected_contains = args.expected_contains.clone().unwrap_or_default();
    let expected_not_contains = args.expected_not_contains.clone().unwrap_or_default();
    let expected_tool_calls = args.expected_tool_calls.clone().unwrap_or_default();

    let input = GoldenPromotionInput {
        description: description.clone(),
        expected_output_contains: expected_contains,
        expected_output_not_contains: expected_not_contains,
        quality_threshold: args.quality_threshold,
        context: args.context.clone(),
        difficulty: args.difficulty.map(Into::into),
        max_latency_ms: args.max_latency_ms,
        expected_tool_calls,
    };

    // Preview mode: show what would be created without saving
    if args.dry_run {
        let preview = test.to_golden_scenario(input);
        return output_golden_preview(&preview, &args);
    }

    // Create the ContinuousLearning instance
    let config = LearningConfig {
        pending_tests_dir: args.pending_dir.clone(),
        ..Default::default()
    };
    let learning = ContinuousLearning::new(config);

    // Perform the promotion
    if matches!(args.format, OutputFormat::Table) {
        println!();
        println!(
            "{} Promoting test '{}' to golden scenario...",
            "Info:".bright_cyan(),
            args.test_id
        );
    }

    let golden = learning
        .promote_to_golden(&args.test_id, input, &args.golden_dir)
        .with_context(|| format!("Failed to promote test: {}", args.test_id))?;

    match args.format {
        OutputFormat::Table => {
            println!();
            println!(
                "{} Successfully promoted to golden scenario!",
                "✓".bright_green()
            );
            println!();
            println!("  {}: {}", "Golden ID".bright_cyan(), golden.id);
            println!("  {}: {}", "Description".bright_cyan(), golden.description);
            println!(
                "  {}: {}",
                "Quality Threshold".bright_cyan(),
                format!("{:.0}%", golden.quality_threshold * 100.0)
            );
            println!(
                "  {}: {}",
                "Saved to".bright_cyan(),
                args.golden_dir.join(format!("{}.json", golden.id)).display()
            );
            println!();
            println!(
                "{} The pending test has been moved to the approved directory.",
                "Note:".bright_cyan()
            );
        }
        OutputFormat::Json => {
            let output = serde_json::json!({
                "status": "promoted",
                "golden": golden,
                "saved_to": args.golden_dir.join(format!("{}.json", golden.id))
            });
            println!("{}", serde_json::to_string_pretty(&output)?);
        }
    }

    Ok(())
}

/// Output golden scenario preview for dry-run mode
fn output_golden_preview(golden: &GoldenScenario, args: &PromoteArgs) -> Result<()> {
    match args.format {
        OutputFormat::Table => {
            println!();
            println!(
                "{}",
                "=== Dry Run: Golden Scenario Preview ===".bright_yellow().bold()
            );
            println!();
            println!("  {}: {}", "ID".bright_cyan(), golden.id);
            println!("  {}: {}", "Description".bright_cyan(), golden.description);
            println!("  {}: {}", "Query".bright_cyan(), golden.query);
            println!(
                "  {}: {}",
                "Quality Threshold".bright_cyan(),
                format!("{:.0}%", golden.quality_threshold * 100.0)
            );
            if let Some(ref ctx) = golden.context {
                println!("  {}: {}", "Context".bright_cyan(), ctx);
            }
            if let Some(ref diff) = golden.difficulty {
                println!("  {}: {:?}", "Difficulty".bright_cyan(), diff);
            }
            if !golden.expected_output_contains.is_empty() {
                println!(
                    "  {}: {:?}",
                    "Expected Contains".bright_cyan(),
                    golden.expected_output_contains
                );
            }
            if !golden.expected_output_not_contains.is_empty() {
                println!(
                    "  {}: {:?}",
                    "Expected Not Contains".bright_cyan(),
                    golden.expected_output_not_contains
                );
            }
            if let Some(latency) = golden.max_latency_ms {
                println!("  {}: {}ms", "Max Latency".bright_cyan(), latency);
            }
            if !golden.expected_tool_calls.is_empty() {
                println!(
                    "  {}: {:?}",
                    "Expected Tool Calls".bright_cyan(),
                    golden.expected_tool_calls
                );
            }
            println!();
            println!(
                "  {}: {}",
                "Would save to".bright_cyan(),
                args.golden_dir.join(format!("{}.json", golden.id)).display()
            );
            println!();
            println!(
                "{} No changes made. Remove --dry-run to promote.",
                "Note:".bright_yellow()
            );
        }
        OutputFormat::Json => {
            let output = serde_json::json!({
                "status": "dry_run",
                "golden": golden,
                "would_save_to": args.golden_dir.join(format!("{}.json", golden.id))
            });
            println!("{}", serde_json::to_string_pretty(&output)?);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write as IoWrite;
    use tempfile::TempDir;

    #[test]
    fn test_difficulty_conversion() {
        assert!(matches!(
            Difficulty::from(DifficultyArg::Simple),
            Difficulty::Simple
        ));
        assert!(matches!(
            Difficulty::from(DifficultyArg::Medium),
            Difficulty::Medium
        ));
        assert!(matches!(
            Difficulty::from(DifficultyArg::Complex),
            Difficulty::Complex
        ));
        assert!(matches!(
            Difficulty::from(DifficultyArg::Adversarial),
            Difficulty::Adversarial
        ));
    }

    #[tokio::test]
    async fn test_list_empty_directory() {
        let temp = TempDir::new().unwrap();
        let pending_dir = temp.path().join("pending_tests");
        std::fs::create_dir_all(&pending_dir).unwrap();

        let args = ListArgs {
            pending_dir,
            format: OutputFormat::Table,
            needs_review: false,
        };

        // Should succeed with empty output
        let result = run_list(args).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_list_with_tests() {
        let temp = TempDir::new().unwrap();
        let pending_dir = temp.path().join("pending_tests");
        std::fs::create_dir_all(&pending_dir).unwrap();

        // Create a test file
        let test_case = serde_json::json!({
            "scenario_id": "test_001",
            "query": "What is Rust?",
            "expected_output": null,
            "observed_output": "Rust is a programming language",
            "generation_source": {"Failure": {"original_scenario_id": "orig"}},
            "confidence": 0.8,
            "generation_reason": "Test failure",
            "needs_review": true
        });
        let mut f = std::fs::File::create(pending_dir.join("test_001.json")).unwrap();
        writeln!(f, "{}", serde_json::to_string_pretty(&test_case).unwrap()).unwrap();

        let args = ListArgs {
            pending_dir,
            format: OutputFormat::Table,
            needs_review: false,
        };

        let result = run_list(args).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_show_nonexistent() {
        let temp = TempDir::new().unwrap();
        let pending_dir = temp.path().join("pending_tests");
        std::fs::create_dir_all(&pending_dir).unwrap();

        let args = ShowArgs {
            test_id: "nonexistent".to_string(),
            pending_dir,
            format: OutputFormat::Table,
        };

        let result = run_show(args).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not found"));
    }

    #[tokio::test]
    async fn test_promote_requires_description() {
        let temp = TempDir::new().unwrap();
        let pending_dir = temp.path().join("pending_tests");
        let golden_dir = temp.path().join("golden");
        std::fs::create_dir_all(&pending_dir).unwrap();

        // Create a test file
        let test_case = serde_json::json!({
            "scenario_id": "test_001",
            "query": "What is Rust?",
            "expected_output": null,
            "observed_output": "Rust is a programming language",
            "generation_source": {"Failure": {"original_scenario_id": "orig"}},
            "confidence": 0.8,
            "generation_reason": "Test failure",
            "needs_review": true
        });
        let mut f = std::fs::File::create(pending_dir.join("test_001.json")).unwrap();
        writeln!(f, "{}", serde_json::to_string_pretty(&test_case).unwrap()).unwrap();

        let args = PromoteArgs {
            test_id: "test_001".to_string(),
            pending_dir,
            golden_dir,
            format: OutputFormat::Table,
            dry_run: false,
            description: None, // No description
            expected_contains: None,
            expected_not_contains: None,
            quality_threshold: 0.85,
            context: None,
            difficulty: None,
            max_latency_ms: None,
            expected_tool_calls: None,
            yes: true, // Skip interactive, should fail
        };

        let result = run_promote(args).await;
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Description is required"));
    }

    #[tokio::test]
    async fn test_promote_success() {
        let temp = TempDir::new().unwrap();
        let pending_dir = temp.path().join("pending_tests");
        let golden_dir = temp.path().join("golden");
        std::fs::create_dir_all(&pending_dir).unwrap();

        // Create a test file
        let test_case = serde_json::json!({
            "scenario_id": "test_002",
            "query": "What is Rust?",
            "expected_output": null,
            "observed_output": "Rust is a programming language",
            "generation_source": {"Failure": {"original_scenario_id": "orig"}},
            "confidence": 0.8,
            "generation_reason": "Test failure",
            "needs_review": true
        });
        let mut f = std::fs::File::create(pending_dir.join("test_002.json")).unwrap();
        writeln!(f, "{}", serde_json::to_string_pretty(&test_case).unwrap()).unwrap();

        let args = PromoteArgs {
            test_id: "test_002".to_string(),
            pending_dir: pending_dir.clone(),
            golden_dir: golden_dir.clone(),
            format: OutputFormat::Table,
            dry_run: false,
            description: Some("Tests Rust knowledge".to_string()),
            expected_contains: Some(vec!["programming".to_string()]),
            expected_not_contains: None,
            quality_threshold: 0.90,
            context: Some("First turn".to_string()),
            difficulty: Some(DifficultyArg::Simple),
            max_latency_ms: Some(5000),
            expected_tool_calls: None,
            yes: true,
        };

        let result = run_promote(args).await;
        assert!(result.is_ok());

        // Verify golden file was created
        assert!(golden_dir.join("test_002.json").exists());

        // Verify pending file was moved
        assert!(!pending_dir.join("test_002.json").exists());
    }

    #[tokio::test]
    async fn test_promote_dry_run_does_not_save() {
        let temp = TempDir::new().unwrap();
        let pending_dir = temp.path().join("pending_tests");
        let golden_dir = temp.path().join("golden");
        std::fs::create_dir_all(&pending_dir).unwrap();

        // Create a test file
        let test_case = serde_json::json!({
            "scenario_id": "test_003",
            "query": "What is Rust?",
            "expected_output": null,
            "observed_output": "Rust is a programming language",
            "generation_source": {"Failure": {"original_scenario_id": "orig"}},
            "confidence": 0.8,
            "generation_reason": "Test failure",
            "needs_review": true
        });
        let mut f = std::fs::File::create(pending_dir.join("test_003.json")).unwrap();
        writeln!(f, "{}", serde_json::to_string_pretty(&test_case).unwrap()).unwrap();

        let args = PromoteArgs {
            test_id: "test_003".to_string(),
            pending_dir: pending_dir.clone(),
            golden_dir: golden_dir.clone(),
            format: OutputFormat::Table,
            dry_run: true, // Dry run mode
            description: Some("Tests Rust knowledge".to_string()),
            expected_contains: Some(vec!["programming".to_string()]),
            expected_not_contains: None,
            quality_threshold: 0.90,
            context: Some("First turn".to_string()),
            difficulty: Some(DifficultyArg::Simple),
            max_latency_ms: Some(5000),
            expected_tool_calls: None,
            yes: true,
        };

        let result = run_promote(args).await;
        assert!(result.is_ok());

        // Verify golden file was NOT created (dry run)
        assert!(!golden_dir.join("test_003.json").exists());

        // Verify pending file still exists (not moved)
        assert!(pending_dir.join("test_003.json").exists());
    }

    #[tokio::test]
    async fn test_promote_json_output() {
        let temp = TempDir::new().unwrap();
        let pending_dir = temp.path().join("pending_tests");
        let golden_dir = temp.path().join("golden");
        std::fs::create_dir_all(&pending_dir).unwrap();

        // Create a test file
        let test_case = serde_json::json!({
            "scenario_id": "test_004",
            "query": "What is Rust?",
            "expected_output": null,
            "observed_output": "Rust is a programming language",
            "generation_source": {"Failure": {"original_scenario_id": "orig"}},
            "confidence": 0.8,
            "generation_reason": "Test failure",
            "needs_review": true
        });
        let mut f = std::fs::File::create(pending_dir.join("test_004.json")).unwrap();
        writeln!(f, "{}", serde_json::to_string_pretty(&test_case).unwrap()).unwrap();

        let args = PromoteArgs {
            test_id: "test_004".to_string(),
            pending_dir: pending_dir.clone(),
            golden_dir: golden_dir.clone(),
            format: OutputFormat::Json, // JSON output
            dry_run: false,
            description: Some("Tests Rust knowledge".to_string()),
            expected_contains: Some(vec!["programming".to_string()]),
            expected_not_contains: None,
            quality_threshold: 0.90,
            context: Some("First turn".to_string()),
            difficulty: Some(DifficultyArg::Simple),
            max_latency_ms: Some(5000),
            expected_tool_calls: None,
            yes: true,
        };

        let result = run_promote(args).await;
        assert!(result.is_ok());

        // Verify golden file was created
        assert!(golden_dir.join("test_004.json").exists());
    }

    #[tokio::test]
    async fn test_promote_dry_run_json_output() {
        let temp = TempDir::new().unwrap();
        let pending_dir = temp.path().join("pending_tests");
        let golden_dir = temp.path().join("golden");
        std::fs::create_dir_all(&pending_dir).unwrap();

        // Create a test file
        let test_case = serde_json::json!({
            "scenario_id": "test_005",
            "query": "What is Rust?",
            "expected_output": null,
            "observed_output": "Rust is a programming language",
            "generation_source": {"Failure": {"original_scenario_id": "orig"}},
            "confidence": 0.8,
            "generation_reason": "Test failure",
            "needs_review": true
        });
        let mut f = std::fs::File::create(pending_dir.join("test_005.json")).unwrap();
        writeln!(f, "{}", serde_json::to_string_pretty(&test_case).unwrap()).unwrap();

        let args = PromoteArgs {
            test_id: "test_005".to_string(),
            pending_dir: pending_dir.clone(),
            golden_dir: golden_dir.clone(),
            format: OutputFormat::Json, // JSON output
            dry_run: true, // Dry run mode
            description: Some("Tests Rust knowledge".to_string()),
            expected_contains: Some(vec!["programming".to_string()]),
            expected_not_contains: None,
            quality_threshold: 0.90,
            context: Some("First turn".to_string()),
            difficulty: Some(DifficultyArg::Simple),
            max_latency_ms: Some(5000),
            expected_tool_calls: None,
            yes: true,
        };

        let result = run_promote(args).await;
        assert!(result.is_ok());

        // Verify golden file was NOT created (dry run)
        assert!(!golden_dir.join("test_005.json").exists());

        // Verify pending file still exists (not moved)
        assert!(pending_dir.join("test_005.json").exists());
    }
}
