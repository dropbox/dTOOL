//! Module Pattern Registry for Self-Linting
//!
//! This module provides a registry of module capabilities that enables
//! introspection-powered self-linting. Instead of hardcoded YAML patterns,
//! modules register their capabilities via `#[dashflow::capability(...)]`
//! and this registry queries that data.
//!
//! ## Overview
//!
//! The self-lint workflow:
//! 1. Modules declare capabilities via proc macro (compile-time)
//! 2. `ModulePatternRegistry` indexes all registered capabilities
//! 3. `dashflow lint` queries the registry instead of YAML patterns
//! 4. Warnings include live example usage from introspection data

use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use std::sync::OnceLock;

/// Severity level for lint warnings
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum Severity {
    /// Informational suggestion
    Info,
    /// Warning - potential reimplementation
    #[default]
    Warn,
    /// Error - definite reimplementation
    Error,
}

impl std::fmt::Display for Severity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Info => write!(f, "info"),
            Self::Warn => write!(f, "warning"),
            Self::Error => write!(f, "error"),
        }
    }
}

/// Pattern that a module replaces (for self-linting)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplacementPattern {
    /// Regex patterns that trigger this lint
    pub triggers: Vec<String>,
    /// Severity level (info, warn, error)
    pub severity: Severity,
    /// Human-readable explanation
    pub message: String,
    /// Cached compiled regexes (skipped during serialization)
    /// Each entry is (compiled_regex, index_into_triggers)
    #[serde(skip)]
    compiled_triggers: OnceLock<Vec<(Regex, usize)>>,
}

impl ReplacementPattern {
    /// Create a new replacement pattern
    #[must_use]
    pub fn new(triggers: Vec<String>, severity: Severity, message: impl Into<String>) -> Self {
        Self {
            triggers,
            severity,
            message: message.into(),
            compiled_triggers: OnceLock::new(),
        }
    }

    /// Create a warning-level pattern
    #[must_use]
    pub fn warn(triggers: Vec<String>, message: impl Into<String>) -> Self {
        Self::new(triggers, Severity::Warn, message)
    }

    /// Create an info-level pattern
    #[must_use]
    pub fn info(triggers: Vec<String>, message: impl Into<String>) -> Self {
        Self::new(triggers, Severity::Info, message)
    }

    /// Check if any trigger matches the given code
    ///
    /// This method lazily compiles regex patterns on first use and caches them
    /// for subsequent calls, avoiding the overhead of regex compilation on every check.
    pub fn matches(&self, code: &str) -> Option<&str> {
        let compiled = self.compiled_triggers.get_or_init(|| {
            self.triggers
                .iter()
                .enumerate()
                .filter_map(|(idx, t)| Regex::new(t).ok().map(|re| (re, idx)))
                .collect()
        });

        for (re, idx) in compiled {
            if re.is_match(code) {
                return Some(&self.triggers[*idx]);
            }
        }
        None
    }
}

/// Function signature for API surface documentation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionSignature {
    /// Function name
    pub name: String,
    /// Parameter list (simplified)
    pub params: String,
    /// Return type
    pub return_type: String,
    /// Brief description
    pub description: String,
}

impl FunctionSignature {
    /// Create a new function signature
    #[must_use]
    pub fn new(
        name: impl Into<String>,
        params: impl Into<String>,
        return_type: impl Into<String>,
        description: impl Into<String>,
    ) -> Self {
        Self {
            name: name.into(),
            params: params.into(),
            return_type: return_type.into(),
            description: description.into(),
        }
    }
}

/// Module capability registration for self-linting
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModuleCapabilityEntry {
    /// Full module path (e.g., "dashflow_opensearch::OpenSearchBM25Retriever")
    pub module_path: String,
    /// Semantic capability tags (e.g., ["search", "bm25", "keyword", "retriever"])
    pub capability_tags: Vec<String>,
    /// Patterns this module replaces
    pub replaces_patterns: Vec<ReplacementPattern>,
    /// Example usage (dynamically generated from docs)
    pub example_usage: String,
    /// Documentation URL
    pub docs_url: Option<String>,
    /// API surface (function signatures)
    pub api_surface: Vec<FunctionSignature>,
}

