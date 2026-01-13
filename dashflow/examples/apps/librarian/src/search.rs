//! Hybrid search combining BM25 keyword and kNN semantic search
//!
//! This module demonstrates incremental migration to DashFlow platform retrievers.
//! See `search_keyword_platform()` and `search_hybrid_platform()` for platform-based implementations.

use anyhow::{Context, Result};
use dashflow::core::embeddings::Embeddings;
use dashflow::core::retrievers::{MergerRetriever, Retriever};
use dashflow::core::utils::sanitize_for_log_default;
use dashflow::embed_query;
use dashflow_opensearch::{OpenSearchBM25Retriever, OpenSearchVectorStore, VectorStoreRetriever};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;
use tracing::{info, instrument};

/// Search result from hybrid query
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    /// Content of the matched chunk
    pub content: String,

    /// Book title
    pub title: String,

    /// Book author
    pub author: String,

    /// Book ID in Gutenberg
    pub book_id: String,

    /// Chunk index within the book
    pub chunk_index: i64,

    /// Combined relevance score
    pub score: f32,
}

/// Book length category for filtering
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BookLength {
    /// Short books: < 15,000 words (roughly < 50 pages)
    Short,
    /// Medium books: 15,000 - 60,000 words (roughly 50-200 pages)
    Medium,
    /// Long books: > 60,000 words (roughly > 200 pages)
    Long,
}

impl BookLength {
    /// Get the word count range for this length category
    /// Returns (min_words, max_words) - either can be None for open-ended
    pub fn word_count_range(&self) -> (Option<usize>, Option<usize>) {
        match self {
            Self::Short => (None, Some(15_000)),
            Self::Medium => (Some(15_000), Some(60_000)),
            Self::Long => (Some(60_000), None),
        }
    }

    /// Parse from string (case-insensitive)
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "short" => Some(Self::Short),
            "medium" => Some(Self::Medium),
            "long" => Some(Self::Long),
            _ => None,
        }
    }
}

impl std::fmt::Display for BookLength {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Short => write!(f, "short"),
            Self::Medium => write!(f, "medium"),
            Self::Long => write!(f, "long"),
        }
    }
}

/// Filters for search queries
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SearchFilters {
    /// Filter by author name (partial match)
    pub author: Option<String>,

    /// Filter by book title (partial match)
    pub title: Option<String>,

    /// Filter by specific book ID
    pub book_id: Option<String>,

    /// Filter by language (ISO 639-1 code: en, fr, de, es, etc.)
    pub language: Option<String>,

    /// Filter by genre (Fiction, Philosophy, Poetry, etc.)
    pub genre: Option<String>,

    /// Filter by publication year range (min year)
    pub year_min: Option<i32>,

    /// Filter by publication year range (max year)
    pub year_max: Option<i32>,

    /// Filter by era (e.g., "19th century", "victorian", "ancient")
    pub era: Option<String>,

    /// Filter by book length (short/medium/long)
    pub length: Option<BookLength>,
}

/// Facet bucket showing count for a value
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FacetBucket {
    /// The value (e.g., "English", "Fiction")
    pub value: String,
    /// Document count for this value
    pub count: u64,
}

/// Facet counts from aggregations
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FacetCounts {
    /// Language facet: count per language
    pub languages: Vec<FacetBucket>,
    /// Genre facet: count per genre
    pub genres: Vec<FacetBucket>,
    /// Author facet: count per author
    pub authors: Vec<FacetBucket>,
    /// Era facet: count per century
    pub eras: Vec<FacetBucket>,
    /// Length facet: count per length category
    pub lengths: Vec<FacetBucket>,
}

impl FacetCounts {
    /// Check if all facets are empty
    pub fn is_empty(&self) -> bool {
        self.languages.is_empty()
            && self.genres.is_empty()
            && self.authors.is_empty()
            && self.eras.is_empty()
            && self.lengths.is_empty()
    }
}

/// Search result with facet counts
#[derive(Debug, Clone)]
pub struct FacetedSearchResult {
    /// The search results
    pub results: Vec<SearchResult>,
    /// Facet counts from aggregations
    pub facets: FacetCounts,
    /// Total matching documents (before limit)
    pub total_hits: u64,
}

/// A named saved filter configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SavedFilter {
    /// Unique name for this filter preset
    pub name: String,
    /// Optional description of what this filter does
    pub description: Option<String>,
    /// The filter configuration
    pub filters: SearchFilters,
    /// When this filter was created
    pub created_at: String,
}

impl SavedFilter {
    /// Create a new saved filter
    pub fn new(name: &str, filters: SearchFilters, description: Option<&str>) -> Self {
        Self {
            name: name.to_string(),
            description: description.map(String::from),
            filters,
            created_at: chrono::Utc::now().to_rfc3339(),
        }
    }
}

/// Store for saved filter presets
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct FilterStore {
    /// Map of filter name to saved filter
    filters: HashMap<String, SavedFilter>,
    /// Storage file path
    #[serde(skip)]
    path: Option<PathBuf>,
}

impl FilterStore {
    /// Create a new filter store that loads from/saves to the given path
    pub fn new(path: impl AsRef<Path>) -> Self {
        let path = path.as_ref().to_path_buf();
        let mut store = Self::load(&path).unwrap_or_default();
        store.path = Some(path);
        store
    }

    /// Load filters from a JSON file
    pub fn load(path: &Path) -> Result<Self> {
        let content = fs::read_to_string(path).context("Failed to read filter store file")?;
        let mut store: Self =
            serde_json::from_str(&content).context("Failed to parse filter store JSON")?;
        store.path = Some(path.to_path_buf());
        Ok(store)
    }

    /// Save filters to the configured path
    pub fn save(&self) -> Result<()> {
        let Some(path) = &self.path else {
            anyhow::bail!("No storage path configured");
        };

        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        let content = serde_json::to_string_pretty(self)?;
        fs::write(path, content)?;
        Ok(())
    }

