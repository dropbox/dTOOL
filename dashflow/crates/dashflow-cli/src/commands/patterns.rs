// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)
//! Pattern detection CLI command.
//!
//! This command provides unified pattern detection from execution traces,
//! consolidating insights from execution analysis, self-improvement patterns,
//! and cross-agent learning.

use crate::output::{create_table, print_info, print_success, print_warning};
use anyhow::{Context, Result};
use clap::{Args, Subcommand, ValueEnum};
use colored::Colorize;

/// M-508: Validate that a value is in the range 0.0-1.0 (inclusive)
fn validate_unit_range(s: &str) -> std::result::Result<f64, String> {
    let value: f64 = s
        .parse()
        .map_err(|_| format!("'{s}' is not a valid number"))?;
    if !(0.0..=1.0).contains(&value) {
        return Err(format!(
            "value must be between 0.0 and 1.0 (inclusive), got {value}"
        ));
    }
    Ok(value)
}
use dashflow::{
    introspection::ExecutionTrace,
    pattern_engine::{
        PatternSource, UnifiedPattern, UnifiedPatternEngine, UnifiedPatternEngineBuilder,
        UnifiedPatternType,
    },
};
use std::fs::File;
use std::io::BufReader;
use std::path::Path;

/// Detect patterns in execution traces
#[derive(Args)]
pub struct PatternsArgs {
    #[command(subcommand)]
    pub command: PatternsCommand,
}

#[derive(Subcommand)]
pub enum PatternsCommand {
    /// Detect patterns from trace files
    Detect(DetectArgs),

    /// Generate a pattern analysis report
    Report(ReportArgs),

    /// List actionable patterns with recommendations
    Actionable(ActionableArgs),

    /// Export patterns to JSON
    Export(ExportArgs),
}

/// Detect patterns from trace files
#[derive(Args)]
pub struct DetectArgs {
    /// Path to trace file(s) - JSON or JSONL format
    #[arg(short, long, required = true)]
    input: Vec<String>,

    /// Pattern sources to enable
    #[arg(long, value_enum, default_values_t = vec![PatternSourceArg::Execution, PatternSourceArg::SelfImprovement, PatternSourceArg::CrossAgent])]
    sources: Vec<PatternSourceArg>,

    /// Minimum pattern strength (0.0-1.0)
    #[arg(long, default_value = "0.5", value_parser = validate_unit_range)]
    min_strength: f64,

    /// Minimum pattern confidence (0.0-1.0)
    #[arg(long, default_value = "0.5", value_parser = validate_unit_range)]
    min_confidence: f64,

    /// Enable deduplication
    #[arg(long, default_value = "true")]
    deduplicate: bool,

    /// Output format
    #[arg(long, value_enum, default_value = "table")]
    format: OutputFormat,

    /// Show only top N patterns
    #[arg(long)]
    top: Option<usize>,

    /// Group by source
    #[arg(long)]
    by_source: bool,

    /// Group by type
    #[arg(long)]
    by_type: bool,
}

/// Generate a pattern analysis report
#[derive(Args)]
pub struct ReportArgs {
    /// Path to trace file(s)
    #[arg(short, long, required = true)]
    input: Vec<String>,

    /// Output file for the report
    #[arg(short, long)]
    output: Option<String>,

    /// Include detailed recommendations
    #[arg(long)]
    detailed: bool,
}

/// List actionable patterns with recommendations
#[derive(Args)]
pub struct ActionableArgs {
    /// Path to trace file(s)
    #[arg(short, long, required = true)]
    input: Vec<String>,

    /// Show only patterns affecting specific nodes
    #[arg(long)]
    node: Option<String>,

    /// Filter by pattern type
    #[arg(long, value_enum)]
    pattern_type: Option<PatternTypeArg>,

    /// Output format
    #[arg(long, value_enum, default_value = "table")]
    format: OutputFormat,
}

/// Export patterns to JSON
#[derive(Args)]
pub struct ExportArgs {
    /// Path to trace file(s)
    #[arg(short, long, required = true)]
    input: Vec<String>,

    /// Output file for JSON export
    #[arg(short, long, default_value = "patterns.json")]
    output: String,

    /// Pretty print JSON
    #[arg(long)]
    pretty: bool,
}

#[derive(Clone, Copy, ValueEnum)]
pub enum PatternSourceArg {
    Execution,
    SelfImprovement,
    CrossAgent,
}

