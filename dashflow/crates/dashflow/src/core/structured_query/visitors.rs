//! Example visitor implementations for translating structured queries to backend-specific formats.
//!
//! This module provides reference implementations of the Visitor trait for
//! popular vector store backends.

use super::{Comparator, Comparison, Expr, Operation, Operator, StructuredQuery, Visitor};
use serde_json::{json, Value};

/// Error type for visitor implementations.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum VisitorError {
    /// A comparator was used that is not allowed by the visitor configuration.
    #[error("Disallowed comparator {comparator}: allowed are {allowed:?}")]
    DisallowedComparator {
        /// The disallowed comparator that was used.
        comparator: Comparator,
        /// The list of comparators that are allowed.
        allowed: Vec<Comparator>,
    },
    /// An operator was used that is not allowed by the visitor configuration.
    #[error("Disallowed operator {operator}: allowed are {allowed:?}")]
    DisallowedOperator {
        /// The disallowed operator that was used.
        operator: Operator,
        /// The list of operators that are allowed.
        allowed: Vec<Operator>,
    },
}

/// Visitor for translating to Pinecone filter format.
///
/// Pinecone uses MongoDB-style query syntax with $ prefixes for operators.
///
/// # Example
/// ```rust
/// use dashflow::core::structured_query::{
///     Comparator, Comparison, StructuredQuery,
///     visitors::PineconeTranslator,
///     Visitor, Expr,
/// };
///
/// let mut translator = PineconeTranslator::new();
/// let comparison = Comparison::new(Comparator::Eq, "category".to_string(), "books");
/// let query = StructuredQuery::new("search term".to_string(), Some(comparison.into()), None);
///
/// let (query_str, kwargs) = translator.visit_structured_query(&query).unwrap();
/// // query_str is "search term"
/// // kwargs contains {"filter": {"category": {"$eq": "books"}}}
/// ```
#[derive(Debug, Clone)]
pub struct PineconeTranslator {
    allowed_comparators: Vec<Comparator>,
    allowed_operators: Vec<Operator>,
}

impl PineconeTranslator {
    /// Create a new `PineconeTranslator` with default allowed operations.
    #[must_use]
    pub fn new() -> Self {
        Self {
            allowed_comparators: vec![
                Comparator::Eq,
                Comparator::Ne,
                Comparator::Lt,
                Comparator::Lte,
                Comparator::Gt,
                Comparator::Gte,
                Comparator::In,
                Comparator::Nin,
            ],
            allowed_operators: vec![Operator::And, Operator::Or],
        }
    }

    fn format_func(&self, func: &Comparator) -> String {
        format!("${}", func.value())
    }

    fn format_operator(&self, op: &Operator) -> String {
        format!("${}", op.value())
    }
}

impl Default for PineconeTranslator {
    fn default() -> Self {
        Self::new()
    }
}

impl Visitor for PineconeTranslator {
    type Output = Value;
    type Error = VisitorError;

    fn allowed_comparators(&self) -> Option<&[Comparator]> {
        Some(&self.allowed_comparators)
    }

    fn allowed_operators(&self) -> Option<&[Operator]> {
        Some(&self.allowed_operators)
    }

    fn error_disallowed_comparator(
        &self,
        comparator: Comparator,
        allowed: &[Comparator],
    ) -> Self::Error {
        VisitorError::DisallowedComparator {
            comparator,
            allowed: allowed.to_vec(),
        }
    }

    fn error_disallowed_operator(&self, operator: Operator, allowed: &[Operator]) -> Self::Error {
        VisitorError::DisallowedOperator {
            operator,
            allowed: allowed.to_vec(),
        }
    }

    fn visit_operation(&mut self, operation: &Operation) -> Result<Self::Output, Self::Error> {
        self.validate_operator(operation.operator)?;

        let args: Result<Vec<Value>, _> = operation
            .arguments
            .iter()
            .map(|arg| arg.accept(self))
            .collect();

        Ok(json!({
            self.format_operator(&operation.operator): args?
        }))
    }