    /// Add a new saved filter
    pub fn add(&mut self, filter: SavedFilter) -> Result<()> {
        self.filters.insert(filter.name.clone(), filter);
        self.save()
    }

    /// Get a saved filter by name
    pub fn get(&self, name: &str) -> Option<&SavedFilter> {
        self.filters.get(name)
    }

    /// Remove a saved filter by name
    pub fn remove(&mut self, name: &str) -> Result<bool> {
        let existed = self.filters.remove(name).is_some();
        if existed {
            self.save()?;
        }
        Ok(existed)
    }

    /// List all saved filter names
    pub fn list(&self) -> Vec<&SavedFilter> {
        self.filters.values().collect()
    }

    /// Check if a filter exists
    pub fn contains(&self, name: &str) -> bool {
        self.filters.contains_key(name)
    }

    /// Get the number of saved filters
    pub fn len(&self) -> usize {
        self.filters.len()
    }

    /// Check if store is empty
    pub fn is_empty(&self) -> bool {
        self.filters.is_empty()
    }
}

/// Hybrid searcher combining BM25 and semantic search
pub struct HybridSearcher {
    embeddings: Arc<dyn Embeddings>,
    opensearch_url: String,
    index_name: String,
    client: reqwest::Client,
}

impl HybridSearcher {
    /// Create a new hybrid searcher
    pub fn new(
        embeddings: Arc<dyn Embeddings>,
        opensearch_url: String,
        index_name: String,
    ) -> Self {
        Self {
            embeddings,
            opensearch_url,
            index_name,
            client: reqwest::Client::new(),
        }
    }

    /// Perform hybrid search combining BM25 and semantic
    #[instrument(skip(self), fields(query = %query, limit = %limit))]
    pub async fn search(&self, query: &str, limit: usize) -> Result<Vec<SearchResult>> {
        self.search_filtered(query, &SearchFilters::default(), limit)
            .await
    }

    /// Perform hybrid search with filters
    #[instrument(skip(self), fields(query = %query, limit = %limit))]
    pub async fn search_filtered(
        &self,
        query: &str,
        filters: &SearchFilters,
        limit: usize,
    ) -> Result<Vec<SearchResult>> {
        let start = Instant::now();

        // Generate query embedding using graph API
        let query_vec = embed_query(Arc::clone(&self.embeddings), query)
            .await
            .context("Failed to generate query embedding")?;

        let embed_time = start.elapsed();
        info!("Embedding generated in {:?}", embed_time);

        // Build filter clauses
        let mut filter_clauses = Vec::new();
        if let Some(author) = &filters.author {
            filter_clauses.push(json!({
                "match": { "author": author }
            }));
        }
        if let Some(title) = &filters.title {
            filter_clauses.push(json!({
                "match": { "title": title }
            }));
        }
        if let Some(book_id) = &filters.book_id {
            filter_clauses.push(json!({
                "term": { "book_id": book_id }
            }));
        }
        if let Some(language) = &filters.language {
            filter_clauses.push(json!({
                "term": { "language": language }
            }));
        }
        if let Some(genre) = &filters.genre {
            filter_clauses.push(json!({
                "term": { "genre": genre }
            }));
        }

        // Handle year range filters
        if filters.year_min.is_some() || filters.year_max.is_some() {
            let mut range_query = serde_json::Map::new();
            if let Some(year_min) = filters.year_min {
                range_query.insert("gte".to_string(), json!(year_min));
            }
            if let Some(year_max) = filters.year_max {
                range_query.insert("lte".to_string(), json!(year_max));
            }
            filter_clauses.push(json!({
                "range": { "year": range_query }
            }));
        }

        // Handle era filter (converts to year range)
        if let Some(era) = &filters.era {
            let (era_min, era_max) = era_to_year_range(era);
            if let Some(min) = era_min {
                let mut range_query = serde_json::Map::new();
                range_query.insert("gte".to_string(), json!(min));
                if let Some(max) = era_max {
                    range_query.insert("lte".to_string(), json!(max));
                }
                filter_clauses.push(json!({
                    "range": { "year": range_query }
                }));
            }
        }

        // Handle length filter (converts to word_count range)
        if let Some(length) = &filters.length {
            let (min_words, max_words) = length.word_count_range();
            let mut range_query = serde_json::Map::new();
            if let Some(min) = min_words {
                range_query.insert("gte".to_string(), json!(min));
            }
            if let Some(max) = max_words {
                range_query.insert("lte".to_string(), json!(max));
            }
            filter_clauses.push(json!({
                "range": { "word_count": range_query }
            }));
        }

        // Build hybrid query using script_score for kNN (OpenSearch 2.x compatible)
        // Note: OpenSearch Neural Search plugin provides better hybrid search,
        // but this approach works without additional plugins
        // Individual queries for reference (hybrid query combines them below)
        let _bm25_query = json!({
            "bool": {
                "must": [
                    { "match": { "content": query } }
                ],
                "filter": filter_clauses
            }
        });

        let _knn_query = json!({
            "bool": {
                "must": [
                    {
                        "script_score": {
                            "query": { "match_all": {} },
                            "script": {
                                "source": "knn_score",
                                "lang": "knn",
                                "params": {
                                    "field": "embedding",
                                    "query_value": query_vec,
                                    "space_type": "cosinesimil"
                                }
                            }
                        }
                    }
                ],
                "filter": filter_clauses
            }
        });

        // Execute both queries and combine results
        // For simplicity, we use a weighted combination approach
        let search_body = json!({
            "size": limit * 2,  // Get more to allow deduplication
            "query": {
                "bool": {
                    "should": [
                        {
                            "match": {
                                "content": {
                                    "query": query,
                                    "boost": 0.3  // BM25 weight
                                }
                            }
                        },
                        {
                            "knn": {
                                "embedding": {
                                    "vector": query_vec,
                                    "k": limit
                                }
                            }
                        }
                    ],
                    "filter": filter_clauses,
                    "minimum_should_match": 1
                }
            },
            "_source": ["content", "title", "author", "book_id", "chunk_index"]
        });

        let url = format!("{}/{}/_search", self.opensearch_url, self.index_name);
        let response = self.client.post(&url).json(&search_body).send().await?;

        if !response.status().is_success() {
            let error_text = response.text().await?;
            anyhow::bail!("Search failed: {}", error_text);
        }

        let result: serde_json::Value = response.json().await?;
        let hits = result
            .get("hits")
            .and_then(|h| h.get("hits"))
            .and_then(|h| h.as_array())
            .context("Invalid search response")?;

        let results: Vec<SearchResult> = hits
            .iter()
            .take(limit)
            .filter_map(|hit| {
                let source = hit.get("_source")?;
                let score = hit.get("_score")?.as_f64()? as f32;

                Some(SearchResult {
                    content: source.get("content")?.as_str()?.to_string(),
                    title: source.get("title")?.as_str()?.to_string(),
                    author: source.get("author")?.as_str()?.to_string(),
                    book_id: source.get("book_id")?.as_str()?.to_string(),
                    chunk_index: source.get("chunk_index")?.as_i64()?,
                    score,
                })
            })
            .collect();

        let search_time = start.elapsed();
        info!(
            "Search completed in {:?}, {} results",
            search_time,
            results.len()
        );

        // Record metrics
        metrics::counter!("librarian_queries_total", "type" => "hybrid").increment(1);
        metrics::histogram!("librarian_search_latency_ms").record(search_time.as_millis() as f64);
        metrics::histogram!("librarian_embedding_latency_ms").record(embed_time.as_millis() as f64);

        Ok(results)
    }

