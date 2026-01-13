//! Chain that combines documents by iteratively refining an answer
//!
//! This chain processes documents sequentially, refining the answer with each new document.

use dashflow::core::documents::Document;
use dashflow::core::error::{Error, Result};
use dashflow::core::language_models::{ChatModel, LLM};
use dashflow::core::messages::HumanMessage;
use dashflow::core::prompts::PromptTemplate;
use std::collections::HashMap;
use std::sync::Arc;

use super::{default_document_prompt, format_document};

/// Chain that combines documents by iterative refinement
///
/// This chain processes documents sequentially:
/// 1. Process the first document with an initial prompt to generate an initial answer
/// 2. For each subsequent document, refine the answer using a refine prompt that includes
///    both the current answer and the new document
///
/// This is useful when you want to build up an answer incrementally, or when you want
/// to ensure each document is considered in order.
///
/// # Example
///
/// ```rust,ignore
/// use dashflow_chains::combine_documents::RefineDocumentsChain;
/// use dashflow::core::prompts::PromptTemplate;
///
/// let initial_prompt = PromptTemplate::from_template(
///     "Write an answer based on this context:\n\n{page_content}"
/// );
///
/// let refine_prompt = PromptTemplate::from_template(
///     "Here's the existing answer:\n{existing_answer}\n\n\
///      Now refine it with this new context:\n{page_content}"
/// );
///
/// let chain = RefineDocumentsChain::new_chat(llm)
///     .with_initial_prompt(initial_prompt)
///     .with_refine_prompt(refine_prompt);
///
/// let docs = vec![/* documents */];
/// let (output, _) = chain.combine_docs(&docs, None).await?;
/// ```
pub struct RefineDocumentsChain {
    /// LLM to use for both initial and refine steps
    llm: LLMType,

    /// Prompt for the first document
    initial_prompt: PromptTemplate,

    /// Prompt for refining with subsequent documents
    refine_prompt: PromptTemplate,

    /// Document prompt for formatting documents
    document_prompt: PromptTemplate,

    /// Variable name for the document content in prompts
    document_variable_name: String,

    /// Variable name for the existing answer in refine prompt
    initial_response_name: String,

    /// Whether to return intermediate refine steps
    return_intermediate_steps: bool,
}

/// Enum to hold either a `ChatModel` or LLM
#[allow(clippy::upper_case_acronyms)] // LLM is a well-known domain acronym in AI/ML
enum LLMType {
    Chat(Arc<dyn ChatModel>),
    LLM(Arc<dyn LLM>),
}

impl RefineDocumentsChain {
    /// Create a new `RefineDocumentsChain` with a `ChatModel`
    pub fn new_chat(llm: Arc<dyn ChatModel>) -> Self {
        #[allow(clippy::expect_used)]
        let initial_prompt = PromptTemplate::from_template(
            "Write an answer based on this context:\n\n{page_content}",
        )
        .expect("default initial prompt should be valid");
        #[allow(clippy::expect_used)]
        let refine_prompt = PromptTemplate::from_template(
            "We have an existing answer:\n{existing_answer}\n\n\
                 Refine the answer with this new context:\n{page_content}",
        )
        .expect("default refine prompt should be valid");

        Self {
            llm: LLMType::Chat(llm),
            initial_prompt,
            refine_prompt,
            document_prompt: default_document_prompt(),
            document_variable_name: "page_content".to_string(),
            initial_response_name: "existing_answer".to_string(),
            return_intermediate_steps: false,
        }
    }

    /// Create a new `RefineDocumentsChain` with an LLM
    pub fn new_llm(llm: Arc<dyn LLM>) -> Self {
        #[allow(clippy::expect_used)]
        let initial_prompt = PromptTemplate::from_template(
            "Write an answer based on this context:\n\n{page_content}",
        )
        .expect("default initial prompt should be valid");
        #[allow(clippy::expect_used)]
        let refine_prompt = PromptTemplate::from_template(
            "We have an existing answer:\n{existing_answer}\n\n\
                 Refine the answer with this new context:\n{page_content}",
        )
        .expect("default refine prompt should be valid");

        Self {
            llm: LLMType::LLM(llm),
            initial_prompt,
            refine_prompt,
            document_prompt: default_document_prompt(),
            document_variable_name: "page_content".to_string(),
            initial_response_name: "existing_answer".to_string(),
            return_intermediate_steps: false,
        }
    }

