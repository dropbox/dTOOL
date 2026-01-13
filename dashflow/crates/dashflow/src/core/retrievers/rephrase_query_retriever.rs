//! `RePhraseQueryRetriever` - Rephrases queries using an LLM before retrieval
//!
//! This retriever uses an LLM to rephrase/optimize a query before passing it
//! to an underlying retriever. This is useful for improving retrieval quality
//! by clarifying ambiguous queries or converting natural language to better
//! search terms.

use crate::core::{
    config::RunnableConfig, documents::Document, error::Error as DashFlowError,
    language_models::ChatModel, messages::Message, retrievers::Retriever,
};
use async_trait::async_trait;
use std::sync::Arc;

/// Default template for rephrasing queries
pub const DEFAULT_TEMPLATE: &str = "You are an assistant tasked with taking a natural language \
query from a user and converting it into a query for a vectorstore. \
In this process, you strip out information that is not relevant for \
the retrieval task. Here is the user query: {question}";

/// `RePhraseQueryRetriever` rephrases queries using an LLM before retrieval
///
/// This retriever wraps an existing retriever and uses an LLM to rephrase
/// the query before passing it to the underlying retriever. This can improve
/// retrieval quality by:
/// - Clarifying ambiguous queries
/// - Converting conversational queries to search-optimized terms
/// - Removing irrelevant context from the query
///
/// # Example
///
/// ```no_run
/// use dashflow::core::retrievers::{Retriever, VectorStoreRetriever, RePhraseQueryRetriever};
/// use dashflow::core::language_models::ChatModel;
/// use dashflow::core::documents::Document;
/// use std::sync::Arc;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// # let vector_store_retriever: Arc<dyn Retriever> = unimplemented!();
/// # let llm: Arc<dyn ChatModel> = unimplemented!();
/// let rephrase_retriever = RePhraseQueryRetriever::from_llm(
///     vector_store_retriever,
///     llm,
///     None, // Use default prompt
/// );
///
/// let docs = rephrase_retriever
///     ._get_relevant_documents("Tell me about cats, but also I like dogs", None)
///     .await?;
/// // LLM will rephrase to something like "cats" before querying the vector store
/// # Ok(())
/// # }
/// ```
pub struct RePhraseQueryRetriever {
    /// The underlying retriever to query with the rephrased query
    retriever: Arc<dyn Retriever>,
    /// The LLM to use for rephrasing queries
    llm: Arc<dyn ChatModel>,
    /// The prompt template to use for rephrasing (uses {question} placeholder)
    prompt_template: String,
}

impl RePhraseQueryRetriever {
    /// Create a new `RePhraseQueryRetriever` from an LLM and base retriever
    ///
    /// # Arguments
    ///
    /// * `retriever` - The underlying retriever to query
    /// * `llm` - The language model to use for rephrasing
    /// * `prompt_template` - Optional custom prompt template. If None, uses default.
    ///   The template should contain a `{question}` placeholder.
    pub fn from_llm(
        retriever: Arc<dyn Retriever>,
        llm: Arc<dyn ChatModel>,
        prompt_template: Option<String>,
    ) -> Self {
        Self {
            retriever,
            llm,
            prompt_template: prompt_template.unwrap_or_else(|| DEFAULT_TEMPLATE.to_string()),
        }
    }

    /// Rephrase a query using the LLM
    async fn rephrase_query(
        &self,
        query: &str,
        config: Option<&RunnableConfig>,
    ) -> Result<String, DashFlowError> {
        // Format the prompt with the query
        let prompt = self.prompt_template.replace("{question}", query);
        let messages = vec![Message::human(prompt)];

        // Generate the rephrased query
        // Signature: generate(messages, stop, tools, tool_choice, config)
        let result = self
            .llm
            .generate(&messages, None, None, None, config)
            .await?;

        if result.generations.is_empty() {
            return Err(DashFlowError::other(
                "LLM returned empty response when rephrasing query",
            ));
        }

        // Extract the text from the message
        let text = result.generations[0].message.content().as_text();
        Ok(text)
    }
}