    fn visit_comparison(&mut self, comparison: &Comparison) -> Result<Self::Output, Self::Error> {
        self.validate_comparator(comparison.comparator)?;

        // For IN/NIN, ensure value is an array
        let value = if matches!(comparison.comparator, Comparator::In | Comparator::Nin) {
            if comparison.value.is_array() {
                comparison.value.clone()
            } else {
                Value::Array(vec![comparison.value.clone()])
            }
        } else {
            comparison.value.clone()
        };

        Ok(json!({
            comparison.attribute.clone(): {
                self.format_func(&comparison.comparator): value
            }
        }))
    }

    fn visit_structured_query(
        &mut self,
        structured_query: &StructuredQuery,
    ) -> Result<(String, std::collections::HashMap<String, Value>), Self::Error> {
        let mut kwargs = std::collections::HashMap::new();

        if let Some(filter) = &structured_query.filter {
            let filter_value = filter.accept(self)?;
            kwargs.insert("filter".to_string(), filter_value);
        }

        Ok((structured_query.query.clone(), kwargs))
    }
}

/// Visitor for translating to Qdrant filter format.
///
/// Qdrant uses a structured filter format with `must/should/must_not` clauses.
///
/// # Example
/// ```rust
/// use dashflow::core::structured_query::{
///     Comparator, Comparison, StructuredQuery,
///     visitors::QdrantTranslator,
///     Visitor, Expr,
/// };
///
/// let mut translator = QdrantTranslator::new("metadata".to_string());
/// let comparison = Comparison::new(Comparator::Eq, "category".to_string(), "books");
/// let query = StructuredQuery::new("search term".to_string(), Some(comparison.into()), None);
///
/// let (query_str, kwargs) = translator.visit_structured_query(&query).unwrap();
/// // query_str is "search term"
/// // kwargs contains {"filter": {...}}
/// ```
#[derive(Debug, Clone)]
pub struct QdrantTranslator {
    metadata_key: String,
    allowed_comparators: Vec<Comparator>,
    allowed_operators: Vec<Operator>,
}

impl QdrantTranslator {
    /// Create a new `QdrantTranslator` with the specified metadata key.
    #[must_use]
    pub fn new(metadata_key: String) -> Self {
        Self {
            metadata_key,
            allowed_comparators: vec![
                Comparator::Eq,
                Comparator::Lt,
                Comparator::Lte,
                Comparator::Gt,
                Comparator::Gte,
                Comparator::Like,
            ],
            allowed_operators: vec![Operator::And, Operator::Or, Operator::Not],
        }
    }
}

impl Visitor for QdrantTranslator {
    type Output = Value;
    type Error = VisitorError;

    fn allowed_comparators(&self) -> Option<&[Comparator]> {
        Some(&self.allowed_comparators)
    }

    fn allowed_operators(&self) -> Option<&[Operator]> {
        Some(&self.allowed_operators)
    }

    fn error_disallowed_comparator(
        &self,
        comparator: Comparator,
        allowed: &[Comparator],
    ) -> Self::Error {
        VisitorError::DisallowedComparator {
            comparator,
            allowed: allowed.to_vec(),
        }
    }

    fn error_disallowed_operator(&self, operator: Operator, allowed: &[Operator]) -> Self::Error {
        VisitorError::DisallowedOperator {
            operator,
            allowed: allowed.to_vec(),
        }
    }

    fn visit_operation(&mut self, operation: &Operation) -> Result<Self::Output, Self::Error> {
        self.validate_operator(operation.operator)?;

        let args: Result<Vec<Value>, _> = operation
            .arguments
            .iter()
            .map(|arg| arg.accept(self))
            .collect();
        let args = args?;

        let operator_key = match operation.operator {
            Operator::And => "must",
            Operator::Or => "should",
            Operator::Not => "must_not",
        };

        Ok(json!({ operator_key: args }))
    }

    fn visit_comparison(&mut self, comparison: &Comparison) -> Result<Self::Output, Self::Error> {
        self.validate_comparator(comparison.comparator)?;

        let key = format!("{}.{}", self.metadata_key, comparison.attribute);

        Ok(match comparison.comparator {
            Comparator::Eq => {
                json!({
                    "key": key,
                    "match": {
                        "value": comparison.value.clone()
                    }
                })
            }
            Comparator::Like => {
                json!({
                    "key": key,
                    "match": {
                        "text": comparison.value.clone()
                    }
                })
            }
            _ => {
                // Range comparison (gt, gte, lt, lte)
                json!({
                    "key": key,
                    "range": {
                        comparison.comparator.value(): comparison.value.clone()
                    }
                })
            }
        })
    }

