//! Conversational question-answering chains with chat history.
//!
//! These chains extend standard retrieval QA by maintaining conversation history,
//! allowing for follow-up questions and context-aware retrieval.
//!
//! # How It Works
//!
//! 1. Takes the user's question and chat history
//! 2. Uses an LLM to condense the chat history + new question into a standalone question
//! 3. Retrieves relevant documents using the standalone question
//! 4. Generates an answer using the documents and original question
//!
//! # Example
//!
//! ```rust,ignore
//! use dashflow_chains::conversational_retrieval::ConversationalRetrievalChain;
//! use dashflow::core::messages::{HumanMessage, AIMessage};
//!
//! let chain = ConversationalRetrievalChain::new(llm, retriever, ChainType::Stuff);
//!
//! // First question
//! let answer1 = chain.run("What is Rust?", vec![]).await?;
//!
//! // Follow-up question with history
//! let history = vec![
//!     ("What is Rust?", answer1.as_str()),
//! ];
//! let answer2 = chain.run("What are its main features?", history).await?;
//! ```

use crate::{
    combine_documents::{MapReduceDocumentsChain, RefineDocumentsChain, StuffDocumentsChain},
    llm::{ChatLLMChain, LLMChain},
    retrieval_qa::ChainType,
};
use dashflow::core::{
    documents::Document,
    error::Result,
    language_models::{ChatModel, LLM},
    messages::BaseMessage,
    prompts::PromptTemplate,
    retrievers::Retriever,
};
use std::collections::HashMap;
use std::sync::Arc;

/// Default prompt for condensing chat history into a standalone question.
const DEFAULT_CONDENSE_PROMPT: &str = r"Given the following conversation and a follow up question, rephrase the follow up question to be a standalone question, in its original language.

Chat History:
{chat_history}
Follow Up Input: {question}
Standalone question:";

/// Default prompt for answering questions.
const DEFAULT_QA_PROMPT: &str = r"Use the following pieces of context to answer the question at the end. If you don't know the answer, just say that you don't know, don't try to make up an answer.

{context}

Question: {question}
Helpful Answer:";

/// Chat turn type - either a tuple of (human, ai) strings or Message objects.
#[derive(Debug, Clone)]
pub enum ChatTurn {
    /// Simple string tuple: (human question, ai answer)
    Tuple(String, String),
    /// Message objects
    Messages(Vec<BaseMessage>),
}

impl ChatTurn {
    /// Convert chat turn to formatted string for the prompt.
    #[must_use]
    pub fn format(&self) -> String {
        match self {
            ChatTurn::Tuple(human, ai) => {
                format!("Human: {human}\nAssistant: {ai}")
            }
            ChatTurn::Messages(messages) => messages
                .iter()
                .map(|msg| {
                    let role = match msg {
                        BaseMessage::Human { .. } => "Human",
                        BaseMessage::AI { .. } => "Assistant",
                        BaseMessage::System { .. } => "System",
                        BaseMessage::Function { .. } => "Function",
                        BaseMessage::Tool { .. } => "Tool",
                    };
                    format!("{}: {}", role, msg.content().as_text())
                })
                .collect::<Vec<_>>()
                .join("\n"),
        }
    }
}

/// Conversational question-answering chain with chat history.
///
/// This chain maintains conversation context by:
/// 1. Condensing chat history + new question into a standalone question
/// 2. Retrieving documents using the standalone question
/// 3. Generating an answer using the documents and original question
///
/// # Type Parameters
///
/// - `M`: Language model type (must implement `LLM` or `ChatModel`)
/// - `R`: Retriever type (must implement `Retriever`)
///
/// # Example
///
/// ```rust,ignore
/// use dashflow_chains::conversational_retrieval::ConversationalRetrievalChain;
///
/// let chain = ConversationalRetrievalChain::new(llm, retriever, ChainType::Stuff);
///
/// // Multi-turn conversation
/// let mut history = vec![];
///
/// let q1 = "What is Rust?";
/// let a1 = chain.run(q1, history.clone()).await?;
/// history.push((q1.to_string(), a1.clone()));
///
/// let q2 = "What are its safety features?";
/// let a2 = chain.run(q2, history.clone()).await?;
/// history.push((q2.to_string(), a2.clone()));
/// ```
pub struct ConversationalRetrievalChain<M, R>
where
    M: Send + Sync,
    R: Retriever,
{
    /// Language model for question generation and answering
    model: Arc<M>,

    /// Retriever for finding relevant documents
    retriever: Arc<R>,

    /// Type of chain to use for combining documents
    chain_type: ChainType,

    /// Prompt for condensing history into standalone question
    condense_question_prompt: PromptTemplate,

    /// Prompt for generating answers
    qa_prompt: Option<PromptTemplate>,

    /// Whether to rephrase the question for the QA prompt
    /// If true, uses the condensed question. If false, uses the original question.
    rephrase_question: bool,

    /// Whether to return source documents with the answer
    return_source_documents: bool,

    /// Whether to return the generated standalone question
    return_generated_question: bool,

    /// Response to return if no documents are found
    response_if_no_docs_found: Option<String>,
}

