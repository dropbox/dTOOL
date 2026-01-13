//! Question-answer generation chains for creating QA pairs from documents.
//!
//! These chains take text documents and automatically generate question-answer
//! pairs that can be used for:
//! - Testing reading comprehension
//! - Creating training datasets
//! - Evaluation and benchmarking
//! - Automated quiz generation
//!
//! # How It Works
//!
//! 1. Splits the input text into chunks using a text splitter
//! 2. For each chunk, uses an LLM to generate a QA pair
//! 3. Parses the JSON output into structured QA pairs
//! 4. Returns a list of question-answer pairs
//!
//! # Example
//!
//! ```rust,ignore
//! use dashflow_chains::qa_generation::QAGenerationChain;
//! use dashflow_text_splitters::RecursiveCharacterTextSplitter;
//!
//! let splitter = RecursiveCharacterTextSplitter::new(1000, 500);
//! let chain = QAGenerationChain::from_llm(llm, splitter);
//!
//! let text = "Your long document text here...";
//! let qa_pairs = chain.generate(text).await?;
//!
//! for qa in qa_pairs {
//!     println!("Q: {}", qa.question);
//!     println!("A: {}", qa.answer);
//! }
//! ```

use dashflow::core::{
    error::{Error, Result},
    language_models::{ChatModel, LLM},
    prompts::PromptTemplate,
};
use dashflow_text_splitters::{RecursiveCharacterTextSplitter, TextSplitter};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::warn;

/// Default prompt for QA generation (for non-chat models).
const DEFAULT_QA_GEN_PROMPT: &str = r#"You are a smart assistant designed to help high school teachers come up with reading comprehension questions.
Given a piece of text, you must come up with a question and answer pair that can be used to test a student's reading comprehension abilities.
When coming up with this question/answer pair, you must respond in the following format:
```
{
    "question": "$YOUR_QUESTION_HERE",
    "answer": "$THE_ANSWER_HERE"
}
```

Everything between the ``` must be valid json.

Please come up with a question/answer pair, in the specified JSON format, for the following text:
----------------
{text}"#;

/// System prompt for QA generation (for chat models).
const SYSTEM_PROMPT: &str = r#"You are a smart assistant designed to help high school teachers come up with reading comprehension questions.
Given a piece of text, you must come up with a question and answer pair that can be used to test a student's reading comprehension abilities.
When coming up with this question/answer pair, you must respond in the following format:
```
{
    "question": "$YOUR_QUESTION_HERE",
    "answer": "$THE_ANSWER_HERE"
}
```

Everything between the ``` must be valid json."#;

/// Human prompt for QA generation (for chat models).
const HUMAN_PROMPT: &str = r"Please come up with a question/answer pair, in the specified JSON format, for the following text:
----------------
{text}";

/// A question-answer pair generated from text.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct QAPair {
    /// The generated question
    pub question: String,

    /// The answer to the question
    pub answer: String,
}

impl QAPair {
    /// Create a new QA pair.
    pub fn new(question: impl Into<String>, answer: impl Into<String>) -> Self {
        QAPair {
            question: question.into(),
            answer: answer.into(),
        }
    }
}

/// Chain for generating question-answer pairs from text documents.
///
/// This chain splits text into chunks and generates QA pairs for each chunk
/// using an LLM. The output is structured JSON that can be used for testing,
/// training, or evaluation purposes.
///
/// # Type Parameter
///
/// - `M`: Language model type (must implement `LLM` or `ChatModel`)
///
/// # Example
///
/// ```rust,ignore
/// use dashflow_chains::qa_generation::QAGenerationChain;
///
/// // Create chain with default text splitter
/// let chain = QAGenerationChain::from_llm(llm, None);
///
/// // Generate QA pairs from text
/// let text = "Rust is a systems programming language...";
/// let qa_pairs = chain.generate(text).await?;
///
/// // Limit number of questions
/// let chain_with_k = QAGenerationChain::from_llm(llm, None).with_k(3);
/// let limited_pairs = chain_with_k.generate(text).await?;
/// ```
pub struct QAGenerationChain<M>
where
    M: Send + Sync,
{
    /// Language model to use for generating QA pairs
    model: Arc<M>,

    /// Prompt template for generating QA pairs
    prompt: PromptTemplate,

    /// Text splitter for chunking input text
    text_splitter: RecursiveCharacterTextSplitter,

    /// Optional limit on number of QA pairs to generate
    k: Option<usize>,
}

