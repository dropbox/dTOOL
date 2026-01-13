//! Vector store-backed memory for semantic retrieval of conversation history.
//!
//! Stores conversation in a vector store and retrieves semantically relevant
//! past conversations based on the current input.

use crate::{utils::get_prompt_input_key, BaseMemory, MemoryError, MemoryResult};
use async_trait::async_trait;
use dashflow::core::{documents::Document, vector_stores::VectorStore};
use std::collections::HashMap;

/// Memory backed by a `VectorStoreRetriever`.
///
/// Stores conversation history in a vector store and retrieves semantically relevant
/// past conversations based on the current input. This enables the system to recall
/// relevant context from earlier in long conversations, even if not immediately recent.
///
/// # Example
///
/// ```rust,ignore
/// use dashflow_memory::VectorStoreRetrieverMemory;
/// use dashflow::core::vector_stores::InMemoryVectorStore;
/// use dashflow::core::retrievers::VectorStoreRetriever;
///
/// // Create a vector store and retriever
/// let vector_store = InMemoryVectorStore::new(embeddings);
/// let retriever = VectorStoreRetriever::from_vectorstore(vector_store);
///
/// // Create memory
/// let memory = VectorStoreRetrieverMemory::new(retriever);
///
/// // Use in a conversation
/// memory.save_context(&inputs, &outputs).await?;
/// let vars = memory.load_memory_variables(&inputs).await?;
/// ```
///
/// # Python Baseline Compatibility
///
/// Matches `dashflow.memory.vectorstore.VectorStoreRetrieverMemory`.
/// Source: ~/`dashflow/libs/dashflow/dashflow_classic/memory/vectorstore.py` (lines 23-123)
#[derive(Clone)]
pub struct VectorStoreRetrieverMemory<VS>
where
    VS: VectorStore,
{
    /// The vector store to use for storage and retrieval
    vector_store: VS,

    /// Key name to locate the memories in the result of `load_memory_variables`
    memory_key: String,

    /// Key name to index the inputs to `load_memory_variables`
    input_key: Option<String>,

    /// Whether to return Document objects or joined strings
    return_docs: bool,

    /// Input keys to exclude when constructing the document (in addition to `memory_key`)
    exclude_input_keys: Vec<String>,

    /// Number of documents to retrieve (k parameter)
    k: usize,
}

impl<VS> VectorStoreRetrieverMemory<VS>
where
    VS: VectorStore,
{
    /// Create a new `VectorStoreRetrieverMemory`.
    ///
    /// # Arguments
    ///
    /// * `vector_store` - The vector store to use for storage and retrieval
    pub fn new(vector_store: VS) -> Self {
        Self {
            vector_store,
            memory_key: "history".to_string(),
            input_key: None,
            return_docs: false,
            exclude_input_keys: Vec::new(),
            k: 4, // Default from Python
        }
    }

    /// Set the memory key used in `load_memory_variables` output.
    pub fn with_memory_key(mut self, key: impl Into<String>) -> Self {
        self.memory_key = key.into();
        self
    }

    /// Set the input key to use for querying.
    pub fn with_input_key(mut self, key: impl Into<String>) -> Self {
        self.input_key = Some(key.into());
        self
    }

    /// Set whether to return Document objects (true) or joined strings (false).
    pub fn with_return_docs(mut self, return_docs: bool) -> Self {
        self.return_docs = return_docs;
        self
    }

    /// Set input keys to exclude when constructing documents.
    pub fn with_exclude_input_keys(mut self, keys: Vec<String>) -> Self {
        self.exclude_input_keys = keys;
        self
    }

    /// Set the number of documents to retrieve.
    pub fn with_k(mut self, k: usize) -> Self {
        self.k = k;
        self
    }

    /// Get the input key for the prompt from the inputs dict.
    fn get_prompt_input_key(&self, inputs: &HashMap<String, String>) -> MemoryResult<String> {
        if let Some(ref key) = self.input_key {
            return Ok(key.clone());
        }
        get_prompt_input_key(inputs, std::slice::from_ref(&self.memory_key))
            .map_err(|e| MemoryError::OperationFailed(e.to_string()))
    }

    /// Convert retrieved documents to memory variables.
    fn documents_to_memory_variables(
        &self,
        docs: Vec<Document>,
    ) -> MemoryResult<HashMap<String, String>> {
        let mut result = HashMap::new();

        if self.return_docs {
            // Return documents as JSON string
            let docs_json: Vec<serde_json::Value> = docs
                .into_iter()
                .map(|doc| {
                    serde_json::json!({
                        "page_content": doc.page_content,
                        "metadata": doc.metadata,
                    })
                })
                .collect();
            let json_str = serde_json::to_string(&docs_json)?;
            result.insert(self.memory_key.clone(), json_str);
        } else {
            // Join page_content with newlines
            let text = docs
                .into_iter()
                .map(|doc| doc.page_content)
                .collect::<Vec<_>>()
                .join("\n");
            result.insert(self.memory_key.clone(), text);
        }

        Ok(result)
    }

    /// Format context from this conversation turn into documents.
    fn form_documents(
        &self,
        inputs: &HashMap<String, String>,
        outputs: &HashMap<String, String>,
    ) -> Vec<Document> {
        // Build set of keys to exclude
        let mut exclude = self.exclude_input_keys.clone();
        exclude.push(self.memory_key.clone());
        let exclude_set: std::collections::HashSet<_> = exclude.into_iter().collect();

        // Filter inputs to exclude certain keys
        let filtered_inputs: HashMap<_, _> = inputs
            .iter()
            .filter(|(k, _)| !exclude_set.contains(k.as_str()))
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();

        // Build text lines from inputs and outputs
        let mut texts = Vec::new();
        for (k, v) in &filtered_inputs {
            texts.push(format!("{k}: {v}"));
        }
        for (k, v) in outputs {
            texts.push(format!("{k}: {v}"));
        }

        let page_content = texts.join("\n");
        vec![Document::new(page_content)]
    }
}

