//! # LLM Requests Chain
//!
//! Chain that fetches content from a URL and uses an LLM to process the results.
//!
//! **Security Note**: This chain can make GET requests to arbitrary URLs,
//! including internal URLs. Control access to who can run this chain and what
//! network access this chain has.
//!
//! ## Example
//!
//! ```rust,no_run
//! use dashflow_chains::{LLMRequestsChain, LLMChain};
//! use dashflow::core::prompts::PromptTemplate;
//! use std::collections::HashMap;
//!
//! #[tokio::main]
//! async fn main() {
//!     // Create an LLM chain with a prompt that uses the extracted content
//!     let prompt = PromptTemplate::from_template(
//!         "Answer the question based on this content:\n\n{requests_result}\n\nQuestion: {question}"
//!     ).unwrap();
//!     // let llm = ...; // your LLM
//!     // let llm_chain = LLMChain::new(llm, prompt);
//!
//!     // Create the LLMRequestsChain
//!     // let chain = LLMRequestsChain::new(llm_chain);
//!
//!     // Use the chain
//!     // let mut inputs = HashMap::new();
//!     // inputs.insert("url".to_string(), "https://example.com".to_string());
//!     // inputs.insert("question".to_string(), "What is this page about?".to_string());
//!     // let result = chain.run(&inputs).await.unwrap();
//! }
//! ```

use crate::LLMChain;
use dashflow::constants::{
    DEFAULT_HTTP_CONNECT_TIMEOUT, DEFAULT_POOL_IDLE_TIMEOUT, DEFAULT_POOL_MAX_IDLE_PER_HOST,
    DEFAULT_TCP_KEEPALIVE,
};
use dashflow::core::error::{Error, Result};
use reqwest::header::{HeaderMap, HeaderValue, USER_AGENT};
use scraper::Html;
use std::collections::HashMap;
use std::sync::Arc;

const DEFAULT_USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/87.0.4280.88 Safari/537.36";

/// Chain that requests a URL and then uses an LLM to parse results.
///
/// This chain:
/// 1. Fetches content from a URL via HTTP GET
/// 2. Parses HTML and extracts text content
/// 3. Limits text to `text_length` characters
/// 4. Passes extracted text to an LLM chain for processing
/// 5. Returns the LLM's response
///
/// # Security Note
///
/// This chain can make GET requests to arbitrary URLs, including internal URLs.
/// Control access to who can run this chain and what network access it has.
///
/// # Input Keys
///
/// - `url`: The URL to fetch (default key name, configurable via `input_key`)
/// - Additional keys are passed through to the LLM chain (e.g., `question`)
///
/// # Output Keys
///
/// - `output`: The LLM's response (default key name, configurable via `output_key`)
///
/// # Example
///
/// ```rust,no_run
/// use dashflow_chains::{LLMRequestsChain, LLMChain};
/// use dashflow::core::prompts::PromptTemplate;
/// use std::collections::HashMap;
///
/// # async fn example() {
/// // Create prompt that expects {requests_result} and {question}
/// let prompt = PromptTemplate::from_template(
///     "Answer based on this content:\n\n{requests_result}\n\nQuestion: {question}"
/// ).unwrap();
/// // let llm = ...; // your LLM
/// // let llm_chain = LLMChain::new(llm, prompt);
/// // let chain = LLMRequestsChain::new(llm_chain);
///
/// let mut inputs = HashMap::new();
/// inputs.insert("url".to_string(), "https://example.com".to_string());
/// inputs.insert("question".to_string(), "What is this page about?".to_string());
/// // let result = chain.run(&inputs).await.unwrap();
/// # }
/// ```
pub struct LLMRequestsChain<M> {
    /// The LLM chain to use for processing the extracted content
    llm_chain: Arc<LLMChain<M>>,

    /// Maximum length of text to extract from the page (default: 8000)
    text_length: usize,

    /// Key name for the extracted content passed to LLM chain (default: "`requests_result`")
    requests_key: String,

