//! LLM-based metadata extraction for documents
//!
//! This module implements metadata extraction from document content using
//! structured output (tool calling) with language models.
//!
//! # Overview
//!
//! `MetadataTagger` uses an LLM with tool calling to extract structured metadata
//! from document content according to a user-defined schema. The extracted
//! metadata is merged with existing document metadata.
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
//! use dashflow::core::document_transformers::MetadataTagger;
//! use dashflow_openai::ChatOpenAI;
//! use dashflow::core::documents::Document;
//! use serde_json::json;
//!
//! let llm = ChatOpenAI::with_config(Default::default()).with_model("gpt-4o-mini");
//!
//! let schema = json!({
//!     "properties": {
//!         "movie_title": { "type": "string" },
//!         "critic": { "type": "string" },
//!         "tone": {
//!             "type": "string",
//!             "enum": ["positive", "negative"]
//!         },
//!         "rating": {
//!             "type": "integer",
//!             "description": "The number of stars the critic rated the movie"
//!         }
//!     },
//!     "required": ["movie_title", "critic", "tone"]
//! });
//!
//! let tagger = MetadataTagger::from_llm(llm, schema, None);
//!
//! let documents = vec![
//!     Document::new("Review of The Bee Movie\nBy Roger Ebert\n\nThis is the greatest movie ever made. 4 out of 5 stars."),
//! ];
//!
//! let tagged = tagger.transform_documents(documents).await?;
//! // tagged[0].metadata now contains: { "movie_title": "The Bee Movie", "critic": "Roger Ebert", "tone": "positive", "rating": 4 }
//! ```

use crate::core::document_transformers::DocumentTransformer;
use crate::core::documents::Document;
use crate::core::error::{Error, Result};
use crate::core::language_models::{ChatModel, ToolChoice, ToolDefinition};
use crate::core::messages::{BaseMessage, Message};
use async_trait::async_trait;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;

/// Default prompt template for metadata extraction
const DEFAULT_PROMPT: &str = "Extract metadata from the following document:\n\n{content}";

/// Document transformer that extracts metadata using an LLM with structured output.
///
/// This transformer uses a language model with tool calling capabilities to extract
/// structured metadata from document content according to a JSON Schema. The extracted
/// metadata is merged with the document's existing metadata.
///
/// # Example
///
/// ```rust,ignore
/// use dashflow::core::document_transformers::MetadataTagger;
/// use dashflow_openai::ChatOpenAI;
/// use serde_json::json;
///
/// let llm = ChatOpenAI::with_config(Default::default()).with_model("gpt-4o-mini");
/// let schema = json!({
///     "properties": {
///         "category": { "type": "string" },
///         "sentiment": { "type": "string", "enum": ["positive", "negative", "neutral"] }
///     },
///     "required": ["category"]
/// });
///
/// let tagger = MetadataTagger::from_llm(llm, schema, None);
/// let tagged = tagger.transform_documents(documents).await?;
/// ```
pub struct MetadataTagger {
    /// Language model for metadata extraction (must support tool calling)
    llm: Arc<dyn ChatModel>,
    /// Prompt template (must contain {content} placeholder)
    prompt_template: String,
    /// Tool definition for structured output
    tool_definition: ToolDefinition,
}

impl MetadataTagger {
    /// Create a new `MetadataTagger` from a chat model and metadata schema.
    ///
    /// # Arguments
    ///
    /// * `llm` - Language model that supports tool calling / function calling
    /// * `metadata_schema` - JSON Schema describing the metadata to extract.
    ///   Should be a valid JSON Schema object with "type": "object" and
    ///   "properties" describing the metadata fields.
    /// * `prompt_template` - Optional custom prompt. If None, uses default.
    ///   Must contain {content} placeholder for document content.
    ///
    /// # Returns
    ///
    /// A new `MetadataTagger` configured with the given schema.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use dashflow_openai::ChatOpenAI;
    /// use serde_json::json;
    ///
    /// let llm = ChatOpenAI::with_config(Default::default()).with_model("gpt-4o-mini");
    /// let schema = json!({
    ///     "properties": {
    ///         "author": { "type": "string" },
    ///         "date": { "type": "string" }
    ///     },
    ///     "required": ["author"]
    /// });
    ///
    /// let tagger = MetadataTagger::from_llm(llm, schema, None);
    /// ```
    pub fn from_llm(
        llm: Arc<dyn ChatModel>,
        metadata_schema: Value,
        prompt_template: Option<String>,
    ) -> Self {
        let prompt_template = prompt_template.unwrap_or_else(|| DEFAULT_PROMPT.to_string());

        // Define the tool/function for structured output
        let tool_definition = ToolDefinition {
            name: "extract_metadata".to_string(),
            description:
                "Extract structured metadata from the document content according to the schema."
                    .to_string(),
            parameters: metadata_schema,
        };

        Self {
            llm,
            prompt_template,
            tool_definition,
        }
    }

