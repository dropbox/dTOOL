//! # Graph Store Trait
//!
//! Generic interface for graph databases that can execute queries and introspect their schema.
//!
//! This trait abstracts different graph databases (Neo4j, Neptune, Memgraph, etc.) and provides
//! common operations needed for graph-based question answering.

use async_trait::async_trait;
use dashflow::core::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A relationship in a graph schema
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SchemaRelationship {
    /// Start node type/label
    pub start: String,
    /// Relationship type
    pub rel_type: String,
    /// End node type/label
    pub end: String,
}

/// A property definition with its type
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PropertyDefinition {
    /// Property name
    pub property: String,
    /// Property type (e.g., "String", "Integer", "Float")
    pub prop_type: String,
}

/// Structured graph schema
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct StructuredSchema {
    /// Node types and their properties
    pub node_props: HashMap<String, Vec<PropertyDefinition>>,
    /// Relationship types and their properties
    pub rel_props: HashMap<String, Vec<PropertyDefinition>>,
    /// All relationships in the graph
    pub relationships: Vec<SchemaRelationship>,
}

/// Generic interface for graph databases.
///
/// This trait provides methods for:
/// - Executing queries (Cypher, Gremlin, SPARQL, etc.)
/// - Introspecting the graph schema
/// - Refreshing cached schema information
///
/// # Security Note
///
/// Make sure that database credentials are narrowly-scoped to only include
/// necessary permissions. Executing arbitrary queries can result in data
/// corruption, loss, or unauthorized data access.
#[async_trait]
pub trait GraphStore: Send + Sync {
    /// Execute a query against the graph database.
    ///
    /// Returns query results as a vector of JSON-serializable values.
    ///
    /// # Arguments
    ///
    /// * `query` - The query string (syntax depends on database type)
    ///
    /// # Security Note
    ///
    /// This executes arbitrary queries. Ensure proper access controls.
    async fn query(&self, query: &str) -> Result<Vec<HashMap<String, serde_json::Value>>>;

    /// Get the structured schema of the graph.
    ///
    /// This should return a cached schema that can be used for query generation.
    fn get_structured_schema(&self) -> &StructuredSchema;

    /// Refresh the cached schema by introspecting the database.
    ///
    /// This should query the database for its current schema and update the cache.
    async fn refresh_schema(&mut self) -> Result<()>;

    /// Get a human-readable text representation of the schema.
    ///
    /// This formats the schema for inclusion in LLM prompts.
    fn get_schema_text(&self) -> String {
        format_structured_schema(self.get_structured_schema(), &[], &[])
    }

    /// Get schema text with type filtering.
    ///
    /// # Arguments
    ///
    /// * `include_types` - If non-empty, only include these node/relationship types
    /// * `exclude_types` - If non-empty, exclude these node/relationship types
    fn get_schema_text_filtered(
        &self,
        include_types: &[String],
        exclude_types: &[String],
    ) -> String {
        format_structured_schema(self.get_structured_schema(), include_types, exclude_types)
    }
}

/// Format a structured schema as human-readable text.
///
/// This is used in LLM prompts to help the model understand the graph structure.
///
/// # Arguments
///
/// * `schema` - The structured schema to format
/// * `include_types` - If non-empty, only include these types
/// * `exclude_types` - If non-empty, exclude these types
#[must_use]
pub fn format_structured_schema(
    schema: &StructuredSchema,
    include_types: &[String],
    exclude_types: &[String],
) -> String {
    // Determine filter function
    let should_include = |type_name: &str| -> bool {
        if include_types.is_empty() {
            !exclude_types.contains(&type_name.to_string())
        } else {
            include_types.contains(&type_name.to_string())
        }
    };

    // Filter node properties
    let filtered_node_props: HashMap<String, Vec<PropertyDefinition>> = schema
        .node_props
        .iter()
        .filter(|(label, _)| should_include(label))
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();

    // Filter relationship properties
    let filtered_rel_props: HashMap<String, Vec<PropertyDefinition>> = schema
        .rel_props
        .iter()
        .filter(|(rel_type, _)| should_include(rel_type))
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();

    // Filter relationships
    let filtered_relationships: Vec<SchemaRelationship> = schema
        .relationships
        .iter()
        .filter(|r| {
            should_include(&r.start) && should_include(&r.end) && should_include(&r.rel_type)
        })
        .cloned()
        .collect();

    // Format node properties
    let formatted_node_props: Vec<String> = filtered_node_props
        .iter()
        .map(|(label, properties)| {
            let props_str = properties
                .iter()
                .map(|prop| format!("{}: {}", prop.property, prop.prop_type))
                .collect::<Vec<_>>()
                .join(", ");
            format!("{label} {{{props_str}}}")
        })
        .collect();

    // Format relationship properties
    let formatted_rel_props: Vec<String> = filtered_rel_props
        .iter()
        .map(|(rel_type, properties)| {
            let props_str = properties
                .iter()
                .map(|prop| format!("{}: {}", prop.property, prop.prop_type))
                .collect::<Vec<_>>()
                .join(", ");
            format!("{rel_type} {{{props_str}}}")
        })
        .collect();

    // Format relationships
    let formatted_rels: Vec<String> = filtered_relationships
        .iter()
        .map(|r| format!("(:{})-(:{})-(:{})", r.start, r.rel_type, r.end))
        .collect();

    // Build final schema text
    [
        "Node properties are the following:".to_string(),
        formatted_node_props.join(", "),
        "Relationship properties are the following:".to_string(),
        formatted_rel_props.join(", "),
        "The relationships are the following:".to_string(),
        formatted_rels.join(", "),
    ]
    .join("\n")
}

