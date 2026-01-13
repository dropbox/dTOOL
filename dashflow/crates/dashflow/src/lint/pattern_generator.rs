//! Dynamic lint pattern generation from introspection type registry.
//!
//! This module generates lint patterns dynamically from the TypeIndex instead of
//! relying solely on static YAML patterns. This ensures lint warnings stay in sync
//! with actual platform capabilities.
//!
//! # How It Works
//!
//! 1. Scans the TypeIndex for types with capability tags
//! 2. Generates regex triggers based on type names and capability tags
//! 3. Merges generated patterns with static patterns from patterns.yaml
//! 4. Prioritizes static patterns (explicit) over generated ones (inferred)
//!
//! # Example
//!
//! If the TypeIndex contains `OpenSearchBM25Retriever` with tags `["bm25", "retriever"]`,
//! this module generates patterns like:
//! - `struct.*BM25` → Warns about BM25 reimplementation
//! - `fn.*bm25_search` → Warns about BM25 search function

use super::introspection::TypeIndex;
use super::patterns::{LintPattern, LintPatterns, LintResult, Severity};
use dashflow_module_discovery::{TypeInfo, TypeKind};
use std::collections::{HashMap, HashSet};

/// Generator for dynamic lint patterns from introspection data
pub struct PatternGenerator<'a> {
    /// Type index to generate patterns from
    type_index: &'a TypeIndex,

    /// Capability tag to pattern rules
    tag_rules: HashMap<&'static str, TagRule>,
}

/// Rules for generating patterns from a capability tag
#[derive(Debug, Clone)]
struct TagRule {
    /// Category for generated patterns
    category: &'static str,
    /// Severity for generated patterns
    severity: Severity,
    /// Additional trigger patterns to generate
    extra_triggers: Vec<&'static str>,
    /// Message template (use {type_name} and {capability} placeholders)
    message_template: &'static str,
}

impl<'a> PatternGenerator<'a> {
    /// Create a new pattern generator
    pub fn new(type_index: &'a TypeIndex) -> Self {
        Self {
            type_index,
            tag_rules: Self::default_tag_rules(),
        }
    }

    /// Default rules for capability tags
    fn default_tag_rules() -> HashMap<&'static str, TagRule> {
        let mut rules = HashMap::new();

        rules.insert(
            "retriever",
            TagRule {
                category: "retrievers",
                severity: Severity::Warn,
                extra_triggers: vec![
                    r"fn\s+.*_retriev",
                    r"fn\s+get_relevant_documents",
                    r"impl.*Retriever\s+for",
                ],
                message_template: "DashFlow has platform retrievers available",
            },
        );

        rules.insert(
            "bm25",
            TagRule {
                category: "retrievers",
                severity: Severity::Warn,
                extra_triggers: vec![r"fn\s+bm25", r"fn\s+keyword_search", r"struct\s+.*BM25"],
                message_template: "Use DashFlow's BM25 retriever for keyword search",
            },
        );

        rules.insert(
            "vector_store",
            TagRule {
                category: "retrievers",
                severity: Severity::Warn,
                extra_triggers: vec![
                    r"fn\s+vector_search",
                    r"fn\s+similarity_search",
                    r"struct\s+.*VectorStore",
                ],
                message_template: "Use DashFlow's vector store abstractions",
            },
        );

        rules.insert(
            "cost_tracking",
            TagRule {
                category: "observability",
                severity: Severity::Warn,
                extra_triggers: vec![r"fn\s+track_cost", r"fn\s+record_cost", r"cost_per_token"],
                message_template: "Use DashFlow's cost tracking infrastructure",
            },
        );

        rules.insert(
            "evaluation",
            TagRule {
                category: "evaluation",
                severity: Severity::Warn,
                extra_triggers: vec![
                    r"fn\s+score_answer",
                    r"fn\s+evaluate",
                    r"struct\s+.*EvalMetrics",
                ],
                message_template: "Use DashFlow's evaluation framework",
            },
        );

