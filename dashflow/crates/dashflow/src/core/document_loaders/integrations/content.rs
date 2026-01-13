// Allow clippy warnings for content loaders
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::clone_on_ref_ptr)]

//! Content platform and media loaders.
//!
//! This module provides loaders for content platforms and media services:
//! - Wikipedia (encyclopedia articles)
//! - `ArXiv` (academic papers and preprints)
//! - News (web news articles)
//!
//! © 2025 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

use async_trait::async_trait;
use serde_json::Value;

use crate::constants::{DEFAULT_HTTP_CONNECT_TIMEOUT, DEFAULT_HTTP_REQUEST_TIMEOUT};
use crate::core::documents::{Document, DocumentLoader};
use crate::core::error::Result;
use crate::core::http_client;

/// Create an HTTP client with standard timeouts (using centralized constants)
fn create_http_client() -> reqwest::Client {
    reqwest::Client::builder()
        .timeout(DEFAULT_HTTP_REQUEST_TIMEOUT)
        .connect_timeout(DEFAULT_HTTP_CONNECT_TIMEOUT)
        .build()
        .unwrap_or_else(|_| reqwest::Client::new())
}

/// Loader for Wikipedia articles via the Wikipedia API.
///
/// Fetches article content from Wikipedia using the `MediaWiki` API.
/// Supports multiple languages and configurable result limits.
///
/// # Example
///
/// ```no_run
/// use dashflow::core::document_loaders::{DocumentLoader, WikipediaLoader};
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let loader = WikipediaLoader::new("Rust programming language")
///     .with_lang("en")
///     .with_max_docs(3);
/// let documents = loader.load().await?;
/// # Ok(())
/// # }
/// ```
pub struct WikipediaLoader {
    query: String,
    lang: String,
    load_max_docs: usize,
}

impl WikipediaLoader {
    /// Create a new Wikipedia loader for the given query.
    pub fn new(query: impl Into<String>) -> Self {
        Self {
            query: query.into(),
            lang: "en".to_string(),
            load_max_docs: 1,
        }
    }

    /// Set the language code (default: "en").
    #[must_use]
    pub fn with_lang(mut self, lang: impl Into<String>) -> Self {
        self.lang = lang.into();
        self
    }

    /// Set the maximum number of documents to load (default: 1).
    #[must_use]
    pub fn with_max_docs(mut self, max: usize) -> Self {
        self.load_max_docs = max;
        self
    }
}