    /// Set a custom prompt template.
    ///
    /// The template must contain a `{content}` placeholder where the document
    /// content will be inserted.
    ///
    /// # Arguments
    ///
    /// * `template` - Custom prompt template
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let tagger = MetadataTagger::from_llm(llm, schema, None)
    ///     .with_prompt_template("Analyze this text and extract metadata:\n{content}");
    /// ```
    #[must_use]
    pub fn with_prompt_template(mut self, template: impl Into<String>) -> Self {
        self.prompt_template = template.into();
        self
    }

    /// Extract metadata from a single document using the LLM.
    ///
    /// # Arguments
    ///
    /// * `document` - Document to extract metadata from
    ///
    /// # Returns
    ///
    /// A `HashMap` of extracted metadata fields, or an empty `HashMap` if extraction fails.
    async fn extract_metadata(&self, document: &Document) -> Result<HashMap<String, Value>> {
        // Build the prompt with document content
        let prompt = self
            .prompt_template
            .replace("{content}", &document.page_content);

        // Create message
        let messages: Vec<BaseMessage> = vec![Message::human(prompt)];

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
                        // Parse the tool call arguments as a HashMap
                        let metadata: HashMap<String, Value> =
                            serde_json::from_value(tool_call.args.clone()).map_err(|e| {
                                Error::OutputParsing(format!(
                                    "Failed to parse metadata extraction result: {e}"
                                ))
                            })?;
                        return Ok(metadata);
                    }
                }
            }
        }

        // If LLM didn't provide valid metadata, return empty map
        Ok(HashMap::new())
    }
}

#[async_trait]
impl DocumentTransformer for MetadataTagger {
    /// Transform documents by extracting and adding metadata using the LLM.
    ///
    /// For each document, this method:
    /// 1. Calls the LLM to extract metadata according to the schema
    /// 2. Merges the extracted metadata with existing metadata
    /// 3. Returns a new document with the combined metadata
    ///
    /// Extracted metadata takes precedence over existing metadata if there are conflicts.
    ///
    /// # Arguments
    ///
    /// * `documents` - Documents to extract metadata from
    ///
    /// # Returns
    ///
    /// A vector of documents with extracted metadata added.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let documents = vec![Document::new("Review of Rust...")];
    /// let tagged = tagger.transform_documents(documents).await?;
    /// ```
    fn transform_documents(&self, _documents: Vec<Document>) -> Result<Vec<Document>> {
        // Synchronous version not supported for LLM-based transformers
        Err(Error::NotImplemented(
            "MetadataTagger requires async operation. Use atransform_documents instead."
                .to_string(),
        ))
    }