impl ModuleCapabilityEntry {
    /// Create a new module capability entry
    #[must_use]
    pub fn new(module_path: impl Into<String>) -> Self {
        Self {
            module_path: module_path.into(),
            capability_tags: Vec::new(),
            replaces_patterns: Vec::new(),
            example_usage: String::new(),
            docs_url: None,
            api_surface: Vec::new(),
        }
    }

    /// Add a capability tag
    #[must_use]
    pub fn with_tag(mut self, tag: impl Into<String>) -> Self {
        self.capability_tags.push(tag.into());
        self
    }

    /// Add multiple capability tags
    #[must_use]
    pub fn with_tags(mut self, tags: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.capability_tags.extend(tags.into_iter().map(Into::into));
        self
    }

    /// Add a replacement pattern
    #[must_use]
    pub fn replaces(mut self, pattern: ReplacementPattern) -> Self {
        self.replaces_patterns.push(pattern);
        self
    }

    /// Set example usage
    #[must_use]
    pub fn with_example(mut self, example: impl Into<String>) -> Self {
        self.example_usage = example.into();
        self
    }

    /// Set documentation URL
    #[must_use]
    pub fn with_docs_url(mut self, url: impl Into<String>) -> Self {
        self.docs_url = Some(url.into());
        self
    }

    /// Add an API function signature
    #[must_use]
    pub fn with_api(mut self, sig: FunctionSignature) -> Self {
        self.api_surface.push(sig);
        self
    }

    /// Check if this module matches any capability tag
    #[must_use]
    pub fn has_capability(&self, tag: &str) -> bool {
        self.capability_tags
            .iter()
            .any(|t| t.eq_ignore_ascii_case(tag))
    }

    /// Check if any pattern matches the given code
    pub fn matches_code(&self, code: &str) -> Option<&ReplacementPattern> {
        self.replaces_patterns.iter().find(|p| p.matches(code).is_some())
    }
}

/// A lint warning generated by pattern matching
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LintWarning {
    /// File path where the warning was found
    pub file_path: String,
    /// Line number (1-indexed)
    pub line: usize,
    /// Column number (1-indexed)
    pub column: usize,
    /// The matched code snippet
    pub matched_code: String,
    /// Severity level
    pub severity: Severity,
    /// Warning message
    pub message: String,
    /// Suggested platform module
    pub platform_module: String,
    /// Example usage code
    pub example_usage: String,
    /// Documentation URL if available
    pub docs_url: Option<String>,
}

impl LintWarning {
    /// Format the warning for display
    #[must_use]
    pub fn format(&self) -> String {
        let mut output = format!(
            "{}: {}\n  --> {}:{}:{}\n   |\n   | {}\n   |\n   = DashFlow has: {}\n",
            self.severity,
            self.message,
            self.file_path,
            self.line,
            self.column,
            self.matched_code,
            self.platform_module
        );

        if !self.example_usage.is_empty() {
            output.push_str(&format!("   = Use: {}\n", self.example_usage));
        }

        if let Some(ref url) = self.docs_url {
            output.push_str(&format!("   = Docs: {}\n", url));
        }

        output
    }
}

/// Registry of all module capabilities (populated via introspection)
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ModulePatternRegistry {
    entries: HashMap<String, ModuleCapabilityEntry>,
}

impl ModulePatternRegistry {
    /// Create a new empty registry
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a registry with the default DashFlow patterns
    #[must_use]
    pub fn with_defaults() -> Self {
        let mut registry = Self::new();
        registry.register_defaults();
        registry
    }

    /// Register a module capability entry
    pub fn register(&mut self, entry: ModuleCapabilityEntry) {
        self.entries.insert(entry.module_path.clone(), entry);
    }

    /// Get an entry by module path
    #[must_use]
    pub fn get(&self, module_path: &str) -> Option<&ModuleCapabilityEntry> {
        self.entries.get(module_path)
    }

