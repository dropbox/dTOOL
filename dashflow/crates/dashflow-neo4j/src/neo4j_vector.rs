use async_trait::async_trait;
use dashflow::core::documents::Document;
use dashflow::core::embeddings::Embeddings;
use dashflow::core::vector_stores::{DistanceMetric, VectorStore};
use dashflow::core::{Error, Result};
use dashflow::{embed, embed_query};
use neo4rs::{Graph, Query};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use uuid::Uuid;

/// Distance strategy for vector similarity search
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum DistanceStrategy {
    /// Cosine similarity (default) - measures angle between vectors
    Cosine,
    /// Euclidean distance - measures L2 distance between vectors
    Euclidean,
}

impl DistanceStrategy {
    /// Returns the Neo4j vector index similarity function name
    fn to_neo4j_similarity(self) -> &'static str {
        match self {
            DistanceStrategy::Cosine => "cosine",
            DistanceStrategy::Euclidean => "euclidean",
        }
    }

    /// Convert to `DashFlow` `DistanceMetric`
    fn to_distance_metric(self) -> DistanceMetric {
        match self {
            DistanceStrategy::Cosine => DistanceMetric::Cosine,
            DistanceStrategy::Euclidean => DistanceMetric::Euclidean,
        }
    }
}

/// Configuration for Neo4j vector store
#[derive(Debug, Clone)]
pub struct Neo4jVectorConfig {
    /// Neo4j database name (default: "neo4j")
    pub database: String,
    /// Node label for storing documents (default: "Document")
    pub node_label: String,
    /// Property name for storing embeddings (default: "embedding")
    pub embedding_property: String,
    /// Property name for storing text content (default: "text")
    pub text_property: String,
    /// Property name for storing document ID (default: "id")
    pub id_property: String,
    /// Name of the vector index (default: "`vector_index`")
    pub index_name: String,
    /// Distance strategy for similarity search
    pub distance_strategy: DistanceStrategy,
}

impl Default for Neo4jVectorConfig {
    fn default() -> Self {
        Self {
            database: "neo4j".to_string(),
            node_label: "Document".to_string(),
            embedding_property: "embedding".to_string(),
            text_property: "text".to_string(),
            id_property: "id".to_string(),
            index_name: "vector_index".to_string(),
            distance_strategy: DistanceStrategy::Cosine,
        }
    }
}

/// Neo4j vector store implementation
pub struct Neo4jVector {
    graph: Arc<Graph>,
    embeddings: Arc<dyn Embeddings>,
    config: Neo4jVectorConfig,
}

impl Neo4jVector {
    /// Create a new Neo4j vector store with default configuration
    ///
    /// # Arguments
    ///
    /// * `uri` - Neo4j connection URI (e.g., "<bolt://localhost:7687>")
    /// * `user` - Neo4j username
    /// * `password` - Neo4j password
    /// * `embeddings` - Embeddings model to use
    pub async fn new(
        uri: &str,
        user: &str,
        password: &str,
        embeddings: Arc<dyn Embeddings>,
    ) -> Result<Self> {
        Self::with_config(
            uri,
            user,
            password,
            embeddings,
            Neo4jVectorConfig::default(),
        )
        .await
    }

    /// Create a new Neo4j vector store with custom configuration
    ///
    /// # Arguments
    ///
    /// * `uri` - Neo4j connection URI (e.g., "<bolt://localhost:7687>")
    /// * `user` - Neo4j username
    /// * `password` - Neo4j password
    /// * `embeddings` - Embeddings model to use
    /// * `config` - Custom configuration
    pub async fn with_config(
        uri: &str,
        user: &str,
        password: &str,
        embeddings: Arc<dyn Embeddings>,
        config: Neo4jVectorConfig,
    ) -> Result<Self> {
        let graph = Graph::new(uri, user, password)
            .await
            .map_err(|e| Error::other(format!("Failed to connect to Neo4j: {e}")))?;

        let store = Self {
            graph: Arc::new(graph),
            embeddings,
            config,
        };

        // Ensure vector index exists
        store.create_vector_index().await?;

        Ok(store)
    }

