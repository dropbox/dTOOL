//! DashOptimize integration for prompt optimization
//!
//! This module provides integration with DashFlow's DashOptimize system
//! for automatic prompt optimization using training data.
//!
//! ## Features
//!
//! - **Prompt Registry**: Store and manage optimized prompts
//! - **Optimization State**: Save/load optimization state from files
//! - **Few-Shot Examples**: Support for optimized few-shot examples
//! - **Training Data Collection**: Collect successful interactions for optimization
//! - **DashFlow Integration**: Bridge to DashFlow's BootstrapFewShot optimizer
//!
//! ## Example
//!
//! ```rust,ignore
//! use codex_dashflow_core::optimize::{PromptRegistry, PromptConfig, TrainingData};
//!
//! // Load optimized prompts
//! let registry = PromptRegistry::load("~/.codex-dashflow/prompts.toml")?;
//!
//! // Get system prompt for reasoning node
//! let system_prompt = registry.get_system_prompt();
//!
//! // Collect training data from successful interactions
//! let mut training = TrainingData::new();
//! training.add_example("List files", "ls -la output...", 0.95);
//! training.save_default()?;
//! ```

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Default system prompt for the coding agent
pub const DEFAULT_SYSTEM_PROMPT: &str = r#"You are a helpful coding assistant with access to tools for interacting with the filesystem and executing shell commands.

When the user asks you to perform tasks that require file system access or shell commands, use the appropriate tools. Available tools:
- shell: Execute shell commands
- read_file: Read the contents of a file
- write_file: Write content to a file
- apply_patch: Apply a unified diff patch to files
- search_files: Search for files or content in the codebase

Always explain what you're doing and why. Be concise but thorough in your explanations."#;

/// A few-shot example for prompt optimization
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FewShotExample {
    /// User input for this example
    pub user_input: String,
    /// Expected assistant response or tool call
    pub expected_output: String,
    /// Optional chain-of-thought reasoning
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning: Option<String>,
    /// Score for this example (0.0-1.0)
    #[serde(default = "default_score")]
    pub score: f64,
}

fn default_score() -> f64 {
    1.0
}

impl FewShotExample {
    /// Create a new few-shot example
    pub fn new(user_input: impl Into<String>, expected_output: impl Into<String>) -> Self {
        Self {
            user_input: user_input.into(),
            expected_output: expected_output.into(),
            reasoning: None,
            score: 1.0,
        }
    }

    /// Add reasoning to the example
    pub fn with_reasoning(mut self, reasoning: impl Into<String>) -> Self {
        self.reasoning = Some(reasoning.into());
        self
    }

    /// Format for inclusion in prompt
    pub fn format_for_prompt(&self) -> String {
        let mut s = format!("User: {}\n", self.user_input);
        if let Some(ref reasoning) = self.reasoning {
            s.push_str(&format!("Thinking: {}\n", reasoning));
        }
        s.push_str(&format!("Assistant: {}", self.expected_output));
        s
    }
}

/// Configuration for a specific prompt
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PromptConfig {
    /// Base instruction/system prompt
    pub instruction: String,
    /// Few-shot examples (if any)
    #[serde(default)]
    pub few_shot_examples: Vec<FewShotExample>,
    /// Optimization metadata
    #[serde(default)]
    pub metadata: OptimizationMetadata,
}

impl Default for PromptConfig {
    fn default() -> Self {
        Self {
            instruction: DEFAULT_SYSTEM_PROMPT.to_string(),
            few_shot_examples: Vec::new(),
            metadata: OptimizationMetadata::default(),
        }
    }
}

impl PromptConfig {
    /// Create a new prompt config with instruction
    pub fn new(instruction: impl Into<String>) -> Self {
        Self {
            instruction: instruction.into(),
            few_shot_examples: Vec::new(),
            metadata: OptimizationMetadata::default(),
        }
    }

    /// Add few-shot examples
    pub fn with_examples(mut self, examples: Vec<FewShotExample>) -> Self {
        self.few_shot_examples = examples;
        self
    }

    /// Build the full system prompt including few-shot examples
    pub fn build_prompt(&self) -> String {
        if self.few_shot_examples.is_empty() {
            return self.instruction.clone();
        }

        let examples_text = self
            .few_shot_examples
            .iter()
            .enumerate()
            .map(|(i, ex)| format!("Example {}:\n{}", i + 1, ex.format_for_prompt()))
            .collect::<Vec<_>>()
            .join("\n\n");

        format!("{}\n\n## Examples\n\n{}", self.instruction, examples_text)
    }
}

/// Metadata about the optimization state
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct OptimizationMetadata {
    /// Optimizer used (e.g., "BootstrapFewShot", "SIMBA")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub optimizer: Option<String>,
    /// Best score achieved
    #[serde(default)]
    pub best_score: f64,
    /// Number of optimization iterations
    #[serde(default)]
    pub iterations: u32,
    /// Timestamp of last optimization
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timestamp: Option<String>,
    /// Training dataset size
    #[serde(default)]
    pub training_size: usize,
    /// Additional custom metadata
    #[serde(default)]
    pub custom: HashMap<String, String>,
}

/// Registry for managing optimized prompts
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct PromptRegistry {
    /// Prompts indexed by name
    #[serde(default)]
    pub prompts: HashMap<String, PromptConfig>,
    /// Version of the registry format
    #[serde(default = "default_version")]
    pub version: u32,
}

fn default_version() -> u32 {
    1
}

impl PromptRegistry {
    /// Create a new empty registry
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a registry with default prompts
    pub fn with_defaults() -> Self {
        let mut registry = Self::new();
        registry
            .prompts
            .insert("system".to_string(), PromptConfig::default());
        registry
    }

    /// Get the system prompt (with any optimized examples)
    pub fn get_system_prompt(&self) -> String {
        self.prompts
            .get("system")
            .map(|p| p.build_prompt())
            .unwrap_or_else(|| DEFAULT_SYSTEM_PROMPT.to_string())
    }

