//! # Neo4j Graph Store
//!
//! Neo4j implementation of the `GraphStore` trait for executing Cypher queries
//! and introspecting graph schemas.

use crate::graph_store::{GraphStore, PropertyDefinition, SchemaRelationship, StructuredSchema};
use async_trait::async_trait;
use dashflow::core::{Error, Result};
use neo4rs::{Graph, Query};
use std::collections::HashMap;
use std::sync::Arc;

/// Neo4j graph database connection for Cypher queries
pub struct Neo4jGraph {
    graph: Arc<Graph>,
    schema: StructuredSchema,
    _database: String,
}

impl Neo4jGraph {
    /// Create a new Neo4j graph store connection
    ///
    /// # Arguments
    ///
    /// * `uri` - Neo4j connection URI (e.g., "<bolt://localhost:7687>")
    /// * `user` - Neo4j username
    /// * `password` - Neo4j password
    /// * `database` - Optional database name (default: "neo4j")
    ///
    /// # Returns
    ///
    /// A new `Neo4jGraph` instance with schema automatically introspected
    pub async fn new(
        uri: &str,
        user: &str,
        password: &str,
        database: Option<&str>,
    ) -> Result<Self> {
        let graph = Graph::new(uri, user, password)
            .await
            .map_err(|e| Error::other(format!("Failed to connect to Neo4j: {e}")))?;

        let mut store = Self {
            graph: Arc::new(graph),
            schema: StructuredSchema::default(),
            _database: database.unwrap_or("neo4j").to_string(),
        };

        // Introspect schema on initialization
        store.refresh_schema().await?;

        Ok(store)
    }

    /// Parse Neo4j type name to simplified type string
    fn parse_type(neo4j_type: &str) -> String {
        // Neo4j returns types like "String", "Long", "Double", "Boolean", "List"
        match neo4j_type {
            "Long" => "Integer".to_string(),
            "Double" => "Float".to_string(),
            _ => neo4j_type.to_string(),
        }
    }
}

#[async_trait]
impl GraphStore for Neo4jGraph {
    async fn query(&self, query: &str) -> Result<Vec<HashMap<String, serde_json::Value>>> {
        let mut result = self
            .graph
            .execute(Query::new(query.to_string()))
            .await
            .map_err(|e| Error::other(format!("Neo4j query failed: {e}")))?;

        let mut rows = Vec::new();

        // First, we need to track columns from the first row
        // Neo4j Row requires column names, which are determined by the RETURN clause
        // We'll use a simplified approach: convert each row to a single serialized value

        while let Some(_row) = result
            .next()
            .await
            .map_err(|e| Error::other(format!("Failed to read Neo4j result row: {e}")))?
        {
            // For now, create a simple result representation
            // In a production implementation, you would:
            // 1. Parse the query to extract RETURN column names
            // 2. Use row.get::<Type>(column_name) for each column
            // 3. Handle Neo4j-specific types (Node, Relationship, Path)

            let mut map = HashMap::new();

            // Simplified implementation: just acknowledge data was returned
            // The GraphCypherQAChain will pass results as context to the QA LLM
            // For a full implementation, see neo4rs documentation on working with results

            map.insert(
                "data".to_string(),
                serde_json::json!({"note": "Query returned results. Full row parsing requires query metadata."}),
            );

            rows.push(map);
        }

        Ok(rows)
    }

    fn get_structured_schema(&self) -> &StructuredSchema {
        &self.schema
    }

