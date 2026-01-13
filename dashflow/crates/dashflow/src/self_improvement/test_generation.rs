// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

//! Test Generation from Execution Traces.
//!
//! This module auto-generates regression tests from successful execution traces.
//! Tests can be generated in Rust or JSON format and capture:
//! - Input data from the trace
//! - Expected output from final_state
//! - Node execution sequence
//! - Timing bounds (optional)
//!
//! ## Usage
//!
//! ```bash
//! # Generate tests from recent traces (default: last 10)
//! dashflow self-improve generate-tests
//!
//! # Limit number of tests generated
//! dashflow self-improve generate-tests --limit 5
//!
//! # Output as JSON instead of Rust code
//! dashflow self-improve generate-tests --json
//!
//! # Save to specific output file
//! dashflow self-improve generate-tests --output tests/generated_tests.rs
//! ```
//!
//! ## Design Principle
//!
//! Generated tests serve as regression tests to catch behavioral changes.
//! They verify that:
//! 1. The same input produces the same (or equivalent) output
//! 2. The execution path remains consistent
//! 3. Performance stays within acceptable bounds

use crate::introspection::ExecutionTrace;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use uuid::Uuid;

// ============================================================================
// Configuration
// ============================================================================

/// Configuration for test generation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestGenerationConfig {
    /// Maximum number of tests to generate
    pub limit: usize,
    /// Only generate tests from successful traces
    pub successful_only: bool,
    /// Include timing assertions (soft bounds based on historical data)
    pub include_timing_bounds: bool,
    /// Timing tolerance multiplier (e.g., 2.0 means allow 2x the original duration)
    pub timing_tolerance: f64,
    /// Include node sequence assertions
    pub include_node_sequence: bool,
    /// Path to traces directory
    pub traces_dir: PathBuf,
    /// Output format
    pub output_format: OutputFormat,
    /// Test name prefix
    pub test_prefix: String,
}

impl Default for TestGenerationConfig {
    fn default() -> Self {
        Self {
            limit: 10,
            successful_only: true,
            include_timing_bounds: false,
            timing_tolerance: 2.0,
            include_node_sequence: true,
            traces_dir: PathBuf::from(".dashflow/traces"),
            output_format: OutputFormat::Rust,
            test_prefix: "regression_test".to_string(),
        }
    }
}

impl TestGenerationConfig {
    /// Create a new `TestGenerationConfig` with default values.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the maximum number of tests to generate.
    ///
    /// Default: 10.
    #[must_use]
    pub fn with_limit(mut self, limit: usize) -> Self {
        self.limit = limit;
        self
    }

    /// Set whether to only generate tests from successful traces.
    ///
    /// Default: true.
    #[must_use]
    pub fn with_successful_only(mut self, successful_only: bool) -> Self {
        self.successful_only = successful_only;
        self
    }

    /// Set whether to include timing assertions.
    ///
    /// When enabled, generates soft timing bounds based on historical data.
    /// Default: false.
    #[must_use]
    pub fn with_include_timing_bounds(mut self, include: bool) -> Self {
        self.include_timing_bounds = include;
        self
    }

    /// Set the timing tolerance multiplier.
    ///
    /// E.g., 2.0 means allow 2x the original duration.
    /// Default: 2.0.
    #[must_use]
    pub fn with_timing_tolerance(mut self, tolerance: f64) -> Self {
        self.timing_tolerance = tolerance;
        self
    }

    /// Set whether to include node sequence assertions.
    ///
    /// Default: true.
    #[must_use]
    pub fn with_include_node_sequence(mut self, include: bool) -> Self {
        self.include_node_sequence = include;
        self
    }

    /// Set the path to the traces directory.
    ///
    /// Default: ".dashflow/traces".
    #[must_use]
    pub fn with_traces_dir(mut self, path: impl Into<PathBuf>) -> Self {
        self.traces_dir = path.into();
        self
    }

    /// Set the output format for generated tests.
    ///
    /// - `Rust`: Rust test code (for use with cargo test)
    /// - `Json`: JSON test specifications
    ///
    /// Default: `Rust`.
    #[must_use]
    pub fn with_output_format(mut self, format: OutputFormat) -> Self {
        self.output_format = format;
        self
    }

