//! Configuration types for loading DashFlow objects from files

use super::env_vars::{env_string_or_default, CHROMA_URL, OLLAMA_BASE_URL, QDRANT_URL};
use super::SecretReference;
use crate::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

// Import ChatModel trait for build() method
use crate::core::language_models::ChatModel;
// Import Embeddings trait for build() method
use crate::core::embeddings::Embeddings;
// Import DocumentCompressor trait for build() method
use crate::core::documents::DocumentCompressor;
// Import Tool trait for build() method
use crate::core::tools::Tool;

/// Top-level configuration container
///
/// This struct represents a complete DashFlow configuration file
/// that can contain chat models, embeddings, retrievers, and chains.
///
/// # Example
///
/// ```yaml
/// chat_models:
///   default:
///     type: openai
///     model: gpt-4
///     api_key:
///       env: OPENAI_API_KEY
///
/// embeddings:
///   default:
///     type: openai
///     model: text-embedding-3-small
///     api_key:
///       env: OPENAI_API_KEY
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DashFlowConfig {
    /// Named chat model configurations
    #[serde(default)]
    pub chat_models: HashMap<String, ChatModelConfig>,

    /// Named embedding configurations
    #[serde(default)]
    pub embeddings: HashMap<String, EmbeddingConfig>,

    /// Named retriever configurations
    #[serde(default)]
    pub retrievers: HashMap<String, RetrieverConfig>,

    /// Named vector store configurations
    #[serde(default)]
    pub vector_stores: HashMap<String, VectorStoreConfig>,

    /// Named chain/runnable configurations
    #[serde(default)]
    pub chains: HashMap<String, ChainConfig>,

    /// Named prompt template configurations
    #[serde(default)]
    pub prompts: HashMap<String, PromptConfig>,

    /// Named reranker configurations
    #[serde(default)]
    pub rerankers: HashMap<String, RerankerConfig>,

    /// Named tool configurations
    #[serde(default)]
    pub tools: HashMap<String, ToolConfig>,
}

// Note: The legacy alias `LangChainConfig` was removed in v1.12.0.
// Use `DashFlowConfig` directly.

impl DashFlowConfig {
    /// Load configuration from YAML string
    pub fn from_yaml(yaml: &str) -> crate::core::error::Result<Self> {
        serde_yml::from_str(yaml).map_err(|e| {
            crate::core::error::Error::Configuration(format!("Failed to parse YAML config: {e}"))
        })
    }

    /// Load configuration from JSON string
    pub fn from_json(json: &str) -> crate::core::error::Result<Self> {
        serde_json::from_str(json).map_err(|e| {
            crate::core::error::Error::Configuration(format!("Failed to parse JSON config: {e}"))
        })
    }

    /// Get a chat model config by name
    #[must_use]
    pub fn get_chat_model(&self, name: &str) -> Option<&ChatModelConfig> {
        self.chat_models.get(name)
    }

    /// Get an embedding config by name
    #[must_use]
    pub fn get_embedding(&self, name: &str) -> Option<&EmbeddingConfig> {
        self.embeddings.get(name)
    }

    /// Get a retriever config by name
    #[must_use]
    pub fn get_retriever(&self, name: &str) -> Option<&RetrieverConfig> {
        self.retrievers.get(name)
    }

    /// Get a vector store config by name
    #[must_use]
    pub fn get_vector_store(&self, name: &str) -> Option<&VectorStoreConfig> {
        self.vector_stores.get(name)
    }

    /// Get a chain config by name
    #[must_use]
    pub fn get_chain(&self, name: &str) -> Option<&ChainConfig> {
        self.chains.get(name)
    }

    /// Get a prompt config by name
    #[must_use]
    pub fn get_prompt(&self, name: &str) -> Option<&PromptConfig> {
        self.prompts.get(name)
    }

    /// Get a reranker config by name
    #[must_use]
    pub fn get_reranker(&self, name: &str) -> Option<&RerankerConfig> {
        self.rerankers.get(name)
    }

    /// Get a tool config by name
    #[must_use]
    pub fn get_tool(&self, name: &str) -> Option<&ToolConfig> {
        self.tools.get(name)
    }
}

