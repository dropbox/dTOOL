// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)
//! dataset - Dataset utilities (generate, validate, inspect)
//!
//! # Output Formats
//!
//! The `stats` subcommand supports `--output-format` for output format selection:
//! - `--output-format table` (default): Human-readable colored table output
//! - `--output-format json`: Machine-readable JSON output for automation
//!
//! Note: The `--format` flag in dataset commands specifies the *input* dataset format
//! (jsonl, csv, parquet), not the output format.
//!
//! # Examples
//!
//! ```bash
//! # Show dataset statistics
//! dashflow dataset stats -i data.jsonl
//! dashflow dataset stats -i data.jsonl --output-format json
//!
//! # Validate dataset format
//! dashflow dataset validate -i data.jsonl --format jsonl
//! ```

use anyhow::{Context, Result};
use clap::{Args, Subcommand, ValueEnum};
use colored::Colorize;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

use crate::output::OutputFormat;

/// Dataset format
#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum DatasetFormat {
    /// JSON Lines (one JSON object per line)
    Jsonl,
    /// CSV format
    Csv,
    /// Parquet format
    Parquet,
    /// OpenAI fine-tuning format
    OpenaiFinetune,
    /// Anthropic format
    Anthropic,
}

#[derive(Args)]
pub struct DatasetArgs {
    #[command(subcommand)]
    pub command: DatasetCommand,
}

#[derive(Subcommand)]
pub enum DatasetCommand {
    /// Validate dataset format and contents
    Validate(ValidateArgs),

    /// Show dataset statistics
    Stats(StatsArgs),

    /// Convert between dataset formats
    Convert(ConvertArgs),

    /// Split dataset into train/val/test
    Split(SplitArgs),

    /// Sample random examples from dataset
    Sample(SampleArgs),

    /// Inspect individual examples
    Inspect(InspectArgs),
}

#[derive(Args)]
pub struct ValidateArgs {
    /// Path to dataset file
    #[arg(short, long)]
    pub input: PathBuf,

    /// Expected format
    #[arg(short, long, value_enum, default_value_t = DatasetFormat::Jsonl)]
    pub format: DatasetFormat,

    /// Schema file for validation (optional)
    #[arg(long)]
    pub schema: Option<PathBuf>,

    /// Required fields (comma-separated)
    #[arg(long)]
    pub required_fields: Option<String>,

    /// Show detailed errors
    #[arg(short, long)]
    pub verbose: bool,
}

#[derive(Args)]
pub struct StatsArgs {
    /// Path to dataset file
    #[arg(short, long)]
    pub input: PathBuf,

    /// Format of the dataset
    #[arg(short, long, value_enum, default_value_t = DatasetFormat::Jsonl)]
    pub format: DatasetFormat,

    /// Show field value distributions
    #[arg(long)]
    pub distribution: bool,

    /// Field to analyze for distribution
    #[arg(long)]
    pub field: Option<String>,

    /// Output format (table or json)
    #[arg(long, value_enum, default_value_t = OutputFormat::Table)]
    pub output_format: OutputFormat,
}

#[derive(Args)]
pub struct ConvertArgs {
    /// Input dataset path
    #[arg(short, long)]
    pub input: PathBuf,

    /// Output dataset path
    #[arg(short, long)]
    pub output: PathBuf,

    /// Input format
    #[arg(long, value_enum)]
    pub from: DatasetFormat,

    /// Output format
    #[arg(long, value_enum)]
    pub to: DatasetFormat,
}

#[derive(Args)]
pub struct SplitArgs {
    /// Input dataset path
    #[arg(short, long)]
    pub input: PathBuf,

    /// Output directory
    #[arg(short, long)]
    pub output_dir: PathBuf,

    /// Training split ratio (0.0-1.0)
    #[arg(long, default_value_t = 0.8)]
    pub train_ratio: f64,

    /// Validation split ratio (0.0-1.0)
    #[arg(long, default_value_t = 0.1)]
    pub val_ratio: f64,

    /// Random seed for reproducibility
    #[arg(long)]
    pub seed: Option<u64>,

    /// Stratify by field
    #[arg(long)]
    pub stratify: Option<String>,
}

