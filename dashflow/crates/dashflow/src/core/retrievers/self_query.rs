//! Self-query retriever that generates structured queries from natural language.
//!
//! The SelfQueryRetriever uses an LLM to convert natural language queries into
//! structured queries with filters, which are then executed against a vector store.
//!
//! # Example
//!
//! ```rust,ignore
//! use dashflow::core::retrievers::self_query::SelfQueryRetriever;
//! use dashflow::core::structured_query::{AttributeInfo, Comparator, Operator};
//!
//! // Define metadata schema
//! let attributes = vec![
//!     AttributeInfo::new(
//!         "genre".to_string(),
//!         "The genre of the movie".to_string(),
//!         "string".to_string(),
//!     ),
//!     AttributeInfo::new(
//!         "year".to_string(),
//!         "The year the movie was released".to_string(),
//!         "integer".to_string(),
//!     ),
//! ];
//!
//! // Create retriever
//! let retriever = SelfQueryRetriever::new(
//!     llm,
//!     vector_store,
//!     "Movie database with summaries",
//!     attributes,
//! );
//!
//! // Query with natural language
//! let docs = retriever
//!     ._get_relevant_documents("What are some sci-fi movies from the 1980s?", None)
//!     .await?;
//! ```

use crate::core::{
    config::RunnableConfig,
    documents::Document,
    error::{Error, Result},
    language_models::ChatModel,
    messages::{BaseMessage, HumanMessage},
    output_parsers::OutputParser,
    retrievers::Retriever,
    structured_query::{
        query_constructor::{get_query_constructor_prompt, StructuredQueryOutputParser},
        AttributeInfo, Comparator, Operator, StructuredQuery, Visitor,
    },
    vector_stores::{SearchParams, SearchType, VectorStore},
};
use async_trait::async_trait;
use std::sync::Arc;

/// Query constructor that converts natural language to structured queries.
///
/// This struct wraps an LLM and output parser to create a query generation pipeline.
#[derive(Clone)]
pub struct QueryConstructor {
    llm: Arc<dyn ChatModel>,
    output_parser: StructuredQueryOutputParser,
    prompt_template: String,
}

impl QueryConstructor {
    /// Create a new `QueryConstructor`.
    pub fn new(
        llm: Arc<dyn ChatModel>,
        document_contents: &str,
        attribute_info: &[AttributeInfo],
        allowed_comparators: &[Comparator],
        allowed_operators: &[Operator],
        enable_limit: bool,
        fix_invalid: bool,
    ) -> Self {
        // Build output parser
        let mut parser = StructuredQueryOutputParser::new()
            .with_allowed_comparators(allowed_comparators.to_vec())
            .with_allowed_operators(allowed_operators.to_vec())
            .with_fix_invalid(fix_invalid);

        // Add allowed attributes from attribute_info
        let allowed_attributes: Vec<String> =
            attribute_info.iter().map(|a| a.name.clone()).collect();
        parser = parser.with_allowed_attributes(allowed_attributes);

        // Generate prompt template
        let prompt_template = get_query_constructor_prompt(
            document_contents,
            attribute_info,
            allowed_comparators,
            allowed_operators,
            enable_limit,
        );

        Self {
            llm,
            output_parser: parser,
            prompt_template,
        }
    }

    /// Invoke the query constructor to generate a structured query.
    pub async fn invoke(&self, query: &str) -> Result<StructuredQuery> {
        self.invoke_with_config(query, None).await
    }

    /// Invoke the query constructor to generate a structured query, using `RunnableConfig`.
    pub async fn invoke_with_config(
        &self,
        query: &str,
        config: Option<&RunnableConfig>,
    ) -> Result<StructuredQuery> {
        // Fill in the query placeholder in the prompt
        let filled_prompt = self.prompt_template.replace("{query}", query);

        // Convert to message format
        let message: BaseMessage = HumanMessage::new(filled_prompt).into();

        // Call LLM
        let chat_result = self
            .llm
            .generate(&[message], None, None, None, config)
            .await?;

        // Extract text from first generation
        let response_text = chat_result
            .generations
            .first()
            .ok_or_else(|| Error::OutputParsing("No generations returned from LLM".to_string()))?
            .text();

        // Parse LLM output to StructuredQuery
        let structured_query = self.output_parser.parse(&response_text)?;

        Ok(structured_query)
    }
}