    /// Perform search with faceted aggregations (filter counts)
    #[instrument(skip(self), fields(query = %query, limit = %limit))]
    pub async fn search_with_facets(
        &self,
        query: &str,
        filters: &SearchFilters,
        limit: usize,
    ) -> Result<FacetedSearchResult> {
        let start = Instant::now();

        // Generate query embedding using graph API
        let query_vec = embed_query(Arc::clone(&self.embeddings), query)
            .await
            .context("Failed to generate query embedding")?;

        // Build filter clauses (same as search_filtered)
        let mut filter_clauses = Vec::new();
        if let Some(author) = &filters.author {
            filter_clauses.push(json!({ "match": { "author": author } }));
        }
        if let Some(title) = &filters.title {
            filter_clauses.push(json!({ "match": { "title": title } }));
        }
        if let Some(book_id) = &filters.book_id {
            filter_clauses.push(json!({ "term": { "book_id": book_id } }));
        }
        if let Some(language) = &filters.language {
            filter_clauses.push(json!({ "term": { "language": language } }));
        }
        if let Some(genre) = &filters.genre {
            filter_clauses.push(json!({ "term": { "genre": genre } }));
        }
        if filters.year_min.is_some() || filters.year_max.is_some() {
            let mut range_query = serde_json::Map::new();
            if let Some(year_min) = filters.year_min {
                range_query.insert("gte".to_string(), json!(year_min));
            }
            if let Some(year_max) = filters.year_max {
                range_query.insert("lte".to_string(), json!(year_max));
            }
            filter_clauses.push(json!({ "range": { "year": range_query } }));
        }
        if let Some(era) = &filters.era {
            let (era_min, era_max) = era_to_year_range(era);
            if let Some(min) = era_min {
                let mut range_query = serde_json::Map::new();
                range_query.insert("gte".to_string(), json!(min));
                if let Some(max) = era_max {
                    range_query.insert("lte".to_string(), json!(max));
                }
                filter_clauses.push(json!({ "range": { "year": range_query } }));
            }
        }
        if let Some(length) = &filters.length {
            let (min_words, max_words) = length.word_count_range();
            let mut range_query = serde_json::Map::new();
            if let Some(min) = min_words {
                range_query.insert("gte".to_string(), json!(min));
            }
            if let Some(max) = max_words {
                range_query.insert("lte".to_string(), json!(max));
            }
            filter_clauses.push(json!({ "range": { "word_count": range_query } }));
        }

        // Build search query with aggregations
        let search_body = json!({
            "size": limit,
            "query": {
                "bool": {
                    "should": [
                        {
                            "match": {
                                "content": {
                                    "query": query,
                                    "boost": 0.3
                                }
                            }
                        },
                        {
                            "knn": {
                                "embedding": {
                                    "vector": query_vec,
                                    "k": limit
                                }
                            }
                        }
                    ],
                    "filter": filter_clauses,
                    "minimum_should_match": 1
                }
            },
            "_source": ["content", "title", "author", "book_id", "chunk_index"],
            "aggs": {
                "by_language": {
                    "terms": { "field": "language", "size": 20 }
                },
                "by_genre": {
                    "terms": { "field": "genre", "size": 20 }
                },
                "by_author": {
                    "terms": { "field": "author.keyword", "size": 20 }
                },
                "by_year": {
                    "histogram": { "field": "year", "interval": 100, "min_doc_count": 1 }
                },
                "by_length": {
                    "range": {
                        "field": "word_count",
                        "ranges": [
                            { "key": "short", "to": 15000 },
                            { "key": "medium", "from": 15000, "to": 60000 },
                            { "key": "long", "from": 60000 }
                        ]
                    }
                }
            }
        });

        let url = format!("{}/{}/_search", self.opensearch_url, self.index_name);
        let response = self.client.post(&url).json(&search_body).send().await?;

        if !response.status().is_success() {
            let error_text = response.text().await?;
            anyhow::bail!("Search failed: {}", error_text);
        }

        let result: serde_json::Value = response.json().await?;

        // Parse hits
        let hits = result
            .get("hits")
            .and_then(|h| h.get("hits"))
            .and_then(|h| h.as_array())
            .context("Invalid search response")?;

        let total_hits = result
            .get("hits")
            .and_then(|h| h.get("total"))
            .and_then(|t| t.get("value"))
            .and_then(|v| v.as_u64())
            .unwrap_or(0);

        let results: Vec<SearchResult> = hits
            .iter()
            .filter_map(|hit| {
                let source = hit.get("_source")?;
                let score = hit.get("_score")?.as_f64()? as f32;

                Some(SearchResult {
                    content: source.get("content")?.as_str()?.to_string(),
                    title: source.get("title")?.as_str()?.to_string(),
                    author: source.get("author")?.as_str()?.to_string(),
                    book_id: source.get("book_id")?.as_str()?.to_string(),
                    chunk_index: source.get("chunk_index")?.as_i64()?,
                    score,
                })
            })
            .collect();

        // Parse aggregations
        let aggs = result.get("aggregations");
        let facets = parse_facets(aggs);

        let search_time = start.elapsed();
        info!(
            "Faceted search completed in {:?}, {} results, {} facets",
            search_time,
            results.len(),
            facets.languages.len() + facets.genres.len() + facets.authors.len()
        );

        metrics::counter!("librarian_queries_total", "type" => "faceted").increment(1);

        Ok(FacetedSearchResult {
            results,
            facets,
            total_hits,
        })
    }

