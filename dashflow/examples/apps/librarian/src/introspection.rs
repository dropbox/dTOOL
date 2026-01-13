//! Introspection and self-improvement module for the Superhuman Librarian
//!
//! This module provides:
//! - Trace storage: Record search executions with timing and quality
//! - Trace analysis: Identify patterns, failures, and improvement opportunities
//! - Improvement suggestions: Generate actionable recommendations
//! - Improvement application: Apply and test improvements

use anyhow::{anyhow, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use tracing::info;

/// A single search execution trace
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchTrace {
    /// Unique trace ID
    pub id: String,
    /// Timestamp of execution
    pub timestamp: DateTime<Utc>,
    /// Original query
    pub query: String,
    /// Search strategy used (semantic, keyword, hybrid, fan_out)
    pub strategy: String,
    /// Number of results returned
    pub result_count: usize,
    /// Relevance scores of results (0.0 to 1.0)
    pub relevance_scores: Vec<f32>,
    /// Average relevance score
    pub avg_relevance: f32,
    /// Total execution time in milliseconds
    pub duration_ms: u64,
    /// Breakdown of timing by phase
    pub timing_breakdown: HashMap<String, u64>,
    /// Whether the search succeeded
    pub success: bool,
    /// Error message if failed
    pub error: Option<String>,
    /// Search filters applied
    pub filters: Option<SearchFilters>,
    /// Query intent classification (if detected)
    pub intent: Option<String>,
}

/// Filters applied during search
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SearchFilters {
    pub author: Option<String>,
    pub title: Option<String>,
    pub book_id: Option<String>,
}

impl SearchTrace {
    /// Create a new search trace
    pub fn new(query: impl Into<String>, strategy: impl Into<String>) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            timestamp: Utc::now(),
            query: query.into(),
            strategy: strategy.into(),
            result_count: 0,
            relevance_scores: Vec::new(),
            avg_relevance: 0.0,
            duration_ms: 0,
            timing_breakdown: HashMap::new(),
            success: true,
            error: None,
            filters: None,
            intent: None,
        }
    }

    /// Record results
    pub fn with_results(mut self, count: usize, relevance_scores: Vec<f32>) -> Self {
        self.result_count = count;
        self.avg_relevance = if relevance_scores.is_empty() {
            0.0
        } else {
            relevance_scores.iter().sum::<f32>() / relevance_scores.len() as f32
        };
        self.relevance_scores = relevance_scores;
        self
    }

    /// Record timing
    pub fn with_duration(mut self, duration_ms: u64) -> Self {
        self.duration_ms = duration_ms;
        self
    }

    /// Add timing breakdown
    pub fn with_timing(mut self, phase: impl Into<String>, ms: u64) -> Self {
        self.timing_breakdown.insert(phase.into(), ms);
        self
    }

    /// Mark as failed
    pub fn with_error(mut self, error: impl Into<String>) -> Self {
        self.success = false;
        self.error = Some(error.into());
        self
    }

    /// Add filters
    pub fn with_filters(mut self, filters: SearchFilters) -> Self {
        self.filters = Some(filters);
        self
    }

    /// Add intent
    pub fn with_intent(mut self, intent: impl Into<String>) -> Self {
        self.intent = Some(intent.into());
        self
    }

    /// Is this a low-quality result?
    pub fn is_low_quality(&self) -> bool {
        self.avg_relevance < 0.5 || self.result_count == 0
    }
}

/// Analysis of search patterns
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraceAnalysis {
    /// Total traces analyzed
    pub total_traces: usize,
    /// Success rate (0.0 to 1.0)
    pub success_rate: f32,
    /// Average relevance score
    pub avg_relevance: f32,
    /// Average latency in ms
    pub avg_latency_ms: u64,
    /// P50 latency
    pub p50_latency_ms: u64,
    /// P95 latency
    pub p95_latency_ms: u64,
    /// Query patterns that often fail
    pub failure_patterns: Vec<FailurePattern>,
    /// Slowest phases
    pub bottlenecks: Vec<Bottleneck>,
    /// Strategy performance comparison
    pub strategy_stats: HashMap<String, StrategyStats>,
}