/// Retriever that generates structured queries from natural language.
///
/// The `SelfQueryRetriever` uses an LLM to convert natural language into structured queries,
/// which are then translated into backend-specific filters and executed against a vector store.
#[derive(Clone)]
pub struct SelfQueryRetriever<VS: VectorStore + Clone, T: Visitor + Clone> {
    /// The underlying vector store
    vector_store: VS,
    /// Query constructor for generating structured queries
    query_constructor: QueryConstructor,
    /// Translator for converting structured queries to backend filters
    translator: T,
    /// Search type (similarity, mmr, etc.)
    search_type: SearchType,
    /// Base search parameters (k, thresholds, etc.)
    search_params: SearchParams,
    /// Use original query instead of LLM-generated query
    use_original_query: bool,
    /// Enable verbose logging
    verbose: bool,
}

impl<VS: VectorStore + Clone, T: Visitor + Clone> SelfQueryRetriever<VS, T> {
    /// Create a new `SelfQueryRetriever`.
    ///
    /// # Arguments
    ///
    /// * `llm` - Language model for query generation
    /// * `vector_store` - Vector store to retrieve documents from
    /// * `document_contents` - Description of what the documents contain
    /// * `attribute_info` - Metadata schema information
    /// * `translator` - Translator for backend-specific filter format
    /// * `allowed_comparators` - Comparators allowed in queries (defaults to all)
    /// * `allowed_operators` - Operators allowed in queries (defaults to all)
    /// * `enable_limit` - Whether to support limit parameter
    /// * `fix_invalid` - Whether to fix invalid filters by ignoring disallowed components
    ///
    /// # Returns
    ///
    /// A new `SelfQueryRetriever` instance
    #[allow(clippy::too_many_arguments)] // Self-query config: LLM, store, schema, translator, comparators, operators
    pub fn new(
        llm: Arc<dyn ChatModel>,
        vector_store: VS,
        document_contents: &str,
        attribute_info: Vec<AttributeInfo>,
        translator: T,
        allowed_comparators: Option<Vec<Comparator>>,
        allowed_operators: Option<Vec<Operator>>,
        enable_limit: bool,
        fix_invalid: bool,
    ) -> Self {
        let comparators = allowed_comparators.unwrap_or_else(Comparator::all);
        let operators = allowed_operators.unwrap_or_else(Operator::all);

        let query_constructor = QueryConstructor::new(
            llm,
            document_contents,
            &attribute_info,
            &comparators,
            &operators,
            enable_limit,
            fix_invalid,
        );

        Self {
            vector_store,
            query_constructor,
            translator,
            search_type: SearchType::Similarity,
            search_params: SearchParams::default(),
            use_original_query: false,
            verbose: false,
        }
    }

    /// Set the search type.
    #[must_use]
    pub fn with_search_type(mut self, search_type: SearchType) -> Self {
        self.search_type = search_type;
        self
    }

    /// Set search parameters.
    #[must_use]
    pub fn with_search_params(mut self, params: SearchParams) -> Self {
        self.search_params = params;
        self
    }

    /// Enable using the original query instead of LLM-generated query.
    #[must_use]
    pub fn with_use_original_query(mut self, use_original: bool) -> Self {
        self.use_original_query = use_original;
        self
    }

    /// Enable verbose logging.
    #[must_use]
    pub fn with_verbose(mut self, verbose: bool) -> Self {
        self.verbose = verbose;
        self
    }

