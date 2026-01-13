//! Prelude module - provides common imports for production code
//!
//! This module re-exports types, traits, and functions commonly used in DashFlow applications.
//! Use `use dashflow::prelude::*;` for convenient imports.
//!
//! For test code, consider using `dashflow::test_prelude::*` which includes
//! additional test utilities like `FakeChatModel` and `MockEmbeddings`.

// Standard library re-exports
pub use std::collections::HashMap;
pub use std::sync::Arc;

// External crate re-exports
pub use async_trait::async_trait;
pub use serde::{Deserialize, Serialize};
pub use serde_json::{json, Value};
pub use uuid::Uuid;

// Core error types
pub use crate::core::error::{Error, ErrorCategory, NetworkErrorKind, Result};

// Messages - essential types for agent communication
pub use crate::core::messages::{
    AIMessage, AIMessageChunk, AsLlmMessage, BaseMessageFields, ContentBlock, HumanMessage,
    IntoLlmMessage, LlmMessageRef, Message, MessageContent, MessageLike, MessageType, ToolCall,
};

// Tools - essential for building agents
pub use crate::core::tools::{StructuredTool, Tool, ToolException, ToolInput};

// Language models - essential traits and types
pub use crate::core::language_models::{
    ChatGeneration, ChatGenerationChunk, ChatModel, ChatResult, Generation, GenerationChunk,
    LLMResult, ToolChoice, ToolDefinition,
};

// Callbacks
pub use crate::core::callbacks::{
    CallbackEvent, CallbackHandler, CallbackManager, ConsoleCallbackHandler,
};

// Documents
pub use crate::core::documents::{Document, DocumentLoader};

// Embeddings
pub use crate::core::embeddings::{CacheConfig, CachedEmbeddings, Embeddings};

// Retrievers - common retriever interface
pub use crate::core::retrievers::{
    Retriever, RetrieverInput, RetrieverOutput, SearchConfig, VectorStoreRetriever,
};

// Vector stores
pub use crate::core::vector_stores::{DistanceMetric, SearchParams, SearchType, VectorStore};

// Output parsers
pub use crate::core::output_parsers::{
    CommaSeparatedListOutputParser, JsonOutputParser, OutputParser, StrOutputParser,
};

// Prompts
pub use crate::core::prompt_values::{ChatPromptValue, StringPromptValue};
pub use crate::core::prompts::{ChatPromptTemplate, MessagesPlaceholder, PromptTemplate};

// Config
pub use crate::core::config::RunnableConfig;

// Runnable - core trait for composability
pub use crate::core::runnable::{
    Graph, Runnable, RunnableLambda, RunnableParallel, RunnablePassthrough, RunnableSequence,
    StreamEventData, StreamEventType,
};

// Config loader
pub use crate::core::config_loader::{
    expand_env_vars, ChainConfig, ChatModelConfig, DashFlowConfig, EmbeddingConfig, PromptConfig,
    VectorStoreConfig,
};

// HTTP client
pub use crate::core::http_client::{
    create_basic_client, create_llm_client, HttpClientBuilder, TlsConfig,
};

// DashFlow graph types - essential for building workflows
pub use crate::edge::{ConditionalEdge, Edge, ParallelEdge, END, START};
pub use crate::executor::CompiledGraph;
pub use crate::graph::{GraphBuilder, StateGraph};
pub use crate::node::Node;
pub use crate::prebuilt::{create_react_agent, AgentState};
pub use crate::state::{GraphState, JsonState, MergeableState};
pub use crate::stream::{StreamEvent, StreamMode};

// Derive macros for custom state types
// The derive macro enables #[derive(GraphState)] for custom state with reducers
// Note: This doesn't conflict with the GraphState trait - they're used in different contexts
pub use dashflow_macros::GraphState;

// Checkpoint types
pub use crate::checkpoint::{
    Checkpoint, CheckpointId, CheckpointMetadata, Checkpointer, ThreadId, ThreadInfo,
};

// Reducers for state management
pub use crate::reducer::{add_messages, AddMessagesReducer, Reducer};

// Quality gates
pub use crate::quality::{QualityGate, QualityGateConfig, QualityScore, ValidationResult};

// Introspection - for AI self-awareness
pub use crate::introspection::{ExecutionContext, GraphManifest, NodeManifest, ToolManifest};

// Approval flow for human-in-the-loop
pub use crate::approval::{ApprovalNode, ApprovalRequest, ApprovalResponse, RiskLevel};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prelude_smoke_exports_compile_and_run() {
        let mut map: HashMap<String, Value> = HashMap::new();
        map.insert("k".to_string(), json!({"v": 1}));

        let shared: Arc<HashMap<String, Value>> = Arc::new(map);
        assert_eq!(shared.get("k"), Some(&json!({"v": 1})));

        let _id = Uuid::nil();
        let _graph: GraphBuilder<JsonState> = GraphBuilder::builder();
        let _ = (START, END);

        let msg = HumanMessage::new("hello");
        assert!(matches!(msg.content(), MessageContent::Text(_)));
    }
}
