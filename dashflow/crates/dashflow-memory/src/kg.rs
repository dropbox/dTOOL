//! Knowledge Graph Memory - Extracts and stores knowledge triples from conversations
//!
//! `ConversationKGMemory` uses an LLM to extract structured knowledge from conversations
//! and stores it in a directed graph. The memory can then retrieve relevant knowledge
//! based on entities mentioned in the current conversation.
//!
//! # Python Baseline
//!
//! This implementation matches `dashflow_community.memory.kg.ConversationKGMemory`
//! and `dashflow_community.graphs.networkx_graph.NetworkxEntityGraph`.

use crate::base_memory::{BaseMemory, MemoryError, MemoryResult};
use crate::prompts::{
    create_entity_extraction_prompt, create_knowledge_triple_extraction_prompt, KG_TRIPLE_DELIMITER,
};
use async_trait::async_trait;
use dashflow::core::chat_history::{
    get_buffer_string, BaseChatMessageHistory, InMemoryChatMessageHistory,
};
use dashflow::core::language_models::LLM;
use dashflow::core::messages::Message;
use dashflow::core::prompts::base::BasePromptTemplate;
use dashflow::core::prompts::string::PromptTemplate;
use petgraph::graph::{DiGraph, NodeIndex};
use petgraph::visit::EdgeRef;
use petgraph::Direction;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// A knowledge triple representing a relationship in the knowledge graph.
///
/// Triples consist of:
/// - subject: The entity being described
/// - predicate: The relationship/property
/// - object: The value or related entity
///
/// # Example
///
/// ```rust,ignore
/// let triple = KnowledgeTriple {
///     subject: "Nevada".to_string(),
///     predicate: "is a".to_string(),
///     object: "state".to_string(),
/// };
/// // Represents: Nevada is a state
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KnowledgeTriple {
    pub subject: String,
    pub predicate: String,
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

    /// Parse a knowledge triple from a string in the format "(subject, predicate, object)".
    ///
    /// # Python Baseline
    ///
    /// Matches `KnowledgeTriple.from_string()` from `networkx_graph.py:18-23`
    #[must_use]
    pub fn from_string(triple_str: &str) -> Option<Self> {
        let trimmed = triple_str.trim();
        // Remove parentheses
        let content = trimmed.strip_prefix('(')?.strip_suffix(')')?;

        // Split by ", "
        let parts: Vec<&str> = content.split(", ").collect();
        if parts.len() != 3 {
            return None;
        }

        Some(Self {
            subject: parts[0].to_string(),
            predicate: parts[1].to_string(),
            object: parts[2].to_string(),
        })
    }
}

/// Parse knowledge triples from LLM output string.
///
/// Triples are separated by `KG_TRIPLE_DELIMITER` ("<|>") and each triple
/// is in the format "(subject, predicate, object)".
///
/// Returns an empty vector if the output is "NONE" or empty.
///
/// # Python Baseline
///
/// Matches `parse_triples()` from `networkx_graph.py:26-39`
pub fn parse_triples(knowledge_str: &str) -> Vec<KnowledgeTriple> {
    let trimmed = knowledge_str.trim();
    if trimmed.is_empty() || trimmed == "NONE" {
        return Vec::new();
    }

    trimmed
        .split(KG_TRIPLE_DELIMITER)
        .filter_map(KnowledgeTriple::from_string)
        .collect()
}

