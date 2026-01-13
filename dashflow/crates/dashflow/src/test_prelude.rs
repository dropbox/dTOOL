//! Test prelude module - provides common imports for test modules
//!
//! This module re-exports types, traits, macros, and functions commonly used in tests.
//! Test modules should use `use dashflow::test_prelude::*;` instead of manually importing.

// Standard library re-exports
pub use std::collections::HashMap;
pub use std::env;
pub use std::path::Path;
pub use std::pin::Pin;
pub use std::sync::{Arc, Mutex, RwLock};
pub use std::time::{Duration, Instant};

// External crate re-exports
pub use async_trait::async_trait;
pub use chrono::Utc;
pub use futures::Stream;
pub use serde_json::{json, Value};
pub use uuid::Uuid;

// Core error types
pub use crate::core::error::Error as DashFlowError;
pub use crate::core::error::{Error, ErrorCategory, NetworkErrorKind, Result};
pub use std::result::Result as StdResult;

// Messages
pub use crate::core::messages::Message as BaseMessage;
pub use crate::core::messages::{filter_messages, get_buffer_string, trim_messages};
pub use crate::core::messages::{
    AIMessage, AIMessageChunk, AsLlmMessage, BaseMessageFields, ContentBlock, ExcludeToolCalls,
    HumanMessage, IntoLlmMessage, LlmMessageRef, Message, MessageContent, MessageLike, MessageType,
    MessageTypeFilter, ToolCall, TrimError, TrimStrategy,
};

// Tools
pub use crate::core::tools::builtin::{
    calculator_tool, file_delete_tool, list_directory_tool, shell_tool,
};
pub use crate::core::tools::{
    sync_function_tool, sync_structured_tool, StructuredTool, Tool, ToolException, ToolInput,
};

// Agents
#[allow(deprecated)] // Re-exporting deprecated types for backward compatibility
pub use crate::core::agents::{
    Agent, AgentAction, AgentCheckpointState, AgentContext, AgentDecision, AgentExecutor,
    AgentExecutorConfig, AgentExecutorResult, AgentFinish, AgentMiddleware, AgentStep,
    BufferMemory, Checkpoint, ConversationBufferWindowMemory, FileCheckpoint,
    HumanInTheLoopMiddleware, JsonChatAgent, LoggingMiddleware, Memory, MemoryCheckpoint,
    ModelFallbackMiddleware, OpenAIFunctionsAgent, OpenAIToolsAgent, RateLimitMiddleware,
    ReActAgent, RetryMiddleware, SelfAskWithSearchAgent, StructuredChatAgent, TimeoutMiddleware,
    ToolCallingAgent, ValidationMiddleware, XmlAgent,
};
// ToolEmulatorMiddleware is only available when testing feature is enabled
#[cfg(any(test, feature = "testing"))]
pub use crate::core::agents::ToolEmulatorMiddleware;

// Runnable
pub use crate::core::config::RunnableConfig;
pub use crate::core::runnable::{
    get_unique_config_specs, ConfigurableFieldSpec, GetSessionHistoryFn, Graph, RouterInput,
    RouterRunnable, Runnable, RunnableBindingBase, RunnableBranch, RunnableEach, RunnableGenerator,
    RunnableLambda, RunnableParallel, RunnablePassthrough, RunnablePick, RunnableRetry,
    RunnableSequence, RunnableWithFallbacks, RunnableWithMessageHistory, StreamEventData,
    StreamEventType, StreamEventsOptions,
};

// Language models
pub use crate::core::language_models::structured::{extract_json, StructuredChatModel};
pub use crate::core::language_models::{
    ChatGeneration, ChatGenerationChunk, ChatModel, ChatResult, FakeChatModel, FakeLLM, Generation,
    GenerationChunk, LLMResult, ReinforceConfig, ReinforceExample, ReinforceJob,
    ReinforceJobStatus, ToolChoice, ToolDefinition,
};

// Callbacks
pub use crate::core::callbacks::{
    CallbackEvent, CallbackHandler, CallbackManager, ConsoleCallbackHandler, NullCallbackHandler,
};

// Documents
pub use crate::core::documents::{Blob, Document, DocumentCompressor, DocumentLoader};
pub use crate::core::retrievers::parent_document_retriever::TextSplitter;

// Embeddings
pub use crate::core::embeddings::{CacheConfig, CachedEmbeddings, Embeddings, MockEmbeddings};

