//! Utility functions for memory implementations.

use dashflow::core::error::{Error, Result};
use std::collections::HashMap;

/// Get the prompt input key from inputs, excluding memory variables.
///
/// Returns the single input key that should be used for the prompt,
/// after excluding memory variables and special keys like "stop".
///
/// # Arguments
///
/// * `inputs` - The input dictionary
/// * `memory_variables` - List of memory variable keys to exclude
///
/// # Returns
///
/// The single prompt input key
///
/// # Errors
///
/// Returns an error if there is not exactly one prompt input key.
///
/// # Python Baseline Compatibility
///
/// Matches `dashflow.memory.utils.get_prompt_input_key`.
/// Source: ~/`dashflow/libs/dashflow/dashflow_classic/memory/utils.py` (lines 4-20)
pub fn get_prompt_input_key(
    inputs: &HashMap<String, String>,
    memory_variables: &[String],
) -> Result<String> {
    // Build set of keys to exclude
    let mut exclude = memory_variables.to_vec();
    exclude.push("stop".to_string()); // "stop" is a special key

    let exclude_set: std::collections::HashSet<_> = exclude.into_iter().collect();

    // Get input keys that aren't in the exclude set
    let prompt_input_keys: Vec<_> = inputs
        .keys()
        .filter(|k| !exclude_set.contains(k.as_str()))
        .cloned()
        .collect();

    // Must have exactly one prompt input key
    if prompt_input_keys.len() != 1 {
        return Err(Error::config(format!(
            "One input key expected, got {prompt_input_keys:?}"
        )));
    }

    Ok(prompt_input_keys[0].clone())
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_get_prompt_input_key_single() {
        let mut inputs = HashMap::new();
        inputs.insert("question".to_string(), "What is Rust?".to_string());
        inputs.insert("history".to_string(), "Previous conversation".to_string());

        let memory_vars = vec!["history".to_string()];
        let key = get_prompt_input_key(&inputs, &memory_vars).unwrap();

        assert_eq!(key, "question");
    }

    #[test]
    fn test_get_prompt_input_key_with_stop() {
        let mut inputs = HashMap::new();
        inputs.insert("input".to_string(), "Hello".to_string());
        inputs.insert("stop".to_string(), "\n".to_string());

        let key = get_prompt_input_key(&inputs, &[]).unwrap();

        // "stop" is excluded, so "input" is the only key
        assert_eq!(key, "input");
    }

    #[test]
    fn test_get_prompt_input_key_multiple_error() {
        let mut inputs = HashMap::new();
        inputs.insert("input1".to_string(), "Hello".to_string());
        inputs.insert("input2".to_string(), "World".to_string());

        let result = get_prompt_input_key(&inputs, &[]);

        // Should error because there are multiple prompt input keys
        assert!(result.is_err());
    }

    #[test]
    fn test_get_prompt_input_key_none_error() {
        let mut inputs = HashMap::new();
        inputs.insert("history".to_string(), "Previous conversation".to_string());

        let memory_vars = vec!["history".to_string()];
        let result = get_prompt_input_key(&inputs, &memory_vars);

        // Should error because there are no prompt input keys
        assert!(result.is_err());
    }
}
