//! Chain that combines documents using map-reduce pattern
//!
//! This chain first applies an LLM to each document individually (map step),
//! then combines the results using a reduce step.

use dashflow::core::documents::Document;
use dashflow::core::error::{Error, Result};
use dashflow::core::language_models::{ChatModel, LLM};
use dashflow::core::messages::HumanMessage;
use dashflow::core::prompts::PromptTemplate;
use futures::future::join_all;
use std::collections::HashMap;
use std::sync::Arc;

use super::{default_document_prompt, format_document, StuffDocumentsChain};

/// Chain that combines documents using a map-reduce pattern
///
/// The map-reduce pattern processes documents in two stages:
/// 1. **Map**: Apply an LLM to each document individually (parallelizable)
/// 2. **Reduce**: Combine the map results into a final output
///
/// This is useful when you have many documents that won't fit in a single prompt,
/// or when you want to process documents in parallel.
///
/// # Example
///
/// ```rust,ignore
/// use dashflow_chains::combine_documents::{MapReduceDocumentsChain, StuffDocumentsChain};
/// use dashflow::core::prompts::PromptTemplate;
///
/// // Map prompt: summarize each document
/// let map_prompt = PromptTemplate::from_template("Summarize this document:\n\n{page_content}").unwrap();
///
/// // Reduce chain: combine all summaries
/// let reduce_chain = StuffDocumentsChain::new_chat(llm.clone())
///     .with_prompt(PromptTemplate::from_template(
///         "Combine these summaries into one:\n\n{context}"
///     ));
///
/// let chain = MapReduceDocumentsChain::new_chat(llm)
///     .with_map_prompt(map_prompt)
///     .with_reduce_chain(reduce_chain);
///
/// let docs = vec![/* many documents */];
/// let (output, _) = chain.combine_docs(&docs, None).await?;
/// ```
pub struct MapReduceDocumentsChain {
    /// LLM for the map step
    map_llm: LLMType,

    /// Prompt for the map step (applied to each document)
    map_prompt: PromptTemplate,

    /// Document prompt for formatting documents in the map step
    document_prompt: PromptTemplate,

    /// Variable name for the document in the map prompt
    document_variable_name: String,

    /// Chain for the reduce step (combines map results)
    reduce_chain: StuffDocumentsChain,

    /// Whether to return intermediate map results
    return_intermediate_steps: bool,
}

/// Enum to hold either a `ChatModel` or LLM
#[allow(clippy::upper_case_acronyms)] // LLM is a well-known domain acronym in AI/ML
enum LLMType {
    Chat(Arc<dyn ChatModel>),
    LLM(Arc<dyn LLM>),
}

impl MapReduceDocumentsChain {
    /// Create a new `MapReduceDocumentsChain` with a `ChatModel`
    pub fn new_chat(map_llm: Arc<dyn ChatModel>) -> Self {
        #[allow(clippy::expect_used)]
        let reduce_prompt = PromptTemplate::from_template("Combine the following summaries:\n\n{context}")
            .expect("default reduce prompt should be valid");
        #[allow(clippy::expect_used)]
        let map_prompt = PromptTemplate::from_template("Summarize:\n\n{page_content}")
            .expect("default map prompt should be valid");

        // Default reduce chain
        let reduce_chain =
            StuffDocumentsChain::new_chat(Arc::clone(&map_llm)).with_prompt(reduce_prompt);

        Self {
            map_llm: LLMType::Chat(map_llm),
            map_prompt,
            document_prompt: default_document_prompt(),
            document_variable_name: "page_content".to_string(),
            reduce_chain,
            return_intermediate_steps: false,
        }
    }

    /// Create a new `MapReduceDocumentsChain` with an LLM
    pub fn new_llm(map_llm: Arc<dyn LLM>) -> Self {
        #[allow(clippy::expect_used)]
        let reduce_prompt = PromptTemplate::from_template("Combine the following summaries:\n\n{context}")
            .expect("default reduce prompt should be valid");
        #[allow(clippy::expect_used)]
        let map_prompt = PromptTemplate::from_template("Summarize:\n\n{page_content}")
            .expect("default map prompt should be valid");

        let reduce_chain =
            StuffDocumentsChain::new_llm(Arc::clone(&map_llm)).with_prompt(reduce_prompt);

        Self {
            map_llm: LLMType::LLM(map_llm),
            map_prompt,
            document_prompt: default_document_prompt(),
            document_variable_name: "page_content".to_string(),
            reduce_chain,
            return_intermediate_steps: false,
        }
    }

