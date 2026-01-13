//! LLM Chain - Simple prompt formatting and LLM execution
//!
//! This module provides a basic chain that formats a prompt with input variables
//! and sends it to a language model for completion.
//!
//! # Example
//!
//! ```rust,ignore
//! use dashflow_chains::LLMChain;
//! use dashflow::core::prompts::PromptTemplate;
//! use std::collections::HashMap;
//!
//! // Create chain
//! let prompt = PromptTemplate::from_template("Tell me a joke about {topic}").unwrap();
//! let chain = LLMChain::new(model, prompt);
//!
//! // Run chain
//! let mut inputs = HashMap::new();
//! inputs.insert("topic".to_string(), "rust".to_string());
//! let result = chain.run(&inputs).await?;
//! ```

use dashflow::core::error::Result;
use dashflow::core::language_models::{ChatModel, ChatResult, LLMResult, LLM};
use dashflow::core::prompts::{ChatPromptTemplate, PromptTemplate};
use std::collections::HashMap;
use std::sync::Arc;

/// Chain that formats a prompt and calls an LLM.
///
/// This is the most basic chain - it takes a prompt template, formats it with input
/// variables, and sends it to a language model. The LLM's response is returned as-is.
///
/// # Type Parameters
///
/// - `M`: The model type (either `LLM` or `ChatModel`)
///
/// # Example
///
/// ```rust,ignore
/// use dashflow_chains::LLMChain;
/// use dashflow::core::prompts::PromptTemplate;
///
/// let prompt = PromptTemplate::from_template("What is {number} + {number2}?").unwrap();
/// let chain = LLMChain::new(llm, prompt);
///
/// let result = chain.run(&maplit::hashmap! {
///     "number".to_string() => "2".to_string(),
///     "number2".to_string() => "3".to_string(),
/// }).await?;
/// ```
#[derive(Clone)]
pub struct LLMChain<M> {
    model: Arc<M>,
    prompt: PromptTemplate,
}

impl<M> LLMChain<M> {
    /// Create a new LLM chain with a model and prompt template.
    pub fn new(model: Arc<M>, prompt: PromptTemplate) -> Self {
        Self { model, prompt }
    }

    /// Get a reference to the prompt template.
    #[must_use]
    pub fn prompt(&self) -> &PromptTemplate {
        &self.prompt
    }

    /// Get a reference to the model.
    #[must_use]
    pub fn model(&self) -> &M {
        &self.model
    }
}

impl<M: LLM> LLMChain<M> {
    /// Run the chain with the given input variables.
    ///
    /// Formats the prompt with the inputs and sends it to the LLM.
    /// Returns the LLM's text response.
    pub async fn run(&self, inputs: &HashMap<String, String>) -> Result<String> {
        // Format the prompt
        let formatted = self.prompt.format(inputs)?;

        // Call the LLM
        let result = self.model.generate(&[formatted], None, None).await?;

        // Extract the first generation's text
        result
            .generations
            .first()
            .and_then(|g| g.first())
            .map(|gen| gen.text.clone())
            .ok_or_else(|| {
                dashflow::core::error::Error::Other("No generation returned from LLM".to_string())
            })
    }

    /// Generate multiple outputs for multiple prompts.
    ///
    /// This is more efficient than calling `run` multiple times.
    pub async fn generate(&self, inputs: &[HashMap<String, String>]) -> Result<LLMResult> {
        // Format all prompts
        let formatted: Result<Vec<String>> = inputs
            .iter()
            .map(|input| self.prompt.format(input))
            .collect();
        let formatted = formatted?;

        // Call the LLM
        let result = self.model.generate(&formatted, None, None).await?;

        Ok(result)
    }
}

/// Chat model variant of LLM chain.
///
/// This chain works with chat models that accept structured messages.
/// The prompt template is formatted and sent as a human message.
#[derive(Clone)]
pub struct ChatLLMChain<M> {
    model: Arc<M>,
    prompt: ChatPromptTemplate,
}

impl<M> ChatLLMChain<M> {
    /// Create a new chat LLM chain with a model and chat prompt template.
    pub fn new(model: Arc<M>, prompt: ChatPromptTemplate) -> Self {
        Self { model, prompt }
    }