#[async_trait]
impl DocumentLoader for WikipediaLoader {
    async fn load(&self) -> Result<Vec<Document>> {
        // M-552: Validate language code to prevent SSRF via subdomain manipulation
        // Valid Wikipedia language codes: 2-4 letters, optionally followed by hyphen and more chars
        // Examples: "en", "de", "zh", "zh-hans", "zh-classical"
        if !self
            .lang
            .chars()
            .all(|c| c.is_ascii_lowercase() || c == '-')
            || self.lang.is_empty()
            || self.lang.len() > 20
            || self.lang.starts_with('-')
            || self.lang.ends_with('-')
        {
            return Err(crate::core::error::Error::InvalidInput(format!(
                "Invalid Wikipedia language code '{}'. Use lowercase letters and hyphens only (e.g., 'en', 'zh-hans').",
                self.lang
            )));
        }

        let client = create_http_client();

        // Search for pages matching the query
        let search_url = format!(
            "https://{}.wikipedia.org/w/api.php?action=opensearch&search={}&limit={}&format=json",
            self.lang,
            urlencoding::encode(&self.query),
            self.load_max_docs
        );

        // Use size-limited read to prevent memory exhaustion from large responses
        let response = client.get(&search_url).send().await?;
        let search_response =
            http_client::read_text_with_limit(response, http_client::DEFAULT_RESPONSE_SIZE_LIMIT)
                .await?;

        let search_json: Value = serde_json::from_str(&search_response)?;
        let titles = search_json
            .get(1)
            .and_then(|v| v.as_array())
            .ok_or_else(|| {
                crate::core::error::Error::InvalidInput(
                    "Invalid Wikipedia API response".to_string(),
                )
            })?;

        if titles.is_empty() {
            return Ok(Vec::new());
        }

        let mut documents = Vec::new();

        for title in titles.iter().take(self.load_max_docs) {
            let title_str = title.as_str().unwrap_or("");

            // Fetch page content
            let content_url = format!(
                "https://{}.wikipedia.org/w/api.php?action=query&titles={}&prop=extracts&explaintext=true&format=json",
                self.lang,
                urlencoding::encode(title_str)
            );

            // Use size-limited read to prevent memory exhaustion
            let resp = client.get(&content_url).send().await?;
            let content_response =
                http_client::read_text_with_limit(resp, http_client::DEFAULT_RESPONSE_SIZE_LIMIT)
                    .await?;

            let content_json: Value = serde_json::from_str(&content_response)?;

            if let Some(pages) = content_json
                .get("query")
                .and_then(|q| q.get("pages"))
                .and_then(|p| p.as_object())
            {
                for (_, page) in pages {
                    if let Some(extract) = page.get("extract").and_then(|e| e.as_str()) {
                        let url = format!(
                            "https://{}.wikipedia.org/wiki/{}",
                            self.lang,
                            urlencoding::encode(title_str)
                        );

                        let doc = Document::new(extract)
                            .with_metadata("source", url)
                            .with_metadata("title", title_str.to_string())
                            .with_metadata("language", self.lang.clone());

                        documents.push(doc);
                    }
                }
            }
        }

        Ok(documents)
    }
}

/// Loader for `ArXiv` papers via the `ArXiv` API.
///
/// Fetches paper metadata and abstracts from `ArXiv` using the Atom/RSS API.
///
/// # Example
///
/// ```no_run
/// use dashflow::core::document_loaders::{DocumentLoader, ArXivLoader};
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let loader = ArXivLoader::new("2103.03404"); // Attention Is All You Need paper ID
/// let documents = loader.load().await?;
/// # Ok(())
/// # }
/// ```
pub struct ArXivLoader {
    query: String,
    max_results: usize,
}

impl ArXivLoader {
    /// Create a new `ArXiv` loader for the given query (can be an ID or search terms).
    pub fn new(query: impl Into<String>) -> Self {
        Self {
            query: query.into(),
            max_results: 10,
        }
    }

    /// Set the maximum number of results to fetch (default: 10).
    #[must_use]
    pub fn with_max_results(mut self, max: usize) -> Self {
        self.max_results = max;
        self
    }
}

