//! Chain that combines documents by stuffing them all into context
//!
//! This is the simplest document combining strategy. It takes all documents,
//! formats them according to a document prompt, joins them with a separator,
//! and passes the combined text to an LLM.

use dashflow::core::documents::Document;
use dashflow::core::error::{Error, Result};
use dashflow::core::language_models::{ChatModel, LLM};
use dashflow::core::messages::HumanMessage;
use dashflow::core::prompts::PromptTemplate;
use std::collections::HashMap;
use std::sync::Arc;

use super::{
    default_document_prompt, format_documents, DEFAULT_DOCUMENTS_KEY, DEFAULT_DOCUMENT_SEPARATOR,
};

/// Chain that combines documents by stuffing them into a single prompt
///
/// This chain takes a list of documents and combines them into a single string
/// by formatting each document with `document_prompt` and joining them with
/// `document_separator`. The combined text is then inserted into the main prompt
/// and passed to the language model.
///
/// This is the simplest combination strategy but may hit token limits with many documents.
///
/// # Example
///
/// ```rust,ignore
/// use dashflow_chains::combine_documents::StuffDocumentsChain;
/// use dashflow::core::documents::Document;
/// use dashflow::core::prompts::PromptTemplate;
///
/// let chain = StuffDocumentsChain::new(llm)
///     .with_prompt(PromptTemplate::from_template("Summarize these docs:\n\n{context}").unwrap())
///     .with_document_variable_name("context");
///
/// let docs = vec![
///     Document::new("First document"),
///     Document::new("Second document"),
/// ];
///
/// let (output, _) = chain.combine_docs(&docs, None).await?;
/// println!("Summary: {}", output);
/// ```
pub struct StuffDocumentsChain {
    /// The language model to use (can be `ChatModel` or LLM)
    llm: LLMType,

    /// Prompt template for the final LLM call
    /// Must contain `document_variable_name` as an input variable
    prompt: PromptTemplate,

    /// Prompt template for formatting each individual document
    document_prompt: PromptTemplate,

    /// Variable name in the main prompt where combined documents will be inserted
    document_variable_name: String,

    /// Separator string to join formatted documents
    document_separator: String,
}

/// Enum to hold either a `ChatModel` or LLM
#[allow(clippy::upper_case_acronyms)] // LLM is a well-known domain acronym in AI/ML
enum LLMType {
    Chat(Arc<dyn ChatModel>),
    LLM(Arc<dyn LLM>),
}

impl StuffDocumentsChain {
    /// Create a new `StuffDocumentsChain` with a `ChatModel`
    ///
    /// Uses default settings:
    /// - `document_prompt`: "{`page_content`}"
    /// - `document_variable_name`: "context"
    /// - `document_separator`: "\n\n"
    /// - prompt: "Summarize the following:\n\n{context}"
    pub fn new_chat(llm: Arc<dyn ChatModel>) -> Self {
        #[allow(clippy::expect_used)]
        let prompt = PromptTemplate::from_template("Summarize the following:\n\n{context}")
            .expect("default prompt should be valid");

        Self {
            llm: LLMType::Chat(llm),
            prompt,
            document_prompt: default_document_prompt(),
            document_variable_name: DEFAULT_DOCUMENTS_KEY.to_string(),
            document_separator: DEFAULT_DOCUMENT_SEPARATOR.to_string(),
        }
    }

    /// Create a new `StuffDocumentsChain` with an LLM
    pub fn new_llm(llm: Arc<dyn LLM>) -> Self {
        #[allow(clippy::expect_used)]
        let prompt = PromptTemplate::from_template("Summarize the following:\n\n{context}")
            .expect("default prompt should be valid");

        Self {
            llm: LLMType::LLM(llm),
            prompt,
            document_prompt: default_document_prompt(),
            document_variable_name: DEFAULT_DOCUMENTS_KEY.to_string(),
            document_separator: DEFAULT_DOCUMENT_SEPARATOR.to_string(),
        }
    }

    /// Set the main prompt template
    #[must_use]
    pub fn with_prompt(mut self, prompt: PromptTemplate) -> Self {
        self.prompt = prompt;
        self
    }

    /// Set the document formatting prompt
    #[must_use]
    pub fn with_document_prompt(mut self, document_prompt: PromptTemplate) -> Self {
        self.document_prompt = document_prompt;
        self
    }

    /// Set the document variable name
    pub fn with_document_variable_name(mut self, name: impl Into<String>) -> Self {
        self.document_variable_name = name.into();
        self
    }

    /// Set the document separator
    pub fn with_document_separator(mut self, separator: impl Into<String>) -> Self {
        self.document_separator = separator.into();
        self
    }

    /// Validate that the prompt contains the document variable
    fn validate(&self) -> Result<()> {
        if !self
            .prompt
            .input_variables
            .contains(&self.document_variable_name)
        {
            return Err(Error::invalid_input(format!(
                "Prompt must contain '{}' as an input variable. Found: {:?}",
                self.document_variable_name, self.prompt.input_variables
            )));
        }
        Ok(())
    }

