// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

//! Graph-Based API - The One Correct Path
//!
//! This module provides convenience functions that route ALL LLM, embedding,
//! and tool calls through the DashFlow graph infrastructure. This ensures:
//!
//! - **ExecutionTrace collection** for optimizers
//! - **Streaming events** for live progress
//! - **Introspection** so agents can explain themselves
//! - **Metrics** for token/cost tracking
//! - **A/B testing** and prompt evolution
//!
//! The overhead of using graphs is ~100us vs 500-5000ms for LLM calls (0.002-0.02%).
//! There is NO performance reason to bypass infrastructure.

use crate::core::documents::Document;
use crate::core::embeddings::Embeddings;
use crate::core::language_models::{ChatModel, ChatResult, ToolChoice, ToolDefinition};
use crate::core::messages::{AsLlmMessage, BaseMessage};
use crate::core::retrievers::Retriever;
use crate::core::tools::{Tool, ToolInput};
use crate::core::vector_stores::VectorStore;
use crate::error::{Error, Result};
use crate::node::Node;
use crate::{CompiledGraph, StateGraph, END};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

/// State for single-shot LLM generation through the graph infrastructure.
///
/// This state type is used internally by [`generate()`] and [`generate_with_options()`]
/// to route LLM calls through the graph system.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct GenerateState {
    /// Input messages for the LLM
    #[serde(default)]
    pub messages: Vec<BaseMessage>,

    /// Optional stop sequences for generation
    #[serde(default)]
    pub stop: Option<Vec<String>>,

    /// Optional tool definitions for function calling
    #[serde(default)]
    pub tools: Option<Vec<ToolDefinition>>,

    /// Optional tool choice configuration
    #[serde(default)]
    pub tool_choice: Option<ToolChoice>,

    /// Output: the generated result (populated by the node)
    #[serde(default)]
    pub result: Option<ChatResult>,
}

impl crate::state::MergeableState for GenerateState {
    fn merge(&mut self, other: &Self) {
        if other.result.is_some() {
            self.result = other.result.clone();
        }
    }
}

/// State for embedding operations through the graph infrastructure.
///
/// This state type is used internally by [`embed()`] and [`embed_query()`]
/// to route embedding calls through the graph system.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct EmbedState {
    /// Input texts to embed
    #[serde(default)]
    pub texts: Vec<String>,

    /// Output: the embedding vectors (populated by the node)
    #[serde(default)]
    pub vectors: Option<Vec<Vec<f32>>>,
}

impl crate::state::MergeableState for EmbedState {
    fn merge(&mut self, other: &Self) {
        if other.vectors.is_some() {
            self.vectors = other.vectors.clone();
        }
    }
}

/// State for tool execution through the graph infrastructure.
///
/// This state type is used internally by [`call_tool()`] and [`call_tool_structured()`]
/// to route tool calls through the graph system.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ToolCallState {
    /// Input to pass to the tool
    #[serde(default)]
    pub input: String,

    /// Output: the tool's result (populated by the node)
    #[serde(default)]
    pub result: Option<String>,
}

impl crate::state::MergeableState for ToolCallState {
    fn merge(&mut self, other: &Self) {
        if other.result.is_some() {
            self.result = other.result.clone();
        }
    }
}

struct GenerateNode<M: ChatModel + ?Sized> {
    model: Arc<M>,
}

