// Allow clippy warnings for output parsers
// - panic: panic!() in parser error paths for invalid input detection
#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::clone_on_ref_ptr,
    clippy::panic
)]

//! Output parsers for structuring language model responses.
//!
//! Output parsers help convert raw text output from language models into structured formats.
//! They implement the `Runnable` trait, making them composable in LCEL chains.
//!
//! # Available Parsers
//!
//! - **`StrOutputParser`**: Pass-through parser that returns the input string unchanged
//! - **`JsonOutputParser`**: Parses JSON output with optional Markdown code block support
//! - **`CommaSeparatedListOutputParser`**: Parses comma-separated values into a list
//! - **`NumberedListOutputParser`**: Parses numbered lists (1. item, 2. item)
//! - **`MarkdownListOutputParser`**: Parses Markdown bullet lists (- item, * item)
//! - **`LineListOutputParser`**: Parses newline-separated lists (one item per line)
//! - **`QuestionListOutputParser`**: Parses numbered question lists for search query generation
//! - **`XMLOutputParser`**: Parses XML-formatted output into a nested structure
//! - **`DatetimeOutputParser`**: Parses datetime strings into `DateTime<Utc>` objects
//! - **`YamlOutputParser`**: Parses YAML output into `serde_json::Value` with schema validation
//! - **`BooleanOutputParser`**: Parses boolean responses (YES/NO, true/false, custom values)
//! - **`EnumOutputParser`**: Parses one value from a set of allowed string values
//! - **`RegexParser`**: Extracts named groups from text using regex patterns
//! - **`RegexDictParser`**: Extracts multiple key-value pairs using a template pattern
//! - **`OutputFixingParser`**: Wraps another parser and automatically retries with LLM feedback on errors
//!
//! # Example
//!
//! ```rust,ignore
//! use dashflow::core::output_parsers::StrOutputParser;
//! use dashflow::core::runnable::Runnable;
//!
//! # async fn example() {
//! let parser = StrOutputParser;
//! let result = parser.invoke("Hello, world!".to_string(), None).await.unwrap();
//! assert_eq!(result, "Hello, world!");
//! # }
//! ```

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use std::collections::HashMap;
use std::pin::Pin;
use std::sync::Arc;

use crate::constants::{REGEX_DFA_SIZE_LIMIT, REGEX_SIZE_LIMIT};
use crate::core::config::RunnableConfig;
use crate::core::deserialization::{
    extract_serialized_fields, get_optional_string, validate_id, Deserializable,
};
use crate::core::error::{Error, Result};

/// Compile a regex pattern with size limits to prevent resource exhaustion.
/// Uses centralized constants from `crate::constants`.
fn compile_bounded_regex(pattern: &str) -> std::result::Result<regex::Regex, regex::Error> {
    regex::RegexBuilder::new(pattern)
        .size_limit(REGEX_SIZE_LIMIT)
        .dfa_size_limit(REGEX_DFA_SIZE_LIMIT)
        .build()
}
use crate::core::language_models::Generation;
use crate::core::runnable::Runnable;
use crate::core::serialization::{Serializable, SerializedObject, SERIALIZATION_VERSION};

// Submodules
mod list_parsers;

// Re-export list parsers
pub use list_parsers::{
    CommaSeparatedListOutputParser, LineListOutputParser, MarkdownListOutputParser,
    NumberedListOutputParser, QuestionListOutputParser,
};

/// Trait for parsing language model outputs into structured formats.
///
/// Output parsers transform raw text from language models into application-specific
/// types. They implement `Runnable<String, Output>` for seamless composition in chains.
///
/// # Type Parameters
///
/// - `Output`: The structured output type this parser produces
///
/// # Implementing Custom Parsers
///
/// To create a custom output parser, implement this trait and provide:
/// - `parse`: Core parsing logic from string to your output type
/// - `get_format_instructions`: Instructions for the LLM on how to format output
///
/// # Example
///
/// ```rust
/// use dashflow::core::output_parsers::OutputParser;
/// use dashflow::core::error::{Error, Result};
///
/// struct BooleanParser {
///     true_val: String,
///     false_val: String,
/// }
///
/// impl OutputParser for BooleanParser {
///     type Output = bool;
///
///     fn parse(&self, text: &str) -> Result<Self::Output> {
///         let cleaned = text.trim().to_uppercase();
///         if cleaned == self.true_val.to_uppercase() {
///             Ok(true)
///         } else if cleaned == self.false_val.to_uppercase() {
///             Ok(false)
///         } else {
///             Err(Error::OutputParsing(format!(
///                 "Expected '{}' or '{}', got '{}'",
///                 self.true_val, self.false_val, text
///             )))
///         }
///     }
///
///     fn get_format_instructions(&self) -> String {
///         format!("Respond with either '{}' or '{}'", self.true_val, self.false_val)
///     }
/// }
/// ```
pub trait OutputParser: Send + Sync {
    /// The structured output type produced by this parser
    type Output: Send + 'static;

    /// Parse a string into the structured output format.
    ///
    /// # Arguments
    ///
    /// - `text`: Raw text output from a language model
    ///
    /// # Returns
    ///
    /// Structured output of type `Self::Output`
    ///
    /// # Errors
    ///
    /// Returns `Error::ParseError` if the text cannot be parsed into the expected format
    fn parse(&self, text: &str) -> Result<Self::Output>;

    /// Parse a list of generations, using the first one.
    ///
    /// This is the default implementation that extracts text from the first generation
    /// and calls `parse`. Override this if you need different behavior.
    ///
    /// # Arguments
    ///
    /// - `generations`: List of candidate generations from a language model
    ///
    /// # Returns
    ///
    /// Structured output from parsing the first generation
    fn parse_result(&self, generations: &[Generation]) -> Result<Self::Output> {
        if generations.is_empty() {
            return Err(Error::OutputParsing(
                "No generations provided to parse".to_string(),
            ));
        }
        self.parse(&generations[0].text)
    }

    /// Get format instructions for the language model.
    ///
    /// These instructions tell the LLM how to format its output so this parser
    /// can successfully parse it. Include these in your prompts.
    ///
    /// # Returns
    ///
    /// Human-readable formatting instructions
    ///
    /// # Example
    ///
    /// ```rust
    /// use dashflow::core::output_parsers::{OutputParser, CommaSeparatedListOutputParser};
    ///
    /// let parser = CommaSeparatedListOutputParser;
    /// println!("Instructions: {}", parser.get_format_instructions());
    /// // Prints: "Your response should be a list of comma separated values, eg: `foo, bar, baz`"
    /// ```
    fn get_format_instructions(&self) -> String {
        String::new()
    }
}

/// String output parser that returns the input text unchanged.
///
/// This is the simplest output parser - it performs no transformation on the input.
/// Useful as a default parser or when you want raw model output.
///
/// # Example
///
/// ```rust
/// use dashflow::core::output_parsers::{OutputParser, StrOutputParser};
///
/// let parser = StrOutputParser;
/// let result = parser.parse("Hello, world!").unwrap();
/// assert_eq!(result, "Hello, world!");
/// ```
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct StrOutputParser;

impl OutputParser for StrOutputParser {
    type Output = String;

    fn parse(&self, text: &str) -> Result<Self::Output> {
        Ok(text.to_string())
    }
}

#[async_trait]
impl Runnable for StrOutputParser {
    type Input = String;
    type Output = String;

    async fn invoke(
        &self,
        input: Self::Input,
        _config: Option<RunnableConfig>,
    ) -> Result<Self::Output> {
        self.parse(&input)
    }

    async fn batch(
        &self,
        inputs: Vec<Self::Input>,
        _config: Option<RunnableConfig>,
    ) -> Result<Vec<Self::Output>> {
        inputs.iter().map(|input| self.parse(input)).collect()
    }

    async fn stream(
        &self,
        input: Self::Input,
        _config: Option<RunnableConfig>,
    ) -> Result<Pin<Box<dyn futures::Stream<Item = Result<Self::Output>> + Send>>> {
        let result = self.parse(&input)?;
        Ok(Box::pin(futures::stream::once(async move { Ok(result) })))
    }
}

impl Serializable for StrOutputParser {
    fn lc_id(&self) -> Vec<String> {
        vec![
            "dashflow".to_string(),
            "output_parsers".to_string(),
            "string".to_string(),
            "StrOutputParser".to_string(),
        ]
    }

    fn is_lc_serializable(&self) -> bool {
        true
    }

    fn to_json(&self) -> SerializedObject {
        // StrOutputParser has no configuration
        SerializedObject::Constructor {
            lc: SERIALIZATION_VERSION,
            id: self.lc_id(),
            kwargs: serde_json::json!({}),
        }
    }

    fn lc_secrets(&self) -> HashMap<String, String> {
        HashMap::new()
    }
}

impl Deserializable for StrOutputParser {
    fn from_json(value: &JsonValue) -> Result<Self> {
        let (_lc, id, _kwargs) = extract_serialized_fields(value)?;
        validate_id(
            &id,
            &[
                "dashflow".to_string(),
                "output_parsers".to_string(),
                "string".to_string(),
                "StrOutputParser".to_string(),
            ],
        )?;
        Ok(StrOutputParser)
    }
}

/// JSON output parser with support for Markdown code blocks.
///
/// Parses JSON from model output, automatically stripping Markdown code fences
/// if present (e.g., ```json ... ```).
///
/// # Example
///
/// ```rust
/// use dashflow::core::output_parsers::{OutputParser, JsonOutputParser};
/// use serde_json::json;
///
/// let parser = JsonOutputParser::new();
///
/// // Parse plain JSON
/// let result = parser.parse(r#"{"name": "Alice", "age": 30}"#).unwrap();
/// assert_eq!(result["name"], "Alice");
///
/// // Parse JSON in Markdown code blocks
/// let markdown = r#"```json
/// {"name": "Bob", "age": 25}
/// ```"#;
/// let result = parser.parse(markdown).unwrap();
/// assert_eq!(result["name"], "Bob");
/// ```
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct JsonOutputParser;

impl JsonOutputParser {
    /// Create a new JSON output parser.
    #[must_use]
    pub fn new() -> Self {
        Self
    }

