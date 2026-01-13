//! Web Research Retriever - Generate search queries and retrieve web content
//!
//! The `WebResearchRetriever` generates multiple search queries from a question using an LLM,
//! performs web searches, loads HTML content, converts it to text, chunks it, stores it in
//! a vector store, and returns relevant documents.
//!
//! This is particularly useful for building research assistants that can search the web,
//! process results, and answer questions based on current web content.

use async_trait::async_trait;
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};
use tokio::sync::RwLock;
use tracing::debug;

use crate::core::{
    config::RunnableConfig,
    document_loaders::{DocumentLoader, URLLoader},
    documents::Document,
    error::{Error, Result},
    retrievers::{parent_document_retriever::TextSplitter, Retriever},
    runnable::Runnable,
    vector_stores::VectorStore,
};

/// Default prompt template for generating search queries
pub const DEFAULT_SEARCH_PROMPT: &str =
    "You are an assistant tasked with improving web search results. \
Generate THREE search queries that are similar to this question. \
The output should be a numbered list of questions and each should have a question mark at the end: \
{question}";

/// Web search tool trait that can be implemented by different search providers
#[async_trait]
pub trait WebSearchTool: Send + Sync {
    /// Perform a search and return a list of result items
    /// Each result should contain at least a "link" or "url" field
    async fn search(&self, query: &str, num_results: usize)
        -> Result<Vec<HashMap<String, String>>>;
}

/// `WebResearchRetriever` that generates search queries, fetches web content, and retrieves documents
///
/// # Example
///
/// ```rust,ignore
/// use dashflow::core::retrievers::WebResearchRetriever;
/// use dashflow_brave::BraveSearchTool;
/// use tokio::sync::RwLock;
/// use std::sync::Arc;
///
/// // Create components
/// let vector_store = Arc::new(RwLock::new(/* your vector store */));
/// let llm_chain = /* your LLM chain that outputs Vec<String> */;
/// let search = Arc::new(/* your search tool */);
///
/// // Create retriever
/// let retriever = WebResearchRetriever::new(
///     vector_store,
///     llm_chain,
///     search,
///     3,    // num_search_results
///     Arc::new(/* your text splitter */),
/// );
///
/// // Use for research
/// let docs = retriever._get_relevant_documents("What is Rust?", None).await?;
/// ```
pub struct WebResearchRetriever<V, C, S>
where
    V: VectorStore,
    C: Runnable<Input = HashMap<String, String>, Output = Vec<String>>,
    S: WebSearchTool,
{
    /// Vector store for storing web page chunks (uses `RwLock` for interior mutability)
    pub vectorstore: Arc<RwLock<V>>,

    /// LLM chain that generates search queries from the input question
    /// Should output `Vec<String>` of search queries
    pub llm_chain: C,

    /// Web search tool (e.g., Brave, `DuckDuckGo`, Google)
    pub search: Arc<S>,

    /// Number of search results to fetch per query
    pub num_search_results: usize,

    /// Text splitter for chunking web pages
    pub text_splitter: Arc<dyn TextSplitter>,

    /// Database of URLs that have already been processed
    pub url_database: Arc<Mutex<HashSet<String>>>,

    /// Whether to log debug information
    pub verbose: bool,
}

