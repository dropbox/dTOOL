// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

//! # Dependency Analysis
//!
//! Provides AI agents with detailed information about their dependency stack:
//! - What version of DashFlow am I using?
//! - What DashFlow crates am I using?
//! - What external crates am I using?
//! - Why do I depend on each crate?
//!
//! # Example
//!
//! ```rust,ignore
//! use dashflow::platform_registry::{AppArchitecture, DependencyAnalysis};
//!
//! let arch = app.analyze_architecture();
//! let deps = arch.dependency_analysis();
//!
//! // AI asks: "What version of DashFlow am I using?"
//! println!("DashFlow: v{}", deps.dashflow_version);
//!
//! // AI asks: "Why do I depend on tokio?"
//! if let Some(tokio) = deps.find_crate("tokio") {
//!     println!("Purpose: {}", tokio.purpose);
//! }
//! ```

use serde::{Deserialize, Serialize};

use super::AppArchitecture;

/// Dependency analysis result
///
/// Provides AI agents with detailed information about their dependency stack:
/// - What version of DashFlow am I using?
/// - What DashFlow crates am I using?
/// - What external crates am I using?
/// - Why do I depend on each crate?
///
/// # Example
///
/// ```rust,ignore
/// use dashflow::platform_registry::{AppArchitecture, DependencyAnalysis};
///
/// let arch = app.analyze_architecture();
/// let deps = arch.dependency_analysis();
///
/// // AI asks: "What version of DashFlow am I using?"
/// println!("DashFlow: v{}", deps.dashflow_version);
///
/// // AI asks: "Why do I depend on tokio?"
/// if let Some(tokio) = deps.find_crate("tokio") {
///     println!("Purpose: {}", tokio.purpose);
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DependencyAnalysis {
    /// DashFlow version being used
    pub dashflow_version: String,
    /// DashFlow crates used by this application
    pub dashflow_crates: Vec<CrateDependency>,
    /// External (non-DashFlow) crates used
    pub external_crates: Vec<CrateDependency>,
    /// Analysis metadata
    pub metadata: DependencyMetadata,
}

impl DependencyAnalysis {
    /// Create a new dependency analysis builder
    #[must_use]
    pub fn builder() -> DependencyAnalysisBuilder {
        DependencyAnalysisBuilder::new()
    }

    /// Convert analysis to JSON string for AI consumption
    ///
    /// # Errors
    ///
    /// Returns error if serialization fails
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    /// Get a summary of the dependency analysis
    #[must_use]
    pub fn summary(&self) -> String {
        let dashflow_count = self.dashflow_crates.len();
        let external_count = self.external_crates.len();
        let total = dashflow_count + external_count;
        format!(
            "Dependencies: {} total ({} DashFlow, {} external), DashFlow v{}",
            total, dashflow_count, external_count, self.dashflow_version
        )
    }

    /// Find a crate by name (searches both DashFlow and external)
    #[must_use]
    pub fn find_crate(&self, name: &str) -> Option<&CrateDependency> {
        self.dashflow_crates
            .iter()
            .chain(self.external_crates.iter())
            .find(|c| c.name.eq_ignore_ascii_case(name))
    }

    /// Get all crates of a specific category
    #[must_use]
    pub fn crates_by_category(&self, category: DependencyCategory) -> Vec<&CrateDependency> {
        self.dashflow_crates
            .iter()
            .chain(self.external_crates.iter())
            .filter(|c| c.category == category)
            .collect()
    }

    /// Get all LLM provider crates
    #[must_use]
    pub fn llm_provider_crates(&self) -> Vec<&CrateDependency> {
        self.crates_by_category(DependencyCategory::LlmProvider)
    }

    /// Get all vector store crates
    #[must_use]
    pub fn vector_store_crates(&self) -> Vec<&CrateDependency> {
        self.crates_by_category(DependencyCategory::VectorStore)
    }

    /// Get all tool crates
    #[must_use]
    pub fn tool_crates(&self) -> Vec<&CrateDependency> {
        self.crates_by_category(DependencyCategory::Tool)
    }

    /// Get all checkpoint backend crates
    #[must_use]
    pub fn checkpoint_crates(&self) -> Vec<&CrateDependency> {
        self.crates_by_category(DependencyCategory::Checkpointer)
    }

    /// Get all async runtime crates
    #[must_use]
    pub fn runtime_crates(&self) -> Vec<&CrateDependency> {
        self.crates_by_category(DependencyCategory::Runtime)
    }

    /// Check if a crate is used
    #[must_use]
    pub fn uses_crate(&self, name: &str) -> bool {
        self.find_crate(name).is_some()
    }

    /// Get total number of crates
    #[must_use]
    pub fn total_crates(&self) -> usize {
        self.dashflow_crates.len() + self.external_crates.len()
    }

    /// Get crates that use a specific API
    #[must_use]
    pub fn crates_using_api(&self, api: &str) -> Vec<&CrateDependency> {
        let api_lower = api.to_lowercase();
        self.dashflow_crates
            .iter()
            .chain(self.external_crates.iter())
            .filter(|c| {
                c.apis_used
                    .iter()
                    .any(|a| a.to_lowercase().contains(&api_lower))
            })
            .collect()
    }

    /// Get a list of all API names used across all dependencies
    #[must_use]
    pub fn all_apis_used(&self) -> Vec<&str> {
        self.dashflow_crates
            .iter()
            .chain(self.external_crates.iter())
            .flat_map(|c| c.apis_used.iter().map(String::as_str))
            .collect()
    }
}

/// Builder for DependencyAnalysis
#[derive(Debug, Default)]
pub struct DependencyAnalysisBuilder {
    dashflow_version: Option<String>,
    dashflow_crates: Vec<CrateDependency>,
    external_crates: Vec<CrateDependency>,
    metadata: Option<DependencyMetadata>,
}

impl DependencyAnalysisBuilder {
    /// Create a new builder
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the DashFlow version
    #[must_use]
    pub fn dashflow_version(mut self, version: impl Into<String>) -> Self {
        self.dashflow_version = Some(version.into());
        self
    }

    /// Add a DashFlow crate dependency
    pub fn add_dashflow_crate(&mut self, crate_dep: CrateDependency) -> &mut Self {
        self.dashflow_crates.push(crate_dep);
        self
    }

    /// Add an external crate dependency
    pub fn add_external_crate(&mut self, crate_dep: CrateDependency) -> &mut Self {
        self.external_crates.push(crate_dep);
        self
    }

    /// Set metadata
    #[must_use]
    pub fn metadata(mut self, metadata: DependencyMetadata) -> Self {
        self.metadata = Some(metadata);
        self
    }

    /// Build the dependency analysis
    #[must_use]
    pub fn build(self) -> DependencyAnalysis {
        DependencyAnalysis {
            dashflow_version: self
                .dashflow_version
                .unwrap_or_else(|| env!("CARGO_PKG_VERSION").to_string()),
            dashflow_crates: self.dashflow_crates,
            external_crates: self.external_crates,
            metadata: self.metadata.unwrap_or_default(),
        }
    }
}