#[derive(Clone, Copy, ValueEnum)]
pub enum PatternTypeArg {
    TokenUsage,
    Performance,
    Error,
    NodeExecution,
    ToolUsage,
    ResourceUsage,
    SuccessCorrelation,
    Behavioral,
    Structural,
}

impl From<PatternTypeArg> for UnifiedPatternType {
    fn from(arg: PatternTypeArg) -> Self {
        match arg {
            PatternTypeArg::TokenUsage => UnifiedPatternType::TokenUsage,
            PatternTypeArg::Performance => UnifiedPatternType::Performance,
            PatternTypeArg::Error => UnifiedPatternType::Error,
            PatternTypeArg::NodeExecution => UnifiedPatternType::NodeExecution,
            PatternTypeArg::ToolUsage => UnifiedPatternType::ToolUsage,
            PatternTypeArg::ResourceUsage => UnifiedPatternType::ResourceUsage,
            PatternTypeArg::SuccessCorrelation => UnifiedPatternType::SuccessCorrelation,
            PatternTypeArg::Behavioral => UnifiedPatternType::Behavioral,
            PatternTypeArg::Structural => UnifiedPatternType::Structural,
        }
    }
}

#[derive(Clone, Copy, ValueEnum)]
pub enum OutputFormat {
    Table,
    Json,
    Compact,
}

pub async fn run(args: PatternsArgs) -> Result<()> {
    match args.command {
        PatternsCommand::Detect(args) => run_detect(args).await,
        PatternsCommand::Report(args) => run_report(args).await,
        PatternsCommand::Actionable(args) => run_actionable(args).await,
        PatternsCommand::Export(args) => run_export(args).await,
    }
}

async fn run_detect(args: DetectArgs) -> Result<()> {
    let input = args.input.clone();
    let traces = tokio::task::spawn_blocking(move || load_traces(&input))
        .await
        .context("Task panicked")??;
    print_info(&format!("Loaded {} execution traces", traces.len()));

    let engine = build_engine(
        &args.sources,
        args.min_strength,
        args.min_confidence,
        args.deduplicate,
    );
    let patterns = engine.detect(&traces);

    if patterns.is_empty() {
        print_warning("No patterns detected matching the criteria");
        return Ok(());
    }

    print_success(&format!("Detected {} patterns", patterns.len()));

    let patterns = if let Some(top) = args.top {
        patterns.into_iter().take(top).collect()
    } else {
        patterns
    };

    if args.by_source {
        display_patterns_by_source(&patterns, args.format);
    } else if args.by_type {
        display_patterns_by_type(&patterns, args.format);
    } else {
        display_patterns(&patterns, args.format);
    }

    Ok(())
}

async fn run_report(args: ReportArgs) -> Result<()> {
    let input = args.input.clone();
    let traces = tokio::task::spawn_blocking(move || load_traces(&input))
        .await
        .context("Task panicked")??;
    print_info(&format!("Loaded {} execution traces", traces.len()));

    let engine = UnifiedPatternEngine::default();
    let report = engine.generate_report(&traces);

    if let Some(output_path) = args.output {
        tokio::fs::write(&output_path, report.as_bytes())
            .await
            .with_context(|| format!("Failed to create output file: {}", output_path))?;
        print_success(&format!("Report written to {}", output_path));
    } else {
        println!("{}", report);
    }

    if args.detailed {
        println!();
        println!("{}", "Detailed Recommendations".bold().underline());
        println!();

        let patterns = engine.actionable_patterns(&traces);
        for pattern in patterns.iter().take(10) {
            println!("{} [{}]", pattern.description.bold(), pattern.source);
            println!(
                "  Strength: {:.0}%  Confidence: {:.0}%",
                pattern.strength * 100.0,
                pattern.confidence * 100.0
            );
            for rec in &pattern.recommendations {
                println!("  {} {}", "→".green(), rec);
            }
            if let Some(impact) = &pattern.impact {
                println!("  {} {}", "Impact:".yellow(), impact);
            }
            println!();
        }
    }

    Ok(())
}