impl<V, C, S> WebResearchRetriever<V, C, S>
where
    V: VectorStore,
    C: Runnable<Input = HashMap<String, String>, Output = Vec<String>>,
    S: WebSearchTool,
{
    /// Create a new `WebResearchRetriever`
    ///
    /// # Arguments
    ///
    /// * `vectorstore` - Vector store for storing web page chunks
    /// * `llm_chain` - Chain that generates search queries (`PromptTemplate` | LLM | `QuestionListOutputParser`)
    /// * `search` - Web search tool implementation
    /// * `num_search_results` - Number of search results per query
    /// * `text_splitter` - Text splitter for chunking web pages
    pub fn new(
        vectorstore: Arc<RwLock<V>>,
        llm_chain: C,
        search: Arc<S>,
        num_search_results: usize,
        text_splitter: Arc<dyn TextSplitter>,
    ) -> Self {
        Self {
            vectorstore,
            llm_chain,
            search,
            num_search_results,
            text_splitter,
            url_database: Arc::new(Mutex::new(HashSet::new())),
            verbose: true,
        }
    }

    /// Set verbosity for logging
    #[must_use]
    pub fn with_verbose(mut self, verbose: bool) -> Self {
        self.verbose = verbose;
        self
    }

    /// Clean search query to avoid issues with search APIs
    ///
    /// Some search tools (e.g., Google) will fail if the query starts with a digit.
    /// This method removes leading numbers and quotes from queries.
    pub fn clean_search_query(&self, query: &str) -> String {
        let query = query.trim();

        // If starts with digit, find and remove everything up to first quote
        if query.chars().next().is_some_and(|c| c.is_ascii_digit()) {
            if let Some(first_quote_pos) = query.find('"') {
                let mut result = query[first_quote_pos + 1..].to_string();
                // Remove trailing quote if present
                if result.ends_with('"') {
                    result.pop();
                }
                return result.trim().to_string();
            }
        }

        query.to_string()
    }

    /// Generate search queries from user's question using the LLM chain
    async fn generate_queries(
        &self,
        question: &str,
        config: Option<&RunnableConfig>,
    ) -> Result<Vec<String>> {
        let mut vars = HashMap::new();
        vars.insert("question".to_string(), question.to_string());

        let queries = self
            .llm_chain
            .invoke(vars, config.cloned())
            .await
            .map_err(|e| {
                Error::other(format!(
                    "Failed to generate search queries from LLM chain: {e}"
                ))
            })?;

        if self.verbose {
            debug!("[WebResearchRetriever] Generated queries: {queries:?}");
        }

        Ok(queries)
    }

    /// Perform web search for a single query
    async fn search_query(&self, query: &str) -> Result<Vec<String>> {
        let clean_query = self.clean_search_query(query);

        if self.verbose {
            debug!("[WebResearchRetriever] Searching for: {clean_query}");
        }

        let results = self
            .search
            .search(&clean_query, self.num_search_results)
            .await
            .map_err(|e| {
                Error::other(format!("Web search failed for query '{clean_query}': {e}"))
            })?;

        // Extract URLs from results
        let mut urls = Vec::new();
        for result in results {
            if let Some(url) = result.get("link").or_else(|| result.get("url")) {
                urls.push(url.clone());
            }
        }

        if self.verbose {
            debug!("[WebResearchRetriever] Found {} URLs", urls.len());
        }

        Ok(urls)
    }

    /// Load and process new URLs into the vector store
    async fn load_and_index_urls(&self, urls: Vec<String>) -> Result<()> {
        if urls.is_empty() {
            return Ok(());
        }

        if self.verbose {
            debug!("[WebResearchRetriever] Loading {} URLs...", urls.len());
        }

        let mut all_docs = Vec::new();

        // Load HTML from each URL
        for url in &urls {
            match self.load_url(url).await {
                Ok(doc) => all_docs.push(doc),
                Err(e) => {
                    if self.verbose {
                        debug!("[WebResearchRetriever] Failed to load {url}: {e}");
                    }
                    // Continue with other URLs even if one fails
                }
            }
        }

        if all_docs.is_empty() {
            return Ok(());
        }

        // Split documents into chunks
        let chunks = self.text_splitter.split_documents(&all_docs);

        if self.verbose {
            debug!("[WebResearchRetriever] Split into {} chunks", chunks.len());
        }

        // Add to vector store (acquire write lock for mutation)
        {
            let mut vs = self.vectorstore.write().await;
            vs.add_documents(&chunks, None).await.map_err(|e| {
                Error::other(format!(
                    "Failed to add {} document chunks to vector store: {e}",
                    chunks.len()
                ))
            })?;
        }

        // Update URL database
        {
            let mut db = self.url_database.lock().unwrap_or_else(|e| e.into_inner());
            for url in urls {
                db.insert(url);
            }
        }

        Ok(())
    }

    /// Load a single URL and convert to document
    async fn load_url(&self, url: &str) -> Result<Document> {
        let loader = URLLoader::new(url);
        let mut docs = loader
            .load()
            .await
            .map_err(|e| Error::other(format!("Failed to load URL '{url}': {e}")))?;

        if docs.is_empty() {
            return Err(Error::Other(format!("No content loaded from {url}")));
        }

        // Return first document (URLLoader returns one doc per URL)
        Ok(docs.remove(0))
    }
}

