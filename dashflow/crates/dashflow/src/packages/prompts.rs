// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

//! Package prompt types for loading prompts from packages into NodeConfig.
//!
//! This module implements Package→Config - enabling prompts from
//! packages to be loaded into NodeConfig structs for use in graphs.
//!
//! ## Example
//!
//! ```rust,ignore
//! use dashflow::packages::{LocalRegistry, PackageId, Version};
//! use dashflow::packages::prompts::{PackagePromptTemplate, PromptLibrary};
//!
//! // Load a prompt from an installed package
//! let registry = LocalRegistry::default_path()?;
//! let prompt = registry.get_prompt("sentiment-pack/analyzer-v2")?;
//!
//! // Convert to NodeConfig format
//! let node_config = prompt.to_node_config();
//!
//! // Use in graph
//! graph.update_node_config("sentiment_node", node_config)?;
//! ```

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use thiserror::Error;

/// Error type for prompt operations.
#[derive(Debug, Clone, Error)]
#[non_exhaustive]
pub enum PromptError {
    /// Prompt not found in package.
    #[error("Prompt '{prompt}' not found in package '{package}'")]
    PromptNotFound {
        /// Package name that was searched.
        package: String,
        /// Prompt name that was not found.
        prompt: String,
    },
    /// Invalid prompt ID format.
    #[error("Invalid prompt ID format: '{0}'. Expected 'package/prompt' or 'prompt'")]
    InvalidPromptId(String),
    /// Package is not a prompt library.
    #[error("Package '{0}' is not a prompt library")]
    NotPromptLibrary(String),
    /// IO error reading prompt file.
    #[error("IO error: {0}")]
    Io(String),
    /// Parse error for prompt file.
    #[error("Parse error: {0}")]
    ParseError(String),
}

/// Result type for prompt operations.
pub type PromptResult<T> = Result<T, PromptError>;

/// A prompt template from a package.
///
/// Unlike the runtime `PromptTemplate` in `core/prompts/string.rs` which handles
/// template formatting, this type represents a prompt as stored in a package
/// with all the metadata needed to configure an LLM node.
///
/// ## Fields
///
/// - `name`: Unique identifier within the package (e.g., "analyzer-v2")
/// - `system`: System prompt for the LLM
/// - `user_template`: Template for user messages (with `{variable}` placeholders)
/// - `input_variables`: Variables that must be provided at runtime
/// - `description`: Human-readable description
/// - `recommended_temperature`: Suggested temperature setting
/// - `recommended_max_tokens`: Suggested max tokens
/// - `metadata`: Additional key-value metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackagePromptTemplate {
    /// Unique name within the package (e.g., "analyzer-v2")
    pub name: String,
    /// System prompt for the LLM
    pub system: String,
    /// Template for user messages (with `{variable}` placeholders)
    #[serde(default)]
    pub user_template: Option<String>,
    /// Variables that must be provided at runtime
    #[serde(default)]
    pub input_variables: Vec<String>,
    /// Human-readable description
    #[serde(default)]
    pub description: String,
    /// Suggested temperature setting (0.0 - 2.0)
    #[serde(default)]
    pub recommended_temperature: Option<f64>,
    /// Suggested max tokens
    #[serde(default)]
    pub recommended_max_tokens: Option<i64>,
    /// Tags for categorization
    #[serde(default)]
    pub tags: Vec<String>,
    /// Additional metadata
    #[serde(default)]
    pub metadata: HashMap<String, serde_json::Value>,
}