    /// Search with BM25 only (keyword search)
    pub async fn search_keyword(&self, query: &str, limit: usize) -> Result<Vec<SearchResult>> {
        let search_body = json!({
            "size": limit,
            "query": {
                "match": {
                    "content": query
                }
            },
            "_source": ["content", "title", "author", "book_id", "chunk_index"]
        });

        let url = format!("{}/{}/_search", self.opensearch_url, self.index_name);
        let response = self.client.post(&url).json(&search_body).send().await?;

        if !response.status().is_success() {
            let error_text = response.text().await?;
            anyhow::bail!("Search failed: {}", error_text);
        }

        let result: serde_json::Value = response.json().await?;
        let hits = result
            .get("hits")
            .and_then(|h| h.get("hits"))
            .and_then(|h| h.as_array())
            .context("Invalid search response")?;

        let results: Vec<SearchResult> = hits
            .iter()
            .filter_map(|hit| {
                let source = hit.get("_source")?;
                let score = hit.get("_score")?.as_f64()? as f32;

                Some(SearchResult {
                    content: source.get("content")?.as_str()?.to_string(),
                    title: source.get("title")?.as_str()?.to_string(),
                    author: source.get("author")?.as_str()?.to_string(),
                    book_id: source.get("book_id")?.as_str()?.to_string(),
                    chunk_index: source.get("chunk_index")?.as_i64()?,
                    score,
                })
            })
            .collect();

        metrics::counter!("librarian_queries_total", "type" => "keyword").increment(1);

        Ok(results)
    }

    /// Search with BM25 keyword search using DashFlow platform retriever
    ///
    /// This method demonstrates using `dashflow_opensearch::OpenSearchBM25Retriever`
    /// instead of custom OpenSearch HTTP calls. It provides the same functionality
    /// as `search_keyword()` but uses the platform abstraction.
    ///
    /// # Arguments
    ///
    /// * `query` - Search query string
    /// * `limit` - Maximum number of results to return
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let results = searcher.search_keyword_platform("whale hunt", 10).await?;
    /// ```
    #[instrument(skip(self))]
    pub async fn search_keyword_platform(
        &self,
        query: &str,
        limit: usize,
    ) -> Result<Vec<SearchResult>> {
        // Use DashFlow platform retriever
        let bm25 = OpenSearchBM25Retriever::from_existing(
            &self.index_name,
            &self.opensearch_url,
            limit,
            "content",
        )
        .await
        .map_err(|e| anyhow::anyhow!("Failed to create BM25 retriever: {}", e))?;

        // Get documents using platform retriever
        let docs = bm25
            ._get_relevant_documents(query, None)
            .await
            .map_err(|e| anyhow::anyhow!("BM25 search failed: {}", e))?;

        // Convert platform Documents to Librarian SearchResults
        let results: Vec<SearchResult> = docs
            .into_iter()
            .filter_map(|doc| {
                Some(SearchResult {
                    content: doc.page_content,
                    title: doc.metadata.get("title")?.as_str()?.to_string(),
                    author: doc.metadata.get("author")?.as_str()?.to_string(),
                    book_id: doc.metadata.get("book_id")?.as_str()?.to_string(),
                    chunk_index: doc.metadata.get("chunk_index")?.as_i64()?,
                    score: 1.0, // Platform retriever doesn't expose scores directly
                })
            })
            .collect();

        metrics::counter!("librarian_queries_total", "type" => "keyword_platform").increment(1);
        info!(
            query = %query,
            results = results.len(),
            "Platform BM25 search completed"
        );

        Ok(results)
    }

    /// Search with kNN only (semantic search)
    pub async fn search_semantic(&self, query: &str, limit: usize) -> Result<Vec<SearchResult>> {
        let query_vec = embed_query(Arc::clone(&self.embeddings), query).await?;

        let search_body = json!({
            "size": limit,
            "query": {
                "knn": {
                    "embedding": {
                        "vector": query_vec,
                        "k": limit
                    }
                }
            },
            "_source": ["content", "title", "author", "book_id", "chunk_index"]
        });

        let url = format!("{}/{}/_search", self.opensearch_url, self.index_name);
        let response = self.client.post(&url).json(&search_body).send().await?;

        if !response.status().is_success() {
            let error_text = response.text().await?;
            anyhow::bail!("Search failed: {}", error_text);
        }

        let result: serde_json::Value = response.json().await?;
        let hits = result
            .get("hits")
            .and_then(|h| h.get("hits"))
            .and_then(|h| h.as_array())
            .context("Invalid search response")?;

        let results: Vec<SearchResult> = hits
            .iter()
            .filter_map(|hit| {
                let source = hit.get("_source")?;
                let score = hit.get("_score")?.as_f64()? as f32;

                Some(SearchResult {
                    content: source.get("content")?.as_str()?.to_string(),
                    title: source.get("title")?.as_str()?.to_string(),
                    author: source.get("author")?.as_str()?.to_string(),
                    book_id: source.get("book_id")?.as_str()?.to_string(),
                    chunk_index: source.get("chunk_index")?.as_i64()?,
                    score,
                })
            })
            .collect();

        metrics::counter!("librarian_queries_total", "type" => "semantic").increment(1);

        Ok(results)
    }

