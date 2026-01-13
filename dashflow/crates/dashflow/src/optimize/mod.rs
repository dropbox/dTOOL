// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

//! # DashOptimize - Native Prompt Optimization for DashFlow
//!
//! DashOptimize provides automatic prompt optimization as a core feature of DashFlow.
//! Instead of manually engineering prompts, define task signatures and training data,
//! then let DashOptimize find optimal prompts algorithmically.
//!
//! ## Core Concepts
//!
//! - **Signature**: Task definition (inputs → outputs with semantic description)
//! - **Optimizable Nodes**: LLM nodes that can improve their prompts with training data
//! - **Optimizers**: Algorithms that find better prompts (BootstrapFewShot, RandomSearch, SIMBA, GEPA, MIPROv2, etc.)
//! - **Metrics**: Functions that measure output quality for optimization
//!
//! ## Example: Basic Optimization
//!
//! ```rust,ignore
//! use dashflow::prelude::*;
//! use dashflow::optimize::*;
//!
//! #[derive(Clone, Serialize, Deserialize)]
//! struct ClassifierState {
//!     text: String,
//!     category: String,
//! }
//!
//! let mut graph = StateGraph::new();
//!
//! // Add optimizable LLM node
//! graph.add_llm_node("classify")
//!     .with_signature("text -> category", "Classify text sentiment")
//!     .with_llm(client);
//!
//! // Training data
//! let trainset = vec![
//!     ClassifierState {
//!         text: "I love this!".to_string(),
//!         category: "positive".to_string(),
//!     },
//!     ClassifierState {
//!         text: "This is terrible".to_string(),
//!         category: "negative".to_string(),
//!     },
//! ];
//!
//! // Optimize before deployment
//! graph.optimize()
//!     .with_data(trainset)
//!     .with_metric(accuracy_metric)
//!     .with_optimizer(BootstrapFewShot::default())
//!     .run()
//!     .await?;
//!
//! let app = graph.compile()?;
//! ```

use crate::node::Node;
use crate::state::GraphState;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

pub mod ab_testing;
pub mod aggregation;
pub mod auto_optimizer;
pub mod cost_monitoring;
pub mod data_collection;
pub mod distillation;
pub mod example;
pub mod ext;
pub mod graph_optimizer;
pub mod knn;
pub mod llm_node;
pub mod metrics;
pub mod modules;
pub mod multi_objective;
pub mod optimizers;
pub mod propose;
pub mod signature;
pub mod telemetry;
pub mod types;

pub use signature::{make_signature, Field, FieldKind, Signature};

// Re-export types
pub use types::{
    Audio, AudioFormat, Citation, Code, Document, File, FileType, History, Image, ImageFormat,
    Language, LlmContent, Message, Reasoning, ReasoningEffort, ReasoningOutput, ReasoningStep,
    Role, ToLlmContent, ToolCall, ToolCalls, ToolResult,
};

// Re-export optimizers
// Note: TraceStep is deprecated - use ExecutionTrace and NodeExecution instead
#[allow(deprecated)]
pub use optimizers::{
    AutoPrompt, AutoPromptBuilder, AutoPromptMetricFn, BootstrapFewShot, BootstrapOptuna, COPROv2,
    COPROv2Builder, COPROv2MetricFn, CandidateProgram, GEPAConfig, GEPAMetricFn, GEPAResult,
    KNNFewShot, LabeledFewShot, OptimizationResult, OptimizerConfig, RandomSearch,
    ScoreWithFeedback, SelectionStrategy, SimbaOutput, SimbaStrategy, StrategyContext, TraceStep,
    GEPA, SIMBA,
};

// Re-export graph optimizer
pub use graph_optimizer::{GraphOptimizer, OptimizationStrategy};

// Re-export aggregation utilities
pub use aggregation::{default_normalize, majority};

// Re-export metrics
pub use metrics::{
    compute_all_json_metrics, exact_match, exact_match_any, f1_score, json_exact_match,
    json_f1_score, json_precision_score, json_recall_score, max_f1, normalize_text,
    precision_score, recall_score, JsonMetricConfig, MetricFn, SemanticF1, SemanticF1Config,
    SemanticF1Result,
};

