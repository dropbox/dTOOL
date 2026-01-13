// Allow clippy warnings for this module
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::clone_on_ref_ptr)]
#![allow(clippy::needless_pass_by_value, clippy::redundant_clone)]

//! Base prompt template trait
//!
//! This module defines the core trait for all prompt templates.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::core::error::{Error, Result};
use crate::core::prompt_values::PromptValue;

/// Format for the prompt template
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum PromptTemplateFormat {
    /// Python f-string format (default): "Hello {name}"
    #[serde(rename = "f-string")]
    #[default]
    FString,
    /// Jinja2 template format: "Hello {{ name }}"
    Jinja2,
    /// Mustache template format: "Hello {{name}}"
    Mustache,
}

/// Base trait for all prompt templates
///
/// A prompt template is a Runnable that takes a dictionary of inputs
/// and produces a `PromptValue`.
pub trait BasePromptTemplate: Send + Sync {
    /// Get the input variables required by this template
    fn input_variables(&self) -> &[String];

    /// Get the optional variables for this template
    fn optional_variables(&self) -> &[String];

    /// Get the partial variables (pre-filled values)
    fn partial_variables(&self) -> &HashMap<String, String>;

    /// Format the prompt with the given inputs
    ///
    /// This returns a `PromptValue` that can be used with language models.
    fn format_prompt(&self, inputs: &HashMap<String, String>) -> Result<Box<dyn PromptValue>>;

    /// Validate that the template has all required variables
    fn validate_inputs(&self, inputs: &HashMap<String, String>) -> Result<()> {
        let provided_keys: std::collections::HashSet<_> = inputs.keys().collect();
        let required_keys: std::collections::HashSet<_> = self.input_variables().iter().collect();
        let optional_keys: std::collections::HashSet<_> =
            self.optional_variables().iter().collect();
        let partial_keys: std::collections::HashSet<_> = self.partial_variables().keys().collect();

        // Check for missing required variables (excluding optional and partial)
        let missing: Vec<_> = required_keys
            .difference(&provided_keys)
            .filter(|k| !optional_keys.contains(*k) && !partial_keys.contains(*k))
            .map(|k| (*k).clone())
            .collect();

        if !missing.is_empty() {
            return Err(Error::InvalidInput(format!(
                "Missing required input variables: {}",
                missing.join(", ")
            )));
        }

        Ok(())
    }

    /// Merge provided inputs with partial variables
    fn merge_inputs(&self, inputs: &HashMap<String, String>) -> HashMap<String, String> {
        let partials = self.partial_variables();
        let mut merged = HashMap::with_capacity(partials.len() + inputs.len());

        // Insert partials first (no clone of HashMap itself, just key-value pairs)
        for (k, v) in partials {
            merged.insert(k.clone(), v.clone());
        }

        // Insert inputs, overwriting any partials with same key
        for (k, v) in inputs {
            merged.insert(k.clone(), v.clone());
        }

        merged
    }
}

/// Extract variables from an f-string template
///
/// Finds all {variable} patterns in the template.
#[must_use]
pub fn extract_fstring_variables(template: &str) -> Vec<String> {
    let re = regex::Regex::new(r"\{([^{}]+)\}").expect("static fstring variable regex pattern");
    let mut variables = Vec::new();

    for cap in re.captures_iter(template) {
        if let Some(var) = cap.get(1) {
            let var_name = var.as_str();
            // Skip format specifiers like {name:10} -> just "name"
            let clean_var = var_name.split(':').next().unwrap_or(var_name);
            if !clean_var.is_empty() && !variables.contains(&clean_var.to_string()) {
                variables.push(clean_var.to_string());
            }
        }
    }

    variables
}

/// Format an f-string template with variables
pub fn format_fstring(template: &str, variables: &HashMap<String, String>) -> Result<String> {
    // Fast path: use simple string replacement when no format specifiers are present
    // This is ~100x faster than regex for common cases
    let mut result = String::with_capacity(template.len());
    let mut remaining = template;

    while let Some(start) = remaining.find('{') {
        // Append everything before the '{'
        result.push_str(&remaining[..start]);
        remaining = &remaining[start..];

        // Find the closing '}'
        if let Some(end) = remaining.find('}') {
            let placeholder = &remaining[1..end];

            // Check for format specifier (e.g., "key:10")
            let (key, _format_spec) = if let Some(colon_pos) = placeholder.find(':') {
                (
                    &placeholder[..colon_pos],
                    Some(&placeholder[colon_pos + 1..]),
                )
            } else {
                (placeholder, None)
            };

            // Replace with variable value
            if let Some(value) = variables.get(key) {
                result.push_str(value);
            } else {
                // If variable not found, keep the placeholder
                result.push_str(&remaining[..=end]);
            }

            // Move past the '}'
            remaining = &remaining[end + 1..];
        } else {
            // No closing brace, treat as literal
            result.push('{');
            remaining = &remaining[1..];
        }
    }

    // Append any remaining text
    result.push_str(remaining);

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::{extract_fstring_variables, format_fstring};
    use crate::test_prelude::*;

    #[test]
    fn test_extract_fstring_variables() {
        let template = "Hello {name}, you are {age} years old";
        let vars = extract_fstring_variables(template);
        assert_eq!(vars, vec!["name", "age"]);
    }

    #[test]
    fn test_extract_fstring_variables_with_format_spec() {
        let template = "Value: {value:10.2f}";
        let vars = extract_fstring_variables(template);
        assert_eq!(vars, vec!["value"]);
    }

    #[test]
    fn test_extract_fstring_variables_dedupe() {
        let template = "Hello {name}, {name}!";
        let vars = extract_fstring_variables(template);
        assert_eq!(vars, vec!["name"]);
    }

    #[test]
    fn test_format_fstring() {
        let template = "Hello {name}, you are {age} years old";
        let mut vars = HashMap::new();
        vars.insert("name".to_string(), "Alice".to_string());
        vars.insert("age".to_string(), "30".to_string());

        let result = format_fstring(template, &vars).unwrap();
        assert_eq!(result, "Hello Alice, you are 30 years old");
    }

    #[test]
    fn test_format_fstring_with_format_spec() {
        let template = "Value: {value:10.2f}";
        let mut vars = HashMap::new();
        vars.insert("value".to_string(), "42.7".to_string());

        let result = format_fstring(template, &vars).unwrap();
        // Format spec is replaced with plain value in our implementation
        assert_eq!(result, "Value: 42.7");
    }

    #[test]
    fn test_format_fstring_repeated() {
        let template = "Hello {name}, nice to meet you {name}!";
        let mut vars = HashMap::new();
        vars.insert("name".to_string(), "Bob".to_string());

        let result = format_fstring(template, &vars).unwrap();
        assert_eq!(result, "Hello Bob, nice to meet you Bob!");
    }
}