    /// Set the map prompt template
    #[must_use]
    pub fn with_map_prompt(mut self, prompt: PromptTemplate) -> Self {
        self.map_prompt = prompt;
        self
    }

    /// Set the document prompt for the map step
    #[must_use]
    pub fn with_document_prompt(mut self, document_prompt: PromptTemplate) -> Self {
        self.document_prompt = document_prompt;
        self
    }

    /// Set the document variable name for the map step
    pub fn with_document_variable_name(mut self, name: impl Into<String>) -> Self {
        self.document_variable_name = name.into();
        self
    }

    /// Set the reduce chain
    #[must_use]
    pub fn with_reduce_chain(mut self, reduce_chain: StuffDocumentsChain) -> Self {
        self.reduce_chain = reduce_chain;
        self
    }

    /// Set whether to return intermediate map results
    #[must_use]
    pub fn with_return_intermediate_steps(mut self, return_steps: bool) -> Self {
        self.return_intermediate_steps = return_steps;
        self
    }

    /// Apply the map step to a single document
    async fn map_doc(&self, doc: &Document, kwargs: &HashMap<String, String>) -> Result<String> {
        // Format the document
        let formatted_doc = format_document(doc, &self.document_prompt)?;

        // Build prompt variables
        let mut prompt_vars = kwargs.clone();
        prompt_vars.insert(self.document_variable_name.clone(), formatted_doc);

        // Also add metadata
        for (key, value) in &doc.metadata {
            if let Some(s) = value.as_str() {
                prompt_vars.insert(key.clone(), s.to_string());
            }
        }

        let prompt_text = self.map_prompt.format(&prompt_vars)?;

        // Call the LLM
        let output = match &self.map_llm {
            LLMType::Chat(chat_model) => {
                let messages = vec![HumanMessage::new(prompt_text).into()];
                let result = chat_model
                    .generate(&messages, None, None, None, None)
                    .await?;
                result
                    .generations
                    .first()
                    .ok_or_else(|| Error::other("LLM returned empty response in map step"))?
                    .text()
            }
            LLMType::LLM(llm) => {
                let result = llm.generate(&[prompt_text], None, None).await?;
                result
                    .generations
                    .first()
                    .and_then(|g| g.first())
                    .ok_or_else(|| Error::other("LLM returned empty response in map step"))?
                    .text
                    .clone()
            }
        };

        Ok(output)
    }