impl PackagePromptTemplate {
    /// Create a new package prompt template.
    pub fn new(name: impl Into<String>, system: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            system: system.into(),
            user_template: None,
            input_variables: Vec::new(),
            description: String::new(),
            recommended_temperature: None,
            recommended_max_tokens: None,
            tags: Vec::new(),
            metadata: HashMap::new(),
        }
    }

    /// Set the user template.
    #[must_use]
    pub fn with_user_template(mut self, template: impl Into<String>) -> Self {
        self.user_template = Some(template.into());
        self
    }

    /// Set the description.
    #[must_use]
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = description.into();
        self
    }

    /// Add an input variable.
    #[must_use]
    pub fn with_input_variable(mut self, var: impl Into<String>) -> Self {
        self.input_variables.push(var.into());
        self
    }

    /// Set input variables.
    #[must_use]
    pub fn with_input_variables(
        mut self,
        vars: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        self.input_variables = vars.into_iter().map(Into::into).collect();
        self
    }

    /// Set the recommended temperature.
    #[must_use]
    pub fn with_temperature(mut self, temp: f64) -> Self {
        self.recommended_temperature = Some(temp);
        self
    }

    /// Set the recommended max tokens.
    #[must_use]
    pub fn with_max_tokens(mut self, max: i64) -> Self {
        self.recommended_max_tokens = Some(max);
        self
    }

    /// Add a tag.
    #[must_use]
    pub fn with_tag(mut self, tag: impl Into<String>) -> Self {
        self.tags.push(tag.into());
        self
    }

    /// Add metadata.
    #[must_use]
    pub fn with_metadata(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.metadata.insert(key.into(), value);
        self
    }

    /// Convert to NodeConfig format (serde_json::Value).
    ///
    /// This produces a JSON object suitable for use with `NodeConfig::with_config()`.
    ///
    /// ## Example
    ///
    /// ```rust,ignore
    /// let prompt = PackagePromptTemplate::new("analyzer", "You are a sentiment analyzer.")
    ///     .with_temperature(0.5)
    ///     .with_max_tokens(1000);
    ///
    /// let config_value = prompt.to_node_config();
    /// // config_value = {"system_prompt": "...", "temperature": 0.5, "max_tokens": 1000}
    ///
    /// let node_config = NodeConfig::new("sentiment", "llm.chat")
    ///     .with_config(config_value);
    /// ```
    #[must_use]
    pub fn to_node_config(&self) -> serde_json::Value {
        let mut config = serde_json::Map::new();

        config.insert(
            "system_prompt".to_string(),
            serde_json::Value::String(self.system.clone()),
        );

        if let Some(ref user_template) = self.user_template {
            config.insert(
                "user_template".to_string(),
                serde_json::Value::String(user_template.clone()),
            );
        }

        if !self.input_variables.is_empty() {
            config.insert(
                "input_variables".to_string(),
                serde_json::Value::Array(
                    self.input_variables
                        .iter()
                        .map(|v| serde_json::Value::String(v.clone()))
                        .collect(),
                ),
            );
        }

        if let Some(temp) = self.recommended_temperature {
            config.insert("temperature".to_string(), serde_json::json!(temp));
        }

        if let Some(max) = self.recommended_max_tokens {
            config.insert("max_tokens".to_string(), serde_json::json!(max));
        }

        // Include metadata as a nested object
        if !self.metadata.is_empty() {
            config.insert(
                "metadata".to_string(),
                serde_json::Value::Object(
                    self.metadata
                        .iter()
                        .map(|(k, v)| (k.clone(), v.clone()))
                        .collect(),
                ),
            );
        }

        serde_json::Value::Object(config)
    }
}

impl Default for PackagePromptTemplate {
    fn default() -> Self {
        Self::new("", "")
    }
}

/// A variable definition for prompt templates.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VariableDefinition {
    /// Variable name (e.g., "text", "context")
    pub name: String,
    /// Human-readable description
    #[serde(default)]
    pub description: String,
    /// Variable type hint (e.g., "string", "json", "array")
    #[serde(default = "default_var_type")]
    pub var_type: String,
    /// Whether this variable is required
    #[serde(default = "default_required")]
    pub required: bool,
    /// Default value if not provided
    #[serde(default)]
    pub default: Option<serde_json::Value>,
}

fn default_var_type() -> String {
    "string".to_string()
}

fn default_required() -> bool {
    true
}

impl VariableDefinition {
    /// Create a new required variable definition.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            description: String::new(),
            var_type: "string".to_string(),
            required: true,
            default: None,
        }
    }

    /// Create an optional variable with a default value.
    pub fn optional(name: impl Into<String>, default: serde_json::Value) -> Self {
        Self {
            name: name.into(),
            description: String::new(),
            var_type: "string".to_string(),
            required: false,
            default: Some(default),
        }
    }

    /// Set the description.
    #[must_use]
    pub fn with_description(mut self, desc: impl Into<String>) -> Self {
        self.description = desc.into();
        self
    }

    /// Set the variable type.
    #[must_use]
    pub fn with_type(mut self, var_type: impl Into<String>) -> Self {
        self.var_type = var_type.into();
        self
    }
}

