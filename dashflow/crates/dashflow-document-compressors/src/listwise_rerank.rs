//! LLM-based listwise document reranking
//!
//! This module implements zero-shot listwise document reranking using a language model.
//! Based on the paper: "Zero-Shot Listwise Document Reranking" (<https://arxiv.org/pdf/2305.02156.pdf>)
//!
//! # Overview
//!
//! `ListwiseRerank` uses an LLM to rerank documents by asking it to sort documents
//! by relevance to a query. Unlike pointwise approaches (which score documents individually)
//! or pairwise approaches (which compare pairs), listwise reranking considers all
//! documents together, potentially capturing relationships between documents.
//!
//! # Requirements
//!
//! The LLM must support tool calling / function calling (structured output).
//! Compatible models include:
//! - `OpenAI` (GPT-3.5, GPT-4)
//! - Anthropic Claude
//! - Mistral
//! - Other models with tool calling support
//!
//! # Example
//!
//! ```rust,ignore
//! use dashflow_document_compressors::LLMListwiseRerank;
//! use dashflow_openai::ChatOpenAI;
//! use dashflow::core::documents::Document;
//!
//! let llm = ChatOpenAI::with_config(Default::default()).with_model("gpt-4o-mini");
//! let reranker = LLMListwiseRerank::from_llm(llm, None).with_top_n(3);
//!
//! let documents = vec![
//!     Document::new("Sally is my friend from school"),
//!     Document::new("Steve is my friend from home"),
//!     Document::new("I didn't always like yogurt"),
//!     Document::new("I wonder why it's called football"),
//!     Document::new("Where's waldo"),
//! ];
//!
//! let reranked = reranker
//!     .compress_documents(documents, "Who is steve", None)
//!     .await?;
//!
//! assert_eq!(reranked.len(), 3);
//! assert!(reranked[0].page_content.contains("Steve"));
//! ```

use async_trait::async_trait;
use dashflow::core::documents::{Document, DocumentCompressor};
use dashflow::core::error::{Error, Result};
use dashflow::core::language_models::{ChatModel, ToolChoice, ToolDefinition};
use dashflow::core::messages::{BaseMessage, Message};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::sync::Arc;

/// Default system prompt template for listwise reranking
const DEFAULT_SYSTEM_TEMPLATE: &str =
    "{context}\n\nSort the Documents by their relevance to the Query.";

/// Schema for the LLM's ranking output
#[derive(Debug, Clone, Serialize, Deserialize)]
struct RankDocuments {
    /// List of document IDs sorted from most to least relevant
    #[serde(rename = "ranked_document_ids")]
    ranked_document_ids: Vec<usize>,
}

/// Document compressor using zero-shot listwise reranking with an LLM.
///
/// This compressor uses a language model to rerank documents by their relevance
/// to a query. It presents all documents to the LLM and asks it to sort them
/// by relevance using structured output (tool calling).
///
/// # Advantages
///
/// - Considers relationships between documents
/// - Can handle complex relevance judgments
/// - Works across many domains without training
///
/// # Disadvantages
///
/// - Slower than embedding-based approaches
/// - More expensive (LLM API calls)
/// - Limited by LLM context window
pub struct LLMListwiseRerank {
    /// Language model for reranking (must support tool calling)
    llm: Arc<dyn ChatModel>,
    /// System prompt template (must contain {context} and {query} placeholders)
    system_template: String,
    /// Number of top documents to return
    top_n: usize,
    /// Tool definition for structured output
    tool_definition: ToolDefinition,
}

