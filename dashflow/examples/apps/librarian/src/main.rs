// Allow direct OpenAI constructor in example app (factory pattern more useful in production)
#![allow(clippy::disallowed_methods)]

//! Superhuman Librarian CLI - Query the indexed book corpus
//!
//! ## Usage
//!
//! ```bash
//! # Single query
//! cargo run -p librarian -- query "Who is Elizabeth Bennet's love interest?"
//!
//! # Query with author filter
//! cargo run -p librarian -- query "monster" --author "Mary Shelley"
//!
//! # Cross-language search: search in English, find French results
//! cargo run -p librarian -- query "miserable poor people Paris" --multilingual
//!
//! # Fan out search (parallel strategies)
//! cargo run -p librarian -- fan-out "revenge and obsession" --strategies semantic,keyword,hybrid
//!
//! # Interactive chat with memory
//! cargo run -p librarian -- chat --user alice
//!
//! # Add a bookmark
//! cargo run -p librarian -- bookmark add --book "1342" --chunk 50 --note "Important passage"
//!
//! # View reading history
//! cargo run -p librarian -- memory show --user alice
//! ```

use anyhow::Result;
use clap::{Parser, Subcommand};
use dashflow::core::embeddings::Embeddings;
use dashflow::core::utils::sanitize_for_log_default;
use dashflow_huggingface::HuggingFaceEmbeddings;
use dashflow_openai::OpenAIEmbeddings;
use librarian::{
    analysis::BookAnalyzer,
    cost::{CostTracker, EmbeddingModel},
    fan_out::{FanOutSearcher, SearchStrategy},
    introspection::{
        format_analysis, format_improvements, format_trace, introspect_last_search, SearchTrace,
        TraceStore,
    },
    memory::{LibrarianMemory, MemoryManager},
    search::{
        classify_query, recommend_search_mode, BookLength, FilterStore, HybridSearcher,
        SavedFilter, SearchFilters, SelfCorrectionConfig,
    },
    telemetry,
};
use std::env;
use std::io::{self, BufRead, Write};
use std::path::PathBuf;
use std::sync::Arc;
use tracing::info;

#[derive(Parser)]
#[command(
    author,
    version,
    about = "Superhuman Librarian - Hybrid RAG over Gutenberg books"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// OpenSearch URL
    #[arg(long, default_value = "http://localhost:9200", global = true)]
    opensearch_url: String,

    /// Index name
    #[arg(long, default_value = "books", global = true)]
    index: String,

    /// Disable telemetry
    #[arg(long, global = true)]
    no_telemetry: bool,

    /// Use multilingual embeddings for cross-language search
    /// Enables searching in one language (e.g., English) to find results in other languages (e.g., French)
    #[arg(long, global = true)]
    multilingual: bool,
}

#[derive(Subcommand)]
enum Commands {
    /// Execute a search query
    Query {
        /// The search query
        query: String,

        /// Number of results to return
        #[arg(short = 'n', long, default_value = "5")]
        limit: usize,

        /// Filter by author
        #[arg(long)]
        author: Option<String>,

        /// Filter by book title
        #[arg(long)]
        title: Option<String>,

        /// Filter by book ID
        #[arg(long)]
        book_id: Option<String>,

        /// Filter by language (ISO 639-1 code: en, fr, de, es, ru, el, zh, la)
        #[arg(long)]
        language: Option<String>,

        /// Filter by genre (Fiction, Philosophy, Poetry, Drama, Science Fiction, Mystery, Adventure, etc.)
        #[arg(long)]
        genre: Option<String>,

        /// Filter by minimum publication year
        #[arg(long)]
        year_min: Option<i32>,

        /// Filter by maximum publication year
        #[arg(long)]
        year_max: Option<i32>,

        /// Filter by era (e.g., "victorian", "19th century", "ancient", "medieval", "romantic")
        #[arg(long)]
        era: Option<String>,

        /// Filter by book length (short: <15K words, medium: 15K-60K words, long: >60K words)
        #[arg(long)]
        length: Option<String>,

        /// Search mode: hybrid (default), keyword, or semantic
        #[arg(long, default_value = "hybrid")]
        mode: String,

        /// Use DashFlow platform retrievers instead of custom implementation
        /// This demonstrates the incremental migration to platform abstractions
        #[arg(long)]
        use_platform: bool,

        /// Use DashFlow StateGraph to orchestrate the query workflow:
        /// fan_out → analyze → synthesize
        #[arg(long)]
        use_graph: bool,

        /// Auto-route: intelligently select search mode based on query type
        /// (factual -> keyword, conceptual -> semantic, ambiguous -> hybrid)
        #[arg(long)]
        auto: bool,

        /// Self-correct: if no results, automatically broaden/rephrase the query
        #[arg(long)]
        self_correct: bool,

        /// Show faceted results with filter counts (e.g., "42 results in French")
        #[arg(long)]
        facets: bool,

        /// Use a saved filter preset by name
        #[arg(long)]
        preset: Option<String>,

        /// Synthesize a natural language answer from search results using an LLM
        /// Requires OPENAI_API_KEY environment variable
        #[arg(long)]
        synthesize: bool,
    },

    /// Fan out search with multiple parallel strategies
    #[command(name = "fan-out")]
    FanOut {
        /// The search query
        query: String,

        /// Number of results to return
        #[arg(short = 'n', long, default_value = "10")]
        limit: usize,

        /// Strategies to use (comma-separated): semantic, keyword, hybrid
        #[arg(long, default_value = "semantic,keyword,hybrid")]
        strategies: String,

        /// Show timing breakdown
        #[arg(long)]
        show_timing: bool,

        /// Stream results as they arrive (show each strategy's results immediately)
        #[arg(long)]
        stream: bool,
    },

    /// Interactive chat mode with memory
    Chat {
        /// User ID for memory persistence
        #[arg(long, default_value = "default")]
        user: String,

        /// Number of search results per query
        #[arg(short = 'n', long, default_value = "3")]
        limit: usize,
    },

    /// Manage bookmarks
    Bookmark {
        #[command(subcommand)]
        action: BookmarkAction,
    },

    /// Manage reading memory
    Memory {
        #[command(subcommand)]
        action: MemoryAction,
    },

    /// Analyze characters in a book
    Characters {
        /// Book ID (Gutenberg ID)
        book_id: String,

        /// Show relationships between characters
        #[arg(long)]
        relationships: bool,

        /// Output format: text or json
        #[arg(long, default_value = "text")]
        format: String,
    },

    /// Analyze themes in a book
    Themes {
        /// Book ID (Gutenberg ID)
        book_id: String,

        /// Show evidence for each theme
        #[arg(long)]
        with_evidence: bool,

        /// Maximum number of themes to show
        #[arg(short = 'n', long, default_value = "10")]
        limit: usize,

        /// Output format: text or json
        #[arg(long, default_value = "text")]
        format: String,
    },

    /// Full analysis of a book (characters, relationships, themes)
    Analyze {
        /// Book ID (Gutenberg ID)
        book_id: String,

        /// Output format: text or json
        #[arg(long, default_value = "text")]
        format: String,
    },

    /// Show index statistics
    Stats,

    /// Introspect the last search to understand why it succeeded or failed
    Introspect {
        /// Optional query description (e.g., "my last search")
        #[arg(default_value = "last")]
        query: String,
    },

    /// Show execution trace for recent searches
    Trace {
        /// Show the last N traces
        #[arg(long, default_value = "1")]
        last: usize,

        /// Show summary analysis instead of individual traces
        #[arg(long)]
        summary: bool,
    },

    /// View and apply improvement suggestions
    Improve {
        #[command(subcommand)]
        action: ImproveAction,
    },

    /// Show API costs for queries
    Costs {
        #[command(subcommand)]
        action: CostsAction,
    },

    /// Show execution graph (DAG) for the search pipeline
    Graph {
        /// Show the last N search executions
        #[arg(long, default_value = "1")]
        last: usize,

        /// Output format: ascii or dot (Graphviz DOT format)
        #[arg(long, default_value = "ascii")]
        format: String,

        /// Show timing annotations on edges
        #[arg(long)]
        with_timing: bool,
    },

    /// Optimize search prompts using DashFlow's prompt optimization
    Prompt {
        #[command(subcommand)]
        action: PromptAction,
    },

    /// Manage saved search filter presets
    Filters {
        #[command(subcommand)]
        action: FilterAction,
    },
}

