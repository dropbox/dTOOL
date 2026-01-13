//! # Graph QA Chain
//!
//! Question answering over a knowledge graph using entity extraction and triplet lookup.
//!
//! ## Overview
//!
//! This chain implements a simple knowledge graph Q&A workflow:
//! 1. **Extract Entities**: Identify named entities (people, places, things) from the question
//! 2. **Retrieve Triplets**: Look up knowledge triplets (subject-predicate-object) for those entities
//! 3. **Answer Question**: Use the retrieved triplets as context to answer the original question
//!
//! ## Example
//!
//! ```rust,no_run
//! use dashflow_chains::{GraphQAChain, EntityGraph, KnowledgeTriple};
//! use std::sync::Arc;
//! use std::collections::HashMap;
//!
//! #[tokio::main]
//! async fn main() {
//!     // Create a knowledge graph
//!     let mut graph = EntityGraph::new();
//!     graph.add_triple(KnowledgeTriple::new(
//!         "Paris".to_string(),
//!         "is_capital_of".to_string(),
//!         "France".to_string()
//!     ));
//!
//!     // let llm = ...;
//!     // let chain = GraphQAChain::from_llm(llm, Arc::new(graph));
//!     //
//!     // let mut inputs = HashMap::new();
//!     // inputs.insert("query".to_string(), "What is the capital of France?".to_string());
//!     //
//!     // let result = chain.call(&inputs).await.unwrap();
//!     // println!("{}", result["result"]);
//! }
//! ```
//!
//! ## Security Note
//!
//! Make sure that the graph is populated with trusted data only. Allowing untrusted
//! data into the graph could lead to information disclosure or misleading answers.

use crate::LLMChain;
use dashflow::core::error::{Error, Result};
use dashflow::core::language_models::LLM;
use dashflow::core::prompts::PromptTemplate;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

/// Delimiter for knowledge triplets
pub const KG_TRIPLE_DELIMITER: &str = "<|>";

/// Default prompt for extracting entities from user input.
pub const ENTITY_EXTRACTION_TEMPLATE: &str = r"Extract all entities from the following text. As a guideline, a proper noun is generally capitalized. You should definitely extract all names and places.

Return the output as a single comma-separated list, or NONE if there is nothing of note to return.

EXAMPLE
i'm trying to improve Langchain's interfaces, the UX, its integrations with various products the user might want ... a lot of stuff.
Output: Langchain
END OF EXAMPLE

EXAMPLE
i'm trying to improve Langchain's interfaces, the UX, its integrations with various products the user might want ... a lot of stuff. I'm working with Sam.
Output: Langchain, Sam
END OF EXAMPLE

Begin!

{input}
Output:";

/// Default prompt for answering questions using knowledge triplets.
pub const GRAPH_QA_TEMPLATE: &str = r"Use the following knowledge triplets to answer the question at the end. If you don't know the answer, just say that you don't know, don't try to make up an answer.

{context}

Question: {question}
Helpful Answer:";

/// Create the default entity extraction prompt.
#[must_use]
pub fn entity_extraction_prompt() -> PromptTemplate {
    PromptTemplate::new(
        ENTITY_EXTRACTION_TEMPLATE.to_string(),
        vec!["input".to_string()],
        dashflow::core::prompts::PromptTemplateFormat::FString,
    )
}

/// Create the default graph QA prompt.
#[must_use]
pub fn graph_qa_prompt() -> PromptTemplate {
    PromptTemplate::new(
        GRAPH_QA_TEMPLATE.to_string(),
        vec!["context".to_string(), "question".to_string()],
        dashflow::core::prompts::PromptTemplateFormat::FString,
    )
}

