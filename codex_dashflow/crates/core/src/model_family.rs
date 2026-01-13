//! Model family detection and capabilities
//!
//! This module identifies model families and their capabilities for proper
//! tool configuration and prompt formatting.

use serde::{Deserialize, Serialize};

use crate::context::TruncationPolicy;

/// A model family is a group of models that share certain characteristics.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ModelFamily {
    /// The full model slug used to derive this model family
    pub slug: String,

    /// The model family name (e.g., "gpt-4", "claude-3")
    pub family: String,

    /// Whether the model needs special apply_patch instructions
    pub needs_apply_patch_instructions: bool,

    /// Whether the model supports reasoning/thinking mode
    pub supports_reasoning: bool,

    /// Whether the model supports parallel tool calls
    pub supports_parallel_tools: bool,

    /// Percentage of context window considered usable for inputs
    pub effective_context_percent: u8,

    /// Truncation policy for this model family
    pub truncation_policy: TruncationPolicy,

    /// Maximum context window size in tokens (if known)
    pub max_context_tokens: Option<u64>,

    /// Provider for this model (openai, anthropic, etc.)
    pub provider: ModelProvider,
}

/// Model provider identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum ModelProvider {
    /// OpenAI models (GPT-4, etc.)
    #[default]
    #[serde(rename = "openai")]
    OpenAI,
    /// Anthropic models (Claude, etc.)
    #[serde(rename = "anthropic")]
    Anthropic,
    /// Local models via Ollama
    #[serde(rename = "ollama")]
    Ollama,
    /// Local models via LMStudio
    #[serde(rename = "lmstudio")]
    LMStudio,
    /// Custom/other providers
    #[serde(rename = "custom")]
    Custom,
}

impl Default for ModelFamily {
    fn default() -> Self {
        Self {
            slug: "unknown".to_string(),
            family: "unknown".to_string(),
            needs_apply_patch_instructions: false,
            supports_reasoning: false,
            supports_parallel_tools: false,
            effective_context_percent: 95,
            truncation_policy: TruncationPolicy::Bytes(10_000),
            max_context_tokens: None,
            provider: ModelProvider::default(),
        }
    }
}

impl ModelFamily {
    /// Create a new model family with the given slug.
    pub fn new(slug: impl Into<String>) -> Self {
        let slug = slug.into();
        let family = derive_family_name(&slug);
        Self {
            slug,
            family,
            ..Default::default()
        }
    }

    /// Set the provider for this model family.
    pub fn with_provider(mut self, provider: ModelProvider) -> Self {
        self.provider = provider;
        self
    }

    /// Set whether reasoning/thinking is supported.
    pub fn with_reasoning(mut self, supports: bool) -> Self {
        self.supports_reasoning = supports;
        self
    }

    /// Set whether parallel tool calls are supported.
    pub fn with_parallel_tools(mut self, supports: bool) -> Self {
        self.supports_parallel_tools = supports;
        self
    }

    /// Set the max context tokens.
    pub fn with_max_context(mut self, tokens: u64) -> Self {
        self.max_context_tokens = Some(tokens);
        self
    }

    /// Set the truncation policy.
    pub fn with_truncation(mut self, policy: TruncationPolicy) -> Self {
        self.truncation_policy = policy;
        self
    }
}

/// Derive the family name from a model slug.
fn derive_family_name(slug: &str) -> String {
    // Try to extract the base family name
    // e.g., "gpt-4-turbo-2024-01-01" -> "gpt-4"
    // e.g., "claude-3-opus-20240229" -> "claude-3"

    let slug_lower = slug.to_lowercase();

    // OpenAI models
    if slug_lower.starts_with("gpt-4o") {
        return "gpt-4o".to_string();
    }
    if slug_lower.starts_with("gpt-4") {
        return "gpt-4".to_string();
    }
    if slug_lower.starts_with("gpt-3.5") {
        return "gpt-3.5".to_string();
    }
    if slug_lower.starts_with("o1") {
        return "o1".to_string();
    }
    if slug_lower.starts_with("o3") {
        return "o3".to_string();
    }

    // Anthropic models
    if slug_lower.contains("claude-3-opus") || slug_lower.contains("claude-3.5-opus") {
        return "claude-3-opus".to_string();
    }
    if slug_lower.contains("claude-3-sonnet") || slug_lower.contains("claude-3.5-sonnet") {
        return "claude-3-sonnet".to_string();
    }
    if slug_lower.contains("claude-3-haiku") || slug_lower.contains("claude-3.5-haiku") {
        return "claude-3-haiku".to_string();
    }
    if slug_lower.contains("claude-3") {
        return "claude-3".to_string();
    }
    if slug_lower.contains("claude-2") {
        return "claude-2".to_string();
    }

    // Default: use the slug as-is
    slug.to_string()
}

