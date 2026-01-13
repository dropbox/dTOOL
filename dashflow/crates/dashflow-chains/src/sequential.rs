//! Sequential Chain - Execute chains in sequence, feeding outputs to next chain
//!
//! This module provides chains for composing multiple processing steps where
//! the output of one step feeds into the input of the next.
//!
//! # Chain Types
//!
//! - [`SequentialChain`]: General-purpose sequential execution with named inputs/outputs
//! - [`SimpleSequentialChain`]: Simplified version with single input/output per chain
//!
//! # Example
//!
//! ```rust,ignore
//! use dashflow_chains::SequentialChain;
//! use std::collections::HashMap;
//!
//! // Create a sequential chain with multiple steps
//! let chain = SequentialChain::builder()
//!     .input_variables(vec!["topic"])
//!     .add_step(
//!         vec!["topic"],
//!         vec!["outline"],
//!         |inputs| {
//!             let topic = inputs.get("topic").unwrap();
//!             let mut result = HashMap::new();
//!             result.insert("outline".to_string(), format!("Outline for {}", topic));
//!             Ok(result)
//!         }
//!     )
//!     .add_step(
//!         vec!["outline"],
//!         vec!["essay"],
//!         |inputs| {
//!             let outline = inputs.get("outline").unwrap();
//!             let mut result = HashMap::new();
//!             result.insert("essay".to_string(), format!("Essay based on: {}", outline));
//!             Ok(result)
//!         }
//!     )
//!     .build();
//!
//! let result = chain.run(&maplit::hashmap! {
//!     "topic".to_string() => "Rust programming".to_string()
//! }).await?;
//! ```

use dashflow::core::error::{Error, Result};
use std::collections::HashMap;
use std::sync::Arc;

/// Type for chain step functions
pub type ChainStepFn =
    Arc<dyn Fn(&HashMap<String, String>) -> Result<HashMap<String, String>> + Send + Sync>;

/// A step in a sequential chain
#[derive(Clone)]
pub struct ChainStep {
    input_variables: Vec<String>,
    output_variables: Vec<String>,
    function: ChainStepFn,
}

impl ChainStep {
    /// Create a new chain step
    pub fn new(
        input_variables: Vec<String>,
        output_variables: Vec<String>,
        function: ChainStepFn,
    ) -> Self {
        Self {
            input_variables,
            output_variables,
            function,
        }
    }

    /// Get input variable names
    #[must_use]
    pub fn input_variables(&self) -> &[String] {
        &self.input_variables
    }

    /// Get output variable names
    #[must_use]
    pub fn output_variables(&self) -> &[String] {
        &self.output_variables
    }

    /// Execute this step
    pub fn execute(&self, inputs: &HashMap<String, String>) -> Result<HashMap<String, String>> {
        // Validate all required inputs are present
        for var in &self.input_variables {
            if !inputs.contains_key(var) {
                return Err(Error::InvalidInput(format!(
                    "Missing required input variable: {var}"
                )));
            }
        }

        (self.function)(inputs)
    }
}

/// Chain where outputs of one step feed directly into the next.
///
/// This chain executes a sequence of processing steps, where each step can have
/// multiple named inputs and outputs. The accumulated outputs from all previous
/// steps are available to each subsequent step.
///
/// # Validation
///
/// The chain validates at build time that:
/// - Each step's required inputs are available from either initial inputs or previous outputs
/// - No step produces an output key that already exists
/// - All requested output variables are produced by some step
///
/// # Example
///
/// ```rust,ignore
/// use dashflow_chains::SequentialChain;
///
/// let chain = SequentialChain::builder()
///     .input_variables(vec!["topic"])
///     .add_step(
///         vec!["topic"],
///         vec!["questions"],
///         |inputs| {
///             // Generate questions from topic
///             Ok(generated_questions)
///         }
///     )
///     .add_step(
///         vec!["topic", "questions"],
///         vec!["answers"],
///         |inputs| {
///             // Generate answers from topic and questions
///             Ok(generated_answers)
///         }
///     )
///     .output_variables(vec!["questions", "answers"])
///     .build();
/// ```
pub struct SequentialChain {
    steps: Vec<ChainStep>,
    input_variables: Vec<String>,
    output_variables: Vec<String>,
    return_all: bool,
}