    /// Set the initial prompt (for the first document)
    #[must_use]
    pub fn with_initial_prompt(mut self, prompt: PromptTemplate) -> Self {
        self.initial_prompt = prompt;
        self
    }

    /// Set the refine prompt (for subsequent documents)
    #[must_use]
    pub fn with_refine_prompt(mut self, prompt: PromptTemplate) -> Self {
        self.refine_prompt = prompt;
        self
    }

    /// Set the document prompt for formatting documents
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

    /// Set the initial response variable name (used in refine prompt)
    pub fn with_initial_response_name(mut self, name: impl Into<String>) -> Self {
        self.initial_response_name = name.into();
        self
    }

    /// Set whether to return intermediate refine steps
    #[must_use]
    pub fn with_return_intermediate_steps(mut self, return_steps: bool) -> Self {
        self.return_intermediate_steps = return_steps;
        self
    }

    /// Call the LLM with a prompt
    async fn call_llm(&self, prompt_text: String) -> Result<String> {
        match &self.llm {
            LLMType::Chat(chat_model) => {
                let messages = vec![HumanMessage::new(prompt_text).into()];
                let result = chat_model
                    .generate(&messages, None, None, None, None)
                    .await?;
                Ok(result
                    .generations
                    .first()
                    .ok_or_else(|| Error::other("LLM returned empty response in refine step"))?
                    .text())
            }
            LLMType::LLM(llm) => {
                let result = llm.generate(&[prompt_text], None, None).await?;
                Ok(result
                    .generations
                    .first()
                    .and_then(|g| g.first())
                    .ok_or_else(|| Error::other("LLM returned empty response in refine step"))?
                    .text
                    .clone())
            }
        }
    }

    /// Build prompt variables for a document
    fn build_doc_vars(
        &self,
        doc: &Document,
        kwargs: &HashMap<String, String>,
    ) -> Result<HashMap<String, String>> {
        let formatted_doc = format_document(doc, &self.document_prompt)?;

        let mut vars = kwargs.clone();
        vars.insert(self.document_variable_name.clone(), formatted_doc);

        // Add metadata
        for (key, value) in &doc.metadata {
            if let Some(s) = value.as_str() {
                vars.insert(key.clone(), s.to_string());
            }
        }

        Ok(vars)
    }