#[async_trait]
impl<M: ChatModel + 'static + ?Sized> Node<GenerateState> for GenerateNode<M> {
    async fn execute(&self, mut state: GenerateState) -> Result<GenerateState> {
        let result = self
            .model
            ._generate(
                &state.messages,
                state.stop.as_deref(),
                state.tools.as_deref(),
                state.tool_choice.as_ref(),
                None,
            )
            .await?;
        state.result = Some(result);
        Ok(state)
    }
    fn name(&self) -> String {
        "generate".to_string()
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

struct EmbedNode<E: Embeddings + ?Sized> {
    embeddings: Arc<E>,
}

#[async_trait]
impl<E: Embeddings + 'static + ?Sized> Node<EmbedState> for EmbedNode<E> {
    async fn execute(&self, mut state: EmbedState) -> Result<EmbedState> {
        let vectors = self
            .embeddings
            ._embed_documents(&state.texts)
            .await
            .map_err(|e| Error::NodeExecution {
                node: "embed".to_string(),
                source: Box::new(e),
            })?;
        state.vectors = Some(vectors);
        Ok(state)
    }
    fn name(&self) -> String {
        "embed".to_string()
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

struct ToolNode<T: Tool> {
    tool: Arc<T>,
}

#[async_trait]
impl<T: Tool + 'static> Node<ToolCallState> for ToolNode<T> {
    async fn execute(&self, mut state: ToolCallState) -> Result<ToolCallState> {
        let result = self
            .tool
            ._call(ToolInput::String(state.input.clone()))
            .await?;
        state.result = Some(result);
        Ok(state)
    }
    fn name(&self) -> String {
        "tool_call".to_string()
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

fn build_generate_graph<M: ChatModel + 'static + ?Sized>(
    model: Arc<M>,
) -> Result<CompiledGraph<GenerateState>> {
    let node = GenerateNode { model };
    let mut graph: StateGraph<GenerateState> = StateGraph::new();
    graph.add_node("generate", node);
    graph.set_entry_point("generate");
    graph.add_edge("generate", END);
    graph.compile()
}

fn build_embed_graph<E: Embeddings + 'static + ?Sized>(
    embeddings: Arc<E>,
) -> Result<CompiledGraph<EmbedState>> {
    let node = EmbedNode { embeddings };
    let mut graph: StateGraph<EmbedState> = StateGraph::new();
    graph.add_node("embed", node);
    graph.set_entry_point("embed");
    graph.add_edge("embed", END);
    graph.compile()
}

fn build_tool_graph<T: Tool + 'static>(tool: Arc<T>) -> Result<CompiledGraph<ToolCallState>> {
    let node = ToolNode { tool };
    let mut graph: StateGraph<ToolCallState> = StateGraph::new();
    graph.add_node("tool_call", node);
    graph.set_entry_point("tool_call");
    graph.add_edge("tool_call", END);
    graph.compile()
}

/// Generate a response from a chat model through the graph infrastructure.
///
/// This is the RECOMMENDED way to call LLMs. Accepts any type implementing [`AsLlmMessage`],
/// so you can pass your own message types without manual conversion.
///
/// # Example
///
/// ```ignore
/// use dashflow::core::messages::{AsLlmMessage, LlmMessageRef};
///
/// struct MyMessage {
///     role: String,
///     content: String,
/// }
///
/// impl AsLlmMessage for MyMessage {
///     fn as_llm_message(&self) -> LlmMessageRef<'_> {
///         LlmMessageRef {
///             role: &self.role,
///             content: &self.content,
///             tool_calls: None,
///             tool_call_id: None,
///             name: None,
///         }
///     }
/// }
///
/// // Use directly without conversion:
/// let messages = vec![MyMessage { role: "user".into(), content: "Hello!".into() }];
/// let result = dashflow::generate(model, &messages).await?;
/// ```
pub async fn generate<M, Msg>(model: Arc<M>, messages: &[Msg]) -> Result<ChatResult>
where
    M: ChatModel + 'static + ?Sized,
    Msg: AsLlmMessage,
{
    generate_with_options(model, messages, None, None, None).await
}