/// Configuration for chat models
///
/// Supports all major LLM providers with provider-specific settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum ChatModelConfig {
    /// `OpenAI` chat models (GPT-4, GPT-3.5, etc.)
    OpenAI {
        /// Model name (e.g., "gpt-4", "gpt-3.5-turbo")
        #[serde(default = "default_openai_model")]
        model: String,

        /// API key (from environment or inline)
        api_key: SecretReference,

        /// Temperature (0.0 to 2.0)
        #[serde(default)]
        temperature: Option<f32>,

        /// Maximum tokens to generate
        #[serde(default)]
        max_tokens: Option<u32>,

        /// Custom base URL (for Azure or proxies)
        #[serde(default)]
        base_url: Option<String>,

        /// Organization ID
        #[serde(default)]
        organization: Option<String>,
    },

    /// Anthropic Claude models
    Anthropic {
        /// Model name (e.g., "claude-3-5-sonnet-20241022")
        #[serde(default = "default_anthropic_model")]
        model: String,

        /// API key (from environment or inline)
        api_key: SecretReference,

        /// Temperature (0.0 to 1.0)
        #[serde(default)]
        temperature: Option<f32>,

        /// Maximum tokens to generate
        #[serde(default)]
        max_tokens: Option<u32>,
    },

    /// Ollama local models
    Ollama {
        /// Model name (e.g., "llama3.2", "mistral")
        #[serde(default = "default_ollama_model")]
        model: String,

        /// Base URL for Ollama server
        #[serde(default = "default_ollama_base_url")]
        base_url: String,

        /// Temperature (0.0 to 1.0)
        #[serde(default)]
        temperature: Option<f32>,
    },

    /// Groq
    Groq {
        /// Model name
        #[serde(default = "default_groq_model")]
        model: String,

        /// API key
        api_key: SecretReference,

        /// Temperature
        #[serde(default)]
        temperature: Option<f32>,
    },

    /// Mistral AI
    Mistral {
        /// Model name
        #[serde(default = "default_mistral_model")]
        model: String,

        /// API key
        api_key: SecretReference,

        /// Temperature
        #[serde(default)]
        temperature: Option<f32>,
    },

    /// `DeepSeek`
    DeepSeek {
        /// Model name
        #[serde(default = "default_deepseek_model")]
        model: String,

        /// API key
        api_key: SecretReference,

        /// Temperature
        #[serde(default)]
        temperature: Option<f32>,
    },

    /// Fireworks AI
    Fireworks {
        /// Model name
        #[serde(default = "default_fireworks_model")]
        model: String,

        /// API key
        api_key: SecretReference,

        /// Temperature
        #[serde(default)]
        temperature: Option<f32>,
    },

    /// xAI (Grok)
    XAI {
        /// Model name
        #[serde(default = "default_xai_model")]
        model: String,

        /// API key
        api_key: SecretReference,

        /// Temperature
        #[serde(default)]
        temperature: Option<f32>,
    },

    /// Perplexity
    Perplexity {
        /// Model name
        #[serde(default = "default_perplexity_model")]
        model: String,

        /// API key
        api_key: SecretReference,

        /// Temperature
        #[serde(default)]
        temperature: Option<f32>,
    },

    /// `HuggingFace` Inference API
    HuggingFace {
        /// Model name or endpoint URL
        model: String,

        /// API key
        api_key: SecretReference,

        /// Temperature
        #[serde(default)]
        temperature: Option<f32>,
    },
}

impl ChatModelConfig {
    /// Get the model name from this configuration
    #[must_use]
    pub fn model(&self) -> &str {
        match self {
            ChatModelConfig::OpenAI { model, .. } => model,
            ChatModelConfig::Anthropic { model, .. } => model,
            ChatModelConfig::Ollama { model, .. } => model,
            ChatModelConfig::Groq { model, .. } => model,
            ChatModelConfig::Mistral { model, .. } => model,
            ChatModelConfig::DeepSeek { model, .. } => model,
            ChatModelConfig::Fireworks { model, .. } => model,
            ChatModelConfig::XAI { model, .. } => model,
            ChatModelConfig::Perplexity { model, .. } => model,
            ChatModelConfig::HuggingFace { model, .. } => model,
        }
    }

    /// Get the provider name for this configuration
    #[must_use]
    pub const fn provider(&self) -> &str {
        match self {
            ChatModelConfig::OpenAI { .. } => "openai",
            ChatModelConfig::Anthropic { .. } => "anthropic",
            ChatModelConfig::Ollama { .. } => "ollama",
            ChatModelConfig::Groq { .. } => "groq",
            ChatModelConfig::Mistral { .. } => "mistral",
            ChatModelConfig::DeepSeek { .. } => "deepseek",
            ChatModelConfig::Fireworks { .. } => "fireworks",
            ChatModelConfig::XAI { .. } => "xai",
            ChatModelConfig::Perplexity { .. } => "perplexity",
            ChatModelConfig::HuggingFace { .. } => "huggingface",
        }
    }
}

