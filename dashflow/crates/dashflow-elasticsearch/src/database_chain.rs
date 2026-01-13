// Allow clippy warnings for this module
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::clone_on_ref_ptr)]
#![allow(clippy::needless_pass_by_value, clippy::redundant_clone)]
// ! Elasticsearch Database Chain for natural language to Elasticsearch DSL query conversion
//!
//! This module provides a chain that converts natural language questions into Elasticsearch
//! DSL queries, executes them, and generates natural language answers from the results.
//!
//! # Architecture
//!
//! The chain follows a two-step process:
//! 1. **Query Generation**: LLM converts the question + index schema into an Elasticsearch DSL query (JSON)
//! 2. **Answer Generation**: LLM converts the question + query results into a natural language answer
//!
//! # Example
//!
//! ```rust,no_run
//! use dashflow_elasticsearch::ElasticsearchDatabaseChain;
//! use dashflow_openai::ChatOpenAI;
//! use elasticsearch::Elasticsearch;
//! use std::sync::Arc;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! // Connect to Elasticsearch
//! let es_client = Elasticsearch::default();
//!
//! // Create LLM
//! let llm = Arc::new(ChatOpenAI::default());
//!
//! // Create chain
//! let chain = ElasticsearchDatabaseChain::from_llm(
//!     llm,
//!     es_client,
//! )?;
//!
//! // Ask a question
//! let result = chain.run("What are the top 5 products by sales?").await?;
//! println!("Answer: {}", result);
//! # Ok(())
//! # }
//! ```

use dashflow::core::{
    config::RunnableConfig,
    error::{Error as DashFlowError, Result},
    language_models::ChatModel,
    messages::{BaseMessage, HumanMessage},
    output_parsers::{JsonOutputParser, OutputParser},
    prompts::PromptTemplate,
};
use elasticsearch::{Elasticsearch, SearchParts};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use std::collections::HashMap;
use std::sync::Arc;

/// Default prompt template for generating Elasticsearch DSL queries
const DEFAULT_DSL_TEMPLATE: &str = r"Given an input question, create a syntactically correct Elasticsearch query to run. Unless the user specifies in their question a specific number of examples they wish to obtain, always limit your query to at most {top_k} results. You can order the results by a relevant column to return the most interesting examples in the database.

Unless told to do not query for all the columns from a specific index, only ask for a few relevant columns given the question.

Pay attention to use only the column names that you can see in the mapping description. Be careful to not query for columns that do not exist. Also, pay attention to which column is in which index. Return the query as valid json.

Use the following format:

Question: Question here
ESQuery: Elasticsearch Query formatted as json

Only use the following Elasticsearch indices:
{indices_info}

Question: {input}
ESQuery:";

/// Default prompt template for generating answers from Elasticsearch results
const DEFAULT_ANSWER_TEMPLATE: &str = r"Given an input question and relevant data from a database, answer the user question.

Use the following format:

Question: Question here
Data: Relevant data here
Answer: Final answer here

Question: {input}
Data: {data}
Answer:";

/// Configuration for `ElasticsearchDatabaseChain`
#[derive(Debug, Clone)]
pub struct ElasticsearchDatabaseChainConfig {
    /// Number of results to return from the query (default: 10)
    pub top_k: usize,
    /// Indices to ignore when listing available indices
    pub ignore_indices: Option<Vec<String>>,
    /// Indices to include (if specified, only these indices will be used)
    pub include_indices: Option<Vec<String>>,
    /// Number of sample documents to include in index info (default: 3)
    pub sample_documents_in_index_info: usize,
    /// Whether to return intermediate steps (query, results) along with final answer
    pub return_intermediate_steps: bool,
}

impl Default for ElasticsearchDatabaseChainConfig {
    fn default() -> Self {
        Self {
            top_k: 10,
            ignore_indices: None,
            include_indices: None,
            sample_documents_in_index_info: 3,
            return_intermediate_steps: false,
        }
    }
}

/// Output from `ElasticsearchDatabaseChain`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ElasticsearchDatabaseChainOutput {
    /// The final answer to the user's question
    pub result: String,
    /// Intermediate steps (if `return_intermediate_steps` is true)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub intermediate_steps: Option<Vec<JsonValue>>,
}

