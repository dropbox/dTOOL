//! Golden Dataset - Test scenarios for evaluation framework
//!
//! This module provides the data structures and loading utilities for golden datasets,
//! which contain curated test scenarios with expected outputs and validation criteria.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

/// A single test scenario with input, expected output, and evaluation criteria
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GoldenScenario {
    /// Unique identifier for this scenario (e.g., "`01_simple_tokio_query`")
    pub id: String,

    /// Human-readable description of what this scenario tests
    pub description: String,

    /// The user query or input message
    pub query: String,

    /// Additional context (e.g., "First turn, no conversation history")
    #[serde(default)]
    pub context: Option<String>,

    /// Strings that MUST appear in the output
    #[serde(default)]
    pub expected_output_contains: Vec<String>,

    /// Strings that MUST NOT appear in the output
    #[serde(default)]
    pub expected_output_not_contains: Vec<String>,

    /// Minimum quality score threshold (0.0-1.0)
    #[serde(default = "default_quality_threshold")]
    pub quality_threshold: f64,

    /// Maximum allowed latency in milliseconds
    #[serde(default)]
    pub max_latency_ms: Option<u64>,

    /// Expected tool calls (for validation)
    #[serde(default)]
    pub expected_tool_calls: Vec<String>,

    /// Maximum allowed cost in USD
    #[serde(default)]
    pub max_cost_usd: Option<f64>,

    /// Maximum number of tokens allowed
    #[serde(default)]
    pub max_tokens: Option<u32>,

    /// Per-dimension quality thresholds
    #[serde(default)]
    pub accuracy_threshold: Option<f64>,

    #[serde(default)]
    pub relevance_threshold: Option<f64>,

    #[serde(default)]
    pub completeness_threshold: Option<f64>,

    #[serde(default)]
    pub safety_threshold: Option<f64>,

    #[serde(default)]
    pub coherence_threshold: Option<f64>,

    #[serde(default)]
    pub conciseness_threshold: Option<f64>,

    /// Use case-insensitive matching for `expected_output_contains/not_contains`
    #[serde(default)]
    pub case_insensitive_validation: bool,

    /// Difficulty level of this scenario (optional)
    #[serde(default)]
    pub difficulty: Option<crate::test_generation::Difficulty>,
}

fn default_quality_threshold() -> f64 {
    0.85
}

/// Collection of golden scenarios loaded from JSON files
#[derive(Debug, Clone)]
pub struct GoldenDataset {
    /// All loaded scenarios
    pub scenarios: Vec<GoldenScenario>,

    /// Source directory path
    pub source_dir: PathBuf,
}

impl GoldenDataset {
    /// Load all golden scenarios from a directory
    ///
    /// This recursively searches the directory for `*.json` files and loads them as scenarios.
    ///
    /// # Arguments
    /// * `dir` - Directory to search for golden dataset JSON files
    ///
    /// # Returns
    /// * `Ok(GoldenDataset)` - Successfully loaded dataset
    /// * `Err(...)` - If directory doesn't exist or no JSON files found
    ///
    /// # Example
    /// ```no_run
    /// use dashflow_evals::golden_dataset::GoldenDataset;
    ///
    /// let dataset = GoldenDataset::load("examples/apps/librarian/data/golden_dataset")
    ///     .expect("Failed to load golden dataset");
    /// println!("Loaded {} scenarios", dataset.scenarios.len());
    /// ```
    pub fn load<P: AsRef<Path>>(dir: P) -> Result<Self> {
        let dir = dir.as_ref();

        if !dir.exists() {
            anyhow::bail!("Golden dataset directory does not exist: {}", dir.display());
        }

        if !dir.is_dir() {
            anyhow::bail!("Path is not a directory: {}", dir.display());
        }

        let mut scenarios = Vec::new();

        // Recursively find all .json files
        for entry in WalkDir::new(dir)
            .follow_links(true)
            .into_iter()
            .filter_map(std::result::Result::ok)
        {
            let path = entry.path();

            // Skip if not a JSON file
            if !path.is_file() || path.extension().and_then(|s| s.to_str()) != Some("json") {
                continue;
            }

            // Load and parse the JSON file
            let content = std::fs::read_to_string(path)
                .with_context(|| format!("Failed to read file: {}", path.display()))?;

            let scenario: GoldenScenario = serde_json::from_str(&content)
                .with_context(|| format!("Failed to parse JSON from: {}", path.display()))?;

            scenarios.push(scenario);
        }

        if scenarios.is_empty() {
            anyhow::bail!("No JSON files found in directory: {}", dir.display());
        }

        // Sort scenarios by ID for consistent ordering
        scenarios.sort_by(|a, b| a.id.cmp(&b.id));

        Ok(Self {
            scenarios,
            source_dir: dir.to_path_buf(),
        })
    }

