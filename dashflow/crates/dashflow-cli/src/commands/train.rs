// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)
//! train - Train or fine-tune models (distillation, RL)
//!
//! The `distill` subcommand is wired to the dashflow distillation library.
//! It uses a teacher model (e.g., GPT-4) to generate high-quality responses
//! for input data, producing training data suitable for fine-tuning a student model.

use anyhow::{Context, Result};
use clap::{Args, Subcommand, ValueEnum};
use colored::Colorize;
use dashflow::constants::{
    DEFAULT_HEALTH_CHECK_INTERVAL, DEFAULT_HTTP_CONNECT_TIMEOUT, DEFAULT_LLM_REQUEST_TIMEOUT,
};
use dashflow::core::config_loader::env_vars::{
    env_string, has_api_key, openai_api_url, DEFAULT_OPENAI_FILES_ENDPOINT,
    DEFAULT_OPENAI_FINE_TUNING_JOBS_ENDPOINT, OPENAI_API_KEY,
};
use dashflow::core::config_loader::{ChatModelConfig, SecretReference};
use dashflow::core::language_models::{ChatModel, ReinforceConfig};
use dashflow::core::messages::Message;
use dashflow::optimize::optimizers::GRPOConfig;
use futures::stream::{FuturesUnordered, StreamExt};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use tokio::sync::Mutex;

/// Training method
#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum TrainMethod {
    /// Knowledge distillation (teacher → student)
    Distill,
    /// OpenAI fine-tuning API
    Finetune,
    /// Reinforcement learning (GRPO)
    Rl,
    /// Synthetic data generation
    Synthetic,
}

#[derive(Args)]
pub struct TrainArgs {
    #[command(subcommand)]
    pub command: TrainCommand,
}

#[derive(Subcommand)]
pub enum TrainCommand {
    /// Distill knowledge from teacher to student model
    Distill(DistillArgs),

    /// Fine-tune model via OpenAI API
    Finetune(FinetuneArgs),

    /// Generate synthetic training data
    Synthetic(SyntheticArgs),

    /// Train with reinforcement learning (GRPO)
    Rl(RlArgs),
}

#[derive(Args)]
pub struct DistillArgs {
    /// Path to training data (JSONL)
    #[arg(short, long)]
    pub trainset: PathBuf,

    /// Teacher model (large, high-quality)
    #[arg(long, default_value = "gpt-4o")]
    pub teacher: String,

    /// Student model (smaller, cheaper)
    #[arg(long, default_value = "gpt-4o-mini")]
    pub student: String,

    /// Output path for distillation results
    #[arg(short, long)]
    pub output: PathBuf,

    /// Number of examples to distill
    #[arg(long)]
    pub limit: Option<usize>,

    /// Include chain-of-thought in distillation
    #[arg(long)]
    pub include_cot: bool,

    /// Temperature for teacher generation
    #[arg(long, default_value_t = 0.7)]
    pub temperature: f64,

    /// Input field name in JSONL (default: "input" or "question")
    #[arg(long, default_value = "input")]
    pub input_field: String,

    /// System prompt for the teacher model
    #[arg(long)]
    pub system_prompt: Option<String>,

    /// Output format: "jsonl" for OpenAI fine-tuning, "json" for structured output
    #[arg(long, value_enum, default_value_t = OutputFormat::Jsonl)]
    pub format: OutputFormat,

    /// Enable verbose output
    #[arg(short, long)]
    pub verbose: bool,
}

/// Output format for distillation results
#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum OutputFormat {
    /// JSONL format suitable for OpenAI fine-tuning
    Jsonl,
    /// JSON format with structured metadata
    Json,
}

#[derive(Args)]
pub struct FinetuneArgs {
    /// Path to training data (JSONL in OpenAI format)
    #[arg(short, long)]
    pub trainset: PathBuf,

    /// Base model to fine-tune
    #[arg(long, default_value = "gpt-4o-mini-2024-07-18")]
    pub base_model: String,

    /// Suffix for the fine-tuned model name
    #[arg(long)]
    pub suffix: Option<String>,

    /// Number of training epochs
    #[arg(long, default_value_t = 3)]
    pub epochs: u32,

    /// Learning rate multiplier
    #[arg(long, default_value_t = 1.0)]
    pub learning_rate: f64,

    /// Batch size
    #[arg(long, default_value_t = 4)]
    pub batch_size: u32,

    /// Path to validation data (optional)
    #[arg(long)]
    pub valset: Option<PathBuf>,

    /// Wait for fine-tuning to complete
    #[arg(long)]
    pub wait: bool,

    /// Enable verbose output
    #[arg(short, long)]
    pub verbose: bool,
}

#[derive(Args)]
pub struct SyntheticArgs {
    /// Path to seed examples (JSONL)
    #[arg(short, long)]
    pub seed: PathBuf,

    /// Number of synthetic examples to generate
    #[arg(short, long, default_value_t = 100)]
    pub count: usize,

    /// Output path for synthetic data
    #[arg(short, long)]
    pub output: PathBuf,

    /// Generator model
    #[arg(long, default_value = "gpt-4o")]
    pub model: String,

    /// Diversity temperature
    #[arg(long, default_value_t = 0.9)]
    pub temperature: f64,

    /// Enable topic/category balancing
    #[arg(long)]
    pub balance: bool,