/// A test case for validating prompt behavior.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptTestCase {
    /// Test case name
    pub name: String,
    /// Input values for variables
    pub inputs: HashMap<String, serde_json::Value>,
    /// Expected output patterns (for validation)
    #[serde(default)]
    pub expected_patterns: Vec<String>,
    /// Description of what this test validates
    #[serde(default)]
    pub description: String,
}

impl PromptTestCase {
    /// Create a new test case.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            inputs: HashMap::new(),
            expected_patterns: Vec::new(),
            description: String::new(),
        }
    }

    /// Add an input.
    #[must_use]
    pub fn with_input(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.inputs.insert(key.into(), value);
        self
    }

    /// Add an expected pattern.
    #[must_use]
    pub fn with_expected_pattern(mut self, pattern: impl Into<String>) -> Self {
        self.expected_patterns.push(pattern.into());
        self
    }
}

/// A prompt library from a package.
///
/// Contains a collection of related prompts with shared variables and test cases.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PromptLibrary {
    /// Prompts in this library
    pub prompts: Vec<PackagePromptTemplate>,
    /// Shared variable definitions
    #[serde(default)]
    pub variables: Vec<VariableDefinition>,
    /// Test cases for the prompts
    #[serde(default)]
    pub test_cases: Vec<PromptTestCase>,
    /// Library description
    #[serde(default)]
    pub description: String,
}

impl PromptLibrary {
    /// Create a new empty prompt library.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a prompt to the library.
    #[must_use]
    pub fn with_prompt(mut self, prompt: PackagePromptTemplate) -> Self {
        self.prompts.push(prompt);
        self
    }

    /// Add a variable definition.
    #[must_use]
    pub fn with_variable(mut self, var: VariableDefinition) -> Self {
        self.variables.push(var);
        self
    }

    /// Add a test case.
    #[must_use]
    pub fn with_test_case(mut self, test: PromptTestCase) -> Self {
        self.test_cases.push(test);
        self
    }

    /// Set the description.
    #[must_use]
    pub fn with_description(mut self, desc: impl Into<String>) -> Self {
        self.description = desc.into();
        self
    }

    /// Get a prompt by name.
    pub fn get_prompt(&self, name: &str) -> Option<&PackagePromptTemplate> {
        self.prompts.iter().find(|p| p.name == name)
    }

    /// Get all prompt names.
    pub fn prompt_names(&self) -> Vec<&str> {
        self.prompts.iter().map(|p| p.name.as_str()).collect()
    }

    /// Get the number of prompts.
    pub fn len(&self) -> usize {
        self.prompts.len()
    }

    /// Check if the library is empty.
    pub fn is_empty(&self) -> bool {
        self.prompts.is_empty()
    }
}