#[derive(Args)]
pub struct SampleArgs {
    /// Input dataset path
    #[arg(short, long)]
    pub input: PathBuf,

    /// Number of samples to extract
    #[arg(short, long, default_value_t = 10)]
    pub count: usize,

    /// Output path (stdout if not specified)
    #[arg(short, long)]
    pub output: Option<PathBuf>,

    /// Random seed
    #[arg(long)]
    pub seed: Option<u64>,
}

#[derive(Args)]
pub struct InspectArgs {
    /// Input dataset path
    #[arg(short, long)]
    pub input: PathBuf,

    /// Index of example to inspect (0-based)
    #[arg(short, long)]
    pub index: Option<usize>,

    /// Show first N examples
    #[arg(short, long, default_value_t = 5)]
    pub head: usize,

    /// Pretty print JSON
    #[arg(long)]
    pub pretty: bool,
}

/// Dataset statistics
#[derive(Debug, Serialize, Deserialize)]
struct DatasetStats {
    total_examples: usize,
    fields: Vec<FieldStats>,
    size_bytes: u64,
    avg_example_size: f64,
}

#[derive(Debug, Serialize, Deserialize)]
struct FieldStats {
    name: String,
    present_count: usize,
    missing_count: usize,
    unique_values: Option<usize>,
    avg_length: Option<f64>,
}

pub async fn run(args: DatasetArgs) -> Result<()> {
    match args.command {
        DatasetCommand::Validate(args) => run_validate(args).await,
        DatasetCommand::Stats(args) => run_stats(args).await,
        DatasetCommand::Convert(args) => run_convert(args).await,
        DatasetCommand::Split(args) => run_split(args).await,
        DatasetCommand::Sample(args) => run_sample(args).await,
        DatasetCommand::Inspect(args) => run_inspect(args).await,
    }
}

async fn run_validate(args: ValidateArgs) -> Result<()> {
    println!("{} dataset validation", "Starting".bright_green());
    println!("  File: {}", args.input.display());
    println!("  Format: {:?}", args.format);

    if !args.input.exists() {
        anyhow::bail!("Dataset not found: {}", args.input.display());
    }

    let content = tokio::fs::read_to_string(&args.input)
        .await
        .with_context(|| format!("Failed to read: {}", args.input.display()))?;

    let required_fields: Vec<&str> = args
        .required_fields
        .as_deref()
        .map(|s| s.split(',').collect())
        .unwrap_or_default();

    let mut errors = Vec::new();
    let mut valid_count = 0;

    for (i, line) in content.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }

        match serde_json::from_str::<serde_json::Value>(line) {
            Ok(obj) => {
                // Check required fields
                for field in &required_fields {
                    if obj.get(*field).is_none() {
                        errors.push(format!(
                            "Line {}: missing required field '{}'",
                            i + 1,
                            field
                        ));
                    }
                }
                valid_count += 1;
            }
            Err(e) => {
                errors.push(format!("Line {}: invalid JSON - {}", i + 1, e));
            }
        }
    }

    println!();
    if errors.is_empty() {
        println!(
            "{} Dataset is valid ({} examples)",
            "✓".bright_green(),
            valid_count
        );
    } else {
        println!("{} Dataset has {} error(s)", "✗".bright_red(), errors.len());
        if args.verbose {
            for error in &errors {
                println!("  - {}", error.bright_red());
            }
        } else {
            println!("  Use --verbose to see all errors");
        }
        anyhow::bail!("Validation failed with {} errors", errors.len());
    }

    Ok(())
}