    /// Get number of scenarios in the dataset
    #[must_use]
    pub fn len(&self) -> usize {
        self.scenarios.len()
    }

    /// Check if dataset is empty
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.scenarios.is_empty()
    }

    /// Filter scenarios by ID prefix
    #[must_use]
    pub fn filter_by_prefix(&self, prefix: &str) -> Vec<&GoldenScenario> {
        self.scenarios
            .iter()
            .filter(|s| s.id.starts_with(prefix))
            .collect()
    }

    /// Get scenario by ID
    #[must_use]
    pub fn get_by_id(&self, id: &str) -> Option<&GoldenScenario> {
        self.scenarios.iter().find(|s| s.id == id)
    }
}

#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::TempDir;

    fn create_test_scenario_json() -> String {
        serde_json::json!({
            "id": "test_scenario_01",
            "description": "Test scenario for unit tests",
            "query": "What is Rust?",
            "context": "First turn",
            "expected_output_contains": ["memory", "safe"],
            "expected_output_not_contains": ["error"],
            "quality_threshold": 0.90,
            "max_latency_ms": 5000,
            "expected_tool_calls": ["search_docs"]
        })
        .to_string()
    }

    #[test]
    fn test_deserialize_golden_scenario() {
        let json = create_test_scenario_json();
        let scenario: GoldenScenario = serde_json::from_str(&json).expect("Failed to parse");

        assert_eq!(scenario.id, "test_scenario_01");
        assert_eq!(scenario.description, "Test scenario for unit tests");
        assert_eq!(scenario.query, "What is Rust?");
        assert_eq!(scenario.context.as_deref(), Some("First turn"));
        assert_eq!(scenario.expected_output_contains, vec!["memory", "safe"]);
        assert_eq!(scenario.expected_output_not_contains, vec!["error"]);
        assert_eq!(scenario.quality_threshold, 0.90);
        assert_eq!(scenario.max_latency_ms, Some(5000));
        assert_eq!(scenario.expected_tool_calls, vec!["search_docs"]);
    }

    #[test]
    fn test_deserialize_minimal_scenario() {
        let json = serde_json::json!({
            "id": "minimal_01",
            "description": "Minimal scenario",
            "query": "Test query"
        })
        .to_string();

        let scenario: GoldenScenario = serde_json::from_str(&json).expect("Failed to parse");

        assert_eq!(scenario.id, "minimal_01");
        assert_eq!(scenario.quality_threshold, 0.85); // default
        assert!(scenario.expected_output_contains.is_empty());
        assert!(scenario.max_latency_ms.is_none());
    }

    #[test]
    fn test_load_golden_dataset_from_directory() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let dataset_dir = temp_dir.path().join("golden_dataset");
        std::fs::create_dir(&dataset_dir).expect("Failed to create dataset dir");

        // Create test JSON files
        for i in 1..=3 {
            let file_path = dataset_dir.join(format!("scenario_{:02}.json", i));
            let mut file = std::fs::File::create(&file_path).expect("Failed to create file");
            let json = serde_json::json!({
                "id": format!("scenario_{:02}", i),
                "description": format!("Test scenario {}", i),
                "query": format!("Query {}", i),
                "quality_threshold": 0.85
            })
            .to_string();
            file.write_all(json.as_bytes()).expect("Failed to write");
        }

        // Load dataset
        let dataset = GoldenDataset::load(&dataset_dir).expect("Failed to load dataset");

        assert_eq!(dataset.len(), 3);
        assert_eq!(dataset.scenarios[0].id, "scenario_01");
        assert_eq!(dataset.scenarios[1].id, "scenario_02");
        assert_eq!(dataset.scenarios[2].id, "scenario_03");
    }

    #[test]
    fn test_load_nonexistent_directory() {
        let result = GoldenDataset::load("/nonexistent/directory");
        assert!(result.is_err());
    }

    #[test]
    fn test_load_empty_directory() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let result = GoldenDataset::load(temp_dir.path());
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("No JSON files found"));
    }

    #[test]
    fn test_filter_by_prefix() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let dataset_dir = temp_dir.path().join("golden_dataset");
        std::fs::create_dir(&dataset_dir).expect("Failed to create dataset dir");

        // Create scenarios with different prefixes
        for (id, prefix) in [
            ("simple_01", "simple"),
            ("complex_01", "complex"),
            ("simple_02", "simple"),
        ] {
            let file_path = dataset_dir.join(format!("{}.json", id));
            let mut file = std::fs::File::create(&file_path).expect("Failed to create file");
            let json = serde_json::json!({
                "id": id,
                "description": format!("{} scenario", prefix),
                "query": "Test query",
                "quality_threshold": 0.85
            })
            .to_string();
            file.write_all(json.as_bytes()).expect("Failed to write");
        }

        let dataset = GoldenDataset::load(&dataset_dir).expect("Failed to load");
        let simple_scenarios = dataset.filter_by_prefix("simple");

        assert_eq!(simple_scenarios.len(), 2);
        assert!(simple_scenarios.iter().all(|s| s.id.starts_with("simple")));
    }

    #[test]
    fn test_get_by_id() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let dataset_dir = temp_dir.path().join("golden_dataset");
        std::fs::create_dir(&dataset_dir).expect("Failed to create dataset dir");

        let file_path = dataset_dir.join("test_01.json");
        let mut file = std::fs::File::create(&file_path).expect("Failed to create file");
        let json = serde_json::json!({
            "id": "test_01",
            "description": "Test",
            "query": "Query",
            "quality_threshold": 0.85
        })
        .to_string();
        file.write_all(json.as_bytes()).expect("Failed to write");

        let dataset = GoldenDataset::load(&dataset_dir).expect("Failed to load");

        assert!(dataset.get_by_id("test_01").is_some());
        assert!(dataset.get_by_id("nonexistent").is_none());
    }

    #[test]
    fn test_per_dimension_thresholds() {
        let json = serde_json::json!({
            "id": "dimension_test",
            "description": "Test per-dimension thresholds",
            "query": "Test",
            "quality_threshold": 0.90,
            "accuracy_threshold": 0.95,
            "relevance_threshold": 0.90,
            "completeness_threshold": 0.85,
            "safety_threshold": 1.0,
            "coherence_threshold": 0.80,
            "conciseness_threshold": 0.75
        })
        .to_string();

        let scenario: GoldenScenario = serde_json::from_str(&json).expect("Failed to parse");

        assert_eq!(scenario.accuracy_threshold, Some(0.95));
        assert_eq!(scenario.relevance_threshold, Some(0.90));
        assert_eq!(scenario.completeness_threshold, Some(0.85));
        assert_eq!(scenario.safety_threshold, Some(1.0));
        assert_eq!(scenario.coherence_threshold, Some(0.80));
        assert_eq!(scenario.conciseness_threshold, Some(0.75));
    }
}