async fn run_actionable(args: ActionableArgs) -> Result<()> {
    let input = args.input.clone();
    let traces = tokio::task::spawn_blocking(move || load_traces(&input))
        .await
        .context("Task panicked")??;
    print_info(&format!("Loaded {} execution traces", traces.len()));

    let engine = UnifiedPatternEngine::default();
    let mut patterns = engine.actionable_patterns(&traces);

    // Filter by node if specified
    if let Some(node) = &args.node {
        patterns.retain(|p| p.affected_nodes.iter().any(|n| n.contains(node)));
    }

    // Filter by type if specified
    if let Some(pattern_type) = args.pattern_type {
        let target_type: UnifiedPatternType = pattern_type.into();
        patterns.retain(|p| {
            std::mem::discriminant(&p.pattern_type) == std::mem::discriminant(&target_type)
        });
    }

    if patterns.is_empty() {
        print_warning("No actionable patterns found");
        return Ok(());
    }

    print_success(&format!("Found {} actionable patterns", patterns.len()));

    match args.format {
        OutputFormat::Table => {
            let mut table = create_table();
            table.set_header(vec!["Pattern", "Source", "Strength", "Recommendations"]);

            for p in &patterns {
                table.add_row(vec![
                    truncate(&p.description, 40),
                    p.source.to_string(),
                    format!("{:.0}%", p.strength * 100.0),
                    p.recommendations
                        .first()
                        .map(|r| truncate(r, 50))
                        .unwrap_or_default(),
                ]);
            }

            println!("{}", table);

            // Show additional recommendations if any
            for pattern in &patterns {
                if pattern.recommendations.len() > 1 {
                    println!(
                        "\n{}: additional recommendations:",
                        pattern.description.bold()
                    );
                    for (i, rec) in pattern.recommendations.iter().skip(1).enumerate() {
                        println!("  {}. {}", i + 2, rec);
                    }
                }
            }
        }
        OutputFormat::Json | OutputFormat::Compact => {
            let json = if matches!(args.format, OutputFormat::Json) {
                serde_json::to_string_pretty(&patterns)?
            } else {
                serde_json::to_string(&patterns)?
            };
            println!("{}", json);
        }
    }

    Ok(())
}

async fn run_export(args: ExportArgs) -> Result<()> {
    let input = args.input.clone();
    let traces = tokio::task::spawn_blocking(move || load_traces(&input))
        .await
        .context("Task panicked")??;
    print_info(&format!("Loaded {} execution traces", traces.len()));

    let engine = UnifiedPatternEngine::default();
    let patterns = engine.detect(&traces);

    let json = if args.pretty {
        serde_json::to_string_pretty(&patterns)?
    } else {
        serde_json::to_string(&patterns)?
    };

    tokio::fs::write(&args.output, json.as_bytes())
        .await
        .with_context(|| format!("Failed to create output file: {}", args.output))?;

    print_success(&format!(
        "Exported {} patterns to {}",
        patterns.len(),
        args.output
    ));

    Ok(())
}

// Helper functions

fn load_traces(paths: &[String]) -> Result<Vec<ExecutionTrace>> {
    let mut traces = Vec::new();

    for path in paths {
        let path = Path::new(path);
        if !path.exists() {
            return Err(anyhow::anyhow!("File not found: {}", path.display()));
        }

        // Try to determine format from extension
        let extension = path.extension().and_then(|e| e.to_str()).unwrap_or("");

        if extension == "jsonl" {
            // JSONL format: one trace per line - use BufReader for efficient line iteration
            let file = File::open(path)
                .with_context(|| format!("Failed to open file: {}", path.display()))?;
            let reader = BufReader::new(file);

            use std::io::BufRead;
            for (line_num, line) in reader.lines().enumerate() {
                let line = line.with_context(|| format!("Failed to read line {}", line_num + 1))?;
                if line.trim().is_empty() {
                    continue;
                }
                let trace: ExecutionTrace = serde_json::from_str(&line)
                    .with_context(|| format!("Failed to parse trace on line {}", line_num + 1))?;
                traces.push(trace);
            }
            // BufReader (and underlying file) dropped here
        } else {
            // JSON array or single object - read entire file at once
            let content = std::fs::read_to_string(path)
                .with_context(|| format!("Failed to read file: {}", path.display()))?;

            if let Ok(array) = serde_json::from_str::<Vec<ExecutionTrace>>(&content) {
                traces.extend(array);
            } else {
                let trace: ExecutionTrace = serde_json::from_str(&content)
                    .with_context(|| format!("Failed to parse trace from: {}", path.display()))?;
                traces.push(trace);
            }
        }
    }

    Ok(traces)
}

