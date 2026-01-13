// Allow direct OpenAI constructor in example app (factory pattern more useful in production)
#![allow(clippy::disallowed_methods)]

//! Librarian Indexer - Download and index Gutenberg books
//!
//! ## Usage
//!
//! ```bash
//! # Index 10 classic books (quick start)
//! cargo run -p librarian --bin indexer -- --preset quick
//!
//! # Index 50 classics
//! cargo run -p librarian --bin indexer -- --preset classics
//!
//! # Index specific book IDs
//! cargo run -p librarian --bin indexer -- --book-ids 1342,2701,84
//!
//! # List available presets
//! cargo run -p librarian --bin indexer -- --list-presets
//! ```

use anyhow::Result;
use clap::Parser;
use dashflow::core::embeddings::Embeddings;
use dashflow_huggingface::HuggingFaceEmbeddings;
use dashflow_openai::OpenAIEmbeddings;
use librarian::{
    catalog::GutenbergCatalog,
    config::{get_book_metadata, BookMetadata, BookPreset, BookSearchConfig},
    indexer::IndexerPipeline,
    telemetry,
};
use std::env;
use std::path::PathBuf;
use std::sync::Arc;
use tracing::info;

#[derive(Parser)]
#[command(author, version, about = "Index Gutenberg books into OpenSearch")]
struct Args {
    /// Preset book collection: quick (10 books), classics (50 books)
    #[arg(long)]
    preset: Option<String>,

    /// Comma-separated list of Gutenberg book IDs
    #[arg(long, value_delimiter = ',')]
    book_ids: Option<Vec<u32>>,

    /// OpenSearch URL
    #[arg(long, default_value = "http://localhost:9200")]
    opensearch_url: String,

    /// Index name
    #[arg(long, default_value = "books")]
    index: String,

    /// Chunk size for text splitting
    #[arg(long, default_value = "1000")]
    chunk_size: usize,

    /// Chunk overlap
    #[arg(long, default_value = "200")]
    chunk_overlap: usize,

    /// Local cache directory for downloaded books
    #[arg(long, default_value = "data/gutenberg")]
    cache_dir: PathBuf,

    /// List available presets and exit
    #[arg(long)]
    list_presets: bool,

    /// Dry run - show what would be indexed
    #[arg(long)]
    dry_run: bool,

    /// Maximum number of books to fetch (useful for testing with gutenberg preset)
    #[arg(long)]
    max_books: Option<usize>,

    /// Skip creating index (assume it exists)
    #[arg(long)]
    skip_create_index: bool,

    /// Disable telemetry
    #[arg(long)]
    no_telemetry: bool,

    /// Embedding provider: "huggingface" (requires HF_TOKEN), "openai" (requires OPENAI_API_KEY), or "auto"
    #[arg(long, default_value = "auto")]
    embeddings: String,

    /// Auto-detect language for books with unknown/default language
    #[arg(long)]
    detect_language: bool,

    /// Incremental indexing - only index books not already in the index
    #[arg(long)]
    incremental: bool,

    /// Use multilingual embedding model for cross-language search
    /// Enables searching in one language (e.g., English) to find results in other languages (e.g., French)
    #[arg(long)]
    multilingual: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    // Initialize telemetry - keep handle to flush traces on exit
    let telemetry_handle = if !args.no_telemetry {
        Some(telemetry::init_telemetry(
            9091,
            Some("http://localhost:4317"),
            "librarian_indexer",
        )?)
    } else {
        tracing_subscriber::fmt()
            .with_env_filter(
                tracing_subscriber::EnvFilter::try_from_default_env()
                    .unwrap_or_else(|_| "info".into()),
            )
            .init();
        None
    };