    /// Parse JSON from text, stripping Markdown code fences if present.
    fn parse_json_markdown(&self, text: &str) -> Result<JsonValue> {
        let text = text.trim();

        // Try to extract JSON from Markdown code blocks
        if text.starts_with("```") {
            // Handle inline case: ```{...}```
            if !text.contains('\n') {
                let json_text = text
                    .strip_prefix("```json")
                    .or_else(|| text.strip_prefix("```"))
                    .and_then(|s| s.strip_suffix("```"))
                    .unwrap_or(text);

                return serde_json::from_str(json_text.trim())
                    .map_err(|e| Error::OutputParsing(format!("Invalid JSON: {e}")));
            }

            // Multi-line case: extract content between fences
            let lines: Vec<&str> = text.lines().collect();

            // Extract JSON content - skip opening and closing ```
            let start = 1; // Skip first line (```json or ```)
            let end = if lines.len() > 1 && lines[lines.len() - 1].trim() == "```" {
                lines.len() - 1
            } else {
                lines.len()
            };

            if start >= end {
                return Err(Error::OutputParsing(
                    "Invalid Markdown code block".to_string(),
                ));
            }

            let json_text = lines[start..end].join("\n");

            serde_json::from_str(&json_text)
                .map_err(|e| Error::OutputParsing(format!("Invalid JSON: {e}")))
        } else {
            serde_json::from_str(text)
                .map_err(|e| Error::OutputParsing(format!("Invalid JSON: {e}")))
        }
    }
}

impl OutputParser for JsonOutputParser {
    type Output = JsonValue;

    fn parse(&self, text: &str) -> Result<Self::Output> {
        self.parse_json_markdown(text)
    }

    fn get_format_instructions(&self) -> String {
        "Return a JSON object.".to_string()
    }
}

#[async_trait]
impl Runnable for JsonOutputParser {
    type Input = String;
    type Output = JsonValue;

    async fn invoke(
        &self,
        input: Self::Input,
        _config: Option<RunnableConfig>,
    ) -> Result<Self::Output> {
        self.parse(&input)
    }

    async fn batch(
        &self,
        inputs: Vec<Self::Input>,
        _config: Option<RunnableConfig>,
    ) -> Result<Vec<Self::Output>> {
        inputs.iter().map(|input| self.parse(input)).collect()
    }

    async fn stream(
        &self,
        input: Self::Input,
        _config: Option<RunnableConfig>,
    ) -> Result<Pin<Box<dyn futures::Stream<Item = Result<Self::Output>> + Send>>> {
        let result = self.parse(&input)?;
        Ok(Box::pin(futures::stream::once(async move { Ok(result) })))
    }
}

impl Serializable for JsonOutputParser {
    fn lc_id(&self) -> Vec<String> {
        vec![
            "dashflow".to_string(),
            "output_parsers".to_string(),
            "json".to_string(),
            "JsonOutputParser".to_string(),
        ]
    }

    fn is_lc_serializable(&self) -> bool {
        true
    }

    fn to_json(&self) -> SerializedObject {
        // JsonOutputParser has no configuration
        SerializedObject::Constructor {
            lc: SERIALIZATION_VERSION,
            id: self.lc_id(),
            kwargs: serde_json::json!({}),
        }
    }

    fn lc_secrets(&self) -> HashMap<String, String> {
        HashMap::new()
    }
}

impl Deserializable for JsonOutputParser {
    fn from_json(value: &JsonValue) -> Result<Self> {
        let (_lc, id, _kwargs) = extract_serialized_fields(value)?;
        validate_id(
            &id,
            &[
                "dashflow".to_string(),
                "output_parsers".to_string(),
                "json".to_string(),
                "JsonOutputParser".to_string(),
            ],
        )?;
        Ok(JsonOutputParser)
    }
}

// ================================================================================================
// XMLOutputParser - Parses XML-formatted LLM outputs
// ================================================================================================

/// Parses XML-formatted output from language models into a nested structure.
///
/// This parser extracts XML from LLM responses (including XML in Markdown code blocks),
/// parses it, and converts it to a nested `HashMap` structure. It handles both simple
/// text content and nested XML elements.
///
/// # XML Structure
///
/// - Elements with only text content: `{tag: text}`
/// - Elements with children: `{tag: [child1, child2, ...]}`
/// - Mixed content is simplified to child elements only
///
/// # Example
///
/// ```rust
/// use dashflow::core::output_parsers::{OutputParser, XMLOutputParser};
/// use std::collections::HashMap;
///
/// let parser = XMLOutputParser::new();
///
/// // Simple XML with text content
/// let xml = "<person><name>Alice</name><age>30</age></person>";
/// let result = parser.parse(xml).unwrap();
///
/// // result is a nested HashMap: {"person": [{"name": "Alice"}, {"age": "30"}]}
/// ```
/// Maximum recursion depth for XML parsing to prevent stack overflow.
///
/// This limit prevents maliciously crafted deeply-nested XML from causing
/// stack overflow. 100 levels should be sufficient for any legitimate use case.
const MAX_XML_DEPTH: usize = 100;

/// # Format Instructions
///
/// The parser can provide format instructions to guide the LLM. When `tags` are specified,
/// it generates instructions about which XML tags to use in the output.
#[derive(Debug, Clone)]
pub struct XMLOutputParser {
    /// Optional list of tags to guide the LLM output format.
    /// When provided, these are included in format instructions.
    pub tags: Option<Vec<String>>,
    /// Cached regex for extracting XML from Markdown code blocks
    #[allow(clippy::type_complexity)]
    // Lazy-init regex cache: Arc<OnceLock<Regex>> for thread-safe sharing
    cached_markdown_regex: std::sync::Arc<std::sync::OnceLock<regex::Regex>>,
}

impl XMLOutputParser {
    /// Create a new `XMLOutputParser` without tag hints.
    #[must_use]
    pub fn new() -> Self {
        XMLOutputParser {
            tags: None,
            cached_markdown_regex: std::sync::Arc::new(std::sync::OnceLock::new()),
        }
    }

    /// Create a new `XMLOutputParser` with expected tags for format instructions.
    ///
    /// # Arguments
    ///
    /// * `tags` - List of XML tags the LLM should use in its output
    #[must_use]
    pub fn with_tags(tags: Vec<String>) -> Self {
        XMLOutputParser {
            tags: Some(tags),
            cached_markdown_regex: std::sync::Arc::new(std::sync::OnceLock::new()),
        }
    }

    /// Extract XML from text, handling Markdown code blocks.
    ///
    /// Looks for XML in:
    /// 1. Markdown code blocks (```xml or ```)
    /// 2. Raw XML in the text
    fn extract_xml(&self, text: &str) -> String {
        // Try to find XML within triple backticks (cached regex compilation)
        let markdown_pattern = self.cached_markdown_regex.get_or_init(|| {
            regex::Regex::new(r"```(?:xml)?\s*(.*?)\s*```")
                .expect("Invalid XML markdown regex pattern")
        });
        if let Some(captures) = markdown_pattern.captures(text) {
            if let Some(m) = captures.get(1) {
                return m.as_str().trim().to_string();
            }
        }

        // Return trimmed text as-is (assume it's raw XML)
        text.trim().to_string()
    }

    /// Parse XML element into a `HashMap` structure.
    ///
    /// Converts XML elements to nested `HashMaps`:
    /// - Text-only elements: {tag: `text_content`}
    /// - Elements with children: {tag: [child1, child2, ...]}
    ///
    /// This is a wrapper that starts parsing at depth 0.
    fn element_to_dict(
        element: &quick_xml::events::BytesStart,
        reader: &mut quick_xml::Reader<&[u8]>,
    ) -> Result<HashMap<String, serde_json::Value>> {
        Self::element_to_dict_with_depth(element, reader, 0)
    }

    /// Parse XML element with depth tracking to prevent stack overflow.
    ///
    /// # Arguments
    /// * `element` - The XML element to parse
    /// * `reader` - The XML reader
    /// * `depth` - Current recursion depth (0 at root)
    ///
    /// # Errors
    /// Returns error if depth exceeds `MAX_XML_DEPTH` or on XML parsing errors.
    fn element_to_dict_with_depth(
        element: &quick_xml::events::BytesStart,
        reader: &mut quick_xml::Reader<&[u8]>,
        depth: usize,
    ) -> Result<HashMap<String, serde_json::Value>> {
        use quick_xml::events::Event;

        if depth > MAX_XML_DEPTH {
            return Err(Error::OutputParsing(format!(
                "XML nesting too deep: {} levels (maximum is {})",
                depth, MAX_XML_DEPTH
            )));
        }

        let tag_name = String::from_utf8_lossy(element.name().as_ref()).to_string();
        let mut text_content = String::new();
        let mut children: Vec<serde_json::Value> = Vec::new();

        loop {
            match reader.read_event() {
                Ok(Event::Start(ref e)) => {
                    // Nested element - recursively parse with incremented depth
                    let child_dict = Self::element_to_dict_with_depth(e, reader, depth + 1)?;
                    children.push(serde_json::json!(child_dict));
                }
                Ok(Event::Empty(_)) => {
                    // Self-closing tag - skip
                }
                Ok(Event::Text(ref e)) => {
                    // Text content
                    let text = e.unescape().map_err(|e| {
                        Error::OutputParsing(format!("Failed to unescape XML text: {e}"))
                    })?;
                    text_content.push_str(&text);
                }
                Ok(Event::End(ref e)) => {
                    // End of this element
                    let end_tag = String::from_utf8_lossy(e.name().as_ref()).to_string();
                    if end_tag == tag_name {
                        // Decide what to return based on content
                        let mut result = HashMap::new();

                        if !children.is_empty() {
                            // Has children - return as array
                            result.insert(tag_name, serde_json::json!(children));
                        } else if !text_content.trim().is_empty() {
                            // Has text content only
                            result.insert(tag_name, serde_json::json!(text_content.trim()));
                        } else {
                            // Empty element
                            result.insert(tag_name, serde_json::Value::Null);
                        }

                        return Ok(result);
                    }
                    return Err(Error::OutputParsing(format!(
                        "Mismatched XML tags: expected </{tag_name}>, got </{end_tag}>"
                    )));
                }
                Ok(Event::Eof) => {
                    return Err(Error::OutputParsing(format!(
                        "Unexpected EOF while parsing element <{tag_name}>"
                    )));
                }
                Err(e) => {
                    return Err(Error::OutputParsing(format!("XML parsing error: {e}")));
                }
                _ => {} // Ignore other events (comments, declarations, etc.)
            }
        }
    }
}