    /// Hybrid search using DashFlow platform retrievers with MergerRetriever.
    ///
    /// This method demonstrates using DashFlow platform abstractions for hybrid search:
    /// - `OpenSearchBM25Retriever` for keyword/BM25 search
    /// - `VectorStoreRetriever` wrapping `OpenSearchVectorStore` for semantic search
    /// - `MergerRetriever` to combine results using round-robin interleaving
    ///
    /// # Arguments
    ///
    /// * `query` - Search query string
    /// * `limit` - Maximum number of results to return
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let results = searcher.search_hybrid_platform("whale hunt", 10).await?;
    /// ```
    #[instrument(skip(self))]
    pub async fn search_hybrid_platform(
        &self,
        query: &str,
        limit: usize,
    ) -> Result<Vec<SearchResult>> {
        let start = Instant::now();

        // Create BM25 retriever for keyword search
        let bm25 = OpenSearchBM25Retriever::from_existing(
            &self.index_name,
            &self.opensearch_url,
            limit,
            "content",
        )
        .await
        .map_err(|e| anyhow::anyhow!("Failed to create BM25 retriever: {}", e))?;

        // Create vector store and wrap it in a retriever for semantic search
        let vector_store = OpenSearchVectorStore::new(
            &self.index_name,
            Arc::clone(&self.embeddings),
            &self.opensearch_url,
        )
        .await
        .map_err(|e| anyhow::anyhow!("Failed to create vector store: {}", e))?;
        let semantic = VectorStoreRetriever::new(vector_store, limit);

        // Combine retrievers using MergerRetriever (round-robin interleaving)
        let merger = MergerRetriever::new(vec![Arc::new(bm25), Arc::new(semantic)]);

        // Get merged results from platform
        let docs = merger
            ._get_relevant_documents(query, None)
            .await
            .map_err(|e| anyhow::anyhow!("Hybrid search failed: {}", e))?;

        // Convert platform Documents to Librarian SearchResults
        // Take only 'limit' results after deduplication
        let mut seen_chunks = std::collections::HashSet::new();
        let results: Vec<SearchResult> = docs
            .into_iter()
            .filter_map(|doc| {
                let book_id = doc.metadata.get("book_id")?.as_str()?.to_string();
                let chunk_index = doc.metadata.get("chunk_index")?.as_i64()?;
                let key = (book_id.clone(), chunk_index);

                // Skip duplicates (same chunk from both retrievers)
                if !seen_chunks.insert(key) {
                    return None;
                }

                Some(SearchResult {
                    content: doc.page_content,
                    title: doc.metadata.get("title")?.as_str()?.to_string(),
                    author: doc.metadata.get("author")?.as_str()?.to_string(),
                    book_id,
                    chunk_index,
                    score: 1.0, // Platform retriever doesn't expose scores directly
                })
            })
            .take(limit)
            .collect();

        let search_time = start.elapsed();
        metrics::counter!("librarian_queries_total", "type" => "hybrid_platform").increment(1);
        metrics::histogram!("librarian_search_latency_ms").record(search_time.as_millis() as f64);

        info!(
            query = %query,
            results = results.len(),
            duration_ms = %search_time.as_millis(),
            "Platform hybrid search completed"
        );

        Ok(results)
    }
}

/// Query classification for intelligent routing
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QueryType {
    /// Factual queries seeking specific information (who, what, when, where)
    Factual,
    /// Conceptual queries seeking themes, meanings, or relationships
    Conceptual,
    /// Ambiguous queries that could be either
    Ambiguous,
}

impl std::fmt::Display for QueryType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Factual => write!(f, "factual"),
            Self::Conceptual => write!(f, "conceptual"),
            Self::Ambiguous => write!(f, "ambiguous"),
        }
    }
}

/// Classify a query to determine the best search strategy
pub fn classify_query(query: &str) -> QueryType {
    let query_lower = query.to_lowercase();
    let words: Vec<&str> = query_lower.split_whitespace().collect();

    // Factual query indicators (seeking specific information)
    let factual_patterns = [
        "who is",
        "who was",
        "who wrote",
        "who said",
        "what is",
        "what was",
        "what happens",
        "what did",
        "when did",
        "when was",
        "when does",
        "where is",
        "where was",
        "where does",
        "where did",
        "name of",
        "called",
        "title of",
        "how many",
        "how old",
        "how much",
        "list",
        "quote",
        "exact",
        "passage",
    ];

    // Conceptual query indicators (seeking themes, meanings)
    let conceptual_patterns = [
        "why does",
        "why did",
        "why is",
        "why was",
        "how does",
        "how did",
        "how is",
        "how was",
        "theme",
        "themes",
        "meaning",
        "significance",
        "symbolism",
        "symbol",
        "represents",
        "represent",
        "relationship",
        "connection",
        "compare",
        "contrast",
        "analysis",
        "analyze",
        "interpret",
        "interpretation",
        "motif",
        "literary",
        "metaphor",
        "allegory",
        "portrayal",
        "depiction",
        "characterization",
        "similar",
        "different",
        "influence",
    ];

    // Check for factual patterns
    let is_factual = factual_patterns
        .iter()
        .any(|pattern| query_lower.contains(pattern));

    // Check for conceptual patterns
    let is_conceptual = conceptual_patterns
        .iter()
        .any(|pattern| query_lower.contains(pattern));

    // Check for quoted strings (likely seeking exact text)
    let has_quotes = query.contains('"') || query.contains('\'');

    // Proper nouns (capitalized words that aren't first word) suggest factual
    let has_proper_nouns = words
        .iter()
        .skip(1)
        .any(|w| w.chars().next().is_some_and(|c| c.is_uppercase()));

    // Determine query type based on indicators
    match (is_factual || has_quotes || has_proper_nouns, is_conceptual) {
        (true, false) => QueryType::Factual,
        (false, true) => QueryType::Conceptual,
        (true, true) => {
            // If both match, prefer factual for quoted/proper noun queries
            if has_quotes || has_proper_nouns {
                QueryType::Factual
            } else {
                QueryType::Conceptual
            }
        }
        (false, false) => QueryType::Ambiguous,
    }
}