async fn run_stats(args: StatsArgs) -> Result<()> {
    if !matches!(args.output_format, OutputFormat::Json) {
        println!("{} dataset statistics", "Computing".bright_green());
        println!("  File: {}", args.input.display());
    }

    if !args.input.exists() {
        anyhow::bail!("Dataset not found: {}", args.input.display());
    }

    let metadata = tokio::fs::metadata(&args.input).await?;
    let content = tokio::fs::read_to_string(&args.input)
        .await
        .with_context(|| format!("Failed to read: {}", args.input.display()))?;

    let mut examples: Vec<serde_json::Value> = Vec::new();
    let mut field_counts: HashMap<String, usize> = HashMap::new();
    let mut field_lengths: HashMap<String, Vec<usize>> = HashMap::new();

    for line in content.lines() {
        if line.trim().is_empty() {
            continue;
        }

        if let Ok(obj) = serde_json::from_str::<serde_json::Value>(line) {
            if let Some(map) = obj.as_object() {
                for (key, value) in map {
                    *field_counts.entry(key.clone()).or_insert(0) += 1;
                    if let Some(s) = value.as_str() {
                        field_lengths.entry(key.clone()).or_default().push(s.len());
                    }
                }
            }
            examples.push(obj);
        }
    }

    let total = examples.len();
    let avg_size = if total > 0 {
        metadata.len() as f64 / total as f64
    } else {
        0.0
    };

    // Build FieldStats for each field
    let mut field_stats: Vec<FieldStats> = field_counts
        .iter()
        .map(|(name, &present_count)| {
            let avg_length = field_lengths.get(name).map(|lens| {
                if lens.is_empty() {
                    0.0
                } else {
                    lens.iter().sum::<usize>() as f64 / lens.len() as f64
                }
            });
            FieldStats {
                name: name.clone(),
                present_count,
                missing_count: total.saturating_sub(present_count),
                unique_values: None, // Could compute if needed
                avg_length,
            }
        })
        .collect();

    // Sort by present_count descending
    field_stats.sort_by(|a, b| b.present_count.cmp(&a.present_count));

    // Build DatasetStats
    let stats = DatasetStats {
        total_examples: total,
        fields: field_stats,
        size_bytes: metadata.len(),
        avg_example_size: avg_size,
    };

    // JSON output mode
    if matches!(args.output_format, OutputFormat::Json) {
        println!("{}", serde_json::to_string_pretty(&stats)?);
        return Ok(());
    }

    // Human-readable output
    println!();
    println!("{}", "=== Dataset Statistics ===".bright_white().bold());
    println!();
    println!(
        "  Total examples: {}",
        stats.total_examples.to_string().bright_cyan()
    );
    println!(
        "  File size: {} bytes ({:.1} KB)",
        stats.size_bytes,
        stats.size_bytes as f64 / 1024.0
    );
    println!("  Avg example size: {:.0} bytes", stats.avg_example_size);
    println!();

    println!("{}", "Fields:".bright_white());
    println!("  {:<20} {:>10} {:>12}", "Field", "Present", "Avg Length");
    println!("  {}", "-".repeat(45));

    for field in &stats.fields {
        let coverage = (field.present_count as f64 / total as f64) * 100.0;
        let coverage_str = if coverage >= 100.0 {
            "100%".bright_green()
        } else if coverage >= 90.0 {
            format!("{:.0}%", coverage).bright_yellow()
        } else {
            format!("{:.0}%", coverage).bright_red()
        };

        println!(
            "  {:<20} {:>10} {:>12}",
            field.name,
            coverage_str,
            field
                .avg_length
                .map_or("-".to_string(), |l| format!("{:.0}", l))
        );
    }

    if args.distribution {
        if let Some(field) = &args.field {
            println!();
            println!("{}", format!("Distribution of '{}':", field).bright_white());

            let mut value_counts: HashMap<String, usize> = HashMap::new();
            for example in &examples {
                if let Some(value) = example.get(field) {
                    let key = match value {
                        serde_json::Value::String(s) => s.clone(),
                        _ => value.to_string(),
                    };
                    *value_counts.entry(key).or_insert(0) += 1;
                }
            }

            let mut counts: Vec<_> = value_counts.iter().collect();
            counts.sort_by(|a, b| b.1.cmp(a.1));

            for (value, count) in counts.iter().take(10) {
                let pct = (**count as f64 / total as f64) * 100.0;
                println!(
                    "  {:<30} {:>6} ({:.1}%)",
                    if value.len() > 30 {
                        format!("{}...", &value[..27])
                    } else {
                        value.to_string()
                    },
                    count,
                    pct
                );
            }
        }
    }

    Ok(())
}