/// Parse comma-separated entity string into individual entities.
///
/// Returns empty vector if input is "NONE" or empty.
#[must_use]
pub fn get_entities(entity_str: &str) -> Vec<String> {
    let trimmed = entity_str.trim();
    if trimmed.is_empty() || trimmed.eq_ignore_ascii_case("NONE") {
        return Vec::new();
    }

    entity_str
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

/// A knowledge triple representing a relationship between entities.
///
/// Triplets take the form: (subject, predicate, object)
/// For example: ("Paris", "`is_capital_of`", "France")
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct KnowledgeTriple {
    /// The subject entity
    pub subject: String,

    /// The relationship/predicate
    pub predicate: String,

    /// The object entity
    pub object: String,
}

impl KnowledgeTriple {
    /// Create a new knowledge triple.
    #[must_use]
    pub fn new(subject: String, predicate: String, object: String) -> Self {
        Self {
            subject,
            predicate,
            object,
        }
    }

    /// Parse a knowledge triple from a string.
    ///
    /// Expected format: "(subject, predicate, object)"
    pub fn from_string(triple_string: &str) -> Result<Self> {
        let trimmed = triple_string.trim();

        // Remove leading '(' and trailing ')'
        let content = trimmed
            .strip_prefix('(')
            .and_then(|s| s.strip_suffix(')'))
            .ok_or_else(|| {
                Error::Other("Invalid triple format: missing parentheses".to_string())
            })?;

        // Split by comma
        let parts: Vec<&str> = content.split(',').map(str::trim).collect();

        if parts.len() != 3 {
            return Err(Error::Other(format!(
                "Invalid triple format: expected 3 parts, got {}",
                parts.len()
            )));
        }

        Ok(Self::new(
            parts[0].to_string(),
            parts[1].to_string(),
            parts[2].to_string(),
        ))
    }

    /// Format the triple as a human-readable string.
    ///
    /// Format: "subject predicate object"
    #[must_use]
    pub fn format(&self) -> String {
        format!("{} {} {}", self.subject, self.predicate, self.object)
    }
}

/// Parse knowledge triplets from a delimited string.
///
/// Triplets should be separated by `KG_TRIPLE_DELIMITER`.
/// Returns empty vector if input is "NONE" or empty.
#[must_use]
pub fn parse_triples(knowledge_str: &str) -> Vec<KnowledgeTriple> {
    let trimmed = knowledge_str.trim();
    if trimmed.is_empty() || trimmed.eq_ignore_ascii_case("NONE") {
        return Vec::new();
    }

    knowledge_str
        .split(KG_TRIPLE_DELIMITER)
        .filter_map(|s| KnowledgeTriple::from_string(s).ok())
        .collect()
}

/// In-memory entity graph for storing knowledge triplets.
///
/// This is a simple directed graph where nodes are entities and edges are relationships.
/// Each edge stores its predicate (relationship type).
///
/// ## Security Note
///
/// Make sure that the graph is populated with trusted data only. Allowing untrusted
/// data into the graph could lead to information disclosure or misleading answers.
#[derive(Debug, Clone)]
pub struct EntityGraph {
    /// Adjacency list: entity -> [(predicate, `target_entity`)]
    edges: HashMap<String, Vec<(String, String)>>,
}

impl EntityGraph {
    /// Create a new empty entity graph.
    #[must_use]
    pub fn new() -> Self {
        Self {
            edges: HashMap::new(),
        }
    }

    /// Add a knowledge triple to the graph.
    ///
    /// Creates nodes if they don't exist. Multiple edges between the same nodes are allowed.
    pub fn add_triple(&mut self, triple: KnowledgeTriple) {
        self.edges
            .entry(triple.subject.clone())
            .or_default()
            .push((triple.predicate, triple.object));
    }

    /// Get all knowledge triplets for an entity.
    ///
    /// Returns triplets where the entity is the subject, formatted as strings.
    /// Depth parameter controls how many hops to traverse (default: 1).
    #[must_use]
    pub fn get_entity_knowledge(&self, entity: &str, depth: usize) -> Vec<String> {
        let mut results = Vec::new();
        let mut visited = HashSet::new();
        self.dfs_knowledge(entity, depth, &mut visited, &mut results);
        results
    }

    /// Depth-first search to collect knowledge triplets.
    fn dfs_knowledge(
        &self,
        entity: &str,
        depth: usize,
        visited: &mut HashSet<String>,
        results: &mut Vec<String>,
    ) {
        if depth == 0 || visited.contains(entity) {
            return;
        }

        visited.insert(entity.to_string());

        if let Some(edges) = self.edges.get(entity) {
            for (predicate, object) in edges {
                results.push(format!("{entity} {predicate} {object}"));
                self.dfs_knowledge(object, depth - 1, visited, results);
            }
        }
    }

    /// Check if the graph contains a node.
    #[must_use]
    pub fn has_node(&self, node: &str) -> bool {
        self.edges.contains_key(node)
    }

    /// Get all triplets in the graph.
    #[must_use]
    pub fn get_triples(&self) -> Vec<KnowledgeTriple> {
        let mut triples = Vec::new();
        for (subject, edges) in &self.edges {
            for (predicate, object) in edges {
                triples.push(KnowledgeTriple::new(
                    subject.clone(),
                    predicate.clone(),
                    object.clone(),
                ));
            }
        }
        triples
    }

    /// Clear all nodes and edges from the graph.
    pub fn clear(&mut self) {
        self.edges.clear();
    }
}

impl Default for EntityGraph {
    fn default() -> Self {
        Self::new()
    }
}

/// Chain for question-answering against a knowledge graph.
///
/// This chain uses entity extraction and knowledge graph lookup to answer questions:
///
/// 1. **Extract Entities**: LLM identifies named entities in the question
/// 2. **Retrieve Knowledge**: Look up triplets for those entities in the graph
/// 3. **Answer Question**: LLM uses retrieved triplets as context to answer
///
/// # Type Parameters
///
/// * `M` - The LLM type to use for entity extraction and question answering
///
/// # Example
///
/// ```rust,no_run
/// use dashflow_chains::{GraphQAChain, EntityGraph, KnowledgeTriple};
/// use std::sync::Arc;
/// use std::collections::HashMap;
///
/// #[tokio::main]
/// async fn main() {
///     // Build knowledge graph
///     let mut graph = EntityGraph::new();
///     graph.add_triple(KnowledgeTriple::new(
///         "Eiffel Tower".to_string(),
///         "located_in".to_string(),
///         "Paris".to_string()
///     ));
///
///     // let llm = ...;
///     // let chain = GraphQAChain::from_llm(llm, Arc::new(graph));
///     //
///     // let mut inputs = HashMap::new();
///     // inputs.insert("query".to_string(), "Where is the Eiffel Tower?".to_string());
///     //
///     // let result = chain.call(&inputs).await.unwrap();
///     // println!("{}", result["result"]);
/// }
/// ```
///
/// ## Security Note
///
/// Make sure that the graph is populated with trusted data only. Allowing untrusted
/// data into the graph could lead to information disclosure or misleading answers.
#[derive(Clone)]
pub struct GraphQAChain<M: LLM> {
    /// Knowledge graph for entity lookup
    graph: Arc<EntityGraph>,

    /// Chain for extracting entities from questions
    entity_extraction_chain: LLMChain<M>,

    /// Chain for answering questions using retrieved triplets
    qa_chain: LLMChain<M>,

    /// Input key name (default: "query")
    input_key: String,

    /// Output key name (default: "result")
    output_key: String,

    /// Maximum depth for knowledge traversal (default: 1)
    depth: usize,
}

impl<M: LLM> GraphQAChain<M> {
    /// Create a new `GraphQAChain` with custom prompts.
    ///
    /// # Arguments
    ///
    /// * `llm` - The language model to use
    /// * `graph` - The entity graph containing knowledge triplets
    /// * `entity_prompt` - Prompt for extracting entities
    /// * `qa_prompt` - Prompt for answering questions
    pub fn new(
        llm: Arc<M>,
        graph: Arc<EntityGraph>,
        entity_prompt: PromptTemplate,
        qa_prompt: PromptTemplate,
    ) -> Self {
        Self {
            graph,
            entity_extraction_chain: LLMChain::new(Arc::clone(&llm), entity_prompt),
            qa_chain: LLMChain::new(llm, qa_prompt),
            input_key: "query".to_string(),
            output_key: "result".to_string(),
            depth: 1,
        }
    }

    /// Create a `GraphQAChain` from a language model with default prompts.
    ///
    /// This is the most common way to create a `GraphQAChain`.
    ///
    /// # Arguments
    ///
    /// * `llm` - The language model to use
    /// * `graph` - The entity graph containing knowledge triplets
    pub fn from_llm(llm: Arc<M>, graph: Arc<EntityGraph>) -> Self {
        Self::new(llm, graph, entity_extraction_prompt(), graph_qa_prompt())
    }

    /// Set the input key name.
    ///
    /// Default: "query"
    pub fn with_input_key(mut self, key: impl Into<String>) -> Self {
        self.input_key = key.into();
        self
    }

    /// Set the output key name.
    ///
    /// Default: "result"
    pub fn with_output_key(mut self, key: impl Into<String>) -> Self {
        self.output_key = key.into();
        self
    }

    /// Set the depth for knowledge graph traversal.
    ///
    /// Default: 1 (only immediate connections)
    #[must_use]
    pub fn with_depth(mut self, depth: usize) -> Self {
        self.depth = depth;
        self
    }

    /// Get the input key name.
    #[must_use]
    pub fn input_key(&self) -> &str {
        &self.input_key
    }

    /// Get the output key name.
    #[must_use]
    pub fn output_key(&self) -> &str {
        &self.output_key
    }

    /// Get the input keys for this chain.
    #[must_use]
    pub fn input_keys(&self) -> Vec<String> {
        vec![self.input_key.clone()]
    }

    /// Get the output keys for this chain.
    #[must_use]
    pub fn output_keys(&self) -> Vec<String> {
        vec![self.output_key.clone()]
    }

    /// Run the graph QA chain.
    ///
    /// This executes the entity extraction and knowledge retrieval process:
    /// 1. Extract entities from the question
    /// 2. Look up knowledge triplets for those entities
    /// 3. Use triplets as context to answer the question
    ///
    /// # Arguments
    ///
    /// * `inputs` - Input values, must contain the `input_key` (default: "query")
    ///
    /// # Returns
    ///
    /// Output values with the `output_key` (default: "result") containing the answer
    pub async fn call(&self, inputs: &HashMap<String, String>) -> Result<HashMap<String, String>> {
        // Get the question
        let question = inputs
            .get(&self.input_key)
            .ok_or_else(|| {
                Error::InvalidInput(format!("Missing required input key: {}", self.input_key))
            })?
            .clone();

        // Step 1: Extract entities from the question
        let mut entity_inputs = HashMap::new();
        entity_inputs.insert("input".to_string(), question.clone());
        let entity_string = self.entity_extraction_chain.run(&entity_inputs).await?;

        // Parse entities
        let entities = get_entities(&entity_string);

        // Step 2: Retrieve knowledge triplets for each entity
        let mut all_triplets = Vec::new();
        for entity in entities {
            let triplets = self.graph.get_entity_knowledge(&entity, self.depth);
            all_triplets.extend(triplets);
        }

        // Format context
        let context = all_triplets.join("\n");

        // Step 3: Answer the question using the retrieved context
        let mut qa_inputs = HashMap::new();
        qa_inputs.insert("question".to_string(), question);
        qa_inputs.insert("context".to_string(), context);
        let answer = self.qa_chain.run(&qa_inputs).await?;

        // Return result
        let mut result = HashMap::new();
        result.insert(self.output_key.clone(), answer);
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dashflow::core::language_models::FakeLLM;

    #[test]
    fn test_get_entities_empty() {
        assert_eq!(get_entities(""), Vec::<String>::new());
        assert_eq!(get_entities("NONE"), Vec::<String>::new());
        assert_eq!(get_entities("  NONE  "), Vec::<String>::new());
    }

    #[test]
    fn test_get_entities_single() {
        assert_eq!(get_entities("Paris"), vec!["Paris"]);
    }

    #[test]
    fn test_get_entities_multiple() {
        assert_eq!(
            get_entities("Paris, France, Europe"),
            vec!["Paris", "France", "Europe"]
        );
    }

    #[test]
    fn test_get_entities_with_whitespace() {
        assert_eq!(
            get_entities("  Paris ,  France  ,Europe  "),
            vec!["Paris", "France", "Europe"]
        );
    }

    #[test]
    fn test_knowledge_triple_creation() {
        let triple = KnowledgeTriple::new(
            "Paris".to_string(),
            "is_capital_of".to_string(),
            "France".to_string(),
        );
        assert_eq!(triple.subject, "Paris");
        assert_eq!(triple.predicate, "is_capital_of");
        assert_eq!(triple.object, "France");
    }

    #[test]
    fn test_knowledge_triple_from_string() {
        let triple = KnowledgeTriple::from_string("(Paris, is_capital_of, France)").unwrap();
        assert_eq!(triple.subject, "Paris");
        assert_eq!(triple.predicate, "is_capital_of");
        assert_eq!(triple.object, "France");
    }

    #[test]
    fn test_knowledge_triple_from_string_invalid() {
        assert!(KnowledgeTriple::from_string("Paris, is_capital_of, France").is_err());
        assert!(KnowledgeTriple::from_string("(Paris, France)").is_err());
    }

    #[test]
    fn test_knowledge_triple_format() {
        let triple = KnowledgeTriple::new(
            "Paris".to_string(),
            "is_capital_of".to_string(),
            "France".to_string(),
        );
        assert_eq!(triple.format(), "Paris is_capital_of France");
    }

    #[test]
    fn test_entity_graph_basic() {
        let mut graph = EntityGraph::new();
        let triple = KnowledgeTriple::new(
            "Paris".to_string(),
            "is_capital_of".to_string(),
            "France".to_string(),
        );
        graph.add_triple(triple);

        assert!(graph.has_node("Paris"));
        assert!(!graph.has_node("London"));

        let knowledge = graph.get_entity_knowledge("Paris", 1);
        assert_eq!(knowledge.len(), 1);
        assert_eq!(knowledge[0], "Paris is_capital_of France");
    }

    #[test]
    fn test_entity_graph_multiple_triples() {
        let mut graph = EntityGraph::new();
        graph.add_triple(KnowledgeTriple::new(
            "Paris".to_string(),
            "is_capital_of".to_string(),
            "France".to_string(),
        ));
        graph.add_triple(KnowledgeTriple::new(
            "Paris".to_string(),
            "has_monument".to_string(),
            "Eiffel Tower".to_string(),
        ));

        let knowledge = graph.get_entity_knowledge("Paris", 1);
        assert_eq!(knowledge.len(), 2);
        assert!(knowledge.contains(&"Paris is_capital_of France".to_string()));
        assert!(knowledge.contains(&"Paris has_monument Eiffel Tower".to_string()));
    }

    #[test]
    fn test_entity_graph_depth() {
        let mut graph = EntityGraph::new();
        graph.add_triple(KnowledgeTriple::new(
            "Paris".to_string(),
            "is_capital_of".to_string(),
            "France".to_string(),
        ));
        graph.add_triple(KnowledgeTriple::new(
            "France".to_string(),
            "is_in".to_string(),
            "Europe".to_string(),
        ));

        // Depth 1: only immediate connections
        let knowledge1 = graph.get_entity_knowledge("Paris", 1);
        assert_eq!(knowledge1.len(), 1);
        assert_eq!(knowledge1[0], "Paris is_capital_of France");

        // Depth 2: includes transitive connections
        let knowledge2 = graph.get_entity_knowledge("Paris", 2);
        assert_eq!(knowledge2.len(), 2);
        assert!(knowledge2.contains(&"Paris is_capital_of France".to_string()));
        assert!(knowledge2.contains(&"France is_in Europe".to_string()));
    }

    #[test]
    fn test_entity_graph_get_triples() {
        let mut graph = EntityGraph::new();
        let triple1 = KnowledgeTriple::new(
            "Paris".to_string(),
            "is_capital_of".to_string(),
            "France".to_string(),
        );
        let triple2 = KnowledgeTriple::new(
            "London".to_string(),
            "is_capital_of".to_string(),
            "UK".to_string(),
        );
        graph.add_triple(triple1.clone());
        graph.add_triple(triple2.clone());

        let triples = graph.get_triples();
        assert_eq!(triples.len(), 2);
        assert!(triples.contains(&triple1));
        assert!(triples.contains(&triple2));
    }

    #[test]
    fn test_entity_graph_clear() {
        let mut graph = EntityGraph::new();
        graph.add_triple(KnowledgeTriple::new(
            "Paris".to_string(),
            "is_capital_of".to_string(),
            "France".to_string(),
        ));
        assert!(graph.has_node("Paris"));

        graph.clear();
        assert!(!graph.has_node("Paris"));
    }

    #[test]
    fn test_entity_extraction_prompt() {
        let prompt = entity_extraction_prompt();
        let mut inputs = HashMap::new();
        inputs.insert("input".to_string(), "I love Paris!".to_string());
        let formatted = prompt.format(&inputs).unwrap();
        assert!(formatted.contains("I love Paris!"));
        assert!(formatted.contains("Extract all entities"));
    }

    #[test]
    fn test_graph_qa_prompt() {
        let prompt = graph_qa_prompt();
        let mut inputs = HashMap::new();
        inputs.insert(
            "context".to_string(),
            "Paris is_capital_of France".to_string(),
        );
        inputs.insert(
            "question".to_string(),
            "What is the capital of France?".to_string(),
        );
        let formatted = prompt.format(&inputs).unwrap();
        assert!(formatted.contains("Paris is_capital_of France"));
        assert!(formatted.contains("What is the capital of France?"));
    }

    #[test]
    fn test_graph_qa_chain_keys() {
        let llm = Arc::new(FakeLLM::new(vec!["answer".to_string()]));
        let graph = Arc::new(EntityGraph::new());
        let chain = GraphQAChain::from_llm(llm, graph);

        assert_eq!(chain.input_key(), "query");
        assert_eq!(chain.output_key(), "result");
        assert_eq!(chain.input_keys(), vec!["query".to_string()]);
        assert_eq!(chain.output_keys(), vec!["result".to_string()]);
    }

    #[test]
    fn test_graph_qa_chain_custom_keys() {
        let llm = Arc::new(FakeLLM::new(vec!["answer".to_string()]));
        let graph = Arc::new(EntityGraph::new());
        let chain = GraphQAChain::from_llm(llm, graph)
            .with_input_key("question")
            .with_output_key("answer");

        assert_eq!(chain.input_key(), "question");
        assert_eq!(chain.output_key(), "answer");
    }

    #[tokio::test]
    async fn test_graph_qa_chain_basic() {
        // Create graph with knowledge
        let mut graph = EntityGraph::new();
        graph.add_triple(KnowledgeTriple::new(
            "Paris".to_string(),
            "is_capital_of".to_string(),
            "France".to_string(),
        ));
        let graph = Arc::new(graph);

        // Create fake LLM with canned responses
        let responses = vec![
            "Paris".to_string(),                           // Entity extraction
            "Paris is the capital of France.".to_string(), // QA response
        ];
        let llm = Arc::new(FakeLLM::new(responses));
        let chain = GraphQAChain::from_llm(llm, graph);

        let mut inputs = HashMap::new();
        inputs.insert(
            "query".to_string(),
            "What is the capital of France?".to_string(),
        );

        let result = chain.call(&inputs).await.unwrap();
        assert!(result.contains_key("result"));
        assert!(result["result"].contains("Paris"));
    }

    #[tokio::test]
    async fn test_graph_qa_chain_missing_input() {
        let llm = Arc::new(FakeLLM::new(vec!["test".to_string()]));
        let graph = Arc::new(EntityGraph::new());
        let chain = GraphQAChain::from_llm(llm, graph);

        let inputs = HashMap::new();
        let result = chain.call(&inputs).await;

        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Missing required input key"));
    }

    #[tokio::test]
    async fn test_graph_qa_chain_multiple_entities() {
        // Create graph with multiple triplets
        let mut graph = EntityGraph::new();
        graph.add_triple(KnowledgeTriple::new(
            "Paris".to_string(),
            "is_capital_of".to_string(),
            "France".to_string(),
        ));
        graph.add_triple(KnowledgeTriple::new(
            "Eiffel Tower".to_string(),
            "located_in".to_string(),
            "Paris".to_string(),
        ));
        let graph = Arc::new(graph);

        // Create fake LLM
        let responses = vec![
            "Paris, Eiffel Tower".to_string(), // Entity extraction
            "The Eiffel Tower is in Paris, France.".to_string(), // QA response
        ];
        let llm = Arc::new(FakeLLM::new(responses));
        let chain = GraphQAChain::from_llm(llm, graph);

        let mut inputs = HashMap::new();
        inputs.insert(
            "query".to_string(),
            "Where is the Eiffel Tower?".to_string(),
        );

        let result = chain.call(&inputs).await.unwrap();
        assert!(result.contains_key("result"));
    }
}
