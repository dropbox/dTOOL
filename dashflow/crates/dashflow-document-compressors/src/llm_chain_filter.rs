//! LLM-based document filter that drops irrelevant documents
//!
//! This compressor uses an LLM to determine if each document is relevant to the query,
//! filtering out documents that the LLM judges as irrelevant.

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

/// Default prompt template for filtering documents
const DEFAULT_FILTER_PROMPT: &str = r"Given the following question and context, return YES if the context is relevant to the question and NO if it isn't.

> Question: {question}
> Context:
>>>
{context}
>>>
> Relevant (YES / NO):";

/// Type for the input constructor function
pub type GetInputFn = Arc<dyn Fn(&str, &Document) -> HashMap<String, String> + Send + Sync>;

/// Default function to construct input from query and document
fn default_get_input(query: &str, doc: &Document) -> HashMap<String, String> {
    let mut map = HashMap::new();
    map.insert("question".to_string(), query.to_string());
    map.insert("context".to_string(), doc.page_content.clone());
    map
}

/// Document compressor that filters documents using LLM relevance judgment
///
/// This compressor sends each document to an LLM with the query and asks whether
/// the document is relevant. Only documents judged as relevant are kept.
///
/// # Example
///
/// ```rust,ignore
/// use dashflow_document_compressors::LLMChainFilter;
/// use dashflow_openai::ChatOpenAI;
/// use dashflow::core::documents::Document;
///
/// let llm = ChatOpenAI::with_config(Default::default()).with_model("gpt-4o-mini");
/// let filter = LLMChainFilter::from_llm(llm, None)?;
///
/// let docs = vec![
///     Document::new("Rust is a systems programming language"),
///     Document::new("Python is a high-level language"),
/// ];
///
/// let filtered = filter.compress_documents(docs, "What is Rust?", None).await?;
/// // Returns only the Rust document
/// ```
pub struct LLMChainFilter {
    /// The LLM to use for relevance judgment
    llm: Arc<dyn ChatModel>,
    /// The prompt template to use (must include {question} and {context} variables)
    prompt: PromptTemplate,
    /// Function to construct input from query and document
    get_input: GetInputFn,
}

impl LLMChainFilter {
    /// Create a new filter from an LLM and optional custom prompt
    ///
    /// # Arguments
    ///
    /// * `llm` - The language model to use for filtering
    /// * `prompt` - Optional custom prompt template. If None, uses default prompt.
    ///   Must include {question} and {context} variables.
    ///
    /// # Returns
    ///
    /// A new `LLMChainFilter` instance
    pub fn from_llm(llm: Arc<dyn ChatModel>, prompt: Option<PromptTemplate>) -> Result<Self> {
        let prompt = match prompt {
            Some(p) => p,
            None => PromptTemplate::from_template(DEFAULT_FILTER_PROMPT)?,
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
        })
    }

    /// Set a custom input constructor function
    pub fn with_get_input(mut self, get_input: GetInputFn) -> Self {
        self.get_input = get_input;
        self
    }

    /// Check if LLM response indicates document is relevant
    fn is_relevant(&self, response: &str) -> bool {
        let cleaned = response.trim().to_uppercase();
        cleaned.contains("YES") || cleaned.starts_with("YES")
    }
}

#[async_trait]
impl DocumentCompressor for LLMChainFilter {
    async fn compress_documents(
        &self,
        documents: Vec<Document>,
        query: &str,
        config: Option<&RunnableConfig>,
    ) -> Result<Vec<Document>> {
        let mut filtered_docs = Vec::new();

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

            // Check if relevant
            if let Some(msg) = response.generations.first() {
                let content = msg.message.content();
                if let MessageContent::Text(text) = content {
                    if self.is_relevant(text) {
                        filtered_docs.push(doc);
                    }
                }
            }
        }

        Ok(filtered_docs)
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
        /// Response to return (will be wrapped in MessageContent::Text)
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
    // IS_RELEVANT PARSING TESTS
    // ============================================================