    /// Combine documents using map-reduce pattern
    ///
    /// # Arguments
    ///
    /// * `docs` - List of documents to process
    /// * `kwargs` - Additional variables for prompts
    ///
    /// # Returns
    ///
    /// Tuple of (`output_text`, `extra_return_dict`)
    /// If `return_intermediate_steps` is true, `extra_return_dict` will contain
    /// "`intermediate_steps`" key with the map results
    pub async fn combine_docs(
        &self,
        docs: &[Document],
        kwargs: Option<HashMap<String, String>>,
    ) -> Result<(String, HashMap<String, String>)> {
        let kwargs = kwargs.unwrap_or_default();

        // Map step: process each document in parallel
        let map_futures: Vec<_> = docs.iter().map(|doc| self.map_doc(doc, &kwargs)).collect();

        let map_results = join_all(map_futures).await;

        // Collect results and check for errors
        let map_outputs: Result<Vec<String>> = map_results.into_iter().collect();
        let map_outputs = map_outputs?;

        // Create documents from map results
        let result_docs: Vec<Document> = map_outputs
            .iter()
            .enumerate()
            .map(|(i, output)| {
                let mut doc = Document::new(output);
                // Preserve original metadata
                if i < docs.len() {
                    doc.metadata = docs[i].metadata.clone();
                }
                doc
            })
            .collect();

        // Reduce step: combine map results
        let (final_output, mut extra_dict) = self
            .reduce_chain
            .combine_docs(&result_docs, Some(kwargs))
            .await?;

        // Add intermediate steps if requested
        if self.return_intermediate_steps {
            let steps_json = serde_json::to_string(&map_outputs)
                .map_err(|e| Error::other(format!("Serialization error: {e}")))?;
            extra_dict.insert("intermediate_steps".to_string(), steps_json);
        }

        Ok((final_output, extra_dict))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use dashflow::core::language_models::{ChatGeneration, ChatResult, ToolChoice, ToolDefinition};
    use dashflow::core::messages::{AIMessage, BaseMessage};

    struct MockChatModel {
        // Maps input substring to response
        responses: HashMap<String, String>,
        default_response: String,
    }

    impl MockChatModel {
        fn new(default_response: impl Into<String>) -> Self {
            Self {
                responses: HashMap::new(),
                default_response: default_response.into(),
            }
        }

        fn with_response(mut self, key: impl Into<String>, response: impl Into<String>) -> Self {
            self.responses.insert(key.into(), response.into());
            self
        }
    }

    #[async_trait]
    impl ChatModel for MockChatModel {
        fn llm_type(&self) -> &str {
            "mock"
        }
        fn as_any(&self) -> &dyn std::any::Any {
            self
        }

        async fn _generate(
            &self,
            messages: &[BaseMessage],
            _stop: Option<&[String]>,
            _tools: Option<&[ToolDefinition]>,
            _tool_choice: Option<&ToolChoice>,
            _run_manager: Option<&dashflow::core::callbacks::CallbackManager>,
        ) -> Result<ChatResult> {
            let content = messages[0].content().as_text();

            // Find matching response based on content
            let response = self
                .responses
                .iter()
                .find(|(key, _)| content.contains(key.as_str()))
                .map(|(_, v)| v.clone())
                .unwrap_or_else(|| self.default_response.clone());

            let message = AIMessage::new(response).into();
            Ok(ChatResult::new(ChatGeneration::new(message)))
        }
    }

    #[tokio::test]
    async fn test_map_reduce_chain() {
        let llm = Arc::new(
            MockChatModel::new("Combined summary")
                .with_response("First", "Summary of first doc")
                .with_response("Second", "Summary of second doc")
                .with_response("Third", "Summary of third doc"),
        );

        let chain = MapReduceDocumentsChain::new_chat(llm);

        let docs = vec![
            Document::new("First document"),
            Document::new("Second document"),
            Document::new("Third document"),
        ];

        let (output, _) = chain.combine_docs(&docs, None).await.unwrap();
        assert_eq!(output, "Combined summary");
    }

    #[tokio::test]
    async fn test_map_reduce_with_intermediate_steps() {
        let llm = Arc::new(
            MockChatModel::new("Final")
                .with_response("Doc1", "Map1")
                .with_response("Doc2", "Map2"),
        );

        let chain = MapReduceDocumentsChain::new_chat(llm).with_return_intermediate_steps(true);

        let docs = vec![Document::new("Doc1"), Document::new("Doc2")];

        let (output, extra) = chain.combine_docs(&docs, None).await.unwrap();

        assert_eq!(output, "Final");
        assert!(extra.contains_key("intermediate_steps"));

        // Verify intermediate steps contains the map results
        let steps: Vec<String> =
            serde_json::from_str(extra.get("intermediate_steps").unwrap()).unwrap();
        assert_eq!(steps.len(), 2);
    }

    #[tokio::test]
    async fn test_map_reduce_custom_prompts() {
        let llm: Arc<dyn ChatModel> = Arc::new(MockChatModel::new("Custom result"));

        let map_prompt =
            PromptTemplate::from_template("Extract key points from: {page_content}").unwrap();
        let reduce_prompt =
            PromptTemplate::from_template("Merge these points:\n{context}").unwrap();

        let reduce_chain = StuffDocumentsChain::new_chat(Arc::clone(&llm)).with_prompt(reduce_prompt);

        let chain = MapReduceDocumentsChain::new_chat(llm)
            .with_map_prompt(map_prompt)
            .with_reduce_chain(reduce_chain);

        let docs = vec![Document::new("Document content")];

        let (output, _) = chain.combine_docs(&docs, None).await.unwrap();
        assert_eq!(output, "Custom result");
    }
}