#[async_trait]
impl Retriever for RePhraseQueryRetriever {
    async fn _get_relevant_documents(
        &self,
        query: &str,
        config: Option<&RunnableConfig>,
    ) -> Result<Vec<Document>, DashFlowError> {
        // Rephrase the query using the LLM
        let rephrased_query = self.rephrase_query(query, config).await?;

        // Retrieve documents using the rephrased query
        self.retriever
            ._get_relevant_documents(&rephrased_query, config)
            .await
    }

    fn name(&self) -> String {
        "RePhraseQueryRetriever".to_string()
    }
}

#[cfg(test)]
mod tests {
    use crate::core::{
        language_models::{ChatGeneration, ChatResult},
        messages::BaseMessage,
    };
    use crate::test_prelude::*;
    use std::pin::Pin;

    // Mock retriever that just returns documents with the query as content
    struct MockRetriever {
        docs: Vec<Document>,
    }

    #[async_trait]
    impl Retriever for MockRetriever {
        async fn _get_relevant_documents(
            &self,
            _query: &str,
            _config: Option<&RunnableConfig>,
        ) -> StdResult<Vec<Document>, DashFlowError> {
            Ok(self.docs.clone())
        }
    }

    /// Mock LLM that returns a fixed rephrased query.
    /// Used only in tests - stream() is not called by test code.
    struct MockLLM {
        response: String,
    }