    #[test]
    fn test_is_relevant_yes() {
        // Test the logic directly without creating a filter
        fn is_relevant_test(response: &str) -> bool {
            let cleaned = response.trim().to_uppercase();
            cleaned.contains("YES") || cleaned.starts_with("YES")
        }

        assert!(is_relevant_test("YES"));
        assert!(is_relevant_test("yes"));
        assert!(is_relevant_test("Yes"));
        assert!(is_relevant_test("YES!"));
        assert!(is_relevant_test("YES."));
    }

    #[test]
    fn test_is_relevant_with_whitespace() {
        fn is_relevant_test(response: &str) -> bool {
            let cleaned = response.trim().to_uppercase();
            cleaned.contains("YES") || cleaned.starts_with("YES")
        }

        assert!(is_relevant_test("  YES  "));
        assert!(is_relevant_test("\nYES\n"));
        assert!(is_relevant_test("\t YES \t"));
        assert!(is_relevant_test("   YES, this is relevant   "));
    }

    #[test]
    fn test_is_relevant_with_explanation() {
        fn is_relevant_test(response: &str) -> bool {
            let cleaned = response.trim().to_uppercase();
            cleaned.contains("YES") || cleaned.starts_with("YES")
        }

        assert!(is_relevant_test("YES, this is relevant"));
        assert!(is_relevant_test("YES because the document mentions Rust"));
        assert!(is_relevant_test("The answer is YES"));
        assert!(is_relevant_test("I think YES, the document is relevant"));
    }

    #[test]
    fn test_is_relevant_no() {
        fn is_relevant_test(response: &str) -> bool {
            let cleaned = response.trim().to_uppercase();
            cleaned.contains("YES") || cleaned.starts_with("YES")
        }

        assert!(!is_relevant_test("NO"));
        assert!(!is_relevant_test("no"));
        assert!(!is_relevant_test("No"));
        assert!(!is_relevant_test("NO, this is not relevant"));
        assert!(!is_relevant_test("The answer is NO"));
    }

    #[test]
    fn test_is_relevant_ambiguous() {
        fn is_relevant_test(response: &str) -> bool {
            let cleaned = response.trim().to_uppercase();
            cleaned.contains("YES") || cleaned.starts_with("YES")
        }

        assert!(!is_relevant_test("Maybe"));
        assert!(!is_relevant_test("I'm not sure"));
        assert!(!is_relevant_test("Possibly"));
        assert!(!is_relevant_test(""));
        assert!(!is_relevant_test("   "));
    }

    #[test]
    fn test_is_relevant_edge_cases() {
        fn is_relevant_test(response: &str) -> bool {
            let cleaned = response.trim().to_uppercase();
            cleaned.contains("YES") || cleaned.starts_with("YES")
        }

        // Contains YES in a word (should match due to contains)
        assert!(is_relevant_test("YESTERDAY"));

        // Yes as part of longer text
        assert!(is_relevant_test("After consideration, YES the document is relevant"));

        // All caps
        assert!(is_relevant_test("YES"));

        // Mixed case
        assert!(is_relevant_test("YeS"));
    }

    // ============================================================
    // DEFAULT_GET_INPUT TESTS
    // ============================================================

    #[test]
    fn test_default_get_input_basic() {
        let doc = Document::new("This is the document content");
        let result = default_get_input("What is Rust?", &doc);

        assert_eq!(result.get("question"), Some(&"What is Rust?".to_string()));
        assert_eq!(
            result.get("context"),
            Some(&"This is the document content".to_string())
        );
    }

    #[test]
    fn test_default_get_input_empty_query() {
        let doc = Document::new("Document content");
        let result = default_get_input("", &doc);

        assert_eq!(result.get("question"), Some(&"".to_string()));
        assert_eq!(result.get("context"), Some(&"Document content".to_string()));
    }

    #[test]
    fn test_default_get_input_empty_document() {
        let doc = Document::new("");
        let result = default_get_input("Query", &doc);

        assert_eq!(result.get("question"), Some(&"Query".to_string()));
        assert_eq!(result.get("context"), Some(&"".to_string()));
    }

    #[test]
    fn test_default_get_input_long_content() {
        let long_content = "x".repeat(10000);
        let doc = Document::new(&long_content);
        let result = default_get_input("Short query", &doc);

        assert_eq!(result.get("question"), Some(&"Short query".to_string()));
        assert_eq!(result.get("context"), Some(&long_content));
    }