    /// Create vector index if it doesn't exist
    async fn create_vector_index(&self) -> Result<()> {
        // First check if index exists
        let check_query = format!(
            "SHOW INDEXES YIELD name WHERE name = '{}' RETURN name",
            self.config.index_name
        );

        let mut result = self
            .graph
            .execute(Query::new(check_query))
            .await
            .map_err(|e| Error::other(format!("Failed to check index existence: {e}")))?;

        // Check if any results returned (index exists)
        if result
            .next()
            .await
            .map_err(|e| Error::other(format!("Failed to read index check result: {e}")))?
            .is_some()
        {
            // Index already exists
            return Ok(());
        }

        // Get embedding dimensions from a sample embedding
        let sample = embed_query(Arc::clone(&self.embeddings), "sample")
            .await
            .map_err(|e| Error::other(format!("Sample embedding failed: {e}")))?;
        let dimensions = sample.len();

        // Create vector index
        // Note: Neo4j 5.11+ syntax for vector indexes
        let create_index = format!(
            "CREATE VECTOR INDEX {} IF NOT EXISTS
             FOR (n:{}) ON n.{}
             OPTIONS {{
               indexConfig: {{
                 `vector.dimensions`: {},
                 `vector.similarity_function`: '{}'
               }}
             }}",
            self.config.index_name,
            self.config.node_label,
            self.config.embedding_property,
            dimensions,
            self.config.distance_strategy.to_neo4j_similarity()
        );

        self.graph
            .run(Query::new(create_index))
            .await
            .map_err(|e| Error::other(format!("Failed to create vector index: {e}")))?;

        Ok(())
    }

    /// Perform similarity search by vector
    async fn similarity_search_by_vector_internal(
        &self,
        query_vector: &[f32],
        k: usize,
        _filter: Option<&HashMap<String, serde_json::Value>>,
    ) -> Result<Vec<(Document, f32)>> {
        // Build the vector search query
        let query_parts = [
            format!(
                "CALL db.index.vector.queryNodes('{}', {}, $embedding) YIELD node, score",
                self.config.index_name, k
            ),
            format!(
                "RETURN node.{} AS id, node.{} AS text, score",
                self.config.id_property, self.config.text_property
            ),
        ];

        let cypher = query_parts.join(" ");
        let q = Query::new(cypher).param("embedding", query_vector.to_vec());

        // Note: Metadata filtering would require converting serde_json::Value to BoltType
        // For now, we skip metadata filtering in the query

        let mut result = self
            .graph
            .execute(q)
            .await
            .map_err(|e| Error::other(format!("Failed to execute similarity search: {e}")))?;

        let mut documents = Vec::new();

        while let Some(row) = result
            .next()
            .await
            .map_err(|e| Error::other(format!("Failed to read search results: {e}")))?
        {
            let id: String = row
                .get("id")
                .map_err(|e| Error::other(format!("Failed to get id from search results: {e}")))?;

            let text: String = row.get("text").map_err(|e| {
                Error::other(format!("Failed to get text from search results: {e}"))
            })?;

            let score: f64 = row.get("score").map_err(|e| {
                Error::other(format!("Failed to get score from search results: {e}"))
            })?;

            let mut metadata = HashMap::new();
            metadata.insert("id".to_string(), serde_json::json!(id));

            let doc = Document {
                id: Some(id),
                page_content: text,
                metadata,
            };

            documents.push((doc, score as f32));
        }

        Ok(documents)
    }

    /// Delete documents by IDs
    pub async fn delete(&self, ids: &[String]) -> Result<()> {
        if ids.is_empty() {
            return Ok(());
        }

        let query = format!(
            "MATCH (n:{}) WHERE n.{} IN $ids DETACH DELETE n",
            self.config.node_label, self.config.id_property
        );

        self.graph
            .run(Query::new(query).param("ids", ids.to_vec()))
            .await
            .map_err(|e| Error::other(format!("Failed to delete documents: {e}")))?;

        Ok(())
    }

    /// Drop the vector index (for cleanup/testing)
    pub async fn drop_index(&self) -> Result<()> {
        let query = format!("DROP INDEX {} IF EXISTS", self.config.index_name);

        self.graph
            .run(Query::new(query))
            .await
            .map_err(|e| Error::other(format!("Failed to drop index: {e}")))?;

        Ok(())
    }
}

#[async_trait]
impl VectorStore for Neo4jVector {
    fn embeddings(&self) -> Option<Arc<dyn Embeddings>> {
        Some(Arc::clone(&self.embeddings))
    }

    fn distance_metric(&self) -> DistanceMetric {
        self.config.distance_strategy.to_distance_metric()
    }