impl std::fmt::Debug for SequentialChain {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SequentialChain")
            .field("input_variables", &self.input_variables)
            .field("output_variables", &self.output_variables)
            .field("return_all", &self.return_all)
            .field("steps_count", &self.steps.len())
            .finish()
    }
}

impl SequentialChain {
    /// Create a new builder for constructing a sequential chain
    #[must_use]
    pub fn builder() -> SequentialChainBuilder {
        SequentialChainBuilder::new()
    }

    /// Get input variable names
    #[must_use]
    pub fn input_variables(&self) -> &[String] {
        &self.input_variables
    }

    /// Get output variable names
    #[must_use]
    pub fn output_variables(&self) -> &[String] {
        &self.output_variables
    }

    /// Run the sequential chain with the given inputs.
    ///
    /// Each step is executed in order, accumulating outputs. The final result
    /// contains only the requested output variables (or all variables if `return_all` is true).
    ///
    /// # Arguments
    ///
    /// * `inputs` - Initial input values
    ///
    /// # Returns
    ///
    /// Output values for the requested output variables
    pub async fn run(&self, inputs: &HashMap<String, String>) -> Result<HashMap<String, String>> {
        // Validate initial inputs
        for var in &self.input_variables {
            if !inputs.contains_key(var) {
                return Err(Error::InvalidInput(format!(
                    "Missing required input variable: {var}"
                )));
            }
        }

        // Accumulate all outputs as we go
        let mut known_values = inputs.clone();

        // Execute each step in sequence
        for step in &self.steps {
            let step_outputs = step.execute(&known_values)?;
            known_values.extend(step_outputs);
        }

        // Return only requested outputs (or all if return_all is true)
        if self.return_all {
            // Remove initial input variables if return_all is true (Python behavior)
            let mut result = HashMap::new();
            for (key, value) in known_values {
                if !self.input_variables.contains(&key) {
                    result.insert(key, value);
                }
            }
            Ok(result)
        } else {
            let mut result = HashMap::new();
            for var in &self.output_variables {
                if let Some(value) = known_values.get(var) {
                    result.insert(var.clone(), value.clone());
                }
            }
            Ok(result)
        }
    }
}

/// Builder for creating a `SequentialChain` with validation
pub struct SequentialChainBuilder {
    steps: Vec<ChainStep>,
    input_variables: Option<Vec<String>>,
    output_variables: Option<Vec<String>>,
    return_all: bool,
}

impl SequentialChainBuilder {
    /// Create a new builder
    #[must_use]
    pub fn new() -> Self {
        Self {
            steps: Vec::new(),
            input_variables: None,
            output_variables: None,
            return_all: false,
        }
    }

    /// Set the initial input variable names for the chain
    pub fn input_variables<I, S>(mut self, vars: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.input_variables = Some(vars.into_iter().map(std::convert::Into::into).collect());
        self
    }

    /// Set the output variable names that should be returned
    pub fn output_variables<I, S>(mut self, vars: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.output_variables = Some(vars.into_iter().map(std::convert::Into::into).collect());
        self
    }

    /// Set whether to return all intermediate outputs (default: false)
    #[must_use]
    pub fn return_all(mut self, return_all: bool) -> Self {
        self.return_all = return_all;
        self
    }

    /// Add a step to the chain
    pub fn add_step<I, O, F>(mut self, inputs: I, outputs: O, function: F) -> Self
    where
        I: IntoIterator<Item = String>,
        O: IntoIterator<Item = String>,
        F: Fn(&HashMap<String, String>) -> Result<HashMap<String, String>> + Send + Sync + 'static,
    {
        let step = ChainStep::new(
            inputs.into_iter().collect(),
            outputs.into_iter().collect(),
            Arc::new(function),
        );
        self.steps.push(step);
        self
    }