async fn run_convert(args: ConvertArgs) -> Result<()> {
    println!("{} dataset format", "Converting".bright_green());
    println!("  Input: {}", args.input.display());
    println!("  Output: {}", args.output.display());
    println!("  From: {:?} → To: {:?}", args.from, args.to);

    if !args.input.exists() {
        anyhow::bail!("Input not found: {}", args.input.display());
    }

    let input = args.input;
    let output = args.output;
    let from = args.from;
    let to = args.to;

    let (read_count, written_count) = tokio::task::spawn_blocking(move || -> Result<(usize, usize)> {
        // Read input data into intermediate representation (Vec<serde_json::Value>)
        let records = read_dataset(&input, from)?;
        let read_count = records.len();

        // Write output in target format
        let written_count = write_dataset(&output, to, &records)?;
        Ok((read_count, written_count))
    })
    .await??;

    println!("  Read {} records", read_count);

    println!();
    println!(
        "{} Converted {} records to {:?}",
        "✓".bright_green(),
        written_count,
        to
    );

    Ok(())
}

/// Read dataset from file in specified format into JSON values
fn read_dataset(path: &std::path::Path, format: DatasetFormat) -> Result<Vec<serde_json::Value>> {
    match format {
        DatasetFormat::Jsonl => read_jsonl(path),
        DatasetFormat::Csv => read_csv(path),
        DatasetFormat::OpenaiFinetune => read_openai_finetune(path),
        DatasetFormat::Anthropic => read_anthropic(path),
        DatasetFormat::Parquet => {
            anyhow::bail!("Parquet format not yet supported. Use CSV or JSONL instead.")
        }
    }
}

/// Write dataset to file in specified format
fn write_dataset(
    path: &std::path::Path,
    format: DatasetFormat,
    records: &[serde_json::Value],
) -> Result<usize> {
    match format {
        DatasetFormat::Jsonl => write_jsonl(path, records),
        DatasetFormat::Csv => write_csv(path, records),
        DatasetFormat::OpenaiFinetune => write_openai_finetune(path, records),
        DatasetFormat::Anthropic => write_anthropic(path, records),
        DatasetFormat::Parquet => {
            anyhow::bail!("Parquet format not yet supported. Use CSV or JSONL instead.")
        }
    }
}

// --- JSONL ---

fn read_jsonl(path: &std::path::Path) -> Result<Vec<serde_json::Value>> {
    let content = std::fs::read_to_string(path)?;
    let mut records = Vec::new();
    for (i, line) in content.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let value: serde_json::Value = serde_json::from_str(line)
            .with_context(|| format!("Invalid JSON at line {}", i + 1))?;
        records.push(value);
    }
    Ok(records)
}

fn write_jsonl(path: &std::path::Path, records: &[serde_json::Value]) -> Result<usize> {
    use std::io::Write;
    let mut file = std::fs::File::create(path)?;
    for record in records {
        writeln!(file, "{}", serde_json::to_string(record)?)?;
    }
    Ok(records.len())
}

// --- CSV ---

fn read_csv(path: &std::path::Path) -> Result<Vec<serde_json::Value>> {
    let mut reader = csv::Reader::from_path(path)?;
    let headers: Vec<String> = reader.headers()?.iter().map(String::from).collect();
    let mut records = Vec::new();

    for result in reader.records() {
        let record = result?;
        let mut obj = serde_json::Map::new();
        for (i, field) in record.iter().enumerate() {
            if i < headers.len() {
                obj.insert(
                    headers[i].clone(),
                    serde_json::Value::String(field.to_string()),
                );
            }
        }
        records.push(serde_json::Value::Object(obj));
    }
    Ok(records)
}

fn write_csv(path: &std::path::Path, records: &[serde_json::Value]) -> Result<usize> {
    if records.is_empty() {
        std::fs::write(path, "")?;
        return Ok(0);
    }

    // Collect all field names from all records
    let mut field_names: Vec<String> = Vec::new();
    for record in records {
        if let Some(obj) = record.as_object() {
            for key in obj.keys() {
                if !field_names.contains(key) {
                    field_names.push(key.clone());
                }
            }
        }
    }
    field_names.sort();

    let mut writer = csv::Writer::from_path(path)?;
    writer.write_record(&field_names)?;

    for record in records {
        let row: Vec<String> = field_names
            .iter()
            .map(|field| {
                record
                    .get(field)
                    .map(|v| match v {
                        serde_json::Value::String(s) => s.clone(),
                        serde_json::Value::Null => String::new(),
                        other => other.to_string(),
                    })
                    .unwrap_or_default()
            })
            .collect();
        writer.write_record(&row)?;
    }
    writer.flush()?;
    Ok(records.len())
}