    /// Get all entries
    pub fn entries(&self) -> impl Iterator<Item = &ModuleCapabilityEntry> {
        self.entries.values()
    }

    /// Number of registered entries
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Check if registry is empty
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Query for modules matching a capability tag
    #[must_use]
    pub fn find_by_capability(&self, tag: &str) -> Vec<&ModuleCapabilityEntry> {
        self.entries
            .values()
            .filter(|e| e.has_capability(tag))
            .collect()
    }

    /// Find modules that replace a given pattern
    #[must_use]
    pub fn find_replacement(&self, code_pattern: &str) -> Option<&ModuleCapabilityEntry> {
        self.entries
            .values()
            .find(|e| e.matches_code(code_pattern).is_some())
    }

    /// Generate lint warnings for a source file
    pub fn lint_file(&self, path: &Path) -> Vec<LintWarning> {
        let content = match std::fs::read_to_string(path) {
            Ok(c) => c,
            Err(_) => return Vec::new(),
        };

        self.lint_content(path.to_string_lossy().as_ref(), &content)
    }

    /// Generate lint warnings for source code content
    pub fn lint_content(&self, file_path: &str, content: &str) -> Vec<LintWarning> {
        let mut warnings = Vec::new();
        let mut prev_line_suppressed = false;

        for (line_num, line) in content.lines().enumerate() {
            let trimmed = line.trim();

            // Check if this line is a suppression comment (applies to NEXT line)
            if trimmed.contains("dashflow-lint: ignore") {
                prev_line_suppressed = true;
                continue;
            }

            // Skip pure comment lines (but don't set suppression)
            if trimmed.starts_with("//") {
                continue;
            }

            // If the previous line had a suppression comment, skip this line
            if prev_line_suppressed {
                prev_line_suppressed = false;
                continue;
            }

            for entry in self.entries.values() {
                if let Some(pattern) = entry.matches_code(line) {
                    warnings.push(LintWarning {
                        file_path: file_path.to_string(),
                        line: line_num + 1,
                        column: 1,
                        matched_code: line.trim().to_string(),
                        severity: pattern.severity,
                        message: pattern.message.clone(),
                        platform_module: entry.module_path.clone(),
                        example_usage: entry.example_usage.clone(),
                        docs_url: entry.docs_url.clone(),
                    });
                    break; // One warning per line
                }
            }
        }

        warnings
    }