    /// Input key name for the URL (default: "url")
    input_key: String,

    /// Output key name for the result (default: "output")
    output_key: String,

    /// Optional custom headers for HTTP requests
    custom_headers: Option<HeaderMap>,
}

impl<M> LLMRequestsChain<M> {
    /// Create a new `LLMRequestsChain` with default settings.
    ///
    /// # Arguments
    ///
    /// * `llm_chain` - The LLM chain to use for processing the extracted content
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use dashflow_chains::{LLMRequestsChain, LLMChain};
    /// # async fn example() {
    /// // let llm_chain = ...; // your LLM chain
    /// // let chain = LLMRequestsChain::new(llm_chain);
    /// # }
    /// ```
    #[must_use]
    pub fn new(llm_chain: LLMChain<M>) -> Self {
        Self {
            llm_chain: Arc::new(llm_chain),
            text_length: 8000,
            requests_key: "requests_result".to_string(),
            input_key: "url".to_string(),
            output_key: "output".to_string(),
            custom_headers: None,
        }
    }

    /// Set the maximum text length to extract from the page.
    ///
    /// # Arguments
    ///
    /// * `text_length` - Maximum characters to extract (default: 8000)
    #[must_use]
    pub fn with_text_length(mut self, text_length: usize) -> Self {
        self.text_length = text_length;
        self
    }

    /// Set the key name for the extracted content in the LLM chain prompt.
    ///
    /// # Arguments
    ///
    /// * `key` - The key name (default: "`requests_result`")
    pub fn with_requests_key(mut self, key: impl Into<String>) -> Self {
        self.requests_key = key.into();
        self
    }

    /// Set the input key name for the URL.
    ///
    /// # Arguments
    ///
    /// * `key` - The input key name (default: "url")
    pub fn with_input_key(mut self, key: impl Into<String>) -> Self {
        self.input_key = key.into();
        self
    }

    /// Set the output key name for the result.
    ///
    /// # Arguments
    ///
    /// * `key` - The output key name (default: "output")
    pub fn with_output_key(mut self, key: impl Into<String>) -> Self {
        self.output_key = key.into();
        self
    }

    /// Set custom headers for HTTP requests.
    ///
    /// # Arguments
    ///
    /// * `headers` - Custom HTTP headers
    #[must_use]
    pub fn with_headers(mut self, headers: HeaderMap) -> Self {
        self.custom_headers = Some(headers);
        self
    }

    /// Get input keys for this chain.
    #[must_use]
    pub fn get_input_keys(&self) -> Vec<String> {
        vec![self.input_key.clone()]
    }

    /// Get output keys for this chain.
    #[must_use]
    pub fn get_output_keys(&self) -> Vec<String> {
        vec![self.output_key.clone()]
    }