    /// Set the test name prefix.
    ///
    /// Default: "regression_test".
    #[must_use]
    pub fn with_test_prefix(mut self, prefix: impl Into<String>) -> Self {
        self.test_prefix = prefix.into();
        self
    }
}

/// Output format for generated tests.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OutputFormat {
    /// Rust test code (for use with cargo test)
    Rust,
    /// JSON test specifications (for use with test runners)
    Json,
}

// ============================================================================
// Generated Test Types
// ============================================================================

/// A generated regression test.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneratedTest {
    /// Unique identifier for this test
    pub id: Uuid,
    /// Test name (suitable for use in code)
    pub name: String,
    /// Human-readable description
    pub description: String,
    /// Source trace ID
    pub source_trace_id: Option<String>,
    /// When the source trace was executed
    pub source_timestamp: Option<DateTime<Utc>>,
    /// Input data for the test
    pub input: TestInput,
    /// Expected output assertions
    pub expected: TestExpectations,
    /// When this test was generated
    pub generated_at: DateTime<Utc>,
}

impl GeneratedTest {
    /// Create a new generated test from an execution trace.
    #[must_use]
    pub fn from_trace(trace: &ExecutionTrace, test_name: &str) -> Self {
        let input = TestInput::from_trace(trace);
        let expected = TestExpectations::from_trace(trace);

        Self {
            id: Uuid::new_v4(),
            name: test_name.to_string(),
            description: format!(
                "Regression test generated from trace {}",
                trace.execution_id.as_deref().unwrap_or("unknown")
            ),
            source_trace_id: trace.execution_id.clone(),
            source_timestamp: trace
                .started_at
                .as_ref()
                .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
                .map(|dt| dt.with_timezone(&Utc)),
            input,
            expected,
            generated_at: Utc::now(),
        }
    }

    fn to_raw_rust_string_literal(content: &str) -> String {
        let mut hash_count = 1usize;
        loop {
            let hashes = "#".repeat(hash_count);
            let terminator = format!("\"{hashes}");
            if !content.contains(&terminator) {
                return format!("r{hashes}\"{content}\"{hashes}");
            }
            hash_count = hash_count.saturating_add(1);
        }
    }

    /// Generate Rust test code for this test.
    #[must_use]
    pub fn to_rust_code(&self, config: &TestGenerationConfig) -> String {
        let mut code = String::new();

        // Test function header
        code.push_str(&format!("#[test]\nfn {}() {{\n", self.name));

        // Documentation comment
        code.push_str(&format!("    // {}\n", self.description));
        if let Some(ref trace_id) = self.source_trace_id {
            code.push_str(&format!("    // Source trace: {}\n", trace_id));
        }
        code.push_str("    \n");

        // Input setup
        code.push_str("    // Setup input\n");
        if let Some(ref input_json) = self.input.input_data {
            let json = input_json.to_string();
            let json_literal = Self::to_raw_rust_string_literal(&json);
            code.push_str(&format!(
                "    let input: serde_json::Value = serde_json::from_str({}).unwrap();\n",
                json_literal
            ));
        } else {
            code.push_str("    let input = serde_json::Value::Null;\n");
        }
        code.push_str("    \n");

        // Expected output assertion
        code.push_str("    // Verify output\n");
        code.push_str("    // NOTE: Execute graph and compare output\n");
        if let Some(ref output_json) = self.expected.final_state {
            let json = output_json.to_string();
            let json_literal = Self::to_raw_rust_string_literal(&json);
            code.push_str(&format!(
                "    let expected_output: serde_json::Value = serde_json::from_str({}).unwrap();\n",
                json_literal
            ));
            code.push_str(
                "    // assert_eq!(result.final_state, Some(expected_output), \"Output mismatch\");\n",
            );
        }
        code.push_str("    \n");

        // Node sequence assertion (if enabled)
        if config.include_node_sequence && !self.expected.expected_nodes.is_empty() {
            code.push_str("    // Verify node execution sequence\n");
            code.push_str("    let expected_nodes = vec![\n");
            for node in &self.expected.expected_nodes {
                code.push_str(&format!("        \"{}\".to_string(),\n", node));
            }
            code.push_str("    ];\n");
            code.push_str("    // assert_eq!(result.nodes_executed.iter().map(|n| &n.node).collect::<Vec<_>>(), expected_nodes);\n");
            code.push_str("    \n");
        }

        // Timing bounds assertion (if enabled)
        if config.include_timing_bounds {
            if let Some(max_duration) = self.expected.max_duration_ms {
                code.push_str("    // Verify timing bounds (soft assertion)\n");
                code.push_str(&format!(
                    "    let max_duration_ms = {}; // {}x tolerance\n",
                    max_duration, config.timing_tolerance
                ));
                code.push_str("    // assert!(result.duration_ms <= max_duration_ms, \"Execution exceeded timing bounds\");\n");
            }
        }

        // Success assertion
        code.push_str("    // Verify success\n");
        if self.expected.should_succeed {
            code.push_str(
                "    // assert!(result.is_success(), \"Expected successful execution\");\n",
            );
        } else {
            code.push_str("    // assert!(!result.is_success(), \"Expected failed execution\");\n");
        }

        code.push_str("}\n");
        code
    }

