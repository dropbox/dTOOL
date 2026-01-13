//! Content Moderation Chain using `OpenAI`'s Moderation API
//!
//! This module provides a chain that checks text for potentially harmful content
//! using `OpenAI`'s moderation endpoint. It can detect various categories of unsafe
//! content including hate speech, self-harm, sexual content, and violence.
//!
//! # Example
//!
//! ```rust,ignore
//! use dashflow_chains::OpenAIModerationChain;
//! use std::env;
//!
//! // Create moderation chain
//! let api_key = env::var("OPENAI_API_KEY").expect("OPENAI_API_KEY not set");
//! let chain = OpenAIModerationChain::new(api_key)
//!     .with_error_on_violation(true); // Error if content is flagged
//!
//! // Check content
//! let result = chain.moderate("Some text to check").await?;
//! println!("Moderation result: {}", result);
//! ```

use async_openai::config::OpenAIConfig;
use async_openai::types::{CreateModerationRequestArgs, ModerationInput, TextModerationModel};
use async_openai::Client;
use dashflow::core::config_loader::env_vars::{env_string, OPENAI_API_KEY};
use dashflow::core::error::{Error, Result};
use std::collections::HashMap;

/// Chain that checks text for harmful content using `OpenAI`'s Moderation API.
///
/// This chain sends text through `OpenAI`'s content moderation endpoint which
/// detects various types of potentially harmful content:
///
/// - Hate speech (including threatening hate)
/// - Harassment (including threatening harassment)
/// - Self-harm content (intent and instructions)
/// - Sexual content (including minors)
/// - Violence (including graphic violence)
///
/// # Configuration
///
/// - `error_on_violation`: If true, returns an error when content is flagged (default: false)
/// - `model`: Choose between "latest" (default) or "stable" moderation models
/// - `input_key`/`output_key`: Keys for input/output in HashMap-based API
///
/// # Example
///
/// ```rust,ignore
/// let chain = OpenAIModerationChain::new(api_key)
///     .with_model("stable")
///     .with_error_on_violation(false);
///
/// // Returns original text if OK, or error message if flagged
/// let result = chain.moderate("Hello world").await?;
/// ```
pub struct OpenAIModerationChain {
    client: Client<OpenAIConfig>,
    model: TextModerationModel,
    error_on_violation: bool,
    input_key: String,
    output_key: String,
}

impl OpenAIModerationChain {
    /// Create a new moderation chain with the given API key.
    ///
    /// # Arguments
    ///
    /// * `api_key` - `OpenAI` API key
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let chain = OpenAIModerationChain::new("sk-...".to_string());
    /// ```
    #[must_use]
    pub fn new(api_key: String) -> Self {
        let config = OpenAIConfig::new().with_api_key(api_key);
        let client = Client::with_config(config);

        Self {
            client,
            model: TextModerationModel::Latest,
            error_on_violation: false,
            input_key: "input".to_string(),
            output_key: "output".to_string(),
        }
    }

    /// Create from environment variable `OPENAI_API_KEY`.
    ///
    /// # Errors
    ///
    /// Returns an error if `OPENAI_API_KEY` is not set.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let chain = OpenAIModerationChain::from_env()?;
    /// ```
    pub fn from_env() -> Result<Self> {
        let api_key = env_string(OPENAI_API_KEY).ok_or_else(|| {
            Error::InvalidInput(
                "OPENAI_API_KEY environment variable not set. \
                 Please set it or use OpenAIModerationChain::new() with an explicit API key."
                    .to_string(),
            )
        })?;
        Ok(Self::new(api_key))
    }

    /// Set the moderation model to use.
    ///
    /// Options:
    /// - "latest" (default): Automatically upgraded over time for best accuracy
    /// - "stable": Receives advance notice before updates, slightly lower accuracy
    ///
    /// # Arguments
    ///
    /// * `model` - Model name ("latest" or "stable")
    #[must_use]
    pub fn with_model(mut self, model: &str) -> Self {
        self.model = match model {
            "stable" => TextModerationModel::Stable,
            _ => TextModerationModel::Latest,
        };
        self
    }

    /// Set whether to return an error when content is flagged.
    ///
    /// If true, the chain will return an error when harmful content is detected.
    /// If false (default), it returns an error message string.
    ///
    /// # Arguments
    ///
    /// * `error` - Whether to error on violations
    #[must_use]
    pub fn with_error_on_violation(mut self, error: bool) -> Self {
        self.error_on_violation = error;
        self
    }

    /// Set the input key for HashMap-based API.
    pub fn with_input_key(mut self, key: impl Into<String>) -> Self {
        self.input_key = key.into();
        self
    }

    /// Set the output key for HashMap-based API.
    pub fn with_output_key(mut self, key: impl Into<String>) -> Self {
        self.output_key = key.into();
        self
    }

    /// Get the input key.
    #[must_use]
    pub fn input_key(&self) -> &str {
        &self.input_key
    }

    /// Get the output key.
    #[must_use]
    pub fn output_key(&self) -> &str {
        &self.output_key
    }