/// Chain for converting natural language questions to Elasticsearch queries
///
/// This chain:
/// 1. Lists available Elasticsearch indices
/// 2. Gets schema information (mappings) for those indices
/// 3. Uses an LLM to convert the question into an Elasticsearch DSL query
/// 4. Executes the query
/// 5. Uses an LLM to convert the results into a natural language answer
///
/// # Example
///
/// ```rust,no_run
/// use dashflow_elasticsearch::ElasticsearchDatabaseChain;
/// use dashflow_openai::ChatOpenAI;
/// use elasticsearch::Elasticsearch;
/// use std::sync::Arc;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let es_client = Elasticsearch::default();
/// let llm = Arc::new(ChatOpenAI::default());
///
/// let chain = ElasticsearchDatabaseChain::from_llm(llm, es_client)?;
/// let result = chain.run("Show me recent errors").await?;
/// # Ok(())
/// # }
/// ```
pub struct ElasticsearchDatabaseChain {
    /// Language model for query and answer generation
    llm: Arc<dyn ChatModel>,
    /// Elasticsearch client
    database: Elasticsearch,
    /// Configuration
    config: ElasticsearchDatabaseChainConfig,
    /// Prompt template for query generation
    query_prompt: PromptTemplate,
    /// Prompt template for answer generation
    answer_prompt: PromptTemplate,
}

impl std::fmt::Debug for ElasticsearchDatabaseChain {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ElasticsearchDatabaseChain")
            .field("llm", &"<ChatModel>")
            .field("database", &"<Elasticsearch>")
            .field("config", &self.config)
            .field("query_prompt", &self.query_prompt)
            .field("answer_prompt", &self.answer_prompt)
            .finish()
    }
}

impl ElasticsearchDatabaseChain {
    /// Create a new `ElasticsearchDatabaseChain`
    ///
    /// # Arguments
    ///
    /// * `llm` - Language model to use for query and answer generation
    /// * `database` - Elasticsearch client
    /// * `config` - Configuration options
    /// * `query_prompt` - Optional custom prompt for query generation
    /// * `answer_prompt` - Optional custom prompt for answer generation
    pub fn new(
        llm: Arc<dyn ChatModel>,
        database: Elasticsearch,
        config: ElasticsearchDatabaseChainConfig,
        query_prompt: Option<PromptTemplate>,
        answer_prompt: Option<PromptTemplate>,
    ) -> Result<Self> {
        // Validate that we don't have both include and ignore indices
        if config.include_indices.is_some() && config.ignore_indices.is_some() {
            return Err(DashFlowError::invalid_input(
                "Cannot specify both 'include_indices' and 'ignore_indices'",
            ));
        }

        let query_prompt = query_prompt.unwrap_or_else(|| {
            PromptTemplate::from_template(DEFAULT_DSL_TEMPLATE)
                .expect("Default DSL template should be valid")
        });

        let answer_prompt = answer_prompt.unwrap_or_else(|| {
            PromptTemplate::from_template(DEFAULT_ANSWER_TEMPLATE)
                .expect("Default answer template should be valid")
        });

        Ok(Self {
            llm,
            database,
            config,
            query_prompt,
            answer_prompt,
        })
    }

    /// Convenience constructor using default configuration
    ///
    /// # Arguments
    ///
    /// * `llm` - Language model for query and answer generation
    /// * `database` - Elasticsearch client
    pub fn from_llm(llm: Arc<dyn ChatModel>, database: Elasticsearch) -> Result<Self> {
        Self::new(
            llm,
            database,
            ElasticsearchDatabaseChainConfig::default(),
            None,
            None,
        )
    }

    /// Set the configuration
    pub fn with_config(mut self, config: ElasticsearchDatabaseChainConfig) -> Result<Self> {
        // Validate
        if config.include_indices.is_some() && config.ignore_indices.is_some() {
            return Err(DashFlowError::invalid_input(
                "Cannot specify both 'include_indices' and 'ignore_indices'",
            ));
        }
        self.config = config;
        Ok(self)
    }

    /// Set custom query prompt
    #[must_use]
    pub fn with_query_prompt(mut self, prompt: PromptTemplate) -> Self {
        self.query_prompt = prompt;
        self
    }

    /// Set custom answer prompt
    #[must_use]
    pub fn with_answer_prompt(mut self, prompt: PromptTemplate) -> Self {
        self.answer_prompt = prompt;
        self
    }

    /// List all available Elasticsearch indices, filtered by config
    async fn list_indices(&self) -> Result<Vec<String>> {
        use elasticsearch::cat::CatIndicesParts;

        // Get all indices
        let response = self
            .database
            .cat()
            .indices(CatIndicesParts::None)
            .format("json")
            .send()
            .await
            .map_err(|e| DashFlowError::elasticsearch(format!("Failed to list indices: {e}")))?;

        let body = response
            .json::<Vec<HashMap<String, JsonValue>>>()
            .await
            .map_err(|e| {
                DashFlowError::elasticsearch(format!("Failed to parse indices response: {e}"))
            })?;

        let mut all_indices: Vec<String> = body
            .iter()
            .filter_map(|index| {
                index
                    .get("index")
                    .and_then(|v| v.as_str())
                    .map(String::from)
            })
            .collect();

        // Filter by include_indices
        if let Some(include) = &self.config.include_indices {
            all_indices.retain(|idx| include.contains(idx));
        }

        // Filter by ignore_indices
        if let Some(ignore) = &self.config.ignore_indices {
            all_indices.retain(|idx| !ignore.contains(idx));
        }

        Ok(all_indices)
    }