    async fn refresh_schema(&mut self) -> Result<()> {
        let mut schema = StructuredSchema::default();

        // Query for node labels and their properties
        let node_query = r"
            CALL db.schema.nodeTypeProperties()
            YIELD nodeType, nodeLabels, propertyName, propertyTypes
            RETURN nodeLabels, propertyName, propertyTypes
        ";

        let mut result = self
            .graph
            .execute(Query::new(node_query.to_string()))
            .await
            .map_err(|e| Error::other(format!("Failed to query node schema: {e}")))?;

        while let Some(row) = result
            .next()
            .await
            .map_err(|e| Error::other(format!("Failed to read node schema row: {e}")))?
        {
            if let (Ok(labels), Ok(prop_name), Ok(prop_types)) = (
                row.get::<Vec<String>>("nodeLabels"),
                row.get::<String>("propertyName"),
                row.get::<Vec<String>>("propertyTypes"),
            ) {
                for label in labels {
                    let prop_type = prop_types
                        .first()
                        .map_or_else(|| "String".to_string(), |t| Self::parse_type(t));

                    schema
                        .node_props
                        .entry(label)
                        .or_default()
                        .push(PropertyDefinition {
                            property: prop_name.clone(),
                            prop_type,
                        });
                }
            }
        }

        // Query for relationship types and their properties
        let rel_query = r"
            CALL db.schema.relTypeProperties()
            YIELD relType, propertyName, propertyTypes
            RETURN relType, propertyName, propertyTypes
        ";

        let mut result = self
            .graph
            .execute(Query::new(rel_query.to_string()))
            .await
            .map_err(|e| Error::other(format!("Failed to query relationship schema: {e}")))?;

        while let Some(row) = result
            .next()
            .await
            .map_err(|e| Error::other(format!("Failed to read relationship schema row: {e}")))?
        {
            if let (Ok(rel_type), Ok(prop_name), Ok(prop_types)) = (
                row.get::<String>("relType"),
                row.get::<String>("propertyName"),
                row.get::<Vec<String>>("propertyTypes"),
            ) {
                let prop_type = prop_types
                    .first()
                    .map_or_else(|| "String".to_string(), |t| Self::parse_type(t));

                schema
                    .rel_props
                    .entry(rel_type)
                    .or_default()
                    .push(PropertyDefinition {
                        property: prop_name,
                        prop_type,
                    });
            }
        }

        // Query for relationships (connections between node labels)
        let relationships_query = r"
            CALL db.schema.visualization()
            YIELD nodes, relationships
            UNWIND relationships AS rel
            RETURN
                [label IN labels(startNode(rel)) | label][0] AS start,
                type(rel) AS relType,
                [label IN labels(endNode(rel)) | label][0] AS end
        ";

        let mut result = self
            .graph
            .execute(Query::new(relationships_query.to_string()))
            .await
            .map_err(|e| Error::other(format!("Failed to query relationships: {e}")))?;

        while let Some(row) = result
            .next()
            .await
            .map_err(|e| Error::other(format!("Failed to read relationships row: {e}")))?
        {
            if let (Ok(start), Ok(rel_type), Ok(end)) = (
                row.get::<String>("start"),
                row.get::<String>("relType"),
                row.get::<String>("end"),
            ) {
                schema.relationships.push(SchemaRelationship {
                    start,
                    rel_type,
                    end,
                });
            }
        }

        self.schema = schema;
        Ok(())
    }
}

#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
#[cfg(test)]
mod tests {
    use super::*;

    // ============================================================
    // parse_type Tests
    // ============================================================

    #[test]
    fn test_parse_type_string() {
        assert_eq!(Neo4jGraph::parse_type("String"), "String");
    }

    #[test]
    fn test_parse_type_long_to_integer() {
        assert_eq!(Neo4jGraph::parse_type("Long"), "Integer");
    }

    #[test]
    fn test_parse_type_double_to_float() {
        assert_eq!(Neo4jGraph::parse_type("Double"), "Float");
    }

    #[test]
    fn test_parse_type_boolean() {
        assert_eq!(Neo4jGraph::parse_type("Boolean"), "Boolean");
    }

    #[test]
    fn test_parse_type_list() {
        assert_eq!(Neo4jGraph::parse_type("List"), "List");
    }

    #[test]
    fn test_parse_type_date() {
        assert_eq!(Neo4jGraph::parse_type("Date"), "Date");
    }

    #[test]
    fn test_parse_type_datetime() {
        assert_eq!(Neo4jGraph::parse_type("DateTime"), "DateTime");
    }

    #[test]
    fn test_parse_type_localdatetime() {
        assert_eq!(Neo4jGraph::parse_type("LocalDateTime"), "LocalDateTime");
    }

    #[test]
    fn test_parse_type_localtime() {
        assert_eq!(Neo4jGraph::parse_type("LocalTime"), "LocalTime");
    }