    fn visit_structured_query(
        &mut self,
        structured_query: &StructuredQuery,
    ) -> Result<(String, std::collections::HashMap<String, Value>), Self::Error> {
        let mut kwargs = std::collections::HashMap::new();

        if let Some(filter) = &structured_query.filter {
            let mut filter_value = filter.accept(self)?;

            // Wrap single field conditions in a must clause
            if filter_value.get("key").is_some() {
                filter_value = json!({ "must": [filter_value] });
            }

            kwargs.insert("filter".to_string(), filter_value);
        }

        Ok((structured_query.query.clone(), kwargs))
    }
}

#[cfg(test)]
mod tests {
    use crate::core::structured_query::{Expr, Visitor};
    use crate::test_prelude::*;

    #[test]
    fn test_pinecone_simple_comparison() {
        let mut translator = PineconeTranslator::new();
        let comp = Comparison::new(Comparator::Eq, "category".to_string(), "books");
        let result = comp.accept(&mut translator).unwrap();

        assert_eq!(
            result,
            json!({
                "category": {
                    "$eq": "books"
                }
            })
        );
    }

    #[test]
    fn test_pinecone_gt_comparison() {
        let mut translator = PineconeTranslator::new();
        let comp = Comparison::new(Comparator::Gt, "price".to_string(), 100);
        let result = comp.accept(&mut translator).unwrap();

        assert_eq!(
            result,
            json!({
                "price": {
                    "$gt": 100
                }
            })
        );
    }

    #[test]
    fn test_pinecone_in_comparison() {
        let mut translator = PineconeTranslator::new();
        let comp = Comparison::new(
            Comparator::In,
            "category".to_string(),
            json!(["books", "electronics"]),
        );
        let result = comp.accept(&mut translator).unwrap();

        assert_eq!(
            result,
            json!({
                "category": {
                    "$in": ["books", "electronics"]
                }
            })
        );
    }

    #[test]
    fn test_pinecone_and_operation() {
        let mut translator = PineconeTranslator::new();
        let comp1 = Comparison::new(Comparator::Gt, "age".to_string(), 18);
        let comp2 = Comparison::new(Comparator::Lt, "age".to_string(), 65);
        let op = Operation::new(Operator::And, vec![comp1.into(), comp2.into()]);
        let result = op.accept(&mut translator).unwrap();

        assert_eq!(
            result,
            json!({
                "$and": [
                    {"age": {"$gt": 18}},
                    {"age": {"$lt": 65}}
                ]
            })
        );
    }

    #[test]
    fn test_pinecone_structured_query() {
        let mut translator = PineconeTranslator::new();
        let comp = Comparison::new(Comparator::Eq, "category".to_string(), "books");
        let query = StructuredQuery::new("search books".to_string(), Some(comp.into()), Some(10));
        let (query_str, kwargs) = translator.visit_structured_query(&query).unwrap();

        assert_eq!(query_str, "search books");
        assert_eq!(
            kwargs.get("filter"),
            Some(&json!({
                "category": {"$eq": "books"}
            }))
        );
    }

    #[test]
    fn test_qdrant_simple_comparison() {
        let mut translator = QdrantTranslator::new("metadata".to_string());
        let comp = Comparison::new(Comparator::Eq, "category".to_string(), "books");
        let result = comp.accept(&mut translator).unwrap();

        assert_eq!(
            result,
            json!({
                "key": "metadata.category",
                "match": {
                    "value": "books"
                }
            })
        );
    }

    #[test]
    fn test_qdrant_range_comparison() {
        let mut translator = QdrantTranslator::new("metadata".to_string());
        let comp = Comparison::new(Comparator::Gt, "price".to_string(), 100);
        let result = comp.accept(&mut translator).unwrap();

        assert_eq!(
            result,
            json!({
                "key": "metadata.price",
                "range": {
                    "gt": 100
                }
            })
        );
    }