impl LLMListwiseRerank {
    /// Create a new `LLMListwiseRerank` from a chat model.
    ///
    /// # Arguments
    ///
    /// * `llm` - Language model that supports tool calling / function calling
    /// * `system_template` - Optional custom system prompt. If None, uses default.
    ///   Must contain {context} placeholder for document listing.
    ///
    /// # Returns
    ///
    /// A new `LLMListwiseRerank` compressor with `top_n=3` by default.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use dashflow_openai::ChatOpenAI;
    ///
    /// let llm = ChatOpenAI::with_config(Default::default()).with_model("gpt-4o-mini");
    /// let reranker = LLMListwiseRerank::from_llm(llm, None);
    /// ```
    pub fn from_llm(llm: Arc<dyn ChatModel>, system_template: Option<String>) -> Self {
        let system_template =
            system_template.unwrap_or_else(|| DEFAULT_SYSTEM_TEMPLATE.to_string());

        // Define the tool/function for structured output
        let tool_definition = ToolDefinition {
            name: "rank_documents".to_string(),
            description: "Rank the documents by their relevance to the user question. \
                 Rank from most to least relevant."
                .to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "ranked_document_ids": {
                        "type": "array",
                        "items": {
                            "type": "integer"
                        },
                        "description": "The integer IDs of the documents, sorted from most to least relevant to the user question."
                    }
                },
                "required": ["ranked_document_ids"]
            }),
        };

        Self {
            llm,
            system_template,
            top_n: 3,
            tool_definition,
        }
    }

    /// Set the number of documents to return after reranking.
    ///
    /// # Arguments
    ///
    /// * `top_n` - Number of top documents to return
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let reranker = LLMListwiseRerank::from_llm(llm, None)
    ///     .with_top_n(5);
    /// ```
    #[must_use]
    pub fn with_top_n(mut self, top_n: usize) -> Self {
        self.top_n = top_n;
        self
    }

    /// Get the current `top_n` setting.
    #[must_use]
    pub fn top_n(&self) -> usize {
        self.top_n
    }

    /// Build the context string for the system prompt.
    ///
    /// Formats documents as:
    /// ```text
    /// Document ID: 0
    /// ```<doc content>```
    ///
    /// Document ID: 1
    /// ```<doc content>```
    ///
    /// Documents = [Document ID: 0, ..., Document ID: N-1]
    /// ```
    fn build_context(&self, documents: &[Document]) -> String {
        let mut context = String::new();

        for (index, doc) in documents.iter().enumerate() {
            context.push_str(&format!(
                "Document ID: {}\n```{}```\n\n",
                index, doc.page_content
            ));
        }

        let document_range = if documents.is_empty() {
            "empty list".to_string()
        } else {
            format!("Document ID: 0, ..., Document ID: {}", documents.len() - 1)
        };

        context.push_str(&format!("Documents = [{document_range}]"));
        context
    }
}