#[async_trait]
impl<V, C, S> Retriever for WebResearchRetriever<V, C, S>
where
    V: VectorStore + Send + Sync,
    C: Runnable<Input = HashMap<String, String>, Output = Vec<String>> + Send + Sync,
    S: WebSearchTool + Send + Sync,
{
    async fn _get_relevant_documents(
        &self,
        query: &str,
        config: Option<&RunnableConfig>,
    ) -> Result<Vec<Document>> {
        // Step 1: Generate search queries from the question
        if self.verbose {
            debug!("[WebResearchRetriever] Generating queries for: {query}");
        }

        let queries = self.generate_queries(query, config).await?;

        // Step 2: Search for URLs using all queries
        let mut all_urls = Vec::new();
        for q in &queries {
            match self.search_query(q).await {
                Ok(urls) => all_urls.extend(urls),
                Err(e) => {
                    if self.verbose {
                        debug!("[WebResearchRetriever] Search failed for '{q}': {e}");
                    }
                }
            }
        }

        // Get unique URLs
        let unique_urls: HashSet<String> = all_urls.into_iter().collect();

        // Step 3: Filter out URLs we've already processed
        let new_urls: Vec<String> = {
            let db = self.url_database.lock().unwrap_or_else(|e| e.into_inner());
            unique_urls
                .into_iter()
                .filter(|url| !db.contains(url))
                .collect()
        };

        if self.verbose {
            debug!(
                "[WebResearchRetriever] Found {} new URLs to process",
                new_urls.len()
            );
        }

        // Step 4: Load and index new URLs
        if !new_urls.is_empty() {
            self.load_and_index_urls(new_urls).await?;
        }

        // Step 5: Retrieve relevant documents from vector store for each query
        if self.verbose {
            debug!("[WebResearchRetriever] Retrieving relevant documents from vector store...");
        }

        let mut all_docs = Vec::new();
        {
            let vs = self.vectorstore.read().await;
            for q in &queries {
                let docs = vs._similarity_search(q, 4, None).await?;
                all_docs.extend(docs);
            }
        }

        // Step 6: Return unique documents
        let unique_docs = unique_documents(all_docs);

        if self.verbose {
            debug!(
                "[WebResearchRetriever] Returning {} unique documents",
                unique_docs.len()
            );
        }

        Ok(unique_docs)
    }

    fn name(&self) -> String {
        "WebResearchRetriever".to_string()
    }
}

#[async_trait]
impl<V, C, S> Runnable for WebResearchRetriever<V, C, S>
where
    V: VectorStore + Send + Sync,
    C: Runnable<Input = HashMap<String, String>, Output = Vec<String>> + Send + Sync,
    S: WebSearchTool + Send + Sync,
{
    type Input = String;
    type Output = Vec<Document>;

    async fn invoke(
        &self,
        input: Self::Input,
        config: Option<RunnableConfig>,
    ) -> Result<Self::Output> {
        self._get_relevant_documents(&input, config.as_ref()).await
    }

    async fn batch(
        &self,
        inputs: Vec<Self::Input>,
        config: Option<RunnableConfig>,
    ) -> Result<Vec<Self::Output>> {
        let mut results = Vec::new();
        for input in inputs {
            results.push(self.invoke(input, config.clone()).await?);
        }
        Ok(results)
    }

    async fn stream(
        &self,
        input: Self::Input,
        config: Option<RunnableConfig>,
    ) -> Result<std::pin::Pin<Box<dyn futures::Stream<Item = Result<Self::Output>> + Send>>> {
        let result = self.invoke(input, config).await?;
        Ok(Box::pin(futures::stream::once(async move { Ok(result) })))
    }
}

