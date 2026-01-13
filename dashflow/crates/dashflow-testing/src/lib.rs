// Allow unwrap in testing utilities - test code should panic on errors
#![allow(clippy::unwrap_used)]

//! # DashFlow Testing Utilities
//!
//! This crate provides testing utilities for DashFlow applications, including:
//!
//! - **MockTool**: A generic mock tool for testing tool-using agents
//! - **MockEmbeddings**: Re-exported from dashflow core for convenience
//!
//! ## Quick Start
//!
//! ```rust,ignore
//! use dashflow_testing::{MockTool, MockEmbeddings};
//!
//! // Create a mock tool
//! let mock_tool = MockTool::new("calculator")
//!     .with_description("Performs calculations")
//!     .with_handler(|input| Ok(format!("Result: {}", input)));
//!
//! // Use mock embeddings for testing
//! let embeddings = MockEmbeddings::new(384);
//! ```

mod mock_tool;

pub use mock_tool::{MockTool, MockToolBuilder};

// Re-export useful testing utilities from dashflow
pub use dashflow::core::embeddings::MockEmbeddings;

/// Prelude module for convenient imports
pub mod prelude {
    pub use crate::{MockEmbeddings, MockTool, MockToolBuilder};
}

#[cfg(test)]
mod tests {
    use super::*;
    use dashflow::core::embeddings::Embeddings;
    use dashflow::core::tools::Tool;

    #[test]
    fn test_mock_tool_creation() {
        let tool = MockTool::new("test_tool").with_description("A test tool");
        assert_eq!(tool.name(), "test_tool");
    }

    #[test]
    fn test_mock_embeddings_reexport() {
        let embeddings = MockEmbeddings::new(384);
        // Just verify it's accessible
        let _ = embeddings;
    }

    // ==========================================================================
    // Re-export Verification Tests
    // ==========================================================================

    #[test]
    fn test_mock_tool_builder_reexport() {
        let builder = MockToolBuilder::new("test");
        let _ = builder.build();
    }

    #[test]
    fn test_prelude_mock_tool() {
        use crate::prelude::MockTool as PreludeMockTool;
        let tool = PreludeMockTool::new("prelude_test");
        assert_eq!(tool.name(), "prelude_test");
    }

    #[test]
    fn test_prelude_mock_tool_builder() {
        use crate::prelude::MockToolBuilder as PreludeBuilder;
        let tool = PreludeBuilder::new("prelude_builder").build();
        assert_eq!(tool.name(), "prelude_builder");
    }

    #[test]
    fn test_prelude_mock_embeddings() {
        use crate::prelude::MockEmbeddings as PreludeEmb;
        let emb = PreludeEmb::new(128);
        let _ = emb;
    }

    // ==========================================================================
    // MockEmbeddings Tests
    // ==========================================================================

    #[test]
    fn test_mock_embeddings_different_dimensions() {
        let dims = [64, 128, 256, 384, 512, 768, 1024, 1536, 3072];
        for dim in dims {
            let emb = MockEmbeddings::new(dim);
            let _ = emb;
        }
    }

    #[test]
    fn test_mock_embeddings_dimension_one() {
        let emb = MockEmbeddings::new(1);
        let _ = emb;
    }

    #[tokio::test]
    async fn test_mock_embeddings_embed_query() {
        let emb = MockEmbeddings::new(128);
        let result = emb._embed_query("test query").await.unwrap();
        assert_eq!(result.len(), 128);
    }

    #[tokio::test]
    async fn test_mock_embeddings_embed_documents() {
        let emb = MockEmbeddings::new(256);
        let docs = vec!["doc1".to_string(), "doc2".to_string(), "doc3".to_string()];
        let results = emb._embed_documents(&docs).await.unwrap();
        assert_eq!(results.len(), 3);
        for result in results {
            assert_eq!(result.len(), 256);
        }
    }

    #[tokio::test]
    async fn test_mock_embeddings_deterministic() {
        let emb = MockEmbeddings::new(64);
        let r1 = emb._embed_query("same text").await.unwrap();
        let r2 = emb._embed_query("same text").await.unwrap();
        assert_eq!(r1, r2);
    }

    #[tokio::test]
    async fn test_mock_embeddings_different_text() {
        let emb = MockEmbeddings::new(64);
        let r1 = emb._embed_query("text one").await.unwrap();
        let r2 = emb._embed_query("text two").await.unwrap();
        assert_ne!(r1, r2);
    }

    #[tokio::test]
    async fn test_mock_embeddings_empty_query() {
        let emb = MockEmbeddings::new(128);
        let result = emb._embed_query("").await.unwrap();
        assert_eq!(result.len(), 128);
    }

