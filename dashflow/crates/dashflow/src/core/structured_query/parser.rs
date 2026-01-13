//! Query parser for structured query language.
//!
//! This module provides a parser for converting filter expressions into
//! structured query representations. It supports:
//! - Comparisons: eq(field, value), gt(field, value), etc.
//! - Operations: and(...), or(...), not(...)
//! - Values: strings, integers, floats, booleans, lists, dates

use super::{Comparator, Comparison, FilterDirective, Operation, Operator};
use crate::core::error::{Error, Result};
use serde_json::Value;

/// Parser for structured query filter expressions.
#[derive(Debug, Clone)]
pub struct QueryParser {
    allowed_comparators: Option<Vec<Comparator>>,
    allowed_operators: Option<Vec<Operator>>,
    allowed_attributes: Option<Vec<String>>,
}

impl QueryParser {
    /// Create a new `QueryParser` with no restrictions.
    #[must_use]
    pub fn new() -> Self {
        Self {
            allowed_comparators: None,
            allowed_operators: None,
            allowed_attributes: None,
        }
    }

    /// Set allowed comparators.
    #[must_use]
    pub fn with_allowed_comparators(mut self, comparators: Vec<Comparator>) -> Self {
        self.allowed_comparators = Some(comparators);
        self
    }

    /// Set allowed operators.
    #[must_use]
    pub fn with_allowed_operators(mut self, operators: Vec<Operator>) -> Self {
        self.allowed_operators = Some(operators);
        self
    }

    /// Set allowed attributes.
    #[must_use]
    pub fn with_allowed_attributes(mut self, attributes: Vec<String>) -> Self {
        self.allowed_attributes = Some(attributes);
        self
    }

    /// Parse a filter expression string into a `FilterDirective`.
    ///
    /// Examples:
    /// - `eq("age", 18)` -> Comparison
    /// - `and(gt("age", 18), lt("age", 65))` -> Operation
    /// - `or(eq("category", "books"), eq("category", "electronics"))` -> Operation
    pub fn parse(&self, input: &str) -> Result<FilterDirective> {
        let input = input.trim();
        if input.is_empty() {
            return Err(Error::ParseError("Empty filter expression".to_string()));
        }

        self.parse_expression(input)
    }

    fn parse_expression(&self, input: &str) -> Result<FilterDirective> {
        let input = input.trim();

        // Check if it's a function call
        if let Some(paren_pos) = input.find('(') {
            let func_name = input[..paren_pos].trim();
            if !input.ends_with(')') {
                return Err(Error::ParseError(format!(
                    "Unmatched parentheses in: {input}"
                )));
            }

            let args_str = &input[paren_pos + 1..input.len() - 1];
            let args = self.parse_arguments(args_str)?;

            return self.parse_function_call(func_name, args);
        }

        Err(Error::ParseError(format!("Invalid expression: {input}")))
    }

    fn parse_function_call(&self, func_name: &str, args: Vec<Value>) -> Result<FilterDirective> {
        // Try to parse as comparator
        if let Ok(comparator) = self.parse_comparator(func_name) {
            if args.len() != 2 {
                return Err(Error::ParseError(format!(
                    "Comparator {} requires exactly 2 arguments, got {}",
                    func_name,
                    args.len()
                )));
            }

            let attribute = args[0]
                .as_str()
                .ok_or_else(|| {
                    Error::ParseError(format!(
                        "First argument to comparator must be a string (attribute name), got: {:?}",
                        args[0]
                    ))
                })?
                .to_string();

            // Check if attribute is allowed
            if let Some(allowed) = &self.allowed_attributes {
                if !allowed.contains(&attribute) {
                    return Err(Error::ParseError(format!(
                        "Attribute '{attribute}' is not in allowed attributes: {allowed:?}"
                    )));
                }
            }

            return Ok(Comparison::new(comparator, attribute, args[1].clone()).into());
        }

        // Try to parse as operator
        if let Ok(operator) = self.parse_operator(func_name) {
            // Special case: AND/OR with single argument returns that argument
            if args.len() == 1 && (operator == Operator::And || operator == Operator::Or) {
                // The single argument must be a filter directive
                // For simplicity, we'll just return an error here
                // In practice, this case is handled by the output parser
                return Err(Error::ParseError(format!(
                    "Operator {func_name} requires at least 2 arguments, got 1"
                )));
            }

            // Parse arguments as filter directives
            let mut filter_args = Vec::new();
            for arg in args {
                // If the arg is a string and looks like a function call, parse it
                if let Some(arg_str) = arg.as_str() {
                    filter_args.push(self.parse_expression(arg_str)?);
                } else {
                    return Err(Error::ParseError(format!(
                        "Operator arguments must be filter expressions, got: {arg:?}"
                    )));
                }
            }

            return Ok(Operation::new(operator, filter_args).into());
        }

        Err(Error::ParseError(format!(
            "Unknown function: {}. Valid functions are comparators ({:?}) and operators ({:?})",
            func_name,
            Comparator::all(),
            Operator::all()
        )))
    }