/// Trait for building chat models from configuration
///
/// Provider crates implement builder functions to construct ChatModel instances
/// from configuration. Due to Rust's orphan rules, this uses functions rather
/// than trait implementations.
///
/// # Usage
///
/// Import the provider's builder function:
///
/// ```rust,ignore
/// // In your Cargo.toml: dashflow-openai = "1.0"
/// use dashflow::core::config_loader::ChatModelConfig;
/// use dashflow_openai::build_chat_model;
///
/// let config: ChatModelConfig = serde_yaml::from_str(yaml)?;
/// let llm = build_chat_model(&config)?;
/// ```
///
/// # Provider-Agnostic Alternative
///
/// For applications that need to work with multiple providers, use `llm_factory`:
///
/// ```rust,ignore
/// use common::llm_factory::{create_llm, LLMRequirements};
///
/// let llm = create_llm(LLMRequirements::default()).await?;
/// ```
///
/// # Available Builder Functions
///
/// - `dashflow_openai::build_chat_model` - OpenAI (gpt-4o, gpt-3.5-turbo, etc.)
/// - `dashflow_anthropic::build_chat_model` - Anthropic (claude-3-5-sonnet, etc.)
/// - `dashflow_ollama::build_chat_model` - Ollama (local models)
/// - `dashflow_groq::build_chat_model` - Groq
/// - `dashflow_mistral::build_chat_model` - Mistral AI
/// - `dashflow_deepseek::build_chat_model` - DeepSeek
/// - `dashflow_fireworks::build_chat_model` - Fireworks AI
/// - `dashflow_xai::build_chat_model` - xAI (Grok)
/// - `dashflow_perplexity::build_chat_model` - Perplexity
/// - `dashflow_huggingface::build_chat_model` - HuggingFace
pub trait ChatModelConfigExt {
    /// Build a ChatModel instance from this configuration
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The configuration is for a different provider than this extension supports
    /// - Secret resolution fails (e.g., environment variable not set)
    /// - Provider-specific initialization fails
    fn build(&self) -> Result<Arc<dyn ChatModel>>;
}

/// Configuration for optimizable LLM nodes
///
/// This configuration combines a chat model provider configuration with a
/// signature (structured I/O) and optional optimization settings. Use this
/// to create `LLMNode` instances that can be optimized with DashOptimize
/// algorithms (BootstrapFewShot, MIPROv2, GRPO, etc.).
///
/// # Example YAML
///
/// ```yaml
/// nodes:
///   classifier:
///     provider:
///       type: openai
///       model: gpt-4o
///       api_key: !env OPENAI_API_KEY
///     signature:
///       spec: "text -> category"
///       instruction: "Classify text sentiment"
///     optimization:
///       trace_collection: true
///       strategy: bootstrap_fewshot
///       max_demos: 5
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LLMNodeConfig {
    /// The chat model provider configuration
    pub provider: ChatModelConfig,

    /// The signature defining structured inputs and outputs
    pub signature: SignatureConfig,

    /// Optional optimization configuration
    #[serde(default)]
    pub optimization: Option<OptimizationConfig>,
}

/// Configuration for a signature (structured I/O specification)
///
/// A signature defines the expected inputs and outputs of an LLM node,
/// enabling structured prompting and optimization.
///
/// # Example
///
/// ```yaml
/// signature:
///   spec: "question -> answer"
///   instruction: "Answer the question accurately and concisely"
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignatureConfig {
    /// Signature specification in DSPy format (e.g., "question -> answer")
    pub spec: String,

    /// Natural language instruction for the LLM
    pub instruction: String,
}

/// Configuration for optimization behavior
///
/// Controls how an LLM node collects data and optimizes its prompts.
///
/// # Example
///
/// ```yaml
/// optimization:
///   trace_collection: true
///   strategy: bootstrap_fewshot
///   max_demos: 5
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OptimizationConfig {
    /// Whether to collect traces for distillation
    #[serde(default)]
    pub trace_collection: bool,

    /// Optimization strategy (e.g., "bootstrap_fewshot", "miprov2", "grpo")
    #[serde(default)]
    pub strategy: Option<String>,

    /// Maximum number of few-shot demonstrations to include
    #[serde(default = "default_max_demos")]
    pub max_demos: usize,
}

const fn default_max_demos() -> usize {
    5
}

/// Configuration for embeddings
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum EmbeddingConfig {
    /// `OpenAI` embeddings
    OpenAI {
        /// Model name (e.g., "text-embedding-3-small")
        #[serde(default = "default_openai_embedding_model")]
        model: String,

        /// API key
        api_key: SecretReference,

        /// Batch size for embedding requests
        #[serde(default = "default_batch_size")]
        batch_size: usize,
    },

    /// Ollama embeddings
    Ollama {
        /// Model name
        model: String,

        /// Base URL
        #[serde(default = "default_ollama_base_url")]
        base_url: String,
    },

    /// `HuggingFace` embeddings
    HuggingFace {
        /// Model name
        model: String,

        /// API key
        api_key: SecretReference,
    },
}

impl EmbeddingConfig {
    /// Get the model name from this configuration
    #[must_use]
    pub fn model(&self) -> &str {
        match self {
            EmbeddingConfig::OpenAI { model, .. } => model,
            EmbeddingConfig::Ollama { model, .. } => model,
            EmbeddingConfig::HuggingFace { model, .. } => model,
        }
    }

    /// Get the provider name for this configuration
    #[must_use]
    pub const fn provider(&self) -> &str {
        match self {
            EmbeddingConfig::OpenAI { .. } => "openai",
            EmbeddingConfig::Ollama { .. } => "ollama",
            EmbeddingConfig::HuggingFace { .. } => "huggingface",
        }
    }
}