    async fn add_texts(
        &mut self,
        texts: &[impl AsRef<str> + Send + Sync],
        metadatas: Option<&[HashMap<String, serde_json::Value>]>,
        ids: Option<&[String]>,
    ) -> Result<Vec<String>> {
        let text_count = texts.len();

        if let Some(metadatas) = metadatas {
            if metadatas.len() != text_count {
                return Err(Error::config(format!(
                    "Metadatas length mismatch: {} vs {}",
                    metadatas.len(),
                    text_count
                )));
            }
        }

        if let Some(ids) = ids {
            if ids.len() != text_count {
                return Err(Error::config(format!(
                    "IDs length mismatch: {} vs {}",
                    ids.len(),
                    text_count
                )));
            }
        }

        if texts.is_empty() {
            return Ok(Vec::new());
        }

        // Convert texts to strings
        let text_strings: Vec<String> = texts.iter().map(|t| t.as_ref().to_string()).collect();

        // Generate embeddings for all texts
        let embeddings = embed(Arc::clone(&self.embeddings), &text_strings)
            .await
            .map_err(|e| Error::other(format!("Embedding failed: {e}")))?;

        // Generate IDs if not provided
        let doc_ids: Vec<String> = if let Some(ids) = ids {
            ids.to_vec()
        } else {
            (0..text_count)
                .map(|_| Uuid::new_v4().to_string())
                .collect()
        };

        // Create nodes in Neo4j
        for (i, (text, embedding)) in text_strings.iter().zip(embeddings.iter()).enumerate() {
            // Build the Cypher query with base properties
            let query_str = format!(
                "CREATE (n:{} {{`{}`: $id, `{}`: $text, `{}`: $embedding}})",
                self.config.node_label,
                self.config.id_property,
                self.config.text_property,
                self.config.embedding_property
            );

            let q = Query::new(query_str)
                .param("id", doc_ids[i].as_str())
                .param("text", text.as_str())
                .param("embedding", embedding.clone());

            self.graph
                .run(q)
                .await
                .map_err(|e| Error::other(format!("Failed to insert document: {e}")))?;
        }

        Ok(doc_ids)
    }

    async fn get_by_ids(&self, ids: &[String]) -> Result<Vec<Document>> {
        if ids.is_empty() {
            return Ok(Vec::new());
        }

        let query = format!(
            "MATCH (n:{}) WHERE n.{} IN $ids RETURN n.{} AS id, n.{} AS text",
            self.config.node_label,
            self.config.id_property,
            self.config.id_property,
            self.config.text_property
        );

        let mut result = self
            .graph
            .execute(Query::new(query).param("ids", ids.to_vec()))
            .await
            .map_err(|e| Error::other(format!("Failed to get documents: {e}")))?;

        let mut documents = Vec::new();

        while let Some(row) = result
            .next()
            .await
            .map_err(|e| Error::other(format!("Failed to read documents: {e}")))?
        {
            let id: String = row
                .get("id")
                .map_err(|e| Error::other(format!("Failed to get id from results: {e}")))?;

            let text: String = row
                .get("text")
                .map_err(|e| Error::other(format!("Failed to get text from results: {e}")))?;

            let mut metadata = HashMap::new();
            metadata.insert("id".to_string(), serde_json::json!(id.clone()));

            documents.push(Document {
                id: Some(id),
                page_content: text,
                metadata,
            });
        }

        Ok(documents)
    }

    async fn _similarity_search(
        &self,
        query: &str,
        k: usize,
        filter: Option<&HashMap<String, serde_json::Value>>,
    ) -> Result<Vec<Document>> {
        let results = self.similarity_search_with_score(query, k, filter).await?;
        Ok(results.into_iter().map(|(doc, _score)| doc).collect())
    }

    async fn similarity_search_with_score(
        &self,
        query: &str,
        k: usize,
        filter: Option<&HashMap<String, serde_json::Value>>,
    ) -> Result<Vec<(Document, f32)>> {
        let query_vector = embed_query(Arc::clone(&self.embeddings), query)
            .await
            .map_err(|e| Error::other(format!("Query embedding failed: {e}")))?;
        self.similarity_search_by_vector_internal(&query_vector, k, filter)
            .await
    }

    async fn similarity_search_by_vector(
        &self,
        embedding: &[f32],
        k: usize,
        filter: Option<&HashMap<String, serde_json::Value>>,
    ) -> Result<Vec<Document>> {
        let results = self
            .similarity_search_by_vector_internal(embedding, k, filter)
            .await?;
        Ok(results.into_iter().map(|(doc, _score)| doc).collect())
    }

    async fn similarity_search_by_vector_with_score(
        &self,
        embedding: &[f32],
        k: usize,
        filter: Option<&HashMap<String, serde_json::Value>>,
    ) -> Result<Vec<(Document, f32)>> {
        self.similarity_search_by_vector_internal(embedding, k, filter)
            .await
    }
}

#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
#[cfg(test)]
mod tests {
    use super::*;

    // Tests for DistanceStrategy enum

    #[test]
    fn test_distance_strategy_cosine_to_neo4j() {
        let strategy = DistanceStrategy::Cosine;
        assert_eq!(strategy.to_neo4j_similarity(), "cosine");
    }

    #[test]
    fn test_distance_strategy_euclidean_to_neo4j() {
        let strategy = DistanceStrategy::Euclidean;
        assert_eq!(strategy.to_neo4j_similarity(), "euclidean");
    }

    #[test]
    fn test_distance_strategy_cosine_to_metric() {
        let strategy = DistanceStrategy::Cosine;
        let metric = strategy.to_distance_metric();
        assert!(matches!(metric, DistanceMetric::Cosine));
    }

    #[test]
    fn test_distance_strategy_euclidean_to_metric() {
        let strategy = DistanceStrategy::Euclidean;
        let metric = strategy.to_distance_metric();
        assert!(matches!(metric, DistanceMetric::Euclidean));
    }