/// A pattern of failing queries
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FailurePattern {
    /// Pattern description
    pub pattern: String,
    /// Number of occurrences
    pub count: usize,
    /// Example queries
    pub examples: Vec<String>,
    /// Suggested fix
    pub suggestion: String,
}

/// A performance bottleneck
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bottleneck {
    /// Phase name
    pub phase: String,
    /// Average time in ms
    pub avg_ms: u64,
    /// Percentage of total time
    pub percentage: f32,
}

/// Statistics for a search strategy
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct StrategyStats {
    pub count: usize,
    pub avg_relevance: f32,
    pub avg_latency_ms: u64,
    pub success_rate: f32,
}

/// An improvement suggestion
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Improvement {
    /// Unique ID
    pub id: usize,
    /// Category of improvement
    pub category: ImprovementCategory,
    /// Description
    pub description: String,
    /// Expected impact
    pub expected_impact: String,
    /// Implementation difficulty (1-5)
    pub difficulty: u8,
    /// Status
    pub status: ImprovementStatus,
    /// Created timestamp
    pub created: DateTime<Utc>,
    /// Applied timestamp
    pub applied: Option<DateTime<Utc>>,
    /// Verification result
    pub verification: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ImprovementCategory {
    QueryRouting,
    CoverageGap,
    Performance,
    EmbeddingQuality,
    IndexConfiguration,
}

impl std::fmt::Display for ImprovementCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ImprovementCategory::QueryRouting => write!(f, "Query Routing"),
            ImprovementCategory::CoverageGap => write!(f, "Coverage Gap"),
            ImprovementCategory::Performance => write!(f, "Performance"),
            ImprovementCategory::EmbeddingQuality => write!(f, "Embedding Quality"),
            ImprovementCategory::IndexConfiguration => write!(f, "Index Configuration"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ImprovementStatus {
    Suggested,
    Approved,
    Applied,
    Verified,
    Failed,
}

impl std::fmt::Display for ImprovementStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ImprovementStatus::Suggested => write!(f, "Suggested"),
            ImprovementStatus::Approved => write!(f, "Approved"),
            ImprovementStatus::Applied => write!(f, "Applied"),
            ImprovementStatus::Verified => write!(f, "Verified"),
            ImprovementStatus::Failed => write!(f, "Failed"),
        }
    }
}

/// Storage for traces and improvements
pub struct TraceStore {
    data_dir: PathBuf,
    traces: Vec<SearchTrace>,
    improvements: Vec<Improvement>,
    max_traces: usize,
}

impl TraceStore {
    /// Create a new trace store
    pub fn new(data_dir: impl AsRef<Path>) -> Result<Self> {
        let data_dir = data_dir.as_ref().to_path_buf();
        fs::create_dir_all(&data_dir)?;
        fs::create_dir_all(data_dir.join("traces"))?;
        fs::create_dir_all(data_dir.join("improvements"))?;

        let mut store = Self {
            data_dir,
            traces: Vec::new(),
            improvements: Vec::new(),
            max_traces: 1000,
        };
        store.load()?;
        Ok(store)
    }

    /// Load traces and improvements from disk
    fn load(&mut self) -> Result<()> {
        // Load traces
        let traces_file = self.data_dir.join("traces.json");
        if traces_file.exists() {
            let content = fs::read_to_string(&traces_file)?;
            self.traces = serde_json::from_str(&content).unwrap_or_default();
        }

        // Load improvements
        let improvements_file = self.data_dir.join("improvements.json");
        if improvements_file.exists() {
            let content = fs::read_to_string(&improvements_file)?;
            self.improvements = serde_json::from_str(&content).unwrap_or_default();
        }

        Ok(())
    }

    /// Save traces and improvements to disk
    pub fn save(&self) -> Result<()> {
        // Save traces
        let traces_file = self.data_dir.join("traces.json");
        let content = serde_json::to_string_pretty(&self.traces)?;
        fs::write(&traces_file, content)?;

        // Save improvements
        let improvements_file = self.data_dir.join("improvements.json");
        let content = serde_json::to_string_pretty(&self.improvements)?;
        fs::write(&improvements_file, content)?;

        Ok(())
    }

