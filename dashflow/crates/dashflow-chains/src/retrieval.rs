//! Retrieval chain helpers for creating common retrieval patterns
//!
//! This module provides helper functions for creating retrieval chains that follow
//! common patterns in `DashFlow` applications, particularly for RAG (Retrieval-Augmented Generation).

use dashflow::core::{
    documents::Document,
    error::{Error, Result},
    language_models::ChatModel,
    messages::BaseMessage,
    prompts::ChatPromptTemplate,
    retrievers::Retriever,
};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;

/// A retriever that reformulates queries based on conversation history.
///
/// This struct wraps a retriever and LLM to handle conversational context.
/// When chat history is present, it uses the LLM to reformulate the query
/// before retrieving documents.
pub struct HistoryAwareRetriever<M, R>
where
    M: ChatModel,
    R: Retriever,
{
    llm: Arc<M>,
    retriever: Arc<R>,
    prompt: ChatPromptTemplate,
}

impl<M, R> HistoryAwareRetriever<M, R>
where
    M: ChatModel,
    R: Retriever,
{
    /// Create a new history-aware retriever
    pub fn new(llm: M, retriever: R, prompt: ChatPromptTemplate) -> Result<Self> {
        use dashflow::core::prompts::BasePromptTemplate;

        // Validate that "input" is in the prompt variables
        if !prompt.input_variables().contains(&"input".to_string()) {
            return Err(Error::InvalidInput(format!(
                "Expected `input` to be a prompt variable, but got {:?}",
                prompt.input_variables()
            )));
        }

        Ok(Self {
            llm: Arc::new(llm),
            retriever: Arc::new(retriever),
            prompt,
        })
    }

    /// Retrieve documents, optionally reformulating the query based on chat history
    pub async fn _get_relevant_documents(
        &self,
        input: &str,
        chat_history: Option<&[BaseMessage]>,
    ) -> Result<Vec<Document>> {
        let query = if let Some(history) = chat_history {
            if history.is_empty() {
                // No history, use input directly
                input.to_string()
            } else {
                // Reformulate query using LLM
                let mut values = HashMap::new();
                values.insert("input".to_string(), input.to_string());

                // Convert chat history to a string representation for the prompt
                let history_str = history
                    .iter()
                    .map(|m| {
                        let role = m.message_type();
                        let content = m.content().as_text();
                        format!("{role}: {content}")
                    })
                    .collect::<Vec<_>>()
                    .join("\n");

                values.insert("chat_history".to_string(), history_str);

                let messages = self.prompt.format_messages(&values)?;
                let result = self.llm.generate(&messages, None, None, None, None).await?;

                result
                    .generations
                    .first()
                    .map_or_else(|| input.to_string(), |gen| gen.message.content().as_text())
            }
        } else {
            // No history provided, use input directly
            input.to_string()
        };

        self.retriever._get_relevant_documents(&query, None).await
    }
}

/// Creates a history-aware retriever that reformulates queries based on conversation history.
///
/// If there is no `chat_history`, the `input` is passed directly to the retriever.
/// If there is `chat_history`, the prompt and LLM generate a search query based on
/// the conversation context, which is then passed to the retriever.
///
/// # Arguments
///
/// * `llm` - Chat model to use for reformulating queries
/// * `retriever` - Retriever that returns documents
/// * `prompt` - Prompt template for reformulation (must include "input" variable)
///
/// # Returns
///
/// A `HistoryAwareRetriever` that can retrieve documents with conversation context.
///
/// # Example
///
/// ```rust,ignore
/// use dashflow_chains::retrieval::create_history_aware_retriever;
/// use dashflow_openai::ChatOpenAI;
/// use dashflow::core::prompts::ChatPromptTemplate;
///
/// let llm = ChatOpenAI::default();
/// let retriever = my_vector_store.as_retriever();
/// let prompt = ChatPromptTemplate::from_messages(vec![
///     ("system", "Given the chat history and latest question, reformulate as standalone."),
///     ("placeholder", "{chat_history}"),
///     ("human", "{input}"),
/// ])?;
///
/// let retriever = create_history_aware_retriever(llm, retriever, prompt)?;
/// let docs = retriever._get_relevant_documents("What did we discuss?", Some(&history)).await?;
/// ```
pub fn create_history_aware_retriever<M, R>(
    llm: M,
    retriever: R,
    prompt: ChatPromptTemplate,
) -> Result<HistoryAwareRetriever<M, R>>
where
    M: ChatModel,
    R: Retriever,
{
    HistoryAwareRetriever::new(llm, retriever, prompt)
}