    #[test]
    fn test_qdrant_like_comparison() {
        let mut translator = QdrantTranslator::new("metadata".to_string());
        let comp = Comparison::new(
            Comparator::Like,
            "description".to_string(),
            "rust programming",
        );
        let result = comp.accept(&mut translator).unwrap();

        assert_eq!(
            result,
            json!({
                "key": "metadata.description",
                "match": {
                    "text": "rust programming"
                }
            })
        );
    }

    #[test]
    fn test_qdrant_and_operation() {
        let mut translator = QdrantTranslator::new("metadata".to_string());
        let comp1 = Comparison::new(Comparator::Gt, "age".to_string(), 18);
        let comp2 = Comparison::new(Comparator::Lt, "age".to_string(), 65);
        let op = Operation::new(Operator::And, vec![comp1.into(), comp2.into()]);
        let result = op.accept(&mut translator).unwrap();

        assert_eq!(
            result,
            json!({
                "must": [
                    {"key": "metadata.age", "range": {"gt": 18}},
                    {"key": "metadata.age", "range": {"lt": 65}}
                ]
            })
        );
    }

    #[test]
    fn test_qdrant_not_operation() {
        let mut translator = QdrantTranslator::new("metadata".to_string());
        let comp = Comparison::new(Comparator::Eq, "deleted".to_string(), true);
        let op = Operation::new(Operator::Not, vec![comp.into()]);
        let result = op.accept(&mut translator).unwrap();

        assert_eq!(
            result,
            json!({
                "must_not": [
                    {"key": "metadata.deleted", "match": {"value": true}}
                ]
            })
        );
    }

    #[test]
    fn test_qdrant_structured_query() {
        let mut translator = QdrantTranslator::new("metadata".to_string());
        let comp = Comparison::new(Comparator::Eq, "category".to_string(), "books");
        let query = StructuredQuery::new("search books".to_string(), Some(comp.into()), Some(5));
        let (query_str, kwargs) = translator.visit_structured_query(&query).unwrap();

        assert_eq!(query_str, "search books");
        assert_eq!(
            kwargs.get("filter"),
            Some(&json!({
                "must": [
                    {"key": "metadata.category", "match": {"value": "books"}}
                ]
            }))
        );
    }

    #[test]
    fn test_pinecone_disallowed_comparator() {
        let mut translator = PineconeTranslator::new();
        let comp = Comparison::new(Comparator::Like, "text".to_string(), "pattern");
        let result = comp.accept(&mut translator);
        assert!(result.is_err());
    }

    #[test]
    fn test_qdrant_disallowed_comparator() {
        let mut translator = QdrantTranslator::new("metadata".to_string());
        let comp = Comparison::new(Comparator::In, "category".to_string(), json!(["a", "b"]));
        let result = comp.accept(&mut translator);
        assert!(result.is_err());
    }
}

/// Visitor for translating to Chroma filter format.
///
/// Chroma uses MongoDB-style query syntax similar to Pinecone.
#[derive(Debug, Clone)]
pub struct ChromaTranslator {
    allowed_comparators: Vec<Comparator>,
    allowed_operators: Vec<Operator>,
}

impl ChromaTranslator {
    /// Create a new `ChromaTranslator` with default allowed operations.
    #[must_use]
    pub fn new() -> Self {
        Self {
            allowed_comparators: vec![
                Comparator::Eq,
                Comparator::Ne,
                Comparator::Lt,
                Comparator::Lte,
                Comparator::Gt,
                Comparator::Gte,
            ],
            allowed_operators: vec![Operator::And, Operator::Or],
        }
    }

    fn format_func(&self, func: &Comparator) -> String {
        format!("${}", func.value())
    }

    fn format_operator(&self, op: &Operator) -> String {
        format!("${}", op.value())
    }
}

impl Default for ChromaTranslator {
    fn default() -> Self {
        Self::new()
    }
}

impl Visitor for ChromaTranslator {
    type Output = Value;
    type Error = VisitorError;

    fn allowed_comparators(&self) -> Option<&[Comparator]> {
        Some(&self.allowed_comparators)
    }

    fn allowed_operators(&self) -> Option<&[Operator]> {
        Some(&self.allowed_operators)
    }

    fn error_disallowed_comparator(
        &self,
        comparator: Comparator,
        allowed: &[Comparator],
    ) -> Self::Error {
        VisitorError::DisallowedComparator {
            comparator,
            allowed: allowed.to_vec(),
        }
    }