    /// Add a trace
    pub fn add_trace(&mut self, trace: SearchTrace) {
        self.traces.push(trace);

        // Trim old traces if needed
        if self.traces.len() > self.max_traces {
            self.traces.remove(0);
        }
    }

    /// Get the most recent trace
    pub fn last_trace(&self) -> Option<&SearchTrace> {
        self.traces.last()
    }

    /// Get the N most recent traces
    pub fn recent_traces(&self, n: usize) -> Vec<&SearchTrace> {
        self.traces.iter().rev().take(n).collect()
    }

    /// Get all traces
    pub fn traces(&self) -> &[SearchTrace] {
        &self.traces
    }

    /// Get all improvements
    pub fn improvements(&self) -> &[Improvement] {
        &self.improvements
    }

    /// Add an improvement suggestion
    pub fn add_improvement(&mut self, mut improvement: Improvement) {
        improvement.id = self.improvements.len() + 1;
        self.improvements.push(improvement);
    }

    /// Get an improvement by ID
    pub fn get_improvement(&self, id: usize) -> Option<&Improvement> {
        self.improvements.iter().find(|i| i.id == id)
    }

    /// Get a mutable improvement by ID
    pub fn get_improvement_mut(&mut self, id: usize) -> Option<&mut Improvement> {
        self.improvements.iter_mut().find(|i| i.id == id)
    }

    /// Analyze all traces
    pub fn analyze(&self) -> TraceAnalysis {
        if self.traces.is_empty() {
            return TraceAnalysis {
                total_traces: 0,
                success_rate: 0.0,
                avg_relevance: 0.0,
                avg_latency_ms: 0,
                p50_latency_ms: 0,
                p95_latency_ms: 0,
                failure_patterns: Vec::new(),
                bottlenecks: Vec::new(),
                strategy_stats: HashMap::new(),
            };
        }

        let total = self.traces.len();
        let successes = self.traces.iter().filter(|t| t.success).count();
        let success_rate = successes as f32 / total as f32;

        let avg_relevance: f32 =
            self.traces.iter().map(|t| t.avg_relevance).sum::<f32>() / total as f32;

        // Calculate latency stats
        let mut latencies: Vec<u64> = self.traces.iter().map(|t| t.duration_ms).collect();
        latencies.sort();
        let avg_latency_ms = latencies.iter().sum::<u64>() / total as u64;
        let p50_latency_ms = latencies.get(total / 2).copied().unwrap_or(0);
        let p95_latency_ms = latencies.get(total * 95 / 100).copied().unwrap_or(0);

        // Find failure patterns
        let failure_patterns = self.find_failure_patterns();

        // Find bottlenecks
        let bottlenecks = self.find_bottlenecks();

        // Calculate strategy stats
        let strategy_stats = self.calculate_strategy_stats();

        TraceAnalysis {
            total_traces: total,
            success_rate,
            avg_relevance,
            avg_latency_ms,
            p50_latency_ms,
            p95_latency_ms,
            failure_patterns,
            bottlenecks,
            strategy_stats,
        }
    }