    fn parse_comparator(&self, name: &str) -> Result<Comparator> {
        let comparator = match name {
            "eq" => Comparator::Eq,
            "ne" => Comparator::Ne,
            "gt" => Comparator::Gt,
            "gte" => Comparator::Gte,
            "lt" => Comparator::Lt,
            "lte" => Comparator::Lte,
            "contain" => Comparator::Contain,
            "like" => Comparator::Like,
            "in" => Comparator::In,
            "nin" => Comparator::Nin,
            _ => return Err(Error::ParseError(format!("Unknown comparator: {name}"))),
        };

        // Check if comparator is allowed
        if let Some(allowed) = &self.allowed_comparators {
            if !allowed.contains(&comparator) {
                return Err(Error::ParseError(format!(
                    "Comparator '{name}' is not in allowed comparators: {allowed:?}"
                )));
            }
        }

        Ok(comparator)
    }

    fn parse_operator(&self, name: &str) -> Result<Operator> {
        let operator = match name {
            "and" => Operator::And,
            "or" => Operator::Or,
            "not" => Operator::Not,
            _ => return Err(Error::ParseError(format!("Unknown operator: {name}"))),
        };

        // Check if operator is allowed
        if let Some(allowed) = &self.allowed_operators {
            if !allowed.contains(&operator) {
                return Err(Error::ParseError(format!(
                    "Operator '{name}' is not in allowed operators: {allowed:?}"
                )));
            }
        }

        Ok(operator)
    }

    fn parse_arguments(&self, args_str: &str) -> Result<Vec<Value>> {
        let args_str = args_str.trim();
        if args_str.is_empty() {
            return Ok(Vec::new());
        }

        let mut args = Vec::new();
        let mut current_arg = String::new();
        let mut depth = 0;
        let mut in_string = false;
        let mut in_list = false;

        for ch in args_str.chars() {
            match ch {
                '"' | '\'' if !in_list => {
                    in_string = !in_string;
                    current_arg.push(ch);
                }
                '[' if !in_string => {
                    in_list = true;
                    current_arg.push(ch);
                }
                ']' if !in_string => {
                    in_list = false;
                    current_arg.push(ch);
                }
                '(' if !in_string && !in_list => {
                    depth += 1;
                    current_arg.push(ch);
                }
                ')' if !in_string && !in_list => {
                    depth -= 1;
                    current_arg.push(ch);
                }
                ',' if !in_string && !in_list && depth == 0 => {
                    args.push(self.parse_value(current_arg.trim())?);
                    current_arg.clear();
                }
                _ => {
                    current_arg.push(ch);
                }
            }
        }

        if !current_arg.trim().is_empty() {
            args.push(self.parse_value(current_arg.trim())?);
        }

        Ok(args)
    }

    fn parse_value(&self, value_str: &str) -> Result<Value> {
        let value_str = value_str.trim();

        // Check for boolean literals
        if value_str == "true" || value_str == "True" || value_str == "TRUE" {
            return Ok(Value::Bool(true));
        }
        if value_str == "false" || value_str == "False" || value_str == "FALSE" {
            return Ok(Value::Bool(false));
        }

        // Check for string literals
        if (value_str.starts_with('"') && value_str.ends_with('"'))
            || (value_str.starts_with('\'') && value_str.ends_with('\''))
        {
            return Ok(Value::String(value_str[1..value_str.len() - 1].to_string()));
        }

        // Check for list literals
        if value_str.starts_with('[') && value_str.ends_with(']') {
            let list_str = &value_str[1..value_str.len() - 1];
            let items = self.parse_arguments(list_str)?;
            return Ok(Value::Array(items));
        }

        // Check for nested function calls (filter expressions)
        if value_str.contains('(') {
            // This is a nested filter expression, return as string for later parsing
            return Ok(Value::String(value_str.to_string()));
        }

        // Try to parse as number
        if let Ok(int_val) = value_str.parse::<i64>() {
            return Ok(Value::Number(int_val.into()));
        }
        if let Ok(float_val) = value_str.parse::<f64>() {
            return Ok(Value::Number(
                serde_json::Number::from_f64(float_val)
                    .ok_or_else(|| Error::ParseError(format!("Invalid float: {value_str}")))?,
            ));
        }

        // Default to string (unquoted strings are allowed in some contexts)
        Ok(Value::String(value_str.to_string()))
    }
}

impl Default for QueryParser {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::QueryParser;
    use crate::test_prelude::*;

