//! Configuration for Codex DashFlow

use serde::{Deserialize, Serialize};

/// Configuration options for Codex DashFlow
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodexConfig {
    /// Default programming language
    pub default_language: String,

    /// Model to use for generation
    pub model: String,

    /// Maximum tokens to generate
    pub max_tokens: usize,

    /// Temperature for generation (0.0-2.0)
    pub temperature: f32,

    /// Include code comments in output
    pub include_comments: bool,

    /// Generate type annotations
    pub type_annotations: bool,
}

impl Default for CodexConfig {
    fn default() -> Self {
        Self {
            default_language: "rust".to_string(),
            model: "gpt-4o-mini".to_string(),
            max_tokens: 2048,
            temperature: 0.3,
            include_comments: true,
            type_annotations: true,
        }
    }
}

impl CodexConfig {
    /// Create config for Rust code generation
    pub fn for_rust() -> Self {
        Self {
            default_language: "rust".to_string(),
            type_annotations: true,
            ..Default::default()
        }
    }

    /// Create config for Python code generation
    pub fn for_python() -> Self {
        Self {
            default_language: "python".to_string(),
            type_annotations: true, // Python type hints
            ..Default::default()
        }
    }

    /// Create config for TypeScript code generation
    pub fn for_typescript() -> Self {
        Self {
            default_language: "typescript".to_string(),
            type_annotations: true,
            ..Default::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = CodexConfig::default();
        assert_eq!(config.default_language, "rust");
        assert!((config.temperature - 0.3).abs() < f32::EPSILON);
    }

    #[test]
    fn test_language_configs() {
        let rust = CodexConfig::for_rust();
        assert_eq!(rust.default_language, "rust");

        let python = CodexConfig::for_python();
        assert_eq!(python.default_language, "python");

        let ts = CodexConfig::for_typescript();
        assert_eq!(ts.default_language, "typescript");
    }
}