fn build_engine(
    sources: &[PatternSourceArg],
    min_strength: f64,
    min_confidence: f64,
    deduplicate: bool,
) -> UnifiedPatternEngine {
    let mut builder = UnifiedPatternEngineBuilder::new()
        .min_strength(min_strength)
        .min_confidence(min_confidence)
        .deduplicate(deduplicate);

    for source in sources {
        builder = match source {
            PatternSourceArg::Execution => builder.enable_execution_patterns(),
            PatternSourceArg::SelfImprovement => builder.enable_self_improvement_patterns(),
            PatternSourceArg::CrossAgent => builder.enable_cross_agent_patterns(),
        };
    }

    builder.build()
}

fn display_patterns(patterns: &[UnifiedPattern], format: OutputFormat) {
    match format {
        OutputFormat::Table => {
            let mut table = create_table();
            table.set_header(vec![
                "ID",
                "Type",
                "Source",
                "Strength",
                "Confidence",
                "Description",
            ]);

            for p in patterns {
                table.add_row(vec![
                    truncate(&p.id, 20),
                    p.pattern_type.to_string(),
                    p.source.to_string(),
                    format!("{:.0}%", p.strength * 100.0),
                    format!("{:.0}%", p.confidence * 100.0),
                    truncate(&p.description, 50),
                ]);
            }

            println!("{}", table);
        }
        OutputFormat::Json => {
            let json = serde_json::to_string_pretty(patterns).unwrap_or_default();
            println!("{}", json);
        }
        OutputFormat::Compact => {
            let json = serde_json::to_string(patterns).unwrap_or_default();
            println!("{}", json);
        }
    }
}

fn display_patterns_by_source(patterns: &[UnifiedPattern], format: OutputFormat) {
    let mut grouped: std::collections::HashMap<PatternSource, Vec<&UnifiedPattern>> =
        std::collections::HashMap::new();

    for pattern in patterns {
        grouped.entry(pattern.source).or_default().push(pattern);
    }

    for (source, source_patterns) in &grouped {
        println!(
            "\n{} ({} patterns)",
            source.to_string().bold().underline(),
            source_patterns.len()
        );

        match format {
            OutputFormat::Table => {
                let mut table = create_table();
                table.set_header(vec!["Type", "Strength", "Confidence", "Description"]);

                for p in source_patterns {
                    table.add_row(vec![
                        p.pattern_type.to_string(),
                        format!("{:.0}%", p.strength * 100.0),
                        format!("{:.0}%", p.confidence * 100.0),
                        truncate(&p.description, 50),
                    ]);
                }

                println!("{}", table);
            }
            OutputFormat::Json | OutputFormat::Compact => {
                let json = if matches!(format, OutputFormat::Json) {
                    serde_json::to_string_pretty(source_patterns).unwrap_or_default()
                } else {
                    serde_json::to_string(source_patterns).unwrap_or_default()
                };
                println!("{}", json);
            }
        }
    }
}

fn display_patterns_by_type(patterns: &[UnifiedPattern], format: OutputFormat) {
    let mut grouped: std::collections::HashMap<String, Vec<&UnifiedPattern>> =
        std::collections::HashMap::new();

    for pattern in patterns {
        grouped
            .entry(pattern.pattern_type.to_string())
            .or_default()
            .push(pattern);
    }

    for (pattern_type, type_patterns) in &grouped {
        println!(
            "\n{} ({} patterns)",
            pattern_type.bold().underline(),
            type_patterns.len()
        );

        match format {
            OutputFormat::Table => {
                let mut table = create_table();
                table.set_header(vec!["Source", "Strength", "Confidence", "Description"]);

                for p in type_patterns {
                    table.add_row(vec![
                        p.source.to_string(),
                        format!("{:.0}%", p.strength * 100.0),
                        format!("{:.0}%", p.confidence * 100.0),
                        truncate(&p.description, 50),
                    ]);
                }

                println!("{}", table);
            }
            OutputFormat::Json | OutputFormat::Compact => {
                let json = if matches!(format, OutputFormat::Json) {
                    serde_json::to_string_pretty(type_patterns).unwrap_or_default()
                } else {
                    serde_json::to_string(type_patterns).unwrap_or_default()
                };
                println!("{}", json);
            }
        }
    }
}