    /// Find patterns in failing queries
    fn find_failure_patterns(&self) -> Vec<FailurePattern> {
        let mut patterns = Vec::new();

        // Pattern 1: Low-quality results with specific keywords
        let low_quality: Vec<_> = self.traces.iter().filter(|t| t.is_low_quality()).collect();
        if !low_quality.is_empty() {
            // Check for symbolism/theme queries
            let symbolism_queries: Vec<_> = low_quality
                .iter()
                .filter(|t| {
                    let q = t.query.to_lowercase();
                    q.contains("symbol") || q.contains("theme") || q.contains("meaning")
                })
                .collect();

            if symbolism_queries.len() >= 2 {
                patterns.push(FailurePattern {
                    pattern: "Symbolism/theme queries have low relevance".to_string(),
                    count: symbolism_queries.len(),
                    examples: symbolism_queries
                        .iter()
                        .take(3)
                        .map(|t| t.query.clone())
                        .collect(),
                    suggestion: "Route symbolism queries to theme search first".to_string(),
                });
            }

            // Check for character queries that fail
            let character_queries: Vec<_> = low_quality
                .iter()
                .filter(|t| {
                    let q = t.query.to_lowercase();
                    q.contains("who is") || q.contains("character") || q.contains("relationship")
                })
                .collect();

            if character_queries.len() >= 2 {
                patterns.push(FailurePattern {
                    pattern: "Character queries have low relevance".to_string(),
                    count: character_queries.len(),
                    examples: character_queries
                        .iter()
                        .take(3)
                        .map(|t| t.query.clone())
                        .collect(),
                    suggestion: "Use character analysis for character-related queries".to_string(),
                });
            }
        }

        // Pattern 2: Semantic search alone fails but keyword succeeds
        let semantic_only: Vec<_> = self
            .traces
            .iter()
            .filter(|t| t.strategy == "semantic" && t.is_low_quality())
            .collect();
        if semantic_only.len() >= 3 {
            patterns.push(FailurePattern {
                pattern: "Semantic-only search often fails".to_string(),
                count: semantic_only.len(),
                examples: semantic_only
                    .iter()
                    .take(3)
                    .map(|t| t.query.clone())
                    .collect(),
                suggestion: "Use hybrid search as default instead of semantic-only".to_string(),
            });
        }

        patterns
    }

    /// Find performance bottlenecks
    fn find_bottlenecks(&self) -> Vec<Bottleneck> {
        let mut phase_totals: HashMap<String, Vec<u64>> = HashMap::new();

        for trace in &self.traces {
            for (phase, ms) in &trace.timing_breakdown {
                phase_totals.entry(phase.clone()).or_default().push(*ms);
            }
        }

        let mut bottlenecks: Vec<_> = phase_totals
            .into_iter()
            .map(|(phase, times)| {
                let avg = times.iter().sum::<u64>() / times.len() as u64;
                Bottleneck {
                    phase,
                    avg_ms: avg,
                    percentage: 0.0, // Calculate below
                }
            })
            .collect();

        // Calculate percentages
        let total_avg: u64 = bottlenecks.iter().map(|b| b.avg_ms).sum();
        if total_avg > 0 {
            for b in &mut bottlenecks {
                b.percentage = (b.avg_ms as f32 / total_avg as f32) * 100.0;
            }
        }

        // Sort by time descending
        bottlenecks.sort_by(|a, b| b.avg_ms.cmp(&a.avg_ms));
        bottlenecks.truncate(5);

        bottlenecks
    }

    /// Calculate stats per strategy
    fn calculate_strategy_stats(&self) -> HashMap<String, StrategyStats> {
        let mut stats: HashMap<String, Vec<&SearchTrace>> = HashMap::new();

        for trace in &self.traces {
            stats.entry(trace.strategy.clone()).or_default().push(trace);
        }

        stats
            .into_iter()
            .map(|(strategy, traces)| {
                let count = traces.len();
                let avg_relevance =
                    traces.iter().map(|t| t.avg_relevance).sum::<f32>() / count as f32;
                let avg_latency_ms =
                    traces.iter().map(|t| t.duration_ms).sum::<u64>() / count as u64;
                let success_rate =
                    traces.iter().filter(|t| t.success).count() as f32 / count as f32;

                (
                    strategy,
                    StrategyStats {
                        count,
                        avg_relevance,
                        avg_latency_ms,
                        success_rate,
                    },
                )
            })
            .collect()
    }