/// Extract entity names from LLM output string.
///
/// Entities are comma-separated. Returns an empty vector if output is "NONE".
///
/// # Python Baseline
///
/// Matches `get_entities()` from `networkx_graph.py:42-47`
#[must_use]
pub fn get_entities(entity_str: &str) -> Vec<String> {
    let trimmed = entity_str.trim();
    if trimmed == "NONE" {
        return Vec::new();
    }

    trimmed
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

/// A directed graph for storing entity relationships.
///
/// Uses petgraph (Rust) instead of networkx (Python) but provides similar
/// functionality. Entities are nodes and relationships are labeled edges.
///
/// # Security Note
///
/// Make sure that the database connection uses credentials that are narrowly-scoped
/// to only include necessary permissions. The calling code may attempt commands that
/// would result in deletion or mutation of data if appropriately prompted.
///
/// # Python Baseline
///
/// Matches `NetworkxEntityGraph` from `networkx_graph.py:50-219`
#[derive(Debug, Clone)]
pub struct NetworkxEntityGraph {
    /// The directed graph storing entity relationships
    graph: DiGraph<String, String>,
    /// Map from entity name to node index for fast lookups
    node_map: HashMap<String, NodeIndex>,
}

impl NetworkxEntityGraph {
    /// Create a new empty knowledge graph.
    #[must_use]
    pub fn new() -> Self {
        Self {
            graph: DiGraph::new(),
            node_map: HashMap::new(),
        }
    }

    /// Add a knowledge triple to the graph.
    ///
    /// Creates nodes for subject and object if they don't exist.
    /// Overwrites existing edges with the same subject and object.
    ///
    /// # Python Baseline
    ///
    /// Matches `add_triple()` from `networkx_graph.py:93-105`
    pub fn add_triple(&mut self, triple: &KnowledgeTriple) {
        // Ensure subject node exists
        let subject_idx = self.ensure_node(&triple.subject);

        // Ensure object node exists
        let object_idx = self.ensure_node(&triple.object);

        // Check if edge already exists and remove it
        if let Some(edge_idx) = self.graph.find_edge(subject_idx, object_idx) {
            self.graph.remove_edge(edge_idx);
        }

        // Add new edge with predicate as weight
        self.graph
            .add_edge(subject_idx, object_idx, triple.predicate.clone());
    }

    /// Delete a knowledge triple from the graph.
    ///
    /// # Python Baseline
    ///
    /// Matches `delete_triple()` from `networkx_graph.py:107-110`
    pub fn delete_triple(&mut self, triple: &KnowledgeTriple) {
        if let (Some(&subject_idx), Some(&object_idx)) = (
            self.node_map.get(&triple.subject),
            self.node_map.get(&triple.object),
        ) {
            if let Some(edge_idx) = self.graph.find_edge(subject_idx, object_idx) {
                self.graph.remove_edge(edge_idx);
            }
        }
    }

    /// Get all triples in the graph.
    ///
    /// Returns a vector of (subject, predicate, object) tuples.
    ///
    /// # Python Baseline
    ///
    /// Matches `get_triples()` from `networkx_graph.py:112-114`
    #[must_use]
    pub fn get_triples(&self) -> Vec<(String, String, String)> {
        self.graph
            .edge_references()
            .map(|edge| {
                let subject = self.graph[edge.source()].clone();
                let object = self.graph[edge.target()].clone();
                let predicate = edge.weight().clone();
                (subject, predicate, object)
            })
            .collect()
    }

    /// Get knowledge about an entity up to a certain depth.
    ///
    /// Returns strings in the format "subject predicate object" for all
    /// relationships reachable from the entity within the depth limit.
    ///
    /// # Python Baseline
    ///
    /// Matches `get_entity_knowledge()` from `networkx_graph.py:116-128`
    #[must_use]
    pub fn get_entity_knowledge(&self, entity: &str, depth: usize) -> Vec<String> {
        let Some(&node_idx) = self.node_map.get(entity) else {
            return Vec::new();
        };

        let mut results = Vec::new();
        let mut visited = std::collections::HashSet::new();
        let mut stack = vec![(node_idx, 0)];

        while let Some((current_idx, current_depth)) = stack.pop() {
            if current_depth >= depth || !visited.insert(current_idx) {
                continue;
            }

            // Get all outgoing edges from this node
            for edge in self.graph.edges_directed(current_idx, Direction::Outgoing) {
                let target = edge.target();
                let subject = &self.graph[current_idx];
                let predicate = edge.weight();
                let object = &self.graph[target];

                results.push(format!("{subject} {predicate} {object}"));

                if current_depth + 1 < depth {
                    stack.push((target, current_depth + 1));
                }
            }
        }

        results
    }

    /// Clear all nodes and edges from the graph.
    ///
    /// # Python Baseline
    ///
    /// Matches `clear()` from `networkx_graph.py:135-137`
    pub fn clear(&mut self) {
        self.graph.clear();
        self.node_map.clear();
    }

    /// Add a node to the graph if it doesn't exist.
    ///
    /// # Python Baseline
    ///
    /// Matches `add_node()` from `networkx_graph.py:143-145`
    pub fn add_node(&mut self, name: &str) {
        self.ensure_node(name);
    }

    /// Remove a node from the graph.
    ///
    /// # Python Baseline
    ///
    /// Matches `remove_node()` from `networkx_graph.py:147-150`
    pub fn remove_node(&mut self, name: &str) {
        if let Some(&idx) = self.node_map.get(name) {
            self.graph.remove_node(idx);
            self.node_map.remove(name);
        }
    }

    /// Check if the graph has a node with the given name.
    ///
    /// # Python Baseline
    ///
    /// Matches `has_node()` from `networkx_graph.py:152-154`
    #[must_use]
    pub fn has_node(&self, name: &str) -> bool {
        self.node_map.contains_key(name)
    }

    /// Get the number of nodes in the graph.
    ///
    /// # Python Baseline
    ///
    /// Matches `get_number_of_nodes()` from `networkx_graph.py:171-173`
    #[must_use]
    pub fn node_count(&self) -> usize {
        self.graph.node_count()
    }

    /// Ensure a node exists in the graph, creating it if necessary.
    /// Returns the node index.
    fn ensure_node(&mut self, name: &str) -> NodeIndex {
        if let Some(&idx) = self.node_map.get(name) {
            idx
        } else {
            let idx = self.graph.add_node(name.to_string());
            self.node_map.insert(name.to_string(), idx);
            idx
        }
    }
}

impl Default for NetworkxEntityGraph {
    fn default() -> Self {
        Self::new()
    }
}

/// Conversation memory that extracts and stores knowledge triples in a graph.
///
/// This memory type uses an LLM to:
/// 1. Extract entities from user input
/// 2. Extract knowledge triples (subject, predicate, object) from conversations
/// 3. Store triples in a knowledge graph
/// 4. Retrieve relevant knowledge when entities are mentioned
///
/// # Example
///
/// ```rust,ignore
/// use dashflow_memory::{ConversationKGMemory, NetworkxEntityGraph};
/// use dashflow_openai::ChatOpenAI;
/// use dashflow::core::chat_history::InMemoryChatMessageHistory;
///
/// let llm = ChatOpenAI::default();
/// let chat_memory = InMemoryChatMessageHistory::new();
/// let kg = NetworkxEntityGraph::new();
///
/// let memory = ConversationKGMemory::new(llm, chat_memory, kg)
///     .with_k(2)
///     .with_memory_key("history");
///
/// // Use in a conversation
/// let mut inputs = std::collections::HashMap::new();
/// inputs.insert("input".to_string(), "Nevada is a state in the US.".to_string());
///
/// memory.save_context(&inputs, &[("output", "Interesting!")]).await?;
///
/// // Later, when Nevada is mentioned again
/// let vars = memory.load_memory_variables(&inputs).await?;
/// // vars["history"] will contain knowledge about Nevada
/// ```
///
/// # Python Baseline
///
/// Matches `ConversationKGMemory` from `dashflow_community.memory.kg:24-136`
pub struct ConversationKGMemory<L: LLM> {
    /// The LLM used for entity and knowledge extraction
    llm: Arc<L>,
    /// Storage for chat message history (already thread-safe via internal `Arc<RwLock>`)
    chat_memory: InMemoryChatMessageHistory,
    /// The knowledge graph storing extracted triples
    kg: Arc<RwLock<NetworkxEntityGraph>>,
    /// Number of previous conversation turns to include in extraction context
    k: usize,
    /// Prefix for human messages in history formatting
    human_prefix: String,
    /// Prefix for AI messages in history formatting
    ai_prefix: String,
    /// Prompt template for extracting knowledge triples
    knowledge_extraction_prompt: PromptTemplate,
    /// Prompt template for extracting entities
    entity_extraction_prompt: PromptTemplate,
    /// Key to store memory context under
    memory_key: String,
    /// Input key (if None, will auto-detect)
    input_key: Option<String>,
    /// Output key (if None, will auto-detect)
    output_key: Option<String>,
    /// Whether to return messages as Message objects or strings
    return_messages: bool,
}

impl<L: LLM> ConversationKGMemory<L> {
    /// Create a new `ConversationKGMemory` with the given LLM and chat history.
    ///
    /// Uses default prompts for entity and knowledge extraction.
    pub fn new(llm: L, chat_memory: InMemoryChatMessageHistory, kg: NetworkxEntityGraph) -> Self {
        Self {
            llm: Arc::new(llm),
            chat_memory,
            kg: Arc::new(RwLock::new(kg)),
            k: 2,
            human_prefix: "Human".to_string(),
            ai_prefix: "AI".to_string(),
            knowledge_extraction_prompt: create_knowledge_triple_extraction_prompt(),
            entity_extraction_prompt: create_entity_extraction_prompt(),
            memory_key: "history".to_string(),
            input_key: None,
            output_key: None,
            return_messages: false,
        }
    }

    /// Set the number of previous conversation turns to include in context.
    #[must_use]
    pub fn with_k(mut self, k: usize) -> Self {
        self.k = k;
        self
    }

    /// Set the human message prefix.
    #[must_use]
    pub fn with_human_prefix(mut self, prefix: String) -> Self {
        self.human_prefix = prefix;
        self
    }

    /// Set the AI message prefix.
    #[must_use]
    pub fn with_ai_prefix(mut self, prefix: String) -> Self {
        self.ai_prefix = prefix;
        self
    }

    /// Set the memory key.
    #[must_use]
    pub fn with_memory_key(mut self, key: String) -> Self {
        self.memory_key = key;
        self
    }

    /// Set the input key.
    #[must_use]
    pub fn with_input_key(mut self, key: String) -> Self {
        self.input_key = Some(key);
        self
    }

    /// Set the output key.
    #[must_use]
    pub fn with_output_key(mut self, key: String) -> Self {
        self.output_key = Some(key);
        self
    }

    /// Set whether to return messages as Message objects.
    #[must_use]
    pub fn with_return_messages(mut self, return_messages: bool) -> Self {
        self.return_messages = return_messages;
        self
    }

    /// Set a custom knowledge extraction prompt.
    #[must_use]
    pub fn with_knowledge_extraction_prompt(mut self, prompt: PromptTemplate) -> Self {
        self.knowledge_extraction_prompt = prompt;
        self
    }

    /// Set a custom entity extraction prompt.
    #[must_use]
    pub fn with_entity_extraction_prompt(mut self, prompt: PromptTemplate) -> Self {
        self.entity_extraction_prompt = prompt;
        self
    }

    /// Extract entities from the input string using the LLM.
    ///
    /// # Python Baseline
    ///
    /// Matches `get_current_entities()` from `kg.py:88-99`
    async fn get_current_entities(&self, input_string: &str) -> MemoryResult<Vec<String>> {
        let messages = self
            .chat_memory
            .get_messages()
            .await
            .map_err(|e| MemoryError::OperationFailed(e.to_string()))?;

        // Get last k*2 messages (k turns = k human + k AI messages)
        let recent_count = (self.k * 2).min(messages.len());
        let recent_messages = &messages[messages.len() - recent_count..];

        let buffer_string = get_buffer_string(recent_messages);

        // Format the entity extraction prompt
        let mut values = HashMap::new();
        values.insert("history".to_string(), buffer_string);
        values.insert("input".to_string(), input_string.to_string());

        let prompt_value = self
            .entity_extraction_prompt
            .format_prompt(&values)
            .map_err(|e| MemoryError::OperationFailed(e.to_string()))?;

        // Call LLM to extract entities
        let prompt_string = prompt_value.to_string();
        let output = self
            .llm
            .generate(&[prompt_string], None, None)
            .await
            .map_err(|e| MemoryError::LLMError(e.to_string()))?;

        let entity_str = output
            .generations
            .first()
            .and_then(|g| g.first())
            .map_or("NONE", |g| g.text.as_str());

        Ok(get_entities(entity_str))
    }

    /// Extract knowledge triples from the input string using the LLM.
    ///
    /// # Python Baseline
    ///
    /// Matches `get_knowledge_triplets()` from `kg.py:106-119`
    async fn get_knowledge_triplets(
        &self,
        input_string: &str,
    ) -> MemoryResult<Vec<KnowledgeTriple>> {
        let messages = self
            .chat_memory
            .get_messages()
            .await
            .map_err(|e| MemoryError::OperationFailed(e.to_string()))?;

        // Get last k*2 messages
        let recent_count = (self.k * 2).min(messages.len());
        let recent_messages = &messages[messages.len() - recent_count..];

        let buffer_string = get_buffer_string(recent_messages);

        // Format the knowledge extraction prompt
        let mut values = HashMap::new();
        values.insert("history".to_string(), buffer_string);
        values.insert("input".to_string(), input_string.to_string());

        let prompt_value = self
            .knowledge_extraction_prompt
            .format_prompt(&values)
            .map_err(|e| MemoryError::OperationFailed(e.to_string()))?;

        // Call LLM to extract knowledge
        let prompt_string = prompt_value.to_string();
        let output = self
            .llm
            .generate(&[prompt_string], None, None)
            .await
            .map_err(|e| MemoryError::LLMError(e.to_string()))?;

        let knowledge_str = output
            .generations
            .first()
            .and_then(|g| g.first())
            .map_or("NONE", |g| g.text.as_str());

        Ok(parse_triples(knowledge_str))
    }

    /// Get the input key from the inputs, auto-detecting if necessary.
    fn get_input_key<'a>(&'a self, inputs: &'a HashMap<String, String>) -> MemoryResult<&'a str> {
        if let Some(ref key) = self.input_key {
            return Ok(key.as_str());
        }

        // Auto-detect: must have exactly one key that's not a memory variable
        let memory_vars: Vec<String> = self.memory_variables();
        let input_keys: Vec<&String> = inputs.keys().filter(|k| !memory_vars.contains(k)).collect();

        if input_keys.len() == 1 {
            Ok(input_keys[0])
        } else {
            Err(MemoryError::OperationFailed(format!(
                "Expected exactly one input key, found: {input_keys:?}"
            )))
        }
    }

    /// Get the output key from the outputs, auto-detecting if necessary.
    fn get_output_key<'a>(&'a self, outputs: &'a HashMap<String, String>) -> MemoryResult<&'a str> {
        if let Some(ref key) = self.output_key {
            return Ok(key.as_str());
        }

        // Auto-detect: must have exactly one key
        if outputs.len() == 1 {
            // SAFETY: len() == 1 check guarantees .next() returns Some
            #[allow(clippy::unwrap_used)]
            Ok(outputs.keys().next().unwrap())
        } else {
            Err(MemoryError::OperationFailed(format!(
                "Expected exactly one output key, found: {:?}",
                outputs.keys().collect::<Vec<_>>()
            )))
        }
    }

    /// Access the knowledge graph (read-only).
    pub async fn kg(&self) -> tokio::sync::RwLockReadGuard<'_, NetworkxEntityGraph> {
        self.kg.read().await
    }

    /// Access the knowledge graph (mutable).
    pub async fn kg_mut(&self) -> tokio::sync::RwLockWriteGuard<'_, NetworkxEntityGraph> {
        self.kg.write().await
    }
}