    /// Generate JSON test specification for this test.
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }
}

/// Input data captured from a trace.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TestInput {
    /// Input data (if captured from first node's state_before)
    pub input_data: Option<serde_json::Value>,
    /// Thread ID (for checkpointing scenarios)
    pub thread_id: Option<String>,
    /// Additional context from trace metadata
    pub metadata: HashMap<String, serde_json::Value>,
}

impl TestInput {
    /// Extract input from an execution trace.
    #[must_use]
    pub fn from_trace(trace: &ExecutionTrace) -> Self {
        let input_data = trace
            .nodes_executed
            .first()
            .and_then(|n| n.state_before.clone());

        Self {
            input_data,
            thread_id: trace.thread_id.clone(),
            metadata: trace.metadata.clone(),
        }
    }
}

/// Expected output assertions.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TestExpectations {
    /// Expected final state
    pub final_state: Option<serde_json::Value>,
    /// Expected node execution sequence
    pub expected_nodes: Vec<String>,
    /// Expected execution to succeed
    pub should_succeed: bool,
    /// Maximum duration in milliseconds (for timing assertions)
    pub max_duration_ms: Option<u64>,
    /// Expected error count (0 for successful executions)
    pub expected_error_count: usize,
}

impl TestExpectations {
    /// Extract expectations from an execution trace.
    #[must_use]
    pub fn from_trace(trace: &ExecutionTrace) -> Self {
        Self {
            final_state: trace.final_state.clone(),
            expected_nodes: trace
                .nodes_executed
                .iter()
                .map(|n| n.node.clone())
                .collect(),
            should_succeed: trace.is_successful(),
            max_duration_ms: Some(trace.total_duration_ms),
            expected_error_count: trace.errors.len(),
        }
    }

    /// Apply timing tolerance to max_duration_ms.
    ///
    /// The tolerance is a multiplier applied to the max duration (e.g., 1.2 adds 20% buffer).
    /// A tolerance of 1.0 leaves the duration unchanged.
    ///
    /// # Arguments
    ///
    /// * `tolerance` - Must be positive (> 0.0). Values <= 0 are clamped to 1.0 with a warning.
    ///
    /// # Example
    ///
    /// ```ignore
    /// // Add 20% timing buffer
    /// expectation.with_timing_tolerance(1.2);
    /// ```
    ///
    /// M-972: Added validation - negative/zero tolerance produces unexpected results.
    #[must_use]
    pub fn with_timing_tolerance(mut self, tolerance: f64) -> Self {
        // M-972: Validate tolerance - must be positive
        let safe_tolerance = if tolerance <= 0.0 || !tolerance.is_finite() {
            tracing::warn!(
                tolerance,
                "Invalid timing_tolerance (must be positive and finite), using 1.0"
            );
            1.0
        } else {
            tolerance
        };

        if let Some(duration) = self.max_duration_ms {
            self.max_duration_ms = Some((duration as f64 * safe_tolerance) as u64);
        }
        self
    }
}

// ============================================================================
// Test Generator
// ============================================================================

/// Result of test generation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestGenerationResult {
    /// Generated tests
    pub tests: Vec<GeneratedTest>,
    /// Traces processed
    pub traces_processed: usize,
    /// Traces skipped (e.g., failed traces when successful_only=true)
    pub traces_skipped: usize,
    /// Errors encountered during generation
    pub errors: Vec<String>,
    /// Output file path (if saved)
    pub output_path: Option<PathBuf>,
}

/// Generator for regression tests from execution traces.
pub struct TestGenerator {
    config: TestGenerationConfig,
}

