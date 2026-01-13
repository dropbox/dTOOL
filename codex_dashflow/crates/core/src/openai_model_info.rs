//! OpenAI model metadata and context window information
//!
//! This module provides information about OpenAI models, particularly their
//! context window sizes, which is critical for managing conversation length
//! and auto-compaction.

use crate::model_family::ModelFamily;

// Shared constants for commonly used window/token sizes.
/// Context window size for newer Codex models (272K tokens)
pub const CONTEXT_WINDOW_272K: i64 = 272_000;

/// Metadata about a model, particularly OpenAI models.
///
/// Contains information about context window sizes and auto-compaction thresholds.
/// This helps present more accurate pricing and resource information in the UI.
#[derive(Debug, Clone)]
pub struct OpenAiModelInfo {
    /// Size of the context window in tokens. This is the maximum size of the input context.
    pub context_window: i64,

    /// Token threshold where we should automatically compact conversation history.
    /// This considers input tokens + output tokens of this turn.
    pub auto_compact_token_limit: Option<i64>,
}

impl OpenAiModelInfo {
    /// Create new model info with a given context window size.
    /// Auto-compact limit defaults to 90% of context window.
    pub const fn new(context_window: i64) -> Self {
        Self {
            context_window,
            auto_compact_token_limit: Some(Self::default_auto_compact_limit(context_window)),
        }
    }

    /// Create model info with a custom auto-compact limit
    pub const fn with_auto_compact_limit(context_window: i64, auto_compact_limit: i64) -> Self {
        Self {
            context_window,
            auto_compact_token_limit: Some(auto_compact_limit),
        }
    }

    /// Calculate default auto-compact limit (90% of context window)
    const fn default_auto_compact_limit(context_window: i64) -> i64 {
        (context_window * 9) / 10
    }
}

/// Get model info for a given model family.
///
/// Returns context window sizes and auto-compaction thresholds for known models.
/// Returns None for unknown models.
pub fn get_model_info(model_family: &ModelFamily) -> Option<OpenAiModelInfo> {
    let slug = model_family.slug.as_str();
    match slug {
        // OSS models have a 128k shared token pool.
        // Arbitrarily splitting it: 3/4 input context, 1/4 output.
        // https://openai.com/index/gpt-oss-model-card/
        "gpt-oss-20b" => Some(OpenAiModelInfo::new(96_000)),
        "gpt-oss-120b" => Some(OpenAiModelInfo::new(96_000)),

        // https://platform.openai.com/docs/models/o3
        "o3" => Some(OpenAiModelInfo::new(200_000)),

        // https://platform.openai.com/docs/models/o4-mini
        "o4-mini" => Some(OpenAiModelInfo::new(200_000)),

        // https://platform.openai.com/docs/models/codex-mini-latest
        "codex-mini-latest" => Some(OpenAiModelInfo::new(200_000)),

        // As of Jun 25, 2025, gpt-4.1 defaults to gpt-4.1-2025-04-14.
        // https://platform.openai.com/docs/models/gpt-4.1
        "gpt-4.1" | "gpt-4.1-2025-04-14" => Some(OpenAiModelInfo::new(1_047_576)),

        // As of Jun 25, 2025, gpt-4o defaults to gpt-4o-2024-08-06.
        // https://platform.openai.com/docs/models/gpt-4o
        "gpt-4o" | "gpt-4o-2024-08-06" => Some(OpenAiModelInfo::new(128_000)),

        // https://platform.openai.com/docs/models/gpt-4o?snapshot=gpt-4o-2024-05-13
        "gpt-4o-2024-05-13" => Some(OpenAiModelInfo::new(128_000)),

        // https://platform.openai.com/docs/models/gpt-4o?snapshot=gpt-4o-2024-11-20
        "gpt-4o-2024-11-20" => Some(OpenAiModelInfo::new(128_000)),

        // https://platform.openai.com/docs/models/gpt-3.5-turbo
        "gpt-3.5-turbo" => Some(OpenAiModelInfo::new(16_385)),

        // GPT-5 Codex models
        _ if slug.starts_with("gpt-5-codex")
            || slug.starts_with("gpt-5.1-codex")
            || slug.starts_with("gpt-5.1-codex-max") =>
        {
            Some(OpenAiModelInfo::new(CONTEXT_WINDOW_272K))
        }

        // GPT-5 family
        _ if slug.starts_with("gpt-5") => Some(OpenAiModelInfo::new(CONTEXT_WINDOW_272K)),

        // Codex models
        _ if slug.starts_with("codex-") => Some(OpenAiModelInfo::new(CONTEXT_WINDOW_272K)),

        // Experimental models
        _ if slug.starts_with("exp-") => Some(OpenAiModelInfo::new(CONTEXT_WINDOW_272K)),

        // Unknown model
        _ => None,
    }
}