    /// Register the default DashFlow platform patterns
    ///
    /// These patterns correspond to the YAML patterns from lint/patterns.yaml
    /// that are being migrated to introspection-based capability registration.
    fn register_defaults(&mut self) {
        // ============================================================================
        // Cost Tracking Patterns
        // ============================================================================

        // cost_tracking - main cost tracking pattern
        self.register(
            ModuleCapabilityEntry::new("dashflow_observability::cost")
                .with_tags(["cost", "tracking", "observability", "tokens", "budget"])
                .replaces(ReplacementPattern::warn(
                    vec![
                        r"struct\s+CostTracker".to_string(),
                        r"struct\s+.*CostTracker".to_string(),
                        r"fn\s+track_cost".to_string(),
                        r"fn\s+.*_cost\(".to_string(),
                        r"api_cost\s*:".to_string(),
                        r"token_cost\s*:".to_string(),
                        r"cost_per_.*token".to_string(),
                        r"struct\s+QueryCostRecord".to_string(),
                    ],
                    "DashFlow has built-in cost tracking with budget enforcement",
                ))
                .with_example(
                    "use dashflow_observability::cost::{CostTracker, ModelPricing};\n\
                     let tracker = CostTracker::new(ModelPricing::comprehensive_defaults());\n\
                     tracker.record_call(\"gpt-4o\", input_tokens, output_tokens)?;",
                ),
        );

        // embedding_cost - embedding model cost tracking
        self.register(
            ModuleCapabilityEntry::new("dashflow_observability::cost::ModelPricing")
                .with_tags(["cost", "embedding", "tokens"])
                .replaces(ReplacementPattern::warn(
                    vec![
                        r"EmbeddingModel\s*\{".to_string(),
                        r"cost_per_million_tokens".to_string(),
                        r"embedding.*cost".to_string(),
                    ],
                    "Use platform cost tracking for embedding models",
                ))
                .with_example(
                    "let pricing = ModelPricing::default()\n    \
                     .with_embedding_model(\"text-embedding-3-small\", 0.02);",
                ),
        );

        // ============================================================================
        // Retriever Patterns
        // ============================================================================

        // bm25_search - BM25 keyword search
        self.register(
            ModuleCapabilityEntry::new("dashflow_opensearch::OpenSearchBM25Retriever")
                .with_tags(["search", "bm25", "keyword", "retriever"])
                .replaces(ReplacementPattern::warn(
                    vec![
                        r"fn\s+search_keyword".to_string(),
                        r"fn\s+keyword_search".to_string(),
                        r"fn\s+bm25_search".to_string(),
                        r"struct\s+.*BM25".to_string(),
                        r"BM25Retriever".to_string(),
                    ],
                    "Use DashFlow's BM25 retriever for keyword search",
                ))
                .with_example(
                    "use dashflow_opensearch::OpenSearchBM25Retriever;\n\
                     let retriever = OpenSearchBM25Retriever::from_existing(\n    \
                     \"my_index\", \"http://localhost:9200\", 10, \"content\").await?;",
                ),
        );

        // semantic_search - vector/semantic search
        self.register(
            ModuleCapabilityEntry::new("dashflow_opensearch::VectorStoreRetriever")
                .with_tags(["search", "semantic", "vector", "embedding", "retriever"])
                .replaces(ReplacementPattern::warn(
                    vec![
                        r"fn\s+search_semantic".to_string(),
                        r"fn\s+semantic_search".to_string(),
                        r"fn\s+vector_search".to_string(),
                        r"fn\s+embedding_search".to_string(),
                        r"struct\s+.*SemanticSearcher".to_string(),
                    ],
                    "Use DashFlow's vector store retriever for semantic search",
                ))
                .with_example(
                    "use dashflow_opensearch::VectorStoreRetriever;\n\
                     let retriever = VectorStoreRetriever::from_existing(vector_store, 10);",
                ),
        );

        // hybrid_search - combined BM25 + vector search
        self.register(
            ModuleCapabilityEntry::new("dashflow::core::retrievers::MergerRetriever")
                .with_tags(["search", "hybrid", "retriever", "merge"])
                .replaces(ReplacementPattern::warn(
                    vec![
                        r"struct\s+HybridSearcher".to_string(),
                        r"struct\s+.*HybridSearch".to_string(),
                        r"fn\s+hybrid_search".to_string(),
                        r"fn\s+search_hybrid".to_string(),
                        r"merge.*retriever".to_string(),
                        r"combine.*retriev".to_string(),
                    ],
                    "Use DashFlow's MergerRetriever for hybrid search",
                ))
                .with_example(
                    "use dashflow::core::retrievers::MergerRetriever;\n\
                     let hybrid = MergerRetriever::new(vec![bm25, semantic]);",
                ),
        );

        // self_query - automatic query filtering
        self.register(
            ModuleCapabilityEntry::new("dashflow::core::retrievers::self_query::SelfQueryRetriever")
                .with_tags(["search", "self-query", "filter", "retriever"])
                .replaces(ReplacementPattern::warn(
                    vec![
                        r"struct\s+.*SelfQuery".to_string(),
                        r"fn\s+self_query".to_string(),
                        r"auto.*filter".to_string(),
                        r"query.*intent.*classif".to_string(),
                    ],
                    "Use DashFlow's self-query retriever for automatic filtering",
                ))
                .with_example("use dashflow::core::retrievers::self_query::SelfQueryRetriever;"),
        );

        // ============================================================================
        // Evaluation Framework Patterns
        // ============================================================================

        // eval_framework - evaluation suites
        self.register(
            ModuleCapabilityEntry::new("dashflow_streaming::evals")
                .with_tags(["eval", "evaluation", "qa", "dataset", "scoring"])
                .replaces(ReplacementPattern::warn(
                    vec![
                        r"struct\s+EvalQuestion".to_string(),
                        r"struct\s+EvalCase".to_string(),
                        r"struct\s+.*GoldenQA".to_string(),
                        r"fn\s+score_answer".to_string(),
                        r"fn\s+evaluate_answer".to_string(),
                        r"eval_dataset".to_string(),
                        r"golden_qa".to_string(),
                        r"expected_answer".to_string(),
                    ],
                    "Use DashFlow's evaluation framework",
                ))
                .with_example(
                    "use dashflow_streaming::evals::{EvalSuite, EvalCase, score_answer};\n\
                     let suite = EvalSuite::load(\"data/eval_suite.json\")?;",
                ),
        );

        // eval_metrics - evaluation metrics
        self.register(
            ModuleCapabilityEntry::new("dashflow_streaming::evals::metrics")
                .with_tags(["eval", "metrics", "precision", "recall", "ndcg"])
                .replaces(ReplacementPattern::warn(
                    vec![
                        r"struct\s+.*EvalMetrics".to_string(),
                        r"average_correctness".to_string(),
                        r"precision_at_k".to_string(),
                        r"recall_at_k".to_string(),
                        r"ndcg".to_string(),
                        r"mrr\s*=".to_string(),
                    ],
                    "Use DashFlow's evaluation metrics",
                ))
                .with_example(
                    "use dashflow_streaming::evals::metrics::{EvalMetrics, average_correctness};",
                ),
        );

        // ============================================================================
        // Language Model Patterns
        // ============================================================================

        // llm_wrapper - LLM client wrappers
        self.register(
            ModuleCapabilityEntry::new("dashflow::core::language_models::ChatModel")
                .with_tags(["llm", "chat", "openai", "anthropic", "model"])
                .replaces(ReplacementPattern::warn(
                    vec![
                        r"struct\s+.*LLMWrapper".to_string(),
                        r"struct\s+.*ChatModel".to_string(),
                        r"fn\s+call_openai".to_string(),
                        r"fn\s+call_anthropic".to_string(),
                        r"fn\s+generate_response".to_string(),
                        r"openai.*completion".to_string(),
                        r"anthropic.*message".to_string(),
                    ],
                    "Use DashFlow's ChatModel trait for LLM interactions",
                ))
                .with_example(
                    "use dashflow_openai::ChatOpenAI;\n\
                     let model = ChatOpenAI::new().with_model(\"gpt-4o-mini\");\n\
                     let response = model.generate(&messages, None, None, None, None).await?;",
                ),
        );

        // embedding_wrapper - embedding generation
        self.register(
            ModuleCapabilityEntry::new("dashflow::core::embeddings::Embeddings")
                .with_tags(["embedding", "vector", "openai", "huggingface"])
                .replaces(ReplacementPattern::warn(
                    vec![
                        r"fn\s+get_embeddings".to_string(),
                        r"fn\s+embed_text".to_string(),
                        r"fn\s+create_embeddings".to_string(),
                        r"struct\s+.*Embedder".to_string(),
                        r"embed.*api".to_string(),
                    ],
                    "Use DashFlow's Embeddings trait for embedding generation",
                ))
                .with_example(
                    "use dashflow_openai::OpenAIEmbeddings;\n\
                     let embedder = OpenAIEmbeddings::new();\n\
                     let vectors = embedder.embed_documents(&texts).await?;",
                ),
        );

        // ============================================================================
        // Chat History Patterns
        // ============================================================================

        // chat_history - conversation memory
        self.register(
            ModuleCapabilityEntry::new("dashflow::core::chat_history::ChatMessageHistory")
                .with_tags(["chat", "history", "memory", "conversation"])
                .replaces(ReplacementPattern::warn(
                    vec![
                        r"struct\s+ChatHistory".to_string(),
                        r"struct\s+ConversationHistory".to_string(),
                        r"struct\s+MessageHistory".to_string(),
                        r"fn\s+add_message".to_string(),
                        r"fn\s+get_history".to_string(),
                        r"conversation_buffer".to_string(),
                    ],
                    "Use DashFlow's chat history for conversation management",
                ))
                .with_example(
                    "use dashflow::core::chat_history::ChatMessageHistory;\n\
                     let mut history = InMemoryChatMessageHistory::new();\n\
                     history.add_user_message(\"Hello\").await?;",
                ),
        );

        // ============================================================================
        // Telemetry Patterns
        // ============================================================================

        // custom_telemetry - custom metrics/telemetry
        self.register(
            ModuleCapabilityEntry::new("dashflow_observability")
                .with_tags(["telemetry", "metrics", "prometheus", "observability"])
                .replaces(ReplacementPattern::warn(
                    vec![
                        r"struct\s+.*Telemetry".to_string(),
                        r"fn\s+record_metric".to_string(),
                        r"fn\s+emit_span".to_string(),
                        r"prometheus.*counter".to_string(),
                        r"prometheus.*histogram".to_string(),
                    ],
                    "Use DashFlow's observability infrastructure for telemetry",
                ))
                .with_example(
                    "use dashflow_prometheus_exporter::{DashFlowExporter, QueryMetrics};\n\
                     let exporter = DashFlowExporter::new();",
                ),
        );

        // tracing_setup - tracing configuration (info level)
        self.register(
            ModuleCapabilityEntry::new("dashflow_observability::tracing")
                .with_tags(["tracing", "logging", "observability"])
                .replaces(ReplacementPattern::info(
                    vec![
                        r"tracing_subscriber.*init".to_string(),
                        r"EnvFilter.*from_default".to_string(),
                    ],
                    "Consider using DashFlow's tracing setup for consistency",
                )),
        );

        // ============================================================================
        // Document Loading Patterns
        // ============================================================================

        // document_loader - document parsing
        self.register(
            ModuleCapabilityEntry::new("dashflow::core::document_loaders")
                .with_tags(["document", "loader", "pdf", "docx", "parsing"])
                .replaces(ReplacementPattern::warn(
                    vec![
                        r"struct\s+.*DocumentLoader".to_string(),
                        r"fn\s+load_documents".to_string(),
                        r"fn\s+parse_pdf".to_string(),
                        r"fn\s+parse_docx".to_string(),
                    ],
                    "Use DashFlow's document loaders",
                ))
                .with_example(
                    "use dashflow_pdf::PdfLoader;\n\
                     let loader = PdfLoader::new(path);\n\
                     let documents = loader.load().await?;",
                ),
        );

        // text_splitter - text chunking
        self.register(
            ModuleCapabilityEntry::new("dashflow::core::text_splitter")
                .with_tags(["text", "splitter", "chunker", "chunk"])
                .replaces(ReplacementPattern::warn(
                    vec![
                        r"struct\s+.*TextSplitter".to_string(),
                        r"struct\s+.*Chunker".to_string(),
                        r"fn\s+split_text".to_string(),
                        r"fn\s+chunk_text".to_string(),
                        r"chunk_size.*overlap".to_string(),
                    ],
                    "Use DashFlow's text splitters",
                ))
                .with_example(
                    "use dashflow::core::text_splitter::RecursiveCharacterTextSplitter;\n\
                     let splitter = RecursiveCharacterTextSplitter::new()\n    \
                     .with_chunk_size(1000).with_chunk_overlap(200);",
                ),
        );

        // ============================================================================
        // RAG Pipeline Patterns
        // ============================================================================

        // rag_chain - RAG pipelines
        self.register(
            ModuleCapabilityEntry::new("dashflow_chains::retrieval::RetrievalQA")
                .with_tags(["rag", "chain", "qa", "retrieval"])
                .replaces(ReplacementPattern::warn(
                    vec![
                        r"struct\s+.*RAGChain".to_string(),
                        r"struct\s+.*QAChain".to_string(),
                        r"fn\s+create_rag_chain".to_string(),
                        r"retrieval.*qa".to_string(),
                    ],
                    "Use DashFlow's RAG chain implementations",
                ))
                .with_example(
                    "use dashflow_chains::retrieval::RetrievalQA;\n\
                     let qa_chain = RetrievalQA::new(llm, retriever);",
                ),
        );

        // answer_synthesis - answer generation (info level)
        self.register(
            ModuleCapabilityEntry::new("dashflow_chains::retrieval::StuffDocumentsChain")
                .with_tags(["synthesis", "answer", "generation"])
                .replaces(ReplacementPattern::info(
                    vec![
                        r"struct\s+AnswerSynthesizer".to_string(),
                        r"fn\s+synthesize_answer".to_string(),
                        r"fn\s+generate_answer".to_string(),
                    ],
                    "Consider using DashFlow's document chains for answer synthesis",
                )),
        );
    }

