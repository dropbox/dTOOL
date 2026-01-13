//! Chains for composing LLM workflows in `DashFlow` Rust
//!
//! This crate provides implementations of various chain patterns for combining
//! and processing documents, creating sequential workflows, and more.
//!
//! # Chain Types
//!
//! ## Core Chains
//!
//! - [`LLMChain`]: Simple prompt formatting and LLM execution
//! - [`ChatLLMChain`]: Chat model variant of LLM chain
//! - [`TransformChain`]: Apply transformation functions to inputs (no LLM)
//! - [`LLMMathChain`]: Solve mathematical problems using LLM + safe expression evaluation
//! - [`SequentialChain`]: Execute multiple chains in sequence with named inputs/outputs
//! - [`SimpleSequentialChain`]: Execute chains in sequence with single input/output per step
//!
//! ## Document Combining Chains
//!
//! - [`StuffDocumentsChain`]: Combine documents by stuffing them all into a single prompt
//! - [`MapReduceDocumentsChain`]: Process documents in parallel (map), then combine results (reduce)
//! - [`RefineDocumentsChain`]: Iteratively refine an answer by processing documents sequentially
//!
//! ## Retrieval Enhancement
//!
//! - [`HypotheticalDocumentEmbedder`]: Generate hypothetical documents for queries to improve retrieval (`HyDE`)
//!
//! ## Conversation Chains
//!
//! - [`ConversationChain`]: Basic conversation with memory
//!
//! ## Question Answering Chains
//!
//! - [`RetrievalQA`]: Combine document retrieval with LLM for question answering
//! - [`ConversationalRetrievalChain`]: QA chain that maintains conversation history
//! - [`QAGenerationChain`]: Generate question-answer pairs from text documents
//! - [`qa_with_sources::QAWithSourcesChain`]: Answer questions with source citations
//! - [`qa_with_sources::RetrievalQAWithSourcesChain`]: Retrieval-based QA with sources
//! - [`router::MultiRetrievalQAChain`]: Route questions to different retrieval QA systems based on input
//!
//! ## SQL Database Chains
//!
//! - Generate SQL queries from natural language questions
//! - Supports multiple SQL dialects (`PostgreSQL`, `MySQL`, `SQLite`, etc.)
//!
//! ## Content Moderation
//!
//! - [`OpenAIModerationChain`]: Check text for harmful content using `OpenAI`'s Moderation API
//!
//! ## Web Request Chains
//!
//! - [`LLMRequestsChain`]: Fetch content from a URL and process it with an LLM
//! - [`api::APIChain`]: Convert natural language questions to API calls and summarize responses
//!
//! ## Advanced Chains
//!
//! - [`flare::FlareChain`]: Forward-looking active retrieval (FLARE) for iterative generation with uncertainty detection
//! - [`constitutional_ai::ConstitutionalChain`]: Self-critique and revision based on constitutional principles
//! - [`llm_checker::LLMCheckerChain`]: Self-verification of LLM outputs with fact-checking
//! - [`graph_qa::GraphQAChain`]: Question answering over knowledge graphs using entity extraction
//! - [`graph_cypher_qa::GraphCypherQAChain`]: Question answering over Neo4j graphs by generating Cypher queries
//!
//! # Example
//!
//! ```rust,ignore
//! use dashflow_chains::combine_documents::StuffDocumentsChain;
//! use dashflow::core::documents::Document;
//! use dashflow::core::prompts::PromptTemplate;
//!
//! // Create a chain to summarize documents
//! let chain = StuffDocumentsChain::new(llm)
//!     .with_prompt(PromptTemplate::from_template("Summarize: {context}").unwrap())
//!     .with_document_variable_name("context");
//!
//! let docs = vec![
//!     Document::new("First document content"),
//!     Document::new("Second document content"),
//! ];
//!
//! let result = chain.combine_docs(&docs, None).await?;
//! ```

#![cfg_attr(
    test,
    allow(
        clippy::disallowed_methods,
        clippy::expect_used,
        clippy::panic,
        clippy::unwrap_used
    )
)]

pub mod api;
pub mod combine_documents;
pub mod constitutional_ai;
pub mod conversation;
pub mod conversational_retrieval;
pub mod cypher_utils;
pub mod flare;
pub mod graph_cypher_qa;
pub mod graph_qa;
pub mod hyde;
pub mod llm;
pub mod llm_checker;
pub mod llm_math;
pub mod llm_requests;
pub mod moderation;
// NOTE: natbot requires dashflow-playwright which is excluded from workspace
// due to RUSTSEC-2020-0071 (time 0.1.45 segfault vulnerability).
// Re-enable when dashflow-playwright is restored to workspace.
// #[cfg(feature = "playwright")]
// pub mod natbot;
pub mod qa_generation;
pub mod qa_with_sources;
pub mod retrieval;
pub mod retrieval_qa;
pub mod router;
pub mod sequential;
pub mod sql_database_chain;
pub mod sql_database_prompts;
pub mod summarize;
pub mod transform;

pub use api::APIChain;
pub use combine_documents::{MapReduceDocumentsChain, RefineDocumentsChain, StuffDocumentsChain};
pub use constitutional_ai::{
    critique_prompt, revision_prompt, ConstitutionalChain, ConstitutionalPrinciple,
};
pub use conversation::ConversationChain;
pub use conversational_retrieval::ConversationalRetrievalChain;
pub use cypher_utils::{extract_cypher, CypherQueryCorrector};
pub use flare::{
    extract_tokens_and_log_probs, flare_prompt, low_confidence_spans, question_generator_prompt,
    FinishedOutputParser, FlareChain, QuestionGenerator, ResponseGenerator,
};
pub use graph_cypher_qa::{cypher_generation_prompt, cypher_qa_prompt, GraphCypherQAChain};
pub use graph_qa::{
    entity_extraction_prompt, get_entities, graph_qa_prompt, EntityGraph, GraphQAChain,
    KnowledgeTriple,
};
pub use hyde::{HypotheticalDocumentEmbedder, HypotheticalDocumentEmbedderLLM};
pub use llm::{ChatLLMChain, LLMChain};
pub use llm_checker::{
    check_assertions_prompt, create_draft_answer_prompt, list_assertions_prompt,
    revised_answer_prompt, LLMCheckerChain,
};
pub use llm_math::LLMMathChain;
pub use llm_requests::LLMRequestsChain;
pub use moderation::OpenAIModerationChain;
// natbot exports disabled - dashflow-playwright excluded from workspace
// due to RUSTSEC-2020-0071 (time 0.1.45 segfault vulnerability).
// #[cfg(feature = "playwright")]
// pub use natbot::{Crawler, ElementInViewPort, NatBotChain};
pub use qa_generation::{QAGenerationChain, QAPair};
pub use qa_with_sources::{
    QAInput, QAWithSourcesChain, QAWithSourcesOutput, RetrievalQAInput, RetrievalQAWithSourcesChain,
};
pub use retrieval::{
    create_history_aware_retriever, create_retrieval_chain, HistoryAwareRetriever, RetrievalChain,
};
pub use retrieval_qa::{ChainType, RetrievalQA};
pub use router::{
    LLMRouterChain, MultiPromptChain, MultiRetrievalQAChain, PromptInfo, RetrieverInfo, Route,
    RouterOutputParser,
};
pub use sequential::{SequentialChain, SimpleSequentialChain};
pub use sql_database_chain::{generate_sql_query, SQLDatabaseInfo, SQLInput};
pub use transform::TransformChain;