    /// Get a specific prompt configuration
    pub fn get_prompt(&self, name: &str) -> Option<&PromptConfig> {
        self.prompts.get(name)
    }

    /// Set a prompt configuration
    pub fn set_prompt(&mut self, name: impl Into<String>, config: PromptConfig) {
        self.prompts.insert(name.into(), config);
    }

    /// Load registry from a TOML file
    pub fn load(path: impl AsRef<Path>) -> Result<Self, OptimizeError> {
        let path = path.as_ref();
        let content = std::fs::read_to_string(path).map_err(|e| OptimizeError::IoError {
            path: path.to_path_buf(),
            source: e,
        })?;
        Self::from_toml(&content)
    }

    /// Save registry to a TOML file
    pub fn save(&self, path: impl AsRef<Path>) -> Result<(), OptimizeError> {
        let path = path.as_ref();
        let content = toml::to_string_pretty(self).map_err(OptimizeError::SerializeError)?;

        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| OptimizeError::IoError {
                path: parent.to_path_buf(),
                source: e,
            })?;
        }

        std::fs::write(path, content).map_err(|e| OptimizeError::IoError {
            path: path.to_path_buf(),
            source: e,
        })
    }

    /// Parse registry from TOML string
    pub fn from_toml(content: &str) -> Result<Self, OptimizeError> {
        toml::from_str(content).map_err(OptimizeError::ParseError)
    }

    /// Load from default path (~/.codex-dashflow/prompts.toml)
    pub fn load_default() -> Result<Self, OptimizeError> {
        let home = dirs::home_dir().ok_or(OptimizeError::NoHomeDir)?;
        let path = home.join(".codex-dashflow").join("prompts.toml");
        if path.exists() {
            Self::load(&path)
        } else {
            Ok(Self::with_defaults())
        }
    }

    /// Save to default path
    pub fn save_default(&self) -> Result<(), OptimizeError> {
        let home = dirs::home_dir().ok_or(OptimizeError::NoHomeDir)?;
        let path = home.join(".codex-dashflow").join("prompts.toml");
        self.save(&path)
    }
}

/// Optimization configuration for running DashOptimize
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct OptimizeConfig {
    /// Which optimizer to use
    #[serde(default = "default_optimizer")]
    pub optimizer: String,
    /// Maximum optimization iterations
    #[serde(default = "default_max_iterations")]
    pub max_iterations: u32,
    /// Target metric name
    #[serde(default = "default_metric")]
    pub metric: String,
    /// Minimum score improvement to accept
    #[serde(default = "default_min_improvement")]
    pub min_improvement: f64,
    /// Number of few-shot examples to generate
    #[serde(default = "default_few_shot_count")]
    pub few_shot_count: usize,
}

fn default_optimizer() -> String {
    "BootstrapFewShot".to_string()
}

fn default_max_iterations() -> u32 {
    10
}

fn default_metric() -> String {
    "task_completion".to_string()
}

fn default_min_improvement() -> f64 {
    0.05
}

fn default_few_shot_count() -> usize {
    3
}

impl Default for OptimizeConfig {
    fn default() -> Self {
        Self {
            optimizer: default_optimizer(),
            max_iterations: default_max_iterations(),
            metric: default_metric(),
            min_improvement: default_min_improvement(),
            few_shot_count: default_few_shot_count(),
        }
    }
}

/// Optimization errors
#[derive(Debug)]
pub enum OptimizeError {
    /// No home directory found
    NoHomeDir,
    /// IO error
    IoError {
        path: PathBuf,
        source: std::io::Error,
    },
    /// TOML parsing error
    ParseError(toml::de::Error),
    /// TOML serialization error
    SerializeError(toml::ser::Error),
}

impl std::fmt::Display for OptimizeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoHomeDir => write!(f, "Could not determine home directory"),
            Self::IoError { path, source } => {
                write!(f, "IO error for {}: {}", path.display(), source)
            }
            Self::ParseError(e) => write!(f, "Failed to parse: {}", e),
            Self::SerializeError(e) => write!(f, "Failed to serialize: {}", e),
        }
    }
}

impl std::error::Error for OptimizeError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::IoError { source, .. } => Some(source),
            Self::ParseError(e) => Some(e),
            Self::SerializeError(e) => Some(e),
            Self::NoHomeDir => None,
        }
    }
}

// ============================================================================
// Training Data Collection
// ============================================================================

/// A training example representing a successful interaction
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TrainingExample {
    /// User's input/request
    pub user_input: String,
    /// Agent's successful output
    pub agent_output: String,
    /// Quality score (0.0-1.0)
    pub score: f64,
    /// Tool calls made during this interaction (if any)
    #[serde(default)]
    pub tool_calls: Vec<String>,
    /// Timestamp when this example was collected
    #[serde(default)]
    pub timestamp: Option<String>,
}

impl TrainingExample {
    /// Create a new training example
    pub fn new(user_input: impl Into<String>, agent_output: impl Into<String>, score: f64) -> Self {
        Self {
            user_input: user_input.into(),
            agent_output: agent_output.into(),
            score,
            tool_calls: Vec::new(),
            timestamp: Some(chrono_timestamp()),
        }
    }

    /// Add tool calls to the example
    pub fn with_tool_calls(mut self, tools: Vec<String>) -> Self {
        self.tool_calls = tools;
        self
    }

    /// Convert to a FewShotExample
    pub fn to_few_shot(&self) -> FewShotExample {
        FewShotExample {
            user_input: self.user_input.clone(),
            expected_output: self.agent_output.clone(),
            reasoning: None,
            score: self.score,
        }
    }
}

/// Collection of training data for prompt optimization
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TrainingData {
    /// Training examples
    #[serde(default)]
    pub examples: Vec<TrainingExample>,
    /// Version of the training data format
    #[serde(default = "default_version")]
    pub version: u32,
}

impl Default for TrainingData {
    fn default() -> Self {
        Self {
            examples: Vec::new(),
            version: 1,
        }
    }
}