        rules.insert(
            "embeddings",
            TagRule {
                category: "models",
                severity: Severity::Warn,
                extra_triggers: vec![
                    r"fn\s+embed_text",
                    r"fn\s+get_embeddings",
                    r"fn\s+create_embedding",
                ],
                message_template: "Use DashFlow's embeddings infrastructure",
            },
        );

        rules.insert(
            "llm",
            TagRule {
                category: "models",
                severity: Severity::Warn,
                extra_triggers: vec![
                    r"fn\s+call_llm",
                    r"fn\s+generate_response",
                    r"fn\s+chat_completion",
                ],
                message_template: "Use DashFlow's LLM abstractions",
            },
        );

        rules.insert(
            "chat",
            TagRule {
                category: "models",
                severity: Severity::Warn,
                extra_triggers: vec![r"fn\s+send_message", r"struct\s+.*ChatModel"],
                message_template: "Use DashFlow's chat model infrastructure",
            },
        );

        rules.insert(
            "chunking",
            TagRule {
                category: "loaders",
                severity: Severity::Warn,
                extra_triggers: vec![
                    r"fn\s+chunk_text",
                    r"fn\s+split_text",
                    r"struct\s+.*Chunker",
                ],
                message_template: "Use DashFlow's text splitters",
            },
        );

        rules.insert(
            "document_loader",
            TagRule {
                category: "loaders",
                severity: Severity::Warn,
                extra_triggers: vec![r"fn\s+load_document", r"fn\s+parse_pdf"],
                message_template: "Use DashFlow's document loaders",
            },
        );

        rules.insert(
            "caching",
            TagRule {
                category: "memory",
                severity: Severity::Info,
                extra_triggers: vec![r"struct\s+.*Cache", r"fn\s+cache_result"],
                message_template: "Consider using DashFlow's caching infrastructure",
            },
        );

        rules.insert(
            "metrics",
            TagRule {
                category: "observability",
                severity: Severity::Info,
                extra_triggers: vec![
                    r"prometheus.*counter",
                    r"prometheus.*histogram",
                    r"fn\s+record_metric",
                ],
                message_template: "Consider using DashFlow's observability infrastructure",
            },
        );

        rules.insert(
            "hybrid_search",
            TagRule {
                category: "retrievers",
                severity: Severity::Warn,
                extra_triggers: vec![
                    r"fn\s+hybrid_search",
                    r"fn\s+merge_results",
                    r"struct\s+.*HybridSearch",
                ],
                message_template: "Use DashFlow's MergerRetriever for hybrid search",
            },
        );