    #[test]
    fn test_parse_simple_comparison() {
        let parser = QueryParser::new();
        let result = parser.parse(r#"eq("age", 18)"#).unwrap();

        match result {
            FilterDirective::Comparison(comp) => {
                assert_eq!(comp.comparator, Comparator::Eq);
                assert_eq!(comp.attribute, "age");
                assert_eq!(comp.value, Value::Number(18.into()));
            }
            _ => panic!("Expected Comparison"),
        }
    }

    #[test]
    fn test_parse_gt_comparison() {
        let parser = QueryParser::new();
        let result = parser.parse(r#"gt("price", 100)"#).unwrap();

        match result {
            FilterDirective::Comparison(comp) => {
                assert_eq!(comp.comparator, Comparator::Gt);
                assert_eq!(comp.attribute, "price");
                assert_eq!(comp.value, Value::Number(100.into()));
            }
            _ => panic!("Expected Comparison"),
        }
    }

    #[test]
    fn test_parse_string_value() {
        let parser = QueryParser::new();
        let result = parser.parse(r#"eq("category", "books")"#).unwrap();

        match result {
            FilterDirective::Comparison(comp) => {
                assert_eq!(comp.attribute, "category");
                assert_eq!(comp.value, Value::String("books".to_string()));
            }
            _ => panic!("Expected Comparison"),
        }
    }

    #[test]
    fn test_parse_boolean_values() {
        let parser = QueryParser::new();
        let result = parser.parse(r#"eq("active", true)"#).unwrap();

        match result {
            FilterDirective::Comparison(comp) => {
                assert_eq!(comp.value, Value::Bool(true));
            }
            _ => panic!("Expected Comparison"),
        }
    }

    #[test]
    fn test_parse_list_value() {
        let parser = QueryParser::new();
        let result = parser
            .parse(r#"in("category", ["books", "electronics"])"#)
            .unwrap();

        match result {
            FilterDirective::Comparison(comp) => {
                assert_eq!(comp.comparator, Comparator::In);
                match &comp.value {
                    Value::Array(arr) => {
                        assert_eq!(arr.len(), 2);
                        assert_eq!(arr[0], Value::String("books".to_string()));
                        assert_eq!(arr[1], Value::String("electronics".to_string()));
                    }
                    _ => panic!("Expected array value"),
                }
            }
            _ => panic!("Expected Comparison"),
        }
    }

    #[test]
    fn test_parse_and_operation() {
        let parser = QueryParser::new();
        let result = parser
            .parse(r#"and(gt("age", 18), lt("age", 65))"#)
            .unwrap();

        match result {
            FilterDirective::Operation(op) => {
                assert_eq!(op.operator, Operator::And);
                assert_eq!(op.arguments.len(), 2);
            }
            _ => panic!("Expected Operation"),
        }
    }

    #[test]
    fn test_parse_or_operation() {
        let parser = QueryParser::new();
        let result = parser
            .parse(r#"or(eq("category", "books"), eq("category", "electronics"))"#)
            .unwrap();

        match result {
            FilterDirective::Operation(op) => {
                assert_eq!(op.operator, Operator::Or);
                assert_eq!(op.arguments.len(), 2);
            }
            _ => panic!("Expected Operation"),
        }
    }

    #[test]
    fn test_parse_not_operation() {
        let parser = QueryParser::new();
        let result = parser.parse(r#"not(eq("deleted", true))"#).unwrap();

        match result {
            FilterDirective::Operation(op) => {
                assert_eq!(op.operator, Operator::Not);
                assert_eq!(op.arguments.len(), 1);
            }
            _ => panic!("Expected Operation"),
        }
    }

    #[test]
    fn test_allowed_comparators() {
        let parser =
            QueryParser::new().with_allowed_comparators(vec![Comparator::Eq, Comparator::Ne]);

        // Should succeed
        parser.parse(r#"eq("age", 18)"#).unwrap();

        // Should fail
        let result = parser.parse(r#"gt("age", 18)"#);
        assert!(result.is_err());
    }

    #[test]
    fn test_allowed_operators() {
        let parser = QueryParser::new().with_allowed_operators(vec![Operator::And, Operator::Or]);

        // Should succeed
        parser
            .parse(r#"and(eq("age", 18), eq("active", true))"#)
            .unwrap();

        // Should fail
        let result = parser.parse(r#"not(eq("deleted", true))"#);
        assert!(result.is_err());
    }

    #[test]
    fn test_allowed_attributes() {
        let parser = QueryParser::new()
            .with_allowed_attributes(vec!["age".to_string(), "category".to_string()]);

        // Should succeed
        parser.parse(r#"eq("age", 18)"#).unwrap();

        // Should fail
        let result = parser.parse(r#"eq("price", 100)"#);
        assert!(result.is_err());
    }

    #[test]
    fn test_empty_expression() {
        let parser = QueryParser::new();
        let result = parser.parse("");
        assert!(result.is_err());
    }

    #[test]
    fn test_invalid_expression() {
        let parser = QueryParser::new();
        let result = parser.parse("invalid");
        assert!(result.is_err());
    }
}
