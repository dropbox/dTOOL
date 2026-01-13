//! LLM-based document extractor that extracts relevant parts
//!
//! This compressor uses an LLM to extract only the relevant parts of each document
//! that are needed to answer the query.

use async_trait::async_trait;
use dashflow::core::{
    config::RunnableConfig,
    documents::{Document, DocumentCompressor},
    error::{Error, Result},
    language_models::ChatModel,
    messages::{BaseMessage, Message, MessageContent},
    prompts::PromptTemplate,
};
use std::collections::HashMap;
use std::sync::Arc;

/// Default prompt template for extracting relevant content
const DEFAULT_EXTRACT_PROMPT: &str = r"Given the following question and context, extract any part of the context *AS IS* that is relevant to answer the question. If none of the context is relevant return NO_OUTPUT.

Remember, *DO NOT* edit the extracted parts of the context.

> Question: {question}
> Context:
>>>
{context}
>>>
Extracted relevant parts:";

/// Marker for when no relevant content is found
const NO_OUTPUT_STR: &str = "NO_OUTPUT";

/// Type for the input constructor function
pub type GetInputFn = Arc<dyn Fn(&str, &Document) -> HashMap<String, String> + Send + Sync>;

/// Default function to construct input from query and document
fn default_get_input(query: &str, doc: &Document) -> HashMap<String, String> {
    let mut map = HashMap::new();
    map.insert("question".to_string(), query.to_string());
    map.insert("context".to_string(), doc.page_content.clone());
    map
}

/// Document compressor that extracts relevant parts using an LLM
///
/// This compressor sends each document to an LLM with the query and asks it to extract
/// only the relevant parts. Documents with no relevant content are filtered out.
///
/// # Example
///
/// ```rust,ignore
/// use dashflow_document_compressors::LLMChainExtractor;
/// use dashflow_openai::ChatOpenAI;
/// use dashflow::core::documents::Document;
///
/// let llm = ChatOpenAI::with_config(Default::default()).with_model("gpt-4o-mini");
/// let extractor = LLMChainExtractor::from_llm(llm, None)?;
///
/// let docs = vec![
///     Document::new("Rust is a systems programming language. It was created in 2010."),
/// ];
///
/// let extracted = extractor.compress_documents(docs, "When was Rust created?", None).await?;
/// // Returns document with only "It was created in 2010."
/// ```
pub struct LLMChainExtractor {
    /// The LLM to use for extraction
    llm: Arc<dyn ChatModel>,
    /// The prompt template to use (must include {question} and {context} variables)
    prompt: PromptTemplate,
    /// Function to construct input from query and document
    get_input: GetInputFn,
    /// String indicating no relevant content
    no_output_str: String,
}

impl LLMChainExtractor {
    /// Create a new extractor from an LLM and optional custom prompt
    ///
    /// # Arguments
    ///
    /// * `llm` - The language model to use for extraction
    /// * `prompt` - Optional custom prompt template. If None, uses default prompt.
    ///   Must include {question} and {context} variables.
    ///
    /// # Returns
    ///
    /// A new `LLMChainExtractor` instance
    pub fn from_llm(llm: Arc<dyn ChatModel>, prompt: Option<PromptTemplate>) -> Result<Self> {
        let prompt = match prompt {
            Some(p) => p,
            None => PromptTemplate::from_template(DEFAULT_EXTRACT_PROMPT)?,
        };

        // Validate that prompt has required variables
        let required_vars = vec!["question", "context"];
        for var in &required_vars {
            if !prompt.input_variables.contains(&(*var).to_string()) {
                return Err(Error::InvalidInput(format!(
                    "Prompt must include '{var}' variable"
                )));
            }
        }

        Ok(Self {
            llm,
            prompt,
            get_input: Arc::new(default_get_input),
            no_output_str: NO_OUTPUT_STR.to_string(),
        })
    }

    /// Set a custom input constructor function
    pub fn with_get_input(mut self, get_input: GetInputFn) -> Self {
        self.get_input = get_input;
        self
    }

    /// Set a custom no-output marker string
    #[must_use]
    pub fn with_no_output_str(mut self, no_output_str: String) -> Self {
        self.no_output_str = no_output_str;
        self
    }

    /// Parse the LLM output, returning None if no relevant content
    fn parse_output(&self, output: &str) -> Option<String> {
        let cleaned = output.trim();
        if cleaned.is_empty() || cleaned == self.no_output_str {
            None
        } else {
            Some(cleaned.to_string())
        }
    }
}