impl Default for TestGenerator {
    fn default() -> Self {
        Self::new()
    }
}

impl TestGenerator {
    /// Create a new test generator with default configuration.
    #[must_use]
    pub fn new() -> Self {
        Self {
            config: TestGenerationConfig::default(),
        }
    }

    /// Create with custom configuration.
    #[must_use]
    pub fn with_config(config: TestGenerationConfig) -> Self {
        Self { config }
    }

    /// Generate tests from traces in the configured directory.
    pub fn generate(&self) -> TestGenerationResult {
        let mut result = TestGenerationResult {
            tests: Vec::new(),
            traces_processed: 0,
            traces_skipped: 0,
            errors: Vec::new(),
            output_path: None,
        };

        // Load traces
        let traces = match self.load_traces() {
            Ok(t) => t,
            Err(e) => {
                result.errors.push(format!("Failed to load traces: {}", e));
                return result;
            }
        };

        // Process traces
        for (index, trace) in traces.iter().enumerate() {
            if result.tests.len() >= self.config.limit {
                break;
            }

            // Skip failed traces if configured
            if self.config.successful_only && !trace.is_successful() {
                result.traces_skipped += 1;
                continue;
            }

            // Generate test name
            let test_name = format!(
                "{}_{}",
                self.config.test_prefix,
                trace
                    .execution_id
                    .as_ref()
                    .map(|id| id.replace('-', "_").chars().take(8).collect::<String>())
                    .unwrap_or_else(|| format!("{}", index))
            );

            // Create test
            let mut test = GeneratedTest::from_trace(trace, &test_name);

            // Apply timing tolerance
            if self.config.include_timing_bounds {
                test.expected = test
                    .expected
                    .with_timing_tolerance(self.config.timing_tolerance);
            }

            result.tests.push(test);
            result.traces_processed += 1;
        }

        result
    }

    /// Generate and save tests to a file.
    pub fn generate_and_save(&self, output_path: &Path) -> TestGenerationResult {
        let mut result = self.generate();

        // Generate output content
        let content = match self.config.output_format {
            OutputFormat::Rust => self.generate_rust_module(&result.tests),
            OutputFormat::Json => match serde_json::to_string_pretty(&result.tests) {
                Ok(json) => json,
                Err(e) => {
                    result
                        .errors
                        .push(format!("Failed to serialize tests: {}", e));
                    return result;
                }
            },
        };

        // Save to file
        if let Err(e) = std::fs::write(output_path, &content) {
            result
                .errors
                .push(format!("Failed to write output file: {}", e));
        } else {
            result.output_path = Some(output_path.to_path_buf());
        }

        result
    }

    /// Generate a complete Rust test module.
    #[must_use]
    pub fn generate_rust_module(&self, tests: &[GeneratedTest]) -> String {
        let mut code = String::new();

        // Module header
        code.push_str("//! Auto-generated regression tests from execution traces.\n");
        code.push_str("//!\n");
        code.push_str(&format!(
            "//! Generated at: {}\n",
            Utc::now().format("%Y-%m-%d %H:%M:%S UTC")
        ));
        code.push_str(&format!("//! Tests generated: {}\n", tests.len()));
        code.push_str("//!\n");
        code.push_str("//! These tests verify that graph execution produces consistent results.\n");
        code.push_str("//! Uncomment the assertions after wiring to your graph executor.\n\n");

        // Imports
        code.push_str("#[cfg(test)]\n");
        code.push_str("mod regression_tests {\n");
        code.push_str("    #[allow(unused_imports)]\n");
        code.push_str("    use serde_json;\n\n");

        // Generate each test
        for test in tests {
            let test_code = test.to_rust_code(&self.config);
            // Indent each line
            for line in test_code.lines() {
                code.push_str("    ");
                code.push_str(line);
                code.push('\n');
            }
            code.push('\n');
        }

        code.push_str("}\n");
        code
    }