#[async_trait]
impl<L: LLM> BaseMemory for ConversationKGMemory<L> {
    /// Load memory variables based on the current entities in the input.
    ///
    /// Extracts entities from the input, retrieves their knowledge from the graph,
    /// and returns formatted context.
    ///
    /// # Python Baseline
    ///
    /// Matches `load_memory_variables()` from `kg.py:44-64`
    async fn load_memory_variables(
        &self,
        inputs: &HashMap<String, String>,
    ) -> MemoryResult<HashMap<String, String>> {
        let input_key = self.get_input_key(inputs)?;
        let input_string = inputs.get(input_key).ok_or_else(|| {
            MemoryError::OperationFailed(format!("Missing input key: {input_key}"))
        })?;

        // Extract entities from current input
        let entities = self.get_current_entities(input_string).await?;

        // Get knowledge about each entity from the graph
        let kg = self.kg.read().await;
        let mut summary_strings = Vec::new();

        for entity in entities {
            let knowledge = kg.get_entity_knowledge(&entity, 1);
            if !knowledge.is_empty() {
                let summary = format!("On {}: {}.", entity, knowledge.join(". "));
                summary_strings.push(summary);
            }
        }

        // Format output based on return_messages setting
        let context_value = if summary_strings.is_empty() {
            String::new()
        } else {
            summary_strings.join("\n")
        };

        let mut result = HashMap::new();
        result.insert(self.memory_key.clone(), context_value);
        Ok(result)
    }