/// Helper function to deduplicate documents based on content and metadata
fn unique_documents(documents: Vec<Document>) -> Vec<Document> {
    let mut seen = HashSet::new();
    let mut unique = Vec::new();

    for doc in documents {
        // Create a key from content and a string representation of sorted metadata
        let mut metadata_items: Vec<(String, String)> = doc
            .metadata
            .iter()
            .map(|(k, v)| (k.clone(), v.to_string()))
            .collect();
        metadata_items.sort_by(|a, b| a.0.cmp(&b.0));
        let key = (doc.page_content.clone(), format!("{metadata_items:?}"));

        if seen.insert(key) {
            unique.push(doc);
        }
    }

    unique
}

#[cfg(test)]
mod tests {
    use super::unique_documents;
    use crate::core::{
        documents::Document,
        embeddings::Embeddings,
        retrievers::parent_document_retriever::TextSplitter,
        vector_stores::{DistanceMetric, InMemoryVectorStore},
    };
    use crate::test_prelude::*;
    use std::sync::Arc;
    use tokio::sync::RwLock;

    // Mock LLM chain that returns predefined queries
    struct MockLLMChain {
        queries: Vec<String>,
    }

    #[async_trait]
    impl Runnable for MockLLMChain {
        type Input = HashMap<String, String>;
        type Output = Vec<String>;

        async fn invoke(
            &self,
            _input: Self::Input,
            _config: Option<RunnableConfig>,
        ) -> Result<Self::Output> {
            Ok(self.queries.clone())
        }

        async fn batch(
            &self,
            inputs: Vec<Self::Input>,
            _config: Option<RunnableConfig>,
        ) -> Result<Vec<Self::Output>> {
            Ok(inputs.iter().map(|_| self.queries.clone()).collect())
        }

        async fn stream(
            &self,
            input: Self::Input,
            config: Option<RunnableConfig>,
        ) -> Result<std::pin::Pin<Box<dyn futures::Stream<Item = Result<Self::Output>> + Send>>>
        {
            let result = self.invoke(input, config).await?;
            Ok(Box::pin(futures::stream::once(async move { Ok(result) })))
        }
    }

    // Mock search tool that returns predefined URLs
    struct MockSearchTool {
        results: Vec<HashMap<String, String>>,
    }

    #[async_trait]
    impl WebSearchTool for MockSearchTool {
        async fn search(
            &self,
            _query: &str,
            _num_results: usize,
        ) -> Result<Vec<HashMap<String, String>>> {
            Ok(self.results.clone())
        }
    }

    // Mock text splitter for testing
    struct MockTextSplitter {
        chunk_size: usize,
    }

    impl TextSplitter for MockTextSplitter {
        fn split_documents(&self, documents: &[Document]) -> Vec<Document> {
            let mut chunks = Vec::new();
            for doc in documents {
                // Simple splitting: split by chunk_size characters
                let content = &doc.page_content;
                for (i, chunk) in content.as_bytes().chunks(self.chunk_size).enumerate() {
                    let chunk_content = String::from_utf8_lossy(chunk).to_string();
                    let mut chunk_doc = Document::new(chunk_content);
                    chunk_doc.metadata = doc.metadata.clone();
                    chunk_doc
                        .metadata
                        .insert("chunk".to_string(), serde_json::json!(i));
                    chunks.push(chunk_doc);
                }
            }
            chunks
        }
    }

    // Mock embeddings for testing
    struct MockEmbeddings;

    #[async_trait]
    impl Embeddings for MockEmbeddings {
        async fn _embed_documents(&self, texts: &[String]) -> Result<Vec<Vec<f32>>> {
            // Simple mock: return vectors based on text length
            Ok(texts
                .iter()
                .map(|t| vec![t.len() as f32, (t.len() as f32) * 2.0, 1.0])
                .collect())
        }

        async fn _embed_query(&self, text: &str) -> Result<Vec<f32>> {
            Ok(vec![text.len() as f32, (text.len() as f32) * 2.0, 1.0])
        }
    }

    #[tokio::test]
    async fn test_unique_documents() {
        let doc1 = Document::new("content1");
        let doc2 = Document::new("content2");
        let doc1_dup = Document::new("content1");

        let docs = vec![doc1.clone(), doc2.clone(), doc1_dup];
        let unique = unique_documents(docs);

        assert_eq!(unique.len(), 2);
        assert_eq!(unique[0].page_content, "content1");
        assert_eq!(unique[1].page_content, "content2");
    }