    /// Check if text contains harmful content.
    ///
    /// Returns the original text if safe, or an error message if flagged.
    ///
    /// # Arguments
    ///
    /// * `text` - Text to moderate
    ///
    /// # Returns
    ///
    /// - If safe: Returns the original text unchanged
    /// - If flagged and `error_on_violation` is false: Returns error message string
    /// - If flagged and `error_on_violation` is true: Returns Err
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The API request fails
    /// - Content is flagged and `error_on_violation` is true
    pub async fn moderate(&self, text: &str) -> Result<String> {
        let request = CreateModerationRequestArgs::default()
            .input(ModerationInput::String(text.to_string()))
            .model(self.model)
            .build()
            .map_err(|e| Error::Other(format!("Failed to build moderation request: {e}")))?;

        let response = self
            .client
            .moderations()
            .create(request)
            .await
            .map_err(|e| Error::Other(format!("OpenAI moderation API error: {e}")))?;

        // Check first result (we only sent one input)
        if let Some(result) = response.results.first() {
            if result.flagged {
                let error_msg = "Text was found that violates OpenAI's content policy.";
                if self.error_on_violation {
                    return Err(Error::Other(error_msg.to_string()));
                }
                return Ok(error_msg.to_string());
            }
        }

        Ok(text.to_string())
    }

    /// Run the chain with `HashMap` inputs.
    ///
    /// This is the standard chain interface compatible with other `DashFlow` chains.
    ///
    /// # Arguments
    ///
    /// * `inputs` - Input map containing the text to moderate under `input_key`
    ///
    /// # Returns
    ///
    /// Output map with result under `output_key`
    pub async fn run(&self, inputs: &HashMap<String, String>) -> Result<HashMap<String, String>> {
        let text = inputs.get(&self.input_key).ok_or_else(|| {
            Error::InvalidInput(format!("Missing required input key: {}", self.input_key))
        })?;

        let output = self.moderate(text).await?;

        let mut result = HashMap::new();
        result.insert(self.output_key.clone(), output);
        Ok(result)
    }

    /// Check multiple texts in a single API call (more efficient).
    ///
    /// # Arguments
    ///
    /// * `texts` - Multiple texts to check
    ///
    /// # Returns
    ///
    /// Vector of results, one per input text
    pub async fn moderate_batch(&self, texts: &[String]) -> Result<Vec<String>> {
        if texts.is_empty() {
            return Ok(Vec::new());
        }

        let request = CreateModerationRequestArgs::default()
            .input(ModerationInput::StringArray(texts.to_vec()))
            .model(self.model)
            .build()
            .map_err(|e| Error::Other(format!("Failed to build moderation request: {e}")))?;

        let response = self
            .client
            .moderations()
            .create(request)
            .await
            .map_err(|e| Error::Other(format!("OpenAI moderation API error: {e}")))?;

        let mut results = Vec::new();
        for (i, result) in response.results.iter().enumerate() {
            if result.flagged {
                let error_msg = "Text was found that violates OpenAI's content policy.";
                if self.error_on_violation {
                    return Err(Error::Other(format!("Text {i} flagged: {error_msg}")));
                }
                results.push(error_msg.to_string());
            } else {
                results.push(texts[i].clone());
            }
        }

        Ok(results)
    }
}

impl std::fmt::Debug for OpenAIModerationChain {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OpenAIModerationChain")
            .field("model", &self.model)
            .field("error_on_violation", &self.error_on_violation)
            .field("input_key", &self.input_key)
            .field("output_key", &self.output_key)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chain_construction() {
        let chain = OpenAIModerationChain::new("test-key".to_string())
            .with_model("stable")
            .with_error_on_violation(true)
            .with_input_key("text")
            .with_output_key("result");

        assert_eq!(chain.model, TextModerationModel::Stable);
        assert!(chain.error_on_violation);
        assert_eq!(chain.input_key(), "text");
        assert_eq!(chain.output_key(), "result");
    }

    #[test]
    fn test_default_keys() {
        let chain = OpenAIModerationChain::new("test-key".to_string());
        assert_eq!(chain.input_key(), "input");
        assert_eq!(chain.output_key(), "output");
    }

    #[tokio::test]
    async fn test_run_missing_input_key() {
        let chain = OpenAIModerationChain::new("test-key".to_string());
        let inputs = HashMap::new(); // Missing "input" key

        let result = chain.run(&inputs).await;
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Missing required input"));
    }

    // Integration tests require real API key
    #[tokio::test]
    #[ignore = "requires OPENAI_API_KEY"]
    async fn test_moderation_safe_content() {
        let chain =
            OpenAIModerationChain::from_env().expect("OPENAI_API_KEY must be set for this test");

        let result = chain.moderate("Hello, how are you today?").await.unwrap();
        assert_eq!(result, "Hello, how are you today?");
    }

    #[tokio::test]
    #[ignore = "requires OPENAI_API_KEY"]
    async fn test_moderation_with_hashmap() {
        let chain =
            OpenAIModerationChain::from_env().expect("OPENAI_API_KEY must be set for this test");

        let mut inputs = HashMap::new();
        inputs.insert("input".to_string(), "Hello world".to_string());

        let result = chain.run(&inputs).await.unwrap();
        assert_eq!(result.get("output").unwrap(), "Hello world");
    }

    #[tokio::test]
    #[ignore = "requires OPENAI_API_KEY"]
    async fn test_moderation_batch() {
        let chain =
            OpenAIModerationChain::from_env().expect("OPENAI_API_KEY must be set for this test");

        let texts = vec![
            "Hello world".to_string(),
            "How are you?".to_string(),
            "Nice weather today".to_string(),
        ];

        let results = chain.moderate_batch(&texts).await.unwrap();
        assert_eq!(results.len(), 3);
        assert_eq!(results[0], "Hello world");
        assert_eq!(results[1], "How are you?");
        assert_eq!(results[2], "Nice weather today");
    }
}
