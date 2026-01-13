//! Internal representation of a structured query language.
//!
//! This module provides types for representing structured queries with filtering
//! and metadata operations. It implements the visitor pattern for translating
//! queries into backend-specific filter formats.
//!
//! # Example
//!
//! ```rust
//! use dashflow::core::structured_query::{
//!     Comparator, Comparison, Operation, Operator, StructuredQuery, Visitor
//! };
//!
//! // Create a comparison: age > 18
//! let filter = Comparison::new(Comparator::Gt, "age".to_string(), 18);
//!
//! // Create a structured query with filter
//! let query = StructuredQuery::new("search term".to_string(), Some(filter.into()), None);
//! ```

pub mod parser;
pub mod query_constructor;
pub mod visitors;

use serde::{Deserialize, Serialize};
use std::fmt;

/// Enumerator of logical operators.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Operator {
    /// Logical AND
    #[serde(rename = "and")]
    And,
    /// Logical OR
    #[serde(rename = "or")]
    Or,
    /// Logical NOT
    #[serde(rename = "not")]
    Not,
}

impl fmt::Display for Operator {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Operator::And => write!(f, "and"),
            Operator::Or => write!(f, "or"),
            Operator::Not => write!(f, "not"),
        }
    }
}

impl Operator {
    /// Get all possible operators
    #[must_use]
    pub fn all() -> Vec<Operator> {
        vec![Operator::And, Operator::Or, Operator::Not]
    }

    /// Get string value
    #[must_use]
    pub fn value(&self) -> &'static str {
        match self {
            Operator::And => "and",
            Operator::Or => "or",
            Operator::Not => "not",
        }
    }
}

/// Enumerator of comparison operators.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Comparator {
    /// Equal
    #[serde(rename = "eq")]
    Eq,
    /// Not equal
    #[serde(rename = "ne")]
    Ne,
    /// Greater than
    #[serde(rename = "gt")]
    Gt,
    /// Greater than or equal
    #[serde(rename = "gte")]
    Gte,
    /// Less than
    #[serde(rename = "lt")]
    Lt,
    /// Less than or equal
    #[serde(rename = "lte")]
    Lte,
    /// Contains (for strings/arrays)
    #[serde(rename = "contain")]
    Contain,
    /// Like (pattern matching)
    #[serde(rename = "like")]
    Like,
    /// In (value in list)
    #[serde(rename = "in")]
    In,
    /// Not in (value not in list)
    #[serde(rename = "nin")]
    Nin,
}

impl fmt::Display for Comparator {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Comparator::Eq => write!(f, "eq"),
            Comparator::Ne => write!(f, "ne"),
            Comparator::Gt => write!(f, "gt"),
            Comparator::Gte => write!(f, "gte"),
            Comparator::Lt => write!(f, "lt"),
            Comparator::Lte => write!(f, "lte"),
            Comparator::Contain => write!(f, "contain"),
            Comparator::Like => write!(f, "like"),
            Comparator::In => write!(f, "in"),
            Comparator::Nin => write!(f, "nin"),
        }
    }
}

impl Comparator {
    /// Get all possible comparators
    #[must_use]
    pub fn all() -> Vec<Comparator> {
        vec![
            Comparator::Eq,
            Comparator::Ne,
            Comparator::Gt,
            Comparator::Gte,
            Comparator::Lt,
            Comparator::Lte,
            Comparator::Contain,
            Comparator::Like,
            Comparator::In,
            Comparator::Nin,
        ]
    }

    /// Get string value
    #[must_use]
    pub fn value(&self) -> &'static str {
        match self {
            Comparator::Eq => "eq",
            Comparator::Ne => "ne",
            Comparator::Gt => "gt",
            Comparator::Gte => "gte",
            Comparator::Lt => "lt",
            Comparator::Lte => "lte",
            Comparator::Contain => "contain",
            Comparator::Like => "like",
            Comparator::In => "in",
            Comparator::Nin => "nin",
        }
    }
}

/// Base trait for all expressions in the query language.
pub trait Expr {
    /// Accept a visitor and return the result.
    fn accept<V: Visitor>(&self, visitor: &mut V) -> Result<V::Output, V::Error>;
}

/// Filtering expression (comparison or operation).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum FilterDirective {
    /// Comparison to a value
    Comparison(Comparison),
    /// Logical operation over other directives
    Operation(Operation),
}

