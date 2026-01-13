// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

//! Aggregation functions for combining multiple values or outputs.
//!
//! This module provides functions for aggregating multiple JSON values into a single result.
//! Useful for combining outputs from multiple model calls, ensemble predictions, or
//! iterative refinement attempts.
//!
//! # Examples
//!
//! ```
//! use dashflow::optimize::aggregation::majority;
//! use serde_json::json;
//!
//! // Create multiple values with the same field
//! let values = vec![
//!     json!({"answer": "Paris"}),
//!     json!({"answer": "Paris"}),
//!     json!({"answer": "London"}),
//!     json!({"answer": "Paris"}),
//! ];
//!
//! // Get the majority vote
//! let result = majority(&values, None, Some("answer")).unwrap();
//! assert_eq!(result.get("answer").and_then(|v| v.as_str()), Some("Paris"));
//! ```

use crate::optimize::metrics::normalize_text;
use crate::{Error, Result};
use serde_json::Value;
use std::collections::HashMap;

/// Default normalization function for text comparison.
///
/// Uses the same normalization as DashOpt evaluate metrics:
/// - Unicode NFD normalization
/// - Lowercasing
/// - Punctuation removal
/// - English article removal ("a", "an", "the")
/// - Whitespace collapse
///
/// Returns None if the normalized text is empty.
///
/// # Examples
///
/// ```
/// use dashflow::optimize::aggregation::default_normalize;
/// assert_eq!(default_normalize("The,  Eiffel  Tower!"), Some("eiffel tower".to_string()));
/// assert_eq!(default_normalize(""), None);
/// ```
pub fn default_normalize(s: &str) -> Option<String> {
    let normalized = normalize_text(s);
    if normalized.is_empty() {
        None
    } else {
        Some(normalized)
    }
}

