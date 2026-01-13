// Allow direct OpenAI constructor in example app (factory pattern more useful in production)
#![allow(clippy::disallowed_methods)]

//! Librarian Evaluation - Run evaluation using DashFlow eval framework
//!
//! This binary evaluates the Librarian's search quality using DashFlow's
//! `EvalSuite` dataset format and scoring functions.
//!
//! ## Usage
//!
//! ```bash
//! # Run with default eval suite
//! cargo run -p librarian --bin librarian_eval
//!
//! # Run with custom eval suite
//! cargo run -p librarian --bin librarian_eval -- --eval-suite data/eval_suite.json
//!
//! # Output as JSON
//! cargo run -p librarian --bin librarian_eval -- --format json
//! ```

use anyhow::Result;
use clap::Parser;
use dashflow::core::embeddings::Embeddings;
use dashflow_huggingface::HuggingFaceEmbeddings;
use dashflow_openai::embeddings::OpenAIEmbeddings;
use dashflow_streaming::evals::{
    average_correctness, score_answer, EvalCase, EvalMetrics, EvalSuite, ScoringMethod,
};
use librarian::{search::HybridSearcher, telemetry};
use serde::{Deserialize, Serialize};
use std::env;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;
use tracing::info;

#[derive(Parser)]
#[command(
    author,
    version,
    about = "Evaluate Book Search using DashFlow eval framework"
)]
struct Args {
    /// Path to DashFlow eval suite JSON file
    #[arg(long, default_value = "data/eval_suite.json")]
    eval_suite: PathBuf,

    /// OpenSearch URL
    #[arg(long, default_value = "http://localhost:9200")]
    opensearch_url: String,

    /// Index name
    #[arg(long, default_value = "books")]
    index: String,

    /// Number of search results to retrieve
    #[arg(long, default_value = "5")]
    top_k: usize,

    /// Output format: text (default) or json
    #[arg(long, default_value = "text")]
    format: String,

    /// Scoring method: contains (default), exact, case_insensitive, fuzzy
    #[arg(long, default_value = "contains")]
    scoring: String,
}

/// Result of evaluating a single case
#[derive(Debug, Clone, Serialize, Deserialize)]
struct CaseResult {
    id: String,
    query: String,
    expected_answer: String,
    retrieved_book: Option<String>,
    book_match: bool,
    answer_score: f64,
    latency_ms: u64,
}

/// Summary of evaluation run using DashFlow EvalMetrics
#[derive(Debug, Clone, Serialize, Deserialize)]
struct EvalReport {
    suite_name: String,
    suite_version: String,
    total_cases: usize,
    book_retrieval_accuracy: f64,
    answer_correctness: f64,
    metrics: EvalMetrics,
    results: Vec<CaseResult>,
}