    /// Build the sequential chain, validating the configuration
    pub fn build(self) -> Result<SequentialChain> {
        let input_variables = self
            .input_variables
            .ok_or_else(|| Error::InvalidInput("Input variables must be specified".to_string()))?;

        if self.steps.is_empty() {
            return Err(Error::InvalidInput(
                "Sequential chain must have at least one step".to_string(),
            ));
        }

        // Validate chain connectivity
        let mut known_variables: std::collections::HashSet<String> =
            input_variables.iter().cloned().collect();

        for (i, step) in self.steps.iter().enumerate() {
            // Check that all inputs for this step are available
            let missing_vars: Vec<_> = step
                .input_variables()
                .iter()
                .filter(|var| !known_variables.contains(*var))
                .collect();

            if !missing_vars.is_empty() {
                return Err(Error::InvalidInput(format!(
                    "Step {} missing required input keys: {:?}, only had {:?}",
                    i,
                    missing_vars,
                    known_variables.iter().collect::<Vec<_>>()
                )));
            }

            // Check for duplicate output keys
            let overlapping: Vec<_> = step
                .output_variables()
                .iter()
                .filter(|var| known_variables.contains(*var))
                .collect();

            if !overlapping.is_empty() {
                return Err(Error::InvalidInput(format!(
                    "Step {i} returned keys that already exist: {overlapping:?}"
                )));
            }

            // Add this step's outputs to known variables
            known_variables.extend(step.output_variables().iter().cloned());
        }

        // Determine output variables
        let output_variables = if let Some(vars) = self.output_variables {
            // Validate that all requested outputs are produced
            let missing: Vec<_> = vars
                .iter()
                .filter(|var| !known_variables.contains(*var))
                .collect();

            if !missing.is_empty() {
                return Err(Error::InvalidInput(format!(
                    "Expected output variables that were not found: {missing:?}"
                )));
            }

            vars
        } else if self.return_all {
            // Return all variables except inputs
            known_variables
                .into_iter()
                .filter(|var| !input_variables.contains(var))
                .collect()
        } else {
            // Default to last step's outputs
            // SAFETY: empty check performed above on line 302
            #[allow(clippy::expect_used)]
            let last_step = self.steps.last().expect("steps validated non-empty above");
            last_step.output_variables().to_vec()
        };

        Ok(SequentialChain {
            steps: self.steps,
            input_variables,
            output_variables,
            return_all: self.return_all,
        })
    }
}

impl Default for SequentialChainBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Simple chain where outputs of one step feed directly into the next.
///
/// This is a simplified version of [`SequentialChain`] where each step has
/// exactly one input and one output. The output of each step becomes the
/// input of the next step.
///
/// # Example
///
/// ```rust,ignore
/// use dashflow_chains::SimpleSequentialChain;
///
/// let chain = SimpleSequentialChain::builder()
///     .add_step(|input| {
///         // First step: generate outline
///         Ok(format!("Outline: {}", input))
///     })
///     .add_step(|input| {
///         // Second step: write essay from outline
///         Ok(format!("Essay based on {}", input))
///     })
///     .build();
///
/// let result = chain.run("Rust programming").await?;
/// ```
pub struct SimpleSequentialChain {
    #[allow(clippy::type_complexity)] // Type-erased function chain requires full trait bounds for Send+Sync safety
    steps: Vec<Arc<dyn Fn(&str) -> Result<String> + Send + Sync>>,
    strip_outputs: bool,
}

impl std::fmt::Debug for SimpleSequentialChain {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SimpleSequentialChain")
            .field("steps_count", &self.steps.len())
            .field("strip_outputs", &self.strip_outputs)
            .finish()
    }
}

impl SimpleSequentialChain {
    /// Create a new builder for constructing a simple sequential chain
    #[must_use]
    pub fn builder() -> SimpleSequentialChainBuilder {
        SimpleSequentialChainBuilder::new()
    }

    /// Run the chain with the given input.
    ///
    /// Each step is executed in order, with the output of each step
    /// becoming the input to the next step.
    ///
    /// # Arguments
    ///
    /// * `input` - Initial input string
    ///
    /// # Returns
    ///
    /// Final output string from the last step
    pub async fn run(&self, input: &str) -> Result<String> {
        let mut current = input.to_string();

        for step in &self.steps {
            current = step(&current)?;
            if self.strip_outputs {
                current = current.trim().to_string();
            }
        }

        Ok(current)
    }
}