// --- OpenAI Fine-tuning Format ---
// Format: {"messages": [{"role": "system", "content": "..."}, {"role": "user", "content": "..."}, {"role": "assistant", "content": "..."}]}

fn read_openai_finetune(path: &std::path::Path) -> Result<Vec<serde_json::Value>> {
    // OpenAI format is already JSONL with messages array
    read_jsonl(path)
}

fn write_openai_finetune(path: &std::path::Path, records: &[serde_json::Value]) -> Result<usize> {
    use std::io::Write;
    let mut file = std::fs::File::create(path)?;
    let mut count = 0;

    for record in records {
        // If already in OpenAI format (has "messages" key), write as-is
        if record.get("messages").is_some() {
            writeln!(file, "{}", serde_json::to_string(record)?)?;
            count += 1;
            continue;
        }

        // Convert from generic format with "input"/"output" or "prompt"/"completion"
        let mut messages = Vec::new();

        // Check for system prompt
        if let Some(system) = record.get("system").and_then(|v| v.as_str()) {
            messages.push(serde_json::json!({"role": "system", "content": system}));
        }

        // Get user message
        let user_content = record
            .get("input")
            .or_else(|| record.get("prompt"))
            .or_else(|| record.get("user"))
            .or_else(|| record.get("question"))
            .and_then(|v| v.as_str());

        // Get assistant message
        let assistant_content = record
            .get("output")
            .or_else(|| record.get("completion"))
            .or_else(|| record.get("assistant"))
            .or_else(|| record.get("answer"))
            .and_then(|v| v.as_str());

        if let Some(user) = user_content {
            messages.push(serde_json::json!({"role": "user", "content": user}));
        }

        if let Some(assistant) = assistant_content {
            messages.push(serde_json::json!({"role": "assistant", "content": assistant}));
        }

        if messages.is_empty() {
            // Can't convert, skip
            continue;
        }

        writeln!(
            file,
            "{}",
            serde_json::to_string(&serde_json::json!({"messages": messages}))?
        )?;
        count += 1;
    }
    Ok(count)
}

// --- Anthropic Format ---
// Format: {"prompt": "\n\nHuman: ...\n\nAssistant:", "completion": "..."}

fn read_anthropic(path: &std::path::Path) -> Result<Vec<serde_json::Value>> {
    // Anthropic format is JSONL with prompt/completion
    read_jsonl(path)
}