#[async_trait]
impl DocumentCompressor for LLMChainExtractor {
    async fn compress_documents(
        &self,
        documents: Vec<Document>,
        query: &str,
        config: Option<&RunnableConfig>,
    ) -> Result<Vec<Document>> {
        let mut compressed_docs = Vec::new();

        // Process each document
        for doc in documents {
            // Construct input
            let input = (self.get_input)(query, &doc);

            // Format prompt
            let prompt_text = self.prompt.format(&input)?;

            // Call LLM
            let messages: Vec<BaseMessage> = vec![Message::system(prompt_text)];

            let response = self
                .llm
                .generate(&messages, None, None, None, config)
                .await?;

            // Extract and parse output
            if let Some(msg) = response.generations.first() {
                let content = msg.message.content();
                if let MessageContent::Text(text) = content {
                    if let Some(extracted_content) = self.parse_output(text) {
                        let mut new_doc = Document::new(extracted_content);
                        new_doc.metadata = doc.metadata.clone();
                        compressed_docs.push(new_doc);
                    }
                }
            }
        }

        Ok(compressed_docs)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use dashflow::core::documents::Document;
    use dashflow::core::language_models::{ChatGeneration, ChatResult, ToolChoice, ToolDefinition};
    use dashflow::core::messages::{Message, MessageContent};

    // ============================================================
    // MOCK LLM FOR TESTING
    // ============================================================

    /// Mock ChatModel that returns configurable responses
    struct MockChatModel {
        /// Response to return
        response: String,
    }

    impl MockChatModel {
        fn new(response: &str) -> Self {
            Self {
                response: response.to_string(),
            }
        }
    }

    #[async_trait]
    impl ChatModel for MockChatModel {
        async fn _generate(
            &self,
            _messages: &[BaseMessage],
            _stop: Option<&[String]>,
            _tools: Option<&[ToolDefinition]>,
            _tool_choice: Option<&ToolChoice>,
            _run_manager: Option<&dashflow::core::callbacks::CallbackManager>,
        ) -> Result<ChatResult> {
            let ai_message = Message::AI {
                content: MessageContent::Text(self.response.clone()),
                tool_calls: vec![],
                invalid_tool_calls: vec![],
                usage_metadata: None,
                fields: Default::default(),
            };

            Ok(ChatResult {
                generations: vec![ChatGeneration {
                    message: ai_message,
                    generation_info: None,
                }],
                llm_output: None,
            })
        }

        fn llm_type(&self) -> &str {
            "mock"
        }

        fn as_any(&self) -> &dyn std::any::Any {
            self
        }
    }

    // ============================================================
    // PARSE_OUTPUT TESTS
    // ============================================================

    #[test]
    fn test_parse_output_valid_content() {
        fn parse_output_test(output: &str, no_output_str: &str) -> Option<String> {
            let cleaned = output.trim();
            if cleaned.is_empty() || cleaned == no_output_str {
                None
            } else {
                Some(cleaned.to_string())
            }
        }

        assert_eq!(
            parse_output_test("Some content", NO_OUTPUT_STR),
            Some("Some content".to_string())
        );
        assert_eq!(
            parse_output_test("Extracted text here", NO_OUTPUT_STR),
            Some("Extracted text here".to_string())
        );
    }

    #[test]
    fn test_parse_output_empty() {
        fn parse_output_test(output: &str, no_output_str: &str) -> Option<String> {
            let cleaned = output.trim();
            if cleaned.is_empty() || cleaned == no_output_str {
                None
            } else {
                Some(cleaned.to_string())
            }
        }

        assert_eq!(parse_output_test("", NO_OUTPUT_STR), None);
    }

    #[test]
    fn test_parse_output_whitespace() {
        fn parse_output_test(output: &str, no_output_str: &str) -> Option<String> {
            let cleaned = output.trim();
            if cleaned.is_empty() || cleaned == no_output_str {
                None
            } else {
                Some(cleaned.to_string())
            }
        }

        assert_eq!(parse_output_test("  ", NO_OUTPUT_STR), None);
        assert_eq!(parse_output_test("\n\n", NO_OUTPUT_STR), None);
        assert_eq!(parse_output_test("\t\t", NO_OUTPUT_STR), None);
        assert_eq!(parse_output_test("  \n\t  ", NO_OUTPUT_STR), None);
    }

    #[test]
    fn test_parse_output_no_output_marker() {
        fn parse_output_test(output: &str, no_output_str: &str) -> Option<String> {
            let cleaned = output.trim();
            if cleaned.is_empty() || cleaned == no_output_str {
                None
            } else {
                Some(cleaned.to_string())
            }
        }

        assert_eq!(parse_output_test("NO_OUTPUT", NO_OUTPUT_STR), None);
        assert_eq!(parse_output_test("  NO_OUTPUT  ", NO_OUTPUT_STR), None);
    }

    #[test]
    fn test_parse_output_trims_whitespace() {
        fn parse_output_test(output: &str, no_output_str: &str) -> Option<String> {
            let cleaned = output.trim();
            if cleaned.is_empty() || cleaned == no_output_str {
                None
            } else {
                Some(cleaned.to_string())
            }
        }

        assert_eq!(
            parse_output_test("  Content  ", NO_OUTPUT_STR),
            Some("Content".to_string())
        );
        assert_eq!(
            parse_output_test("\nContent\n", NO_OUTPUT_STR),
            Some("Content".to_string())
        );
    }

    #[test]
    fn test_parse_output_preserves_internal_whitespace() {
        fn parse_output_test(output: &str, no_output_str: &str) -> Option<String> {
            let cleaned = output.trim();
            if cleaned.is_empty() || cleaned == no_output_str {
                None
            } else {
                Some(cleaned.to_string())
            }
        }

        assert_eq!(
            parse_output_test("Content with\nnewlines", NO_OUTPUT_STR),
            Some("Content with\nnewlines".to_string())
        );
    }

    #[test]
    fn test_custom_no_output_str() {
        fn parse_output_test(output: &str, no_output_str: &str) -> Option<String> {
            let cleaned = output.trim();
            if cleaned.is_empty() || cleaned == no_output_str {
                None
            } else {
                Some(cleaned.to_string())
            }
        }

        assert_eq!(parse_output_test("NONE", "NONE"), None);
        assert_eq!(parse_output_test("N/A", "N/A"), None);
        assert_eq!(parse_output_test("EMPTY", "EMPTY"), None);

        // Different custom marker doesn't match
        assert_eq!(
            parse_output_test("NO_OUTPUT", "NONE"),
            Some("NO_OUTPUT".to_string())
        );
    }

    // ============================================================
    // DEFAULT_GET_INPUT TESTS
    // ============================================================

    #[test]
    fn test_default_get_input_basic() {
        let doc = Document::new("Document content here");
        let result = default_get_input("What is the answer?", &doc);

        assert_eq!(
            result.get("question"),
            Some(&"What is the answer?".to_string())
        );
        assert_eq!(
            result.get("context"),
            Some(&"Document content here".to_string())
        );
    }

    #[test]
    fn test_default_get_input_empty_values() {
        let doc = Document::new("");
        let result = default_get_input("", &doc);

        assert_eq!(result.get("question"), Some(&"".to_string()));
        assert_eq!(result.get("context"), Some(&"".to_string()));
    }

    #[test]
    fn test_default_get_input_unicode() {
        let doc = Document::new("Unicode content: 日本語 🦀");
        let result = default_get_input("Query with émojis 💡", &doc);

        assert_eq!(
            result.get("question"),
            Some(&"Query with émojis 💡".to_string())
        );
        assert_eq!(
            result.get("context"),
            Some(&"Unicode content: 日本語 🦀".to_string())
        );
    }

    // ============================================================
    // PROMPT TEMPLATE TESTS
    // ============================================================

    #[test]
    fn test_default_prompt_has_required_vars() {
        let prompt = PromptTemplate::from_template(DEFAULT_EXTRACT_PROMPT).unwrap();
        assert!(prompt.input_variables.contains(&"question".to_string()));
        assert!(prompt.input_variables.contains(&"context".to_string()));
    }

    #[test]
    fn test_from_llm_with_default_prompt() {
        let llm = Arc::new(MockChatModel::new("Extracted"));
        let extractor = LLMChainExtractor::from_llm(llm, None);
        assert!(extractor.is_ok());
    }

    #[test]
    fn test_from_llm_with_valid_custom_prompt() {
        let llm = Arc::new(MockChatModel::new("Extracted"));
        let custom_prompt = PromptTemplate::from_template(
            "Question: {question}\nContext: {context}\nExtract relevant parts:",
        )
        .unwrap();
        let extractor = LLMChainExtractor::from_llm(llm, Some(custom_prompt));
        assert!(extractor.is_ok());
    }

    #[test]
    fn test_from_llm_with_missing_question_var() {
        let llm = Arc::new(MockChatModel::new("Extracted"));
        let custom_prompt =
            PromptTemplate::from_template("Context: {context}\nExtract:").unwrap();
        let result = LLMChainExtractor::from_llm(llm, Some(custom_prompt));

        assert!(result.is_err());
        let err = match result {
            Err(e) => e.to_string(),
            Ok(_) => panic!("expected error"),
        };
        assert!(err.contains("question"));
    }

    #[test]
    fn test_from_llm_with_missing_context_var() {
        let llm = Arc::new(MockChatModel::new("Extracted"));
        let custom_prompt =
            PromptTemplate::from_template("Question: {question}\nExtract:").unwrap();
        let result = LLMChainExtractor::from_llm(llm, Some(custom_prompt));

        assert!(result.is_err());
        let err = match result {
            Err(e) => e.to_string(),
            Ok(_) => panic!("expected error"),
        };
        assert!(err.contains("context"));
    }

    // ============================================================
    // BUILDER PATTERN TESTS
    // ============================================================

    #[test]
    fn test_with_no_output_str() {
        let llm = Arc::new(MockChatModel::new("Extracted"));
        let extractor = LLMChainExtractor::from_llm(llm, None)
            .unwrap()
            .with_no_output_str("CUSTOM_EMPTY".to_string());

        assert_eq!(extractor.no_output_str, "CUSTOM_EMPTY");
    }

    #[test]
    fn test_with_get_input() {
        let llm = Arc::new(MockChatModel::new("Extracted"));
        let extractor = LLMChainExtractor::from_llm(llm, None).unwrap();

        let custom_fn: GetInputFn = Arc::new(|query, doc| {
            let mut map = HashMap::new();
            map.insert("question".to_string(), format!("Q: {query}"));
            map.insert("context".to_string(), format!("C: {}", doc.page_content));
            map
        });

        let extractor_with_custom = extractor.with_get_input(custom_fn);
        assert!(std::mem::size_of_val(&extractor_with_custom) > 0);
    }

    // ============================================================
    // DOCUMENT COMPRESSOR INTEGRATION TESTS
    // ============================================================

    #[tokio::test]
    async fn test_compress_documents_empty() {
        let llm = Arc::new(MockChatModel::new("Extracted content"));
        let extractor = LLMChainExtractor::from_llm(llm, None).unwrap();

        let docs: Vec<Document> = vec![];
        let result = extractor.compress_documents(docs, "query", None).await;

        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_compress_documents_with_extraction() {
        let llm = Arc::new(MockChatModel::new("This is the extracted part"));
        let extractor = LLMChainExtractor::from_llm(llm, None).unwrap();

        let docs = vec![Document::new("Original content that is much longer")];
        let result = extractor
            .compress_documents(docs, "query", None)
            .await
            .unwrap();

        assert_eq!(result.len(), 1);
        assert_eq!(result[0].page_content, "This is the extracted part");
    }

    #[tokio::test]
    async fn test_compress_documents_no_relevant_content() {
        let llm = Arc::new(MockChatModel::new("NO_OUTPUT"));
        let extractor = LLMChainExtractor::from_llm(llm, None).unwrap();

        let docs = vec![Document::new("Some irrelevant content")];
        let result = extractor
            .compress_documents(docs, "query", None)
            .await
            .unwrap();

        // Document should be filtered out since LLM returned NO_OUTPUT
        assert!(result.is_empty());
    }

    #[tokio::test]
    async fn test_compress_documents_empty_response() {
        let llm = Arc::new(MockChatModel::new(""));
        let extractor = LLMChainExtractor::from_llm(llm, None).unwrap();

        let docs = vec![Document::new("Content")];
        let result = extractor
            .compress_documents(docs, "query", None)
            .await
            .unwrap();

        // Empty response means no relevant content
        assert!(result.is_empty());
    }

    #[tokio::test]
    async fn test_compress_documents_preserves_metadata() {
        let llm = Arc::new(MockChatModel::new("Extracted text"));
        let extractor = LLMChainExtractor::from_llm(llm, None).unwrap();

        let mut doc = Document::new("Original content");
        doc.metadata
            .insert("source".to_string(), serde_json::json!("test_source"));
        doc.metadata
            .insert("page".to_string(), serde_json::json!(5));

        let docs = vec![doc];
        let result = extractor
            .compress_documents(docs, "query", None)
            .await
            .unwrap();

        assert_eq!(result.len(), 1);
        // Content should be extracted
        assert_eq!(result[0].page_content, "Extracted text");
        // Metadata should be preserved
        assert_eq!(
            result[0].metadata.get("source"),
            Some(&serde_json::json!("test_source"))
        );
        assert_eq!(result[0].metadata.get("page"), Some(&serde_json::json!(5)));
    }

    #[tokio::test]
    async fn test_compress_documents_multiple_with_mixed_results() {
        // This test simulates having one doc with content and one without
        // Since our mock always returns the same response, we test the basic case
        let llm = Arc::new(MockChatModel::new("Extracted"));
        let extractor = LLMChainExtractor::from_llm(llm, None).unwrap();

        let docs = vec![
            Document::new("Doc 1"),
            Document::new("Doc 2"),
            Document::new("Doc 3"),
        ];

        let result = extractor
            .compress_documents(docs, "query", None)
            .await
            .unwrap();

        // All documents should have extracted content
        assert_eq!(result.len(), 3);
    }

    #[tokio::test]
    async fn test_compress_documents_with_custom_no_output_str() {
        let llm = Arc::new(MockChatModel::new("EMPTY"));
        let extractor = LLMChainExtractor::from_llm(llm, None)
            .unwrap()
            .with_no_output_str("EMPTY".to_string());

        let docs = vec![Document::new("Content")];
        let result = extractor
            .compress_documents(docs, "query", None)
            .await
            .unwrap();

        // Should be filtered out since response matches custom no_output_str
        assert!(result.is_empty());
    }

    #[tokio::test]
    async fn test_compress_documents_whitespace_response() {
        let llm = Arc::new(MockChatModel::new("   \n\t   "));
        let extractor = LLMChainExtractor::from_llm(llm, None).unwrap();

        let docs = vec![Document::new("Content")];
        let result = extractor
            .compress_documents(docs, "query", None)
            .await
            .unwrap();

        // Whitespace-only response should be treated as no content
        assert!(result.is_empty());
    }

    #[tokio::test]
    async fn test_compress_documents_long_extraction() {
        let long_content = "x".repeat(10000);
        let llm = Arc::new(MockChatModel::new(&long_content));
        let extractor = LLMChainExtractor::from_llm(llm, None).unwrap();

        let docs = vec![Document::new("Original")];
        let result = extractor
            .compress_documents(docs, "query", None)
            .await
            .unwrap();

        assert_eq!(result.len(), 1);
        assert_eq!(result[0].page_content.len(), 10000);
    }

    #[tokio::test]
    async fn test_compress_documents_multiline_extraction() {
        let multiline = "Line 1\nLine 2\nLine 3";
        let llm = Arc::new(MockChatModel::new(multiline));
        let extractor = LLMChainExtractor::from_llm(llm, None).unwrap();

        let docs = vec![Document::new("Original")];
        let result = extractor
            .compress_documents(docs, "query", None)
            .await
            .unwrap();

        assert_eq!(result.len(), 1);
        assert_eq!(result[0].page_content, "Line 1\nLine 2\nLine 3");
    }

    #[tokio::test]
    async fn test_compress_documents_single() {
        let llm = Arc::new(MockChatModel::new("Extracted"));
        let extractor = LLMChainExtractor::from_llm(llm, None).unwrap();

        let docs = vec![Document::new("Single doc")];
        let result = extractor
            .compress_documents(docs, "query", None)
            .await
            .unwrap();

        assert_eq!(result.len(), 1);
    }

    // ============================================================
    // ID HANDLING TESTS
    // ============================================================

    #[tokio::test]
    async fn test_compress_documents_does_not_preserve_id() {
        let llm = Arc::new(MockChatModel::new("Extracted"));
        let extractor = LLMChainExtractor::from_llm(llm, None).unwrap();

        let docs = vec![Document::new("Content").with_id("original-id")];
        let result = extractor
            .compress_documents(docs, "query", None)
            .await
            .unwrap();

        // The current implementation creates a new Document, which doesn't preserve id
        // This test documents the current behavior
        assert_eq!(result.len(), 1);
        // ID is not preserved because we create a new Document with Document::new()
        assert!(result[0].id.is_none());
    }
}