/// Parse a prompt ID into package and prompt name.
///
/// Accepts formats:
/// - `"package-name/prompt-name"` - fully qualified
/// - `"prompt-name"` - unqualified (requires default package)
///
/// Returns `(package_name, prompt_name)` where `package_name` is `None` for unqualified IDs.
pub fn parse_prompt_id(prompt_id: &str) -> PromptResult<(Option<String>, String)> {
    let prompt_id = prompt_id.trim();

    if prompt_id.is_empty() {
        return Err(PromptError::InvalidPromptId(prompt_id.to_string()));
    }

    if let Some(idx) = prompt_id.find('/') {
        let package = &prompt_id[..idx];
        let prompt = &prompt_id[idx + 1..];

        if package.is_empty() || prompt.is_empty() {
            return Err(PromptError::InvalidPromptId(prompt_id.to_string()));
        }

        Ok((Some(package.to_string()), prompt.to_string()))
    } else {
        Ok((None, prompt_id.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_package_prompt_template_new() {
        let prompt = PackagePromptTemplate::new("analyzer", "You are a sentiment analyzer.");
        assert_eq!(prompt.name, "analyzer");
        assert_eq!(prompt.system, "You are a sentiment analyzer.");
        assert!(prompt.user_template.is_none());
        assert!(prompt.input_variables.is_empty());
    }

    #[test]
    fn test_package_prompt_template_builder() {
        let prompt = PackagePromptTemplate::new("analyzer", "System prompt")
            .with_description("Analyzes sentiment")
            .with_user_template("Analyze: {text}")
            .with_input_variables(["text", "context"])
            .with_temperature(0.7)
            .with_max_tokens(1000)
            .with_tag("nlp")
            .with_metadata("version", serde_json::json!("1.0"));

        assert_eq!(prompt.description, "Analyzes sentiment");
        assert_eq!(prompt.user_template, Some("Analyze: {text}".to_string()));
        assert_eq!(prompt.input_variables, vec!["text", "context"]);
        assert_eq!(prompt.recommended_temperature, Some(0.7));
        assert_eq!(prompt.recommended_max_tokens, Some(1000));
        assert_eq!(prompt.tags, vec!["nlp"]);
        assert!(prompt.metadata.contains_key("version"));
    }

    #[test]
    fn test_to_node_config() {
        let prompt = PackagePromptTemplate::new("test", "You are helpful.")
            .with_user_template("{question}")
            .with_temperature(0.5)
            .with_max_tokens(500);

        let config = prompt.to_node_config();

        assert_eq!(config["system_prompt"], "You are helpful.");
        assert_eq!(config["user_template"], "{question}");
        assert_eq!(config["temperature"], 0.5);
        assert_eq!(config["max_tokens"], 500);
    }

    #[test]
    fn test_to_node_config_minimal() {
        let prompt = PackagePromptTemplate::new("minimal", "System only");
        let config = prompt.to_node_config();

        assert_eq!(config["system_prompt"], "System only");
        assert!(config.get("temperature").is_none());
        assert!(config.get("max_tokens").is_none());
    }

    #[test]
    fn test_variable_definition() {
        let var = VariableDefinition::new("text")
            .with_description("The text to analyze")
            .with_type("string");

        assert_eq!(var.name, "text");
        assert_eq!(var.description, "The text to analyze");
        assert!(var.required);
    }

    #[test]
    fn test_variable_definition_optional() {
        let var = VariableDefinition::optional("format", serde_json::json!("json"));

        assert!(!var.required);
        assert_eq!(var.default, Some(serde_json::json!("json")));
    }

    #[test]
    fn test_prompt_test_case() {
        let test = PromptTestCase::new("happy_path")
            .with_input("text", serde_json::json!("I love this!"))
            .with_expected_pattern("positive");

        assert_eq!(test.name, "happy_path");
        assert!(test.inputs.contains_key("text"));
        assert_eq!(test.expected_patterns, vec!["positive"]);
    }

    #[test]
    fn test_prompt_library() {
        let lib = PromptLibrary::new()
            .with_description("Sentiment analysis prompts")
            .with_prompt(PackagePromptTemplate::new("v1", "System v1"))
            .with_prompt(PackagePromptTemplate::new("v2", "System v2"))
            .with_variable(VariableDefinition::new("text"));

        assert_eq!(lib.len(), 2);
        assert!(!lib.is_empty());
        assert!(lib.get_prompt("v1").is_some());
        assert!(lib.get_prompt("v2").is_some());
        assert!(lib.get_prompt("v3").is_none());
        assert_eq!(lib.prompt_names(), vec!["v1", "v2"]);
    }

    #[test]
    fn test_parse_prompt_id_qualified() {
        let (pkg, prompt) = parse_prompt_id("sentiment-pack/analyzer-v2").unwrap();
        assert_eq!(pkg, Some("sentiment-pack".to_string()));
        assert_eq!(prompt, "analyzer-v2");
    }

    #[test]
    fn test_parse_prompt_id_unqualified() {
        let (pkg, prompt) = parse_prompt_id("analyzer-v2").unwrap();
        assert!(pkg.is_none());
        assert_eq!(prompt, "analyzer-v2");
    }

    #[test]
    fn test_parse_prompt_id_invalid() {
        assert!(parse_prompt_id("").is_err());
        assert!(parse_prompt_id("/").is_err());
        assert!(parse_prompt_id("pkg/").is_err());
        assert!(parse_prompt_id("/prompt").is_err());
    }

    #[test]
    fn test_prompt_error_display() {
        let err = PromptError::PromptNotFound {
            package: "pkg".to_string(),
            prompt: "test".to_string(),
        };
        assert!(err.to_string().contains("pkg"));
        assert!(err.to_string().contains("test"));

        let err = PromptError::InvalidPromptId("bad".to_string());
        assert!(err.to_string().contains("bad"));
    }
}
