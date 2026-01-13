//! Indexer pipeline for ingesting books into OpenSearch

use crate::config::{BookMetadata, BookSearchConfig};
use crate::downloader::{chunk_text, strip_gutenberg_boilerplate, GutenbergDownloader};
use crate::lang::{detect_language, language_name};
use anyhow::{Context, Result};
use dashflow::core::embeddings::Embeddings;
use dashflow::embed;
use serde_json::json;
use std::sync::Arc;
use tracing::{info, instrument, warn};

/// Statistics from indexing a book
#[derive(Debug, Clone)]
pub struct IndexStats {
    pub book_id: u32,
    pub title: String,
    pub author: String,
    pub language: String,
    pub genre: String,
    pub year: Option<i32>,
    pub word_count: usize,
    pub chunks_created: usize,
    pub chunks_indexed: usize,
}

/// Pipeline for indexing books into OpenSearch
pub struct IndexerPipeline {
    downloader: GutenbergDownloader,
    embeddings: Arc<dyn Embeddings>,
    opensearch_url: String,
    index_name: String,
    chunk_size: usize,
    chunk_overlap: usize,
    client: reqwest::Client,
    /// Auto-detect language for books with unknown/default language
    auto_detect_language: bool,
}

impl IndexerPipeline {
    /// Create a new indexer pipeline
    pub fn new(config: &BookSearchConfig, embeddings: Arc<dyn Embeddings>) -> Self {
        Self {
            downloader: GutenbergDownloader::new(config.cache_dir.clone()),
            embeddings,
            opensearch_url: config.opensearch_url.clone(),
            index_name: config.index_name.clone(),
            chunk_size: config.chunk_size,
            chunk_overlap: config.chunk_overlap,
            client: reqwest::Client::new(),
            auto_detect_language: false,
        }
    }

    /// Create a new indexer pipeline with language auto-detection enabled
    pub fn with_language_detection(
        config: &BookSearchConfig,
        embeddings: Arc<dyn Embeddings>,
    ) -> Self {
        let mut pipeline = Self::new(config, embeddings);
        pipeline.auto_detect_language = true;
        pipeline
    }

    /// Enable or disable language auto-detection
    pub fn set_auto_detect_language(&mut self, enabled: bool) {
        self.auto_detect_language = enabled;
    }

    /// Create the OpenSearch index with proper mappings for hybrid search
    #[instrument(skip(self))]
    pub async fn create_index(&self, embedding_dim: usize) -> Result<()> {
        let url = format!("{}/{}", self.opensearch_url, self.index_name);

        // Check if index exists
        let response = self.client.head(&url).send().await?;
        if response.status().is_success() {
            info!("Index '{}' already exists", self.index_name);
            return Ok(());
        }

        // Create index with hybrid search mappings
        let mapping = json!({
            "settings": {
                "index": {
                    "knn": true,
                    "knn.algo_param.ef_search": 100
                },
                "number_of_shards": 1,
                "number_of_replicas": 0
            },
            "mappings": {
                "properties": {
                    "content": {
                        "type": "text",
                        "analyzer": "english"
                    },
                    "embedding": {
                        "type": "knn_vector",
                        "dimension": embedding_dim,
                        "method": {
                            "name": "hnsw",
                            "space_type": "cosinesimil",
                            "engine": "lucene",
                            "parameters": {
                                "ef_construction": 128,
                                "m": 16
                            }
                        }
                    },
                    "book_id": { "type": "keyword" },
                    "title": { "type": "text", "fields": { "keyword": { "type": "keyword" } } },
                    "author": { "type": "text", "fields": { "keyword": { "type": "keyword" } } },
                    "language": { "type": "keyword" },
                    "genre": { "type": "keyword" },
                    "year": { "type": "integer" },
                    "word_count": { "type": "integer" },
                    "chunk_index": { "type": "integer" },
                    "source": { "type": "keyword" }
                }
            }
        });

        let response = self.client.put(&url).json(&mapping).send().await?;

        if !response.status().is_success() {
            let error_text = response.text().await?;
            anyhow::bail!("Failed to create index: {}", error_text);
        }

        info!(
            "Created index '{}' with hybrid search mappings",
            self.index_name
        );
        Ok(())
    }