// Retrievers
pub use crate::core::retrievers::{
    bm25_retriever::BM25Retriever,
    knn_retriever::KNNRetriever,
    merger_retriever::MergerRetriever,
    parent_document_retriever::ParentDocumentRetriever,
    rephrase_query_retriever::{RePhraseQueryRetriever, DEFAULT_TEMPLATE},
    self_query::SelfQueryRetriever,
    tfidf_retriever::TFIDFRetriever,
    time_weighted_retriever::TimeWeightedVectorStoreRetriever,
    web_research_retriever::{WebResearchRetriever, WebSearchTool, DEFAULT_SEARCH_PROMPT},
    ContextualCompressionRetriever, EnsembleRetriever, MultiQueryRetriever, MultiVectorRetriever,
    Retriever, RetrieverInput, RetrieverOutput, SearchConfig, VectorStoreRetriever,
};

// Deprecated stub retrievers - only available with the "stub-retrievers" feature
#[cfg(feature = "stub-retrievers")]
#[allow(deprecated)]
pub use crate::core::retrievers::{
    elasticsearch_bm25_retriever::ElasticSearchBM25Retriever,
    pinecone_hybrid_search_retriever::{PineconeHybridConfig, PineconeHybridSearchRetriever},
    weaviate_hybrid_search_retriever::{WeaviateHybridConfig, WeaviateHybridSearchRetriever},
};

// Indexing
pub use crate::core::indexing::{
    deduplicate_documents, hash_document, index, CleanupMode, DeleteResponse, DocumentIndex,
    HashAlgorithm, InMemoryRecordManager, IndexingResult, RecordManager, UpsertResponse,
};

// Vector stores
pub use crate::core::vector_stores::{
    DistanceMetric, InMemoryVectorStore, SearchParams, SearchType, VectorStore,
};

// Caches
pub use crate::core::caches::{BaseCache, InMemoryCache};

// Stores
pub use crate::core::stores::{BaseStore, InMemoryByteStore, InMemoryStore};

// Rate limiters
pub use crate::core::rate_limiters::{InMemoryRateLimiter, RateLimiter};

// Retry
pub use crate::core::retry::{with_retry, RetryPolicy, RetryStrategy};

// Output parsers
pub use crate::core::output_parsers::{
    BooleanOutputParser, CommaSeparatedListOutputParser, DatetimeOutputParser, EnumOutputParser,
    JsonOutputParser, LineListOutputParser, MarkdownListOutputParser, NumberedListOutputParser,
    OutputFixingParser, OutputParser, PandasDataFrameOutputParser, QuestionListOutputParser,
    RegexDictParser, RegexParser, StrOutputParser, TransformOutputParser, XMLOutputParser,
    YamlOutputParser,
};

// Example selectors
pub use crate::core::prompts::example_selector::LengthBasedExampleSelector;

// Document loaders
pub use crate::core::document_loaders::JSONLoader as JsonLoader;
pub use crate::core::document_loaders::{
    BinaryFileLoader, CSVLoader, DirectoryLoader, GraphQLLoader, HTMLLoader, IniLoader, JSONLoader,
    JsonnetLoader, MarkdownLoader, NotebookLoader, PowerPointLoader, RSTLoader, SRTLoader,
    SwiftLoader, TOMLLoader, TSVLoader, TextLoader, UnstructuredFileLoader, XMLLoader, YAMLLoader,
};

// Document transformers
pub use crate::core::document_transformers::{
    beautiful_soup_transformer::BeautifulSoupTransformer,
    embeddings_clustering_filter::EmbeddingsClusteringFilter,
    embeddings_redundant_filter::EmbeddingsRedundantFilter,
    google_translate_transformer::{
        GoogleTranslateConfig, GoogleTranslateTransformer, TranslationParams,
    },
    html2text_transformer::Html2TextTransformer,
    long_context_reorder::LongContextReorder,
    markdownify_transformer::{HeadingStyle, MarkdownifyTransformer},
    metadata_tagger::MetadataTagger,
    nuclia_text_transformer::NucliaTextTransformer,
    DocumentTransformer,
};

// Document formatting
pub use crate::core::chains::{
    format_documents, DEFAULT_DOCUMENTS_KEY, DEFAULT_DOCUMENT_SEPARATOR,
};

// Tracers - includes RunType
pub use crate::core::tracers::{
    DashFlowTracer, FunctionCallbackHandler, RootListenersTracer, RunCollectorCallbackHandler,
    RunTree, RunType,
};

// Agent patterns
pub use crate::core::agent_patterns::{
    DebateContribution, DebateRound, DebateState, Debater, ExecutionPlan, IterationResult,
    MultiAgentDebate, PlanAndExecuteAgent, PlanAndExecuteAgentBuilder, PlanAndExecuteConfig,
    PlanStep, ReflectionAgent, ReflectionState,
};

// Structured query
pub use crate::core::retrievers::self_query::QueryConstructor;
pub use crate::core::structured_query::query_constructor::StructuredQueryOutputParser;
pub use crate::core::structured_query::visitors::{
    ChromaTranslator, ElasticsearchTranslator, PineconeTranslator, QdrantTranslator,
    WeaviateTranslator,
};
pub use crate::core::structured_query::{
    AttributeInfo, Comparator, Comparison, FilterDirective, Operation, Operator, StructuredQuery,
};