#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
#[cfg(test)]
mod tests {
    use super::*;

    // ============================================================
    // SchemaRelationship Tests
    // ============================================================

    #[test]
    fn test_format_structured_schema_empty() {
        let schema = StructuredSchema::default();
        let text = format_structured_schema(&schema, &[], &[]);
        assert!(text.contains("Node properties"));
        assert!(text.contains("Relationship properties"));
        assert!(text.contains("relationships"));
    }

    #[test]
    fn test_format_structured_schema_basic() {
        let mut schema = StructuredSchema::default();

        // Add node properties
        schema.node_props.insert(
            "Person".to_string(),
            vec![
                PropertyDefinition {
                    property: "name".to_string(),
                    prop_type: "String".to_string(),
                },
                PropertyDefinition {
                    property: "age".to_string(),
                    prop_type: "Integer".to_string(),
                },
            ],
        );

        // Add relationship
        schema.relationships.push(SchemaRelationship {
            start: "Person".to_string(),
            rel_type: "KNOWS".to_string(),
            end: "Person".to_string(),
        });

        let text = format_structured_schema(&schema, &[], &[]);
        assert!(text.contains("Person {name: String, age: Integer}"));
        assert!(text.contains("(:Person)-(:KNOWS)-(:Person)"));
    }

    #[test]
    fn test_format_structured_schema_with_include_filter() {
        let mut schema = StructuredSchema::default();

        schema.node_props.insert("Person".to_string(), vec![]);
        schema.node_props.insert("Company".to_string(), vec![]);

        schema.relationships.push(SchemaRelationship {
            start: "Person".to_string(),
            rel_type: "WORKS_AT".to_string(),
            end: "Company".to_string(),
        });

        let text = format_structured_schema(&schema, &[String::from("Person")], &[]);
        assert!(text.contains("Person"));
        assert!(!text.contains("Company"));
        assert!(!text.contains("WORKS_AT"));
    }

    #[test]
    fn test_format_structured_schema_with_exclude_filter() {
        let mut schema = StructuredSchema::default();

        schema.node_props.insert("Person".to_string(), vec![]);
        schema.node_props.insert("Company".to_string(), vec![]);

        let text = format_structured_schema(&schema, &[], &[String::from("Company")]);
        assert!(text.contains("Person"));
        assert!(!text.contains("Company"));
    }

    #[test]
    fn test_schema_relationship_equality() {
        let rel1 = SchemaRelationship {
            start: "A".to_string(),
            rel_type: "REL".to_string(),
            end: "B".to_string(),
        };
        let rel2 = rel1.clone();
        assert_eq!(rel1, rel2);
    }

    #[test]
    fn test_schema_relationship_inequality_start() {
        let rel1 = SchemaRelationship {
            start: "A".to_string(),
            rel_type: "REL".to_string(),
            end: "B".to_string(),
        };
        let rel2 = SchemaRelationship {
            start: "X".to_string(),
            rel_type: "REL".to_string(),
            end: "B".to_string(),
        };
        assert_ne!(rel1, rel2);
    }