    /// Export the registry to JSON
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    /// Import registry from JSON
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_severity_display() {
        assert_eq!(Severity::Info.to_string(), "info");
        assert_eq!(Severity::Warn.to_string(), "warning");
        assert_eq!(Severity::Error.to_string(), "error");
    }

    #[test]
    fn test_replacement_pattern_matches() {
        let pattern = ReplacementPattern::warn(
            vec![r"struct\s+CostTracker".to_string()],
            "test message",
        );
        assert!(pattern.matches("pub struct CostTracker {").is_some());
        assert!(pattern.matches("struct CostTracker").is_some());
        assert!(pattern.matches("fn main() {}").is_none());
    }

    #[test]
    fn test_module_capability_entry_builder() {
        let entry = ModuleCapabilityEntry::new("test::module")
            .with_tag("tag1")
            .with_tags(["tag2", "tag3"])
            .with_example("use test::module;")
            .with_docs_url("https://docs.example.com");

        assert_eq!(entry.module_path, "test::module");
        assert_eq!(entry.capability_tags, vec!["tag1", "tag2", "tag3"]);
        assert_eq!(entry.example_usage, "use test::module;");
        assert_eq!(entry.docs_url, Some("https://docs.example.com".to_string()));
    }

    #[test]
    fn test_module_has_capability() {
        let entry = ModuleCapabilityEntry::new("test::module")
            .with_tags(["Search", "BM25"]);

        assert!(entry.has_capability("search"));
        assert!(entry.has_capability("SEARCH"));
        assert!(entry.has_capability("bm25"));
        assert!(!entry.has_capability("vector"));
    }