    #[tokio::test]
    async fn test_mock_embeddings_empty_documents() {
        let emb = MockEmbeddings::new(128);
        let docs: Vec<String> = vec![];
        let results = emb._embed_documents(&docs).await.unwrap();
        assert!(results.is_empty());
    }

    #[tokio::test]
    async fn test_mock_embeddings_unicode() {
        let emb = MockEmbeddings::new(128);
        let result = emb._embed_query("你好世界").await.unwrap();
        assert_eq!(result.len(), 128);
    }

    #[tokio::test]
    async fn test_mock_embeddings_long_text() {
        let emb = MockEmbeddings::new(128);
        let long_text = "a".repeat(10000);
        let result = emb._embed_query(&long_text).await.unwrap();
        assert_eq!(result.len(), 128);
    }

    // ==========================================================================
    // Integration Tests
    // ==========================================================================

    #[tokio::test]
    async fn test_mock_tool_with_mock_embeddings_workflow() {
        use dashflow::core::tools::{Tool, ToolInput};

        // Simulate a tool that uses embeddings
        let embeddings = MockEmbeddings::new(128);

        let tool = MockTool::new("semantic_search")
            .with_description("Searches using semantic similarity");

        // Use embeddings
        let query_emb = embeddings._embed_query("search query").await.unwrap();
        assert_eq!(query_emb.len(), 128);

        // Use tool
        let result = tool
            ._call(ToolInput::String("search for rust".to_string()))
            .await
            .unwrap();
        assert_eq!(result, "Mock tool response");
    }

    #[tokio::test]
    async fn test_multiple_tools_workflow() {
        use dashflow::core::tools::{Tool, ToolInput};

        let search = MockTool::new("search").with_response("search results");
        let analyze = MockTool::new("analyze").with_response("analysis done");
        let summarize = MockTool::new("summarize").with_response("summary");

        let r1 = search
            ._call(ToolInput::String("query".to_string()))
            .await
            .unwrap();
        let r2 = analyze
            ._call(ToolInput::String(r1))
            .await
            .unwrap();
        let r3 = summarize
            ._call(ToolInput::String(r2))
            .await
            .unwrap();

        assert_eq!(r3, "summary");
        assert_eq!(search.call_count(), 1);
        assert_eq!(analyze.call_count(), 1);
        assert_eq!(summarize.call_count(), 1);
    }

    #[test]
    fn test_mock_tool_as_trait_object() {
        let tool: Box<dyn Tool> = Box::new(MockTool::new("boxed"));
        assert_eq!(tool.name(), "boxed");
    }

    #[test]
    fn test_mock_tool_in_vec() {
        let tools: Vec<MockTool> = vec![
            MockTool::new("tool1"),
            MockTool::new("tool2"),
            MockTool::new("tool3"),
        ];
        assert_eq!(tools.len(), 3);
        assert_eq!(tools[0].name(), "tool1");
        assert_eq!(tools[1].name(), "tool2");
        assert_eq!(tools[2].name(), "tool3");
    }

    #[tokio::test]
    async fn test_tool_chain_with_handler() {
        use dashflow::core::tools::ToolInput;

        let tool1 = MockTool::new("step1")
            .with_handler(|input| Ok(format!("processed:{}", input)));
        let tool2 = MockTool::new("step2")
            .with_handler(|input| Ok(format!("final:{}", input)));

        let r1 = tool1
            ._call(ToolInput::String("start".to_string()))
            .await
            .unwrap();
        let r2 = tool2._call(ToolInput::String(r1)).await.unwrap();

        assert_eq!(r2, "final:processed:start");
    }

    // ==========================================================================
    // Error Scenarios
    // ==========================================================================

    #[tokio::test]
    async fn test_tool_failure_recovery_pattern() {
        use dashflow::core::tools::ToolInput;

        let tool = MockTool::new("flaky");

        // First call fails
        tool.fail_next();
        let r1 = tool._call(ToolInput::String("attempt1".to_string())).await;
        assert!(r1.is_err());

        // Retry succeeds
        let r2 = tool._call(ToolInput::String("attempt2".to_string())).await;
        assert!(r2.is_ok());

        // Track both attempts
        assert_eq!(tool.call_count(), 2);
    }

    #[tokio::test]
    async fn test_tool_conditional_failure() {
        use dashflow::core::tools::ToolInput;

        let tool = MockTool::new("conditional")
            .with_handler(|input| {
                if input.contains("fail") {
                    Err(dashflow::core::Error::tool_error("Triggered failure"))
                } else {
                    Ok("success".to_string())
                }
            });

        let r1 = tool
            ._call(ToolInput::String("normal input".to_string()))
            .await;
        assert!(r1.is_ok());

        let r2 = tool
            ._call(ToolInput::String("please fail now".to_string()))
            .await;
        assert!(r2.is_err());
    }
}