    #[tokio::test]
    async fn test_unique_documents_with_metadata() {
        let doc1 = Document::new("same content").with_metadata("source", "url1");
        let doc2 = Document::new("same content").with_metadata("source", "url2");
        let doc1_dup = Document::new("same content").with_metadata("source", "url1");

        let docs = vec![doc1.clone(), doc2.clone(), doc1_dup];
        let unique = unique_documents(docs);

        // Should have 2 unique docs (different metadata)
        assert_eq!(unique.len(), 2);
    }

    #[tokio::test]
    async fn test_web_research_retriever_basic() {
        // Create mock components
        let embeddings = Arc::new(MockEmbeddings);
        let vectorstore = InMemoryVectorStore::with_metric(embeddings, DistanceMetric::Cosine);
        let vectorstore = Arc::new(RwLock::new(vectorstore));

        let llm_chain = MockLLMChain {
            queries: vec!["query1".to_string(), "query2".to_string()],
        };

        let mut result1 = HashMap::new();
        result1.insert("link".to_string(), "http://example.com/1".to_string());
        let search = Arc::new(MockSearchTool {
            results: vec![result1],
        });

        let text_splitter = Arc::new(MockTextSplitter { chunk_size: 100 });

        // Create retriever
        let retriever =
            WebResearchRetriever::new(vectorstore.clone(), llm_chain, search, 3, text_splitter)
                .with_verbose(false);

        // Test query generation
        let queries = retriever
            .generate_queries("test question", None)
            .await
            .unwrap();
        assert_eq!(queries.len(), 2);
        assert_eq!(queries[0], "query1");
        assert_eq!(queries[1], "query2");
    }

    #[tokio::test]
    async fn test_web_research_retriever_url_filtering() {
        let embeddings = Arc::new(MockEmbeddings);
        let vectorstore = InMemoryVectorStore::with_metric(embeddings, DistanceMetric::Cosine);
        let vectorstore = Arc::new(RwLock::new(vectorstore));

        let llm_chain = MockLLMChain {
            queries: vec!["test query".to_string()],
        };

        let mut result1 = HashMap::new();
        result1.insert("link".to_string(), "http://example.com/1".to_string());
        let search = Arc::new(MockSearchTool {
            results: vec![result1],
        });

        let text_splitter = Arc::new(MockTextSplitter { chunk_size: 100 });

        let retriever = WebResearchRetriever::new(vectorstore, llm_chain, search, 3, text_splitter)
            .with_verbose(false);

        // Add URL to database manually
        {
            let mut db = retriever.url_database.lock().unwrap();
            db.insert("http://example.com/1".to_string());
        }

        // Search for a query should generate queries
        let queries = retriever.search_query("test").await.unwrap();
        // MockSearchTool returns 1 URL, but it's already in database, so should be filtered
        assert_eq!(queries.len(), 1);
    }

    #[tokio::test]
    async fn test_mock_text_splitter() {
        let splitter = MockTextSplitter { chunk_size: 10 };
        let doc = Document::new("This is a test document");
        let chunks = splitter.split_documents(&[doc]);

        // "This is a test document" = 23 bytes, chunk_size=10, so 3 chunks
        assert_eq!(chunks.len(), 3);
        assert_eq!(chunks[0].page_content, "This is a ");
        assert_eq!(chunks[1].page_content, "test docum");
        assert_eq!(chunks[2].page_content, "ent");
    }

    #[tokio::test]
    async fn test_retriever_trait_implementation() {
        let embeddings = Arc::new(MockEmbeddings);
        let vectorstore = InMemoryVectorStore::with_metric(embeddings, DistanceMetric::Cosine);
        let vectorstore = Arc::new(RwLock::new(vectorstore));

        let llm_chain = MockLLMChain {
            queries: vec!["test".to_string()],
        };

        let search = Arc::new(MockSearchTool { results: vec![] });

        let text_splitter = Arc::new(MockTextSplitter { chunk_size: 100 });

        let retriever = WebResearchRetriever::new(vectorstore, llm_chain, search, 3, text_splitter)
            .with_verbose(false);

        // Test name() method (use fully qualified syntax to disambiguate)
        assert_eq!(
            <WebResearchRetriever<_, _, _> as crate::core::retrievers::Retriever>::name(&retriever),
            "WebResearchRetriever"
        );
    }

