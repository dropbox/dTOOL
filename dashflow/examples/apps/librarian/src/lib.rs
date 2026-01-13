//! # Superhuman Librarian
//!
//! The ultimate RAG paragon with hybrid search, memory, and analysis.
//!
//! ## Features
//! - **Hybrid Search**: Combines keyword (BM25) and semantic (kNN) search via OpenSearch
//! - **Local Embeddings**: HuggingFace Inference API (no OpenAI dependency for embeddings)
//! - **Full Telemetry**: Prometheus metrics, Grafana dashboards, Jaeger tracing
//! - **Evaluation Framework**: Golden Q&A dataset with automated scoring
//! - **Memory**: Conversation and reading history via DashFlow checkpointers
//! - **Fan Out**: Parallel multi-book search demonstrating DashFlow's parallel execution
//! - **Analysis**: Characters, themes, and relationships extraction
//!
//! ## Architecture
//! ```text
//! Gutenberg Books → Chunker → Embeddings → OpenSearch (Hybrid Index)
//!                                              ↓
//! Query → Intent → Fan Out Search → Rerank → Generate → Quality Check
//!                      ↓
//!                  ┌───┴───┐
//!            Semantic  Keyword  Filtered
//!                  └───┬───┘
//!                  Merge Results
//! ```

pub mod analysis;
pub mod catalog;
pub mod config;
pub mod cost;
pub mod downloader;
pub mod fan_out;
pub mod indexer;
pub mod introspection;
pub mod lang;
pub mod memory;
pub mod search;
pub mod synthesis;
pub mod telemetry;
pub mod workflow;

pub use analysis::{
    BookAnalyzer, Character, CharacterAnalysis, EvidenceChunk, Relationship, RelationshipType,
    Theme, ThemeAnalysis,
};
pub use catalog::{CatalogEntry, GutenbergCatalog};
pub use config::{get_book_metadata, BookMetadata, BookPreset, BookSearchConfig};
pub use cost::{CostTracker, EmbeddingModel, QueryCostRecord};
pub use downloader::GutenbergDownloader;
pub use fan_out::{FanOutResult, FanOutSearcher};
pub use indexer::{IncrementalIndexResult, IndexerPipeline};
pub use introspection::{
    format_analysis, format_improvements, format_trace, introspect_last_search, Improvement,
    ImprovementCategory, ImprovementStatus, SearchTrace, TraceAnalysis, TraceStore,
};
pub use lang::{detect_language, language_name};
pub use memory::{Bookmark, LibrarianMemory, MemoryManager, Note, ReadingProgress, Turn};
pub use search::{
    BookLength, CorrectedSearchResult, FacetBucket, FacetCounts, FacetedSearchResult, FilterStore,
    HybridSearcher, SavedFilter, SearchFilters, SearchResult, SelfCorrectionConfig,
};
pub use synthesis::AnswerSynthesizer;
pub use workflow::{QueryWorkflowState, StrategyTiming};