    /// Number of concurrent LLM calls (default: 4)
    #[arg(long, default_value_t = 4)]
    pub concurrency: usize,

    /// Maximum retries per failed generation (default: 3)
    #[arg(long, default_value_t = 3)]
    pub retries: usize,

    /// Enable verbose output
    #[arg(short, long)]
    pub verbose: bool,
}

#[derive(Args)]
pub struct RlArgs {
    /// Path to training data (JSONL)
    #[arg(short, long)]
    pub trainset: PathBuf,

    /// Path to graph definition
    #[arg(short, long)]
    pub graph: PathBuf,

    /// Output path for GRPO config
    #[arg(short, long)]
    pub output: PathBuf,

    /// Number of training iterations
    #[arg(long, default_value_t = 10)]
    pub iterations: usize,

    /// Number of examples per training step
    #[arg(long, default_value_t = 4)]
    pub examples_per_step: usize,

    /// Number of rollouts (samples) per example
    #[arg(long, default_value_t = 4)]
    pub rollouts: usize,

    /// Kafka broker address for DashStream events
    #[arg(long, default_value = "localhost:9092")]
    pub kafka_brokers: String,

    /// Kafka topic for DashStream events
    #[arg(long, default_value = "dashstream-events")]
    pub kafka_topic: String,

    /// Enable verbose output
    #[arg(short, long)]
    pub verbose: bool,
}

/// Distillation result summary
#[derive(Debug, Serialize, Deserialize)]
struct DistillResult {
    teacher: String,
    student: String,
    examples_processed: usize,
    examples_succeeded: usize,
    examples_failed: usize,
    output_path: String,
    duration_seconds: f64,
    estimated_cost: f64,
}

/// Single training example in OpenAI fine-tuning format
#[derive(Debug, Serialize, Deserialize)]
struct FineTuneExample {
    messages: Vec<FineTuneMessage>,
}

/// Message in OpenAI fine-tuning format
#[derive(Debug, Serialize, Deserialize)]
struct FineTuneMessage {
    role: String,
    content: String,
}

pub async fn run(args: TrainArgs) -> Result<()> {
    match args.command {
        TrainCommand::Distill(args) => run_distill(args).await,
        TrainCommand::Finetune(args) => run_finetune(args).await,
        TrainCommand::Synthetic(args) => run_synthetic(args).await,
        TrainCommand::Rl(args) => run_rl(args).await,
    }
}