// Prompts
pub use crate::core::prompt_values::{ChatPromptValue, StringPromptValue};
pub use crate::core::prompts::base::PromptTemplateFormat;
pub use crate::core::prompts::chat::MessageTemplate;
pub use crate::core::prompts::{ChatPromptTemplate, MessagesPlaceholder, PromptTemplate};

// Chat history
pub use crate::core::chat_history::{
    BaseChatMessageHistory, FileChatMessageHistory, InMemoryChatMessageHistory,
};

// Config loader
pub use crate::core::config_loader::{
    expand_env_vars, ChainConfig, ChainStepConfig, ChatModelConfig, DashFlowConfig,
    EmbeddingConfig, PromptConfig, SecretReference, VectorStoreConfig,
};

// Serialization
pub use crate::core::serialization::{Serializable, SerializedObject, SERIALIZATION_VERSION};

// Schema
pub use crate::core::schema::json_schema;

// Node and Edge for graphs
pub use crate::edge::Edge;
pub use crate::node::Node;

// Http client
pub use crate::core::http_client::HttpClientBuilder;
pub use crate::core::http_client::TlsConfig;

// Observability
pub use crate::core::observability::{
    CustomMetricsRegistry, DeterministicEventEmitter, LLMMetrics, MetricChangeResult,
    MetricChangeVerifier, TEST_EVENT_METRIC_NAME,
};

// Config
pub use crate::core::config::RunnableConfig as Config;

// Utils
pub use crate::core::utils::{
    abatch_iterate, batch_iterate, env_var_is_set, get_from_dict_or_env, get_from_env,
    RuntimeEnvironment, LC_AUTO_PREFIX,
};
pub use crate::core::vector_stores::maximal_marginal_relevance;

// Serde helpers
pub use crate::core::serde_helpers::{from_json, to_json, to_json_pretty};

// JSON Value alias
pub use serde_json::Value as JsonValue;

// Deserialization helpers
pub use crate::core::deserialization::{
    get_optional_f32, get_optional_string, get_string, get_string_array, get_string_map, get_u32,
};

// Usage metadata
pub use crate::core::usage::UsageMetadata;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_prelude_smoke_exports_compile_and_run() {
        let started = Instant::now();
        let _ = Duration::from_millis(1);

        let mut map: HashMap<String, Value> = HashMap::new();
        map.insert("answer".to_string(), json!(42));
        assert_eq!(map.get("answer"), Some(&json!(42)));

        let _id = Uuid::nil();
        let tool = calculator_tool();
        assert_eq!(tool.name(), "calculator");

        let elapsed = started.elapsed();
        assert!(elapsed >= Duration::from_millis(0));
    }
}

// HTTP client
pub use crate::core::http_client::{create_basic_client, create_llm_client};

// Introspection types for test trace construction
pub use crate::introspection::{ErrorTrace, ExecutionTrace, ExecutionTraceBuilder, NodeExecution};

/// Creates a standard test trace with typical execution nodes.
///
/// Returns a trace with:
/// - 4 nodes: input (100ms), reasoning (500ms), tool_call (1000ms), output (200ms)
/// - Total duration: 1800ms
/// - Total tokens: 5000
/// - Tools used: search, calculate
/// - Completed: true
///
/// This is suitable for most test cases that need a realistic execution trace.
pub fn create_standard_test_trace() -> ExecutionTrace {
    ExecutionTraceBuilder::new()
        .thread_id("test-thread")
        .execution_id("test-exec")
        .add_node_execution(NodeExecution::new("input", 100).with_tokens(500))
        .add_node_execution(NodeExecution::new("reasoning", 500).with_tokens(3000))
        .add_node_execution(
            NodeExecution::new("tool_call", 1000)
                .with_tokens(1000)
                .with_tools(vec!["search".to_string(), "calculate".to_string()]),
        )
        .add_node_execution(NodeExecution::new("output", 200).with_tokens(500))
        .total_duration_ms(1800)
        .total_tokens(5000)
        .completed(true)
        .build()
}

/// Creates a test trace with an error.
///
/// Returns a trace with:
/// - 2 nodes: input (100ms), processing (500ms with "Connection timeout" error)
/// - Total duration: 600ms
/// - Completed: false
///
/// This is suitable for testing error handling and failure scenarios.
pub fn create_error_test_trace() -> ExecutionTrace {
    ExecutionTraceBuilder::new()
        .add_node_execution(NodeExecution::new("input", 100))
        .add_node_execution(NodeExecution::new("processing", 500).with_error("Connection timeout"))
        .add_error(ErrorTrace::new("processing", "Connection timeout"))
        .total_duration_ms(600)
        .completed(false)
        .build()
}
