//! Chroma vector store integration for `DashFlow` Rust.
//!
//! This crate provides a Rust implementation of the Chroma vector store for `DashFlow`.
//! Chroma is an open-source embedding database that provides an efficient way to store
//! and query vector embeddings.
//!
//! # Prerequisites
//!
//! You need a running Chroma server. The easiest way is with Docker:
//!
//! ```bash
//! docker run -p 8000:8000 chromadb/chroma
//! ```
//!
//! # Examples
//!
//! ## Basic Usage
//!
//! ```ignore
//! use dashflow_chroma::ChromaVectorStore;
//! use dashflow::core::embeddings::{Embeddings, MockEmbeddings};
//! use dashflow::core::vector_stores::VectorStore;
//! use std::sync::Arc;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! # let embeddings: Arc<dyn Embeddings> = Arc::new(MockEmbeddings::new(384));
//! // Create a Chroma vector store
//! let mut store = ChromaVectorStore::new(
//!     "my_collection",
//!     embeddings,
//!     Some("http://localhost:8000"),
//! ).await?;
//!
//! // Add documents
//! let texts = ["Hello world", "Goodbye world"];
//! let ids = store.add_texts(&texts, None, None).await?;
//!
//! // Search
//! let results = store._similarity_search("Hello", 2, None).await?;
//! # Ok(())
//! # }
//! ```
//!
//! ## Complete Example
//!
//! For a comprehensive example demonstrating all features including metadata filtering,
//! MMR search, and CRUD operations, run:
//!
//! ```bash
//! cargo run --package dashflow-chroma --example chroma_basic
//! ```
//!
//! See `examples/chroma_basic.rs` for the full source code.
//!
//! # See Also
//!
//! - [`VectorStore`](dashflow::core::vector_stores::VectorStore) - The trait this implements
//! - [`Embeddings`](dashflow::core::embeddings::Embeddings) - Required for generating vectors
//! - [`dashflow-qdrant`](https://docs.rs/dashflow-qdrant) - Alternative: Qdrant vector store with similar API
//! - [`dashflow-pinecone`](https://docs.rs/dashflow-pinecone) - Alternative: Pinecone cloud-native vector store
//! - [Chroma Documentation](https://docs.trychroma.com/) - Official Chroma docs

mod chroma;
mod config_ext;

pub use chroma::ChromaVectorStore;
pub use config_ext::build_vector_store;

#[cfg(test)]
mod tests {
    use super::*;

    // ========================================================================
    // PUBLIC RE-EXPORTS
    // ========================================================================

    #[test]
    fn test_chroma_vector_store_type_is_exported() {
        // Verify ChromaVectorStore can be referenced through the crate
        fn _takes_type<T>() {}
        _takes_type::<ChromaVectorStore>();
    }

    #[test]
    fn test_build_vector_store_fn_is_exported() {
        // Verify build_vector_store is available at module level
        // We can't directly test the async fn signature, but we can verify it exists
        // by checking its return type at compile time
        async fn _verify_signature() {
            use dashflow::core::config_loader::{EmbeddingConfig, SecretReference, VectorStoreConfig};
            use dashflow::core::embeddings::Embeddings;
            use dashflow_test_utils::MockEmbeddings;
            use std::sync::Arc;

            let embeddings: Arc<dyn Embeddings> = Arc::new(MockEmbeddings::with_dimensions(1536));
            let config = VectorStoreConfig::Chroma {
                collection_name: "test".to_string(),
                url: "http://localhost:8000".to_string(),
                embedding: Box::new(EmbeddingConfig::OpenAI {
                    model: "text-embedding-3-small".to_string(),
                    api_key: SecretReference::from_env("OPENAI_API_KEY"),
                    batch_size: 512,
                }),
            };

            // This line verifies the function exists and has the expected signature
            let _result: Result<ChromaVectorStore, dashflow::core::Error> =
                build_vector_store(&config, embeddings).await;
        }
    }

    // ========================================================================
    // MODULE STRUCTURE
    // ========================================================================

    #[test]
    fn test_chroma_module_exists() {
        // The chroma module should exist (tested implicitly by re-export)
        let _ = std::any::type_name::<ChromaVectorStore>();
    }

    #[test]
    fn test_config_ext_module_exists() {
        // The config_ext module should exist (tested implicitly by re-export)
        let _ = build_vector_store as fn(_, _) -> _;
    }

    // ========================================================================
    // TYPE PROPERTIES
    // ========================================================================

    #[test]
    fn test_chroma_vector_store_is_send() {
        fn assert_send<T: Send>() {}
        assert_send::<ChromaVectorStore>();
    }

    #[test]
    fn test_chroma_vector_store_is_sync() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<ChromaVectorStore>();
    }

    #[test]
    fn test_chroma_vector_store_type_name() {
        let name = std::any::type_name::<ChromaVectorStore>();
        assert!(
            name.contains("ChromaVectorStore"),
            "Type name should contain 'ChromaVectorStore', got: {name}"
        );
    }

    // ========================================================================
    // CRATE METADATA (via Cargo.toml)
    // ========================================================================

    #[test]
    fn test_crate_name() {
        // Verify we can reference the crate
        assert!(
            std::any::type_name::<ChromaVectorStore>().contains("dashflow_chroma"),
            "Should be in dashflow_chroma crate"
        );
    }
}