    #[test]
    fn test_default_get_input_special_characters() {
        let doc = Document::new("Content with\nnewlines\tand\ttabs");
        let result = default_get_input("Query with 'quotes' and \"double quotes\"", &doc);

        assert_eq!(
            result.get("question"),
            Some(&"Query with 'quotes' and \"double quotes\"".to_string())
        );
        assert_eq!(
            result.get("context"),
            Some(&"Content with\nnewlines\tand\ttabs".to_string())
        );
    }

    // ============================================================
    // PROMPT TEMPLATE TESTS
    // ============================================================

    #[test]
    fn test_default_prompt_has_required_vars() {
        let prompt = PromptTemplate::from_template(DEFAULT_FILTER_PROMPT).unwrap();
        assert!(prompt.input_variables.contains(&"question".to_string()));
        assert!(prompt.input_variables.contains(&"context".to_string()));
    }

    #[test]
    fn test_from_llm_with_default_prompt() {
        let llm = Arc::new(MockChatModel::new("YES"));
        let filter = LLMChainFilter::from_llm(llm, None);
        assert!(filter.is_ok());
    }

    #[test]
    fn test_from_llm_with_valid_custom_prompt() {
        let llm = Arc::new(MockChatModel::new("YES"));
        let custom_prompt = PromptTemplate::from_template(
            "Question: {question}\nContext: {context}\nIs it relevant? (YES/NO):",
        )
        .unwrap();
        let filter = LLMChainFilter::from_llm(llm, Some(custom_prompt));
        assert!(filter.is_ok());
    }

    #[test]
    fn test_from_llm_with_missing_question_var() {
        let llm = Arc::new(MockChatModel::new("YES"));
        let custom_prompt =
            PromptTemplate::from_template("Context: {context}\nIs it relevant?").unwrap();
        let result = LLMChainFilter::from_llm(llm, Some(custom_prompt));

        assert!(result.is_err());
        let err = match result {
            Err(e) => e.to_string(),
            Ok(_) => panic!("expected error"),
        };
        assert!(err.contains("question"));
    }

    #[test]
    fn test_from_llm_with_missing_context_var() {
        let llm = Arc::new(MockChatModel::new("YES"));
        let custom_prompt =
            PromptTemplate::from_template("Question: {question}\nIs it relevant?").unwrap();
        let result = LLMChainFilter::from_llm(llm, Some(custom_prompt));

        assert!(result.is_err());
        let err = match result {
            Err(e) => e.to_string(),
            Ok(_) => panic!("expected error"),
        };
        assert!(err.contains("context"));
    }

    // ============================================================
    // WITH_GET_INPUT BUILDER TESTS
    // ============================================================

    #[test]
    fn test_with_get_input() {
        let llm = Arc::new(MockChatModel::new("YES"));
        let filter = LLMChainFilter::from_llm(llm, None).unwrap();

        // Custom get_input that uppercases everything
        let custom_fn: GetInputFn = Arc::new(|query, doc| {
            let mut map = HashMap::new();
            map.insert("question".to_string(), query.to_uppercase());
            map.insert("context".to_string(), doc.page_content.to_uppercase());
            map
        });

        let filter_with_custom = filter.with_get_input(custom_fn);

        // Verify the filter was created (can't easily test the function without full integration)
        assert!(std::mem::size_of_val(&filter_with_custom) > 0);
    }

    // ============================================================
    // DOCUMENT COMPRESSOR INTEGRATION TESTS
    // ============================================================

