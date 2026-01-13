//! Optimizer metadata registry for introspection.
//!
//! Provides structured metadata about all optimizers that can be queried
//! via `dashflow introspect` commands.
//!
//! ## Usage
//!
//! ```rust
//! use dashflow::optimize::optimizers::registry;
//!
//! // Get all optimizers
//! let all = registry::all_optimizers();
//!
//! // Get recommendation based on data size
//! let rec = registry::recommend_optimizer(100, false);
//! println!("Recommended: {}", rec);
//!
//! // Get specific optimizer metadata
//! if let Some(meta) = registry::get_optimizer("MIPROv2") {
//!     println!("{}: {}", meta.name, meta.description);
//! }
//! ```

use serde::{Deserialize, Serialize};

/// Optimizer tier indicating recommended usage level
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum OptimizerTier {
    /// Recommended defaults for most use cases
    Tier1Recommended,
    /// Specialized optimizers for specific scenarios
    Tier2Specialized,
    /// Niche optimizers for advanced use cases
    Tier3Niche,
}

impl OptimizerTier {
    /// Get tier number (1, 2, or 3)
    pub fn number(&self) -> u8 {
        match self {
            OptimizerTier::Tier1Recommended => 1,
            OptimizerTier::Tier2Specialized => 2,
            OptimizerTier::Tier3Niche => 3,
        }
    }

    /// Get tier display name
    pub fn display_name(&self) -> &'static str {
        match self {
            OptimizerTier::Tier1Recommended => "Recommended",
            OptimizerTier::Tier2Specialized => "Specialized",
            OptimizerTier::Tier3Niche => "Niche",
        }
    }
}

impl std::fmt::Display for OptimizerTier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Tier {} ({})", self.number(), self.display_name())
    }
}

/// Metadata about an optimizer
///
/// Note: This struct uses `Serialize` only since it's constructed from static data
/// and not deserialized from external sources.
#[derive(Debug, Clone, Serialize)]
pub struct OptimizerMetadata {
    /// Optimizer name (e.g., "MIPROv2")
    pub name: &'static str,

    /// Short description
    pub description: &'static str,

    /// Recommendation tier
    pub tier: OptimizerTier,

    /// When to use this optimizer
    pub use_when: &'static str,

    /// When NOT to use this optimizer
    pub cannot_use_when: &'static str,

    /// Minimum training examples required
    pub min_examples: usize,

    /// Academic citation (arxiv link or source)
    pub citation: &'static str,

    /// Key benchmark result (if available)
    pub benchmark: Option<&'static str>,

    /// Required capabilities (e.g., "finetunable_model", "embedding_model")
    #[serde(serialize_with = "serialize_requirements")]
    pub requirements: &'static [&'static str],
}

/// Serialize static string slice as a Vec<String>
fn serialize_requirements<S>(
    requirements: &&'static [&'static str],
    serializer: S,
) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    use serde::ser::SerializeSeq;
    let mut seq = serializer.serialize_seq(Some(requirements.len()))?;
    for req in *requirements {
        seq.serialize_element(req)?;
    }
    seq.end()
}