    /// Save context from the conversation and update the knowledge graph.
    ///
    /// Extracts knowledge triples from the input and adds them to the graph.
    ///
    /// # Python Baseline
    ///
    /// Matches `save_context()` from `kg.py:128-131`
    async fn save_context(
        &mut self,
        inputs: &HashMap<String, String>,
        outputs: &HashMap<String, String>,
    ) -> MemoryResult<()> {
        // Save to chat history first
        let input_key = self.get_input_key(inputs)?;
        let output_key = self.get_output_key(outputs)?;

        let input_str = inputs.get(input_key).ok_or_else(|| {
            MemoryError::OperationFailed(format!("Missing input key: {input_key}"))
        })?;
        let output_str = outputs.get(output_key).ok_or_else(|| {
            MemoryError::OperationFailed(format!("Missing output key: {output_key}"))
        })?;

        let human_msg = Message::human(input_str.clone());
        let ai_msg = Message::ai(output_str.clone());

        // Add messages to history (no lock needed - InMemoryChatMessageHistory is already thread-safe)
        self.chat_memory
            .add_messages(&[human_msg, ai_msg])
            .await
            .map_err(|e| MemoryError::OperationFailed(e.to_string()))?;

        // Extract and update knowledge graph
        let triples = self.get_knowledge_triplets(input_str).await?;
        let mut kg = self.kg.write().await;
        for triple in triples {
            kg.add_triple(&triple);
        }

        Ok(())
    }