    /// Load traces from the configured directory.
    fn load_traces(&self) -> Result<Vec<ExecutionTrace>, String> {
        let traces_dir = &self.config.traces_dir;

        if !traces_dir.exists() {
            return Ok(Vec::new());
        }

        let mut traces = Vec::new();

        let entries = std::fs::read_dir(traces_dir)
            .map_err(|e| format!("Failed to read traces directory: {}", e))?;

        // Collect entries with modification times for sorting
        let mut entries_with_time: Vec<_> = entries
            .filter_map(|e| e.ok())
            .filter_map(|e| {
                let path = e.path();
                if path.extension().is_some_and(|ext| ext == "json") {
                    e.metadata()
                        .ok()
                        .and_then(|m| m.modified().ok())
                        .map(|t| (path, t))
                } else {
                    None
                }
            })
            .collect();

        // Sort by modification time (newest first)
        entries_with_time.sort_by(|(_, a), (_, b)| b.cmp(a));

        // Load traces (up to limit)
        for (path, _) in entries_with_time.into_iter().take(self.config.limit * 2) {
            match std::fs::read_to_string(&path) {
                Ok(content) => match serde_json::from_str::<ExecutionTrace>(&content) {
                    Ok(trace) => traces.push(trace),
                    Err(e) => {
                        // Log but don't fail on individual trace parse errors
                        tracing::warn!(
                            path = ?path,
                            error = %e,
                            "Failed to parse trace"
                        );
                    }
                },
                Err(e) => {
                    tracing::warn!(
                        path = ?path,
                        error = %e,
                        "Failed to read trace"
                    );
                }
            }
        }

        Ok(traces)
    }
}

// ============================================================================
// CLI Support
// ============================================================================