impl Default for XMLOutputParser {
    fn default() -> Self {
        Self::new()
    }
}

impl OutputParser for XMLOutputParser {
    type Output = HashMap<String, serde_json::Value>;

    fn parse(&self, text: &str) -> Result<Self::Output> {
        use quick_xml::events::Event;
        use quick_xml::Reader;

        // Extract XML from text (handle markdown code blocks)
        let xml_text = self.extract_xml(text);

        // Create XML reader
        let mut reader = Reader::from_str(&xml_text);
        reader.config_mut().trim_text(true);

        // Find the root element and parse it
        loop {
            match reader.read_event() {
                Ok(Event::Start(ref e)) => {
                    // Found root element - parse it
                    return Self::element_to_dict(e, &mut reader);
                }
                Ok(Event::Empty(ref e)) => {
                    // Self-closing root element
                    let tag_name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                    let mut result = HashMap::new();
                    result.insert(tag_name, serde_json::Value::Null);
                    return Ok(result);
                }
                Ok(Event::Eof) => {
                    return Err(Error::OutputParsing(
                        "No root element found in XML".to_string(),
                    ));
                }
                Ok(Event::Decl(_) | Event::Comment(_) | Event::PI(_)) => {
                    // Skip XML declaration, comments, and processing instructions
                    continue;
                }
                Err(e) => {
                    return Err(Error::OutputParsing(format!(
                        "Failed to parse XML: {e}. Text was: {xml_text}"
                    )));
                }
                _ => {}
            }
        }
    }

    fn get_format_instructions(&self) -> String {
        const XML_FORMAT_INSTRUCTIONS: &str = r#"The output should be formatted as a XML file.
1. Output should conform to the tags below.
2. If tags are not given, make them on your own.
3. Remember to always open and close all the tags.

As an example, for the tags ["foo", "bar", "baz"]:
1. String "<foo>\n   <bar>\n      <baz></baz>\n   </bar>\n</foo>" is a well-formatted instance of the schema.
2. String "<foo>\n   <bar>\n   </foo>" is a badly-formatted instance.
3. String "<foo>\n   <tag>\n   </tag>\n</foo>" is a badly-formatted instance.

Here are the output tags:
```
{tags}
```"#;

        if let Some(ref tags) = self.tags {
            XML_FORMAT_INSTRUCTIONS.replace("{tags}", &tags.join("\n"))
        } else {
            XML_FORMAT_INSTRUCTIONS.replace("{tags}", "(No specific tags required)")
        }
    }
}

#[async_trait]
impl Runnable for XMLOutputParser {
    type Input = String;
    type Output = HashMap<String, serde_json::Value>;

    async fn invoke(
        &self,
        input: Self::Input,
        _config: Option<RunnableConfig>,
    ) -> Result<Self::Output> {
        self.parse(&input)
    }

    async fn batch(
        &self,
        inputs: Vec<Self::Input>,
        _config: Option<RunnableConfig>,
    ) -> Result<Vec<Self::Output>> {
        inputs.iter().map(|input| self.parse(input)).collect()
    }

    async fn stream(
        &self,
        input: Self::Input,
        _config: Option<RunnableConfig>,
    ) -> Result<Pin<Box<dyn futures::Stream<Item = Result<Self::Output>> + Send>>> {
        let result = self.parse(&input)?;
        Ok(Box::pin(futures::stream::once(async move { Ok(result) })))
    }
}

impl Serializable for XMLOutputParser {
    fn lc_id(&self) -> Vec<String> {
        vec![
            "dashflow".to_string(),
            "output_parsers".to_string(),
            "xml".to_string(),
            "XMLOutputParser".to_string(),
        ]
    }

    fn is_lc_serializable(&self) -> bool {
        true
    }

    fn to_json(&self) -> SerializedObject {
        let mut kwargs = serde_json::Map::new();
        if let Some(ref tags) = self.tags {
            kwargs.insert("tags".to_string(), serde_json::json!(tags));
        }

        SerializedObject::Constructor {
            lc: SERIALIZATION_VERSION,
            id: self.lc_id(),
            kwargs: serde_json::Value::Object(kwargs),
        }
    }

    fn lc_secrets(&self) -> HashMap<String, String> {
        HashMap::new()
    }
}

impl Deserializable for XMLOutputParser {
    fn from_json(value: &JsonValue) -> Result<Self> {
        let (_lc, id, kwargs) = extract_serialized_fields(value)?;
        validate_id(
            &id,
            &[
                "dashflow".to_string(),
                "output_parsers".to_string(),
                "xml".to_string(),
                "XMLOutputParser".to_string(),
            ],
        )?;

        let tags: Option<Vec<String>> = kwargs
            .get("tags")
            .and_then(|v| serde_json::from_value(v.clone()).ok());

        Ok(XMLOutputParser {
            tags,
            cached_markdown_regex: std::sync::Arc::new(std::sync::OnceLock::new()),
        })
    }
}

/// Datetime output parser for parsing datetime strings.
///
/// Parses datetime strings from model output using a configurable format string.
/// Defaults to ISO 8601 format: "%Y-%m-%dT%H:%M:%S%.fZ"
///
/// # Example
///
/// ```rust
/// use dashflow::core::output_parsers::{OutputParser, DatetimeOutputParser};
/// use chrono::Datelike;
///
/// let parser = DatetimeOutputParser::new();
///
/// // Parse ISO 8601 datetime
/// let result = parser.parse("2023-07-04T14:30:00.000000Z").unwrap();
/// assert_eq!(result.year(), 2023);
/// assert_eq!(result.month(), 7);
/// assert_eq!(result.day(), 4);
///
/// // Custom format
/// let custom = DatetimeOutputParser::with_format("%Y-%m-%d");
/// let result2 = custom.parse("2023-07-04").unwrap();
/// assert_eq!(result2.year(), 2023);
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatetimeOutputParser {
    /// The datetime format string (using chrono format syntax)
    pub format: String,
}

impl DatetimeOutputParser {
    /// Creates a new `DatetimeOutputParser` with ISO 8601 format.
    #[must_use]
    pub fn new() -> Self {
        Self {
            format: "%Y-%m-%dT%H:%M:%S%.fZ".to_string(),
        }
    }

    /// Creates a `DatetimeOutputParser` with a custom format string.
    #[must_use]
    pub fn with_format(format: impl Into<String>) -> Self {
        Self {
            format: format.into(),
        }
    }

    fn format_examples(&self) -> String {
        if self.format == "%Y-%m-%dT%H:%M:%S%.fZ" {
            // Default format - use hardcoded examples
            "2023-07-04T14:30:00.000000Z, 1999-12-31T23:59:59.999999Z, or 2025-01-01T00:00:00.000000Z".to_string()
        } else {
            // Generate examples from current time
            let now = Utc::now();
            let example1 = now.format(&self.format).to_string();
            let example2 = (now - chrono::Duration::days(365))
                .format(&self.format)
                .to_string();
            let example3 = (now - chrono::Duration::days(1))
                .format(&self.format)
                .to_string();
            format!("{example1}, {example2}, or {example3}")
        }
    }
}

impl Default for DatetimeOutputParser {
    fn default() -> Self {
        Self::new()
    }
}

impl OutputParser for DatetimeOutputParser {
    type Output = DateTime<Utc>;

    fn parse(&self, text: &str) -> Result<Self::Output> {
        // Try parsing with timezone first
        if let Ok(dt) = DateTime::parse_from_str(text.trim(), &self.format) {
            return Ok(dt.with_timezone(&Utc));
        }

        // Try parsing as naive datetime and assume UTC
        if let Ok(ndt) = chrono::NaiveDateTime::parse_from_str(text.trim(), &self.format) {
            return Ok(DateTime::<Utc>::from_naive_utc_and_offset(ndt, Utc));
        }

        // Try parsing as date-only format and set time to midnight UTC
        if let Ok(date) = chrono::NaiveDate::parse_from_str(text.trim(), &self.format) {
            if let Some(ndt) = date.and_hms_opt(0, 0, 0) {
                return Ok(DateTime::<Utc>::from_naive_utc_and_offset(ndt, Utc));
            }
        }

        Err(Error::OutputParsing(format!(
            "Could not parse datetime string: {text}"
        )))
    }

    fn get_format_instructions(&self) -> String {
        format!(
            "Write a datetime string that matches the following pattern: '{}'.\n\n\
             Examples: {}\n\n\
             Return ONLY this string, no other words!",
            self.format,
            self.format_examples()
        )
    }
}

#[async_trait]
impl Runnable for DatetimeOutputParser {
    type Input = String;
    type Output = DateTime<Utc>;

    async fn invoke(
        &self,
        input: Self::Input,
        _config: Option<RunnableConfig>,
    ) -> Result<Self::Output> {
        self.parse(&input)
    }

    async fn batch(
        &self,
        inputs: Vec<Self::Input>,
        _config: Option<RunnableConfig>,
    ) -> Result<Vec<Self::Output>> {
        inputs.iter().map(|input| self.parse(input)).collect()
    }

    async fn stream(
        &self,
        input: Self::Input,
        _config: Option<RunnableConfig>,
    ) -> Result<Pin<Box<dyn futures::Stream<Item = Result<Self::Output>> + Send>>> {
        let result = self.parse(&input)?;
        Ok(Box::pin(futures::stream::once(async move { Ok(result) })))
    }
}

impl Serializable for DatetimeOutputParser {
    fn lc_id(&self) -> Vec<String> {
        vec![
            "dashflow".to_string(),
            "output_parsers".to_string(),
            "datetime".to_string(),
            "DatetimeOutputParser".to_string(),
        ]
    }

    fn is_lc_serializable(&self) -> bool {
        true
    }

    fn to_json(&self) -> SerializedObject {
        SerializedObject::Constructor {
            lc: SERIALIZATION_VERSION,
            id: self.lc_id(),
            kwargs: serde_json::json!({
                "format": self.format
            }),
        }
    }

    fn lc_secrets(&self) -> HashMap<String, String> {
        HashMap::new()
    }
}

