//! # Graph Cypher QA Chain
//!
//! Question answering over a graph database by generating and executing Cypher queries.
//!
//! ## Overview
//!
//! This chain implements natural language to Cypher query conversion for Neo4j and compatible databases:
//! 1. **Generate Cypher**: LLM converts natural language question to Cypher query using graph schema
//! 2. **Execute Query**: Run the Cypher query against the graph database
//! 3. **Answer Question**: LLM formats the query results into a natural language answer
//!
//! ## Security Note
//!
//! **This chain can execute arbitrary Cypher queries.** Make sure that:
//! - Database credentials are narrowly-scoped with minimal permissions
//! - You trust the source of questions (or implement additional validation)
//! - You use `allow_dangerous_requests: true` to explicitly acknowledge risks
//!
//! Failure to properly scope permissions may result in data corruption, loss, or unauthorized access.
//!
//! See <https://python.dashflow.com/docs/security> for more information.
//!
//! ## Example
//!
//! ```rust,no_run
//! use dashflow_chains::GraphCypherQAChain;
//! use dashflow_neo4j::Neo4jGraph;
//! use std::sync::Arc;
//! use std::collections::HashMap;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     // Connect to Neo4j graph database
//!     let graph = Neo4jGraph::new(
//!         "bolt://localhost:7687",
//!         "neo4j",
//!         "password",
//!         None
//!     ).await?;
//!
//!     // let llm = ...;
//!     // let chain = GraphCypherQAChain::from_llm(
//!     //     llm,
//!     //     Arc::new(graph),
//!     //     true  // allow_dangerous_requests
//!     // )?;
//!     //
//!     // let mut inputs = HashMap::new();
//!     // inputs.insert("query".to_string(), "Who are the managers that own Neo4j stocks?".to_string());
//!     //
//!     // let result = chain.call(&inputs).await?;
//!     // println!("{}", result["result"]);
//!     Ok(())
//! }
//! ```

use crate::cypher_utils::{extract_cypher, CypherQueryCorrector};
use crate::LLMChain;
use dashflow::core::error::{Error, Result};
use dashflow::core::language_models::LLM;
use dashflow::core::prompts::PromptTemplate;
use dashflow_neo4j::GraphStore;
use std::collections::HashMap;
use std::sync::Arc;

/// Default Cypher generation prompt
pub const CYPHER_GENERATION_TEMPLATE: &str = r"Task: Generate Cypher statement to query a graph database.
Instructions:
Use only the provided relationship types and properties in the schema.
Do not use any other relationship types or properties that are not provided.
Schema:
{schema}
Note: Do not include any explanations or apologies in your responses.
Do not respond to any questions that might ask anything else than for you to construct a Cypher statement.
Do not include any text except the generated Cypher statement.

The question is:
{question}";

/// Default QA prompt for answering based on context
pub const CYPHER_QA_TEMPLATE: &str = r"You are an assistant that helps to form nice and human understandable answers.
The information part contains the provided information that you must use to construct an answer.
The provided information is authoritative, you must never doubt it or try to use your internal knowledge to correct it.
Make the answer sound as a response to the question. Do not mention that you based the result on the given information.
Here is an example:

Question: Which managers own Neo4j stocks?
Context: [manager:CTL LLC, manager:JANE STREET GROUP LLC]
Helpful Answer: CTL LLC, JANE STREET GROUP LLC owns Neo4j stocks.

Follow this example when generating answers.
If the provided information is empty, say that you don't know the answer.
Information:
{context}

Question: {question}
Helpful Answer:";

/// Create the default Cypher generation prompt
#[must_use]
pub fn cypher_generation_prompt() -> PromptTemplate {
    PromptTemplate::new(
        CYPHER_GENERATION_TEMPLATE.to_string(),
        vec!["schema".to_string(), "question".to_string()],
        dashflow::core::prompts::PromptTemplateFormat::FString,
    )
}

/// Create the default Cypher QA prompt
#[must_use]
pub fn cypher_qa_prompt() -> PromptTemplate {
    PromptTemplate::new(
        CYPHER_QA_TEMPLATE.to_string(),
        vec!["context".to_string(), "question".to_string()],
        dashflow::core::prompts::PromptTemplateFormat::FString,
    )
}

