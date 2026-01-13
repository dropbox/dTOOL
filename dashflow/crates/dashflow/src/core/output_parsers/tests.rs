// Import everything from parent module (output_parsers/mod.rs)
use super::*;

use chrono::{Datelike, Timelike};

#[test]
fn test_str_output_parser() {
    let parser = StrOutputParser;
    assert_eq!(parser.parse("hello").unwrap(), "hello");
    assert_eq!(parser.parse("").unwrap(), "");
    assert_eq!(
        parser.parse("multi\nline\ntext").unwrap(),
        "multi\nline\ntext"
    );
}

#[test]
fn test_json_output_parser_plain() {
    let parser = JsonOutputParser::new();

    let result = parser.parse(r#"{"name": "Alice", "age": 30}"#).unwrap();
    assert_eq!(result["name"], "Alice");
    assert_eq!(result["age"], 30);
}

#[test]
fn test_json_output_parser_markdown() {
    let parser = JsonOutputParser::new();

    // Multi-line Markdown code block
    let markdown = r#"```json
{
  "name": "Bob",
  "age": 25
}
```"#;
    let result = parser.parse(markdown).unwrap();
    assert_eq!(result["name"], "Bob");
    assert_eq!(result["age"], 25);

    // Inline Markdown code block
    let inline = r#"```{"name": "Carol"}```"#;
    let result = parser.parse(inline).unwrap();
    assert_eq!(result["name"], "Carol");
}

#[test]
fn test_json_output_parser_invalid() {
    let parser = JsonOutputParser::new();
    assert!(parser.parse("not json").is_err());
    assert!(parser.parse("{invalid}").is_err());
}

#[test]
fn test_comma_separated_list_parser() {
    let parser = CommaSeparatedListOutputParser;

    // Simple case
    let result = parser.parse("foo, bar, baz").unwrap();
    assert_eq!(result, vec!["foo", "bar", "baz"]);

    // No spaces
    let result = parser.parse("foo,bar,baz").unwrap();
    assert_eq!(result, vec!["foo", "bar", "baz"]);

    // With quotes
    let result = parser.parse(r#""hello, world", test, "foo""#).unwrap();
    assert_eq!(result, vec!["hello, world", "test", "foo"]);

    // Single item
    let result = parser.parse("single").unwrap();
    assert_eq!(result, vec!["single"]);

    // Empty string
    let result = parser.parse("").unwrap();
    assert_eq!(result, vec![""]);
}

#[test]
fn test_numbered_list_parser() {
    let parser = NumberedListOutputParser::new();

    let text = "1. First item\n2. Second item\n3. Third item";
    let result = parser.parse(text).unwrap();
    assert_eq!(result, vec!["First item", "Second item", "Third item"]);

    // With extra spacing
    let text = "1.  First\n2.  Second";
    let result = parser.parse(text).unwrap();
    assert_eq!(result, vec!["First", "Second"]);

    // No matches
    let result = parser.parse("no numbered list here").unwrap();
    assert!(result.is_empty());
}

#[test]
fn test_markdown_list_parser() {
    let parser = MarkdownListOutputParser::new();

    // Dash bullets
    let text = "- First item\n- Second item\n- Third item";
    let result = parser.parse(text).unwrap();
    assert_eq!(result, vec!["First item", "Second item", "Third item"]);

    // Star bullets
    let text = "* First\n* Second";
    let result = parser.parse(text).unwrap();
    assert_eq!(result, vec!["First", "Second"]);

    // Mixed
    let text = "- First\n* Second\n- Third";
    let result = parser.parse(text).unwrap();
    assert_eq!(result, vec!["First", "Second", "Third"]);

    // With indentation
    let text = "  - Indented\n    * Also indented";
    let result = parser.parse(text).unwrap();
    assert_eq!(result, vec!["Indented", "Also indented"]);

    // No matches
    let result = parser.parse("no list here").unwrap();
    assert!(result.is_empty());
}

#[tokio::test]
async fn test_str_parser_runnable() {
    let parser = StrOutputParser;
    let result = parser.invoke("test".to_string(), None).await.unwrap();
    assert_eq!(result, "test");

    let batch_result = parser
        .batch(vec!["a".to_string(), "b".to_string()], None)
        .await
        .unwrap();
    assert_eq!(batch_result, vec!["a", "b"]);
}

#[tokio::test]
async fn test_json_parser_runnable() {
    let parser = JsonOutputParser::new();
    let result = parser
        .invoke(r#"{"key": "value"}"#.to_string(), None)
        .await
        .unwrap();
    assert_eq!(result["key"], "value");
}

#[tokio::test]
async fn test_list_parser_runnable() {
    let parser = CommaSeparatedListOutputParser;
    let result = parser.invoke("a, b, c".to_string(), None).await.unwrap();
    assert_eq!(result, vec!["a", "b", "c"]);
}

#[test]
fn test_line_list_parser() {
    let parser = LineListOutputParser;

    // Basic newline-separated list
    let text = "apple\nbanana\ncherry";
    let result = parser.parse(text).unwrap();
    assert_eq!(result, vec!["apple", "banana", "cherry"]);

    // With extra whitespace
    let text = "  apple  \n  banana  \n  cherry  ";
    let result = parser.parse(text).unwrap();
    assert_eq!(result, vec!["apple", "banana", "cherry"]);

    // With empty lines
    let text = "apple\n\nbanana\n\n\ncherry";
    let result = parser.parse(text).unwrap();
    assert_eq!(result, vec!["apple", "banana", "cherry"]);

    // Single item
    let text = "single item";
    let result = parser.parse(text).unwrap();
    assert_eq!(result, vec!["single item"]);

    // Empty string
    let text = "";
    let result = parser.parse(text).unwrap();
    assert!(result.is_empty());
}

#[tokio::test]
async fn test_line_list_parser_runnable() {
    let parser = LineListOutputParser;
    let result = parser
        .invoke("line1\nline2\nline3".to_string(), None)
        .await
        .unwrap();
    assert_eq!(result, vec!["line1", "line2", "line3"]);
}

// Serialization tests
#[test]
fn test_str_output_parser_serialization() {
    let parser = StrOutputParser;
    assert!(parser.is_lc_serializable());

    let serialized = parser.to_json();
    assert_eq!(
        serialized.id(),
        &["dashflow", "output_parsers", "string", "StrOutputParser"]
    );
    assert!(serialized.is_constructor());

    let json_value = parser.to_json_value().unwrap();
    assert_eq!(json_value["type"], "constructor");
    assert_eq!(json_value["lc"], 1);
    assert_eq!(json_value["kwargs"], serde_json::json!({}));
}

#[test]
fn test_json_output_parser_serialization() {
    let parser = JsonOutputParser::new();
    assert!(parser.is_lc_serializable());

    let serialized = parser.to_json();
    assert_eq!(
        serialized.id(),
        &["dashflow", "output_parsers", "json", "JsonOutputParser"]
    );

    let json_str = parser.to_json_string(true).unwrap();
    assert!(json_str.contains("JsonOutputParser"));
    assert!(
        json_str.contains("\"type\":\"constructor\"")
            || json_str.contains("\"type\": \"constructor\"")
    );
}

#[test]
fn test_comma_separated_list_parser_serialization() {
    let parser = CommaSeparatedListOutputParser;
    assert!(parser.is_lc_serializable());

    let serialized = parser.to_json();
    assert_eq!(
        serialized.id(),
        &[
            "dashflow",
            "output_parsers",
            "list",
            "CommaSeparatedListOutputParser"
        ]
    );

    let json_value = parser.to_json_value().unwrap();
    assert_eq!(json_value["lc"], 1);
    assert_eq!(json_value["kwargs"], serde_json::json!({}));
}

#[test]
fn test_line_list_parser_serialization() {
    let parser = LineListOutputParser;
    assert!(parser.is_lc_serializable());

    let serialized = parser.to_json();
    assert_eq!(
        serialized.id(),
        &["dashflow", "output_parsers", "list", "LineListOutputParser"]
    );
}

#[test]
fn test_numbered_list_parser_serialization_default() {
    let parser = NumberedListOutputParser::new();
    assert!(parser.is_lc_serializable());

    let serialized = parser.to_json();
    assert_eq!(
        serialized.id(),
        &[
            "dashflow",
            "output_parsers",
            "list",
            "NumberedListOutputParser"
        ]
    );

    // Default pattern should not be serialized (empty kwargs)
    let json_value = parser.to_json_value().unwrap();
    assert_eq!(json_value["kwargs"], serde_json::json!({}));
}

#[test]
fn test_numbered_list_parser_serialization_custom_pattern() {
    let custom_pattern = r"\d+\)\s+(.+)"; // Custom pattern: 1) item instead of 1. item
    let parser = NumberedListOutputParser::with_pattern(custom_pattern).unwrap();
    assert!(parser.is_lc_serializable());

    let json_value = parser.to_json_value().unwrap();
    // Custom pattern should be serialized
    assert_eq!(json_value["kwargs"]["pattern"], custom_pattern);
}

#[test]
fn test_markdown_list_parser_serialization_default() {
    let parser = MarkdownListOutputParser::new();
    assert!(parser.is_lc_serializable());

    let serialized = parser.to_json();
    assert_eq!(
        serialized.id(),
        &[
            "dashflow",
            "output_parsers",
            "list",
            "MarkdownListOutputParser"
        ]
    );

    // Default pattern should not be serialized (empty kwargs)
    let json_value = parser.to_json_value().unwrap();
    assert_eq!(json_value["kwargs"], serde_json::json!({}));
}

#[test]
fn test_markdown_list_parser_serialization_custom_pattern() {
    let custom_pattern = r"^\+\s+(.+)$"; // Custom pattern: + item instead of - or *
    let parser = MarkdownListOutputParser::with_pattern(custom_pattern).unwrap();
    assert!(parser.is_lc_serializable());

    let json_value = parser.to_json_value().unwrap();
    // Custom pattern should be serialized
    assert_eq!(json_value["kwargs"]["pattern"], custom_pattern);
}

#[test]
fn test_parser_serialization_roundtrip_json() {
    // Test that serialization produces valid JSON
    let parsers: Vec<Box<dyn Serializable>> = vec![
        Box::new(StrOutputParser),
        Box::new(JsonOutputParser::new()),
        Box::new(CommaSeparatedListOutputParser),
        Box::new(LineListOutputParser),
        Box::new(NumberedListOutputParser::new()),
        Box::new(MarkdownListOutputParser::new()),
    ];

    for parser in parsers {
        let json_str = parser.to_json_string(false).unwrap();
        // Verify it's valid JSON by parsing it back
        let parsed: serde_json::Value = serde_json::from_str(&json_str).unwrap();
        assert_eq!(parsed["lc"], 1);
        assert_eq!(parsed["type"], "constructor");
    }
}

#[test]
fn test_parser_no_secrets() {
    // Parsers should not have any secrets
    let parser = JsonOutputParser::new();
    assert!(parser.lc_secrets().is_empty());

    let parser = NumberedListOutputParser::new();
    assert!(parser.lc_secrets().is_empty());
}

// Deserialization tests
use crate::core::deserialization::{from_json_str, Deserializable};

#[test]
fn test_str_output_parser_deserialization() {
    let json = serde_json::json!({
        "lc": 1,
        "type": "constructor",
        "id": ["dashflow", "output_parsers", "string", "StrOutputParser"],
        "kwargs": {}
    });

    let parser = StrOutputParser::from_json(&json).unwrap();
    assert_eq!(parser.parse("test").unwrap(), "test");
}

#[test]
fn test_json_output_parser_deserialization() {
    let json = serde_json::json!({
        "lc": 1,
        "type": "constructor",
        "id": ["dashflow", "output_parsers", "json", "JsonOutputParser"],
        "kwargs": {}
    });

    let parser = JsonOutputParser::from_json(&json).unwrap();
    let result: serde_json::Value = parser.parse(r#"{"key": "value"}"#).unwrap();
    assert_eq!(result["key"], "value");
}

#[test]
fn test_comma_separated_list_parser_deserialization() {
    let json = serde_json::json!({
        "lc": 1,
        "type": "constructor",
        "id": ["dashflow", "output_parsers", "list", "CommaSeparatedListOutputParser"],
        "kwargs": {}
    });

    let parser = CommaSeparatedListOutputParser::from_json(&json).unwrap();
    let result = parser.parse("a,b,c").unwrap();
    assert_eq!(result, vec!["a", "b", "c"]);
}

#[test]
fn test_line_list_parser_deserialization() {
    let json = serde_json::json!({
        "lc": 1,
        "type": "constructor",
        "id": ["dashflow", "output_parsers", "list", "LineListOutputParser"],
        "kwargs": {}
    });

    let parser = LineListOutputParser::from_json(&json).unwrap();
    let result = parser.parse("a\nb\nc").unwrap();
    assert_eq!(result, vec!["a", "b", "c"]);
}

#[test]
fn test_numbered_list_parser_deserialization_default() {
    let json = serde_json::json!({
        "lc": 1,
        "type": "constructor",
        "id": ["dashflow", "output_parsers", "list", "NumberedListOutputParser"],
        "kwargs": {}
    });

    let parser = NumberedListOutputParser::from_json(&json).unwrap();
    let result = parser.parse("1. First\n2. Second\n3. Third").unwrap();
    assert_eq!(result, vec!["First", "Second", "Third"]);
}

#[test]
fn test_numbered_list_parser_deserialization_custom_pattern() {
    let json = serde_json::json!({
        "lc": 1,
        "type": "constructor",
        "id": ["dashflow", "output_parsers", "list", "NumberedListOutputParser"],
        "kwargs": {
            "pattern": r"\d+\)\s+(.+)"
        }
    });

    let parser = NumberedListOutputParser::from_json(&json).unwrap();
    let result = parser.parse("1) First\n2) Second").unwrap();
    assert_eq!(result, vec!["First", "Second"]);
}

#[test]
fn test_markdown_list_parser_deserialization_default() {
    let json = serde_json::json!({
        "lc": 1,
        "type": "constructor",
        "id": ["dashflow", "output_parsers", "list", "MarkdownListOutputParser"],
        "kwargs": {}
    });

    let parser = MarkdownListOutputParser::from_json(&json).unwrap();
    let result = parser.parse("- First\n- Second\n* Third").unwrap();
    assert_eq!(result, vec!["First", "Second", "Third"]);
}

#[test]
fn test_parser_roundtrip_serialization_deserialization() {
    // Test StrOutputParser
    let original_str = StrOutputParser;
    let json_str = original_str.to_json_string(false).unwrap();
    let reconstructed: StrOutputParser = from_json_str(&json_str).unwrap();
    assert_eq!(reconstructed.parse("test").unwrap(), "test");

    // Test NumberedListOutputParser with custom pattern
    let original_numbered = NumberedListOutputParser::with_pattern(r"\d+\)\s+(.+)").unwrap();
    let json_numbered = original_numbered.to_json_string(false).unwrap();
    let reconstructed_numbered: NumberedListOutputParser = from_json_str(&json_numbered).unwrap();
    assert_eq!(
        reconstructed_numbered.parse("1) First\n2) Second").unwrap(),
        vec!["First", "Second"]
    );
}

#[test]
fn test_parser_deserialization_type_mismatch() {
    let json = serde_json::json!({
        "lc": 1,
        "type": "constructor",
        "id": ["dashflow", "wrong", "Type"],
        "kwargs": {}
    });

    let result = StrOutputParser::from_json(&json);
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("Type mismatch"));
}

// ============================================================================
// XMLOutputParser Tests
// ============================================================================

#[test]
fn test_xml_parser_simple_text_element() {
    let parser = XMLOutputParser::new();

    // Single text element
    let xml = "<name>Alice</name>";
    let result = parser.parse(xml).unwrap();
    assert_eq!(result["name"], "Alice");
}

#[test]
fn test_xml_parser_nested_elements() {
    let parser = XMLOutputParser::new();

    // Nested structure
    let xml = "<person><name>Alice</name><age>30</age></person>";
    let result = parser.parse(xml).unwrap();

    // person should be an array of children
    let person = result["person"].as_array().unwrap();
    assert_eq!(person.len(), 2);
    assert_eq!(person[0]["name"], "Alice");
    assert_eq!(person[1]["age"], "30");
}

#[test]
fn test_xml_parser_deeply_nested() {
    let parser = XMLOutputParser::new();

    // Deep nesting
    let xml = r#"<foo>
        <bar>
            <baz>content</baz>
        </bar>
    </foo>"#;
    let result = parser.parse(xml).unwrap();

    let foo = result["foo"].as_array().unwrap();
    let bar = foo[0]["bar"].as_array().unwrap();
    let baz = &bar[0]["baz"];
    assert_eq!(baz, "content");
}

#[test]
fn test_xml_parser_markdown_code_block() {
    let parser = XMLOutputParser::new();

    // XML in markdown code block
    let markdown = r#"```xml
<person>
<name>Bob</name>
</person>
```"#;
    let result = parser.parse(markdown).unwrap();

    let person = result["person"].as_array().unwrap();
    assert_eq!(person[0]["name"], "Bob");
}

#[test]
fn test_xml_parser_markdown_plain_code_block() {
    let parser = XMLOutputParser::new();

    // XML in plain code block (no xml tag)
    let markdown = r#"```
<data>
<value>42</value>
</data>
```"#;
    let result = parser.parse(markdown).unwrap();

    let data = result["data"].as_array().unwrap();
    assert_eq!(data[0]["value"], "42");
}

#[test]
fn test_xml_parser_empty_element() {
    let parser = XMLOutputParser::new();

    // Empty element
    let xml = "<empty></empty>";
    let result = parser.parse(xml).unwrap();
    assert_eq!(result["empty"], serde_json::Value::Null);
}

#[test]
fn test_xml_parser_self_closing_element() {
    let parser = XMLOutputParser::new();

    // Self-closing element
    let xml = "<empty/>";
    let result = parser.parse(xml).unwrap();
    assert_eq!(result["empty"], serde_json::Value::Null);
}

#[test]
fn test_xml_parser_multiple_children_same_tag() {
    let parser = XMLOutputParser::new();

    // Multiple children with same tag name
    let xml = r#"<items>
        <item>first</item>
        <item>second</item>
        <item>third</item>
    </items>"#;
    let result = parser.parse(xml).unwrap();

    let items = result["items"].as_array().unwrap();
    assert_eq!(items.len(), 3);
    assert_eq!(items[0]["item"], "first");
    assert_eq!(items[1]["item"], "second");
    assert_eq!(items[2]["item"], "third");
}

#[test]
fn test_xml_parser_with_xml_declaration() {
    let parser = XMLOutputParser::new();

    // XML with declaration
    let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<root>
<data>content</data>
</root>"#;
    let result = parser.parse(xml).unwrap();

    let root = result["root"].as_array().unwrap();
    assert_eq!(root[0]["data"], "content");
}

#[test]
fn test_xml_parser_malformed_xml() {
    let parser = XMLOutputParser::new();

    // Mismatched tags
    let xml = "<foo><bar></foo>";
    let result = parser.parse(xml);
    assert!(result.is_err());
    // quick-xml catches this as a parsing error, not our custom error
    let err_msg = result.unwrap_err().to_string();
    assert!(err_msg.contains("XML parsing error") || err_msg.contains("parse"));
}

#[test]
fn test_xml_parser_no_root_element() {
    let parser = XMLOutputParser::new();

    // No XML content
    let xml = "just plain text";
    let result = parser.parse(xml);
    assert!(result.is_err());
}

#[test]
fn test_xml_parser_depth_limit() {
    let parser = XMLOutputParser::new();

    // Create XML deeper than MAX_XML_DEPTH (100)
    // depth 101 should fail: <a><b><c>...(101 levels)...</c></b></a>
    let depth = 110; // Well over the 100 limit
    let open_tags: String = (0..depth).map(|i| format!("<t{}>", i)).collect();
    let close_tags: String = (0..depth).rev().map(|i| format!("</t{}>", i)).collect();
    let deeply_nested = format!("{}{}", open_tags, close_tags);

    let result = parser.parse(&deeply_nested);
    assert!(result.is_err(), "Deeply nested XML should fail");

    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg.contains("too deep") || err_msg.contains("nesting"),
        "Error should mention depth: {}",
        err_msg
    );
}

#[test]
fn test_xml_parser_depth_limit_allows_valid_nesting() {
    let parser = XMLOutputParser::new();

    // Create XML at exactly the limit (should succeed)
    // Depth of 50 is well within limit
    let depth = 50;
    let open_tags: String = (0..depth).map(|i| format!("<t{}>", i)).collect();
    let close_tags: String = (0..depth).rev().map(|i| format!("</t{}>", i)).collect();
    let nested = format!("{}content{}", open_tags, close_tags);

    let result = parser.parse(&nested);
    assert!(
        result.is_ok(),
        "Valid nested XML should succeed: {:?}",
        result.err()
    );
}

#[test]
fn test_xml_parser_format_instructions_without_tags() {
    let parser = XMLOutputParser::new();
    let instructions = parser.get_format_instructions();

    assert!(instructions.contains("XML file"));
    assert!(instructions.contains("(No specific tags required)"));
}

#[test]
fn test_xml_parser_format_instructions_with_tags() {
    let parser = XMLOutputParser::with_tags(vec![
        "foo".to_string(),
        "bar".to_string(),
        "baz".to_string(),
    ]);
    let instructions = parser.get_format_instructions();

    assert!(instructions.contains("XML file"));
    assert!(instructions.contains("foo"));
    assert!(instructions.contains("bar"));
    assert!(instructions.contains("baz"));
}

#[test]
fn test_xml_parser_serialization() {
    let parser = XMLOutputParser::new();
    let serialized = parser.to_json();

    assert_eq!(
        serialized.id(),
        &["dashflow", "output_parsers", "xml", "XMLOutputParser"]
    );
    assert!(serialized.is_constructor());
}

#[test]
fn test_xml_parser_serialization_with_tags() {
    let parser = XMLOutputParser::with_tags(vec!["foo".to_string(), "bar".to_string()]);
    let json_value = parser.to_json_value().unwrap();

    assert_eq!(json_value["type"], "constructor");
    assert_eq!(
        json_value["id"],
        serde_json::json!(["dashflow", "output_parsers", "xml", "XMLOutputParser"])
    );

    let tags = json_value["kwargs"]["tags"].as_array().unwrap();
    assert_eq!(tags.len(), 2);
    assert_eq!(tags[0], "foo");
    assert_eq!(tags[1], "bar");
}

#[test]
fn test_xml_parser_deserialization() {
    let json = serde_json::json!({
        "lc": 1,
        "type": "constructor",
        "id": ["dashflow", "output_parsers", "xml", "XMLOutputParser"],
        "kwargs": {}
    });

    let parser = XMLOutputParser::from_json(&json).unwrap();
    assert!(parser.tags.is_none());

    let result = parser.parse("<test>value</test>").unwrap();
    assert_eq!(result["test"], "value");
}

#[test]
fn test_xml_parser_deserialization_with_tags() {
    let json = serde_json::json!({
        "lc": 1,
        "type": "constructor",
        "id": ["dashflow", "output_parsers", "xml", "XMLOutputParser"],
        "kwargs": {
            "tags": ["foo", "bar"]
        }
    });

    let parser = XMLOutputParser::from_json(&json).unwrap();
    assert_eq!(
        parser.tags,
        Some(vec!["foo".to_string(), "bar".to_string()])
    );
}

#[tokio::test]
async fn test_xml_parser_as_runnable() {
    let parser = XMLOutputParser::new();
    let xml = "<name>Alice</name>".to_string();

    let result = parser.invoke(xml, None).await.unwrap();
    assert_eq!(result["name"], "Alice");
}

#[tokio::test]
async fn test_xml_parser_batch() {
    let parser = XMLOutputParser::new();
    let inputs = vec![
        "<name>Alice</name>".to_string(),
        "<name>Bob</name>".to_string(),
    ];

    let results = parser.batch(inputs, None).await.unwrap();
    assert_eq!(results.len(), 2);
    assert_eq!(results[0]["name"], "Alice");
    assert_eq!(results[1]["name"], "Bob");
}

#[test]
fn test_datetime_parser_default_format() {
    let parser = DatetimeOutputParser::new();

    // Test ISO 8601 format
    let result = parser.parse("2023-07-04T14:30:00.000000Z").unwrap();
    assert_eq!(result.year(), 2023);
    assert_eq!(result.month(), 7);
    assert_eq!(result.day(), 4);
    assert_eq!(result.hour(), 14);
    assert_eq!(result.minute(), 30);
    assert_eq!(result.second(), 0);
}

#[test]
fn test_datetime_parser_custom_format() {
    let parser = DatetimeOutputParser::with_format("%Y-%m-%d");

    let result = parser.parse("2023-07-04").unwrap();
    assert_eq!(result.year(), 2023);
    assert_eq!(result.month(), 7);
    assert_eq!(result.day(), 4);
}

#[test]
fn test_datetime_parser_with_timezone() {
    let parser = DatetimeOutputParser::with_format("%Y-%m-%d %H:%M:%S %z");

    let result = parser.parse("2023-07-04 14:30:00 +0000").unwrap();
    assert_eq!(result.year(), 2023);
    assert_eq!(result.month(), 7);
    assert_eq!(result.hour(), 14);
}

#[test]
fn test_datetime_parser_error() {
    let parser = DatetimeOutputParser::new();

    let result = parser.parse("invalid datetime");
    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("Could not parse datetime string"));
}

#[test]
fn test_datetime_parser_format_instructions() {
    let parser = DatetimeOutputParser::new();
    let instructions = parser.get_format_instructions();

    assert!(instructions.contains("%Y-%m-%dT%H:%M:%S%.fZ"));
    assert!(instructions.contains("Examples:"));
    assert!(instructions.contains("2023-07-04T14:30:00.000000Z"));
}

#[test]
fn test_datetime_parser_custom_format_instructions() {
    let parser = DatetimeOutputParser::with_format("%Y/%m/%d");
    let instructions = parser.get_format_instructions();

    assert!(instructions.contains("%Y/%m/%d"));
    assert!(instructions.contains("Examples:"));
}

#[test]
fn test_datetime_parser_serialization() {
    let parser = DatetimeOutputParser::with_format("%Y-%m-%d %H:%M:%S");
    let json_value = parser.to_json();

    let SerializedObject::Constructor { lc, id, kwargs } = json_value else {
        panic!("Expected Constructor variant");
    };

    assert_eq!(lc, SERIALIZATION_VERSION);
    assert_eq!(
        id,
        vec![
            "dashflow".to_string(),
            "output_parsers".to_string(),
            "datetime".to_string(),
            "DatetimeOutputParser".to_string()
        ]
    );
    assert_eq!(kwargs["format"], "%Y-%m-%d %H:%M:%S");
}

#[test]
fn test_datetime_parser_deserialization() {
    let json = serde_json::json!({
        "lc": 1,
        "type": "constructor",
        "id": ["dashflow", "output_parsers", "datetime", "DatetimeOutputParser"],
        "kwargs": {
            "format": "%Y-%m-%d"
        }
    });

    let parser = DatetimeOutputParser::from_json(&json).unwrap();
    assert_eq!(parser.format, "%Y-%m-%d");

    let result = parser.parse("2023-07-04").unwrap();
    assert_eq!(result.year(), 2023);
}

#[test]
fn test_datetime_parser_deserialization_default_format() {
    let json = serde_json::json!({
        "lc": 1,
        "type": "constructor",
        "id": ["dashflow", "output_parsers", "datetime", "DatetimeOutputParser"],
        "kwargs": {}
    });

    let parser = DatetimeOutputParser::from_json(&json).unwrap();
    assert_eq!(parser.format, "%Y-%m-%dT%H:%M:%S%.fZ");
}

#[tokio::test]
async fn test_datetime_parser_as_runnable() {
    let parser = DatetimeOutputParser::new();
    let input = "2023-07-04T14:30:00.000000Z".to_string();

    let result = parser.invoke(input, None).await.unwrap();
    assert_eq!(result.year(), 2023);
    assert_eq!(result.month(), 7);
}

#[tokio::test]
async fn test_datetime_parser_batch() {
    let parser = DatetimeOutputParser::new();
    let inputs = vec![
        "2023-07-04T14:30:00.000000Z".to_string(),
        "1999-12-31T23:59:59.999999Z".to_string(),
    ];

    let results = parser.batch(inputs, None).await.unwrap();
    assert_eq!(results.len(), 2);
    assert_eq!(results[0].year(), 2023);
    assert_eq!(results[1].year(), 1999);
}

#[test]
fn test_yaml_parser_plain() {
    let parser = YamlOutputParser::new();

    let yaml = "name: Alice\nage: 30\ncity: Seattle";
    let result = parser.parse(yaml).unwrap();

    assert_eq!(result["name"], "Alice");
    assert_eq!(result["age"], 30);
    assert_eq!(result["city"], "Seattle");
}

#[test]
fn test_yaml_parser_markdown_yaml() {
    let parser = YamlOutputParser::new();

    let yaml = "```yaml\nname: Bob\nage: 25\n```";
    let result = parser.parse(yaml).unwrap();

    assert_eq!(result["name"], "Bob");
    assert_eq!(result["age"], 25);
}

#[test]
fn test_yaml_parser_markdown_yml() {
    let parser = YamlOutputParser::new();

    let yaml = "```yml\nname: Charlie\nage: 35\n```";
    let result = parser.parse(yaml).unwrap();

    assert_eq!(result["name"], "Charlie");
    assert_eq!(result["age"], 35);
}

#[test]
fn test_yaml_parser_markdown_no_lang() {
    let parser = YamlOutputParser::new();

    let yaml = "```\nname: David\nage: 40\n```";
    let result = parser.parse(yaml).unwrap();

    assert_eq!(result["name"], "David");
    assert_eq!(result["age"], 40);
}

#[test]
fn test_yaml_parser_array() {
    let parser = YamlOutputParser::new();

    let yaml = "- name: Alice\n  age: 30\n- name: Bob\n  age: 25";
    let result = parser.parse(yaml).unwrap();

    assert!(result.is_array());
    let arr = result.as_array().unwrap();
    assert_eq!(arr.len(), 2);
    assert_eq!(arr[0]["name"], "Alice");
    assert_eq!(arr[1]["name"], "Bob");
}

#[test]
fn test_yaml_parser_nested() {
    let parser = YamlOutputParser::new();

    let yaml = "person:\n  name: Alice\n  address:\n    city: Seattle\n    zip: 98101";
    let result = parser.parse(yaml).unwrap();

    assert_eq!(result["person"]["name"], "Alice");
    assert_eq!(result["person"]["address"]["city"], "Seattle");
    assert_eq!(result["person"]["address"]["zip"], 98101);
}

#[test]
fn test_yaml_parser_error() {
    let parser = YamlOutputParser::new();

    let yaml = "invalid: yaml: : content";
    let result = parser.parse(yaml);

    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("Failed to parse YAML"));
}

#[test]
fn test_yaml_parser_format_instructions_no_schema() {
    let parser = YamlOutputParser::new();
    let instructions = parser.get_format_instructions();

    assert_eq!(instructions, "Output should be formatted as valid YAML.");
}

#[test]
fn test_yaml_parser_format_instructions_with_schema() {
    let schema = r#"{"properties": {"name": {"type": "string"}}}"#;
    let parser = YamlOutputParser::with_schema(schema);
    let instructions = parser.get_format_instructions();

    assert!(instructions.contains("JSON schema"));
    assert!(instructions.contains(schema));
    assert!(instructions.contains("triple backticks"));
}

#[test]
fn test_yaml_parser_serialization() {
    let parser = YamlOutputParser::with_schema(r#"{"type": "object"}"#);
    let json_value = parser.to_json();

    let SerializedObject::Constructor { lc, id, kwargs } = json_value else {
        panic!("Expected Constructor variant");
    };

    assert_eq!(lc, SERIALIZATION_VERSION);
    assert_eq!(
        id,
        vec![
            "dashflow".to_string(),
            "output_parsers".to_string(),
            "yaml".to_string(),
            "YamlOutputParser".to_string()
        ]
    );
    assert_eq!(kwargs["schema"], r#"{"type": "object"}"#);
}

#[test]
fn test_yaml_parser_serialization_no_schema() {
    let parser = YamlOutputParser::new();
    let json_value = parser.to_json();

    let SerializedObject::Constructor { kwargs, .. } = json_value else {
        panic!("Expected Constructor variant");
    };

    assert!(!kwargs.as_object().unwrap().contains_key("schema"));
}

#[test]
fn test_yaml_parser_deserialization() {
    let json = serde_json::json!({
        "lc": 1,
        "type": "constructor",
        "id": ["dashflow", "output_parsers", "yaml", "YamlOutputParser"],
        "kwargs": {
            "schema": r#"{"type": "object"}"#
        }
    });

    let parser = YamlOutputParser::from_json(&json).unwrap();
    assert_eq!(parser.schema, Some(r#"{"type": "object"}"#.to_string()));
}

#[test]
fn test_yaml_parser_deserialization_no_schema() {
    let json = serde_json::json!({
        "lc": 1,
        "type": "constructor",
        "id": ["dashflow", "output_parsers", "yaml", "YamlOutputParser"],
        "kwargs": {}
    });

    let parser = YamlOutputParser::from_json(&json).unwrap();
    assert!(parser.schema.is_none());
}

#[tokio::test]
async fn test_yaml_parser_as_runnable() {
    let parser = YamlOutputParser::new();
    let input = "name: Alice\nage: 30".to_string();

    let result = parser.invoke(input, None).await.unwrap();
    assert_eq!(result["name"], "Alice");
    assert_eq!(result["age"], 30);
}

#[tokio::test]
async fn test_yaml_parser_batch() {
    let parser = YamlOutputParser::new();
    let inputs = vec![
        "name: Alice\nage: 30".to_string(),
        "name: Bob\nage: 25".to_string(),
    ];

    let results = parser.batch(inputs, None).await.unwrap();
    assert_eq!(results.len(), 2);
    assert_eq!(results[0]["name"], "Alice");
    assert_eq!(results[1]["name"], "Bob");
}

// ===== BooleanOutputParser Tests =====

#[test]
fn test_boolean_parser_true() {
    let parser = BooleanOutputParser::new();
    assert!(parser.parse("YES").unwrap());
    assert!(parser.parse("yes").unwrap());
    assert!(parser.parse("Yes").unwrap());
    assert!(parser.parse("The answer is YES").unwrap());
}

#[test]
fn test_boolean_parser_false() {
    let parser = BooleanOutputParser::new();
    assert!(!parser.parse("NO").unwrap());
    assert!(!parser.parse("no").unwrap());
    assert!(!parser.parse("No").unwrap());
    assert!(!parser.parse("The answer is NO").unwrap());
}

#[test]
fn test_boolean_parser_custom_values() {
    let parser = BooleanOutputParser::new()
        .with_true_val("CORRECT")
        .with_false_val("INCORRECT");

    assert!(parser.parse("CORRECT").unwrap());
    assert!(!parser.parse("INCORRECT").unwrap());
}

#[test]
fn test_boolean_parser_ambiguous() {
    let parser = BooleanOutputParser::new();
    let result = parser.parse("YES and NO");
    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("Ambiguous response"));
}

#[test]
fn test_boolean_parser_missing() {
    let parser = BooleanOutputParser::new();
    let result = parser.parse("MAYBE");
    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("expected output value"));
}

#[test]
fn test_boolean_parser_word_boundaries() {
    let parser = BooleanOutputParser::new();
    // "YESNO" should not match because we use word boundaries
    let result = parser.parse("YESNO");
    assert!(result.is_err());
}

#[test]
fn test_boolean_parser_multiline() {
    let parser = BooleanOutputParser::new();
    let text = "Let me think about this.\nThe answer is YES.\nThat's my final answer.";
    assert!(parser.parse(text).unwrap());
}

#[test]
fn test_boolean_parser_format_instructions() {
    let parser = BooleanOutputParser::new();
    let instructions = parser.get_format_instructions();
    assert!(instructions.contains("YES"));
    assert!(instructions.contains("NO"));
}

#[test]
fn test_boolean_parser_format_instructions_custom() {
    let parser = BooleanOutputParser::new()
        .with_true_val("TRUE")
        .with_false_val("FALSE");
    let instructions = parser.get_format_instructions();
    assert!(instructions.contains("TRUE"));
    assert!(instructions.contains("FALSE"));
}

#[tokio::test]
async fn test_boolean_parser_as_runnable() {
    let parser = BooleanOutputParser::new();
    let result = parser.invoke("YES".to_string(), None).await.unwrap();
    assert!(result);
}

#[tokio::test]
async fn test_boolean_parser_batch() {
    let parser = BooleanOutputParser::new();
    let inputs = vec!["YES".to_string(), "NO".to_string()];
    let results = parser.batch(inputs, None).await.unwrap();
    assert_eq!(results, vec![true, false]);
}

#[test]
fn test_boolean_parser_serialization() {
    let parser = BooleanOutputParser::new()
        .with_true_val("TRUE")
        .with_false_val("FALSE");

    let serialized = parser.to_json();
    match serialized {
        SerializedObject::Constructor { lc, id, kwargs } => {
            assert_eq!(lc, 1);
            assert_eq!(
                id,
                vec![
                    "dashflow",
                    "output_parsers",
                    "boolean",
                    "BooleanOutputParser"
                ]
            );

            let obj = kwargs.as_object().unwrap();
            let true_val = obj.get("true_val").and_then(|v| v.as_str());
            let false_val = obj.get("false_val").and_then(|v| v.as_str());
            assert_eq!(true_val, Some("TRUE"));
            assert_eq!(false_val, Some("FALSE"));
        }
        _ => panic!("Expected Constructor variant"),
    }
}

#[test]
fn test_boolean_parser_deserialization() {
    let json_str = r#"{
        "true_val": "AGREE",
        "false_val": "DISAGREE"
    }"#;

    let parser: BooleanOutputParser = serde_json::from_str(json_str).unwrap();
    assert_eq!(parser.true_val, "AGREE");
    assert_eq!(parser.false_val, "DISAGREE");
}

#[test]
fn test_boolean_parser_from_serialized() {
    let json_str = r#"{
        "lc": 1,
        "type": "constructor",
        "id": ["dashflow", "output_parsers", "boolean", "BooleanOutputParser"],
        "kwargs": {
            "true_val": "AGREE",
            "false_val": "DISAGREE"
        }
    }"#;

    let json_value: JsonValue = serde_json::from_str(json_str).unwrap();
    let parser = BooleanOutputParser::from_json(&json_value).unwrap();
    assert_eq!(parser.true_val, "AGREE");
    assert_eq!(parser.false_val, "DISAGREE");
}

#[test]
fn test_boolean_parser_roundtrip() {
    let parser = BooleanOutputParser::new()
        .with_true_val("CONFIRM")
        .with_false_val("DENY");

    let serialized = serde_json::to_string(&parser).unwrap();
    let deserialized: BooleanOutputParser = serde_json::from_str(&serialized).unwrap();

    assert_eq!(deserialized.true_val, "CONFIRM");
    assert_eq!(deserialized.false_val, "DENY");
}

// ==================== EnumOutputParser Tests ====================

#[test]
fn test_enum_parser_valid() {
    let parser = EnumOutputParser::new(vec!["red", "green", "blue"]);

    assert_eq!(parser.parse("red").unwrap(), "red");
    assert_eq!(parser.parse("green").unwrap(), "green");
    assert_eq!(parser.parse("blue").unwrap(), "blue");
}

#[test]
fn test_enum_parser_case_insensitive() {
    let parser = EnumOutputParser::new(vec!["red", "green", "blue"]);

    assert_eq!(parser.parse("RED").unwrap(), "red");
    assert_eq!(parser.parse("Green").unwrap(), "green");
    assert_eq!(parser.parse("BLUE").unwrap(), "blue");
}

#[test]
fn test_enum_parser_whitespace() {
    let parser = EnumOutputParser::new(vec!["red", "green", "blue"]);

    assert_eq!(parser.parse("  red  ").unwrap(), "red");
    assert_eq!(parser.parse("\ngreen\n").unwrap(), "green");
}

#[test]
fn test_enum_parser_invalid() {
    let parser = EnumOutputParser::new(vec!["red", "green", "blue"]);

    let result = parser.parse("yellow");
    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(err_msg.contains("yellow"));
    assert!(err_msg.contains("red"));
}

#[test]
fn test_enum_parser_empty_values() {
    let parser = EnumOutputParser::new(Vec::<String>::new());

    let result = parser.parse("anything");
    assert!(result.is_err());
}

#[test]
fn test_enum_parser_format_instructions() {
    let parser = EnumOutputParser::new(vec!["red", "green", "blue"]);
    let instructions = parser.get_format_instructions();

    assert!(instructions.contains("red"));
    assert!(instructions.contains("green"));
    assert!(instructions.contains("blue"));
}

#[tokio::test]
async fn test_enum_parser_as_runnable() {
    let parser = EnumOutputParser::new(vec!["red", "green", "blue"]);
    let result = parser.invoke("red".to_string(), None).await.unwrap();
    assert_eq!(result, "red");
}

#[tokio::test]
async fn test_enum_parser_batch() {
    let parser = EnumOutputParser::new(vec!["red", "green", "blue"]);
    let inputs = vec!["red".to_string(), "blue".to_string(), "green".to_string()];
    let results = parser.batch(inputs, None).await.unwrap();
    assert_eq!(results, vec!["red", "blue", "green"]);
}

#[test]
fn test_enum_parser_serialization() {
    let parser = EnumOutputParser::new(vec!["option1", "option2", "option3"]);

    let serialized = parser.to_json();
    match serialized {
        SerializedObject::Constructor { lc, id, kwargs } => {
            assert_eq!(lc, 1);
            assert_eq!(
                id,
                vec!["dashflow", "output_parsers", "enum", "EnumOutputParser"]
            );

            let obj = kwargs.as_object().unwrap();
            let values = obj.get("values").and_then(|v| v.as_array());
            assert!(values.is_some());
            let values = values.unwrap();
            assert_eq!(values.len(), 3);
        }
        _ => panic!("Expected Constructor variant"),
    }
}

#[test]
fn test_enum_parser_deserialization() {
    let json_str = r#"{
        "values": ["small", "medium", "large"]
    }"#;

    let parser: EnumOutputParser = serde_json::from_str(json_str).unwrap();
    assert_eq!(parser.values.len(), 3);
    assert_eq!(parser.values[0], "small");
    assert_eq!(parser.values[1], "medium");
    assert_eq!(parser.values[2], "large");
}

#[test]
fn test_enum_parser_from_serialized() {
    let json_str = r#"{
        "lc": 1,
        "type": "constructor",
        "id": ["dashflow", "output_parsers", "enum", "EnumOutputParser"],
        "kwargs": {
            "values": ["small", "medium", "large"]
        }
    }"#;

    let json_value: JsonValue = serde_json::from_str(json_str).unwrap();
    let parser = EnumOutputParser::from_json(&json_value).unwrap();
    assert_eq!(parser.values.len(), 3);
    assert_eq!(parser.values[0], "small");
}

#[test]
fn test_enum_parser_roundtrip() {
    let parser = EnumOutputParser::new(vec!["low", "medium", "high"]);

    let serialized = serde_json::to_string(&parser).unwrap();
    let deserialized: EnumOutputParser = serde_json::from_str(&serialized).unwrap();

    assert_eq!(deserialized.values, parser.values);
    assert_eq!(deserialized.values.len(), 3);
}

// RegexParser tests
#[test]
fn test_regex_parser_basic() {
    let parser = RegexParser::new(r"Name: (\w+), Age: (\d+)", vec!["name", "age"], None)
        .expect("Valid regex");

    let result = parser.parse("Name: Alice, Age: 30").unwrap();
    assert_eq!(result.get("name").unwrap(), "Alice");
    assert_eq!(result.get("age").unwrap(), "30");
}

#[test]
fn test_regex_parser_no_match_no_default() {
    let parser = RegexParser::new(r"Result: (.+)", vec!["result"], None).expect("Valid regex");

    let result = parser.parse("Random text");
    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("Could not parse output"));
}

#[test]
fn test_regex_parser_no_match_with_default() {
    let parser =
        RegexParser::new(r"Result: (.+)", vec!["result"], Some("result")).expect("Valid regex");

    let result = parser.parse("Random text").unwrap();
    assert_eq!(result.get("result").unwrap(), "Random text");
}

#[test]
fn test_regex_parser_multiple_captures() {
    let parser = RegexParser::new(
        r"(\w+): (\d+), (\w+): (\d+)",
        vec!["key1", "val1", "key2", "val2"],
        None,
    )
    .expect("Valid regex");

    let result = parser.parse("foo: 10, bar: 20").unwrap();
    assert_eq!(result.get("key1").unwrap(), "foo");
    assert_eq!(result.get("val1").unwrap(), "10");
    assert_eq!(result.get("key2").unwrap(), "bar");
    assert_eq!(result.get("val2").unwrap(), "20");
}

#[test]
fn test_regex_parser_partial_captures() {
    // Regex with optional second group
    let parser = RegexParser::new(
        r"Value: (\d+)(?:, Extra: (\w+))?",
        vec!["value", "extra"],
        None,
    )
    .expect("Valid regex");

    // With both groups
    let result1 = parser.parse("Value: 42, Extra: test").unwrap();
    assert_eq!(result1.get("value").unwrap(), "42");
    assert_eq!(result1.get("extra").unwrap(), "test");

    // With only first group
    let result2 = parser.parse("Value: 42").unwrap();
    assert_eq!(result2.get("value").unwrap(), "42");
    // extra key won't be in the map since capture didn't match
    assert!(!result2.contains_key("extra"));
}

#[test]
fn test_regex_parser_invalid_regex() {
    let result = RegexParser::new(r"[invalid(regex", vec!["test"], None);
    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("Invalid regex pattern"));
}

#[test]
fn test_regex_parser_format_instructions() {
    let parser = RegexParser::new(r"Answer: (.+)", vec!["answer"], None).expect("Valid regex");

    let instructions = parser.get_format_instructions();
    assert!(instructions.contains("Answer: (.+)"));
}

#[tokio::test]
async fn test_regex_parser_as_runnable() {
    let parser = RegexParser::new(r"Score: (\d+)", vec!["score"], None).expect("Valid regex");

    let result = parser.invoke("Score: 85".to_string(), None).await.unwrap();
    assert_eq!(result.get("score").unwrap(), "85");
}

#[tokio::test]
async fn test_regex_parser_batch() {
    let parser = RegexParser::new(r"Num: (\d+)", vec!["num"], None).expect("Valid regex");

    let results = parser
        .batch(vec!["Num: 1".to_string(), "Num: 2".to_string()], None)
        .await
        .unwrap();

    assert_eq!(results.len(), 2);
    assert_eq!(results[0].get("num").unwrap(), "1");
    assert_eq!(results[1].get("num").unwrap(), "2");
}

#[test]
fn test_regex_parser_serialization() {
    let parser =
        RegexParser::new(r"Value: (\d+)", vec!["value"], Some("value")).expect("Valid regex");

    let serialized = parser.to_json();
    match serialized {
        SerializedObject::Constructor { id, kwargs, .. } => {
            assert_eq!(id[0], "dashflow");
            assert_eq!(id[1], "output_parsers");
            assert_eq!(id[2], "regex");
            assert_eq!(id[3], "RegexParser");

            let regex = kwargs.get("regex").unwrap().as_str().unwrap();
            assert_eq!(regex, r"Value: (\d+)");

            let output_keys = kwargs.get("output_keys").unwrap().as_array().unwrap();
            assert_eq!(output_keys.len(), 1);

            let default_key = kwargs.get("default_output_key").unwrap().as_str().unwrap();
            assert_eq!(default_key, "value");
        }
        _ => panic!("Expected Constructor variant"),
    }
}

#[test]
fn test_regex_parser_deserialization() {
    let json = serde_json::json!({
        "lc": 1,
        "type": "constructor",
        "id": ["dashflow", "output_parsers", "regex", "RegexParser"],
        "kwargs": {
            "regex": r"Test: (\w+)",
            "output_keys": ["test"],
            "default_output_key": "test"
        }
    });

    let parser = RegexParser::from_json(&json).unwrap();
    assert_eq!(parser.regex_str, r"Test: (\w+)");
    assert_eq!(parser.output_keys, vec!["test"]);
    assert_eq!(parser.default_output_key, Some("test".to_string()));

    // Verify regex works
    let result = parser.parse("Test: hello").unwrap();
    assert_eq!(result.get("test").unwrap(), "hello");
}

#[test]
fn test_regex_parser_roundtrip() {
    let parser = RegexParser::new(r"Code: (\d+)", vec!["code"], None).expect("Valid regex");

    // Serialize via Serializable trait
    let serialized_obj = parser.to_json();
    let json_value = serde_json::to_value(&serialized_obj).unwrap();

    // Deserialize via Deserializable trait
    let deserialized = RegexParser::from_json(&json_value).unwrap();

    assert_eq!(deserialized.regex_str, parser.regex_str);
    assert_eq!(deserialized.output_keys, parser.output_keys);

    // Verify functionality
    let result = deserialized.parse("Code: 42").unwrap();
    assert_eq!(result.get("code").unwrap(), "42");
}

#[test]
fn test_regex_parser_default_output_key_with_multiple_keys() {
    // Test that only the default key gets the text when no match
    let parser =
        RegexParser::new(r"A: (\w+), B: (\w+)", vec!["a", "b"], Some("a")).expect("Valid regex");

    let result = parser.parse("unmatched text").unwrap();
    assert_eq!(result.get("a").unwrap(), "unmatched text");
    assert_eq!(result.get("b").unwrap(), ""); // non-default keys get empty string
}

// RegexDictParser tests
#[test]
fn test_regex_dict_parser_basic() {
    let mut key_to_format = HashMap::new();
    key_to_format.insert("name".to_string(), "Name".to_string());
    key_to_format.insert("age".to_string(), "Age".to_string());

    let parser = RegexDictParser::new(key_to_format, None, None);

    let result = parser.parse("Name: Alice\nAge: 30").unwrap();
    assert_eq!(result.get("name").unwrap(), "Alice");
    assert_eq!(result.get("age").unwrap(), "30");
}

#[test]
fn test_regex_dict_parser_missing_field() {
    let mut key_to_format = HashMap::new();
    key_to_format.insert("name".to_string(), "Name".to_string());

    let parser = RegexDictParser::new(key_to_format, None, None);

    let result = parser.parse("Age: 30");
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("No match found"));
}

#[test]
fn test_regex_dict_parser_multiple_matches() {
    let mut key_to_format = HashMap::new();
    key_to_format.insert("name".to_string(), "Name".to_string());

    let parser = RegexDictParser::new(key_to_format, None, None);

    let result = parser.parse("Name: Alice\nName: Bob");
    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("Multiple matches found"));
}

#[test]
fn test_regex_dict_parser_no_update_value() {
    let mut key_to_format = HashMap::new();
    key_to_format.insert("name".to_string(), "Name".to_string());
    key_to_format.insert("age".to_string(), "Age".to_string());

    let parser = RegexDictParser::new(key_to_format, None, Some("NO_UPDATE".to_string()));

    let result = parser.parse("Name: Alice\nAge: NO_UPDATE").unwrap();
    assert_eq!(result.get("name").unwrap(), "Alice");
    assert!(!result.contains_key("age")); // age is skipped
}

#[test]
fn test_regex_dict_parser_custom_pattern() {
    let mut key_to_format = HashMap::new();
    key_to_format.insert("city".to_string(), "City".to_string());
    key_to_format.insert("country".to_string(), "Country".to_string());

    // Custom pattern: "Field = value"
    let parser = RegexDictParser::new(key_to_format, Some(r"{} = (.+)".to_string()), None);

    let result = parser.parse("City = Paris\nCountry = France").unwrap();
    assert_eq!(result.get("city").unwrap(), "Paris");
    assert_eq!(result.get("country").unwrap(), "France");
}

#[test]
fn test_regex_dict_parser_format_instructions() {
    let mut key_to_format = HashMap::new();
    key_to_format.insert("name".to_string(), "Name".to_string());
    key_to_format.insert("age".to_string(), "Age".to_string());

    let parser = RegexDictParser::new(key_to_format, None, None);

    let instructions = parser.get_format_instructions();
    assert!(instructions.contains("Name"));
    assert!(instructions.contains("Age"));
}

#[tokio::test]
async fn test_regex_dict_parser_as_runnable() {
    let mut key_to_format = HashMap::new();
    key_to_format.insert("score".to_string(), "Score".to_string());

    let parser = RegexDictParser::new(key_to_format, None, None);

    let result = parser.invoke("Score: 95".to_string(), None).await.unwrap();
    assert_eq!(result.get("score").unwrap(), "95");
}

#[tokio::test]
async fn test_regex_dict_parser_batch() {
    let mut key_to_format = HashMap::new();
    key_to_format.insert("num".to_string(), "Num".to_string());

    let parser = RegexDictParser::new(key_to_format, None, None);

    let results = parser
        .batch(vec!["Num: 1".to_string(), "Num: 2".to_string()], None)
        .await
        .unwrap();

    assert_eq!(results.len(), 2);
    assert_eq!(results[0].get("num").unwrap(), "1");
    assert_eq!(results[1].get("num").unwrap(), "2");
}

#[test]
fn test_regex_dict_parser_serialization() {
    let mut key_to_format = HashMap::new();
    key_to_format.insert("name".to_string(), "Name".to_string());

    let parser = RegexDictParser::new(key_to_format, None, Some("SKIP".to_string()));

    let serialized = parser.to_json();
    match serialized {
        SerializedObject::Constructor { id, kwargs, .. } => {
            assert_eq!(id[0], "dashflow");
            assert_eq!(id[1], "output_parsers");
            assert_eq!(id[2], "regex_dict");
            assert_eq!(id[3], "RegexDictParser");

            let pattern = kwargs.get("regex_pattern").unwrap().as_str().unwrap();
            assert_eq!(pattern, r"{}:\s?([^.'\n']*)\.?");

            let key_to_format = kwargs
                .get("output_key_to_format")
                .unwrap()
                .as_object()
                .unwrap();
            assert_eq!(key_to_format.get("name").unwrap().as_str().unwrap(), "Name");

            let no_update = kwargs.get("no_update_value").unwrap().as_str().unwrap();
            assert_eq!(no_update, "SKIP");
        }
        _ => panic!("Expected Constructor variant"),
    }
}

#[test]
fn test_regex_dict_parser_deserialization() {
    let json = serde_json::json!({
        "lc": 1,
        "type": "constructor",
        "id": ["dashflow", "output_parsers", "regex_dict", "RegexDictParser"],
        "kwargs": {
            "regex_pattern": r"{}:\s?(.+)",
            "output_key_to_format": {
                "field1": "Field1"
            },
            "no_update_value": "N/A"
        }
    });

    let parser = RegexDictParser::from_json(&json).unwrap();
    assert_eq!(parser.regex_pattern, r"{}:\s?(.+)");
    assert_eq!(parser.output_key_to_format.get("field1").unwrap(), "Field1");
    assert_eq!(parser.no_update_value, Some("N/A".to_string()));

    // Verify functionality
    let result = parser.parse("Field1: test").unwrap();
    assert_eq!(result.get("field1").unwrap(), "test");
}

#[test]
fn test_regex_dict_parser_roundtrip() {
    let mut key_to_format = HashMap::new();
    key_to_format.insert("item".to_string(), "Item".to_string());

    let parser = RegexDictParser::new(key_to_format, None, None);

    // Serialize via Serializable trait
    let serialized_obj = parser.to_json();
    let json_value = serde_json::to_value(&serialized_obj).unwrap();

    // Deserialize via Deserializable trait
    let deserialized = RegexDictParser::from_json(&json_value).unwrap();

    assert_eq!(deserialized.regex_pattern, parser.regex_pattern);
    assert_eq!(
        deserialized.output_key_to_format,
        parser.output_key_to_format
    );

    // Verify functionality
    let result = deserialized.parse("Item: value").unwrap();
    assert_eq!(result.get("item").unwrap(), "value");
}

#[test]
fn test_regex_dict_parser_special_chars() {
    let mut key_to_format = HashMap::new();
    // Test regex escaping with special characters
    key_to_format.insert("field".to_string(), "Field[*]".to_string());

    let parser = RegexDictParser::new(key_to_format, None, None);

    let result = parser.parse("Field[*]: test_value").unwrap();
    assert_eq!(result.get("field").unwrap(), "test_value");
}