    /// Generate improvement suggestions based on analysis
    pub fn generate_suggestions(&self) -> Vec<Improvement> {
        let analysis = self.analyze();
        let mut suggestions = Vec::new();

        // Check failure patterns
        for pattern in &analysis.failure_patterns {
            suggestions.push(Improvement {
                id: 0, // Will be assigned when added
                category: ImprovementCategory::QueryRouting,
                description: pattern.suggestion.clone(),
                expected_impact: format!(
                    "Fix {} queries ({} affected)",
                    pattern.pattern, pattern.count
                ),
                difficulty: 2,
                status: ImprovementStatus::Suggested,
                created: Utc::now(),
                applied: None,
                verification: None,
            });
        }

        // Check for slow phases
        for bottleneck in &analysis.bottlenecks {
            if bottleneck.percentage > 40.0 && bottleneck.avg_ms > 100 {
                suggestions.push(Improvement {
                    id: 0,
                    category: ImprovementCategory::Performance,
                    description: format!(
                        "Optimize '{}' phase ({}ms avg, {:.0}% of total)",
                        bottleneck.phase, bottleneck.avg_ms, bottleneck.percentage
                    ),
                    expected_impact: format!("Reduce latency by ~{}ms", bottleneck.avg_ms / 2),
                    difficulty: 3,
                    status: ImprovementStatus::Suggested,
                    created: Utc::now(),
                    applied: None,
                    verification: None,
                });
            }
        }

        // Check for low overall relevance
        if analysis.avg_relevance < 0.6 {
            suggestions.push(Improvement {
                id: 0,
                category: ImprovementCategory::EmbeddingQuality,
                description: "Average relevance is low - consider reranking or better embeddings"
                    .to_string(),
                expected_impact: "Improve relevance from {:.2} to >0.7".to_string(),
                difficulty: 4,
                status: ImprovementStatus::Suggested,
                created: Utc::now(),
                applied: None,
                verification: None,
            });
        }

        // Check for low success rate
        if analysis.success_rate < 0.9 {
            suggestions.push(Improvement {
                id: 0,
                category: ImprovementCategory::CoverageGap,
                description: format!(
                    "Success rate is {:.0}% - add fallback search strategies",
                    analysis.success_rate * 100.0
                ),
                expected_impact: "Improve success rate to >95%".to_string(),
                difficulty: 3,
                status: ImprovementStatus::Suggested,
                created: Utc::now(),
                applied: None,
                verification: None,
            });
        }

        suggestions
    }

    /// Apply an improvement (marks it as applied)
    pub fn apply_improvement(&mut self, id: usize) -> Result<()> {
        let improvement = self
            .get_improvement_mut(id)
            .ok_or_else(|| anyhow!("Improvement {} not found", id))?;

        if improvement.status != ImprovementStatus::Suggested
            && improvement.status != ImprovementStatus::Approved
        {
            return Err(anyhow!(
                "Improvement {} cannot be applied (status: {})",
                id,
                improvement.status
            ));
        }

        improvement.status = ImprovementStatus::Applied;
        improvement.applied = Some(Utc::now());
        info!("Applied improvement {}: {}", id, improvement.description);

        Ok(())
    }
}