impl TrainingData {
    /// Create empty training data
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a training example
    pub fn add_example(
        &mut self,
        user_input: impl Into<String>,
        agent_output: impl Into<String>,
        score: f64,
    ) {
        self.examples
            .push(TrainingExample::new(user_input, agent_output, score));
    }

    /// Add a training example with tool calls
    pub fn add_example_with_tools(
        &mut self,
        user_input: impl Into<String>,
        agent_output: impl Into<String>,
        score: f64,
        tools: Vec<String>,
    ) {
        let example = TrainingExample::new(user_input, agent_output, score).with_tool_calls(tools);
        self.examples.push(example);
    }

    /// Filter examples by minimum score
    pub fn filter_by_score(&self, min_score: f64) -> Vec<&TrainingExample> {
        self.examples
            .iter()
            .filter(|e| e.score >= min_score)
            .collect()
    }

    /// Get the top N examples by score
    pub fn top_examples(&self, n: usize) -> Vec<&TrainingExample> {
        let mut sorted: Vec<_> = self.examples.iter().collect();
        sorted.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        sorted.into_iter().take(n).collect()
    }

    /// Convert training examples to few-shot examples
    pub fn to_few_shot_examples(&self, max_count: usize) -> Vec<FewShotExample> {
        self.top_examples(max_count)
            .into_iter()
            .map(|e| e.to_few_shot())
            .collect()
    }

    /// Load training data from a file
    pub fn load(path: impl AsRef<Path>) -> Result<Self, OptimizeError> {
        let path = path.as_ref();
        let content = std::fs::read_to_string(path).map_err(|e| OptimizeError::IoError {
            path: path.to_path_buf(),
            source: e,
        })?;
        Self::from_toml(&content)
    }

    /// Save training data to a file
    pub fn save(&self, path: impl AsRef<Path>) -> Result<(), OptimizeError> {
        let path = path.as_ref();
        let content = toml::to_string_pretty(self).map_err(OptimizeError::SerializeError)?;

        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| OptimizeError::IoError {
                path: parent.to_path_buf(),
                source: e,
            })?;
        }

        std::fs::write(path, content).map_err(|e| OptimizeError::IoError {
            path: path.to_path_buf(),
            source: e,
        })
    }

    /// Parse from TOML string
    pub fn from_toml(content: &str) -> Result<Self, OptimizeError> {
        toml::from_str(content).map_err(OptimizeError::ParseError)
    }

    /// Load from default path (~/.codex-dashflow/training.toml)
    pub fn load_default() -> Result<Self, OptimizeError> {
        let home = dirs::home_dir().ok_or(OptimizeError::NoHomeDir)?;
        let path = home.join(".codex-dashflow").join("training.toml");
        if path.exists() {
            Self::load(&path)
        } else {
            Ok(Self::new())
        }
    }

    /// Save to default path
    pub fn save_default(&self) -> Result<(), OptimizeError> {
        let home = dirs::home_dir().ok_or(OptimizeError::NoHomeDir)?;
        let path = home.join(".codex-dashflow").join("training.toml");
        self.save(&path)
    }

    /// Get number of examples
    pub fn len(&self) -> usize {
        self.examples.len()
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.examples.is_empty()
    }

    /// Calculate average score
    pub fn average_score(&self) -> f64 {
        if self.examples.is_empty() {
            return 0.0;
        }
        let sum: f64 = self.examples.iter().map(|e| e.score).sum();
        sum / self.examples.len() as f64
    }
}

// ============================================================================
// Optimization Runner
// ============================================================================

/// Result of running optimization
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct OptimizationResult {
    /// Initial score before optimization
    pub initial_score: f64,
    /// Final score after optimization
    pub final_score: f64,
    /// Score improvement
    pub improvement: f64,
    /// Number of few-shot examples generated
    pub examples_generated: usize,
    /// Optimizer used
    pub optimizer: String,
    /// Duration in seconds
    pub duration_secs: f64,
}

impl OptimizationResult {
    /// Calculate improvement percentage
    pub fn improvement_percent(&self) -> f64 {
        if self.initial_score == 0.0 {
            return 0.0;
        }
        (self.improvement / self.initial_score) * 100.0
    }
}

/// Run optimization on the prompt registry using training data
///
/// This function uses the collected training data to generate optimized
/// few-shot examples for the system prompt.
pub fn optimize_prompts(
    registry: &mut PromptRegistry,
    training: &TrainingData,
    config: &OptimizeConfig,
) -> Result<OptimizationResult, OptimizeError> {
    use std::time::Instant;

    let start = Instant::now();

    // Get current system prompt config
    let mut system_config = registry.get_prompt("system").cloned().unwrap_or_default();

    // Calculate initial score (average of current examples)
    let initial_score = if system_config.few_shot_examples.is_empty() {
        0.0
    } else {
        system_config
            .few_shot_examples
            .iter()
            .map(|e| e.score)
            .sum::<f64>()
            / system_config.few_shot_examples.len() as f64
    };

    // Select best training examples as few-shot demos
    let min_score = 0.7; // Only use high-quality examples
    let good_examples: Vec<_> = training.filter_by_score(min_score);

    if good_examples.is_empty() {
        return Ok(OptimizationResult {
            initial_score,
            final_score: initial_score,
            improvement: 0.0,
            examples_generated: 0,
            optimizer: config.optimizer.clone(),
            duration_secs: start.elapsed().as_secs_f64(),
        });
    }

    // Convert to few-shot examples
    let few_shot_examples = training.to_few_shot_examples(config.few_shot_count);
    let examples_generated = few_shot_examples.len();

    // Calculate final score
    let final_score = if few_shot_examples.is_empty() {
        initial_score
    } else {
        few_shot_examples.iter().map(|e| e.score).sum::<f64>() / few_shot_examples.len() as f64
    };

    // Update system config with new examples
    system_config.few_shot_examples = few_shot_examples;
    system_config.metadata = OptimizationMetadata {
        optimizer: Some(config.optimizer.clone()),
        best_score: final_score,
        iterations: 1,
        timestamp: Some(chrono_timestamp()),
        training_size: training.len(),
        ..Default::default()
    };

    // Update registry
    registry.set_prompt("system", system_config);

    let duration_secs = start.elapsed().as_secs_f64();

    Ok(OptimizationResult {
        initial_score,
        final_score,
        improvement: final_score - initial_score,
        examples_generated,
        optimizer: config.optimizer.clone(),
        duration_secs,
    })
}