/// A retrieval chain that combines document retrieval with a processing chain.
///
/// This is the standard pattern for RAG applications. It retrieves documents
/// and passes them along with the input to a combining function.
pub struct RetrievalChain<R, F, Fut>
where
    R: Retriever,
    F: Fn(HashMap<String, Value>) -> Fut + Send + Sync,
    Fut: std::future::Future<Output = Result<String>> + Send,
{
    retriever: Arc<R>,
    combine_fn: Arc<F>,
}

impl<R, F, Fut> RetrievalChain<R, F, Fut>
where
    R: Retriever,
    F: Fn(HashMap<String, Value>) -> Fut + Send + Sync,
    Fut: std::future::Future<Output = Result<String>> + Send,
{
    /// Create a new retrieval chain
    pub fn new(retriever: R, combine_fn: F) -> Self {
        Self {
            retriever: Arc::new(retriever),
            combine_fn: Arc::new(combine_fn),
        }
    }

    /// Run the retrieval chain
    ///
    /// Returns a `HashMap` containing:
    /// - "context": The retrieved documents (as JSON array)
    /// - "answer": The output from the combine function
    /// - Original input keys are also preserved
    pub async fn invoke(
        &self,
        mut input: HashMap<String, Value>,
    ) -> Result<HashMap<String, Value>> {
        // Extract the "input" key for retrieval
        let query = input
            .get("input")
            .and_then(|v| v.as_str())
            .ok_or_else(|| Error::InvalidInput("Missing 'input' key in input".to_string()))?
            .to_string();

        // Retrieve documents
        let documents = self.retriever._get_relevant_documents(&query, None).await?;

        // Add documents to context
        let context_value = serde_json::to_value(&documents)?;
        input.insert("context".to_string(), context_value);

        // Ensure chat_history exists (empty array if not present)
        if !input.contains_key("chat_history") {
            input.insert("chat_history".to_string(), Value::Array(vec![]));
        }

        // Call the combine function
        let answer = (self.combine_fn)(input.clone()).await?;
        input.insert("answer".to_string(), Value::String(answer));

        Ok(input)
    }
}

/// Creates a retrieval chain that retrieves documents and processes them.
///
/// This is the standard pattern for RAG (Retrieval-Augmented Generation) applications.
/// It retrieves relevant documents based on the input query, adds them to the context,
/// and passes everything to a combining function (typically for generating an answer).
///
/// # Arguments
///
/// * `retriever` - Retriever that returns documents (uses "input" key from the input dict)
/// * `combine_fn` - Async function that takes the input + context and produces an answer string
///
/// # Returns
///
/// A `RetrievalChain` that returns a `HashMap` containing "context" (retrieved docs),
/// "answer" (output from `combine_fn`), and the original input keys.
///
/// # Example
///
/// ```rust,ignore
/// use dashflow_chains::retrieval::create_retrieval_chain;
/// use dashflow_openai::ChatOpenAI;
///
/// let llm = Arc::new(ChatOpenAI::default());
/// let retriever = my_vector_store.as_retriever();
///
/// // Create a simple combine function
/// let chain = create_retrieval_chain(retriever, move |input: HashMap<String, Value>| {
///     let llm = llm.clone();
///     async move {
///         let context = input.get("context").unwrap();
///         let query = input.get("input").unwrap().as_str().unwrap();
///
///         let prompt = format!("Answer based on context:\n{}\n\nQuestion: {}", context, query);
///         let response = llm.generate(&[Message::human(prompt)]).await?;
///         Ok(response.content().as_text().unwrap_or("").to_string())
///     }
/// });
///
/// let result = chain.invoke(hashmap!{
///     "input" => json!("What is DashFlow?")
/// }).await?;
/// // result contains {"context": [...docs...], "answer": "..."}
/// ```
pub fn create_retrieval_chain<R, F, Fut>(retriever: R, combine_fn: F) -> RetrievalChain<R, F, Fut>
where
    R: Retriever,
    F: Fn(HashMap<String, Value>) -> Fut + Send + Sync,
    Fut: std::future::Future<Output = Result<String>> + Send,
{
    RetrievalChain::new(retriever, combine_fn)
}