/// Generate a response with full options and automatic telemetry.
///
/// **NOTE:** As of DashFlow 1.12, this function uses the "batteries included"
/// API with automatic telemetry. No graph is created for single calls.
///
/// Accepts any type implementing [`AsLlmMessage`], so you can pass your own
/// message types without manual conversion.
///
/// Supports both concrete types (`Arc<OpenAIChatModel>`) and trait objects
/// (`Arc<dyn ChatModel>`) via the `?Sized` bound.
///
/// # Telemetry
///
/// All calls are automatically recorded to `~/.dashflow/learning.db` for
/// optimization and learning. Set `DASHFLOW_TELEMETRY_DISABLED=1` to disable.
pub async fn generate_with_options<M, Msg>(
    model: Arc<M>,
    messages: &[Msg],
    stop: Option<Vec<String>>,
    tools: Option<Vec<ToolDefinition>>,
    tool_choice: Option<ToolChoice>,
) -> Result<ChatResult>
where
    M: ChatModel + 'static + ?Sized,
    Msg: AsLlmMessage,
{
    // Convert messages using AsLlmMessage trait
    let base_messages: Vec<BaseMessage> = messages
        .iter()
        .map(|m| m.as_llm_message().to_base_message())
        .collect();

    // Start telemetry recording
    let record = crate::telemetry::llm_call()
        .model(model.model_name().unwrap_or("unknown"))
        .provider(model.llm_type())
        .messages(&base_messages)
        .start();

    // Direct call - no graph wrapping (batteries included telemetry)
    let result = model
        .generate(
            &base_messages,
            stop.as_deref(),
            tools.as_deref(),
            tool_choice.as_ref(),
            None,
        )
        .await;

    // Record telemetry
    match &result {
        Ok(response) => {
            let text = response
                .generations
                .first()
                .map(|g| g.message.content().as_text())
                .unwrap_or_default();
            record.success().response_text(&text).finish();
        }
        Err(e) => {
            record.error(e).finish();
        }
    }

    // Map core error to api error
    result.map_err(|e| Error::NodeExecution {
        node: "generate".to_string(),
        source: Box::new(e),
    })
}

/// Build a generate graph for streaming. Use graph.stream() on the returned graph.
pub fn build_generate_graph_for_streaming<M: ChatModel + 'static + ?Sized>(
    model: Arc<M>,
) -> Result<(CompiledGraph<GenerateState>, GenerateState)> {
    let graph = build_generate_graph(model)?;
    let state = GenerateState::default();
    Ok((graph, state))
}

/// Embed documents through the graph infrastructure.
/// This is the RECOMMENDED way to call embeddings.
pub async fn embed<E: Embeddings + 'static + ?Sized>(
    embeddings: Arc<E>,
    texts: &[String],
) -> Result<Vec<Vec<f32>>> {
    let graph = build_embed_graph(embeddings)?;
    let state = EmbedState {
        texts: texts.to_vec(),
        vectors: None,
    };
    let text_count = texts.len();
    let result = graph.invoke(state).await?;
    result
        .final_state
        .vectors
        .ok_or_else(|| {
            Error::Validation(format!(
                "Embedding completed but no vectors returned (input: {} text(s))",
                text_count
            ))
        })
}

/// Embed a single query through the graph infrastructure.
pub async fn embed_query<E: Embeddings + 'static + ?Sized>(
    embeddings: Arc<E>,
    text: &str,
) -> Result<Vec<f32>> {
    let vectors = embed(embeddings, &[text.to_string()]).await?;
    vectors
        .into_iter()
        .next()
        .ok_or_else(|| {
            Error::Validation(format!(
                "Embedding returned empty vectors (query length: {} chars)",
                text.len()
            ))
        })
}

/// Execute a tool through the graph infrastructure.
/// This is the RECOMMENDED way to call tools.
pub async fn call_tool<T: Tool + 'static>(tool: Arc<T>, input: &str) -> Result<String> {
    let tool_name = tool.name().to_string();
    let graph = build_tool_graph(tool)?;
    let state = ToolCallState {
        input: input.to_string(),
        result: None,
    };
    let result = graph.invoke(state).await?;
    result
        .final_state
        .result
        .ok_or_else(|| {
            Error::Validation(format!(
                "Tool '{}' execution completed but returned no result (input length: {} chars)",
                tool_name,
                input.len()
            ))
        })
}