/// Recommend search mode based on query type
pub fn recommend_search_mode(query_type: QueryType) -> &'static str {
    match query_type {
        QueryType::Factual => "keyword",
        QueryType::Conceptual => "semantic",
        QueryType::Ambiguous => "hybrid",
    }
}

/// Self-correction strategy to broaden queries
#[derive(Debug, Clone)]
pub struct SelfCorrectionConfig {
    /// Maximum number of correction attempts
    pub max_attempts: usize,
    /// Whether to remove stop words on correction
    pub remove_stop_words: bool,
    /// Whether to try synonyms
    pub try_synonyms: bool,
}

impl Default for SelfCorrectionConfig {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            remove_stop_words: true,
            try_synonyms: true,
        }
    }
}

/// Result of a self-correcting search
#[derive(Debug, Clone)]
pub struct CorrectedSearchResult {
    /// The results found
    pub results: Vec<SearchResult>,
    /// Number of correction attempts made
    pub attempts: usize,
    /// The queries tried (original + corrections)
    pub queries_tried: Vec<String>,
    /// Whether correction was needed
    pub corrected: bool,
}

/// Self-correcting search that automatically broadens queries if no results
impl HybridSearcher {
    /// Search with automatic query correction if no results found
    #[instrument(skip(self), fields(query = %query))]
    pub async fn search_with_correction(
        &self,
        query: &str,
        limit: usize,
        config: &SelfCorrectionConfig,
    ) -> Result<CorrectedSearchResult> {
        let mut queries_tried = vec![query.to_string()];

        // First, try the original query
        let results = self.search(query, limit).await?;
        if !results.is_empty() {
            return Ok(CorrectedSearchResult {
                results,
                attempts: 1,
                queries_tried,
                corrected: false,
            });
        }

        // M-235: Sanitize user query to prevent log injection
        info!(query = %sanitize_for_log_default(query), "No results, attempting self-correction");

        // Generate query variations
        let variations = generate_query_variations(query, config);

        for (attempt, variation) in variations.iter().enumerate() {
            if attempt >= config.max_attempts {
                break;
            }

            queries_tried.push(variation.clone());
            // M-235: Sanitize derived query variation
            info!(attempt = attempt + 1, variation = %sanitize_for_log_default(variation), "Correction attempt");

            let results = self.search(variation, limit).await?;
            if !results.is_empty() {
                // M-235: Sanitize corrected query in log
                info!(
                    results = results.len(),
                    variation = %sanitize_for_log_default(variation),
                    "Found results with corrected query"
                );
                return Ok(CorrectedSearchResult {
                    results,
                    attempts: attempt + 2, // +1 for original, +1 for 0-indexing
                    queries_tried,
                    corrected: true,
                });
            }
        }

        // Try semantic search as last resort (catches conceptual matches)
        info!("Trying semantic search as fallback...");
        queries_tried.push(format!("[semantic] {}", query));
        let results = self.search_semantic(query, limit).await?;
        let corrected = !results.is_empty();

        Ok(CorrectedSearchResult {
            results,
            attempts: queries_tried.len(),
            queries_tried,
            corrected,
        })
    }
}

/// Generate query variations for self-correction
fn generate_query_variations(query: &str, config: &SelfCorrectionConfig) -> Vec<String> {
    let mut variations = Vec::new();
    let words: Vec<&str> = query.split_whitespace().collect();

    // Stop words to remove for broader search
    let stop_words = [
        "the", "a", "an", "is", "are", "was", "were", "be", "been", "being", "have", "has", "had",
        "do", "does", "did", "will", "would", "could", "should", "may", "might", "must", "shall",
        "can", "need", "dare", "of", "in", "to", "for", "with", "on", "at", "by", "from", "about",
        "into", "through", "during", "before", "after", "above", "below", "and", "or", "but", "if",
        "then", "else", "when", "where", "why", "how", "what", "which", "who", "whom", "this",
        "that", "these", "those",
    ];

    // 1. Remove stop words
    if config.remove_stop_words && words.len() > 2 {
        let filtered: Vec<&str> = words
            .iter()
            .filter(|w| !stop_words.contains(&w.to_lowercase().as_str()))
            .copied()
            .collect();
        if !filtered.is_empty() && filtered.len() < words.len() {
            variations.push(filtered.join(" "));
        }
    }

    // 2. Extract key terms (nouns/verbs likely to be content words)
    // Simple heuristic: longer words are more likely to be content words
    let content_words: Vec<&str> = words
        .iter()
        .filter(|w| w.len() >= 4 && !stop_words.contains(&w.to_lowercase().as_str()))
        .copied()
        .collect();
    if content_words.len() >= 2 && content_words.len() < words.len() {
        variations.push(content_words.join(" "));
    }

    // 3. Try just the most significant words (first noun-like terms)
    if words.len() > 3 {
        let significant: Vec<&str> = words
            .iter()
            .filter(|w| w.len() >= 5)
            .take(2)
            .copied()
            .collect();
        if !significant.is_empty() {
            variations.push(significant.join(" "));
        }
    }

    // 4. Common synonyms for book-related terms
    if config.try_synonyms {
        let synonyms: Vec<(&str, &str)> = vec![
            ("monster", "creature"),
            ("ship", "vessel"),
            ("whale", "leviathan"),
            ("love", "affection"),
            ("death", "mortality"),
            ("journey", "voyage"),
            ("hero", "protagonist"),
            ("villain", "antagonist"),
            ("revenge", "vengeance"),
            ("madness", "insanity"),
        ];

        for (word, synonym) in &synonyms {
            if query.to_lowercase().contains(*word) {
                variations.push(query.to_lowercase().replace(*word, synonym));
            }
        }
    }

    // Remove duplicates and the original query
    let query_lower = query.to_lowercase();
    variations
        .into_iter()
        .filter(|v| v.to_lowercase() != query_lower && !v.is_empty())
        .collect()
}