/// Returns the most common value for the target field across multiple values.
///
/// This function implements majority voting across multiple JSON values:
/// 1. Extracts values from the specified field in each value
/// 2. Normalizes values (if normalize function provided)
/// 3. Counts occurrences of each value
/// 4. Returns the value containing the most common field value
///
/// # Arguments
///
/// * `values` - Slice of JSON values to aggregate
/// * `normalize` - Optional normalization function. When it returns None, that value is ignored.
/// * `field` - Optional field name to vote on. If None, uses the last field in the first value.
///
/// # Note on Default Field Selection (M-867)
///
/// When `field` is None, this function selects the "last" field from the first value's
/// object based on serde_json's map iteration order. Since serde_json::Map preserves
/// insertion order (like IndexMap), the "last" field depends on JSON parse order:
///
/// - For `{"a": 1, "b": 2}`, "b" would be selected (last key in JSON)
/// - But insertion order isn't always intuitive from the logical structure
///
/// **Recommendation**: Always specify the `field` parameter explicitly for predictable
/// behavior. Only rely on default field selection when the value schema is well-known
/// and consistent.
///
/// # Returns
///
/// A single JSON value containing the most common field value. In case of a tie,
/// earlier values are prioritized.
///
/// # Errors
///
/// Returns an error if:
/// - The values slice is empty
/// - The field does not exist in values
/// - The field value cannot be converted to a string
///
/// # Examples
///
/// ```
/// use dashflow::optimize::aggregation::{majority, default_normalize};
/// use serde_json::json;
///
/// // Simple majority vote
/// let values = vec![
///     json!({"answer": "Paris"}),
///     json!({"answer": "paris"}),  // Will normalize to "paris"
///     json!({"answer": "London"}),
/// ];
///
/// let result = majority(&values, Some(default_normalize), Some("answer")).unwrap();
/// // "Paris" wins (2 votes after normalization vs 1 for "London")
/// assert_eq!(result.get("answer").and_then(|v| v.as_str()), Some("Paris"));
/// ```
///
/// ```
/// use dashflow::optimize::aggregation::majority;
/// use serde_json::json;
///
/// // Vote on specific field
/// let values = vec![
///     json!({"answer": "Paris", "confidence": "high"}),
///     json!({"answer": "London", "confidence": "low"}),
///     json!({"answer": "Paris", "confidence": "medium"}),
/// ];
///
/// let result = majority(&values, None, Some("answer")).unwrap();
/// assert_eq!(result.get("answer").and_then(|v| v.as_str()), Some("Paris"));
/// ```
pub fn majority(
    values: &[Value],
    normalize: Option<fn(&str) -> Option<String>>,
    field: Option<&str>,
) -> Result<Value> {
    if values.is_empty() {
        return Err(Error::Validation(
            "Cannot compute majority on empty values".to_string(),
        ));
    }

    // Determine which field to vote on
    let field_name = if let Some(f) = field {
        f.to_string()
    } else {
        // LIMITATION (M-867): Auto-select "last" field based on serde_json map order
        // serde_json::Map preserves insertion order (IndexMap), so "last" depends on
        // the order fields appear in the JSON source. This may be surprising if users
        // expect alphabetical or logical ordering. See doc comment for recommendation
        // to always specify field explicitly.
        if let Some(obj) = values[0].as_object() {
            let auto_field = obj
                .keys()
                .next_back()
                .ok_or_else(|| Error::Validation("First value has no fields".to_string()))?
                .to_string();
            tracing::debug!(
                auto_selected_field = %auto_field,
                available_fields = ?obj.keys().collect::<Vec<_>>(),
                "No field specified - auto-selected last field from first value (consider specifying field explicitly)"
            );
            auto_field
        } else {
            return Err(Error::Validation(
                "First value is not an object".to_string(),
            ));
        }
    };

    // Extract and normalize values
    let mut values_with_indices: Vec<(String, usize)> = Vec::new();

    for (idx, value) in values.iter().enumerate() {
        let field_value = value
            .get(&field_name)
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                Error::Validation(format!(
                    "Field '{}' not found or not a string in value at index {}",
                    field_name, idx
                ))
            })?;

        let normalized_value = if let Some(norm_fn) = normalize {
            match norm_fn(field_value) {
                Some(normalized) => normalized,
                None => continue, // Skip None values from normalization
            }
        } else {
            field_value.to_string()
        };

        values_with_indices.push((normalized_value, idx));
    }

    if values_with_indices.is_empty() {
        return Err(Error::Validation(
            "All values normalized to None".to_string(),
        ));
    }

    // Count occurrences and track first index for each value
    let mut value_counts: HashMap<&str, (usize, usize)> = HashMap::new(); // (count, first_index)
    for (value, idx) in &values_with_indices {
        value_counts
            .entry(value.as_str())
            .and_modify(|(count, _)| *count += 1)
            .or_insert((1, *idx));
    }

    // Find majority value (value with max count, ties broken by first occurrence)
    let (majority_idx, _) = value_counts
        .values()
        .max_by_key(|(count, first_idx)| (*count, std::cmp::Reverse(*first_idx)))
        .map(|(_, first_idx)| (first_idx, ()))
        .ok_or_else(|| Error::Validation("No values to aggregate".to_string()))?;

    Ok(values[*majority_idx].clone())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_default_normalize() {
        assert_eq!(
            default_normalize("The,  Eiffel  Tower!"),
            Some("eiffel tower".to_string())
        );
        assert_eq!(default_normalize("An apple"), Some("apple".to_string()));
        assert_eq!(default_normalize("A test"), Some("test".to_string()));
        assert_eq!(
            default_normalize("hello   world"),
            Some("hello world".to_string())
        );

        // Empty after normalization
        assert_eq!(default_normalize(""), None);
        assert_eq!(default_normalize("   "), None);
        assert_eq!(default_normalize("a an the"), None);
    }

    #[test]
    fn test_majority_simple() {
        let values = vec![
            json!({"answer": "Paris"}),
            json!({"answer": "Paris"}),
            json!({"answer": "London"}),
        ];

        let result = majority(&values, None, Some("answer")).unwrap();
        assert_eq!(result.get("answer").and_then(|v| v.as_str()), Some("Paris"));
    }

    #[test]
    fn test_majority_with_normalization() {
        let values = vec![
            json!({"answer": "The Paris"}),
            json!({"answer": "paris"}),
            json!({"answer": "PARIS!"}),
            json!({"answer": "London"}),
        ];

        let result = majority(&values, Some(default_normalize), Some("answer")).unwrap();
        // All "Paris" variants normalize to "paris" (3 votes vs 1 for "London")
        assert_eq!(
            result.get("answer").and_then(|v| v.as_str()),
            Some("The Paris")
        ); // Returns first matching value
    }

    #[test]
    fn test_majority_specific_field() {
        let values = vec![
            json!({"answer": "A", "confidence": "high"}),
            json!({"answer": "B", "confidence": "low"}),
            json!({"answer": "A", "confidence": "high"}),
        ];

        // Vote on answer field
        let result = majority(&values, None, Some("answer")).unwrap();
        assert_eq!(result.get("answer").and_then(|v| v.as_str()), Some("A"));

        // Vote on confidence field
        let result = majority(&values, None, Some("confidence")).unwrap();
        assert_eq!(
            result.get("confidence").and_then(|v| v.as_str()),
            Some("high")
        );
    }

    #[test]
    fn test_majority_tie_prefers_earlier() {
        let values = vec![json!({"answer": "First"}), json!({"answer": "Second"})];

        let result = majority(&values, None, Some("answer")).unwrap();
        // In case of tie, first value wins
        assert_eq!(result.get("answer").and_then(|v| v.as_str()), Some("First"));
    }

    #[test]
    fn test_majority_empty_values() {
        let values: Vec<Value> = vec![];
        let result = majority(&values, None, Some("answer"));
        assert!(result.is_err());
    }

    #[test]
    fn test_majority_missing_field() {
        let values = vec![
            json!({"answer": "Paris"}),
            json!({"different_field": "London"}),
        ];

        let result = majority(&values, None, Some("answer"));
        // Should error because second value doesn't have "answer" field
        assert!(result.is_err());
    }

    #[test]
    fn test_majority_uses_last_field_by_default() {
        let values = vec![
            json!({"question": "What?", "answer": "Paris"}),
            json!({"question": "What?", "answer": "Paris"}),
            json!({"question": "What?", "answer": "London"}),
        ];

        // Should use "answer" (last field)
        let result = majority(&values, None, None).unwrap();
        assert_eq!(result.get("answer").and_then(|v| v.as_str()), Some("Paris"));
    }

    #[test]
    fn test_majority_with_normalize_filtering_none() {
        // Custom normalize that filters out certain values
        fn filter_normalize(s: &str) -> Option<String> {
            if s == "IGNORE" {
                None
            } else {
                Some(s.to_lowercase())
            }
        }

        let values = vec![
            json!({"answer": "IGNORE"}),
            json!({"answer": "Paris"}),
            json!({"answer": "paris"}),
        ];

        let result = majority(&values, Some(filter_normalize), Some("answer")).unwrap();
        // "IGNORE" filtered out, "Paris" and "paris" both normalize to "paris"
        assert_eq!(result.get("answer").and_then(|v| v.as_str()), Some("Paris"));
        // First matching value
    }

    #[test]
    fn test_majority_all_filtered_out() {
        fn reject_all(_s: &str) -> Option<String> {
            None
        }

        let values = vec![json!({"answer": "Paris"}), json!({"answer": "London"})];

        let result = majority(&values, Some(reject_all), Some("answer"));
        // All values filtered out
        assert!(result.is_err());
    }

    #[test]
    fn test_majority_non_object_value() {
        let values = vec![json!("not an object")];
        let result = majority(&values, None, None);
        assert!(result.is_err());
    }

    #[test]
    fn test_majority_field_not_string() {
        let values = vec![json!({"answer": 42}), json!({"answer": 42})];

        let result = majority(&values, None, Some("answer"));
        // Should error because field is not a string
        assert!(result.is_err());
    }
}