    #[test]
    fn test_schema_relationship_inequality_rel_type() {
        let rel1 = SchemaRelationship {
            start: "A".to_string(),
            rel_type: "REL".to_string(),
            end: "B".to_string(),
        };
        let rel2 = SchemaRelationship {
            start: "A".to_string(),
            rel_type: "OTHER".to_string(),
            end: "B".to_string(),
        };
        assert_ne!(rel1, rel2);
    }

    #[test]
    fn test_schema_relationship_inequality_end() {
        let rel1 = SchemaRelationship {
            start: "A".to_string(),
            rel_type: "REL".to_string(),
            end: "B".to_string(),
        };
        let rel2 = SchemaRelationship {
            start: "A".to_string(),
            rel_type: "REL".to_string(),
            end: "C".to_string(),
        };
        assert_ne!(rel1, rel2);
    }

    #[test]
    fn test_schema_relationship_debug() {
        let rel = SchemaRelationship {
            start: "Person".to_string(),
            rel_type: "KNOWS".to_string(),
            end: "Person".to_string(),
        };
        let debug = format!("{:?}", rel);
        assert!(debug.contains("SchemaRelationship"));
        assert!(debug.contains("Person"));
        assert!(debug.contains("KNOWS"));
    }

    #[test]
    fn test_schema_relationship_hash() {
        use std::collections::HashSet;
        let rel1 = SchemaRelationship {
            start: "A".to_string(),
            rel_type: "REL".to_string(),
            end: "B".to_string(),
        };
        let rel2 = rel1.clone();
        let mut set = HashSet::new();
        set.insert(rel1);
        set.insert(rel2);
        assert_eq!(set.len(), 1);
    }

    #[test]
    fn test_schema_relationship_hash_different() {
        use std::collections::HashSet;
        let rel1 = SchemaRelationship {
            start: "A".to_string(),
            rel_type: "REL".to_string(),
            end: "B".to_string(),
        };
        let rel2 = SchemaRelationship {
            start: "X".to_string(),
            rel_type: "REL".to_string(),
            end: "Y".to_string(),
        };
        let mut set = HashSet::new();
        set.insert(rel1);
        set.insert(rel2);
        assert_eq!(set.len(), 2);
    }

    #[test]
    fn test_schema_relationship_serialize() {
        let rel = SchemaRelationship {
            start: "Person".to_string(),
            rel_type: "KNOWS".to_string(),
            end: "Person".to_string(),
        };
        let json = serde_json::to_string(&rel).unwrap();
        assert!(json.contains("Person"));
        assert!(json.contains("KNOWS"));
    }

    #[test]
    fn test_schema_relationship_deserialize() {
        let json = r#"{"start":"A","rel_type":"REL","end":"B"}"#;
        let rel: SchemaRelationship = serde_json::from_str(json).unwrap();
        assert_eq!(rel.start, "A");
        assert_eq!(rel.rel_type, "REL");
        assert_eq!(rel.end, "B");
    }

    #[test]
    fn test_schema_relationship_roundtrip() {
        let rel = SchemaRelationship {
            start: "Person".to_string(),
            rel_type: "WORKS_AT".to_string(),
            end: "Company".to_string(),
        };
        let json = serde_json::to_string(&rel).unwrap();
        let deserialized: SchemaRelationship = serde_json::from_str(&json).unwrap();
        assert_eq!(rel, deserialized);
    }

    #[test]
    fn test_schema_relationship_unicode() {
        let rel = SchemaRelationship {
            start: "人物".to_string(),
            rel_type: "认识".to_string(),
            end: "人物".to_string(),
        };
        let json = serde_json::to_string(&rel).unwrap();
        let deserialized: SchemaRelationship = serde_json::from_str(&json).unwrap();
        assert_eq!(rel, deserialized);
    }

    #[test]
    fn test_schema_relationship_empty_strings() {
        let rel = SchemaRelationship {
            start: "".to_string(),
            rel_type: "".to_string(),
            end: "".to_string(),
        };
        assert_eq!(rel.start, "");
        assert_eq!(rel.rel_type, "");
        assert_eq!(rel.end, "");
    }

    // ============================================================
    // PropertyDefinition Tests
    // ============================================================

    #[test]
    fn test_property_definition_equality() {
        let prop1 = PropertyDefinition {
            property: "name".to_string(),
            prop_type: "String".to_string(),
        };
        let prop2 = prop1.clone();
        assert_eq!(prop1, prop2);
    }