    #[test]
    fn test_registry_find_by_capability() {
        let mut registry = ModulePatternRegistry::new();
        registry.register(
            ModuleCapabilityEntry::new("module1").with_tags(["search", "bm25"]),
        );
        registry.register(
            ModuleCapabilityEntry::new("module2").with_tags(["search", "vector"]),
        );
        registry.register(
            ModuleCapabilityEntry::new("module3").with_tags(["eval"]),
        );

        let search_modules = registry.find_by_capability("search");
        assert_eq!(search_modules.len(), 2);

        let bm25_modules = registry.find_by_capability("bm25");
        assert_eq!(bm25_modules.len(), 1);
        assert_eq!(bm25_modules[0].module_path, "module1");
    }

    #[test]
    fn test_registry_find_replacement() {
        let mut registry = ModulePatternRegistry::new();
        registry.register(
            ModuleCapabilityEntry::new("dashflow::cost")
                .replaces(ReplacementPattern::warn(
                    vec![r"struct\s+CostTracker".to_string()],
                    "Use built-in cost tracking",
                )),
        );

        let replacement = registry.find_replacement("pub struct CostTracker {");
        assert!(replacement.is_some());
        assert_eq!(replacement.unwrap().module_path, "dashflow::cost");

        let no_match = registry.find_replacement("fn main() {}");
        assert!(no_match.is_none());
    }