#[derive(Subcommand)]
enum FilterAction {
    /// Save current filters as a named preset
    Save {
        /// Name for the filter preset
        name: String,

        /// Optional description
        #[arg(long)]
        description: Option<String>,

        /// Filter by author
        #[arg(long)]
        author: Option<String>,

        /// Filter by language (ISO 639-1 code)
        #[arg(long)]
        language: Option<String>,

        /// Filter by genre
        #[arg(long)]
        genre: Option<String>,

        /// Filter by minimum year
        #[arg(long)]
        year_min: Option<i32>,

        /// Filter by maximum year
        #[arg(long)]
        year_max: Option<i32>,

        /// Filter by era
        #[arg(long)]
        era: Option<String>,

        /// Filter by length (short/medium/long)
        #[arg(long)]
        length: Option<String>,
    },

    /// List all saved filter presets
    List,

    /// Show details of a specific filter preset
    Show {
        /// Name of the filter preset
        name: String,
    },

    /// Delete a saved filter preset
    Delete {
        /// Name of the filter preset to delete
        name: String,
    },
}

#[derive(Subcommand)]
enum BookmarkAction {
    /// Add a new bookmark
    Add {
        /// Book ID
        #[arg(long)]
        book: String,

        /// Chunk index
        #[arg(long)]
        chunk: i64,

        /// Optional note
        #[arg(long)]
        note: Option<String>,

        /// User ID
        #[arg(long, default_value = "default")]
        user: String,
    },

    /// List bookmarks
    List {
        /// User ID
        #[arg(long, default_value = "default")]
        user: String,

        /// Filter by book ID
        #[arg(long)]
        book: Option<String>,
    },
}

#[derive(Subcommand)]
enum MemoryAction {
    /// Show memory for a user
    Show {
        /// User ID
        #[arg(long, default_value = "default")]
        user: String,
    },

    /// Clear memory for a user
    Clear {
        /// User ID
        #[arg(long, default_value = "default")]
        user: String,

        /// Confirm clear
        #[arg(long)]
        confirm: bool,
    },
}

#[derive(Subcommand)]
enum ImproveAction {
    /// Show improvement suggestions
    Suggestions,

    /// Apply an improvement by ID
    Apply {
        /// Improvement ID to apply
        id: usize,
    },

    /// List all improvements (including applied)
    List,

    /// Generate new suggestions based on recent traces
    Generate,
}

#[derive(Subcommand)]
enum CostsAction {
    /// Show cost summary
    Summary,

    /// Show cost breakdown by query type
    Breakdown,

    /// Show recent queries with costs
    Recent {
        /// Number of recent queries to show
        #[arg(short = 'n', long, default_value = "10")]
        limit: usize,
    },

    /// Reset cost tracking
    Reset {
        /// Confirm reset
        #[arg(long)]
        confirm: bool,
    },
}

#[derive(Subcommand)]
enum PromptAction {
    /// Show current search prompts
    Show,

    /// Analyze prompt effectiveness based on search traces
    Analyze,

    /// Suggest optimized prompts based on search patterns
    Suggest {
        /// Number of suggestions to generate
        #[arg(short = 'n', long, default_value = "3")]
        count: usize,
    },

    /// Apply an optimized prompt
    Apply {
        /// Prompt ID to apply
        id: usize,
    },