    /// Get the chain type identifier.
    #[must_use]
    pub fn chain_type(&self) -> &'static str {
        "llm_requests_chain"
    }

    /// Fetch content from a URL and extract text.
    ///
    /// # Arguments
    ///
    /// * `url` - The URL to fetch
    ///
    /// # Returns
    ///
    /// Extracted text content, limited to `text_length` characters
    async fn fetch_and_extract(&self, url: &str) -> Result<String> {
        // Build HTTP client with headers
        let mut headers = HeaderMap::new();
        headers.insert(USER_AGENT, HeaderValue::from_static(DEFAULT_USER_AGENT));

        // Add custom headers if provided
        if let Some(custom_headers) = &self.custom_headers {
            for (key, value) in custom_headers {
                headers.insert(key.clone(), value.clone());
            }
        }

        // Build HTTP client with optimized connection pooling
        // Apply LLM-optimized settings manually since we need custom headers
        let client = reqwest::Client::builder()
            .pool_max_idle_per_host(DEFAULT_POOL_MAX_IDLE_PER_HOST) // LLM-optimized connection pooling
            .pool_idle_timeout(DEFAULT_POOL_IDLE_TIMEOUT) // Longer connection reuse
            .tcp_keepalive(DEFAULT_TCP_KEEPALIVE) // Proactive broken connection detection
            .connect_timeout(DEFAULT_HTTP_CONNECT_TIMEOUT) // Connection establishment timeout
            .default_headers(headers)
            .build()
            .map_err(|e| Error::Other(format!("Failed to build HTTP client: {e}")))?;

        // Fetch the URL
        let response = client
            .get(url)
            .send()
            .await
            .map_err(|e| Error::Other(format!("Failed to fetch URL: {e}")))?;

        // Check status
        if !response.status().is_success() {
            return Err(Error::Other(format!(
                "HTTP request failed with status: {}",
                response.status()
            )));
        }

        // Get response text
        let html = response
            .text()
            .await
            .map_err(|e| Error::Other(format!("Failed to read response: {e}")))?;

        // Parse HTML and extract text
        let document = Html::parse_document(&html);

        // Get all text content from the document
        let text = document.root_element().text().collect::<Vec<_>>().join(" ");

        // Clean up whitespace and limit length
        let cleaned = text.split_whitespace().collect::<Vec<_>>().join(" ");

        // Truncate to text_length
        let truncated = if cleaned.len() > self.text_length {
            &cleaned[..self.text_length]
        } else {
            &cleaned
        };

        Ok(truncated.to_string())
    }
}

