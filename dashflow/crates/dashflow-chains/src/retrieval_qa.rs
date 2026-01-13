//! Question-answering chains that combine document retrieval with LLMs.
//!
//! These chains enable question answering over a corpus of documents by:
//! 1. Retrieving relevant documents using a retriever
//! 2. Combining the documents with the question
//! 3. Passing to an LLM to generate an answer
//!
//! # Chain Types
//!
//! - [`RetrievalQA`]: Basic QA chain that retrieves documents and generates answers
//! - [`crate::conversational_retrieval::ConversationalRetrievalChain`]: QA chain that maintains conversation history
//!
//! # Example
//!
//! ```rust,ignore
//! use dashflow_chains::retrieval_qa::RetrievalQA;
//! use dashflow::core::retrievers::VectorStoreRetriever;
//! use dashflow::core::prompts::PromptTemplate;
//!
//! // Create a retriever from your vector store
//! let retriever = VectorStoreRetriever::from_vectorstore(vector_store);
//!
//! // Create QA chain
//! let chain = RetrievalQA::from_chain_type(
//!     llm,
//!     retriever,
//!     "stuff",  // chain type (stuff, map_reduce, or refine)
//! ).await?;
//!
//! // Ask questions
//! let answer = chain.run("What is DashFlow?").await?;
//! ```

use crate::combine_documents::{
    MapReduceDocumentsChain, RefineDocumentsChain, StuffDocumentsChain,
};
use dashflow::core::{
    documents::Document,
    error::{Error, Result},
    language_models::{ChatModel, LLM},
    prompts::PromptTemplate,
    retrievers::Retriever,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

/// Type of chain to use for combining documents.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ChainType {
    /// Stuff all documents into a single prompt.
    /// Works best for small numbers of documents.
    #[serde(rename = "stuff")]
    #[default]
    Stuff,

    /// Map over documents then reduce the results.
    /// Good for large numbers of documents.
    #[serde(rename = "map_reduce")]
    MapReduce,

    /// Iteratively refine the answer by processing documents sequentially.
    /// Good for building up a comprehensive answer.
    #[serde(rename = "refine")]
    Refine,
}

/// Default prompt template for question answering.
const DEFAULT_QA_PROMPT: &str = r"Use the following pieces of context to answer the question at the end. If you don't know the answer, just say that you don't know, don't try to make up an answer.

{context}

Question: {question}
Helpful Answer:";

/// Question-answering chain that combines document retrieval with an LLM.
///
/// This chain retrieves relevant documents using a retriever, then passes them
/// to an LLM along with the question to generate an answer.
///
/// # Type Parameters
///
/// - `M`: Language model type (must implement `LLM` or `ChatModel`)
/// - `R`: Retriever type (must implement `Retriever`)
///
/// # Example
///
/// ```rust,ignore
/// use dashflow_chains::retrieval_qa::RetrievalQA;
///
/// // Create from LLM and retriever
/// let chain = RetrievalQA::new(llm, retriever, ChainType::Stuff);
///
/// // Ask a question
/// let result = chain.run("What is Rust?").await?;
/// println!("Answer: {}", result);
///
/// // Get answer with source documents
/// let chain_with_sources = chain.with_return_source_documents(true);
/// let (answer, sources) = chain_with_sources.run_with_sources("What is Rust?").await?;
/// ```
pub struct RetrievalQA<M, R>
where
    M: Send + Sync,
    R: Retriever,
{
    /// Language model to use for generating answers
    model: Arc<M>,

    /// Retriever for finding relevant documents
    retriever: Arc<R>,

    /// Type of chain to use for combining documents
    chain_type: ChainType,

    /// Optional custom prompt template
    prompt: Option<PromptTemplate>,

    /// Whether to return source documents along with the answer
    return_source_documents: bool,

    /// Input key for the question (default: "query")
    input_key: String,

    /// Output key for the answer (default: "result")
    output_key: String,
}

