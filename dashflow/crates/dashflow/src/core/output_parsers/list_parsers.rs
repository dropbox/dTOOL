// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

//! List output parsers for comma-separated, numbered, markdown, and line-based lists.
//!
//! This module provides parsers for various list formats commonly found in LLM outputs.

use super::*;

/// Parses comma-separated values into a list of strings.
///
/// Handles both simple comma separation and CSV-style quoting. Uses Rust's
/// CSV reader for robust parsing.
///
/// # Example
///
/// ```rust
/// use dashflow::core::output_parsers::{OutputParser, CommaSeparatedListOutputParser};
///
/// let parser = CommaSeparatedListOutputParser;
///
/// // Simple comma-separated
/// let result = parser.parse("foo, bar, baz").unwrap();
/// assert_eq!(result, vec!["foo", "bar", "baz"]);
///
/// // With quotes
/// let result = parser.parse(r#""hello, world", test, "foo""#).unwrap();
/// assert_eq!(result, vec!["hello, world", "test", "foo"]);
/// ```
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct CommaSeparatedListOutputParser;

impl OutputParser for CommaSeparatedListOutputParser {
    type Output = Vec<String>;

    fn parse(&self, text: &str) -> Result<Self::Output> {
        // Handle empty string edge case
        if text.is_empty() {
            return Ok(vec![String::new()]);
        }

        // Fast path for simple comma-separated lists (no quotes, no newlines)
        // This avoids the ~10μs csv crate setup overhead for common cases
        if !text.contains('"') && !text.contains('\n') {
            return Ok(text.split(',').map(|s| s.trim().to_string()).collect());
        }

        // Robust path: use csv crate for complex inputs with quotes/newlines
        let mut reader = csv::ReaderBuilder::new()
            .has_headers(false)
            .trim(csv::Trim::All)
            .flexible(true) // Allow records with varying number of fields
            .from_reader(std::io::Cursor::new(text));

        let mut items = Vec::new();
        for result in reader.records() {
            match result {
                Ok(record) => {
                    for field in &record {
                        // Manually strip quotes if present (fallback for edge cases)
                        let cleaned = field.trim();
                        let cleaned = if cleaned.starts_with('"')
                            && cleaned.ends_with('"')
                            && cleaned.len() >= 2
                        {
                            &cleaned[1..cleaned.len() - 1]
                        } else {
                            cleaned
                        };
                        items.push(cleaned.to_string());
                    }
                }
                Err(_) => {
                    // Fallback to simple split if CSV parsing fails
                    return Ok(text
                        .split(',')
                        .map(|s| {
                            let s = s.trim();
                            // Strip quotes manually
                            if s.starts_with('"') && s.ends_with('"') && s.len() >= 2 {
                                s[1..s.len() - 1].to_string()
                            } else {
                                s.to_string()
                            }
                        })
                        .collect());
                }
            }
        }

        Ok(items)
    }

    fn get_format_instructions(&self) -> String {
        "Your response should be a list of comma separated values, eg: `foo, bar, baz`".to_string()
    }
}

#[async_trait]
impl Runnable for CommaSeparatedListOutputParser {
    type Input = String;
    type Output = Vec<String>;

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

impl Serializable for CommaSeparatedListOutputParser {
    fn lc_id(&self) -> Vec<String> {
        vec![
            "dashflow".to_string(),
            "output_parsers".to_string(),
            "list".to_string(),
            "CommaSeparatedListOutputParser".to_string(),
        ]
    }

    fn is_lc_serializable(&self) -> bool {
        true
    }