async fn run_distill(args: DistillArgs) -> Result<()> {
    println!("{} knowledge distillation", "Starting".bright_green());
    println!("  Teacher: {}", args.teacher.bright_cyan());
    println!("  Student: {}", args.student.bright_yellow());
    println!("  Training data: {}", args.trainset.display());

    if !args.trainset.exists() {
        anyhow::bail!("Training data not found: {}", args.trainset.display());
    }

    // Check for OPENAI_API_KEY
    if !has_api_key(OPENAI_API_KEY) {
        anyhow::bail!(
            "OPENAI_API_KEY environment variable not set. \
             Distillation requires an OpenAI API key to call the teacher model."
        );
    }

    let start = std::time::Instant::now();

    // Load training data
    let content = tokio::fs::read_to_string(&args.trainset)
        .await
        .with_context(|| format!("Failed to read: {}", args.trainset.display()))?;

    let mut examples: Vec<serde_json::Value> = content
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(serde_json::from_str)
        .collect::<serde_json::Result<_>>()?;

    if let Some(limit) = args.limit {
        examples.truncate(limit);
    }

    println!("  {} {} examples", "Loaded".bright_cyan(), examples.len());

    // Create teacher model
    let teacher_config = ChatModelConfig::OpenAI {
        model: args.teacher.clone(),
        api_key: SecretReference::EnvVar {
            env: "OPENAI_API_KEY".to_string(),
        },
        temperature: Some(args.temperature as f32),
        max_tokens: None,
        base_url: None,
        organization: None,
    };
    let teacher_model: Arc<dyn ChatModel> = dashflow_openai::build_chat_model(&teacher_config)?;

    println!(
        "  {} teacher model: {}",
        "Created".bright_cyan(),
        args.teacher
    );

    // Default system prompt if not provided
    let system_prompt = args.system_prompt.clone().unwrap_or_else(|| {
        "You are a helpful assistant. Answer the question accurately and concisely.".to_string()
    });

    // Prepare output file
    use tokio::io::AsyncWriteExt;
    let mut output_file = tokio::fs::File::create(&args.output)
        .await
        .with_context(|| format!("Failed to create output file: {}", args.output.display()))?;

    let mut succeeded = 0;
    let mut failed = 0;
    let mut estimated_cost = 0.0;

    println!();
    println!("{}", "Processing examples...".bright_white());

    // Process each example through the teacher model
    for (i, example) in examples.iter().enumerate() {
        // Extract input from the example
        let input_text = extract_input_field(example, &args.input_field)?;

        if args.verbose {
            println!(
                "  [{}/{}] Processing: {}...",
                i + 1,
                examples.len(),
                truncate_str(&input_text, 50)
            );
        }

        // Build messages for the teacher
        let messages = if args.include_cot {
            vec![
                Message::system(format!(
                    "{}\n\nThink step by step before providing your final answer.",
                    system_prompt
                )),
                Message::human(input_text.clone()),
            ]
        } else {
            vec![
                Message::system(system_prompt.clone()),
                Message::human(input_text.clone()),
            ]
        };

        // Call the teacher model
        match teacher_model
            .generate(&messages, None, None, None, None)
            .await
        {
            Ok(result) => {
                if let Some(generation) = result.generations.first() {
                    let response = generation.message.as_text();

                    // Create fine-tuning example in OpenAI format
                    let ft_example = FineTuneExample {
                        messages: vec![
                            FineTuneMessage {
                                role: "system".to_string(),
                                content: system_prompt.clone(),
                            },
                            FineTuneMessage {
                                role: "user".to_string(),
                                content: input_text.clone(),
                            },
                            FineTuneMessage {
                                role: "assistant".to_string(),
                                content: response.clone(),
                            },
                        ],
                    };

                    // Write to output (JSONL format - one JSON object per line)
                    let json_line = serde_json::to_string(&ft_example)?;
                    output_file
                        .write_all(format!("{}\n", json_line).as_bytes())
                        .await?;

                    // Estimate cost (rough approximation based on model pricing)
                    let input_tokens = input_text.len() / 4; // ~4 chars per token
                    let output_tokens = response.len() / 4;
                    estimated_cost += estimate_cost(&args.teacher, input_tokens, output_tokens);

                    succeeded += 1;

                    if args.verbose {
                        println!(
                            "    {} Response: {}...",
                            "✓".bright_green(),
                            truncate_str(&response, 60)
                        );
                    }
                } else {
                    failed += 1;
                    if args.verbose {
                        eprintln!("    {} No response generated", "✗".bright_red());
                    }
                }
            }
            Err(e) => {
                failed += 1;
                if args.verbose {
                    eprintln!("    {} Error: {}", "✗".bright_red(), e);
                }
            }
        }

        // Progress indicator for non-verbose mode
        if !args.verbose && (i + 1) % 10 == 0 {
            println!(
                "  Progress: {}/{} ({} succeeded, {} failed)",
                i + 1,
                examples.len(),
                succeeded,
                failed
            );
        }
    }

    let duration = start.elapsed();

    // Write summary if JSON format requested
    if matches!(args.format, OutputFormat::Json) {
        let result = DistillResult {
            teacher: args.teacher.clone(),
            student: args.student.clone(),
            examples_processed: examples.len(),
            examples_succeeded: succeeded,
            examples_failed: failed,
            output_path: args.output.display().to_string(),
            duration_seconds: duration.as_secs_f64(),
            estimated_cost,
        };

        // Write summary to a separate file
        let summary_path = args.output.with_extension("summary.json");
        let summary_json = serde_json::to_string_pretty(&result)?;
        tokio::fs::write(&summary_path, &summary_json).await?;
        println!("  Summary written to: {}", summary_path.display());
    }

    println!();
    println!("{}", "=== Distillation Complete ===".bright_white().bold());
    println!("  Total examples: {}", examples.len());
    println!("  Succeeded: {}", succeeded.to_string().bright_green());
    if failed > 0 {
        println!("  Failed: {}", failed.to_string().bright_red());
    }
    println!("  Duration: {:.1}s", duration.as_secs_f64());
    println!("  Estimated cost: ${:.4}", estimated_cost);
    println!(
        "  Output: {}",
        args.output.display().to_string().bright_cyan()
    );
    println!();
    println!(
        "{} Training data ready for fine-tuning {}",
        "✓".bright_green(),
        args.student.bright_yellow()
    );

    Ok(())
}

/// Extract the input field from a JSON example
fn extract_input_field(example: &serde_json::Value, field_name: &str) -> Result<String> {
    // Try the specified field first
    if let Some(value) = example.get(field_name) {
        return value_to_string(value);
    }

    // Try common alternative field names
    let alternatives = ["question", "prompt", "text", "query", "content"];
    for alt in alternatives {
        if let Some(value) = example.get(alt) {
            return value_to_string(value);
        }
    }

    // If it's a simple string value, use it directly
    if let Some(s) = example.as_str() {
        return Ok(s.to_string());
    }

    anyhow::bail!(
        "Could not find input field '{}' (or alternatives: question, prompt, text, query) in example: {}",
        field_name,
        serde_json::to_string(example).unwrap_or_else(|_| "?".to_string())
    )
}

/// Convert a JSON value to a string
fn value_to_string(value: &serde_json::Value) -> Result<String> {
    match value {
        serde_json::Value::String(s) => Ok(s.clone()),
        serde_json::Value::Number(n) => Ok(n.to_string()),
        serde_json::Value::Bool(b) => Ok(b.to_string()),
        _ => Ok(serde_json::to_string(value)?),
    }
}

/// Truncate a string for display
fn truncate_str(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len])
    }
}