    /// Index a single book with metadata
    #[instrument(skip(self), fields(book_id = %book.id, title = %book.title))]
    pub async fn index_book(&self, book: &BookMetadata) -> Result<IndexStats> {
        info!(
            "Processing: {} by {} ({}, {})",
            book.title, book.author, book.language, book.genre
        );

        // Download book
        let text = self.downloader.download(book.id).await?;

        // Strip boilerplate
        let clean_text = strip_gutenberg_boilerplate(&text);

        // Auto-detect language if enabled and language is default "en" (possibly unknown)
        let detected_language = if self.auto_detect_language && book.language == "en" {
            let detected = detect_language(clean_text);
            if detected != "en" {
                info!(
                    "Auto-detected language: {} ({}) for '{}'",
                    language_name(detected),
                    detected,
                    book.title
                );
            }
            detected.to_string()
        } else {
            book.language.clone()
        };

        // Calculate word count for the entire book
        let word_count = clean_text.split_whitespace().count();
        info!("{} words in book", word_count);

        // Chunk text
        let chunks = chunk_text(clean_text, self.chunk_size, self.chunk_overlap);
        let chunks_created = chunks.len();
        info!("{} chunks created", chunks_created);

        // Generate embeddings for all chunks using graph API
        info!("Generating embeddings...");
        let embeddings = embed(Arc::clone(&self.embeddings), &chunks)
            .await
            .context("Failed to generate embeddings")?;

        // Bulk index to OpenSearch in batches (to avoid request size limits)
        info!("Indexing to OpenSearch...");
        const BATCH_SIZE: usize = 500; // ~2.5MB per batch with 1024-dim embeddings
        let url = format!("{}/_bulk", self.opensearch_url);
        let mut chunks_indexed = 0;
        let total_batches = chunks_created.div_ceil(BATCH_SIZE);

        for (batch_num, batch) in chunks
            .iter()
            .zip(embeddings.iter())
            .enumerate()
            .collect::<Vec<_>>()
            .chunks(BATCH_SIZE)
            .enumerate()
        {
            let mut bulk_body = String::new();

            for (i, (chunk, embedding)) in batch {
                // Bulk action line
                bulk_body.push_str(&format!(
                    r#"{{"index":{{"_index":"{}"}}}}"#,
                    self.index_name
                ));
                bulk_body.push('\n');

                // Document line with full metadata (uses detected language if auto-detection is enabled)
                let mut doc = json!({
                    "content": chunk,
                    "embedding": embedding,
                    "book_id": book.id.to_string(),
                    "title": &book.title,
                    "author": &book.author,
                    "language": &detected_language,
                    "genre": &book.genre,
                    "word_count": word_count,
                    "chunk_index": i,
                    "source": format!("gutenberg:{}", book.id)
                });
                // Add year if present
                if let Some(year) = book.year {
                    doc["year"] = json!(year);
                }
                bulk_body.push_str(&doc.to_string());
                bulk_body.push('\n');
            }

            let response = self
                .client
                .post(&url)
                .header("Content-Type", "application/x-ndjson")
                .body(bulk_body)
                .send()
                .await?;

            if !response.status().is_success() {
                let error_text = response.text().await?;
                anyhow::bail!(
                    "Bulk index failed (batch {}/{}): {}",
                    batch_num + 1,
                    total_batches,
                    error_text
                );
            }

            // Parse response to check for errors
            let result: serde_json::Value = response.json().await?;
            let errors = result
                .get("errors")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            if errors {
                warn!(
                    "Some documents failed to index in batch {}/{}",
                    batch_num + 1,
                    total_batches
                );
            }

            chunks_indexed += batch.len();
            if total_batches > 1 {
                info!(
                    "Indexed batch {}/{} ({}/{} chunks)",
                    batch_num + 1,
                    total_batches,
                    chunks_indexed,
                    chunks_created
                );
            }
        }

        info!("Indexed {} chunks for '{}'", chunks_indexed, book.title);

        Ok(IndexStats {
            book_id: book.id,
            title: book.title.clone(),
            author: book.author.clone(),
            language: detected_language,
            genre: book.genre.clone(),
            year: book.year,
            word_count,
            chunks_created,
            chunks_indexed,
        })
    }