#[async_trait]
impl<VS> BaseMemory for VectorStoreRetrieverMemory<VS>
where
    VS: VectorStore + Send + Sync,
{
    fn memory_variables(&self) -> Vec<String> {
        vec![self.memory_key.clone()]
    }

    async fn load_memory_variables(
        &self,
        inputs: &HashMap<String, String>,
    ) -> MemoryResult<HashMap<String, String>> {
        // Get the input key to use for querying
        let input_key = self.get_prompt_input_key(inputs)?;
        let query = inputs.get(&input_key).ok_or_else(|| {
            MemoryError::OperationFailed(format!("Input key '{input_key}' not found in inputs"))
        })?;

        // Perform similarity search on vector store
        let docs = self
            .vector_store
            ._similarity_search(query, self.k, None)
            .await
            .map_err(|e| MemoryError::OperationFailed(format!("Vector search failed: {e}")))?;

        // Convert documents to memory variables
        self.documents_to_memory_variables(docs)
    }

    async fn save_context(
        &mut self,
        inputs: &HashMap<String, String>,
        outputs: &HashMap<String, String>,
    ) -> MemoryResult<()> {
        // Form documents from this conversation turn
        let documents = self.form_documents(inputs, outputs);

        // Add documents to vector store
        self.vector_store
            .add_documents(&documents, None)
            .await
            .map_err(|e| MemoryError::OperationFailed(format!("Failed to add documents: {e}")))?;

        Ok(())
    }

    async fn clear(&mut self) -> MemoryResult<()> {
        // VectorStoreRetrieverMemory doesn't implement clear in Python
        // The vector store persists all data
        Ok(())
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use dashflow::core::{embeddings::Embeddings, vector_stores::InMemoryVectorStore};
    use std::sync::Arc;

    // Mock embeddings for testing
    struct MockEmbeddings;

    #[async_trait]
    impl Embeddings for MockEmbeddings {
        async fn _embed_documents(
            &self,
            texts: &[String],
        ) -> dashflow::core::error::Result<Vec<Vec<f32>>> {
            // Return simple embeddings based on text length
            Ok(texts
                .iter()
                .map(|t| vec![t.len() as f32, 0.5, 0.1])
                .collect())
        }

        async fn _embed_query(&self, text: &str) -> dashflow::core::error::Result<Vec<f32>> {
            Ok(vec![text.len() as f32, 0.5, 0.1])
        }
    }

    #[tokio::test]
    async fn test_vector_store_retriever_memory_basic() {
        let embeddings = Arc::new(MockEmbeddings);
        let vector_store = InMemoryVectorStore::new(embeddings);

        let mut memory = VectorStoreRetrieverMemory::new(vector_store).with_k(2);

        // Save first conversation turn
        let mut inputs1 = HashMap::new();
        inputs1.insert("input".to_string(), "What is Rust?".to_string());

        let mut outputs1 = HashMap::new();
        outputs1.insert(
            "output".to_string(),
            "Rust is a systems programming language.".to_string(),
        );

        memory.save_context(&inputs1, &outputs1).await.unwrap();

        // Save second conversation turn
        let mut inputs2 = HashMap::new();
        inputs2.insert("input".to_string(), "What about Python?".to_string());

        let mut outputs2 = HashMap::new();
        outputs2.insert(
            "output".to_string(),
            "Python is a high-level language.".to_string(),
        );

        memory.save_context(&inputs2, &outputs2).await.unwrap();

        // Load memory - should retrieve relevant past conversations
        let mut query_inputs = HashMap::new();
        query_inputs.insert(
            "input".to_string(),
            "Tell me about programming languages".to_string(),
        );

        let vars = memory.load_memory_variables(&query_inputs).await.unwrap();

        // Should have history key
        assert!(vars.contains_key("history"));

        // Should be a string (not docs)
        let history = vars.get("history").unwrap();

        // Should contain content from previous conversations
        assert!(!history.is_empty());
    }

    #[tokio::test]
    async fn test_vector_store_retriever_memory_return_docs() {
        let embeddings = Arc::new(MockEmbeddings);
        let vector_store = InMemoryVectorStore::new(embeddings);

        let mut memory = VectorStoreRetrieverMemory::new(vector_store)
            .with_return_docs(true)
            .with_k(1);

        // Save a conversation turn
        let mut inputs = HashMap::new();
        inputs.insert("input".to_string(), "Hello".to_string());

        let mut outputs = HashMap::new();
        outputs.insert("output".to_string(), "Hi there!".to_string());

        memory.save_context(&inputs, &outputs).await.unwrap();

        // Load memory with return_docs=true
        let mut query_inputs = HashMap::new();
        query_inputs.insert("input".to_string(), "Hello again".to_string());

        let vars = memory.load_memory_variables(&query_inputs).await.unwrap();

        // Should return documents as JSON string
        let history = vars.get("history").unwrap();

        // Parse JSON string
        let docs: Vec<serde_json::Value> = serde_json::from_str(history).unwrap();
        assert!(!docs.is_empty());

        // First doc should have page_content and metadata
        let doc = &docs[0];
        assert!(doc.get("page_content").is_some());
        assert!(doc.get("metadata").is_some());
    }

    #[tokio::test]
    async fn test_vector_store_retriever_memory_exclude_keys() {
        let embeddings = Arc::new(MockEmbeddings);
        let vector_store = InMemoryVectorStore::new(embeddings);

        let mut memory = VectorStoreRetrieverMemory::new(vector_store)
            .with_exclude_input_keys(vec!["system_prompt".to_string()])
            .with_k(1);

        // Save with excluded key
        let mut inputs = HashMap::new();
        inputs.insert("input".to_string(), "Hello".to_string());
        inputs.insert(
            "system_prompt".to_string(),
            "You are a helpful assistant".to_string(),
        );

        let mut outputs = HashMap::new();
        outputs.insert("output".to_string(), "Hi!".to_string());

        memory.save_context(&inputs, &outputs).await.unwrap();

        // Load memory
        let mut query_inputs = HashMap::new();
        query_inputs.insert("input".to_string(), "Hello again".to_string());

        let vars = memory.load_memory_variables(&query_inputs).await.unwrap();
        let history = vars.get("history").unwrap();

        // Should not contain the excluded system_prompt
        assert!(!history.contains("You are a helpful assistant"));
        // Should contain the input and output
        assert!(history.contains("Hello") || history.contains("Hi"));
    }

    #[tokio::test]
    async fn test_vector_store_retriever_memory_custom_keys() {
        let embeddings = Arc::new(MockEmbeddings);
        let vector_store = InMemoryVectorStore::new(embeddings);

        let mut memory = VectorStoreRetrieverMemory::new(vector_store)
            .with_memory_key("context")
            .with_input_key("question")
            .with_k(1);

        // Save with custom keys
        let mut inputs = HashMap::new();
        inputs.insert("question".to_string(), "What is 2+2?".to_string());

        let mut outputs = HashMap::new();
        outputs.insert("answer".to_string(), "4".to_string());

        memory.save_context(&inputs, &outputs).await.unwrap();

        // Load memory using custom keys
        let mut query_inputs = HashMap::new();
        query_inputs.insert("question".to_string(), "What is 3+3?".to_string());

        let vars = memory.load_memory_variables(&query_inputs).await.unwrap();

        // Should use custom memory key
        assert!(vars.contains_key("context"));
        assert!(!vars.contains_key("history"));
    }

    #[tokio::test]
    async fn test_memory_variables() {
        let embeddings = Arc::new(MockEmbeddings);
        let vector_store = InMemoryVectorStore::new(embeddings);

        let memory = VectorStoreRetrieverMemory::new(vector_store).with_memory_key("past");

        let vars = memory.memory_variables();
        assert_eq!(vars, vec!["past".to_string()]);
    }

    #[tokio::test]
    async fn test_clear() {
        let embeddings = Arc::new(MockEmbeddings);
        let vector_store = InMemoryVectorStore::new(embeddings);

        let mut memory = VectorStoreRetrieverMemory::new(vector_store);

        // Clear should succeed (no-op in Python baseline)
        assert!(memory.clear().await.is_ok());
    }

    #[tokio::test]
    async fn test_empty_vector_store_memory() {
        let embeddings = Arc::new(MockEmbeddings);
        let vector_store = InMemoryVectorStore::new(embeddings);

        let memory = VectorStoreRetrieverMemory::new(vector_store);

        // Query empty memory
        let mut query_inputs = HashMap::new();
        query_inputs.insert("input".to_string(), "test query".to_string());

        let vars = memory.load_memory_variables(&query_inputs).await.unwrap();

        // Should have history key
        assert!(vars.contains_key("history"));

        // Should be empty string since no documents stored
        let history = vars.get("history").unwrap();
        assert_eq!(history, "");
    }

    #[tokio::test]
    async fn test_single_document_save_and_retrieval() {
        let embeddings = Arc::new(MockEmbeddings);
        let vector_store = InMemoryVectorStore::new(embeddings);

        let mut memory = VectorStoreRetrieverMemory::new(vector_store).with_k(1);

        // Save single conversation turn
        let mut inputs = HashMap::new();
        inputs.insert("input".to_string(), "Hello world".to_string());

        let mut outputs = HashMap::new();
        outputs.insert("output".to_string(), "Hi there!".to_string());

        memory.save_context(&inputs, &outputs).await.unwrap();

        // Load memory - should retrieve the saved document
        let mut query_inputs = HashMap::new();
        query_inputs.insert("input".to_string(), "Hello".to_string());

        let vars = memory.load_memory_variables(&query_inputs).await.unwrap();
        let history = vars.get("history").unwrap();

        // Should contain both input and output
        assert!(history.contains("Hello world"));
        assert!(history.contains("Hi there!"));
    }

    #[tokio::test]
    async fn test_k_parameter_limits_retrieval() {
        let embeddings = Arc::new(MockEmbeddings);
        let vector_store = InMemoryVectorStore::new(embeddings);

        let mut memory = VectorStoreRetrieverMemory::new(vector_store).with_k(2);

        // Save 5 conversation turns
        for i in 1..=5 {
            let mut inputs = HashMap::new();
            inputs.insert("input".to_string(), format!("Message {}", i));

            let mut outputs = HashMap::new();
            outputs.insert("output".to_string(), format!("Response {}", i));

            memory.save_context(&inputs, &outputs).await.unwrap();
        }

        // Load memory with k=2
        let mut query_inputs = HashMap::new();
        query_inputs.insert("input".to_string(), "test query".to_string());

        let vars = memory.load_memory_variables(&query_inputs).await.unwrap();
        let history = vars.get("history").unwrap();

        // Should retrieve at most 2 documents (k=2)
        // With k=2, we expect at most 2 documents
        // Each document contains "input: ..." and "output: ..." lines
        // So we should see at most 2 occurrences of "Message" (one per document)
        let message_count = history.matches("Message").count();
        assert!(
            message_count <= 2,
            "Expected at most 2 documents (k=2), got {} messages",
            message_count
        );
    }

    #[tokio::test]
    async fn test_very_large_k_parameter() {
        let embeddings = Arc::new(MockEmbeddings);
        let vector_store = InMemoryVectorStore::new(embeddings);

        let mut memory = VectorStoreRetrieverMemory::new(vector_store).with_k(1000);

        // Save only 3 documents
        for i in 1..=3 {
            let mut inputs = HashMap::new();
            inputs.insert("input".to_string(), format!("Message {}", i));

            let mut outputs = HashMap::new();
            outputs.insert("output".to_string(), format!("Response {}", i));

            memory.save_context(&inputs, &outputs).await.unwrap();
        }

        // Load memory with k=1000 (much larger than available documents)
        let mut query_inputs = HashMap::new();
        query_inputs.insert("input".to_string(), "test query".to_string());

        // Should not error even though k > available documents
        let vars = memory.load_memory_variables(&query_inputs).await.unwrap();
        let history = vars.get("history").unwrap();

        // Should retrieve all 3 documents (not error out)
        // Count "Message" occurrences (one per document)
        let message_count = history.matches("Message").count();
        assert!(
            message_count <= 3,
            "Expected at most 3 documents, got {} messages",
            message_count
        );
    }

    #[tokio::test]
    async fn test_missing_input_key_error() {
        let embeddings = Arc::new(MockEmbeddings);
        let vector_store = InMemoryVectorStore::new(embeddings);

        let memory = VectorStoreRetrieverMemory::new(vector_store).with_input_key("question");

        // Load memory without the expected input key
        let mut query_inputs = HashMap::new();
        query_inputs.insert("input".to_string(), "test query".to_string());

        // Should error because "question" key is not present
        let result = memory.load_memory_variables(&query_inputs).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("question"));
    }

    #[tokio::test]
    async fn test_unicode_and_special_characters() {
        let embeddings = Arc::new(MockEmbeddings);
        let vector_store = InMemoryVectorStore::new(embeddings);

        let mut memory = VectorStoreRetrieverMemory::new(vector_store).with_k(5);

        // Save conversations with Unicode characters
        let test_cases = vec![
            ("Chinese", "你好世界", "欢迎来到Rust"),
            ("Arabic", "مرحبا", "كيف حالك"),
            ("Emoji", "Hello 🌍", "Welcome 👋"),
            ("Mixed", "Test ñ ü ö", "Response with émojis 🚀"),
        ];

        for (name, input_text, output_text) in test_cases {
            let mut inputs = HashMap::new();
            inputs.insert("input".to_string(), format!("{}: {}", name, input_text));

            let mut outputs = HashMap::new();
            outputs.insert("output".to_string(), output_text.to_string());

            memory.save_context(&inputs, &outputs).await.unwrap();
        }

        // Load memory - should handle Unicode correctly
        let mut query_inputs = HashMap::new();
        query_inputs.insert("input".to_string(), "你好".to_string());

        let vars = memory.load_memory_variables(&query_inputs).await.unwrap();
        let history = vars.get("history").unwrap();

        // Should not be empty and should contain valid UTF-8
        assert!(!history.is_empty());
        // Verify it's valid UTF-8 by checking we can iterate over chars
        assert!(history.chars().count() > 0);
    }

    #[tokio::test]
    async fn test_default_k_value() {
        let embeddings = Arc::new(MockEmbeddings);
        let vector_store = InMemoryVectorStore::new(embeddings);

        let mut memory = VectorStoreRetrieverMemory::new(vector_store);

        // Save 10 documents
        for i in 1..=10 {
            let mut inputs = HashMap::new();
            inputs.insert("input".to_string(), format!("Message {}", i));

            let mut outputs = HashMap::new();
            outputs.insert("output".to_string(), format!("Response {}", i));

            memory.save_context(&inputs, &outputs).await.unwrap();
        }

        // Load memory with default k (should be 4 per Python baseline)
        let mut query_inputs = HashMap::new();
        query_inputs.insert("input".to_string(), "test query".to_string());

        let vars = memory.load_memory_variables(&query_inputs).await.unwrap();
        let history = vars.get("history").unwrap();

        // Should retrieve at most 4 documents (default k)
        // Count "Message" occurrences (one per document)
        let message_count = history.matches("Message").count();
        assert!(
            message_count <= 4,
            "Expected at most 4 documents (default k=4), got {} messages",
            message_count
        );
    }

    #[tokio::test]
    async fn test_form_documents_excludes_memory_key() {
        let embeddings = Arc::new(MockEmbeddings);
        let vector_store = InMemoryVectorStore::new(embeddings);

        let mut memory = VectorStoreRetrieverMemory::new(vector_store).with_memory_key("history");

        // Save with history key in inputs (should be excluded)
        let mut inputs = HashMap::new();
        inputs.insert("input".to_string(), "Hello".to_string());
        inputs.insert("history".to_string(), "Previous history here".to_string());

        let mut outputs = HashMap::new();
        outputs.insert("output".to_string(), "Hi!".to_string());

        memory.save_context(&inputs, &outputs).await.unwrap();

        // Load memory
        let mut query_inputs = HashMap::new();
        query_inputs.insert("input".to_string(), "Hello again".to_string());

        let vars = memory.load_memory_variables(&query_inputs).await.unwrap();
        let history = vars.get("history").unwrap();

        // Should not contain the excluded history key content
        assert!(!history.contains("Previous history here"));
        // Should contain the input and output
        assert!(history.contains("Hello") || history.contains("Hi"));
    }
}