    fn error_disallowed_operator(&self, operator: Operator, allowed: &[Operator]) -> Self::Error {
        VisitorError::DisallowedOperator {
            operator,
            allowed: allowed.to_vec(),
        }
    }

    fn visit_operation(&mut self, operation: &Operation) -> Result<Self::Output, Self::Error> {
        self.validate_operator(operation.operator)?;

        let args: Result<Vec<Value>, _> = operation
            .arguments
            .iter()
            .map(|arg| arg.accept(self))
            .collect();

        Ok(json!({
            self.format_operator(&operation.operator): args?
        }))
    }

    fn visit_comparison(&mut self, comparison: &Comparison) -> Result<Self::Output, Self::Error> {
        self.validate_comparator(comparison.comparator)?;

        Ok(json!({
            comparison.attribute.clone(): {
                self.format_func(&comparison.comparator): comparison.value.clone()
            }
        }))
    }

    fn visit_structured_query(
        &mut self,
        structured_query: &StructuredQuery,
    ) -> Result<(String, std::collections::HashMap<String, Value>), Self::Error> {
        let mut kwargs = std::collections::HashMap::new();

        if let Some(filter) = &structured_query.filter {
            let filter_value = filter.accept(self)?;
            kwargs.insert("filter".to_string(), filter_value);
        }

        Ok((structured_query.query.clone(), kwargs))
    }
}

/// Visitor for translating to Weaviate filter format.
///
/// Weaviate uses structured filter format with operator/operands structure.
#[derive(Debug, Clone)]
pub struct WeaviateTranslator {
    allowed_comparators: Vec<Comparator>,
    allowed_operators: Vec<Operator>,
}

impl WeaviateTranslator {
    /// Create a new `WeaviateTranslator` with default allowed operations.
    #[must_use]
    pub fn new() -> Self {
        Self {
            allowed_comparators: vec![
                Comparator::Eq,
                Comparator::Ne,
                Comparator::Lt,
                Comparator::Lte,
                Comparator::Gt,
                Comparator::Gte,
            ],
            allowed_operators: vec![Operator::And, Operator::Or],
        }
    }

    fn format_comparator(&self, comp: &Comparator) -> &'static str {
        match comp {
            Comparator::Eq => "Equal",
            Comparator::Ne => "NotEqual",
            Comparator::Lt => "LessThan",
            Comparator::Lte => "LessThanEqual",
            Comparator::Gt => "GreaterThan",
            Comparator::Gte => "GreaterThanEqual",
            _other => {
                // Validation ensures only allowed comparators reach here; catch bugs in debug builds
                debug_assert!(
                    false,
                    "WeaviateTranslator received unexpected comparator {_other:?} - validation may have a bug"
                );
                "Equal"
            }
        }
    }

    fn format_operator(&self, op: &Operator) -> &'static str {
        match op {
            Operator::And => "And",
            Operator::Or => "Or",
            _other => {
                // Validation ensures only allowed operators reach here; catch bugs in debug builds
                debug_assert!(
                    false,
                    "WeaviateTranslator received unexpected operator {_other:?} - validation may have a bug"
                );
                "And"
            }
        }
    }
}

impl Default for WeaviateTranslator {
    fn default() -> Self {
        Self::new()
    }
}

impl Visitor for WeaviateTranslator {
    type Output = Value;
    type Error = VisitorError;

    fn allowed_comparators(&self) -> Option<&[Comparator]> {
        Some(&self.allowed_comparators)
    }

    fn allowed_operators(&self) -> Option<&[Operator]> {
        Some(&self.allowed_operators)
    }

    fn error_disallowed_comparator(
        &self,
        comparator: Comparator,
        allowed: &[Comparator],
    ) -> Self::Error {
        VisitorError::DisallowedComparator {
            comparator,
            allowed: allowed.to_vec(),
        }
    }

    fn error_disallowed_operator(&self, operator: Operator, allowed: &[Operator]) -> Self::Error {
        VisitorError::DisallowedOperator {
            operator,
            allowed: allowed.to_vec(),
        }
    }

