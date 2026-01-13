//! @dashflow-module
//! @name optimizer_traits
//! @category optimize
//! @status stable
//!
//! # Common Traits for Optimizer Algorithms
//!
//! This module provides shared traits that define the common interface
//! for all prompt optimization algorithms.
//!
//! ## Traits
//!
//! - [`SignatureOptimizer`] - Core trait for all signature optimizers
//! - [`OptimizerInfo`] - Metadata about an optimizer (tier, requirements, etc.)

use crate::core::language_models::ChatModel;
use crate::optimize::example::Example;
use crate::optimize::signature::Signature;
use crate::Result;
use async_trait::async_trait;
use std::sync::Arc;

/// Optimizer tier indicating recommended usage level
///
/// Tiers help users and automated systems select the right optimizer:
/// - **Tier 1**: Recommended defaults for most use cases
/// - **Tier 2**: Specialized optimizers for specific scenarios
/// - **Tier 3**: Niche optimizers for advanced use cases
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum OptimizerTier {
    /// Recommended defaults for most use cases (MIPROv2, BootstrapFewShot, GRPO)
    Tier1Recommended,
    /// Specialized optimizers for specific scenarios (SIMBA, COPRO, BootstrapFinetune)
    Tier2Specialized,
    /// Niche optimizers for advanced use cases (RandomSearch, GEPA, Ensemble, etc.)
    Tier3Niche,
}

impl OptimizerTier {
    /// Get the tier number (1, 2, or 3)
    pub fn level(&self) -> u8 {
        match self {
            OptimizerTier::Tier1Recommended => 1,
            OptimizerTier::Tier2Specialized => 2,
            OptimizerTier::Tier3Niche => 3,
        }
    }

    /// Get a human-readable description
    pub fn description(&self) -> &'static str {
        match self {
            OptimizerTier::Tier1Recommended => "Recommended for most use cases",
            OptimizerTier::Tier2Specialized => "Specialized for specific scenarios",
            OptimizerTier::Tier3Niche => "Advanced/niche use cases",
        }
    }
}

impl std::fmt::Display for OptimizerTier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Tier {} ({})", self.level(), self.description())
    }
}

/// Information about optimizer requirements and capabilities
pub trait OptimizerInfo {
    /// Get the optimizer name
    fn name(&self) -> &'static str;

    /// Get a short description of what this optimizer does
    fn description(&self) -> &'static str;

    /// Get the optimizer tier (1=recommended, 2=specialized, 3=niche)
    fn tier(&self) -> OptimizerTier;

    /// Get the minimum number of training examples required
    fn min_examples(&self) -> usize {
        2 // Default: most optimizers need at least 2 examples
    }

    /// Get academic/source citation
    fn citation(&self) -> &'static str {
        "DSPy framework"
    }

    /// Check if this optimizer can be used with the given context
    ///
    /// # Arguments
    ///
    /// * `num_examples` - Number of training examples available
    /// * `can_finetune` - Whether the model supports finetuning
    ///
    /// # Returns
    ///
    /// `true` if the optimizer can be used with these parameters.
    fn can_use(&self, num_examples: usize, can_finetune: bool) -> bool {
        let _ = can_finetune; // Most optimizers don't require finetuning
        num_examples >= self.min_examples()
    }

    /// Get a short description of when to use this optimizer
    fn use_when(&self) -> &'static str {
        "General purpose optimization"
    }

    /// Get a short description of when NOT to use this optimizer
    fn cannot_use_when(&self) -> &'static str {
        "Never" // Most optimizers have no absolute restrictions
    }

    /// Get any specific requirements (e.g., "embedding_model", "finetunable_model")
    fn requirements(&self) -> &'static [&'static str] {
        &[]
    }
}