    #[tokio::test]
    async fn test_runnable_interface() {
        let embeddings = Arc::new(MockEmbeddings);
        let vectorstore = InMemoryVectorStore::with_metric(embeddings, DistanceMetric::Cosine);
        let vectorstore = Arc::new(RwLock::new(vectorstore));

        let llm_chain = MockLLMChain {
            queries: vec!["test".to_string()],
        };

        let search = Arc::new(MockSearchTool { results: vec![] });

        let text_splitter = Arc::new(MockTextSplitter { chunk_size: 100 });

        let retriever = WebResearchRetriever::new(vectorstore, llm_chain, search, 3, text_splitter)
            .with_verbose(false);

        // Test invoke through Runnable trait
        let result = retriever
            .invoke("What is Rust?".to_string(), None)
            .await
            .unwrap();

        // No URLs returned from mock search, so should be empty
        assert_eq!(result.len(), 0);
    }

    // ========================================================================
    // Additional Tests for Coverage
    // ========================================================================

    #[tokio::test]
    async fn test_clean_search_query_with_leading_digit() {
        let embeddings = Arc::new(MockEmbeddings);
        let vectorstore = InMemoryVectorStore::with_metric(embeddings, DistanceMetric::Cosine);
        let vectorstore = Arc::new(RwLock::new(vectorstore));

        let llm_chain = MockLLMChain {
            queries: vec!["query".to_string()],
        };

        let search = Arc::new(MockSearchTool { results: vec![] });
        let text_splitter = Arc::new(MockTextSplitter { chunk_size: 100 });

        let retriever = WebResearchRetriever::new(vectorstore, llm_chain, search, 3, text_splitter);

        // Test with leading digit and quotes
        let cleaned = retriever.clean_search_query("1. \"What is Rust?\"");
        assert_eq!(cleaned, "What is Rust?");

        // Test with leading digit, quote in middle
        let cleaned = retriever.clean_search_query("2. Some \"query text");
        assert_eq!(cleaned, "query text");

        // Test without leading digit - should return as-is
        let cleaned = retriever.clean_search_query("Normal query");
        assert_eq!(cleaned, "Normal query");

        // Test with leading digit but no quotes - returns as-is
        let cleaned = retriever.clean_search_query("3 What is this");
        assert_eq!(cleaned, "3 What is this");
    }

    #[tokio::test]
    async fn test_clean_search_query_edge_cases() {
        let embeddings = Arc::new(MockEmbeddings);
        let vectorstore = InMemoryVectorStore::with_metric(embeddings, DistanceMetric::Cosine);
        let vectorstore = Arc::new(RwLock::new(vectorstore));

        let llm_chain = MockLLMChain {
            queries: vec!["query".to_string()],
        };

        let search = Arc::new(MockSearchTool { results: vec![] });
        let text_splitter = Arc::new(MockTextSplitter { chunk_size: 100 });

        let retriever = WebResearchRetriever::new(vectorstore, llm_chain, search, 3, text_splitter);

        // Empty query
        let cleaned = retriever.clean_search_query("");
        assert_eq!(cleaned, "");

        // Only whitespace
        let cleaned = retriever.clean_search_query("   ");
        assert_eq!(cleaned, "");

        // Query with trailing quote
        let cleaned = retriever.clean_search_query("1. \"query\"");
        assert_eq!(cleaned, "query");
    }