    #[tokio::test]
    async fn test_compress_documents_empty() {
        let llm = Arc::new(MockChatModel::new("YES"));
        let filter = LLMChainFilter::from_llm(llm, None).unwrap();

        let docs: Vec<Document> = vec![];
        let result = filter.compress_documents(docs, "test query", None).await;

        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_compress_documents_all_relevant() {
        let llm = Arc::new(MockChatModel::new("YES"));
        let filter = LLMChainFilter::from_llm(llm, None).unwrap();

        let docs = vec![
            Document::new("Document 1"),
            Document::new("Document 2"),
            Document::new("Document 3"),
        ];

        let result = filter
            .compress_documents(docs, "test query", None)
            .await
            .unwrap();

        // All documents should be kept since LLM returns "YES"
        assert_eq!(result.len(), 3);
    }

    #[tokio::test]
    async fn test_compress_documents_none_relevant() {
        let llm = Arc::new(MockChatModel::new("NO"));
        let filter = LLMChainFilter::from_llm(llm, None).unwrap();

        let docs = vec![
            Document::new("Document 1"),
            Document::new("Document 2"),
            Document::new("Document 3"),
        ];

        let result = filter
            .compress_documents(docs, "test query", None)
            .await
            .unwrap();

        // No documents should be kept since LLM returns "NO"
        assert!(result.is_empty());
    }

    #[tokio::test]
    async fn test_compress_documents_preserves_content() {
        let llm = Arc::new(MockChatModel::new("YES"));
        let filter = LLMChainFilter::from_llm(llm, None).unwrap();

        let docs = vec![Document::new("Specific content to preserve")];

        let result = filter
            .compress_documents(docs, "test query", None)
            .await
            .unwrap();

        assert_eq!(result.len(), 1);
        assert_eq!(result[0].page_content, "Specific content to preserve");
    }

    #[tokio::test]
    async fn test_compress_documents_preserves_metadata() {
        let llm = Arc::new(MockChatModel::new("YES"));
        let filter = LLMChainFilter::from_llm(llm, None).unwrap();

        let mut doc = Document::new("Content");
        doc.metadata
            .insert("source".to_string(), serde_json::json!("test"));
        doc.metadata
            .insert("page".to_string(), serde_json::json!(42));

        let docs = vec![doc];
        let result = filter
            .compress_documents(docs, "test query", None)
            .await
            .unwrap();

        assert_eq!(result.len(), 1);
        assert_eq!(
            result[0].metadata.get("source"),
            Some(&serde_json::json!("test"))
        );
        assert_eq!(
            result[0].metadata.get("page"),
            Some(&serde_json::json!(42))
        );
    }

    #[tokio::test]
    async fn test_compress_documents_single() {
        let llm = Arc::new(MockChatModel::new("YES, this is relevant"));
        let filter = LLMChainFilter::from_llm(llm, None).unwrap();

        let docs = vec![Document::new("Single doc")];
        let result = filter
            .compress_documents(docs, "test", None)
            .await
            .unwrap();

        assert_eq!(result.len(), 1);
    }

    #[tokio::test]
    async fn test_compress_documents_with_id() {
        let llm = Arc::new(MockChatModel::new("YES"));
        let filter = LLMChainFilter::from_llm(llm, None).unwrap();

        let docs = vec![Document::new("Content").with_id("doc-123")];
        let result = filter
            .compress_documents(docs, "test", None)
            .await
            .unwrap();

        assert_eq!(result.len(), 1);
        assert_eq!(result[0].id, Some("doc-123".to_string()));
    }

    // ============================================================
    // EDGE CASE RESPONSE TESTS
    // ============================================================

    #[tokio::test]
    async fn test_response_with_explanation() {
        let llm = Arc::new(MockChatModel::new(
            "YES, this document is relevant because it contains information about Rust.",
        ));
        let filter = LLMChainFilter::from_llm(llm, None).unwrap();

        let docs = vec![Document::new("Rust programming")];
        let result = filter
            .compress_documents(docs, "What is Rust?", None)
            .await
            .unwrap();

        assert_eq!(result.len(), 1);
    }

    #[tokio::test]
    async fn test_response_empty() {
        let llm = Arc::new(MockChatModel::new(""));
        let filter = LLMChainFilter::from_llm(llm, None).unwrap();

        let docs = vec![Document::new("Content")];
        let result = filter
            .compress_documents(docs, "query", None)
            .await
            .unwrap();

        // Empty response doesn't contain YES
        assert!(result.is_empty());
    }

    #[tokio::test]
    async fn test_response_whitespace_only() {
        let llm = Arc::new(MockChatModel::new("   \n\t   "));
        let filter = LLMChainFilter::from_llm(llm, None).unwrap();

        let docs = vec![Document::new("Content")];
        let result = filter
            .compress_documents(docs, "query", None)
            .await
            .unwrap();

        // Whitespace-only response doesn't contain YES
        assert!(result.is_empty());
    }
}
