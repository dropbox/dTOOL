//! LLM Math Chain - Solve mathematical problems using LLM + expression evaluation
//!
//! This chain uses an LLM to translate natural language math questions into
//! mathematical expressions, then evaluates them safely using expression evaluation.
//!
//! # Example
//!
//! ```rust,ignore
//! use dashflow_chains::llm_math::LLMMathChain;
//! use dashflow::core::language_models::LLM;
//!
//! let chain = LLMMathChain::new(llm)
//!     .with_input_key("question")
//!     .with_output_key("answer");
//!
//! let result = chain.run("What is 37593 * 67?").await?;
//! // result: "Answer: 2518731"
//! ```

use crate::llm::LLMChain;
use dashflow::core::error::{Error, Result};
use dashflow::core::language_models::LLM;
use dashflow::core::prompts::PromptTemplate;
use regex::Regex;
use std::collections::HashMap;
use std::sync::Arc;

/// Default prompt template for LLM Math Chain
const DEFAULT_PROMPT: &str = r#"Translate a math problem into a expression that can be executed using Python's numexpr library. Use the output of running this code to answer the question.

Question: ${{Question with math problem.}}
```text
${{single line mathematical expression that solves the problem}}
```
...numexpr.evaluate(text)...
```output
${{Output of running the code}}
```
Answer: ${{Answer}}

Begin.

Question: What is 37593 * 67?
```text
37593 * 67
```
...numexpr.evaluate("37593 * 67")...
```output
2518731
```
Answer: 2518731

Question: 37593^(1/5)
```text
37593**(1/5)
```
...numexpr.evaluate("37593**(1/5)")...
```output
8.222831614237718
```
Answer: 8.222831614237718

Question: {question}
"#;

/// Chain that interprets a prompt and executes mathematical expressions
///
/// This chain:
/// 1. Takes a natural language math question
/// 2. Uses an LLM to translate it into a mathematical expression
/// 3. Evaluates the expression safely
/// 4. Returns the formatted answer
///
/// # Safety
///
/// This chain uses the `meval` crate for safe expression evaluation.
/// It does not execute arbitrary code and is sandboxed.
///
/// # Example
///
/// ```rust,ignore
/// use dashflow_chains::llm_math::LLMMathChain;
///
/// let chain = LLMMathChain::from_llm(llm)?;
/// let result = chain.run("What is 25 * 4?").await?;
/// assert!(result.contains("100"));
/// ```
pub struct LLMMathChain<M: LLM> {
    llm_chain: LLMChain<M>,
    input_key: String,
    output_key: String,
}

impl<M: LLM> LLMMathChain<M> {
    /// Create a new `LLMMathChain` from an LLM
    ///
    /// # Arguments
    ///
    /// * `llm` - Language model to use for generating mathematical expressions
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let chain = LLMMathChain::from_llm(llm)?;
    /// ```
    pub fn from_llm(llm: Arc<M>) -> Result<Self> {
        let prompt = PromptTemplate::from_template(DEFAULT_PROMPT)?;
        let llm_chain = LLMChain::new(llm, prompt);

        Ok(Self {
            llm_chain,
            input_key: "question".to_string(),
            output_key: "answer".to_string(),
        })
    }

    /// Set custom input key (default: "question")
    pub fn with_input_key(mut self, key: impl Into<String>) -> Self {
        self.input_key = key.into();
        self
    }

    /// Set custom output key (default: "answer")
    pub fn with_output_key(mut self, key: impl Into<String>) -> Self {
        self.output_key = key.into();
        self
    }

    /// Run the chain with a math question
    ///
    /// # Arguments
    ///
    /// * `question` - Natural language math question
    ///
    /// # Returns
    ///
    /// The answer as a string, formatted as "Answer: \<result\>"
    pub async fn run(&self, question: impl Into<String>) -> Result<String> {
        let question = question.into();
        let mut inputs = HashMap::new();
        inputs.insert(self.input_key.clone(), question.clone());

        let result = self.call(&inputs).await?;
        Ok(result
            .get(&self.output_key)
            .ok_or_else(|| Error::Other(format!("Output key '{}' not found", self.output_key)))?
            .clone())
    }

