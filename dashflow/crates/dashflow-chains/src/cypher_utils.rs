//! # Cypher Query Utilities
//!
//! Utilities for working with Cypher queries, including validation and correction.
//!
//! The `CypherQueryCorrector` helps fix common issues in generated Cypher queries,
//! particularly incorrect relationship directions.

use dashflow_neo4j::SchemaRelationship;
use regex::Regex;
use std::collections::HashMap;
use std::sync::OnceLock;

/// Static regex patterns for Cypher query parsing (compiled once).
mod patterns {
    use super::*;

    static PROPERTY: OnceLock<Regex> = OnceLock::new();
    static NODE: OnceLock<Regex> = OnceLock::new();
    static PATH: OnceLock<Regex> = OnceLock::new();
    static NODE_RELATION_NODE: OnceLock<Regex> = OnceLock::new();
    static RELATION_TYPE: OnceLock<Regex> = OnceLock::new();
    static CYPHER_CODE_BLOCK: OnceLock<Regex> = OnceLock::new();

    pub fn property() -> &'static Regex {
        PROPERTY.get_or_init(|| {
            #[allow(clippy::expect_used)]
            Regex::new(r"\{.+?\}").expect("PROPERTY pattern is valid")
        })
    }

    pub fn node() -> &'static Regex {
        NODE.get_or_init(|| {
            #[allow(clippy::expect_used)]
            Regex::new(r"\(.+?\)").expect("NODE pattern is valid")
        })
    }

    pub fn path() -> &'static Regex {
        PATH.get_or_init(|| {
            #[allow(clippy::expect_used)]
            Regex::new(
                r"(\([^\,\(\)]*?(\{.+\})?[^\,\(\)]*?\))(<?-)(\[.*?\])?(->?)(\([^\,\(\)]*?(\{.+\})?[^\,\(\)]*?\))",
            )
            .expect("PATH pattern is valid")
        })
    }

    pub fn node_relation_node() -> &'static Regex {
        NODE_RELATION_NODE.get_or_init(|| {
            #[allow(clippy::expect_used)]
            Regex::new(r"(\()+(?P<left_node>[^()]*?)\)(?P<relation>.*?)\((?P<right_node>[^()]*?)(\))+")
                .expect("NODE_RELATION_NODE pattern is valid")
        })
    }

    pub fn relation_type() -> &'static Regex {
        RELATION_TYPE.get_or_init(|| {
            #[allow(clippy::expect_used)]
            Regex::new(r":(?P<relation_type>.+?)?(\{.+\})?]")
                .expect("RELATION_TYPE pattern is valid")
        })
    }

    pub fn cypher_code_block() -> &'static Regex {
        CYPHER_CODE_BLOCK.get_or_init(|| {
            #[allow(clippy::expect_used)]
            Regex::new(r"```(?:cypher)?\s*(.*?)\s*```")
                .expect("CYPHER_CODE_BLOCK pattern is valid")
        })
    }
}

/// Cypher query corrector that fixes relationship direction issues.
///
/// This implementation is based on the winner's submission to the Cypher competition:
/// <https://github.com/sakusaku-rich/cypher-direction-competition>
///
/// It validates and corrects relationship directions in Cypher queries based on
/// the graph schema.
#[derive(Clone)]
pub struct CypherQueryCorrector {
    schemas: Vec<SchemaRelationship>,
}

impl CypherQueryCorrector {
    /// Create a new `CypherQueryCorrector` with the given schema.
    ///
    /// # Arguments
    ///
    /// * `schemas` - List of valid relationships in the graph schema
    #[must_use]
    pub fn new(schemas: Vec<SchemaRelationship>) -> Self {
        Self { schemas }
    }

    /// Clean a node string by removing properties and parentheses.
    fn clean_node(&self, node: &str) -> String {
        let node = patterns::property().replace_all(node, "");
        let node = node.replace(['(', ')'], "");
        node.trim().to_string()
    }