#[cfg(test)]
mod tests {
    use super::*;
    use dashflow::core::documents::Document;
    use dashflow::core::error::Result;
    use dashflow::core::language_models::{ToolChoice, ToolDefinition};

    // Mock retriever for testing
    struct MockRetriever;

    #[async_trait::async_trait]
    impl Retriever for MockRetriever {
        async fn _get_relevant_documents(
            &self,
            query: &str,
            _config: Option<&dashflow::core::config::RunnableConfig>,
        ) -> Result<Vec<Document>> {
            Ok(vec![Document::new(format!("Doc about: {}", query))])
        }
    }

    // Mock chat model for testing
    struct MockChatModel;

    #[async_trait::async_trait]
    impl ChatModel for MockChatModel {
        async fn _generate(
            &self,
            messages: &[dashflow::core::messages::BaseMessage],
            _stop: Option<&[String]>,
            _tools: Option<&[ToolDefinition]>,
            _tool_choice: Option<&ToolChoice>,
            _run_manager: Option<&dashflow::core::callbacks::CallbackManager>,
        ) -> Result<dashflow::core::language_models::ChatResult> {
            // Return a reformulated query
            let last_msg = messages
                .last()
                .map(|m| m.content().as_text())
                .unwrap_or_else(|| "default query".to_string());

            let generation = dashflow::core::language_models::ChatGeneration::new(
                dashflow::core::messages::BaseMessage::ai(format!("reformulated: {}", last_msg)),
            );
            Ok(dashflow::core::language_models::ChatResult::new(generation))
        }

        fn llm_type(&self) -> &str {
            "mock"
        }
        fn as_any(&self) -> &dyn std::any::Any {
            self
        }
    }

    #[tokio::test]
    async fn test_history_aware_retriever_no_history() {
        let llm = MockChatModel;
        let retriever = MockRetriever;
        let prompt = ChatPromptTemplate::from_messages(vec![
            ("system", "Reformulate the question"),
            ("human", "{input}"),
        ])
        .unwrap();

        let chain = create_history_aware_retriever(llm, retriever, prompt).unwrap();
        let docs = chain
            ._get_relevant_documents("What is Rust?", None)
            .await
            .unwrap();

        assert_eq!(docs.len(), 1);
        assert!(docs[0].page_content.contains("What is Rust?"));
    }

    #[tokio::test]
    async fn test_history_aware_retriever_with_history() {
        let llm = MockChatModel;
        let retriever = MockRetriever;
        let prompt = ChatPromptTemplate::from_messages(vec![
            ("system", "Reformulate based on history"),
            ("placeholder", "{chat_history}"),
            ("human", "{input}"),
        ])
        .unwrap();

        let chain = create_history_aware_retriever(llm, retriever, prompt).unwrap();
        let history = vec![
            BaseMessage::human("What is Python?"),
            BaseMessage::ai("Python is a programming language"),
        ];

        let docs = chain
            ._get_relevant_documents("What about Rust?", Some(&history))
            .await
            .unwrap();

        assert_eq!(docs.len(), 1);
        // The query should have been reformulated
        assert!(docs[0].page_content.contains("reformulated"));
    }

    #[tokio::test]
    async fn test_retrieval_chain() {
        let retriever = MockRetriever;

        // Simple combine function that just returns a fixed answer
        let chain = create_retrieval_chain(retriever, |input: HashMap<String, Value>| async move {
            let context = input.get("context").unwrap();
            Ok(format!("Answer based on: {}", context))
        });

        let mut input = HashMap::new();
        input.insert(
            "input".to_string(),
            Value::String("What is DashFlow?".to_string()),
        );

        let result = chain.invoke(input).await.unwrap();

        assert!(result.contains_key("context"));
        assert!(result.contains_key("answer"));
        assert!(result
            .get("answer")
            .unwrap()
            .as_str()
            .unwrap()
            .contains("Answer based on"));
    }
}