// Re-export modules
pub use modules::{ChainOfThoughtNode, MultiChainComparisonNode, ReActNode, SimpleTool, Tool};

// Re-export extension traits
pub use ext::DspyGraphExt;

// Re-export LLMNode for optimizable LLM nodes
pub use llm_node::LLMNode;

// Re-export distillation
pub use distillation::{
    CostAnalysis, DistillationConfig, DistillationConfigBuilder, DistillationReport,
    DistillationResult, ModelDistillation, QualityGap, ROIMetrics, SyntheticDataConfig,
    SyntheticDataGenerator,
};

// Re-export KNN and Example
pub use example::Example;
pub use knn::KNN;

// Re-export telemetry for optimizer instrumentation
pub use telemetry::{
    record_candidate_evaluated, record_demos_added, record_error, record_iteration,
    record_optimization_complete, record_optimization_start, record_rules_generated,
    OptimizerMetrics,
};

// Re-export cost monitoring
// DEPRECATED: Use dashflow_observability::cost instead for the consolidated implementation.
// This module will be removed in a future release.
#[allow(deprecated)]
pub use cost_monitoring::{
    AlertLevel, BudgetConfig, BudgetEnforcer, CostMonitor, CostMonitorError, CostReport,
    ModelPrice, ModelPricing, TokenUsage, UsageRecord,
};

// Re-export A/B testing
pub use ab_testing::{
    ABTest, ConfidenceInterval, ResultsReport, StatisticalAnalysis, TTestResult, TrafficSplitter,
    Variant, VariantReport,
};

// Re-export data collection
pub use data_collection::{
    DataCollector, DataFormat, DataSource, DataStore, DistributionAnalysis, DistributionAnalyzer,
    TrainingExample,
};

// Re-export auto optimizer (automatic optimizer selection)
pub use auto_optimizer::{
    recommend, select_for_examples, select_optimizer, AlternativeOptimizer, AutoOptimizer,
    ComputeBudget, OptimizationContext, OptimizationContextBuilder, OptimizationOutcome,
    OptimizerStats, SelectionResult, TaskType,
};

/// Nodes that can be optimized with training data
///
/// Optimization adjusts the node's behavior (prompts, few-shot examples, etc.)
/// to maximize performance on a given metric using training examples.
#[async_trait]
pub trait Optimizable<S: GraphState>: Node<S> {
    /// Optimize this node using training examples
    ///
    /// # Arguments
    /// * `examples` - Training data (states with expected outputs)
    /// * `metric` - Function that scores quality (0.0 to 1.0)
    /// * `config` - Optimizer configuration
    ///
    /// # Returns
    /// Results of optimization (score, iterations, etc.)
    async fn optimize(
        &mut self,
        examples: &[S],
        metric: &MetricFn<S>,
        config: &OptimizerConfig,
    ) -> crate::Result<OptimizationResult>;

    /// Get current optimization state (prompts, examples, etc.)
    fn get_optimization_state(&self) -> OptimizationState;

    /// Load pre-computed optimization state
    fn set_optimization_state(&mut self, state: OptimizationState);
}

/// State captured during optimization (can be saved/loaded)
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct OptimizationState {
    /// System prompt / instruction
    pub instruction: String,

    /// Few-shot examples (if any)
    pub few_shot_examples: Vec<FewShotExample>,

    /// Metadata (optimizer used, timestamp, etc.)
    pub metadata: std::collections::HashMap<String, String>,
}

/// A few-shot example used in optimized prompts
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FewShotExample {
    /// Input fields as JSON
    pub input: serde_json::Value,

    /// Output fields as JSON
    pub output: serde_json::Value,

    /// Optional reasoning/chain-of-thought
    pub reasoning: Option<String>,
}

impl OptimizationState {
    /// Create a new optimization state with the given instruction.
    pub fn new(instruction: impl Into<String>) -> Self {
        Self {
            instruction: instruction.into(),
            few_shot_examples: Vec::new(),
            metadata: std::collections::HashMap::new(),
        }
    }