/// Estimate cost based on model and token counts
fn estimate_cost(model: &str, input_tokens: usize, output_tokens: usize) -> f64 {
    // Pricing per 1K tokens (as of 2024)
    let (input_price, output_price) = if model.contains("gpt-4o-mini") {
        (0.00015, 0.0006) // GPT-4o-mini pricing
    } else if model.contains("gpt-4o") {
        (0.0025, 0.01) // GPT-4o pricing
    } else if model.contains("gpt-4-turbo") || model.contains("gpt-4-1106") {
        (0.01, 0.03) // GPT-4 Turbo pricing
    } else if model.contains("gpt-4") {
        (0.03, 0.06) // GPT-4 pricing
    } else if model.contains("gpt-3.5") {
        (0.0005, 0.0015) // GPT-3.5 pricing
    } else {
        (0.01, 0.03) // Default to GPT-4 Turbo pricing
    };

    let input_cost = (input_tokens as f64 / 1000.0) * input_price;
    let output_cost = (output_tokens as f64 / 1000.0) * output_price;
    input_cost + output_cost
}

async fn run_finetune(args: FinetuneArgs) -> Result<()> {
    println!("{} OpenAI fine-tuning", "Starting".bright_green());
    println!("  Base model: {}", args.base_model.bright_cyan());
    println!("  Training data: {}", args.trainset.display());

    if !args.trainset.exists() {
        anyhow::bail!("Training data not found: {}", args.trainset.display());
    }

    // Check for OPENAI_API_KEY
    let api_key = env_string(OPENAI_API_KEY).ok_or_else(|| {
        anyhow::anyhow!(
            "OPENAI_API_KEY environment variable not set. \
             Fine-tuning requires an OpenAI API key."
        )
    })?;

    // Load and validate training data format
    let content = tokio::fs::read_to_string(&args.trainset)
        .await
        .with_context(|| format!("Failed to read: {}", args.trainset.display()))?;

    let examples: Vec<serde_json::Value> = content
        .lines()
        .filter(|l| !l.trim().is_empty())
        .enumerate()
        .map(|(i, l)| {
            serde_json::from_str(l).with_context(|| format!("Invalid JSON at line {}", i + 1))
        })
        .collect::<Result<_>>()?;

    // Validate format (must have "messages" field for OpenAI fine-tuning)
    for (i, example) in examples.iter().enumerate() {
        if example.get("messages").is_none() {
            anyhow::bail!(
                "Example {} missing 'messages' field. \
                 Run 'dashflow train distill' first to generate properly formatted data.",
                i + 1
            );
        }
    }

    println!(
        "  {} {} examples",
        "Validated".bright_cyan(),
        examples.len()
    );

    // Build HTTP client
    let client = reqwest::Client::builder()
        .timeout(DEFAULT_LLM_REQUEST_TIMEOUT)
        .connect_timeout(DEFAULT_HTTP_CONNECT_TIMEOUT)
        .build()
        .context("Failed to create HTTP client")?;

    // Step 1: Upload training file
    println!("  {} training file...", "Uploading".bright_cyan());

    let form = reqwest::multipart::Form::new()
        .text("purpose", "fine-tune")
        .part(
            "file",
            reqwest::multipart::Part::text(content.clone())
                .file_name("training_data.jsonl")
                .mime_str("application/json")
                .context("Invalid MIME type")?,
        );

    let upload_response = client
        .post(openai_api_url(DEFAULT_OPENAI_FILES_ENDPOINT))
        .bearer_auth(&api_key)
        .multipart(form)
        .send()
        .await
        .context("Failed to upload training file")?;

    if !upload_response.status().is_success() {
        let status = upload_response.status();
        let error_text = upload_response.text().await.unwrap_or_default();
        anyhow::bail!("File upload failed ({}): {}", status, error_text);
    }

    #[derive(serde::Deserialize)]
    struct UploadResponse {
        id: String,
    }

    let upload_result: UploadResponse = upload_response
        .json()
        .await
        .context("Failed to parse upload response")?;

    println!(
        "  {} File ID: {}",
        "Uploaded".bright_green(),
        upload_result.id
    );

    // Step 2: Create fine-tuning job
    println!("  {} fine-tuning job...", "Creating".bright_cyan());

    #[derive(serde::Serialize)]
    struct Hyperparameters {
        n_epochs: u32,
        learning_rate_multiplier: f64,
        batch_size: u32,
    }

    #[derive(serde::Serialize)]
    struct CreateJobRequest {
        model: String,
        training_file: String,
        hyperparameters: Hyperparameters,
        #[serde(skip_serializing_if = "Option::is_none")]
        validation_file: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        suffix: Option<String>,
    }

    let mut job_request = CreateJobRequest {
        model: args.base_model.clone(),
        training_file: upload_result.id,
        hyperparameters: Hyperparameters {
            n_epochs: args.epochs,
            learning_rate_multiplier: args.learning_rate,
            batch_size: args.batch_size,
        },
        validation_file: None,
        suffix: args.suffix.clone(),
    };

    // Upload validation file if provided
    if let Some(valset_path) = &args.valset {
        if tokio::fs::try_exists(valset_path).await.unwrap_or(false) {
            let val_content = tokio::fs::read_to_string(valset_path)
                .await
                .with_context(|| {
                    format!("Failed to read validation file: {}", valset_path.display())
                })?;

            let val_form = reqwest::multipart::Form::new()
                .text("purpose", "fine-tune")
                .part(
                    "file",
                    reqwest::multipart::Part::text(val_content)
                        .file_name("validation_data.jsonl")
                        .mime_str("application/json")
                        .context("Invalid MIME type")?,
                );

            let val_response = client
                .post(openai_api_url(DEFAULT_OPENAI_FILES_ENDPOINT))
                .bearer_auth(&api_key)
                .multipart(val_form)
                .send()
                .await
                .context("Failed to upload validation file")?;

            if val_response.status().is_success() {
                let val_result: UploadResponse = val_response.json().await?;
                job_request.validation_file = Some(val_result.id);
                println!("  {} validation file", "Uploaded".bright_green());
            }
        }
    }

    let job_response = client
        .post(openai_api_url(DEFAULT_OPENAI_FINE_TUNING_JOBS_ENDPOINT))
        .bearer_auth(&api_key)
        .json(&job_request)
        .send()
        .await
        .context("Failed to create fine-tuning job")?;

    if !job_response.status().is_success() {
        let status = job_response.status();
        let error_text = job_response.text().await.unwrap_or_default();
        anyhow::bail!("Job creation failed ({}): {}", status, error_text);
    }

    #[derive(serde::Deserialize)]
    struct JobResponse {
        id: String,
        status: String,
        fine_tuned_model: Option<String>,
    }

    let job_result: JobResponse = job_response
        .json()
        .await
        .context("Failed to parse job response")?;

    println!();
    println!(
        "{}",
        "=== Fine-tuning Job Created ===".bright_white().bold()
    );
    println!("  Job ID: {}", job_result.id.bright_cyan());
    println!("  Base model: {}", args.base_model);
    println!("  Training examples: {}", examples.len());
    println!("  Epochs: {}", args.epochs);
    println!("  Learning rate: {}x", args.learning_rate);
    println!("  Status: {}", job_result.status.bright_yellow());

    // Step 3: Optionally wait for completion
    if args.wait {
        println!();
        println!("{} for fine-tuning to complete...", "Waiting".bright_cyan());

        let poll_interval = DEFAULT_HEALTH_CHECK_INTERVAL;
        loop {
            tokio::time::sleep(poll_interval).await;

            let status_response = client
                .get(format!(
                    "{}/{}",
                    openai_api_url(DEFAULT_OPENAI_FINE_TUNING_JOBS_ENDPOINT),
                    job_result.id
                ))
                .bearer_auth(&api_key)
                .send()
                .await?;

            if !status_response.status().is_success() {
                continue;
            }

            let status: JobResponse = status_response.json().await?;

            match status.status.as_str() {
                "succeeded" => {
                    println!();
                    println!("{} Fine-tuning completed!", "✓".bright_green());
                    if let Some(model_id) = status.fine_tuned_model {
                        println!("  Fine-tuned model: {}", model_id.bright_cyan());
                    }
                    break;
                }
                "failed" | "cancelled" => {
                    anyhow::bail!("Fine-tuning job {}: {}", status.status, job_result.id);
                }
                _ => {
                    print!(".");
                    std::io::Write::flush(&mut std::io::stdout())?;
                }
            }
        }
    } else {
        println!();
        println!(
            "{} Check status: openai api fine_tuning.jobs.retrieve -i {}",
            "Tip:".bright_yellow(),
            job_result.id
        );
    }

    println!();
    println!("{} Fine-tuning job submitted", "✓".bright_green());

    Ok(())
}

