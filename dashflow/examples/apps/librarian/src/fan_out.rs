//! Fan Out parallel search demonstrating DashFlow's parallel execution
//!
//! This module showcases how DashFlow can execute multiple search strategies
//! simultaneously, merging results for better coverage and quality.

use crate::search::{HybridSearcher, SearchFilters, SearchResult};
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{mpsc, Semaphore};
use tracing::{info, instrument, warn};

/// Search strategy for fan out
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum SearchStrategy {
    /// Semantic search using embeddings
    Semantic,
    /// Keyword search using BM25
    Keyword,
    /// Hybrid combination
    Hybrid,
    /// Filtered by author
    FilteredAuthor(String),
    /// Filtered by book
    FilteredBook(String),
}

impl std::fmt::Display for SearchStrategy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Semantic => write!(f, "semantic"),
            Self::Keyword => write!(f, "keyword"),
            Self::Hybrid => write!(f, "hybrid"),
            Self::FilteredAuthor(a) => write!(f, "author:{}", a),
            Self::FilteredBook(b) => write!(f, "book:{}", b),
        }
    }
}

/// Result from a single search strategy
#[derive(Clone, Debug)]
pub struct StrategyResult {
    /// The strategy used
    pub strategy: SearchStrategy,
    /// Results from this strategy
    pub results: Vec<SearchResult>,
    /// Execution time
    pub execution_time: Duration,
}

/// Combined result from fan out search
#[derive(Clone, Debug)]
pub struct FanOutResult {
    /// Original query
    pub query: String,
    /// Merged and deduplicated results
    pub results: Vec<SearchResult>,
    /// Per-strategy results (for telemetry)
    pub strategy_results: Vec<StrategyResult>,
    /// Total execution time (parallel)
    pub total_time: Duration,
    /// Sum of individual strategy times (shows parallelism benefit)
    pub sequential_time: Duration,
    /// Parallelism speedup factor
    pub speedup: f64,
}

/// Fan out searcher that executes multiple strategies in parallel
pub struct FanOutSearcher {
    searcher: Arc<HybridSearcher>,
    /// Maximum concurrent searches
    max_concurrent: usize,
}

impl FanOutSearcher {
    /// Create a new fan out searcher
    pub fn new(searcher: Arc<HybridSearcher>) -> Self {
        Self {
            searcher,
            max_concurrent: 5,
        }
    }

    /// Set maximum concurrent searches
    pub fn with_max_concurrent(mut self, max: usize) -> Self {
        self.max_concurrent = max;
        self
    }

    /// Execute fan out search with multiple strategies
    #[instrument(skip(self), fields(query = %query, strategies = strategies.len()))]
    pub async fn search(
        &self,
        query: &str,
        strategies: Vec<SearchStrategy>,
        limit_per_strategy: usize,
    ) -> Result<FanOutResult> {
        let start = Instant::now();

        // Semaphore to limit concurrent searches
        let semaphore = Arc::new(Semaphore::new(self.max_concurrent));

        // Execute all strategies in parallel
        let mut handles = Vec::new();
        for strategy in strategies.clone() {
            let searcher = Arc::clone(&self.searcher);
            let query = query.to_string();
            let permit = Arc::clone(&semaphore).acquire_owned().await?;

            handles.push(tokio::spawn(async move {
                let _permit = permit; // Hold permit while executing
                let result =
                    execute_strategy(&searcher, &query, &strategy, limit_per_strategy).await;
                (strategy, result)
            }));
        }

        // Collect results
        let mut strategy_results = Vec::new();
        let mut all_results: Vec<SearchResult> = Vec::new();

        for handle in handles {
            match handle.await {
                Ok((strategy, Ok(result))) => {
                    info!(
                        "Strategy {} returned {} results in {:?}",
                        strategy,
                        result.results.len(),
                        result.execution_time
                    );
                    all_results.extend(result.results.clone());
                    strategy_results.push(result);
                }
                Ok((strategy, Err(e))) => {
                    warn!("Strategy {} failed: {}", strategy, e);
                }
                Err(e) => {
                    warn!("Task join error: {}", e);
                }
            }
        }

        // Calculate timing metrics
        let total_time = start.elapsed();
        let sequential_time: Duration = strategy_results.iter().map(|r| r.execution_time).sum();
        let speedup = if total_time.as_millis() > 0 {
            sequential_time.as_millis() as f64 / total_time.as_millis() as f64
        } else {
            1.0
        };

        // Merge and deduplicate results
        let merged_results = merge_results(all_results, limit_per_strategy * 2);

        info!(
            "Fan out search completed in {:?} (speedup: {:.2}x)",
            total_time, speedup
        );

        // Record metrics
        metrics::counter!("librarian_fan_out_searches_total").increment(1);
        metrics::histogram!("librarian_fan_out_latency_ms").record(total_time.as_millis() as f64);
        metrics::histogram!("librarian_fan_out_speedup").record(speedup);
        metrics::gauge!("librarian_fan_out_strategies").set(strategies.len() as f64);

        Ok(FanOutResult {
            query: query.to_string(),
            results: merged_results,
            strategy_results,
            total_time,
            sequential_time,
            speedup,
        })
    }