/// Find a model family for the given model slug.
pub fn find_family_for_model(slug: &str) -> Option<ModelFamily> {
    let slug_lower = slug.to_lowercase();

    // OpenAI o-series (reasoning models)
    if slug_lower.starts_with("o1") || slug_lower.starts_with("o3") {
        return Some(
            ModelFamily::new(slug)
                .with_provider(ModelProvider::OpenAI)
                .with_reasoning(true)
                .with_parallel_tools(false),
        );
    }

    // OpenAI GPT-4o
    if slug_lower.starts_with("gpt-4o") {
        return Some(
            ModelFamily::new(slug)
                .with_provider(ModelProvider::OpenAI)
                .with_parallel_tools(true)
                .with_max_context(128_000),
        );
    }

    // OpenAI GPT-4
    if slug_lower.starts_with("gpt-4") {
        let max_ctx = if slug_lower.contains("turbo") || slug_lower.contains("128k") {
            128_000
        } else {
            8_192
        };
        return Some(
            ModelFamily::new(slug)
                .with_provider(ModelProvider::OpenAI)
                .with_parallel_tools(true)
                .with_max_context(max_ctx),
        );
    }

    // OpenAI GPT-3.5
    if slug_lower.starts_with("gpt-3.5") {
        return Some(
            ModelFamily::new(slug)
                .with_provider(ModelProvider::OpenAI)
                .with_parallel_tools(true)
                .with_max_context(16_385),
        );
    }

    // Anthropic Claude 3
    if slug_lower.contains("claude-3") {
        return Some(
            ModelFamily::new(slug)
                .with_provider(ModelProvider::Anthropic)
                .with_parallel_tools(true)
                .with_max_context(200_000),
        );
    }

    // Anthropic Claude 2
    if slug_lower.contains("claude-2") {
        return Some(
            ModelFamily::new(slug)
                .with_provider(ModelProvider::Anthropic)
                .with_parallel_tools(false)
                .with_max_context(100_000),
        );
    }

    None
}

/// Create a default model family for unknown models.
pub fn default_model_family(slug: &str) -> ModelFamily {
    ModelFamily::new(slug)
}