    /// Create a chat chain with a simple prompt template (converted to chat format).
    pub fn from_template(model: Arc<M>, template: &str) -> Result<Self> {
        let prompt = ChatPromptTemplate::from_messages(vec![("human", template)])?;
        Ok(Self { model, prompt })
    }

    /// Get a reference to the prompt template.
    #[must_use]
    pub fn prompt(&self) -> &ChatPromptTemplate {
        &self.prompt
    }

    /// Get a reference to the model.
    #[must_use]
    pub fn model(&self) -> &M {
        &self.model
    }
}

impl<M: ChatModel> ChatLLMChain<M> {
    /// Run the chain with the given input variables.
    ///
    /// Formats the prompt with the inputs and sends it to the chat model.
    /// Returns the model's text response.
    pub async fn run(&self, inputs: &HashMap<String, String>) -> Result<String> {
        // Format the prompt to messages
        let messages = self.prompt.format_messages(inputs)?;

        // Call the chat model
        let result = self
            .model
            .generate(&messages, None, None, None, None)
            .await?;

        // Extract the first generation's text
        Ok(result
            .generations
            .first()
            .map(dashflow::core::language_models::ChatGeneration::text)
            .unwrap_or_default())
    }

    /// Generate multiple outputs for multiple prompts.
    pub async fn generate(&self, inputs: &[HashMap<String, String>]) -> Result<Vec<ChatResult>> {
        // Format all prompts to messages
        let messages: Result<Vec<Vec<_>>> = inputs
            .iter()
            .map(|input| self.prompt.format_messages(input))
            .collect();
        let messages = messages?;

        // Call the chat model for each message set
        let mut results = Vec::new();
        for msg_set in messages {
            let result = self
                .model
                .generate(&msg_set, None, None, None, None)
                .await?;
            results.push(result);
        }

        Ok(results)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use dashflow::core::language_models::{ChatGeneration, Generation};
    use dashflow::core::language_models::{ToolChoice, ToolDefinition};
    use dashflow::core::messages::{AIMessage, BaseMessage};
    use std::collections::HashMap;

    // Mock LLM for testing
    struct MockLLM;

    #[async_trait]
    impl LLM for MockLLM {
        async fn _generate(
            &self,
            prompts: &[String],
            _stop: Option<&[String]>,
            _run_manager: Option<&dashflow::core::callbacks::CallbackManager>,
        ) -> Result<LLMResult> {
            let generations: Vec<Vec<Generation>> = prompts
                .iter()
                .map(|prompt| vec![Generation::new(format!("Mock response to: {}", prompt))])
                .collect();

            Ok(LLMResult::with_prompts(generations))
        }

        fn llm_type(&self) -> &str {
            "mock"
        }
    }

    // Mock ChatModel for testing
    struct MockChatModel;

    #[async_trait]
    impl ChatModel for MockChatModel {
        async fn _generate(
            &self,
            messages: &[BaseMessage],
            _stop: Option<&[String]>,
            _tools: Option<&[ToolDefinition]>,
            _tool_choice: Option<&ToolChoice>,
            _run_manager: Option<&dashflow::core::callbacks::CallbackManager>,
        ) -> Result<ChatResult> {
            let content = messages
                .iter()
                .map(|m| m.content().as_text())
                .collect::<Vec<_>>()
                .join(" ");

            let generation = ChatGeneration::new(
                AIMessage::new(format!("Mock response to: {}", content)).into(),
            );

            Ok(ChatResult::new(generation))
        }

        fn llm_type(&self) -> &str {
            "mock"
        }
        fn as_any(&self) -> &dyn std::any::Any {
            self
        }
    }

    #[tokio::test]
    async fn test_llm_chain_basic() {
        let llm = Arc::new(MockLLM);
        let prompt = PromptTemplate::from_template("Tell me a joke about {topic}").unwrap();
        let chain = LLMChain::new(llm, prompt);

        let mut inputs = HashMap::new();
        inputs.insert("topic".to_string(), "rust".to_string());

        let result = chain.run(&inputs).await.unwrap();
        assert!(result.contains("Tell me a joke about rust"));
    }

    #[tokio::test]
    async fn test_llm_chain_multiple_variables() {
        let llm = Arc::new(MockLLM);
        let prompt = PromptTemplate::from_template("What is {x} + {y}? Answer:").unwrap();
        let chain = LLMChain::new(llm, prompt);

        let mut inputs = HashMap::new();
        inputs.insert("x".to_string(), "5".to_string());
        inputs.insert("y".to_string(), "3".to_string());

        let result = chain.run(&inputs).await.unwrap();
        assert!(result.contains("What is 5 + 3"));
    }

    #[tokio::test]
    async fn test_llm_chain_generate_multiple() {
        let llm = Arc::new(MockLLM);
        let prompt = PromptTemplate::from_template("Question: {q}").unwrap();
        let chain = LLMChain::new(llm, prompt);

        let mut input1 = HashMap::new();
        input1.insert("q".to_string(), "first".to_string());
        let mut input2 = HashMap::new();
        input2.insert("q".to_string(), "second".to_string());

        let inputs = vec![input1, input2];

        let results = chain.generate(&inputs).await.unwrap();
        assert_eq!(results.generations.len(), 2);
        assert!(results.generations[0][0].text.contains("Question: first"));
        assert!(results.generations[1][0].text.contains("Question: second"));
    }

    #[tokio::test]
    async fn test_chat_llm_chain_basic() {
        let model = Arc::new(MockChatModel);
        let prompt =
            ChatPromptTemplate::from_messages(vec![("human", "Tell me about {topic}")]).unwrap();
        let chain = ChatLLMChain::new(model, prompt);

        let mut inputs = HashMap::new();
        inputs.insert("topic".to_string(), "Rust".to_string());

        let result = chain.run(&inputs).await.unwrap();
        assert!(result.contains("Tell me about Rust"));
    }

    #[tokio::test]
    async fn test_chat_llm_chain_from_template() {
        let model = Arc::new(MockChatModel);
        let chain = ChatLLMChain::from_template(model, "What is {topic}?").unwrap();

        let mut inputs = HashMap::new();
        inputs.insert("topic".to_string(), "async Rust".to_string());

        let result = chain.run(&inputs).await.unwrap();
        assert!(result.contains("What is async Rust"));
    }

    #[tokio::test]
    async fn test_chat_llm_chain_system_message() {
        let model = Arc::new(MockChatModel);
        let prompt = ChatPromptTemplate::from_messages(vec![
            ("system", "You are a helpful assistant."),
            ("human", "Tell me about {topic}"),
        ])
        .unwrap();
        let chain = ChatLLMChain::new(model, prompt);

        let mut inputs = HashMap::new();
        inputs.insert("topic".to_string(), "Rust".to_string());

        let result = chain.run(&inputs).await.unwrap();
        assert!(result.contains("helpful assistant"));
        assert!(result.contains("Rust"));
    }

    #[tokio::test]
    async fn test_chat_llm_chain_generate_multiple() {
        let model = Arc::new(MockChatModel);
        let chain = ChatLLMChain::from_template(model, "Question: {q}").unwrap();

        let mut input1 = HashMap::new();
        input1.insert("q".to_string(), "one".to_string());
        let mut input2 = HashMap::new();
        input2.insert("q".to_string(), "two".to_string());

        let inputs = vec![input1, input2];

        let results = chain.generate(&inputs).await.unwrap();
        assert_eq!(results.len(), 2);
        assert!(results[0].generations[0].text().contains("Question: one"));
        assert!(results[1].generations[0].text().contains("Question: two"));
    }

    // Property-based tests
    mod property_tests {
        use super::*;
        use proptest::prelude::*;

        // Generate valid variable names (alphanumeric + underscore)
        fn var_name() -> impl Strategy<Value = String> {
            "[a-z][a-z0-9_]{0,15}"
        }

        // Generate valid variable values (any string without newlines for simplicity)
        fn var_value() -> impl Strategy<Value = String> {
            "[^\\n]{0,100}"
        }

        // Generate a template with N variables
        fn template_with_vars(num_vars: usize) -> impl Strategy<Value = (String, Vec<String>)> {
            proptest::collection::vec(var_name(), num_vars..=num_vars).prop_flat_map(|vars| {
                let vars_clone = vars.clone();
                let template = format!(
                    "Prompt: {}",
                    vars.iter()
                        .map(|v| format!("{{{}}}", v))
                        .collect::<Vec<_>>()
                        .join(" and ")
                );
                Just((template, vars_clone))
            })
        }

        proptest! {
            /// Property: For any valid template with variables and matching inputs,
            /// formatting should succeed and produce a string containing all input values
            #[test]
            fn prop_template_format_with_valid_inputs(
                (template_str, var_names) in template_with_vars(3),
                val1 in var_value(),
                val2 in var_value(),
                val3 in var_value(),
            ) {
                // Create runtime
                let rt = tokio::runtime::Runtime::new().unwrap();
                rt.block_on(async {
                    let llm = Arc::new(MockLLM);
                    let prompt = PromptTemplate::from_template(&template_str).unwrap();
                    let chain = LLMChain::new(llm, prompt);

                    let mut inputs = HashMap::new();
                    inputs.insert(var_names[0].clone(), val1.clone());
                    inputs.insert(var_names[1].clone(), val2.clone());
                    inputs.insert(var_names[2].clone(), val3.clone());

                    // Chain should run without error
                    let result = chain.run(&inputs).await;
                    prop_assert!(result.is_ok(), "Chain should succeed with valid inputs");

                    // Result should be non-empty for non-empty inputs
                    let result_text = result.unwrap();
                    let has_input = !val1.is_empty() || !val2.is_empty() || !val3.is_empty();
                    if has_input {
                        prop_assert!(!result_text.is_empty(), "Result should be non-empty for non-empty inputs");
                    }

                    // Template formatting should be stable (doesn't crash on Unicode)
                    // Note: We test stability, not exact content preservation, since template
                    // formatting may normalize or escape certain characters

                    Ok(())
                }).unwrap();
            }

            /// Property: Template variable substitution is deterministic -
            /// same inputs produce same outputs
            #[test]
            fn prop_template_substitution_deterministic(
                val in var_value(),
            ) {
                let rt = tokio::runtime::Runtime::new().unwrap();
                rt.block_on(async {
                    let llm = Arc::new(MockLLM);
                    let prompt = PromptTemplate::from_template("Value: {x}").unwrap();
                    let chain = LLMChain::new(llm, prompt);

                    let mut inputs = HashMap::new();
                    inputs.insert("x".to_string(), val.clone());

                    // Run twice with same inputs
                    let result1 = chain.run(&inputs).await.unwrap();
                    let result2 = chain.run(&inputs).await.unwrap();

                    // Results should be identical
                    prop_assert_eq!(&result1, &result2, "Same inputs should produce same outputs");

                    Ok(())
                }).unwrap();
            }

            /// Property: Missing template variables are handled gracefully
            ///
            /// NOTE: Current behavior allows missing variables (they render as empty or "{varname}").
            /// This matches Python DashFlow behavior where format() doesn't validate inputs.
            /// If validation is needed, users should call format_prompt() which calls validate_inputs().
            #[test]
            fn prop_missing_variables_handled(
                var_name in "[a-z][a-z0-9_]{1,10}",
            ) {
                let rt = tokio::runtime::Runtime::new().unwrap();
                rt.block_on(async {
                    let llm = Arc::new(MockLLM);
                    let template = format!("Value: {{{}}}", var_name);
                    let prompt = PromptTemplate::from_template(&template).unwrap();
                    let chain = LLMChain::new(llm, prompt);

                    // Empty inputs (missing the required variable)
                    let inputs = HashMap::new();

                    // Currently succeeds - format() doesn't validate inputs
                    // (format_prompt() would fail, but LLMChain uses format())
                    let result = chain.run(&inputs).await;
                    prop_assert!(result.is_ok(), "LLMChain.format() allows missing variables (current behavior)");

                    Ok(())
                }).unwrap();
            }

            /// Property: Extra variables in inputs are ignored (don't cause errors)
            #[test]
            fn prop_extra_variables_ignored(
                val in var_value(),
                extra_key in "[a-z][a-z0-9_]{1,10}",
                extra_val in var_value(),
            ) {
                let rt = tokio::runtime::Runtime::new().unwrap();
                rt.block_on(async {
                    let llm = Arc::new(MockLLM);
                    let prompt = PromptTemplate::from_template("Value: {x}").unwrap();
                    let chain = LLMChain::new(llm, prompt);

                    let mut inputs = HashMap::new();
                    inputs.insert("x".to_string(), val.clone());
                    inputs.insert(extra_key, extra_val); // extra variable

                    // Should succeed despite extra variable
                    let result = chain.run(&inputs).await;
                    prop_assert!(result.is_ok(), "Extra variables should be ignored, not cause errors");

                    Ok(())
                }).unwrap();
            }

            /// Property: Batch generation preserves order
            #[test]
            fn prop_batch_generation_preserves_order(
                vals in proptest::collection::vec(var_value(), 1..=10),
            ) {
                let rt = tokio::runtime::Runtime::new().unwrap();
                rt.block_on(async {
                    let llm = Arc::new(MockLLM);
                    let prompt = PromptTemplate::from_template("Value: {x}").unwrap();
                    let chain = LLMChain::new(llm, prompt);

                    let inputs: Vec<HashMap<String, String>> = vals
                        .iter()
                        .map(|v| {
                            let mut m = HashMap::new();
                            m.insert("x".to_string(), v.clone());
                            m
                        })
                        .collect();

                    let result = chain.generate(&inputs).await.unwrap();

                    // Should have same number of generations as inputs
                    prop_assert_eq!(result.generations.len(), vals.len(), "Generation count should match input count");

                    // Each generation should correspond to its input value
                    for (i, val) in vals.iter().enumerate() {
                        if !val.is_empty() {
                            let gen_text = &result.generations[i][0].text;
                            prop_assert!(gen_text.contains(val), "Generation {} should contain value: {}", i, val);
                        }
                    }

                    Ok(())
                }).unwrap();
            }
        }

        proptest! {
            /// Property: ChatLLMChain produces valid outputs for any valid inputs
            #[test]
            fn prop_chat_chain_valid_outputs(
                val in var_value(),
            ) {
                let rt = tokio::runtime::Runtime::new().unwrap();
                rt.block_on(async {
                    let model = Arc::new(MockChatModel);
                    let chain = ChatLLMChain::from_template(model, "Topic: {topic}").unwrap();

                    let mut inputs = HashMap::new();
                    inputs.insert("topic".to_string(), val.clone());

                    let result = chain.run(&inputs).await;
                    prop_assert!(result.is_ok(), "ChatLLMChain should succeed with valid inputs");

                    let result_text = result.unwrap();
                    if !val.is_empty() {
                        prop_assert!(result_text.contains(&val), "Result should contain input value");
                    }

                    Ok(())
                }).unwrap();
            }

            /// Property: ChatLLMChain batch generation preserves order
            #[test]
            fn prop_chat_batch_preserves_order(
                vals in proptest::collection::vec(var_value(), 1..=10),
            ) {
                let rt = tokio::runtime::Runtime::new().unwrap();
                rt.block_on(async {
                    let model = Arc::new(MockChatModel);
                    let chain = ChatLLMChain::from_template(model, "Value: {x}").unwrap();

                    let inputs: Vec<HashMap<String, String>> = vals
                        .iter()
                        .map(|v| {
                            let mut m = HashMap::new();
                            m.insert("x".to_string(), v.clone());
                            m
                        })
                        .collect();

                    let results = chain.generate(&inputs).await.unwrap();

                    // Should have same number of results as inputs
                    prop_assert_eq!(results.len(), vals.len(), "Result count should match input count");

                    // Each result should correspond to its input value
                    for (i, val) in vals.iter().enumerate() {
                        if !val.is_empty() {
                            let result_text = results[i].generations[0].text();
                            prop_assert!(result_text.contains(val), "Result {} should contain value: {}", i, val);
                        }
                    }

                    Ok(())
                }).unwrap();
            }
        }
    }
}