/// Crate dependency information
///
/// Describes a crate dependency with its purpose and usage.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrateDependency {
    /// Crate name
    pub name: String,
    /// Version (if known)
    pub version: Option<String>,
    /// Purpose/description of why this crate is used
    pub purpose: String,
    /// APIs used from this crate
    pub apis_used: Vec<String>,
    /// Dependency category
    pub category: DependencyCategory,
    /// Whether this is a direct or transitive dependency
    pub is_direct: bool,
    /// Features enabled for this crate
    pub features: Vec<String>,
    /// Whether this is an optional dependency
    pub optional: bool,
}

impl CrateDependency {
    /// Create a new crate dependency
    #[must_use]
    pub fn new(name: impl Into<String>, purpose: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            version: None,
            purpose: purpose.into(),
            apis_used: Vec::new(),
            category: DependencyCategory::Other,
            is_direct: true,
            features: Vec::new(),
            optional: false,
        }
    }

    /// Set the version
    #[must_use]
    pub fn with_version(mut self, version: impl Into<String>) -> Self {
        self.version = Some(version.into());
        self
    }

    /// Add an API used from this crate
    #[must_use]
    pub fn with_api(mut self, api: impl Into<String>) -> Self {
        self.apis_used.push(api.into());
        self
    }

    /// Add multiple APIs
    #[must_use]
    pub fn with_apis(mut self, apis: Vec<impl Into<String>>) -> Self {
        self.apis_used.extend(apis.into_iter().map(Into::into));
        self
    }

    /// Set the category
    #[must_use]
    pub fn with_category(mut self, category: DependencyCategory) -> Self {
        self.category = category;
        self
    }

    /// Mark as transitive (not direct) dependency
    #[must_use]
    pub fn transitive(mut self) -> Self {
        self.is_direct = false;
        self
    }

    /// Add enabled features
    #[must_use]
    pub fn with_features(mut self, features: Vec<impl Into<String>>) -> Self {
        self.features.extend(features.into_iter().map(Into::into));
        self
    }

    /// Mark as optional dependency
    #[must_use]
    pub fn optional(mut self) -> Self {
        self.optional = true;
        self
    }

    /// Check if this crate uses a specific API
    #[must_use]
    pub fn uses_api(&self, api: &str) -> bool {
        let api_lower = api.to_lowercase();
        self.apis_used
            .iter()
            .any(|a| a.to_lowercase().contains(&api_lower))
    }
}

/// Dependency category
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DependencyCategory {
    /// Core DashFlow functionality
    Core,
    /// LLM provider integration
    LlmProvider,
    /// Vector store integration
    VectorStore,
    /// Agent tool
    Tool,
    /// Checkpoint backend
    Checkpointer,
    /// Embedding provider
    Embedding,
    /// Async runtime
    Runtime,
    /// Serialization
    Serialization,
    /// HTTP/Networking
    Networking,
    /// Logging/Tracing
    Observability,
    /// Testing
    Testing,
    /// Other
    #[default]
    Other,
}

impl std::fmt::Display for DependencyCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Core => write!(f, "Core"),
            Self::LlmProvider => write!(f, "LLM Provider"),
            Self::VectorStore => write!(f, "Vector Store"),
            Self::Tool => write!(f, "Tool"),
            Self::Checkpointer => write!(f, "Checkpointer"),
            Self::Embedding => write!(f, "Embedding"),
            Self::Runtime => write!(f, "Runtime"),
            Self::Serialization => write!(f, "Serialization"),
            Self::Networking => write!(f, "Networking"),
            Self::Observability => write!(f, "Observability"),
            Self::Testing => write!(f, "Testing"),
            Self::Other => write!(f, "Other"),
        }
    }
}

/// Dependency analysis metadata
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DependencyMetadata {
    /// Source of the analysis (e.g., "Cargo.toml", "runtime")
    pub source: Option<String>,
    /// When the analysis was performed
    pub analyzed_at: Option<String>,
    /// Cargo.toml path if parsed from file
    pub cargo_toml_path: Option<String>,
    /// Notes about the analysis
    pub notes: Vec<String>,
}

impl DependencyMetadata {
    /// Create new metadata
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the source
    #[must_use]
    pub fn with_source(mut self, source: impl Into<String>) -> Self {
        self.source = Some(source.into());
        self
    }

    /// Set the Cargo.toml path
    #[must_use]
    pub fn with_cargo_path(mut self, path: impl Into<String>) -> Self {
        self.cargo_toml_path = Some(path.into());
        self
    }

    /// Add a note
    #[must_use]
    pub fn with_note(mut self, note: impl Into<String>) -> Self {
        self.notes.push(note.into());
        self
    }
}

impl AppArchitecture {
    /// Perform dependency analysis on this architecture
    ///
    /// Analyzes the dependencies used by the application, categorizing them
    /// into DashFlow crates and external crates, and providing purpose descriptions.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let arch = app.analyze_architecture();
    /// let deps = arch.dependency_analysis();
    ///
    /// println!("DashFlow version: {}", deps.dashflow_version);
    ///
    /// for crate_dep in &deps.dashflow_crates {
    ///     println!("{}: {}", crate_dep.name, crate_dep.purpose);
    /// }
    /// ```
    #[must_use]
    pub fn dependency_analysis(&self) -> DependencyAnalysis {
        let mut builder = DependencyAnalysisBuilder::new()
            .dashflow_version(self.metadata.dashflow_version.clone())
            .metadata(
                DependencyMetadata::new()
                    .with_source("AppArchitecture")
                    .with_note("Analysis derived from architecture dependency list"),
            );

        // Categorize dependencies from the architecture
        for dep in &self.dependencies {
            let category = Self::infer_category(&dep.name);
            let crate_dep = CrateDependency::new(&dep.name, &dep.purpose)
                .with_version(dep.version.clone().unwrap_or_default())
                .with_apis(dep.apis_used.clone())
                .with_category(category);

            if dep.is_dashflow {
                builder.add_dashflow_crate(crate_dep);
            } else {
                builder.add_external_crate(crate_dep);
            }
        }

        builder.build()
    }