        rules
    }

    /// Generate patterns from the type index
    pub fn generate(&self) -> Vec<GeneratedPattern> {
        let mut patterns = Vec::new();
        let mut seen_types: HashSet<String> = HashSet::new();

        // Get all types from the index using semantic search with a broad query
        // to get types with capability tags
        let types_with_tags = self
            .type_index
            .search_semantic("retriever search embedding", 1000);

        // Also search for type names directly
        for (type_info, _score) in types_with_tags {
            if seen_types.contains(&type_info.path) {
                continue;
            }
            seen_types.insert(type_info.path.clone());

            // Generate patterns for each capability tag
            for tag in &type_info.capability_tags {
                if let Some(rule) = self.tag_rules.get(tag.as_str()) {
                    let pattern = self.generate_pattern_for_type(type_info, tag, rule);
                    patterns.push(pattern);
                }
            }
        }

        // Deduplicate by pattern name
        let mut unique_patterns: HashMap<String, GeneratedPattern> = HashMap::new();
        for pattern in patterns {
            unique_patterns
                .entry(pattern.name.clone())
                .or_insert(pattern);
        }

        unique_patterns.into_values().collect()
    }

    /// Generate a lint pattern for a specific type and tag
    fn generate_pattern_for_type(
        &self,
        type_info: &TypeInfo,
        tag: &str,
        rule: &TagRule,
    ) -> GeneratedPattern {
        let mut triggers = Vec::new();

        // Generate trigger based on type name
        // e.g., OpenSearchBM25Retriever -> struct.*BM25.*Retriever
        let name_pattern = self.type_name_to_pattern(&type_info.name, type_info.kind);
        triggers.push(name_pattern);

        // Add extra triggers from rule
        triggers.extend(rule.extra_triggers.iter().map(|s| s.to_string()));

        GeneratedPattern {
            name: format!("generated_{}_{}", tag, self.sanitize_name(&type_info.name)),
            category: rule.category.to_string(),
            severity: rule.severity,
            triggers,
            platform_type: type_info.path.clone(),
            platform_crate: type_info.crate_name.clone(),
            message: rule.message_template.to_string(),
            type_description: type_info.description.clone(),
        }
    }

    /// Convert a type name to a regex pattern
    fn type_name_to_pattern(&self, name: &str, kind: TypeKind) -> String {
        // Split camelCase/PascalCase into parts
        let parts = self.split_camel_case(name);

        // Build a flexible pattern
        let prefix = match kind {
            TypeKind::Struct => r"struct\s+.*",
            TypeKind::Trait => r"trait\s+.*",
            TypeKind::Enum => r"enum\s+.*",
            TypeKind::Function => r"fn\s+",
            TypeKind::TypeAlias => r"type\s+.*",
            TypeKind::Const => r"const\s+.*",
        };

        // Use key parts of the name for matching
        // e.g., ["Open", "Search", "BM25", "Retriever"] -> "BM25.*Retriever"
        let significant_parts: Vec<&str> = parts
            .iter()
            .filter(|p| {
                // Keep significant parts (not generic prefixes)
                let p_lower = p.to_lowercase();
                !["open", "search", "vector", "chat", "model", "base"].contains(&p_lower.as_str())
            })
            .map(|s| s.as_str())
            .collect();

        if significant_parts.is_empty() {
            // If no significant parts, use the full name
            format!("{}{}", prefix, name)
        } else if significant_parts.len() == 1 {
            format!("{}{}", prefix, significant_parts[0])
        } else {
            // Join with .* for flexibility
            format!(
                "{}{}.*{}",
                prefix,
                significant_parts[0],
                significant_parts.last().unwrap_or(&"")
            )
        }
    }

    /// Split a camelCase/PascalCase name into parts
    /// Handles consecutive uppercase like "BM25" as a single part
    fn split_camel_case(&self, name: &str) -> Vec<String> {
        let mut parts = Vec::new();
        let mut current = String::new();
        let chars: Vec<char> = name.chars().collect();

        for (i, &c) in chars.iter().enumerate() {
            let is_upper = c.is_uppercase();
            let next_is_lower = chars.get(i + 1).is_some_and(|next| next.is_lowercase());

            if is_upper && !current.is_empty() {
                // Start new part if:
                // - Current part has lowercase (transitioning from lower to upper)
                // - Or this uppercase is followed by lowercase (end of acronym)
                let current_has_lower = current.chars().any(|ch| ch.is_lowercase());
                if current_has_lower || next_is_lower {
                    parts.push(current);
                    current = String::new();
                }
            }
            current.push(c);
        }

        if !current.is_empty() {
            parts.push(current);
        }

        parts
    }

    /// Sanitize a name for use as a pattern identifier
    fn sanitize_name(&self, name: &str) -> String {
        name.chars()
            .filter(|c| c.is_alphanumeric() || *c == '_')
            .collect::<String>()
            .to_lowercase()
    }

    /// Convert generated patterns to LintPatterns (merging with static patterns)
    pub fn merge_with_static(&self, static_patterns: &LintPatterns) -> LintResult<LintPatterns> {
        let generated = self.generate();

        // Start with static patterns (they have priority)
        let mut yaml = String::from("version: \"2.0\"\npatterns:\n");

        // Add static patterns first (they're already loaded)
        // Then add generated patterns that don't conflict
        let static_names: HashSet<&str> = static_patterns.iter().map(|p| p.name.as_str()).collect();

        for gen in &generated {
            // Skip if a static pattern covers this
            if static_names.contains(gen.name.as_str()) {
                continue;
            }

            yaml.push_str(&format!(
                "  - name: {}\n    category: {}\n    severity: {}\n    triggers:\n",
                gen.name,
                gen.category,
                match gen.severity {
                    Severity::Info => "info",
                    Severity::Warn => "warn",
                    Severity::Error => "error",
                }
            ));

            for trigger in &gen.triggers {
                yaml.push_str(&format!("      - \"{}\"\n", escape_yaml_string(trigger)));
            }

            yaml.push_str(&format!(
                "    platform_module: \"{}\"\n    message: \"{}\"\n",
                gen.platform_type,
                escape_yaml_string(&gen.message)
            ));

            if !gen.type_description.is_empty() {
                yaml.push_str(&format!(
                    "    # Source: {} - {}\n",
                    gen.platform_crate, gen.type_description
                ));
            }
        }

        // Parse the generated YAML and merge
        let mut merged = static_patterns.clone();

        // Add generated patterns using YAML parsing (to properly initialize private fields)
        for gen in generated {
            if !static_names.contains(gen.name.as_str()) {
                let pattern_yaml = format!(
                    r#"name: {name}
category: {category}
severity: {severity}
triggers:
{triggers}
platform_module: "{platform_module}"
message: "{message}"
exceptions:
  - "*/test*"
  - "*/tests/*"
"#,
                    name = gen.name,
                    category = gen.category,
                    severity = match gen.severity {
                        Severity::Info => "info",
                        Severity::Warn => "warn",
                        Severity::Error => "error",
                    },
                    triggers = gen
                        .triggers
                        .iter()
                        .map(|t| format!("  - \"{}\"", escape_yaml_string(t)))
                        .collect::<Vec<_>>()
                        .join("\n"),
                    platform_module = escape_yaml_string(&gen.platform_type),
                    message = escape_yaml_string(&gen.message),
                );

                // Parse single pattern from YAML
                if let Ok(mut pattern) = serde_yml::from_str::<LintPattern>(&pattern_yaml) {
                    if pattern.compile().is_ok() {
                        merged.patterns.push(pattern);
                    }
                }
            }
        }

        Ok(merged)
    }
}