    /// Transform documents asynchronously by extracting and adding metadata.
    ///
    /// This is the primary method for using `MetadataTagger`. For each document,
    /// it extracts metadata using the LLM and merges it with existing metadata.
    ///
    /// # Arguments
    ///
    /// * `documents` - Documents to extract metadata from
    ///
    /// # Returns
    ///
    /// A vector of documents with extracted metadata added.
    async fn atransform_documents(&self, documents: Vec<Document>) -> Result<Vec<Document>> {
        let mut result = Vec::with_capacity(documents.len());

        for document in documents {
            // Extract metadata for this document
            let extracted_metadata = self.extract_metadata(&document).await?;

            // Merge with existing metadata
            // Start with existing metadata
            let mut combined_metadata: HashMap<String, Value> = document.metadata.clone();

            // Add extracted metadata (overwrites existing keys)
            combined_metadata.extend(extracted_metadata);

            // Create new document with combined metadata
            result.push(Document {
                page_content: document.page_content,
                metadata: combined_metadata,
                id: document.id,
            });
        }

        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use crate::core::language_models::{ChatGeneration, ChatModel, ChatResult};
    use crate::core::messages::{Message, ToolCall};
    use crate::test_prelude::*;
    use serde_json::json;

    // Mock ChatModel for testing
    struct MockChatModel {
        metadata: Value,
    }

    #[async_trait]
    impl ChatModel for MockChatModel {
        async fn _generate(
            &self,
            _messages: &[BaseMessage],
            _stop: Option<&[String]>,
            _tools: Option<&[ToolDefinition]>,
            _tool_choice: Option<&ToolChoice>,
            _run_manager: Option<&crate::core::callbacks::CallbackManager>,
        ) -> Result<ChatResult> {
            let tool_call = ToolCall {
                id: "call_1".to_string(),
                name: "extract_metadata".to_string(),
                args: self.metadata.clone(),
                tool_type: "tool_call".to_string(),
                index: None,
            };

            let ai_message = Message::AI {
                content: "Extracted metadata".into(),
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

    #[tokio::test]
    async fn test_metadata_tagger_basic() {
        let mock_llm = MockChatModel {
            metadata: json!({
                "movie_title": "The Bee Movie",
                "critic": "Roger Ebert",
                "tone": "positive",
                "rating": 4
            }),
        };

        let schema = json!({
            "type": "object",
            "properties": {
                "movie_title": { "type": "string" },
                "critic": { "type": "string" },
                "tone": { "type": "string", "enum": ["positive", "negative"] },
                "rating": { "type": "integer" }
            },
            "required": ["movie_title", "critic", "tone"]
        });

        let tagger = MetadataTagger::from_llm(Arc::new(mock_llm), schema, None);

        let documents = vec![Document::new(
            "Review of The Bee Movie\nBy Roger Ebert\n\nThis is the greatest movie ever made. 4 out of 5 stars.",
        )];

        let result = tagger.atransform_documents(documents).await.unwrap();
        assert_eq!(result.len(), 1);

        let metadata = &result[0].metadata;
        assert_eq!(
            metadata.get("movie_title").unwrap().as_str().unwrap(),
            "The Bee Movie"
        );
        assert_eq!(
            metadata.get("critic").unwrap().as_str().unwrap(),
            "Roger Ebert"
        );
        assert_eq!(metadata.get("tone").unwrap().as_str().unwrap(), "positive");
        assert_eq!(metadata.get("rating").unwrap().as_i64().unwrap(), 4);
    }

    #[tokio::test]
    async fn test_metadata_tagger_preserves_existing_metadata() {
        let mock_llm = MockChatModel {
            metadata: json!({
                "sentiment": "positive"
            }),
        };

        let schema = json!({
            "type": "object",
            "properties": {
                "sentiment": { "type": "string" }
            }
        });

        let tagger = MetadataTagger::from_llm(Arc::new(mock_llm), schema, None);

        let mut doc = Document::new("Great product!");
        doc.metadata.insert("source".to_string(), json!("amazon"));

        let result = tagger.atransform_documents(vec![doc]).await.unwrap();
        assert_eq!(result.len(), 1);

        let metadata = &result[0].metadata;
        assert_eq!(
            metadata.get("sentiment").unwrap().as_str().unwrap(),
            "positive"
        );
        assert_eq!(metadata.get("source").unwrap().as_str().unwrap(), "amazon");
    }

    #[tokio::test]
    async fn test_metadata_tagger_custom_prompt() {
        let mock_llm = MockChatModel {
            metadata: json!({
                "category": "technology"
            }),
        };

        let schema = json!({
            "type": "object",
            "properties": {
                "category": { "type": "string" }
            }
        });

        let tagger = MetadataTagger::from_llm(Arc::new(mock_llm), schema, None)
            .with_prompt_template("Categorize this text:\n\n{content}".to_string());

        let documents = vec![Document::new("Rust is a systems programming language.")];
        let result = tagger.atransform_documents(documents).await.unwrap();

        assert_eq!(result.len(), 1);
        assert_eq!(
            result[0]
                .metadata
                .get("category")
                .unwrap()
                .as_str()
                .unwrap(),
            "technology"
        );
    }

    #[tokio::test]
    async fn test_metadata_tagger_fallback_on_no_tool_calls() {
        // Mock that returns no tool calls
        struct NoToolCallMock;

        #[async_trait]
        impl ChatModel for NoToolCallMock {
            async fn _generate(
                &self,
                _messages: &[BaseMessage],
                _stop: Option<&[String]>,
                _tools: Option<&[ToolDefinition]>,
                _tool_choice: Option<&ToolChoice>,
                _run_manager: Option<&crate::core::callbacks::CallbackManager>,
            ) -> Result<ChatResult> {
                let ai_message = Message::AI {
                    content: "No metadata extracted".into(),
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

        let schema = json!({
            "type": "object",
            "properties": {
                "category": { "type": "string" }
            }
        });

        let tagger = MetadataTagger::from_llm(Arc::new(NoToolCallMock), schema, None);
        let documents = vec![Document::new("Test content")];

        let result = tagger.atransform_documents(documents).await.unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].page_content, "Test content");
        // Should have no extracted metadata, only original (empty)
        assert!(result[0].metadata.is_empty());
    }
}
