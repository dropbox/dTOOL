//! Query constructor for converting natural language to structured queries.
//!
//! This module provides components for building LLM-based query construction:
//! - `StructuredQueryOutputParser`: Parses LLM JSON output to `StructuredQuery`
//! - Prompt templates: Few-shot prompts for guiding LLM query generation
//! - Builder functions: Compose prompt | LLM | parser chains

use super::parser::QueryParser;
use super::{AttributeInfo, Comparator, FilterDirective, Operator, StructuredQuery};
use crate::core::error::{Error, Result};
use crate::core::output_parsers::{JsonOutputParser, OutputParser};
use serde_json::Value as JsonValue;
use std::collections::HashMap;

/// Output parser that converts LLM JSON output into a `StructuredQuery`.
///
/// Expects JSON with keys: "query", "filter" (optional), "limit" (optional).
/// The filter value is a string expression like `eq("category", "books")` which
/// is parsed using `QueryParser`.
///
/// # Example
///
/// ```rust,ignore
/// use dashflow::core::structured_query::query_constructor::StructuredQueryOutputParser;
/// use dashflow::core::output_parsers::OutputParser;
///
/// let parser = StructuredQueryOutputParser::new()
///     .with_allowed_comparators(vec![Comparator::Eq, Comparator::Ne])
///     .with_allowed_operators(vec![Operator::And, Operator::Or]);
///
/// let llm_output = r#"{
///     "query": "science fiction books",
///     "filter": "eq(\"category\", \"books\")",
///     "limit": 10
/// }"#;
///
/// let structured_query = parser.parse(llm_output)?;
/// assert_eq!(structured_query.query, "science fiction books");
/// assert_eq!(structured_query.limit, Some(10));
/// ```
#[derive(Debug, Clone)]
pub struct StructuredQueryOutputParser {
    /// Allowed comparators for filter validation
    allowed_comparators: Option<Vec<Comparator>>,
    /// Allowed operators for filter validation
    allowed_operators: Option<Vec<Operator>>,
    /// Allowed attributes for filter validation
    allowed_attributes: Option<Vec<String>>,
    /// Whether to fix invalid filter directives by ignoring disallowed components
    fix_invalid: bool,
}