/// Run test generation from CLI.
///
/// # Arguments
///
/// * `limit` - Maximum number of tests to generate
/// * `json` - Output as JSON instead of Rust
/// * `output_path` - Optional output file path
/// * `traces_dir` - Custom traces directory (default: .dashflow/traces)
///
/// # Returns
///
/// The test generation result.
pub fn run_test_generation_cli(
    limit: Option<usize>,
    json: bool,
    output_path: Option<&Path>,
    traces_dir: Option<&str>,
) -> TestGenerationResult {
    let mut config = TestGenerationConfig::default();

    if let Some(limit) = limit {
        config.limit = limit;
    }

    if json {
        config.output_format = OutputFormat::Json;
    }

    if let Some(dir) = traces_dir {
        config.traces_dir = PathBuf::from(dir);
    }

    let generator = TestGenerator::with_config(config);

    if let Some(path) = output_path {
        generator.generate_and_save(path)
    } else {
        generator.generate()
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::introspection::{ExecutionTrace, NodeExecution};

    fn create_test_trace() -> ExecutionTrace {
        ExecutionTrace {
            execution_id: Some("test-trace-123".to_string()),
            thread_id: Some("thread-1".to_string()),
            parent_execution_id: None,
            root_execution_id: None,
            depth: Some(0),
            nodes_executed: vec![
                NodeExecution {
                    node: "input_parser".to_string(),
                    duration_ms: 50,
                    tokens_used: 100,
                    state_before: Some(serde_json::json!({"query": "test input"})),
                    state_after: Some(serde_json::json!({"query": "test input", "parsed": true})),
                    tools_called: vec![],
                    success: true,
                    error_message: None,
                    index: 0,
                    started_at: None,
                    metadata: HashMap::new(),
                },
                NodeExecution {
                    node: "llm_executor".to_string(),
                    duration_ms: 500,
                    tokens_used: 1500,
                    state_before: Some(serde_json::json!({"query": "test input", "parsed": true})),
                    state_after: Some(
                        serde_json::json!({"query": "test input", "parsed": true, "response": "test output"}),
                    ),
                    tools_called: vec!["gpt-4".to_string()],
                    success: true,
                    error_message: None,
                    index: 1,
                    started_at: None,
                    metadata: HashMap::new(),
                },
            ],
            total_duration_ms: 550,
            total_tokens: 1600,
            errors: vec![],
            completed: true,
            started_at: Some("2025-12-15T00:00:00Z".to_string()),
            ended_at: Some("2025-12-15T00:00:01Z".to_string()),
            final_state: Some(
                serde_json::json!({"query": "test input", "parsed": true, "response": "test output"}),
            ),
            metadata: HashMap::new(),
            execution_metrics: None,
            performance_metrics: None,
        }
    }

    #[test]
    fn test_generated_test_from_trace() {
        let trace = create_test_trace();
        let test = GeneratedTest::from_trace(&trace, "test_example");

        assert_eq!(test.name, "test_example");
        assert_eq!(test.source_trace_id, Some("test-trace-123".to_string()));
        assert!(test.expected.should_succeed);
        assert_eq!(test.expected.expected_nodes.len(), 2);
        assert_eq!(test.expected.expected_nodes[0], "input_parser");
        assert_eq!(test.expected.expected_nodes[1], "llm_executor");
    }

    #[test]
    fn test_rust_code_generation() {
        let trace = create_test_trace();
        let test = GeneratedTest::from_trace(&trace, "test_example");
        let config = TestGenerationConfig::default();

        let code = test.to_rust_code(&config);

        assert!(code.contains("#[test]"));
        assert!(code.contains("fn test_example()"));
        assert!(code.contains("input_parser"));
        assert!(code.contains("llm_executor"));
    }

    #[test]
    fn test_json_serialization() {
        let trace = create_test_trace();
        let test = GeneratedTest::from_trace(&trace, "test_example");

        let json = test.to_json().unwrap();
        let parsed: GeneratedTest = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed.name, test.name);
        assert_eq!(parsed.source_trace_id, test.source_trace_id);
    }

    #[test]
    fn test_timing_tolerance() {
        let trace = create_test_trace();
        let test = GeneratedTest::from_trace(&trace, "test_example");

        // Original duration is 550ms
        assert_eq!(test.expected.max_duration_ms, Some(550));

        // Apply 2x tolerance
        let with_tolerance = test.expected.with_timing_tolerance(2.0);
        assert_eq!(with_tolerance.max_duration_ms, Some(1100));
    }

    #[test]
    fn test_generator_empty_traces() {
        let config = TestGenerationConfig {
            traces_dir: PathBuf::from("/nonexistent/path"),
            ..Default::default()
        };
        let generator = TestGenerator::with_config(config);
        let result = generator.generate();

        assert!(result.tests.is_empty());
        assert_eq!(result.traces_processed, 0);
    }

    #[test]
    fn test_rust_module_generation() {
        let trace = create_test_trace();
        let test = GeneratedTest::from_trace(&trace, "test_example");
        let config = TestGenerationConfig::default();
        let generator = TestGenerator::with_config(config);

        let module = generator.generate_rust_module(&[test]);

        assert!(module.contains("mod regression_tests"));
        assert!(module.contains("#[cfg(test)]"));
        assert!(module.contains("use serde_json"));
    }

    #[test]
    fn test_test_generation_config_builder_new() {
        let config = TestGenerationConfig::new();
        let default_config = TestGenerationConfig::default();
        assert_eq!(config.limit, default_config.limit);
        assert_eq!(config.successful_only, default_config.successful_only);
        assert_eq!(
            config.include_timing_bounds,
            default_config.include_timing_bounds
        );
        assert_eq!(config.timing_tolerance, default_config.timing_tolerance);
    }

    #[test]
    fn test_test_generation_config_builder_full_chain() {
        let config = TestGenerationConfig::new()
            .with_limit(50)
            .with_successful_only(false)
            .with_include_timing_bounds(true)
            .with_timing_tolerance(3.0)
            .with_include_node_sequence(false)
            .with_traces_dir("/custom/traces")
            .with_output_format(OutputFormat::Json)
            .with_test_prefix("custom_test");

        assert_eq!(config.limit, 50);
        assert!(!config.successful_only);
        assert!(config.include_timing_bounds);
        assert_eq!(config.timing_tolerance, 3.0);
        assert!(!config.include_node_sequence);
        assert_eq!(config.traces_dir, PathBuf::from("/custom/traces"));
        assert_eq!(config.output_format, OutputFormat::Json);
        assert_eq!(config.test_prefix, "custom_test");
    }

    #[test]
    fn test_test_generation_config_builder_partial_chain() {
        // Test that partial builder chains preserve defaults
        let config = TestGenerationConfig::new()
            .with_limit(25)
            .with_include_timing_bounds(true);

        // Custom values
        assert_eq!(config.limit, 25);
        assert!(config.include_timing_bounds);

        // Default values preserved
        assert!(config.successful_only);
        assert_eq!(config.timing_tolerance, 2.0);
        assert!(config.include_node_sequence);
        assert_eq!(config.traces_dir, PathBuf::from(".dashflow/traces"));
        assert_eq!(config.output_format, OutputFormat::Rust);
        assert_eq!(config.test_prefix, "regression_test");
    }
}