/// Convert era name to year range for filtering
/// Returns (min_year, max_year) - either or both can be None
fn era_to_year_range(era: &str) -> (Option<i32>, Option<i32>) {
    match era.to_lowercase().as_str() {
        // Ancient periods
        "ancient" | "antiquity" => (Some(-3000), Some(500)),
        "classical" | "greek" | "roman" => (Some(-800), Some(500)),

        // Medieval period
        "medieval" | "middle ages" => (Some(500), Some(1500)),
        "dark ages" => (Some(500), Some(1000)),

        // Early Modern
        "renaissance" => (Some(1400), Some(1600)),
        "enlightenment" => (Some(1685), Some(1815)),

        // 18th-19th centuries
        "18th century" | "1700s" => (Some(1700), Some(1799)),
        "19th century" | "1800s" => (Some(1800), Some(1899)),
        "victorian" => (Some(1837), Some(1901)),
        "romantic" | "romanticism" => (Some(1780), Some(1850)),

        // 20th century
        "20th century" | "1900s" | "modern" => (Some(1900), Some(1999)),
        "edwardian" => (Some(1901), Some(1910)),
        "interwar" => (Some(1918), Some(1939)),
        "postwar" | "post-war" => (Some(1945), Some(1970)),

        // Contemporary
        "21st century" | "2000s" | "contemporary" => (Some(2000), None),

        // Specific centuries
        "16th century" => (Some(1500), Some(1599)),
        "17th century" => (Some(1600), Some(1699)),

        // Unknown era - return no filter
        _ => (None, None),
    }
}

/// Parse OpenSearch aggregation results into FacetCounts
fn parse_facets(aggs: Option<&serde_json::Value>) -> FacetCounts {
    let mut facets = FacetCounts::default();

    let Some(aggs) = aggs else {
        return facets;
    };

    // Parse language facet (terms aggregation)
    if let Some(by_language) = aggs.get("by_language") {
        facets.languages = parse_term_buckets(by_language);
    }

    // Parse genre facet (terms aggregation)
    if let Some(by_genre) = aggs.get("by_genre") {
        facets.genres = parse_term_buckets(by_genre);
    }

    // Parse author facet (terms aggregation)
    if let Some(by_author) = aggs.get("by_author") {
        facets.authors = parse_term_buckets(by_author);
    }

    // Parse year facet (histogram aggregation) - convert to era names
    if let Some(by_year) = aggs.get("by_year") {
        facets.eras = parse_histogram_buckets_as_eras(by_year);
    }

    // Parse length facet (range aggregation)
    if let Some(by_length) = aggs.get("by_length") {
        facets.lengths = parse_range_buckets(by_length);
    }

    facets
}