/// Get metadata for all optimizers
pub fn all_optimizers() -> Vec<OptimizerMetadata> {
    vec![
        // Tier 1: Recommended
        OptimizerMetadata {
            name: "MIPROv2",
            description: "Multi-stage instruction and demo optimization with Bayesian search",
            tier: OptimizerTier::Tier1Recommended,
            use_when: "Complex multi-stage programs, 50+ training examples",
            cannot_use_when: "Zero training data",
            min_examples: 2,
            citation: "https://arxiv.org/abs/2406.11695",
            benchmark: Some("5/7 benchmarks, up to 13% accuracy improvement"),
            requirements: &["prompt_model", "task_model"],
        },
        OptimizerMetadata {
            name: "BootstrapFewShot",
            description: "Generate few-shot examples from successful traces",
            tier: OptimizerTier::Tier1Recommended,
            use_when: "Quick prototyping, limited data (~10 examples)",
            cannot_use_when: "Zero examples",
            min_examples: 10,
            citation: "https://arxiv.org/abs/2310.03714",
            benchmark: Some("DSPy foundation, proven across all benchmarks"),
            requirements: &["teacher_model"],
        },
        OptimizerMetadata {
            name: "GRPO",
            description: "Group Relative Policy Optimization for model finetuning",
            tier: OptimizerTier::Tier1Recommended,
            use_when: "Model finetuning available, RL-based optimization needed",
            cannot_use_when: "API-only models (GPT-4, Claude)",
            min_examples: 10,
            citation: "https://arxiv.org/abs/2402.03300",
            benchmark: Some("51.7% -> 60.9% on MATH benchmark"),
            requirements: &["finetunable_model", "reinforce_api"],
        },
        // Tier 2: Specialized
        OptimizerMetadata {
            name: "SIMBA",
            description: "Stochastic Introspective Mini-Batch Ascent with self-reflection",
            tier: OptimizerTier::Tier2Specialized,
            use_when: "Need adaptive, self-improving optimization",
            cannot_use_when: "Very small datasets",
            min_examples: 20,
            citation: "https://github.com/stanfordnlp/dspy",
            benchmark: None,
            requirements: &["metric_function"],
        },
        OptimizerMetadata {
            name: "COPRO",
            description: "Collaborative Prompt Optimizer for instruction refinement",
            tier: OptimizerTier::Tier2Specialized,
            use_when: "Instruction-only optimization (no few-shot needed)",
            cannot_use_when: "Unstructured creative tasks",
            min_examples: 10,
            citation: "https://arxiv.org/abs/2310.03714",
            benchmark: None,
            requirements: &["metric_function"],
        },
        OptimizerMetadata {
            name: "COPROv2",
            description: "COPRO with confidence-based scoring and extended candidate selection",
            tier: OptimizerTier::Tier2Specialized,
            use_when: "COPRO-like optimization with improved candidate scoring",
            cannot_use_when: "Unstructured creative tasks",
            min_examples: 10,
            citation: "https://arxiv.org/abs/2310.03714",
            benchmark: None,
            requirements: &["metric_function"],
        },
        OptimizerMetadata {
            name: "BootstrapFinetune",
            description: "Distill prompt-based program into model weight updates",
            tier: OptimizerTier::Tier2Specialized,
            use_when: "Distilling to smaller/faster model",
            cannot_use_when: "API-only models",
            min_examples: 50,
            citation: "https://arxiv.org/abs/2310.03714",
            benchmark: None,
            requirements: &["finetunable_model"],
        },
        OptimizerMetadata {
            name: "AutoPrompt",
            description: "Gradient-free discrete prompt search with automatic prompt generation",
            tier: OptimizerTier::Tier2Specialized,
            use_when: "Need gradient-free prompt discovery, token-level optimization",
            cannot_use_when: "Tasks requiring nuanced instruction refinement",
            min_examples: 10,
            citation: "https://arxiv.org/abs/2010.15980",
            benchmark: Some("Elicits factual knowledge from LMs without fine-tuning"),
            requirements: &[],
        },
        // Tier 3: Niche
        OptimizerMetadata {
            name: "GEPA",
            description: "Genetic Evolution Prompt Adaptation with LLM reflection",
            tier: OptimizerTier::Tier3Niche,
            use_when: "Need genetic/evolutionary optimization approach",
            cannot_use_when: "Time-constrained scenarios",
            min_examples: 10,
            citation: "https://arxiv.org/abs/2507.19457",
            benchmark: None,
            requirements: &[],
        },
        OptimizerMetadata {
            name: "RandomSearch",
            description: "Random search over demonstrations with BootstrapFewShot",
            tier: OptimizerTier::Tier3Niche,
            use_when: "Simple baseline exploration",
            cannot_use_when: "Need systematic optimization",
            min_examples: 50,
            citation: "https://arxiv.org/abs/2310.03714",
            benchmark: None,
            requirements: &[],
        },
        OptimizerMetadata {
            name: "Ensemble",
            description: "Combine multiple program variants with reduction function",
            tier: OptimizerTier::Tier3Niche,
            use_when: "Have multiple program variants to combine",
            cannot_use_when: "Single program scenario",
            min_examples: 0,
            citation: "Standard ensemble learning",
            benchmark: None,
            requirements: &["multiple_variants"],
        },
        OptimizerMetadata {
            name: "KNNFewShot",
            description: "K-nearest neighbors for example selection using embeddings",
            tier: OptimizerTier::Tier3Niche,
            use_when: "Need embedding-based example selection",
            cannot_use_when: "No embedding model available",
            min_examples: 20,
            citation: "https://arxiv.org/abs/2310.03714",
            benchmark: None,
            requirements: &["embedding_model"],
        },
        OptimizerMetadata {
            name: "LabeledFewShot",
            description: "Direct use of labeled examples without bootstrapping",
            tier: OptimizerTier::Tier3Niche,
            use_when: "Have high-quality labeled examples",
            cannot_use_when: "Need bootstrapped examples",
            min_examples: 5,
            citation: "https://arxiv.org/abs/2310.03714",
            benchmark: None,
            requirements: &["labeled_data"],
        },
        OptimizerMetadata {
            name: "BetterTogether",
            description: "Meta-optimizer combining multiple optimization strategies",
            tier: OptimizerTier::Tier3Niche,
            use_when: "Want to combine multiple optimization approaches",
            cannot_use_when: "Simple optimization needs",
            min_examples: 20,
            citation: "https://github.com/stanfordnlp/dspy",
            benchmark: None,
            requirements: &[],
        },
        OptimizerMetadata {
            name: "BootstrapOptuna",
            description: "Optuna-backed hyperparameter optimization for demonstrations",
            tier: OptimizerTier::Tier3Niche,
            use_when: "Need Bayesian hyperparameter search",
            cannot_use_when: "Simple optimization needs",
            min_examples: 50,
            citation: "DSPy + Optuna (arxiv:1907.10902)",
            benchmark: None,
            requirements: &["optuna"],
        },
        OptimizerMetadata {
            name: "AvatarOptimizer",
            description: "Iterative instruction refinement via positive/negative feedback analysis",
            tier: OptimizerTier::Tier3Niche,
            use_when: "Optimizing agent instructions based on execution feedback patterns",
            cannot_use_when: "No clear success/failure signal available",
            min_examples: 10,
            citation: "https://github.com/stanfordnlp/dspy",
            benchmark: None,
            requirements: &["positive_negative_examples"],
        },
        OptimizerMetadata {
            name: "InferRules",
            description: "Generate human-readable rules from training examples",
            tier: OptimizerTier::Tier3Niche,
            use_when: "Need interpretable, human-readable optimization output",
            cannot_use_when: "Rules would be too brittle for the task",
            min_examples: 20,
            citation: "https://github.com/stanfordnlp/dspy",
            benchmark: None,
            requirements: &[],
        },
    ]
}