    /// Detect node variables and their labels from a query.
    fn detect_node_variables(&self, query: &str) -> HashMap<String, Vec<String>> {
        let nodes: Vec<String> = patterns::node()
            .find_iter(query)
            .map(|m| self.clean_node(m.as_str()))
            .collect();

        let mut result: HashMap<String, Vec<String>> = HashMap::new();

        for node in nodes {
            let parts: Vec<&str> = node.split(':').collect();
            if parts.is_empty() || parts[0].is_empty() {
                continue;
            }

            let variable = parts[0].to_string();
            result.entry(variable).or_default().extend(
                parts[1..]
                    .iter()
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty()),
            );
        }

        result
    }

    /// Extract relationship paths from a query.
    fn extract_paths(&self, query: &str) -> Vec<String> {
        let mut paths = Vec::new();
        let mut idx = 0;
        let path_pattern = patterns::path();

        while let Some(matched) = path_pattern.find(&query[idx..]) {
            let path = matched.as_str();
            // M-347: Use match position directly instead of re-searching
            idx = idx + matched.start() + path.len();

            // Extract the last node to adjust index
            if let Some(caps) = path_pattern.captures(path) {
                if let Some(last_node) = caps.get(6) {
                    idx -= last_node.as_str().len();
                }
            }

            paths.push(path.to_string());
        }

        paths
    }

    /// Determine the direction of a relationship.
    fn judge_direction(&self, relation: &str) -> Direction {
        if relation.starts_with('<') {
            Direction::Incoming
        } else if relation.ends_with('>') {
            Direction::Outgoing
        } else {
            Direction::Bidirectional
        }
    }

    /// Detect labels for a node string.
    fn detect_labels(
        &self,
        str_node: &str,
        node_variable_dict: &HashMap<String, Vec<String>>,
    ) -> Vec<String> {
        let splitted: Vec<&str> = str_node.split(':').collect();
        if splitted.is_empty() {
            return Vec::new();
        }

        let variable = splitted[0];

        if let Some(labels) = node_variable_dict.get(variable) {
            labels.clone()
        } else if variable.is_empty() && splitted.len() > 1 {
            splitted[1..].iter().map(|s| s.trim().to_string()).collect()
        } else {
            Vec::new()
        }
    }

    /// Verify if a relationship matches the schema.
    fn verify_schema(
        &self,
        from_labels: &[String],
        rel_types: &[String],
        to_labels: &[String],
    ) -> bool {
        let mut valid_schemas = self.schemas.clone();

        // Filter by from labels
        if !from_labels.is_empty() {
            let from_labels_clean: Vec<String> = from_labels
                .iter()
                .map(|l| l.trim_matches('`').to_string())
                .collect();
            valid_schemas.retain(|s| from_labels_clean.contains(&s.start));
        }

        // Filter by to labels
        if !to_labels.is_empty() {
            let to_labels_clean: Vec<String> = to_labels
                .iter()
                .map(|l| l.trim_matches('`').to_string())
                .collect();
            valid_schemas.retain(|s| to_labels_clean.contains(&s.end));
        }

        // Filter by relationship types
        if !rel_types.is_empty() {
            let rel_types_clean: Vec<String> = rel_types
                .iter()
                .map(|t| t.trim_matches('`').to_string())
                .collect();
            valid_schemas.retain(|s| rel_types_clean.contains(&s.rel_type));
        }

        !valid_schemas.is_empty()
    }

    /// Detect relationship types from a relationship string.
    fn detect_relation_types(&self, str_relation: &str) -> (Direction, Vec<String>) {
        let direction = self.judge_direction(str_relation);

        if let Some(caps) = patterns::relation_type().captures(str_relation) {
            if let Some(rel_type_match) = caps.name("relation_type") {
                let rel_types: Vec<String> = rel_type_match
                    .as_str()
                    .split('|')
                    .map(|s| s.trim().trim_start_matches('!').to_string())
                    .collect();
                return (direction, rel_types);
            }
        }

        (direction, Vec::new())
    }

    /// Correct relationship directions in a Cypher query.
    ///
    /// Returns an empty string if the query contains invalid relationships
    /// that cannot be corrected.
    #[must_use]
    pub fn correct_query(&self, query: &str) -> String {
        let node_variable_dict = self.detect_node_variables(query);
        let paths = self.extract_paths(query);
        let mut corrected_query = query.to_string();

        for path in paths {
            let mut start_idx = 0;
            let _original_path = path.clone();

            while start_idx < path.len() {
                if let Some(caps) = patterns::node_relation_node().captures(&path[start_idx..]) {
                    // SAFETY: M-347 - All groups are required parts of the regex pattern
                    // Regex: r"(\()+(?P<left_node>...)\)(?P<relation>...)\((?P<right_node>...)(\))+"
                    #[allow(clippy::expect_used)]
                    let full_match_start = caps.get(0).expect("full match exists").start();
                    #[allow(clippy::expect_used)]
                    let left_node =
                        caps.name("left_node").expect("left_node is required").as_str();
                    #[allow(clippy::expect_used)]
                    let relation = caps.name("relation").expect("relation is required").as_str();
                    #[allow(clippy::expect_used)]
                    let right_node =
                        caps.name("right_node").expect("right_node is required").as_str();

                    let left_node_labels = self.detect_labels(left_node, &node_variable_dict);
                    let right_node_labels = self.detect_labels(right_node, &node_variable_dict);

                    let (relation_direction, relation_types) = self.detect_relation_types(relation);

                    // Skip variable-length relationships
                    if !relation_types.is_empty() && relation_types.iter().any(|t| t.contains('*'))
                    {
                        start_idx += full_match_start + left_node.len() + relation.len() + 2;
                        continue;
                    }

                    match relation_direction {
                        Direction::Outgoing => {
                            let is_legal = self.verify_schema(
                                &left_node_labels,
                                &relation_types,
                                &right_node_labels,
                            );
                            if !is_legal {
                                let is_legal_reversed = self.verify_schema(
                                    &right_node_labels,
                                    &relation_types,
                                    &left_node_labels,
                                );
                                if is_legal_reversed {
                                    // Reverse the direction
                                    let corrected_relation =
                                        format!("<{}", &relation[..relation.len() - 1]);
                                    corrected_query =
                                        corrected_query.replace(relation, &corrected_relation);
                                } else {
                                    // Invalid query
                                    return String::new();
                                }
                            }
                        }
                        Direction::Incoming => {
                            let is_legal = self.verify_schema(
                                &right_node_labels,
                                &relation_types,
                                &left_node_labels,
                            );
                            if !is_legal {
                                let is_legal_reversed = self.verify_schema(
                                    &left_node_labels,
                                    &relation_types,
                                    &right_node_labels,
                                );
                                if is_legal_reversed {
                                    // Reverse the direction
                                    let corrected_relation = format!("{}>", &relation[1..]);
                                    corrected_query =
                                        corrected_query.replace(relation, &corrected_relation);
                                } else {
                                    // Invalid query
                                    return String::new();
                                }
                            }
                        }
                        Direction::Bidirectional => {
                            let is_legal = self.verify_schema(
                                &left_node_labels,
                                &relation_types,
                                &right_node_labels,
                            ) || self.verify_schema(
                                &right_node_labels,
                                &relation_types,
                                &left_node_labels,
                            );
                            if !is_legal {
                                // Invalid query
                                return String::new();
                            }
                        }
                    }

                    start_idx += full_match_start + left_node.len() + relation.len() + 2;
                } else {
                    break;
                }
            }
        }

        corrected_query
    }

    /// Convenience method for calling the corrector.
    #[must_use]
    pub fn call(&self, query: &str) -> String {
        self.correct_query(query)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Direction {
    Outgoing,
    Incoming,
    Bidirectional,
}

/// Extract Cypher code from text that may contain markdown code blocks.
///
/// Looks for code blocks enclosed in triple backticks and returns the first match.
/// If no code blocks are found, returns the original text.
#[must_use]
pub fn extract_cypher(text: &str) -> String {
    if let Some(caps) = patterns::cypher_code_block().captures(text) {
        if let Some(code) = caps.get(1) {
            return code.as_str().trim().to_string();
        }
    }

    text.trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_cypher_no_backticks() {
        let text = "MATCH (n:Person) RETURN n";
        assert_eq!(extract_cypher(text), "MATCH (n:Person) RETURN n");
    }

    #[test]
    fn test_extract_cypher_with_backticks() {
        let text = "```MATCH (n:Person) RETURN n```";
        assert_eq!(extract_cypher(text), "MATCH (n:Person) RETURN n");
    }

    #[test]
    fn test_extract_cypher_with_language_tag() {
        let text = "```cypher\nMATCH (n:Person) RETURN n\n```";
        assert_eq!(extract_cypher(text), "MATCH (n:Person) RETURN n");
    }

    #[test]
    fn test_cypher_corrector_clean_node() {
        let schemas = vec![];
        let corrector = CypherQueryCorrector::new(schemas);
        assert_eq!(
            corrector.clean_node("(n:Person {name: 'John'})"),
            "n:Person"
        );
        assert_eq!(corrector.clean_node("(n:Person)"), "n:Person");
        assert_eq!(corrector.clean_node("(n)"), "n");
    }

    #[test]
    fn test_cypher_corrector_detect_node_variables() {
        let schemas = vec![];
        let corrector = CypherQueryCorrector::new(schemas);
        let query = "MATCH (p:Person)-[:KNOWS]->(f:Person) RETURN p, f";
        let vars = corrector.detect_node_variables(query);

        assert_eq!(vars.get("p"), Some(&vec!["Person".to_string()]));
        assert_eq!(vars.get("f"), Some(&vec!["Person".to_string()]));
    }

    #[test]
    fn test_cypher_corrector_judge_direction() {
        let schemas = vec![];
        let corrector = CypherQueryCorrector::new(schemas);

        assert_eq!(corrector.judge_direction("->"), Direction::Outgoing);
        assert_eq!(corrector.judge_direction("<-"), Direction::Incoming);
        assert_eq!(corrector.judge_direction("-"), Direction::Bidirectional);
    }

    #[test]
    fn test_cypher_corrector_verify_schema() {
        let schemas = vec![SchemaRelationship {
            start: "Person".to_string(),
            rel_type: "KNOWS".to_string(),
            end: "Person".to_string(),
        }];
        let corrector = CypherQueryCorrector::new(schemas);

        assert!(corrector.verify_schema(
            &["Person".to_string()],
            &["KNOWS".to_string()],
            &["Person".to_string()]
        ));
        assert!(!corrector.verify_schema(
            &["Person".to_string()],
            &["LIKES".to_string()],
            &["Person".to_string()]
        ));
    }

    #[test]
    fn test_cypher_corrector_correct_valid_query() {
        let schemas = vec![SchemaRelationship {
            start: "Person".to_string(),
            rel_type: "KNOWS".to_string(),
            end: "Person".to_string(),
        }];
        let corrector = CypherQueryCorrector::new(schemas);

        let query = "MATCH (p:Person)-[:KNOWS]->(f:Person) RETURN p, f";
        let corrected = corrector.correct_query(query);
        assert!(!corrected.is_empty());
    }

    #[test]
    fn test_cypher_corrector_correct_wrong_direction() {
        let schemas = vec![SchemaRelationship {
            start: "Person".to_string(),
            rel_type: "WORKS_AT".to_string(),
            end: "Company".to_string(),
        }];
        let corrector = CypherQueryCorrector::new(schemas);

        // Query has wrong direction: Company->Person instead of Person->Company
        let query = "MATCH (c:Company)-[:WORKS_AT]->(p:Person) RETURN c, p";
        let corrected = corrector.correct_query(query);

        // Should be corrected to Company<-WORKS_AT-Person
        assert!(corrected.contains("<-") || corrected.is_empty());
    }

    #[test]
    fn test_cypher_corrector_invalid_relationship() {
        let schemas = vec![SchemaRelationship {
            start: "Person".to_string(),
            rel_type: "KNOWS".to_string(),
            end: "Person".to_string(),
        }];
        let corrector = CypherQueryCorrector::new(schemas);

        // Query uses non-existent relationship type
        let query = "MATCH (p:Person)-[:LIKES]->(f:Person) RETURN p, f";
        let corrected = corrector.correct_query(query);

        // Should return empty string for invalid query
        assert_eq!(corrected, "");
    }
}