/// Generate a simple timestamp string
fn chrono_timestamp() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    format!("{}", duration.as_secs())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_few_shot_example() {
        let example = FewShotExample::new(
            "List all Rust files",
            "I'll search for Rust files using the shell tool.",
        )
        .with_reasoning("The user wants to find .rs files, so I should use a find or ls command");

        assert_eq!(example.score, 1.0);
        let formatted = example.format_for_prompt();
        assert!(formatted.contains("List all Rust files"));
        assert!(formatted.contains("Thinking:"));
    }

    #[test]
    fn test_prompt_config_default() {
        let config = PromptConfig::default();
        assert!(config.instruction.contains("coding assistant"));
        assert!(config.few_shot_examples.is_empty());
    }

    #[test]
    fn test_prompt_config_with_examples() {
        let examples = vec![
            FewShotExample::new("What files are here?", "Let me check..."),
            FewShotExample::new("Read the README", "I'll read that file..."),
        ];

        let config = PromptConfig::new("You are a helpful assistant.").with_examples(examples);

        let prompt = config.build_prompt();
        assert!(prompt.contains("You are a helpful assistant."));
        assert!(prompt.contains("Example 1:"));
        assert!(prompt.contains("Example 2:"));
        assert!(prompt.contains("What files are here?"));
    }

    #[test]
    fn test_prompt_registry_defaults() {
        let registry = PromptRegistry::with_defaults();
        let system_prompt = registry.get_system_prompt();
        assert!(system_prompt.contains("coding assistant"));
    }

    #[test]
    fn test_prompt_registry_custom() {
        let mut registry = PromptRegistry::new();
        registry.set_prompt("system", PromptConfig::new("Custom system prompt"));

        let prompt = registry.get_system_prompt();
        assert_eq!(prompt, "Custom system prompt");
    }

    #[test]
    fn test_prompt_registry_from_toml() {
        let toml = r#"
version = 1

[prompts.system]
instruction = "You are a code review assistant."

[[prompts.system.few_shot_examples]]
user_input = "Review this code"
expected_output = "Let me analyze the code..."
score = 0.95
"#;
        let registry = PromptRegistry::from_toml(toml).unwrap();
        assert_eq!(registry.version, 1);

        let config = registry.get_prompt("system").unwrap();
        assert!(config.instruction.contains("code review"));
        assert_eq!(config.few_shot_examples.len(), 1);
        assert_eq!(config.few_shot_examples[0].score, 0.95);
    }

    #[test]
    fn test_optimization_metadata() {
        let metadata = OptimizationMetadata {
            optimizer: Some("BootstrapFewShot".to_string()),
            best_score: 0.87,
            iterations: 5,
            training_size: 100,
            ..Default::default()
        };

        assert_eq!(metadata.optimizer, Some("BootstrapFewShot".to_string()));
        assert_eq!(metadata.best_score, 0.87);
    }

    #[test]
    fn test_optimize_config_defaults() {
        let config = OptimizeConfig::default();
        assert_eq!(config.optimizer, "BootstrapFewShot");
        assert_eq!(config.max_iterations, 10);
        assert_eq!(config.few_shot_count, 3);
    }

    // Training Data tests
    #[test]
    fn test_training_example_new() {
        let example = TrainingExample::new("List files", "Here are the files...", 0.9);
        assert_eq!(example.user_input, "List files");
        assert_eq!(example.agent_output, "Here are the files...");
        assert_eq!(example.score, 0.9);
        assert!(example.tool_calls.is_empty());
        assert!(example.timestamp.is_some());
    }

    #[test]
    fn test_training_example_with_tool_calls() {
        let example = TrainingExample::new("List files", "Output...", 0.85)
            .with_tool_calls(vec!["shell".to_string(), "read_file".to_string()]);
        assert_eq!(example.tool_calls.len(), 2);
        assert_eq!(example.tool_calls[0], "shell");
    }

    #[test]
    fn test_training_example_to_few_shot() {
        let example = TrainingExample::new("Input", "Output", 0.95);
        let few_shot = example.to_few_shot();
        assert_eq!(few_shot.user_input, "Input");
        assert_eq!(few_shot.expected_output, "Output");
        assert_eq!(few_shot.score, 0.95);
    }

    #[test]
    fn test_training_data_new() {
        let data = TrainingData::new();
        assert!(data.examples.is_empty());
        assert_eq!(data.version, 1);
    }

    #[test]
    fn test_training_data_add_example() {
        let mut data = TrainingData::new();
        data.add_example("Question", "Answer", 0.8);
        assert_eq!(data.len(), 1);
        assert_eq!(data.examples[0].user_input, "Question");
    }

    #[test]
    fn test_training_data_add_example_with_tools() {
        let mut data = TrainingData::new();
        data.add_example_with_tools("Query", "Response", 0.9, vec!["tool1".to_string()]);
        assert_eq!(data.len(), 1);
        assert_eq!(data.examples[0].tool_calls.len(), 1);
    }

    #[test]
    fn test_training_data_filter_by_score() {
        let mut data = TrainingData::new();
        data.add_example("Low", "Low output", 0.3);
        data.add_example("Medium", "Medium output", 0.6);
        data.add_example("High", "High output", 0.9);

        let filtered = data.filter_by_score(0.5);
        assert_eq!(filtered.len(), 2);
    }

    #[test]
    fn test_training_data_top_examples() {
        let mut data = TrainingData::new();
        data.add_example("Third", "Output3", 0.7);
        data.add_example("First", "Output1", 0.95);
        data.add_example("Second", "Output2", 0.85);

        let top = data.top_examples(2);
        assert_eq!(top.len(), 2);
        assert_eq!(top[0].user_input, "First"); // Highest score
        assert_eq!(top[1].user_input, "Second"); // Second highest
    }

    #[test]
    fn test_training_data_to_few_shot_examples() {
        let mut data = TrainingData::new();
        data.add_example("Q1", "A1", 0.9);
        data.add_example("Q2", "A2", 0.8);

        let few_shots = data.to_few_shot_examples(2);
        assert_eq!(few_shots.len(), 2);
        assert_eq!(few_shots[0].score, 0.9);
    }

    #[test]
    fn test_training_data_average_score() {
        let mut data = TrainingData::new();
        data.add_example("Q1", "A1", 0.8);
        data.add_example("Q2", "A2", 0.6);
        assert!((data.average_score() - 0.7).abs() < 0.01);
    }

    #[test]
    fn test_training_data_average_score_empty() {
        let data = TrainingData::new();
        assert_eq!(data.average_score(), 0.0);
    }

    #[test]
    fn test_training_data_is_empty() {
        let data = TrainingData::new();
        assert!(data.is_empty());

        let mut data2 = TrainingData::new();
        data2.add_example("Q", "A", 0.5);
        assert!(!data2.is_empty());
    }

    #[test]
    fn test_training_data_from_toml() {
        let toml = r#"
version = 1

[[examples]]
user_input = "List files"
agent_output = "Here are the files..."
score = 0.9
tool_calls = ["shell"]
"#;
        let data = TrainingData::from_toml(toml).unwrap();
        assert_eq!(data.examples.len(), 1);
        assert_eq!(data.examples[0].user_input, "List files");
        assert_eq!(data.examples[0].tool_calls[0], "shell");
    }

    // Optimization tests
    #[test]
    fn test_optimization_result_improvement_percent() {
        let result = OptimizationResult {
            initial_score: 0.5,
            final_score: 0.75,
            improvement: 0.25,
            examples_generated: 3,
            optimizer: "BootstrapFewShot".to_string(),
            duration_secs: 1.5,
        };
        assert!((result.improvement_percent() - 50.0).abs() < 0.01);
    }

    #[test]
    fn test_optimization_result_improvement_percent_zero_initial() {
        let result = OptimizationResult {
            initial_score: 0.0,
            final_score: 0.5,
            improvement: 0.5,
            examples_generated: 2,
            optimizer: "Test".to_string(),
            duration_secs: 0.5,
        };
        assert_eq!(result.improvement_percent(), 0.0);
    }

    #[test]
    fn test_optimize_prompts_empty_training() {
        let mut registry = PromptRegistry::with_defaults();
        let training = TrainingData::new();
        let config = OptimizeConfig::default();

        let result = optimize_prompts(&mut registry, &training, &config).unwrap();
        assert_eq!(result.examples_generated, 0);
        assert_eq!(result.improvement, 0.0);
    }

    #[test]
    fn test_optimize_prompts_low_quality_training() {
        let mut registry = PromptRegistry::with_defaults();
        let mut training = TrainingData::new();
        // Add low-quality examples (below 0.7 threshold)
        training.add_example("Q1", "A1", 0.3);
        training.add_example("Q2", "A2", 0.5);
        let config = OptimizeConfig::default();

        let result = optimize_prompts(&mut registry, &training, &config).unwrap();
        assert_eq!(result.examples_generated, 0); // No examples above threshold
    }

    #[test]
    fn test_optimize_prompts_with_good_training() {
        let mut registry = PromptRegistry::with_defaults();
        let mut training = TrainingData::new();
        training.add_example(
            "List files in current directory",
            "Let me run ls for you...",
            0.9,
        );
        training.add_example("Read the README", "Here's the README content...", 0.85);
        training.add_example("Create a new file", "I'll create that file for you...", 0.8);
        let config = OptimizeConfig::default();

        let result = optimize_prompts(&mut registry, &training, &config).unwrap();

        // Should have generated examples
        assert!(result.examples_generated > 0);
        assert!(result.examples_generated <= config.few_shot_count);

        // Should have updated the registry
        let system_config = registry.get_prompt("system").unwrap();
        assert!(!system_config.few_shot_examples.is_empty());
        assert!(system_config.metadata.optimizer.is_some());
    }

    #[test]
    fn test_optimize_prompts_respects_few_shot_count() {
        let mut registry = PromptRegistry::with_defaults();
        let mut training = TrainingData::new();
        // Add many high-quality examples
        for i in 0..10 {
            training.add_example(
                format!("Q{}", i),
                format!("A{}", i),
                0.8 + (i as f64) * 0.01,
            );
        }
        let config = OptimizeConfig {
            few_shot_count: 3,
            ..Default::default()
        };

        let result = optimize_prompts(&mut registry, &training, &config).unwrap();
        assert_eq!(result.examples_generated, 3);
    }

    // Additional tests for comprehensive coverage

    #[test]
    fn test_few_shot_example_format_without_reasoning() {
        let example = FewShotExample::new("What is 2+2?", "The answer is 4.");
        let formatted = example.format_for_prompt();
        assert!(formatted.contains("User: What is 2+2?"));
        assert!(formatted.contains("Assistant: The answer is 4."));
        assert!(!formatted.contains("Thinking:"));
    }

    #[test]
    fn test_few_shot_example_default_score() {
        let example = FewShotExample::new("Q", "A");
        assert_eq!(example.score, 1.0);
        assert!(example.reasoning.is_none());
    }

    #[test]
    fn test_prompt_config_new() {
        let config = PromptConfig::new("Custom instruction");
        assert_eq!(config.instruction, "Custom instruction");
        assert!(config.few_shot_examples.is_empty());
    }

    #[test]
    fn test_prompt_config_build_prompt_no_examples() {
        let config = PromptConfig::new("Base instruction only");
        let prompt = config.build_prompt();
        assert_eq!(prompt, "Base instruction only");
        assert!(!prompt.contains("Examples"));
    }

    #[test]
    fn test_prompt_registry_new() {
        let registry = PromptRegistry::new();
        assert!(registry.prompts.is_empty());
        // Default derive gives 0, serde default gives 1 on deserialization
        assert_eq!(registry.version, 0);
    }

    #[test]
    fn test_prompt_registry_get_prompt_missing() {
        let registry = PromptRegistry::new();
        assert!(registry.get_prompt("nonexistent").is_none());
    }

    #[test]
    fn test_prompt_registry_get_system_prompt_missing() {
        let registry = PromptRegistry::new();
        let prompt = registry.get_system_prompt();
        assert!(prompt.contains("coding assistant")); // Falls back to default
    }

    #[test]
    fn test_optimization_metadata_default() {
        let metadata = OptimizationMetadata::default();
        assert!(metadata.optimizer.is_none());
        assert_eq!(metadata.best_score, 0.0);
        assert_eq!(metadata.iterations, 0);
        assert!(metadata.timestamp.is_none());
        assert_eq!(metadata.training_size, 0);
        assert!(metadata.custom.is_empty());
    }

    #[test]
    fn test_optimization_metadata_debug() {
        let metadata = OptimizationMetadata {
            optimizer: Some("TestOptimizer".to_string()),
            best_score: 0.95,
            iterations: 10,
            timestamp: Some("12345".to_string()),
            training_size: 50,
            custom: HashMap::new(),
        };
        let debug_str = format!("{:?}", metadata);
        assert!(debug_str.contains("TestOptimizer"));
        assert!(debug_str.contains("0.95"));
    }

    #[test]
    fn test_optimize_config_individual_fields() {
        let config = OptimizeConfig {
            optimizer: "CustomOptimizer".to_string(),
            max_iterations: 20,
            metric: "accuracy".to_string(),
            min_improvement: 0.1,
            few_shot_count: 5,
        };
        assert_eq!(config.optimizer, "CustomOptimizer");
        assert_eq!(config.max_iterations, 20);
        assert_eq!(config.metric, "accuracy");
        assert_eq!(config.min_improvement, 0.1);
        assert_eq!(config.few_shot_count, 5);
    }

    #[test]
    fn test_optimize_error_display_no_home_dir() {
        let err = OptimizeError::NoHomeDir;
        let msg = format!("{}", err);
        assert!(msg.contains("home directory"));
    }

    #[test]
    fn test_optimize_error_display_io_error() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
        let err = OptimizeError::IoError {
            path: PathBuf::from("/some/path"),
            source: io_err,
        };
        let msg = format!("{}", err);
        assert!(msg.contains("/some/path"));
        assert!(msg.contains("file not found"));
    }

    #[test]
    fn test_optimize_error_source_io() {
        use std::error::Error;
        let io_err = std::io::Error::new(std::io::ErrorKind::PermissionDenied, "denied");
        let err = OptimizeError::IoError {
            path: PathBuf::from("/path"),
            source: io_err,
        };
        assert!(err.source().is_some());
    }

    #[test]
    fn test_optimize_error_source_no_home_dir() {
        use std::error::Error;
        let err = OptimizeError::NoHomeDir;
        assert!(err.source().is_none());
    }

    #[test]
    fn test_training_data_filter_by_score_exact_threshold() {
        let mut data = TrainingData::new();
        data.add_example("Exact", "Output", 0.5);
        data.add_example("Below", "Output", 0.49);
        data.add_example("Above", "Output", 0.51);

        let filtered = data.filter_by_score(0.5);
        assert_eq!(filtered.len(), 2); // 0.5 and 0.51
    }

    #[test]
    fn test_training_data_top_examples_more_than_available() {
        let mut data = TrainingData::new();
        data.add_example("Only", "Output", 0.8);

        let top = data.top_examples(10);
        assert_eq!(top.len(), 1); // Only returns what's available
    }

    #[test]
    fn test_training_data_top_examples_empty() {
        let data = TrainingData::new();
        let top = data.top_examples(5);
        assert!(top.is_empty());
    }

    #[test]
    fn test_training_data_to_few_shot_examples_empty() {
        let data = TrainingData::new();
        let few_shots = data.to_few_shot_examples(3);
        assert!(few_shots.is_empty());
    }

    #[test]
    fn test_training_data_len() {
        let mut data = TrainingData::new();
        assert_eq!(data.len(), 0);
        data.add_example("Q1", "A1", 0.5);
        assert_eq!(data.len(), 1);
        data.add_example("Q2", "A2", 0.6);
        assert_eq!(data.len(), 2);
    }

    #[test]
    fn test_training_example_timestamp_present() {
        let example = TrainingExample::new("Q", "A", 0.9);
        assert!(example.timestamp.is_some());
        let ts = example.timestamp.unwrap();
        // Should be a numeric string (Unix timestamp)
        assert!(ts.parse::<u64>().is_ok());
    }

    #[test]
    fn test_default_score_function() {
        assert_eq!(default_score(), 1.0);
    }

    #[test]
    fn test_default_version_function() {
        assert_eq!(default_version(), 1);
    }

    #[test]
    fn test_default_optimizer_function() {
        assert_eq!(default_optimizer(), "BootstrapFewShot");
    }

    #[test]
    fn test_default_max_iterations_function() {
        assert_eq!(default_max_iterations(), 10);
    }

    #[test]
    fn test_default_metric_function() {
        assert_eq!(default_metric(), "task_completion");
    }

    #[test]
    fn test_default_min_improvement_function() {
        assert_eq!(default_min_improvement(), 0.05);
    }

    #[test]
    fn test_default_few_shot_count_function() {
        assert_eq!(default_few_shot_count(), 3);
    }

    #[test]
    fn test_chrono_timestamp_numeric() {
        let ts = chrono_timestamp();
        assert!(ts.parse::<u64>().is_ok());
    }

    #[test]
    fn test_prompt_registry_save_and_load() {
        use tempfile::tempdir;
        let tmp = tempdir().expect("create tempdir");
        let path = tmp.path().join("prompts.toml");

        let mut registry = PromptRegistry::new();
        registry.set_prompt("test", PromptConfig::new("Test instruction"));

        registry.save(&path).expect("save");
        let loaded = PromptRegistry::load(&path).expect("load");

        assert_eq!(
            loaded.get_prompt("test").unwrap().instruction,
            "Test instruction"
        );
    }

    #[test]
    fn test_training_data_save_and_load() {
        use tempfile::tempdir;
        let tmp = tempdir().expect("create tempdir");
        let path = tmp.path().join("training.toml");

        let mut data = TrainingData::new();
        data.add_example("Q1", "A1", 0.8);
        data.add_example_with_tools("Q2", "A2", 0.9, vec!["shell".to_string()]);

        data.save(&path).expect("save");
        let loaded = TrainingData::load(&path).expect("load");

        assert_eq!(loaded.len(), 2);
        assert_eq!(loaded.examples[1].tool_calls.len(), 1);
    }

    #[test]
    fn test_prompt_registry_load_nonexistent() {
        let result = PromptRegistry::load("/nonexistent/path/prompts.toml");
        assert!(result.is_err());
    }

    #[test]
    fn test_training_data_load_nonexistent() {
        let result = TrainingData::load("/nonexistent/path/training.toml");
        assert!(result.is_err());
    }

    #[test]
    fn test_prompt_registry_from_toml_invalid() {
        let invalid_toml = "this is not valid toml [[[";
        let result = PromptRegistry::from_toml(invalid_toml);
        assert!(result.is_err());
    }

    #[test]
    fn test_training_data_from_toml_invalid() {
        let invalid_toml = "[[invalid";
        let result = TrainingData::from_toml(invalid_toml);
        assert!(result.is_err());
    }

    #[test]
    fn test_few_shot_example_clone() {
        let example = FewShotExample::new("Q", "A").with_reasoning("Think");
        let cloned = example.clone();
        assert_eq!(cloned.user_input, "Q");
        assert_eq!(cloned.reasoning, Some("Think".to_string()));
    }

    #[test]
    fn test_prompt_config_clone() {
        let config =
            PromptConfig::new("Instruction").with_examples(vec![FewShotExample::new("Q", "A")]);
        let cloned = config.clone();
        assert_eq!(cloned.instruction, "Instruction");
        assert_eq!(cloned.few_shot_examples.len(), 1);
    }

    #[test]
    fn test_training_example_clone() {
        let example = TrainingExample::new("Q", "A", 0.9);
        let cloned = example.clone();
        assert_eq!(cloned.user_input, "Q");
        assert_eq!(cloned.score, 0.9);
    }

    #[test]
    fn test_training_data_clone() {
        let mut data = TrainingData::new();
        data.add_example("Q", "A", 0.8);
        let cloned = data.clone();
        assert_eq!(cloned.len(), 1);
    }

    #[test]
    fn test_optimization_result_clone() {
        let result = OptimizationResult {
            initial_score: 0.5,
            final_score: 0.8,
            improvement: 0.3,
            examples_generated: 2,
            optimizer: "Test".to_string(),
            duration_secs: 1.0,
        };
        let cloned = result.clone();
        assert_eq!(cloned.final_score, 0.8);
    }

    #[test]
    fn test_optimize_config_clone() {
        let config = OptimizeConfig::default();
        let cloned = config.clone();
        assert_eq!(cloned.optimizer, "BootstrapFewShot");
    }

    #[test]
    fn test_prompt_registry_version_default() {
        let registry = PromptRegistry::new();
        // Default derive gives 0; serde default_version gives 1 during deserialization
        assert_eq!(registry.version, 0);
    }

    #[test]
    fn test_training_data_version_default() {
        let data = TrainingData::new();
        assert_eq!(data.version, 1);
    }

    #[test]
    fn test_few_shot_example_serde_roundtrip() {
        let example = FewShotExample::new("Question", "Answer").with_reasoning("Think about it");
        let json = serde_json::to_string(&example).unwrap();
        let restored: FewShotExample = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.user_input, "Question");
        assert_eq!(restored.expected_output, "Answer");
        assert_eq!(restored.reasoning, Some("Think about it".to_string()));
    }

    #[test]
    fn test_prompt_config_serde_roundtrip() {
        let config = PromptConfig::new("Test").with_examples(vec![FewShotExample::new("Q", "A")]);
        let json = serde_json::to_string(&config).unwrap();
        let restored: PromptConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.instruction, "Test");
        assert_eq!(restored.few_shot_examples.len(), 1);
    }

    #[test]
    fn test_training_example_serde_roundtrip() {
        let example =
            TrainingExample::new("Q", "A", 0.85).with_tool_calls(vec!["tool1".to_string()]);
        let json = serde_json::to_string(&example).unwrap();
        let restored: TrainingExample = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.score, 0.85);
        assert_eq!(restored.tool_calls.len(), 1);
    }

    #[test]
    fn test_training_data_serde_roundtrip() {
        let mut data = TrainingData::new();
        data.add_example("Q", "A", 0.9);
        let json = serde_json::to_string(&data).unwrap();
        let restored: TrainingData = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.len(), 1);
    }

    #[test]
    fn test_optimization_result_serde_roundtrip() {
        let result = OptimizationResult {
            initial_score: 0.4,
            final_score: 0.7,
            improvement: 0.3,
            examples_generated: 3,
            optimizer: "Test".to_string(),
            duration_secs: 2.5,
        };
        let json = serde_json::to_string(&result).unwrap();
        let restored: OptimizationResult = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.improvement, 0.3);
    }

    #[test]
    fn test_optimize_config_serde_roundtrip() {
        let config = OptimizeConfig {
            optimizer: "Custom".to_string(),
            max_iterations: 15,
            metric: "recall".to_string(),
            min_improvement: 0.02,
            few_shot_count: 4,
        };
        let json = serde_json::to_string(&config).unwrap();
        let restored: OptimizeConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.optimizer, "Custom");
        assert_eq!(restored.max_iterations, 15);
    }

    #[test]
    fn test_optimization_metadata_serde_roundtrip() {
        let mut custom = HashMap::new();
        custom.insert("key".to_string(), "value".to_string());
        let metadata = OptimizationMetadata {
            optimizer: Some("Opt".to_string()),
            best_score: 0.99,
            iterations: 5,
            timestamp: Some("123".to_string()),
            training_size: 100,
            custom,
        };
        let json = serde_json::to_string(&metadata).unwrap();
        let restored: OptimizationMetadata = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.best_score, 0.99);
        assert_eq!(restored.custom.get("key"), Some(&"value".to_string()));
    }

    #[test]
    fn test_prompt_registry_serde_roundtrip() {
        let mut registry = PromptRegistry::new();
        registry.set_prompt("test", PromptConfig::new("Instruction"));
        let json = serde_json::to_string(&registry).unwrap();
        let restored: PromptRegistry = serde_json::from_str(&json).unwrap();
        assert!(restored.get_prompt("test").is_some());
    }

    #[test]
    fn test_optimize_prompts_updates_metadata() {
        let mut registry = PromptRegistry::with_defaults();
        let mut training = TrainingData::new();
        training.add_example("Q", "A", 0.9);
        let config = OptimizeConfig::default();

        optimize_prompts(&mut registry, &training, &config).unwrap();

        let system_config = registry.get_prompt("system").unwrap();
        assert!(system_config.metadata.timestamp.is_some());
        assert_eq!(system_config.metadata.training_size, 1);
    }

    #[test]
    fn test_optimize_prompts_with_existing_examples() {
        let mut registry = PromptRegistry::new();
        let existing_config =
            PromptConfig::new("Test").with_examples(vec![FewShotExample::new("Old Q", "Old A")]);
        registry.set_prompt("system", existing_config);

        let mut training = TrainingData::new();
        training.add_example("New Q", "New A", 0.95);

        let config = OptimizeConfig::default();
        let result = optimize_prompts(&mut registry, &training, &config).unwrap();

        // Initial score should be calculated from existing examples
        assert_eq!(result.initial_score, 1.0); // default score for existing example
    }

    #[test]
    fn test_default_system_prompt_content() {
        assert!(DEFAULT_SYSTEM_PROMPT.contains("shell"));
        assert!(DEFAULT_SYSTEM_PROMPT.contains("read_file"));
        assert!(DEFAULT_SYSTEM_PROMPT.contains("write_file"));
        assert!(DEFAULT_SYSTEM_PROMPT.contains("apply_patch"));
        assert!(DEFAULT_SYSTEM_PROMPT.contains("search_files"));
    }

    #[test]
    fn test_prompt_config_build_prompt_multiple_examples() {
        let examples = vec![
            FewShotExample::new("Q1", "A1"),
            FewShotExample::new("Q2", "A2"),
            FewShotExample::new("Q3", "A3"),
        ];
        let config = PromptConfig::new("Instruction").with_examples(examples);
        let prompt = config.build_prompt();

        assert!(prompt.contains("Example 1:"));
        assert!(prompt.contains("Example 2:"));
        assert!(prompt.contains("Example 3:"));
        assert!(prompt.contains("## Examples"));
    }

    #[test]
    fn test_training_data_filter_by_score_all_pass() {
        let mut data = TrainingData::new();
        data.add_example("Q1", "A1", 0.9);
        data.add_example("Q2", "A2", 0.95);
        data.add_example("Q3", "A3", 0.85);

        let filtered = data.filter_by_score(0.8);
        assert_eq!(filtered.len(), 3);
    }

    #[test]
    fn test_training_data_filter_by_score_none_pass() {
        let mut data = TrainingData::new();
        data.add_example("Q1", "A1", 0.3);
        data.add_example("Q2", "A2", 0.4);

        let filtered = data.filter_by_score(0.9);
        assert!(filtered.is_empty());
    }

    #[test]
    fn test_training_data_top_examples_sorting() {
        let mut data = TrainingData::new();
        data.add_example("Low", "A", 0.5);
        data.add_example("High", "A", 0.99);
        data.add_example("Mid", "A", 0.75);

        let top = data.top_examples(3);
        assert_eq!(top[0].user_input, "High");
        assert_eq!(top[1].user_input, "Mid");
        assert_eq!(top[2].user_input, "Low");
    }

    #[test]
    fn test_optimization_result_negative_improvement() {
        let result = OptimizationResult {
            initial_score: 0.8,
            final_score: 0.6,
            improvement: -0.2,
            examples_generated: 1,
            optimizer: "Test".to_string(),
            duration_secs: 0.1,
        };
        assert!(result.improvement_percent() < 0.0);
    }

    #[test]
    fn test_prompt_registry_set_overwrites() {
        let mut registry = PromptRegistry::new();
        registry.set_prompt("key", PromptConfig::new("First"));
        registry.set_prompt("key", PromptConfig::new("Second"));

        let config = registry.get_prompt("key").unwrap();
        assert_eq!(config.instruction, "Second");
    }

    #[test]
    fn test_training_example_empty_tool_calls() {
        let example = TrainingExample::new("Q", "A", 0.5);
        assert!(example.tool_calls.is_empty());
    }

    #[test]
    fn test_optimization_metadata_custom_map() {
        let mut custom = HashMap::new();
        custom.insert("model".to_string(), "gpt-4".to_string());
        custom.insert("temperature".to_string(), "0.7".to_string());

        let metadata = OptimizationMetadata {
            custom,
            ..Default::default()
        };

        assert_eq!(metadata.custom.len(), 2);
        assert_eq!(metadata.custom.get("model"), Some(&"gpt-4".to_string()));
    }
}