impl StructuredQueryOutputParser {
    /// Create a new `StructuredQueryOutputParser` with no restrictions.
    #[must_use]
    pub fn new() -> Self {
        Self {
            allowed_comparators: None,
            allowed_operators: None,
            allowed_attributes: None,
            fix_invalid: false,
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

    /// Enable fixing invalid filter directives by ignoring disallowed components.
    #[must_use]
    pub fn with_fix_invalid(mut self, fix_invalid: bool) -> Self {
        self.fix_invalid = fix_invalid;
        self
    }

    /// Parse a filter string into a `FilterDirective` using `QueryParser`.
    fn parse_filter(&self, filter_str: &str) -> Result<Option<FilterDirective>> {
        // Build QueryParser with allowed comparators/operators/attributes
        let mut parser = QueryParser::new();

        if let Some(ref comparators) = self.allowed_comparators {
            parser = parser.with_allowed_comparators(comparators.clone());
        }

        if let Some(ref operators) = self.allowed_operators {
            parser = parser.with_allowed_operators(operators.clone());
        }

        if let Some(ref attributes) = self.allowed_attributes {
            parser = parser.with_allowed_attributes(attributes.clone());
        }

        let filter_directive = parser.parse(filter_str)?;

        // Apply fix_invalid if enabled
        if self.fix_invalid {
            Ok(fix_filter_directive(
                filter_directive,
                self.allowed_comparators.as_deref(),
                self.allowed_operators.as_deref(),
                self.allowed_attributes.as_deref(),
            ))
        } else {
            Ok(Some(filter_directive))
        }
    }
}

impl Default for StructuredQueryOutputParser {
    fn default() -> Self {
        Self::new()
    }
}

impl OutputParser for StructuredQueryOutputParser {
    type Output = StructuredQuery;

    fn parse(&self, text: &str) -> Result<Self::Output> {
        // First parse as JSON
        let json_parser = JsonOutputParser::new();
        let parsed_json = json_parser.parse(text)?;

        // Validate expected keys are present
        let obj = parsed_json
            .as_object()
            .ok_or_else(|| Error::OutputParsing("Expected JSON object".to_string()))?;

        if !obj.contains_key("query") || !obj.contains_key("filter") {
            return Err(Error::OutputParsing(
                "Missing required keys 'query' and 'filter'".to_string(),
            ));
        }

        // Extract query string
        let mut query = obj
            .get("query")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        // Handle empty query
        if query.is_empty() {
            query = " ".to_string();
        }

        // Extract and parse filter
        let filter = match obj.get("filter") {
            Some(JsonValue::String(filter_str))
                if filter_str == "NO_FILTER" || filter_str.is_empty() =>
            {
                None
            }
            Some(JsonValue::String(filter_str)) => self.parse_filter(filter_str)?,
            Some(JsonValue::Null) => None,
            Some(_) => {
                return Err(Error::OutputParsing(
                    "Filter must be a string or null".to_string(),
                ));
            }
            None => None,
        };

        // Extract optional limit
        let limit = obj
            .get("limit")
            .and_then(serde_json::Value::as_u64)
            .map(|v| v as usize);

        Ok(StructuredQuery {
            query,
            filter,
            limit,
        })
    }

    fn get_format_instructions(&self) -> String {
        "Return a JSON object with keys 'query' (string), 'filter' (string expression or \"NO_FILTER\"), and optionally 'limit' (integer).".to_string()
    }
}

/// Fix invalid filter directive by removing disallowed components.
///
/// Recursively walks the filter tree and removes:
/// - Comparisons with disallowed comparators or attributes
/// - Operations with disallowed operators
/// - Operations that end up with no valid arguments
///
/// Returns None if the entire filter becomes invalid.
// SAFETY: expect() used after length validation (fixed_args.len() == 1)
#[allow(clippy::expect_used)]
fn fix_filter_directive(
    filter: FilterDirective,
    allowed_comparators: Option<&[Comparator]>,
    allowed_operators: Option<&[Operator]>,
    allowed_attributes: Option<&[String]>,
) -> Option<FilterDirective> {
    // If no restrictions, return as-is
    if allowed_comparators.is_none() && allowed_operators.is_none() && allowed_attributes.is_none()
    {
        return Some(filter);
    }

    match filter {
        FilterDirective::Comparison(comp) => {
            // Check comparator
            if let Some(allowed) = allowed_comparators {
                if !allowed.contains(&comp.comparator) {
                    return None;
                }
            }

            // Check attribute
            if let Some(allowed) = allowed_attributes {
                if !allowed.contains(&comp.attribute) {
                    return None;
                }
            }

            Some(FilterDirective::Comparison(comp))
        }
        FilterDirective::Operation(mut op) => {
            // Check operator
            if let Some(allowed) = allowed_operators {
                if !allowed.contains(&op.operator) {
                    return None;
                }
            }

            // Recursively fix arguments
            let fixed_args: Vec<FilterDirective> = op
                .arguments
                .into_iter()
                .filter_map(|arg| {
                    fix_filter_directive(
                        arg,
                        allowed_comparators,
                        allowed_operators,
                        allowed_attributes,
                    )
                })
                .collect();

            if fixed_args.is_empty() {
                return None;
            }

            // Simplify single-argument And/Or operations
            if fixed_args.len() == 1 && matches!(op.operator, Operator::And | Operator::Or) {
                // Safety: just checked fixed_args.len() == 1
                return Some(
                    fixed_args
                        .into_iter()
                        .next()
                        .expect("fixed_args length validated as 1"),
                );
            }

            op.arguments = fixed_args;
            Some(FilterDirective::Operation(op))
        }
    }
}

// ============================================================================
// Prompt Templates
// ============================================================================

/// Default schema prompt template (without limit support).
pub const DEFAULT_SCHEMA_PROMPT: &str = r#"<< Structured Request Schema >>
When responding use a markdown code snippet with a JSON object formatted in the following schema:

```json
{{
    "query": string \ text string to compare to document contents
    "filter": string \ logical condition statement for filtering documents
}}
```

The query string should contain only text that is expected to match the contents of documents. Any conditions in the filter should not be mentioned in the query as well.

A logical condition statement is composed of one or more comparison and logical operation statements.

A comparison statement takes the form: `comp(attr, val)`:
- `comp` ({allowed_comparators}): comparator
- `attr` (string):  name of attribute to apply the comparison to
- `val` (string): is the comparison value

A logical operation statement takes the form `op(statement1, statement2, ...)`:
- `op` ({allowed_operators}): logical operator
- `statement1`, `statement2`, ... (comparison statements or logical operation statements): one or more statements to apply the operation to

Make sure that you only use the comparators and logical operators listed above and no others.
Make sure that filters only refer to attributes that exist in the data source.
Make sure that filters only use the attributed names with its function names if there are functions applied on them.
Make sure that filters only use format `YYYY-MM-DD` when handling date data typed values.
Make sure that filters take into account the descriptions of attributes and only make comparisons that are feasible given the type of data being stored.
Make sure that filters are only used as needed. If there are no filters that should be applied return "NO_FILTER" for the filter value."#;

/// Schema prompt template with limit support.
pub const SCHEMA_WITH_LIMIT_PROMPT: &str = r#"<< Structured Request Schema >>
When responding use a markdown code snippet with a JSON object formatted in the following schema:

```json
{{
    "query": string \ text string to compare to document contents
    "filter": string \ logical condition statement for filtering documents
    "limit": int \ the number of documents to retrieve
}}
```

The query string should contain only text that is expected to match the contents of documents. Any conditions in the filter should not be mentioned in the query as well.

A logical condition statement is composed of one or more comparison and logical operation statements.

A comparison statement takes the form: `comp(attr, val)`:
- `comp` ({allowed_comparators}): comparator
- `attr` (string):  name of attribute to apply the comparison to
- `val` (string): is the comparison value

A logical operation statement takes the form `op(statement1, statement2, ...)`:
- `op` ({allowed_operators}): logical operator
- `statement1`, `statement2`, ... (comparison statements or logical operation statements): one or more statements to apply the operation to

Make sure that you only use the comparators and logical operators listed above and no others.
Make sure that filters only refer to attributes that exist in the data source.
Make sure that filters only use the attributed names with its function names if there are functions applied on them.
Make sure that filters only use format `YYYY-MM-DD` when handling date data typed values.
Make sure that filters take into account the descriptions of attributes and only make comparisons that are feasible given the type of data being stored.
Make sure that filters are only used as needed. If there are no filters that should be applied return "NO_FILTER" for the filter value.
Make sure the `limit` is always an int value. It is an optional parameter so leave it blank if it does not make sense."#;

/// Default prefix for the prompt.
pub const DEFAULT_PREFIX: &str = r"Your goal is to structure the user's query to match the request schema provided below.

{schema}";

/// Default suffix for the prompt (with data source).
pub const DEFAULT_SUFFIX: &str = r#"<< Example {i}. >>
Data Source:
```json
{{
    "content": "{content}",
    "attributes": {attributes}
}}
```

User Query:
{query}

Structured Request:
"#;

/// Suffix without data source (for user-specified examples).
pub const SUFFIX_WITHOUT_DATA_SOURCE: &str = r"<< Example {i}. >>
User Query:
{query}

Structured Request:
";

/// Example prompt template.
pub const EXAMPLE_PROMPT_TEMPLATE: &str = r"<< Example {i}. >>
Data Source:
{data_source}

User Query:
{user_query}

Structured Request:
{structured_request}";

// ============================================================================
// Helper Functions
// ============================================================================

/// Format attribute information as JSON string for prompt.
///
/// Converts a list of `AttributeInfo` into a formatted JSON string with double-brace
/// escaping for use in string templates.
#[must_use]
pub fn format_attribute_info(attributes: &[AttributeInfo]) -> String {
    let mut info_map: HashMap<String, HashMap<String, serde_json::Value>> = HashMap::new();

    for attr in attributes {
        let mut attr_map = HashMap::new();
        if !attr.description.is_empty() {
            attr_map.insert(
                "description".to_string(),
                serde_json::Value::String(attr.description.clone()),
            );
        }
        if !attr.attr_type.is_empty() {
            attr_map.insert(
                "type".to_string(),
                serde_json::Value::String(attr.attr_type.clone()),
            );
        }
        info_map.insert(attr.name.clone(), attr_map);
    }

    let json_str = serde_json::to_string_pretty(&info_map).unwrap_or_default();

    // Escape braces for use in templates
    json_str.replace('{', "{{").replace('}', "}}")
}

/// Build a formatted prompt for query construction.
///
/// Creates a few-shot prompt that guides the LLM to generate structured queries.
///
/// # Arguments
///
/// * `document_contents` - Description of what the documents contain
/// * `attribute_info` - List of attributes with descriptions and types
/// * `allowed_comparators` - Which comparators are allowed (default: all)
/// * `allowed_operators` - Which operators are allowed (default: all)
/// * `enable_limit` - Whether to include limit in the output schema
///
/// # Returns
///
/// A formatted prompt string ready to be sent to an LLM.
///
/// # Example
///
/// ```rust,ignore
/// use dashflow::core::structured_query::query_constructor::get_query_constructor_prompt;
/// use dashflow::core::structured_query::{AttributeInfo, Comparator, Operator};
///
/// let attributes = vec![
///     AttributeInfo::new("category".to_string())
///         .with_description("The category of the product")
///         .with_type("string"),
///     AttributeInfo::new("price".to_string())
///         .with_description("Price in dollars")
///         .with_type("integer"),
/// ];
///
/// let prompt = get_query_constructor_prompt(
///     "Product catalog",
///     &attributes,
///     &[Comparator::Eq, Comparator::Gt, Comparator::Lt],
///     &[Operator::And, Operator::Or],
///     false,
/// );
/// ```
#[must_use]
pub fn get_query_constructor_prompt(
    document_contents: &str,
    attribute_info: &[AttributeInfo],
    allowed_comparators: &[Comparator],
    allowed_operators: &[Operator],
    enable_limit: bool,
) -> String {
    // Choose schema template based on limit flag
    let schema_template = if enable_limit {
        SCHEMA_WITH_LIMIT_PROMPT
    } else {
        DEFAULT_SCHEMA_PROMPT
    };

    // Format comparators and operators for schema
    let comparators_str = allowed_comparators
        .iter()
        .map(super::Comparator::value)
        .collect::<Vec<_>>()
        .join(" | ");

    let operators_str = allowed_operators
        .iter()
        .map(super::Operator::value)
        .collect::<Vec<_>>()
        .join(" | ");

    // Fill in schema template
    let schema = schema_template
        .replace("{allowed_comparators}", &comparators_str)
        .replace("{allowed_operators}", &operators_str);

    // Format attributes
    let attributes_str = format_attribute_info(attribute_info);

    // Build full prompt with prefix
    let prefix = DEFAULT_PREFIX.replace("{schema}", &schema);

    // Build suffix with document contents and attributes
    // Note: {query} is kept as a template variable for later replacement
    let suffix = DEFAULT_SUFFIX
        .replace("{i}", "1")
        .replace("{content}", document_contents)
        .replace("{attributes}", &attributes_str);

    format!("{prefix}\n\n{suffix}")
}

#[cfg(test)]
mod tests {
    use super::{fix_filter_directive, format_attribute_info, get_query_constructor_prompt};
    use crate::core::structured_query::{Comparison, Operation};
    use crate::test_prelude::*;
    use serde_json::json;

    #[test]
    fn test_structured_query_output_parser_basic() {
        let parser = StructuredQueryOutputParser::new();

        let text = r#"{
            "query": "science fiction books",
            "filter": "eq(\"category\", \"books\")"
        }"#;

        let result = parser.parse(text).unwrap();
        assert_eq!(result.query, "science fiction books");
        assert!(result.filter.is_some());
        assert_eq!(result.limit, None);
    }

    #[test]
    fn test_structured_query_output_parser_with_limit() {
        let parser = StructuredQueryOutputParser::new();

        let text = r#"{
            "query": "recent articles",
            "filter": "NO_FILTER",
            "limit": 10
        }"#;

        let result = parser.parse(text).unwrap();
        assert_eq!(result.query, "recent articles");
        assert!(result.filter.is_none());
        assert_eq!(result.limit, Some(10));
    }