impl<M> QAGenerationChain<M>
where
    M: Send + Sync,
{
    /// Create a new QA generation chain.
    ///
    /// # Arguments
    ///
    /// * `model` - Language model to use
    /// * `prompt` - Prompt template for QA generation
    /// * `text_splitter` - Text splitter for chunking documents
    pub fn new(
        model: M,
        prompt: PromptTemplate,
        text_splitter: RecursiveCharacterTextSplitter,
    ) -> Self {
        QAGenerationChain {
            model: Arc::new(model),
            prompt,
            text_splitter,
            k: None,
        }
    }

    /// Set the maximum number of QA pairs to generate.
    #[must_use]
    pub fn with_k(mut self, k: usize) -> Self {
        self.k = Some(k);
        self
    }

    /// Set a custom prompt template.
    #[must_use]
    pub fn with_prompt(mut self, prompt: PromptTemplate) -> Self {
        self.prompt = prompt;
        self
    }

    /// Set a custom text splitter.
    #[must_use]
    pub fn with_text_splitter(mut self, splitter: RecursiveCharacterTextSplitter) -> Self {
        self.text_splitter = splitter;
        self
    }

    /// Parse JSON output from LLM into QA pair.
    fn parse_qa_from_json(&self, text: &str) -> Result<QAPair> {
        // Extract JSON from markdown code blocks if present
        let json_text = if let Some(start) = text.find("```") {
            // Find the content between ``` markers
            let after_start = &text[start + 3..];
            // Skip optional "json" language identifier
            let content_start = after_start.find('\n').map_or(0, |i| i + 1);
            let content = &after_start[content_start..];

            if let Some(end) = content.find("```") {
                &content[..end]
            } else {
                content
            }
        } else {
            text
        };

        // Parse JSON
        serde_json::from_str::<QAPair>(json_text.trim()).map_err(|e| {
            Error::Other(format!(
                "Failed to parse QA pair from JSON: {e}. Text: {json_text}"
            ))
        })
    }
}

/// Implementation for LLM-based QA generation chains
impl<M> QAGenerationChain<M>
where
    M: LLM + Send + Sync + 'static,
{
    /// Create a QA generation chain from an LLM.
    ///
    /// # Arguments
    ///
    /// * `model` - Language model to use
    /// * `text_splitter` - Optional text splitter (uses default if None)
    pub fn from_llm(model: M, text_splitter: Option<RecursiveCharacterTextSplitter>) -> Self {
        // SAFETY: M-347 - DEFAULT_QA_GEN_PROMPT is a compile-time constant template
        #[allow(clippy::expect_used)]
        let prompt = PromptTemplate::from_template(DEFAULT_QA_GEN_PROMPT)
            .expect("DEFAULT_QA_GEN_PROMPT is a valid template");
        let splitter = text_splitter.unwrap_or_else(|| {
            RecursiveCharacterTextSplitter::new()
                .with_chunk_size(1000)
                .with_chunk_overlap(500)
        });

        QAGenerationChain::new(model, prompt, splitter)
    }

    /// Generate QA pairs from text.
    ///
    /// # Arguments
    ///
    /// * `text` - Input text to generate QA pairs from
    ///
    /// # Returns
    ///
    /// List of generated QA pairs
    pub async fn generate(&self, text: &str) -> Result<Vec<QAPair>> {
        // Split text into chunks
        let docs = self
            .text_splitter
            .create_documents(&[text.to_string()], None);

        // Limit number of chunks if k is set
        let docs_to_process = if let Some(k) = self.k {
            docs.into_iter().take(k).collect::<Vec<_>>()
        } else {
            docs
        };

        // Generate prompts for each chunk
        let prompts: Vec<String> = docs_to_process
            .iter()
            .map(|doc| {
                let mut inputs = std::collections::HashMap::new();
                inputs.insert("text".to_string(), doc.page_content.clone());
                self.prompt
                    .format(&inputs)
                    .unwrap_or_else(|_| String::new())
            })
            .collect();

        // Generate QA pairs using LLM
        let result = self.model.generate(&prompts, None, None).await?;

        // Parse QA pairs from results
        let mut qa_pairs = Vec::new();
        for generation in result.generations {
            if let Some(gen) = generation.first() {
                match self.parse_qa_from_json(&gen.text) {
                    Ok(qa) => qa_pairs.push(qa),
                    Err(e) => {
                        warn!("Failed to parse QA pair: {e}");
                        // Continue with other pairs rather than failing completely
                    }
                }
            }
        }

        Ok(qa_pairs)
    }
}