/// Execute a tool with structured input through the graph infrastructure.
pub async fn call_tool_structured<T: Tool + 'static>(
    tool: Arc<T>,
    input: ToolInput,
) -> Result<String> {
    match input {
        ToolInput::String(s) => call_tool(tool, &s).await,
        ToolInput::Structured(v) => call_tool(tool, &v.to_string()).await,
    }
}

// ============================================================================
// Retriever Graph Infrastructure
// ============================================================================

/// State for retrieval operations through the graph infrastructure.
///
/// This state type is used internally by [`retrieve()`] to route retriever
/// calls through the graph system.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct RetrieveState {
    /// The query to search for
    #[serde(default)]
    pub query: String,

    /// Output: retrieved documents (populated by the node)
    #[serde(default)]
    pub documents: Option<Vec<Document>>,
}

impl crate::state::MergeableState for RetrieveState {
    fn merge(&mut self, other: &Self) {
        if other.documents.is_some() {
            self.documents = other.documents.clone();
        }
    }
}

struct RetrieveNode<R: Retriever + ?Sized> {
    retriever: Arc<R>,
}

#[async_trait]
impl<R: Retriever + 'static + ?Sized> Node<RetrieveState> for RetrieveNode<R> {
    async fn execute(&self, mut state: RetrieveState) -> Result<RetrieveState> {
        let documents = self
            .retriever
            ._get_relevant_documents(&state.query, None)
            .await
            .map_err(|e| Error::NodeExecution {
                node: "retrieve".to_string(),
                source: Box::new(e),
            })?;
        state.documents = Some(documents);
        Ok(state)
    }
    fn name(&self) -> String {
        "retrieve".to_string()
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

fn build_retrieve_graph<R: Retriever + 'static + ?Sized>(
    retriever: Arc<R>,
) -> Result<CompiledGraph<RetrieveState>> {
    let node = RetrieveNode { retriever };
    let mut graph: StateGraph<RetrieveState> = StateGraph::new();
    graph.add_node("retrieve", node);
    graph.set_entry_point("retrieve");
    graph.add_edge("retrieve", END);
    graph.compile()
}

/// Retrieve documents through the graph infrastructure.
/// This is the RECOMMENDED way to call retrievers.
pub async fn retrieve<R: Retriever + 'static + ?Sized>(
    retriever: Arc<R>,
    query: &str,
) -> Result<Vec<Document>> {
    let graph = build_retrieve_graph(retriever)?;
    let state = RetrieveState {
        query: query.to_string(),
        documents: None,
    };
    let result = graph.invoke(state).await?;
    result
        .final_state
        .documents
        .ok_or_else(|| {
            Error::Validation(format!(
                "Retrieval completed but no documents returned (query: '{}...')",
                query.chars().take(50).collect::<String>()
            ))
        })
}

// ============================================================================
// Vector Store Graph Infrastructure
// ============================================================================

/// State for vector search operations through the graph infrastructure.
///
/// This state type is used internally by [`vector_search()`] and
/// [`vector_search_with_filter()`] to route vector store calls through the graph system.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct VectorSearchState {
    /// The query to search for (will be embedded by the vector store)
    #[serde(default)]
    pub query: String,

    /// Number of similar documents to retrieve
    #[serde(default)]
    pub k: usize,

    /// Optional metadata filter for the search
    #[serde(default)]
    pub filter: Option<HashMap<String, serde_json::Value>>,

    /// Output: retrieved documents (populated by the node)
    #[serde(default)]
    pub documents: Option<Vec<Document>>,
}

impl crate::state::MergeableState for VectorSearchState {
    fn merge(&mut self, other: &Self) {
        if other.documents.is_some() {
            self.documents = other.documents.clone();
        }
    }
}