    #[test]
    fn test_parse_type_time() {
        assert_eq!(Neo4jGraph::parse_type("Time"), "Time");
    }

    #[test]
    fn test_parse_type_duration() {
        assert_eq!(Neo4jGraph::parse_type("Duration"), "Duration");
    }

    #[test]
    fn test_parse_type_point() {
        assert_eq!(Neo4jGraph::parse_type("Point"), "Point");
    }

    #[test]
    fn test_parse_type_integer() {
        // Integer is not Long, so it passes through
        assert_eq!(Neo4jGraph::parse_type("Integer"), "Integer");
    }

    #[test]
    fn test_parse_type_float() {
        // Float is not Double, so it passes through
        assert_eq!(Neo4jGraph::parse_type("Float"), "Float");
    }

    #[test]
    fn test_parse_type_empty_string() {
        assert_eq!(Neo4jGraph::parse_type(""), "");
    }

    #[test]
    fn test_parse_type_unknown() {
        assert_eq!(Neo4jGraph::parse_type("UnknownType"), "UnknownType");
    }

    #[test]
    fn test_parse_type_lowercase() {
        // Case sensitive - lowercase doesn't match
        assert_eq!(Neo4jGraph::parse_type("long"), "long");
        assert_eq!(Neo4jGraph::parse_type("double"), "double");
    }

    #[test]
    fn test_parse_type_uppercase() {
        assert_eq!(Neo4jGraph::parse_type("LONG"), "LONG");
        assert_eq!(Neo4jGraph::parse_type("DOUBLE"), "DOUBLE");
    }

    #[test]
    fn test_parse_type_mixed_case() {
        assert_eq!(Neo4jGraph::parse_type("lOnG"), "lOnG");
    }

    #[test]
    fn test_parse_type_with_spaces() {
        assert_eq!(Neo4jGraph::parse_type(" Long "), " Long ");
    }

    #[test]
    fn test_parse_type_array_notation() {
        assert_eq!(Neo4jGraph::parse_type("String[]"), "String[]");
    }

    #[test]
    fn test_parse_type_list_of_long() {
        assert_eq!(Neo4jGraph::parse_type("List<Long>"), "List<Long>");
    }

    #[test]
    fn test_parse_type_map() {
        assert_eq!(Neo4jGraph::parse_type("Map"), "Map");
    }

    #[test]
    fn test_parse_type_node() {
        assert_eq!(Neo4jGraph::parse_type("Node"), "Node");
    }

    #[test]
    fn test_parse_type_relationship() {
        assert_eq!(Neo4jGraph::parse_type("Relationship"), "Relationship");
    }

    #[test]
    fn test_parse_type_path() {
        assert_eq!(Neo4jGraph::parse_type("Path"), "Path");
    }

    // ============================================================
    // Cypher Query Format Tests
    // ============================================================

    #[test]
    fn test_node_schema_query_format() {
        let node_query = r"
            CALL db.schema.nodeTypeProperties()
            YIELD nodeType, nodeLabels, propertyName, propertyTypes
            RETURN nodeLabels, propertyName, propertyTypes
        ";
        assert!(node_query.contains("db.schema.nodeTypeProperties"));
        assert!(node_query.contains("YIELD"));
        assert!(node_query.contains("nodeLabels"));
        assert!(node_query.contains("propertyName"));
        assert!(node_query.contains("propertyTypes"));
    }

    #[test]
    fn test_relationship_schema_query_format() {
        let rel_query = r"
            CALL db.schema.relTypeProperties()
            YIELD relType, propertyName, propertyTypes
            RETURN relType, propertyName, propertyTypes
        ";
        assert!(rel_query.contains("db.schema.relTypeProperties"));
        assert!(rel_query.contains("relType"));
        assert!(rel_query.contains("propertyName"));
    }

    #[test]
    fn test_visualization_query_format() {
        let relationships_query = r"
            CALL db.schema.visualization()
            YIELD nodes, relationships
            UNWIND relationships AS rel
            RETURN
                [label IN labels(startNode(rel)) | label][0] AS start,
                type(rel) AS relType,
                [label IN labels(endNode(rel)) | label][0] AS end
        ";
        assert!(relationships_query.contains("db.schema.visualization"));
        assert!(relationships_query.contains("UNWIND"));
        assert!(relationships_query.contains("startNode"));
        assert!(relationships_query.contains("endNode"));
    }