    /// Index multiple books using metadata
    pub async fn index_books_with_metadata(
        &self,
        books: &[BookMetadata],
    ) -> Vec<Result<IndexStats>> {
        let mut results = Vec::new();

        for book in books {
            results.push(self.index_book(book).await);
        }

        results
    }

    /// Index multiple books (legacy tuple format - converts to metadata)
    pub async fn index_books(&self, books: &[(u32, &str, &str)]) -> Vec<Result<IndexStats>> {
        let book_metadata: Vec<BookMetadata> = books
            .iter()
            .map(|(id, title, author)| BookMetadata::from_tuple(*id, title, author))
            .collect();
        self.index_books_with_metadata(&book_metadata).await
    }

    /// Get index statistics
    pub async fn get_index_stats(&self) -> Result<IndexStatsResponse> {
        let url = format!("{}/{}/_count", self.opensearch_url, self.index_name);
        let response: serde_json::Value = self.client.get(&url).send().await?.json().await?;
        let count = response.get("count").and_then(|v| v.as_u64()).unwrap_or(0);

        Ok(IndexStatsResponse {
            index_name: self.index_name.clone(),
            document_count: count,
        })
    }

    /// Get list of book IDs already in the index (for incremental indexing)
    pub async fn get_indexed_book_ids(&self) -> Result<std::collections::HashSet<u32>> {
        let url = format!("{}/{}/_search", self.opensearch_url, self.index_name);

        // Use terms aggregation to get all unique book_id values
        let query = json!({
            "size": 0,
            "aggs": {
                "book_ids": {
                    "terms": {
                        "field": "book_id",
                        "size": 100000  // Support up to 100K unique books
                    }
                }
            }
        });

        let response = self.client.post(&url).json(&query).send().await?;

        if !response.status().is_success() {
            // Index might not exist yet
            info!(
                "Index '{}' does not exist or query failed, returning empty set",
                self.index_name
            );
            return Ok(std::collections::HashSet::new());
        }

        let result: serde_json::Value = response.json().await?;

        let mut book_ids = std::collections::HashSet::new();
        if let Some(buckets) = result
            .get("aggregations")
            .and_then(|a| a.get("book_ids"))
            .and_then(|b| b.get("buckets"))
            .and_then(|b| b.as_array())
        {
            for bucket in buckets {
                if let Some(key) = bucket.get("key").and_then(|k| k.as_str()) {
                    if let Ok(id) = key.parse::<u32>() {
                        book_ids.insert(id);
                    }
                }
            }
        }

        info!("Found {} existing books in index", book_ids.len());
        Ok(book_ids)
    }

    /// Index books incrementally (skip books already in index)
    pub async fn index_books_incremental(
        &self,
        books: &[BookMetadata],
    ) -> Result<IncrementalIndexResult> {
        // Get existing book IDs
        let existing_ids = self.get_indexed_book_ids().await?;

        // Filter to only new books
        let new_books: Vec<&BookMetadata> = books
            .iter()
            .filter(|b| !existing_ids.contains(&b.id))
            .collect();

        let skipped_count = books.len() - new_books.len();
        if skipped_count > 0 {
            info!(
                "Skipping {} already-indexed books, indexing {} new books",
                skipped_count,
                new_books.len()
            );
        }

        // Index new books
        let mut results = Vec::new();
        let mut indexed_count = 0;
        let mut failed_count = 0;

        for book in new_books {
            match self.index_book(book).await {
                Ok(stats) => {
                    indexed_count += 1;
                    results.push(Ok(stats));
                }
                Err(e) => {
                    failed_count += 1;
                    warn!("Failed to index book {}: {}", book.id, e);
                    results.push(Err(e));
                }
            }
        }

        Ok(IncrementalIndexResult {
            total_requested: books.len(),
            skipped_existing: skipped_count,
            indexed_new: indexed_count,
            failed: failed_count,
            results,
        })
    }
}

/// Result from incremental indexing
#[derive(Debug)]
pub struct IncrementalIndexResult {
    pub total_requested: usize,
    pub skipped_existing: usize,
    pub indexed_new: usize,
    pub failed: usize,
    pub results: Vec<Result<IndexStats>>,
}

/// Response from index stats query
#[derive(Debug)]
pub struct IndexStatsResponse {
    pub index_name: String,
    pub document_count: u64,
}
