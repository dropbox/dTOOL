//! # LLM Checker Chain
//!
//! Chain for question-answering with self-verification.
//!
//! Heavily borrowed from <https://github.com/jagilley/fact-checker>
//!
//! This chain implements a 4-step process to verify LLM outputs:
//! 1. **Create Draft Answer**: Generate initial answer to the question
//! 2. **List Assertions**: Extract factual claims from the draft
//! 3. **Check Assertions**: Verify each claim's truthfulness
//! 4. **Revise Answer**: Generate final answer incorporating verification results
//!
//! ## Example
//!
//! ```rust,no_run
//! use dashflow_chains::LLMCheckerChain;
//! use std::sync::Arc;
//! use std::collections::HashMap;
//!
//! #[tokio::main]
//! async fn main() {
//!     // let llm = ...;
//!     // let checker = LLMCheckerChain::from_llm(llm);
//!     //
//!     // let mut inputs = HashMap::new();
//!     // inputs.insert("query".to_string(), "When was the Eiffel Tower built?".to_string());
//!     //
//!     // let result = checker.call(&inputs).await.unwrap();
//!     // println!("Verified answer: {}", result["result"]);
//! }
//! ```

use crate::LLMChain;
use dashflow::core::error::Result;
use dashflow::core::language_models::LLM;
use dashflow::core::prompts::PromptTemplate;
use std::collections::HashMap;
use std::sync::Arc;

/// Prompt template for creating initial draft answer
pub const CREATE_DRAFT_ANSWER_TEMPLATE: &str = "{question}\n\n";

/// Prompt template for listing assertions from the draft
pub const LIST_ASSERTIONS_TEMPLATE: &str = "Here is a statement:
{statement}
Make a bullet point list of the assumptions you made when producing the above statement.\n\n";

/// Prompt template for checking assertions
pub const CHECK_ASSERTIONS_TEMPLATE: &str = "Here is a bullet point list of assertions:
{assertions}
For each assertion, determine whether it is true or false. If it is false, explain why.\n\n";

/// Prompt template for creating revised answer
pub const REVISED_ANSWER_TEMPLATE: &str = "{checked_assertions}

Question: In light of the above assertions and checks, how would you answer the question '{question}'?

Answer:";

/// Create the default prompt for generating draft answers.
#[must_use]
pub fn create_draft_answer_prompt() -> PromptTemplate {
    PromptTemplate::new(
        CREATE_DRAFT_ANSWER_TEMPLATE.to_string(),
        vec!["question".to_string()],
        dashflow::core::prompts::PromptTemplateFormat::FString,
    )
}

/// Create the default prompt for listing assertions.
#[must_use]
pub fn list_assertions_prompt() -> PromptTemplate {
    PromptTemplate::new(
        LIST_ASSERTIONS_TEMPLATE.to_string(),
        vec!["statement".to_string()],
        dashflow::core::prompts::PromptTemplateFormat::FString,
    )
}

/// Create the default prompt for checking assertions.
#[must_use]
pub fn check_assertions_prompt() -> PromptTemplate {
    PromptTemplate::new(
        CHECK_ASSERTIONS_TEMPLATE.to_string(),
        vec!["assertions".to_string()],
        dashflow::core::prompts::PromptTemplateFormat::FString,
    )
}

/// Create the default prompt for generating revised answers.
#[must_use]
pub fn revised_answer_prompt() -> PromptTemplate {
    PromptTemplate::new(
        REVISED_ANSWER_TEMPLATE.to_string(),
        vec!["checked_assertions".to_string(), "question".to_string()],
        dashflow::core::prompts::PromptTemplateFormat::FString,
    )
}

/// Chain for question-answering with self-verification.
///
/// This chain uses a 4-step process to generate and verify answers:
///
/// 1. **Create Draft**: LLM generates initial answer
/// 2. **List Assertions**: LLM extracts factual claims
/// 3. **Check Assertions**: LLM verifies each claim
/// 4. **Revise Answer**: LLM creates final answer incorporating verification
///
/// The chain forces the LLM to explicitly state and verify its assumptions,
/// leading to more accurate and reliable answers.
///
/// # Type Parameters
///
/// * `M` - The LLM type to use for all verification steps
///
/// # Example
///
/// ```rust,no_run
/// use dashflow_chains::LLMCheckerChain;
/// use std::sync::Arc;
/// use std::collections::HashMap;
///
/// #[tokio::main]
/// async fn main() {
///     // let llm = ...;
///     // let checker = LLMCheckerChain::from_llm(llm);
///     //
///     // let mut inputs = HashMap::new();
///     // inputs.insert("query".to_string(), "What year was the first iPhone released?".to_string());
///     //
///     // let result = checker.call(&inputs).await.unwrap();
///     // println!("{}", result["result"]);
/// }
/// ```
#[derive(Clone)]
pub struct LLMCheckerChain<M: LLM> {
    /// Chain for creating draft answer
    create_draft_answer_chain: LLMChain<M>,