/// Introspect a search and provide analysis
pub fn introspect_last_search(store: &TraceStore) -> Result<String> {
    let trace = store
        .last_trace()
        .ok_or_else(|| anyhow!("No search traces found. Run a search first."))?;

    let mut output = String::new();
    output.push_str(&format!("\n=== Introspection: {} ===\n\n", trace.query));

    // Basic info
    output.push_str(&format!(
        "Timestamp: {}\n",
        trace.timestamp.format("%Y-%m-%d %H:%M:%S")
    ));
    output.push_str(&format!("Strategy: {}\n", trace.strategy));
    output.push_str(&format!("Duration: {}ms\n", trace.duration_ms));
    output.push_str(&format!("Results: {}\n", trace.result_count));
    output.push_str(&format!("Avg Relevance: {:.2}\n", trace.avg_relevance));

    // Quality assessment
    output.push_str("\n--- Quality Assessment ---\n");
    if trace.success {
        if trace.avg_relevance >= 0.7 {
            output.push_str("Status: GOOD - High relevance results\n");
        } else if trace.avg_relevance >= 0.5 {
            output.push_str("Status: FAIR - Moderate relevance results\n");
        } else if trace.result_count > 0 {
            output.push_str("Status: POOR - Low relevance results\n");
        } else {
            output.push_str("Status: FAILED - No results found\n");
        }
    } else {
        output.push_str(&format!(
            "Status: ERROR - {}\n",
            trace.error.as_deref().unwrap_or("Unknown error")
        ));
    }

    // Timing breakdown
    if !trace.timing_breakdown.is_empty() {
        output.push_str("\n--- Timing Breakdown ---\n");
        let mut timings: Vec<_> = trace.timing_breakdown.iter().collect();
        timings.sort_by(|a, b| b.1.cmp(a.1));
        for (phase, ms) in timings {
            let pct = (*ms as f32 / trace.duration_ms as f32) * 100.0;
            output.push_str(&format!("  {}: {}ms ({:.0}%)\n", phase, ms, pct));
        }
    }

    // Diagnosis
    output.push_str("\n--- Diagnosis ---\n");
    if trace.is_low_quality() {
        let q = trace.query.to_lowercase();
        if q.contains("symbol") || q.contains("theme") || q.contains("meaning") {
            output
                .push_str("Root cause: Symbolism/theme queries may not match literal text well\n");
            output.push_str("Suggestion: Try `librarian themes <book_id>` for theme analysis\n");
        } else if q.contains("who is") || q.contains("character") {
            output.push_str("Root cause: Character queries may need structured analysis\n");
            output
                .push_str("Suggestion: Try `librarian characters <book_id>` for character info\n");
        } else if trace.result_count == 0 {
            output.push_str("Root cause: No matching documents in index\n");
            output.push_str(
                "Suggestion: Check if relevant books are indexed, try different keywords\n",
            );
        } else {
            output
                .push_str("Root cause: Embedding model may not capture domain-specific meaning\n");
            output.push_str("Suggestion: Try keyword search with --mode keyword\n");
        }
    } else {
        output.push_str("Search performed well. No issues detected.\n");
    }

    Ok(output)
}

/// Format trace for display
pub fn format_trace(trace: &SearchTrace) -> String {
    let mut output = String::new();

    output.push_str(&format!("\n=== Execution Trace: {} ===\n\n", trace.id));
    output.push_str(&format!("Query: \"{}\"\n", trace.query));
    output.push_str(&format!(
        "Time: {}\n",
        trace.timestamp.format("%Y-%m-%d %H:%M:%S")
    ));
    output.push_str(&format!("Strategy: {}\n", trace.strategy));

    if let Some(intent) = &trace.intent {
        output.push_str(&format!("Intent: {}\n", intent));
    }

    if let Some(filters) = &trace.filters {
        if filters.author.is_some() || filters.title.is_some() || filters.book_id.is_some() {
            output.push_str("Filters:\n");
            if let Some(author) = &filters.author {
                output.push_str(&format!("  Author: {}\n", author));
            }
            if let Some(title) = &filters.title {
                output.push_str(&format!("  Title: {}\n", title));
            }
            if let Some(book_id) = &filters.book_id {
                output.push_str(&format!("  Book ID: {}\n", book_id));
            }
        }
    }

    output.push_str("\n--- Execution Timeline ---\n");

    // Build timeline
    let mut phases: Vec<_> = trace.timing_breakdown.iter().collect();
    phases.sort_by_key(|(_, ms)| *ms);
    phases.reverse();

    let mut cumulative = 0u64;
    for (phase, ms) in &phases {
        let bar_len = (**ms as f32 / trace.duration_ms as f32 * 40.0) as usize;
        let bar = "█".repeat(bar_len.max(1));
        output.push_str(&format!("t={:>4}ms: {} {}ms\n", cumulative, phase, ms));
        output.push_str(&format!("         {}\n", bar));
        cumulative += *ms;
    }

    output.push_str(&format!("\nTotal: {}ms\n", trace.duration_ms));

    // Results summary
    output.push_str("\n--- Results ---\n");
    if trace.success {
        output.push_str("Status: SUCCESS\n");
        output.push_str(&format!("Results: {}\n", trace.result_count));
        output.push_str(&format!("Avg Relevance: {:.2}\n", trace.avg_relevance));

        if !trace.relevance_scores.is_empty() {
            output.push_str("Relevance distribution:\n");
            let high = trace.relevance_scores.iter().filter(|&&s| s >= 0.7).count();
            let med = trace
                .relevance_scores
                .iter()
                .filter(|&s| (0.5..0.7).contains(s))
                .count();
            let low = trace.relevance_scores.iter().filter(|&&s| s < 0.5).count();
            output.push_str(&format!("  High (>=0.7): {}\n", high));
            output.push_str(&format!("  Medium (0.5-0.7): {}\n", med));
            output.push_str(&format!("  Low (<0.5): {}\n", low));
        }
    } else {
        output.push_str("Status: FAILED\n");
        if let Some(error) = &trace.error {
            output.push_str(&format!("Error: {}\n", error));
        }
    }

    output
}