impl<M, R> RetrievalQA<M, R>
where
    M: Send + Sync,
    R: Retriever,
{
    /// Create a new `RetrievalQA` chain.
    ///
    /// # Arguments
    ///
    /// * `model` - Language model to use for generating answers
    /// * `retriever` - Retriever for finding relevant documents
    /// * `chain_type` - Type of chain to use for combining documents
    pub fn new(model: M, retriever: R, chain_type: ChainType) -> Self {
        RetrievalQA {
            model: Arc::new(model),
            retriever: Arc::new(retriever),
            chain_type,
            prompt: None,
            return_source_documents: false,
            input_key: "query".to_string(),
            output_key: "result".to_string(),
        }
    }

    /// Set a custom prompt template.
    ///
    /// The prompt should include `{context}` and `{question}` variables.
    #[must_use]
    pub fn with_prompt(mut self, prompt: PromptTemplate) -> Self {
        self.prompt = Some(prompt);
        self
    }

    /// Set whether to return source documents with the answer.
    #[must_use]
    pub fn with_return_source_documents(mut self, return_sources: bool) -> Self {
        self.return_source_documents = return_sources;
        self
    }

    /// Set the input key for the question.
    pub fn with_input_key(mut self, key: impl Into<String>) -> Self {
        self.input_key = key.into();
        self
    }

    /// Set the output key for the answer.
    pub fn with_output_key(mut self, key: impl Into<String>) -> Self {
        self.output_key = key.into();
        self
    }

    /// Get the prompt template, using default if none was set.
    fn get_prompt(&self) -> PromptTemplate {
        self.prompt.clone().unwrap_or_else(|| {
            // SAFETY: M-347 - DEFAULT_QA_PROMPT is a compile-time constant template
            #[allow(clippy::expect_used)]
            PromptTemplate::from_template(DEFAULT_QA_PROMPT)
                .expect("DEFAULT_QA_PROMPT is a valid template")
        })
    }
}

/// Implementation for LLM-based `RetrievalQA` chains
impl<M, R> RetrievalQA<M, R>
where
    M: LLM + Send + Sync + 'static,
    R: Retriever + Send + Sync + 'static,
{
    /// Create a `RetrievalQA` chain from an LLM, retriever, and chain type.
    ///
    /// This is a convenience constructor that sets up the chain with default settings.
    pub fn from_chain_type(model: M, retriever: R, chain_type: &str) -> Result<Self> {
        let chain_type_enum = match chain_type {
            "stuff" => ChainType::Stuff,
            "map_reduce" => ChainType::MapReduce,
            "refine" => ChainType::Refine,
            _ => {
                return Err(Error::invalid_input(format!(
                    "Invalid chain type: {chain_type}. Must be 'stuff', 'map_reduce', or 'refine'"
                )))
            }
        };

        Ok(RetrievalQA::new(model, retriever, chain_type_enum))
    }

    /// Run the chain on a single question.
    ///
    /// Returns just the answer text.
    pub async fn run(&self, question: &str) -> Result<String> {
        let (answer, _) = self.run_with_sources(question).await?;
        Ok(answer)
    }

    /// Run the chain and return both answer and source documents.
    ///
    /// Returns a tuple of (answer, `source_documents`).
    pub async fn run_with_sources(&self, question: &str) -> Result<(String, Vec<Document>)> {
        // Retrieve relevant documents
        let docs = self
            .retriever
            ._get_relevant_documents(question, None)
            .await?;

        if docs.is_empty() {
            return Ok((
                "I don't have enough information to answer that question.".to_string(),
                vec![],
            ));
        }

        // Combine documents and generate answer based on chain type
        let (answer, _) = match self.chain_type {
            ChainType::Stuff => {
                let model_arc = Arc::clone(&self.model) as Arc<dyn LLM>;
                let chain = StuffDocumentsChain::new_llm(model_arc)
                    .with_prompt(self.get_prompt())
                    .with_document_variable_name("context");

                let mut inputs = HashMap::new();
                inputs.insert("question".to_string(), question.to_string());

                chain.combine_docs(&docs, Some(inputs)).await?
            }
            ChainType::MapReduce => {
                // For MapReduce, we need to set up a reduce chain with the QA prompt
                let model_arc = Arc::clone(&self.model) as Arc<dyn LLM>;
                let reduce_chain = StuffDocumentsChain::new_llm(Arc::clone(&model_arc))
                    .with_prompt(self.get_prompt())
                    .with_document_variable_name("context");

                let chain = MapReduceDocumentsChain::new_llm(model_arc)
                    .with_reduce_chain(reduce_chain)
                    .with_document_variable_name("context");

                let mut inputs = HashMap::new();
                inputs.insert("question".to_string(), question.to_string());

                chain.combine_docs(&docs, Some(inputs)).await?
            }
            ChainType::Refine => {
                let model_arc = Arc::clone(&self.model) as Arc<dyn LLM>;
                let chain = RefineDocumentsChain::new_llm(model_arc)
                    .with_refine_prompt(self.get_prompt())
                    .with_document_variable_name("context");

                let mut inputs = HashMap::new();
                inputs.insert("question".to_string(), question.to_string());

                chain.combine_docs(&docs, Some(inputs)).await?
            }
        };

        Ok((answer, docs))
    }

    /// Run the chain with detailed output including source documents.
    ///
    /// Returns a `HashMap` with the answer under the `output_key` and optionally
    /// the source documents under "`source_documents`".
    pub async fn call(&self, inputs: HashMap<String, String>) -> Result<HashMap<String, String>> {
        let question = inputs.get(&self.input_key).ok_or_else(|| {
            Error::invalid_input(format!("Missing input key: {}", self.input_key))
        })?;

        let (answer, sources) = self.run_with_sources(question).await?;

        let mut output = HashMap::new();
        output.insert(self.output_key.clone(), answer);

        if self.return_source_documents {
            let sources_json = serde_json::to_string(&sources)
                .map_err(|e| Error::Other(format!("Failed to serialize sources: {e}")))?;
            output.insert("source_documents".to_string(), sources_json);
        }

        Ok(output)
    }
}