/// Chain for question-answering against a graph database by generating Cypher statements.
///
/// This chain converts natural language questions into Cypher queries, executes them
/// against a graph database, and formats the results into natural language answers.
///
/// # Security Warning
///
/// This chain can execute arbitrary Cypher queries. Ensure database credentials have
/// minimal necessary permissions to prevent data corruption, loss, or unauthorized access.
///
/// # Type Parameters
///
/// * `M` - The LLM type to use for Cypher generation and answer formatting
/// * `G` - The graph database type that implements `GraphStore`
///
/// # Example
///
/// ```rust,no_run
/// use dashflow_chains::GraphCypherQAChain;
/// use dashflow_neo4j::Neo4jGraph;
/// use std::sync::Arc;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let graph = Neo4jGraph::new("bolt://localhost:7687", "neo4j", "password", None).await?;
/// // let llm = ...;
/// // let chain = GraphCypherQAChain::from_llm(llm, Arc::new(graph), true)?;
/// # Ok(())
/// # }
/// ```
#[derive(Clone)]
pub struct GraphCypherQAChain<M: LLM, G: GraphStore> {
    /// Graph database connection
    graph: Arc<G>,
    /// Chain for generating Cypher queries
    cypher_generation_chain: LLMChain<M>,
    /// Chain for answering questions based on query results
    qa_chain: LLMChain<M>,
    /// Cached graph schema text
    graph_schema: String,
    /// Input key name (default: "query")
    input_key: String,
    /// Output key name (default: "result")
    output_key: String,
    /// Maximum number of results to return (default: 10)
    top_k: usize,
    /// Whether to return intermediate steps (Cypher query, raw results)
    return_intermediate_steps: bool,
    /// Whether to return raw database results without LLM formatting
    return_direct: bool,
    /// Optional Cypher query corrector for validation
    cypher_query_corrector: Option<CypherQueryCorrector>,
    /// Safety flag: must be true to allow execution
    _allow_dangerous_requests: bool,
}

impl<M: LLM, G: GraphStore> GraphCypherQAChain<M, G> {
    /// Create a new `GraphCypherQAChain` from an LLM and graph store.
    ///
    /// This is the primary constructor method.
    ///
    /// # Arguments
    ///
    /// * `llm` - The language model to use for both Cypher generation and QA
    /// * `graph` - The graph database connection
    /// * `allow_dangerous_requests` - Must be `true` to acknowledge security risks
    ///
    /// # Returns
    ///
    /// A new `GraphCypherQAChain` with default prompts and settings
    ///
    /// # Errors
    ///
    /// Returns an error if `allow_dangerous_requests` is not `true`
    pub fn from_llm(llm: Arc<M>, graph: Arc<G>, allow_dangerous_requests: bool) -> Result<Self> {
        Self::from_llm_with_prompts(
            llm,
            graph,
            cypher_generation_prompt(),
            cypher_qa_prompt(),
            allow_dangerous_requests,
        )
    }

    /// Create a `GraphCypherQAChain` with custom prompts.
    ///
    /// # Arguments
    ///
    /// * `llm` - The language model to use
    /// * `graph` - The graph database connection
    /// * `cypher_prompt` - Custom prompt for generating Cypher queries
    /// * `qa_prompt` - Custom prompt for answering questions
    /// * `allow_dangerous_requests` - Must be `true` to acknowledge security risks
    pub fn from_llm_with_prompts(
        llm: Arc<M>,
        graph: Arc<G>,
        cypher_prompt: PromptTemplate,
        qa_prompt: PromptTemplate,
        allow_dangerous_requests: bool,
    ) -> Result<Self> {
        if !allow_dangerous_requests {
            return Err(Error::InvalidInput(
                "In order to use this chain, you must acknowledge that it can make \
                 dangerous requests by setting `allow_dangerous_requests` to `true`. \
                 You must narrowly scope the permissions of the database connection \
                 to only include necessary permissions. Failure to do so may result \
                 in data corruption or loss or reading sensitive data if such data is \
                 present in the database. Only use this chain if you understand the risks \
                 and have taken the necessary precautions. \
                 See https://python.dashflow.com/docs/security for more information."
                    .to_string(),
            ));
        }

        let graph_schema = graph.get_schema_text();

        Ok(Self {
            graph,
            cypher_generation_chain: LLMChain::new(Arc::clone(&llm), cypher_prompt),
            qa_chain: LLMChain::new(llm, qa_prompt),
            graph_schema,
            input_key: "query".to_string(),
            output_key: "result".to_string(),
            top_k: 10,
            return_intermediate_steps: false,
            return_direct: false,
            cypher_query_corrector: None,
            _allow_dangerous_requests: allow_dangerous_requests,
        })
    }