    /// Reset prompts to defaults
    Reset {
        /// Confirm reset
        #[arg(long)]
        confirm: bool,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // Initialize telemetry - keep handle to flush traces on exit
    let telemetry_handle = if !cli.no_telemetry {
        Some(telemetry::init_telemetry(
            9091,
            Some("http://localhost:4317"),
            "librarian",
        )?)
    } else {
        // Just logging
        tracing_subscriber::fmt()
            .with_env_filter(
                tracing_subscriber::EnvFilter::try_from_default_env()
                    .unwrap_or_else(|_| "info".into()),
            )
            .init();
        None
    };

    // Lazy embeddings initialization - only when needed
    // Commands like Stats, Trace, Introspect don't need embeddings
    let init_embeddings = |multilingual: bool| -> Result<Arc<dyn Embeddings>> {
        if env::var("HF_TOKEN").is_ok() || env::var("HUGGINGFACEHUB_API_TOKEN").is_ok() {
            let model = if multilingual {
                "sentence-transformers/paraphrase-multilingual-MiniLM-L12-v2"
            } else {
                "sentence-transformers/all-MiniLM-L6-v2"
            };
            info!("Using HuggingFace embeddings with model: {}", model);
            if multilingual {
                info!("Cross-language search enabled");
            }
            Ok(Arc::new(HuggingFaceEmbeddings::new().with_model(model)))
        } else if env::var("OPENAI_API_KEY").is_ok() {
            info!("Using OpenAI embeddings (text-embedding-3-small)");
            if multilingual {
                info!("Cross-language search enabled");
            }
            Ok(Arc::new(
                OpenAIEmbeddings::new()
                    .with_model("text-embedding-3-small")
                    .with_dimensions(1024),
            ))
        } else {
            anyhow::bail!(
                "No embedding API key found. Set HF_TOKEN or OPENAI_API_KEY environment variable."
            );
        }
    };

    let create_searcher = |embeddings: Arc<dyn Embeddings>| -> Arc<HybridSearcher> {
        Arc::new(HybridSearcher::new(
            embeddings,
            cli.opensearch_url.clone(),
            cli.index.clone(),
        ))
    };

    // Initialize memory manager with file-based storage
    // Memory is stored in ./data/memory/ directory
    let memory_dir = PathBuf::from("data/memory");
    let memory_manager = Arc::new(MemoryManager::new(memory_dir));

    // Initialize cost tracker
    let cost_dir = PathBuf::from("data/costs");
    let mut cost_tracker = CostTracker::load(&cost_dir)?;

    // Handle commands that don't need embeddings (early return)
    match &cli.command {
        Commands::Stats => {
            let url = format!("{}/{}/_count", cli.opensearch_url, cli.index);
            let client = reqwest::Client::new();
            let response: serde_json::Value = client.get(&url).send().await?.json().await?;
            let count = response.get("count").and_then(|v| v.as_u64()).unwrap_or(0);
            println!("\nIndex Statistics:");
            println!("  Index name: {}", cli.index);
            println!("  Document count: {}", count);
            if let Some(handle) = telemetry_handle {
                handle.shutdown();
            }
            return Ok(());
        }
        Commands::Trace { last, summary } => {
            let store = TraceStore::new(PathBuf::from("data/introspection"))?;
            if *summary {
                println!("{}", format_analysis(&store.analyze()));
            } else {
                let traces = store.recent_traces(*last);
                if traces.is_empty() {
                    println!("No traces found.");
                } else {
                    for t in traces {
                        println!("{}", format_trace(t));
                    }
                }
            }
            if let Some(handle) = telemetry_handle {
                handle.shutdown();
            }
            return Ok(());
        }
        Commands::Introspect { .. } => {
            let store = TraceStore::new(PathBuf::from("data/introspection"))?;
            println!("{}", introspect_last_search(&store)?);
            if let Some(handle) = telemetry_handle {
                handle.shutdown();
            }
            return Ok(());
        }
        Commands::Costs { action } => {
            match action {
                CostsAction::Summary => {
                    println!("\nAPI Cost Summary");
                    println!("================\n");
                    println!("{}", cost_tracker.summary());
                }
                CostsAction::Breakdown => {
                    println!("\nCost Breakdown by Query Type");
                    println!("============================\n");
                    println!("{}", cost_tracker.breakdown());
                }
                CostsAction::Recent { limit } => {
                    println!("\nRecent Queries (last {})", limit);
                    println!("========================\n");
                    for record in cost_tracker.recent_queries(*limit) {
                        println!(
                            "  {} | {} | {} | ${:.6}",
                            record.timestamp.format("%Y-%m-%d %H:%M:%S"),
                            record.query.chars().take(40).collect::<String>(),
                            record.mode,
                            record.cost
                        );
                    }
                }
                CostsAction::Reset { confirm } => {
                    if !*confirm {
                        println!("Use --confirm to reset cost tracking data.");
                    } else {
                        cost_tracker.reset();
                        cost_tracker.save(&cost_dir)?;
                        println!("Cost tracking data has been reset.");
                    }
                }
            }
            if let Some(handle) = telemetry_handle {
                handle.shutdown();
            }
            return Ok(());
        }
        Commands::Graph {
            last,
            format,
            with_timing,
        } => {
            let store = TraceStore::new(PathBuf::from("data/introspection"))?;
            let traces = store.recent_traces(*last);
            if traces.is_empty() {
                println!("No traces found. Run some searches first to generate execution graphs.");
            } else {
                for trace in traces {
                    if format == "dot" {
                        println!("{}", render_graph_dot(trace, *with_timing));
                    } else {
                        println!("{}", render_graph_ascii(trace, *with_timing));
                    }
                }
            }
            if let Some(handle) = telemetry_handle {
                handle.shutdown();
            }
            return Ok(());
        }
        Commands::Improve { action } => {
            let mut store = TraceStore::new(PathBuf::from("data/introspection"))?;
            match action {
                ImproveAction::Suggestions => {
                    let suggestions = store.generate_suggestions();
                    if suggestions.is_empty() {
                        println!(
                            "No suggestions available. Run more searches to generate suggestions."
                        );
                    } else {
                        println!("{}", format_improvements(&suggestions));
                    }
                }
                ImproveAction::Apply { id } => {
                    store.apply_improvement(*id)?;
                    store.save()?;
                    println!("Applied improvement #{}", id);
                    println!(
                        "Note: Some improvements require code changes. Check the description."
                    );
                }
                ImproveAction::List => {
                    let improvements = store.improvements();
                    println!("{}", format_improvements(improvements));
                }
                ImproveAction::Generate => {
                    let suggestions = store.generate_suggestions();
                    for suggestion in suggestions {
                        store.add_improvement(suggestion);
                    }
                    store.save()?;
                    println!(
                        "Generated {} new improvement suggestions.",
                        store.improvements().len()
                    );
                    println!("Use `librarian improve suggestions` to view them.");
                }
            }
            if let Some(handle) = telemetry_handle {
                handle.shutdown();
            }
            return Ok(());
        }
        Commands::Prompt { action } => {
            let store = TraceStore::new(PathBuf::from("data/introspection"))?;
            let prompt_dir = PathBuf::from("data/prompts");
            std::fs::create_dir_all(&prompt_dir)?;

            match action {
                PromptAction::Show => {
                    println!("\nCurrent Search Prompts");
                    println!("======================\n");
                    println!("{}", show_current_prompts(&prompt_dir)?);
                }
                PromptAction::Analyze => {
                    println!("\nPrompt Effectiveness Analysis");
                    println!("==============================\n");
                    println!("{}", analyze_prompt_effectiveness(&store)?);
                }
                PromptAction::Suggest { count } => {
                    println!("\nOptimized Prompt Suggestions");
                    println!("============================\n");
                    let suggestions = suggest_optimized_prompts(&store, *count)?;
                    for (i, suggestion) in suggestions.iter().enumerate() {
                        println!("{}. {}", i + 1, suggestion);
                        println!();
                    }
                }
                PromptAction::Apply { id } => {
                    apply_prompt_suggestion(&prompt_dir, *id)?;
                    println!("Applied prompt suggestion #{}", id);
                }
                PromptAction::Reset { confirm } => {
                    if !*confirm {
                        println!("Use --confirm to reset prompts to defaults.");
                    } else {
                        reset_prompts(&prompt_dir)?;
                        println!("Prompts reset to defaults.");
                    }
                }
            }
            if let Some(handle) = telemetry_handle {
                handle.shutdown();
            }
            return Ok(());
        }
        Commands::Filters { action } => {
            let filter_path = PathBuf::from("data/filters.json");
            let mut store = FilterStore::new(&filter_path);

            match action {
                FilterAction::Save {
                    name,
                    description,
                    author,
                    language,
                    genre,
                    year_min,
                    year_max,
                    era,
                    length,
                } => {
                    let length_filter = length.as_ref().and_then(|l| {
                        let parsed = BookLength::parse(l);
                        if parsed.is_none() {
                            eprintln!(
                                "Warning: Invalid length '{}'. Use: short, medium, or long",
                                l
                            );
                        }
                        parsed
                    });

                    let filters = SearchFilters {
                        author: author.clone(),
                        title: None,
                        book_id: None,
                        language: language.clone(),
                        genre: genre.clone(),
                        year_min: *year_min,
                        year_max: *year_max,
                        era: era.clone(),
                        length: length_filter,
                    };

                    let saved = SavedFilter::new(name, filters, description.as_deref());
                    store.add(saved)?;
                    println!("Saved filter preset '{}'", name);
                }
                FilterAction::List => {
                    let filters = store.list();
                    if filters.is_empty() {
                        println!("No saved filter presets.");
                        println!("\nCreate one with:");
                        println!("  librarian filters save <name> --language en --genre Fiction");
                    } else {
                        println!("\nSaved Filter Presets ({}):\n", filters.len());
                        for filter in filters {
                            println!(
                                "  {} - {}",
                                filter.name,
                                filter.description.as_deref().unwrap_or("(no description)")
                            );
                        }
                        println!(
                            "\nUse a preset: librarian query \"search query\" --preset <name>"
                        );
                    }
                }
                FilterAction::Show { name } => match store.get(name) {
                    Some(filter) => {
                        println!("\nFilter Preset: {}", filter.name);
                        if let Some(desc) = &filter.description {
                            println!("Description: {}", desc);
                        }
                        println!("Created: {}", filter.created_at);
                        println!("\nFilters:");
                        if let Some(v) = &filter.filters.author {
                            println!("  Author: {}", v);
                        }
                        if let Some(v) = &filter.filters.language {
                            println!("  Language: {}", v);
                        }
                        if let Some(v) = &filter.filters.genre {
                            println!("  Genre: {}", v);
                        }
                        if let Some(v) = &filter.filters.year_min {
                            println!("  Year Min: {}", v);
                        }
                        if let Some(v) = &filter.filters.year_max {
                            println!("  Year Max: {}", v);
                        }
                        if let Some(v) = &filter.filters.era {
                            println!("  Era: {}", v);
                        }
                        if let Some(v) = &filter.filters.length {
                            println!("  Length: {}", v);
                        }
                    }
                    None => {
                        eprintln!("Filter preset '{}' not found.", name);
                    }
                },
                FilterAction::Delete { name } => match store.remove(name) {
                    Ok(true) => println!("Deleted filter preset '{}'", name),
                    Ok(false) => eprintln!("Filter preset '{}' not found.", name),
                    Err(e) => eprintln!("Error deleting preset: {}", e),
                },
            }
            if let Some(handle) = telemetry_handle {
                handle.shutdown();
            }
            return Ok(());
        }
        // Handle keyword-only queries without embeddings
        Commands::Query {
            ref query,
            limit,
            ref mode,
            auto: false,
            self_correct: false,
            use_platform,
            ..
        } if mode == "keyword" => {
            if *use_platform {
                // Use DashFlow platform retriever (demonstrates platform integration)
                info!(
                    "Keyword search using DashFlow platform retriever: {}",
                    query
                );
                use dashflow::core::retrievers::Retriever;
                use dashflow_opensearch::OpenSearchBM25Retriever;

                let bm25 = OpenSearchBM25Retriever::from_existing(
                    &cli.index,
                    &cli.opensearch_url,
                    *limit,
                    "content",
                )
                .await?;

                let docs = bm25
                    ._get_relevant_documents(query, None)
                    .await
                    .map_err(|e| anyhow::anyhow!("Platform BM25 search failed: {}", e))?;

                let results: Vec<librarian::SearchResult> = docs
                    .into_iter()
                    .filter_map(|doc| {
                        Some(librarian::SearchResult {
                            content: doc.page_content,
                            title: doc.metadata.get("title")?.as_str()?.to_string(),
                            author: doc.metadata.get("author")?.as_str()?.to_string(),
                            book_id: doc.metadata.get("book_id")?.as_str()?.to_string(),
                            chunk_index: doc.metadata.get("chunk_index")?.as_i64()?,
                            score: 1.0, // Platform retriever doesn't expose scores
                        })
                    })
                    .collect();

                if results.is_empty() {
                    println!("\nNo results found.");
                } else {
                    print_results_with_query(&results, query);
                }
                println!("\n(Used DashFlow platform retriever)");
            } else {
                // Original custom implementation
                // M-235: Sanitize user query to prevent log injection
                info!(query = %sanitize_for_log_default(query), "Keyword-only search (no embeddings required)");

                let client = reqwest::Client::new();
                let search_body = serde_json::json!({
                    "size": limit,
                    "query": {
                        "match": {
                            "content": query
                        }
                    },
                    "_source": ["content", "title", "author", "book_id", "chunk_index"]
                });

                let url = format!("{}/{}/_search", cli.opensearch_url, cli.index);
                let response = client.post(&url).json(&search_body).send().await?;

                if !response.status().is_success() {
                    let error_text = response.text().await?;
                    anyhow::bail!("Search failed: {}", error_text);
                }

                let result: serde_json::Value = response.json().await?;
                let hits = result
                    .get("hits")
                    .and_then(|h| h.get("hits"))
                    .and_then(|h| h.as_array());

                if let Some(hits) = hits {
                    let results: Vec<librarian::SearchResult> = hits
                        .iter()
                        .filter_map(|hit| {
                            let source = hit.get("_source")?;
                            let score = hit.get("_score")?.as_f64()? as f32;
                            Some(librarian::SearchResult {
                                content: source.get("content")?.as_str()?.to_string(),
                                title: source.get("title")?.as_str()?.to_string(),
                                author: source.get("author")?.as_str()?.to_string(),
                                book_id: source.get("book_id")?.as_str()?.to_string(),
                                chunk_index: source.get("chunk_index")?.as_i64()?,
                                score,
                            })
                        })
                        .collect();

                    print_results_with_query(&results, query);
                } else {
                    println!("\nNo results found.");
                }
            }

            if let Some(handle) = telemetry_handle {
                handle.shutdown();
            }
            return Ok(());
        }
        _ => {} // Commands that need embeddings continue below
    }

    // Initialize embeddings for commands that need them
    let embeddings = init_embeddings(cli.multilingual)?;
    let searcher = create_searcher(Arc::clone(&embeddings));

    // Determine embedding model for cost tracking
    let embedding_model =
        if env::var("HF_TOKEN").is_ok() || env::var("HUGGINGFACEHUB_API_TOKEN").is_ok() {
            EmbeddingModel::HuggingFace
        } else {
            EmbeddingModel::OpenAI
        };

    match cli.command {
        Commands::Query {
            query,
            limit,
            author,
            title,
            book_id,
            language,
            genre,
            year_min,
            year_max,
            era,
            length,
            mode,
            use_platform,
            use_graph,
            auto,
            self_correct,
            facets,
            preset,
            synthesize,
        } => {
            // Note: use_platform for keyword-only mode is handled in the early match above
            // Initialize trace store for recording search traces
            let mut trace_store = TraceStore::new(PathBuf::from("data/introspection"))?;
            let search_start = std::time::Instant::now();

            // Determine effective search mode
            let effective_mode = if auto {
                let query_type = classify_query(&query);
                let recommended = recommend_search_mode(query_type);
                println!(
                    "Query classified as: {} → using {} search",
                    query_type, recommended
                );
                recommended.to_string()
            } else {
                mode
            };

            // M-235: Sanitize user query to prevent log injection
            info!(query = %sanitize_for_log_default(&query), mode = %effective_mode, "Searching for query");

            // Track query cost
            cost_tracker.record_query(&query, &effective_mode, embedding_model);

            // Variables to capture results for tracing
            let mut final_results: Vec<librarian::SearchResult> = Vec::new();
            let mut search_error: Option<String> = None;
            let mut synthesized_answer: Option<String> = None;
            let mut used_graph = false;

            if self_correct {
                // Use self-correcting search
                let config = SelfCorrectionConfig::default();
                match searcher
                    .search_with_correction(&query, limit, &config)
                    .await
                {
                    Ok(result) => {
                        if result.corrected {
                            println!("\n[Self-correction] Original query returned no results.");
                            println!("Tried {} variations:", result.attempts);
                            for (i, q) in result.queries_tried.iter().enumerate() {
                                let marker =
                                    if i == result.attempts - 1 && !result.results.is_empty() {
                                        "✓"
                                    } else {
                                        "✗"
                                    };
                                println!("  {} {}", marker, q);
                            }
                        }
                        final_results = result.results.clone();
                        print_results_with_query(&result.results, &query);
                    }
                    Err(e) => {
                        search_error = Some(e.to_string());
                        eprintln!("Search error: {}", e);
                    }
                }
            } else {
                // Build filters - either from preset or from CLI args
                let filters = if let Some(preset_name) = &preset {
                    // Load from preset
                    let filter_path = PathBuf::from("data/filters.json");
                    let filter_store = FilterStore::new(&filter_path);
                    match filter_store.get(preset_name) {
                        Some(saved) => {
                            println!(
                                "Using preset '{}': {}",
                                preset_name,
                                saved.description.as_deref().unwrap_or("(no description)")
                            );
                            saved.filters.clone()
                        }
                        None => {
                            eprintln!("Error: Preset '{}' not found. Use 'librarian filters list' to see available presets.", preset_name);
                            return Ok(());
                        }
                    }
                } else {
                    // Parse length filter if provided
                    let length_filter = length.as_ref().and_then(|l| {
                        let parsed = BookLength::parse(l);
                        if parsed.is_none() {
                            eprintln!(
                                "Warning: Invalid length '{}'. Use: short, medium, or long",
                                l
                            );
                        }
                        parsed
                    });

                    SearchFilters {
                        author,
                        title,
                        book_id,
                        language,
                        genre,
                        year_min,
                        year_max,
                        era,
                        length: length_filter,
                    }
                };

                let filters_are_set = filters.author.is_some()
                    || filters.title.is_some()
                    || filters.book_id.is_some()
                    || filters.language.is_some()
                    || filters.genre.is_some()
                    || filters.year_min.is_some()
                    || filters.year_max.is_some()
                    || filters.era.is_some()
                    || filters.length.is_some();

                if use_graph {
                    if facets
                        || use_platform
                        || preset.is_some()
                        || filters_are_set
                        || effective_mode == "keyword"
                    {
                        eprintln!(
                            "Note: --use-graph currently supports unfiltered, non-faceted searches (no --use-platform/--preset) and non-keyword mode; falling back to non-graph search."
                        );
                    } else {
                        use librarian::workflow::build_query_workflow_graph;

                        let strategies = match effective_mode.as_str() {
                            "semantic" => vec![SearchStrategy::Semantic],
                            _ => vec![
                                SearchStrategy::Semantic,
                                SearchStrategy::Keyword,
                                SearchStrategy::Hybrid,
                            ],
                        };

                        let mut graph_synthesize = synthesize;
                        if graph_synthesize && std::env::var("OPENAI_API_KEY").is_err() {
                            graph_synthesize = false;
                            eprintln!(
                                "Error: --synthesize requires OPENAI_API_KEY environment variable"
                            );
                        }

                        let model = if graph_synthesize {
                            use dashflow::core::config_loader::{ChatModelConfig, SecretReference};
                            use dashflow_openai::build_chat_model;

                            let config = ChatModelConfig::OpenAI {
                                model: "gpt-4o-mini".to_string(),
                                api_key: SecretReference::from_env("OPENAI_API_KEY"),
                                temperature: None,
                                max_tokens: None,
                                base_url: None,
                                organization: None,
                            };

                            match build_chat_model(&config) {
                                Ok(m) => Some(m),
                                Err(e) => {
                                    eprintln!("Failed to initialize chat model: {}", e);
                                    None
                                }
                            }
                        } else {
                            None
                        };

                        let fan_out = Arc::new(FanOutSearcher::new(Arc::clone(&searcher)));
                        match build_query_workflow_graph(&fan_out, model.as_ref()) {
                            Ok(graph) => {
                                let state = librarian::QueryWorkflowState::new(
                                    query.clone(),
                                    limit,
                                    strategies,
                                    graph_synthesize,
                                );

                                match graph.invoke(state).await {
                                    Ok(result) => {
                                        let state = result.state().clone();
                                        used_graph = true;
                                        final_results = state.results.clone();
                                        synthesized_answer = state.answer.clone();
                                        print_results_with_query(&state.results, &query);
                                    }
                                    Err(e) => {
                                        used_graph = true;
                                        search_error = Some(e.to_string());
                                        eprintln!("Search error: {}", e);
                                    }
                                }
                            }
                            Err(e) => {
                                eprintln!("Failed to build query workflow graph: {}", e);
                            }
                        }
                    }
                }

                if facets {
                    // Faceted search with aggregations
                    match searcher.search_with_facets(&query, &filters, limit).await {
                        Ok(result) => {
                            final_results = result.results.clone();
                            print_faceted_results(&result, &query);
                        }
                        Err(e) => {
                            search_error = Some(e.to_string());
                            eprintln!("Search error: {}", e);
                        }
                    }
                } else if !used_graph {
                    // Choose implementation based on --use-platform flag
                    let search_result = if use_platform && effective_mode != "semantic" {
                        // Use DashFlow platform retrievers with MergerRetriever
                        match effective_mode.as_str() {
                            "keyword" => searcher.search_keyword_platform(&query, limit).await,
                            // hybrid mode (default) uses MergerRetriever
                            _ => searcher.search_hybrid_platform(&query, limit).await,
                        }
                    } else {
                        // Use original custom implementation
                        match effective_mode.as_str() {
                            "keyword" => searcher.search_keyword(&query, limit).await,
                            "semantic" => searcher.search_semantic(&query, limit).await,
                            _ => searcher.search_filtered(&query, &filters, limit).await,
                        }
                    };

                    match search_result {
                        Ok(results) => {
                            final_results = results.clone();
                            print_results_with_query(&results, &query);
                            if use_platform && effective_mode != "semantic" {
                                println!(
                                    "\n(Used DashFlow platform retrievers with MergerRetriever)"
                                );
                            }
                        }
                        Err(e) => {
                            search_error = Some(e.to_string());
                            eprintln!("Search error: {}", e);
                        }
                    }
                }
            }

            // Synthesize answer if requested
            if synthesize && !final_results.is_empty() && search_error.is_none() {
                if let Some(answer) = synthesized_answer {
                    println!("\n=== SYNTHESIZED ANSWER ===\n");
                    println!("{}", answer);
                    println!();
                } else if !used_graph {
                    use dashflow::core::config_loader::{ChatModelConfig, SecretReference};
                    use dashflow_openai::build_chat_model;
                    use librarian::AnswerSynthesizer;

                    println!("\n=== SYNTHESIZED ANSWER ===\n");

                    // Check for API key
                    if std::env::var("OPENAI_API_KEY").is_err() {
                        eprintln!(
                            "Error: --synthesize requires OPENAI_API_KEY environment variable"
                        );
                    } else {
                        // Use config-driven model instantiation (non-deprecated)
                        let config = ChatModelConfig::OpenAI {
                            model: "gpt-4o-mini".to_string(),
                            api_key: SecretReference::from_env("OPENAI_API_KEY"),
                            temperature: None,
                            max_tokens: None,
                            base_url: None,
                            organization: None,
                        };
                        let model = match build_chat_model(&config) {
                            Ok(m) => m,
                            Err(e) => {
                                eprintln!("Failed to initialize chat model: {}", e);
                                return Ok(());
                            }
                        };
                        let synthesizer = AnswerSynthesizer::new(model);

                        match synthesizer.synthesize(&query, &final_results).await {
                            Ok(answer) => {
                                println!("{}", answer);
                            }
                            Err(e) => {
                                eprintln!("Synthesis failed: {}", e);
                            }
                        }
                    }
                    println!();
                }
            }

            // Record the search trace
            let duration_ms = search_start.elapsed().as_millis() as u64;
            let relevance_scores: Vec<f32> = final_results.iter().map(|r| r.score).collect();

            let mut trace = SearchTrace::new(&query, &effective_mode)
                .with_results(final_results.len(), relevance_scores)
                .with_duration(duration_ms)
                .with_timing("search", duration_ms);

            // Add intent if auto-routing was used
            if auto {
                let query_type = classify_query(&query);
                trace = trace.with_intent(query_type.to_string());
            }

            // Mark as error if search failed
            if let Some(err) = search_error {
                trace = trace.with_error(err);
            }

            // Save the trace
            trace_store.add_trace(trace);
            trace_store.save()?;

            // Save cost data
            cost_tracker.save(&cost_dir)?;
        }

        Commands::FanOut {
            query,
            limit,
            strategies,
            show_timing,
            stream,
        } => {
            // M-235: Sanitize user query to prevent log injection
            info!(query = %sanitize_for_log_default(&query), "Fan out search");

            let strategies: Vec<SearchStrategy> = strategies
                .split(',')
                .filter_map(|s| match s.trim() {
                    "semantic" => Some(SearchStrategy::Semantic),
                    "keyword" => Some(SearchStrategy::Keyword),
                    "hybrid" => Some(SearchStrategy::Hybrid),
                    _ => None,
                })
                .collect();

            if strategies.is_empty() {
                println!("No valid strategies provided. Use: semantic, keyword, hybrid");
                return Ok(());
            }

            let fan_out = FanOutSearcher::new(searcher);

            if stream {
                // Streaming mode: show results as each strategy completes
                println!("\nStreaming Fan Out Search...");
                println!("============================");

                let (tx, mut rx) = tokio::sync::mpsc::channel(10);
                let strategies_clone = strategies.clone();
                let query_clone = query.clone();

                // Spawn the streaming search
                let search_handle = tokio::spawn(async move {
                    fan_out
                        .search_streaming(&query_clone, strategies_clone, limit, tx)
                        .await
                });

                // Print results as they arrive
                let mut total_results = 0;
                while let Some(strategy_result) = rx.recv().await {
                    println!(
                        "\n[{}] completed in {:?} - {} results",
                        strategy_result.strategy,
                        strategy_result.execution_time,
                        strategy_result.results.len()
                    );

                    for (i, result) in strategy_result.results.iter().take(3).enumerate() {
                        println!(
                            "\n  {}. {} - {} (score: {:.3})",
                            total_results + i + 1,
                            result.title,
                            result.author,
                            result.score
                        );
                        let preview = result.content.chars().take(150).collect::<String>();
                        println!("     {}...", preview);
                    }
                    total_results += strategy_result.results.len().min(3);
                }

                // Wait for search to complete
                search_handle.await??;

                println!("\n[Stream complete]");
            } else {
                // Non-streaming mode: wait for all results
                let result = fan_out.search(&query, strategies, limit).await?;

                println!("\nFan Out Search Results:");
                println!("========================");

                if show_timing {
                    println!("\nTiming breakdown:");
                    for sr in &result.strategy_results {
                        println!(
                            "  {} - {:?} ({} results)",
                            sr.strategy,
                            sr.execution_time,
                            sr.results.len()
                        );
                    }
                    println!("\nTotal time: {:?}", result.total_time);
                    println!("Sequential time: {:?}", result.sequential_time);
                    println!("Speedup: {:.2}x", result.speedup);
                }

                print_results_with_query(&result.results, &query);
            }
        }

        Commands::Chat { user, limit } => {
            println!("Superhuman Librarian Chat");
            println!("=========================");
            println!("Type your questions, or 'quit' to exit.\n");

            let mut memory = memory_manager.load(&user).await?;

            // Show context if there is previous conversation
            if !memory.conversation.is_empty() {
                println!(
                    "(Continuing previous conversation with {} turns)",
                    memory.conversation.len()
                );
            }

            let stdin = io::stdin();
            let mut stdout = io::stdout();

            loop {
                print!("You: ");
                stdout.flush()?;

                let mut input = String::new();
                if stdin.lock().read_line(&mut input)? == 0 {
                    break; // EOF
                }

                let input = input.trim();
                if input.is_empty() {
                    continue;
                }
                if input.eq_ignore_ascii_case("quit") || input.eq_ignore_ascii_case("exit") {
                    break;
                }

                // Search for relevant context
                let results = searcher.search(input, limit).await?;

                // Generate response based on results
                let response = if results.is_empty() {
                    "I couldn't find any relevant passages in the library. Could you try rephrasing your question?".to_string()
                } else {
                    // Build context from search results
                    let mut context = String::new();
                    for result in &results {
                        context.push_str(&format!(
                            "From {} by {} (chunk {}):\n{}\n\n",
                            result.title, result.author, result.chunk_index, result.content
                        ));
                    }

                    format!(
                        "Based on my search, I found {} relevant passages:\n\n{}",
                        results.len(),
                        context
                    )
                };

                println!("\nLibrarian: {}\n", response);

                // Update memory
                let books: Vec<String> = results.iter().map(|r| r.book_id.clone()).collect();
                memory.add_turn(input, &response, books);
                memory.record_search(input, None, results.len());

                // Save memory periodically
                memory_manager.save(&memory).await?;
            }

            println!("\nGoodbye! Your conversation has been saved.");
        }

        Commands::Bookmark { action } => match action {
            BookmarkAction::Add {
                book,
                chunk,
                note,
                user,
            } => {
                let mut memory = memory_manager.load(&user).await?;
                let id = memory.add_bookmark(book.clone(), chunk, note.clone());
                memory_manager.save(&memory).await?;

                println!("Bookmark added!");
                println!("  ID: {}", id);
                println!("  Book: {}", book);
                println!("  Chunk: {}", chunk);
                if let Some(n) = note {
                    println!("  Note: {}", n);
                }
            }

            BookmarkAction::List { user, book } => {
                let memory = memory_manager.load(&user).await?;

                let bookmarks: Vec<_> = memory
                    .bookmarks
                    .iter()
                    .filter(|b| book.as_ref().map_or(true, |id| &b.book_id == id))
                    .collect();

                if bookmarks.is_empty() {
                    println!("No bookmarks found.");
                } else {
                    println!("Bookmarks ({}):\n", bookmarks.len());
                    for bm in bookmarks {
                        println!("  Book: {} | Chunk: {}", bm.book_id, bm.chunk_index);
                        if let Some(note) = &bm.note {
                            println!("  Note: {}", note);
                        }
                        println!("  Created: {}", bm.created);
                        println!();
                    }
                }
            }
        },

        Commands::Memory { action } => match action {
            MemoryAction::Show { user } => {
                let memory = memory_manager.load(&user).await?;

                println!("Memory for user: {}\n", user);
                println!("Conversation turns: {}", memory.conversation.len());
                println!("Books read: {}", memory.reading_progress.len());
                println!("Bookmarks: {}", memory.bookmarks.len());
                println!("Notes: {}", memory.notes.len());
                println!("Recent searches: {}", memory.recent_searches.len());

                if let Some(book) = &memory.current_book {
                    println!("\nCurrently reading: {}", book);
                }

                if !memory.favorite_authors.is_empty() {
                    println!("\nFavorite authors: {}", memory.favorite_authors.join(", "));
                }

                if !memory.recent_searches.is_empty() {
                    println!("\nRecent searches:");
                    for search in memory.recent_searches.iter().rev().take(5) {
                        println!(
                            "  {} ({} results) - {}",
                            search.query, search.result_count, search.timestamp
                        );
                    }
                }
            }

            MemoryAction::Clear { user, confirm } => {
                if !confirm {
                    println!("Use --confirm to clear memory for user: {}", user);
                    return Ok(());
                }

                let memory = LibrarianMemory::new(&user);
                memory_manager.save(&memory).await?;
                println!("Memory cleared for user: {}", user);
            }
        },

        Commands::Characters {
            book_id,
            relationships,
            format,
        } => {
            info!("Analyzing characters for book: {}", book_id);

            let analyzer = BookAnalyzer::new(searcher);
            let mut analysis = analyzer.extract_characters(&book_id).await?;

            if relationships {
                analyzer.find_relationships(&mut analysis).await?;
            }

            if format == "json" {
                println!("{}", serde_json::to_string_pretty(&analysis)?);
            } else {
                print_character_analysis(&analysis, relationships);
            }
        }

        Commands::Themes {
            book_id,
            with_evidence,
            limit,
            format,
        } => {
            info!("Analyzing themes for book: {}", book_id);

            let analyzer = BookAnalyzer::new(searcher);
            let mut analysis = analyzer.extract_themes(&book_id).await?;

            // Limit themes
            analysis.themes.truncate(limit);

            if format == "json" {
                println!("{}", serde_json::to_string_pretty(&analysis)?);
            } else {
                print_theme_analysis(&analysis, with_evidence);
            }
        }

        Commands::Analyze { book_id, format } => {
            info!("Full analysis for book: {}", book_id);

            let analyzer = BookAnalyzer::new(searcher);
            let (character_analysis, theme_analysis) = analyzer.analyze_book(&book_id).await?;

            if format == "json" {
                let combined = serde_json::json!({
                    "characters": character_analysis,
                    "themes": theme_analysis
                });
                println!("{}", serde_json::to_string_pretty(&combined)?);
            } else {
                println!("\n{}", "=".repeat(60));
                println!(
                    "FULL ANALYSIS: {} (ID: {})",
                    character_analysis.book_title, book_id
                );
                println!("{}\n", "=".repeat(60));

                print_character_analysis(&character_analysis, true);
                println!();
                print_theme_analysis(&theme_analysis, true);
            }
        }

        // Commands handled early (no embeddings needed)
        Commands::Stats
        | Commands::Trace { .. }
        | Commands::Introspect { .. }
        | Commands::Costs { .. }
        | Commands::Graph { .. }
        | Commands::Improve { .. }
        | Commands::Prompt { .. }
        | Commands::Filters { .. } => {
            unreachable!("Handled in early match")
        }
    }

    // Shutdown telemetry to flush any pending traces
    if let Some(handle) = telemetry_handle {
        handle.shutdown();
    }

    Ok(())
}

/// Extract a preview around a match, or fall back to start of content
fn extract_preview(content: &str, query: &str, window: usize) -> String {
    let content_lower = content.to_lowercase();
    // Try to find any word from the query (skip short words like "the", "is", etc.)
    for word in query.split_whitespace() {
        if word.len() < 3 {
            continue; // Skip short words
        }
        let word_lower = word.to_lowercase();
        if let Some(pos) = content_lower.find(&word_lower) {
            let start = pos.saturating_sub(window / 2);
            let end = (pos + word.len() + window / 2).min(content.len());
            // Safely extract chars
            let preview: String = content.chars().skip(start).take(end - start).collect();
            let prefix = if start > 0 { "..." } else { "" };
            let suffix = if end < content.len() { "..." } else { "" };
            return format!("{}{}{}", prefix, preview.replace('\n', " "), suffix);
        }
    }
    // Fallback to first 200 chars
    let preview: String = content.chars().take(200).collect();
    format!("{}...", preview.replace('\n', " "))
}

fn print_results_with_query(results: &[librarian::SearchResult], query: &str) {
    if results.is_empty() {
        println!("\nNo results found.");
    } else {
        println!("\nFound {} results:\n", results.len());
        for (i, result) in results.iter().enumerate() {
            println!(
                "{}. {} by {} (score: {:.4})",
                i + 1,
                result.title,
                result.author,
                result.score
            );
            println!(
                "   Book ID: {}, Chunk: {}",
                result.book_id, result.chunk_index
            );
            println!("   Content preview:");

            // Show content around the match if query provided, otherwise first 200 chars
            let preview = extract_preview(&result.content, query, 200);
            println!("   \"{}\"", preview);
            println!();
        }
    }
}

fn print_faceted_results(result: &librarian::FacetedSearchResult, query: &str) {
    // Print facet counts first
    println!("\n=== FILTER COUNTS ===");
    println!("Total matching chunks: {}\n", result.total_hits);

    if !result.facets.languages.is_empty() {
        println!("By Language:");
        for bucket in &result.facets.languages {
            println!("  {} ({} results)", bucket.value, bucket.count);
        }
        println!();
    }

    if !result.facets.genres.is_empty() {
        println!("By Genre:");
        for bucket in &result.facets.genres {
            println!("  {} ({} results)", bucket.value, bucket.count);
        }
        println!();
    }

    if !result.facets.authors.is_empty() {
        println!("By Author (top 10):");
        for bucket in result.facets.authors.iter().take(10) {
            println!("  {} ({} results)", bucket.value, bucket.count);
        }
        println!();
    }

    if !result.facets.eras.is_empty() {
        println!("By Era:");
        for bucket in &result.facets.eras {
            println!("  {} ({} results)", bucket.value, bucket.count);
        }
        println!();
    }

    if !result.facets.lengths.is_empty() {
        println!("By Length:");
        for bucket in &result.facets.lengths {
            println!("  {} ({} results)", bucket.value, bucket.count);
        }
        println!();
    }

    // Print the actual results
    println!("=== SEARCH RESULTS ===");
    print_results_with_query(&result.results, query);
}

fn print_character_analysis(analysis: &librarian::CharacterAnalysis, show_relationships: bool) {
    println!(
        "\nCHARACTERS in {} (ID: {})",
        analysis.book_title, analysis.book_id
    );
    println!("{}", "-".repeat(50));

    if analysis.characters.is_empty() {
        println!("No known characters found for this book.");
        println!("(Character data is pre-defined for major classic works)");
        return;
    }

    for (i, character) in analysis.characters.iter().enumerate() {
        println!(
            "\n{}. {} ({} mentions)",
            i + 1,
            character.name,
            character.mention_count
        );

        if !character.aliases.is_empty() {
            let aliases: Vec<_> = character
                .aliases
                .iter()
                .filter(|a| *a != &character.name)
                .collect();
            if !aliases.is_empty() {
                println!(
                    "   Also known as: {}",
                    aliases
                        .iter()
                        .map(|s| s.as_str())
                        .collect::<Vec<_>>()
                        .join(", ")
                );
            }
        }

        // Show a sample evidence
        if let Some(evidence) = character.evidence_chunks.first() {
            let preview: String = evidence.content.chars().take(150).collect();
            println!("   Sample: \"{}...\"", preview.replace('\n', " "));
        }
    }

    if show_relationships && !analysis.relationships.is_empty() {
        println!("\n\nRELATIONSHIPS");
        println!("{}", "-".repeat(50));

        for rel in &analysis.relationships {
            println!(
                "\n{} <-> {} ({})",
                rel.character1, rel.character2, rel.relationship_type
            );
            println!("   {}", rel.description);

            if let Some(evidence) = rel.evidence.first() {
                let preview: String = evidence.content.chars().take(120).collect();
                println!("   Evidence: \"{}...\"", preview.replace('\n', " "));
            }
        }
    }
}

fn print_theme_analysis(analysis: &librarian::ThemeAnalysis, show_evidence: bool) {
    println!(
        "\nTHEMES in {} (ID: {})",
        analysis.book_title, analysis.book_id
    );
    println!("{}", "-".repeat(50));

    if analysis.themes.is_empty() {
        println!("No themes found. Make sure the book is indexed.");
        return;
    }

    for (i, theme) in analysis.themes.iter().enumerate() {
        println!(
            "\n{}. {} (relevance: {:.2})",
            i + 1,
            theme.name,
            theme.relevance_score
        );
        println!("   {}", theme.description);
        println!(
            "   Keywords: {}",
            theme
                .keywords
                .iter()
                .take(5)
                .cloned()
                .collect::<Vec<_>>()
                .join(", ")
        );

        if show_evidence && !theme.evidence.is_empty() {
            println!("   Evidence:");
            for (j, evidence) in theme.evidence.iter().take(2).enumerate() {
                let preview: String = evidence.content.chars().take(100).collect();
                println!(
                    "     {}. (chunk {}) \"{}...\"",
                    j + 1,
                    evidence.chunk_index,
                    preview.replace('\n', " ")
                );
            }
        }
    }
}

/// Render search execution as ASCII DAG
fn render_graph_ascii(trace: &librarian::introspection::SearchTrace, with_timing: bool) -> String {
    let mut output = String::new();
    output.push_str(&format!(
        "\nExecution Graph: {} ({})\n",
        trace.query,
        trace.timestamp.format("%Y-%m-%d %H:%M:%S")
    ));
    output.push_str(&format!("{}\n\n", "=".repeat(60)));

    // Define the pipeline stages based on the trace
    let stages = build_pipeline_stages(trace);

    // Render as ASCII DAG
    for (i, stage) in stages.iter().enumerate() {
        // Stage box
        let timing = if with_timing {
            format!(" ({}ms)", stage.duration_ms)
        } else {
            String::new()
        };

        let box_content = format!("{}{}", stage.name, timing);
        let box_width = box_content.len() + 4;

        output.push_str(&format!("    +{}+\n", "-".repeat(box_width)));
        output.push_str(&format!("    | {} |\n", box_content));
        output.push_str(&format!("    +{}+\n", "-".repeat(box_width)));

        // Show stage details if available
        if !stage.details.is_empty() {
            output.push_str(&format!("    | {}\n", stage.details));
        }

        // Arrow to next stage
        if i < stages.len() - 1 {
            output.push_str("         |\n");
            output.push_str("         v\n");
        }
    }

    // Summary
    output.push_str(&format!(
        "\nTotal: {}ms | Results: {} | Avg Relevance: {:.2}\n",
        trace.duration_ms, trace.result_count, trace.avg_relevance
    ));

    output
}

/// Pipeline stage for visualization
struct PipelineStage {
    name: String,
    duration_ms: u64,
    details: String,
}

/// Build pipeline stages from a search trace
fn build_pipeline_stages(trace: &librarian::introspection::SearchTrace) -> Vec<PipelineStage> {
    let mut stages = Vec::new();

    // Input stage
    stages.push(PipelineStage {
        name: "Input Query".to_string(),
        duration_ms: 0,
        details: format!("\"{}\"", trace.query.chars().take(40).collect::<String>()),
    });

    // Query classification (if available)
    if let Some(intent) = &trace.intent {
        stages.push(PipelineStage {
            name: "Query Classification".to_string(),
            duration_ms: trace
                .timing_breakdown
                .get("classification")
                .copied()
                .unwrap_or(0),
            details: format!("Intent: {}", intent),
        });
    }

    // Embedding generation
    stages.push(PipelineStage {
        name: "Embedding Generation".to_string(),
        duration_ms: trace
            .timing_breakdown
            .get("embedding")
            .copied()
            .unwrap_or(0),
        details: String::new(),
    });

    // Search execution based on strategy
    let search_stage_name = match trace.strategy.as_str() {
        "semantic" => "Semantic Search (Vector)",
        "keyword" => "Keyword Search (BM25)",
        "hybrid" => "Hybrid Search (Vector + BM25)",
        "fan_out" => "Fan-Out Search (Parallel)",
        _ => "Search Execution",
    };

    stages.push(PipelineStage {
        name: search_stage_name.to_string(),
        duration_ms: trace
            .timing_breakdown
            .get("search")
            .copied()
            .unwrap_or(trace.duration_ms),
        details: String::new(),
    });

    // Result ranking
    stages.push(PipelineStage {
        name: "Result Ranking".to_string(),
        duration_ms: trace.timing_breakdown.get("ranking").copied().unwrap_or(0),
        details: format!("{} results", trace.result_count),
    });

    // Output
    stages.push(PipelineStage {
        name: "Output".to_string(),
        duration_ms: 0,
        details: if trace.success { "Success" } else { "Failed" }.to_string(),
    });

    stages
}

/// Render search execution as Graphviz DOT format
fn render_graph_dot(trace: &librarian::introspection::SearchTrace, with_timing: bool) -> String {
    let mut output = String::new();
    output.push_str("digraph SearchPipeline {\n");
    output.push_str("    rankdir=TB;\n");
    output.push_str("    node [shape=box, style=filled, fillcolor=lightblue];\n");
    output.push('\n');

    let stages = build_pipeline_stages(trace);

    // Define nodes
    for (i, stage) in stages.iter().enumerate() {
        let label = if with_timing && stage.duration_ms > 0 {
            format!("{}\\n({}ms)", stage.name, stage.duration_ms)
        } else {
            stage.name.clone()
        };
        output.push_str(&format!("    stage{} [label=\"{}\"];\n", i, label));
    }

    output.push('\n');

    // Define edges
    for i in 0..stages.len() - 1 {
        output.push_str(&format!("    stage{} -> stage{};\n", i, i + 1));
    }

    output.push_str("}\n");
    output
}

/// Show current search prompts
fn show_current_prompts(prompt_dir: &std::path::Path) -> Result<String> {
    let mut output = String::new();

    // Default prompts
    let default_prompts = vec![
        ("Query Expansion", "Given a user query, expand it with synonyms and related terms to improve search coverage."),
        ("Result Reranking", "Given search results and a query, rerank the results by relevance to the user's intent."),
        ("Answer Synthesis", "Given relevant passages and a question, synthesize a comprehensive answer."),
    ];

    // Check for custom prompts
    let custom_prompt_file = prompt_dir.join("custom_prompts.json");
    let custom_prompts: std::collections::HashMap<String, String> = if custom_prompt_file.exists() {
        let content = std::fs::read_to_string(&custom_prompt_file)?;
        serde_json::from_str(&content).unwrap_or_default()
    } else {
        std::collections::HashMap::new()
    };

    for (name, default) in &default_prompts {
        output.push_str(&format!("{}:\n", name));
        output.push_str(&format!("{}\n", "-".repeat(40)));

        if let Some(custom) = custom_prompts.get(*name) {
            output.push_str(&format!("  [CUSTOM] {}\n", custom));
        } else {
            output.push_str(&format!("  [DEFAULT] {}\n", default));
        }
        output.push('\n');
    }

    Ok(output)
}

/// Analyze prompt effectiveness based on traces
fn analyze_prompt_effectiveness(store: &TraceStore) -> Result<String> {
    let mut output = String::new();

    let traces = store.traces();
    if traces.is_empty() {
        return Ok(
            "No search traces found. Run some searches to analyze prompt effectiveness."
                .to_string(),
        );
    }

    // Analyze by strategy
    let mut strategy_stats: std::collections::HashMap<String, (usize, f32, u64)> =
        std::collections::HashMap::new();

    for trace in traces {
        let entry = strategy_stats
            .entry(trace.strategy.clone())
            .or_insert((0, 0.0, 0));
        entry.0 += 1;
        entry.1 += trace.avg_relevance;
        entry.2 += trace.duration_ms;
    }

    output.push_str("Strategy Performance:\n\n");
    output.push_str(&format!(
        "{:<15} {:>8} {:>12} {:>12}\n",
        "Strategy", "Count", "Avg Rel", "Avg Latency"
    ));
    output.push_str(&format!("{}\n", "-".repeat(50)));

    for (strategy, (count, total_rel, total_latency)) in &strategy_stats {
        let avg_rel = total_rel / *count as f32;
        let avg_latency = total_latency / *count as u64;
        output.push_str(&format!(
            "{:<15} {:>8} {:>12.2} {:>10}ms\n",
            strategy, count, avg_rel, avg_latency
        ));
    }

    // Query pattern analysis
    output.push_str("\n\nQuery Pattern Analysis:\n\n");

    let low_relevance_count = traces.iter().filter(|t| t.avg_relevance < 0.5).count();
    let no_result_count = traces.iter().filter(|t| t.result_count == 0).count();

    output.push_str(&format!(
        "  Low relevance queries (<0.5): {} ({:.1}%)\n",
        low_relevance_count,
        (low_relevance_count as f32 / traces.len() as f32) * 100.0
    ));
    output.push_str(&format!(
        "  Zero result queries: {} ({:.1}%)\n",
        no_result_count,
        (no_result_count as f32 / traces.len() as f32) * 100.0
    ));

    // Recommendations
    output.push_str("\n\nRecommendations:\n\n");

    if low_relevance_count > traces.len() / 4 {
        output.push_str("  - Consider using query expansion to improve relevance\n");
    }
    if no_result_count > traces.len() / 10 {
        output.push_str("  - Enable self-correction (--self-correct) to handle edge cases\n");
    }

    let hybrid_stats = strategy_stats.get("hybrid");
    let semantic_stats = strategy_stats.get("semantic");

    if let (Some((_, hybrid_rel, _)), Some((_, semantic_rel, _))) = (hybrid_stats, semantic_stats) {
        let hybrid_avg = hybrid_rel / hybrid_stats.map(|(c, _, _)| *c as f32).unwrap_or(1.0);
        let semantic_avg = semantic_rel / semantic_stats.map(|(c, _, _)| *c as f32).unwrap_or(1.0);

        if semantic_avg > hybrid_avg + 0.1 {
            output.push_str(
                "  - Semantic search outperforming hybrid - consider defaulting to semantic\n",
            );
        }
    }

    Ok(output)
}

/// Suggest optimized prompts based on search patterns
fn suggest_optimized_prompts(store: &TraceStore, count: usize) -> Result<Vec<String>> {
    let traces = store.traces();
    let mut suggestions = Vec::new();

    if traces.is_empty() {
        suggestions.push("Run searches to generate optimization suggestions.".to_string());
        return Ok(suggestions);
    }

    // Analyze common patterns in low-relevance queries
    let low_rel_queries: Vec<_> = traces
        .iter()
        .filter(|t| t.avg_relevance < 0.5)
        .map(|t| &t.query)
        .collect();

    // Suggestion 1: Query expansion prompt
    if !low_rel_queries.is_empty() {
        suggestions.push(format!(
            "Query Expansion Prompt:\n   \"Expand the query '{{query}}' with:\n   1. Synonyms for key terms\n   2. Related concepts\n   3. Alternative phrasings\n   Return expanded query.\"\n   \n   Rationale: {} queries had low relevance - expansion may help.",
            low_rel_queries.len()
        ));
    }

    // Suggestion 2: Intent clarification
    let ambiguous_queries = traces
        .iter()
        .filter(|t| {
            t.intent.is_none() || t.intent.as_ref().is_some_and(|i| i == "ambiguous")
        })
        .count();

    if ambiguous_queries > traces.len() / 5 {
        suggestions.push(format!(
            "Intent Clarification Prompt:\n   \"Classify the query '{{query}}' into:\n   - factual (who/what/when)\n   - conceptual (why/how/explain)\n   - comparative (compare/contrast)\n   Return the intent type.\"\n   \n   Rationale: {} queries were ambiguous.",
            ambiguous_queries
        ));
    }

    // Suggestion 3: Result filtering
    let high_result_count = traces.iter().filter(|t| t.result_count > 20).count();

    if high_result_count > traces.len() / 4 {
        suggestions.push(format!(
            "Result Filtering Prompt:\n   \"Given {} search results for '{{query}}',\n   identify the top 5 most relevant based on:\n   1. Direct answer presence\n   2. Source authority\n   3. Recency\"\n   \n   Rationale: {} queries returned many results.",
            traces.iter().map(|t| t.result_count).sum::<usize>() / traces.len(),
            high_result_count
        ));
    }

    // Ensure we have at least the requested count
    while suggestions.len() < count {
        suggestions.push(
            "Additional optimization: Enable hybrid search for balanced results.".to_string(),
        );
    }

    suggestions.truncate(count);
    Ok(suggestions)
}

/// Apply a prompt suggestion
fn apply_prompt_suggestion(prompt_dir: &std::path::Path, id: usize) -> Result<()> {
    let suggestion_file = prompt_dir.join("suggestion_history.json");
    let custom_file = prompt_dir.join("custom_prompts.json");

    // Load existing suggestions if any
    let suggestions: Vec<String> = if suggestion_file.exists() {
        let content = std::fs::read_to_string(&suggestion_file)?;
        serde_json::from_str(&content).unwrap_or_default()
    } else {
        Vec::new()
    };

    if id == 0 || id > suggestions.len() {
        anyhow::bail!(
            "Invalid suggestion ID: {}. Run 'librarian prompt suggest' first.",
            id
        );
    }

    // Load current custom prompts
    let mut custom_prompts: std::collections::HashMap<String, String> = if custom_file.exists() {
        let content = std::fs::read_to_string(&custom_file)?;
        serde_json::from_str(&content).unwrap_or_default()
    } else {
        std::collections::HashMap::new()
    };

    // Apply the suggestion (extract prompt name from suggestion)
    let suggestion = &suggestions[id - 1];
    let prompt_name = if suggestion.contains("Query Expansion") {
        "Query Expansion"
    } else if suggestion.contains("Intent Clarification") {
        "Intent Clarification"
    } else {
        "Result Filtering"
    };

    custom_prompts.insert(prompt_name.to_string(), suggestion.clone());

    // Save
    let content = serde_json::to_string_pretty(&custom_prompts)?;
    std::fs::write(&custom_file, content)?;

    Ok(())
}

/// Reset prompts to defaults
fn reset_prompts(prompt_dir: &std::path::Path) -> Result<()> {
    let custom_file = prompt_dir.join("custom_prompts.json");
    if custom_file.exists() {
        std::fs::remove_file(&custom_file)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use std::collections::HashMap;

    fn create_test_trace(
        query: &str,
        strategy: &str,
        results: usize,
        relevance: f32,
    ) -> librarian::introspection::SearchTrace {
        librarian::introspection::SearchTrace {
            id: uuid::Uuid::new_v4().to_string(),
            timestamp: Utc::now(),
            query: query.to_string(),
            strategy: strategy.to_string(),
            result_count: results,
            relevance_scores: vec![relevance],
            avg_relevance: relevance,
            duration_ms: 100,
            timing_breakdown: HashMap::from([
                ("embedding".to_string(), 20),
                ("search".to_string(), 70),
                ("ranking".to_string(), 10),
            ]),
            success: results > 0,
            error: None,
            filters: None,
            intent: Some("factual".to_string()),
        }
    }

    #[test]
    fn test_build_pipeline_stages() {
        let trace = create_test_trace("test query", "hybrid", 5, 0.75);
        let stages = build_pipeline_stages(&trace);

        // Should have: Input, Classification, Embedding, Search, Ranking, Output = 6 stages
        assert_eq!(stages.len(), 6);
        assert_eq!(stages[0].name, "Input Query");
        assert_eq!(stages[1].name, "Query Classification");
        assert_eq!(stages[2].name, "Embedding Generation");
        assert_eq!(stages[3].name, "Hybrid Search (Vector + BM25)");
        assert_eq!(stages[4].name, "Result Ranking");
        assert_eq!(stages[5].name, "Output");
    }

    #[test]
    fn test_render_graph_ascii() {
        let trace = create_test_trace("Who is Elizabeth Bennet?", "semantic", 3, 0.85);
        let output = render_graph_ascii(&trace, false);

        // Check that output contains key sections
        assert!(output.contains("Execution Graph:"));
        assert!(output.contains("Input Query"));
        assert!(output.contains("Semantic Search (Vector)"));
        assert!(output.contains("Output"));
        assert!(output.contains("Total:"));
    }

    #[test]
    fn test_render_graph_ascii_with_timing() {
        let trace = create_test_trace("whale hunting", "keyword", 10, 0.60);
        let output = render_graph_ascii(&trace, true);

        // With timing, should show ms values
        assert!(output.contains("ms)"));
        assert!(output.contains("Keyword Search (BM25)"));
    }

    #[test]
    fn test_render_graph_dot() {
        let trace = create_test_trace("themes of revenge", "hybrid", 7, 0.70);
        let output = render_graph_dot(&trace, false);

        // Check DOT format structure
        assert!(output.starts_with("digraph SearchPipeline {"));
        assert!(output.contains("rankdir=TB"));
        assert!(output.contains("->"));
        assert!(output.ends_with("}\n"));
    }

    #[test]
    fn test_render_graph_dot_with_timing() {
        let trace = create_test_trace("Darcy proposal", "semantic", 2, 0.90);
        let output = render_graph_dot(&trace, true);

        // With timing, labels should include ms
        assert!(output.contains("\\n(")); // newline in DOT label
    }

    #[test]
    fn test_pipeline_stages_no_intent() {
        let mut trace = create_test_trace("test", "keyword", 1, 0.5);
        trace.intent = None; // Remove intent classification

        let stages = build_pipeline_stages(&trace);

        // Without intent, should have 5 stages (no classification)
        assert_eq!(stages.len(), 5);
        assert_eq!(stages[1].name, "Embedding Generation"); // Skips classification
    }

    #[test]
    fn test_pipeline_stages_fan_out() {
        let trace = create_test_trace("test", "fan_out", 15, 0.65);
        let stages = build_pipeline_stages(&trace);

        // Fan-out should show Fan-Out Search
        let search_stage_name = stages
            .iter()
            .find(|stage| stage.name.contains("Fan-Out"))
            .map(|stage| stage.name.as_str());
        assert_eq!(search_stage_name, Some("Fan-Out Search (Parallel)"));
    }
}