impl<M, R> ConversationalRetrievalChain<M, R>
where
    M: Send + Sync,
    R: Retriever,
{
    /// Create a new `ConversationalRetrievalChain`.
    ///
    /// # Arguments
    ///
    /// * `model` - Language model to use
    /// * `retriever` - Retriever for finding documents
    /// * `chain_type` - Type of chain for combining documents
    pub fn new(model: M, retriever: R, chain_type: ChainType) -> Self {
        #[allow(clippy::expect_used)]
        let condense_question_prompt = PromptTemplate::from_template(DEFAULT_CONDENSE_PROMPT)
            .expect("DEFAULT_CONDENSE_PROMPT is a valid template");

        ConversationalRetrievalChain {
            model: Arc::new(model),
            retriever: Arc::new(retriever),
            chain_type,
            // SAFETY: M-347 - DEFAULT_CONDENSE_PROMPT is a compile-time constant template
            condense_question_prompt,
            qa_prompt: None,
            rephrase_question: true,
            return_source_documents: false,
            return_generated_question: false,
            response_if_no_docs_found: None,
        }
    }

    /// Set a custom prompt for condensing questions.
    #[must_use]
    pub fn with_condense_prompt(mut self, prompt: PromptTemplate) -> Self {
        self.condense_question_prompt = prompt;
        self
    }

    /// Set a custom prompt for generating answers.
    #[must_use]
    pub fn with_qa_prompt(mut self, prompt: PromptTemplate) -> Self {
        self.qa_prompt = Some(prompt);
        self
    }

    /// Set whether to rephrase the question for the QA step.
    #[must_use]
    pub fn with_rephrase_question(mut self, rephrase: bool) -> Self {
        self.rephrase_question = rephrase;
        self
    }

    /// Set whether to return source documents.
    #[must_use]
    pub fn with_return_source_documents(mut self, return_sources: bool) -> Self {
        self.return_source_documents = return_sources;
        self
    }

    /// Set whether to return the generated standalone question.
    #[must_use]
    pub fn with_return_generated_question(mut self, return_question: bool) -> Self {
        self.return_generated_question = return_question;
        self
    }

    /// Set the response to return if no documents are found.
    pub fn with_response_if_no_docs(mut self, response: impl Into<String>) -> Self {
        self.response_if_no_docs_found = Some(response.into());
        self
    }

    /// Get the QA prompt, using default if none was set.
    fn get_qa_prompt(&self) -> PromptTemplate {
        self.qa_prompt.clone().unwrap_or_else(|| {
            // SAFETY: M-347 - DEFAULT_QA_PROMPT is a compile-time constant template
            #[allow(clippy::expect_used)]
            PromptTemplate::from_template(DEFAULT_QA_PROMPT)
                .expect("DEFAULT_QA_PROMPT is a valid template")
        })
    }

    /// Format chat history into a string.
    fn format_chat_history(&self, history: &[(String, String)]) -> String {
        if history.is_empty() {
            return String::new();
        }

        history
            .iter()
            .map(|(human, ai)| format!("Human: {human}\nAssistant: {ai}"))
            .collect::<Vec<_>>()
            .join("\n")
    }
}