    // List presets
    if args.list_presets {
        println!("\nAvailable Presets:\n");
        println!("  quick    - 10 famous books (~5MB, ~5K chunks)");
        println!("             Perfect for quick testing and demos");
        println!();
        println!("  classics - 50 classic books (~25MB, ~25K chunks)");
        println!("             Comprehensive coverage of literary canon");
        println!();
        println!("  full     - 135 classic books (~70MB, ~70K chunks)");
        println!("             Extended coverage: Victorian, American, Russian, French literature");
        println!("             Philosophy, poetry, plays, mystery, adventure, and more");
        println!();
        println!("  massive  - 1000+ books (~500MB, ~500K chunks)");
        println!("             COMPREHENSIVE library: Complete works of major authors");
        println!("             Shakespeare, Dickens, Twain, London, Poe, Hardy, Trollope, Wells,");
        println!("             Doyle, Hawthorne, James, Eliot, Conrad, Dumas, Hugo, Scott,");
        println!("             Stevenson, plus philosophy, ancient classics, religious texts,");
        println!("             scientific works, poetry, drama, mystery, sci-fi, and more");
        println!();
        println!("  multilingual - 80+ books in original languages (~60MB)");
        println!(
            "             French: Hugo, Dumas, Verne, Proust, Flaubert, Balzac, Zola, Molière"
        );
        println!("             German: Goethe, Kafka, Nietzsche, Mann, Schiller, Lessing");
        println!("             Spanish: Cervantes, Rojas, Calderón, Galdós");
        println!("             Italian: Dante, Machiavelli, Manzoni, Ariosto, Petrarca");
        println!("             Portuguese: Camões, Machado de Assis, Eça de Queirós");
        println!("             Latin: Ovid, Caesar, Cicero, Tacitus, Virgil, Augustine");
        println!();
        println!("  gutenberg  - ALL ~70,000 English text books from Project Gutenberg");
        println!("             Uses dynamic catalog from gutenberg.org (fetches on demand)");
        println!("             Use --max-books N for testing with limited book count");
        println!("             Example: --preset gutenberg --max-books 10000 --dry-run");
        println!();
        println!("\nUsage:");
        println!("  cargo run -p librarian --bin indexer -- --preset quick");
        println!(
            "  cargo run -p librarian --bin indexer -- --preset multilingual --detect-language"
        );
        println!("  cargo run -p librarian --bin indexer -- --preset massive");
        println!("  cargo run -p librarian --bin indexer -- --book-ids 1342,2701");
        println!();
        println!("\nCross-Language Search:");
        println!("  Use --multilingual flag to enable searching in one language to find results in other languages.");
        println!("  This uses multilingual embedding models that map concepts across languages.");
        println!();
        println!("  Example: Index multilingual books with cross-language search enabled:");
        println!("    cargo run -p librarian --bin indexer -- --preset multilingual --multilingual --detect-language");
        println!();
        println!("  Then search in English to find French results:");
        println!(
            "    cargo run -p librarian -- query \"miserable poor people Paris\" --multilingual"
        );
        println!("    (Will find \"Les Misérables\" by Victor Hugo even though it's in French)");
        return Ok(());
    }

    // Determine which books to index
    let books: Vec<BookMetadata> = if let Some(ids) = args.book_ids {
        ids.into_iter()
            .map(get_book_metadata) // Use known metadata if available, else fallback to generic
            .collect()
    } else if let Some(preset_name) = &args.preset {
        let preset: BookPreset = preset_name
            .parse()
            .map_err(|e: String| anyhow::anyhow!(e))?;

        // Handle dynamic catalog presets (e.g., Gutenberg)
        if preset.uses_dynamic_catalog() {
            info!("Loading books from Gutenberg catalog...");
            let cache_dir = args.cache_dir.join("catalog");
            let mut catalog = GutenbergCatalog::new(&cache_dir).await?;

            // Refresh book list if empty or need more than cached
            let cached = catalog.book_ids().len();
            let need_refresh = cached == 0 || args.max_books.is_some_and(|m| cached < m);

            if need_refresh {
                let count = catalog.refresh_book_list_limited(args.max_books).await?;
                info!("Found {} English text books", count);
            } else {
                info!("Using cached: {} books", cached);
            }

            // Convert to metadata, respecting max_books limit
            let ids: Vec<u32> = if let Some(max) = args.max_books {
                catalog.book_ids().iter().take(max).copied().collect()
            } else {
                catalog.book_ids().to_vec()
            };

            ids.iter()
                .map(|&id| get_book_metadata(id)) // Use known metadata if available
                .collect()
        } else {
            preset.books_with_metadata()
        }
    } else {
        anyhow::bail!("Specify --preset or --book-ids. Use --list-presets to see options.");
    };

    println!("\n╔══════════════════════════════════════════════════════════════╗");
    println!("║              Book Search Indexer                             ║");
    println!("╠══════════════════════════════════════════════════════════════╣");
    println!(
        "║  Books to index:   {:>4}                                      ║",
        books.len()
    );
    println!(
        "║  Chunk size:       {:>5} chars                               ║",
        args.chunk_size
    );
    println!(
        "║  Chunk overlap:    {:>5} chars                               ║",
        args.chunk_overlap
    );
    println!(
        "║  Index:            {:>20}                   ║",
        args.index
    );
    println!("╚══════════════════════════════════════════════════════════════╝\n");

    if args.dry_run {
        println!("DRY RUN - Books that would be indexed:\n");
        for book in &books {
            let year_str = book
                .year
                .map(|y| y.to_string())
                .unwrap_or_else(|| "?".to_string());
            println!(
                "  [{:>5}] {} - {} ({}, {}, {})",
                book.id, book.title, book.author, book.language, book.genre, year_str
            );
        }
        return Ok(());
    }

    // Select embedding model based on multilingual flag
    // Multilingual models map concepts across languages, enabling cross-language search
    let embedding_model = if args.multilingual {
        // paraphrase-multilingual-MiniLM-L12-v2 supports 50+ languages and maps them to same vector space
        "sentence-transformers/paraphrase-multilingual-MiniLM-L12-v2".to_string()
    } else {
        "sentence-transformers/all-MiniLM-L6-v2".to_string()
    };

    // Create config
    let config = BookSearchConfig {
        opensearch_url: args.opensearch_url,
        index_name: args.index,
        embedding_model: embedding_model.clone(),
        embedding_dim: 384, // Both models use 384 dimensions
        chunk_size: args.chunk_size,
        chunk_overlap: args.chunk_overlap,
        cache_dir: args.cache_dir,
        metrics_port: 9091,
        otlp_endpoint: Some("http://localhost:4317".to_string()),
    };