/// Format improvements for display
pub fn format_improvements(improvements: &[Improvement]) -> String {
    let mut output = String::new();

    if improvements.is_empty() {
        output.push_str("No improvement suggestions available.\n");
        output.push_str("Run more searches to generate suggestions.\n");
        return output;
    }

    output.push_str("\n=== Improvement Suggestions ===\n\n");

    for imp in improvements {
        let status_icon = match imp.status {
            ImprovementStatus::Suggested => "💡",
            ImprovementStatus::Approved => "✅",
            ImprovementStatus::Applied => "🔧",
            ImprovementStatus::Verified => "✨",
            ImprovementStatus::Failed => "❌",
        };

        output.push_str(&format!(
            "{} #{}: [{}] {}\n",
            status_icon, imp.id, imp.category, imp.description
        ));
        output.push_str(&format!("   Expected impact: {}\n", imp.expected_impact));
        output.push_str(&format!("   Difficulty: {}/5\n", imp.difficulty));
        output.push_str(&format!("   Status: {}\n", imp.status));
        output.push('\n');
    }

    output.push_str("To apply an improvement: librarian improve --apply <id>\n");

    output
}

/// Format analysis for display
pub fn format_analysis(analysis: &TraceAnalysis) -> String {
    let mut output = String::new();

    output.push_str("\n=== Search Performance Analysis ===\n\n");

    if analysis.total_traces == 0 {
        output.push_str("No traces to analyze. Run some searches first.\n");
        return output;
    }

    // Overall stats
    output.push_str("--- Overall Statistics ---\n");
    output.push_str(&format!("Total searches: {}\n", analysis.total_traces));
    output.push_str(&format!(
        "Success rate: {:.0}%\n",
        analysis.success_rate * 100.0
    ));
    output.push_str(&format!("Avg relevance: {:.2}\n", analysis.avg_relevance));
    output.push_str(&format!("Avg latency: {}ms\n", analysis.avg_latency_ms));
    output.push_str(&format!("P50 latency: {}ms\n", analysis.p50_latency_ms));
    output.push_str(&format!("P95 latency: {}ms\n", analysis.p95_latency_ms));

    // Strategy comparison
    if !analysis.strategy_stats.is_empty() {
        output.push_str("\n--- Strategy Performance ---\n");
        output.push_str(&format!(
            "{:<15} {:>8} {:>12} {:>12} {:>10}\n",
            "Strategy", "Count", "Relevance", "Latency", "Success"
        ));
        output.push_str(&format!("{:-<57}\n", ""));

        let mut strategies: Vec<_> = analysis.strategy_stats.iter().collect();
        strategies.sort_by(|a, b| b.1.count.cmp(&a.1.count));

        for (name, stats) in strategies {
            output.push_str(&format!(
                "{:<15} {:>8} {:>12.2} {:>10}ms {:>9.0}%\n",
                name,
                stats.count,
                stats.avg_relevance,
                stats.avg_latency_ms,
                stats.success_rate * 100.0
            ));
        }
    }

    // Bottlenecks
    if !analysis.bottlenecks.is_empty() {
        output.push_str("\n--- Performance Bottlenecks ---\n");
        for b in &analysis.bottlenecks {
            let bar_len = (b.percentage / 2.5) as usize;
            let bar = "█".repeat(bar_len.max(1));
            output.push_str(&format!(
                "{:<20} {:>6}ms ({:>4.0}%) {}\n",
                b.phase, b.avg_ms, b.percentage, bar
            ));
        }
    }

    // Failure patterns
    if !analysis.failure_patterns.is_empty() {
        output.push_str("\n--- Failure Patterns ---\n");
        for pattern in &analysis.failure_patterns {
            output.push_str(&format!(
                "⚠️  {} ({} occurrences)\n",
                pattern.pattern, pattern.count
            ));
            output.push_str(&format!("   Examples: {}\n", pattern.examples.join(", ")));
            output.push_str(&format!("   Suggestion: {}\n\n", pattern.suggestion));
        }
    }

    output
}