/// Result of a single synthetic generation attempt
struct GenerationResult {
    example: Option<serde_json::Value>,
    input_tokens: usize,
    output_tokens: usize,
    retries_used: usize,
}

/// Generate a single synthetic example with retry logic
async fn generate_single_example(
    model: Arc<dyn ChatModel>,
    system_prompt: &str,
    user_prompt: String,
    max_retries: usize,
) -> GenerationResult {
    let messages = vec![
        Message::system(system_prompt.to_string()),
        Message::human(user_prompt),
    ];

    let input_tokens = system_prompt.len() / 4;
    let mut retries_used = 0;

    for attempt in 0..=max_retries {
        if attempt > 0 {
            // Exponential backoff: 100ms, 200ms, 400ms, ...
            let delay_ms = 100 * (1 << (attempt - 1));
            tokio::time::sleep(std::time::Duration::from_millis(delay_ms)).await;
            retries_used = attempt;
        }

        match model.generate(&messages, None, None, None, None).await {
            Ok(result) => {
                if let Some(generation) = result.generations.first() {
                    let response = generation.message.as_text();
                    let output_tokens = response.len() / 4;

                    // Try to extract JSON from the response (handle markdown code blocks)
                    let json_str = extract_json_from_response(&response);

                    if let Ok(generated_example) =
                        serde_json::from_str::<serde_json::Value>(json_str)
                    {
                        return GenerationResult {
                            example: Some(generated_example),
                            input_tokens,
                            output_tokens,
                            retries_used,
                        };
                    }
                    // Invalid JSON, retry if attempts remain
                }
            }
            Err(_) => {
                // API error, retry if attempts remain
            }
        }
    }

    // All retries exhausted
    GenerationResult {
        example: None,
        input_tokens,
        output_tokens: 0,
        retries_used,
    }
}

/// Extract JSON from a response that might be wrapped in markdown code blocks
fn extract_json_from_response(response: &str) -> &str {
    let trimmed = response.trim();

    // Try to extract from ```json ... ``` or ``` ... ```
    if let Some(start) = trimmed.find("```") {
        let after_ticks = &trimmed[start + 3..];
        // Skip optional language identifier (e.g., "json")
        let content_start = after_ticks.find('\n').map(|i| i + 1).unwrap_or(0);
        let content = &after_ticks[content_start..];

        if let Some(end) = content.find("```") {
            return content[..end].trim();
        }
    }

    trimmed
}