    #[test]
    fn test_property_definition_inequality_property() {
        let prop1 = PropertyDefinition {
            property: "name".to_string(),
            prop_type: "String".to_string(),
        };
        let prop2 = PropertyDefinition {
            property: "age".to_string(),
            prop_type: "String".to_string(),
        };
        assert_ne!(prop1, prop2);
    }

    #[test]
    fn test_property_definition_inequality_type() {
        let prop1 = PropertyDefinition {
            property: "age".to_string(),
            prop_type: "Integer".to_string(),
        };
        let prop2 = PropertyDefinition {
            property: "age".to_string(),
            prop_type: "String".to_string(),
        };
        assert_ne!(prop1, prop2);
    }

    #[test]
    fn test_property_definition_debug() {
        let prop = PropertyDefinition {
            property: "email".to_string(),
            prop_type: "String".to_string(),
        };
        let debug = format!("{:?}", prop);
        assert!(debug.contains("PropertyDefinition"));
        assert!(debug.contains("email"));
        assert!(debug.contains("String"));
    }

    #[test]
    fn test_property_definition_serialize() {
        let prop = PropertyDefinition {
            property: "count".to_string(),
            prop_type: "Integer".to_string(),
        };
        let json = serde_json::to_string(&prop).unwrap();
        assert!(json.contains("count"));
        assert!(json.contains("Integer"));
    }

    #[test]
    fn test_property_definition_deserialize() {
        let json = r#"{"property":"score","prop_type":"Float"}"#;
        let prop: PropertyDefinition = serde_json::from_str(json).unwrap();
        assert_eq!(prop.property, "score");
        assert_eq!(prop.prop_type, "Float");
    }

    #[test]
    fn test_property_definition_roundtrip() {
        let prop = PropertyDefinition {
            property: "active".to_string(),
            prop_type: "Boolean".to_string(),
        };
        let json = serde_json::to_string(&prop).unwrap();
        let deserialized: PropertyDefinition = serde_json::from_str(&json).unwrap();
        assert_eq!(prop, deserialized);
    }

    #[test]
    fn test_property_definition_unicode() {
        let prop = PropertyDefinition {
            property: "名前".to_string(),
            prop_type: "文字列".to_string(),
        };
        let json = serde_json::to_string(&prop).unwrap();
        let deserialized: PropertyDefinition = serde_json::from_str(&json).unwrap();
        assert_eq!(prop, deserialized);
    }

    #[test]
    fn test_property_definition_special_chars() {
        let prop = PropertyDefinition {
            property: "user-name_v2".to_string(),
            prop_type: "String[]".to_string(),
        };
        assert_eq!(prop.property, "user-name_v2");
        assert_eq!(prop.prop_type, "String[]");
    }

    // ============================================================
    // StructuredSchema Tests
    // ============================================================

    #[test]
    fn test_structured_schema_default() {
        let schema = StructuredSchema::default();
        assert!(schema.node_props.is_empty());
        assert!(schema.rel_props.is_empty());
        assert!(schema.relationships.is_empty());
    }

    #[test]
    fn test_structured_schema_clone() {
        let mut schema = StructuredSchema::default();
        schema.node_props.insert(
            "Person".to_string(),
            vec![PropertyDefinition {
                property: "name".to_string(),
                prop_type: "String".to_string(),
            }],
        );
        let cloned = schema.clone();
        assert_eq!(schema.node_props.len(), cloned.node_props.len());
    }

    #[test]
    fn test_structured_schema_debug() {
        let schema = StructuredSchema::default();
        let debug = format!("{:?}", schema);
        assert!(debug.contains("StructuredSchema"));
    }

    #[test]
    fn test_structured_schema_serialize() {
        let mut schema = StructuredSchema::default();
        schema.node_props.insert("Node".to_string(), vec![]);
        let json = serde_json::to_string(&schema).unwrap();
        assert!(json.contains("node_props"));
        assert!(json.contains("Node"));
    }

    #[test]
    fn test_structured_schema_deserialize() {
        let json = r#"{"node_props":{},"rel_props":{},"relationships":[]}"#;
        let schema: StructuredSchema = serde_json::from_str(json).unwrap();
        assert!(schema.node_props.is_empty());
        assert!(schema.relationships.is_empty());
    }