/// Parse term aggregation buckets
fn parse_term_buckets(agg: &serde_json::Value) -> Vec<FacetBucket> {
    agg.get("buckets")
        .and_then(|b| b.as_array())
        .map(|buckets| {
            buckets
                .iter()
                .filter_map(|bucket| {
                    let value = bucket.get("key")?.as_str()?.to_string();
                    let count = bucket.get("doc_count")?.as_u64()?;
                    Some(FacetBucket { value, count })
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Parse histogram buckets and convert to era names (centuries)
fn parse_histogram_buckets_as_eras(agg: &serde_json::Value) -> Vec<FacetBucket> {
    agg.get("buckets")
        .and_then(|b| b.as_array())
        .map(|buckets| {
            buckets
                .iter()
                .filter_map(|bucket| {
                    let year = bucket.get("key")?.as_i64()? as i32;
                    let count = bucket.get("doc_count")?.as_u64()?;
                    // Convert year to century name
                    let era_name = match year {
                        y if y < 0 => "Ancient".to_string(),
                        y if y < 500 => "Classical".to_string(),
                        y if y < 1500 => "Medieval".to_string(),
                        y if y < 1600 => "16th Century".to_string(),
                        y if y < 1700 => "17th Century".to_string(),
                        y if y < 1800 => "18th Century".to_string(),
                        y if y < 1900 => "19th Century".to_string(),
                        y if y < 2000 => "20th Century".to_string(),
                        _ => "21st Century".to_string(),
                    };
                    Some(FacetBucket {
                        value: era_name,
                        count,
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Parse range aggregation buckets
fn parse_range_buckets(agg: &serde_json::Value) -> Vec<FacetBucket> {
    agg.get("buckets")
        .and_then(|b| b.as_array())
        .map(|buckets| {
            buckets
                .iter()
                .filter_map(|bucket| {
                    let value = bucket.get("key")?.as_str()?.to_string();
                    let count = bucket.get("doc_count")?.as_u64()?;
                    Some(FacetBucket { value, count })
                })
                .collect()
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    // `cargo verify` runs clippy with `-D warnings` for all targets, including unit tests.
    #![allow(clippy::unwrap_used)]

    use super::*;

    #[test]
    fn test_classify_factual_queries() {
        assert_eq!(classify_query("Who wrote Moby Dick?"), QueryType::Factual);
        assert_eq!(
            classify_query("What is the name of the ship in Moby Dick?"),
            QueryType::Factual
        );
        assert_eq!(
            classify_query("When did Captain Ahab die?"),
            QueryType::Factual
        );
        assert_eq!(
            classify_query("Where was Frankenstein's monster created?"),
            QueryType::Factual
        );
        assert_eq!(classify_query("\"Call me Ishmael\""), QueryType::Factual);
    }

    #[test]
    fn test_classify_conceptual_queries() {
        assert_eq!(
            classify_query("Why does Ahab pursue the whale?"),
            QueryType::Conceptual
        );
        assert_eq!(
            classify_query("What themes appear in Gothic literature?"),
            QueryType::Conceptual
        );
        assert_eq!(
            classify_query("How does Austen portray marriage?"),
            QueryType::Conceptual
        );
        assert_eq!(
            classify_query("symbolism of the white whale"),
            QueryType::Conceptual
        );
    }

    #[test]
    fn test_classify_ambiguous_queries() {
        assert_eq!(classify_query("white whale"), QueryType::Ambiguous);
        assert_eq!(classify_query("monster"), QueryType::Ambiguous);
    }

    #[test]
    fn test_generate_query_variations() {
        let config = SelfCorrectionConfig::default();

        // Test stop word removal
        let variations = generate_query_variations("what is the meaning of life", &config);
        assert!(variations.iter().any(|v| v == "meaning life"));

        // Test synonym replacement
        let variations = generate_query_variations("the monster attacks", &config);
        assert!(variations.iter().any(|v| v.contains("creature")));

        // Empty query should return empty variations
        let variations = generate_query_variations("", &config);
        assert!(variations.is_empty());

        // Single word should not generate stop-word removal variation
        let variations = generate_query_variations("whale", &config);
        assert!(!variations.iter().any(|v| v.is_empty()));
    }

    #[test]
    fn test_self_correction_config_default() {
        let config = SelfCorrectionConfig::default();
        assert_eq!(config.max_attempts, 3);
        assert!(config.remove_stop_words);
        assert!(config.try_synonyms);
    }

    #[test]
    fn test_era_to_year_range() {
        // Victorian era
        let (min, max) = era_to_year_range("victorian");
        assert_eq!(min, Some(1837));
        assert_eq!(max, Some(1901));

        // 19th century
        let (min, max) = era_to_year_range("19th century");
        assert_eq!(min, Some(1800));
        assert_eq!(max, Some(1899));

        // Ancient
        let (min, max) = era_to_year_range("ancient");
        assert_eq!(min, Some(-3000));
        assert_eq!(max, Some(500));

        // Case insensitivity
        let (min, max) = era_to_year_range("VICTORIAN");
        assert_eq!(min, Some(1837));
        assert_eq!(max, Some(1901));

        // Unknown era returns None
        let (min, max) = era_to_year_range("unknown_era");
        assert_eq!(min, None);
        assert_eq!(max, None);
    }

    #[test]
    fn test_search_filters_default() {
        let filters = SearchFilters::default();
        assert!(filters.author.is_none());
        assert!(filters.language.is_none());
        assert!(filters.genre.is_none());
        assert!(filters.year_min.is_none());
        assert!(filters.year_max.is_none());
        assert!(filters.era.is_none());
        assert!(filters.length.is_none());
    }

    #[test]
    fn test_book_length() {
        // Test parsing
        assert_eq!(BookLength::parse("short"), Some(BookLength::Short));
        assert_eq!(BookLength::parse("Medium"), Some(BookLength::Medium));
        assert_eq!(BookLength::parse("LONG"), Some(BookLength::Long));
        assert_eq!(BookLength::parse("invalid"), None);

        // Test word count ranges
        let (min, max) = BookLength::Short.word_count_range();
        assert_eq!(min, None);
        assert_eq!(max, Some(15_000));

        let (min, max) = BookLength::Medium.word_count_range();
        assert_eq!(min, Some(15_000));
        assert_eq!(max, Some(60_000));

        let (min, max) = BookLength::Long.word_count_range();
        assert_eq!(min, Some(60_000));
        assert_eq!(max, None);

        // Test display
        assert_eq!(format!("{}", BookLength::Short), "short");
        assert_eq!(format!("{}", BookLength::Medium), "medium");
        assert_eq!(format!("{}", BookLength::Long), "long");
    }

    #[test]
    fn test_filter_store() {
        // Create a temp file for testing
        let temp_dir = std::env::temp_dir();
        let temp_path = temp_dir.join("librarian_test_filters.json");

        // Clean up any existing file
        let _ = std::fs::remove_file(&temp_path);

        // Create a new store
        let mut store = FilterStore::new(&temp_path);
        assert!(store.is_empty());
        assert_eq!(store.len(), 0);

        // Create a filter
        let filters = SearchFilters {
            language: Some("en".to_string()),
            genre: Some("Fiction".to_string()),
            ..Default::default()
        };
        let saved = SavedFilter::new(
            "english-fiction",
            filters,
            Some("English fiction books"),
        );

        // Add the filter
        store.add(saved).unwrap();
        assert!(!store.is_empty());
        assert_eq!(store.len(), 1);
        assert!(store.contains("english-fiction"));

        // Retrieve the filter
        let retrieved = store.get("english-fiction").unwrap();
        assert_eq!(retrieved.name, "english-fiction");
        assert_eq!(
            retrieved.description.as_deref(),
            Some("English fiction books")
        );
        assert_eq!(retrieved.filters.language.as_deref(), Some("en"));
        assert_eq!(retrieved.filters.genre.as_deref(), Some("Fiction"));

        // Test list
        let all = store.list();
        assert_eq!(all.len(), 1);

        // Remove the filter
        let removed = store.remove("english-fiction").unwrap();
        assert!(removed);
        assert!(store.is_empty());

        // Removing non-existent filter returns false
        let removed = store.remove("non-existent").unwrap();
        assert!(!removed);

        // Clean up
        let _ = std::fs::remove_file(&temp_path);
    }
}