async fn run_synthetic(args: SyntheticArgs) -> Result<()> {
    println!("{} synthetic data generation", "Starting".bright_green());
    println!("  Seed data: {}", args.seed.display());
    println!("  Target count: {}", args.count);
    println!("  Model: {}", args.model.bright_cyan());
    println!("  Concurrency: {}", args.concurrency);
    println!("  Max retries: {}", args.retries);

    if !args.seed.exists() {
        anyhow::bail!("Seed data not found: {}", args.seed.display());
    }

    // Check for OPENAI_API_KEY
    if !has_api_key(OPENAI_API_KEY) {
        anyhow::bail!(
            "OPENAI_API_KEY environment variable not set. \
             Synthetic data generation requires an OpenAI API key."
        );
    }

    let start = std::time::Instant::now();

    // Load seed examples
    let content = tokio::fs::read_to_string(&args.seed)
        .await
        .with_context(|| format!("Failed to read: {}", args.seed.display()))?;

    let seed_examples: Vec<serde_json::Value> = content
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(serde_json::from_str)
        .collect::<serde_json::Result<_>>()?;

    if seed_examples.is_empty() {
        anyhow::bail!("Seed file is empty. Need at least one example to generate from.");
    }

    println!(
        "  {} {} seed examples",
        "Loaded".bright_cyan(),
        seed_examples.len()
    );

    // Create generator model
    let generator_config = ChatModelConfig::OpenAI {
        model: args.model.clone(),
        api_key: SecretReference::EnvVar {
            env: "OPENAI_API_KEY".to_string(),
        },
        temperature: Some(args.temperature as f32),
        max_tokens: None,
        base_url: None,
        organization: None,
    };
    let generator_model: Arc<dyn ChatModel> = dashflow_openai::build_chat_model(&generator_config)?;

    // Build generation prompt based on seed examples
    let seed_sample: Vec<&serde_json::Value> = seed_examples.iter().take(5).collect();
    let seed_json = serde_json::to_string_pretty(&seed_sample)?;

    let system_prompt = Arc::new(format!(
        "You are a data generation assistant. Generate new, diverse examples similar \
         to the following seed examples. Each example should follow the same JSON structure \
         but with unique content.\n\nSeed examples:\n{}\n\nGenerate one new example in \
         JSON format. Only output the JSON, no explanation.",
        seed_json
    ));

    // Thread-safe output and counters
    let output_file = Arc::new(Mutex::new(
        tokio::fs::File::create(&args.output)
            .await
            .with_context(|| format!("Failed to create output file: {}", args.output.display()))?,
    ));

    let generated = Arc::new(AtomicUsize::new(0));
    let failed = Arc::new(AtomicUsize::new(0));
    let total_retries = Arc::new(AtomicUsize::new(0));
    let estimated_cost = Arc::new(Mutex::new(0.0f64));

    println!();
    println!("{}", "Generating synthetic examples...".bright_white());

    // Process in batches with concurrency control
    let concurrency = args.concurrency.max(1);
    let mut pending_count = args.count;
    let mut task_index = 0;

    while pending_count > 0 {
        // Create a batch of concurrent tasks
        let batch_size = pending_count.min(concurrency);
        let mut futures: FuturesUnordered<_> = FuturesUnordered::new();

        for _ in 0..batch_size {
            let seed_idx = task_index % seed_examples.len();
            let current_seed = seed_examples[seed_idx].clone();
            task_index += 1;

            let user_prompt = if args.balance {
                format!(
                    "Generate a new example similar to this one, but ensure it covers \
                     a different topic or category:\n{}",
                    serde_json::to_string_pretty(&current_seed).unwrap_or_default()
                )
            } else {
                format!(
                    "Generate a new example similar to this one:\n{}",
                    serde_json::to_string_pretty(&current_seed).unwrap_or_default()
                )
            };

            let model = generator_model.clone();
            let sys_prompt = system_prompt.clone();
            let max_retries = args.retries;
            let output = output_file.clone();
            let gen_count = generated.clone();
            let fail_count = failed.clone();
            let retry_count = total_retries.clone();
            let cost = estimated_cost.clone();
            let model_name = args.model.clone();
            let verbose = args.verbose;

            futures.push(async move {
                let result =
                    generate_single_example(model, &sys_prompt, user_prompt, max_retries).await;

                retry_count.fetch_add(result.retries_used, Ordering::Relaxed);

                if let Some(example) = result.example {
                    // Write to output file
                    let json_line = serde_json::to_string(&example).unwrap_or_default();
                    {
                        use tokio::io::AsyncWriteExt;
                        let mut file = output.lock().await;
                        if let Err(e) = file
                            .write_all(format!("{}\n", json_line).as_bytes())
                            .await
                        {
                            eprintln!(
                                "    {} Failed to write example to output file: {}",
                                "✗".bright_red(),
                                e
                            );
                            // Continue processing but log the error
                        }
                    }

                    // Update cost
                    {
                        let mut cost_guard = cost.lock().await;
                        *cost_guard +=
                            estimate_cost(&model_name, result.input_tokens, result.output_tokens);
                    }

                    let count = gen_count.fetch_add(1, Ordering::Relaxed) + 1;
                    if verbose {
                        println!("    {} Generated example {}", "✓".bright_green(), count);
                    }
                    true
                } else {
                    fail_count.fetch_add(1, Ordering::Relaxed);
                    if verbose {
                        eprintln!(
                            "    {} Failed after {} retries",
                            "✗".bright_red(),
                            result.retries_used
                        );
                    }
                    false
                }
            });
        }

        // Wait for this batch to complete
        while futures.next().await.is_some() {}

        pending_count = pending_count.saturating_sub(batch_size);

        // Progress indicator for non-verbose mode
        let current_gen = generated.load(Ordering::Relaxed);
        let current_fail = failed.load(Ordering::Relaxed);
        let total_processed = current_gen + current_fail;

        if !args.verbose && total_processed > 0 && total_processed % 10 == 0 {
            println!(
                "  Progress: {}/{} ({} succeeded, {} failed)",
                total_processed, args.count, current_gen, current_fail
            );
        }
    }

    let duration = start.elapsed();
    let final_generated = generated.load(Ordering::Relaxed);
    let final_failed = failed.load(Ordering::Relaxed);
    let final_retries = total_retries.load(Ordering::Relaxed);
    let final_cost = *estimated_cost.lock().await;

    println!();
    println!(
        "{}",
        "=== Synthetic Generation Complete ==="
            .bright_white()
            .bold()
    );
    println!(
        "  Generated: {} examples",
        final_generated.to_string().bright_green()
    );
    if final_failed > 0 {
        println!("  Failed: {}", final_failed.to_string().bright_red());
    }
    if final_retries > 0 {
        println!("  Total retries: {}", final_retries);
    }
    println!("  Duration: {:.1}s", duration.as_secs_f64());
    println!("  Estimated cost: ${:.4}", final_cost);
    println!(
        "  Output: {}",
        args.output.display().to_string().bright_cyan()
    );
    println!();
    println!("{} Synthetic data generation complete", "✓".bright_green());

    Ok(())
}