/// Builder for creating a `SimpleSequentialChain`
pub struct SimpleSequentialChainBuilder {
    #[allow(clippy::type_complexity)] // Mirrors SimpleSequentialChain type for builder pattern
    steps: Vec<Arc<dyn Fn(&str) -> Result<String> + Send + Sync>>,
    strip_outputs: bool,
}

impl SimpleSequentialChainBuilder {
    /// Create a new builder
    #[must_use]
    pub fn new() -> Self {
        Self {
            steps: Vec::new(),
            strip_outputs: false,
        }
    }

    /// Set whether to strip whitespace from outputs before passing to next step
    #[must_use]
    pub fn strip_outputs(mut self, strip: bool) -> Self {
        self.strip_outputs = strip;
        self
    }

    /// Add a step to the chain
    pub fn add_step<F>(mut self, function: F) -> Self
    where
        F: Fn(&str) -> Result<String> + Send + Sync + 'static,
    {
        self.steps.push(Arc::new(function));
        self
    }

    /// Build the simple sequential chain
    pub fn build(self) -> Result<SimpleSequentialChain> {
        if self.steps.is_empty() {
            return Err(Error::InvalidInput(
                "SimpleSequentialChain must have at least one step".to_string(),
            ));
        }

        Ok(SimpleSequentialChain {
            steps: self.steps,
            strip_outputs: self.strip_outputs,
        })
    }
}

impl Default for SimpleSequentialChainBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_sequential_chain_basic() {
        let chain = SequentialChain::builder()
            .input_variables(vec!["topic".to_string()])
            .add_step(
                vec!["topic".to_string()],
                vec!["outline".to_string()],
                |inputs| {
                    let topic = inputs.get("topic").unwrap();
                    let mut result = HashMap::new();
                    result.insert("outline".to_string(), format!("Outline for: {}", topic));
                    Ok(result)
                },
            )
            .add_step(
                vec!["outline".to_string()],
                vec!["essay".to_string()],
                |inputs| {
                    let outline = inputs.get("outline").unwrap();
                    let mut result = HashMap::new();
                    result.insert("essay".to_string(), format!("Essay: {}", outline));
                    Ok(result)
                },
            )
            .build()
            .unwrap();

        let mut inputs = HashMap::new();
        inputs.insert("topic".to_string(), "Rust".to_string());