impl Deserializable for DatetimeOutputParser {
    fn from_json(value: &JsonValue) -> Result<Self> {
        let (_lc, id, kwargs) = extract_serialized_fields(value)?;
        validate_id(
            &id,
            &[
                "dashflow".to_string(),
                "output_parsers".to_string(),
                "datetime".to_string(),
                "DatetimeOutputParser".to_string(),
            ],
        )?;

        let format = get_optional_string(kwargs, "format")?
            .unwrap_or_else(|| "%Y-%m-%dT%H:%M:%S%.fZ".to_string());

        Ok(DatetimeOutputParser { format })
    }
}

/// YAML output parser with Markdown code block support.
///
/// Parses YAML from model output, automatically stripping Markdown code fences
/// if present (e.g., ```yaml ... ``` or ```yml ... ```). Returns a `serde_json::Value`
/// for flexible handling of the parsed structure.
///
/// # Example
///
/// ```rust
/// use dashflow::core::output_parsers::{OutputParser, YamlOutputParser};
/// use serde_json::json;
///
/// let parser = YamlOutputParser::new();
///
/// // Parse plain YAML
/// let result = parser.parse("name: Alice\nage: 30").unwrap();
/// assert_eq!(result["name"], "Alice");
/// assert_eq!(result["age"], 30);
///
/// // Parse YAML in Markdown code blocks
/// let result2 = parser.parse("```yaml\nname: Bob\nage: 25\n```").unwrap();
/// assert_eq!(result2["name"], "Bob");
/// ```
#[derive(Debug, Clone)]
pub struct YamlOutputParser {
    /// Optional schema string for format instructions (JSON Schema format)
    pub schema: Option<String>,
    /// Cached regex for extracting YAML from Markdown code blocks
    #[allow(clippy::type_complexity)]
    // Lazy-init regex cache: Arc<OnceLock<Regex>> for thread-safe sharing
    cached_markdown_regex: std::sync::Arc<std::sync::OnceLock<regex::Regex>>,
}

impl YamlOutputParser {
    /// Creates a new `YamlOutputParser` without schema validation.
    #[must_use]
    pub fn new() -> Self {
        Self {
            schema: None,
            cached_markdown_regex: std::sync::Arc::new(std::sync::OnceLock::new()),
        }
    }

    /// Creates a `YamlOutputParser` with a JSON schema for format instructions.
    #[must_use]
    pub fn with_schema(schema: impl Into<String>) -> Self {
        Self {
            schema: Some(schema.into()),
            cached_markdown_regex: std::sync::Arc::new(std::sync::OnceLock::new()),
        }
    }

    fn extract_yaml_from_markdown<'a>(&self, text: &'a str) -> &'a str {
        // Regex to match ```yaml or ```yml code blocks (cached compilation)
        let re = self.cached_markdown_regex.get_or_init(|| {
            regex::Regex::new(r"(?ms)^```(?:ya?ml)?\s*\n(?P<yaml>.*?)```")
                .expect("Invalid YAML markdown regex pattern")
        });

        if let Some(captures) = re.captures(text.trim()) {
            if let Some(yaml_match) = captures.name("yaml") {
                return yaml_match.as_str();
            }
        }

        text
    }
}

impl Default for YamlOutputParser {
    fn default() -> Self {
        Self::new()
    }
}

impl OutputParser for YamlOutputParser {
    type Output = JsonValue;

    fn parse(&self, text: &str) -> Result<Self::Output> {
        // Extract YAML from markdown code blocks if present
        let yaml_str = self.extract_yaml_from_markdown(text.trim());

        // Parse YAML into serde_json::Value
        serde_yml::from_str(yaml_str).map_err(|e| {
            Error::OutputParsing(format!(
                "Failed to parse YAML from completion {text}. Got: {e}"
            ))
        })
    }

    fn get_format_instructions(&self) -> String {
        if let Some(schema) = &self.schema {
            // Use the YAML_FORMAT_INSTRUCTIONS template
            format!(
                "The output should be formatted as a YAML instance that conforms to the given JSON schema below.\n\n\
                 # Examples\n\
                 ## Schema\n\
                 ```\n\
                 {{\"title\": \"Players\", \"description\": \"A list of players\", \"type\": \"array\", \"items\": {{\"$ref\": \"#/definitions/Player\"}}, \"definitions\": {{\"Player\": {{\"title\": \"Player\", \"type\": \"object\", \"properties\": {{\"name\": {{\"title\": \"Name\", \"description\": \"Player name\", \"type\": \"string\"}}, \"avg\": {{\"title\": \"Avg\", \"description\": \"Batting average\", \"type\": \"number\"}}}}, \"required\": [\"name\", \"avg\"]}}}}}}\n\
                 ```\n\
                 ## Well formatted instance\n\
                 ```\n\
                 - name: John Doe\n\
                   avg: 0.3\n\
                 - name: Jane Maxfield\n\
                   avg: 1.4\n\
                 ```\n\n\
                 ## Schema\n\
                 ```\n\
                 {{\"properties\": {{\"habit\": {{ \"description\": \"A common daily habit\", \"type\": \"string\" }}, \"sustainable_alternative\": {{ \"description\": \"An environmentally friendly alternative to the habit\", \"type\": \"string\"}}}}, \"required\": [\"habit\", \"sustainable_alternative\"]}}\n\
                 ```\n\
                 ## Well formatted instance\n\
                 ```\n\
                 habit: Using disposable water bottles for daily hydration.\n\
                 sustainable_alternative: Switch to a reusable water bottle to reduce plastic waste and decrease your environmental footprint.\n\
                 ```\n\n\
                 Please follow the standard YAML formatting conventions with an indent of 2 spaces and make sure that the data types adhere strictly to the following JSON schema:\n\
                 ```\n\
                 {schema}\n\
                 ```\n\n\
                 Make sure to always enclose the YAML output in triple backticks (```). Please do not add anything other than valid YAML output!"
            )
        } else {
            "Output should be formatted as valid YAML.".to_string()
        }
    }
}

#[async_trait]
impl Runnable for YamlOutputParser {
    type Input = String;
    type Output = JsonValue;

    async fn invoke(
        &self,
        input: Self::Input,
        _config: Option<RunnableConfig>,
    ) -> Result<Self::Output> {
        self.parse(&input)
    }

    async fn batch(
        &self,
        inputs: Vec<Self::Input>,
        _config: Option<RunnableConfig>,
    ) -> Result<Vec<Self::Output>> {
        inputs.iter().map(|input| self.parse(input)).collect()
    }

    async fn stream(
        &self,
        input: Self::Input,
        _config: Option<RunnableConfig>,
    ) -> Result<Pin<Box<dyn futures::Stream<Item = Result<Self::Output>> + Send>>> {
        let result = self.parse(&input)?;
        Ok(Box::pin(futures::stream::once(async move { Ok(result) })))
    }
}

impl Serializable for YamlOutputParser {
    fn lc_id(&self) -> Vec<String> {
        vec![
            "dashflow".to_string(),
            "output_parsers".to_string(),
            "yaml".to_string(),
            "YamlOutputParser".to_string(),
        ]
    }

    fn is_lc_serializable(&self) -> bool {
        true
    }

    fn to_json(&self) -> SerializedObject {
        let mut kwargs = serde_json::Map::new();
        if let Some(schema) = &self.schema {
            kwargs.insert("schema".to_string(), JsonValue::String(schema.clone()));
        }

        SerializedObject::Constructor {
            lc: SERIALIZATION_VERSION,
            id: self.lc_id(),
            kwargs: JsonValue::Object(kwargs),
        }
    }

    fn lc_secrets(&self) -> HashMap<String, String> {
        HashMap::new()
    }
}

impl Deserializable for YamlOutputParser {
    fn from_json(value: &JsonValue) -> Result<Self> {
        let (_lc, id, kwargs) = extract_serialized_fields(value)?;
        validate_id(
            &id,
            &[
                "dashflow".to_string(),
                "output_parsers".to_string(),
                "yaml".to_string(),
                "YamlOutputParser".to_string(),
            ],
        )?;

        let schema = get_optional_string(kwargs, "schema")?;

        Ok(YamlOutputParser {
            schema,
            cached_markdown_regex: std::sync::Arc::new(std::sync::OnceLock::new()),
        })
    }
}

// ============================================================================
// BooleanOutputParser
// ============================================================================

/// Parse LLM output to a boolean value.
///
/// The parser searches for configurable true/false strings in the output text,
/// using case-insensitive matching with word boundaries. Useful for yes/no
/// questions, binary classification, and confirmation prompts.
///
/// # Default Values
///
/// - `true_val`: "YES"
/// - `false_val`: "NO"
///
/// # Examples
///
/// ```
/// use dashflow::core::output_parsers::{BooleanOutputParser, OutputParser};
///
/// let parser = BooleanOutputParser::new();
///
/// assert_eq!(parser.parse("YES").unwrap(), true);
/// assert_eq!(parser.parse("no").unwrap(), false);
/// assert_eq!(parser.parse("The answer is YES").unwrap(), true);
///
/// // Custom values
/// let parser = BooleanOutputParser::new()
///     .with_true_val("AGREE")
///     .with_false_val("DISAGREE");
///
/// assert_eq!(parser.parse("I AGREE").unwrap(), true);
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BooleanOutputParser {
    /// The string value that should be parsed as True (default: "YES")
    #[serde(default = "default_true_val")]
    pub true_val: String,

    /// The string value that should be parsed as False (default: "NO")
    #[serde(default = "default_false_val")]
    pub false_val: String,

    /// Cached regex for parsing (not serialized, initialized on first use)
    #[serde(skip)]
    cached_regex: std::sync::OnceLock<regex::Regex>,
}

fn default_true_val() -> String {
    "YES".to_string()
}

fn default_false_val() -> String {
    "NO".to_string()
}

impl BooleanOutputParser {
    /// Create a new `BooleanOutputParser` with default values (YES/NO)
    #[must_use]
    pub fn new() -> Self {
        Self {
            true_val: default_true_val(),
            false_val: default_false_val(),
            cached_regex: std::sync::OnceLock::new(),
        }
    }