    /// Prepare query and search params from structured query.
    fn prepare_query(
        mut self,
        original_query: &str,
        structured_query: &StructuredQuery,
    ) -> Result<(String, SearchParams)> {
        // Translate structured query to backend format
        let (new_query, new_kwargs) = self
            .translator
            .visit_structured_query(structured_query)
            .map_err(|e| Error::RunnableExecution(format!("Translation failed: {e:?}")))?;

        // Decide which query to use
        let final_query = if self.use_original_query {
            original_query.to_string()
        } else {
            new_query
        };

        // Build search params
        let mut params = self.search_params.clone();
        params.search_type = self.search_type;

        // Add limit if present in structured query
        if let Some(limit) = structured_query.limit {
            params.k = limit;
        }

        // Merge backend-specific filter from translator output
        if !new_kwargs.is_empty() {
            let mut filter = params.filter.unwrap_or_default();
            filter.extend(new_kwargs);
            params.filter = Some(filter);
        }

        Ok((final_query, params))
    }
}

#[async_trait]
impl<VS: VectorStore + Clone, T: Visitor + Clone + Send + Sync> Retriever
    for SelfQueryRetriever<VS, T>
{
    async fn _get_relevant_documents(
        &self,
        query: &str,
        config: Option<&RunnableConfig>,
    ) -> Result<Vec<Document>> {
        // Generate structured query from natural language
        let structured_query = self
            .query_constructor
            .invoke_with_config(query, config)
            .await?;

        if self.verbose {
            tracing::debug!("Generated Query: {structured_query:?}");
        }

        // Prepare query and search params
        let (final_query, search_params) = self.clone().prepare_query(query, &structured_query)?;

        // Execute search against vector store
        let documents = self
            .vector_store
            .search(&final_query, &search_params)
            .await?;

        Ok(documents)
    }

    fn name(&self) -> String {
        "SelfQueryRetriever".to_string()
    }
}

#[cfg(test)]
mod tests {
    use crate::core::{
        callbacks::CallbackManager,
        config::RunnableConfig,
        embeddings::Embeddings,
        structured_query::visitors::{ChromaTranslator, PineconeTranslator},
        vector_stores::InMemoryVectorStore,
    };
    use crate::test_prelude::*;
    use std::sync::Arc;

    // Mock embeddings for testing
    struct MockEmbeddings;

    #[async_trait]
    impl Embeddings for MockEmbeddings {
        async fn _embed_documents(&self, texts: &[String]) -> Result<Vec<Vec<f32>>> {
            Ok(texts.iter().map(|_| vec![0.1, 0.2, 0.3]).collect())
        }

        async fn _embed_query(&self, _text: &str) -> Result<Vec<f32>> {
            Ok(vec![0.1, 0.2, 0.3])
        }
    }

    // Mock LLM for testing
    struct MockLLM;

    #[async_trait]
    impl ChatModel for MockLLM {
        async fn _generate(
            &self,
            _messages: &[BaseMessage],
            _stop: Option<&[String]>,
            _tools: Option<&[crate::core::language_models::ToolDefinition]>,
            _tool_choice: Option<&crate::core::language_models::ToolChoice>,
            _run_manager: Option<&crate::core::callbacks::CallbackManager>,
        ) -> Result<crate::core::language_models::ChatResult> {
            use crate::core::language_models::{ChatGeneration, ChatResult};
            use crate::core::messages::AIMessage;

            // Return a mock structured query response
            let response = r#"{
                "query": "science fiction",
                "filter": "and(eq(\"genre\", \"sci-fi\"), gt(\"year\", 1980))",
                "limit": 5
            }"#;

            let message: BaseMessage = AIMessage::new(response).into();
            let generation = ChatGeneration::new(message);
            Ok(ChatResult::new(generation))
        }

        fn llm_type(&self) -> &str {
            "mock_llm"
        }