    /// Combine documents into a single output using the language model
    ///
    /// # Arguments
    ///
    /// * `docs` - List of documents to combine
    /// * `kwargs` - Additional variables for the prompt (e.g., question, context)
    ///
    /// # Returns
    ///
    /// Tuple of (`output_text`, `extra_return_dict`)
    pub async fn combine_docs(
        &self,
        docs: &[Document],
        kwargs: Option<HashMap<String, String>>,
    ) -> Result<(String, HashMap<String, String>)> {
        self.validate()?;

        // Format and join all documents
        let formatted_docs =
            format_documents(docs, &self.document_prompt, &self.document_separator)?;

        // Build the final prompt
        let mut prompt_vars = kwargs.unwrap_or_default();
        prompt_vars.insert(self.document_variable_name.clone(), formatted_docs);

        let prompt_text = self.prompt.format(&prompt_vars)?;

        // Call the LLM
        let output = match &self.llm {
            LLMType::Chat(chat_model) => {
                let messages = vec![HumanMessage::new(prompt_text).into()];
                let result = chat_model
                    .generate(&messages, None, None, None, None)
                    .await?;
                result
                    .generations
                    .first()
                    .ok_or_else(|| Error::other("LLM returned empty response"))?
                    .text()
            }
            LLMType::LLM(llm) => {
                let result = llm.generate(&[prompt_text], None, None).await?;
                result
                    .generations
                    .first()
                    .and_then(|g| g.first())
                    .ok_or_else(|| Error::other("LLM returned empty response"))?
                    .text
                    .clone()
            }
        };

        Ok((output, HashMap::new()))
    }

    /// Get the estimated prompt length for the given documents
    ///
    /// This can be used to check if documents will fit within token limits
    /// before calling `combine_docs`.
    ///
    /// # Returns
    ///
    /// Approximate number of tokens (currently just character count / 4)
    pub fn prompt_length(
        &self,
        docs: &[Document],
        kwargs: Option<HashMap<String, String>>,
    ) -> Result<usize> {
        let formatted_docs =
            format_documents(docs, &self.document_prompt, &self.document_separator)?;

        let mut prompt_vars = kwargs.unwrap_or_default();
        prompt_vars.insert(self.document_variable_name.clone(), formatted_docs);

        let prompt_text = self.prompt.format(&prompt_vars)?;

        // Rough approximation: 1 token ≈ 4 characters
        Ok(prompt_text.len() / 4)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use dashflow::core::language_models::{ChatGeneration, ChatResult, Generation, LLMResult};
    use dashflow::core::language_models::{ToolChoice, ToolDefinition};
    use dashflow::core::messages::{AIMessage, BaseMessage};

    /// Mock ChatModel for testing
    struct MockChatModel {
        response: String,
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
            _messages: &[BaseMessage],
            _stop: Option<&[String]>,
            _tools: Option<&[ToolDefinition]>,
            _tool_choice: Option<&ToolChoice>,
            _run_manager: Option<&dashflow::core::callbacks::CallbackManager>,
        ) -> Result<ChatResult> {
            let message = AIMessage::new(self.response.clone()).into();
            Ok(ChatResult::new(ChatGeneration::new(message)))
        }
    }

    struct MockLLM {
        response: String,
    }

    #[async_trait]
    impl LLM for MockLLM {
        async fn _generate(
            &self,
            _prompts: &[String],
            _stop: Option<&[String]>,
            _run_manager: Option<&dashflow::core::callbacks::CallbackManager>,
        ) -> Result<LLMResult> {
            Ok(LLMResult::new(Generation::new(&self.response)))
        }

        fn llm_type(&self) -> &str {
            "mock"
        }
    }

    #[tokio::test]
    async fn test_stuff_documents_chain_chat() {
        let llm = Arc::new(MockChatModel {
            response: "Summary of documents".to_string(),
        });

        let chain = StuffDocumentsChain::new_chat(llm)
            .with_prompt(PromptTemplate::from_template("Summarize:\n{context}").unwrap());

        let docs = vec![
            Document::new("First document"),
            Document::new("Second document"),
        ];

        let (output, _) = chain.combine_docs(&docs, None).await.unwrap();
        assert_eq!(output, "Summary of documents");
    }

    #[tokio::test]
    async fn test_stuff_documents_chain_llm() {
        let llm = Arc::new(MockLLM {
            response: "Summary of documents".to_string(),
        });

        let chain = StuffDocumentsChain::new_llm(llm)
            .with_prompt(PromptTemplate::from_template("Summarize:\n{context}").unwrap());

        let docs = vec![
            Document::new("First document"),
            Document::new("Second document"),
        ];

        let (output, _) = chain.combine_docs(&docs, None).await.unwrap();
        assert_eq!(output, "Summary of documents");
    }

    #[tokio::test]
    async fn test_stuff_documents_chain_with_kwargs() {
        let llm = Arc::new(MockChatModel {
            response: "Answer".to_string(),
        });

        let chain = StuffDocumentsChain::new_chat(llm).with_prompt(
            PromptTemplate::from_template("Question: {question}\n\nContext: {context}").unwrap(),
        );

        let docs = vec![Document::new("Document content")];

        let mut kwargs = HashMap::new();
        kwargs.insert("question".to_string(), "What is this about?".to_string());

        let (output, _) = chain.combine_docs(&docs, Some(kwargs)).await.unwrap();
        assert_eq!(output, "Answer");
    }

    #[test]
    fn test_prompt_validation() {
        let llm = Arc::new(MockChatModel {
            response: "test".to_string(),
        });

        let chain = StuffDocumentsChain::new_chat(llm)
            .with_prompt(PromptTemplate::from_template("No context variable here").unwrap())
            .with_document_variable_name("context");

        let result = chain.validate();
        assert!(result.is_err());
    }

    #[test]
    fn test_prompt_length() {
        let llm = Arc::new(MockChatModel {
            response: "test".to_string(),
        });

        let chain = StuffDocumentsChain::new_chat(llm);

        let docs = vec![Document::new("Short document")];

        let length = chain.prompt_length(&docs, None).unwrap();
        assert!(length > 0);
    }
}