/// Built-in eval suite for fallback
fn default_eval_suite() -> EvalSuite {
    EvalSuite {
        name: "librarian_classics_default".to_string(),
        description: "Built-in evaluation questions for classic literature".to_string(),
        version: "1.0.0".to_string(),
        cases: vec![
            EvalCase {
                id: "pride_001".to_string(),
                query: "What is Mr. Darcy's first name?".to_string(),
                expected_answer: "Fitzwilliam".to_string(),
                metadata: Some(serde_json::json!({
                    "book": "Pride and Prejudice",
                    "difficulty": "easy"
                })),
            },
            EvalCase {
                id: "moby_001".to_string(),
                query: "What is Captain Ahab's ship called?".to_string(),
                expected_answer: "The Pequod".to_string(),
                metadata: Some(serde_json::json!({
                    "book": "Moby Dick",
                    "difficulty": "easy"
                })),
            },
            EvalCase {
                id: "frank_001".to_string(),
                query: "Who created Frankenstein's monster?".to_string(),
                expected_answer: "Victor Frankenstein".to_string(),
                metadata: Some(serde_json::json!({
                    "book": "Frankenstein",
                    "difficulty": "easy"
                })),
            },
            EvalCase {
                id: "alice_001".to_string(),
                query: "What does the White Rabbit carry?".to_string(),
                expected_answer: "A pocket watch".to_string(),
                metadata: Some(serde_json::json!({
                    "book": "Alice's Adventures in Wonderland",
                    "difficulty": "easy"
                })),
            },
            EvalCase {
                id: "sherlock_001".to_string(),
                query: "Where does Sherlock Holmes live?".to_string(),
                expected_answer: "221B Baker Street".to_string(),
                metadata: Some(serde_json::json!({
                    "book": "The Adventures of Sherlock Holmes",
                    "difficulty": "easy"
                })),
            },
            EvalCase {
                id: "tale_001".to_string(),
                query: "What are the two cities in A Tale of Two Cities?".to_string(),
                expected_answer: "London and Paris".to_string(),
                metadata: Some(serde_json::json!({
                    "book": "A Tale of Two Cities",
                    "difficulty": "easy"
                })),
            },
            EvalCase {
                id: "tom_001".to_string(),
                query: "Who is Tom Sawyer's best friend?".to_string(),
                expected_answer: "Huckleberry Finn".to_string(),
                metadata: Some(serde_json::json!({
                    "book": "The Adventures of Tom Sawyer",
                    "difficulty": "easy"
                })),
            },
            EvalCase {
                id: "dracula_001".to_string(),
                query: "What does Count Dracula fear?".to_string(),
                expected_answer: "Sunlight, garlic, crosses, holy water".to_string(),
                metadata: Some(serde_json::json!({
                    "book": "Dracula",
                    "difficulty": "medium"
                })),
            },
            EvalCase {
                id: "war_001".to_string(),
                query: "What happens at the end of War and Peace?".to_string(),
                expected_answer: "Pierre and Natasha marry; the war ends".to_string(),
                metadata: Some(serde_json::json!({
                    "book": "War and Peace",
                    "difficulty": "hard"
                })),
            },
            EvalCase {
                id: "prince_001".to_string(),
                query: "According to Machiavelli, is it better to be feared or loved?".to_string(),
                expected_answer: "It is better to be feared than loved".to_string(),
                metadata: Some(serde_json::json!({
                    "book": "The Prince",
                    "difficulty": "medium"
                })),
            },
        ],
    }
}

/// Parse scoring method from string
fn parse_scoring_method(s: &str) -> ScoringMethod {
    match s.to_lowercase().as_str() {
        "exact" => ScoringMethod::ExactMatch,
        "case_insensitive" => ScoringMethod::CaseInsensitiveMatch,
        "fuzzy" => ScoringMethod::FuzzyMatch,
        _ => ScoringMethod::Contains,
    }
}