/// Implementation for ChatModel-based QA generation chains
impl<M> QAGenerationChain<M>
where
    M: ChatModel + Send + Sync + 'static,
{
    /// Create a QA generation chain from a chat model.
    ///
    /// # Arguments
    ///
    /// * `model` - Chat model to use
    /// * `text_splitter` - Optional text splitter (uses default if None)
    pub fn from_chat_model(
        model: M,
        text_splitter: Option<RecursiveCharacterTextSplitter>,
    ) -> Self {
        // For chat models, we still use the simple prompt template
        // The chat-specific prompting will be handled by the model
        // SAFETY: M-347 - HUMAN_PROMPT is a compile-time constant template
        #[allow(clippy::expect_used)]
        let prompt = PromptTemplate::from_template(HUMAN_PROMPT)
            .expect("HUMAN_PROMPT is a valid template");
        let splitter = text_splitter.unwrap_or_else(|| {
            RecursiveCharacterTextSplitter::new()
                .with_chunk_size(1000)
                .with_chunk_overlap(500)
        });

        QAGenerationChain::new(model, prompt, splitter)
    }

    /// Generate QA pairs from text using a chat model.
    pub async fn generate_chat(&self, text: &str) -> Result<Vec<QAPair>> {
        use dashflow::core::messages::{BaseMessage, MessageContent};

        // Split text into chunks
        let docs = self
            .text_splitter
            .create_documents(&[text.to_string()], None);

        // Limit number of chunks if k is set
        let docs_to_process = if let Some(k) = self.k {
            docs.into_iter().take(k).collect::<Vec<_>>()
        } else {
            docs
        };

        // Generate QA pairs for each chunk
        let mut qa_pairs = Vec::new();

        for doc in docs_to_process {
            // Format the human prompt with the document content
            let mut inputs = std::collections::HashMap::new();
            inputs.insert("text".to_string(), doc.page_content.clone());
            let human_text = self
                .prompt
                .format(&inputs)
                .unwrap_or_else(|_| String::new());

            // Create messages: system + human
            let messages = vec![
                BaseMessage::System {
                    content: MessageContent::Text(SYSTEM_PROMPT.to_string()),
                    fields: Default::default(),
                },
                BaseMessage::Human {
                    content: MessageContent::Text(human_text),
                    fields: Default::default(),
                },
            ];

            // Generate response using chat model
            let result = self
                .model
                .generate(&messages, None, None, None, None)
                .await?;

            // Parse QA pair from the first generation
            if let Some(gen) = result.generations.first() {
                match self.parse_qa_from_json(&gen.text()) {
                    Ok(qa) => qa_pairs.push(qa),
                    Err(e) => {
                        warn!("Failed to parse QA pair: {e}");
                        // Continue with other pairs rather than failing completely
                    }
                }
            }
        }

        Ok(qa_pairs)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use dashflow::core::{
        error::Result,
        language_models::{Generation, LLMResult, LLM},
    };

    // Mock LLM that returns valid JSON QA pairs
    struct MockQAGenerationLLM;

    #[async_trait]
    impl LLM for MockQAGenerationLLM {
        async fn _generate(
            &self,
            prompts: &[String],
            _stop: Option<&[String]>,
            _run_manager: Option<&dashflow::core::callbacks::CallbackManager>,
        ) -> Result<LLMResult> {
            let generations: Vec<Vec<Generation>> = prompts
                .iter()
                .enumerate()
                .map(|(i, _)| {
                    vec![Generation::new(format!(
                        r#"```
{{
    "question": "What is mentioned in passage {}?",
    "answer": "The passage discusses important concepts."
}}
```"#,
                        i + 1
                    ))]
                })
                .collect();

            Ok(LLMResult::with_prompts(generations))
        }

        fn llm_type(&self) -> &str {
            "mock"
        }
    }

    #[tokio::test]
    async fn test_qa_generation_basic() {
        let llm = MockQAGenerationLLM;
        let chain = QAGenerationChain::from_llm(llm, None);

        let text = "Rust is a systems programming language. It provides memory safety without garbage collection. Rust has a strong type system.";
        let qa_pairs = chain.generate(text).await.unwrap();

        assert!(!qa_pairs.is_empty());
        assert!(!qa_pairs[0].question.is_empty());
        assert!(!qa_pairs[0].answer.is_empty());
    }

    #[tokio::test]
    async fn test_qa_generation_with_k_limit() {
        let llm = MockQAGenerationLLM;
        let chain = QAGenerationChain::from_llm(llm, None).with_k(2);

        // Create longer text that would generate more chunks
        let text = "Part 1. ".repeat(200) + &"Part 2. ".repeat(200);
        let qa_pairs = chain.generate(&text).await.unwrap();

        // Should be limited to k=2 QA pairs
        assert!(qa_pairs.len() <= 2);
    }

    #[tokio::test]
    async fn test_qa_generation_parse_json() {
        let llm = MockQAGenerationLLM;
        let splitter = RecursiveCharacterTextSplitter::new()
            .with_chunk_size(1000)
            .with_chunk_overlap(500);
        let prompt = PromptTemplate::from_template(DEFAULT_QA_GEN_PROMPT).unwrap();
        let chain = QAGenerationChain::new(llm, prompt, splitter);

        // Test parsing with markdown code blocks
        let json_with_markdown = r#"```
{
    "question": "Test question?",
    "answer": "Test answer"
}
```"#;
        let qa = chain.parse_qa_from_json(json_with_markdown).unwrap();
        assert_eq!(qa.question, "Test question?");
        assert_eq!(qa.answer, "Test answer");

        // Test parsing without markdown
        let json_without_markdown = r#"{
    "question": "Another question?",
    "answer": "Another answer"
}"#;
        let qa2 = chain.parse_qa_from_json(json_without_markdown).unwrap();
        assert_eq!(qa2.question, "Another question?");
        assert_eq!(qa2.answer, "Another answer");
    }

    #[tokio::test]
    async fn test_qa_generation_empty_text() {
        let llm = MockQAGenerationLLM;
        let chain = QAGenerationChain::from_llm(llm, None);

        let qa_pairs = chain.generate("").await.unwrap();
        // Empty text should produce no QA pairs
        assert!(qa_pairs.is_empty());
    }

    #[tokio::test]
    async fn test_qa_generation_short_text() {
        let llm = MockQAGenerationLLM;
        let chain = QAGenerationChain::from_llm(llm, None);

        let text = "Short text.";
        let qa_pairs = chain.generate(text).await.unwrap();

        // Even short text should generate at least one QA pair
        assert!(!qa_pairs.is_empty());
    }

    #[tokio::test]
    async fn test_qa_pair_creation() {
        let qa = QAPair::new("What is Rust?", "A programming language");
        assert_eq!(qa.question, "What is Rust?");
        assert_eq!(qa.answer, "A programming language");
    }

    #[tokio::test]
    async fn test_qa_pair_serialization() {
        let qa = QAPair::new("Test question", "Test answer");
        let json = serde_json::to_string(&qa).unwrap();
        let deserialized: QAPair = serde_json::from_str(&json).unwrap();
        assert_eq!(qa, deserialized);
    }
}