#[async_trait]
impl DocumentCompressor for LLMListwiseRerank {
    /// Rerank documents using LLM-based listwise sorting.
    ///
    /// Presents all documents to the LLM with their IDs and asks the LLM to
    /// return a ranked list of IDs sorted by relevance to the query.
    ///
    /// # Arguments
    ///
    /// * `documents` - Documents to rerank
    /// * `query` - Query to rank documents against
    /// * `_callbacks` - Optional callbacks (currently unused)
    ///
    /// # Returns
    ///
    /// Top-n documents sorted by LLM's relevance judgment.
    /// If LLM fails to provide valid ranking, returns documents in original order.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let reranked = reranker
    ///     .compress_documents(docs, "What is machine learning?", None)
    ///     .await?;
    /// ```
    async fn compress_documents(
        &self,
        documents: Vec<Document>,
        query: &str,
        _config: Option<&dashflow::core::config::RunnableConfig>,
    ) -> Result<Vec<Document>> {
        if documents.is_empty() {
            return Ok(Vec::new());
        }

        // Build context with document IDs
        let context = self.build_context(&documents);

        // Create messages
        let system_content = self.system_template.replace("{context}", &context);
        let messages: Vec<BaseMessage> =
            vec![Message::system(system_content), Message::human(query)];

        // Call LLM with tool definition
        let tool_choice = ToolChoice::Required;
        let result = self
            .llm
            .generate(
                &messages,
                None,
                Some(std::slice::from_ref(&self.tool_definition)),
                Some(&tool_choice),
                None,
            )
            .await?;

        // Extract tool call from response
        if let Some(generation) = result.generations.first() {
            if let Message::AI { tool_calls, .. } = &generation.message {
                if !tool_calls.is_empty() {
                    if let Some(tool_call) = tool_calls.first() {
                        // Parse the tool call arguments
                        let parsed: RankDocuments = serde_json::from_value(tool_call.args.clone())
                            .map_err(|e| {
                                Error::OutputParsing(format!("Failed to parse ranking: {e}"))
                            })?;

                        // Reorder documents according to ranking
                        let mut reranked = Vec::new();
                        for &doc_id in &parsed.ranked_document_ids {
                            if doc_id < documents.len() {
                                reranked.push(documents[doc_id].clone());
                            }
                        }

                        // If ranking is incomplete, add remaining docs
                        for (i, doc) in documents.iter().enumerate() {
                            if !parsed.ranked_document_ids.contains(&i) {
                                reranked.push(doc.clone());
                            }
                        }

                        return Ok(reranked.into_iter().take(self.top_n).collect());
                    }
                }
            }
        }

        // Fallback: return documents in original order if LLM didn't provide ranking
        Ok(documents.into_iter().take(self.top_n).collect())
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use dashflow::core::documents::Document;
    use dashflow::core::language_models::{ChatGeneration, ChatResult};
    use dashflow::core::messages::{Message, MessageContent, ToolCall};

    // ============================================================
    // MOCK CHAT MODELS
    // ============================================================

    /// Mock ChatModel that returns a configurable ranking via tool call
    struct MockChatModel {
        ranking: Vec<usize>,
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
            let tool_call = ToolCall {
                id: "call_123".to_string(),
                name: "rank_documents".to_string(),
                args: json!({"ranked_document_ids": self.ranking}),
                tool_type: "tool_call".to_string(),
                index: None,
            };

            let ai_message = Message::AI {
                content: "Here's the ranking".into(),
                tool_calls: vec![tool_call],
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

    /// Mock ChatModel that returns no tool calls (fallback behavior)
    struct NoToolCallModel;

    #[async_trait]
    impl ChatModel for NoToolCallModel {
        async fn _generate(
            &self,
            _messages: &[BaseMessage],
            _stop: Option<&[String]>,
            _tools: Option<&[ToolDefinition]>,
            _tool_choice: Option<&ToolChoice>,
            _run_manager: Option<&dashflow::core::callbacks::CallbackManager>,
        ) -> Result<ChatResult> {
            let ai_message = Message::AI {
                content: MessageContent::Text("I'll rank them: 2, 1, 0".to_string()),
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
    // BASIC FUNCTIONALITY TESTS
    // ============================================================

    #[tokio::test]
    async fn test_listwise_rerank_basic() {
        // Mock model that ranks documents in reverse order
        let llm = Arc::new(MockChatModel {
            ranking: vec![2, 1, 0],
        });

        let reranker = LLMListwiseRerank::from_llm(llm, None).with_top_n(2);

        let documents = vec![
            Document::new("First document"),
            Document::new("Second document"),
            Document::new("Third document"),
        ];

        let result = reranker
            .compress_documents(documents, "test query", None)
            .await
            .unwrap();

        assert_eq!(result.len(), 2);
        assert_eq!(result[0].page_content, "Third document");
        assert_eq!(result[1].page_content, "Second document");
    }

    #[tokio::test]
    async fn test_listwise_rerank_partial_ranking() {
        // Mock model that only ranks first 2 documents
        let llm = Arc::new(MockChatModel {
            ranking: vec![1, 0],
        });

        let reranker = LLMListwiseRerank::from_llm(llm, None).with_top_n(3);

        let documents = vec![
            Document::new("First document"),
            Document::new("Second document"),
            Document::new("Third document"),
        ];

        let result = reranker
            .compress_documents(documents, "test query", None)
            .await
            .unwrap();

        // Should get 3 documents (top_n=3), with ranked ones first
        assert_eq!(result.len(), 3);
        assert_eq!(result[0].page_content, "Second document");
        assert_eq!(result[1].page_content, "First document");
        assert_eq!(result[2].page_content, "Third document");
    }

    #[tokio::test]
    async fn test_listwise_rerank_empty_input() {
        let llm = Arc::new(MockChatModel { ranking: vec![] });
        let reranker = LLMListwiseRerank::from_llm(llm, None);

        let documents: Vec<Document> = vec![];
        let result = reranker
            .compress_documents(documents, "test query", None)
            .await
            .unwrap();

        assert_eq!(result.len(), 0);
    }

    // ============================================================
    // BUILD_CONTEXT TESTS
    // ============================================================

    #[tokio::test]
    async fn test_build_context_basic() {
        let llm = Arc::new(MockChatModel { ranking: vec![] });
        let reranker = LLMListwiseRerank::from_llm(llm, None);

        let documents = vec![
            Document::new("Doc 1 content"),
            Document::new("Doc 2 content"),
        ];

        let context = reranker.build_context(&documents);

        assert!(context.contains("Document ID: 0"));
        assert!(context.contains("Doc 1 content"));
        assert!(context.contains("Document ID: 1"));
        assert!(context.contains("Doc 2 content"));
        assert!(context.contains("Documents = [Document ID: 0, ..., Document ID: 1]"));
    }

    #[tokio::test]
    async fn test_build_context_single_document() {
        let llm = Arc::new(MockChatModel { ranking: vec![] });
        let reranker = LLMListwiseRerank::from_llm(llm, None);

        let documents = vec![Document::new("Only document")];
        let context = reranker.build_context(&documents);

        assert!(context.contains("Document ID: 0"));
        assert!(context.contains("Only document"));
        assert!(context.contains("Documents = [Document ID: 0, ..., Document ID: 0]"));
    }

    #[tokio::test]
    async fn test_build_context_empty() {
        let llm = Arc::new(MockChatModel { ranking: vec![] });
        let reranker = LLMListwiseRerank::from_llm(llm, None);

        let documents: Vec<Document> = vec![];
        let context = reranker.build_context(&documents);

        assert!(context.contains("Documents = [empty list]"));
    }

    #[tokio::test]
    async fn test_build_context_many_documents() {
        let llm = Arc::new(MockChatModel { ranking: vec![] });
        let reranker = LLMListwiseRerank::from_llm(llm, None);

        let documents: Vec<Document> = (0..10)
            .map(|i| Document::new(format!("Document {i} content")))
            .collect();

        let context = reranker.build_context(&documents);

        assert!(context.contains("Document ID: 0"));
        assert!(context.contains("Document ID: 9"));
        assert!(context.contains("Documents = [Document ID: 0, ..., Document ID: 9]"));
    }

    // ============================================================
    // BUILDER PATTERN TESTS
    // ============================================================

    #[test]
    fn test_builder_pattern() {
        let llm = Arc::new(MockChatModel { ranking: vec![] });
        let reranker = LLMListwiseRerank::from_llm(llm, None).with_top_n(5);

        assert_eq!(reranker.top_n(), 5);
    }

    #[test]
    fn test_default_top_n() {
        let llm = Arc::new(MockChatModel { ranking: vec![] });
        let reranker = LLMListwiseRerank::from_llm(llm, None);

        assert_eq!(reranker.top_n(), 3);
    }

    #[test]
    fn test_with_top_n_zero() {
        let llm = Arc::new(MockChatModel { ranking: vec![] });
        let reranker = LLMListwiseRerank::from_llm(llm, None).with_top_n(0);

        assert_eq!(reranker.top_n(), 0);
    }

    #[test]
    fn test_custom_system_template() {
        let llm = Arc::new(MockChatModel { ranking: vec![] });
        let custom_template = "Custom: {context}".to_string();
        let reranker = LLMListwiseRerank::from_llm(llm, Some(custom_template.clone()));

        assert_eq!(reranker.system_template, custom_template);
    }

    // ============================================================
    // EDGE CASES TESTS
    // ============================================================

    #[tokio::test]
    async fn test_single_document() {
        let llm = Arc::new(MockChatModel { ranking: vec![0] });
        let reranker = LLMListwiseRerank::from_llm(llm, None).with_top_n(5);

        let documents = vec![Document::new("Only document")];
        let result = reranker
            .compress_documents(documents, "query", None)
            .await
            .unwrap();

        assert_eq!(result.len(), 1);
        assert_eq!(result[0].page_content, "Only document");
    }

    #[tokio::test]
    async fn test_top_n_zero_returns_empty() {
        let llm = Arc::new(MockChatModel {
            ranking: vec![2, 1, 0],
        });
        let reranker = LLMListwiseRerank::from_llm(llm, None).with_top_n(0);

        let documents = vec![
            Document::new("Doc A"),
            Document::new("Doc B"),
            Document::new("Doc C"),
        ];

        let result = reranker
            .compress_documents(documents, "query", None)
            .await
            .unwrap();

        assert!(result.is_empty());
    }

    #[tokio::test]
    async fn test_top_n_larger_than_docs() {
        let llm = Arc::new(MockChatModel {
            ranking: vec![1, 0],
        });
        let reranker = LLMListwiseRerank::from_llm(llm, None).with_top_n(100);

        let documents = vec![Document::new("Doc A"), Document::new("Doc B")];

        let result = reranker
            .compress_documents(documents, "query", None)
            .await
            .unwrap();

        assert_eq!(result.len(), 2);
    }

    #[tokio::test]
    async fn test_out_of_range_document_ids() {
        // Model returns IDs that don't exist
        let llm = Arc::new(MockChatModel {
            ranking: vec![10, 20, 0], // 10 and 20 don't exist
        });
        let reranker = LLMListwiseRerank::from_llm(llm, None).with_top_n(3);

        let documents = vec![
            Document::new("Doc A"),
            Document::new("Doc B"),
            Document::new("Doc C"),
        ];

        let result = reranker
            .compress_documents(documents, "query", None)
            .await
            .unwrap();

        // Only valid ID (0) should be included, plus missing docs added back
        // Implementation adds docs that weren't in ranking
        assert!(result.len() <= 3);
        assert!(result.iter().any(|d| d.page_content == "Doc A"));
    }

    #[tokio::test]
    async fn test_duplicate_ids_in_ranking() {
        // Model returns duplicate IDs
        let llm = Arc::new(MockChatModel {
            ranking: vec![0, 0, 1],
        });
        let reranker = LLMListwiseRerank::from_llm(llm, None).with_top_n(3);

        let documents = vec![
            Document::new("Doc A"),
            Document::new("Doc B"),
            Document::new("Doc C"),
        ];

        let result = reranker
            .compress_documents(documents, "query", None)
            .await
            .unwrap();

        // Should handle duplicates gracefully
        assert!(result.len() <= 3);
    }

    #[tokio::test]
    async fn test_fallback_no_tool_call() {
        // Model doesn't return tool calls
        let llm = Arc::new(NoToolCallModel);
        let reranker = LLMListwiseRerank::from_llm(llm, None).with_top_n(2);

        let documents = vec![
            Document::new("Doc A"),
            Document::new("Doc B"),
            Document::new("Doc C"),
        ];

        let result = reranker
            .compress_documents(documents, "query", None)
            .await
            .unwrap();

        // Should fallback to original order
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].page_content, "Doc A");
        assert_eq!(result[1].page_content, "Doc B");
    }

    // ============================================================
    // METADATA PRESERVATION TESTS
    // ============================================================

    #[tokio::test]
    async fn test_preserves_metadata() {
        let llm = Arc::new(MockChatModel {
            ranking: vec![1, 0],
        });
        let reranker = LLMListwiseRerank::from_llm(llm, None).with_top_n(2);

        let mut doc1 = Document::new("Doc A");
        doc1.metadata
            .insert("source".to_string(), serde_json::json!("source_a"));

        let mut doc2 = Document::new("Doc B");
        doc2.metadata
            .insert("source".to_string(), serde_json::json!("source_b"));

        let documents = vec![doc1, doc2];
        let result = reranker
            .compress_documents(documents, "query", None)
            .await
            .unwrap();

        // Doc B should be first (ranked higher)
        assert_eq!(result[0].page_content, "Doc B");
        assert_eq!(
            result[0].metadata.get("source"),
            Some(&serde_json::json!("source_b"))
        );

        assert_eq!(result[1].page_content, "Doc A");
        assert_eq!(
            result[1].metadata.get("source"),
            Some(&serde_json::json!("source_a"))
        );
    }

    #[tokio::test]
    async fn test_preserves_document_id() {
        let llm = Arc::new(MockChatModel {
            ranking: vec![1, 0],
        });
        let reranker = LLMListwiseRerank::from_llm(llm, None).with_top_n(2);

        let documents = vec![
            Document::new("Doc A").with_id("id-a"),
            Document::new("Doc B").with_id("id-b"),
        ];

        let result = reranker
            .compress_documents(documents, "query", None)
            .await
            .unwrap();

        assert_eq!(result[0].id, Some("id-b".to_string()));
        assert_eq!(result[1].id, Some("id-a".to_string()));
    }

    // ============================================================
    // MANY DOCUMENTS TESTS
    // ============================================================

    #[tokio::test]
    async fn test_many_documents() {
        // Rank in reverse order
        let ranking: Vec<usize> = (0..50).rev().collect();
        let llm = Arc::new(MockChatModel { ranking });
        let reranker = LLMListwiseRerank::from_llm(llm, None).with_top_n(10);

        let documents: Vec<Document> = (0..50)
            .map(|i| Document::new(format!("Document {i}")))
            .collect();

        let result = reranker
            .compress_documents(documents, "query", None)
            .await
            .unwrap();

        assert_eq!(result.len(), 10);
        // First result should be Document 49 (highest ranked)
        assert_eq!(result[0].page_content, "Document 49");
    }

    // ============================================================
    // SPECIAL CONTENT TESTS
    // ============================================================

    #[tokio::test]
    async fn test_documents_with_special_characters() {
        let llm = Arc::new(MockChatModel {
            ranking: vec![0, 1],
        });
        let reranker = LLMListwiseRerank::from_llm(llm, None).with_top_n(2);

        let documents = vec![
            Document::new("Doc with ```code blocks``` and **markdown**"),
            Document::new("Doc with 日本語 and émojis 🦀"),
        ];

        let result = reranker
            .compress_documents(documents, "query", None)
            .await
            .unwrap();

        assert_eq!(result.len(), 2);
        assert!(result[0].page_content.contains("```code blocks```"));
        assert!(result[1].page_content.contains("🦀"));
    }

    #[tokio::test]
    async fn test_documents_with_newlines() {
        let llm = Arc::new(MockChatModel { ranking: vec![0] });
        let reranker = LLMListwiseRerank::from_llm(llm, None).with_top_n(1);

        let documents = vec![Document::new("Line 1\nLine 2\nLine 3")];

        let result = reranker
            .compress_documents(documents, "query", None)
            .await
            .unwrap();

        assert_eq!(result.len(), 1);
        assert_eq!(result[0].page_content, "Line 1\nLine 2\nLine 3");
    }
}