    #[test]
    fn test_registry_lint_content() {
        let registry = ModulePatternRegistry::with_defaults();

        let content = r#"
use std::collections::HashMap;

pub struct CostTracker {
    total: f64,
}

fn track_cost(amount: f64) {
    // implementation
}
"#;

        let warnings = registry.lint_content("test.rs", content);
        assert!(!warnings.is_empty());
        assert!(warnings.iter().any(|w| w.line == 4)); // struct CostTracker line
    }

    #[test]
    fn test_lint_warning_format() {
        let warning = LintWarning {
            file_path: "src/cost.rs".to_string(),
            line: 15,
            column: 1,
            matched_code: "pub struct CostTracker {".to_string(),
            severity: Severity::Warn,
            message: "DashFlow has built-in cost tracking".to_string(),
            platform_module: "dashflow_observability::cost".to_string(),
            example_usage: "use dashflow_observability::cost::CostTracker;".to_string(),
            docs_url: Some("https://docs.dashflow.dev/cost".to_string()),
        };

        let formatted = warning.format();
        assert!(formatted.contains("warning:"));
        assert!(formatted.contains("src/cost.rs:15:1"));
        assert!(formatted.contains("DashFlow has:"));
        assert!(formatted.contains("dashflow_observability::cost"));
    }