    /// Chain for listing assertions
    list_assertions_chain: LLMChain<M>,

    /// Chain for checking assertions
    check_assertions_chain: LLMChain<M>,

    /// Chain for creating revised answer
    revised_answer_chain: LLMChain<M>,

    /// Input key name (default: "query")
    input_key: String,

    /// Output key name (default: "result")
    output_key: String,
}

impl<M: LLM> LLMCheckerChain<M> {
    /// Create a new `LLMCheckerChain` with custom prompts.
    ///
    /// # Arguments
    ///
    /// * `llm` - The language model to use for all steps
    /// * `create_draft_prompt` - Prompt for creating initial draft
    /// * `list_assertions_prompt` - Prompt for listing assertions
    /// * `check_assertions_prompt` - Prompt for checking assertions
    /// * `revised_answer_prompt` - Prompt for creating revised answer
    pub fn new(
        llm: Arc<M>,
        create_draft_prompt: PromptTemplate,
        list_assertions_prompt: PromptTemplate,
        check_assertions_prompt: PromptTemplate,
        revised_answer_prompt: PromptTemplate,
    ) -> Self {
        Self {
            create_draft_answer_chain: LLMChain::new(Arc::clone(&llm), create_draft_prompt),
            list_assertions_chain: LLMChain::new(Arc::clone(&llm), list_assertions_prompt),
            check_assertions_chain: LLMChain::new(Arc::clone(&llm), check_assertions_prompt),
            revised_answer_chain: LLMChain::new(llm, revised_answer_prompt),
            input_key: "query".to_string(),
            output_key: "result".to_string(),
        }
    }

    /// Create an `LLMCheckerChain` from a language model with default prompts.
    ///
    /// This is the most common way to create an `LLMCheckerChain`.
    ///
    /// # Arguments
    ///
    /// * `llm` - The language model to use for all verification steps
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use dashflow_chains::LLMCheckerChain;
    /// use std::sync::Arc;
    ///
    /// # async fn example() {
    /// // let llm = ...;
    /// // let checker = LLMCheckerChain::from_llm(llm);
    /// # }
    /// ```
    pub fn from_llm(llm: Arc<M>) -> Self {
        Self::new(
            llm,
            create_draft_answer_prompt(),
            list_assertions_prompt(),
            check_assertions_prompt(),
            revised_answer_prompt(),
        )
    }

    /// Set the input key name.
    ///
    /// Default: "query"
    pub fn with_input_key(mut self, key: impl Into<String>) -> Self {
        self.input_key = key.into();
        self
    }

    /// Set the output key name.
    ///
    /// Default: "result"
    pub fn with_output_key(mut self, key: impl Into<String>) -> Self {
        self.output_key = key.into();
        self
    }

    /// Get the input key name.
    #[must_use]
    pub fn input_key(&self) -> &str {
        &self.input_key
    }

    /// Get the output key name.
    #[must_use]
    pub fn output_key(&self) -> &str {
        &self.output_key
    }

    /// Get the input keys for this chain.
    #[must_use]
    pub fn input_keys(&self) -> Vec<String> {
        vec![self.input_key.clone()]
    }

    /// Get the output keys for this chain.
    #[must_use]
    pub fn output_keys(&self) -> Vec<String> {
        vec![self.output_key.clone()]
    }