    /// Add few-shot examples to the optimization state.
    #[must_use]
    pub fn with_examples(mut self, examples: Vec<FewShotExample>) -> Self {
        self.few_shot_examples = examples;
        self
    }

    /// Add metadata to the optimization state.
    #[must_use]
    pub fn with_metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }
}

// Core trace types - always available for local optimization (per DESIGN_INVARIANTS.md)
pub mod trace_types;

// Re-export core trace types (always available, no feature gate)
// Note: TraceEntry is deprecated - use ExecutionTrace and NodeExecution instead
#[allow(deprecated)]
pub use trace_types::{FailedPrediction, Prediction, PredictionOrFailed, TraceData, TraceEntry};

// Kafka-based trace collection - requires dashstream feature
#[cfg(feature = "dashstream")]
pub mod trace;

// Re-export TraceCollector when dashstream is enabled
// Note: TraceCollector is deprecated - use ExecutionTrace and ExecutionTraceBuilder instead
#[cfg(feature = "dashstream")]
#[allow(deprecated)]
pub use trace::TraceCollector;

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // ============================================================================
    // FewShotExample Tests
    // ============================================================================

    #[test]
    fn test_few_shot_example_creation() {
        let example = FewShotExample {
            input: json!({"text": "Hello world"}),
            output: json!({"sentiment": "positive"}),
            reasoning: None,
        };
        assert!(example.input.is_object());
        assert!(example.output.is_object());
        assert!(example.reasoning.is_none());
    }

    #[test]
    fn test_few_shot_example_with_reasoning() {
        let example = FewShotExample {
            input: json!({"question": "What is 2+2?"}),
            output: json!({"answer": "4"}),
            reasoning: Some("First I add 2 and 2 to get 4.".to_string()),
        };
        assert!(example.reasoning.is_some());
        assert_eq!(
            example.reasoning.as_ref().unwrap(),
            "First I add 2 and 2 to get 4."
        );
    }

    #[test]
    fn test_few_shot_example_clone() {
        let original = FewShotExample {
            input: json!({"x": 1}),
            output: json!({"y": 2}),
            reasoning: Some("reason".to_string()),
        };
        let cloned = original.clone();
        assert_eq!(original.input, cloned.input);
        assert_eq!(original.output, cloned.output);
        assert_eq!(original.reasoning, cloned.reasoning);
    }

    #[test]
    fn test_few_shot_example_debug() {
        let example = FewShotExample {
            input: json!({}),
            output: json!({}),
            reasoning: None,
        };
        let debug = format!("{:?}", example);
        assert!(debug.contains("FewShotExample"));
    }

    #[test]
    fn test_few_shot_example_serialization() {
        let example = FewShotExample {
            input: json!({"input_field": "value"}),
            output: json!({"output_field": "result"}),
            reasoning: Some("My reasoning".to_string()),
        };
        let json = serde_json::to_string(&example).expect("Serialization failed");
        assert!(json.contains("input_field"));
        assert!(json.contains("output_field"));
        assert!(json.contains("My reasoning"));
    }

    #[test]
    fn test_few_shot_example_deserialization() {
        let json = r#"{
            "input": {"text": "test"},
            "output": {"label": "positive"},
            "reasoning": "Because it contains positive words"
        }"#;
        let example: FewShotExample =
            serde_json::from_str(json).expect("Deserialization failed");
        assert_eq!(example.input["text"], "test");
        assert_eq!(example.output["label"], "positive");
        assert!(example.reasoning.is_some());
    }

    #[test]
    fn test_few_shot_example_deserialization_no_reasoning() {
        let json = r#"{
            "input": {"a": 1},
            "output": {"b": 2},
            "reasoning": null
        }"#;
        let example: FewShotExample =
            serde_json::from_str(json).expect("Deserialization failed");
        assert!(example.reasoning.is_none());
    }

    #[test]
    fn test_few_shot_example_complex_json() {
        let example = FewShotExample {
            input: json!({
                "nested": {
                    "array": [1, 2, 3],
                    "object": {"key": "value"}
                },
                "number": 42.5
            }),
            output: json!({
                "results": ["a", "b", "c"]
            }),
            reasoning: None,
        };
        assert!(example.input["nested"]["array"].is_array());
        assert_eq!(example.input["number"], 42.5);
    }

    // ============================================================================
    // OptimizationState Tests
    // ============================================================================

    #[test]
    fn test_optimization_state_new() {
        let state = OptimizationState::new("You are a helpful assistant.");
        assert_eq!(state.instruction, "You are a helpful assistant.");
        assert!(state.few_shot_examples.is_empty());
        assert!(state.metadata.is_empty());
    }

    #[test]
    fn test_optimization_state_new_with_string() {
        let state = OptimizationState::new(String::from("Classify text"));
        assert_eq!(state.instruction, "Classify text");
    }

    #[test]
    fn test_optimization_state_with_examples() {
        let examples = vec![
            FewShotExample {
                input: json!({"text": "hello"}),
                output: json!({"sentiment": "positive"}),
                reasoning: None,
            },
            FewShotExample {
                input: json!({"text": "goodbye"}),
                output: json!({"sentiment": "neutral"}),
                reasoning: None,
            },
        ];
        let state = OptimizationState::new("Classify sentiment").with_examples(examples);
        assert_eq!(state.few_shot_examples.len(), 2);
    }

    #[test]
    fn test_optimization_state_with_metadata() {
        let state = OptimizationState::new("Instruction")
            .with_metadata("optimizer", "BootstrapFewShot")
            .with_metadata("timestamp", "2025-01-01T00:00:00Z");
        assert_eq!(state.metadata.len(), 2);
        assert_eq!(state.metadata.get("optimizer").unwrap(), "BootstrapFewShot");
        assert_eq!(
            state.metadata.get("timestamp").unwrap(),
            "2025-01-01T00:00:00Z"
        );
    }

    #[test]
    fn test_optimization_state_builder_chain() {
        let examples = vec![FewShotExample {
            input: json!({"q": "test"}),
            output: json!({"a": "answer"}),
            reasoning: Some("chain".to_string()),
        }];
        let state = OptimizationState::new("Chained builder")
            .with_examples(examples)
            .with_metadata("key1", "value1")
            .with_metadata("key2", "value2");

        assert_eq!(state.instruction, "Chained builder");
        assert_eq!(state.few_shot_examples.len(), 1);
        assert_eq!(state.metadata.len(), 2);
    }

    #[test]
    fn test_optimization_state_clone() {
        let original = OptimizationState::new("Original")
            .with_metadata("version", "1.0");
        let cloned = original.clone();
        assert_eq!(original.instruction, cloned.instruction);
        assert_eq!(original.metadata.len(), cloned.metadata.len());
    }

    #[test]
    fn test_optimization_state_debug() {
        let state = OptimizationState::new("Debug test");
        let debug = format!("{:?}", state);
        assert!(debug.contains("OptimizationState"));
        assert!(debug.contains("Debug test"));
    }

    #[test]
    fn test_optimization_state_serialization() {
        let state = OptimizationState::new("Serialize test")
            .with_metadata("optimizer", "RandomSearch");
        let json = serde_json::to_string(&state).expect("Serialization failed");
        assert!(json.contains("Serialize test"));
        assert!(json.contains("RandomSearch"));
    }

    #[test]
    fn test_optimization_state_deserialization() {
        let json = r#"{
            "instruction": "Deserialized instruction",
            "few_shot_examples": [],
            "metadata": {"source": "test"}
        }"#;
        let state: OptimizationState =
            serde_json::from_str(json).expect("Deserialization failed");
        assert_eq!(state.instruction, "Deserialized instruction");
        assert!(state.few_shot_examples.is_empty());
        assert_eq!(state.metadata.get("source").unwrap(), "test");
    }

    #[test]
    fn test_optimization_state_roundtrip() {
        let examples = vec![FewShotExample {
            input: json!({"input": "value"}),
            output: json!({"output": "result"}),
            reasoning: Some("reasoning text".to_string()),
        }];
        let original = OptimizationState::new("Roundtrip test")
            .with_examples(examples)
            .with_metadata("key", "value");

        let json = serde_json::to_string(&original).expect("Serialization failed");
        let restored: OptimizationState =
            serde_json::from_str(&json).expect("Deserialization failed");

        assert_eq!(original.instruction, restored.instruction);
        assert_eq!(
            original.few_shot_examples.len(),
            restored.few_shot_examples.len()
        );
        assert_eq!(original.metadata.len(), restored.metadata.len());
    }

    #[test]
    fn test_optimization_state_empty_instruction() {
        let state = OptimizationState::new("");
        assert!(state.instruction.is_empty());
    }

    #[test]
    fn test_optimization_state_unicode() {
        let state = OptimizationState::new("分類任務：将输入文本分类")
            .with_metadata("언어", "中文");
        assert!(state.instruction.contains("分類"));
        assert!(state.metadata.contains_key("언어"));
    }

    #[test]
    fn test_optimization_state_overwrite_metadata() {
        let state = OptimizationState::new("Test")
            .with_metadata("key", "original")
            .with_metadata("key", "overwritten");
        assert_eq!(state.metadata.get("key").unwrap(), "overwritten");
    }

    #[test]
    fn test_optimization_state_many_examples() {
        let examples: Vec<FewShotExample> = (0..100)
            .map(|i| FewShotExample {
                input: json!({"index": i}),
                output: json!({"doubled": i * 2}),
                reasoning: None,
            })
            .collect();
        let state = OptimizationState::new("Many examples").with_examples(examples);
        assert_eq!(state.few_shot_examples.len(), 100);
        assert_eq!(state.few_shot_examples[50].input["index"], 50);
    }

    #[test]
    fn test_optimization_state_multiline_instruction() {
        let instruction = "Line 1: Do this.\nLine 2: Do that.\nLine 3: Return result.";
        let state = OptimizationState::new(instruction);
        assert!(state.instruction.contains('\n'));
        assert_eq!(state.instruction.lines().count(), 3);
    }

    // ============================================================================
    // Integration Tests
    // ============================================================================

    #[test]
    fn test_complete_optimization_workflow() {
        // Simulate a complete optimization workflow
        let training_examples = vec![
            FewShotExample {
                input: json!({"text": "Great product!"}),
                output: json!({"sentiment": "positive"}),
                reasoning: Some("Contains positive word 'great'".to_string()),
            },
            FewShotExample {
                input: json!({"text": "Terrible experience"}),
                output: json!({"sentiment": "negative"}),
                reasoning: Some("Contains negative word 'terrible'".to_string()),
            },
            FewShotExample {
                input: json!({"text": "It's okay"}),
                output: json!({"sentiment": "neutral"}),
                reasoning: Some("Neither positive nor negative".to_string()),
            },
        ];

        let optimized_state = OptimizationState::new(
            "You are a sentiment classifier. Analyze the text and determine if it's positive, negative, or neutral.",
        )
        .with_examples(training_examples)
        .with_metadata("optimizer", "BootstrapFewShot")
        .with_metadata("iterations", "100")
        .with_metadata("final_score", "0.95");

        // Verify structure
        assert!(optimized_state
            .instruction
            .contains("sentiment classifier"));
        assert_eq!(optimized_state.few_shot_examples.len(), 3);
        assert_eq!(
            optimized_state.metadata.get("optimizer").unwrap(),
            "BootstrapFewShot"
        );

        // Verify serialization preserves all data
        let json = serde_json::to_string(&optimized_state).expect("Serialization failed");
        let restored: OptimizationState =
            serde_json::from_str(&json).expect("Deserialization failed");

        assert_eq!(optimized_state.instruction, restored.instruction);
        assert_eq!(
            optimized_state.few_shot_examples.len(),
            restored.few_shot_examples.len()
        );
        assert_eq!(
            optimized_state.few_shot_examples[0].reasoning,
            restored.few_shot_examples[0].reasoning
        );
    }
}