#[async_trait]
impl DocumentLoader for ArXivLoader {
    async fn load(&self) -> Result<Vec<Document>> {
        let client = create_http_client();

        // Build ArXiv API URL
        let url = format!(
            "https://export.arxiv.org/api/query?search_query=all:{}&start=0&max_results={}",
            urlencoding::encode(&self.query),
            self.max_results
        );

        // Use size-limited read to prevent memory exhaustion from large responses
        let resp = client.get(&url).send().await?;
        let response =
            http_client::read_text_with_limit(resp, http_client::DEFAULT_RESPONSE_SIZE_LIMIT)
                .await?;

        // Parse Atom XML response
        let mut documents = Vec::new();

        // Simple regex-based parsing of Atom XML (static patterns are validated at compile time)
        let entry_re =
            regex::Regex::new(r"<entry>(.*?)</entry>").expect("static entry regex pattern");
        let title_re =
            regex::Regex::new(r"<title>([^<]*)</title>").expect("static title regex pattern");
        let summary_re =
            regex::Regex::new(r"<summary>([^<]*)</summary>").expect("static summary regex pattern");
        let id_re = regex::Regex::new(r"<id>([^<]*)</id>").expect("static id regex pattern");
        let published_re = regex::Regex::new(r"<published>([^<]*)</published>")
            .expect("static published regex pattern");
        let author_re =
            regex::Regex::new(r"<name>([^<]*)</name>").expect("static author regex pattern");

        for entry_cap in entry_re.captures_iter(&response) {
            let entry_text = entry_cap.get(1).map_or("", |m| m.as_str());

            let title = title_re
                .captures(entry_text)
                .and_then(|c| c.get(1))
                .map(|m| m.as_str().trim().to_string())
                .unwrap_or_default();

            let summary = summary_re
                .captures(entry_text)
                .and_then(|c| c.get(1))
                .map(|m| m.as_str().trim().to_string())
                .unwrap_or_default();

            let id = id_re
                .captures(entry_text)
                .and_then(|c| c.get(1))
                .map(|m| m.as_str().trim().to_string())
                .unwrap_or_default();

            let published = published_re
                .captures(entry_text)
                .and_then(|c| c.get(1))
                .map(|m| m.as_str().trim().to_string())
                .unwrap_or_default();

            // Extract all authors
            let authors: Vec<String> = author_re
                .captures_iter(entry_text)
                .filter_map(|c| c.get(1).map(|m| m.as_str().trim().to_string()))
                .collect();

            if !summary.is_empty() {
                let content = format!("{title}\n\n{summary}");

                let mut doc = Document::new(content)
                    .with_metadata("source", id.clone())
                    .with_metadata("title", title);

                if !published.is_empty() {
                    doc = doc.with_metadata("published", published);
                }

                if !authors.is_empty() {
                    doc = doc.with_metadata("authors", authors.join(", "));
                }

                documents.push(doc);
            }
        }

        if documents.is_empty() {
            return Err(crate::core::error::Error::InvalidInput(format!(
                "No papers found for query: {}",
                self.query
            )));
        }

        Ok(documents)
    }
}

/// Loader for news articles from web URLs.
///
/// Fetches and extracts text content from news article URLs using HTML parsing.
///
/// # Example
///
/// ```no_run
/// use dashflow::core::document_loaders::{DocumentLoader, NewsLoader};
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let loader = NewsLoader::new("https://example.com/news/article");
/// let documents = loader.load().await?;
/// # Ok(())
/// # }
/// ```
pub struct NewsLoader {
    url: String,
}

impl NewsLoader {
    /// Create a new news loader for the given URL.
    pub fn new(url: impl Into<String>) -> Self {
        Self { url: url.into() }
    }
}

#[async_trait]
impl DocumentLoader for NewsLoader {
    async fn load(&self) -> Result<Vec<Document>> {
        // M-552: SSRF protection - validate URL before fetching
        http_client::validate_url_for_ssrf(&self.url)?;

        let client = create_http_client();
        // Use size-limited read to prevent memory exhaustion from large responses
        let resp = client.get(&self.url).send().await?;
        let response =
            http_client::read_text_with_limit(resp, http_client::DEFAULT_RESPONSE_SIZE_LIMIT)
                .await?;

        // Use html2text to extract text content
        let text = html2text::from_read(response.as_bytes(), 80);

        let doc = Document::new(text)
            .with_metadata("source", self.url.clone())
            .with_metadata("format", "news");

        Ok(vec![doc])
    }
}

// NOTE: YouTubeTranscriptLoader, GoogleSpeechToTextLoader, AssemblyAILoader,
// WeatherLoader, and PsychicLoader were removed (placeholder implementations).
// See git history for implementation notes if these need to be added.

#[cfg(test)]
mod tests {
    use super::*;

    // ==========================================================================
    // WikipediaLoader Tests
    // ==========================================================================

    #[test]
    fn test_wikipedia_loader_new() {
        let loader = WikipediaLoader::new("Rust programming language");
        assert_eq!(loader.query, "Rust programming language");
        assert_eq!(loader.lang, "en");
        assert_eq!(loader.load_max_docs, 1);
    }