    /// Clear the conversation memory and knowledge graph.
    ///
    /// # Python Baseline
    ///
    /// Matches `clear()` from `kg.py:133-135`
    async fn clear(&mut self) -> MemoryResult<()> {
        // Clear chat memory (no lock needed - InMemoryChatMessageHistory is already thread-safe)
        self.chat_memory
            .clear()
            .await
            .map_err(|e| MemoryError::OperationFailed(e.to_string()))?;

        // Clear knowledge graph
        let mut kg = self.kg.write().await;
        kg.clear();

        Ok(())
    }

    /// Return the list of memory variables (just the memory key).
    ///
    /// # Python Baseline
    ///
    /// Matches `memory_variables` property from `kg.py:66-72`
    fn memory_variables(&self) -> Vec<String> {
        vec![self.memory_key.clone()]
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use dashflow::core::language_models::{Generation, LLMResult};
    use std::sync::Mutex;

    // MockLLM for testing ConversationKGMemory
    #[derive(Clone)]
    struct MockLLM {
        responses: Arc<Mutex<Vec<String>>>,
        index: Arc<Mutex<usize>>,
    }

    impl MockLLM {
        fn new() -> Self {
            Self {
                responses: Arc::new(Mutex::new(Vec::new())),
                index: Arc::new(Mutex::new(0)),
            }
        }

        fn add_response(&mut self, response: impl Into<String>) {
            self.responses.lock().unwrap().push(response.into());
        }
    }

    #[async_trait]
    impl LLM for MockLLM {
        async fn _generate(
            &self,
            _prompts: &[String],
            _stop: Option<&[String]>,
            _run_manager: Option<&dashflow::core::callbacks::CallbackManager>,
        ) -> dashflow::core::error::Result<LLMResult> {
            let mut idx = self.index.lock().unwrap();
            let responses = self.responses.lock().unwrap();
            let response = responses.get(*idx).unwrap_or(&"NONE".to_string()).clone();
            *idx += 1;

            Ok(LLMResult {
                generations: vec![vec![Generation {
                    text: response,
                    generation_info: None,
                }]],
                llm_output: None,
            })
        }

        fn llm_type(&self) -> &str {
            "mock"
        }
    }

    #[test]
    fn test_knowledge_triple_from_string() {
        let triple_str = "(Nevada, is a, state)";
        let triple = KnowledgeTriple::from_string(triple_str).unwrap();

        assert_eq!(triple.subject, "Nevada");
        assert_eq!(triple.predicate, "is a");
        assert_eq!(triple.object, "state");
    }

    #[test]
    fn test_knowledge_triple_from_string_invalid() {
        assert!(KnowledgeTriple::from_string("invalid").is_none());
        assert!(KnowledgeTriple::from_string("(Nevada, is a)").is_none());
    }

    #[test]
    fn test_parse_triples() {
        let output = "(Nevada, is a, state)<|>(Nevada, is in, US)<|>(Nevada, is the number 1 producer of, gold)";
        let triples = parse_triples(output);

        assert_eq!(triples.len(), 3);
        assert_eq!(triples[0].subject, "Nevada");
        assert_eq!(triples[0].predicate, "is a");
        assert_eq!(triples[0].object, "state");
        assert_eq!(triples[1].subject, "Nevada");
        assert_eq!(triples[1].predicate, "is in");
        assert_eq!(triples[1].object, "US");
    }

    #[test]
    fn test_parse_triples_none() {
        assert_eq!(parse_triples("NONE").len(), 0);
        assert_eq!(parse_triples("").len(), 0);
    }

    #[test]
    fn test_get_entities() {
        let output = "Langchain, Person #2";
        let entities = get_entities(output);

        assert_eq!(entities, vec!["Langchain", "Person #2"]);
    }

    #[test]
    fn test_get_entities_none() {
        assert_eq!(get_entities("NONE").len(), 0);
    }

    #[test]
    fn test_networkx_entity_graph_add_triple() {
        let mut kg = NetworkxEntityGraph::new();
        let triple = KnowledgeTriple::new(
            "Nevada".to_string(),
            "is a".to_string(),
            "state".to_string(),
        );

        kg.add_triple(&triple);

        assert!(kg.has_node("Nevada"));
        assert!(kg.has_node("state"));
        assert_eq!(kg.node_count(), 2);

        let triples = kg.get_triples();
        assert_eq!(triples.len(), 1);
        assert_eq!(triples[0].0, "Nevada");
        assert_eq!(triples[0].1, "is a");
        assert_eq!(triples[0].2, "state");
    }

    #[test]
    fn test_networkx_entity_graph_get_entity_knowledge() {
        let mut kg = NetworkxEntityGraph::new();

        kg.add_triple(&KnowledgeTriple::new(
            "Nevada".to_string(),
            "is a".to_string(),
            "state".to_string(),
        ));
        kg.add_triple(&KnowledgeTriple::new(
            "Nevada".to_string(),
            "is in".to_string(),
            "US".to_string(),
        ));

        let knowledge = kg.get_entity_knowledge("Nevada", 1);
        assert_eq!(knowledge.len(), 2);
        assert!(knowledge.contains(&"Nevada is a state".to_string()));
        assert!(knowledge.contains(&"Nevada is in US".to_string()));
    }

    #[test]
    fn test_networkx_entity_graph_clear() {
        let mut kg = NetworkxEntityGraph::new();

        kg.add_triple(&KnowledgeTriple::new(
            "Nevada".to_string(),
            "is a".to_string(),
            "state".to_string(),
        ));

        assert_eq!(kg.node_count(), 2);

        kg.clear();

        assert_eq!(kg.node_count(), 0);
        assert_eq!(kg.get_triples().len(), 0);
    }

    // ConversationKGMemory async tests
    #[tokio::test]
    async fn test_conversation_kg_memory_creation() {
        let llm = MockLLM::new();
        let chat_memory = InMemoryChatMessageHistory::new();
        let kg = NetworkxEntityGraph::new();
        let memory = ConversationKGMemory::new(llm, chat_memory, kg);

        assert_eq!(memory.memory_variables(), vec!["history".to_string()]);
    }

    #[tokio::test]
    async fn test_conversation_kg_memory_save_and_load() {
        let mut llm = MockLLM::new();
        // save_context calls get_knowledge_triplets (1 LLM call)
        // load_memory_variables calls get_current_entities (1 LLM call)
        // So we need 2 responses total: triples for save, then entities for load
        llm.add_response("(Nevada, is a, state)<|>(Nevada, is in, US)");
        llm.add_response("Nevada");

        let chat_memory = InMemoryChatMessageHistory::new();
        let kg = NetworkxEntityGraph::new();
        let mut memory = ConversationKGMemory::new(llm, chat_memory, kg);

        // Save context
        let mut inputs = HashMap::new();
        inputs.insert(
            "input".to_string(),
            "Nevada is a state in the US.".to_string(),
        );
        let mut outputs = HashMap::new();
        outputs.insert("output".to_string(), "That's correct!".to_string());

        memory.save_context(&inputs, &outputs).await.unwrap();

        // Load memory - should contain extracted knowledge
        let vars = memory.load_memory_variables(&inputs).await.unwrap();
        assert!(vars.contains_key("history"));
        let history = vars.get("history").unwrap();
        // Should contain knowledge about Nevada
        assert!(history.contains("Nevada"));
    }

    #[tokio::test]
    async fn test_conversation_kg_memory_empty_extraction() {
        let mut llm = MockLLM::new();
        // Mock responses returning NONE (no entities/triples found)
        llm.add_response("NONE");
        llm.add_response("NONE");

        let chat_memory = InMemoryChatMessageHistory::new();
        let kg = NetworkxEntityGraph::new();
        let mut memory = ConversationKGMemory::new(llm, chat_memory, kg);

        let mut inputs = HashMap::new();
        inputs.insert("input".to_string(), "Hello!".to_string());
        let mut outputs = HashMap::new();
        outputs.insert("output".to_string(), "Hi!".to_string());

        memory.save_context(&inputs, &outputs).await.unwrap();

        // Load memory - should be empty or minimal
        let vars = memory.load_memory_variables(&inputs).await.unwrap();
        assert!(vars.contains_key("history"));
    }

    #[tokio::test]
    async fn test_conversation_kg_memory_multiple_entities() {
        let mut llm = MockLLM::new();
        // save_context calls get_knowledge_triplets (1 LLM call)
        // load_memory_variables calls get_current_entities (1 LLM call)
        // So we need 2 responses total: triples for save, then entities for load
        llm.add_response(
            "(Alice, works with, Bob)<|>(Alice, lives in, Seattle)<|>(Bob, lives in, Seattle)",
        );
        llm.add_response("Alice, Bob, Seattle");

        let chat_memory = InMemoryChatMessageHistory::new();
        let kg = NetworkxEntityGraph::new();
        let mut memory = ConversationKGMemory::new(llm, chat_memory, kg);

        let mut inputs = HashMap::new();
        inputs.insert(
            "input".to_string(),
            "Alice and Bob both work together and live in Seattle.".to_string(),
        );
        let mut outputs = HashMap::new();
        outputs.insert("output".to_string(), "Interesting!".to_string());

        memory.save_context(&inputs, &outputs).await.unwrap();

        // Load with entity in input
        let mut load_inputs = HashMap::new();
        load_inputs.insert("input".to_string(), "Tell me about Alice".to_string());

        let vars = memory.load_memory_variables(&load_inputs).await.unwrap();
        let history = vars.get("history").unwrap();
        // Should retrieve knowledge about Alice
        assert!(history.contains("Alice"));
    }

    #[tokio::test]
    async fn test_conversation_kg_memory_custom_k() {
        let mut llm = MockLLM::new();
        llm.add_response("Nevada");
        llm.add_response("(Nevada, is a, state)");

        let chat_memory = InMemoryChatMessageHistory::new();
        let kg = NetworkxEntityGraph::new();
        let mut memory = ConversationKGMemory::new(llm, chat_memory, kg).with_k(1);

        let mut inputs = HashMap::new();
        inputs.insert("input".to_string(), "Nevada is a state.".to_string());
        let mut outputs = HashMap::new();
        outputs.insert("output".to_string(), "Yes.".to_string());

        memory.save_context(&inputs, &outputs).await.unwrap();

        let vars = memory.load_memory_variables(&inputs).await.unwrap();
        assert!(vars.contains_key("history"));
    }

    #[tokio::test]
    async fn test_conversation_kg_memory_custom_memory_key() {
        let llm = MockLLM::new();
        let chat_memory = InMemoryChatMessageHistory::new();
        let kg = NetworkxEntityGraph::new();
        let memory = ConversationKGMemory::new(llm, chat_memory, kg)
            .with_memory_key("kg_context".to_string());

        assert_eq!(memory.memory_variables(), vec!["kg_context".to_string()]);
    }

    #[tokio::test]
    async fn test_conversation_kg_memory_custom_input_output_keys() {
        let mut llm = MockLLM::new();
        llm.add_response("Rust");
        llm.add_response("(Rust, is a, programming language)");

        let chat_memory = InMemoryChatMessageHistory::new();
        let kg = NetworkxEntityGraph::new();
        let mut memory = ConversationKGMemory::new(llm, chat_memory, kg)
            .with_input_key("question".to_string())
            .with_output_key("answer".to_string());

        let mut inputs = HashMap::new();
        inputs.insert("question".to_string(), "What is Rust?".to_string());
        inputs.insert("other_key".to_string(), "ignored".to_string());
        let mut outputs = HashMap::new();
        outputs.insert("answer".to_string(), "A programming language.".to_string());

        memory.save_context(&inputs, &outputs).await.unwrap();

        let vars = memory.load_memory_variables(&inputs).await.unwrap();
        assert!(vars.contains_key("history"));
    }

    #[tokio::test]
    async fn test_conversation_kg_memory_missing_input_key_error() {
        let llm = MockLLM::new();
        let chat_memory = InMemoryChatMessageHistory::new();
        let kg = NetworkxEntityGraph::new();
        let mut memory = ConversationKGMemory::new(llm, chat_memory, kg);

        let mut inputs = HashMap::new();
        inputs.insert("key1".to_string(), "value1".to_string());
        inputs.insert("key2".to_string(), "value2".to_string());
        let mut outputs = HashMap::new();
        outputs.insert("output".to_string(), "response".to_string());

        let result = memory.save_context(&inputs, &outputs).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_conversation_kg_memory_clear() {
        let mut llm = MockLLM::new();
        llm.add_response("Nevada");
        llm.add_response("(Nevada, is a, state)");

        let chat_memory = InMemoryChatMessageHistory::new();
        let kg = NetworkxEntityGraph::new();
        let mut memory = ConversationKGMemory::new(llm, chat_memory, kg);

        // Add some context
        let mut inputs = HashMap::new();
        inputs.insert("input".to_string(), "Nevada is a state.".to_string());
        let mut outputs = HashMap::new();
        outputs.insert("output".to_string(), "Yes.".to_string());
        memory.save_context(&inputs, &outputs).await.unwrap();

        // Clear
        memory.clear().await.unwrap();

        // Verify cleared
        let vars = memory.load_memory_variables(&inputs).await.unwrap();
        let history = vars.get("history").unwrap();
        // History should be empty or contain minimal content
        assert!(history.is_empty() || history == "On the entities mentioned below:\n");
    }

    #[tokio::test]
    async fn test_conversation_kg_memory_unicode() {
        let mut llm = MockLLM::new();
        llm.add_response("你好, مرحبا");
        llm.add_response("(你好, means, hello)<|>(مرحبا, means, hello)");

        let chat_memory = InMemoryChatMessageHistory::new();
        let kg = NetworkxEntityGraph::new();
        let mut memory = ConversationKGMemory::new(llm, chat_memory, kg);

        let mut inputs = HashMap::new();
        inputs.insert(
            "input".to_string(),
            "你好 and مرحبا both mean hello.".to_string(),
        );
        let mut outputs = HashMap::new();
        outputs.insert("output".to_string(), "Correct! 🌍".to_string());

        memory.save_context(&inputs, &outputs).await.unwrap();

        let vars = memory.load_memory_variables(&inputs).await.unwrap();
        assert!(vars.contains_key("history"));
    }
}