struct VectorSearchNode<V: VectorStore + ?Sized> {
    store: Arc<V>,
}

#[async_trait]
impl<V: VectorStore + 'static + ?Sized> Node<VectorSearchState> for VectorSearchNode<V> {
    async fn execute(&self, mut state: VectorSearchState) -> Result<VectorSearchState> {
        let documents = self
            .store
            ._similarity_search(&state.query, state.k, state.filter.as_ref())
            .await
            .map_err(|e| Error::NodeExecution {
                node: "vector_search".to_string(),
                source: Box::new(e),
            })?;
        state.documents = Some(documents);
        Ok(state)
    }
    fn name(&self) -> String {
        "vector_search".to_string()
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

fn build_vector_search_graph<V: VectorStore + 'static + ?Sized>(
    store: Arc<V>,
) -> Result<CompiledGraph<VectorSearchState>> {
    let node = VectorSearchNode { store };
    let mut graph: StateGraph<VectorSearchState> = StateGraph::new();
    graph.add_node("vector_search", node);
    graph.set_entry_point("vector_search");
    graph.add_edge("vector_search", END);
    graph.compile()
}

/// Perform vector similarity search through the graph infrastructure.
/// This is the RECOMMENDED way to call vector stores.
pub async fn vector_search<V: VectorStore + 'static + ?Sized>(
    store: Arc<V>,
    query: &str,
    k: usize,
) -> Result<Vec<Document>> {
    vector_search_with_filter(store, query, k, None).await
}