    /// Set the string value to parse as true
    #[must_use]
    pub fn with_true_val(mut self, val: impl Into<String>) -> Self {
        self.true_val = val.into();
        self.cached_regex = std::sync::OnceLock::new(); // Reset cache
        self
    }

    /// Set the string value to parse as false
    #[must_use]
    pub fn with_false_val(mut self, val: impl Into<String>) -> Self {
        self.false_val = val.into();
        self.cached_regex = std::sync::OnceLock::new(); // Reset cache
        self
    }
}

impl Default for BooleanOutputParser {
    fn default() -> Self {
        Self::new()
    }
}

impl OutputParser for BooleanOutputParser {
    type Output = bool;

    fn parse(&self, text: &str) -> Result<Self::Output> {
        // Get or build cached regex pattern: \b(YES|NO)\b
        // \b = word boundary, case-insensitive
        // Note: Using get_or_init with expect since regex should always be valid after escaping
        let re = self.cached_regex.get_or_init(|| {
            let pattern = format!(
                r"\b({}|{})\b",
                regex::escape(&self.true_val),
                regex::escape(&self.false_val)
            );
            regex::RegexBuilder::new(&pattern)
                .case_insensitive(true)
                .multi_line(true)
                .build()
                .expect("Regex compilation should not fail after regex::escape")
        });

        // Find all matches and convert to uppercase for comparison
        let mut found_true = false;
        let mut found_false = false;

        for cap in re.captures_iter(text) {
            if let Some(m) = cap.get(1) {
                let matched = m.as_str().to_uppercase();
                if matched == self.true_val.to_uppercase() {
                    found_true = true;
                }
                if matched == self.false_val.to_uppercase() {
                    found_false = true;
                }
            }
        }

        // Check for ambiguity
        if found_true && found_false {
            return Err(Error::OutputParsing(format!(
                "Ambiguous response. Both {} and {} found in: {}",
                self.true_val, self.false_val, text
            )));
        }

        if found_true {
            return Ok(true);
        }

        if found_false {
            return Ok(false);
        }

        // Neither found
        Err(Error::OutputParsing(format!(
            "BooleanOutputParser expected output value to include either {} or {}. Received: {}",
            self.true_val, self.false_val, text
        )))
    }

    fn get_format_instructions(&self) -> String {
        format!(
            "Respond with either '{}' for true or '{}' for false.",
            self.true_val, self.false_val
        )
    }
}

#[async_trait]
impl Runnable for BooleanOutputParser {
    type Input = String;
    type Output = bool;

    async fn invoke(
        &self,
        input: Self::Input,
        _config: Option<RunnableConfig>,
    ) -> Result<Self::Output> {
        self.parse(&input)
    }

    async fn batch(
        &self,
        inputs: Vec<Self::Input>,
        _config: Option<RunnableConfig>,
    ) -> Result<Vec<Self::Output>> {
        inputs.iter().map(|s| self.parse(s)).collect()
    }

    async fn stream(
        &self,
        input: Self::Input,
        _config: Option<RunnableConfig>,
    ) -> Result<Pin<Box<dyn futures::Stream<Item = Result<Self::Output>> + Send>>> {
        let result = self.parse(&input)?;
        Ok(Box::pin(futures::stream::once(async move { Ok(result) })))
    }
}

impl Serializable for BooleanOutputParser {
    fn lc_id(&self) -> Vec<String> {
        vec![
            "dashflow".to_string(),
            "output_parsers".to_string(),
            "boolean".to_string(),
            "BooleanOutputParser".to_string(),
        ]
    }

    fn is_lc_serializable(&self) -> bool {
        true
    }

    fn to_json(&self) -> SerializedObject {
        let mut kwargs = serde_json::Map::new();
        kwargs.insert(
            "true_val".to_string(),
            JsonValue::String(self.true_val.clone()),
        );
        kwargs.insert(
            "false_val".to_string(),
            JsonValue::String(self.false_val.clone()),
        );

        SerializedObject::Constructor {
            lc: SERIALIZATION_VERSION,
            id: self.lc_id(),
            kwargs: JsonValue::Object(kwargs),
        }
    }

    fn lc_secrets(&self) -> HashMap<String, String> {
        HashMap::new()
    }
}

impl Deserializable for BooleanOutputParser {
    fn from_json(value: &JsonValue) -> Result<Self> {
        let (_lc, id, kwargs) = extract_serialized_fields(value)?;
        validate_id(
            &id,
            &[
                "dashflow".to_string(),
                "output_parsers".to_string(),
                "boolean".to_string(),
                "BooleanOutputParser".to_string(),
            ],
        )?;

        let true_val = get_optional_string(kwargs, "true_val")?.unwrap_or_else(default_true_val);
        let false_val = get_optional_string(kwargs, "false_val")?.unwrap_or_else(default_false_val);

        Ok(BooleanOutputParser {
            true_val,
            false_val,
            cached_regex: std::sync::OnceLock::new(),
        })
    }
}

// ==================== EnumOutputParser ====================

/// Output parser that parses one value from a set of allowed string values.
///
/// Similar to Python's `EnumOutputParser`, but uses a list of strings rather than
/// a Python Enum type. This is more idiomatic in Rust and avoids reflection.
///
/// # Examples
///
/// ```rust
/// use dashflow::core::output_parsers::{EnumOutputParser, OutputParser};
///
/// let parser = EnumOutputParser::new(vec!["red", "green", "blue"]);
/// assert_eq!(parser.parse("red").unwrap(), "red");
/// assert_eq!(parser.parse("GREEN").unwrap(), "green"); // case-insensitive
/// assert!(parser.parse("yellow").is_err()); // not in allowed values
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnumOutputParser {
    /// The list of allowed string values
    pub values: Vec<String>,
}

impl EnumOutputParser {
    /// Create a new `EnumOutputParser` with the given allowed values
    ///
    /// # Arguments
    ///
    /// * `values` - A vector of allowed string values
    ///
    /// # Examples
    ///
    /// ```rust
    /// use dashflow::core::output_parsers::EnumOutputParser;
    ///
    /// let parser = EnumOutputParser::new(vec!["small", "medium", "large"]);
    /// ```
    pub fn new<I, S>(values: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        Self {
            values: values.into_iter().map(std::convert::Into::into).collect(),
        }
    }
}

impl OutputParser for EnumOutputParser {
    type Output = String;

    fn parse(&self, text: &str) -> Result<Self::Output> {
        let cleaned = text.trim();

        // Case-insensitive match against allowed values
        for value in &self.values {
            if cleaned.eq_ignore_ascii_case(value) {
                return Ok(value.clone());
            }
        }

        // Not found
        Err(Error::OutputParsing(format!(
            "Response '{}' is not one of the expected values: {}",
            text,
            self.values.join(", ")
        )))
    }

    fn get_format_instructions(&self) -> String {
        format!(
            "Select one of the following options: {}",
            self.values.join(", ")
        )
    }
}

#[async_trait]
impl Runnable for EnumOutputParser {
    type Input = String;
    type Output = String;

    async fn invoke(
        &self,
        input: Self::Input,
        _config: Option<RunnableConfig>,
    ) -> Result<Self::Output> {
        self.parse(&input)
    }

    async fn batch(
        &self,
        inputs: Vec<Self::Input>,
        _config: Option<RunnableConfig>,
    ) -> Result<Vec<Self::Output>> {
        inputs.iter().map(|s| self.parse(s)).collect()
    }

    async fn stream(
        &self,
        input: Self::Input,
        _config: Option<RunnableConfig>,
    ) -> Result<Pin<Box<dyn futures::Stream<Item = Result<Self::Output>> + Send>>> {
        let result = self.parse(&input)?;
        Ok(Box::pin(futures::stream::once(async move { Ok(result) })))
    }
}

impl Serializable for EnumOutputParser {
    fn lc_id(&self) -> Vec<String> {
        vec![
            "dashflow".to_string(),
            "output_parsers".to_string(),
            "enum".to_string(),
            "EnumOutputParser".to_string(),
        ]
    }

    fn is_lc_serializable(&self) -> bool {
        true
    }

    fn to_json(&self) -> SerializedObject {
        let mut kwargs = serde_json::Map::new();
        kwargs.insert(
            "values".to_string(),
            JsonValue::Array(
                self.values
                    .iter()
                    .map(|v| JsonValue::String(v.clone()))
                    .collect(),
            ),
        );

        SerializedObject::Constructor {
            lc: SERIALIZATION_VERSION,
            id: self.lc_id(),
            kwargs: JsonValue::Object(kwargs),
        }
    }

    fn lc_secrets(&self) -> HashMap<String, String> {
        HashMap::new()
    }
}

impl Deserializable for EnumOutputParser {
    fn from_json(value: &JsonValue) -> Result<Self> {
        let (_lc, id, kwargs) = extract_serialized_fields(value)?;
        validate_id(
            &id,
            &[
                "dashflow".to_string(),
                "output_parsers".to_string(),
                "enum".to_string(),
                "EnumOutputParser".to_string(),
            ],
        )?;

        let values_array = kwargs
            .get("values")
            .and_then(|v| v.as_array())
            .ok_or_else(|| Error::InvalidInput("Missing 'values' field".to_string()))?;

        let values: Vec<String> = values_array
            .iter()
            .filter_map(|v| v.as_str().map(std::string::ToString::to_string))
            .collect();

        Ok(EnumOutputParser { values })
    }
}

/// Parser that extracts named groups from text using regex patterns.
///
/// `RegexParser` uses a regular expression to extract one or more values from text
/// output. It maps regex capture groups to output keys, producing a `HashMap` of
/// extracted values.
///
/// # Fields
///
/// - `regex`: The compiled regex pattern to match
/// - `regex_str`: The regex pattern string (for serialization)
/// - `output_keys`: The keys to use for each capture group in order
/// - `default_output_key`: Optional default key that gets the full text if no match
///
/// # Examples
///
/// ```rust
/// use dashflow::core::output_parsers::{RegexParser, OutputParser};
///
/// // Extract name and age from text
/// let parser = RegexParser::new(
///     r"Name: (\w+), Age: (\d+)",
///     vec!["name", "age"],
///     None,
/// ).unwrap();
///
/// let result = parser.parse("Name: Alice, Age: 30").unwrap();
/// assert_eq!(result.get("name").unwrap(), "Alice");
/// assert_eq!(result.get("age").unwrap(), "30");
///
/// // If no match and no default_output_key, returns error
/// assert!(parser.parse("Random text").is_err());
/// ```
///
/// # With Default Output
///
/// ```rust
/// use dashflow::core::output_parsers::{RegexParser, OutputParser};
///
/// // With default output key
/// let parser = RegexParser::new(
///     r"Result: (.+)",
///     vec!["result"],
///     Some("result"),
/// ).unwrap();
///
/// // Returns full text in default key if no match
/// let result = parser.parse("Some unmatched text").unwrap();
/// assert_eq!(result.get("result").unwrap(), "Some unmatched text");
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegexParser {
    /// Regex pattern (not serialized directly)
    #[serde(skip)]
    pub regex: Option<regex::Regex>,
    /// Regex pattern string for serialization
    pub regex_str: String,
    /// The keys to use for each capture group in order
    pub output_keys: Vec<String>,
    /// Optional default key to use if regex doesn't match
    pub default_output_key: Option<String>,
}