fn write_anthropic(path: &std::path::Path, records: &[serde_json::Value]) -> Result<usize> {
    use std::io::Write;
    let mut file = std::fs::File::create(path)?;
    let mut count = 0;

    for record in records {
        // If already in Anthropic format, write as-is
        if record.get("prompt").is_some() && record.get("completion").is_some() {
            writeln!(file, "{}", serde_json::to_string(record)?)?;
            count += 1;
            continue;
        }

        // Convert from OpenAI messages format
        if let Some(messages) = record.get("messages").and_then(|v| v.as_array()) {
            // Build up the conversation, with final assistant message as completion
            let mut turns: Vec<(String, String)> = Vec::new(); // (role, content)

            for msg in messages {
                let role = msg.get("role").and_then(|v| v.as_str()).unwrap_or("");
                let content = msg.get("content").and_then(|v| v.as_str()).unwrap_or("");
                turns.push((role.to_string(), content.to_string()));
            }

            // Find the last assistant message - that becomes the completion
            let last_assistant_idx = turns.iter().rposition(|(role, _)| role == "assistant");

            let completion = match last_assistant_idx {
                Some(idx) => turns[idx].1.clone(),
                None => String::new(),
            };

            // Build prompt from all messages except final assistant
            let mut prompt_parts = Vec::new();
            let message_count = if let Some(idx) = last_assistant_idx {
                idx
            } else {
                turns.len()
            };

            for (role, content) in turns.iter().take(message_count) {
                match role.as_str() {
                    "system" => {
                        prompt_parts.push(format!("\n\nHuman: {}", content));
                        prompt_parts.push("\n\nAssistant: I understand.".to_string());
                    }
                    "user" => {
                        prompt_parts.push(format!("\n\nHuman: {}", content));
                    }
                    "assistant" => {
                        prompt_parts.push(format!("\n\nAssistant: {}", content));
                    }
                    _ => {}
                }
            }

            // Ensure prompt ends with Assistant: marker
            let mut prompt = prompt_parts.join("");
            if !prompt.ends_with("\n\nAssistant:") && !prompt.ends_with("\n\nAssistant: ") {
                prompt.push_str("\n\nAssistant:");
            }

            writeln!(
                file,
                "{}",
                serde_json::to_string(&serde_json::json!({
                    "prompt": prompt,
                    "completion": format!(" {}", completion)
                }))?
            )?;
            count += 1;
            continue;
        }

        // Convert from generic input/output format
        let user_content = record
            .get("input")
            .or_else(|| record.get("user"))
            .or_else(|| record.get("question"))
            .and_then(|v| v.as_str());

        let assistant_content = record
            .get("output")
            .or_else(|| record.get("assistant"))
            .or_else(|| record.get("answer"))
            .and_then(|v| v.as_str());

        if let (Some(user), Some(assistant)) = (user_content, assistant_content) {
            writeln!(
                file,
                "{}",
                serde_json::to_string(&serde_json::json!({
                    "prompt": format!("\n\nHuman: {}\n\nAssistant:", user),
                    "completion": format!(" {}", assistant)
                }))?
            )?;
            count += 1;
        }
    }
    Ok(count)
}

async fn run_split(args: SplitArgs) -> Result<()> {
    println!("{} dataset", "Splitting".bright_green());
    println!("  Input: {}", args.input.display());
    println!("  Output dir: {}", args.output_dir.display());
    println!(
        "  Ratios: train={:.0}%, val={:.0}%, test={:.0}%",
        args.train_ratio * 100.0,
        args.val_ratio * 100.0,
        (1.0 - args.train_ratio - args.val_ratio) * 100.0
    );

    if !args.input.exists() {
        anyhow::bail!("Input not found: {}", args.input.display());
    }

    tokio::fs::create_dir_all(&args.output_dir).await?;

    let content = tokio::fs::read_to_string(&args.input).await?;
    let mut examples: Vec<&str> = content.lines().filter(|l| !l.trim().is_empty()).collect();

    // Shuffle with seed if provided
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    if let Some(seed) = args.seed {
        // Simple deterministic shuffle based on seed
        examples.sort_by(|a, b| {
            let mut ha = DefaultHasher::new();
            seed.hash(&mut ha);
            a.hash(&mut ha);
            let mut hb = DefaultHasher::new();
            seed.hash(&mut hb);
            b.hash(&mut hb);
            ha.finish().cmp(&hb.finish())
        });
    }

    let total = examples.len();
    let train_end = (total as f64 * args.train_ratio) as usize;
    let val_end = train_end + (total as f64 * args.val_ratio) as usize;

    let train: Vec<_> = examples[..train_end].to_vec();
    let val: Vec<_> = examples[train_end..val_end].to_vec();
    let test: Vec<_> = examples[val_end..].to_vec();

    tokio::fs::write(args.output_dir.join("train.jsonl"), train.join("\n")).await?;
    tokio::fs::write(args.output_dir.join("val.jsonl"), val.join("\n")).await?;
    tokio::fs::write(args.output_dir.join("test.jsonl"), test.join("\n")).await?;

    println!();
    println!("{}", "=== Split Complete ===".bright_white().bold());
    println!("  Train: {} examples", train.len());
    println!("  Val: {} examples", val.len());
    println!("  Test: {} examples", test.len());
    println!();
    println!("{} Dataset split complete", "✓".bright_green());

    Ok(())
}