/// Perform vector similarity search with filter through the graph infrastructure.
pub async fn vector_search_with_filter<V: VectorStore + 'static + ?Sized>(
    store: Arc<V>,
    query: &str,
    k: usize,
    filter: Option<HashMap<String, serde_json::Value>>,
) -> Result<Vec<Document>> {
    let graph = build_vector_search_graph(store)?;
    let state = VectorSearchState {
        query: query.to_string(),
        k,
        filter: filter.clone(),
        documents: None,
    };
    let result = graph.invoke(state).await?;
    result
        .final_state
        .documents
        .ok_or_else(|| {
            Error::Validation(format!(
                "Vector search completed but no documents returned (query: '{}...', k={}, filter={})",
                query.chars().take(50).collect::<String>(),
                k,
                filter.is_some()
            ))
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::language_models::{ChatGeneration, ChatResult};
    use crate::core::messages::{AIMessage, HumanMessage};

    #[derive(Clone)]
    struct MockChatModel {
        response: String,
    }

    #[async_trait]
    impl ChatModel for MockChatModel {
        async fn _generate(
            &self,
            _messages: &[BaseMessage],
            _stop: Option<&[String]>,
            _tools: Option<&[ToolDefinition]>,
            _tool_choice: Option<&ToolChoice>,
            _run_manager: Option<&crate::core::callbacks::CallbackManager>,
        ) -> crate::core::error::Result<ChatResult> {
            Ok(ChatResult {
                generations: vec![ChatGeneration {
                    message: AIMessage::new(self.response.clone()).into(),
                    generation_info: None,
                }],
                llm_output: None,
            })
        }

        fn llm_type(&self) -> &str {
            "mock"
        }
        fn as_any(&self) -> &dyn std::any::Any {
            self
        }
    }

    #[derive(Clone)]
    struct MockEmbeddings;

    #[async_trait]
    impl Embeddings for MockEmbeddings {
        async fn _embed_documents(
            &self,
            texts: &[String],
        ) -> std::result::Result<Vec<Vec<f32>>, crate::core::error::Error> {
            Ok(texts.iter().map(|_| vec![0.1, 0.2, 0.3]).collect())
        }
        async fn _embed_query(
            &self,
            _text: &str,
        ) -> std::result::Result<Vec<f32>, crate::core::error::Error> {
            Ok(vec![0.1, 0.2, 0.3])
        }
    }

    #[derive(Clone)]
    struct MockTool;

    #[async_trait]
    impl Tool for MockTool {
        fn name(&self) -> &str {
            "mock_tool"
        }
        fn description(&self) -> &str {
            "A mock tool"
        }
        fn args_schema(&self) -> serde_json::Value {
            serde_json::json!({"type": "object"})
        }
        async fn _call(
            &self,
            input: ToolInput,
        ) -> std::result::Result<String, crate::core::error::Error> {
            match input {
                ToolInput::String(s) => Ok(format!("Mock result: {s}")),
                ToolInput::Structured(v) => Ok(format!("Mock result: {v}")),
            }
        }
    }

    #[tokio::test]
    async fn test_generate_uses_graph() {
        let model = Arc::new(MockChatModel {
            response: "Hello!".to_string(),
        });
        // HumanMessage implements AsLlmMessage directly, no need for .into()
        let messages = vec![HumanMessage::new("Hi")];
        let result = generate(model, &messages).await.unwrap();
        assert_eq!(result.generations.len(), 1);
    }

    #[tokio::test]
    async fn test_embed_uses_graph() {
        let embeddings = Arc::new(MockEmbeddings);
        let texts = vec!["test".to_string()];
        let vectors = embed(embeddings, &texts).await.unwrap();
        assert_eq!(vectors.len(), 1);
    }

    #[tokio::test]
    async fn test_call_tool_uses_graph() {
        let tool = Arc::new(MockTool);
        let result = call_tool(tool, "input").await.unwrap();
        assert!(result.contains("Mock result"));
    }

    // Test AsLlmMessage trait with custom message type
    use crate::core::messages::LlmMessageRef;

    struct CustomAppMessage {
        role: String,
        content: String,
    }

    impl AsLlmMessage for CustomAppMessage {
        fn as_llm_message(&self) -> LlmMessageRef<'_> {
            LlmMessageRef {
                role: &self.role,
                content: &self.content,
                tool_calls: None,
                tool_call_id: None,
                name: None,
            }
        }
    }

    #[tokio::test]
    async fn test_generate_with_custom_messages() {
        let model = Arc::new(MockChatModel {
            response: "Hello from custom!".to_string(),
        });
        let messages = vec![
            CustomAppMessage {
                role: "user".to_string(),
                content: "Hi there".to_string(),
            },
        ];
        let result = generate(model, &messages).await.unwrap();
        assert_eq!(result.generations.len(), 1);
        assert_eq!(
            result.generations[0].message.as_text(),
            "Hello from custom!"
        );
    }

    #[tokio::test]
    async fn test_generate_with_human_message_directly() {
        // Test that HumanMessage still works (backward compatibility)
        let model = Arc::new(MockChatModel {
            response: "Hi!".to_string(),
        });
        let messages = vec![HumanMessage::new("Hello")];
        let result = generate(model, &messages).await.unwrap();
        assert_eq!(result.generations.len(), 1);
    }

    #[tokio::test]
    async fn test_generate_with_ai_message() {
        // Test that AIMessage works
        let model = Arc::new(MockChatModel {
            response: "Following up!".to_string(),
        });
        let messages = vec![AIMessage::new("Previous response")];
        let result = generate(model, &messages).await.unwrap();
        assert_eq!(result.generations.len(), 1);
    }

    #[tokio::test]
    async fn test_generate_with_trait_object() {
        // Test that Arc<dyn ChatModel> works with ?Sized bounds
        let concrete = Arc::new(MockChatModel {
            response: "From trait object!".to_string(),
        });
        // Convert to trait object
        let model: Arc<dyn ChatModel> = concrete;
        let messages = vec![HumanMessage::new("Hello via trait object")];
        let result = generate(model, &messages).await.unwrap();
        assert_eq!(result.generations.len(), 1);
        assert_eq!(
            result.generations[0].message.as_text(),
            "From trait object!"
        );
    }
}