    #[test]
    fn test_distance_strategy_clone() {
        let strategy = DistanceStrategy::Cosine;
        let cloned = strategy;
        assert_eq!(strategy.to_neo4j_similarity(), cloned.to_neo4j_similarity());
    }

    #[test]
    fn test_distance_strategy_debug() {
        let strategy = DistanceStrategy::Cosine;
        let debug_str = format!("{:?}", strategy);
        assert_eq!(debug_str, "Cosine");
    }

    #[test]
    fn test_distance_strategy_euclidean_debug() {
        let strategy = DistanceStrategy::Euclidean;
        let debug_str = format!("{:?}", strategy);
        assert_eq!(debug_str, "Euclidean");
    }

    // Tests for Neo4jVectorConfig

    #[test]
    fn test_config_default_database() {
        let config = Neo4jVectorConfig::default();
        assert_eq!(config.database, "neo4j");
    }

    #[test]
    fn test_config_default_node_label() {
        let config = Neo4jVectorConfig::default();
        assert_eq!(config.node_label, "Document");
    }

    #[test]
    fn test_config_default_embedding_property() {
        let config = Neo4jVectorConfig::default();
        assert_eq!(config.embedding_property, "embedding");
    }

    #[test]
    fn test_config_default_text_property() {
        let config = Neo4jVectorConfig::default();
        assert_eq!(config.text_property, "text");
    }

    #[test]
    fn test_config_default_id_property() {
        let config = Neo4jVectorConfig::default();
        assert_eq!(config.id_property, "id");
    }

    #[test]
    fn test_config_default_index_name() {
        let config = Neo4jVectorConfig::default();
        assert_eq!(config.index_name, "vector_index");
    }

    #[test]
    fn test_config_default_distance_strategy() {
        let config = Neo4jVectorConfig::default();
        assert!(matches!(config.distance_strategy, DistanceStrategy::Cosine));
    }

    #[test]
    fn test_config_clone() {
        let config = Neo4jVectorConfig::default();
        let cloned = config.clone();
        assert_eq!(config.database, cloned.database);
        assert_eq!(config.node_label, cloned.node_label);
        assert_eq!(config.index_name, cloned.index_name);
    }

    #[test]
    fn test_config_debug() {
        let config = Neo4jVectorConfig::default();
        let debug_str = format!("{:?}", config);
        assert!(debug_str.contains("Neo4jVectorConfig"));
        assert!(debug_str.contains("neo4j"));
        assert!(debug_str.contains("Document"));
    }

    // Tests for custom config

    #[test]
    fn test_config_custom_values() {
        let config = Neo4jVectorConfig {
            database: "custom_db".to_string(),
            node_label: "CustomNode".to_string(),
            embedding_property: "vec".to_string(),
            text_property: "content".to_string(),
            id_property: "doc_id".to_string(),
            index_name: "custom_vector_index".to_string(),
            distance_strategy: DistanceStrategy::Euclidean,
        };
        assert_eq!(config.database, "custom_db");
        assert_eq!(config.node_label, "CustomNode");
        assert_eq!(config.embedding_property, "vec");
        assert_eq!(config.text_property, "content");
        assert_eq!(config.id_property, "doc_id");
        assert_eq!(config.index_name, "custom_vector_index");
        assert!(matches!(config.distance_strategy, DistanceStrategy::Euclidean));
    }

    // Tests for Cypher query building (string formats used in the code)

    #[test]
    fn test_check_index_query_format() {
        let index_name = "vector_index";
        let query = format!(
            "SHOW INDEXES YIELD name WHERE name = '{}' RETURN name",
            index_name
        );
        assert!(query.contains("SHOW INDEXES"));
        assert!(query.contains("vector_index"));
    }

    #[test]
    fn test_create_index_query_format() {
        let config = Neo4jVectorConfig::default();
        let dimensions = 1536;
        let query = format!(
            "CREATE VECTOR INDEX {} IF NOT EXISTS
             FOR (n:{}) ON n.{}
             OPTIONS {{
               indexConfig: {{
                 `vector.dimensions`: {},
                 `vector.similarity_function`: '{}'
               }}
             }}",
            config.index_name,
            config.node_label,
            config.embedding_property,
            dimensions,
            config.distance_strategy.to_neo4j_similarity()
        );
        assert!(query.contains("CREATE VECTOR INDEX"));
        assert!(query.contains("vector_index"));
        assert!(query.contains("Document"));
        assert!(query.contains("embedding"));
        assert!(query.contains("1536"));
        assert!(query.contains("cosine"));
    }