    #[async_trait]
    impl ChatModel for MockLLM {
        async fn _generate(
            &self,
            _messages: &[BaseMessage],
            _stop: Option<&[String]>,
            _tools: Option<&[crate::core::language_models::ToolDefinition]>,
            _tool_choice: Option<&crate::core::language_models::ToolChoice>,
            _run_manager: Option<&crate::core::callbacks::CallbackManager>,
        ) -> StdResult<ChatResult, DashFlowError> {
            Ok(ChatResult {
                generations: vec![ChatGeneration {
                    message: Message::ai(self.response.clone()),
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

        /// Streaming not implemented for test mock.
        /// Test code only uses _generate(), so this is never called.
        async fn stream(
            &self,
            _messages: &[BaseMessage],
            _stop: Option<&[String]>,
            _tools: Option<&[crate::core::language_models::ToolDefinition]>,
            _tool_choice: Option<&crate::core::language_models::ToolChoice>,
            _config: Option<&RunnableConfig>,
        ) -> StdResult<
            Pin<
                Box<
                    dyn futures::Stream<
                            Item = StdResult<
                                crate::core::language_models::ChatGenerationChunk,
                                DashFlowError,
                            >,
                        > + Send,
                >,
            >,
            DashFlowError,
        > {
            Err(DashFlowError::other(
                "MockLLM.stream() not implemented - test mock",
            ))
        }
    }

    #[tokio::test]
    async fn test_rephrase_query_retriever() {
        let docs = vec![
            Document::new("Document about cats"),
            Document::new("Document about dogs"),
        ];

        let base_retriever = Arc::new(MockRetriever { docs: docs.clone() }) as Arc<dyn Retriever>;

        let llm = Arc::new(MockLLM {
            response: "cats".to_string(),
        }) as Arc<dyn ChatModel>;

        let rephrase_retriever = RePhraseQueryRetriever::from_llm(base_retriever, llm, None);

        let result = rephrase_retriever
            ._get_relevant_documents("Tell me about cats, but also I like dogs", None)
            .await;

        assert!(result.is_ok());
        let retrieved_docs = result.unwrap();
        assert_eq!(retrieved_docs.len(), 2);
        assert_eq!(retrieved_docs[0].page_content, "Document about cats");
    }

    #[tokio::test]
    async fn test_custom_prompt_template() {
        let docs = vec![Document::new("Test document")];

        let base_retriever = Arc::new(MockRetriever { docs: docs.clone() }) as Arc<dyn Retriever>;

        let llm = Arc::new(MockLLM {
            response: "rephrased".to_string(),
        }) as Arc<dyn ChatModel>;

        let custom_template = "Custom prompt with question: {question}";
        let rephrase_retriever = RePhraseQueryRetriever::from_llm(
            base_retriever,
            llm,
            Some(custom_template.to_string()),
        );

        let result = rephrase_retriever
            ._get_relevant_documents("original query", None)
            .await;

        assert!(result.is_ok());
    }

    #[test]
    fn test_default_template_format() {
        assert!(DEFAULT_TEMPLATE.contains("{question}"));
    }

    /// Mock LLM that returns empty generations.
    /// Used only in tests - stream() is not called by test code.
    struct EmptyLLM;

    #[async_trait]
    impl ChatModel for EmptyLLM {
        async fn _generate(
            &self,
            _messages: &[BaseMessage],
            _stop: Option<&[String]>,
            _tools: Option<&[crate::core::language_models::ToolDefinition]>,
            _tool_choice: Option<&crate::core::language_models::ToolChoice>,
            _run_manager: Option<&crate::core::callbacks::CallbackManager>,
        ) -> StdResult<ChatResult, DashFlowError> {
            Ok(ChatResult {
                generations: vec![],
                llm_output: None,
            })
        }

        fn llm_type(&self) -> &str {
            "empty_mock"
        }

        fn as_any(&self) -> &dyn std::any::Any {
            self
        }

        /// Streaming not implemented for test mock.
        /// Test code only uses _generate(), so this is never called.
        async fn stream(
            &self,
            _messages: &[BaseMessage],
            _stop: Option<&[String]>,
            _tools: Option<&[crate::core::language_models::ToolDefinition]>,
            _tool_choice: Option<&crate::core::language_models::ToolChoice>,
            _config: Option<&RunnableConfig>,
        ) -> StdResult<
            Pin<
                Box<
                    dyn futures::Stream<
                            Item = StdResult<
                                crate::core::language_models::ChatGenerationChunk,
                                DashFlowError,
                            >,
                        > + Send,
                >,
            >,
            DashFlowError,
        > {
            Err(DashFlowError::other(
                "EmptyLLM.stream() not implemented - test mock",
            ))
        }
    }

    #[tokio::test]
    async fn test_llm_returns_empty_response() {
        let docs = vec![Document::new("Test document")];
        let base_retriever = Arc::new(MockRetriever { docs }) as Arc<dyn Retriever>;
        let llm = Arc::new(EmptyLLM) as Arc<dyn ChatModel>;

        let rephrase_retriever = RePhraseQueryRetriever::from_llm(base_retriever, llm, None);

        let result = rephrase_retriever
            ._get_relevant_documents("test query", None)
            .await;

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("empty response"));
    }

    // Mock retriever that captures the query it receives
    struct CaptureQueryRetriever {
        query: Arc<tokio::sync::Mutex<Option<String>>>,
        docs: Vec<Document>,
    }

    #[async_trait]
    impl Retriever for CaptureQueryRetriever {
        async fn _get_relevant_documents(
            &self,
            query: &str,
            _config: Option<&RunnableConfig>,
        ) -> StdResult<Vec<Document>, DashFlowError> {
            let mut q = self.query.lock().await;
            *q = Some(query.to_string());
            Ok(self.docs.clone())
        }
    }

    #[tokio::test]
    async fn test_query_actually_rephrased() {
        let captured_query = Arc::new(tokio::sync::Mutex::new(None));
        let docs = vec![Document::new("Result")];

        let base_retriever = Arc::new(CaptureQueryRetriever {
            query: captured_query.clone(),
            docs,
        }) as Arc<dyn Retriever>;

        let llm = Arc::new(MockLLM {
            response: "rephrased_query".to_string(),
        }) as Arc<dyn ChatModel>;

        let rephrase_retriever = RePhraseQueryRetriever::from_llm(base_retriever, llm, None);

        let _result = rephrase_retriever
            ._get_relevant_documents("original_query", None)
            .await
            .unwrap();

        // Verify the base retriever received the rephrased query, not the original
        let captured = captured_query.lock().await;
        assert_eq!(captured.as_ref().unwrap(), "rephrased_query");
    }

    #[tokio::test]
    async fn test_empty_query() {
        let docs = vec![Document::new("Test")];
        let base_retriever = Arc::new(MockRetriever { docs: docs.clone() }) as Arc<dyn Retriever>;
        let llm = Arc::new(MockLLM {
            response: "empty_input_rephrased".to_string(),
        }) as Arc<dyn ChatModel>;

        let rephrase_retriever = RePhraseQueryRetriever::from_llm(base_retriever, llm, None);

        let result = rephrase_retriever._get_relevant_documents("", None).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_long_query() {
        let long_query = "a".repeat(10_000);
        let docs = vec![Document::new("Test")];
        let base_retriever = Arc::new(MockRetriever { docs: docs.clone() }) as Arc<dyn Retriever>;
        let llm = Arc::new(MockLLM {
            response: "simplified".to_string(),
        }) as Arc<dyn ChatModel>;

        let rephrase_retriever = RePhraseQueryRetriever::from_llm(base_retriever, llm, None);

        let result = rephrase_retriever
            ._get_relevant_documents(&long_query, None)
            .await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_special_characters_in_query() {
        let special_query = "Query with\nnewlines\tand\ttabs \"quotes\" 'apostrophes' 😀 emoji";
        let docs = vec![Document::new("Test")];
        let base_retriever = Arc::new(MockRetriever { docs: docs.clone() }) as Arc<dyn Retriever>;
        let llm = Arc::new(MockLLM {
            response: "clean_query".to_string(),
        }) as Arc<dyn ChatModel>;

        let rephrase_retriever = RePhraseQueryRetriever::from_llm(base_retriever, llm, None);

        let result = rephrase_retriever
            ._get_relevant_documents(special_query, None)
            .await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_multiple_placeholders_in_template() {
        let docs = vec![Document::new("Test")];
        let base_retriever = Arc::new(MockRetriever { docs: docs.clone() }) as Arc<dyn Retriever>;
        let llm = Arc::new(MockLLM {
            response: "rephrased".to_string(),
        }) as Arc<dyn ChatModel>;

        // Template with multiple {question} placeholders
        let template = "Question: {question}. Repeat: {question}";
        let rephrase_retriever =
            RePhraseQueryRetriever::from_llm(base_retriever, llm, Some(template.to_string()));

        let result = rephrase_retriever
            ._get_relevant_documents("test", None)
            .await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_template_without_placeholder() {
        let docs = vec![Document::new("Test")];
        let base_retriever = Arc::new(MockRetriever { docs: docs.clone() }) as Arc<dyn Retriever>;
        let llm = Arc::new(MockLLM {
            response: "rephrased".to_string(),
        }) as Arc<dyn ChatModel>;

        // Template without {question} placeholder
        let template = "Just rephrase this query generically";
        let rephrase_retriever =
            RePhraseQueryRetriever::from_llm(base_retriever, llm, Some(template.to_string()));

        let result = rephrase_retriever
            ._get_relevant_documents("actual query", None)
            .await;
        // Should still work, template just won't include the query
        assert!(result.is_ok());
    }

    // Mock retriever that returns an error
    struct ErrorRetriever;

    #[async_trait]
    impl Retriever for ErrorRetriever {
        async fn _get_relevant_documents(
            &self,
            _query: &str,
            _config: Option<&RunnableConfig>,
        ) -> StdResult<Vec<Document>, DashFlowError> {
            Err(DashFlowError::other("Retriever error"))
        }
    }

    #[tokio::test]
    async fn test_base_retriever_error_propagates() {
        let base_retriever = Arc::new(ErrorRetriever) as Arc<dyn Retriever>;
        let llm = Arc::new(MockLLM {
            response: "rephrased".to_string(),
        }) as Arc<dyn ChatModel>;

        let rephrase_retriever = RePhraseQueryRetriever::from_llm(base_retriever, llm, None);

        let result = rephrase_retriever
            ._get_relevant_documents("test query", None)
            .await;

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("Retriever error"));
    }

    /// Mock LLM that returns an error.
    /// Used only in tests - stream() is not called by test code.
    struct ErrorLLM;

    #[async_trait]
    impl ChatModel for ErrorLLM {
        async fn _generate(
            &self,
            _messages: &[BaseMessage],
            _stop: Option<&[String]>,
            _tools: Option<&[crate::core::language_models::ToolDefinition]>,
            _tool_choice: Option<&crate::core::language_models::ToolChoice>,
            _run_manager: Option<&crate::core::callbacks::CallbackManager>,
        ) -> StdResult<ChatResult, DashFlowError> {
            Err(DashFlowError::other("LLM generation failed"))
        }

        fn llm_type(&self) -> &str {
            "error_mock"
        }

        fn as_any(&self) -> &dyn std::any::Any {
            self
        }

        /// Streaming not implemented for test mock.
        /// Test code only uses _generate(), so this is never called.
        async fn stream(
            &self,
            _messages: &[BaseMessage],
            _stop: Option<&[String]>,
            _tools: Option<&[crate::core::language_models::ToolDefinition]>,
            _tool_choice: Option<&crate::core::language_models::ToolChoice>,
            _config: Option<&RunnableConfig>,
        ) -> StdResult<
            Pin<
                Box<
                    dyn futures::Stream<
                            Item = StdResult<
                                crate::core::language_models::ChatGenerationChunk,
                                DashFlowError,
                            >,
                        > + Send,
                >,
            >,
            DashFlowError,
        > {
            Err(DashFlowError::other(
                "ErrorLLM.stream() not implemented - test mock",
            ))
        }
    }

    #[tokio::test]
    async fn test_llm_error_propagates() {
        let docs = vec![Document::new("Test")];
        let base_retriever = Arc::new(MockRetriever { docs }) as Arc<dyn Retriever>;
        let llm = Arc::new(ErrorLLM) as Arc<dyn ChatModel>;

        let rephrase_retriever = RePhraseQueryRetriever::from_llm(base_retriever, llm, None);

        let result = rephrase_retriever
            ._get_relevant_documents("test query", None)
            .await;

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("LLM generation failed"));
    }

    #[test]
    fn test_name_method() {
        let docs = vec![Document::new("Test")];
        let base_retriever = Arc::new(MockRetriever { docs }) as Arc<dyn Retriever>;
        let llm = Arc::new(MockLLM {
            response: "test".to_string(),
        }) as Arc<dyn ChatModel>;

        let rephrase_retriever = RePhraseQueryRetriever::from_llm(base_retriever, llm, None);

        assert_eq!(rephrase_retriever.name(), "RePhraseQueryRetriever");
    }

    #[tokio::test]
    async fn test_whitespace_only_query() {
        let docs = vec![Document::new("Test")];
        let base_retriever = Arc::new(MockRetriever { docs: docs.clone() }) as Arc<dyn Retriever>;
        let llm = Arc::new(MockLLM {
            response: "cleaned".to_string(),
        }) as Arc<dyn ChatModel>;

        let rephrase_retriever = RePhraseQueryRetriever::from_llm(base_retriever, llm, None);

        let result = rephrase_retriever
            ._get_relevant_documents("   \t\n  ", None)
            .await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_llm_returns_multiline_response() {
        let captured_query = Arc::new(tokio::sync::Mutex::new(None));
        let docs = vec![Document::new("Result")];

        let base_retriever = Arc::new(CaptureQueryRetriever {
            query: captured_query.clone(),
            docs,
        }) as Arc<dyn Retriever>;

        let llm = Arc::new(MockLLM {
            response: "line1\nline2\nline3".to_string(),
        }) as Arc<dyn ChatModel>;

        let rephrase_retriever = RePhraseQueryRetriever::from_llm(base_retriever, llm, None);

        let result = rephrase_retriever
            ._get_relevant_documents("test", None)
            .await;
        assert!(result.is_ok());

        // Verify multiline response is passed through
        let captured = captured_query.lock().await;
        assert_eq!(captured.as_ref().unwrap(), "line1\nline2\nline3");
    }

    #[tokio::test]
    async fn test_llm_returns_very_long_response() {
        let captured_query = Arc::new(tokio::sync::Mutex::new(None));
        let docs = vec![Document::new("Result")];

        let base_retriever = Arc::new(CaptureQueryRetriever {
            query: captured_query.clone(),
            docs,
        }) as Arc<dyn Retriever>;

        let long_response = "x".repeat(50_000);
        let llm = Arc::new(MockLLM {
            response: long_response.clone(),
        }) as Arc<dyn ChatModel>;

        let rephrase_retriever = RePhraseQueryRetriever::from_llm(base_retriever, llm, None);

        let result = rephrase_retriever
            ._get_relevant_documents("short", None)
            .await;
        assert!(result.is_ok());

        // Verify long response is passed through
        let captured = captured_query.lock().await;
        assert_eq!(captured.as_ref().unwrap(), &long_response);
    }

    // Mock retriever that checks config is passed
    struct ConfigCheckRetriever {
        config_received: Arc<tokio::sync::Mutex<bool>>,
    }

    #[async_trait]
    impl Retriever for ConfigCheckRetriever {
        async fn _get_relevant_documents(
            &self,
            _query: &str,
            config: Option<&RunnableConfig>,
        ) -> StdResult<Vec<Document>, DashFlowError> {
            let mut received = self.config_received.lock().await;
            *received = config.is_some();
            Ok(vec![Document::new("Test")])
        }
    }

    #[tokio::test]
    async fn test_config_passed_to_retriever() {
        let config_received = Arc::new(tokio::sync::Mutex::new(false));

        let base_retriever = Arc::new(ConfigCheckRetriever {
            config_received: config_received.clone(),
        }) as Arc<dyn Retriever>;

        let llm = Arc::new(MockLLM {
            response: "rephrased".to_string(),
        }) as Arc<dyn ChatModel>;

        let rephrase_retriever = RePhraseQueryRetriever::from_llm(base_retriever, llm, None);

        let config = RunnableConfig::default();
        let _result = rephrase_retriever
            ._get_relevant_documents("test", Some(&config))
            .await;

        // Verify config was passed to base retriever
        let received = config_received.lock().await;
        assert!(*received);
    }

    #[tokio::test]
    async fn test_config_passed_to_llm() {
        let llm = Arc::new(MockLLM {
            response: "rephrased".to_string(),
        }) as Arc<dyn ChatModel>;

        // Test that generate works with config
        let messages = vec![Message::human("test")];
        let config = RunnableConfig::default();

        // This internally calls _generate through generate
        let result = llm
            .generate(&messages, None, None, None, Some(&config))
            .await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_query_with_curly_braces() {
        let query_with_braces = "Find {key: value} in JSON";
        let docs = vec![Document::new("Test")];
        let base_retriever = Arc::new(MockRetriever { docs: docs.clone() }) as Arc<dyn Retriever>;
        let llm = Arc::new(MockLLM {
            response: "json_query".to_string(),
        }) as Arc<dyn ChatModel>;

        let rephrase_retriever = RePhraseQueryRetriever::from_llm(base_retriever, llm, None);

        let result = rephrase_retriever
            ._get_relevant_documents(query_with_braces, None)
            .await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_default_template_placeholder_replaced() {
        let captured_query = Arc::new(tokio::sync::Mutex::new(None));
        let docs = vec![Document::new("Result")];

        let base_retriever = Arc::new(CaptureQueryRetriever {
            query: captured_query.clone(),
            docs,
        }) as Arc<dyn Retriever>;

        /// Mock LLM that returns the prompt it received.
        /// Used only in tests - stream() is not called by test code.
        struct PromptEchoLLM {
            last_prompt: Arc<tokio::sync::Mutex<Option<String>>>,
        }

        #[async_trait]
        impl ChatModel for PromptEchoLLM {
            async fn _generate(
                &self,
                messages: &[BaseMessage],
                _stop: Option<&[String]>,
                _tools: Option<&[crate::core::language_models::ToolDefinition]>,
                _tool_choice: Option<&crate::core::language_models::ToolChoice>,
                _run_manager: Option<&crate::core::callbacks::CallbackManager>,
            ) -> StdResult<ChatResult, DashFlowError> {
                let prompt = messages[0].content().as_text();
                let mut last = self.last_prompt.lock().await;
                *last = Some(prompt.clone());

                Ok(ChatResult {
                    generations: vec![ChatGeneration {
                        message: Message::ai("query"),
                        generation_info: None,
                    }],
                    llm_output: None,
                })
            }

            fn llm_type(&self) -> &str {
                "prompt_echo"
            }

            fn as_any(&self) -> &dyn std::any::Any {
                self
            }

            /// Streaming not implemented for test mock.
            /// Test code only uses _generate(), so this is never called.
            async fn stream(
                &self,
                _messages: &[BaseMessage],
                _stop: Option<&[String]>,
                _tools: Option<&[crate::core::language_models::ToolDefinition]>,
                _tool_choice: Option<&crate::core::language_models::ToolChoice>,
                _config: Option<&RunnableConfig>,
            ) -> StdResult<
                Pin<
                    Box<
                        dyn futures::Stream<
                                Item = StdResult<
                                    crate::core::language_models::ChatGenerationChunk,
                                    DashFlowError,
                                >,
                            > + Send,
                    >,
                >,
                DashFlowError,
            > {
                Err(DashFlowError::other(
                    "PromptEchoLLM.stream() not implemented - test mock",
                ))
            }
        }

        let last_prompt = Arc::new(tokio::sync::Mutex::new(None));
        let llm = Arc::new(PromptEchoLLM {
            last_prompt: last_prompt.clone(),
        }) as Arc<dyn ChatModel>;

        let rephrase_retriever = RePhraseQueryRetriever::from_llm(base_retriever, llm, None);

        let test_query = "my test query";
        let _result = rephrase_retriever
            ._get_relevant_documents(test_query, None)
            .await
            .unwrap();

        // Verify the prompt contained the query (not the placeholder)
        let prompt = last_prompt.lock().await;
        let prompt_str = prompt.as_ref().unwrap();
        assert!(prompt_str.contains(test_query));
        assert!(!prompt_str.contains("{question}"));
    }

    #[tokio::test]
    async fn test_no_documents_from_base_retriever() {
        let base_retriever = Arc::new(MockRetriever { docs: vec![] }) as Arc<dyn Retriever>;
        let llm = Arc::new(MockLLM {
            response: "rephrased".to_string(),
        }) as Arc<dyn ChatModel>;

        let rephrase_retriever = RePhraseQueryRetriever::from_llm(base_retriever, llm, None);

        let result = rephrase_retriever
            ._get_relevant_documents("test", None)
            .await
            .unwrap();

        assert_eq!(result.len(), 0);
    }

    #[tokio::test]
    async fn test_many_documents_from_base_retriever() {
        let docs: Vec<Document> = (0..1000)
            .map(|i| Document::new(format!("Document {}", i)))
            .collect();

        let base_retriever = Arc::new(MockRetriever { docs: docs.clone() }) as Arc<dyn Retriever>;
        let llm = Arc::new(MockLLM {
            response: "rephrased".to_string(),
        }) as Arc<dyn ChatModel>;

        let rephrase_retriever = RePhraseQueryRetriever::from_llm(base_retriever, llm, None);

        let result = rephrase_retriever
            ._get_relevant_documents("test", None)
            .await
            .unwrap();

        assert_eq!(result.len(), 1000);
    }
}