impl RegexParser {
    /// Create a new `RegexParser`
    ///
    /// # Arguments
    ///
    /// * `regex_str` - The regex pattern to match
    /// * `output_keys` - Keys for each capture group in order
    /// * `default_output_key` - Optional default key for unmatched text
    ///
    /// # Returns
    ///
    /// Returns Ok(RegexParser) if regex compiles, Err otherwise
    ///
    /// # Examples
    ///
    /// ```rust
    /// use dashflow::core::output_parsers::RegexParser;
    ///
    /// let parser = RegexParser::new(
    ///     r"(\w+): (\d+)",
    ///     vec!["word", "number"],
    ///     None,
    /// ).unwrap();
    /// ```
    pub fn new<S: Into<String>>(
        regex_str: S,
        output_keys: Vec<S>,
        default_output_key: Option<S>,
    ) -> Result<Self> {
        let regex_string = regex_str.into();
        let regex = compile_bounded_regex(&regex_string)
            .map_err(|e| Error::InvalidInput(format!("Invalid regex pattern: {e}")))?;

        Ok(Self {
            regex: Some(regex),
            regex_str: regex_string,
            output_keys: output_keys
                .into_iter()
                .map(std::convert::Into::into)
                .collect(),
            default_output_key: default_output_key.map(std::convert::Into::into),
        })
    }
}

impl OutputParser for RegexParser {
    type Output = HashMap<String, String>;

    fn parse(&self, text: &str) -> Result<Self::Output> {
        let regex = self
            .regex
            .as_ref()
            .ok_or_else(|| Error::InvalidInput("Regex not compiled".to_string()))?;

        if let Some(captures) = regex.captures(text) {
            // Extract capture groups (skip group 0 which is the full match)
            let mut result = HashMap::new();
            for (i, key) in self.output_keys.iter().enumerate() {
                if let Some(capture) = captures.get(i + 1) {
                    result.insert(key.clone(), capture.as_str().to_string());
                }
            }
            Ok(result)
        } else if let Some(default_key) = &self.default_output_key {
            // No match but default key provided - return full text
            let mut result = HashMap::new();
            for key in &self.output_keys {
                if key == default_key {
                    result.insert(key.clone(), text.to_string());
                } else {
                    result.insert(key.clone(), String::new());
                }
            }
            Ok(result)
        } else {
            // No match and no default key - error
            Err(Error::OutputParsing(format!(
                "Could not parse output: {text}"
            )))
        }
    }

    fn get_format_instructions(&self) -> String {
        format!("Your response should match the pattern: {}", self.regex_str)
    }
}

#[async_trait]
impl Runnable for RegexParser {
    type Input = String;
    type Output = HashMap<String, String>;

    async fn invoke(
        &self,
        input: Self::Input,
        _config: Option<RunnableConfig>,
    ) -> Result<Self::Output> {
        self.parse(&input)
    }

    async fn batch(
        &self,
        inputs: Vec<Self::Input>,
        _config: Option<RunnableConfig>,
    ) -> Result<Vec<Self::Output>> {
        inputs.iter().map(|s| self.parse(s)).collect()
    }

    async fn stream(
        &self,
        input: Self::Input,
        _config: Option<RunnableConfig>,
    ) -> Result<Pin<Box<dyn futures::Stream<Item = Result<Self::Output>> + Send>>> {
        let result = self.parse(&input)?;
        Ok(Box::pin(futures::stream::once(async move { Ok(result) })))
    }
}

impl Serializable for RegexParser {
    fn lc_id(&self) -> Vec<String> {
        vec![
            "dashflow".to_string(),
            "output_parsers".to_string(),
            "regex".to_string(),
            "RegexParser".to_string(),
        ]
    }

    fn is_lc_serializable(&self) -> bool {
        true
    }

    fn to_json(&self) -> SerializedObject {
        let mut kwargs = serde_json::Map::new();
        kwargs.insert(
            "regex".to_string(),
            JsonValue::String(self.regex_str.clone()),
        );
        kwargs.insert(
            "output_keys".to_string(),
            JsonValue::Array(
                self.output_keys
                    .iter()
                    .map(|k| JsonValue::String(k.clone()))
                    .collect(),
            ),
        );
        if let Some(ref default_key) = self.default_output_key {
            kwargs.insert(
                "default_output_key".to_string(),
                JsonValue::String(default_key.clone()),
            );
        }

        SerializedObject::Constructor {
            lc: SERIALIZATION_VERSION,
            id: self.lc_id(),
            kwargs: JsonValue::Object(kwargs),
        }
    }

    fn lc_secrets(&self) -> HashMap<String, String> {
        HashMap::new()
    }
}

impl Deserializable for RegexParser {
    fn from_json(value: &JsonValue) -> Result<Self> {
        let (_lc, id, kwargs) = extract_serialized_fields(value)?;
        validate_id(
            &id,
            &[
                "dashflow".to_string(),
                "output_parsers".to_string(),
                "regex".to_string(),
                "RegexParser".to_string(),
            ],
        )?;

        let regex_str = kwargs
            .get("regex")
            .and_then(|v| v.as_str())
            .ok_or_else(|| Error::InvalidInput("Missing 'regex' field".to_string()))?
            .to_string();

        let output_keys_array = kwargs
            .get("output_keys")
            .and_then(|v| v.as_array())
            .ok_or_else(|| Error::InvalidInput("Missing 'output_keys' field".to_string()))?;

        let output_keys: Vec<String> = output_keys_array
            .iter()
            .filter_map(|v| v.as_str().map(std::string::ToString::to_string))
            .collect();

        let default_output_key = get_optional_string(kwargs, "default_output_key")?;

        RegexParser::new(regex_str, output_keys, default_output_key)
    }
}

/// Parser that extracts multiple key-value pairs using a template regex pattern.
///
/// `RegexDictParser` uses a template pattern with `{}` placeholder that gets replaced
/// with expected format strings. It extracts values for multiple keys from text output.
///
/// # Fields
///
/// - `regex_pattern`: Template pattern with `{}` placeholder (default: `"{}:\s?([^.'\n']*)\.?"`)
/// - `output_key_to_format`: Map of output keys to their expected format strings
/// - `no_update_value`: Optional value to skip when found (e.g., "`NO_UPDATE`")
///
/// # Examples
///
/// ```rust
/// use dashflow::core::output_parsers::{RegexDictParser, OutputParser};
/// use std::collections::HashMap;
///
/// // Extract name and age with default pattern
/// let mut key_to_format = HashMap::new();
/// key_to_format.insert("name".to_string(), "Name".to_string());
/// key_to_format.insert("age".to_string(), "Age".to_string());
///
/// let parser = RegexDictParser::new(key_to_format, None, None);
///
/// let result = parser.parse("Name: Alice\nAge: 30").unwrap();
/// assert_eq!(result.get("name").unwrap(), "Alice");
/// assert_eq!(result.get("age").unwrap(), "30");
/// ```
///
/// # Custom Pattern
///
/// ```rust
/// use dashflow::core::output_parsers::{RegexDictParser, OutputParser};
/// use std::collections::HashMap;
///
/// let mut key_to_format = HashMap::new();
/// key_to_format.insert("city".to_string(), "City".to_string());
///
/// // Custom pattern: "City = <value>"
/// let parser = RegexDictParser::new(
///     key_to_format,
///     Some(r"{} = (.+)".to_string()),
///     None,
/// );
///
/// let result = parser.parse("City = Paris").unwrap();
/// assert_eq!(result.get("city").unwrap(), "Paris");
/// ```
#[derive(Debug, Clone)]
pub struct RegexDictParser {
    /// Template regex pattern with {} placeholder
    pub regex_pattern: String,
    /// Map of output keys to expected format strings
    pub output_key_to_format: HashMap<String, String>,
    /// Optional value to skip (e.g., "`NO_UPDATE`")
    pub no_update_value: Option<String>,
    /// Pre-compiled regexes for each output key (`output_key`, `compiled_regex`)
    /// Stored in Vec for direct iteration without `HashMap` lookup or mutex overhead
    compiled_regexes: Vec<(String, regex::Regex)>,
}

fn default_regex_pattern() -> String {
    r"{}:\s?([^.'\n']*)\.?".to_string()
}

impl RegexDictParser {
    /// Create a new `RegexDictParser`
    ///
    /// # Arguments
    ///
    /// * `output_key_to_format` - Map of output keys to expected format strings
    /// * `regex_pattern` - Optional custom template pattern (default: `"{}:\s?([^.'\n']*)\.?"`)
    /// * `no_update_value` - Optional value to skip when found
    ///
    /// # Examples
    ///
    /// ```rust
    /// use dashflow::core::output_parsers::RegexDictParser;
    /// use std::collections::HashMap;
    ///
    /// let mut key_to_format = HashMap::new();
    /// key_to_format.insert("name".to_string(), "Name".to_string());
    ///
    /// let parser = RegexDictParser::new(key_to_format, None, None);
    /// ```
    /// Create a new `RegexDictParser`
    ///
    /// # Panics
    ///
    /// Panics if the regex pattern (default or custom) is invalid for any output key.
    /// Use [`try_new`](Self::try_new) for a non-panicking alternative.
    pub fn new(
        output_key_to_format: HashMap<String, String>,
        regex_pattern: Option<String>,
        no_update_value: Option<String>,
    ) -> Self {
        Self::try_new(output_key_to_format, regex_pattern, no_update_value).expect(
            "RegexDictParser::new: invalid regex pattern (use try_new for Result). This is a programming error.",
        )
    }