/// Extension trait for building Embeddings instances from EmbeddingConfig
///
/// This trait is implemented in provider crates to enable config-driven
/// embedding instantiation. Due to Rust's orphan rules, the implementation
/// cannot be in this crate.
///
/// # Usage
///
/// ```rust,ignore
/// // In your Cargo.toml: dashflow-openai = "1.0"
/// use dashflow::core::config_loader::EmbeddingConfig;
/// use dashflow_openai::build_embeddings;
///
/// let config: EmbeddingConfig = serde_yaml::from_str(yaml)?;
/// let embeddings = build_embeddings(&config)?;
/// ```
///
/// # Available provider implementations
///
/// - `dashflow_openai::build_embeddings` - OpenAI
/// - `dashflow_ollama::build_embeddings` - Ollama
/// - `dashflow_huggingface::build_embeddings` - HuggingFace
pub trait EmbeddingConfigExt {
    /// Build an Embeddings instance from this configuration
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The configuration is for a different provider than this extension supports
    /// - Secret resolution fails (e.g., environment variable not set)
    /// - Provider-specific initialization fails
    fn build(&self) -> Result<Arc<dyn Embeddings>>;
}

/// Configuration for document rerankers/compressors
///
/// Rerankers are used to reorder and filter retrieved documents based on
/// relevance to a query. They implement the `DocumentCompressor` trait.
///
/// # Example
///
/// ```yaml
/// rerankers:
///   default:
///     type: jina
///     model: jina-reranker-v1-base-en
///     api_key:
///       env: JINA_API_KEY
///     top_n: 3
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum RerankerConfig {
    /// Jina AI reranker
    Jina {
        /// Model name (e.g., "jina-reranker-v1-base-en")
        #[serde(default = "default_jina_reranker_model")]
        model: String,

        /// API key
        api_key: SecretReference,

        /// Number of top documents to return
        #[serde(default = "default_reranker_top_n")]
        top_n: Option<usize>,
    },
    // Future: Add Cohere, Voyage, etc.
}

impl RerankerConfig {
    /// Get the model name from this configuration
    #[must_use]
    pub fn model(&self) -> &str {
        match self {
            RerankerConfig::Jina { model, .. } => model,
        }
    }

    /// Get the provider name for this configuration
    #[must_use]
    pub const fn provider(&self) -> &str {
        match self {
            RerankerConfig::Jina { .. } => "jina",
        }
    }

    /// Get the top_n setting
    #[must_use]
    pub fn top_n(&self) -> Option<usize> {
        match self {
            RerankerConfig::Jina { top_n, .. } => *top_n,
        }
    }
}

/// Extension trait for building DocumentCompressor instances from RerankerConfig
///
/// This trait is implemented in provider crates to enable config-driven
/// reranker instantiation. Due to Rust's orphan rules, the implementation
/// cannot be in this crate.
///
/// # Usage
///
/// ```rust,ignore
/// // In your Cargo.toml: dashflow-jina = "1.0"
/// use dashflow::core::config_loader::RerankerConfig;
/// use dashflow_jina::build_reranker;
///
/// let config: RerankerConfig = serde_yaml::from_str(yaml)?;
/// let reranker = build_reranker(&config)?;
/// ```
///
/// # Available provider implementations
///
/// - `dashflow_jina::build_reranker` - Jina AI
pub trait RerankerConfigExt {
    /// Build a DocumentCompressor instance from this configuration
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The configuration is for a different provider than this extension supports
    /// - Secret resolution fails (e.g., environment variable not set)
    /// - Provider-specific initialization fails
    fn build(&self) -> Result<Arc<dyn DocumentCompressor>>;
}

/// Configuration for tools
///
/// Tools are callable functions that agents can use to interact with external
/// systems, search the web, access databases, etc.
///
/// # Example
///
/// ```yaml
/// tools:
///   web_search:
///     type: tavily
///     api_key:
///       env: TAVILY_API_KEY
///     max_results: 5
///     search_depth: advanced
///     include_answer: true
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum ToolConfig {
    /// Tavily AI-optimized web search tool
    Tavily {
        /// API key (from environment or inline)
        api_key: SecretReference,

        /// Maximum number of results (1-20)
        #[serde(default = "default_tavily_max_results")]
        max_results: u32,

        /// Search depth ("basic" or "advanced")
        #[serde(default = "default_tavily_search_depth")]
        search_depth: String,

        /// Topic category ("general", "news", or "finance")
        #[serde(default = "default_tavily_topic")]
        topic: String,

        /// Include LLM-generated answer
        #[serde(default)]
        include_answer: bool,

        /// Include image search results
        #[serde(default)]
        include_images: bool,

        /// Include raw HTML content
        #[serde(default)]
        include_raw_content: bool,
    },
    // Future: DuckDuckGo, Wikipedia, Calculator, etc.
}

impl ToolConfig {
    /// Get the tool type name for this configuration
    #[must_use]
    pub const fn tool_type(&self) -> &str {
        match self {
            ToolConfig::Tavily { .. } => "tavily",
        }
    }
}