/// Get context window size for a model, with a default fallback
pub fn get_context_window(model_family: &ModelFamily, default: i64) -> i64 {
    get_model_info(model_family)
        .map(|info| info.context_window)
        .unwrap_or(default)
}

/// Get auto-compact token limit for a model
pub fn get_auto_compact_limit(model_family: &ModelFamily) -> Option<i64> {
    get_model_info(model_family).and_then(|info| info.auto_compact_token_limit)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn model_family(slug: &str) -> ModelFamily {
        ModelFamily::new(slug)
    }

    #[test]
    fn test_gpt4o_context_window() {
        let family = model_family("gpt-4o");
        let info = get_model_info(&family).unwrap();
        assert_eq!(info.context_window, 128_000);
    }

    #[test]
    fn test_gpt4o_specific_version() {
        let family = model_family("gpt-4o-2024-08-06");
        let info = get_model_info(&family).unwrap();
        assert_eq!(info.context_window, 128_000);
    }

    #[test]
    fn test_gpt35_turbo() {
        let family = model_family("gpt-3.5-turbo");
        let info = get_model_info(&family).unwrap();
        assert_eq!(info.context_window, 16_385);
    }

    #[test]
    fn test_o3_model() {
        let family = model_family("o3");
        let info = get_model_info(&family).unwrap();
        assert_eq!(info.context_window, 200_000);
    }

    #[test]
    fn test_o4_mini() {
        let family = model_family("o4-mini");
        let info = get_model_info(&family).unwrap();
        assert_eq!(info.context_window, 200_000);
    }

    #[test]
    fn test_gpt41_context_window() {
        let family = model_family("gpt-4.1");
        let info = get_model_info(&family).unwrap();
        assert_eq!(info.context_window, 1_047_576);
    }

    #[test]
    fn test_gpt5_codex() {
        let family = model_family("gpt-5-codex-preview");
        let info = get_model_info(&family).unwrap();
        assert_eq!(info.context_window, CONTEXT_WINDOW_272K);
    }

    #[test]
    fn test_gpt5_family() {
        let family = model_family("gpt-5-preview");
        let info = get_model_info(&family).unwrap();
        assert_eq!(info.context_window, CONTEXT_WINDOW_272K);
    }

    #[test]
    fn test_codex_prefix() {
        let family = model_family("codex-mini-latest");
        let info = get_model_info(&family).unwrap();
        assert_eq!(info.context_window, 200_000);
    }

    #[test]
    fn test_codex_generic() {
        let family = model_family("codex-something");
        let info = get_model_info(&family).unwrap();
        assert_eq!(info.context_window, CONTEXT_WINDOW_272K);
    }

    #[test]
    fn test_experimental_model() {
        let family = model_family("exp-new-model");
        let info = get_model_info(&family).unwrap();
        assert_eq!(info.context_window, CONTEXT_WINDOW_272K);
    }

    #[test]
    fn test_oss_models() {
        let family_20b = model_family("gpt-oss-20b");
        let info_20b = get_model_info(&family_20b).unwrap();
        assert_eq!(info_20b.context_window, 96_000);

        let family_120b = model_family("gpt-oss-120b");
        let info_120b = get_model_info(&family_120b).unwrap();
        assert_eq!(info_120b.context_window, 96_000);
    }

    #[test]
    fn test_unknown_model() {
        let family = model_family("unknown-model-xyz");
        assert!(get_model_info(&family).is_none());
    }

    #[test]
    fn test_auto_compact_limit() {
        let family = model_family("gpt-4o");
        let info = get_model_info(&family).unwrap();
        // 90% of 128_000 = 115_200
        assert_eq!(info.auto_compact_token_limit, Some(115_200));
    }

    #[test]
    fn test_get_context_window_with_default() {
        let known = model_family("gpt-4o");
        assert_eq!(get_context_window(&known, 50_000), 128_000);

        let unknown = model_family("unknown-model");
        assert_eq!(get_context_window(&unknown, 50_000), 50_000);
    }

    #[test]
    fn test_get_auto_compact_limit_unknown() {
        let unknown = model_family("unknown-model");
        assert!(get_auto_compact_limit(&unknown).is_none());
    }

    #[test]
    fn test_model_info_new() {
        let info = OpenAiModelInfo::new(100_000);
        assert_eq!(info.context_window, 100_000);
        assert_eq!(info.auto_compact_token_limit, Some(90_000));
    }

    #[test]
    fn test_model_info_with_custom_limit() {
        let info = OpenAiModelInfo::with_auto_compact_limit(100_000, 80_000);
        assert_eq!(info.context_window, 100_000);
        assert_eq!(info.auto_compact_token_limit, Some(80_000));
    }
}