    /// Enable Cypher query validation and correction.
    ///
    /// When enabled, generated Cypher queries will be validated against the graph schema
    /// and corrected if possible. Invalid queries that cannot be corrected will result in
    /// empty results.
    #[must_use]
    pub fn with_validation(mut self, validate: bool) -> Self {
        if validate {
            let relationships = self.graph.get_structured_schema().relationships.clone();
            self.cypher_query_corrector = Some(CypherQueryCorrector::new(relationships));
        } else {
            self.cypher_query_corrector = None;
        }
        self
    }

    /// Set the input key name.
    pub fn with_input_key(mut self, key: impl Into<String>) -> Self {
        self.input_key = key.into();
        self
    }

    /// Set the output key name.
    pub fn with_output_key(mut self, key: impl Into<String>) -> Self {
        self.output_key = key.into();
        self
    }

    /// Set the maximum number of results to return from the query.
    #[must_use]
    pub fn with_top_k(mut self, k: usize) -> Self {
        self.top_k = k;
        self
    }

    /// Set whether to return intermediate steps (Cypher query and raw results).
    #[must_use]
    pub fn with_return_intermediate_steps(mut self, return_steps: bool) -> Self {
        self.return_intermediate_steps = return_steps;
        self
    }

    /// Set whether to return raw database results without LLM formatting.
    #[must_use]
    pub fn with_return_direct(mut self, return_direct: bool) -> Self {
        self.return_direct = return_direct;
        self
    }

    /// Filter the graph schema by included types.
    ///
    /// Only the specified node and relationship types will be included in the schema
    /// provided to the LLM for Cypher generation.
    #[must_use]
    #[allow(clippy::needless_pass_by_value)] // Builder pattern takes ownership for ergonomics
    pub fn with_include_types(mut self, include_types: Vec<String>) -> Self {
        self.graph_schema = self.graph.get_schema_text_filtered(&include_types, &[]);
        self
    }

    /// Filter the graph schema by excluded types.
    ///
    /// The specified node and relationship types will be excluded from the schema
    /// provided to the LLM for Cypher generation.
    #[must_use]
    #[allow(clippy::needless_pass_by_value)] // Builder pattern takes ownership for ergonomics
    pub fn with_exclude_types(mut self, exclude_types: Vec<String>) -> Self {
        self.graph_schema = self.graph.get_schema_text_filtered(&[], &exclude_types);
        self
    }

    /// Get the input keys for this chain.
    #[must_use]
    pub fn input_keys(&self) -> Vec<String> {
        vec![self.input_key.clone()]
    }

    /// Get the output keys for this chain.
    #[must_use]
    pub fn output_keys(&self) -> Vec<String> {
        let mut keys = vec![self.output_key.clone()];
        if self.return_intermediate_steps {
            keys.push("intermediate_steps".to_string());
        }
        keys
    }