    #[test]
    fn test_suppression_comment_skipped() {
        let registry = ModulePatternRegistry::with_defaults();

        // Test that suppression comment on previous line suppresses the warning
        let content_suppressed = r#"
// dashflow-lint: ignore cost_tracking
pub struct CostTracker {
    total: f64,
}
"#;

        let warnings_suppressed = registry.lint_content("test.rs", content_suppressed);
        // The suppression comment on line N should suppress warnings on line N+1
        assert!(
            warnings_suppressed.is_empty(),
            "Suppression comment should suppress the next line warning"
        );

        // Test that without suppression, the warning IS generated
        let content_no_suppression = r#"
pub struct CostTracker {
    total: f64,
}
"#;

        let warnings_no_suppression = registry.lint_content("test.rs", content_no_suppression);
        assert!(
            !warnings_no_suppression.is_empty(),
            "Without suppression, CostTracker should generate a warning"
        );

        // Test that suppression only applies to the NEXT line (not multiple lines)
        let content_multi_line = r#"
// dashflow-lint: ignore cost_tracking
fn some_function() {}
pub struct CostTracker {
    total: f64,
}
"#;

        let warnings_multi = registry.lint_content("test.rs", content_multi_line);
        // The suppression applies to "fn some_function()", not to "pub struct CostTracker"
        assert!(
            !warnings_multi.is_empty(),
            "Suppression should only apply to the immediately next line"
        );
    }

    #[test]
    fn test_registry_serialization() {
        let registry = ModulePatternRegistry::with_defaults();

        let json = registry.to_json().unwrap();
        let restored = ModulePatternRegistry::from_json(&json).unwrap();

        assert_eq!(registry.len(), restored.len());
    }

    #[test]
    fn test_with_defaults_includes_patterns() {
        let registry = ModulePatternRegistry::with_defaults();

        // Should have all 17 patterns from YAML migrated
        assert_eq!(registry.len(), 17);

        // Check key patterns are registered
        assert!(registry.get("dashflow_observability::cost").is_some());
        assert!(registry.get("dashflow_opensearch::OpenSearchBM25Retriever").is_some());
        assert!(registry.get("dashflow_streaming::evals").is_some());
        assert!(registry.get("dashflow::core::retrievers::MergerRetriever").is_some());
        assert!(registry.get("dashflow::core::language_models::ChatModel").is_some());
        assert!(registry.get("dashflow::core::embeddings::Embeddings").is_some());
        assert!(registry.get("dashflow::core::chat_history::ChatMessageHistory").is_some());
        assert!(registry.get("dashflow::core::text_splitter").is_some());
        assert!(registry.get("dashflow_chains::retrieval::RetrievalQA").is_some());
    }
}
