//! API cost tracking for librarian queries
//!
//! Tracks embedding API costs per query and provides summary and breakdown views.

use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

/// Embedding model for cost calculation
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EmbeddingModel {
    /// OpenAI text-embedding-3-small
    OpenAI,
    /// HuggingFace Inference API (free tier)
    HuggingFace,
}

impl EmbeddingModel {
    /// Cost per 1M tokens for this model
    pub fn cost_per_million_tokens(&self) -> f64 {
        match self {
            // OpenAI text-embedding-3-small: $0.02 per 1M tokens (as of 2024)
            EmbeddingModel::OpenAI => 0.02,
            // HuggingFace Inference API: Free tier
            EmbeddingModel::HuggingFace => 0.0,
        }
    }
}

/// A record of a single query and its cost
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryCostRecord {
    /// Timestamp of the query
    pub timestamp: DateTime<Utc>,
    /// The query text
    pub query: String,
    /// Search mode used
    pub mode: String,
    /// Embedding model used
    pub model: EmbeddingModel,
    /// Estimated tokens used
    pub tokens: usize,
    /// Estimated cost in USD
    pub cost: f64,
}

/// Cost tracker for API usage
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CostTracker {
    /// All recorded queries
    records: Vec<QueryCostRecord>,
    /// Total tokens used
    total_tokens: usize,
    /// Total cost in USD
    total_cost: f64,
}

impl Default for CostTracker {
    fn default() -> Self {
        Self::new()
    }
}

impl CostTracker {
    /// Create a new cost tracker
    pub fn new() -> Self {
        Self {
            records: Vec::new(),
            total_tokens: 0,
            total_cost: 0.0,
        }
    }

    /// Load cost tracker from disk
    pub fn load(dir: &Path) -> Result<Self> {
        let path = dir.join("costs.json");
        if path.exists() {
            let data = fs::read_to_string(&path)?;
            Ok(serde_json::from_str(&data)?)
        } else {
            Ok(Self::new())
        }
    }

    /// Save cost tracker to disk
    pub fn save(&self, dir: &Path) -> Result<()> {
        fs::create_dir_all(dir)?;
        let path = dir.join("costs.json");
        let data = serde_json::to_string_pretty(self)?;
        fs::write(&path, data)?;
        Ok(())
    }

    /// Record a query and its estimated cost
    pub fn record_query(&mut self, query: &str, mode: &str, model: EmbeddingModel) {
        // Estimate tokens (rough approximation: ~4 chars per token)
        let tokens = (query.len() / 4).max(1);
        let cost = (tokens as f64 / 1_000_000.0) * model.cost_per_million_tokens();

        let record = QueryCostRecord {
            timestamp: Utc::now(),
            query: query.to_string(),
            mode: mode.to_string(),
            model,
            tokens,
            cost,
        };

        self.records.push(record);
        self.total_tokens += tokens;
        self.total_cost += cost;
    }

    /// Get total tokens used
    pub fn total_tokens(&self) -> usize {
        self.total_tokens
    }

    /// Get total cost
    pub fn total_cost(&self) -> f64 {
        self.total_cost
    }

    /// Get recent queries
    pub fn recent_queries(&self, limit: usize) -> impl Iterator<Item = &QueryCostRecord> {
        self.records.iter().rev().take(limit)
    }

    /// Reset all tracking data
    pub fn reset(&mut self) {
        self.records.clear();
        self.total_tokens = 0;
        self.total_cost = 0.0;
    }

    /// Generate summary string
    pub fn summary(&self) -> String {
        let query_count = self.records.len();
        let avg_tokens = if query_count > 0 {
            self.total_tokens / query_count
        } else {
            0
        };

        format!(
            "Total Queries: {}\n\
             Total Tokens: {}\n\
             Average Tokens/Query: {}\n\
             Total Cost: ${:.6}\n\
             Average Cost/Query: ${:.8}",
            query_count,
            self.total_tokens,
            avg_tokens,
            self.total_cost,
            if query_count > 0 {
                self.total_cost / query_count as f64
            } else {
                0.0
            }
        )
    }

    /// Generate breakdown by query mode
    pub fn breakdown(&self) -> String {
        let mut by_mode: HashMap<String, (usize, usize, f64)> = HashMap::new();

        for record in &self.records {
            let entry = by_mode.entry(record.mode.clone()).or_insert((0, 0, 0.0));
            entry.0 += 1; // query count
            entry.1 += record.tokens; // total tokens
            entry.2 += record.cost; // total cost
        }

        let mut lines = Vec::new();
        lines.push(format!(
            "{:<15} {:>10} {:>12} {:>12}",
            "Mode", "Queries", "Tokens", "Cost"
        ));
        lines.push("-".repeat(51));

        for (mode, (count, tokens, cost)) in by_mode.iter() {
            lines.push(format!(
                "{:<15} {:>10} {:>12} ${:>11.6}",
                mode, count, tokens, cost
            ));
        }

        lines.push("-".repeat(51));
        lines.push(format!(
            "{:<15} {:>10} {:>12} ${:>11.6}",
            "TOTAL",
            self.records.len(),
            self.total_tokens,
            self.total_cost
        ));

        lines.join("\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cost_tracker_new() {
        let tracker = CostTracker::new();
        assert_eq!(tracker.total_tokens(), 0);
        assert!(tracker.total_cost().abs() < f64::EPSILON);
    }

    #[test]
    fn test_record_query_openai() {
        let mut tracker = CostTracker::new();
        tracker.record_query(
            "What is the meaning of life?",
            "hybrid",
            EmbeddingModel::OpenAI,
        );

        assert!(tracker.total_tokens() > 0);
        assert!(tracker.total_cost() > 0.0);
    }

    #[test]
    fn test_record_query_huggingface() {
        let mut tracker = CostTracker::new();
        tracker.record_query("test query", "semantic", EmbeddingModel::HuggingFace);

        assert!(tracker.total_tokens() > 0);
        // HuggingFace is free
        assert!(tracker.total_cost().abs() < f64::EPSILON);
    }

    #[test]
    fn test_reset() {
        let mut tracker = CostTracker::new();
        tracker.record_query("query 1", "hybrid", EmbeddingModel::OpenAI);
        tracker.record_query("query 2", "keyword", EmbeddingModel::OpenAI);

        assert!(!tracker.records.is_empty());

        tracker.reset();

        assert_eq!(tracker.records.len(), 0);
        assert_eq!(tracker.total_tokens(), 0);
        assert!(tracker.total_cost().abs() < f64::EPSILON);
    }

    #[test]
    fn test_breakdown() {
        let mut tracker = CostTracker::new();
        tracker.record_query("query 1", "hybrid", EmbeddingModel::OpenAI);
        tracker.record_query("query 2", "semantic", EmbeddingModel::OpenAI);
        tracker.record_query("query 3", "hybrid", EmbeddingModel::OpenAI);

        let breakdown = tracker.breakdown();
        assert!(breakdown.contains("hybrid"));
        assert!(breakdown.contains("semantic"));
        assert!(breakdown.contains("TOTAL"));
    }
}