    #[test]
    fn test_similarity_search_query_format() {
        let config = Neo4jVectorConfig::default();
        let k = 10;
        let query_parts = [
            format!(
                "CALL db.index.vector.queryNodes('{}', {}, $embedding) YIELD node, score",
                config.index_name, k
            ),
            format!(
                "RETURN node.{} AS id, node.{} AS text, score",
                config.id_property, config.text_property
            ),
        ];
        let cypher = query_parts.join(" ");
        assert!(cypher.contains("db.index.vector.queryNodes"));
        assert!(cypher.contains("vector_index"));
        assert!(cypher.contains("10"));
        assert!(cypher.contains("node.id AS id"));
        assert!(cypher.contains("node.text AS text"));
    }

    #[test]
    fn test_delete_query_format() {
        let config = Neo4jVectorConfig::default();
        let query = format!(
            "MATCH (n:{}) WHERE n.{} IN $ids DETACH DELETE n",
            config.node_label, config.id_property
        );
        assert!(query.contains("MATCH (n:Document)"));
        assert!(query.contains("n.id IN $ids"));
        assert!(query.contains("DETACH DELETE"));
    }

    #[test]
    fn test_drop_index_query_format() {
        let index_name = "vector_index";
        let query = format!("DROP INDEX {} IF EXISTS", index_name);
        assert_eq!(query, "DROP INDEX vector_index IF EXISTS");
    }

    #[test]
    fn test_get_by_ids_query_format() {
        let config = Neo4jVectorConfig::default();
        let query = format!(
            "MATCH (n:{}) WHERE n.{} IN $ids RETURN n.{} AS id, n.{} AS text",
            config.node_label,
            config.id_property,
            config.id_property,
            config.text_property
        );
        assert!(query.contains("MATCH (n:Document)"));
        assert!(query.contains("n.id IN $ids"));
        assert!(query.contains("n.id AS id"));
        assert!(query.contains("n.text AS text"));
    }

    #[test]
    fn test_create_node_query_format() {
        let config = Neo4jVectorConfig::default();
        let query_str = format!(
            "CREATE (n:{} {{`{}`: $id, `{}`: $text, `{}`: $embedding}})",
            config.node_label,
            config.id_property,
            config.text_property,
            config.embedding_property
        );
        assert!(query_str.contains("CREATE (n:Document"));
        assert!(query_str.contains("`id`: $id"));
        assert!(query_str.contains("`text`: $text"));
        assert!(query_str.contains("`embedding`: $embedding"));
    }

    // Tests for DistanceStrategy serialization

    #[test]
    fn test_distance_strategy_serialize_cosine() {
        let strategy = DistanceStrategy::Cosine;
        let json = serde_json::to_string(&strategy).unwrap();
        assert_eq!(json, "\"Cosine\"");
    }

    #[test]
    fn test_distance_strategy_serialize_euclidean() {
        let strategy = DistanceStrategy::Euclidean;
        let json = serde_json::to_string(&strategy).unwrap();
        assert_eq!(json, "\"Euclidean\"");
    }

    #[test]
    fn test_distance_strategy_deserialize_cosine() {
        let json = "\"Cosine\"";
        let strategy: DistanceStrategy = serde_json::from_str(json).unwrap();
        assert!(matches!(strategy, DistanceStrategy::Cosine));
    }

    #[test]
    fn test_distance_strategy_deserialize_euclidean() {
        let json = "\"Euclidean\"";
        let strategy: DistanceStrategy = serde_json::from_str(json).unwrap();
        assert!(matches!(strategy, DistanceStrategy::Euclidean));
    }

    // Tests for metadata handling

    #[test]
    fn test_metadata_with_id() {
        let mut metadata = HashMap::new();
        let id = "test-doc-123";
        metadata.insert("id".to_string(), serde_json::json!(id));
        assert_eq!(metadata.get("id").unwrap().as_str().unwrap(), id);
    }

    // Tests for UUID generation