    /// Try to create a new `RegexDictParser`, returning an error if the regex pattern is invalid.
    ///
    /// # Arguments
    ///
    /// * `output_key_to_format` - Map of output keys to expected format strings
    /// * `regex_pattern` - Optional custom template pattern (default: `"{}:\s?([^.'\n']*)\.?"`)
    /// * `no_update_value` - Optional value to skip when found
    ///
    /// # Errors
    ///
    /// Returns an error if the regex pattern is invalid for any output key.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use dashflow::core::output_parsers::RegexDictParser;
    /// use std::collections::HashMap;
    ///
    /// let mut key_to_format = HashMap::new();
    /// key_to_format.insert("name".to_string(), "Name".to_string());
    ///
    /// let parser = RegexDictParser::try_new(key_to_format, None, None)?;
    /// # Ok::<(), dashflow::Error>(())
    /// ```
    pub fn try_new(
        output_key_to_format: HashMap<String, String>,
        regex_pattern: Option<String>,
        no_update_value: Option<String>,
    ) -> Result<Self> {
        let regex_pattern = regex_pattern.unwrap_or_else(default_regex_pattern);

        // Pre-compile all regexes at construction time (performance optimization)
        // This eliminates regex compilation, string allocation, and mutex overhead from parse()
        let mut compiled_regexes = Vec::with_capacity(output_key_to_format.len());

        for (output_key, expected_format) in &output_key_to_format {
            // Escape the expected format and create the specific pattern (done once here)
            let escaped_format = regex::escape(expected_format);
            let specific_pattern = regex_pattern.replace("{}", &escaped_format);

            // Compile regex once at construction (with size limits for ReDoS prevention)
            let compiled_re = compile_bounded_regex(&specific_pattern).map_err(|e| {
                Error::InvalidInput(format!(
                    "Invalid regex pattern '{specific_pattern}' for key '{output_key}': {e}"
                ))
            })?;

            compiled_regexes.push((output_key.clone(), compiled_re));
        }

        Ok(Self {
            regex_pattern,
            output_key_to_format,
            no_update_value,
            compiled_regexes,
        })
    }
}

impl OutputParser for RegexDictParser {
    type Output = HashMap<String, String>;

    fn parse(&self, text: &str) -> Result<Self::Output> {
        let mut result = HashMap::new();

        // Direct iteration over pre-compiled regexes (no HashMap lookup, no mutex, no allocations)
        for (output_key, compiled_re) in &self.compiled_regexes {
            // Find all matches using the pre-compiled regex
            let matches: Vec<&str> = compiled_re
                .captures_iter(text)
                .filter_map(|cap| cap.get(1).map(|m| m.as_str()))
                .collect();

            // Get the expected format for error messages
            let expected_format = self
                .output_key_to_format
                .get(output_key)
                .expect("output_key should exist in output_key_to_format");

            if matches.is_empty() {
                return Err(Error::OutputParsing(format!(
                    "No match found for output key: {output_key} with expected format {expected_format} on text {text}"
                )));
            }

            if matches.len() > 1 {
                return Err(Error::OutputParsing(format!(
                    "Multiple matches found for output key: {output_key} with expected format {expected_format} on text {text}"
                )));
            }

            // Check if value should be skipped
            let value = matches[0];
            if let Some(ref no_update) = self.no_update_value {
                if value == no_update {
                    continue;
                }
            }

            result.insert(output_key.clone(), value.to_string());
        }

        Ok(result)
    }

    fn get_format_instructions(&self) -> String {
        let keys: Vec<String> = self.output_key_to_format.values().cloned().collect();

        format!(
            "Your response should include these fields: {}",
            keys.join(", ")
        )
    }
}

#[async_trait]
impl Runnable for RegexDictParser {
    type Input = String;
    type Output = HashMap<String, String>;

    async fn invoke(
        &self,
        input: Self::Input,
        _config: Option<RunnableConfig>,
    ) -> Result<Self::Output> {
        self.parse(&input)
    }

    async fn batch(
        &self,
        inputs: Vec<Self::Input>,
        _config: Option<RunnableConfig>,
    ) -> Result<Vec<Self::Output>> {
        inputs.iter().map(|s| self.parse(s)).collect()
    }

    async fn stream(
        &self,
        input: Self::Input,
        _config: Option<RunnableConfig>,
    ) -> Result<Pin<Box<dyn futures::Stream<Item = Result<Self::Output>> + Send>>> {
        let result = self.parse(&input)?;
        Ok(Box::pin(futures::stream::once(async move { Ok(result) })))
    }
}

impl Serializable for RegexDictParser {
    fn lc_id(&self) -> Vec<String> {
        vec![
            "dashflow".to_string(),
            "output_parsers".to_string(),
            "regex_dict".to_string(),
            "RegexDictParser".to_string(),
        ]
    }

    fn is_lc_serializable(&self) -> bool {
        true
    }

    fn to_json(&self) -> SerializedObject {
        let mut kwargs = serde_json::Map::new();
        kwargs.insert(
            "regex_pattern".to_string(),
            JsonValue::String(self.regex_pattern.clone()),
        );

        let mut key_to_format_map = serde_json::Map::new();
        for (key, format) in &self.output_key_to_format {
            key_to_format_map.insert(key.clone(), JsonValue::String(format.clone()));
        }
        kwargs.insert(
            "output_key_to_format".to_string(),
            JsonValue::Object(key_to_format_map),
        );

        if let Some(ref no_update) = self.no_update_value {
            kwargs.insert(
                "no_update_value".to_string(),
                JsonValue::String(no_update.clone()),
            );
        }

        SerializedObject::Constructor {
            lc: SERIALIZATION_VERSION,
            id: self.lc_id(),
            kwargs: JsonValue::Object(kwargs),
        }
    }

    fn lc_secrets(&self) -> HashMap<String, String> {
        HashMap::new()
    }
}

impl Deserializable for RegexDictParser {
    fn from_json(value: &JsonValue) -> Result<Self> {
        let (_lc, id, kwargs) = extract_serialized_fields(value)?;
        validate_id(
            &id,
            &[
                "dashflow".to_string(),
                "output_parsers".to_string(),
                "regex_dict".to_string(),
                "RegexDictParser".to_string(),
            ],
        )?;

        let regex_pattern = kwargs
            .get("regex_pattern")
            .and_then(|v| v.as_str())
            .map(std::string::ToString::to_string);

        let output_key_to_format_obj = kwargs
            .get("output_key_to_format")
            .and_then(|v| v.as_object())
            .ok_or_else(|| {
                Error::InvalidInput("Missing 'output_key_to_format' field".to_string())
            })?;

        let mut output_key_to_format = HashMap::new();
        for (key, value) in output_key_to_format_obj {
            if let Some(format_str) = value.as_str() {
                output_key_to_format.insert(key.clone(), format_str.to_string());
            }
        }

        let no_update_value = get_optional_string(kwargs, "no_update_value")?;

        // Use the new() constructor to ensure regexes are pre-compiled
        Ok(RegexDictParser::new(
            output_key_to_format,
            regex_pattern,
            no_update_value,
        ))
    }
}

#[cfg(test)]
mod tests;

// ============================================================================
// OutputFixingParser - Auto-fixing parser with LLM feedback
// ============================================================================

/// Parser that wraps another parser and automatically retries with LLM feedback on errors.
///
/// `OutputFixingParser` attempts to parse with the base parser. If parsing fails, it uses
/// an LLM to fix the malformed output based on the error message and format instructions.
///
/// This is useful when LLM outputs don't perfectly match the expected format, as the
/// LLM can self-correct based on specific error feedback.
///
/// # Type Parameters
///
/// - `T`: The output type of the wrapped parser
///
/// # Example
///
/// ```rust,ignore
/// use dashflow::core::output_parsers::{OutputFixingParser, JsonOutputParser};
/// use dashflow::core::prompts::PromptTemplate;
/// use dashflow_openai::ChatOpenAI;
///
/// let base_parser = JsonOutputParser::new();
/// let llm = ChatOpenAI::with_config(Default::default()).with_model("gpt-4o-mini");
/// let prompt = PromptTemplate::from_template(NAIVE_FIX_PROMPT);
///
/// let fixing_parser = OutputFixingParser::from_llm(
///     llm,
///     base_parser,
///     prompt,
///     2  // max retries
/// );
///
/// // Will automatically fix malformed JSON
/// let result = fixing_parser.parse("```{\"key: \"value\"}```").await?;
/// ```
pub struct OutputFixingParser<T> {
    /// The base parser to use for initial parsing attempt
    parser: Arc<dyn OutputParser<Output = T>>,
    /// The runnable chain (prompt | llm | `str_parser`) to fix errors
    retry_chain: Arc<dyn Runnable<Input = HashMap<String, String>, Output = String>>,
    /// Maximum number of retry attempts
    max_retries: usize,
}

/// Default prompt for fixing parse errors
pub const NAIVE_FIX_PROMPT: &str = r"Instructions:
--------------
{instructions}
--------------
Completion:
--------------
{completion}
--------------

Above, the Completion did not satisfy the constraints given in the Instructions.
Error:
--------------
{error}
--------------

Please try again. Please only respond with an answer that satisfies the constraints laid out in the Instructions:";

/// Helper struct that wraps a prompt template and LLM into a retry chain
struct OutputFixingRetryChain<L> {
    prompt: crate::core::prompts::PromptTemplate,
    llm: Arc<L>,
}