/// Implementation for LLM-based conversational retrieval chains
impl<M, R> ConversationalRetrievalChain<M, R>
where
    M: LLM + Send + Sync + 'static,
    R: Retriever + Send + Sync + 'static,
{
    /// Run the chain with a question and chat history.
    ///
    /// # Arguments
    ///
    /// * `question` - The new question to answer
    /// * `chat_history` - List of (human, ai) conversation turns
    ///
    /// # Returns
    ///
    /// The generated answer
    pub async fn run(&self, question: &str, chat_history: Vec<(String, String)>) -> Result<String> {
        let (answer, _, _) = self.run_full(question, chat_history).await?;
        Ok(answer)
    }

    /// Run the chain and return answer, sources, and generated question.
    pub async fn run_full(
        &self,
        question: &str,
        chat_history: Vec<(String, String)>,
    ) -> Result<(String, Vec<Document>, Option<String>)> {
        // Step 1: Condense chat history + question into standalone question
        let standalone_question = if chat_history.is_empty() {
            question.to_string()
        } else {
            let history_str = self.format_chat_history(&chat_history);
            let mut inputs = HashMap::new();
            inputs.insert("chat_history".to_string(), history_str);
            inputs.insert("question".to_string(), question.to_string());

            let question_chain = LLMChain::new(
                Arc::clone(&self.model),
                self.condense_question_prompt.clone(),
            );

            question_chain.run(&inputs).await?.trim().to_string()
        };

        // Step 2: Retrieve documents using standalone question
        let docs = self
            .retriever
            ._get_relevant_documents(&standalone_question, None)
            .await?;

        // Step 3: Handle no documents case
        if docs.is_empty() {
            if let Some(ref response) = self.response_if_no_docs_found {
                return Ok((
                    response.clone(),
                    vec![],
                    if self.return_generated_question {
                        Some(standalone_question)
                    } else {
                        None
                    },
                ));
            }
            return Ok((
                "I don't have enough information to answer that question.".to_string(),
                vec![],
                if self.return_generated_question {
                    Some(standalone_question)
                } else {
                    None
                },
            ));
        }

        // Step 4: Generate answer using documents
        // Use standalone question if rephrase_question is true, otherwise use original
        let question_for_qa = if self.rephrase_question {
            &standalone_question
        } else {
            question
        };

        let (answer, _) = match self.chain_type {
            ChainType::Stuff => {
                let model_arc = Arc::clone(&self.model) as Arc<dyn LLM>;
                let chain = StuffDocumentsChain::new_llm(model_arc)
                    .with_prompt(self.get_qa_prompt())
                    .with_document_variable_name("context");

                let mut inputs = HashMap::new();
                inputs.insert("question".to_string(), question_for_qa.to_string());

                chain.combine_docs(&docs, Some(inputs)).await?
            }
            ChainType::MapReduce => {
                // For MapReduce, we need to set up a reduce chain with the QA prompt
                let model_arc = Arc::clone(&self.model) as Arc<dyn LLM>;
                let reduce_chain = StuffDocumentsChain::new_llm(Arc::clone(&model_arc))
                    .with_prompt(self.get_qa_prompt())
                    .with_document_variable_name("context");

                let chain = MapReduceDocumentsChain::new_llm(model_arc)
                    .with_reduce_chain(reduce_chain)
                    .with_document_variable_name("context");

                let mut inputs = HashMap::new();
                inputs.insert("question".to_string(), question_for_qa.to_string());

                chain.combine_docs(&docs, Some(inputs)).await?
            }
            ChainType::Refine => {
                let model_arc = Arc::clone(&self.model) as Arc<dyn LLM>;
                let chain = RefineDocumentsChain::new_llm(model_arc)
                    .with_refine_prompt(self.get_qa_prompt())
                    .with_document_variable_name("context");

                let mut inputs = HashMap::new();
                inputs.insert("question".to_string(), question_for_qa.to_string());

                chain.combine_docs(&docs, Some(inputs)).await?
            }
        };

        Ok((
            answer,
            if self.return_source_documents {
                docs
            } else {
                vec![]
            },
            if self.return_generated_question {
                Some(standalone_question)
            } else {
                None
            },
        ))
    }
}