    /// Infer the dependency category from the crate name
    pub(crate) fn infer_category(name: &str) -> DependencyCategory {
        let name_lower = name.to_lowercase();

        // DashFlow categories
        if name_lower.contains("openai")
            || name_lower.contains("anthropic")
            || name_lower.contains("bedrock")
            || name_lower.contains("gemini")
            || name_lower.contains("ollama")
            || name_lower.contains("cohere")
            || name_lower.contains("mistral")
            || name_lower.contains("groq")
            || name_lower.contains("fireworks")
            || name_lower.contains("together")
            || name_lower.contains("replicate")
            || name_lower.contains("xai")
            || name_lower.contains("deepseek")
            || name_lower.contains("perplexity")
        {
            return DependencyCategory::LlmProvider;
        }

        if name_lower.contains("chroma")
            || name_lower.contains("pinecone")
            || name_lower.contains("qdrant")
            || name_lower.contains("pgvector")
            || name_lower.contains("milvus")
            || name_lower.contains("weaviate")
            || name_lower.contains("elasticsearch")
            || name_lower.contains("opensearch")
            || name_lower.contains("redis") && name_lower.contains("vector")
            || name_lower.contains("faiss")
            || name_lower.contains("lancedb")
            || name_lower.contains("typesense")
        {
            return DependencyCategory::VectorStore;
        }

        if name_lower.contains("-tool")
            || name_lower.contains("shell")
            || name_lower.contains("calculator")
            || name_lower.contains("webscrape")
            || name_lower.contains("playwright")
        {
            return DependencyCategory::Tool;
        }

        if name_lower.contains("checkpointer") || name_lower.contains("checkpoint") {
            return DependencyCategory::Checkpointer;
        }

        if name_lower.contains("embedding")
            || name_lower.contains("voyage")
            || name_lower.contains("jina")
            || name_lower.contains("nomic")
        {
            return DependencyCategory::Embedding;
        }

        // External categories
        if name_lower == "tokio" || name_lower == "async-std" || name_lower == "smol" {
            return DependencyCategory::Runtime;
        }

        if name_lower == "serde"
            || name_lower == "serde_json"
            || name_lower == "bincode"
            || name_lower == "rmp-serde"
        {
            return DependencyCategory::Serialization;
        }

        if name_lower == "reqwest"
            || name_lower == "hyper"
            || name_lower == "axum"
            || name_lower == "actix-web"
            || name_lower == "tonic"
        {
            return DependencyCategory::Networking;
        }

        if name_lower == "tracing"
            || name_lower == "log"
            || name_lower == "env_logger"
            || name_lower.contains("prometheus")
        {
            return DependencyCategory::Observability;
        }

        if name_lower.contains("test") || name_lower == "mockall" || name_lower == "proptest" {
            return DependencyCategory::Testing;
        }

        if name_lower == "dashflow" || name_lower.starts_with("dashflow-") {
            return DependencyCategory::Core;
        }

        DependencyCategory::Other
    }
}

/// Parse dependencies from a Cargo.toml file content
///
/// This is a utility function for parsing Cargo.toml files to extract
/// dependency information.
///
/// # Arguments
///
/// * `toml_content` - The content of a Cargo.toml file
///
/// # Returns
///
/// A `DependencyAnalysis` populated with the parsed dependencies.
///
/// # Example
///
/// ```rust,ignore
/// use dashflow::platform_registry::parse_cargo_toml;
///
/// let toml = r#"
/// [dependencies]
/// dashflow = "1.11.2"
/// dashflow-openai = "1.11.2"
/// tokio = { version = "1.35", features = ["full"] }
/// "#;
///
/// let analysis = parse_cargo_toml(toml);
/// println!("Found {} dependencies", analysis.total_crates());
/// ```
#[must_use]
pub fn parse_cargo_toml(toml_content: &str) -> DependencyAnalysis {
    let mut builder = DependencyAnalysisBuilder::new()
        .metadata(DependencyMetadata::new().with_source("Cargo.toml"));

    // Simple TOML parsing for dependencies section
    let mut in_dependencies = false;
    let mut in_dev_dependencies = false;

    for line in toml_content.lines() {
        let line = line.trim();

        // Track which section we're in
        if line.starts_with('[') {
            in_dependencies = line == "[dependencies]";
            in_dev_dependencies = line == "[dev-dependencies]";
            continue;
        }

        // Parse dependency lines
        if (in_dependencies || in_dev_dependencies) && !line.is_empty() && !line.starts_with('#') {
            if let Some((name, rest)) = line.split_once('=') {
                let name = name.trim().trim_matches('"');
                let version = extract_version(rest.trim());
                let features = extract_features(rest.trim());

                let is_dashflow = name == "dashflow" || name.starts_with("dashflow-");
                let category = AppArchitecture::infer_category(name);
                let purpose = infer_purpose(name);

                let mut crate_dep = CrateDependency::new(name, purpose).with_category(category);

                if let Some(v) = version {
                    crate_dep = crate_dep.with_version(v);
                }

                if !features.is_empty() {
                    crate_dep = crate_dep.with_features(features);
                }

                if in_dev_dependencies {
                    crate_dep = crate_dep.optional();
                }

                if is_dashflow {
                    // Extract DashFlow version from the core crate
                    if name == "dashflow" {
                        if let Some(v) = &crate_dep.version {
                            builder = builder.dashflow_version(v.clone());
                        }
                    }
                    builder.add_dashflow_crate(crate_dep);
                } else {
                    builder.add_external_crate(crate_dep);
                }
            }
        }
    }

    builder.build()
}

/// Extract version from a TOML dependency value
pub(crate) fn extract_version(value: &str) -> Option<String> {
    let value = value.trim();

    // Simple string version: "1.0.0"
    if value.starts_with('"') && value.ends_with('"') {
        return Some(value.trim_matches('"').to_string());
    }

    // Table format: { version = "1.0.0", ... }
    if value.starts_with('{') {
        // Look for version = "..."
        for part in value.split(',') {
            let part = part.trim().trim_matches(|c| c == '{' || c == '}');
            if let Some((key, val)) = part.split_once('=') {
                if key.trim() == "version" {
                    return Some(val.trim().trim_matches('"').to_string());
                }
            }
        }
    }

    None
}

/// Extract features from a TOML dependency value
pub(crate) fn extract_features(value: &str) -> Vec<String> {
    let value = value.trim();

    // Table format: { version = "1.0.0", features = ["a", "b"] }
    if value.contains("features") {
        // Find the features array
        if let Some(start) = value.find('[') {
            if let Some(end) = value[start..].find(']') {
                let features_str = &value[start + 1..start + end];
                return features_str
                    .split(',')
                    .map(|s| s.trim().trim_matches('"').to_string())
                    .filter(|s| !s.is_empty())
                    .collect();
            }
        }
    }

    Vec::new()
}