    fn visit_operation(&mut self, operation: &Operation) -> Result<Self::Output, Self::Error> {
        self.validate_operator(operation.operator)?;

        let args: Result<Vec<Value>, _> = operation
            .arguments
            .iter()
            .map(|arg| arg.accept(self))
            .collect();

        Ok(json!({
            "operator": self.format_operator(&operation.operator),
            "operands": args?
        }))
    }

    fn visit_comparison(&mut self, comparison: &Comparison) -> Result<Self::Output, Self::Error> {
        self.validate_comparator(comparison.comparator)?;

        // Determine value type
        let (value_type, value) = if comparison.value.is_boolean() {
            ("valueBoolean", comparison.value.clone())
        } else if comparison.value.is_i64() {
            ("valueInt", comparison.value.clone())
        } else if comparison.value.is_f64() {
            ("valueNumber", comparison.value.clone())
        } else {
            ("valueText", comparison.value.clone())
        };

        Ok(json!({
            "path": [comparison.attribute.clone()],
            "operator": self.format_comparator(&comparison.comparator),
            value_type: value
        }))
    }

    fn visit_structured_query(
        &mut self,
        structured_query: &StructuredQuery,
    ) -> Result<(String, std::collections::HashMap<String, Value>), Self::Error> {
        let mut kwargs = std::collections::HashMap::new();

        if let Some(filter) = &structured_query.filter {
            let filter_value = filter.accept(self)?;
            kwargs.insert("where_filter".to_string(), filter_value);
        }

        Ok((structured_query.query.clone(), kwargs))
    }
}

/// Visitor for translating to Elasticsearch filter format.
///
/// Elasticsearch uses its query DSL with bool queries.
#[derive(Debug, Clone)]
pub struct ElasticsearchTranslator {
    allowed_comparators: Vec<Comparator>,
    allowed_operators: Vec<Operator>,
}

impl ElasticsearchTranslator {
    /// Create a new `ElasticsearchTranslator` with default allowed operations.
    #[must_use]
    pub fn new() -> Self {
        Self {
            allowed_comparators: vec![
                Comparator::Eq,
                Comparator::Ne,
                Comparator::Lt,
                Comparator::Lte,
                Comparator::Gt,
                Comparator::Gte,
                Comparator::Contain,
                Comparator::Like,
            ],
            allowed_operators: vec![Operator::And, Operator::Or, Operator::Not],
        }
    }
}

impl Default for ElasticsearchTranslator {
    fn default() -> Self {
        Self::new()
    }
}

impl Visitor for ElasticsearchTranslator {
    type Output = Value;
    type Error = VisitorError;

    fn allowed_comparators(&self) -> Option<&[Comparator]> {
        Some(&self.allowed_comparators)
    }

    fn allowed_operators(&self) -> Option<&[Operator]> {
        Some(&self.allowed_operators)
    }

    fn error_disallowed_comparator(
        &self,
        comparator: Comparator,
        allowed: &[Comparator],
    ) -> Self::Error {
        VisitorError::DisallowedComparator {
            comparator,
            allowed: allowed.to_vec(),
        }
    }

    fn error_disallowed_operator(&self, operator: Operator, allowed: &[Operator]) -> Self::Error {
        VisitorError::DisallowedOperator {
            operator,
            allowed: allowed.to_vec(),
        }
    }

    fn visit_operation(&mut self, operation: &Operation) -> Result<Self::Output, Self::Error> {
        self.validate_operator(operation.operator)?;

        let args: Result<Vec<Value>, _> = operation
            .arguments
            .iter()
            .map(|arg| arg.accept(self))
            .collect();
        let args = args?;

        let bool_key = match operation.operator {
            Operator::And => "must",
            Operator::Or => "should",
            Operator::Not => "must_not",
        };

        Ok(json!({
            "bool": {
                bool_key: args
            }
        }))
    }