/// Common interface for all signature optimizers
///
/// This trait defines the core compilation interface that all optimizers
/// must implement. It allows optimizers to be used interchangeably.
///
/// # Example
///
/// ```rust,ignore
/// use dashflow::optimize::optimizers::traits::SignatureOptimizer;
///
/// async fn optimize_with_any<O: SignatureOptimizer>(
///     optimizer: &O,
///     signature: &Signature,
///     examples: &[Example],
///     llm: Arc<dyn ChatModel>,
/// ) -> Result<Signature> {
///     if !optimizer.can_use(examples.len(), false) {
///         return Err("Not enough examples".into());
///     }
///     optimizer.compile(signature, examples, llm).await
/// }
/// ```
#[async_trait]
pub trait SignatureOptimizer: OptimizerInfo + Send + Sync {
    /// Optimize a signature using training data
    ///
    /// This is the core method that all optimizers implement.
    /// It takes a signature and training examples, and returns
    /// an optimized version of the signature.
    ///
    /// # Arguments
    ///
    /// * `signature` - The signature to optimize
    /// * `trainset` - Training examples
    /// * `llm` - Language model for optimization
    ///
    /// # Returns
    ///
    /// The optimized signature, or an error if optimization failed.
    async fn compile(
        &self,
        signature: &Signature,
        trainset: &[Example],
        llm: Arc<dyn ChatModel>,
    ) -> Result<Signature>;

    /// Optimize with a validation set for early stopping
    ///
    /// Default implementation ignores the validation set.
    /// Optimizers that support validation should override this.
    async fn compile_with_validation(
        &self,
        signature: &Signature,
        trainset: &[Example],
        valset: &[Example],
        llm: Arc<dyn ChatModel>,
    ) -> Result<Signature> {
        let _ = valset; // Ignore validation set by default
        self.compile(signature, trainset, llm).await
    }
}

/// A type-erased optimizer that can be stored in collections
pub type DynOptimizer = Arc<dyn SignatureOptimizer>;

/// Result of running an optimizer, with metadata
#[derive(Clone, Debug)]
pub struct OptimizationRun {
    /// Name of the optimizer that was used
    pub optimizer_name: String,

    /// Initial score before optimization
    pub initial_score: f64,

    /// Final score after optimization
    pub final_score: f64,

    /// Number of iterations performed
    pub iterations: usize,

    /// Time spent optimizing (seconds)
    pub duration_secs: f64,

    /// Whether optimization converged
    pub converged: bool,
}

impl OptimizationRun {
    /// Calculate improvement as a ratio
    pub fn improvement_ratio(&self) -> f64 {
        if self.initial_score == 0.0 {
            0.0
        } else {
            (self.final_score - self.initial_score) / self.initial_score
        }
    }

    /// Calculate improvement as a percentage
    pub fn improvement_percent(&self) -> f64 {
        self.improvement_ratio() * 100.0
    }

    /// Check if optimization improved the score
    pub fn improved(&self) -> bool {
        self.final_score > self.initial_score
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_optimizer_tier_level() {
        assert_eq!(OptimizerTier::Tier1Recommended.level(), 1);
        assert_eq!(OptimizerTier::Tier2Specialized.level(), 2);
        assert_eq!(OptimizerTier::Tier3Niche.level(), 3);
    }

    #[test]
    fn test_optimizer_tier_display() {
        let tier = OptimizerTier::Tier1Recommended;
        let s = tier.to_string();
        assert!(s.contains("Tier 1"));
        assert!(s.contains("Recommended"));
    }

    #[test]
    fn test_optimization_run_improvement() {
        let run = OptimizationRun {
            optimizer_name: "MIPROv2".to_string(),
            initial_score: 0.5,
            final_score: 0.65,
            iterations: 10,
            duration_secs: 30.0,
            converged: true,
        };

        assert!(run.improved());
        assert!((run.improvement_ratio() - 0.3).abs() < 0.0001);
        assert!((run.improvement_percent() - 30.0).abs() < 0.01);
    }

    #[test]
    fn test_optimization_run_no_improvement() {
        let run = OptimizationRun {
            optimizer_name: "Test".to_string(),
            initial_score: 0.5,
            final_score: 0.4,
            iterations: 5,
            duration_secs: 10.0,
            converged: false,
        };

        assert!(!run.improved());
        assert!(run.improvement_ratio() < 0.0);
    }
}
