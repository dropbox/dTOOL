//! Document Compressors for `DashFlow` Rust
//!
//! This crate provides LLM-based document compressors for post-processing retrieved documents.
//! Common use cases include:
//! - Filtering irrelevant documents using LLM judgment
//! - Extracting only relevant portions of documents
//! - Filtering by embedding similarity to the query
//!
//! # Examples
//!
//! ## `LLMChainFilter`
//!
//! Filter documents based on LLM relevance judgment:
//!
//! ```rust,ignore
//! use dashflow_document_compressors::LLMChainFilter;
//! use dashflow_openai::ChatOpenAI;
//!
//! let llm = ChatOpenAI::with_config(Default::default()).with_model("gpt-4o-mini");
//! let filter = LLMChainFilter::from_llm(llm, None)?;
//!
//! let filtered = filter.compress_documents(documents, "What is Rust?", None).await?;
//! ```
//!
//! ## `LLMChainExtractor`
//!
//! Extract only relevant parts of documents:
//!
//! ```rust,ignore
//! use dashflow_document_compressors::LLMChainExtractor;
//! use dashflow_openai::ChatOpenAI;
//!
//! let llm = ChatOpenAI::with_config(Default::default()).with_model("gpt-4o-mini");
//! let extractor = LLMChainExtractor::from_llm(llm, None)?;
//!
//! let extracted = extractor.compress_documents(documents, "What is Rust?", None).await?;
//! ```
//!
//! ## `EmbeddingsFilter`
//!
//! Filter by embedding similarity:
//!
//! ```rust,ignore
//! use dashflow_document_compressors::EmbeddingsFilter;
//! use dashflow_openai::OpenAIEmbeddings;
//!
//! let embeddings = OpenAIEmbeddings::new();
//! let filter = EmbeddingsFilter::new(embeddings).with_k(5);
//!
//! let filtered = filter.compress_documents(documents, "What is Rust?", None).await?;
//! ```
//!
//! ## `CrossEncoderRerank`
//!
//! Rerank documents using cross-encoder models:
//!
//! ```rust,ignore
//! use dashflow_document_compressors::{CrossEncoderRerank, CrossEncoder};
//!
//! // Requires a CrossEncoder implementation (ONNX, API, etc.)
//! let model: Box<dyn CrossEncoder> = get_cross_encoder_model();
//! let reranker = CrossEncoderRerank::new(model).with_top_n(5);
//!
//! let reranked = reranker.compress_documents(documents, "What is Rust?", None).await?;
//! ```
//!
//! ## `LLMListwiseRerank`
//!
//! Rerank documents using LLM judgment:
//!
//! ```rust,ignore
//! use dashflow_document_compressors::LLMListwiseRerank;
//! use dashflow_openai::ChatOpenAI;
//!
//! let llm = ChatOpenAI::with_config(Default::default()).with_model("gpt-4o-mini");
//! let reranker = LLMListwiseRerank::from_llm(llm, None).with_top_n(3);
//!
//! let reranked = reranker.compress_documents(documents, "Who is Steve?", None).await?;
//! ```

mod cross_encoder;
mod cross_encoder_rerank;
mod embeddings_filter;
mod listwise_rerank;
mod llm_chain_extractor;
mod llm_chain_filter;

pub use cross_encoder::CrossEncoder;
pub use cross_encoder_rerank::CrossEncoderRerank;
pub use embeddings_filter::EmbeddingsFilter;
pub use listwise_rerank::LLMListwiseRerank;
pub use llm_chain_extractor::LLMChainExtractor;
pub use llm_chain_filter::LLMChainFilter;