/// A generated lint pattern (before conversion to LintPattern)
#[derive(Debug, Clone)]
pub struct GeneratedPattern {
    /// Pattern name
    pub name: String,
    /// Category
    pub category: String,
    /// Severity
    pub severity: Severity,
    /// Regex triggers
    pub triggers: Vec<String>,
    /// Full path of platform type
    pub platform_type: String,
    /// Crate containing the platform type
    pub platform_crate: String,
    /// Warning message
    pub message: String,
    /// Description from type docs
    pub type_description: String,
}

/// Escape a string for YAML output
fn escape_yaml_string(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_split_camel_case() {
        // Note: Box::leak acceptable in tests - memory freed on process exit,
        // PatternGenerator requires &'static TypeIndex by design
        let gen = PatternGenerator {
            type_index: Box::leak(Box::new(TypeIndex::build(PathBuf::from(".")))),
            tag_rules: HashMap::new(),
        };

        let parts = gen.split_camel_case("OpenSearchBM25Retriever");
        assert_eq!(parts, vec!["Open", "Search", "BM25", "Retriever"]);

        let parts = gen.split_camel_case("VectorStore");
        assert_eq!(parts, vec!["Vector", "Store"]);
    }

    #[test]
    fn test_sanitize_name() {
        // Note: Box::leak acceptable in tests - memory freed on process exit
        let gen = PatternGenerator {
            type_index: Box::leak(Box::new(TypeIndex::build(PathBuf::from(".")))),
            tag_rules: HashMap::new(),
        };

        assert_eq!(
            gen.sanitize_name("OpenSearchBM25Retriever"),
            "opensearchbm25retriever"
        );
        // Note: sanitize_name keeps underscores (valid in identifiers), removes dashes
        assert_eq!(gen.sanitize_name("Foo-Bar_Baz"), "foobar_baz");
    }
}
