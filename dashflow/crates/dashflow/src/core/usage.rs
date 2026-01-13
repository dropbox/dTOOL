//! Usage metadata types for tracking token counts and costs

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Breakdown of input token counts
///
/// Does not need to sum to full input token count. Does not need to have all keys.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct InputTokenDetails {
    /// Audio input tokens
    #[serde(skip_serializing_if = "Option::is_none")]
    pub audio: Option<u32>,

    /// Input tokens that were cached (cache miss - cache was created)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_creation: Option<u32>,

    /// Input tokens that were cached (cache hit - read from cache)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_read: Option<u32>,

    /// Extra provider-specific keys
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

/// Breakdown of output token counts
///
/// Does not need to sum to full output token count. Does not need to have all keys.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct OutputTokenDetails {
    /// Audio output tokens
    #[serde(skip_serializing_if = "Option::is_none")]
    pub audio: Option<u32>,

    /// Reasoning output tokens (e.g., from `OpenAI`'s o1 models)
    ///
    /// Tokens generated in a chain of thought process that are not
    /// returned as part of model output.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning: Option<u32>,

    /// Extra provider-specific keys
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

/// Token usage metadata for LLM calls
///
/// Standard usages metadata for LLM responses.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct UsageMetadata {
    /// Count of input (prompt) tokens
    pub input_tokens: u32,

    /// Count of output (completion) tokens
    pub output_tokens: u32,

    /// Total token count (input + output)
    pub total_tokens: u32,

    /// Detailed breakdown of input token counts
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input_token_details: Option<InputTokenDetails>,

    /// Detailed breakdown of output token counts
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_token_details: Option<OutputTokenDetails>,
}

impl UsageMetadata {
    /// Create new usage metadata
    #[must_use]
    pub fn new(input_tokens: u32, output_tokens: u32) -> Self {
        Self {
            input_tokens,
            output_tokens,
            total_tokens: input_tokens + output_tokens,
            input_token_details: None,
            output_token_details: None,
        }
    }

    /// Create with detailed token breakdowns
    #[must_use]
    pub fn with_details(
        input_tokens: u32,
        output_tokens: u32,
        input_details: Option<InputTokenDetails>,
        output_details: Option<OutputTokenDetails>,
    ) -> Self {
        Self {
            input_tokens,
            output_tokens,
            total_tokens: input_tokens + output_tokens,
            input_token_details: input_details,
            output_token_details: output_details,
        }
    }

    /// Add (merge) another usage metadata
    ///
    /// Sums all token counts.
    #[must_use]
    pub fn add(&self, other: &UsageMetadata) -> Self {
        Self {
            input_tokens: self.input_tokens + other.input_tokens,
            output_tokens: self.output_tokens + other.output_tokens,
            total_tokens: self.total_tokens + other.total_tokens,
            input_token_details: None, // Details are not merged
            output_token_details: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{InputTokenDetails, OutputTokenDetails};
    use crate::test_prelude::*;

    #[test]
    fn test_usage_metadata_new() {
        let usage = UsageMetadata::new(100, 50);
        assert_eq!(usage.input_tokens, 100);
        assert_eq!(usage.output_tokens, 50);
        assert_eq!(usage.total_tokens, 150);
    }

    #[test]
    fn test_usage_metadata_add() {
        let usage1 = UsageMetadata::new(100, 50);
        let usage2 = UsageMetadata::new(200, 100);
        let combined = usage1.add(&usage2);

        assert_eq!(combined.input_tokens, 300);
        assert_eq!(combined.output_tokens, 150);
        assert_eq!(combined.total_tokens, 450);
    }

    #[test]
    fn test_usage_metadata_serialization() {
        let usage = UsageMetadata::new(100, 50);
        let json = serde_json::to_string(&usage).unwrap();
        let deserialized: UsageMetadata = serde_json::from_str(&json).unwrap();

        assert_eq!(usage, deserialized);
    }

    #[test]
    fn test_input_token_details() {
        let details = InputTokenDetails {
            cache_read: Some(50),
            cache_creation: Some(100),
            ..Default::default()
        };

        let json = serde_json::to_string(&details).unwrap();
        let deserialized: InputTokenDetails = serde_json::from_str(&json).unwrap();

        assert_eq!(details, deserialized);
        assert_eq!(deserialized.cache_read, Some(50));
        assert_eq!(deserialized.cache_creation, Some(100));
    }

    #[test]
    fn test_output_token_details() {
        let details = OutputTokenDetails {
            reasoning: Some(200),
            audio: Some(10),
            ..Default::default()
        };

        let json = serde_json::to_string(&details).unwrap();
        let deserialized: OutputTokenDetails = serde_json::from_str(&json).unwrap();

        assert_eq!(details, deserialized);
        assert_eq!(deserialized.reasoning, Some(200));
        assert_eq!(deserialized.audio, Some(10));
    }
}