/// Get expected book from case metadata
fn get_expected_book(case: &EvalCase) -> Option<String> {
    case.metadata
        .as_ref()
        .and_then(|m| m.get("book"))
        .and_then(|b| b.as_str())
        .map(|s| s.to_string())
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .init();

    // Load eval suite from DashFlow format
    let suite = if args.eval_suite.exists() {
        match EvalSuite::load(&args.eval_suite) {
            Ok(s) => {
                info!(
                    "Loaded eval suite '{}' v{} ({} cases)",
                    s.name,
                    s.version,
                    s.len()
                );
                s
            }
            Err(e) => {
                info!("Failed to load eval suite: {}. Using default.", e);
                default_eval_suite()
            }
        }
    } else {
        info!("Eval suite file not found, using default");
        default_eval_suite()
    };

    // Parse scoring method
    let scoring_method = parse_scoring_method(&args.scoring);
    info!("Using scoring method: {:?}", scoring_method);

    // Initialize embeddings - auto-detect available API key
    let embeddings: Arc<dyn Embeddings> = if env::var("HF_TOKEN").is_ok()
        || env::var("HUGGINGFACEHUB_API_TOKEN").is_ok()
    {
        info!("Using HuggingFace embeddings (HF_TOKEN detected)");
        Arc::new(HuggingFaceEmbeddings::new().with_model("sentence-transformers/all-MiniLM-L6-v2"))
    } else if env::var("OPENAI_API_KEY").is_ok() {
        info!("Using OpenAI embeddings (OPENAI_API_KEY detected)");
        Arc::new(
            OpenAIEmbeddings::new()
                .with_model("text-embedding-3-small")
                .with_dimensions(1024),
        )
    } else {
        anyhow::bail!(
            "No embedding API key found. Set HF_TOKEN or OPENAI_API_KEY environment variable."
        );
    };

    // Create searcher
    let searcher = HybridSearcher::new(embeddings, args.opensearch_url.clone(), args.index.clone());

    // Run evaluation
    let mut results = Vec::new();
    let mut total_book_matches = 0;
    let mut scores = Vec::new();
    let mut latencies = Vec::new();
    let mut errors = 0;

    println!(
        "\nRunning evaluation on '{}' ({} cases)...\n",
        suite.name,
        suite.len()
    );

    for (i, case) in suite.cases.iter().enumerate() {
        let start = Instant::now();

        // Search for the query
        let search_results = match searcher.search(&case.query, args.top_k).await {
            Ok(r) => r,
            Err(e) => {
                info!("Search error for case {}: {}", case.id, e);
                errors += 1;
                continue;
            }
        };

        let latency_ms = start.elapsed().as_millis() as u64;
        latencies.push(latency_ms as f64);

        // Check book retrieval
        let retrieved_book = search_results.first().map(|r| r.title.clone());
        let expected_book = get_expected_book(case);

        let book_match =
            if let (Some(expected), Some(retrieved)) = (&expected_book, &retrieved_book) {
                retrieved.to_lowercase().contains(&expected.to_lowercase())
            } else {
                !search_results.is_empty()
            };

        if book_match {
            total_book_matches += 1;
        }

        // Score answer using DashFlow scoring
        // For retrieval eval, we check if the retrieved content contains the expected answer
        let retrieved_content = search_results
            .first()
            .map(|r| r.content.clone())
            .unwrap_or_default();

        let answer_score = score_answer(&retrieved_content, &case.expected_answer, scoring_method);
        scores.push(answer_score);

        results.push(CaseResult {
            id: case.id.clone(),
            query: case.query.clone(),
            expected_answer: case.expected_answer.clone(),
            retrieved_book,
            book_match,
            answer_score,
            latency_ms,
        });

        // Print progress
        let status = if book_match { "PASS" } else { "FAIL" };
        let score_pct = (answer_score * 100.0) as u32;
        println!(
            "  [{}/{}] {} ({:>3}%) - {} [{}ms]",
            i + 1,
            suite.len(),
            status,
            score_pct,
            case.query,
            latency_ms
        );
    }

    // Calculate metrics using DashFlow functions
    let correctness = average_correctness(&scores);
    let book_accuracy = total_book_matches as f64 / suite.len() as f64;

    // Calculate latency stats
    let avg_latency = latencies.iter().sum::<f64>() / latencies.len().max(1) as f64;
    let mut sorted_latencies = latencies.clone();
    sorted_latencies.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let p95_idx = (sorted_latencies.len() as f64 * 0.95) as usize;
    let p95_latency = sorted_latencies
        .get(p95_idx)
        .copied()
        .unwrap_or(avg_latency);

    // Build DashFlow EvalMetrics
    let metrics = EvalMetrics {
        correctness: Some(correctness),
        relevance: Some(book_accuracy),
        safety: Some(1.0),
        hallucination_rate: None,
        p95_latency,
        avg_latency,
        success_rate: 1.0 - (errors as f64 / suite.len() as f64),
        error_rate: errors as f64 / suite.len() as f64,
        total_tokens: 0,
        cost_per_run: 0.0,
        tool_calls: suite.len() as u64,
    };

    // Record telemetry
    telemetry::record_quality_score(correctness);

    // Build report
    let report = EvalReport {
        suite_name: suite.name.clone(),
        suite_version: suite.version.clone(),
        total_cases: suite.len(),
        book_retrieval_accuracy: book_accuracy,
        answer_correctness: correctness,
        metrics,
        results,
    };

    // Output results
    if args.format == "json" {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        println!("\n╔══════════════════════════════════════════════════════════════════════╗");
        println!(
            "║  Evaluation Results: {} v{}",
            report.suite_name, report.suite_version
        );
        println!("╠══════════════════════════════════════════════════════════════════════╣");
        println!(
            "║  Total cases:           {:>6}                                       ║",
            report.total_cases
        );
        println!(
            "║  Book retrieval:        {:>5.1}%                                       ║",
            report.book_retrieval_accuracy * 100.0
        );
        println!(
            "║  Answer correctness:    {:>5.1}%                                       ║",
            report.answer_correctness * 100.0
        );
        println!("╠══════════════════════════════════════════════════════════════════════╣");
        println!(
            "║  Avg latency:           {:>5.0}ms                                       ║",
            report.metrics.avg_latency
        );
        println!(
            "║  P95 latency:           {:>5.0}ms                                       ║",
            report.metrics.p95_latency
        );
        println!(
            "║  Success rate:          {:>5.1}%                                       ║",
            report.metrics.success_rate * 100.0
        );
        println!("╚══════════════════════════════════════════════════════════════════════╝");
    }

    Ok(())
}