fn truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        // M-497: Use char_indices to find safe UTF-8 boundary
        // Avoid panics from slicing in the middle of multi-byte characters
        let target_len = max_len.saturating_sub(3);
        let truncate_at = s
            .char_indices()
            .take_while(|(i, _)| *i < target_len)
            .last()
            .map(|(i, c)| i + c.len_utf8())
            .unwrap_or(0);
        format!("{}...", &s[..truncate_at])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dashflow::introspection::{ExecutionTraceBuilder, NodeExecution};
    use std::io::Write;
    use tempfile::NamedTempFile;

    fn create_test_trace() -> ExecutionTrace {
        ExecutionTraceBuilder::new()
            .thread_id("test-thread")
            .add_node_execution(NodeExecution::new("node1", 100).with_tokens(500))
            .add_node_execution(NodeExecution::new("node2", 200).with_tokens(1000))
            .total_duration_ms(300)
            .total_tokens(1500)
            .completed(true)
            .build()
    }

    #[test]
    fn test_load_traces_json() {
        let trace = create_test_trace();
        let json = serde_json::to_string(&trace).unwrap();

        let mut file = NamedTempFile::with_suffix(".json").unwrap();
        file.write_all(json.as_bytes()).unwrap();

        let traces = load_traces(&[file.path().to_string_lossy().to_string()]).unwrap();
        assert_eq!(traces.len(), 1);
        assert_eq!(traces[0].total_tokens, 1500);
    }

    #[test]
    fn test_load_traces_jsonl() {
        let trace1 = create_test_trace();
        let trace2 = create_test_trace();
        let jsonl = format!(
            "{}\n{}",
            serde_json::to_string(&trace1).unwrap(),
            serde_json::to_string(&trace2).unwrap()
        );

        let mut file = NamedTempFile::with_suffix(".jsonl").unwrap();
        file.write_all(jsonl.as_bytes()).unwrap();

        let traces = load_traces(&[file.path().to_string_lossy().to_string()]).unwrap();
        assert_eq!(traces.len(), 2);
    }

    #[test]
    fn test_load_traces_json_array() {
        let traces_in = vec![create_test_trace(), create_test_trace()];
        let json = serde_json::to_string(&traces_in).unwrap();

        let mut file = NamedTempFile::with_suffix(".json").unwrap();
        file.write_all(json.as_bytes()).unwrap();

        let traces = load_traces(&[file.path().to_string_lossy().to_string()]).unwrap();
        assert_eq!(traces.len(), 2);
    }

    #[test]
    fn test_build_engine_all_sources() {
        let engine = build_engine(
            &[
                PatternSourceArg::Execution,
                PatternSourceArg::SelfImprovement,
                PatternSourceArg::CrossAgent,
            ],
            0.5,
            0.5,
            true,
        );

        // Engine should be created without panic
        let patterns = engine.detect(&[]);
        assert!(patterns.is_empty()); // No traces = no patterns
    }

    #[test]
    fn test_build_engine_single_source() {
        let engine = build_engine(&[PatternSourceArg::Execution], 0.3, 0.3, false);
        let patterns = engine.detect(&[]);
        assert!(patterns.is_empty());
    }

    #[test]
    fn test_truncate() {
        assert_eq!(truncate("hello", 10), "hello");
        assert_eq!(truncate("hello world", 8), "hello...");
        assert_eq!(truncate("hi", 2), "hi");
    }

    #[test]
    fn test_pattern_type_conversion() {
        let token: UnifiedPatternType = PatternTypeArg::TokenUsage.into();
        assert!(matches!(token, UnifiedPatternType::TokenUsage));

        let perf: UnifiedPatternType = PatternTypeArg::Performance.into();
        assert!(matches!(perf, UnifiedPatternType::Performance));
    }

    #[test]
    fn test_validate_unit_range_valid() {
        // M-508: Test valid values
        assert!((validate_unit_range("0.0").unwrap() - 0.0).abs() < f64::EPSILON);
        assert!((validate_unit_range("0.5").unwrap() - 0.5).abs() < f64::EPSILON);
        assert!((validate_unit_range("1.0").unwrap() - 1.0).abs() < f64::EPSILON);
        assert!((validate_unit_range("0").unwrap() - 0.0).abs() < f64::EPSILON);
        assert!((validate_unit_range("1").unwrap() - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_validate_unit_range_invalid() {
        // M-508: Test invalid values
        assert!(validate_unit_range("-0.1").is_err());
        assert!(validate_unit_range("1.1").is_err());
        assert!(validate_unit_range("2.0").is_err());
        assert!(validate_unit_range("-1").is_err());
        assert!(validate_unit_range("abc").is_err());
    }
}