    /// Smart fan out: automatically choose strategies based on query
    #[instrument(skip(self), fields(query = %query))]
    pub async fn smart_search(
        &self,
        query: &str,
        limit: usize,
        known_authors: &[String],
    ) -> Result<FanOutResult> {
        let mut strategies = vec![SearchStrategy::Semantic, SearchStrategy::Keyword];

        // Add author filters if authors are mentioned in query
        for author in known_authors {
            let author_lower = author.to_lowercase();
            if query.to_lowercase().contains(&author_lower) {
                strategies.push(SearchStrategy::FilteredAuthor(author.clone()));
            }
        }

        // Always add hybrid if we have other strategies
        if strategies.len() > 2 {
            strategies.push(SearchStrategy::Hybrid);
        }

        self.search(query, strategies, limit).await
    }

    /// Execute fan out search with streaming results
    ///
    /// Results are sent through the channel as each strategy completes,
    /// allowing callers to display results as they arrive.
    #[instrument(skip(self, tx), fields(query = %query, strategies = strategies.len()))]
    pub async fn search_streaming(
        &self,
        query: &str,
        strategies: Vec<SearchStrategy>,
        limit_per_strategy: usize,
        tx: mpsc::Sender<StrategyResult>,
    ) -> Result<()> {
        // Semaphore to limit concurrent searches
        let semaphore = Arc::new(Semaphore::new(self.max_concurrent));

        // Execute all strategies in parallel
        let mut handles = Vec::new();
        for strategy in strategies {
            let searcher = Arc::clone(&self.searcher);
            let query = query.to_string();
            let permit = Arc::clone(&semaphore).acquire_owned().await?;
            let tx = tx.clone();

            handles.push(tokio::spawn(async move {
                let _permit = permit; // Hold permit while executing
                let result =
                    execute_strategy(&searcher, &query, &strategy, limit_per_strategy).await;
                if let Ok(strategy_result) = result {
                    let _ = tx.send(strategy_result).await;
                }
            }));
        }

        // Wait for all to complete
        for handle in handles {
            let _ = handle.await;
        }

        Ok(())
    }
}

/// Execute a single search strategy
async fn execute_strategy(
    searcher: &HybridSearcher,
    query: &str,
    strategy: &SearchStrategy,
    limit: usize,
) -> Result<StrategyResult> {
    let start = Instant::now();

    let results = match strategy {
        SearchStrategy::Semantic => searcher.search_semantic(query, limit).await?,
        SearchStrategy::Keyword => searcher.search_keyword(query, limit).await?,
        SearchStrategy::Hybrid => searcher.search(query, limit).await?,
        SearchStrategy::FilteredAuthor(author) => {
            let filters = SearchFilters {
                author: Some(author.clone()),
                ..Default::default()
            };
            searcher.search_filtered(query, &filters, limit).await?
        }
        SearchStrategy::FilteredBook(book_id) => {
            let filters = SearchFilters {
                book_id: Some(book_id.clone()),
                ..Default::default()
            };
            searcher.search_filtered(query, &filters, limit).await?
        }
    };

    Ok(StrategyResult {
        strategy: strategy.clone(),
        results,
        execution_time: start.elapsed(),
    })
}

/// Merge and deduplicate results from multiple strategies
fn merge_results(mut results: Vec<SearchResult>, limit: usize) -> Vec<SearchResult> {
    // Track seen content to deduplicate
    let mut seen: HashMap<String, usize> = HashMap::new();
    let mut merged: Vec<SearchResult> = Vec::new();

    // Sort by score descending
    results.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    for result in results {
        // Create dedup key: book_id + chunk_index
        let key = format!("{}:{}", result.book_id, result.chunk_index);

        if let Some(&existing_idx) = seen.get(&key) {
            // Update score if this one is higher
            if result.score > merged[existing_idx].score {
                merged[existing_idx] = result;
            }
        } else {
            seen.insert(key, merged.len());
            merged.push(result);
        }
    }

    // Take top N
    merged.truncate(limit);
    merged
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strategy_display() {
        assert_eq!(SearchStrategy::Semantic.to_string(), "semantic");
        assert_eq!(SearchStrategy::Keyword.to_string(), "keyword");
        assert_eq!(
            SearchStrategy::FilteredAuthor("Austen".to_string()).to_string(),
            "author:Austen"
        );
    }

    #[test]
    fn test_merge_results_dedup() {
        let results = vec![
            SearchResult {
                content: "Content 1".to_string(),
                title: "Book 1".to_string(),
                author: "Author 1".to_string(),
                book_id: "1".to_string(),
                chunk_index: 0,
                score: 0.5,
            },
            SearchResult {
                content: "Content 1 duplicate".to_string(),
                title: "Book 1".to_string(),
                author: "Author 1".to_string(),
                book_id: "1".to_string(),
                chunk_index: 0, // Same chunk
                score: 0.8,     // Higher score
            },
            SearchResult {
                content: "Content 2".to_string(),
                title: "Book 2".to_string(),
                author: "Author 2".to_string(),
                book_id: "2".to_string(),
                chunk_index: 0,
                score: 0.6,
            },
        ];

        let merged = merge_results(results, 10);

        // Should have 2 unique results
        assert_eq!(merged.len(), 2);
        // Highest score for duplicate should win
        assert!((merged[0].score - 0.8).abs() < f32::EPSILON);
    }
}