/// Get the provider from a model slug.
pub fn provider_for_model(slug: &str) -> ModelProvider {
    let slug_lower = slug.to_lowercase();

    if slug_lower.starts_with("gpt-")
        || slug_lower.starts_with("o1")
        || slug_lower.starts_with("o3")
    {
        ModelProvider::OpenAI
    } else if slug_lower.contains("claude") {
        ModelProvider::Anthropic
    } else if slug_lower.contains("ollama") || slug_lower.starts_with("llama") {
        ModelProvider::Ollama
    } else if slug_lower.contains("lmstudio") {
        ModelProvider::LMStudio
    } else {
        ModelProvider::Custom
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_family_gpt4() {
        let family = find_family_for_model("gpt-4-turbo-2024-01-01").unwrap();
        assert_eq!(family.family, "gpt-4");
        assert_eq!(family.provider, ModelProvider::OpenAI);
        assert!(family.supports_parallel_tools);
        assert_eq!(family.max_context_tokens, Some(128_000));
    }

    #[test]
    fn test_find_family_gpt4o() {
        let family = find_family_for_model("gpt-4o").unwrap();
        assert_eq!(family.family, "gpt-4o");
        assert_eq!(family.provider, ModelProvider::OpenAI);
        assert!(family.supports_parallel_tools);
    }

    #[test]
    fn test_find_family_o1() {
        let family = find_family_for_model("o1-preview").unwrap();
        assert_eq!(family.family, "o1");
        assert_eq!(family.provider, ModelProvider::OpenAI);
        assert!(family.supports_reasoning);
        assert!(!family.supports_parallel_tools);
    }

    #[test]
    fn test_find_family_claude3() {
        let family = find_family_for_model("claude-3-opus-20240229").unwrap();
        assert_eq!(family.family, "claude-3-opus");
        assert_eq!(family.provider, ModelProvider::Anthropic);
        assert!(family.supports_parallel_tools);
        assert_eq!(family.max_context_tokens, Some(200_000));
    }

    #[test]
    fn test_find_family_unknown() {
        let family = find_family_for_model("unknown-model");
        assert!(family.is_none());
    }

    #[test]
    fn test_default_model_family() {
        let family = default_model_family("custom-model");
        assert_eq!(family.slug, "custom-model");
        assert_eq!(family.family, "custom-model");
        assert_eq!(family.provider, ModelProvider::OpenAI);
    }

    #[test]
    fn test_provider_for_model() {
        assert_eq!(provider_for_model("gpt-4"), ModelProvider::OpenAI);
        assert_eq!(provider_for_model("o1-preview"), ModelProvider::OpenAI);
        assert_eq!(
            provider_for_model("claude-3-opus"),
            ModelProvider::Anthropic
        );
        assert_eq!(provider_for_model("llama-2-7b"), ModelProvider::Ollama);
        assert_eq!(provider_for_model("random-model"), ModelProvider::Custom);
    }

    #[test]
    fn test_derive_family_name() {
        assert_eq!(derive_family_name("gpt-4-turbo-2024-01-01"), "gpt-4");
        assert_eq!(derive_family_name("gpt-4o-mini"), "gpt-4o");
        assert_eq!(
            derive_family_name("claude-3-opus-20240229"),
            "claude-3-opus"
        );
        assert_eq!(derive_family_name("claude-3-sonnet"), "claude-3-sonnet");
        assert_eq!(derive_family_name("o1-preview"), "o1");
        assert_eq!(derive_family_name("custom-model"), "custom-model");
    }

    #[test]
    fn test_model_family_builder() {
        let family = ModelFamily::new("test-model")
            .with_provider(ModelProvider::Anthropic)
            .with_reasoning(true)
            .with_parallel_tools(true)
            .with_max_context(100_000);

        assert_eq!(family.slug, "test-model");
        assert_eq!(family.provider, ModelProvider::Anthropic);
        assert!(family.supports_reasoning);
        assert!(family.supports_parallel_tools);
        assert_eq!(family.max_context_tokens, Some(100_000));
    }

    #[test]
    fn test_model_provider_serialization() {
        let json = serde_json::to_string(&ModelProvider::Anthropic).unwrap();
        assert_eq!(json, "\"anthropic\"");

        let provider: ModelProvider = serde_json::from_str("\"openai\"").unwrap();
        assert_eq!(provider, ModelProvider::OpenAI);
    }

    // Additional comprehensive tests

    #[test]
    fn test_model_family_default() {
        let family = ModelFamily::default();
        assert_eq!(family.slug, "unknown");
        assert_eq!(family.family, "unknown");
        assert!(!family.needs_apply_patch_instructions);
        assert!(!family.supports_reasoning);
        assert!(!family.supports_parallel_tools);
        assert_eq!(family.effective_context_percent, 95);
        assert!(family.max_context_tokens.is_none());
        assert_eq!(family.provider, ModelProvider::OpenAI);
    }

    #[test]
    fn test_model_family_debug() {
        let family = ModelFamily::new("test");
        let debug_str = format!("{:?}", family);
        assert!(debug_str.contains("ModelFamily"));
        assert!(debug_str.contains("test"));
    }

    #[test]
    fn test_model_family_clone() {
        let family = ModelFamily::new("test").with_reasoning(true);
        let cloned = family.clone();
        assert_eq!(family.slug, cloned.slug);
        assert_eq!(family.supports_reasoning, cloned.supports_reasoning);
    }

    #[test]
    fn test_model_family_eq() {
        let f1 = ModelFamily::new("test");
        let f2 = ModelFamily::new("test");
        assert_eq!(f1, f2);

        let f3 = ModelFamily::new("other");
        assert_ne!(f1, f3);
    }

    #[test]
    fn test_model_family_hash() {
        use std::collections::HashSet;
        let mut set = HashSet::new();
        set.insert(ModelFamily::new("test1"));
        set.insert(ModelFamily::new("test2"));
        assert_eq!(set.len(), 2);
    }

    #[test]
    fn test_model_family_serde_roundtrip() {
        let family = ModelFamily::new("gpt-4")
            .with_provider(ModelProvider::OpenAI)
            .with_reasoning(true)
            .with_parallel_tools(true)
            .with_max_context(128_000);

        let json = serde_json::to_string(&family).unwrap();
        let restored: ModelFamily = serde_json::from_str(&json).unwrap();

        assert_eq!(family.slug, restored.slug);
        assert_eq!(family.provider, restored.provider);
        assert_eq!(family.supports_reasoning, restored.supports_reasoning);
        assert_eq!(family.max_context_tokens, restored.max_context_tokens);
    }

    #[test]
    fn test_model_family_with_truncation() {
        let family = ModelFamily::new("test").with_truncation(TruncationPolicy::Bytes(5000));
        assert_eq!(family.truncation_policy, TruncationPolicy::Bytes(5000));
    }

    #[test]
    fn test_model_provider_default() {
        let provider = ModelProvider::default();
        assert_eq!(provider, ModelProvider::OpenAI);
    }

    #[test]
    fn test_model_provider_debug() {
        let debug_str = format!("{:?}", ModelProvider::Anthropic);
        assert!(debug_str.contains("Anthropic"));
    }

    #[test]
    fn test_model_provider_clone() {
        let provider = ModelProvider::Ollama;
        let cloned = provider;
        assert_eq!(provider, cloned);
    }

    #[test]
    fn test_model_provider_copy() {
        let provider = ModelProvider::LMStudio;
        let copied: ModelProvider = provider; // Copy trait
        assert_eq!(provider, copied);
    }

    #[test]
    fn test_model_provider_hash() {
        use std::collections::HashSet;
        let mut set = HashSet::new();
        set.insert(ModelProvider::OpenAI);
        set.insert(ModelProvider::Anthropic);
        set.insert(ModelProvider::OpenAI); // Duplicate
        assert_eq!(set.len(), 2);
    }

    #[test]
    fn test_model_provider_all_variants_serde() {
        for provider in [
            ModelProvider::OpenAI,
            ModelProvider::Anthropic,
            ModelProvider::Ollama,
            ModelProvider::LMStudio,
            ModelProvider::Custom,
        ] {
            let json = serde_json::to_string(&provider).unwrap();
            let restored: ModelProvider = serde_json::from_str(&json).unwrap();
            assert_eq!(provider, restored);
        }
    }

    #[test]
    fn test_derive_family_name_gpt35() {
        assert_eq!(derive_family_name("gpt-3.5-turbo"), "gpt-3.5");
        assert_eq!(derive_family_name("gpt-3.5-turbo-16k"), "gpt-3.5");
    }

    #[test]
    fn test_derive_family_name_o3() {
        assert_eq!(derive_family_name("o3-preview"), "o3");
        assert_eq!(derive_family_name("o3-mini"), "o3");
    }

    #[test]
    fn test_derive_family_name_claude_variants() {
        assert_eq!(derive_family_name("claude-3.5-opus"), "claude-3-opus");
        assert_eq!(derive_family_name("claude-3.5-sonnet"), "claude-3-sonnet");
        assert_eq!(derive_family_name("claude-3.5-haiku"), "claude-3-haiku");
        assert_eq!(derive_family_name("claude-2.1"), "claude-2");
    }

    #[test]
    fn test_derive_family_name_claude3_generic() {
        assert_eq!(derive_family_name("claude-3-unknown"), "claude-3");
    }

    #[test]
    fn test_find_family_gpt4_basic() {
        let family = find_family_for_model("gpt-4").unwrap();
        assert_eq!(family.family, "gpt-4");
        assert_eq!(family.max_context_tokens, Some(8_192));
    }

    #[test]
    fn test_find_family_gpt4_128k() {
        let family = find_family_for_model("gpt-4-128k").unwrap();
        assert_eq!(family.max_context_tokens, Some(128_000));
    }

    #[test]
    fn test_find_family_gpt35() {
        let family = find_family_for_model("gpt-3.5-turbo").unwrap();
        assert_eq!(family.family, "gpt-3.5");
        assert_eq!(family.provider, ModelProvider::OpenAI);
        assert!(family.supports_parallel_tools);
        assert_eq!(family.max_context_tokens, Some(16_385));
    }

    #[test]
    fn test_find_family_o3() {
        let family = find_family_for_model("o3-mini").unwrap();
        assert_eq!(family.family, "o3");
        assert!(family.supports_reasoning);
        assert!(!family.supports_parallel_tools);
    }

    #[test]
    fn test_find_family_claude2() {
        let family = find_family_for_model("claude-2.1").unwrap();
        assert_eq!(family.family, "claude-2");
        assert_eq!(family.provider, ModelProvider::Anthropic);
        assert!(!family.supports_parallel_tools);
        assert_eq!(family.max_context_tokens, Some(100_000));
    }

    #[test]
    fn test_find_family_claude3_sonnet() {
        let family = find_family_for_model("claude-3-sonnet").unwrap();
        assert_eq!(family.family, "claude-3-sonnet");
        assert!(family.supports_parallel_tools);
    }

    #[test]
    fn test_find_family_claude3_haiku() {
        let family = find_family_for_model("claude-3-haiku").unwrap();
        assert_eq!(family.family, "claude-3-haiku");
    }

    #[test]
    fn test_provider_for_model_o3() {
        assert_eq!(provider_for_model("o3-mini"), ModelProvider::OpenAI);
    }

    #[test]
    fn test_provider_for_model_ollama() {
        assert_eq!(provider_for_model("ollama/llama2"), ModelProvider::Ollama);
    }

    #[test]
    fn test_provider_for_model_lmstudio() {
        assert_eq!(
            provider_for_model("lmstudio-model"),
            ModelProvider::LMStudio
        );
    }

    #[test]
    fn test_model_family_new_sets_family() {
        let family = ModelFamily::new("gpt-4-turbo");
        assert_eq!(family.slug, "gpt-4-turbo");
        assert_eq!(family.family, "gpt-4"); // Derived from slug
    }

    #[test]
    fn test_model_family_builder_chaining() {
        let family = ModelFamily::new("test")
            .with_provider(ModelProvider::Custom)
            .with_reasoning(true)
            .with_parallel_tools(false)
            .with_max_context(50_000)
            .with_truncation(TruncationPolicy::Bytes(1000));

        assert_eq!(family.provider, ModelProvider::Custom);
        assert!(family.supports_reasoning);
        assert!(!family.supports_parallel_tools);
        assert_eq!(family.max_context_tokens, Some(50_000));
        assert_eq!(family.truncation_policy, TruncationPolicy::Bytes(1000));
    }

    #[test]
    fn test_derive_family_name_case_insensitive() {
        // derive_family_name lowercases the input
        assert_eq!(derive_family_name("GPT-4-TURBO"), "gpt-4");
        assert_eq!(derive_family_name("Claude-3-Opus"), "claude-3-opus");
    }

    #[test]
    fn test_find_family_case_insensitive() {
        // find_family_for_model should be case-insensitive
        let family = find_family_for_model("GPT-4-TURBO").unwrap();
        assert_eq!(family.provider, ModelProvider::OpenAI);

        let family = find_family_for_model("CLAUDE-3-OPUS").unwrap();
        assert_eq!(family.provider, ModelProvider::Anthropic);
    }

    #[test]
    fn test_provider_for_model_case_insensitive() {
        assert_eq!(provider_for_model("GPT-4"), ModelProvider::OpenAI);
        assert_eq!(provider_for_model("CLAUDE-3"), ModelProvider::Anthropic);
    }

    #[test]
    fn test_model_family_fields_independent() {
        // Test that setting one field doesn't affect others
        let base = ModelFamily::default();
        let modified = base.clone().with_reasoning(true);

        assert!(modified.supports_reasoning);
        assert!(!modified.supports_parallel_tools); // Unchanged
        assert_eq!(modified.provider, ModelProvider::OpenAI); // Unchanged
    }

    #[test]
    fn test_model_family_effective_context_percent() {
        let family = ModelFamily::default();
        assert_eq!(family.effective_context_percent, 95);
    }

    #[test]
    fn test_model_family_needs_apply_patch_instructions() {
        let family = ModelFamily::default();
        assert!(!family.needs_apply_patch_instructions);
    }

    #[test]
    fn test_default_model_family_uses_slug_as_family() {
        // When the slug doesn't match any known pattern, it's used as-is
        let family = default_model_family("my-custom-model-v1");
        assert_eq!(family.family, "my-custom-model-v1");
    }

    #[test]
    fn test_find_family_returns_none_for_empty_string() {
        let family = find_family_for_model("");
        assert!(family.is_none());
    }

    #[test]
    fn test_provider_for_model_empty_string() {
        // Empty string should return Custom
        assert_eq!(provider_for_model(""), ModelProvider::Custom);
    }

    #[test]
    fn test_model_provider_serde_rename() {
        // Test that serde renames work correctly
        let json = serde_json::to_string(&ModelProvider::OpenAI).unwrap();
        assert_eq!(json, "\"openai\"");

        let json = serde_json::to_string(&ModelProvider::Ollama).unwrap();
        assert_eq!(json, "\"ollama\"");

        let json = serde_json::to_string(&ModelProvider::LMStudio).unwrap();
        assert_eq!(json, "\"lmstudio\"");

        let json = serde_json::to_string(&ModelProvider::Custom).unwrap();
        assert_eq!(json, "\"custom\"");
    }
}