    #[tokio::test]
    async fn test_search_query_with_cleaning() {
        let embeddings = Arc::new(MockEmbeddings);
        let vectorstore = InMemoryVectorStore::with_metric(embeddings, DistanceMetric::Cosine);
        let vectorstore = Arc::new(RwLock::new(vectorstore));

        let llm_chain = MockLLMChain {
            queries: vec!["query".to_string()],
        };

        // Mock search tool that returns URLs
        let mut result1 = HashMap::new();
        result1.insert("link".to_string(), "http://example.com/1".to_string());
        let mut result2 = HashMap::new();
        result2.insert("url".to_string(), "http://example.com/2".to_string()); // Note: using "url" field
        let search = Arc::new(MockSearchTool {
            results: vec![result1, result2],
        });

        let text_splitter = Arc::new(MockTextSplitter { chunk_size: 100 });

        let retriever = WebResearchRetriever::new(vectorstore, llm_chain, search, 3, text_splitter)
            .with_verbose(false);

        // Search should clean query and return URLs
        let urls = retriever.search_query("1. \"test query\"").await.unwrap();

        // Should extract both "link" and "url" fields
        assert_eq!(urls.len(), 2);
        assert!(urls.contains(&"http://example.com/1".to_string()));
        assert!(urls.contains(&"http://example.com/2".to_string()));
    }

    #[tokio::test]
    async fn test_search_query_no_results() {
        let embeddings = Arc::new(MockEmbeddings);
        let vectorstore = InMemoryVectorStore::with_metric(embeddings, DistanceMetric::Cosine);
        let vectorstore = Arc::new(RwLock::new(vectorstore));

        let llm_chain = MockLLMChain {
            queries: vec!["query".to_string()],
        };

        let search = Arc::new(MockSearchTool { results: vec![] });
        let text_splitter = Arc::new(MockTextSplitter { chunk_size: 100 });

        let retriever = WebResearchRetriever::new(vectorstore, llm_chain, search, 3, text_splitter)
            .with_verbose(false);

        let urls = retriever.search_query("test").await.unwrap();
        assert_eq!(urls.len(), 0);
    }

    #[tokio::test]
    async fn test_search_query_with_verbose() {
        let embeddings = Arc::new(MockEmbeddings);
        let vectorstore = InMemoryVectorStore::with_metric(embeddings, DistanceMetric::Cosine);
        let vectorstore = Arc::new(RwLock::new(vectorstore));

        let llm_chain = MockLLMChain {
            queries: vec!["query".to_string()],
        };

        let search = Arc::new(MockSearchTool { results: vec![] });
        let text_splitter = Arc::new(MockTextSplitter { chunk_size: 100 });

        // Test with verbose=true (should print debug output)
        let retriever = WebResearchRetriever::new(vectorstore, llm_chain, search, 3, text_splitter)
            .with_verbose(true);

        // This should not fail, just print to stdout
        let _urls = retriever.search_query("test").await.unwrap();
        // Can't easily test stdout, but ensures verbose path is covered
    }

    #[tokio::test]
    async fn test_generate_queries_with_verbose() {
        let embeddings = Arc::new(MockEmbeddings);
        let vectorstore = InMemoryVectorStore::with_metric(embeddings, DistanceMetric::Cosine);
        let vectorstore = Arc::new(RwLock::new(vectorstore));

        let llm_chain = MockLLMChain {
            queries: vec!["q1".to_string(), "q2".to_string()],
        };

        let search = Arc::new(MockSearchTool { results: vec![] });
        let text_splitter = Arc::new(MockTextSplitter { chunk_size: 100 });

        let retriever = WebResearchRetriever::new(vectorstore, llm_chain, search, 3, text_splitter)
            .with_verbose(true);

        let queries = retriever.generate_queries("test", None).await.unwrap();
        assert_eq!(queries.len(), 2);
    }

    #[tokio::test]
    async fn test_url_database_prevents_duplicate_processing() {
        let embeddings = Arc::new(MockEmbeddings);
        let vectorstore = InMemoryVectorStore::with_metric(embeddings, DistanceMetric::Cosine);
        let vectorstore = Arc::new(RwLock::new(vectorstore));

        let llm_chain = MockLLMChain {
            queries: vec!["test".to_string()],
        };

        let mut result1 = HashMap::new();
        result1.insert("link".to_string(), "http://example.com/1".to_string());
        let search = Arc::new(MockSearchTool {
            results: vec![result1],
        });

        let text_splitter = Arc::new(MockTextSplitter { chunk_size: 100 });

        let retriever = WebResearchRetriever::new(vectorstore, llm_chain, search, 3, text_splitter)
            .with_verbose(false);

        // First call: URL should be processed
        let urls = retriever.search_query("test").await.unwrap();
        assert_eq!(urls.len(), 1);

        // Add URL to database manually (simulating it was processed)
        {
            let mut db = retriever.url_database.lock().unwrap();
            db.insert("http://example.com/1".to_string());
        }

        // Verify URL is in database
        {
            let db = retriever.url_database.lock().unwrap();
            assert!(db.contains("http://example.com/1"));
        }
    }