    fn visit_comparison(&mut self, comparison: &Comparison) -> Result<Self::Output, Self::Error> {
        self.validate_comparator(comparison.comparator)?;

        Ok(match comparison.comparator {
            Comparator::Eq => json!({
                "term": {
                    format!("metadata.{}", comparison.attribute): comparison.value.clone()
                }
            }),
            Comparator::Ne => json!({
                "bool": {
                    "must_not": [{
                        "term": {
                            format!("metadata.{}", comparison.attribute): comparison.value.clone()
                        }
                    }]
                }
            }),
            Comparator::Lt | Comparator::Lte | Comparator::Gt | Comparator::Gte => json!({
                "range": {
                    format!("metadata.{}", comparison.attribute): {
                        comparison.comparator.value(): comparison.value.clone()
                    }
                }
            }),
            Comparator::Contain => json!({
                "match": {
                    format!("metadata.{}", comparison.attribute): {
                        "query": comparison.value.clone()
                    }
                }
            }),
            Comparator::Like => json!({
                "wildcard": {
                    format!("metadata.{}", comparison.attribute): {
                        "value": comparison.value.clone()
                    }
                }
            }),
            _ => json!({
                "term": {
                    format!("metadata.{}", comparison.attribute): comparison.value.clone()
                }
            }),
        })
    }

    fn visit_structured_query(
        &mut self,
        structured_query: &StructuredQuery,
    ) -> Result<(String, std::collections::HashMap<String, Value>), Self::Error> {
        let mut kwargs = std::collections::HashMap::new();

        if let Some(filter) = &structured_query.filter {
            let filter_value = filter.accept(self)?;
            kwargs.insert("filter".to_string(), filter_value);
        }

        Ok((structured_query.query.clone(), kwargs))
    }
}

#[cfg(test)]
mod additional_translator_tests {
    use crate::core::structured_query::{Expr, Visitor};
    use crate::test_prelude::*;

    // Chroma Translator Tests
    #[test]
    fn test_chroma_simple_comparison() {
        let mut translator = ChromaTranslator::new();
        let comp = Comparison::new(Comparator::Eq, "category".to_string(), "books");
        let result = comp.accept(&mut translator).unwrap();

        assert_eq!(
            result,
            json!({
                "category": {
                    "$eq": "books"
                }
            })
        );
    }

    #[test]
    fn test_chroma_and_operation() {
        let mut translator = ChromaTranslator::new();
        let comp1 = Comparison::new(Comparator::Gt, "price".to_string(), 10);
        let comp2 = Comparison::new(Comparator::Lt, "price".to_string(), 100);
        let op = Operation::new(Operator::And, vec![comp1.into(), comp2.into()]);
        let result = op.accept(&mut translator).unwrap();

        assert_eq!(
            result,
            json!({
                "$and": [
                    {"price": {"$gt": 10}},
                    {"price": {"$lt": 100}}
                ]
            })
        );
    }

    #[test]
    fn test_chroma_structured_query() {
        let mut translator = ChromaTranslator::new();
        let comp = Comparison::new(Comparator::Eq, "category".to_string(), "books");
        let query = StructuredQuery::new("search books".to_string(), Some(comp.into()), Some(5));
        let (query_str, kwargs) = translator.visit_structured_query(&query).unwrap();

        assert_eq!(query_str, "search books");
        assert_eq!(
            kwargs.get("filter"),
            Some(&json!({
                "category": {"$eq": "books"}
            }))
        );
    }

    // Weaviate Translator Tests
    #[test]
    fn test_weaviate_simple_comparison() {
        let mut translator = WeaviateTranslator::new();
        let comp = Comparison::new(Comparator::Eq, "category".to_string(), "books");
        let result = comp.accept(&mut translator).unwrap();

        assert_eq!(
            result,
            json!({
                "path": ["category"],
                "operator": "Equal",
                "valueText": "books"
            })
        );
    }

    #[test]
    fn test_weaviate_number_comparison() {
        let mut translator = WeaviateTranslator::new();
        let comp = Comparison::new(Comparator::Gt, "price".to_string(), 100);
        let result = comp.accept(&mut translator).unwrap();

        assert_eq!(
            result,
            json!({
                "path": ["price"],
                "operator": "GreaterThan",
                "valueInt": 100
            })
        );
    }

    #[test]
    fn test_weaviate_and_operation() {
        let mut translator = WeaviateTranslator::new();
        let comp1 = Comparison::new(Comparator::Gt, "age".to_string(), 18);
        let comp2 = Comparison::new(Comparator::Lt, "age".to_string(), 65);
        let op = Operation::new(Operator::And, vec![comp1.into(), comp2.into()]);
        let result = op.accept(&mut translator).unwrap();

        assert_eq!(
            result,
            json!({
                "operator": "And",
                "operands": [
                    {"path": ["age"], "operator": "GreaterThan", "valueInt": 18},
                    {"path": ["age"], "operator": "LessThan", "valueInt": 65}
                ]
            })
        );
    }