    #[test]
    fn test_wikipedia_loader_with_lang() {
        let loader = WikipediaLoader::new("Test").with_lang("de");
        assert_eq!(loader.lang, "de");

        let loader2 = WikipediaLoader::new("Test").with_lang("fr".to_string());
        assert_eq!(loader2.lang, "fr");
    }

    #[test]
    fn test_wikipedia_loader_with_max_docs() {
        let loader = WikipediaLoader::new("Test").with_max_docs(5);
        assert_eq!(loader.load_max_docs, 5);

        let loader2 = WikipediaLoader::new("Test").with_max_docs(0);
        assert_eq!(loader2.load_max_docs, 0);
    }

    #[test]
    fn test_wikipedia_loader_chained_config() {
        let loader = WikipediaLoader::new("Machine Learning")
            .with_lang("zh-hans")
            .with_max_docs(3);

        assert_eq!(loader.query, "Machine Learning");
        assert_eq!(loader.lang, "zh-hans");
        assert_eq!(loader.load_max_docs, 3);
    }

    #[tokio::test]
    async fn test_wikipedia_loader_invalid_lang_empty() {
        let loader = WikipediaLoader::new("Test").with_lang("");
        let result = loader.load().await;
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("Invalid Wikipedia language code"));
    }

    #[tokio::test]
    async fn test_wikipedia_loader_invalid_lang_uppercase() {
        let loader = WikipediaLoader::new("Test").with_lang("EN");
        let result = loader.load().await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_wikipedia_loader_invalid_lang_numbers() {
        let loader = WikipediaLoader::new("Test").with_lang("en123");
        let result = loader.load().await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_wikipedia_loader_invalid_lang_too_long() {
        let loader = WikipediaLoader::new("Test").with_lang("a".repeat(25));
        let result = loader.load().await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_wikipedia_loader_invalid_lang_starts_with_hyphen() {
        let loader = WikipediaLoader::new("Test").with_lang("-en");
        let result = loader.load().await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_wikipedia_loader_invalid_lang_ends_with_hyphen() {
        let loader = WikipediaLoader::new("Test").with_lang("en-");
        let result = loader.load().await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_wikipedia_loader_valid_lang_with_hyphen() {
        // Valid language codes like zh-hans should pass validation
        // (though they may fail on network request)
        let loader = WikipediaLoader::new("Test").with_lang("zh-hans");
        // Just test that it doesn't immediately error on language validation
        // Network errors are expected
        let result = loader.load().await;
        // Either succeeds or fails with network error (not lang validation)
        if let Err(e) = &result {
            let err_str = e.to_string();
            assert!(!err_str.contains("Invalid Wikipedia language code"));
        }
    }

    // ==========================================================================
    // ArXivLoader Tests
    // ==========================================================================

    #[test]
    fn test_arxiv_loader_new() {
        let loader = ArXivLoader::new("2103.03404");
        assert_eq!(loader.query, "2103.03404");
        assert_eq!(loader.max_results, 10);
    }

    #[test]
    fn test_arxiv_loader_with_string() {
        let loader = ArXivLoader::new("machine learning".to_string());
        assert_eq!(loader.query, "machine learning");
    }

    #[test]
    fn test_arxiv_loader_with_max_results() {
        let loader = ArXivLoader::new("test").with_max_results(25);
        assert_eq!(loader.max_results, 25);

        let loader2 = ArXivLoader::new("test").with_max_results(1);
        assert_eq!(loader2.max_results, 1);
    }

    #[test]
    fn test_arxiv_loader_chained_config() {
        let loader = ArXivLoader::new("quantum computing").with_max_results(50);
        assert_eq!(loader.query, "quantum computing");
        assert_eq!(loader.max_results, 50);
    }

    // ==========================================================================
    // NewsLoader Tests
    // ==========================================================================

    #[test]
    fn test_news_loader_new() {
        let loader = NewsLoader::new("https://example.com/news/article");
        assert_eq!(loader.url, "https://example.com/news/article");
    }

    #[test]
    fn test_news_loader_new_with_string() {
        let loader = NewsLoader::new("https://example.com".to_string());
        assert_eq!(loader.url, "https://example.com");
    }

    #[tokio::test]
    async fn test_news_loader_ssrf_private_ip() {
        // Test SSRF protection for private IPs
        let loader = NewsLoader::new("http://192.168.1.1/");
        let result = loader.load().await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_news_loader_ssrf_localhost() {
        let loader = NewsLoader::new("http://localhost/");
        let result = loader.load().await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_news_loader_ssrf_loopback() {
        let loader = NewsLoader::new("http://127.0.0.1/");
        let result = loader.load().await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_news_loader_ssrf_metadata_endpoint() {
        // AWS metadata endpoint
        let loader = NewsLoader::new("http://169.254.169.254/");
        let result = loader.load().await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_news_loader_invalid_url() {
        let loader = NewsLoader::new("not-a-valid-url");
        let result = loader.load().await;
        assert!(result.is_err());
    }

    // ==========================================================================
    // create_http_client Tests
    // ==========================================================================

    #[test]
    fn test_create_http_client_succeeds() {
        // Should not panic
        let _client = create_http_client();
    }

    // ==========================================================================
    // Integration-style Tests (no network, just validation)
    // ==========================================================================

    #[test]
    fn test_loaders_implement_document_loader() {
        // Compile-time check that all loaders implement DocumentLoader
        fn _assert_document_loader<T: DocumentLoader>() {}

        _assert_document_loader::<WikipediaLoader>();
        _assert_document_loader::<ArXivLoader>();
        _assert_document_loader::<NewsLoader>();
    }

    #[test]
    fn test_wikipedia_loader_query_special_characters() {
        // Ensure special characters in query don't cause issues during construction
        let loader = WikipediaLoader::new("C++ (programming language)");
        assert_eq!(loader.query, "C++ (programming language)");

        let loader2 = WikipediaLoader::new("Rust & Safety");
        assert_eq!(loader2.query, "Rust & Safety");
    }

    #[test]
    fn test_arxiv_loader_query_special_characters() {
        let loader = ArXivLoader::new("Attention Is All You Need");
        assert_eq!(loader.query, "Attention Is All You Need");

        let loader2 = ArXivLoader::new("deep learning + neural networks");
        assert_eq!(loader2.query, "deep learning + neural networks");
    }

    #[test]
    fn test_wikipedia_valid_language_codes() {
        // Test various valid Wikipedia language codes
        let valid_codes = ["en", "de", "fr", "ja", "zh", "zh-hans", "zh-classical", "simple"];

        for code in valid_codes {
            let loader = WikipediaLoader::new("Test").with_lang(code);
            assert_eq!(loader.lang, code);
        }
    }

    #[test]
    fn test_wikipedia_loader_max_docs_boundary() {
        let loader = WikipediaLoader::new("Test").with_max_docs(usize::MAX);
        assert_eq!(loader.load_max_docs, usize::MAX);
    }

    #[test]
    fn test_arxiv_loader_max_results_boundary() {
        let loader = ArXivLoader::new("Test").with_max_results(usize::MAX);
        assert_eq!(loader.max_results, usize::MAX);
    }

    #[test]
    fn test_news_loader_various_protocols() {
        // HTTPS should work
        let loader1 = NewsLoader::new("https://example.com");
        assert_eq!(loader1.url, "https://example.com");

        // HTTP should work (will be validated at load time)
        let loader2 = NewsLoader::new("http://example.com");
        assert_eq!(loader2.url, "http://example.com");
    }

    #[test]
    fn test_wikipedia_loader_empty_query() {
        let loader = WikipediaLoader::new("");
        assert_eq!(loader.query, "");
    }

    #[test]
    fn test_arxiv_loader_empty_query() {
        let loader = ArXivLoader::new("");
        assert_eq!(loader.query, "");
    }
}