    /// Execute the graph Cypher QA chain.
    ///
    /// This performs the following steps:
    /// 1. Generate Cypher query from the question using the LLM and graph schema
    /// 2. Optionally validate and correct the Cypher query
    /// 3. Execute the query against the graph database
    /// 4. Format the results into a natural language answer (unless `return_direct` is true)
    ///
    /// # Arguments
    ///
    /// * `inputs` - Must contain the `input_key` (default: "query") with the question
    ///
    /// # Returns
    ///
    /// A `HashMap` containing:
    /// - The `output_key` (default: "result") with the answer
    /// - If `return_intermediate_steps` is true: "`intermediate_steps`" with query details
    pub async fn call(&self, inputs: &HashMap<String, String>) -> Result<HashMap<String, String>> {
        let question = inputs
            .get(&self.input_key)
            .ok_or_else(|| {
                Error::InvalidInput(format!("Missing required input key: {}", self.input_key))
            })?
            .clone();

        let mut intermediate_steps = Vec::new();

        // Step 1: Generate Cypher query
        let mut cypher_inputs = HashMap::new();
        cypher_inputs.insert("question".to_string(), question.clone());
        cypher_inputs.insert("schema".to_string(), self.graph_schema.clone());

        let generated_cypher = self.cypher_generation_chain.run(&cypher_inputs).await?;

        // Extract Cypher from markdown code blocks if present
        let mut cypher_query = extract_cypher(&generated_cypher);

        // Step 2: Correct Cypher query if validator is enabled
        if let Some(corrector) = &self.cypher_query_corrector {
            cypher_query = corrector.call(&cypher_query);
        }

        intermediate_steps.push(format!("Generated Cypher: {cypher_query}"));

        // Step 3: Execute the query
        let context = if cypher_query.is_empty() {
            "[]".to_string()
        } else {
            match self.graph.query(&cypher_query).await {
                Ok(results) => {
                    let limited_results: Vec<_> = results.into_iter().take(self.top_k).collect();
                    serde_json::to_string(&limited_results).unwrap_or_else(|_| "[]".to_string())
                }
                Err(e) => {
                    intermediate_steps.push(format!("Query execution error: {e}"));
                    "[]".to_string()
                }
            }
        };

        intermediate_steps.push(format!("Query results: {context}"));

        // Step 4: Format the answer
        let final_result = if self.return_direct {
            context
        } else {
            let mut qa_inputs = HashMap::new();
            qa_inputs.insert("question".to_string(), question);
            qa_inputs.insert("context".to_string(), context);
            self.qa_chain.run(&qa_inputs).await?
        };

        // Build output
        let mut output = HashMap::new();
        output.insert(self.output_key.clone(), final_result);

        if self.return_intermediate_steps {
            output.insert(
                "intermediate_steps".to_string(),
                intermediate_steps.join("\n"),
            );
        }

        Ok(output)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use dashflow::core::language_models::FakeLLM;
    use dashflow_neo4j::{PropertyDefinition, StructuredSchema};

    // Mock GraphStore for testing
    struct MockGraphStore {
        schema: StructuredSchema,
        query_results: Vec<HashMap<String, serde_json::Value>>,
    }

    #[async_trait]
    impl GraphStore for MockGraphStore {
        async fn query(&self, _query: &str) -> Result<Vec<HashMap<String, serde_json::Value>>> {
            Ok(self.query_results.clone())
        }

        fn get_structured_schema(&self) -> &StructuredSchema {
            &self.schema
        }

        async fn refresh_schema(&mut self) -> Result<()> {
            Ok(())
        }
    }

    #[test]
    fn test_cypher_generation_prompt() {
        let prompt = cypher_generation_prompt();
        let mut inputs = HashMap::new();
        inputs.insert("schema".to_string(), "Person {name: String}".to_string());
        inputs.insert("question".to_string(), "Who are the people?".to_string());
        let formatted = prompt.format(&inputs).unwrap();
        assert!(formatted.contains("Person {name: String}"));
        assert!(formatted.contains("Who are the people?"));
    }

    #[test]
    fn test_cypher_qa_prompt() {
        let prompt = cypher_qa_prompt();
        let mut inputs = HashMap::new();
        inputs.insert("context".to_string(), "[{\"name\": \"Alice\"}]".to_string());
        inputs.insert("question".to_string(), "Who are the people?".to_string());
        let formatted = prompt.format(&inputs).unwrap();
        assert!(formatted.contains("[{\"name\": \"Alice\"}]"));
        assert!(formatted.contains("Who are the people?"));
    }

    #[test]
    fn test_chain_requires_dangerous_flag() {
        let llm = Arc::new(FakeLLM::new(vec!["test".to_string()]));
        let graph = Arc::new(MockGraphStore {
            schema: StructuredSchema::default(),
            query_results: vec![],
        });

        let result = GraphCypherQAChain::from_llm(llm, graph, false);
        assert!(result.is_err());
        if let Err(e) = result {
            assert!(e.to_string().contains("dangerous"));
        }
    }

    #[tokio::test]
    async fn test_chain_basic_execution() {
        let mut schema = StructuredSchema::default();
        schema.node_props.insert(
            "Person".to_string(),
            vec![PropertyDefinition {
                property: "name".to_string(),
                prop_type: "String".to_string(),
            }],
        );

        let mut result_row = HashMap::new();
        result_row.insert("name".to_string(), serde_json::json!("Alice"));

        let graph = Arc::new(MockGraphStore {
            schema,
            query_results: vec![result_row],
        });

        let responses = vec![
            "MATCH (p:Person) RETURN p.name".to_string(), // Cypher generation
            "The person is Alice.".to_string(),           // QA response
        ];
        let llm = Arc::new(FakeLLM::new(responses));

        let chain = GraphCypherQAChain::from_llm(llm, graph, true).unwrap();

        let mut inputs = HashMap::new();
        inputs.insert("query".to_string(), "Who are the people?".to_string());

        let result = chain.call(&inputs).await.unwrap();
        assert!(result.contains_key("result"));
        assert!(result["result"].contains("Alice"));
    }

    #[tokio::test]
    async fn test_chain_with_intermediate_steps() {
        let graph = Arc::new(MockGraphStore {
            schema: StructuredSchema::default(),
            query_results: vec![],
        });

        let responses = vec![
            "MATCH (n) RETURN n".to_string(),
            "No results found.".to_string(),
        ];
        let llm = Arc::new(FakeLLM::new(responses));

        let chain = GraphCypherQAChain::from_llm(llm, graph, true)
            .unwrap()
            .with_return_intermediate_steps(true);

        let mut inputs = HashMap::new();
        inputs.insert("query".to_string(), "test query".to_string());

        let result = chain.call(&inputs).await.unwrap();
        assert!(result.contains_key("intermediate_steps"));
        assert!(result["intermediate_steps"].contains("Generated Cypher"));
    }

    #[tokio::test]
    async fn test_chain_with_return_direct() {
        let mut result_row = HashMap::new();
        result_row.insert("name".to_string(), serde_json::json!("Bob"));

        let graph = Arc::new(MockGraphStore {
            schema: StructuredSchema::default(),
            query_results: vec![result_row],
        });

        let responses = vec!["MATCH (p:Person) RETURN p.name".to_string()];
        let llm = Arc::new(FakeLLM::new(responses));

        let chain = GraphCypherQAChain::from_llm(llm, graph, true)
            .unwrap()
            .with_return_direct(true);

        let mut inputs = HashMap::new();
        inputs.insert("query".to_string(), "test".to_string());

        let result = chain.call(&inputs).await.unwrap();
        assert!(result["result"].contains("Bob"));
    }

    #[tokio::test]
    async fn test_chain_with_top_k() {
        let mut results = Vec::new();
        for i in 1..=20 {
            let mut row = HashMap::new();
            row.insert("id".to_string(), serde_json::json!(i));
            results.push(row);
        }

        let graph = Arc::new(MockGraphStore {
            schema: StructuredSchema::default(),
            query_results: results,
        });

        let responses = vec![
            "MATCH (n) RETURN n.id".to_string(),
            "Found results.".to_string(),
        ];
        let llm = Arc::new(FakeLLM::new(responses));

        let chain = GraphCypherQAChain::from_llm(llm, graph, true)
            .unwrap()
            .with_top_k(5)
            .with_return_intermediate_steps(true);

        let mut inputs = HashMap::new();
        inputs.insert("query".to_string(), "test".to_string());

        let result = chain.call(&inputs).await.unwrap();
        // The chain should limit results to top_k=5
        let steps = &result["intermediate_steps"];
        // Should only have 5 results in the context
        assert!(steps.contains("Query results"));
    }
}