/// Implementation for ChatModel-based conversational retrieval chains
impl<M, R> ConversationalRetrievalChain<M, R>
where
    M: ChatModel + Send + Sync + 'static,
    R: Retriever + Send + Sync + 'static,
{
    /// Run the chain with a question and chat history (`ChatModel` version).
    pub async fn run_chat(
        &self,
        question: &str,
        chat_history: Vec<(String, String)>,
    ) -> Result<String> {
        let (answer, _, _) = self.run_chat_full(question, chat_history).await?;
        Ok(answer)
    }

    /// Run the chain and return answer, sources, and generated question (`ChatModel` version).
    pub async fn run_chat_full(
        &self,
        question: &str,
        chat_history: Vec<(String, String)>,
    ) -> Result<(String, Vec<Document>, Option<String>)> {
        // Step 1: Condense chat history + question into standalone question
        let standalone_question = if chat_history.is_empty() {
            question.to_string()
        } else {
            let history_str = self.format_chat_history(&chat_history);
            let mut inputs = HashMap::new();
            inputs.insert("chat_history".to_string(), history_str);
            inputs.insert("question".to_string(), question.to_string());

            let question_chain = ChatLLMChain::from_template(
                Arc::clone(&self.model),
                &self.condense_question_prompt.template,
            )?;

            question_chain.run(&inputs).await?.trim().to_string()
        };

        // Step 2: Retrieve documents using standalone question
        let docs = self
            .retriever
            ._get_relevant_documents(&standalone_question, None)
            .await?;

        // Step 3: Handle no documents case
        if docs.is_empty() {
            if let Some(ref response) = self.response_if_no_docs_found {
                return Ok((
                    response.clone(),
                    vec![],
                    if self.return_generated_question {
                        Some(standalone_question)
                    } else {
                        None
                    },
                ));
            }
            return Ok((
                "I don't have enough information to answer that question.".to_string(),
                vec![],
                if self.return_generated_question {
                    Some(standalone_question)
                } else {
                    None
                },
            ));
        }

        // Step 4: Generate answer using documents
        let question_for_qa = if self.rephrase_question {
            &standalone_question
        } else {
            question
        };

        let (answer, _) = match self.chain_type {
            ChainType::Stuff => {
                let model_arc = Arc::clone(&self.model) as Arc<dyn ChatModel>;
                let chain = StuffDocumentsChain::new_chat(model_arc)
                    .with_prompt(self.get_qa_prompt())
                    .with_document_variable_name("context");

                let mut inputs = HashMap::new();
                inputs.insert("question".to_string(), question_for_qa.to_string());

                chain.combine_docs(&docs, Some(inputs)).await?
            }
            ChainType::MapReduce => {
                // For MapReduce, we need to set up a reduce chain with the QA prompt
                let model_arc = Arc::clone(&self.model) as Arc<dyn ChatModel>;
                let reduce_chain = StuffDocumentsChain::new_chat(Arc::clone(&model_arc))
                    .with_prompt(self.get_qa_prompt())
                    .with_document_variable_name("context");

                let chain = MapReduceDocumentsChain::new_chat(model_arc)
                    .with_reduce_chain(reduce_chain)
                    .with_document_variable_name("context");

                let mut inputs = HashMap::new();
                inputs.insert("question".to_string(), question_for_qa.to_string());

                chain.combine_docs(&docs, Some(inputs)).await?
            }
            ChainType::Refine => {
                let model_arc = Arc::clone(&self.model) as Arc<dyn ChatModel>;
                let chain = RefineDocumentsChain::new_chat(model_arc)
                    .with_refine_prompt(self.get_qa_prompt())
                    .with_document_variable_name("context");

                let mut inputs = HashMap::new();
                inputs.insert("question".to_string(), question_for_qa.to_string());

                chain.combine_docs(&docs, Some(inputs)).await?
            }
        };

        Ok((
            answer,
            if self.return_source_documents {
                docs
            } else {
                vec![]
            },
            if self.return_generated_question {
                Some(standalone_question)
            } else {
                None
            },
        ))
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
    struct MockConversationalLLM {
        condensed_question: String,
        answer: String,
    }

    impl MockConversationalLLM {
        fn new() -> Self {
            MockConversationalLLM {
                condensed_question: "What is Rust and what are its features?".to_string(),
                answer: "Based on the context, Rust is a systems programming language with safety features.".to_string(),
            }
        }
    }

    #[async_trait]
    impl LLM for MockConversationalLLM {
        async fn _generate(
            &self,
            prompts: &[String],
            _stop: Option<&[String]>,
            _run_manager: Option<&dashflow::core::callbacks::CallbackManager>,
        ) -> Result<LLMResult> {
            let generations: Vec<Vec<Generation>> = prompts
                .iter()
                .map(|prompt| {
                    // If prompt contains "Standalone question", it's the condense step
                    let text = if prompt.contains("Standalone question") {
                        self.condensed_question.clone()
                    } else {
                        self.answer.clone()
                    };

                    vec![Generation::new(text)]
                })
                .collect();

            Ok(LLMResult::with_prompts(generations))
        }

        fn llm_type(&self) -> &str {
            "mock"
        }
    }

    // Mock Embeddings
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

        let texts = vec![
            "Rust is a systems programming language.",
            "Rust provides memory safety without garbage collection.",
            "Rust has a strong type system and ownership model.",
        ];
        store.add_texts(&texts, None, None).await.unwrap();

        VectorStoreRetriever::new(
            store,
            SearchType::Similarity,
            SearchConfig::default().with_k(2),
        )
    }

    #[tokio::test]
    async fn test_conversational_retrieval_no_history() {
        let llm = MockConversationalLLM::new();
        let retriever = create_test_retriever().await;

        let chain = ConversationalRetrievalChain::new(llm, retriever, ChainType::Stuff);

        let answer = chain.run("What is Rust?", vec![]).await.unwrap();
        assert!(!answer.is_empty());
        assert!(answer.contains("Rust"));
    }

    #[tokio::test]
    async fn test_conversational_retrieval_with_history() {
        let llm = MockConversationalLLM::new();
        let retriever = create_test_retriever().await;

        let chain = ConversationalRetrievalChain::new(llm, retriever, ChainType::Stuff);

        let history = vec![(
            "What is Rust?".to_string(),
            "Rust is a programming language.".to_string(),
        )];

        let answer = chain.run("What are its features?", history).await.unwrap();
        assert!(!answer.is_empty());
    }

    #[tokio::test]
    async fn test_conversational_retrieval_with_sources() {
        let llm = MockConversationalLLM::new();
        let retriever = create_test_retriever().await;

        let chain = ConversationalRetrievalChain::new(llm, retriever, ChainType::Stuff)
            .with_return_source_documents(true);

        let (answer, sources, _) = chain.run_full("What is Rust?", vec![]).await.unwrap();

        assert!(!answer.is_empty());
        assert!(!sources.is_empty());
    }

    #[tokio::test]
    async fn test_conversational_retrieval_with_generated_question() {
        let llm = MockConversationalLLM::new();
        let retriever = create_test_retriever().await;

        let chain = ConversationalRetrievalChain::new(llm, retriever, ChainType::Stuff)
            .with_return_generated_question(true);

        let history = vec![("What is Rust?".to_string(), "It's a language.".to_string())];

        let (answer, _, generated_q) = chain
            .run_full("What about its safety?", history)
            .await
            .unwrap();

        assert!(!answer.is_empty());
        assert!(generated_q.is_some());
        assert!(!generated_q.unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_conversational_retrieval_multi_turn() {
        let llm = MockConversationalLLM::new();
        let retriever = create_test_retriever().await;

        let chain = ConversationalRetrievalChain::new(llm, retriever, ChainType::Stuff);

        // First turn
        let mut history = vec![];
        let q1 = "What is Rust?";
        let a1 = chain.run(q1, history.clone()).await.unwrap();
        assert!(!a1.is_empty());

        // Second turn with history
        history.push((q1.to_string(), a1));
        let q2 = "What are its features?";
        let a2 = chain.run(q2, history.clone()).await.unwrap();
        assert!(!a2.is_empty());

        // Third turn with more history
        history.push((q2.to_string(), a2));
        let q3 = "Is it safe?";
        let a3 = chain.run(q3, history).await.unwrap();
        assert!(!a3.is_empty());
    }

    #[tokio::test]
    async fn test_conversational_retrieval_without_rephrase() {
        let llm = MockConversationalLLM::new();
        let retriever = create_test_retriever().await;

        let chain = ConversationalRetrievalChain::new(llm, retriever, ChainType::Stuff)
            .with_rephrase_question(false);

        let history = vec![(
            "Tell me about Rust".to_string(),
            "It's a language".to_string(),
        )];

        let answer = chain.run("More details please", history).await.unwrap();
        assert!(!answer.is_empty());
    }

    #[tokio::test]
    async fn test_format_chat_history() {
        let llm = MockConversationalLLM::new();
        let retriever = create_test_retriever().await;
        let chain = ConversationalRetrievalChain::new(llm, retriever, ChainType::Stuff);

        let history = vec![
            ("Question 1".to_string(), "Answer 1".to_string()),
            ("Question 2".to_string(), "Answer 2".to_string()),
        ];

        let formatted = chain.format_chat_history(&history);
        assert!(formatted.contains("Human: Question 1"));
        assert!(formatted.contains("Assistant: Answer 1"));
        assert!(formatted.contains("Human: Question 2"));
        assert!(formatted.contains("Assistant: Answer 2"));
    }
}