    /// Call the chain with inputs
    ///
    /// # Arguments
    ///
    /// * `inputs` - Map containing the question under `input_key`
    ///
    /// # Returns
    ///
    /// Map containing the answer under `output_key`
    pub async fn call(&self, inputs: &HashMap<String, String>) -> Result<HashMap<String, String>> {
        let question = inputs.get(&self.input_key).ok_or_else(|| {
            Error::InvalidInput(format!("Input key '{}' not found", self.input_key))
        })?;

        // Call LLM to generate expression
        let mut llm_inputs = HashMap::new();
        llm_inputs.insert("question".to_string(), question.clone());

        let llm_output = self.llm_chain.run(&llm_inputs).await?;

        // Process LLM output and evaluate expression
        let answer = self.process_llm_result(&llm_output)?;

        let mut result = HashMap::new();
        result.insert(self.output_key.clone(), answer);
        Ok(result)
    }

    /// Process LLM output and evaluate mathematical expression
    ///
    /// The LLM output can be in several formats:
    /// 1. Code block: ```text\n<expression>\n```
    /// 2. Already answered: "Answer: \<result\>"
    /// 3. Contains answer: "... Answer: \<result\>"
    fn process_llm_result(&self, llm_output: &str) -> Result<String> {
        let llm_output = llm_output.trim();

        // Try to extract code block with expression ((?s) enables DOTALL mode)
        // SAFETY: M-347 - Compile-time constant regex pattern
        #[allow(clippy::expect_used)]
        let text_block_regex =
            Regex::new(r"(?s)```text(.*?)```").expect("text_block regex is valid");
        if let Some(captures) = text_block_regex.captures(llm_output) {
            // SAFETY: Group 1 (.*?) is required by the regex pattern
            #[allow(clippy::expect_used)]
            let expression = captures.get(1).expect("group 1 exists").as_str().trim();
            let result = self.evaluate_expression(expression)?;
            return Ok(format!("Answer: {result}"));
        }

        // Check if already formatted as answer
        if llm_output.starts_with("Answer:") {
            return Ok(llm_output.to_string());
        }

        // Check if contains "Answer:" somewhere
        if llm_output.contains("Answer:") {
            let parts: Vec<&str> = llm_output.split("Answer:").collect();
            // SAFETY: split() always returns at least one element
            #[allow(clippy::expect_used)]
            let answer = parts
                .last()
                .expect("split always returns at least one element")
                .trim();
            return Ok(format!("Answer: {answer}"));
        }

        // Unknown format
        Err(Error::InvalidInput(format!(
            "unknown format from LLM: {llm_output}"
        )))
    }