/// Get recommended optimizer for a scenario
pub fn recommend_optimizer(num_examples: usize, can_finetune: bool) -> &'static str {
    if can_finetune && num_examples >= 10 {
        "GRPO"
    } else if num_examples >= 50 {
        "MIPROv2"
    } else if num_examples >= 2 {
        "BootstrapFewShot"
    } else {
        "Cannot optimize with fewer than 2 examples"
    }
}

/// Get optimizer by name (case-insensitive)
pub fn get_optimizer(name: &str) -> Option<OptimizerMetadata> {
    all_optimizers()
        .into_iter()
        .find(|o| o.name.eq_ignore_ascii_case(name))
}

/// Get optimizers by tier
pub fn get_by_tier(tier: OptimizerTier) -> Vec<OptimizerMetadata> {
    all_optimizers()
        .into_iter()
        .filter(|o| o.tier == tier)
        .collect()
}

/// Get optimizers that can work with the given number of examples
pub fn get_by_min_examples(num_examples: usize) -> Vec<OptimizerMetadata> {
    all_optimizers()
        .into_iter()
        .filter(|o| o.min_examples <= num_examples)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_all_optimizers_count() {
        let all = all_optimizers();
        assert_eq!(all.len(), 17, "Should have 17 optimizers");
    }

    #[test]
    fn test_tier_counts() {
        let all = all_optimizers();
        let tier1 = all
            .iter()
            .filter(|o| o.tier == OptimizerTier::Tier1Recommended)
            .count();
        let tier2 = all
            .iter()
            .filter(|o| o.tier == OptimizerTier::Tier2Specialized)
            .count();
        let tier3 = all
            .iter()
            .filter(|o| o.tier == OptimizerTier::Tier3Niche)
            .count();

        assert_eq!(tier1, 3, "Tier 1 should have 3 optimizers");
        assert_eq!(tier2, 5, "Tier 2 should have 5 optimizers");
        assert_eq!(tier3, 9, "Tier 3 should have 9 optimizers");
    }

    #[test]
    fn test_get_optimizer() {
        let mipro = get_optimizer("MIPROv2").expect("MIPROv2 should exist");
        assert_eq!(mipro.name, "MIPROv2");
        assert_eq!(mipro.tier, OptimizerTier::Tier1Recommended);

        // Case insensitive
        let mipro_lower = get_optimizer("miprov2").expect("miprov2 should match");
        assert_eq!(mipro_lower.name, "MIPROv2");
    }

    #[test]
    fn test_recommend_optimizer() {
        assert_eq!(recommend_optimizer(100, false), "MIPROv2");
        assert_eq!(recommend_optimizer(100, true), "GRPO");
        assert_eq!(recommend_optimizer(20, false), "BootstrapFewShot");
        assert_eq!(recommend_optimizer(5, false), "BootstrapFewShot");
        assert!(recommend_optimizer(1, false).contains("Cannot"));
    }

    #[test]
    fn test_get_by_tier() {
        let tier1 = get_by_tier(OptimizerTier::Tier1Recommended);
        assert_eq!(tier1.len(), 3);
        assert!(tier1.iter().any(|o| o.name == "MIPROv2"));
    }

    #[test]
    fn test_get_by_min_examples() {
        let available = get_by_min_examples(10);
        assert!(available.iter().any(|o| o.name == "BootstrapFewShot"));
        assert!(!available.iter().any(|o| o.name == "RandomSearch")); // requires 50
    }
}