fn default_tavily_max_results() -> u32 {
    5
}

fn default_tavily_search_depth() -> String {
    "basic".to_string()
}

fn default_tavily_topic() -> String {
    "general".to_string()
}

/// Extension trait for building Tool instances from ToolConfig
///
/// This trait is implemented in provider crates to enable config-driven
/// tool instantiation. Due to Rust's orphan rules, the implementation
/// cannot be in this crate.
///
/// # Usage
///
/// ```rust,ignore
/// // In your Cargo.toml: dashflow-tavily = "1.0"
/// use dashflow::core::config_loader::ToolConfig;
/// use dashflow_tavily::build_tool;
///
/// let config: ToolConfig = serde_yaml::from_str(yaml)?;
/// let tool = build_tool(&config)?;
/// ```
///
/// # Available provider implementations
///
/// - `dashflow_tavily::build_tool` - Tavily AI web search
pub trait ToolConfigExt {
    /// Build a Tool instance from this configuration
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The configuration is for a different tool type than this extension supports
    /// - Secret resolution fails (e.g., environment variable not set)
    /// - Tool-specific initialization fails
    fn build(&self) -> Result<Arc<dyn Tool>>;
}

/// Configuration for retrievers
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum RetrieverConfig {
    /// Vector store retriever
    VectorStore {
        /// Reference to a vector store config
        vector_store: String,

        /// Number of documents to retrieve
        #[serde(default = "default_k")]
        k: usize,

        /// Search type
        #[serde(default)]
        search_type: SearchType,
    },
}

/// Vector store search type
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum SearchType {
    /// Similarity search
    #[default]
    Similarity,

    /// Maximum marginal relevance
    Mmr,
}

/// Configuration for vector stores
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum VectorStoreConfig {
    /// Qdrant vector store
    Qdrant {
        /// Collection name
        collection_name: String,

        /// Qdrant server URL
        #[serde(default = "default_qdrant_url")]
        url: String,

        /// API key (optional, for Qdrant Cloud)
        #[serde(default)]
        api_key: Option<SecretReference>,

        /// Embedding configuration (reference or inline)
        embedding: Box<EmbeddingConfig>,
    },

    /// Chroma vector store
    Chroma {
        /// Collection name
        collection_name: String,

        /// Chroma server URL
        #[serde(default = "default_chroma_url")]
        url: String,

        /// Embedding configuration (reference or inline)
        embedding: Box<EmbeddingConfig>,
    },
}

impl VectorStoreConfig {
    /// Get the provider name for this config
    #[must_use]
    pub fn provider(&self) -> &'static str {
        match self {
            Self::Qdrant { .. } => "qdrant",
            Self::Chroma { .. } => "chroma",
        }
    }

    /// Get the collection name
    #[must_use]
    pub fn collection_name(&self) -> &str {
        match self {
            Self::Qdrant {
                collection_name, ..
            }
            | Self::Chroma {
                collection_name, ..
            } => collection_name,
        }
    }

    /// Get the URL
    #[must_use]
    pub fn url(&self) -> &str {
        match self {
            Self::Qdrant { url, .. } | Self::Chroma { url, .. } => url,
        }
    }

    /// Get the embedding config
    #[must_use]
    pub fn embedding(&self) -> &EmbeddingConfig {
        match self {
            Self::Qdrant { embedding, .. } | Self::Chroma { embedding, .. } => embedding,
        }
    }
}

// NOTE: VectorStoreConfigExt trait not possible because VectorStore trait
// is not dyn-compatible (add_texts has generic parameters).
// Use standalone build_vector_store() functions in provider crates instead:
// - dashflow_chroma::build_vector_store(&config) -> Result<ChromaVectorStore>
// - dashflow_qdrant::build_vector_store(&config) -> Result<QdrantVectorStore>

// Default value functions

fn default_openai_model() -> String {
    "gpt-4".to_string()
}

fn default_anthropic_model() -> String {
    "claude-3-5-sonnet-20241022".to_string()
}

fn default_ollama_model() -> String {
    "llama3.2".to_string()
}

fn default_ollama_base_url() -> String {
    env_string_or_default(OLLAMA_BASE_URL, "http://localhost:11434")
}

fn default_groq_model() -> String {
    "llama-3.3-70b-versatile".to_string()
}

fn default_mistral_model() -> String {
    "mistral-large-latest".to_string()
}

fn default_deepseek_model() -> String {
    "deepseek-chat".to_string()
}

fn default_fireworks_model() -> String {
    "accounts/fireworks/models/llama-v3p1-70b-instruct".to_string()
}

fn default_xai_model() -> String {
    "grok-beta".to_string()
}

fn default_perplexity_model() -> String {
    "llama-3.1-sonar-small-128k-online".to_string()
}

fn default_openai_embedding_model() -> String {
    "text-embedding-3-small".to_string()
}

const fn default_batch_size() -> usize {
    32
}

const fn default_k() -> usize {
    4
}