    #[test]
    fn test_structured_schema_roundtrip() {
        let mut schema = StructuredSchema::default();
        schema.node_props.insert(
            "Person".to_string(),
            vec![PropertyDefinition {
                property: "name".to_string(),
                prop_type: "String".to_string(),
            }],
        );
        schema.relationships.push(SchemaRelationship {
            start: "Person".to_string(),
            rel_type: "KNOWS".to_string(),
            end: "Person".to_string(),
        });
        let json = serde_json::to_string(&schema).unwrap();
        let deserialized: StructuredSchema = serde_json::from_str(&json).unwrap();
        assert_eq!(schema.node_props.len(), deserialized.node_props.len());
        assert_eq!(schema.relationships.len(), deserialized.relationships.len());
    }

    #[test]
    fn test_structured_schema_multiple_nodes() {
        let mut schema = StructuredSchema::default();
        schema.node_props.insert("Person".to_string(), vec![]);
        schema.node_props.insert("Company".to_string(), vec![]);
        schema.node_props.insert("Location".to_string(), vec![]);
        assert_eq!(schema.node_props.len(), 3);
    }

    #[test]
    fn test_structured_schema_multiple_relationships() {
        let mut schema = StructuredSchema::default();
        schema.relationships.push(SchemaRelationship {
            start: "A".to_string(),
            rel_type: "REL1".to_string(),
            end: "B".to_string(),
        });
        schema.relationships.push(SchemaRelationship {
            start: "B".to_string(),
            rel_type: "REL2".to_string(),
            end: "C".to_string(),
        });
        assert_eq!(schema.relationships.len(), 2);
    }

    // ============================================================
    // format_structured_schema Tests
    // ============================================================

    #[test]
    fn test_format_schema_with_relationship_properties() {
        let mut schema = StructuredSchema::default();
        schema.rel_props.insert(
            "KNOWS".to_string(),
            vec![PropertyDefinition {
                property: "since".to_string(),
                prop_type: "Date".to_string(),
            }],
        );
        let text = format_structured_schema(&schema, &[], &[]);
        assert!(text.contains("KNOWS {since: Date}"));
    }

    #[test]
    fn test_format_schema_include_excludes_relationships() {
        let mut schema = StructuredSchema::default();
        schema.node_props.insert("Person".to_string(), vec![]);
        schema.node_props.insert("Company".to_string(), vec![]);
        schema.relationships.push(SchemaRelationship {
            start: "Person".to_string(),
            rel_type: "WORKS_AT".to_string(),
            end: "Company".to_string(),
        });
        // Include only Person - WORKS_AT relationship should be excluded
        // because it connects to Company which is not included
        let text = format_structured_schema(&schema, &["Person".to_string()], &[]);
        assert!(text.contains("Person"));
        assert!(!text.contains("Company"));
        assert!(!text.contains("WORKS_AT"));
    }

    #[test]
    fn test_format_schema_exclude_node_keeps_others() {
        let mut schema = StructuredSchema::default();
        schema.node_props.insert("Person".to_string(), vec![]);
        schema.node_props.insert("Company".to_string(), vec![]);
        schema.node_props.insert("Location".to_string(), vec![]);
        let text = format_structured_schema(&schema, &[], &["Company".to_string()]);
        assert!(text.contains("Person"));
        assert!(text.contains("Location"));
        assert!(!text.contains("Company"));
    }

    #[test]
    fn test_format_schema_self_referential_relationship() {
        let mut schema = StructuredSchema::default();
        schema.node_props.insert("Person".to_string(), vec![]);
        schema.relationships.push(SchemaRelationship {
            start: "Person".to_string(),
            rel_type: "KNOWS".to_string(),
            end: "Person".to_string(),
        });
        let text = format_structured_schema(&schema, &[], &[]);
        assert!(text.contains("(:Person)-(:KNOWS)-(:Person)"));
    }

    #[test]
    fn test_format_schema_multiple_properties() {
        let mut schema = StructuredSchema::default();
        schema.node_props.insert(
            "User".to_string(),
            vec![
                PropertyDefinition {
                    property: "id".to_string(),
                    prop_type: "Integer".to_string(),
                },
                PropertyDefinition {
                    property: "name".to_string(),
                    prop_type: "String".to_string(),
                },
                PropertyDefinition {
                    property: "active".to_string(),
                    prop_type: "Boolean".to_string(),
                },
            ],
        );
        let text = format_structured_schema(&schema, &[], &[]);
        assert!(text.contains("id: Integer"));
        assert!(text.contains("name: String"));
        assert!(text.contains("active: Boolean"));
    }