    /// Get mapping information for indices, including sample documents
    async fn get_indices_info(&self, indices: &[String]) -> Result<String> {
        use elasticsearch::indices::IndicesGetMappingParts;

        if indices.is_empty() {
            return Ok(String::new());
        }

        // Get mappings for all indices
        let index_refs: Vec<&str> = indices.iter().map(std::string::String::as_str).collect();
        let mapping_response = self
            .database
            .indices()
            .get_mapping(IndicesGetMappingParts::Index(&index_refs))
            .send()
            .await
            .map_err(|e| {
                DashFlowError::elasticsearch(format!("Failed to get index mappings: {e}"))
            })?;

        let mut mappings: HashMap<String, JsonValue> = mapping_response
            .json()
            .await
            .map_err(|e| DashFlowError::elasticsearch(format!("Failed to parse mappings: {e}")))?;

        // Add sample documents if configured
        if self.config.sample_documents_in_index_info > 0 {
            for index in indices {
                if let Some(mapping) = mappings.get_mut(index) {
                    // Fetch sample documents using match_all query
                    let sample_query = serde_json::json!({
                        "query": {"match_all": {}},
                        "size": self.config.sample_documents_in_index_info
                    });

                    if let Ok(response) = self
                        .database
                        .search(SearchParts::Index(&[index.as_str()]))
                        .body(sample_query)
                        .send()
                        .await
                    {
                        if let Ok(result) = response.json::<JsonValue>().await {
                            if let Some(hits) = result.get("hits").and_then(|h| h.get("hits")) {
                                if let Some(hit_array) = hits.as_array() {
                                    let sample_docs: Vec<String> = hit_array
                                        .iter()
                                        .filter_map(|hit| {
                                            hit.get("_source").map(|src| format!("{src}"))
                                        })
                                        .collect();

                                    if !sample_docs.is_empty() {
                                        // Append sample documents to mapping (mimicking Python format)
                                        let mapping_str = mapping.to_string();
                                        let samples = sample_docs.join("\n");
                                        let combined =
                                            format!("{mapping_str}\n\n/*\n{samples}\n*/");
                                        *mapping = serde_json::json!({"mappings": combined});
                                    }
                                }
                            }
                        }
                    } else {
                        // Ignore errors fetching sample documents, just use mapping
                    }
                }
            }
        }

        // Format mappings as a string (matching Python format)
        let info = indices
            .iter()
            .filter_map(|index| {
                mappings.get(index).map(|mapping| {
                    // Extract the "mappings" field if it exists, otherwise use the whole mapping
                    let mapping_content = if let Some(mappings_field) = mapping.get("mappings") {
                        if mappings_field.is_string() {
                            // Already formatted with samples
                            mappings_field.as_str().unwrap_or("").to_string()
                        } else {
                            serde_json::to_string_pretty(mappings_field).unwrap_or_default()
                        }
                    } else {
                        serde_json::to_string_pretty(mapping).unwrap_or_default()
                    };

                    format!("Mapping for index {index}:\n{mapping_content}")
                })
            })
            .collect::<Vec<_>>()
            .join("\n\n");

        Ok(info)
    }

    /// Execute an Elasticsearch query
    async fn execute_search(&self, indices: &[String], query: &str) -> Result<String> {
        // Parse query as JSON
        let query_json: JsonValue = serde_json::from_str(query).map_err(|e| {
            DashFlowError::invalid_input(format!("Invalid Elasticsearch query JSON: {e}"))
        })?;

        // Execute search
        let response = self
            .database
            .search(SearchParts::Index(
                &indices
                    .iter()
                    .map(std::string::String::as_str)
                    .collect::<Vec<_>>(),
            ))
            .body(query_json)
            .send()
            .await
            .map_err(|e| DashFlowError::elasticsearch(format!("Failed to execute search: {e}")))?;

        // Get response as JSON
        let result: JsonValue = response
            .json()
            .await
            .map_err(|e| DashFlowError::elasticsearch(format!("Failed to parse results: {e}")))?;

        // Convert to pretty-printed string
        Ok(serde_json::to_string_pretty(&result).unwrap_or_else(|_| result.to_string()))
    }

    /// Run the chain with a question and return the answer
    ///
    /// # Arguments
    ///
    /// * `question` - The natural language question to answer
    pub async fn run(&self, question: &str) -> Result<String> {
        let output = self.call(question, None).await?;
        Ok(output.result)
    }