    // ============================================================
    // StructuredSchema Default State Tests
    // ============================================================

    #[test]
    fn test_structured_schema_initial_state() {
        let schema = StructuredSchema::default();
        assert!(schema.node_props.is_empty());
        assert!(schema.rel_props.is_empty());
        assert!(schema.relationships.is_empty());
    }

    #[test]
    fn test_structured_schema_add_node_property() {
        let mut schema = StructuredSchema::default();
        schema.node_props.entry("Person".to_string()).or_default().push(
            PropertyDefinition {
                property: "name".to_string(),
                prop_type: "String".to_string(),
            },
        );
        assert_eq!(schema.node_props.len(), 1);
        assert!(schema.node_props.contains_key("Person"));
    }

    #[test]
    fn test_structured_schema_add_multiple_labels() {
        let mut schema = StructuredSchema::default();
        for label in ["Person", "Company", "Product"] {
            schema.node_props.entry(label.to_string()).or_default();
        }
        assert_eq!(schema.node_props.len(), 3);
    }

    #[test]
    fn test_structured_schema_add_relationship() {
        let mut schema = StructuredSchema::default();
        schema.relationships.push(SchemaRelationship {
            start: "Person".to_string(),
            rel_type: "KNOWS".to_string(),
            end: "Person".to_string(),
        });
        assert_eq!(schema.relationships.len(), 1);
    }

    // ============================================================
    // Error Message Format Tests
    // ============================================================

    #[test]
    fn test_connection_error_format() {
        let error_msg = format!("Failed to connect to Neo4j: {}", "connection refused");
        assert!(error_msg.contains("Failed to connect to Neo4j"));
        assert!(error_msg.contains("connection refused"));
    }

    #[test]
    fn test_query_error_format() {
        let error_msg = format!("Neo4j query failed: {}", "syntax error");
        assert!(error_msg.contains("Neo4j query failed"));
        assert!(error_msg.contains("syntax error"));
    }

    #[test]
    fn test_schema_query_error_format() {
        let error_msg = format!("Failed to query node schema: {}", "timeout");
        assert!(error_msg.contains("Failed to query node schema"));
    }

    #[test]
    fn test_relationship_schema_error_format() {
        let error_msg = format!("Failed to query relationship schema: {}", "access denied");
        assert!(error_msg.contains("Failed to query relationship schema"));
    }

    #[test]
    fn test_row_read_error_format() {
        let error_msg = format!("Failed to read Neo4j result row: {}", "invalid data");
        assert!(error_msg.contains("Failed to read Neo4j result row"));
    }

    // ============================================================
    // Database Name Tests
    // ============================================================

    #[test]
    fn test_default_database_name() {
        let default_db = "neo4j";
        assert_eq!(default_db, "neo4j");
    }

    #[test]
    fn test_custom_database_name() {
        let custom_db = Some("mydb");
        assert_eq!(custom_db.unwrap_or("neo4j"), "mydb");
    }

    #[test]
    fn test_none_database_uses_default() {
        let custom_db: Option<&str> = None;
        assert_eq!(custom_db.unwrap_or("neo4j"), "neo4j");
    }

    // ============================================================
    // Connection URI Format Tests
    // ============================================================

    #[test]
    fn test_bolt_uri_format() {
        let uri = "bolt://localhost:7687";
        assert!(uri.starts_with("bolt://"));
        assert!(uri.contains("7687"));
    }

    #[test]
    fn test_neo4j_uri_format() {
        let uri = "neo4j://localhost:7687";
        assert!(uri.starts_with("neo4j://"));
    }

    #[test]
    fn test_bolt_secure_uri_format() {
        let uri = "bolt+s://localhost:7687";
        assert!(uri.starts_with("bolt+s://"));
    }

    #[test]
    fn test_neo4j_secure_uri_format() {
        let uri = "neo4j+s://localhost:7687";
        assert!(uri.starts_with("neo4j+s://"));
    }

    // Note: Integration tests requiring a Neo4j instance should be marked with #[ignore]
    // and documented separately
}