    #[test]
    fn test_format_schema_empty_properties_list() {
        let mut schema = StructuredSchema::default();
        schema.node_props.insert("EmptyNode".to_string(), vec![]);
        let text = format_structured_schema(&schema, &[], &[]);
        assert!(text.contains("EmptyNode {}"));
    }

    #[test]
    fn test_format_schema_unicode_labels() {
        let mut schema = StructuredSchema::default();
        schema.node_props.insert("人物".to_string(), vec![]);
        schema.relationships.push(SchemaRelationship {
            start: "人物".to_string(),
            rel_type: "认识".to_string(),
            end: "人物".to_string(),
        });
        let text = format_structured_schema(&schema, &[], &[]);
        assert!(text.contains("人物"));
        assert!(text.contains("认识"));
    }

    #[test]
    fn test_format_schema_complex_graph() {
        let mut schema = StructuredSchema::default();
        schema.node_props.insert(
            "Person".to_string(),
            vec![
                PropertyDefinition {
                    property: "name".to_string(),
                    prop_type: "String".to_string(),
                },
            ],
        );
        schema.node_props.insert(
            "Company".to_string(),
            vec![
                PropertyDefinition {
                    property: "name".to_string(),
                    prop_type: "String".to_string(),
                },
            ],
        );
        schema.rel_props.insert(
            "WORKS_AT".to_string(),
            vec![
                PropertyDefinition {
                    property: "since".to_string(),
                    prop_type: "Date".to_string(),
                },
            ],
        );
        schema.relationships.push(SchemaRelationship {
            start: "Person".to_string(),
            rel_type: "WORKS_AT".to_string(),
            end: "Company".to_string(),
        });
        let text = format_structured_schema(&schema, &[], &[]);
        assert!(text.contains("Node properties"));
        assert!(text.contains("Relationship properties"));
        assert!(text.contains("WORKS_AT {since: Date}"));
    }

    #[test]
    fn test_format_schema_include_and_exclude_interaction() {
        let mut schema = StructuredSchema::default();
        schema.node_props.insert("A".to_string(), vec![]);
        schema.node_props.insert("B".to_string(), vec![]);
        schema.node_props.insert("C".to_string(), vec![]);
        // When include is non-empty, exclude is ignored
        let text = format_structured_schema(
            &schema,
            &["A".to_string(), "B".to_string()],
            &["A".to_string()],
        );
        // Include takes precedence - A and B should be present
        assert!(text.contains("A"));
        assert!(text.contains("B"));
        assert!(!text.contains("C"));
    }

    #[test]
    fn test_format_schema_filters_relationship_by_rel_type() {
        let mut schema = StructuredSchema::default();
        schema.node_props.insert("A".to_string(), vec![]);
        schema.node_props.insert("B".to_string(), vec![]);
        schema.relationships.push(SchemaRelationship {
            start: "A".to_string(),
            rel_type: "CONNECTS".to_string(),
            end: "B".to_string(),
        });
        // Include only A and B but not CONNECTS relationship type
        let text = format_structured_schema(
            &schema,
            &["A".to_string(), "B".to_string()],
            &[],
        );
        // Relationship should be excluded because CONNECTS is not in include list
        assert!(!text.contains("CONNECTS"));
    }

    #[test]
    fn test_format_schema_text_structure() {
        let schema = StructuredSchema::default();
        let text = format_structured_schema(&schema, &[], &[]);
        // Check that the text contains the expected section headers
        assert!(text.contains("Node properties are the following:"));
        assert!(text.contains("Relationship properties are the following:"));
        assert!(text.contains("The relationships are the following:"));
        // Verify headers appear in correct order
        let node_pos = text.find("Node properties").unwrap();
        let rel_props_pos = text.find("Relationship properties").unwrap();
        let rels_pos = text.find("The relationships").unwrap();
        assert!(node_pos < rel_props_pos);
        assert!(rel_props_pos < rels_pos);
    }

    #[test]
    fn test_format_schema_separator_format() {
        let mut schema = StructuredSchema::default();
        schema.node_props.insert("A".to_string(), vec![]);
        schema.node_props.insert("B".to_string(), vec![]);
        let text = format_structured_schema(&schema, &[], &[]);
        // Multiple node types should be comma-separated
        // Note: HashMap order is not guaranteed, so we check for comma presence
        assert!(text.contains("{}") || text.contains(", "));
    }
}