impl Expr for FilterDirective {
    fn accept<V: Visitor>(&self, visitor: &mut V) -> Result<V::Output, V::Error> {
        match self {
            FilterDirective::Comparison(c) => c.accept(visitor),
            FilterDirective::Operation(o) => o.accept(visitor),
        }
    }
}

impl From<Comparison> for FilterDirective {
    fn from(c: Comparison) -> Self {
        FilterDirective::Comparison(c)
    }
}

impl From<Operation> for FilterDirective {
    fn from(o: Operation) -> Self {
        FilterDirective::Operation(o)
    }
}

/// Comparison to a value.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Comparison {
    /// The comparator to use
    pub comparator: Comparator,
    /// The attribute to compare
    pub attribute: String,
    /// The value to compare to (can be any JSON value)
    pub value: serde_json::Value,
}

impl Comparison {
    /// Create a new comparison.
    pub fn new<V: Into<serde_json::Value>>(
        comparator: Comparator,
        attribute: String,
        value: V,
    ) -> Self {
        Self {
            comparator,
            attribute,
            value: value.into(),
        }
    }
}

impl Expr for Comparison {
    fn accept<V: Visitor>(&self, visitor: &mut V) -> Result<V::Output, V::Error> {
        visitor.visit_comparison(self)
    }
}

/// Logical operation over other directives.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Operation {
    /// The operator to use
    pub operator: Operator,
    /// The arguments to the operator
    pub arguments: Vec<FilterDirective>,
}

impl Operation {
    /// Create a new operation.
    #[must_use]
    pub fn new(operator: Operator, arguments: Vec<FilterDirective>) -> Self {
        Self {
            operator,
            arguments,
        }
    }
}

impl Expr for Operation {
    fn accept<V: Visitor>(&self, visitor: &mut V) -> Result<V::Output, V::Error> {
        visitor.visit_operation(self)
    }
}

/// Structured query with query string, optional filter, and optional limit.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StructuredQuery {
    /// Query string
    pub query: String,
    /// Filtering expression
    #[serde(skip_serializing_if = "Option::is_none")]
    pub filter: Option<FilterDirective>,
    /// Limit on the number of results
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<usize>,
}

impl StructuredQuery {
    /// Create a new structured query.
    #[must_use]
    pub fn new(query: String, filter: Option<FilterDirective>, limit: Option<usize>) -> Self {
        Self {
            query,
            filter,
            limit,
        }
    }
}

// Note: StructuredQuery intentionally does NOT implement Expr trait
// because visit_structured_query returns a different type (String, HashMap)
// than visit_operation and visit_comparison (which return V::Output).
// Call visitor.visit_structured_query() directly instead of using accept().

/// Visitor trait for translating internal query representation to backend-specific formats.
///
/// This trait defines the interface for translating structured queries into
/// backend-specific filter formats using the visitor pattern.
pub trait Visitor {
    /// Output type produced by the visitor
    type Output;
    /// Error type for the visitor
    type Error: std::error::Error;

    /// Get allowed comparators (None means all are allowed)
    fn allowed_comparators(&self) -> Option<&[Comparator]> {
        None
    }

    /// Get allowed operators (None means all are allowed)
    fn allowed_operators(&self) -> Option<&[Operator]> {
        None
    }

    /// Validate a comparator
    fn validate_comparator(&self, comparator: Comparator) -> Result<(), Self::Error> {
        if let Some(allowed) = self.allowed_comparators() {
            if !allowed.contains(&comparator) {
                return Err(self.error_disallowed_comparator(comparator, allowed));
            }
        }
        Ok(())
    }

    /// Validate an operator
    fn validate_operator(&self, operator: Operator) -> Result<(), Self::Error> {
        if let Some(allowed) = self.allowed_operators() {
            if !allowed.contains(&operator) {
                return Err(self.error_disallowed_operator(operator, allowed));
            }
        }
        Ok(())
    }

    /// Create error for disallowed comparator
    fn error_disallowed_comparator(
        &self,
        comparator: Comparator,
        allowed: &[Comparator],
    ) -> Self::Error;

    /// Create error for disallowed operator
    fn error_disallowed_operator(&self, operator: Operator, allowed: &[Operator]) -> Self::Error;

    /// Translate an Operation.
    fn visit_operation(&mut self, operation: &Operation) -> Result<Self::Output, Self::Error>;