        let result = chain.run(&inputs).await.unwrap();
        assert_eq!(result.len(), 1);
        assert!(result.get("essay").unwrap().contains("Rust"));
        assert!(!result.contains_key("outline")); // Only last step output by default
    }

    #[tokio::test]
    async fn test_sequential_chain_return_all() {
        let chain = SequentialChain::builder()
            .input_variables(vec!["topic".to_string()])
            .add_step(
                vec!["topic".to_string()],
                vec!["outline".to_string()],
                |inputs| {
                    let topic = inputs.get("topic").unwrap();
                    let mut result = HashMap::new();
                    result.insert("outline".to_string(), format!("Outline: {}", topic));
                    Ok(result)
                },
            )
            .add_step(
                vec!["outline".to_string()],
                vec!["essay".to_string()],
                |inputs| {
                    let outline = inputs.get("outline").unwrap();
                    let mut result = HashMap::new();
                    result.insert("essay".to_string(), format!("Essay: {}", outline));
                    Ok(result)
                },
            )
            .return_all(true)
            .build()
            .unwrap();

        let mut inputs = HashMap::new();
        inputs.insert("topic".to_string(), "Rust".to_string());

        let result = chain.run(&inputs).await.unwrap();
        assert_eq!(result.len(), 2);
        assert!(result.contains_key("outline"));
        assert!(result.contains_key("essay"));
        assert!(!result.contains_key("topic")); // Input not included in return_all
    }

    #[tokio::test]
    async fn test_sequential_chain_custom_outputs() {
        let chain = SequentialChain::builder()
            .input_variables(vec!["topic".to_string()])
            .add_step(
                vec!["topic".to_string()],
                vec!["outline".to_string()],
                |inputs| {
                    let topic = inputs.get("topic").unwrap();
                    let mut result = HashMap::new();
                    result.insert("outline".to_string(), format!("Outline: {}", topic));
                    Ok(result)
                },
            )
            .add_step(
                vec!["outline".to_string()],
                vec!["essay".to_string()],
                |inputs| {
                    let outline = inputs.get("outline").unwrap();
                    let mut result = HashMap::new();
                    result.insert("essay".to_string(), format!("Essay: {}", outline));
                    Ok(result)
                },
            )
            .output_variables(vec!["outline".to_string()])
            .build()
            .unwrap();

        let mut inputs = HashMap::new();
        inputs.insert("topic".to_string(), "Rust".to_string());

        let result = chain.run(&inputs).await.unwrap();
        assert_eq!(result.len(), 1);
        assert!(result.contains_key("outline"));
        assert!(!result.contains_key("essay"));
    }

    #[tokio::test]
    async fn test_sequential_chain_missing_input() {
        let chain = SequentialChain::builder()
            .input_variables(vec!["topic".to_string()])
            .add_step(
                vec!["missing".to_string()],
                vec!["output".to_string()],
                |_| Ok(HashMap::new()),
            )
            .build();

        assert!(chain.is_err());
        assert!(chain
            .unwrap_err()
            .to_string()
            .contains("missing required input"));
    }

    #[tokio::test]
    async fn test_sequential_chain_duplicate_output() {
        let chain = SequentialChain::builder()
            .input_variables(vec!["topic".to_string()])
            .add_step(
                vec!["topic".to_string()],
                vec!["result".to_string()],
                |_| Ok(HashMap::new()),
            )
            .add_step(
                vec!["result".to_string()],
                vec!["result".to_string()],
                |_| Ok(HashMap::new()),
            )
            .build();

        assert!(chain.is_err());
        assert!(chain
            .unwrap_err()
            .to_string()
            .contains("keys that already exist"));
    }

    #[tokio::test]
    async fn test_simple_sequential_chain() {
        let chain = SimpleSequentialChain::builder()
            .add_step(|input| Ok(format!("Step 1: {}", input)))
            .add_step(|input| Ok(format!("Step 2: {}", input)))
            .add_step(|input| Ok(format!("Step 3: {}", input)))
            .build()
            .unwrap();

        let result = chain.run("Hello").await.unwrap();
        assert_eq!(result, "Step 3: Step 2: Step 1: Hello");
    }

    #[tokio::test]
    async fn test_simple_sequential_chain_strip() {
        let chain = SimpleSequentialChain::builder()
            .add_step(|input| Ok(format!("  {}  ", input)))
            .add_step(|input| Ok(format!("  {}  ", input)))
            .strip_outputs(true)
            .build()
            .unwrap();

        let result = chain.run("Hello").await.unwrap();
        assert_eq!(result, "Hello");
    }

    #[tokio::test]
    async fn test_simple_sequential_chain_empty() {
        let chain = SimpleSequentialChain::builder().build();
        assert!(chain.is_err());
    }

    // Property-based tests
    mod property_tests {
        use super::*;
        use proptest::prelude::*;

        // Generate valid variable names
        fn var_name() -> impl Strategy<Value = String> {
            "[a-z][a-z0-9_]{0,10}"
        }

        // Generate valid values
        fn var_value() -> impl Strategy<Value = String> {
            "[^\\n]{0,50}"
        }

        proptest! {
            /// Property: Chain composition preserves data flow
            /// For any sequence of steps A → B → C, the output of A flows to B, and B to C
            #[test]
            fn prop_chain_preserves_data_flow(
                val1 in var_value(),
                val2 in var_value(),
            ) {
                let rt = tokio::runtime::Runtime::new().unwrap();
                rt.block_on(async {
                    let val1_clone1 = val1.clone();
                    let val1_clone2 = val1.clone();
                    let val2_clone1 = val2.clone();
                    let val2_clone2 = val2.clone();

                    // Create chain: input → step1 → step2 → output
                    let chain = SequentialChain::builder()
                        .input_variables(vec!["input".to_string()])
                        .add_step(
                            vec!["input".to_string()],
                            vec!["intermediate".to_string()],
                            move |inputs| {
                                let mut result = HashMap::new();
                                result.insert(
                                    "intermediate".to_string(),
                                    format!("{}-{}", inputs.get("input").unwrap(), val1_clone1)
                                );
                                Ok(result)
                            },
                        )
                        .add_step(
                            vec!["intermediate".to_string()],
                            vec!["output".to_string()],
                            move |inputs| {
                                let mut result = HashMap::new();
                                result.insert(
                                    "output".to_string(),
                                    format!("{}-{}", inputs.get("intermediate").unwrap(), val2_clone1)
                                );
                                Ok(result)
                            },
                        )
                        .build()
                        .unwrap();

                    let mut inputs = HashMap::new();
                    inputs.insert("input".to_string(), "start".to_string());

                    let result = chain.run(&inputs).await.unwrap();

                    // Output should contain data from all steps in order
                    let output = result.get("output").unwrap();
                    prop_assert!(output.contains("start"), "Output should contain initial input");
                    if !val1_clone2.is_empty() {
                        prop_assert!(output.contains(&val1_clone2), "Output should contain step1 value");
                    }
                    if !val2_clone2.is_empty() {
                        prop_assert!(output.contains(&val2_clone2), "Output should contain step2 value");
                    }

                    Ok(())
                }).unwrap();
            }

            /// Property: Input/output variable mapping is consistent
            /// Variables declared as outputs in one step are available as inputs to next step
            #[test]
            fn prop_variable_mapping_consistent(
                var1 in var_name(),
                var2 in var_name(),
                val in var_value(),
            ) {
                // Ensure var1 != var2
                prop_assume!(var1 != var2);

                let rt = tokio::runtime::Runtime::new().unwrap();
                rt.block_on(async {
                    let var1_clone = var1.clone();
                    let var2_clone = var2.clone();

                    let chain = SequentialChain::builder()
                        .input_variables(vec![var1.clone()])
                        .add_step(
                            vec![var1.clone()],
                            vec![var2.clone()],
                            move |inputs| {
                                let input_val = inputs.get(&var1_clone).unwrap();
                                let mut result = HashMap::new();
                                result.insert(var2_clone.clone(), format!("processed-{}", input_val));
                                Ok(result)
                            },
                        )
                        .build()
                        .unwrap();

                    let mut inputs = HashMap::new();
                    inputs.insert(var1.clone(), val.clone());

                    let result = chain.run(&inputs).await.unwrap();

                    // Output should be keyed by var2 (not var1)
                    prop_assert!(result.contains_key(&var2), "Output should contain var2 key");
                    if !val.is_empty() {
                        prop_assert!(result.get(&var2).unwrap().contains(&val), "Output value should contain input");
                    }

                    Ok(())
                }).unwrap();
            }

            /// Property: Intermediate values accessible when return_all is true
            #[test]
            fn prop_intermediate_values_accessible(
                val in var_value(),
            ) {
                let rt = tokio::runtime::Runtime::new().unwrap();
                rt.block_on(async {
                    let chain = SequentialChain::builder()
                        .input_variables(vec!["input".to_string()])
                        .add_step(
                            vec!["input".to_string()],
                            vec!["step1".to_string()],
                            |inputs| {
                                let mut result = HashMap::new();
                                result.insert(
                                    "step1".to_string(),
                                    format!("s1-{}", inputs.get("input").unwrap())
                                );
                                Ok(result)
                            },
                        )
                        .add_step(
                            vec!["step1".to_string()],
                            vec!["step2".to_string()],
                            |inputs| {
                                let mut result = HashMap::new();
                                result.insert(
                                    "step2".to_string(),
                                    format!("s2-{}", inputs.get("step1").unwrap())
                                );
                                Ok(result)
                            },
                        )
                        .return_all(true)
                        .build()
                        .unwrap();

                    let mut inputs = HashMap::new();
                    inputs.insert("input".to_string(), val.clone());

                    let result = chain.run(&inputs).await.unwrap();

                    // With return_all, both intermediate and final outputs should be present
                    prop_assert!(result.contains_key("step1"), "Should contain step1 output");
                    prop_assert!(result.contains_key("step2"), "Should contain step2 output");
                    prop_assert_eq!(result.len(), 2, "Should have exactly 2 outputs");

                    Ok(())
                }).unwrap();
            }

            /// Property: Custom output_variables filters results correctly
            #[test]
            fn prop_output_variables_filters(
                val in var_value(),
            ) {
                let rt = tokio::runtime::Runtime::new().unwrap();
                rt.block_on(async {
                    let chain = SequentialChain::builder()
                        .input_variables(vec!["input".to_string()])
                        .add_step(
                            vec!["input".to_string()],
                            vec!["step1".to_string()],
                            |inputs| {
                                let mut result = HashMap::new();
                                result.insert("step1".to_string(), format!("s1-{}", inputs.get("input").unwrap()));
                                Ok(result)
                            },
                        )
                        .add_step(
                            vec!["step1".to_string()],
                            vec!["step2".to_string()],
                            |inputs| {
                                let mut result = HashMap::new();
                                result.insert("step2".to_string(), format!("s2-{}", inputs.get("step1").unwrap()));
                                Ok(result)
                            },
                        )
                        .output_variables(vec!["step1".to_string()])
                        .build()
                        .unwrap();

                    let mut inputs = HashMap::new();
                    inputs.insert("input".to_string(), val);

                    let result = chain.run(&inputs).await.unwrap();

                    // Only step1 should be in output (step2 filtered out)
                    prop_assert!(result.contains_key("step1"), "Should contain step1 (specified in output_variables)");
                    prop_assert!(!result.contains_key("step2"), "Should NOT contain step2 (not in output_variables)");
                    prop_assert_eq!(result.len(), 1, "Should have exactly 1 output");

                    Ok(())
                }).unwrap();
            }

            /// Property: SimpleSequentialChain applies steps in order
            #[test]
            fn prop_simple_chain_order(
                val in "[a-zA-Z]{1,10}",
            ) {
                let rt = tokio::runtime::Runtime::new().unwrap();
                rt.block_on(async {
                    let chain = SimpleSequentialChain::builder()
                        .add_step(|input| Ok(format!("A-{}", input)))
                        .add_step(|input| Ok(format!("B-{}", input)))
                        .add_step(|input| Ok(format!("C-{}", input)))
                        .build()
                        .unwrap();

                    let result = chain.run(&val).await.unwrap();

                    // Result should show steps applied in order: C(B(A(input)))
                    prop_assert!(result.starts_with("C-B-A-"), "Should start with C-B-A- prefix");
                    prop_assert!(result.ends_with(&val), "Should end with original input value");
                    prop_assert_eq!(result, format!("C-B-A-{}", val), "Exact order should be C-B-A-input");

                    Ok(())
                }).unwrap();
            }

            /// Property: SimpleSequentialChain strip_outputs removes whitespace
            #[test]
            fn prop_simple_chain_strip(
                val in "[a-zA-Z]{1,10}",
                spaces_before in 0..5usize,
                spaces_after in 0..5usize,
            ) {
                let rt = tokio::runtime::Runtime::new().unwrap();
                rt.block_on(async {
                    let chain = SimpleSequentialChain::builder()
                        .add_step(move |input| Ok(format!("{}{}{}", " ".repeat(spaces_before), input, " ".repeat(spaces_after))))
                        .strip_outputs(true)
                        .build()
                        .unwrap();

                    let result = chain.run(&val).await.unwrap();

                    // With strip_outputs, result should equal input (whitespace removed)
                    prop_assert_eq!(&result, &val, "Stripped result should equal input");
                    prop_assert!(!result.starts_with(' '), "Should not start with space");
                    prop_assert!(!result.ends_with(' '), "Should not end with space");

                    Ok(())
                }).unwrap();
            }
        }
    }
}