fn default_qdrant_url() -> String {
    env_string_or_default(QDRANT_URL, "http://localhost:6333")
}

fn default_chroma_url() -> String {
    env_string_or_default(CHROMA_URL, "http://localhost:8000")
}

fn default_jina_reranker_model() -> String {
    "jina-reranker-v1-base-en".to_string()
}

const fn default_reranker_top_n() -> Option<usize> {
    Some(3)
}

/// Configuration for prompt templates
///
/// Prompts can be defined inline or referenced by name.
///
/// # Example
///
/// ```yaml
/// prompts:
///   qa_prompt:
///     template: |
///       Context: {context}
///       Question: {question}
///       Answer:
///     input_variables:
///       - context
///       - question
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum PromptConfig {
    /// Simple string template
    Simple(String),

    /// Structured prompt with input variables
    Structured {
        /// The prompt template string
        template: String,

        /// Input variable names
        #[serde(default)]
        input_variables: Vec<String>,

        /// Optional system message prefix
        #[serde(default)]
        system_message: Option<String>,

        /// Template format (default: "f-string")
        #[serde(default)]
        template_format: Option<String>,
    },
}

impl PromptConfig {
    /// Get the template string
    #[must_use]
    pub fn template(&self) -> &str {
        match self {
            PromptConfig::Simple(template) => template,
            PromptConfig::Structured { template, .. } => template,
        }
    }

    /// Get input variables
    #[must_use]
    pub fn input_variables(&self) -> Vec<String> {
        match self {
            PromptConfig::Simple(_) => Vec::new(),
            PromptConfig::Structured {
                input_variables, ..
            } => input_variables.clone(),
        }
    }
}

/// Configuration for chains (composed runnables)
///
/// Chains define a sequence of operations to be executed.
///
/// # Example
///
/// ```yaml
/// chains:
///   qa_chain:
///     steps:
///       - type: retriever
///         ref: document_retriever
///       - type: prompt
///         ref: qa_prompt
///       - type: chat_model
///         ref: default
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChainConfig {
    /// Description of what this chain does
    #[serde(default)]
    pub description: Option<String>,

    /// Ordered list of steps in the chain
    pub steps: Vec<ChainStepConfig>,

    /// Optional configuration for parallel execution
    #[serde(default)]
    pub parallel: bool,

    /// Optional fallback behavior
    #[serde(default)]
    pub fallbacks: Vec<ChainStepConfig>,
}

/// A single step in a chain
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ChainStepConfig {
    /// Reference to a named chat model
    ChatModel {
        /// Reference name
        #[serde(rename = "ref")]
        reference: String,
    },

    /// Reference to a named retriever
    Retriever {
        /// Reference name
        #[serde(rename = "ref")]
        reference: String,
    },

    /// Reference to a named prompt
    Prompt {
        /// Reference name
        #[serde(rename = "ref")]
        reference: String,
    },

    /// Inline prompt template
    PromptTemplate {
        /// Template string
        template: String,

        /// Input variables
        #[serde(default)]
        input_variables: Vec<String>,
    },

    /// Lambda/transform step
    Lambda {
        /// Description of the transformation
        description: String,

        /// Optional input key mapping
        #[serde(default)]
        input_key: Option<String>,

        /// Optional output key mapping
        #[serde(default)]
        output_key: Option<String>,
    },

    /// Passthrough step (no transformation)
    Passthrough,

    /// Custom runnable (requires runtime implementation)
    Custom {
        /// Custom type identifier
        custom_type: String,

        /// Configuration parameters
        #[serde(default)]
        params: HashMap<String, serde_json::Value>,
    },
}

impl ChainStepConfig {
    /// Get the reference name if this step is a reference
    #[must_use]
    pub fn reference(&self) -> Option<&str> {
        match self {
            ChainStepConfig::ChatModel { reference }
            | ChainStepConfig::Retriever { reference }
            | ChainStepConfig::Prompt { reference } => Some(reference),
            _ => None,
        }
    }