async fn run_sample(args: SampleArgs) -> Result<()> {
    if !args.input.exists() {
        anyhow::bail!("Input not found: {}", args.input.display());
    }

    let content = tokio::fs::read_to_string(&args.input).await?;
    let examples: Vec<&str> = content.lines().filter(|l| !l.trim().is_empty()).collect();

    // Simple sampling (could use seed for reproducibility)
    let samples: Vec<_> = examples.iter().take(args.count).cloned().collect();

    let output = samples.join("\n");

    if let Some(output_path) = &args.output {
        tokio::fs::write(output_path, &output).await?;
        println!(
            "Sampled {} examples to {}",
            args.count,
            output_path.display()
        );
    } else {
        println!("{}", output);
    }

    Ok(())
}

async fn run_inspect(args: InspectArgs) -> Result<()> {
    if !args.input.exists() {
        anyhow::bail!("Input not found: {}", args.input.display());
    }

    let content = tokio::fs::read_to_string(&args.input).await?;
    let examples: Vec<&str> = content.lines().filter(|l| !l.trim().is_empty()).collect();

    if let Some(index) = args.index {
        if index >= examples.len() {
            anyhow::bail!("Index {} out of range (0-{})", index, examples.len() - 1);
        }

        println!("{}", format!("Example {}:", index).bright_white().bold());
        if args.pretty {
            let parsed: serde_json::Value = serde_json::from_str(examples[index])?;
            println!("{}", serde_json::to_string_pretty(&parsed)?);
        } else {
            println!("{}", examples[index]);
        }
    } else {
        println!(
            "{}",
            format!("First {} examples:", args.head)
                .bright_white()
                .bold()
        );
        for (i, example) in examples.iter().take(args.head).enumerate() {
            println!();
            println!("{}", format!("[{}]", i).bright_cyan());
            if args.pretty {
                let parsed: serde_json::Value = serde_json::from_str(example)?;
                println!("{}", serde_json::to_string_pretty(&parsed)?);
            } else {
                println!("{}", example);
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[tokio::test]
    async fn test_validate_valid_jsonl() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, r#"{{"input": "hello", "output": "world"}}"#).unwrap();
        writeln!(file, r#"{{"input": "foo", "output": "bar"}}"#).unwrap();

        let args = ValidateArgs {
            input: file.path().to_path_buf(),
            format: DatasetFormat::Jsonl,
            schema: None,
            required_fields: Some("input,output".to_string()),
            verbose: false,
        };

        let result = run_validate(args).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_validate_missing_field() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, r#"{{"input": "hello"}}"#).unwrap(); // missing "output"

        let args = ValidateArgs {
            input: file.path().to_path_buf(),
            format: DatasetFormat::Jsonl,
            schema: None,
            required_fields: Some("input,output".to_string()),
            verbose: true,
        };

        let result = run_validate(args).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_stats_json_output() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, r#"{{"input": "hello", "output": "world"}}"#).unwrap();
        writeln!(file, r#"{{"input": "foo", "output": "bar"}}"#).unwrap();
        writeln!(file, r#"{{"input": "test", "output": "value"}}"#).unwrap();

        let args = StatsArgs {
            input: file.path().to_path_buf(),
            format: DatasetFormat::Jsonl,
            distribution: false,
            field: None,
            output_format: OutputFormat::Json,
        };

        // Just verify it runs without error (JSON output goes to stdout)
        let result = run_stats(args).await;
        assert!(result.is_ok());
    }

    #[test]
    fn test_dataset_stats_serialization() {
        let stats = DatasetStats {
            total_examples: 100,
            fields: vec![
                FieldStats {
                    name: "input".to_string(),
                    present_count: 100,
                    missing_count: 0,
                    unique_values: Some(95),
                    avg_length: Some(42.5),
                },
                FieldStats {
                    name: "output".to_string(),
                    present_count: 98,
                    missing_count: 2,
                    unique_values: None,
                    avg_length: Some(128.3),
                },
            ],
            size_bytes: 15360,
            avg_example_size: 153.6,
        };

        let json = serde_json::to_string(&stats).unwrap();
        assert!(json.contains("\"total_examples\":100"));
        assert!(json.contains("\"input\""));
        assert!(json.contains("\"present_count\":100"));

        // Verify round-trip
        let parsed: DatasetStats = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.total_examples, 100);
        assert_eq!(parsed.fields.len(), 2);
        assert_eq!(parsed.fields[0].name, "input");
    }

    // --- Conversion tests ---

    #[test]
    fn test_jsonl_to_csv_conversion() {
        let mut input = NamedTempFile::new().unwrap();
        writeln!(input, r#"{{"name": "Alice", "age": "30"}}"#).unwrap();
        writeln!(input, r#"{{"name": "Bob", "age": "25"}}"#).unwrap();

        let records = read_jsonl(input.path()).unwrap();
        assert_eq!(records.len(), 2);

        let output = NamedTempFile::new().unwrap();
        let count = write_csv(output.path(), &records).unwrap();
        assert_eq!(count, 2);

        // Read back and verify
        let csv_content = std::fs::read_to_string(output.path()).unwrap();
        assert!(csv_content.contains("age,name")); // headers sorted
        assert!(csv_content.contains("30,Alice"));
        assert!(csv_content.contains("25,Bob"));
    }

    #[test]
    fn test_csv_to_jsonl_conversion() {
        let mut input = NamedTempFile::new().unwrap();
        writeln!(input, "name,score").unwrap();
        writeln!(input, "Alice,95").unwrap();
        writeln!(input, "Bob,87").unwrap();

        let records = read_csv(input.path()).unwrap();
        assert_eq!(records.len(), 2);
        assert_eq!(records[0]["name"], "Alice");
        assert_eq!(records[0]["score"], "95");

        let output = NamedTempFile::new().unwrap();
        let count = write_jsonl(output.path(), &records).unwrap();
        assert_eq!(count, 2);
    }

    #[test]
    fn test_jsonl_to_openai_finetune() {
        let mut input = NamedTempFile::new().unwrap();
        writeln!(input, r#"{{"input": "Hello", "output": "Hi there!"}}"#).unwrap();
        writeln!(
            input,
            r#"{{"prompt": "Question?", "completion": "Answer!"}}"#
        )
        .unwrap();

        let records = read_jsonl(input.path()).unwrap();
        let output = NamedTempFile::new().unwrap();
        let count = write_openai_finetune(output.path(), &records).unwrap();
        assert_eq!(count, 2);

        let result = std::fs::read_to_string(output.path()).unwrap();
        // Check that conversion created messages format
        assert!(result.contains("messages"));
        assert!(result.contains("user"));
        assert!(result.contains("assistant"));
    }

    #[test]
    fn test_jsonl_to_anthropic() {
        let mut input = NamedTempFile::new().unwrap();
        writeln!(input, r#"{{"input": "What is 2+2?", "output": "4"}}"#).unwrap();

        let records = read_jsonl(input.path()).unwrap();
        let output = NamedTempFile::new().unwrap();
        let count = write_anthropic(output.path(), &records).unwrap();
        assert_eq!(count, 1);

        let result = std::fs::read_to_string(output.path()).unwrap();
        assert!(result.contains("Human:"));
        assert!(result.contains("Assistant:"));
        assert!(result.contains("completion"));
    }

    #[test]
    fn test_openai_to_anthropic() {
        let mut input = NamedTempFile::new().unwrap();
        writeln!(input, r#"{{"messages": [{{"role": "user", "content": "Hi"}}, {{"role": "assistant", "content": "Hello!"}}]}}"#).unwrap();

        let records = read_openai_finetune(input.path()).unwrap();
        let output = NamedTempFile::new().unwrap();
        let count = write_anthropic(output.path(), &records).unwrap();
        assert_eq!(count, 1);

        let result = std::fs::read_to_string(output.path()).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(result.trim()).unwrap();
        assert!(parsed["prompt"].as_str().unwrap().contains("Human: Hi"));
        assert!(parsed["completion"].as_str().unwrap().contains("Hello!"));
    }

    #[test]
    fn test_empty_dataset_conversion() {
        let input = NamedTempFile::new().unwrap();
        // Empty file

        let records = read_jsonl(input.path()).unwrap();
        assert!(records.is_empty());

        let output = NamedTempFile::new().unwrap();
        let count = write_csv(output.path(), &records).unwrap();
        assert_eq!(count, 0);
    }
}