    /// Translate a Comparison.
    fn visit_comparison(&mut self, comparison: &Comparison) -> Result<Self::Output, Self::Error>;

    /// Translate a `StructuredQuery`.
    /// Returns a tuple of (`query_string`, kwargs) where kwargs contains the filter and other params.
    fn visit_structured_query(
        &mut self,
        structured_query: &StructuredQuery,
    ) -> Result<(String, std::collections::HashMap<String, serde_json::Value>), Self::Error>;
}

/// Information about a data source attribute.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AttributeInfo {
    /// Attribute name
    pub name: String,
    /// Attribute description
    pub description: String,
    /// Attribute type (e.g., "string", "integer", "float")
    #[serde(rename = "type")]
    pub attr_type: String,
}

impl AttributeInfo {
    /// Create a new `AttributeInfo`.
    #[must_use]
    pub fn new(name: String, description: String, attr_type: String) -> Self {
        Self {
            name,
            description,
            attr_type,
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::test_prelude::*;

    #[test]
    fn test_comparator_display() {
        assert_eq!(Comparator::Eq.to_string(), "eq");
        assert_eq!(Comparator::Gt.to_string(), "gt");
        assert_eq!(Comparator::In.to_string(), "in");
    }

    #[test]
    fn test_operator_display() {
        assert_eq!(Operator::And.to_string(), "and");
        assert_eq!(Operator::Or.to_string(), "or");
        assert_eq!(Operator::Not.to_string(), "not");
    }

    #[test]
    fn test_comparison_creation() {
        let comp = Comparison::new(Comparator::Eq, "age".to_string(), 18);
        assert_eq!(comp.comparator, Comparator::Eq);
        assert_eq!(comp.attribute, "age");
        assert_eq!(comp.value, serde_json::json!(18));
    }

    #[test]
    fn test_operation_creation() {
        let comp1 = Comparison::new(Comparator::Gt, "age".to_string(), 18);
        let comp2 = Comparison::new(Comparator::Lt, "age".to_string(), 65);
        let op = Operation::new(Operator::And, vec![comp1.into(), comp2.into()]);
        assert_eq!(op.operator, Operator::And);
        assert_eq!(op.arguments.len(), 2);
    }

    #[test]
    fn test_structured_query_creation() {
        let comp = Comparison::new(Comparator::Eq, "category".to_string(), "books");
        let query = StructuredQuery::new("search term".to_string(), Some(comp.into()), Some(10));
        assert_eq!(query.query, "search term");
        assert!(query.filter.is_some());
        assert_eq!(query.limit, Some(10));
    }

    #[test]
    fn test_structured_query_no_filter() {
        let query = StructuredQuery::new("search term".to_string(), None, None);
        assert_eq!(query.query, "search term");
        assert!(query.filter.is_none());
        assert!(query.limit.is_none());
    }

    #[test]
    fn test_attribute_info() {
        let attr = AttributeInfo::new(
            "year".to_string(),
            "Year of publication".to_string(),
            "integer".to_string(),
        );
        assert_eq!(attr.name, "year");
        assert_eq!(attr.description, "Year of publication");
        assert_eq!(attr.attr_type, "integer");
    }

    #[test]
    fn test_serialization() {
        let comp = Comparison::new(Comparator::Eq, "age".to_string(), 18);
        let json = serde_json::to_string(&comp).unwrap();
        let deserialized: Comparison = serde_json::from_str(&json).unwrap();
        assert_eq!(comp, deserialized);
    }

    #[test]
    fn test_structured_query_serialization() {
        let comp = Comparison::new(Comparator::Gt, "price".to_string(), 100);
        let query = StructuredQuery::new("expensive items".to_string(), Some(comp.into()), Some(5));
        let json = serde_json::to_string(&query).unwrap();
        let deserialized: StructuredQuery = serde_json::from_str(&json).unwrap();
        assert_eq!(query, deserialized);
    }

    #[test]
    fn test_comparator_all() {
        let all = Comparator::all();
        assert_eq!(all.len(), 10);
        assert!(all.contains(&Comparator::Eq));
        assert!(all.contains(&Comparator::In));
    }

    #[test]
    fn test_operator_all() {
        let all = Operator::all();
        assert_eq!(all.len(), 3);
        assert!(all.contains(&Operator::And));
        assert!(all.contains(&Operator::Or));
        assert!(all.contains(&Operator::Not));
    }
}