    #[test]
    fn test_weaviate_structured_query() {
        let mut translator = WeaviateTranslator::new();
        let comp = Comparison::new(Comparator::Eq, "category".to_string(), "books");
        let query = StructuredQuery::new("search books".to_string(), Some(comp.into()), Some(10));
        let (query_str, kwargs) = translator.visit_structured_query(&query).unwrap();

        assert_eq!(query_str, "search books");
        assert_eq!(
            kwargs.get("where_filter"),
            Some(&json!({
                "path": ["category"],
                "operator": "Equal",
                "valueText": "books"
            }))
        );
    }

    // Elasticsearch Translator Tests
    #[test]
    fn test_elasticsearch_eq_comparison() {
        let mut translator = ElasticsearchTranslator::new();
        let comp = Comparison::new(Comparator::Eq, "category".to_string(), "books");
        let result = comp.accept(&mut translator).unwrap();

        assert_eq!(
            result,
            json!({
                "term": {
                    "metadata.category": "books"
                }
            })
        );
    }

    #[test]
    fn test_elasticsearch_ne_comparison() {
        let mut translator = ElasticsearchTranslator::new();
        let comp = Comparison::new(Comparator::Ne, "status".to_string(), "deleted");
        let result = comp.accept(&mut translator).unwrap();

        assert_eq!(
            result,
            json!({
                "bool": {
                    "must_not": [{
                        "term": {
                            "metadata.status": "deleted"
                        }
                    }]
                }
            })
        );
    }

    #[test]
    fn test_elasticsearch_range_comparison() {
        let mut translator = ElasticsearchTranslator::new();
        let comp = Comparison::new(Comparator::Gt, "price".to_string(), 100);
        let result = comp.accept(&mut translator).unwrap();

        assert_eq!(
            result,
            json!({
                "range": {
                    "metadata.price": {
                        "gt": 100
                    }
                }
            })
        );
    }

    #[test]
    fn test_elasticsearch_and_operation() {
        let mut translator = ElasticsearchTranslator::new();
        let comp1 = Comparison::new(Comparator::Eq, "category".to_string(), "books");
        let comp2 = Comparison::new(Comparator::Gt, "price".to_string(), 10);
        let op = Operation::new(Operator::And, vec![comp1.into(), comp2.into()]);
        let result = op.accept(&mut translator).unwrap();

        assert_eq!(
            result,
            json!({
                "bool": {
                    "must": [
                        {"term": {"metadata.category": "books"}},
                        {"range": {"metadata.price": {"gt": 10}}}
                    ]
                }
            })
        );
    }

    #[test]
    fn test_elasticsearch_structured_query() {
        let mut translator = ElasticsearchTranslator::new();
        let comp = Comparison::new(Comparator::Eq, "category".to_string(), "books");
        let query = StructuredQuery::new("search books".to_string(), Some(comp.into()), Some(20));
        let (query_str, kwargs) = translator.visit_structured_query(&query).unwrap();

        assert_eq!(query_str, "search books");
        assert_eq!(
            kwargs.get("filter"),
            Some(&json!({
                "term": {
                    "metadata.category": "books"
                }
            }))
        );
    }

    #[test]
    fn test_elasticsearch_contain_comparison() {
        let mut translator = ElasticsearchTranslator::new();
        let comp = Comparison::new(Comparator::Contain, "description".to_string(), "rust");
        let result = comp.accept(&mut translator).unwrap();

        assert_eq!(
            result,
            json!({
                "match": {
                    "metadata.description": {
                        "query": "rust"
                    }
                }
            })
        );
    }

    #[test]
    fn test_elasticsearch_like_comparison() {
        let mut translator = ElasticsearchTranslator::new();
        let comp = Comparison::new(Comparator::Like, "name".to_string(), "rust*");
        let result = comp.accept(&mut translator).unwrap();

        assert_eq!(
            result,
            json!({
                "wildcard": {
                    "metadata.name": {
                        "value": "rust*"
                    }
                }
            })
        );
    }
}