#[async_trait]
impl<L> Runnable for OutputFixingRetryChain<L>
where
    L: Runnable<
            Input = Vec<crate::core::messages::Message>,
            Output = crate::core::messages::Message,
        > + Send
        + Sync,
{
    type Input = HashMap<String, String>;
    type Output = String;

    async fn invoke(
        &self,
        input: Self::Input,
        config: Option<RunnableConfig>,
    ) -> Result<Self::Output> {
        use crate::core::prompts::BasePromptTemplate;

        // Convert HashMap to PromptValue using the template
        let prompt_value = self.prompt.format_prompt(&input)?;

        // Convert PromptValue to messages
        let messages = prompt_value.to_messages();

        // Invoke LLM
        let message = self.llm.invoke(messages, config).await?;

        // Extract text from message
        Ok(message.content().as_text())
    }

    async fn batch(
        &self,
        inputs: Vec<Self::Input>,
        config: Option<RunnableConfig>,
    ) -> Result<Vec<Self::Output>> {
        let mut results = Vec::new();
        for input in inputs {
            results.push(self.invoke(input, config.clone()).await?);
        }
        Ok(results)
    }

    async fn stream(
        &self,
        input: Self::Input,
        config: Option<RunnableConfig>,
    ) -> Result<Pin<Box<dyn futures::Stream<Item = Result<Self::Output>> + Send>>> {
        let result = self.invoke(input, config).await?;
        Ok(Box::pin(futures::stream::once(async move { Ok(result) })))
    }
}

impl<T: Send + 'static> OutputFixingParser<T> {
    /// Create a new `OutputFixingParser` with a pre-built retry chain.
    ///
    /// # Arguments
    ///
    /// - `parser`: The base parser to wrap
    /// - `retry_chain`: The runnable chain (prompt | llm | `str_parser`) to fix errors
    /// - `max_retries`: Maximum number of retry attempts
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use dashflow::core::output_parsers::{OutputFixingParser, JsonOutputParser, StrOutputParser};
    /// use dashflow::core::prompts::PromptTemplate;
    /// use dashflow::core::runnable::Runnable;
    ///
    /// let parser = JsonOutputParser::new();
    /// let prompt = PromptTemplate::from_template(NAIVE_FIX_PROMPT);
    /// let llm = ChatOpenAI::with_config(Default::default()).with_model("gpt-4o-mini");
    ///
    /// // Build the retry chain: prompt | llm | str_parser
    /// let retry_chain = prompt.pipe(llm).pipe(StrOutputParser);
    ///
    /// let fixing_parser = OutputFixingParser::new(parser, retry_chain, 2);
    /// ```
    pub fn new<P, R>(parser: P, retry_chain: R, max_retries: usize) -> Self
    where
        P: OutputParser<Output = T> + 'static,
        R: Runnable<Input = HashMap<String, String>, Output = String> + 'static,
    {
        Self {
            parser: Arc::new(parser),
            retry_chain: Arc::new(retry_chain),
            max_retries,
        }
    }

    /// Create an `OutputFixingParser` from an LLM and a base parser.
    ///
    /// This is a convenience method that builds the retry chain automatically
    /// using the `NAIVE_FIX_PROMPT` template.
    ///
    /// # Arguments
    ///
    /// - `llm`: The language model to use for fixing
    /// - `parser`: The base parser to wrap
    /// - `max_retries`: Maximum number of retry attempts (default: 1)
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use dashflow::core::output_parsers::{OutputFixingParser, JsonOutputParser};
    /// use dashflow_openai::ChatOpenAI;
    ///
    /// let base_parser = JsonOutputParser::new();
    /// let llm = ChatOpenAI::with_config(Default::default()).with_model("gpt-4o-mini");
    ///
    /// let fixing_parser = OutputFixingParser::from_llm(llm, base_parser, 2)?;
    ///
    /// // Will automatically fix malformed JSON
    /// let result = fixing_parser.invoke("```{\"key: \"value\"}```".to_string(), None).await?;
    /// ```
    pub fn from_llm<L, P>(llm: L, parser: P, max_retries: usize) -> Result<Self>
    where
        L: Runnable<
                Input = Vec<crate::core::messages::Message>,
                Output = crate::core::messages::Message,
            > + Send
            + Sync
            + 'static,
        P: OutputParser<Output = T> + 'static,
    {
        use crate::core::prompts::PromptTemplate;

        // Create the prompt template
        let prompt = PromptTemplate::from_template(NAIVE_FIX_PROMPT)?;

        // Build the retry chain wrapper
        let chain = OutputFixingRetryChain {
            prompt,
            llm: Arc::new(llm),
        };

        Ok(Self {
            parser: Arc::new(parser),
            retry_chain: Arc::new(chain),
            max_retries,
        })
    }

    /// Parse with automatic error fixing.
    ///
    /// Attempts to parse with the base parser. If that fails, retries up to `max_retries`
    /// times using the LLM to fix the output based on error feedback.
    pub async fn parse_with_retries(&self, text: &str) -> Result<T> {
        let mut completion = text.to_string();
        let mut retries = 0;

        loop {
            match self.parser.parse(&completion) {
                Ok(result) => return Ok(result),
                Err(e) if retries >= self.max_retries => {
                    // Max retries exceeded
                    return Err(Error::OutputParsing(format!(
                        "Failed to parse after {} retries: {}",
                        self.max_retries, e
                    )));
                }
                Err(e) => {
                    // Retry with LLM feedback
                    retries += 1;

                    let mut input = HashMap::new();
                    input.insert(
                        "instructions".to_string(),
                        self.parser.get_format_instructions(),
                    );
                    input.insert("completion".to_string(), completion.clone());
                    input.insert("error".to_string(), format!("{e}"));

                    completion = self.retry_chain.invoke(input, None).await.map_err(|e| {
                        Error::OutputParsing(format!("LLM fix attempt failed: {e}"))
                    })?;
                }
            }
        }
    }
}

impl<T: Send + 'static> OutputParser for OutputFixingParser<T> {
    type Output = T;

    fn parse(&self, text: &str) -> Result<Self::Output> {
        // Synchronous parse: just try the base parser once
        // For full retry functionality, use async parse_with_retries
        self.parser.parse(text)
    }

    fn get_format_instructions(&self) -> String {
        self.parser.get_format_instructions()
    }
}

#[async_trait]
impl<T: Send + 'static> Runnable for OutputFixingParser<T> {
    type Input = String;
    type Output = T;

    async fn invoke(
        &self,
        input: Self::Input,
        _config: Option<RunnableConfig>,
    ) -> Result<Self::Output> {
        // Async invoke uses retry logic
        self.parse_with_retries(&input).await
    }

    async fn batch(
        &self,
        inputs: Vec<Self::Input>,
        _config: Option<RunnableConfig>,
    ) -> Result<Vec<Self::Output>> {
        let mut results = Vec::new();
        for input in inputs {
            results.push(self.parse_with_retries(&input).await?);
        }
        Ok(results)
    }

    async fn stream(
        &self,
        input: Self::Input,
        _config: Option<RunnableConfig>,
    ) -> Result<Pin<Box<dyn futures::Stream<Item = Result<Self::Output>> + Send>>> {
        let result = self.parse_with_retries(&input).await?;
        Ok(Box::pin(futures::stream::once(async move { Ok(result) })))
    }
}

/// Transform output parser
///
/// Wraps another parser and transforms its output using a function.
pub struct TransformOutputParser<I, O> {
    parser: Box<dyn OutputParser<Output = I>>,
    transform: Arc<dyn Fn(I) -> O + Send + Sync>,
}

impl<I, O> TransformOutputParser<I, O> {
    /// Creates a new `TransformOutputParser` that applies a transformation to the wrapped parser's output.
    pub fn new<P, F>(parser: P, transform: F) -> Self
    where
        P: OutputParser<Output = I> + 'static,
        F: Fn(I) -> O + Send + Sync + 'static,
    {
        Self {
            parser: Box::new(parser),
            transform: Arc::new(transform),
        }
    }
}

#[async_trait::async_trait]
impl<I: Send + Sync + 'static, O: Send + Sync + 'static> OutputParser
    for TransformOutputParser<I, O>
{
    type Output = O;

    fn get_format_instructions(&self) -> String {
        self.parser.get_format_instructions()
    }

    fn parse(&self, text: &str) -> Result<Self::Output> {
        let intermediate = self.parser.parse(text)?;
        Ok((self.transform)(intermediate))
    }
}

/// Pandas `DataFrame` output parser
///
/// Parses LLM output as CSV and converts to a simple table structure.
/// Note: Rust doesn't have pandas, so this returns a `Vec<HashMap>` structure.
pub struct PandasDataFrameOutputParser {
    delimiter: String,
}

impl PandasDataFrameOutputParser {
    /// Creates a new CSV/DataFrame parser with the default comma delimiter.
    #[must_use]
    pub fn new() -> Self {
        Self {
            delimiter: ",".to_string(),
        }
    }

    /// Set a custom delimiter for CSV parsing.
    ///
    /// # Panics
    ///
    /// Panics if delimiter is empty or not a single ASCII character.
    /// CSV parsing requires a single-byte delimiter.
    #[must_use]
    pub fn with_delimiter(mut self, delimiter: &str) -> Self {
        assert!(
            delimiter.len() == 1 && delimiter.is_ascii(),
            "CSV delimiter must be a single ASCII character, got: {:?}",
            delimiter
        );
        self.delimiter = delimiter.to_string();
        self
    }
}

impl Default for PandasDataFrameOutputParser {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl OutputParser for PandasDataFrameOutputParser {
    type Output = Vec<std::collections::HashMap<String, String>>;

    fn get_format_instructions(&self) -> String {
        format!(
            "Output a CSV table with headers in the first row, using '{}' as delimiter.",
            self.delimiter
        )
    }

    fn parse(&self, text: &str) -> Result<Self::Output> {
        // Safety: delimiter validated in with_delimiter() to be single ASCII char
        let delimiter_byte = self
            .delimiter
            .as_bytes()
            .first()
            .copied()
            .ok_or_else(|| Error::invalid_input("CSV delimiter cannot be empty"))?;

        let mut reader = csv::ReaderBuilder::new()
            .delimiter(delimiter_byte)
            .from_reader(text.as_bytes());

        let headers = reader
            .headers()
            .map_err(|e| Error::other(format!("Failed to parse CSV headers: {e}")))?
            .clone();

        let mut results = Vec::new();
        for record in reader.records() {
            let record =
                record.map_err(|e| Error::other(format!("Failed to parse CSV record: {e}")))?;

            let mut row = std::collections::HashMap::new();
            for (i, field) in record.iter().enumerate() {
                if let Some(header) = headers.get(i) {
                    row.insert(header.to_string(), field.to_string());
                }
            }
            results.push(row);
        }

        Ok(results)
    }
}