    /// Run the chain with a question and optional config
    ///
    /// # Arguments
    ///
    /// * `question` - The natural language question to answer
    /// * `config` - Optional runnable configuration
    ///
    /// # Returns
    ///
    /// Output containing the answer and optionally intermediate steps
    pub async fn call(
        &self,
        question: &str,
        _config: Option<&RunnableConfig>,
    ) -> Result<ElasticsearchDatabaseChainOutput> {
        let mut intermediate_steps = Vec::new();

        // Step 1: List indices
        let indices = self.list_indices().await?;
        if indices.is_empty() {
            return Err(DashFlowError::invalid_input(
                "No Elasticsearch indices available to query",
            ));
        }

        // Step 2: Get indices info (mappings + samples)
        let indices_info = self.get_indices_info(&indices).await?;

        // Step 3: Generate Elasticsearch query using LLM
        let query_input = format!("{question}\nESQuery:");
        let query_vars = HashMap::from([
            ("input".to_string(), query_input.clone()),
            ("top_k".to_string(), self.config.top_k.to_string()),
            ("indices_info".to_string(), indices_info.clone()),
        ]);

        if self.config.return_intermediate_steps {
            intermediate_steps.push(serde_json::to_value(&query_vars).unwrap_or_default());
        }

        let query_prompt_str = self.query_prompt.format(&query_vars)?;
        let query_message: BaseMessage = HumanMessage::new(query_prompt_str).into();
        let query_response = self
            .llm
            .generate(&[query_message], None, None, None, None)
            .await?;

        let es_query_text = query_response
            .generations
            .first()
            .ok_or_else(|| DashFlowError::api("No query generated by LLM"))?
            .message
            .content()
            .as_text();

        // Parse the LLM output as JSON (handles markdown code blocks)
        let json_parser = JsonOutputParser::new();
        let es_query_json = json_parser.parse(&es_query_text)?;

        // Convert parsed JSON back to string for execute_search
        let es_query = serde_json::to_string(&es_query_json)
            .map_err(|e| DashFlowError::api(format!("Failed to serialize query: {e}")))?;

        if self.config.return_intermediate_steps {
            intermediate_steps.push(serde_json::json!({"es_query": &es_query}));
        }

        // Step 4: Execute query
        let search_results = self.execute_search(&indices, &es_query).await?;

        if self.config.return_intermediate_steps {
            intermediate_steps.push(serde_json::json!({"search_results": &search_results}));
        }

        // Step 5: Generate answer using LLM
        let answer_vars = HashMap::from([
            ("input".to_string(), query_input),
            ("data".to_string(), search_results),
        ]);

        let answer_prompt_str = self.answer_prompt.format(&answer_vars)?;
        let answer_message: BaseMessage = HumanMessage::new(answer_prompt_str).into();
        let answer_response = self
            .llm
            .generate(&[answer_message], None, None, None, None)
            .await?;

        let final_answer = answer_response
            .generations
            .first()
            .ok_or_else(|| DashFlowError::api("No answer generated by LLM"))?
            .message
            .content()
            .as_text();

        if self.config.return_intermediate_steps {
            intermediate_steps.push(serde_json::json!({"final_answer": &final_answer}));
        }

        Ok(ElasticsearchDatabaseChainOutput {
            result: final_answer,
            intermediate_steps: if self.config.return_intermediate_steps {
                Some(intermediate_steps)
            } else {
                None
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use dashflow::core::{
        language_models::{ChatGeneration, ChatModel, ChatResult, ToolChoice, ToolDefinition},
        messages::AIMessage,
    };

    /// Mock ChatModel for testing
    struct MockChatModel {
        /// Response to return - can be set to simulate different LLM behaviors
        response: String,
    }

    impl MockChatModel {
        fn new(response: impl Into<String>) -> Self {
            Self {
                response: response.into(),
            }
        }

        /// Returns a query response (JSON Elasticsearch query)
        fn query_response() -> Self {
            Self::new(r#"{"query": {"match_all": {}}, "size": 10}"#)
        }
    }

    #[async_trait]
    impl ChatModel for MockChatModel {
        async fn _generate(
            &self,
            _messages: &[BaseMessage],
            _stop: Option<&[String]>,
            _tools: Option<&[ToolDefinition]>,
            _tool_choice: Option<&ToolChoice>,
            _run_manager: Option<&dashflow::core::callbacks::CallbackManager>,
        ) -> Result<ChatResult> {
            let generation = ChatGeneration::new(AIMessage::new(self.response.clone()).into());
            Ok(ChatResult::new(generation))
        }

        fn llm_type(&self) -> &str {
            "mock"
        }

        fn as_any(&self) -> &dyn std::any::Any {
            self
        }
    }

    #[test]
    fn test_default_config() {
        let config = ElasticsearchDatabaseChainConfig::default();
        assert_eq!(config.top_k, 10);
        assert_eq!(config.sample_documents_in_index_info, 3);
        assert!(!config.return_intermediate_steps);
        assert!(config.include_indices.is_none());
        assert!(config.ignore_indices.is_none());
    }

    #[test]
    fn test_config_validation_both_include_and_ignore() {
        let llm = Arc::new(MockChatModel::query_response());
        let database = Elasticsearch::default();

        let config = ElasticsearchDatabaseChainConfig {
            include_indices: Some(vec!["index1".to_string()]),
            ignore_indices: Some(vec!["index2".to_string()]),
            ..Default::default()
        };

        let result = ElasticsearchDatabaseChain::new(llm, database, config, None, None);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Cannot specify both"));
    }

    #[test]
    fn test_config_validation_only_include() {
        let llm = Arc::new(MockChatModel::query_response());
        let database = Elasticsearch::default();

        let config = ElasticsearchDatabaseChainConfig {
            include_indices: Some(vec!["index1".to_string()]),
            ..Default::default()
        };

        let result = ElasticsearchDatabaseChain::new(llm, database, config, None, None);
        assert!(result.is_ok());
    }

    #[test]
    fn test_config_validation_only_ignore() {
        let llm = Arc::new(MockChatModel::query_response());
        let database = Elasticsearch::default();

        let config = ElasticsearchDatabaseChainConfig {
            ignore_indices: Some(vec!["index1".to_string()]),
            ..Default::default()
        };

        let result = ElasticsearchDatabaseChain::new(llm, database, config, None, None);
        assert!(result.is_ok());
    }

    #[test]
    fn test_with_config_validation() {
        let llm = Arc::new(MockChatModel::query_response());
        let database = Elasticsearch::default();

        let chain = ElasticsearchDatabaseChain::from_llm(llm, database).unwrap();

        // Valid config
        let config = ElasticsearchDatabaseChainConfig {
            top_k: 5,
            ..Default::default()
        };
        assert!(chain.with_config(config).is_ok());

        // Invalid config (both include and ignore)
        let llm2 = Arc::new(MockChatModel::query_response());
        let database2 = Elasticsearch::default();
        let chain2 = ElasticsearchDatabaseChain::from_llm(llm2, database2).unwrap();

        let invalid_config = ElasticsearchDatabaseChainConfig {
            include_indices: Some(vec!["index1".to_string()]),
            ignore_indices: Some(vec!["index2".to_string()]),
            ..Default::default()
        };
        assert!(chain2.with_config(invalid_config).is_err());
    }

    #[test]
    fn test_custom_prompts() {
        let llm = Arc::new(MockChatModel::query_response());
        let database = Elasticsearch::default();

        let query_prompt =
            PromptTemplate::from_template("Custom query: {input} {top_k} {indices_info}").unwrap();
        let answer_prompt = PromptTemplate::from_template("Custom answer: {input} {data}").unwrap();

        let chain = ElasticsearchDatabaseChain::new(
            llm,
            database,
            ElasticsearchDatabaseChainConfig::default(),
            Some(query_prompt),
            Some(answer_prompt),
        )
        .unwrap();

        // Verify chain was created successfully with custom prompts
        assert!(chain.query_prompt.template.contains("Custom query"));
        assert!(chain.answer_prompt.template.contains("Custom answer"));
    }

    #[test]
    fn test_default_prompts() {
        let llm = Arc::new(MockChatModel::query_response());
        let database = Elasticsearch::default();

        let chain = ElasticsearchDatabaseChain::from_llm(llm, database).unwrap();

        // Verify default prompts are used
        assert!(chain
            .query_prompt
            .template
            .contains("syntactically correct Elasticsearch query"));
        assert!(chain
            .answer_prompt
            .template
            .contains("answer the user question"));
    }

    #[test]
    fn test_chain_builder_pattern() {
        let llm = Arc::new(MockChatModel::query_response());
        let database = Elasticsearch::default();

        let custom_query_prompt =
            PromptTemplate::from_template("Query: {input} {top_k} {indices_info}").unwrap();
        let custom_answer_prompt = PromptTemplate::from_template("Answer: {input} {data}").unwrap();

        let chain = ElasticsearchDatabaseChain::from_llm(llm, database)
            .unwrap()
            .with_query_prompt(custom_query_prompt)
            .with_answer_prompt(custom_answer_prompt);

        assert!(chain.query_prompt.template.contains("Query:"));
        assert!(chain.answer_prompt.template.contains("Answer:"));
    }

    #[test]
    fn test_config_values() {
        let config = ElasticsearchDatabaseChainConfig {
            top_k: 25,
            ignore_indices: Some(vec!["system".to_string(), "logs".to_string()]),
            include_indices: None,
            sample_documents_in_index_info: 5,
            return_intermediate_steps: true,
        };

        assert_eq!(config.top_k, 25);
        assert_eq!(config.sample_documents_in_index_info, 5);
        assert!(config.return_intermediate_steps);
        assert!(config.ignore_indices.is_some());
        assert_eq!(config.ignore_indices.as_ref().unwrap().len(), 2);
    }

    // ========================================================================
    // ADDITIONAL UNIT TESTS
    // ========================================================================

    // Output serialization tests
    #[test]
    fn test_output_serialization() {
        let output = ElasticsearchDatabaseChainOutput {
            result: "The answer is 42".to_string(),
            intermediate_steps: None,
        };
        let json = serde_json::to_string(&output).unwrap();
        assert!(json.contains("The answer is 42"));
        assert!(!json.contains("intermediate_steps"));
    }

    #[test]
    fn test_output_serialization_with_steps() {
        let output = ElasticsearchDatabaseChainOutput {
            result: "Answer here".to_string(),
            intermediate_steps: Some(vec![
                serde_json::json!({"step": 1}),
                serde_json::json!({"step": 2}),
            ]),
        };
        let json = serde_json::to_string(&output).unwrap();
        assert!(json.contains("Answer here"));
        assert!(json.contains("intermediate_steps"));
        assert!(json.contains("step"));
    }

    #[test]
    fn test_output_deserialization() {
        let json = r#"{"result": "Test result"}"#;
        let output: ElasticsearchDatabaseChainOutput = serde_json::from_str(json).unwrap();
        assert_eq!(output.result, "Test result");
        assert!(output.intermediate_steps.is_none());
    }

    #[test]
    fn test_output_deserialization_with_steps() {
        let json = r#"{"result": "Answer", "intermediate_steps": [{"query": "test"}]}"#;
        let output: ElasticsearchDatabaseChainOutput = serde_json::from_str(json).unwrap();
        assert_eq!(output.result, "Answer");
        assert!(output.intermediate_steps.is_some());
        assert_eq!(output.intermediate_steps.unwrap().len(), 1);
    }

    #[test]
    fn test_output_clone() {
        let output = ElasticsearchDatabaseChainOutput {
            result: "Cloned".to_string(),
            intermediate_steps: Some(vec![serde_json::json!({"data": "value"})]),
        };
        let cloned = output.clone();
        assert_eq!(cloned.result, "Cloned");
        assert_eq!(cloned.intermediate_steps.unwrap().len(), 1);
    }

    #[test]
    fn test_output_debug() {
        let output = ElasticsearchDatabaseChainOutput {
            result: "Debug test".to_string(),
            intermediate_steps: None,
        };
        let debug = format!("{:?}", output);
        assert!(debug.contains("Debug test"));
    }

    // Config edge cases
    #[test]
    fn test_config_empty_include_indices() {
        let config = ElasticsearchDatabaseChainConfig {
            include_indices: Some(vec![]),
            ..Default::default()
        };
        assert!(config.include_indices.unwrap().is_empty());
    }

    #[test]
    fn test_config_empty_ignore_indices() {
        let config = ElasticsearchDatabaseChainConfig {
            ignore_indices: Some(vec![]),
            ..Default::default()
        };
        assert!(config.ignore_indices.unwrap().is_empty());
    }

    #[test]
    fn test_config_zero_top_k() {
        let config = ElasticsearchDatabaseChainConfig {
            top_k: 0,
            ..Default::default()
        };
        assert_eq!(config.top_k, 0);
    }

    #[test]
    fn test_config_large_top_k() {
        let config = ElasticsearchDatabaseChainConfig {
            top_k: 1000,
            ..Default::default()
        };
        assert_eq!(config.top_k, 1000);
    }

    #[test]
    fn test_config_zero_sample_documents() {
        let config = ElasticsearchDatabaseChainConfig {
            sample_documents_in_index_info: 0,
            ..Default::default()
        };
        assert_eq!(config.sample_documents_in_index_info, 0);
    }

    // Prompt template tests
    #[test]
    fn test_default_dsl_template_variables() {
        assert!(DEFAULT_DSL_TEMPLATE.contains("{top_k}"));
        assert!(DEFAULT_DSL_TEMPLATE.contains("{indices_info}"));
        assert!(DEFAULT_DSL_TEMPLATE.contains("{input}"));
    }

    #[test]
    fn test_default_answer_template_variables() {
        assert!(DEFAULT_ANSWER_TEMPLATE.contains("{input}"));
        assert!(DEFAULT_ANSWER_TEMPLATE.contains("{data}"));
    }

    #[test]
    fn test_custom_prompt_creation() {
        let template = "Custom: {input} {top_k} {indices_info}";
        let prompt = PromptTemplate::from_template(template).unwrap();
        assert!(prompt.template.contains("Custom"));
    }

    #[test]
    fn test_answer_prompt_format() {
        let prompt = PromptTemplate::from_template(DEFAULT_ANSWER_TEMPLATE).unwrap();
        assert!(prompt.template.contains("Question:"));
        assert!(prompt.template.contains("Data:"));
        assert!(prompt.template.contains("Answer:"));
    }

    // JSON parsing tests
    #[test]
    fn test_parse_valid_es_query() {
        let query_str = r#"{"query": {"match_all": {}}}"#;
        let query: JsonValue = serde_json::from_str(query_str).unwrap();
        assert!(query.get("query").is_some());
    }

    #[test]
    fn test_parse_complex_es_query() {
        let query_str = r#"{
            "query": {
                "bool": {
                    "must": [{"match": {"title": "test"}}],
                    "filter": [{"term": {"status": "published"}}]
                }
            },
            "size": 10
        }"#;
        let query: JsonValue = serde_json::from_str(query_str).unwrap();
        assert!(query["query"]["bool"]["must"].is_array());
    }

    #[test]
    fn test_parse_aggs_query() {
        let query_str = r#"{
            "size": 0,
            "aggs": {
                "categories": {
                    "terms": {"field": "category.keyword"}
                }
            }
        }"#;
        let query: JsonValue = serde_json::from_str(query_str).unwrap();
        assert!(query.get("aggs").is_some());
    }

    #[test]
    fn test_parse_invalid_json() {
        let invalid = "not valid json";
        let result: std::result::Result<JsonValue, _> = serde_json::from_str(invalid);
        assert!(result.is_err());
    }

    // Index filtering logic tests
    #[test]
    fn test_filter_include_indices() {
        let all_indices = vec!["logs".to_string(), "users".to_string(), "products".to_string()];
        let include = vec!["users".to_string(), "products".to_string()];

        let filtered: Vec<String> = all_indices
            .into_iter()
            .filter(|idx| include.contains(idx))
            .collect();

        assert_eq!(filtered.len(), 2);
        assert!(!filtered.contains(&"logs".to_string()));
    }

    #[test]
    fn test_filter_ignore_indices() {
        let all_indices = vec!["logs".to_string(), "users".to_string(), "products".to_string()];
        let ignore = vec!["logs".to_string()];

        let filtered: Vec<String> = all_indices
            .into_iter()
            .filter(|idx| !ignore.contains(idx))
            .collect();

        assert_eq!(filtered.len(), 2);
        assert!(!filtered.contains(&"logs".to_string()));
    }

    #[test]
    fn test_filter_empty_result() {
        let all_indices = vec!["logs".to_string()];
        let include = vec!["users".to_string()];

        let filtered: Vec<String> = all_indices
            .into_iter()
            .filter(|idx| include.contains(idx))
            .collect();

        assert!(filtered.is_empty());
    }

    // Mapping format tests
    #[test]
    fn test_format_mapping_info() {
        let index = "test_index";
        let mapping = serde_json::json!({
            "properties": {
                "title": {"type": "text"},
                "count": {"type": "integer"}
            }
        });
        let info = format!(
            "Mapping for index {}:\n{}",
            index,
            serde_json::to_string_pretty(&mapping).unwrap()
        );
        assert!(info.contains("test_index"));
        assert!(info.contains("title"));
    }

    #[test]
    fn test_format_multiple_mappings() {
        let indices = vec!["index1".to_string(), "index2".to_string()];
        let info: Vec<String> = indices
            .iter()
            .map(|idx| format!("Mapping for index {}:\n{{}}", idx))
            .collect();
        let combined = info.join("\n\n");
        assert!(combined.contains("index1"));
        assert!(combined.contains("index2"));
    }

    // Search result parsing tests
    #[test]
    fn test_parse_search_hits() {
        let response = serde_json::json!({
            "hits": {
                "total": {"value": 100},
                "hits": [
                    {"_id": "1", "_source": {"title": "Doc 1"}},
                    {"_id": "2", "_source": {"title": "Doc 2"}}
                ]
            }
        });
        let hits = response["hits"]["hits"].as_array().unwrap();
        assert_eq!(hits.len(), 2);
    }

    #[test]
    fn test_parse_empty_search_result() {
        let response = serde_json::json!({
            "hits": {
                "total": {"value": 0},
                "hits": []
            }
        });
        let hits = response["hits"]["hits"].as_array().unwrap();
        assert!(hits.is_empty());
    }

    #[test]
    fn test_parse_search_with_aggs() {
        let response = serde_json::json!({
            "hits": {"hits": []},
            "aggregations": {
                "categories": {
                    "buckets": [
                        {"key": "tech", "doc_count": 10},
                        {"key": "science", "doc_count": 5}
                    ]
                }
            }
        });
        assert!(response.get("aggregations").is_some());
        let buckets = response["aggregations"]["categories"]["buckets"].as_array().unwrap();
        assert_eq!(buckets.len(), 2);
    }

    // Intermediate steps tests
    #[test]
    fn test_intermediate_steps_query_vars() {
        let query_vars = HashMap::from([
            ("input".to_string(), "test question".to_string()),
            ("top_k".to_string(), "10".to_string()),
            ("indices_info".to_string(), "index mappings here".to_string()),
        ]);
        let json = serde_json::to_value(&query_vars).unwrap();
        assert_eq!(json["input"], "test question");
    }

    #[test]
    fn test_intermediate_steps_es_query() {
        let es_query = r#"{"query": {"match_all": {}}}"#;
        let step = serde_json::json!({"es_query": es_query});
        assert!(step["es_query"].as_str().unwrap().contains("match_all"));
    }

    #[test]
    fn test_intermediate_steps_search_results() {
        let results = r#"{"hits": {"hits": []}}"#;
        let step = serde_json::json!({"search_results": results});
        assert!(step["search_results"].as_str().unwrap().contains("hits"));
    }

    // Mock response tests
    #[test]
    fn test_mock_chat_model_query_response() {
        let response = r#"{"query": {"match_all": {}}, "size": 10}"#;
        let mock = MockChatModel::new(response);
        assert_eq!(mock.response, response);
    }

    #[test]
    fn test_mock_chat_model_answer_response() {
        let response = "Based on the data, the answer is 42.";
        let mock = MockChatModel::new(response);
        assert_eq!(mock.response, response);
    }

    // Chain debug format test
    #[test]
    fn test_chain_debug_format() {
        let llm = Arc::new(MockChatModel::query_response());
        let database = Elasticsearch::default();
        let chain = ElasticsearchDatabaseChain::from_llm(llm, database).unwrap();
        let debug = format!("{:?}", chain);
        assert!(debug.contains("ElasticsearchDatabaseChain"));
        assert!(debug.contains("<ChatModel>"));
        assert!(debug.contains("<Elasticsearch>"));
    }

    // Config clone and debug tests
    #[test]
    fn test_config_clone() {
        let config = ElasticsearchDatabaseChainConfig {
            top_k: 15,
            include_indices: Some(vec!["test".to_string()]),
            ignore_indices: None,
            sample_documents_in_index_info: 2,
            return_intermediate_steps: true,
        };
        let cloned = config.clone();
        assert_eq!(cloned.top_k, 15);
        assert_eq!(cloned.include_indices.unwrap().len(), 1);
    }

    #[test]
    fn test_config_debug() {
        let config = ElasticsearchDatabaseChainConfig::default();
        let debug = format!("{:?}", config);
        assert!(debug.contains("top_k"));
        assert!(debug.contains("10"));
    }

    // Error message tests
    #[test]
    fn test_invalid_input_error_message() {
        let error = DashFlowError::invalid_input("Cannot specify both 'include_indices' and 'ignore_indices'");
        let msg = error.to_string();
        assert!(msg.contains("Cannot specify both"));
    }

    // Response formatting tests
    #[test]
    fn test_format_response_pretty() {
        let result = serde_json::json!({"key": "value"});
        let formatted = serde_json::to_string_pretty(&result).unwrap();
        assert!(formatted.contains('\n'));
    }

    #[test]
    fn test_format_response_compact() {
        let result = serde_json::json!({"key": "value"});
        let formatted = result.to_string();
        assert!(!formatted.contains('\n'));
    }

    // Integration tests (require running Elasticsearch instance)
    // These are marked with #[ignore] and should be run explicitly with:
    // cargo test -- --ignored

    #[tokio::test]
    #[ignore = "requires Elasticsearch on localhost:9200"]
    async fn test_integration_list_indices() {
        // Requires: Elasticsearch running on localhost:9200
        let llm = Arc::new(MockChatModel::query_response());
        let database = Elasticsearch::default();

        let chain = ElasticsearchDatabaseChain::from_llm(llm, database).unwrap();

        // This will fail if Elasticsearch is not running
        let result = chain.list_indices().await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    #[ignore = "requires Elasticsearch on localhost:9200"]
    async fn test_integration_get_indices_info() {
        // Requires: Elasticsearch running on localhost:9200 with at least one index
        let llm = Arc::new(MockChatModel::query_response());
        let database = Elasticsearch::default();

        let chain = ElasticsearchDatabaseChain::from_llm(llm, database).unwrap();

        let indices = chain.list_indices().await.unwrap();
        if !indices.is_empty() {
            let info = chain.get_indices_info(&indices[..1]).await;
            assert!(info.is_ok());
            let info_str = info.unwrap();
            assert!(info_str.contains("Mapping for index"));
        }
    }
}