    /// Combine documents using iterative refinement
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
    /// "`intermediate_steps`" key with all refinement steps
    pub async fn combine_docs(
        &self,
        docs: &[Document],
        kwargs: Option<HashMap<String, String>>,
    ) -> Result<(String, HashMap<String, String>)> {
        if docs.is_empty() {
            return Err(Error::invalid_input(
                "RefineDocumentsChain requires at least one document",
            ));
        }

        let kwargs = kwargs.unwrap_or_default();
        let mut refine_steps = Vec::new();

        // Process first document with initial prompt
        let initial_vars = self.build_doc_vars(&docs[0], &kwargs)?;
        let initial_prompt_text = self.initial_prompt.format(&initial_vars)?;
        let mut current_answer = self.call_llm(initial_prompt_text).await?;
        refine_steps.push(current_answer.clone());

        // Process remaining documents with refine prompt
        for doc in &docs[1..] {
            let mut refine_vars = self.build_doc_vars(doc, &kwargs)?;
            refine_vars.insert(self.initial_response_name.clone(), current_answer.clone());

            let refine_prompt_text = self.refine_prompt.format(&refine_vars)?;
            current_answer = self.call_llm(refine_prompt_text).await?;
            refine_steps.push(current_answer.clone());
        }

        // Build extra return dict
        let mut extra_dict = HashMap::new();
        if self.return_intermediate_steps {
            let steps_json = serde_json::to_string(&refine_steps)
                .map_err(|e| Error::other(format!("Serialization error: {e}")))?;
            extra_dict.insert("intermediate_steps".to_string(), steps_json);
        }

        Ok((current_answer, extra_dict))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use dashflow::core::language_models::{ChatGeneration, ChatResult};
    use dashflow::core::language_models::{ToolChoice, ToolDefinition};
    use dashflow::core::messages::{AIMessage, BaseMessage};

    struct MockChatModel {
        // Maps input substring to response
        responses: HashMap<String, String>,
        default_response: String,
        call_count: std::sync::Arc<std::sync::Mutex<usize>>,
    }

    impl MockChatModel {
        fn new(default_response: impl Into<String>) -> Self {
            Self {
                responses: HashMap::new(),
                default_response: default_response.into(),
                call_count: std::sync::Arc::new(std::sync::Mutex::new(0)),
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
            *self.call_count.lock().unwrap() += 1;

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
    async fn test_refine_chain() {
        let llm = Arc::new(
            MockChatModel::new("Fallback")
                .with_response("First", "Initial answer from first doc")
                .with_response(
                    "Initial answer from first doc",
                    "Refined answer with second doc",
                ),
        );

        let chain = RefineDocumentsChain::new_chat(llm);

        let docs = vec![
            Document::new("First document"),
            Document::new("Second document"),
        ];

        let (output, _) = chain.combine_docs(&docs, None).await.unwrap();
        // Second response should be refined since first answer appears in refine prompt
        assert_eq!(output, "Refined answer with second doc");
    }

    #[tokio::test]
    async fn test_refine_single_document() {
        let llm = Arc::new(MockChatModel::new("Single doc answer"));

        let chain = RefineDocumentsChain::new_chat(llm);
        let docs = vec![Document::new("Only document")];

        let (output, _) = chain.combine_docs(&docs, None).await.unwrap();
        assert_eq!(output, "Single doc answer");
    }

    #[tokio::test]
    async fn test_refine_with_intermediate_steps() {
        let llm = Arc::new(
            MockChatModel::new("Fallback")
                .with_response("Doc1", "Step1")
                .with_response("Step1", "Step2")
                .with_response("Step2", "Step3"),
        );

        let chain = RefineDocumentsChain::new_chat(llm).with_return_intermediate_steps(true);

        let docs = vec![
            Document::new("Doc1"),
            Document::new("Doc2"),
            Document::new("Doc3"),
        ];

        let (output, extra) = chain.combine_docs(&docs, None).await.unwrap();

        assert_eq!(output, "Step3");
        assert!(extra.contains_key("intermediate_steps"));

        let steps: Vec<String> =
            serde_json::from_str(extra.get("intermediate_steps").unwrap()).unwrap();
        assert_eq!(steps.len(), 3);
        assert_eq!(steps[0], "Step1");
        assert_eq!(steps[1], "Step2");
        assert_eq!(steps[2], "Step3");
    }

    #[tokio::test]
    async fn test_refine_empty_docs() {
        let llm = Arc::new(MockChatModel::new("test"));
        let chain = RefineDocumentsChain::new_chat(llm);

        let result = chain.combine_docs(&[], None).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_refine_custom_prompts() {
        let llm = Arc::new(MockChatModel::new("Custom result"));

        let initial = PromptTemplate::from_template("First: {page_content}").unwrap();
        let refine =
            PromptTemplate::from_template("Existing: {existing_answer}\nNew: {page_content}")
                .unwrap();

        let chain = RefineDocumentsChain::new_chat(llm)
            .with_initial_prompt(initial)
            .with_refine_prompt(refine);

        let docs = vec![Document::new("Content")];

        let (output, _) = chain.combine_docs(&docs, None).await.unwrap();
        assert_eq!(output, "Custom result");
    }
}