    /// Get the step type name
    #[must_use]
    pub const fn step_type(&self) -> &str {
        match self {
            ChainStepConfig::ChatModel { .. } => "chat_model",
            ChainStepConfig::Retriever { .. } => "retriever",
            ChainStepConfig::Prompt { .. } => "prompt",
            ChainStepConfig::PromptTemplate { .. } => "prompt_template",
            ChainStepConfig::Lambda { .. } => "lambda",
            ChainStepConfig::Passthrough => "passthrough",
            ChainStepConfig::Custom { .. } => "custom",
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::test_prelude::*;

    #[test]
    fn test_parse_openai_config() {
        let yaml = r#"
chat_models:
  default:
    type: openai
    model: gpt-4
    temperature: 0.7
    api_key:
      env: OPENAI_API_KEY
"#;

        let config = DashFlowConfig::from_yaml(yaml).unwrap();
        let chat_model = config.get_chat_model("default").unwrap();

        match chat_model {
            ChatModelConfig::OpenAI {
                model, temperature, ..
            } => {
                assert_eq!(model, "gpt-4");
                assert_eq!(temperature.unwrap(), 0.7);
            }
            _ => panic!("Expected OpenAI config"),
        }
    }

    #[test]
    fn test_parse_anthropic_config() {
        let yaml = r#"
chat_models:
  claude:
    type: anthropic
    model: claude-3-5-sonnet-20241022
    api_key: sk-ant-test
    max_tokens: 1000
"#;

        let config = DashFlowConfig::from_yaml(yaml).unwrap();
        let chat_model = config.get_chat_model("claude").unwrap();

        match chat_model {
            ChatModelConfig::Anthropic {
                model, max_tokens, ..
            } => {
                assert_eq!(model, "claude-3-5-sonnet-20241022");
                assert_eq!(max_tokens.unwrap(), 1000);
            }
            _ => panic!("Expected Anthropic config"),
        }
    }

    #[test]
    fn test_parse_ollama_config() {
        let yaml = r#"
chat_models:
  local:
    type: ollama
    model: llama3.2
    base_url: http://localhost:11434
"#;

        let config = DashFlowConfig::from_yaml(yaml).unwrap();
        let chat_model = config.get_chat_model("local").unwrap();

        match chat_model {
            ChatModelConfig::Ollama {
                model, base_url, ..
            } => {
                assert_eq!(model, "llama3.2");
                assert_eq!(base_url, "http://localhost:11434");
            }
            _ => panic!("Expected Ollama config"),
        }
    }

    #[test]
    fn test_parse_embedding_config() {
        let yaml = r#"
embeddings:
  default:
    type: openai
    model: text-embedding-3-small
    api_key:
      env: OPENAI_API_KEY
    batch_size: 64
"#;

        let config = DashFlowConfig::from_yaml(yaml).unwrap();
        let embedding = config.get_embedding("default").unwrap();

        match embedding {
            EmbeddingConfig::OpenAI {
                model, batch_size, ..
            } => {
                assert_eq!(model, "text-embedding-3-small");
                assert_eq!(*batch_size, 64);
            }
            _ => panic!("Expected OpenAI embedding config"),
        }
    }

    #[test]
    fn test_parse_vector_store_config() {
        let yaml = r#"
vector_stores:
  docs:
    type: qdrant
    collection_name: documents
    url: http://localhost:6333
    embedding:
      type: openai
      model: text-embedding-3-small
      api_key:
        env: OPENAI_API_KEY
"#;

        let config = DashFlowConfig::from_yaml(yaml).unwrap();
        let vector_store = config.get_vector_store("docs").unwrap();

        match vector_store {
            VectorStoreConfig::Qdrant {
                collection_name,
                url,
                embedding,
                ..
            } => {
                assert_eq!(collection_name, "documents");
                assert_eq!(url, "http://localhost:6333");
                match **embedding {
                    EmbeddingConfig::OpenAI { ref model, .. } => {
                        assert_eq!(model, "text-embedding-3-small");
                    }
                    _ => panic!("Expected OpenAI embedding"),
                }
            }
            _ => panic!("Expected Qdrant config"),
        }
    }

    #[test]
    fn test_default_values() {
        let yaml = r#"
chat_models:
  default:
    type: openai
    api_key:
      env: OPENAI_API_KEY
"#;

        let config = DashFlowConfig::from_yaml(yaml).unwrap();
        let chat_model = config.get_chat_model("default").unwrap();

        match chat_model {
            ChatModelConfig::OpenAI { model, .. } => {
                assert_eq!(model, "gpt-4");
            }
            _ => panic!("Expected OpenAI config"),
        }
    }

    #[test]
    fn test_parse_json_config() {
        let json = r#"{
            "chat_models": {
                "default": {
                    "type": "openai",
                    "model": "gpt-4",
                    "api_key": {"env": "OPENAI_API_KEY"}
                }
            }
        }"#;

        let config = DashFlowConfig::from_json(json).unwrap();
        assert!(config.get_chat_model("default").is_some());
    }

    #[test]
    fn test_multiple_providers() {
        let yaml = r#"
chat_models:
  openai:
    type: openai
    model: gpt-4
    api_key:
      env: OPENAI_API_KEY

  anthropic:
    type: anthropic
    model: claude-3-5-sonnet-20241022
    api_key:
      env: ANTHROPIC_API_KEY

  ollama:
    type: ollama
    model: llama3.2
"#;

        let config = DashFlowConfig::from_yaml(yaml).unwrap();
        assert_eq!(config.chat_models.len(), 3);
        assert!(config.get_chat_model("openai").is_some());
        assert!(config.get_chat_model("anthropic").is_some());
        assert!(config.get_chat_model("ollama").is_some());
    }

    #[test]
    fn test_parse_prompt_config_simple() {
        let yaml = r#"
prompts:
  simple:
    "Context: {context}\nQuestion: {question}\nAnswer:"
"#;

        let config = DashFlowConfig::from_yaml(yaml).unwrap();
        let prompt = config.get_prompt("simple").unwrap();

        match prompt {
            PromptConfig::Simple(template) => {
                assert!(template.contains("Context"));
                assert!(template.contains("Question"));
            }
            _ => panic!("Expected simple prompt"),
        }
    }

    #[test]
    fn test_parse_prompt_config_structured() {
        let yaml = r#"
prompts:
  qa_prompt:
    template: |
      Context: {context}
      Question: {question}
      Answer:
    input_variables:
      - context
      - question
"#;

        let config = DashFlowConfig::from_yaml(yaml).unwrap();
        let prompt = config.get_prompt("qa_prompt").unwrap();

        match prompt {
            PromptConfig::Structured {
                template,
                input_variables,
                ..
            } => {
                assert!(template.contains("Context"));
                assert_eq!(input_variables.len(), 2);
                assert_eq!(input_variables[0], "context");
                assert_eq!(input_variables[1], "question");
            }
            _ => panic!("Expected structured prompt"),
        }
    }

    #[test]
    fn test_parse_chain_config() {
        let yaml = r#"
chains:
  qa_chain:
    description: "Question answering chain with retrieval"
    steps:
      - type: retriever
        ref: document_retriever
      - type: prompt
        ref: qa_prompt
      - type: chat_model
        ref: default
"#;

        let config = DashFlowConfig::from_yaml(yaml).unwrap();
        let chain = config.get_chain("qa_chain").unwrap();

        assert_eq!(
            chain.description.as_ref().unwrap(),
            "Question answering chain with retrieval"
        );
        assert_eq!(chain.steps.len(), 3);

        match &chain.steps[0] {
            ChainStepConfig::Retriever { reference } => {
                assert_eq!(reference, "document_retriever");
            }
            _ => panic!("Expected retriever step"),
        }

        match &chain.steps[1] {
            ChainStepConfig::Prompt { reference } => {
                assert_eq!(reference, "qa_prompt");
            }
            _ => panic!("Expected prompt step"),
        }

        match &chain.steps[2] {
            ChainStepConfig::ChatModel { reference } => {
                assert_eq!(reference, "default");
            }
            _ => panic!("Expected chat model step"),
        }
    }

    #[test]
    fn test_parse_chain_with_inline_prompt() {
        let yaml = r#"
chains:
  simple_chain:
    steps:
      - type: prompt_template
        template: "Say hello to {name}"
        input_variables:
          - name
      - type: chat_model
        ref: default
"#;

        let config = DashFlowConfig::from_yaml(yaml).unwrap();
        let chain = config.get_chain("simple_chain").unwrap();

        assert_eq!(chain.steps.len(), 2);

        match &chain.steps[0] {
            ChainStepConfig::PromptTemplate {
                template,
                input_variables,
            } => {
                assert_eq!(template, "Say hello to {name}");
                assert_eq!(input_variables.len(), 1);
                assert_eq!(input_variables[0], "name");
            }
            _ => panic!("Expected prompt_template step"),
        }
    }

    #[test]
    fn test_chain_step_reference() {
        let step = ChainStepConfig::ChatModel {
            reference: "my_model".to_string(),
        };
        assert_eq!(step.reference(), Some("my_model"));
        assert_eq!(step.step_type(), "chat_model");

        let step = ChainStepConfig::Passthrough;
        assert_eq!(step.reference(), None);
        assert_eq!(step.step_type(), "passthrough");
    }

    #[test]
    fn test_prompt_config_methods() {
        let simple = PromptConfig::Simple("Hello {name}".to_string());
        assert_eq!(simple.template(), "Hello {name}");
        assert_eq!(simple.input_variables().len(), 0);

        let structured = PromptConfig::Structured {
            template: "Hello {name}".to_string(),
            input_variables: vec!["name".to_string()],
            system_message: None,
            template_format: None,
        };
        assert_eq!(structured.template(), "Hello {name}");
        assert_eq!(structured.input_variables().len(), 1);
    }

    #[test]
    fn test_complete_config_with_chains() {
        let yaml = r#"
chat_models:
  default:
    type: openai
    model: gpt-4
    api_key:
      env: OPENAI_API_KEY

retrievers:
  docs:
    type: vectorstore
    vector_store: my_vectorstore
    k: 4

prompts:
  qa_prompt:
    template: "Context: {context}\n\nQuestion: {question}\n\nAnswer:"
    input_variables:
      - context
      - question

chains:
  qa_chain:
    steps:
      - type: retriever
        ref: docs
      - type: prompt
        ref: qa_prompt
      - type: chat_model
        ref: default
"#;

        let config = DashFlowConfig::from_yaml(yaml).unwrap();

        assert_eq!(config.chat_models.len(), 1);
        assert_eq!(config.retrievers.len(), 1);
        assert_eq!(config.prompts.len(), 1);
        assert_eq!(config.chains.len(), 1);

        let chain = config.get_chain("qa_chain").unwrap();
        assert_eq!(chain.steps.len(), 3);
    }
}