    /// Run the LLM checker chain with self-verification.
    ///
    /// This executes the 4-step verification process:
    /// 1. Create draft answer
    /// 2. List assertions from draft
    /// 3. Check each assertion
    /// 4. Generate revised answer
    ///
    /// # Arguments
    ///
    /// * `inputs` - Input values, must contain the `input_key` (default: "query")
    ///
    /// # Returns
    ///
    /// Output values with the `output_key` (default: "result") containing the verified answer
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use std::collections::HashMap;
    /// use std::sync::Arc;
    /// use dashflow_chains::LLMCheckerChain;
    /// use dashflow::core::language_models::FakeLLM;
    ///
    /// # async fn example() {
    /// # let llm = Arc::new(FakeLLM::new(vec!["test response".to_string()]));
    /// # let checker = LLMCheckerChain::from_llm(llm);
    /// let mut inputs = HashMap::new();
    /// inputs.insert("query".to_string(), "When was Python created?".to_string());
    ///
    /// let result = checker.call(&inputs).await.unwrap();
    /// println!("Verified: {}", result["result"]);
    /// # }
    /// ```
    pub async fn call(&self, inputs: &HashMap<String, String>) -> Result<HashMap<String, String>> {
        // Get the question
        let question = inputs
            .get(&self.input_key)
            .ok_or_else(|| {
                dashflow::core::error::Error::InvalidInput(format!(
                    "Missing required input key: {}",
                    self.input_key
                ))
            })?
            .clone();

        // Step 1: Create draft answer
        let mut draft_inputs = HashMap::new();
        draft_inputs.insert("question".to_string(), question.clone());
        let statement = self.create_draft_answer_chain.run(&draft_inputs).await?;

        // Step 2: List assertions
        let mut assertions_inputs = HashMap::new();
        assertions_inputs.insert("statement".to_string(), statement);
        let assertions = self.list_assertions_chain.run(&assertions_inputs).await?;

        // Step 3: Check assertions
        let mut check_inputs = HashMap::new();
        check_inputs.insert("assertions".to_string(), assertions);
        let checked_assertions = self.check_assertions_chain.run(&check_inputs).await?;

        // Step 4: Generate revised answer
        let mut revision_inputs = HashMap::new();
        revision_inputs.insert("checked_assertions".to_string(), checked_assertions);
        revision_inputs.insert("question".to_string(), question);
        let revised_statement = self.revised_answer_chain.run(&revision_inputs).await?;

        // Return final result
        let mut result = HashMap::new();
        result.insert(self.output_key.clone(), revised_statement);
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_draft_answer_prompt() {
        let prompt = create_draft_answer_prompt();
        let mut inputs = HashMap::new();
        inputs.insert("question".to_string(), "What is 2+2?".to_string());
        let formatted = prompt.format(&inputs).unwrap();
        assert_eq!(formatted, "What is 2+2?\n\n");
    }

    #[test]
    fn test_list_assertions_prompt() {
        let prompt = list_assertions_prompt();
        let mut inputs = HashMap::new();
        inputs.insert(
            "statement".to_string(),
            "Paris is the capital of France.".to_string(),
        );
        let formatted = prompt.format(&inputs).unwrap();
        assert!(formatted.contains("Paris is the capital of France."));
        assert!(formatted.contains("bullet point list"));
    }

    #[test]
    fn test_check_assertions_prompt() {
        let prompt = check_assertions_prompt();
        let mut inputs = HashMap::new();
        inputs.insert(
            "assertions".to_string(),
            "- Paris is in France\n- France is in Europe".to_string(),
        );
        let formatted = prompt.format(&inputs).unwrap();
        assert!(formatted.contains("Paris is in France"));
        assert!(formatted.contains("true or false"));
    }

    #[test]
    fn test_revised_answer_prompt() {
        let prompt = revised_answer_prompt();
        let mut inputs = HashMap::new();
        inputs.insert(
            "checked_assertions".to_string(),
            "All claims verified as true".to_string(),
        );
        inputs.insert(
            "question".to_string(),
            "What is the capital of France?".to_string(),
        );
        let formatted = prompt.format(&inputs).unwrap();
        assert!(formatted.contains("All claims verified as true"));
        assert!(formatted.contains("What is the capital of France?"));
        assert!(formatted.contains("In light of"));
    }

    #[test]
    fn test_llm_checker_chain_keys() {
        use dashflow::core::language_models::FakeLLM;

        let llm = Arc::new(FakeLLM::new(vec!["answer".to_string()]));
        let checker = LLMCheckerChain::from_llm(llm);

        assert_eq!(checker.input_key(), "query");
        assert_eq!(checker.output_key(), "result");
        assert_eq!(checker.input_keys(), vec!["query".to_string()]);
        assert_eq!(checker.output_keys(), vec!["result".to_string()]);
    }

    #[test]
    fn test_llm_checker_chain_custom_keys() {
        use dashflow::core::language_models::FakeLLM;

        let llm = Arc::new(FakeLLM::new(vec!["answer".to_string()]));
        let checker = LLMCheckerChain::from_llm(llm)
            .with_input_key("question")
            .with_output_key("answer");

        assert_eq!(checker.input_key(), "question");
        assert_eq!(checker.output_key(), "answer");
    }

    #[tokio::test]
    async fn test_llm_checker_chain_basic() {
        use dashflow::core::language_models::FakeLLM;

        // Create fake LLM with canned responses for each step
        let responses = vec![
            "The Eiffel Tower was built in 1889.".to_string(), // Draft
            "- Built in 1889\n- Located in Paris".to_string(), // Assertions
            "- Built in 1889: TRUE\n- Located in Paris: TRUE".to_string(), // Checked
            "The Eiffel Tower was built in 1889 in Paris.".to_string(), // Revised
        ];
        let llm = Arc::new(FakeLLM::new(responses));
        let checker = LLMCheckerChain::from_llm(llm);

        let mut inputs = HashMap::new();
        inputs.insert(
            "query".to_string(),
            "When was the Eiffel Tower built?".to_string(),
        );

        let result = checker.call(&inputs).await.unwrap();
        assert!(result.contains_key("result"));
        assert!(result["result"].contains("1889"));
        assert!(result["result"].contains("Paris"));
    }

    #[tokio::test]
    async fn test_llm_checker_chain_missing_input() {
        use dashflow::core::language_models::FakeLLM;

        let llm = Arc::new(FakeLLM::new(vec!["test".to_string()]));
        let checker = LLMCheckerChain::from_llm(llm);

        let inputs = HashMap::new();
        let result = checker.call(&inputs).await;

        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Missing required input key"));
    }
}