    // Initialize embeddings based on provider selection
    if args.multilingual {
        info!("Multilingual mode enabled - using cross-language embedding model");
        info!("This allows searching in one language to find results in other languages");
    }

    let (embeddings, embedding_dim): (Arc<dyn Embeddings>, usize) = match args.embeddings.as_str() {
        "huggingface" => {
            info!(
                "Initializing HuggingFace embeddings with model: {}",
                config.embedding_model
            );
            (
                Arc::new(HuggingFaceEmbeddings::new().with_model(&config.embedding_model)),
                config.embedding_dim,
            )
        }
        "openai" => {
            // OpenAI's text-embedding-3-small already has strong multilingual support (100+ languages)
            info!(
                "Initializing OpenAI embeddings (text-embedding-3-small supports 100+ languages)"
            );
            (
                Arc::new(
                    OpenAIEmbeddings::new()
                        .with_model("text-embedding-3-small")
                        .with_dimensions(1024),
                ),
                1024,
            )
        }
        _ => {
            // Auto-detect: prefer HuggingFace if HF_TOKEN is set, otherwise use OpenAI
            if env::var("HF_TOKEN").is_ok() || env::var("HUGGINGFACEHUB_API_TOKEN").is_ok() {
                info!(
                    "Auto-detected HF_TOKEN, using HuggingFace embeddings with model: {}",
                    config.embedding_model
                );
                (
                    Arc::new(HuggingFaceEmbeddings::new().with_model(&config.embedding_model)),
                    config.embedding_dim,
                )
            } else if env::var("OPENAI_API_KEY").is_ok() {
                // OpenAI's text-embedding-3-small already has strong multilingual support
                info!(
                    "Auto-detected OPENAI_API_KEY, using OpenAI embeddings (multilingual-capable)"
                );
                (
                    Arc::new(
                        OpenAIEmbeddings::new()
                            .with_model("text-embedding-3-small")
                            .with_dimensions(1024),
                    ),
                    1024,
                )
            } else {
                anyhow::bail!("No embedding API key found. Set HF_TOKEN or OPENAI_API_KEY environment variable.\nAlternatively, use --embeddings=huggingface or --embeddings=openai to select a provider.");
            }
        }
    };

    // Create indexer (with optional language detection)
    let indexer = if args.detect_language {
        info!("Language auto-detection enabled");
        IndexerPipeline::with_language_detection(&config, embeddings)
    } else {
        IndexerPipeline::new(&config, embeddings)
    };

    // Create index if needed
    if !args.skip_create_index {
        info!(
            "Creating OpenSearch index with embedding dimension {}...",
            embedding_dim
        );
        indexer.create_index(embedding_dim).await?;
    }

    // Index books (optionally incremental)
    let mut total_chunks = 0;
    let mut successful_books = 0;
    let mut skipped_books = 0;

    if args.incremental {
        // Incremental indexing - skip books already in the index
        info!("Incremental indexing enabled, checking for existing books...");
        match indexer.index_books_incremental(&books).await {
            Ok(result) => {
                skipped_books = result.skipped_existing;
                for stats in result.results.into_iter().flatten() {
                    total_chunks += stats.chunks_indexed;
                    successful_books += 1;
                    println!(
                        "  [{}] {} chunks indexed ({})",
                        stats.book_id, stats.chunks_indexed, stats.language
                    );
                }
            }
            Err(e) => {
                eprintln!("Error during incremental indexing: {}", e);
            }
        }
    } else {
        // Full indexing - index all books
        for book in &books {
            match indexer.index_book(book).await {
                Ok(stats) => {
                    total_chunks += stats.chunks_indexed;
                    successful_books += 1;
                    // Show detected language (may differ from original if auto-detection enabled)
                    println!(
                        "  [{}] {} chunks indexed ({})",
                        book.id, stats.chunks_indexed, stats.language
                    );
                }
                Err(e) => {
                    eprintln!("  [{}] FAILED: {}", book.id, e);
                }
            }
        }
    }

    // Update index size metric
    telemetry::record_index_size(total_chunks as u64);

    println!("\n╔══════════════════════════════════════════════════════════════╗");
    println!("║              Indexing Complete                               ║");
    println!("╠══════════════════════════════════════════════════════════════╣");
    if args.incremental {
        println!(
            "║  Books skipped:    {:>4} (already indexed)                    ║",
            skipped_books
        );
    }
    println!(
        "║  Books indexed:    {:>4} / {:>4}                               ║",
        successful_books,
        books.len() - skipped_books
    );
    println!(
        "║  Total chunks:     {:>7}                                    ║",
        total_chunks
    );
    println!("╚══════════════════════════════════════════════════════════════╝");

    println!("\nRun queries:");
    println!("  cargo run -p librarian -- query \"Who is Elizabeth Bennet?\"");

    // Shutdown telemetry to flush any pending traces
    if let Some(handle) = telemetry_handle {
        handle.shutdown();
    }

    Ok(())
}