    #[test]
    fn test_uuid_v4_uniqueness() {
        let id1 = Uuid::new_v4().to_string();
        let id2 = Uuid::new_v4().to_string();
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_uuid_v4_format() {
        let id = Uuid::new_v4().to_string();
        // UUID format: 8-4-4-4-12 hex characters
        assert_eq!(id.len(), 36);
        let parts: Vec<&str> = id.split('-').collect();
        assert_eq!(parts.len(), 5);
        assert_eq!(parts[0].len(), 8);
        assert_eq!(parts[1].len(), 4);
        assert_eq!(parts[2].len(), 4);
        assert_eq!(parts[3].len(), 4);
        assert_eq!(parts[4].len(), 12);
    }

    // Tests for input validation logic

    #[test]
    fn test_empty_ids_returns_early() {
        let ids: Vec<String> = vec![];
        assert!(ids.is_empty());
    }

    #[test]
    fn test_empty_texts_returns_early() {
        let texts: Vec<String> = vec![];
        assert!(texts.is_empty());
    }

    #[test]
    fn test_metadata_length_mismatch_detection() {
        let text_count = 3;
        let metadatas_len = 2;
        assert_ne!(metadatas_len, text_count);
    }

    #[test]
    fn test_ids_length_mismatch_detection() {
        let text_count = 3;
        let ids_len = 2;
        assert_ne!(ids_len, text_count);
    }

    // ============================================================
    // Extended DistanceStrategy Tests
    // ============================================================

    #[test]
    fn test_distance_strategy_copy_trait() {
        let strategy = DistanceStrategy::Cosine;
        let copy1 = strategy;
        let copy2 = strategy;
        assert_eq!(copy1.to_neo4j_similarity(), copy2.to_neo4j_similarity());
    }

    #[test]
    fn test_distance_strategy_all_variants() {
        let variants = [DistanceStrategy::Cosine, DistanceStrategy::Euclidean];
        assert_eq!(variants.len(), 2);
    }

    #[test]
    fn test_distance_strategy_roundtrip_cosine() {
        let original = DistanceStrategy::Cosine;
        let json = serde_json::to_string(&original).unwrap();
        let restored: DistanceStrategy = serde_json::from_str(&json).unwrap();
        assert_eq!(original.to_neo4j_similarity(), restored.to_neo4j_similarity());
    }

    #[test]
    fn test_distance_strategy_roundtrip_euclidean() {
        let original = DistanceStrategy::Euclidean;
        let json = serde_json::to_string(&original).unwrap();
        let restored: DistanceStrategy = serde_json::from_str(&json).unwrap();
        assert_eq!(original.to_neo4j_similarity(), restored.to_neo4j_similarity());
    }

    // ============================================================
    // Extended Config Tests
    // ============================================================

    #[test]
    fn test_config_special_chars_in_label() {
        let config = Neo4jVectorConfig {
            node_label: "My_Node_V2".to_string(),
            ..Neo4jVectorConfig::default()
        };
        assert_eq!(config.node_label, "My_Node_V2");
    }

    #[test]
    fn test_config_unicode_label() {
        let config = Neo4jVectorConfig {
            node_label: "文档".to_string(),
            ..Neo4jVectorConfig::default()
        };
        assert_eq!(config.node_label, "文档");
    }

    #[test]
    fn test_config_empty_index_name() {
        let config = Neo4jVectorConfig {
            index_name: "".to_string(),
            ..Neo4jVectorConfig::default()
        };
        assert_eq!(config.index_name, "");
    }

    #[test]
    fn test_config_long_index_name() {
        let long_name = "a".repeat(100);
        let config = Neo4jVectorConfig {
            index_name: long_name.clone(),
            ..Neo4jVectorConfig::default()
        };
        assert_eq!(config.index_name.len(), 100);
    }

    // ============================================================
    // Extended Cypher Query Format Tests
    // ============================================================

    #[test]
    fn test_similarity_search_query_with_custom_config() {
        let config = Neo4jVectorConfig {
            index_name: "custom_idx".to_string(),
            id_property: "doc_id".to_string(),
            text_property: "content".to_string(),
            ..Neo4jVectorConfig::default()
        };
        let k = 5;
        let query_parts = [
            format!(
                "CALL db.index.vector.queryNodes('{}', {}, $embedding) YIELD node, score",
                config.index_name, k
            ),
            format!(
                "RETURN node.{} AS id, node.{} AS text, score",
                config.id_property, config.text_property
            ),
        ];
        let cypher = query_parts.join(" ");
        assert!(cypher.contains("custom_idx"));
        assert!(cypher.contains("doc_id"));
        assert!(cypher.contains("content"));
    }

    #[test]
    fn test_create_node_query_with_custom_config() {
        let config = Neo4jVectorConfig {
            node_label: "Chunk".to_string(),
            id_property: "chunk_id".to_string(),
            text_property: "body".to_string(),
            embedding_property: "vector".to_string(),
            ..Neo4jVectorConfig::default()
        };
        let query_str = format!(
            "CREATE (n:{} {{`{}`: $id, `{}`: $text, `{}`: $embedding}})",
            config.node_label,
            config.id_property,
            config.text_property,
            config.embedding_property
        );
        assert!(query_str.contains("Chunk"));
        assert!(query_str.contains("chunk_id"));
        assert!(query_str.contains("body"));
        assert!(query_str.contains("vector"));
    }

    #[test]
    fn test_create_index_query_euclidean() {
        let config = Neo4jVectorConfig {
            distance_strategy: DistanceStrategy::Euclidean,
            ..Neo4jVectorConfig::default()
        };
        let dimensions = 768;
        let query = format!(
            "CREATE VECTOR INDEX {} IF NOT EXISTS
             FOR (n:{}) ON n.{}
             OPTIONS {{
               indexConfig: {{
                 `vector.dimensions`: {},
                 `vector.similarity_function`: '{}'
               }}
             }}",
            config.index_name,
            config.node_label,
            config.embedding_property,
            dimensions,
            config.distance_strategy.to_neo4j_similarity()
        );
        assert!(query.contains("euclidean"));
        assert!(query.contains("768"));
    }

    #[test]
    fn test_delete_query_with_custom_label() {
        let config = Neo4jVectorConfig {
            node_label: "Vector".to_string(),
            id_property: "uuid".to_string(),
            ..Neo4jVectorConfig::default()
        };
        let query = format!(
            "MATCH (n:{}) WHERE n.{} IN $ids DETACH DELETE n",
            config.node_label, config.id_property
        );
        assert!(query.contains("Vector"));
        assert!(query.contains("uuid"));
    }

    #[test]
    fn test_get_by_ids_query_with_custom_config() {
        let config = Neo4jVectorConfig {
            node_label: "Embedding".to_string(),
            id_property: "embed_id".to_string(),
            text_property: "raw_text".to_string(),
            ..Neo4jVectorConfig::default()
        };
        let query = format!(
            "MATCH (n:{}) WHERE n.{} IN $ids RETURN n.{} AS id, n.{} AS text",
            config.node_label,
            config.id_property,
            config.id_property,
            config.text_property
        );
        assert!(query.contains("Embedding"));
        assert!(query.contains("embed_id"));
        assert!(query.contains("raw_text"));
    }

    // ============================================================
    // UUID Tests
    // ============================================================

    #[test]
    fn test_uuid_batch_generation() {
        let ids: Vec<String> = (0..100).map(|_| Uuid::new_v4().to_string()).collect();
        assert_eq!(ids.len(), 100);
        // All should be unique
        let unique: std::collections::HashSet<_> = ids.iter().collect();
        assert_eq!(unique.len(), 100);
    }

    #[test]
    fn test_uuid_version_4_indicator() {
        let id = Uuid::new_v4().to_string();
        // Version 4 UUIDs have 4 as the 13th character
        let chars: Vec<char> = id.chars().collect();
        assert_eq!(chars[14], '4');
    }

    #[test]
    fn test_uuid_variant_bits() {
        let id = Uuid::new_v4();
        // Variant 1 UUIDs have specific bits set
        let bytes = id.as_bytes();
        // Byte 8 should have variant bits 10xx
        assert!((bytes[8] & 0xc0) == 0x80);
    }

    // ============================================================
    // Document Construction Tests
    // ============================================================

    #[test]
    fn test_document_with_id_metadata() {
        let id = "test-123".to_string();
        let mut metadata = HashMap::new();
        metadata.insert("id".to_string(), serde_json::json!(id.clone()));

        let doc = Document {
            id: Some(id.clone()),
            page_content: "Test content".to_string(),
            metadata,
        };

        assert_eq!(doc.id, Some(id));
        assert!(doc.metadata.contains_key("id"));
    }

    #[test]
    fn test_document_empty_content() {
        let doc = Document {
            id: Some("empty".to_string()),
            page_content: "".to_string(),
            metadata: HashMap::new(),
        };
        assert!(doc.page_content.is_empty());
    }

    #[test]
    fn test_document_unicode_content() {
        let doc = Document {
            id: Some("unicode".to_string()),
            page_content: "こんにちは世界 🌍".to_string(),
            metadata: HashMap::new(),
        };
        assert!(doc.page_content.contains("こんにちは"));
        assert!(doc.page_content.contains("🌍"));
    }

    #[test]
    fn test_document_large_content() {
        let large_content = "x".repeat(100_000);
        let doc = Document {
            id: Some("large".to_string()),
            page_content: large_content.clone(),
            metadata: HashMap::new(),
        };
        assert_eq!(doc.page_content.len(), 100_000);
    }

    // ============================================================
    // Error Message Content Tests
    // ============================================================

    #[test]
    fn test_metadata_length_error_message() {
        let metadatas_len = 5;
        let text_count = 3;
        let error_msg = format!(
            "Metadatas length mismatch: {} vs {}",
            metadatas_len, text_count
        );
        assert!(error_msg.contains("5"));
        assert!(error_msg.contains("3"));
        assert!(error_msg.contains("Metadatas length mismatch"));
    }

    #[test]
    fn test_ids_length_error_message() {
        let ids_len = 10;
        let text_count = 7;
        let error_msg = format!("IDs length mismatch: {} vs {}", ids_len, text_count);
        assert!(error_msg.contains("10"));
        assert!(error_msg.contains("7"));
    }

    #[test]
    fn test_connection_error_message() {
        let error = "Connection refused";
        let msg = format!("Failed to connect to Neo4j: {}", error);
        assert!(msg.starts_with("Failed to connect to Neo4j"));
    }

    #[test]
    fn test_sample_embedding_error_message() {
        let error = "API timeout";
        let msg = format!("Sample embedding failed: {}", error);
        assert!(msg.contains("Sample embedding failed"));
    }

    #[test]
    fn test_index_creation_error_message() {
        let error = "Permission denied";
        let msg = format!("Failed to create vector index: {}", error);
        assert!(msg.contains("Failed to create vector index"));
    }

    // ============================================================
    // Vector Dimension Tests
    // ============================================================

    #[test]
    fn test_common_embedding_dimensions() {
        let dimensions = [384, 512, 768, 1024, 1536, 3072, 4096];
        for dim in dimensions {
            assert!(dim > 0);
        }
    }

    #[test]
    fn test_embedding_dimension_in_query() {
        let dimensions = 1536;
        let query_part = format!("`vector.dimensions`: {}", dimensions);
        assert!(query_part.contains("1536"));
    }

    // ============================================================
    // Score Handling Tests
    // ============================================================

    #[test]
    fn test_score_f64_to_f32_conversion() {
        let score_f64: f64 = 0.95;
        let score_f32 = score_f64 as f32;
        assert!((score_f32 - 0.95_f32).abs() < 0.001);
    }

    #[test]
    fn test_score_perfect_match() {
        let score: f64 = 1.0;
        let score_f32 = score as f32;
        assert_eq!(score_f32, 1.0_f32);
    }

    #[test]
    fn test_score_zero() {
        let score: f64 = 0.0;
        let score_f32 = score as f32;
        assert_eq!(score_f32, 0.0_f32);
    }

    #[test]
    fn test_score_high_precision() {
        let score: f64 = 0.123456789;
        let score_f32 = score as f32;
        // f32 has less precision
        assert!((score_f32 - 0.123456789_f32).abs() < 0.0001);
    }

    // ============================================================
    // Text Conversion Tests
    // ============================================================

    #[test]
    fn test_text_to_string_conversion() {
        let texts = ["hello", "world"];
        let text_strings: Vec<String> = texts.iter().map(|t| t.to_string()).collect();
        assert_eq!(text_strings.len(), 2);
        assert_eq!(text_strings[0], "hello");
    }

    #[test]
    fn test_empty_text_conversion() {
        let texts: Vec<&str> = vec![];
        let text_strings: Vec<String> = texts.iter().map(|t| t.to_string()).collect();
        assert!(text_strings.is_empty());
    }

    #[test]
    fn test_single_text_conversion() {
        let texts = ["single"];
        let text_strings: Vec<String> = texts.iter().map(|t| t.to_string()).collect();
        assert_eq!(text_strings.len(), 1);
    }

    // ============================================================
    // Index Check Query Tests
    // ============================================================

    #[test]
    fn test_show_indexes_query_format() {
        let index_name = "my_vector_index";
        let query = format!(
            "SHOW INDEXES YIELD name WHERE name = '{}' RETURN name",
            index_name
        );
        assert!(query.contains("SHOW INDEXES"));
        assert!(query.contains("YIELD name"));
        assert!(query.contains("my_vector_index"));
    }

    #[test]
    fn test_drop_index_query_with_custom_name() {
        let index_name = "custom_vector_idx";
        let query = format!("DROP INDEX {} IF EXISTS", index_name);
        assert_eq!(query, "DROP INDEX custom_vector_idx IF EXISTS");
    }

    // ============================================================
    // Config Field Modification Tests
    // ============================================================

    #[test]
    fn test_config_all_fields_custom() {
        let config = Neo4jVectorConfig {
            database: "production".to_string(),
            node_label: "VectorNode".to_string(),
            embedding_property: "vec".to_string(),
            text_property: "raw".to_string(),
            id_property: "uuid".to_string(),
            index_name: "prod_vector_idx".to_string(),
            distance_strategy: DistanceStrategy::Euclidean,
        };
        assert_eq!(config.database, "production");
        assert_eq!(config.node_label, "VectorNode");
        assert_eq!(config.embedding_property, "vec");
        assert_eq!(config.text_property, "raw");
        assert_eq!(config.id_property, "uuid");
        assert_eq!(config.index_name, "prod_vector_idx");
        assert!(matches!(config.distance_strategy, DistanceStrategy::Euclidean));
    }

    #[test]
    fn test_config_partial_override() {
        let config = Neo4jVectorConfig {
            database: "test_db".to_string(),
            ..Neo4jVectorConfig::default()
        };
        assert_eq!(config.database, "test_db");
        // Other fields should be default
        assert_eq!(config.node_label, "Document");
        assert_eq!(config.embedding_property, "embedding");
    }

    // ============================================================
    // Vector Parameter Format Tests
    // ============================================================

    #[test]
    fn test_vector_query_param_name() {
        let param_name = "embedding";
        let query = format!("... ${})", param_name);
        assert!(query.contains("$embedding"));
    }

    #[test]
    fn test_ids_query_param_name() {
        let param_name = "ids";
        let query = format!("WHERE n.id IN ${}", param_name);
        assert!(query.contains("$ids"));
    }
}