    /// Safely evaluate a mathematical expression
    ///
    /// Uses the `meval` crate for safe expression evaluation.
    /// Supports standard math operations: +, -, *, /, ^, sqrt, etc.
    ///
    /// # Arguments
    ///
    /// * `expression` - Mathematical expression to evaluate
    ///
    /// # Returns
    ///
    /// String representation of the result
    fn evaluate_expression(&self, expression: &str) -> Result<String> {
        // Convert Python ** to ^ for fasteval
        let normalized = expression.replace("**", "^");

        // Evaluate using fasteval
        match fasteval::ez_eval(&normalized, &mut fasteval::EmptyNamespace) {
            Ok(result) => Ok(result.to_string()),
            Err(e) => Err(Error::InvalidInput(format!(
                "Failed to evaluate expression '{expression}': {e}. Please try again with a valid numerical expression"
            ))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dashflow::core::callbacks::CallbackManager;
    use dashflow::core::language_models::{Generation, LLMResult, LLM};

    /// Mock LLM for testing
    struct MockMathLLM {
        responses: HashMap<String, String>,
    }

    impl MockMathLLM {
        fn new() -> Self {
            let mut responses = HashMap::new();

            // Simple question that doesn't need evaluation
            responses.insert("What is 1 plus 1?".to_string(), "Answer: 2".to_string());

            // Complex question that needs evaluation
            responses.insert(
                "What is the square root of 2?".to_string(),
                "```text\n2^(1/2)\n```".to_string(),
            );

            // Another math question
            responses.insert(
                "What is 37593 * 67?".to_string(),
                "```text\n37593 * 67\n```".to_string(),
            );

            // Power operation
            responses.insert(
                "What is 37593^(1/5)?".to_string(),
                "```text\n37593^(1/5)\n```".to_string(),
            );

            // Invalid response
            responses.insert("foo".to_string(), "foo".to_string());

            Self { responses }
        }

        fn get_response(&self, prompt: &str) -> String {
            // Extract the last line of the prompt which contains the question
            let mut question = if let Some(last_line) = prompt.lines().last() {
                last_line.trim()
            } else {
                prompt.trim()
            };

            // Strip "Question: " prefix if present
            if question.starts_with("Question: ") {
                question = &question[10..];
            }

            self.responses
                .get(question)
                .cloned()
                .unwrap_or_else(|| format!("Unknown question: {}", question))
        }
    }

    #[async_trait::async_trait]
    impl LLM for MockMathLLM {
        async fn _generate(
            &self,
            prompts: &[String],
            _stop: Option<&[String]>,
            _callbacks: Option<&CallbackManager>,
        ) -> Result<LLMResult> {
            let generations = prompts
                .iter()
                .map(|prompt| {
                    let text = self.get_response(prompt);

                    Generation {
                        text,
                        generation_info: None,
                    }
                })
                .collect();

            Ok(LLMResult {
                generations: vec![generations],
                llm_output: None,
            })
        }

        fn llm_type(&self) -> &str {
            "mock_math"
        }
    }

    #[tokio::test]
    async fn test_simple_question() {
        let llm = Arc::new(MockMathLLM::new());
        let chain = LLMMathChain::from_llm(llm).unwrap();

        let result = chain.run("What is 1 plus 1?").await.unwrap();
        assert_eq!(result, "Answer: 2");
    }

    #[tokio::test]
    async fn test_complex_question() {
        let llm = Arc::new(MockMathLLM::new());
        let chain = LLMMathChain::from_llm(llm).unwrap();

        let result = chain.run("What is the square root of 2?").await.unwrap();
        assert!(result.starts_with("Answer:"));

        // Parse the number from the answer
        let answer_str = result.strip_prefix("Answer: ").unwrap();
        let answer: f64 = answer_str.parse().unwrap();

        // Check it's approximately sqrt(2)
        assert!((answer - 2f64.sqrt()).abs() < 0.0001);
    }

    #[tokio::test]
    async fn test_multiplication() {
        let llm = Arc::new(MockMathLLM::new());
        let chain = LLMMathChain::from_llm(llm).unwrap();

        let result = chain.run("What is 37593 * 67?").await.unwrap();
        assert_eq!(result, "Answer: 2518731");
    }

    #[tokio::test]
    async fn test_power_operation() {
        let llm = Arc::new(MockMathLLM::new());
        let chain = LLMMathChain::from_llm(llm).unwrap();

        let result = chain.run("What is 37593^(1/5)?").await.unwrap();
        assert!(result.starts_with("Answer:"));

        // Parse the number from the answer
        let answer_str = result.strip_prefix("Answer: ").unwrap();
        let answer: f64 = answer_str.parse().unwrap();

        // Check it's approximately 37593^(1/5) = 8.222831614237718
        assert!((answer - 8.222831614237718).abs() < 0.0001);
    }

    #[tokio::test]
    async fn test_error_handling() {
        let llm = Arc::new(MockMathLLM::new());
        let chain = LLMMathChain::from_llm(llm).unwrap();

        let result = chain.run("foo").await;
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("unknown format from LLM"));
    }

    #[tokio::test]
    async fn test_custom_keys() {
        let llm = Arc::new(MockMathLLM::new());
        let chain = LLMMathChain::from_llm(llm)
            .unwrap()
            .with_input_key("q")
            .with_output_key("a");

        let mut inputs = HashMap::new();
        inputs.insert("q".to_string(), "What is 1 plus 1?".to_string());

        let result = chain.call(&inputs).await.unwrap();
        assert_eq!(result.get("a").unwrap(), "Answer: 2");
    }

    #[test]
    fn test_evaluate_expression() {
        let llm = Arc::new(MockMathLLM::new());
        let chain = LLMMathChain::from_llm(llm).unwrap();

        // Test basic operations
        assert_eq!(chain.evaluate_expression("2 + 2").unwrap(), "4");
        assert_eq!(chain.evaluate_expression("10 - 3").unwrap(), "7");
        assert_eq!(chain.evaluate_expression("5 * 6").unwrap(), "30");
        assert_eq!(chain.evaluate_expression("20 / 4").unwrap(), "5");

        // Test power operation
        let result = chain.evaluate_expression("2^3").unwrap();
        assert_eq!(result, "8");

        // Test Python ** syntax
        let result = chain.evaluate_expression("2**3").unwrap();
        assert_eq!(result, "8");

        // Test complex expression
        let result = chain.evaluate_expression("(10 + 5) * 2").unwrap();
        assert_eq!(result, "30");
    }
}