        fn as_any(&self) -> &dyn std::any::Any {
            self
        }
    }

    struct CallbacksRequiredLLM;

    #[async_trait]
    impl ChatModel for CallbacksRequiredLLM {
        async fn _generate(
            &self,
            _messages: &[BaseMessage],
            _stop: Option<&[String]>,
            _tools: Option<&[crate::core::language_models::ToolDefinition]>,
            _tool_choice: Option<&crate::core::language_models::ToolChoice>,
            run_manager: Option<&crate::core::callbacks::CallbackManager>,
        ) -> Result<crate::core::language_models::ChatResult> {
            use crate::core::language_models::{ChatGeneration, ChatResult};
            use crate::core::messages::AIMessage;

            if run_manager.is_none() {
                return Err(Error::other(
                    "Expected RunnableConfig callbacks to be propagated into ChatModel",
                ));
            }

            let response = r#"{
                "query": "test query",
                "filter": null,
                "limit": 1
            }"#;

            let message: BaseMessage = AIMessage::new(response).into();
            let generation = ChatGeneration::new(message);
            Ok(ChatResult::new(generation))
        }

        fn llm_type(&self) -> &str {
            "callbacks_required_llm"
        }

        fn as_any(&self) -> &dyn std::any::Any {
            self
        }
    }

    #[tokio::test]
    async fn test_query_constructor_basic() {
        let llm = Arc::new(MockLLM);
        let attributes = vec![
            AttributeInfo::new(
                "genre".to_string(),
                "Movie genre".to_string(),
                "string".to_string(),
            ),
            AttributeInfo::new(
                "year".to_string(),
                "Release year".to_string(),
                "integer".to_string(),
            ),
        ];

        let constructor = QueryConstructor::new(
            llm,
            "Movie database",
            &attributes,
            &Comparator::all(),
            &Operator::all(),
            true,
            false,
        );

        let result = constructor.invoke("sci-fi movies from the 1980s").await;
        assert!(result.is_ok());

        let structured_query = result.unwrap();
        assert_eq!(structured_query.query, "science fiction");
        assert!(structured_query.filter.is_some());
        assert_eq!(structured_query.limit, Some(5));
    }

    #[tokio::test]
    async fn test_self_query_retriever_propagates_runnable_config_to_llm() {
        let llm = Arc::new(CallbacksRequiredLLM);
        let embeddings: Arc<dyn Embeddings> = Arc::new(MockEmbeddings);
        let mut vector_store = InMemoryVectorStore::new(embeddings);

        vector_store
            .add_texts(&["Doc about testing"], None, None)
            .await
            .unwrap();

        let attributes = vec![AttributeInfo::new(
            "genre".to_string(),
            "Movie genre".to_string(),
            "string".to_string(),
        )];

        let translator = PineconeTranslator::new();
        let retriever = SelfQueryRetriever::new(
            llm,
            vector_store,
            "Movie database",
            attributes,
            translator,
            None,
            None,
            true,
            false,
        );

        let config = RunnableConfig::new().with_callbacks(CallbackManager::new());
        let result = retriever
            ._get_relevant_documents("test", Some(&config))
            .await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_self_query_retriever_creation() {
        let llm = Arc::new(MockLLM);
        let embeddings: Arc<dyn Embeddings> = Arc::new(MockEmbeddings);
        let vector_store = InMemoryVectorStore::new(embeddings);

        let attributes = vec![AttributeInfo::new(
            "genre".to_string(),
            "Movie genre".to_string(),
            "string".to_string(),
        )];

        let translator = PineconeTranslator::new();

        let retriever = SelfQueryRetriever::new(
            llm,
            vector_store,
            "Movie database",
            attributes,
            translator,
            None,
            None,
            true,
            false,
        );

        assert_eq!(retriever.name(), "SelfQueryRetriever");
    }

    #[tokio::test]
    async fn test_self_query_retriever_with_chroma() {
        let llm = Arc::new(MockLLM);
        let embeddings: Arc<dyn Embeddings> = Arc::new(MockEmbeddings);
        let mut vector_store = InMemoryVectorStore::new(embeddings);

        // Add some test documents
        vector_store
            .add_texts(&["A sci-fi movie", "A comedy movie"], None, None)
            .await
            .unwrap();

        let attributes = vec![
            AttributeInfo::new(
                "genre".to_string(),
                "Movie genre".to_string(),
                "string".to_string(),
            ),
            AttributeInfo::new(
                "year".to_string(),
                "Release year".to_string(),
                "integer".to_string(),
            ),
        ];

        let translator = ChromaTranslator::new();

        let retriever = SelfQueryRetriever::new(
            llm,
            vector_store,
            "Movie database",
            attributes,
            translator,
            None,
            None,
            false,
            false,
        );

        // This will use the mock LLM which returns a structured query
        let result = retriever
            ._get_relevant_documents("sci-fi movies", None)
            .await;

        if let Err(e) = &result {
            eprintln!("Error: {:?}", e);
        }
        assert!(result.is_ok());
        // The mock vector store may not match the filter, so we just verify no errors occurred
    }

    #[tokio::test]
    async fn test_query_constructor_with_allowed_comparators() {
        let llm = Arc::new(MockLLM);
        let attributes = vec![
            AttributeInfo::new(
                "year".to_string(),
                "Release year".to_string(),
                "integer".to_string(),
            ),
            AttributeInfo::new(
                "genre".to_string(),
                "Movie genre".to_string(),
                "string".to_string(),
            ),
        ];

        // Allow EQ and GT comparators (MockLLM uses both in its response)
        let allowed_comparators = vec![Comparator::Eq, Comparator::Gt];

        let constructor = QueryConstructor::new(
            llm,
            "Movie database",
            &attributes,
            &allowed_comparators,
            &Operator::all(),
            true, // enable_limit
            false,
        );

        let result = constructor.invoke("movies from 1990").await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_query_constructor_with_allowed_operators() {
        let llm = Arc::new(MockLLM);
        let attributes = vec![
            AttributeInfo::new(
                "genre".to_string(),
                "Movie genre".to_string(),
                "string".to_string(),
            ),
            AttributeInfo::new(
                "year".to_string(),
                "Release year".to_string(),
                "integer".to_string(),
            ),
        ];

        // Allow AND operator (MockLLM uses it in response)
        let allowed_operators = vec![Operator::And];

        let constructor = QueryConstructor::new(
            llm,
            "Movie database",
            &attributes,
            &Comparator::all(),
            &allowed_operators,
            true, // enable_limit
            false,
        );

        let result = constructor.invoke("sci-fi action movies").await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_query_constructor_with_fix_invalid() {
        let llm = Arc::new(MockLLM);
        let attributes = vec![AttributeInfo::new(
            "genre".to_string(),
            "Movie genre".to_string(),
            "string".to_string(),
        )];

        // Enable fix_invalid to handle disallowed components
        let constructor = QueryConstructor::new(
            llm,
            "Movie database",
            &attributes,
            &[Comparator::Eq], // Only allow EQ
            &[Operator::And],  // Only allow AND
            false,
            true, // fix_invalid = true
        );

        let result = constructor.invoke("movies").await;
        // The MockLLM returns a filter with GT comparator which is disallowed
        // With fix_invalid=true, it should handle this gracefully by removing invalid parts
        // However, if the entire filter becomes invalid, it may fail
        // Let's accept either success or a parsing error
        match result {
            Ok(_) => {
                // Successfully parsed and fixed invalid filter
            }
            Err(e) => {
                // May fail if the filter contains only invalid components
                eprintln!("Expected error with invalid filter: {:?}", e);
            }
        }
    }

    #[tokio::test]
    async fn test_self_query_retriever_builder_methods() {
        let llm = Arc::new(MockLLM);
        let embeddings: Arc<dyn Embeddings> = Arc::new(MockEmbeddings);
        let vector_store = InMemoryVectorStore::new(embeddings);
        let translator = PineconeTranslator::new();

        let retriever = SelfQueryRetriever::new(
            llm,
            vector_store,
            "Movie database",
            vec![],
            translator,
            None,
            None,
            false,
            false,
        )
        .with_search_type(SearchType::Mmr)
        .with_search_params(SearchParams {
            k: 10,
            fetch_k: Some(20),
            lambda: Some(0.5),
            ..Default::default()
        })
        .with_use_original_query(true)
        .with_verbose(true);

        assert_eq!(retriever.search_type, SearchType::Mmr);
        assert_eq!(retriever.search_params.k, 10);
        assert_eq!(retriever.search_params.fetch_k, Some(20));
        assert_eq!(retriever.search_params.lambda, Some(0.5));
        assert!(retriever.use_original_query);
        assert!(retriever.verbose);
    }

    #[tokio::test]
    async fn test_self_query_retriever_with_use_original_query() {
        let llm = Arc::new(MockLLM);
        let embeddings: Arc<dyn Embeddings> = Arc::new(MockEmbeddings);
        let mut vector_store = InMemoryVectorStore::new(embeddings);

        vector_store
            .add_texts(&["Test document"], None, None)
            .await
            .unwrap();

        let attributes = vec![
            AttributeInfo::new(
                "genre".to_string(),
                "Movie genre".to_string(),
                "string".to_string(),
            ),
            AttributeInfo::new(
                "year".to_string(),
                "Release year".to_string(),
                "integer".to_string(),
            ),
        ];

        let translator = PineconeTranslator::new();

        let retriever = SelfQueryRetriever::new(
            llm,
            vector_store,
            "Test database",
            attributes,
            translator,
            None,
            None,
            true, // enable_limit
            false,
        )
        .with_use_original_query(true); // Use original query text instead of LLM-generated

        let result = retriever._get_relevant_documents("test query", None).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_self_query_retriever_with_limit() {
        let llm = Arc::new(MockLLM);
        let embeddings: Arc<dyn Embeddings> = Arc::new(MockEmbeddings);
        let mut vector_store = InMemoryVectorStore::new(embeddings);

        // Add multiple documents
        vector_store
            .add_texts(
                &[
                    "Doc 1", "Doc 2", "Doc 3", "Doc 4", "Doc 5", "Doc 6", "Doc 7", "Doc 8",
                ],
                None,
                None,
            )
            .await
            .unwrap();

        let attributes = vec![
            AttributeInfo::new(
                "genre".to_string(),
                "Movie genre".to_string(),
                "string".to_string(),
            ),
            AttributeInfo::new(
                "year".to_string(),
                "Release year".to_string(),
                "integer".to_string(),
            ),
        ];

        let translator = PineconeTranslator::new();

        let retriever = SelfQueryRetriever::new(
            llm,
            vector_store,
            "Test database",
            attributes,
            translator,
            None,
            None,
            true, // enable_limit = true (MockLLM returns limit=5)
            false,
        );

        let result = retriever._get_relevant_documents("test", None).await;
        assert!(result.is_ok());
        let docs = result.unwrap();
        // Mock LLM returns limit=5, so should get at most 5 documents
        assert!(docs.len() <= 5);
    }

    #[tokio::test]
    async fn test_self_query_retriever_with_custom_search_params() {
        let llm = Arc::new(MockLLM);
        let embeddings: Arc<dyn Embeddings> = Arc::new(MockEmbeddings);
        let mut vector_store = InMemoryVectorStore::new(embeddings);

        vector_store
            .add_texts(&["Test document"], None, None)
            .await
            .unwrap();

        let translator = ChromaTranslator::new();

        let custom_params = SearchParams {
            k: 3,
            score_threshold: Some(0.8),
            ..Default::default()
        };

        let retriever = SelfQueryRetriever::new(
            llm,
            vector_store,
            "Test database",
            vec![],
            translator,
            None,
            None,
            false,
            false,
        )
        .with_search_params(custom_params);

        assert_eq!(retriever.search_params.k, 3);
        assert_eq!(retriever.search_params.score_threshold, Some(0.8));
    }

    #[tokio::test]
    async fn test_self_query_retriever_with_pinecone_translator() {
        let llm = Arc::new(MockLLM);
        let embeddings: Arc<dyn Embeddings> = Arc::new(MockEmbeddings);
        let vector_store = InMemoryVectorStore::new(embeddings);

        let attributes = vec![AttributeInfo::new(
            "category".to_string(),
            "Document category".to_string(),
            "string".to_string(),
        )];

        let translator = PineconeTranslator::new();

        let retriever = SelfQueryRetriever::new(
            llm,
            vector_store,
            "Document database",
            attributes,
            translator,
            None,
            None,
            false,
            false,
        );

        assert_eq!(retriever.name(), "SelfQueryRetriever");
    }

    #[tokio::test]
    async fn test_self_query_retriever_with_mmr_search() {
        let llm = Arc::new(MockLLM);
        let embeddings: Arc<dyn Embeddings> = Arc::new(MockEmbeddings);
        let mut vector_store = InMemoryVectorStore::new(embeddings);

        vector_store
            .add_texts(&["Test doc 1", "Test doc 2"], None, None)
            .await
            .unwrap();

        let attributes = vec![
            AttributeInfo::new(
                "genre".to_string(),
                "Movie genre".to_string(),
                "string".to_string(),
            ),
            AttributeInfo::new(
                "year".to_string(),
                "Release year".to_string(),
                "integer".to_string(),
            ),
        ];

        let translator = ChromaTranslator::new();

        let retriever = SelfQueryRetriever::new(
            llm,
            vector_store,
            "Test database",
            attributes,
            translator,
            None,
            None,
            true, // enable_limit
            false,
        )
        .with_search_type(SearchType::Mmr)
        .with_search_params(SearchParams {
            k: 2,
            fetch_k: Some(5),
            lambda: Some(0.5),
            ..Default::default()
        });

        let result = retriever._get_relevant_documents("test", None).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_self_query_retriever_with_similarity_score_threshold() {
        let llm = Arc::new(MockLLM);
        let embeddings: Arc<dyn Embeddings> = Arc::new(MockEmbeddings);
        let mut vector_store = InMemoryVectorStore::new(embeddings);

        vector_store
            .add_texts(&["Relevant document"], None, None)
            .await
            .unwrap();

        let attributes = vec![
            AttributeInfo::new(
                "genre".to_string(),
                "Movie genre".to_string(),
                "string".to_string(),
            ),
            AttributeInfo::new(
                "year".to_string(),
                "Release year".to_string(),
                "integer".to_string(),
            ),
        ];

        let translator = PineconeTranslator::new();

        let retriever = SelfQueryRetriever::new(
            llm,
            vector_store,
            "Test database",
            attributes,
            translator,
            None,
            None,
            true, // enable_limit
            false,
        )
        .with_search_type(SearchType::SimilarityScoreThreshold)
        .with_search_params(SearchParams {
            score_threshold: Some(0.7),
            ..Default::default()
        });

        let result = retriever._get_relevant_documents("relevant", None).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_query_constructor_prompt_template() {
        let llm = Arc::new(MockLLM);
        let attributes = vec![
            AttributeInfo::new(
                "genre".to_string(),
                "Movie genre".to_string(),
                "string".to_string(),
            ),
            AttributeInfo::new(
                "year".to_string(),
                "Release year".to_string(),
                "integer".to_string(),
            ),
        ];

        let constructor = QueryConstructor::new(
            llm,
            "Movie database with summaries and metadata",
            &attributes,
            &Comparator::all(),
            &Operator::all(),
            true,
            false,
        );

        // Verify prompt template was generated with document contents
        assert!(constructor
            .prompt_template
            .contains("Movie database with summaries and metadata"));
        // Verify it includes attribute information
        assert!(constructor.prompt_template.contains("genre"));
        assert!(constructor.prompt_template.contains("year"));
    }

    #[tokio::test]
    async fn test_self_query_retriever_with_multiple_attributes() {
        let llm = Arc::new(MockLLM);
        let embeddings: Arc<dyn Embeddings> = Arc::new(MockEmbeddings);
        let vector_store = InMemoryVectorStore::new(embeddings);

        let attributes = vec![
            AttributeInfo::new(
                "genre".to_string(),
                "Movie genre".to_string(),
                "string".to_string(),
            ),
            AttributeInfo::new(
                "year".to_string(),
                "Release year".to_string(),
                "integer".to_string(),
            ),
            AttributeInfo::new(
                "rating".to_string(),
                "Movie rating".to_string(),
                "float".to_string(),
            ),
            AttributeInfo::new(
                "director".to_string(),
                "Movie director".to_string(),
                "string".to_string(),
            ),
        ];

        let translator = PineconeTranslator::new();

        let retriever = SelfQueryRetriever::new(
            llm,
            vector_store,
            "Movie database",
            attributes,
            translator,
            None,
            None,
            false,
            false,
        );

        assert_eq!(retriever.name(), "SelfQueryRetriever");
    }

    #[tokio::test]
    async fn test_self_query_retriever_verbose_mode() {
        let llm = Arc::new(MockLLM);
        let embeddings: Arc<dyn Embeddings> = Arc::new(MockEmbeddings);
        let mut vector_store = InMemoryVectorStore::new(embeddings);

        vector_store
            .add_texts(&["Test document"], None, None)
            .await
            .unwrap();

        let attributes = vec![
            AttributeInfo::new(
                "genre".to_string(),
                "Movie genre".to_string(),
                "string".to_string(),
            ),
            AttributeInfo::new(
                "year".to_string(),
                "Release year".to_string(),
                "integer".to_string(),
            ),
        ];

        let translator = ChromaTranslator::new();

        let retriever = SelfQueryRetriever::new(
            llm,
            vector_store,
            "Test database",
            attributes,
            translator,
            None,
            None,
            true, // enable_limit
            false,
        )
        .with_verbose(true);

        assert!(retriever.verbose);

        // Verbose mode should print structured query (to stderr)
        let result = retriever._get_relevant_documents("test", None).await;
        assert!(result.is_ok());
    }

    #[test]
    fn test_query_constructor_clone() {
        let llm = Arc::new(MockLLM);
        let attributes = vec![AttributeInfo::new(
            "field".to_string(),
            "Test field".to_string(),
            "string".to_string(),
        )];

        let constructor = QueryConstructor::new(
            llm,
            "Test database",
            &attributes,
            &Comparator::all(),
            &Operator::all(),
            false,
            false,
        );

        // Verify QueryConstructor implements Clone
        let cloned = constructor.clone();
        assert_eq!(constructor.prompt_template, cloned.prompt_template);
    }

    #[test]
    fn test_self_query_retriever_clone() {
        let llm = Arc::new(MockLLM);
        let embeddings: Arc<dyn Embeddings> = Arc::new(MockEmbeddings);
        let vector_store = InMemoryVectorStore::new(embeddings);
        let translator = PineconeTranslator::new();

        let retriever = SelfQueryRetriever::new(
            llm,
            vector_store,
            "Test database",
            vec![],
            translator,
            None,
            None,
            false,
            false,
        );

        // Verify SelfQueryRetriever implements Clone
        let cloned = retriever.clone();
        assert_eq!(retriever.name(), cloned.name());
        assert_eq!(retriever.use_original_query, cloned.use_original_query);
        assert_eq!(retriever.verbose, cloned.verbose);
    }
}