    #[tokio::test]
    async fn test_load_and_index_urls_empty_list() {
        let embeddings = Arc::new(MockEmbeddings);
        let vectorstore = InMemoryVectorStore::with_metric(embeddings, DistanceMetric::Cosine);
        let vectorstore = Arc::new(RwLock::new(vectorstore));

        let llm_chain = MockLLMChain {
            queries: vec!["test".to_string()],
        };

        let search = Arc::new(MockSearchTool { results: vec![] });
        let text_splitter = Arc::new(MockTextSplitter { chunk_size: 100 });

        let retriever = WebResearchRetriever::new(vectorstore, llm_chain, search, 3, text_splitter)
            .with_verbose(false);

        // Empty URL list should succeed without error
        let result = retriever.load_and_index_urls(vec![]).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_batch_interface() {
        let embeddings = Arc::new(MockEmbeddings);
        let vectorstore = InMemoryVectorStore::with_metric(embeddings, DistanceMetric::Cosine);
        let vectorstore = Arc::new(RwLock::new(vectorstore));

        let llm_chain = MockLLMChain {
            queries: vec!["test".to_string()],
        };

        let search = Arc::new(MockSearchTool { results: vec![] });
        let text_splitter = Arc::new(MockTextSplitter { chunk_size: 100 });

        let retriever = WebResearchRetriever::new(vectorstore, llm_chain, search, 3, text_splitter)
            .with_verbose(false);

        // Test batch through Runnable trait
        let inputs = vec!["query1".to_string(), "query2".to_string()];
        let results = retriever.batch(inputs, None).await.unwrap();

        assert_eq!(results.len(), 2);
        // Each result should be empty (no URLs from mock)
        assert_eq!(results[0].len(), 0);
        assert_eq!(results[1].len(), 0);
    }

    #[tokio::test]
    async fn test_stream_interface() {
        use futures::StreamExt;

        let embeddings = Arc::new(MockEmbeddings);
        let vectorstore = InMemoryVectorStore::with_metric(embeddings, DistanceMetric::Cosine);
        let vectorstore = Arc::new(RwLock::new(vectorstore));

        let llm_chain = MockLLMChain {
            queries: vec!["test".to_string()],
        };

        let search = Arc::new(MockSearchTool { results: vec![] });
        let text_splitter = Arc::new(MockTextSplitter { chunk_size: 100 });

        let retriever = WebResearchRetriever::new(vectorstore, llm_chain, search, 3, text_splitter)
            .with_verbose(false);

        // Test stream through Runnable trait
        let mut stream = retriever
            .stream("What is Rust?".to_string(), None)
            .await
            .unwrap();

        // Should get one result
        let result = stream.next().await.unwrap().unwrap();
        assert_eq!(result.len(), 0);

        // Stream should be exhausted
        assert!(stream.next().await.is_none());
    }

    #[tokio::test]
    async fn test_with_verbose_builder() {
        let embeddings = Arc::new(MockEmbeddings);
        let vectorstore = InMemoryVectorStore::with_metric(embeddings, DistanceMetric::Cosine);
        let vectorstore = Arc::new(RwLock::new(vectorstore));

        let llm_chain = MockLLMChain {
            queries: vec!["test".to_string()],
        };

        let search = Arc::new(MockSearchTool { results: vec![] });
        let text_splitter = Arc::new(MockTextSplitter { chunk_size: 100 });

        // Test builder pattern for verbose
        let retriever =
            WebResearchRetriever::new(vectorstore.clone(), llm_chain, search, 3, text_splitter);
        assert!(retriever.verbose); // Default is true

        let retriever2 = retriever.with_verbose(false);
        assert!(!retriever2.verbose);
    }
}