/// Implementation for ChatModel-based `RetrievalQA` chains
impl<M, R> RetrievalQA<M, R>
where
    M: ChatModel + Send + Sync + 'static,
    R: Retriever + Send + Sync + 'static,
{
    /// Create a `RetrievalQA` chain from a `ChatModel`, retriever, and chain type.
    pub fn from_chat_model(model: M, retriever: R, chain_type: &str) -> Result<Self> {
        let chain_type_enum = match chain_type {
            "stuff" => ChainType::Stuff,
            "map_reduce" => ChainType::MapReduce,
            "refine" => ChainType::Refine,
            _ => {
                return Err(Error::invalid_input(format!(
                    "Invalid chain type: {chain_type}. Must be 'stuff', 'map_reduce', or 'refine'"
                )))
            }
        };

        Ok(RetrievalQA::new(model, retriever, chain_type_enum))
    }

    /// Run the chain on a single question (`ChatModel` version).
    pub async fn run_chat(&self, question: &str) -> Result<String> {
        let (answer, _) = self.run_chat_with_sources(question).await?;
        Ok(answer)
    }

    /// Run the chain and return both answer and source documents (`ChatModel` version).
    pub async fn run_chat_with_sources(&self, question: &str) -> Result<(String, Vec<Document>)> {
        // Retrieve relevant documents
        let docs = self
            .retriever
            ._get_relevant_documents(question, None)
            .await?;

        if docs.is_empty() {
            return Ok((
                "I don't have enough information to answer that question.".to_string(),
                vec![],
            ));
        }

        // Combine documents and generate answer based on chain type
        let (answer, _) = match self.chain_type {
            ChainType::Stuff => {
                let model_arc = Arc::clone(&self.model) as Arc<dyn ChatModel>;
                let chain = StuffDocumentsChain::new_chat(model_arc)
                    .with_prompt(self.get_prompt())
                    .with_document_variable_name("context");

                let mut inputs = HashMap::new();
                inputs.insert("question".to_string(), question.to_string());

                chain.combine_docs(&docs, Some(inputs)).await?
            }
            ChainType::MapReduce => {
                // For MapReduce, we need to set up a reduce chain with the QA prompt
                let model_arc = Arc::clone(&self.model) as Arc<dyn ChatModel>;
                let reduce_chain = StuffDocumentsChain::new_chat(Arc::clone(&model_arc))
                    .with_prompt(self.get_prompt())
                    .with_document_variable_name("context");

                let chain = MapReduceDocumentsChain::new_chat(model_arc)
                    .with_reduce_chain(reduce_chain)
                    .with_document_variable_name("context");

                let mut inputs = HashMap::new();
                inputs.insert("question".to_string(), question.to_string());

                chain.combine_docs(&docs, Some(inputs)).await?
            }
            ChainType::Refine => {
                let model_arc = Arc::clone(&self.model) as Arc<dyn ChatModel>;
                let chain = RefineDocumentsChain::new_chat(model_arc)
                    .with_refine_prompt(self.get_prompt())
                    .with_document_variable_name("context");

                let mut inputs = HashMap::new();
                inputs.insert("question".to_string(), question.to_string());

                chain.combine_docs(&docs, Some(inputs)).await?
            }
        };

        Ok((answer, docs))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use dashflow::core::{
        embeddings::Embeddings,
        error::Result,
        language_models::{Generation, LLMResult, LLM},
        retrievers::{SearchConfig, SearchType, VectorStoreRetriever},
        vector_stores::{InMemoryVectorStore, VectorStore},
    };
    use std::sync::Arc;

    // Mock LLM for testing
    struct MockLLM;

    #[async_trait]
    impl LLM for MockLLM {
        async fn _generate(
            &self,
            prompts: &[String],
            _stop: Option<&[String]>,
            _run_manager: Option<&dashflow::core::callbacks::CallbackManager>,
        ) -> Result<LLMResult> {
            let generations: Vec<Vec<Generation>> = prompts
                .iter()
                .map(|_| {
                    vec![Generation::new(
                        "This is a helpful answer based on the provided context.",
                    )]
                })
                .collect();

            Ok(LLMResult::with_prompts(generations))
        }

        fn llm_type(&self) -> &str {
            "mock"
        }
    }

    // Mock Embeddings for testing
    struct MockEmbeddings;

    #[async_trait]
    impl Embeddings for MockEmbeddings {
        async fn _embed_documents(&self, texts: &[String]) -> Result<Vec<Vec<f32>>> {
            Ok(texts
                .iter()
                .enumerate()
                .map(|(i, _)| vec![i as f32, 0.5, 0.1])
                .collect())
        }

        async fn _embed_query(&self, text: &str) -> Result<Vec<f32>> {
            Ok(vec![text.len() as f32, 0.5, 0.1])
        }
    }

    async fn create_test_retriever() -> VectorStoreRetriever<InMemoryVectorStore> {
        let embeddings = Arc::new(MockEmbeddings);
        let mut store = InMemoryVectorStore::new(embeddings);

        // Add test documents
        let texts = vec![
            "Rust is a systems programming language.",
            "DashFlow is a framework for building LLM applications.",
            "Vector stores are used for semantic search.",
        ];
        store.add_texts(&texts, None, None).await.unwrap();

        VectorStoreRetriever::new(
            store,
            SearchType::Similarity,
            SearchConfig::default().with_k(2),
        )
    }

    #[tokio::test]
    async fn test_retrieval_qa_basic() {
        let llm = MockLLM;
        let retriever = create_test_retriever().await;

        let chain = RetrievalQA::new(llm, retriever, ChainType::Stuff);

        let answer = chain.run("What is Rust?").await.unwrap();
        assert!(!answer.is_empty());
        assert!(answer.contains("helpful answer"));
    }

    #[tokio::test]
    async fn test_retrieval_qa_with_sources() {
        let llm = MockLLM;
        let retriever = create_test_retriever().await;

        let chain = RetrievalQA::new(llm, retriever, ChainType::Stuff);

        let (answer, sources) = chain.run_with_sources("What is DashFlow?").await.unwrap();

        assert!(!answer.is_empty());
        assert!(!sources.is_empty());
        assert!(sources.len() <= 2); // We set k=2 in retriever
    }

    #[tokio::test]
    async fn test_retrieval_qa_from_chain_type() {
        let llm = MockLLM;
        let retriever = create_test_retriever().await;

        let chain = RetrievalQA::from_chain_type(llm, retriever, "stuff").unwrap();

        let answer = chain.run("Test question").await.unwrap();
        assert!(!answer.is_empty());
    }

    #[tokio::test]
    async fn test_retrieval_qa_invalid_chain_type() {
        let llm = MockLLM;
        let retriever = create_test_retriever().await;

        let result = RetrievalQA::from_chain_type(llm, retriever, "invalid");
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_retrieval_qa_with_custom_prompt() {
        let llm = MockLLM;
        let retriever = create_test_retriever().await;

        let custom_prompt = PromptTemplate::from_template(
            "Context: {context}\n\nQuestion: {question}\n\nAnswer briefly:",
        )
        .unwrap();

        let chain = RetrievalQA::new(llm, retriever, ChainType::Stuff).with_prompt(custom_prompt);

        let answer = chain.run("What is Rust?").await.unwrap();
        assert!(!answer.is_empty());
    }

    #[tokio::test]
    async fn test_retrieval_qa_with_return_sources() {
        let llm = MockLLM;
        let retriever = create_test_retriever().await;

        let chain =
            RetrievalQA::new(llm, retriever, ChainType::Stuff).with_return_source_documents(true);

        let mut inputs = HashMap::new();
        inputs.insert("query".to_string(), "What is a vector store?".to_string());

        let output = chain.call(inputs).await.unwrap();

        assert!(output.contains_key("result"));
        assert!(output.contains_key("source_documents"));
    }

    #[tokio::test]
    async fn test_retrieval_qa_map_reduce() {
        let llm = MockLLM;
        let retriever = create_test_retriever().await;

        let chain = RetrievalQA::new(llm, retriever, ChainType::MapReduce);

        let answer = chain.run("What technologies are mentioned?").await.unwrap();
        assert!(!answer.is_empty());
    }

    #[tokio::test]
    async fn test_retrieval_qa_refine() {
        let llm = MockLLM;
        let retriever = create_test_retriever().await;

        let chain = RetrievalQA::new(llm, retriever, ChainType::Refine);

        let answer = chain.run("Explain the technologies").await.unwrap();
        assert!(!answer.is_empty());
    }
}