async fn run_rl(args: RlArgs) -> Result<()> {
    println!(
        "{} reinforcement learning (GRPO)",
        "Starting".bright_green()
    );
    println!("  Graph: {}", args.graph.display());
    println!("  Training data: {}", args.trainset.display());
    println!("  Training steps: {}", args.iterations);
    println!("  Examples per step: {}", args.examples_per_step);
    println!("  Rollouts per example: {}", args.rollouts);
    println!("  Kafka: {}:{}", args.kafka_brokers, args.kafka_topic);

    if !args.trainset.exists() {
        anyhow::bail!("Training data not found: {}", args.trainset.display());
    }
    if !args.graph.exists() {
        anyhow::bail!("Graph definition not found: {}", args.graph.display());
    }

    // Check for OPENAI_API_KEY
    if !has_api_key(OPENAI_API_KEY) {
        anyhow::bail!(
            "OPENAI_API_KEY environment variable not set. \
             GRPO training requires an OpenAI API key for reinforcement learning."
        );
    }

    let start = std::time::Instant::now();

    // Load training data
    let trainset_content = tokio::fs::read_to_string(&args.trainset)
        .await
        .with_context(|| format!("Failed to read: {}", args.trainset.display()))?;

    let training_examples: Vec<serde_json::Value> = trainset_content
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(serde_json::from_str)
        .collect::<serde_json::Result<_>>()
        .context("Failed to parse training data as JSONL")?;

    println!(
        "  {} {} training examples",
        "Loaded".bright_cyan(),
        training_examples.len()
    );

    // Load graph definition
    let graph_content = tokio::fs::read_to_string(&args.graph)
        .await
        .with_context(|| format!("Failed to read graph: {}", args.graph.display()))?;

    // Validate graph is valid JSON
    let graph_def: serde_json::Value = serde_json::from_str(&graph_content)
        .with_context(|| "Graph definition must be valid JSON")?;

    // Check for required graph fields
    if graph_def.get("nodes").is_none() {
        anyhow::bail!(
            "Graph definition missing 'nodes' field. \
             Expected format: {{\"nodes\": [...], \"edges\": [...]}}"
        );
    }

    let node_count = graph_def["nodes"].as_array().map(|a| a.len()).unwrap_or(0);
    println!(
        "  {} graph ({} nodes)",
        "Validated".bright_cyan(),
        node_count
    );

    // Create actual GRPOConfig using library types
    let grpo_config = GRPOConfig::new()
        .with_num_train_steps(args.iterations)
        .with_num_examples_per_step(args.examples_per_step)
        .with_num_rollouts_per_step(args.rollouts)
        .with_kafka_brokers(args.kafka_brokers.clone())
        .with_kafka_topic(args.kafka_topic.clone())
        .with_reinforce_config(ReinforceConfig::default());

    // Calculate training statistics
    let total_rollouts = args.iterations * args.examples_per_step * args.rollouts;

    println!();
    println!("{}", "GRPO Configuration (from library):".bright_white());
    println!("  num_train_steps: {}", grpo_config.num_train_steps);
    println!(
        "  num_examples_per_step: {}",
        grpo_config.num_examples_per_step
    );
    println!(
        "  num_rollouts_per_step: {}",
        grpo_config.num_rollouts_per_step
    );
    println!("  kafka_brokers: {}", grpo_config.kafka_brokers);
    println!("  kafka_topic: {}", grpo_config.kafka_topic);
    println!("  failure_score: {}", grpo_config.failure_score);
    println!(
        "  format_failure_score: {}",
        grpo_config.format_failure_score
    );
    println!(
        "  {} total rollouts",
        total_rollouts.to_string().bright_cyan()
    );

    // GRPO requires a compiled graph with state types and a metric function.
    // The CLI generates a validated config; full training uses library API.
    println!();
    println!(
        "{}",
        "=== GRPO Training Configuration ===".bright_white().bold()
    );
    println!();
    println!(
        "{}",
        "GRPO requires a custom metric function (reward signal) that cannot be".bright_yellow()
    );
    println!(
        "{}",
        "specified via CLI. Use the validated config with the library API:".bright_yellow()
    );
    println!();
    println!("  // Load the config generated by this command:");
    println!("  let config: GRPOConfig = serde_json::from_str(&config_json)?;");
    println!();
    println!("  // Define your reward function:");
    println!("  let metric = Arc::new(|example, prediction, _trace| {{");
    println!("      let expected = example.get(\"answer\").unwrap();");
    println!("      let actual = prediction.get(\"answer\").unwrap();");
    println!("      Ok(if expected == actual {{ 1.0 }} else {{ 0.0 }})");
    println!("  }});");
    println!();
    println!("  // Run GRPO training:");
    println!("  let grpo = GRPO::new(metric, config);");
    println!("  let job = grpo.optimize_with_pregenerated_traces(");
    println!("      trainset, thread_ids_per_step, &chat_model");
    println!("  ).await?;");
    println!();

    // Serialize the actual GRPOConfig struct (type-safe, library-compatible)
    let config_output = serde_json::json!({
        "grpo_config": grpo_config,
        "metadata": {
            "graph_file": args.graph.display().to_string(),
            "trainset_file": args.trainset.display().to_string(),
            "trainset_count": training_examples.len(),
            "total_rollouts": total_rollouts,
            "generated_by": "dashflow train rl",
        }
    });

    tokio::fs::write(&args.output, serde_json::to_string_pretty(&config_output)?)
        .await
        .with_context(|| format!("Failed to write config: {}", args.output.display()))?;

    let duration = start.elapsed();

    println!("{}", "=== GRPO Config Saved ===".bright_white().bold());
    println!(
        "  Config file: {}",
        args.output.display().to_string().bright_cyan()
    );
    println!("  Duration: {:.1}s", duration.as_secs_f64());
    println!();
    println!(
        "{} GRPO configuration validated and saved.",
        "✓".bright_green()
    );
    println!(
        "  Config uses {} library types (type-safe, deserializable).",
        "GRPOConfig".bright_cyan()
    );
    println!();
    println!(
        "{} See: crates/dashflow/src/optimize/optimizers/grpo.rs for examples",
        "Tip:".bright_yellow()
    );

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_train_method_values() {
        let _distill = TrainMethod::Distill;
        let _finetune = TrainMethod::Finetune;
        let _rl = TrainMethod::Rl;
        let _synthetic = TrainMethod::Synthetic;
    }

    #[test]
    fn test_extract_input_field_direct() {
        let example = serde_json::json!({"input": "What is 2+2?"});
        let result = extract_input_field(&example, "input").unwrap();
        assert_eq!(result, "What is 2+2?");
    }

    #[test]
    fn test_extract_input_field_alternative() {
        let example = serde_json::json!({"question": "What is the capital of France?"});
        let result = extract_input_field(&example, "input").unwrap();
        assert_eq!(result, "What is the capital of France?");
    }

    #[test]
    fn test_estimate_cost() {
        // GPT-4o pricing: $0.0025/1K input, $0.01/1K output
        let cost = estimate_cost("gpt-4o", 1000, 500);
        // 1K input = $0.0025, 0.5K output = $0.005
        assert!((cost - 0.0075).abs() < 0.0001);
    }

    #[test]
    fn test_truncate_str_short() {
        assert_eq!(truncate_str("hello", 10), "hello");
    }

    #[test]
    fn test_truncate_str_long() {
        assert_eq!(truncate_str("hello world", 5), "hello...");
    }

    #[test]
    fn test_extract_json_plain() {
        let response = r#"{"key": "value"}"#;
        assert_eq!(extract_json_from_response(response), r#"{"key": "value"}"#);
    }

    #[test]
    fn test_extract_json_with_whitespace() {
        let response = r#"
        {"key": "value"}
        "#;
        assert_eq!(extract_json_from_response(response), r#"{"key": "value"}"#);
    }

    #[test]
    fn test_extract_json_from_code_block() {
        let response = r#"```json
{"key": "value"}
```"#;
        assert_eq!(extract_json_from_response(response), r#"{"key": "value"}"#);
    }

    #[test]
    fn test_extract_json_from_plain_code_block() {
        let response = r#"```
{"key": "value"}
```"#;
        assert_eq!(extract_json_from_response(response), r#"{"key": "value"}"#);
    }

    #[test]
    fn test_extract_json_with_surrounding_text() {
        // When there's text before the code block
        let response = r#"Here's the generated example:
```json
{"name": "test"}
```"#;
        assert_eq!(extract_json_from_response(response), r#"{"name": "test"}"#);
    }
}