#[cfg(test)]
mod tests {
    // `cargo verify` runs clippy with `-D warnings` for all targets, including unit tests.
    #![allow(clippy::unwrap_used)]

    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_trace_creation() {
        let trace = SearchTrace::new("test query", "hybrid")
            .with_results(5, vec![0.9, 0.8, 0.7, 0.6, 0.5])
            .with_duration(150)
            .with_timing("embedding", 50)
            .with_timing("search", 100);

        assert_eq!(trace.query, "test query");
        assert_eq!(trace.strategy, "hybrid");
        assert_eq!(trace.result_count, 5);
        assert!((trace.avg_relevance - 0.7).abs() < 0.01);
        assert_eq!(trace.duration_ms, 150);
        assert!(trace.success);
    }

    #[test]
    fn test_trace_low_quality() {
        let good_trace = SearchTrace::new("test", "hybrid").with_results(5, vec![0.9, 0.8, 0.7]);
        assert!(!good_trace.is_low_quality());

        let bad_trace = SearchTrace::new("test", "hybrid").with_results(5, vec![0.3, 0.2, 0.1]);
        assert!(bad_trace.is_low_quality());

        let empty_trace = SearchTrace::new("test", "hybrid").with_results(0, vec![]);
        assert!(empty_trace.is_low_quality());
    }

    #[test]
    fn test_trace_store() {
        let temp_dir = TempDir::new().unwrap();
        let mut store = TraceStore::new(temp_dir.path().join("introspection")).unwrap();

        let trace = SearchTrace::new("test query", "hybrid")
            .with_results(3, vec![0.8, 0.7, 0.6])
            .with_duration(100);

        store.add_trace(trace);
        assert_eq!(store.traces().len(), 1);

        store.save().unwrap();

        // Reload and verify
        let store2 = TraceStore::new(temp_dir.path().join("introspection")).unwrap();
        assert_eq!(store2.traces().len(), 1);
        assert_eq!(store2.last_trace().unwrap().query, "test query");
    }

    #[test]
    fn test_analysis() {
        let temp_dir = TempDir::new().unwrap();
        let mut store = TraceStore::new(temp_dir.path().join("introspection")).unwrap();

        // Add some traces
        for i in 0..10 {
            let trace = SearchTrace::new(format!("query {}", i), "hybrid")
                .with_results(5, vec![0.7, 0.6, 0.5, 0.4, 0.3])
                .with_duration(100 + i * 10)
                .with_timing("embedding", 30)
                .with_timing("search", 70);
            store.add_trace(trace);
        }

        let analysis = store.analyze();
        assert_eq!(analysis.total_traces, 10);
        assert!((analysis.success_rate - 1.0).abs() < 0.01);
        assert!(analysis.avg_latency_ms > 0);
    }

    #[test]
    fn test_improvements() {
        let temp_dir = TempDir::new().unwrap();
        let mut store = TraceStore::new(temp_dir.path().join("introspection")).unwrap();

        let improvement = Improvement {
            id: 0,
            category: ImprovementCategory::QueryRouting,
            description: "Test improvement".to_string(),
            expected_impact: "Better results".to_string(),
            difficulty: 2,
            status: ImprovementStatus::Suggested,
            created: Utc::now(),
            applied: None,
            verification: None,
        };

        store.add_improvement(improvement);
        assert_eq!(store.improvements().len(), 1);
        assert_eq!(store.improvements()[0].id, 1);

        store.apply_improvement(1).unwrap();
        assert_eq!(store.improvements()[0].status, ImprovementStatus::Applied);
    }
}