    #[test]
    fn test_structured_query_output_parser_empty_query() {
        let parser = StructuredQueryOutputParser::new();

        let text = r#"{
            "query": "",
            "filter": "NO_FILTER"
        }"#;

        let result = parser.parse(text).unwrap();
        assert_eq!(result.query, " "); // Empty query becomes single space
        assert!(result.filter.is_none());
    }

    #[test]
    fn test_structured_query_output_parser_complex_filter() {
        let parser = StructuredQueryOutputParser::new();

        let text = r#"{
            "query": "books",
            "filter": "and(eq(\"category\", \"books\"), gt(\"price\", 10))"
        }"#;

        let result = parser.parse(text).unwrap();
        assert_eq!(result.query, "books");
        assert!(result.filter.is_some());

        // Verify it's an And operation
        if let Some(FilterDirective::Operation(op)) = result.filter {
            assert_eq!(op.operator, Operator::And);
            assert_eq!(op.arguments.len(), 2);
        } else {
            panic!("Expected Operation");
        }
    }

    #[test]
    fn test_structured_query_output_parser_markdown_json() {
        let parser = StructuredQueryOutputParser::new();

        let text = r#"```json
{
    "query": "laptops",
    "filter": "lt(\"price\", 1000)"
}
```"#;

        let result = parser.parse(text).unwrap();
        assert_eq!(result.query, "laptops");
        assert!(result.filter.is_some());
    }

    #[test]
    fn test_structured_query_output_parser_missing_keys() {
        let parser = StructuredQueryOutputParser::new();

        let text = r#"{"query": "test"}"#; // Missing filter key
        assert!(parser.parse(text).is_err());
    }

    #[test]
    fn test_structured_query_output_parser_invalid_json() {
        let parser = StructuredQueryOutputParser::new();
        assert!(parser.parse("not json").is_err());
    }

    #[test]
    fn test_structured_query_output_parser_with_allowed_comparators() {
        let parser =
            StructuredQueryOutputParser::new().with_allowed_comparators(vec![Comparator::Eq]);

        let text = r#"{
            "query": "test",
            "filter": "eq(\"field\", \"value\")"
        }"#;

        let result = parser.parse(text).unwrap();
        assert!(result.filter.is_some());

        // Try with disallowed comparator
        let text_invalid = r#"{
            "query": "test",
            "filter": "gt(\"field\", 10)"
        }"#;

        // Should fail because Gt is not allowed
        assert!(parser.parse(text_invalid).is_err());
    }

    #[test]
    fn test_fix_filter_directive_comparison() {
        let comp = FilterDirective::Comparison(Comparison {
            comparator: Comparator::Eq,
            attribute: "name".to_string(),
            value: json!("Alice"),
        });

        // Allowed comparator
        let result = fix_filter_directive(
            comp.clone(),
            Some(&[Comparator::Eq, Comparator::Ne]),
            None,
            None,
        );
        assert!(result.is_some());

        // Disallowed comparator
        let result = fix_filter_directive(
            comp.clone(),
            Some(&[Comparator::Gt, Comparator::Lt]),
            None,
            None,
        );
        assert!(result.is_none());

        // Disallowed attribute
        let result = fix_filter_directive(
            comp,
            None,
            None,
            Some(&["age".to_string(), "city".to_string()]),
        );
        assert!(result.is_none());
    }

    #[test]
    fn test_fix_filter_directive_operation() {
        let op = FilterDirective::Operation(Operation {
            operator: Operator::And,
            arguments: vec![
                FilterDirective::Comparison(Comparison {
                    comparator: Comparator::Eq,
                    attribute: "name".to_string(),
                    value: json!("Alice"),
                }),
                FilterDirective::Comparison(Comparison {
                    comparator: Comparator::Gt,
                    attribute: "age".to_string(),
                    value: json!(18),
                }),
            ],
        });

        // Allow only Eq comparator - should remove Gt comparison
        let result = fix_filter_directive(op.clone(), Some(&[Comparator::Eq]), None, None);

        assert!(result.is_some());
        // Should have simplified to just the Eq comparison (single arg And is simplified)
        if let Some(FilterDirective::Comparison(comp)) = result {
            assert_eq!(comp.comparator, Comparator::Eq);
        } else {
            panic!("Expected simplified Comparison");
        }
    }

    #[test]
    fn test_fix_filter_directive_empty_operation() {
        let op = FilterDirective::Operation(Operation {
            operator: Operator::And,
            arguments: vec![FilterDirective::Comparison(Comparison {
                comparator: Comparator::Gt,
                attribute: "age".to_string(),
                value: json!(18),
            })],
        });

        // Disallow Gt - operation should become empty and return None
        let result = fix_filter_directive(op, Some(&[Comparator::Eq]), None, None);

        assert!(result.is_none());
    }

    #[test]
    fn test_format_attribute_info() {
        let attributes = vec![
            AttributeInfo::new(
                "name".to_string(),
                "Person's name".to_string(),
                "string".to_string(),
            ),
            AttributeInfo::new(
                "age".to_string(),
                "Person's age".to_string(),
                "integer".to_string(),
            ),
        ];

        let formatted = format_attribute_info(&attributes);

        // Should contain escaped braces
        assert!(formatted.contains("{{"));
        assert!(formatted.contains("}}"));

        // Should contain attribute names and descriptions
        assert!(formatted.contains("name"));
        assert!(formatted.contains("age"));
        assert!(formatted.contains("Person's name"));
        assert!(formatted.contains("Person's age"));
    }

    #[test]
    fn test_get_query_constructor_prompt_basic() {
        let attributes = vec![AttributeInfo::new(
            "category".to_string(),
            "Product category".to_string(),
            "string".to_string(),
        )];

        let prompt = get_query_constructor_prompt(
            "Product catalog",
            &attributes,
            &[Comparator::Eq, Comparator::Ne],
            &[Operator::And, Operator::Or],
            false,
        );

        // Should contain schema instructions
        assert!(prompt.contains("Structured Request Schema"));
        assert!(prompt.contains("eq | ne")); // Comparators
        assert!(prompt.contains("and | or")); // Operators

        // Should contain document contents
        assert!(prompt.contains("Product catalog"));

        // Should contain attribute information
        assert!(prompt.contains("category"));
        assert!(prompt.contains("Product category"));
    }

    #[test]
    fn test_get_query_constructor_prompt_with_limit() {
        let attributes = vec![AttributeInfo::new(
            "title".to_string(),
            "Document title".to_string(),
            "string".to_string(),
        )];

        let prompt = get_query_constructor_prompt(
            "Article database",
            &attributes,
            &[Comparator::Eq],
            &[Operator::And],
            true, // Enable limit
        );

        // Should contain limit in schema
        assert!(prompt.contains("\"limit\": int"));
    }

    #[test]
    fn test_get_query_constructor_prompt_all_comparators() {
        let attributes = vec![AttributeInfo::new(
            "price".to_string(),
            "Product price".to_string(),
            "integer".to_string(),
        )];

        let all_comparators = Comparator::all();
        let all_operators = Operator::all();

        let prompt = get_query_constructor_prompt(
            "Products",
            &attributes,
            &all_comparators,
            &all_operators,
            false,
        );

        // Should contain all comparator types
        assert!(prompt.contains("eq"));
        assert!(prompt.contains("ne"));
        assert!(prompt.contains("gt"));
        assert!(prompt.contains("lt"));

        // Should contain all operator types
        assert!(prompt.contains("and"));
        assert!(prompt.contains("or"));
        assert!(prompt.contains("not"));
    }
}