/// Infer the purpose of a crate from its name
pub(crate) fn infer_purpose(name: &str) -> String {
    let name_lower = name.to_lowercase();

    // DashFlow crates
    if name_lower == "dashflow" {
        return "Core graph orchestration framework".to_string();
    }
    if let Some(suffix) = name_lower.strip_prefix("dashflow-") {
        return match suffix {
            "openai" => "OpenAI LLM provider (GPT-4, GPT-3.5)".to_string(),
            "anthropic" => "Anthropic LLM provider (Claude)".to_string(),
            "bedrock" => "AWS Bedrock LLM provider".to_string(),
            "gemini" => "Google Gemini LLM provider".to_string(),
            "ollama" => "Ollama local LLM provider".to_string(),
            "cohere" => "Cohere LLM and embedding provider".to_string(),
            "mistral" => "Mistral LLM provider".to_string(),
            "groq" => "Groq LLM provider".to_string(),
            "together" => "Together AI LLM provider".to_string(),
            "fireworks" => "Fireworks AI LLM provider".to_string(),
            "replicate" => "Replicate model provider".to_string(),
            "xai" => "xAI (Grok) LLM provider".to_string(),
            "deepseek" => "DeepSeek LLM provider".to_string(),
            "perplexity" => "Perplexity LLM provider".to_string(),
            "chroma" => "Chroma vector store".to_string(),
            "pinecone" => "Pinecone vector store".to_string(),
            "qdrant" => "Qdrant vector store".to_string(),
            "pgvector" => "PostgreSQL pgvector integration".to_string(),
            "milvus" => "Milvus vector store".to_string(),
            "weaviate" => "Weaviate vector store".to_string(),
            "elasticsearch" => "Elasticsearch vector store".to_string(),
            "opensearch" => "OpenSearch vector store".to_string(),
            "faiss" => "FAISS vector similarity search".to_string(),
            "lancedb" => "LanceDB vector store".to_string(),
            "typesense" => "Typesense search engine".to_string(),
            "redis" => "Redis integration".to_string(),
            "redis-checkpointer" => "Redis checkpoint backend".to_string(),
            "postgres-checkpointer" => "PostgreSQL checkpoint backend".to_string(),
            "s3-checkpointer" => "S3 checkpoint backend".to_string(),
            "dynamodb-checkpointer" => "DynamoDB checkpoint backend".to_string(),
            "shell-tool" => "Safe shell command execution".to_string(),
            "file-tool" => "File system operations".to_string(),
            "git-tool" => "Git repository operations".to_string(),
            "http-requests" => "HTTP request tool".to_string(),
            "calculator" => "Calculator tool".to_string(),
            "webscrape" => "Web scraping tool".to_string(),
            "playwright" => "Browser automation tool".to_string(),
            "human-tool" => "Human-in-the-loop tool".to_string(),
            "json-tool" => "JSON manipulation tool".to_string(),
            "streaming" => "Real-time execution streaming".to_string(),
            "context" => "Context and token management".to_string(),
            "memory" => "Conversation memory".to_string(),
            "chains" => "LLM chain compositions".to_string(),
            "evals" => "Evaluation framework".to_string(),
            "langsmith" => "LangSmith integration".to_string(),
            "langserve" => "LangServe deployment".to_string(),
            "observability" => "Observability and monitoring".to_string(),
            "testing" => "Testing utilities".to_string(),
            "derive" => "Derive macros".to_string(),
            "macros" => "Procedural macros".to_string(),
            "voyage" => "Voyage AI embedding provider".to_string(),
            "jina" => "Jina embedding provider".to_string(),
            "nomic" => "Nomic embedding provider".to_string(),
            "huggingface" => "HuggingFace integration".to_string(),
            _ => format!("DashFlow {} integration", suffix),
        };
    }

    // Common external crates
    match name_lower.as_str() {
        "tokio" => "Async runtime for Rust".to_string(),
        "async-std" => "Async standard library".to_string(),
        "futures" => "Async futures utilities".to_string(),
        "serde" => "Serialization/deserialization framework".to_string(),
        "serde_json" => "JSON serialization".to_string(),
        "reqwest" => "HTTP client".to_string(),
        "hyper" => "HTTP implementation".to_string(),
        "axum" => "Web framework".to_string(),
        "actix-web" => "Web framework".to_string(),
        "tonic" => "gRPC framework".to_string(),
        "tracing" => "Application-level tracing".to_string(),
        "log" => "Logging facade".to_string(),
        "env_logger" => "Environment-based logger".to_string(),
        "anyhow" => "Error handling".to_string(),
        "thiserror" => "Error derive macros".to_string(),
        "clap" => "Command-line argument parsing".to_string(),
        "regex" => "Regular expressions".to_string(),
        "chrono" => "Date and time library".to_string(),
        "uuid" => "UUID generation".to_string(),
        "rand" => "Random number generation".to_string(),
        "base64" => "Base64 encoding/decoding".to_string(),
        "sha2" => "SHA-2 hash functions".to_string(),
        "sqlx" => "Async SQL toolkit".to_string(),
        "diesel" => "ORM and query builder".to_string(),
        "redis" => "Redis client".to_string(),
        "deadpool" => "Connection pooling".to_string(),
        "bb8" => "Connection pooling".to_string(),
        "parking_lot" => "Synchronization primitives".to_string(),
        "dashmap" => "Concurrent hash map".to_string(),
        "crossbeam" => "Concurrent programming tools".to_string(),
        "rayon" => "Parallel iteration".to_string(),
        "bytes" => "Byte buffer utilities".to_string(),
        "url" => "URL parsing".to_string(),
        "once_cell" => "Lazy initialization".to_string(),
        "lazy_static" => "Lazy statics".to_string(),
        "async-trait" => "Async trait support".to_string(),
        "pin-project" => "Pin projection".to_string(),
        "tower" => "Service abstraction".to_string(),
        "tower-http" => "HTTP service utilities".to_string(),
        _ => format!("{} dependency", name),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // =============================================================================
    // DependencyAnalysis Tests
    // =============================================================================

    #[test]
    fn test_dependency_analysis_builder_basic() {
        let analysis = DependencyAnalysisBuilder::new()
            .dashflow_version("1.11.0")
            .build();
        assert_eq!(analysis.dashflow_version, "1.11.0");
        assert!(analysis.dashflow_crates.is_empty());
        assert!(analysis.external_crates.is_empty());
    }

    #[test]
    fn test_dependency_analysis_builder_default_version() {
        let analysis = DependencyAnalysisBuilder::new().build();
        // Should use CARGO_PKG_VERSION if not specified
        assert!(!analysis.dashflow_version.is_empty());
    }

    #[test]
    fn test_dependency_analysis_builder_add_dashflow_crate() {
        let mut builder = DependencyAnalysisBuilder::new();
        builder.add_dashflow_crate(CrateDependency::new("dashflow-openai", "OpenAI provider"));
        let analysis = builder.build();
        assert_eq!(analysis.dashflow_crates.len(), 1);
        assert_eq!(analysis.dashflow_crates[0].name, "dashflow-openai");
    }

    #[test]
    fn test_dependency_analysis_builder_add_external_crate() {
        let mut builder = DependencyAnalysisBuilder::new();
        builder.add_external_crate(CrateDependency::new("tokio", "Async runtime"));
        let analysis = builder.build();
        assert_eq!(analysis.external_crates.len(), 1);
        assert_eq!(analysis.external_crates[0].name, "tokio");
    }

    #[test]
    fn test_dependency_analysis_builder_with_metadata() {
        let metadata = DependencyMetadata::new()
            .with_source("test")
            .with_note("test note");
        let analysis = DependencyAnalysisBuilder::new()
            .metadata(metadata)
            .build();
        assert_eq!(analysis.metadata.source, Some("test".to_string()));
    }

    #[test]
    fn test_dependency_analysis_to_json() {
        let analysis = DependencyAnalysisBuilder::new()
            .dashflow_version("1.11.0")
            .build();
        let json = analysis.to_json().unwrap();
        assert!(json.contains("1.11.0"));
    }

    #[test]
    fn test_dependency_analysis_summary() {
        let mut builder = DependencyAnalysisBuilder::new().dashflow_version("1.11.0");
        builder.add_dashflow_crate(CrateDependency::new("df1", "DashFlow 1"));
        builder.add_dashflow_crate(CrateDependency::new("df2", "DashFlow 2"));
        builder.add_external_crate(CrateDependency::new("ext1", "External 1"));
        let analysis = builder.build();

        let summary = analysis.summary();
        assert!(summary.contains("3 total"));
        assert!(summary.contains("2 DashFlow"));
        assert!(summary.contains("1 external"));
        assert!(summary.contains("1.11.0"));
    }

    #[test]
    fn test_dependency_analysis_find_crate_dashflow() {
        let mut builder = DependencyAnalysisBuilder::new();
        builder.add_dashflow_crate(CrateDependency::new("dashflow-openai", "OpenAI"));
        let analysis = builder.build();

        assert!(analysis.find_crate("dashflow-openai").is_some());
        assert!(analysis.find_crate("DASHFLOW-OPENAI").is_some()); // Case insensitive
        assert!(analysis.find_crate("nonexistent").is_none());
    }

    #[test]
    fn test_dependency_analysis_find_crate_external() {
        let mut builder = DependencyAnalysisBuilder::new();
        builder.add_external_crate(CrateDependency::new("tokio", "Async runtime"));
        let analysis = builder.build();

        assert!(analysis.find_crate("tokio").is_some());
    }

    #[test]
    fn test_dependency_analysis_crates_by_category() {
        let mut builder = DependencyAnalysisBuilder::new();
        builder.add_dashflow_crate(
            CrateDependency::new("openai", "OpenAI")
                .with_category(DependencyCategory::LlmProvider)
        );
        builder.add_dashflow_crate(
            CrateDependency::new("chroma", "Chroma")
                .with_category(DependencyCategory::VectorStore)
        );
        builder.add_external_crate(
            CrateDependency::new("tokio", "Tokio")
                .with_category(DependencyCategory::Runtime)
        );
        let analysis = builder.build();

        assert_eq!(analysis.crates_by_category(DependencyCategory::LlmProvider).len(), 1);
        assert_eq!(analysis.crates_by_category(DependencyCategory::VectorStore).len(), 1);
        assert_eq!(analysis.crates_by_category(DependencyCategory::Runtime).len(), 1);
        assert_eq!(analysis.crates_by_category(DependencyCategory::Testing).len(), 0);
    }

    #[test]
    fn test_dependency_analysis_llm_provider_crates() {
        let mut builder = DependencyAnalysisBuilder::new();
        builder.add_dashflow_crate(
            CrateDependency::new("openai", "OpenAI")
                .with_category(DependencyCategory::LlmProvider)
        );
        let analysis = builder.build();
        assert_eq!(analysis.llm_provider_crates().len(), 1);
    }

    #[test]
    fn test_dependency_analysis_vector_store_crates() {
        let mut builder = DependencyAnalysisBuilder::new();
        builder.add_dashflow_crate(
            CrateDependency::new("chroma", "Chroma")
                .with_category(DependencyCategory::VectorStore)
        );
        let analysis = builder.build();
        assert_eq!(analysis.vector_store_crates().len(), 1);
    }

    #[test]
    fn test_dependency_analysis_tool_crates() {
        let mut builder = DependencyAnalysisBuilder::new();
        builder.add_dashflow_crate(
            CrateDependency::new("shell", "Shell tool")
                .with_category(DependencyCategory::Tool)
        );
        let analysis = builder.build();
        assert_eq!(analysis.tool_crates().len(), 1);
    }

    #[test]
    fn test_dependency_analysis_checkpoint_crates() {
        let mut builder = DependencyAnalysisBuilder::new();
        builder.add_dashflow_crate(
            CrateDependency::new("redis-cp", "Redis checkpointer")
                .with_category(DependencyCategory::Checkpointer)
        );
        let analysis = builder.build();
        assert_eq!(analysis.checkpoint_crates().len(), 1);
    }

    #[test]
    fn test_dependency_analysis_runtime_crates() {
        let mut builder = DependencyAnalysisBuilder::new();
        builder.add_external_crate(
            CrateDependency::new("tokio", "Tokio")
                .with_category(DependencyCategory::Runtime)
        );
        let analysis = builder.build();
        assert_eq!(analysis.runtime_crates().len(), 1);
    }

    #[test]
    fn test_dependency_analysis_uses_crate() {
        let mut builder = DependencyAnalysisBuilder::new();
        builder.add_dashflow_crate(CrateDependency::new("openai", "OpenAI"));
        let analysis = builder.build();

        assert!(analysis.uses_crate("openai"));
        assert!(!analysis.uses_crate("anthropic"));
    }

    #[test]
    fn test_dependency_analysis_total_crates() {
        let mut builder = DependencyAnalysisBuilder::new();
        builder.add_dashflow_crate(CrateDependency::new("df1", "DF1"));
        builder.add_dashflow_crate(CrateDependency::new("df2", "DF2"));
        builder.add_external_crate(CrateDependency::new("ext1", "EXT1"));
        let analysis = builder.build();

        assert_eq!(analysis.total_crates(), 3);
    }

    #[test]
    fn test_dependency_analysis_crates_using_api() {
        let mut builder = DependencyAnalysisBuilder::new();
        builder.add_dashflow_crate(
            CrateDependency::new("openai", "OpenAI")
                .with_api("ChatModel::invoke")
        );
        builder.add_dashflow_crate(
            CrateDependency::new("anthropic", "Anthropic")
                .with_api("ChatModel::invoke")
        );
        builder.add_external_crate(CrateDependency::new("tokio", "Tokio"));
        let analysis = builder.build();

        let using_chat = analysis.crates_using_api("ChatModel");
        assert_eq!(using_chat.len(), 2);
    }

    #[test]
    fn test_dependency_analysis_all_apis_used() {
        let mut builder = DependencyAnalysisBuilder::new();
        builder.add_dashflow_crate(
            CrateDependency::new("openai", "OpenAI")
                .with_api("api1")
                .with_api("api2")
        );
        builder.add_external_crate(
            CrateDependency::new("tokio", "Tokio")
                .with_api("api3")
        );
        let analysis = builder.build();

        let apis = analysis.all_apis_used();
        assert_eq!(apis.len(), 3);
    }

    // =============================================================================
    // CrateDependency Tests
    // =============================================================================

    #[test]
    fn test_crate_dependency_new() {
        let dep = CrateDependency::new("test-crate", "Test purpose");
        assert_eq!(dep.name, "test-crate");
        assert_eq!(dep.purpose, "Test purpose");
        assert!(dep.version.is_none());
        assert!(dep.apis_used.is_empty());
        assert_eq!(dep.category, DependencyCategory::Other);
        assert!(dep.is_direct);
        assert!(dep.features.is_empty());
        assert!(!dep.optional);
    }

    #[test]
    fn test_crate_dependency_with_version() {
        let dep = CrateDependency::new("test", "Purpose")
            .with_version("1.0.0");
        assert_eq!(dep.version, Some("1.0.0".to_string()));
    }

    #[test]
    fn test_crate_dependency_with_api() {
        let dep = CrateDependency::new("test", "Purpose")
            .with_api("SomeApi::method");
        assert_eq!(dep.apis_used.len(), 1);
        assert_eq!(dep.apis_used[0], "SomeApi::method");
    }

    #[test]
    fn test_crate_dependency_with_apis() {
        let dep = CrateDependency::new("test", "Purpose")
            .with_apis(vec!["api1", "api2", "api3"]);
        assert_eq!(dep.apis_used.len(), 3);
    }

    #[test]
    fn test_crate_dependency_with_category() {
        let dep = CrateDependency::new("test", "Purpose")
            .with_category(DependencyCategory::LlmProvider);
        assert_eq!(dep.category, DependencyCategory::LlmProvider);
    }

    #[test]
    fn test_crate_dependency_transitive() {
        let dep = CrateDependency::new("test", "Purpose")
            .transitive();
        assert!(!dep.is_direct);
    }

    #[test]
    fn test_crate_dependency_with_features() {
        let dep = CrateDependency::new("test", "Purpose")
            .with_features(vec!["full", "rt-multi-thread"]);
        assert_eq!(dep.features.len(), 2);
        assert!(dep.features.contains(&"full".to_string()));
    }

    #[test]
    fn test_crate_dependency_optional() {
        let dep = CrateDependency::new("test", "Purpose")
            .optional();
        assert!(dep.optional);
    }

    #[test]
    fn test_crate_dependency_uses_api() {
        let dep = CrateDependency::new("test", "Purpose")
            .with_api("ChatModel::invoke")
            .with_api("Embeddings::embed");

        assert!(dep.uses_api("ChatModel"));
        assert!(dep.uses_api("chatmodel")); // Case insensitive
        assert!(dep.uses_api("invoke"));
        assert!(dep.uses_api("Embeddings"));
        assert!(!dep.uses_api("NonExistent"));
    }

    #[test]
    fn test_crate_dependency_chained() {
        let dep = CrateDependency::new("tokio", "Async runtime")
            .with_version("1.35.0")
            .with_api("spawn")
            .with_api("sleep")
            .with_category(DependencyCategory::Runtime)
            .with_features(vec!["full", "rt-multi-thread"])
            .transitive()
            .optional();

        assert_eq!(dep.name, "tokio");
        assert_eq!(dep.version, Some("1.35.0".to_string()));
        assert_eq!(dep.apis_used.len(), 2);
        assert_eq!(dep.category, DependencyCategory::Runtime);
        assert_eq!(dep.features.len(), 2);
        assert!(!dep.is_direct);
        assert!(dep.optional);
    }

    // =============================================================================
    // DependencyCategory Tests
    // =============================================================================

    #[test]
    fn test_dependency_category_default() {
        let default = DependencyCategory::default();
        assert_eq!(default, DependencyCategory::Other);
    }

    #[test]
    fn test_dependency_category_display() {
        assert_eq!(format!("{}", DependencyCategory::Core), "Core");
        assert_eq!(format!("{}", DependencyCategory::LlmProvider), "LLM Provider");
        assert_eq!(format!("{}", DependencyCategory::VectorStore), "Vector Store");
        assert_eq!(format!("{}", DependencyCategory::Tool), "Tool");
        assert_eq!(format!("{}", DependencyCategory::Checkpointer), "Checkpointer");
        assert_eq!(format!("{}", DependencyCategory::Embedding), "Embedding");
        assert_eq!(format!("{}", DependencyCategory::Runtime), "Runtime");
        assert_eq!(format!("{}", DependencyCategory::Serialization), "Serialization");
        assert_eq!(format!("{}", DependencyCategory::Networking), "Networking");
        assert_eq!(format!("{}", DependencyCategory::Observability), "Observability");
        assert_eq!(format!("{}", DependencyCategory::Testing), "Testing");
        assert_eq!(format!("{}", DependencyCategory::Other), "Other");
    }

    #[test]
    fn test_dependency_category_equality() {
        assert_eq!(DependencyCategory::Core, DependencyCategory::Core);
        assert_ne!(DependencyCategory::Core, DependencyCategory::Tool);
    }

    #[test]
    fn test_dependency_category_clone() {
        let original = DependencyCategory::LlmProvider;
        let cloned = original;
        assert_eq!(original, cloned);
    }

    // =============================================================================
    // DependencyMetadata Tests
    // =============================================================================

    #[test]
    fn test_dependency_metadata_new() {
        let meta = DependencyMetadata::new();
        assert!(meta.source.is_none());
        assert!(meta.analyzed_at.is_none());
        assert!(meta.cargo_toml_path.is_none());
        assert!(meta.notes.is_empty());
    }

    #[test]
    fn test_dependency_metadata_with_source() {
        let meta = DependencyMetadata::new()
            .with_source("Cargo.toml");
        assert_eq!(meta.source, Some("Cargo.toml".to_string()));
    }

    #[test]
    fn test_dependency_metadata_with_cargo_path() {
        let meta = DependencyMetadata::new()
            .with_cargo_path("/path/to/Cargo.toml");
        assert_eq!(meta.cargo_toml_path, Some("/path/to/Cargo.toml".to_string()));
    }

    #[test]
    fn test_dependency_metadata_with_note() {
        let meta = DependencyMetadata::new()
            .with_note("Note 1")
            .with_note("Note 2");
        assert_eq!(meta.notes.len(), 2);
    }

    #[test]
    fn test_dependency_metadata_chained() {
        let meta = DependencyMetadata::new()
            .with_source("Cargo.toml")
            .with_cargo_path("/app/Cargo.toml")
            .with_note("Analysis note");
        assert_eq!(meta.source, Some("Cargo.toml".to_string()));
        assert_eq!(meta.cargo_toml_path, Some("/app/Cargo.toml".to_string()));
        assert_eq!(meta.notes.len(), 1);
    }

    // =============================================================================
    // infer_category Tests (via AppArchitecture::infer_category)
    // =============================================================================

    #[test]
    fn test_infer_category_llm_providers() {
        assert_eq!(AppArchitecture::infer_category("dashflow-openai"), DependencyCategory::LlmProvider);
        assert_eq!(AppArchitecture::infer_category("dashflow-anthropic"), DependencyCategory::LlmProvider);
        assert_eq!(AppArchitecture::infer_category("dashflow-bedrock"), DependencyCategory::LlmProvider);
        assert_eq!(AppArchitecture::infer_category("dashflow-gemini"), DependencyCategory::LlmProvider);
        assert_eq!(AppArchitecture::infer_category("dashflow-ollama"), DependencyCategory::LlmProvider);
        assert_eq!(AppArchitecture::infer_category("dashflow-cohere"), DependencyCategory::LlmProvider);
        assert_eq!(AppArchitecture::infer_category("dashflow-mistral"), DependencyCategory::LlmProvider);
        assert_eq!(AppArchitecture::infer_category("dashflow-groq"), DependencyCategory::LlmProvider);
        assert_eq!(AppArchitecture::infer_category("dashflow-together"), DependencyCategory::LlmProvider);
        assert_eq!(AppArchitecture::infer_category("dashflow-fireworks"), DependencyCategory::LlmProvider);
        assert_eq!(AppArchitecture::infer_category("dashflow-replicate"), DependencyCategory::LlmProvider);
        assert_eq!(AppArchitecture::infer_category("dashflow-xai"), DependencyCategory::LlmProvider);
        assert_eq!(AppArchitecture::infer_category("dashflow-deepseek"), DependencyCategory::LlmProvider);
        assert_eq!(AppArchitecture::infer_category("dashflow-perplexity"), DependencyCategory::LlmProvider);
    }

    #[test]
    fn test_infer_category_vector_stores() {
        assert_eq!(AppArchitecture::infer_category("dashflow-chroma"), DependencyCategory::VectorStore);
        assert_eq!(AppArchitecture::infer_category("dashflow-pinecone"), DependencyCategory::VectorStore);
        assert_eq!(AppArchitecture::infer_category("dashflow-qdrant"), DependencyCategory::VectorStore);
        assert_eq!(AppArchitecture::infer_category("dashflow-pgvector"), DependencyCategory::VectorStore);
        assert_eq!(AppArchitecture::infer_category("dashflow-milvus"), DependencyCategory::VectorStore);
        assert_eq!(AppArchitecture::infer_category("dashflow-weaviate"), DependencyCategory::VectorStore);
        assert_eq!(AppArchitecture::infer_category("dashflow-elasticsearch"), DependencyCategory::VectorStore);
        assert_eq!(AppArchitecture::infer_category("dashflow-opensearch"), DependencyCategory::VectorStore);
        assert_eq!(AppArchitecture::infer_category("dashflow-faiss"), DependencyCategory::VectorStore);
        assert_eq!(AppArchitecture::infer_category("dashflow-lancedb"), DependencyCategory::VectorStore);
        assert_eq!(AppArchitecture::infer_category("dashflow-typesense"), DependencyCategory::VectorStore);
    }

    #[test]
    fn test_infer_category_tools() {
        assert_eq!(AppArchitecture::infer_category("dashflow-shell-tool"), DependencyCategory::Tool);
        assert_eq!(AppArchitecture::infer_category("dashflow-calculator"), DependencyCategory::Tool);
        assert_eq!(AppArchitecture::infer_category("dashflow-webscrape"), DependencyCategory::Tool);
        assert_eq!(AppArchitecture::infer_category("dashflow-playwright"), DependencyCategory::Tool);
    }

    #[test]
    fn test_infer_category_checkpointers() {
        assert_eq!(AppArchitecture::infer_category("dashflow-checkpointer"), DependencyCategory::Checkpointer);
        assert_eq!(AppArchitecture::infer_category("redis-checkpoint"), DependencyCategory::Checkpointer);
    }

    #[test]
    fn test_infer_category_embeddings() {
        assert_eq!(AppArchitecture::infer_category("dashflow-embedding"), DependencyCategory::Embedding);
        assert_eq!(AppArchitecture::infer_category("dashflow-voyage"), DependencyCategory::Embedding);
        assert_eq!(AppArchitecture::infer_category("dashflow-jina"), DependencyCategory::Embedding);
        assert_eq!(AppArchitecture::infer_category("dashflow-nomic"), DependencyCategory::Embedding);
    }

    #[test]
    fn test_infer_category_runtime() {
        assert_eq!(AppArchitecture::infer_category("tokio"), DependencyCategory::Runtime);
        assert_eq!(AppArchitecture::infer_category("async-std"), DependencyCategory::Runtime);
        assert_eq!(AppArchitecture::infer_category("smol"), DependencyCategory::Runtime);
    }

    #[test]
    fn test_infer_category_serialization() {
        assert_eq!(AppArchitecture::infer_category("serde"), DependencyCategory::Serialization);
        assert_eq!(AppArchitecture::infer_category("serde_json"), DependencyCategory::Serialization);
        assert_eq!(AppArchitecture::infer_category("bincode"), DependencyCategory::Serialization);
        assert_eq!(AppArchitecture::infer_category("rmp-serde"), DependencyCategory::Serialization);
    }

    #[test]
    fn test_infer_category_networking() {
        assert_eq!(AppArchitecture::infer_category("reqwest"), DependencyCategory::Networking);
        assert_eq!(AppArchitecture::infer_category("hyper"), DependencyCategory::Networking);
        assert_eq!(AppArchitecture::infer_category("axum"), DependencyCategory::Networking);
        assert_eq!(AppArchitecture::infer_category("actix-web"), DependencyCategory::Networking);
        assert_eq!(AppArchitecture::infer_category("tonic"), DependencyCategory::Networking);
    }

    #[test]
    fn test_infer_category_observability() {
        assert_eq!(AppArchitecture::infer_category("tracing"), DependencyCategory::Observability);
        assert_eq!(AppArchitecture::infer_category("log"), DependencyCategory::Observability);
        assert_eq!(AppArchitecture::infer_category("env_logger"), DependencyCategory::Observability);
        assert_eq!(AppArchitecture::infer_category("prometheus-client"), DependencyCategory::Observability);
    }

    #[test]
    fn test_infer_category_testing() {
        assert_eq!(AppArchitecture::infer_category("mockall"), DependencyCategory::Testing);
        assert_eq!(AppArchitecture::infer_category("proptest"), DependencyCategory::Testing);
        assert_eq!(AppArchitecture::infer_category("some-test-crate"), DependencyCategory::Testing);
    }

    #[test]
    fn test_infer_category_core() {
        assert_eq!(AppArchitecture::infer_category("dashflow"), DependencyCategory::Core);
        assert_eq!(AppArchitecture::infer_category("dashflow-core"), DependencyCategory::Core);
    }

    #[test]
    fn test_infer_category_other() {
        assert_eq!(AppArchitecture::infer_category("random-crate"), DependencyCategory::Other);
    }

    // =============================================================================
    // parse_cargo_toml Tests
    // =============================================================================

    #[test]
    fn test_parse_cargo_toml_simple() {
        let toml = r#"
[dependencies]
dashflow = "1.11.0"
tokio = "1.35.0"
"#;
        let analysis = parse_cargo_toml(toml);
        assert_eq!(analysis.dashflow_version, "1.11.0");
        assert!(analysis.uses_crate("dashflow"));
        assert!(analysis.uses_crate("tokio"));
    }

    #[test]
    fn test_parse_cargo_toml_table_format() {
        let toml = r#"
[dependencies]
tokio = { version = "1.35.0", features = ["full"] }
"#;
        let analysis = parse_cargo_toml(toml);
        let tokio = analysis.find_crate("tokio").unwrap();
        assert_eq!(tokio.version, Some("1.35.0".to_string()));
        assert!(tokio.features.contains(&"full".to_string()));
    }

    #[test]
    fn test_parse_cargo_toml_dev_dependencies() {
        let toml = r#"
[dev-dependencies]
mockall = "0.12.0"
"#;
        let analysis = parse_cargo_toml(toml);
        let mockall = analysis.find_crate("mockall").unwrap();
        assert!(mockall.optional); // dev-deps are marked optional
    }

    #[test]
    fn test_parse_cargo_toml_multiple_sections() {
        let toml = r#"
[dependencies]
dashflow = "1.11.0"
tokio = "1.35.0"

[dev-dependencies]
mockall = "0.12.0"
"#;
        let analysis = parse_cargo_toml(toml);
        assert_eq!(analysis.total_crates(), 3);
    }

    #[test]
    fn test_parse_cargo_toml_comments_ignored() {
        let toml = r#"
[dependencies]
# This is a comment
dashflow = "1.11.0"
"#;
        let analysis = parse_cargo_toml(toml);
        assert!(analysis.uses_crate("dashflow"));
        assert_eq!(analysis.total_crates(), 1);
    }

    #[test]
    fn test_parse_cargo_toml_empty() {
        let toml = "";
        let analysis = parse_cargo_toml(toml);
        assert_eq!(analysis.total_crates(), 0);
    }

    #[test]
    fn test_parse_cargo_toml_no_dependencies() {
        let toml = r#"
[package]
name = "test"
version = "0.1.0"
"#;
        let analysis = parse_cargo_toml(toml);
        assert_eq!(analysis.total_crates(), 0);
    }

    // =============================================================================
    // extract_version Tests
    // =============================================================================

    #[test]
    fn test_extract_version_simple() {
        assert_eq!(extract_version("\"1.0.0\""), Some("1.0.0".to_string()));
    }

    #[test]
    fn test_extract_version_table() {
        assert_eq!(
            extract_version("{ version = \"1.35.0\", features = [\"full\"] }"),
            Some("1.35.0".to_string())
        );
    }

    #[test]
    fn test_extract_version_no_version() {
        assert_eq!(extract_version("{ path = \"../crate\" }"), None);
    }

    #[test]
    fn test_extract_version_empty() {
        assert_eq!(extract_version(""), None);
    }

    // =============================================================================
    // extract_features Tests
    // =============================================================================

    #[test]
    fn test_extract_features_none() {
        let features = extract_features("\"1.0.0\"");
        assert!(features.is_empty());
    }

    #[test]
    fn test_extract_features_single() {
        let features = extract_features("{ version = \"1.0.0\", features = [\"full\"] }");
        assert_eq!(features.len(), 1);
        assert!(features.contains(&"full".to_string()));
    }

    #[test]
    fn test_extract_features_multiple() {
        let features = extract_features("{ version = \"1.0.0\", features = [\"rt\", \"macros\", \"net\"] }");
        assert_eq!(features.len(), 3);
        assert!(features.contains(&"rt".to_string()));
        assert!(features.contains(&"macros".to_string()));
        assert!(features.contains(&"net".to_string()));
    }

    #[test]
    fn test_extract_features_empty_array() {
        let features = extract_features("{ version = \"1.0.0\", features = [] }");
        assert!(features.is_empty());
    }

    // =============================================================================
    // infer_purpose Tests
    // =============================================================================

    #[test]
    fn test_infer_purpose_dashflow() {
        assert_eq!(infer_purpose("dashflow"), "Core graph orchestration framework");
    }

    #[test]
    fn test_infer_purpose_dashflow_openai() {
        assert_eq!(infer_purpose("dashflow-openai"), "OpenAI LLM provider (GPT-4, GPT-3.5)");
    }

    #[test]
    fn test_infer_purpose_dashflow_anthropic() {
        assert_eq!(infer_purpose("dashflow-anthropic"), "Anthropic LLM provider (Claude)");
    }

    #[test]
    fn test_infer_purpose_dashflow_chroma() {
        assert_eq!(infer_purpose("dashflow-chroma"), "Chroma vector store");
    }

    #[test]
    fn test_infer_purpose_tokio() {
        assert_eq!(infer_purpose("tokio"), "Async runtime for Rust");
    }

    #[test]
    fn test_infer_purpose_serde() {
        assert_eq!(infer_purpose("serde"), "Serialization/deserialization framework");
    }

    #[test]
    fn test_infer_purpose_reqwest() {
        assert_eq!(infer_purpose("reqwest"), "HTTP client");
    }

    #[test]
    fn test_infer_purpose_unknown() {
        assert_eq!(infer_purpose("unknown-crate"), "unknown-crate dependency");
    }

    #[test]
    fn test_infer_purpose_dashflow_unknown_suffix() {
        assert_eq!(infer_purpose("dashflow-custom"), "DashFlow custom integration");
    }

    // =============================================================================
    // Serialization Tests
    // =============================================================================

    #[test]
    fn test_dependency_analysis_serde_roundtrip() {
        let mut builder = DependencyAnalysisBuilder::new().dashflow_version("1.11.0");
        builder.add_dashflow_crate(
            CrateDependency::new("openai", "OpenAI")
                .with_version("1.11.0")
                .with_category(DependencyCategory::LlmProvider)
        );
        let original = builder.build();

        let json = serde_json::to_string(&original).unwrap();
        let deserialized: DependencyAnalysis = serde_json::from_str(&json).unwrap();

        assert_eq!(original.dashflow_version, deserialized.dashflow_version);
        assert_eq!(original.dashflow_crates.len(), deserialized.dashflow_crates.len());
    }

    #[test]
    fn test_dependency_category_serde() {
        let categories = vec![
            DependencyCategory::Core,
            DependencyCategory::LlmProvider,
            DependencyCategory::VectorStore,
            DependencyCategory::Tool,
            DependencyCategory::Checkpointer,
            DependencyCategory::Embedding,
            DependencyCategory::Runtime,
            DependencyCategory::Serialization,
            DependencyCategory::Networking,
            DependencyCategory::Observability,
            DependencyCategory::Testing,
            DependencyCategory::Other,
        ];

        for cat in categories {
            let json = serde_json::to_string(&cat).unwrap();
            let deserialized: DependencyCategory = serde_json::from_str(&json).unwrap();
            assert_eq!(cat, deserialized);
        }
    }

    #[test]
    fn test_crate_dependency_serde_roundtrip() {
        let dep = CrateDependency::new("test", "Purpose")
            .with_version("1.0.0")
            .with_api("api1")
            .with_category(DependencyCategory::Runtime)
            .with_features(vec!["f1", "f2"])
            .transitive()
            .optional();

        let json = serde_json::to_string(&dep).unwrap();
        let deserialized: CrateDependency = serde_json::from_str(&json).unwrap();

        assert_eq!(dep.name, deserialized.name);
        assert_eq!(dep.version, deserialized.version);
        assert_eq!(dep.category, deserialized.category);
        assert_eq!(dep.is_direct, deserialized.is_direct);
        assert_eq!(dep.optional, deserialized.optional);
    }
}
