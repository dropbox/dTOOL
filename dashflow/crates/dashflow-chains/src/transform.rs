//! Transform Chain - Simple text transformation without LLM
//!
//! This chain applies a transformation function to input text. It's useful for
//! preprocessing, postprocessing, or any deterministic text operation that doesn't
//! require a language model.
//!
//! # Example
//!
//! ```rust,ignore
//! use dashflow_chains::TransformChain;
//!
//! // Create a chain that converts text to uppercase
//! let chain = TransformChain::new(
//!     vec!["text"],
//!     vec!["output"],
//!     |inputs| {
//!         let text = inputs.get("text").unwrap();
//!         let mut result = HashMap::new();
//!         result.insert("output".to_string(), text.to_uppercase());
//!         Ok(result)
//!     }
//! );
//!
//! let result = chain.transform(&maplit::hashmap! {
//!     "text" => "hello world"
//! }).await?;
//! ```

use dashflow::core::error::Result;
use std::collections::HashMap;

/// Type for transform functions
pub type TransformFn =
    Box<dyn Fn(&HashMap<String, String>) -> Result<HashMap<String, String>> + Send + Sync>;

/// Chain that applies a transformation function to inputs.
///
/// This is a simple, synchronous chain that doesn't involve any LLM calls.
/// It's useful for:
/// - Text preprocessing (normalization, cleaning)
/// - Text postprocessing (formatting, extraction)
/// - Deterministic transformations (case conversion, regex operations)
/// - Combining multiple inputs into one
///
/// # Type Safety
///
/// The chain validates that all required input keys are present before
/// executing the transform function.
pub struct TransformChain {
    input_variables: Vec<String>,
    output_variables: Vec<String>,
    transform: TransformFn,
}