impl<M: dashflow::core::language_models::LLM> LLMRequestsChain<M> {
    /// Run the chain with the given input variables.
    ///
    /// # Arguments
    ///
    /// * `inputs` - Input variables including the URL and any additional variables for the LLM
    ///
    /// # Returns
    ///
    /// A `HashMap` containing the LLM's response under the output key
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The URL input key is missing
    /// - The HTTP request fails
    /// - HTML parsing fails
    /// - The LLM chain fails
    pub async fn run(&self, inputs: &HashMap<String, String>) -> Result<HashMap<String, String>> {
        // Get the URL from inputs
        let url = inputs
            .get(&self.input_key)
            .ok_or_else(|| Error::InvalidInput(format!("Missing input key: {}", self.input_key)))?;

        // Fetch and extract content
        let extracted_content = self.fetch_and_extract(url).await?;

        // Prepare inputs for LLM chain
        // Pass through all other keys and add the requests_result
        let mut llm_inputs = inputs
            .iter()
            .filter(|(k, _)| *k != &self.input_key)
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect::<HashMap<_, _>>();

        llm_inputs.insert(self.requests_key.clone(), extracted_content);

        // Call LLM chain
        let llm_result = self.llm_chain.run(&llm_inputs).await?;

        // Return with our output key
        let mut output = HashMap::new();
        output.insert(self.output_key.clone(), llm_result);

        Ok(output)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::LLMChain;
    use dashflow::core::callbacks::CallbackManager;
    use dashflow::core::language_models::{Generation, LLMResult, LLM};
    use dashflow::core::prompts::PromptTemplate;
    use std::sync::Arc;

    // Mock LLM for testing
    #[derive(Clone)]
    struct MockLLM {
        response: String,
    }

    impl MockLLM {
        fn new(response: impl Into<String>) -> Self {
            Self {
                response: response.into(),
            }
        }
    }

    #[async_trait::async_trait]
    impl LLM for MockLLM {
        async fn _generate(
            &self,
            _prompts: &[String],
            _stop: Option<&[String]>,
            _callbacks: Option<&CallbackManager>,
        ) -> Result<LLMResult> {
            let generations = vec![Generation {
                text: self.response.clone(),
                generation_info: None,
            }];

            Ok(LLMResult {
                generations: vec![generations],
                llm_output: None,
            })
        }

        fn llm_type(&self) -> &str {
            "mock_llm"
        }
    }

    #[tokio::test]
    async fn test_llm_requests_chain_construction() {
        let llm = Arc::new(MockLLM::new("test response"));
        let prompt = PromptTemplate::from_template("Test: {requests_result}").unwrap();
        let llm_chain = LLMChain::new(llm, prompt);

        let chain = LLMRequestsChain::new(llm_chain);

        assert_eq!(chain.text_length, 8000);
        assert_eq!(chain.requests_key, "requests_result");
        assert_eq!(chain.input_key, "url");
        assert_eq!(chain.output_key, "output");
    }

    #[tokio::test]
    async fn test_llm_requests_chain_builder() {
        let llm = Arc::new(MockLLM::new("test response"));
        let prompt = PromptTemplate::from_template("Test: {requests_result}").unwrap();
        let llm_chain = LLMChain::new(llm, prompt);

        let chain = LLMRequestsChain::new(llm_chain)
            .with_text_length(5000)
            .with_requests_key("content")
            .with_input_key("link")
            .with_output_key("result");

        assert_eq!(chain.text_length, 5000);
        assert_eq!(chain.requests_key, "content");
        assert_eq!(chain.input_key, "link");
        assert_eq!(chain.output_key, "result");
    }

    #[tokio::test]
    async fn test_get_input_output_keys() {
        let llm = Arc::new(MockLLM::new("test response"));
        let prompt = PromptTemplate::from_template("Test: {requests_result}").unwrap();
        let llm_chain = LLMChain::new(llm, prompt);
        let chain = LLMRequestsChain::new(llm_chain);

        assert_eq!(chain.get_input_keys(), vec!["url".to_string()]);
        assert_eq!(chain.get_output_keys(), vec!["output".to_string()]);
    }

    #[tokio::test]
    async fn test_chain_type() {
        let llm = Arc::new(MockLLM::new("test response"));
        let prompt = PromptTemplate::from_template("Test: {requests_result}").unwrap();
        let llm_chain = LLMChain::new(llm, prompt);
        let chain = LLMRequestsChain::new(llm_chain);

        assert_eq!(chain.chain_type(), "llm_requests_chain");
    }

    #[tokio::test]
    async fn test_missing_url_input() {
        let llm = Arc::new(MockLLM::new("test response"));
        let prompt = PromptTemplate::from_template("Test: {requests_result}").unwrap();
        let llm_chain = LLMChain::new(llm, prompt);
        let chain = LLMRequestsChain::new(llm_chain);

        let inputs = HashMap::new();
        let result = chain.run(&inputs).await;

        assert!(result.is_err());
        match result {
            Err(Error::InvalidInput(msg)) => {
                assert!(msg.contains("url"));
            }
            _ => panic!("Expected InvalidInput error"),
        }
    }

    // Integration test with real HTTP (ignored by default)
    #[tokio::test]
    #[ignore = "requires network"]
    async fn test_llm_requests_chain_real_http() {
        let llm = Arc::new(MockLLM::new("This is a test response"));
        let prompt = PromptTemplate::from_template(
            "Summarize this content: {requests_result}\n\nQuestion: {question}",
        )
        .unwrap();
        let llm_chain = LLMChain::new(llm, prompt);

        let chain = LLMRequestsChain::new(llm_chain);

        let mut inputs = HashMap::new();
        inputs.insert("url".to_string(), "https://example.com".to_string());
        inputs.insert(
            "question".to_string(),
            "What is this page about?".to_string(),
        );

        let result = chain.run(&inputs).await;
        assert!(result.is_ok());

        let output = result.unwrap();
        assert!(output.contains_key("output"));
    }

    // Test text extraction and length limiting
    #[tokio::test]
    async fn test_text_length_limiting() {
        let llm = Arc::new(MockLLM::new("test response"));
        let prompt = PromptTemplate::from_template("Content: {requests_result}").unwrap();
        let llm_chain = LLMChain::new(llm, prompt);

        // Set very short text length for testing
        let chain = LLMRequestsChain::new(llm_chain).with_text_length(10);

        assert_eq!(chain.text_length, 10);
    }
}