    fn to_json(&self) -> SerializedObject {
        // CommaSeparatedListOutputParser has no configuration
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

impl Deserializable for CommaSeparatedListOutputParser {
    fn from_json(value: &JsonValue) -> Result<Self> {
        let (_lc, id, _kwargs) = extract_serialized_fields(value)?;
        validate_id(
            &id,
            &[
                "dashflow".to_string(),
                "output_parsers".to_string(),
                "list".to_string(),
                "CommaSeparatedListOutputParser".to_string(),
            ],
        )?;
        Ok(CommaSeparatedListOutputParser)
    }
}

/// Parses numbered lists (1. item, 2. item, etc.) into a list of strings.
///
/// # Example
///
/// ```rust
/// use dashflow::core::output_parsers::{OutputParser, NumberedListOutputParser};
///
/// let parser = NumberedListOutputParser::new();
/// let text = "1. First item\n2. Second item\n3. Third item";
/// let result = parser.parse(text).unwrap();
/// assert_eq!(result, vec!["First item", "Second item", "Third item"]);
/// ```
#[derive(Debug, Clone)]
pub struct NumberedListOutputParser {
    /// Regex pattern for matching numbered list items
    pattern: regex::Regex,
    /// Pattern string for serialization (stored separately since Regex doesn't serialize)
    pattern_str: String,
}

impl Default for NumberedListOutputParser {
    fn default() -> Self {
        Self::new()
    }
}

impl NumberedListOutputParser {
    /// Default regex pattern for numbered lists
    const DEFAULT_PATTERN: &'static str = r"\d+\.\s+([^\n]+)";

    /// Create a new numbered list parser with the default pattern.
    #[must_use]
    pub fn new() -> Self {
        Self {
            pattern: regex::Regex::new(Self::DEFAULT_PATTERN).expect("Invalid regex pattern"),
            pattern_str: Self::DEFAULT_PATTERN.to_string(),
        }
    }

    /// Create a parser with a custom regex pattern.
    pub fn with_pattern(pattern: &str) -> Result<Self> {
        Ok(Self {
            pattern: regex::Regex::new(pattern)
                .map_err(|e| Error::OutputParsing(format!("Invalid regex: {e}")))?,
            pattern_str: pattern.to_string(),
        })
    }
}

impl OutputParser for NumberedListOutputParser {
    type Output = Vec<String>;

    fn parse(&self, text: &str) -> Result<Self::Output> {
        Ok(self
            .pattern
            .captures_iter(text)
            .filter_map(|cap| cap.get(1).map(|m| m.as_str().to_string()))
            .collect())
    }

    fn get_format_instructions(&self) -> String {
        "Your response should be a numbered list with each item on a new line. For example:\n\n1. foo\n\n2. bar\n\n3. baz"
            .to_string()
    }
}

#[async_trait]
impl Runnable for NumberedListOutputParser {
    type Input = String;
    type Output = Vec<String>;

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

impl Serializable for NumberedListOutputParser {
    fn lc_id(&self) -> Vec<String> {
        vec![
            "dashflow".to_string(),
            "output_parsers".to_string(),
            "list".to_string(),
            "NumberedListOutputParser".to_string(),
        ]
    }

    fn is_lc_serializable(&self) -> bool {
        true
    }

    fn to_json(&self) -> SerializedObject {
        let mut kwargs = serde_json::Map::new();

        // Only include pattern if it's not the default
        if self.pattern_str != Self::DEFAULT_PATTERN {
            kwargs.insert("pattern".to_string(), serde_json::json!(self.pattern_str));
        }

        SerializedObject::Constructor {
            lc: SERIALIZATION_VERSION,
            id: self.lc_id(),
            kwargs: kwargs.into(),
        }
    }

    fn lc_secrets(&self) -> HashMap<String, String> {
        HashMap::new()
    }
}

impl Deserializable for NumberedListOutputParser {
    fn from_json(value: &JsonValue) -> Result<Self> {
        let (_lc, id, kwargs) = extract_serialized_fields(value)?;
        validate_id(
            &id,
            &[
                "dashflow".to_string(),
                "output_parsers".to_string(),
                "list".to_string(),
                "NumberedListOutputParser".to_string(),
            ],
        )?;

        // Extract optional pattern field
        match get_optional_string(kwargs, "pattern")? {
            Some(pattern) => Self::with_pattern(&pattern),
            None => Ok(Self::new()),
        }
    }
}

/// Parses Markdown bullet lists (- item, * item) into a list of strings.
///
/// # Example
///
/// ```rust
/// use dashflow::core::output_parsers::{OutputParser, MarkdownListOutputParser};
///
/// let parser = MarkdownListOutputParser::new();
/// let text = "- First item\n- Second item\n* Third item";
/// let result = parser.parse(text).unwrap();
/// assert_eq!(result, vec!["First item", "Second item", "Third item"]);
/// ```
#[derive(Debug, Clone)]
pub struct MarkdownListOutputParser {
    /// Regex pattern for matching Markdown list items
    pattern: regex::Regex,
    /// Pattern string for serialization (stored separately since Regex doesn't serialize)
    pattern_str: String,
}

impl Default for MarkdownListOutputParser {
    fn default() -> Self {
        Self::new()
    }
}

impl MarkdownListOutputParser {
    /// Default regex pattern for Markdown lists
    const DEFAULT_PATTERN: &'static str = r"^\s*[-*]\s+([^\n]+)$";

    /// Create a new Markdown list parser with the default pattern.
    #[must_use]
    pub fn new() -> Self {
        Self {
            pattern: regex::RegexBuilder::new(Self::DEFAULT_PATTERN)
                .multi_line(true)
                .build()
                .expect("Invalid regex pattern"),
            pattern_str: Self::DEFAULT_PATTERN.to_string(),
        }
    }

    /// Create a parser with a custom regex pattern.
    pub fn with_pattern(pattern: &str) -> Result<Self> {
        Ok(Self {
            pattern: regex::Regex::new(pattern)
                .map_err(|e| Error::OutputParsing(format!("Invalid regex: {e}")))?,
            pattern_str: pattern.to_string(),
        })
    }
}

impl OutputParser for MarkdownListOutputParser {
    type Output = Vec<String>;

    fn parse(&self, text: &str) -> Result<Self::Output> {
        Ok(self
            .pattern
            .captures_iter(text)
            .filter_map(|cap| cap.get(1).map(|m| m.as_str().to_string()))
            .collect())
    }

    fn get_format_instructions(&self) -> String {
        "Your response should be a markdown list, eg: `- foo\\n- bar\\n- baz`".to_string()
    }
}

#[async_trait]
impl Runnable for MarkdownListOutputParser {
    type Input = String;
    type Output = Vec<String>;

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

impl Serializable for MarkdownListOutputParser {
    fn lc_id(&self) -> Vec<String> {
        vec![
            "dashflow".to_string(),
            "output_parsers".to_string(),
            "list".to_string(),
            "MarkdownListOutputParser".to_string(),
        ]
    }

    fn is_lc_serializable(&self) -> bool {
        true
    }

    fn to_json(&self) -> SerializedObject {
        let mut kwargs = serde_json::Map::new();

        // Only include pattern if it's not the default
        if self.pattern_str != Self::DEFAULT_PATTERN {
            kwargs.insert("pattern".to_string(), serde_json::json!(self.pattern_str));
        }

        SerializedObject::Constructor {
            lc: SERIALIZATION_VERSION,
            id: self.lc_id(),
            kwargs: kwargs.into(),
        }
    }

    fn lc_secrets(&self) -> HashMap<String, String> {
        HashMap::new()
    }
}

impl Deserializable for MarkdownListOutputParser {
    fn from_json(value: &JsonValue) -> Result<Self> {
        let (_lc, id, kwargs) = extract_serialized_fields(value)?;
        validate_id(
            &id,
            &[
                "dashflow".to_string(),
                "output_parsers".to_string(),
                "list".to_string(),
                "MarkdownListOutputParser".to_string(),
            ],
        )?;

        // Extract optional pattern field
        match get_optional_string(kwargs, "pattern")? {
            Some(pattern) => Self::with_pattern(&pattern),
            None => Ok(Self::new()),
        }
    }
}

/// Parser for newline-separated lists.
///
/// Splits text by newlines and filters out empty lines. This is useful for parsing
/// LLM outputs where each line represents a separate item.
///
/// # Example
///
/// ```rust
/// use dashflow::core::output_parsers::{OutputParser, LineListOutputParser};
///
/// let parser = LineListOutputParser;
/// let result = parser.parse("apple\nbanana\ncherry").unwrap();
/// assert_eq!(result, vec!["apple", "banana", "cherry"]);
/// ```
#[derive(Debug, Clone, Default)]
pub struct LineListOutputParser;

impl OutputParser for LineListOutputParser {
    type Output = Vec<String>;

    fn parse(&self, text: &str) -> Result<Self::Output> {
        Ok(text
            .trim()
            .split('\n')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect())
    }

    fn get_format_instructions(&self) -> String {
        "Your response should be a list of items, one per line.".to_string()
    }
}

#[async_trait]
impl Runnable for LineListOutputParser {
    type Input = String;
    type Output = Vec<String>;

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

impl Serializable for LineListOutputParser {
    fn lc_id(&self) -> Vec<String> {
        vec![
            "dashflow".to_string(),
            "output_parsers".to_string(),
            "list".to_string(),
            "LineListOutputParser".to_string(),
        ]
    }

    fn is_lc_serializable(&self) -> bool {
        true
    }

    fn to_json(&self) -> SerializedObject {
        // LineListOutputParser has no configuration
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

impl Deserializable for LineListOutputParser {
    fn from_json(value: &JsonValue) -> Result<Self> {
        let (_lc, id, _kwargs) = extract_serialized_fields(value)?;
        validate_id(
            &id,
            &[
                "dashflow".to_string(),
                "output_parsers".to_string(),
                "list".to_string(),
                "LineListOutputParser".to_string(),
            ],
        )?;
        Ok(LineListOutputParser)
    }
}

// ================================================================================================
// QuestionListOutputParser - Parses numbered question lists
// ================================================================================================

/// Parses numbered question lists from language model outputs.
///
/// This parser is designed to extract questions from LLM responses that are formatted as
/// numbered lists (e.g., "1. Question one?", "2. Question two?"). It uses a regex pattern
/// to identify lines starting with a digit and a period, and extracts the complete line
/// including the number.
///
/// This is particularly useful for retrieval systems that generate multiple search queries
/// from a single question, such as `WebResearchRetriever`.
///
/// # Example
///
/// ```rust
/// use dashflow::core::output_parsers::{OutputParser, QuestionListOutputParser};
///
/// let parser = QuestionListOutputParser;
/// let text = "1. What is DashFlow?\n2. How does DashFlow work?\n3. What are DashFlow features?";
/// let result = parser.parse(text).unwrap();
/// assert_eq!(result.len(), 3);
/// assert!(result[0].starts_with("1."));
/// ```
#[derive(Debug, Clone, Default)]
pub struct QuestionListOutputParser;

impl OutputParser for QuestionListOutputParser {
    type Output = Vec<String>;

    fn parse(&self, text: &str) -> Result<Self::Output> {
        // Regex pattern to match lines starting with a digit followed by a period
        // Example matches: "1. Question?", "2. Another question?"
        let re = regex::Regex::new(r"\d+\..*?(?:\n|$)")
            .map_err(|e| Error::OutputParsing(format!("Regex compilation failed: {e}")))?;

        let questions: Vec<String> = re
            .find_iter(text)
            .map(|m| m.as_str().trim().to_string())
            .collect();

        Ok(questions)
    }

    fn get_format_instructions(&self) -> String {
        "Your response should be a numbered list of questions, one per line, with each question ending in a question mark. Example:\n1. First question?\n2. Second question?\n3. Third question?".to_string()
    }
}

#[async_trait]
impl Runnable for QuestionListOutputParser {
    type Input = String;
    type Output = Vec<String>;

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
    ) -> Result<std::pin::Pin<Box<dyn futures::Stream<Item = Result<Self::Output>> + Send>>> {
        let result = self.parse(&input)?;
        Ok(Box::pin(futures::stream::once(async move { Ok(result) })))
    }
}

impl Serializable for QuestionListOutputParser {
    fn lc_id(&self) -> Vec<String> {
        vec![
            "dashflow".to_string(),
            "output_parsers".to_string(),
            "list".to_string(),
            "QuestionListOutputParser".to_string(),
        ]
    }

    fn is_lc_serializable(&self) -> bool {
        true
    }

    fn to_json(&self) -> SerializedObject {
        // QuestionListOutputParser has no configuration
        SerializedObject::Constructor {
            lc: SERIALIZATION_VERSION,
            id: self.lc_id(),
            kwargs: serde_json::json!({}),
        }
    }
}

impl Deserializable for QuestionListOutputParser {
    fn from_json(value: &serde_json::Value) -> Result<Self>
    where
        Self: Sized,
    {
        let (_lc, id, _kwargs) = extract_serialized_fields(value)?;
        validate_id(
            &id,
            &[
                "dashflow".to_string(),
                "output_parsers".to_string(),
                "list".to_string(),
                "QuestionListOutputParser".to_string(),
            ],
        )?;
        Ok(QuestionListOutputParser)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // =========================================================================
    // CommaSeparatedListOutputParser Tests
    // =========================================================================

    #[test]
    fn test_comma_separated_simple() {
        let parser = CommaSeparatedListOutputParser;
        let result = parser.parse("foo, bar, baz").unwrap();
        assert_eq!(result, vec!["foo", "bar", "baz"]);
    }

    #[test]
    fn test_comma_separated_with_quotes() {
        let parser = CommaSeparatedListOutputParser;
        let result = parser.parse(r#""hello, world", test, "foo""#).unwrap();
        assert_eq!(result, vec!["hello, world", "test", "foo"]);
    }

    #[test]
    fn test_comma_separated_no_spaces() {
        let parser = CommaSeparatedListOutputParser;
        let result = parser.parse("a,b,c").unwrap();
        assert_eq!(result, vec!["a", "b", "c"]);
    }

    #[test]
    fn test_comma_separated_extra_spaces() {
        let parser = CommaSeparatedListOutputParser;
        let result = parser.parse("  a  ,  b  ,  c  ").unwrap();
        assert_eq!(result, vec!["a", "b", "c"]);
    }

    #[test]
    fn test_comma_separated_single_item() {
        let parser = CommaSeparatedListOutputParser;
        let result = parser.parse("single").unwrap();
        assert_eq!(result, vec!["single"]);
    }

    #[test]
    fn test_comma_separated_empty_string() {
        let parser = CommaSeparatedListOutputParser;
        let result = parser.parse("").unwrap();
        assert_eq!(result, vec![String::new()]);
    }

    #[test]
    fn test_comma_separated_format_instructions() {
        let parser = CommaSeparatedListOutputParser;
        let instructions = parser.get_format_instructions();
        assert!(instructions.contains("comma separated"));
    }

    #[test]
    fn test_comma_separated_lc_id() {
        let parser = CommaSeparatedListOutputParser;
        let id = parser.lc_id();
        assert_eq!(id.len(), 4);
        assert_eq!(id[3], "CommaSeparatedListOutputParser");
    }

    #[test]
    fn test_comma_separated_serializable() {
        let parser = CommaSeparatedListOutputParser;
        assert!(parser.is_lc_serializable());
    }

    // =========================================================================
    // NumberedListOutputParser Tests
    // =========================================================================

    #[test]
    fn test_numbered_list_basic() {
        let parser = NumberedListOutputParser::new();
        let text = "1. First item\n2. Second item\n3. Third item";
        let result = parser.parse(text).unwrap();
        assert_eq!(result, vec!["First item", "Second item", "Third item"]);
    }

    #[test]
    fn test_numbered_list_with_extra_text() {
        let parser = NumberedListOutputParser::new();
        let text = "Here are the items:\n1. First\n2. Second\nEnd of list.";
        let result = parser.parse(text).unwrap();
        assert_eq!(result, vec!["First", "Second"]);
    }

    #[test]
    fn test_numbered_list_double_digits() {
        let parser = NumberedListOutputParser::new();
        let text = "10. Item ten\n11. Item eleven";
        let result = parser.parse(text).unwrap();
        assert_eq!(result, vec!["Item ten", "Item eleven"]);
    }

    #[test]
    fn test_numbered_list_empty() {
        let parser = NumberedListOutputParser::new();
        let result = parser.parse("No numbered items here").unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_numbered_list_custom_pattern() {
        // Custom pattern for parenthesized numbers: (1) item
        let parser =
            NumberedListOutputParser::with_pattern(r"\((\d+)\)\s+([^\n]+)").unwrap();
        let text = "(1) First\n(2) Second";
        let result = parser.parse(text).unwrap();
        // The custom pattern captures the number, not the text after it
        assert_eq!(result, vec!["1", "2"]);
    }

    #[test]
    fn test_numbered_list_invalid_pattern() {
        let result = NumberedListOutputParser::with_pattern("[invalid");
        assert!(result.is_err());
    }

    #[test]
    fn test_numbered_list_default() {
        let parser = NumberedListOutputParser::default();
        let result = parser.parse("1. Test").unwrap();
        assert_eq!(result, vec!["Test"]);
    }

    #[test]
    fn test_numbered_list_format_instructions() {
        let parser = NumberedListOutputParser::new();
        let instructions = parser.get_format_instructions();
        assert!(instructions.contains("numbered list"));
    }

    #[test]
    fn test_numbered_list_lc_id() {
        let parser = NumberedListOutputParser::new();
        let id = parser.lc_id();
        assert_eq!(id[3], "NumberedListOutputParser");
    }

    // =========================================================================
    // MarkdownListOutputParser Tests
    // =========================================================================

    #[test]
    fn test_markdown_list_dashes() {
        let parser = MarkdownListOutputParser::new();
        let text = "- First item\n- Second item\n- Third item";
        let result = parser.parse(text).unwrap();
        assert_eq!(result, vec!["First item", "Second item", "Third item"]);
    }

    #[test]
    fn test_markdown_list_asterisks() {
        let parser = MarkdownListOutputParser::new();
        let text = "* First\n* Second\n* Third";
        let result = parser.parse(text).unwrap();
        assert_eq!(result, vec!["First", "Second", "Third"]);
    }

    #[test]
    fn test_markdown_list_mixed() {
        let parser = MarkdownListOutputParser::new();
        let text = "- Dash item\n* Asterisk item\n- Another dash";
        let result = parser.parse(text).unwrap();
        assert_eq!(result, vec!["Dash item", "Asterisk item", "Another dash"]);
    }

    #[test]
    fn test_markdown_list_with_indentation() {
        let parser = MarkdownListOutputParser::new();
        let text = "  - Indented item\n    - More indented";
        let result = parser.parse(text).unwrap();
        assert_eq!(result, vec!["Indented item", "More indented"]);
    }

    #[test]
    fn test_markdown_list_empty() {
        let parser = MarkdownListOutputParser::new();
        let result = parser.parse("No list here").unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_markdown_list_custom_pattern() {
        let parser = MarkdownListOutputParser::with_pattern(r">\s+(.+)").unwrap();
        let text = "> Quote one\n> Quote two";
        let result = parser.parse(text).unwrap();
        assert_eq!(result, vec!["Quote one", "Quote two"]);
    }

    #[test]
    fn test_markdown_list_invalid_pattern() {
        let result = MarkdownListOutputParser::with_pattern("[invalid");
        assert!(result.is_err());
    }

    #[test]
    fn test_markdown_list_default() {
        let parser = MarkdownListOutputParser::default();
        let result = parser.parse("- Test").unwrap();
        assert_eq!(result, vec!["Test"]);
    }

    #[test]
    fn test_markdown_list_format_instructions() {
        let parser = MarkdownListOutputParser::new();
        let instructions = parser.get_format_instructions();
        assert!(instructions.contains("markdown list"));
    }

    // =========================================================================
    // LineListOutputParser Tests
    // =========================================================================

    #[test]
    fn test_line_list_basic() {
        let parser = LineListOutputParser;
        let result = parser.parse("apple\nbanana\ncherry").unwrap();
        assert_eq!(result, vec!["apple", "banana", "cherry"]);
    }

    #[test]
    fn test_line_list_with_empty_lines() {
        let parser = LineListOutputParser;
        let result = parser.parse("apple\n\nbanana\n\ncherry").unwrap();
        assert_eq!(result, vec!["apple", "banana", "cherry"]);
    }

    #[test]
    fn test_line_list_with_whitespace() {
        let parser = LineListOutputParser;
        let result = parser.parse("  apple  \n  banana  \n  cherry  ").unwrap();
        assert_eq!(result, vec!["apple", "banana", "cherry"]);
    }

    #[test]
    fn test_line_list_single_line() {
        let parser = LineListOutputParser;
        let result = parser.parse("single item").unwrap();
        assert_eq!(result, vec!["single item"]);
    }

    #[test]
    fn test_line_list_empty_string() {
        let parser = LineListOutputParser;
        let result = parser.parse("").unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_line_list_only_whitespace() {
        let parser = LineListOutputParser;
        let result = parser.parse("   \n   \n   ").unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_line_list_format_instructions() {
        let parser = LineListOutputParser;
        let instructions = parser.get_format_instructions();
        assert!(instructions.contains("one per line"));
    }

    #[test]
    fn test_line_list_lc_id() {
        let parser = LineListOutputParser;
        let id = parser.lc_id();
        assert_eq!(id[3], "LineListOutputParser");
    }

    #[test]
    fn test_line_list_default() {
        let parser = LineListOutputParser::default();
        let result = parser.parse("a\nb").unwrap();
        assert_eq!(result, vec!["a", "b"]);
    }

    // =========================================================================
    // QuestionListOutputParser Tests
    // =========================================================================

    #[test]
    fn test_question_list_basic() {
        let parser = QuestionListOutputParser;
        let text = "1. What is DashFlow?\n2. How does it work?\n3. What are the features?";
        let result = parser.parse(text).unwrap();
        assert_eq!(result.len(), 3);
        assert!(result[0].starts_with("1."));
        assert!(result[1].starts_with("2."));
        assert!(result[2].starts_with("3."));
    }

    #[test]
    fn test_question_list_with_preamble() {
        let parser = QuestionListOutputParser;
        let text = "Here are some questions:\n1. Question one?\n2. Question two?";
        let result = parser.parse(text).unwrap();
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn test_question_list_double_digits() {
        let parser = QuestionListOutputParser;
        let text = "10. Question ten\n11. Question eleven";
        let result = parser.parse(text).unwrap();
        assert_eq!(result.len(), 2);
        assert!(result[0].starts_with("10."));
    }

    #[test]
    fn test_question_list_empty() {
        let parser = QuestionListOutputParser;
        let result = parser.parse("No questions here").unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_question_list_format_instructions() {
        let parser = QuestionListOutputParser;
        let instructions = parser.get_format_instructions();
        assert!(instructions.contains("numbered list"));
        assert!(instructions.contains("question"));
    }

    #[test]
    fn test_question_list_lc_id() {
        let parser = QuestionListOutputParser;
        let id = parser.lc_id();
        assert_eq!(id[3], "QuestionListOutputParser");
    }

    #[test]
    fn test_question_list_default() {
        let parser = QuestionListOutputParser::default();
        let result = parser.parse("1. Test?").unwrap();
        assert_eq!(result.len(), 1);
    }

    // =========================================================================
    // Serialization/Deserialization Tests
    // =========================================================================

    #[test]
    fn test_comma_separated_roundtrip() {
        let parser = CommaSeparatedListOutputParser;
        let json = parser.to_json();
        if let SerializedObject::Constructor { kwargs, .. } = json {
            let value = serde_json::json!({
                "lc": 1,
                "type": "constructor",
                "id": ["dashflow", "output_parsers", "list", "CommaSeparatedListOutputParser"],
                "kwargs": kwargs
            });
            let restored = CommaSeparatedListOutputParser::from_json(&value).unwrap();
            // Verify it works the same
            let orig_result = parser.parse("a, b").unwrap();
            let restored_result = restored.parse("a, b").unwrap();
            assert_eq!(orig_result, restored_result);
        }
    }

    #[test]
    fn test_numbered_list_roundtrip_default() {
        let parser = NumberedListOutputParser::new();
        let json = parser.to_json();
        if let SerializedObject::Constructor { kwargs, .. } = json {
            let value = serde_json::json!({
                "lc": 1,
                "type": "constructor",
                "id": ["dashflow", "output_parsers", "list", "NumberedListOutputParser"],
                "kwargs": kwargs
            });
            let restored = NumberedListOutputParser::from_json(&value).unwrap();
            let result = restored.parse("1. Test").unwrap();
            assert_eq!(result, vec!["Test"]);
        }
    }

    #[test]
    fn test_line_list_roundtrip() {
        let parser = LineListOutputParser;
        let json = parser.to_json();
        if let SerializedObject::Constructor { kwargs, .. } = json {
            let value = serde_json::json!({
                "lc": 1,
                "type": "constructor",
                "id": ["dashflow", "output_parsers", "list", "LineListOutputParser"],
                "kwargs": kwargs
            });
            let restored = LineListOutputParser::from_json(&value).unwrap();
            let result = restored.parse("a\nb").unwrap();
            assert_eq!(result, vec!["a", "b"]);
        }
    }

    #[test]
    fn test_question_list_roundtrip() {
        let parser = QuestionListOutputParser;
        let json = parser.to_json();
        if let SerializedObject::Constructor { kwargs, .. } = json {
            let value = serde_json::json!({
                "lc": 1,
                "type": "constructor",
                "id": ["dashflow", "output_parsers", "list", "QuestionListOutputParser"],
                "kwargs": kwargs
            });
            let restored = QuestionListOutputParser::from_json(&value).unwrap();
            let result = restored.parse("1. Test?").unwrap();
            assert_eq!(result.len(), 1);
        }
    }

    // =========================================================================
    // Async Runnable Tests
    // =========================================================================

    #[tokio::test]
    async fn test_comma_separated_invoke() {
        let parser = CommaSeparatedListOutputParser;
        let result = parser.invoke("a, b, c".to_string(), None).await.unwrap();
        assert_eq!(result, vec!["a", "b", "c"]);
    }

    #[tokio::test]
    async fn test_numbered_list_invoke() {
        let parser = NumberedListOutputParser::new();
        let result = parser
            .invoke("1. First\n2. Second".to_string(), None)
            .await
            .unwrap();
        assert_eq!(result, vec!["First", "Second"]);
    }

    #[tokio::test]
    async fn test_markdown_list_invoke() {
        let parser = MarkdownListOutputParser::new();
        let result = parser.invoke("- Item".to_string(), None).await.unwrap();
        assert_eq!(result, vec!["Item"]);
    }

    #[tokio::test]
    async fn test_line_list_invoke() {
        let parser = LineListOutputParser;
        let result = parser.invoke("a\nb".to_string(), None).await.unwrap();
        assert_eq!(result, vec!["a", "b"]);
    }

    #[tokio::test]
    async fn test_question_list_invoke() {
        let parser = QuestionListOutputParser;
        let result = parser.invoke("1. Test?".to_string(), None).await.unwrap();
        assert_eq!(result.len(), 1);
    }

    #[tokio::test]
    async fn test_comma_separated_batch() {
        let parser = CommaSeparatedListOutputParser;
        let inputs = vec!["a, b".to_string(), "c, d".to_string()];
        let results = parser.batch(inputs, None).await.unwrap();
        assert_eq!(results.len(), 2);
        assert_eq!(results[0], vec!["a", "b"]);
        assert_eq!(results[1], vec!["c", "d"]);
    }

    #[tokio::test]
    async fn test_line_list_batch() {
        let parser = LineListOutputParser;
        let inputs = vec!["a\nb".to_string(), "c\nd".to_string()];
        let results = parser.batch(inputs, None).await.unwrap();
        assert_eq!(results.len(), 2);
        assert_eq!(results[0], vec!["a", "b"]);
        assert_eq!(results[1], vec!["c", "d"]);
    }
}