impl TransformChain {
    /// Create a new transform chain.
    ///
    /// # Arguments
    ///
    /// * `input_variables` - Names of required input keys
    /// * `output_variables` - Names of output keys the transform will produce
    /// * `transform` - Function that transforms inputs to outputs
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let chain = TransformChain::new(
    ///     vec!["text".to_string()],
    ///     vec!["word_count".to_string()],
    ///     Box::new(|inputs| {
    ///         let text = inputs.get("text").unwrap();
    ///         let count = text.split_whitespace().count();
    ///         let mut result = HashMap::new();
    ///         result.insert("word_count".to_string(), count.to_string());
    ///         Ok(result)
    ///     })
    /// );
    /// ```
    #[must_use]
    pub fn new(
        input_variables: Vec<String>,
        output_variables: Vec<String>,
        transform: TransformFn,
    ) -> Self {
        Self {
            input_variables,
            output_variables,
            transform,
        }
    }

    /// Create a simple transform chain that applies a function to a single input.
    ///
    /// # Arguments
    ///
    /// * `input_key` - Name of input key
    /// * `output_key` - Name of output key
    /// * `func` - Function that transforms input string to output string
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let chain = TransformChain::simple(
    ///     "text",
    ///     "upper",
    ///     |text| Ok(text.to_uppercase())
    /// );
    /// ```
    pub fn simple<F>(input_key: &str, output_key: &str, func: F) -> Self
    where
        F: Fn(&str) -> Result<String> + Send + Sync + 'static,
    {
        let input_key_owned = input_key.to_string();
        let output_key_owned = output_key.to_string();

        Self::new(
            vec![input_key.to_string()],
            vec![output_key.to_string()],
            Box::new(move |inputs| {
                let input = inputs.get(&input_key_owned).ok_or_else(|| {
                    dashflow::core::error::Error::InvalidInput(format!(
                        "Missing required input: {input_key_owned}"
                    ))
                })?;

                let output = func(input)?;
                let mut result = HashMap::new();
                result.insert(output_key_owned.clone(), output);
                Ok(result)
            }),
        )
    }

    /// Get the input variable names.
    #[must_use]
    pub fn input_variables(&self) -> &[String] {
        &self.input_variables
    }

    /// Get the output variable names.
    #[must_use]
    pub fn output_variables(&self) -> &[String] {
        &self.output_variables
    }

    /// Transform the inputs using the transformation function.
    ///
    /// # Arguments
    ///
    /// * `inputs` - Input values keyed by variable name
    ///
    /// # Returns
    ///
    /// Output values keyed by variable name
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Any required input variable is missing
    /// - The transform function returns an error
    pub fn transform(&self, inputs: &HashMap<String, String>) -> Result<HashMap<String, String>> {
        // Validate all required inputs are present
        for var in &self.input_variables {
            if !inputs.contains_key(var) {
                return Err(dashflow::core::error::Error::InvalidInput(format!(
                    "Missing required input variable: {var}"
                )));
            }
        }

        // Apply the transformation
        (self.transform)(inputs)
    }

    /// Transform and merge outputs with original inputs.
    ///
    /// This is useful when you want to preserve the original inputs
    /// along with the transformation outputs.
    ///
    /// # Arguments
    ///
    /// * `inputs` - Input values
    ///
    /// # Returns
    ///
    /// Combined inputs and outputs
    pub fn transform_with_inputs(
        &self,
        inputs: &HashMap<String, String>,
    ) -> Result<HashMap<String, String>> {
        let mut result = inputs.clone();
        let outputs = self.transform(inputs)?;
        result.extend(outputs);
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_transform_chain_basic() {
        let chain = TransformChain::new(
            vec!["text".to_string()],
            vec!["upper".to_string()],
            Box::new(|inputs| {
                let text = inputs.get("text").unwrap();
                let mut result = HashMap::new();
                result.insert("upper".to_string(), text.to_uppercase());
                Ok(result)
            }),
        );

        let mut inputs = HashMap::new();
        inputs.insert("text".to_string(), "hello world".to_string());

        let result = chain.transform(&inputs).unwrap();
        assert_eq!(result.get("upper").unwrap(), "HELLO WORLD");
    }

    #[test]
    fn test_transform_chain_simple() {
        let chain = TransformChain::simple("text", "upper", |text| Ok(text.to_uppercase()));

        let mut inputs = HashMap::new();
        inputs.insert("text".to_string(), "hello rust".to_string());

        let result = chain.transform(&inputs).unwrap();
        assert_eq!(result.get("upper").unwrap(), "HELLO RUST");
    }

    #[test]
    fn test_transform_chain_word_count() {
        let chain = TransformChain::simple("text", "count", |text| {
            Ok(text.split_whitespace().count().to_string())
        });

        let mut inputs = HashMap::new();
        inputs.insert("text".to_string(), "one two three four".to_string());

        let result = chain.transform(&inputs).unwrap();
        assert_eq!(result.get("count").unwrap(), "4");
    }

    #[test]
    fn test_transform_chain_multiple_inputs() {
        let chain = TransformChain::new(
            vec!["first".to_string(), "last".to_string()],
            vec!["full_name".to_string()],
            Box::new(|inputs| {
                let first = inputs.get("first").unwrap();
                let last = inputs.get("last").unwrap();
                let mut result = HashMap::new();
                result.insert("full_name".to_string(), format!("{} {}", first, last));
                Ok(result)
            }),
        );

        let mut inputs = HashMap::new();
        inputs.insert("first".to_string(), "John".to_string());
        inputs.insert("last".to_string(), "Doe".to_string());

        let result = chain.transform(&inputs).unwrap();
        assert_eq!(result.get("full_name").unwrap(), "John Doe");
    }

    #[test]
    fn test_transform_chain_missing_input() {
        let chain = TransformChain::simple("text", "upper", |text| Ok(text.to_uppercase()));

        let inputs = HashMap::new(); // Missing required "text" input

        let result = chain.transform(&inputs);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Missing required"));
    }

    #[test]
    fn test_transform_chain_with_inputs() {
        let chain = TransformChain::simple("text", "length", |text| Ok(text.len().to_string()));

        let mut inputs = HashMap::new();
        inputs.insert("text".to_string(), "hello".to_string());

        let result = chain.transform_with_inputs(&inputs).unwrap();
        assert_eq!(result.get("text").unwrap(), "hello");
        assert_eq!(result.get("length").unwrap(), "5");
    }

    #[test]
    fn test_transform_chain_error_handling() {
        let chain = TransformChain::simple("number", "parsed", |text| {
            text.parse::<i32>()
                .map(|n| n.to_string())
                .map_err(|e| dashflow::core::error::Error::InvalidInput(e.to_string()))
        });

        let mut inputs = HashMap::new();
        inputs.insert("number".to_string(), "not a number".to_string());

        let result = chain.transform(&inputs);
        assert!(result.is_err());
    }

    #[test]
    fn test_transform_chain_text_cleaning() {
        let chain = TransformChain::simple("text", "clean", |text| {
            // Remove extra whitespace and trim
            Ok(text
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ")
                .trim()
                .to_string())
        });

        let mut inputs = HashMap::new();
        inputs.insert("text".to_string(), "  hello    world  ".to_string());

        let result = chain.transform(&inputs).unwrap();
        assert_eq!(result.get("clean").unwrap(), "hello world");
    }
}
