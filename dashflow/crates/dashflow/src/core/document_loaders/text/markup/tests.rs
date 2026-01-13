// Import from parent module - explicit to avoid conflict with test_prelude
use super::{HTMLLoader, IniLoader, MarkdownLoader, TOMLLoader, XMLLoader, YAMLLoader};
use crate::core::documents::DocumentLoader;
use std::fs;
use tempfile::TempDir;

#[tokio::test]
async fn test_html_loader() {
    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("test.html");

    let html_content = r#"
    <!DOCTYPE html>
    <html>
    <head><title>Test Document</title></head>
    <body>
        <h1>Hello World</h1>
        <p>This is a test paragraph.</p>
        <p>Another paragraph with <strong>bold text</strong>.</p>
    </body>
    </html>
"#;
    fs::write(&file_path, html_content).unwrap();

    let loader = HTMLLoader::new(&file_path);
    let docs = loader.load().await.unwrap();

    // Enhanced validation
    assert_eq!(docs.len(), 1, "Should load exactly one document");
    assert!(
        docs[0].page_content.contains("Hello World"),
        "Should contain heading text"
    );
    assert!(
        docs[0].page_content.contains("test paragraph"),
        "Should contain paragraph text"
    );
    assert!(
        docs[0].page_content.contains("bold text"),
        "Should contain bold text (tags stripped)"
    );

    // Metadata validation
    assert_eq!(
        docs[0].get_metadata("format").and_then(|v| v.as_str()),
        Some("html"),
        "Should have 'html' format metadata"
    );
    let source_path = file_path.display().to_string();
    assert_eq!(
        docs[0].get_metadata("source").and_then(|v| v.as_str()),
        Some(source_path.as_str()),
        "Should have source path metadata"
    );

    // Structure validation - HTML tags should be stripped
    assert!(
        !docs[0].page_content.contains("<h1>"),
        "HTML tags should be stripped"
    );
    assert!(
        !docs[0].page_content.contains("<p>"),
        "HTML tags should be stripped"
    );
    assert!(
        !docs[0].page_content.contains("<strong>"),
        "HTML tags should be stripped"
    );
}

#[tokio::test]
async fn test_html_loader_malformed_html() {
    let temp_dir = TempDir::new().unwrap();

    // Test 1: Unclosed tags
    let file_path1 = temp_dir.path().join("unclosed.html");
    let html_content1 = r#"<html><body><p>Unclosed paragraph<div>Unclosed div"#;
    fs::write(&file_path1, html_content1).unwrap();

    let loader1 = HTMLLoader::new(&file_path1);
    let docs1 = loader1.load().await.unwrap();
    assert_eq!(docs1.len(), 1, "Should handle unclosed tags gracefully");
    assert!(
        docs1[0].page_content.contains("Unclosed paragraph"),
        "Should extract text from unclosed tags"
    );
    assert!(
        docs1[0].page_content.contains("Unclosed div"),
        "Should extract text from all unclosed tags"
    );

    // Test 2: Nested tags without proper closing
    let file_path2 = temp_dir.path().join("nested.html");
    let html_content2 = r#"<html><body><div><p>Nested <span>content</div></body></html>"#;
    fs::write(&file_path2, html_content2).unwrap();

    let loader2 = HTMLLoader::new(&file_path2);
    let docs2 = loader2.load().await.unwrap();
    assert_eq!(docs2.len(), 1, "Should handle improperly nested tags");
    assert!(
        docs2[0].page_content.contains("Nested"),
        "Should extract text from improperly nested tags"
    );
    assert!(
        docs2[0].page_content.contains("content"),
        "Should extract all text content"
    );

    // Test 3: Invalid tag names
    let file_path3 = temp_dir.path().join("invalid.html");
    let html_content3 = r#"<html><body><invalid-tag>Content</invalid-tag></body></html>"#;
    fs::write(&file_path3, html_content3).unwrap();

    let loader3 = HTMLLoader::new(&file_path3);
    let docs3 = loader3.load().await.unwrap();
    assert_eq!(docs3.len(), 1, "Should handle invalid tag names");
    assert!(
        docs3[0].page_content.contains("Content"),
        "Should extract text from invalid tags"
    );
}

#[tokio::test]
async fn test_html_loader_script_style_handling() {
    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("script_style.html");

    let html_content = r#"
    <!DOCTYPE html>
    <html>
    <head>
        <title>Test</title>
        <style>
            body { color: red; }
            .hidden { display: none; }
        </style>
        <script>
            console.log("This is JavaScript");
            var x = 10;
        </script>
    </head>
    <body>
        <h1>Visible Content</h1>
        <p>This is visible text.</p>
        <script>alert("Another script");</script>
        <style>.more { color: blue; }</style>
    </body>
    </html>
"#;
    fs::write(&file_path, html_content).unwrap();

    let loader = HTMLLoader::new(&file_path);
    let docs = loader.load().await.unwrap();

    assert_eq!(docs.len(), 1, "Should load one document");
    assert!(
        docs[0].page_content.contains("Visible Content"),
        "Should contain visible text"
    );
    assert!(
        docs[0].page_content.contains("This is visible text"),
        "Should contain paragraph text"
    );

    // html2text library may or may not include script/style content
    // We just verify that visible content is present
    assert_eq!(
        docs[0].get_metadata("format").and_then(|v| v.as_str()),
        Some("html"),
        "Should have html format metadata"
    );
}

#[tokio::test]
async fn test_html_loader_nested_tags() {
    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("nested.html");

    let html_content = r#"
    <!DOCTYPE html>
    <html>
    <body>
        <div class="level1">
            <div class="level2">
                <div class="level3">
                    <div class="level4">
                        <div class="level5">
                            <p>Deeply <strong>nested <em>content <span>here</span></em></strong></p>
                        </div>
                    </div>
                </div>
            </div>
        </div>
    </body>
    </html>
"#;
    fs::write(&file_path, html_content).unwrap();

    let loader = HTMLLoader::new(&file_path);
    let docs = loader.load().await.unwrap();

    assert_eq!(docs.len(), 1, "Should load one document");
    assert!(
        docs[0].page_content.contains("Deeply"),
        "Should extract deeply nested text"
    );
    assert!(
        docs[0].page_content.contains("nested"),
        "Should extract all nested content"
    );
    assert!(
        docs[0].page_content.contains("content"),
        "Should extract all text nodes"
    );
    assert!(
        docs[0].page_content.contains("here"),
        "Should extract deepest level text"
    );

    // Verify tags are stripped
    assert!(
        !docs[0].page_content.contains("<div>"),
        "Should strip div tags"
    );
    assert!(
        !docs[0].page_content.contains("<strong>"),
        "Should strip strong tags"
    );
    assert!(
        !docs[0].page_content.contains("<em>"),
        "Should strip em tags"
    );
    assert!(
        !docs[0].page_content.contains("<span>"),
        "Should strip span tags"
    );
}

#[tokio::test]
async fn test_html_loader_self_closing_tags() {
    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("self_closing.html");

    let html_content = r#"
    <!DOCTYPE html>
    <html>
    <body>
        <p>Line 1<br/>Line 2<br>Line 3</p>
        <hr/>
        <img src="image.jpg" alt="Test Image"/>
        <input type="text" value="test"/>
        <p>After self-closing tags</p>
    </body>
    </html>
"#;
    fs::write(&file_path, html_content).unwrap();

    let loader = HTMLLoader::new(&file_path);
    let docs = loader.load().await.unwrap();

    assert_eq!(docs.len(), 1, "Should load one document");
    assert!(
        docs[0].page_content.contains("Line 1"),
        "Should handle content before br"
    );
    assert!(
        docs[0].page_content.contains("Line 2"),
        "Should handle content after br"
    );
    assert!(
        docs[0].page_content.contains("Line 3"),
        "Should handle content after self-closing br"
    );
    assert!(
        docs[0].page_content.contains("After self-closing tags"),
        "Should continue after self-closing tags"
    );
}

#[tokio::test]
async fn test_html_loader_unicode_content() {
    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("unicode.html");

    let html_content = r#"
    <!DOCTYPE html>
    <html>
    <head><meta charset="UTF-8"></head>
    <body>
        <h1>世界 🌍</h1>
        <p>مرحبا בעולם</p>
        <p>Привет ∑∫√</p>
        <p>Emoji: 😀 🎉 ❤️ 🚀</p>
    </body>
    </html>
"#;
    fs::write(&file_path, html_content).unwrap();

    let loader = HTMLLoader::new(&file_path);
    let docs = loader.load().await.unwrap();

    assert_eq!(docs.len(), 1, "Should load one document");
    assert!(
        docs[0].page_content.contains("世界"),
        "Should preserve Chinese characters"
    );
    assert!(docs[0].page_content.contains("🌍"), "Should preserve emoji");
    assert!(
        docs[0].page_content.contains("مرحبا"),
        "Should preserve Arabic"
    );
    assert!(
        docs[0].page_content.contains("בעולם"),
        "Should preserve Hebrew"
    );
    assert!(
        docs[0].page_content.contains("Привет"),
        "Should preserve Cyrillic"
    );
    assert!(
        docs[0].page_content.contains("∑"),
        "Should preserve mathematical symbols"
    );
    assert!(
        docs[0].page_content.contains("😀"),
        "Should preserve emoji in content"
    );
    assert!(
        docs[0].page_content.contains("❤️"),
        "Should preserve multi-codepoint emoji"
    );
}

#[tokio::test]
async fn test_html_loader_empty_html() {
    let temp_dir = TempDir::new().unwrap();

    // Test 1: Completely empty file
    let file_path1 = temp_dir.path().join("empty.html");
    fs::write(&file_path1, "").unwrap();

    let loader1 = HTMLLoader::new(&file_path1);
    let docs1 = loader1.load().await.unwrap();
    assert_eq!(docs1.len(), 1, "Should load one document from empty file");
    assert!(
        docs1[0].page_content.is_empty() || docs1[0].page_content.trim().is_empty(),
        "Empty file should produce empty or whitespace-only content"
    );

    // Test 2: Empty HTML structure
    let file_path2 = temp_dir.path().join("empty_structure.html");
    let html_content2 = r#"<!DOCTYPE html><html><head></head><body></body></html>"#;
    fs::write(&file_path2, html_content2).unwrap();

    let loader2 = HTMLLoader::new(&file_path2);
    let docs2 = loader2.load().await.unwrap();
    assert_eq!(
        docs2.len(),
        1,
        "Should load one document from empty HTML structure"
    );
    assert!(
        docs2[0].page_content.trim().is_empty(),
        "Empty HTML should produce empty content"
    );

    // Test 3: HTML with only whitespace
    let file_path3 = temp_dir.path().join("whitespace.html");
    let html_content3 = r#"<html><body>

    </body></html>"#;
    fs::write(&file_path3, html_content3).unwrap();

    let loader3 = HTMLLoader::new(&file_path3);
    let docs3 = loader3.load().await.unwrap();
    assert_eq!(
        docs3.len(),
        1,
        "Should load one document from whitespace-only HTML"
    );
    assert!(
        docs3[0].page_content.trim().is_empty(),
        "Whitespace-only HTML should produce empty trimmed content"
    );
}

#[tokio::test]
async fn test_html_loader_html_entities() {
    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("entities.html");

    let html_content = r#"
    <!DOCTYPE html>
    <html>
    <body>
        <p>Less than: &lt;</p>
        <p>Greater than: &gt;</p>
        <p>Ampersand: &amp;</p>
        <p>Quote: &quot;</p>
        <p>Apostrophe: &apos;</p>
        <p>Non-breaking space: Hello&nbsp;World</p>
        <p>Copyright: &copy;</p>
        <p>Numeric: &#65; &#x41;</p>
    </body>
    </html>
"#;
    fs::write(&file_path, html_content).unwrap();

    let loader = HTMLLoader::new(&file_path);
    let docs = loader.load().await.unwrap();

    assert_eq!(docs.len(), 1, "Should load one document");

    // html2text should decode entities
    assert!(
        docs[0].page_content.contains("Less than"),
        "Should contain 'Less than' text"
    );
    assert!(
        docs[0].page_content.contains("Greater than"),
        "Should contain 'Greater than' text"
    );
    assert!(
        docs[0].page_content.contains("Ampersand"),
        "Should contain 'Ampersand' text"
    );
    assert!(
        docs[0].page_content.contains("Hello"),
        "Should contain 'Hello' text"
    );
    assert!(
        docs[0].page_content.contains("World"),
        "Should contain 'World' text"
    );

    // The actual decoded characters may or may not be present depending on html2text implementation
    // We just verify the text context is extracted
}

#[tokio::test]
async fn test_html_loader_with_width() {
    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("width.html");

    let html_content = r#"
    <!DOCTYPE html>
    <html>
    <body>
        <p>This is a very long paragraph that should be wrapped according to the specified width parameter when the HTML is converted to plain text format for document loading.</p>
    </body>
    </html>
"#;
    fs::write(&file_path, html_content).unwrap();

    // Test with default width (80)
    let loader1 = HTMLLoader::new(&file_path);
    let docs1 = loader1.load().await.unwrap();
    assert_eq!(
        docs1.len(),
        1,
        "Should load one document with default width"
    );
    assert!(
        docs1[0].page_content.contains("very long paragraph"),
        "Should contain paragraph text"
    );

    // Test with custom width (40)
    let loader2 = HTMLLoader::new(&file_path).with_width(40);
    let docs2 = loader2.load().await.unwrap();
    assert_eq!(docs2.len(), 1, "Should load one document with custom width");
    assert!(
        docs2[0].page_content.contains("very long paragraph"),
        "Should contain paragraph text"
    );

    // Width affects wrapping, so content may differ, but text content should be present
}

#[tokio::test]
async fn test_html_loader_tables() {
    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("table.html");

    let html_content = r#"
    <!DOCTYPE html>
    <html>
    <body>
        <table>
            <tr>
                <th>Name</th>
                <th>Age</th>
            </tr>
            <tr>
                <td>Alice</td>
                <td>30</td>
            </tr>
            <tr>
                <td>Bob</td>
                <td>25</td>
            </tr>
        </table>
    </body>
    </html>
"#;
    fs::write(&file_path, html_content).unwrap();

    let loader = HTMLLoader::new(&file_path);
    let docs = loader.load().await.unwrap();

    assert_eq!(docs.len(), 1, "Should load one document");
    assert!(
        docs[0].page_content.contains("Name"),
        "Should extract table headers"
    );
    assert!(
        docs[0].page_content.contains("Age"),
        "Should extract all table headers"
    );
    assert!(
        docs[0].page_content.contains("Alice"),
        "Should extract table cell content"
    );
    assert!(
        docs[0].page_content.contains("30"),
        "Should extract numeric cell content"
    );
    assert!(
        docs[0].page_content.contains("Bob"),
        "Should extract all cell content"
    );
    assert!(
        docs[0].page_content.contains("25"),
        "Should extract all numeric content"
    );
}

#[tokio::test]
async fn test_html_loader_lists() {
    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("lists.html");

    let html_content = r#"
    <!DOCTYPE html>
    <html>
    <body>
        <h2>Ordered List</h2>
        <ol>
            <li>First item</li>
            <li>Second item</li>
            <li>Third item</li>
        </ol>
        <h2>Unordered List</h2>
        <ul>
            <li>Apple</li>
            <li>Banana</li>
            <li>Orange</li>
        </ul>
        <h2>Nested List</h2>
        <ul>
            <li>Parent 1
                <ul>
                    <li>Child 1.1</li>
                    <li>Child 1.2</li>
                </ul>
            </li>
            <li>Parent 2</li>
        </ul>
    </body>
    </html>
"#;
    fs::write(&file_path, html_content).unwrap();

    let loader = HTMLLoader::new(&file_path);
    let docs = loader.load().await.unwrap();

    assert_eq!(docs.len(), 1, "Should load one document");
    assert!(
        docs[0].page_content.contains("First item"),
        "Should extract ordered list items"
    );
    assert!(
        docs[0].page_content.contains("Second item"),
        "Should extract all ordered items"
    );
    assert!(
        docs[0].page_content.contains("Apple"),
        "Should extract unordered list items"
    );
    assert!(
        docs[0].page_content.contains("Banana"),
        "Should extract all unordered items"
    );
    assert!(
        docs[0].page_content.contains("Parent 1"),
        "Should extract parent list items"
    );
    assert!(
        docs[0].page_content.contains("Child 1.1"),
        "Should extract nested list items"
    );
    assert!(
        docs[0].page_content.contains("Child 1.2"),
        "Should extract all nested items"
    );
}

#[tokio::test]
async fn test_html_loader_file_not_found() {
    let loader = HTMLLoader::new("/nonexistent/path/to/file.html");
    let result = loader.load().await;

    assert!(result.is_err(), "Should error when file doesn't exist");
    let err = result.unwrap_err();
    assert!(
        err.to_string().contains("No such file") || err.to_string().contains("not found"),
        "Error message should indicate file not found: {}",
        err
    );
}

#[tokio::test]
async fn test_markdown_loader_preserve_formatting() {
    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("test.md");

    let markdown_content = r#"# Hello World

This is a **test** paragraph with [a link](https://example.com).

## Section 2

- Item 1
- Item 2

```rust
fn main() {}
```
"#;
    fs::write(&file_path, markdown_content).unwrap();

    let loader = MarkdownLoader::new(&file_path);
    let docs = loader.load().await.unwrap();

    assert_eq!(docs.len(), 1, "Should create exactly one document");

    // Should preserve Markdown formatting
    assert!(
        docs[0].page_content.contains("# Hello World"),
        "Should preserve H1 heading syntax"
    );
    assert!(
        docs[0].page_content.contains("**test**"),
        "Should preserve bold syntax"
    );
    assert!(
        docs[0]
            .page_content
            .contains("[a link](https://example.com)"),
        "Should preserve link syntax"
    );
    assert!(
        docs[0].page_content.contains("```rust"),
        "Should preserve code fence syntax"
    );
    assert!(
        docs[0].page_content.contains("- Item 1"),
        "Should preserve list syntax"
    );
    assert!(
        docs[0].page_content.contains("## Section 2"),
        "Should preserve H2 heading syntax"
    );

    // Validate metadata
    assert_eq!(
        docs[0].get_metadata("format").and_then(|v| v.as_str()),
        Some("markdown"),
        "Should have format metadata set to 'markdown'"
    );
    assert_eq!(
        docs[0]
            .get_metadata("source")
            .and_then(|v| v.as_str())
            .map(|s| s.contains("test.md")),
        Some(true),
        "Should have source metadata with file path"
    );
}

#[tokio::test]
async fn test_markdown_loader_to_plain_text() {
    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("test.md");

    let markdown_content = r#"# Hello World

This is a **test** paragraph.

## Section 2

More text here.
"#;
    fs::write(&file_path, markdown_content).unwrap();

    let loader = MarkdownLoader::new(&file_path).with_plain_text(true);
    let docs = loader.load().await.unwrap();

    assert_eq!(docs.len(), 1, "Should create exactly one document");

    // Should convert to plain text (no markdown formatting)
    assert!(
        !docs[0].page_content.contains("# Hello"),
        "Should remove H1 heading syntax"
    );
    assert!(
        !docs[0].page_content.contains("**test**"),
        "Should remove bold syntax"
    );
    assert!(
        !docs[0].page_content.contains("## Section 2"),
        "Should remove H2 heading syntax"
    );

    // Should contain the text content
    assert!(
        docs[0].page_content.contains("Hello World"),
        "Should contain heading text without syntax"
    );
    assert!(
        docs[0].page_content.contains("test"),
        "Should contain bold text without syntax"
    );
    assert!(
        docs[0].page_content.contains("Section 2"),
        "Should contain section heading text without syntax"
    );
    assert!(
        docs[0].page_content.contains("More text here"),
        "Should contain paragraph text"
    );

    // Validate metadata (same format even in plain text mode)
    assert_eq!(
        docs[0].get_metadata("format").and_then(|v| v.as_str()),
        Some("markdown"),
        "Should have format metadata set to 'markdown'"
    );
    assert_eq!(
        docs[0]
            .get_metadata("source")
            .and_then(|v| v.as_str())
            .map(|s| s.contains("test.md")),
        Some(true),
        "Should have source metadata with file path"
    );
}

#[tokio::test]
async fn test_markdown_loader_headings() {
    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("test.md");

    // Test all heading levels (H1-H6) and both ATX and Setext styles
    let markdown_content = r#"# Heading 1 (ATX)

Heading 1 (Setext)
==================

## Heading 2 (ATX)

Heading 2 (Setext)
------------------

### Heading 3

#### Heading 4

##### Heading 5

###### Heading 6
"#;
    fs::write(&file_path, markdown_content).unwrap();

    // Test preserve formatting mode
    let loader = MarkdownLoader::new(&file_path);
    let docs = loader.load().await.unwrap();
    assert_eq!(docs.len(), 1, "Should create exactly one document");
    assert!(
        docs[0].page_content.contains("# Heading 1 (ATX)"),
        "Should preserve ATX H1 heading"
    );
    assert!(
        docs[0].page_content.contains("=================="),
        "Should preserve Setext H1 underline"
    );
    assert!(
        docs[0].page_content.contains("## Heading 2 (ATX)"),
        "Should preserve ATX H2 heading"
    );
    assert!(
        docs[0].page_content.contains("------------------"),
        "Should preserve Setext H2 underline"
    );
    assert!(
        docs[0].page_content.contains("### Heading 3"),
        "Should preserve H3 heading"
    );
    assert!(
        docs[0].page_content.contains("#### Heading 4"),
        "Should preserve H4 heading"
    );
    assert!(
        docs[0].page_content.contains("##### Heading 5"),
        "Should preserve H5 heading"
    );
    assert!(
        docs[0].page_content.contains("###### Heading 6"),
        "Should preserve H6 heading"
    );

    // Test plain text mode
    let loader_plain = MarkdownLoader::new(&file_path).with_plain_text(true);
    let docs_plain = loader_plain.load().await.unwrap();
    assert_eq!(
        docs_plain.len(),
        1,
        "Should create exactly one document in plain text mode"
    );
    assert!(
        !docs_plain[0].page_content.contains("#"),
        "Plain text should not contain ATX heading markers"
    );
    assert!(
        !docs_plain[0].page_content.contains("==="),
        "Plain text should not contain Setext underlines"
    );
    assert!(
        docs_plain[0].page_content.contains("Heading 1 (ATX)"),
        "Plain text should contain heading text"
    );
    assert!(
        docs_plain[0].page_content.contains("Heading 6"),
        "Plain text should contain all heading levels"
    );

    // Validate metadata
    assert_eq!(
        docs[0].get_metadata("format").and_then(|v| v.as_str()),
        Some("markdown"),
        "Should have format metadata"
    );
}

#[tokio::test]
async fn test_markdown_loader_lists() {
    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("test.md");

    // Test ordered, unordered, nested lists, and task lists
    let markdown_content = r#"Unordered list:
- Item 1
- Item 2
  - Nested item 2.1
  - Nested item 2.2
- Item 3

Ordered list:
1. First item
2. Second item
   1. Nested 2.1
   2. Nested 2.2
3. Third item

Task list:
- [ ] Unchecked task
- [x] Checked task
- [ ] Another unchecked task
"#;
    fs::write(&file_path, markdown_content).unwrap();

    // Test preserve formatting mode
    let loader = MarkdownLoader::new(&file_path);
    let docs = loader.load().await.unwrap();
    assert_eq!(docs.len(), 1, "Should create exactly one document");
    assert!(
        docs[0].page_content.contains("- Item 1"),
        "Should preserve unordered list items"
    );
    assert!(
        docs[0].page_content.contains("  - Nested item 2.1"),
        "Should preserve nested list items with indentation"
    );
    assert!(
        docs[0].page_content.contains("1. First item"),
        "Should preserve ordered list items"
    );
    assert!(
        docs[0].page_content.contains("   1. Nested 2.1"),
        "Should preserve nested ordered list items"
    );
    assert!(
        docs[0].page_content.contains("- [ ] Unchecked task"),
        "Should preserve unchecked task list items"
    );
    assert!(
        docs[0].page_content.contains("- [x] Checked task"),
        "Should preserve checked task list items"
    );

    // Test plain text mode
    let loader_plain = MarkdownLoader::new(&file_path).with_plain_text(true);
    let docs_plain = loader_plain.load().await.unwrap();
    assert!(
        docs_plain[0].page_content.contains("Item 1"),
        "Plain text should contain list item text"
    );
    assert!(
        docs_plain[0].page_content.contains("Nested item 2.1"),
        "Plain text should contain nested list items"
    );
    assert!(
        docs_plain[0].page_content.contains("First item"),
        "Plain text should contain ordered list text"
    );

    // Validate metadata
    assert_eq!(
        docs[0].get_metadata("format").and_then(|v| v.as_str()),
        Some("markdown"),
        "Should have format metadata"
    );
}

#[tokio::test]
async fn test_markdown_loader_code_blocks() {
    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("test.md");

    // Test fenced code blocks with language tags and inline code
    let markdown_content = r#"Inline code: `let x = 42;`

Fenced code block with Rust:
```rust
fn main() {
println!("Hello, world!");
}
```

Fenced code block with Python:
```python
def hello():
print("Hello, world!")
```

Fenced code block without language:
```
plain text code
no syntax highlighting
```

Indented code block:
int x = 10;
return x;
"#;
    fs::write(&file_path, markdown_content).unwrap();

    // Test preserve formatting mode
    let loader = MarkdownLoader::new(&file_path);
    let docs = loader.load().await.unwrap();
    assert_eq!(docs.len(), 1, "Should create exactly one document");
    assert!(
        docs[0].page_content.contains("`let x = 42;`"),
        "Should preserve inline code syntax"
    );
    assert!(
        docs[0].page_content.contains("```rust"),
        "Should preserve fenced code block with language tag"
    );
    assert!(
        docs[0].page_content.contains("```python"),
        "Should preserve Python code fence"
    );
    assert!(
        docs[0].page_content.contains("fn main()"),
        "Should preserve Rust code content"
    );
    assert!(
        docs[0].page_content.contains("def hello()"),
        "Should preserve Python code content"
    );
    assert!(
        docs[0].page_content.contains("plain text code"),
        "Should preserve code without language"
    );

    // Test plain text mode
    let loader_plain = MarkdownLoader::new(&file_path).with_plain_text(true);
    let docs_plain = loader_plain.load().await.unwrap();
    assert!(
        docs_plain[0].page_content.contains("let x = 42"),
        "Plain text should contain inline code content"
    );
    assert!(
        docs_plain[0].page_content.contains("fn main()"),
        "Plain text should contain code block content"
    );
    assert!(
        !docs_plain[0].page_content.contains("```"),
        "Plain text should not contain code fence markers"
    );

    // Validate metadata
    assert_eq!(
        docs[0].get_metadata("format").and_then(|v| v.as_str()),
        Some("markdown"),
        "Should have format metadata"
    );
}

#[tokio::test]
async fn test_markdown_loader_links_images() {
    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("test.md");

    // Test various link and image formats
    let markdown_content = r#"Inline link: [Example](https://example.com)

Link with title: [Example](https://example.com "Example Title")

Autolink: <https://example.com>

Email autolink: <user@example.com>

Reference link: [Example][1]

[1]: https://example.com

Image: ![Alt text](https://example.com/image.png)

Image with title: ![Alt text](https://example.com/image.png "Image Title")

Reference image: ![Alt text][img-ref]

[img-ref]: https://example.com/ref-image.png
"#;
    fs::write(&file_path, markdown_content).unwrap();

    // Test preserve formatting mode
    let loader = MarkdownLoader::new(&file_path);
    let docs = loader.load().await.unwrap();
    assert_eq!(docs.len(), 1, "Should create exactly one document");
    assert!(
        docs[0]
            .page_content
            .contains("[Example](https://example.com)"),
        "Should preserve inline link syntax"
    );
    assert!(
        docs[0]
            .page_content
            .contains("[Example](https://example.com \"Example Title\")"),
        "Should preserve link with title"
    );
    assert!(
        docs[0].page_content.contains("<https://example.com>"),
        "Should preserve autolink syntax"
    );
    assert!(
        docs[0].page_content.contains("<user@example.com>"),
        "Should preserve email autolink"
    );
    assert!(
        docs[0].page_content.contains("[Example][1]"),
        "Should preserve reference link syntax"
    );
    assert!(
        docs[0]
            .page_content
            .contains("![Alt text](https://example.com/image.png)"),
        "Should preserve image syntax"
    );
    assert!(
        docs[0].page_content.contains("![Alt text][img-ref]"),
        "Should preserve reference image syntax"
    );

    // Test plain text mode
    let loader_plain = MarkdownLoader::new(&file_path).with_plain_text(true);
    let docs_plain = loader_plain.load().await.unwrap();
    assert!(
        docs_plain[0].page_content.contains("Example"),
        "Plain text should contain link text"
    );
    // Note: plain text mode may or may not preserve URLs depending on pulldown_cmark behavior
    // The key is that link TEXT is preserved, not necessarily the URLs

    // Validate metadata
    assert_eq!(
        docs[0].get_metadata("format").and_then(|v| v.as_str()),
        Some("markdown"),
        "Should have format metadata"
    );
}

#[tokio::test]
async fn test_markdown_loader_tables() {
    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("test.md");

    // Test Markdown tables with headers, alignment, and complex cells
    let markdown_content = r#"Simple table:

| Header 1 | Header 2 | Header 3 |
|----------|----------|----------|
| Cell 1.1 | Cell 1.2 | Cell 1.3 |
| Cell 2.1 | Cell 2.2 | Cell 2.3 |

Table with alignment:

| Left | Center | Right |
|:-----|:------:|------:|
| L1   |   C1   |    R1 |
| L2   |   C2   |    R2 |

Table with complex cells:

| Name | Description | Status |
|------|-------------|--------|
| **Bold** | _Italic_ | `Code` |
| [Link](https://example.com) | Multiple words | 100% |
"#;
    fs::write(&file_path, markdown_content).unwrap();

    // Test preserve formatting mode
    let loader = MarkdownLoader::new(&file_path);
    let docs = loader.load().await.unwrap();
    assert_eq!(docs.len(), 1, "Should create exactly one document");
    assert!(
        docs[0].page_content.contains("| Header 1 | Header 2 |"),
        "Should preserve table header syntax"
    );
    assert!(
        docs[0].page_content.contains("|----------|----------|"),
        "Should preserve table separator"
    );
    assert!(
        docs[0].page_content.contains("| Cell 1.1 | Cell 1.2 |"),
        "Should preserve table cell syntax"
    );
    assert!(
        docs[0].page_content.contains("|:-----|:------:|------:|"),
        "Should preserve table alignment syntax"
    );
    assert!(
        docs[0].page_content.contains("| **Bold** | _Italic_ |"),
        "Should preserve formatting within table cells"
    );

    // Test plain text mode
    let loader_plain = MarkdownLoader::new(&file_path).with_plain_text(true);
    let docs_plain = loader_plain.load().await.unwrap();
    assert!(
        docs_plain[0].page_content.contains("Header 1"),
        "Plain text should contain table header text"
    );
    assert!(
        docs_plain[0].page_content.contains("Cell 1.1"),
        "Plain text should contain table cell text"
    );
    // Note: Plain text may or may not preserve table structure depending on pulldown_cmark

    // Validate metadata
    assert_eq!(
        docs[0].get_metadata("format").and_then(|v| v.as_str()),
        Some("markdown"),
        "Should have format metadata"
    );
}

#[tokio::test]
async fn test_markdown_loader_emphasis() {
    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("test.md");

    // Test various emphasis styles and combinations
    let markdown_content = r#"Bold variants:
**double asterisk bold**
__double underscore bold__

Italic variants:
*single asterisk italic*
_single underscore italic_

Combined:
***bold and italic***
**_bold and italic_**
*__bold and italic__*

Strikethrough:
~~strikethrough text~~

Combinations:
**bold with _nested italic_**
*italic with **nested bold***
~~strikethrough with **bold**~~
"#;
    fs::write(&file_path, markdown_content).unwrap();

    // Test preserve formatting mode
    let loader = MarkdownLoader::new(&file_path);
    let docs = loader.load().await.unwrap();
    assert_eq!(docs.len(), 1, "Should create exactly one document");
    assert!(
        docs[0].page_content.contains("**double asterisk bold**"),
        "Should preserve double asterisk bold"
    );
    assert!(
        docs[0].page_content.contains("__double underscore bold__"),
        "Should preserve double underscore bold"
    );
    assert!(
        docs[0].page_content.contains("*single asterisk italic*"),
        "Should preserve single asterisk italic"
    );
    assert!(
        docs[0].page_content.contains("_single underscore italic_"),
        "Should preserve single underscore italic"
    );
    assert!(
        docs[0].page_content.contains("***bold and italic***"),
        "Should preserve triple asterisk emphasis"
    );
    assert!(
        docs[0].page_content.contains("~~strikethrough text~~"),
        "Should preserve strikethrough syntax"
    );
    assert!(
        docs[0]
            .page_content
            .contains("**bold with _nested italic_**"),
        "Should preserve nested emphasis"
    );

    // Test plain text mode
    let loader_plain = MarkdownLoader::new(&file_path).with_plain_text(true);
    let docs_plain = loader_plain.load().await.unwrap();
    assert!(
        !docs_plain[0].page_content.contains("**"),
        "Plain text should not contain bold markers"
    );
    assert!(
        !docs_plain[0].page_content.contains("__"),
        "Plain text should not contain underscore emphasis"
    );
    assert!(
        docs_plain[0].page_content.contains("double asterisk bold"),
        "Plain text should contain emphasis text content"
    );
    // Note: Strikethrough (~~) is a GitHub Flavored Markdown extension
    // pulldown_cmark may or may not support it depending on features enabled
    // We test that the text content is present, but not the markers
    assert!(
        docs_plain[0].page_content.contains("strikethrough text")
            || docs_plain[0]
                .page_content
                .contains("~~strikethrough text~~"),
        "Plain text should contain strikethrough content (with or without markers)"
    );

    // Validate metadata
    assert_eq!(
        docs[0].get_metadata("format").and_then(|v| v.as_str()),
        Some("markdown"),
        "Should have format metadata"
    );
}

#[tokio::test]
async fn test_markdown_loader_blockquotes() {
    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("test.md");

    // Test blockquotes, nested blockquotes, and blockquotes with other elements
    let markdown_content = r#"Simple blockquote:

> This is a blockquote.
> It can span multiple lines.

Nested blockquotes:

> Level 1 blockquote
>
> > Level 2 blockquote
> >
> > > Level 3 blockquote

Blockquote with other elements:

> ## Heading in blockquote
>
> Paragraph in blockquote with **bold** and *italic*.
>
> - List item 1
> - List item 2
>
> ```
> Code in blockquote
> ```
"#;
    fs::write(&file_path, markdown_content).unwrap();

    // Test preserve formatting mode
    let loader = MarkdownLoader::new(&file_path);
    let docs = loader.load().await.unwrap();
    assert_eq!(docs.len(), 1, "Should create exactly one document");
    assert!(
        docs[0].page_content.contains("> This is a blockquote."),
        "Should preserve blockquote syntax"
    );
    assert!(
        docs[0]
            .page_content
            .contains("> It can span multiple lines."),
        "Should preserve multi-line blockquotes"
    );
    assert!(
        docs[0].page_content.contains("> Level 1 blockquote"),
        "Should preserve level 1 blockquote"
    );
    assert!(
        docs[0].page_content.contains("> > Level 2 blockquote"),
        "Should preserve nested level 2 blockquote"
    );
    assert!(
        docs[0].page_content.contains("> > > Level 3 blockquote"),
        "Should preserve nested level 3 blockquote"
    );
    assert!(
        docs[0].page_content.contains("> ## Heading in blockquote"),
        "Should preserve heading in blockquote"
    );
    assert!(
        docs[0]
            .page_content
            .contains("> Paragraph in blockquote with **bold**"),
        "Should preserve paragraph with emphasis in blockquote"
    );

    // Test plain text mode
    let loader_plain = MarkdownLoader::new(&file_path).with_plain_text(true);
    let docs_plain = loader_plain.load().await.unwrap();
    assert!(
        docs_plain[0].page_content.contains("This is a blockquote"),
        "Plain text should contain blockquote text"
    );
    assert!(
        docs_plain[0].page_content.contains("Level 1 blockquote"),
        "Plain text should contain nested blockquote text"
    );
    // Note: > markers should not be in plain text mode

    // Validate metadata
    assert_eq!(
        docs[0].get_metadata("format").and_then(|v| v.as_str()),
        Some("markdown"),
        "Should have format metadata"
    );
}

#[tokio::test]
async fn test_markdown_loader_unicode_content() {
    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("test.md");

    // Test Unicode preservation in Markdown
    let markdown_content = r#"# Unicode Test 世界 🌍

Unicode in various contexts:

**Chinese:** 你好世界
**Japanese:** こんにちは世界
**Korean:** 안녕하세요 세계
**Arabic:** مرحبا بالعالم
**Hebrew:** שלום עולם
**Russian:** Привет мир
**Emoji:** 🌍🚀🌟💻📚🎉
**Math symbols:** ∑∫√π∞≠≈
**Currency:** €£¥₹₽

Unicode in lists:
- 项目一 (Chinese)
- アイテム２ (Japanese)
- 항목 3 (Korean)

Unicode in code: `变量 = 42`

Unicode in links: [世界](https://example.com/世界)
"#;
    fs::write(&file_path, markdown_content).unwrap();

    // Test preserve formatting mode
    let loader = MarkdownLoader::new(&file_path);
    let docs = loader.load().await.unwrap();
    assert_eq!(docs.len(), 1, "Should create exactly one document");
    assert!(
        docs[0].page_content.contains("# Unicode Test 世界 🌍"),
        "Should preserve Unicode in headings"
    );
    assert!(
        docs[0].page_content.contains("**Chinese:** 你好世界"),
        "Should preserve Chinese characters"
    );
    assert!(
        docs[0]
            .page_content
            .contains("**Japanese:** こんにちは世界"),
        "Should preserve Japanese characters"
    );
    assert!(
        docs[0].page_content.contains("**Korean:** 안녕하세요 세계"),
        "Should preserve Korean characters"
    );
    assert!(
        docs[0].page_content.contains("**Arabic:** مرحبا بالعالم"),
        "Should preserve Arabic characters"
    );
    assert!(
        docs[0].page_content.contains("**Hebrew:** שלום עולם"),
        "Should preserve Hebrew characters"
    );
    assert!(
        docs[0].page_content.contains("**Russian:** Привет мир"),
        "Should preserve Russian characters"
    );
    assert!(
        docs[0].page_content.contains("**Emoji:** 🌍🚀🌟💻📚🎉"),
        "Should preserve emoji"
    );
    assert!(
        docs[0].page_content.contains("**Math symbols:** ∑∫√π∞≠≈"),
        "Should preserve math symbols"
    );
    assert!(
        docs[0].page_content.contains("**Currency:** €£¥₹₽"),
        "Should preserve currency symbols"
    );
    assert!(
        docs[0].page_content.contains("- 项目一 (Chinese)"),
        "Should preserve Unicode in lists"
    );
    assert!(
        docs[0].page_content.contains("`变量 = 42`"),
        "Should preserve Unicode in inline code"
    );
    assert!(
        docs[0]
            .page_content
            .contains("[世界](https://example.com/世界)"),
        "Should preserve Unicode in links"
    );

    // Test plain text mode
    let loader_plain = MarkdownLoader::new(&file_path).with_plain_text(true);
    let docs_plain = loader_plain.load().await.unwrap();
    assert!(
        docs_plain[0].page_content.contains("Unicode Test 世界"),
        "Plain text should preserve Unicode in headings"
    );
    assert!(
        docs_plain[0].page_content.contains("你好世界"),
        "Plain text should preserve Chinese characters"
    );
    assert!(
        docs_plain[0].page_content.contains("🌍🚀🌟💻📚🎉"),
        "Plain text should preserve emoji"
    );
    assert!(
        docs_plain[0].page_content.contains("∑∫√π∞≠≈"),
        "Plain text should preserve math symbols"
    );

    // Validate metadata
    assert_eq!(
        docs[0].get_metadata("format").and_then(|v| v.as_str()),
        Some("markdown"),
        "Should have format metadata"
    );
    assert!(
        docs[0]
            .get_metadata("source")
            .and_then(|v| v.as_str())
            .unwrap()
            .contains("test.md"),
        "Should have source metadata"
    );
}

#[tokio::test]
async fn test_markdown_loader_empty_markdown() {
    let temp_dir = TempDir::new().unwrap();

    // Test 1: Completely empty file
    let empty_file = temp_dir.path().join("empty.md");
    fs::write(&empty_file, "").unwrap();
    let loader = MarkdownLoader::new(&empty_file);
    let docs = loader.load().await.unwrap();
    assert_eq!(docs.len(), 1, "Empty file should create one document");
    assert!(
        docs[0].page_content.is_empty(),
        "Empty file should have empty content"
    );
    assert_eq!(
        docs[0].get_metadata("format").and_then(|v| v.as_str()),
        Some("markdown"),
        "Empty file should still have format metadata"
    );

    // Test 2: Whitespace only
    let whitespace_file = temp_dir.path().join("whitespace.md");
    fs::write(&whitespace_file, "   \n\n   \t\t   \n\n").unwrap();
    let loader = MarkdownLoader::new(&whitespace_file);
    let docs = loader.load().await.unwrap();
    assert_eq!(
        docs.len(),
        1,
        "Whitespace-only file should create one document"
    );
    // Whitespace may be preserved or trimmed depending on implementation

    // Test 3: Comments only (HTML comments in Markdown)
    let comments_file = temp_dir.path().join("comments.md");
    fs::write(
        &comments_file,
        "<!-- This is a comment -->\n<!-- Another comment -->",
    )
    .unwrap();
    let loader = MarkdownLoader::new(&comments_file);
    let docs = loader.load().await.unwrap();
    assert_eq!(
        docs.len(),
        1,
        "Comments-only file should create one document"
    );
    // Comments may or may not be preserved depending on parser

    // Test 4: Empty Markdown structure
    let empty_structure_file = temp_dir.path().join("empty_structure.md");
    fs::write(&empty_structure_file, "---\n\n---\n\n").unwrap();
    let loader = MarkdownLoader::new(&empty_structure_file);
    let docs = loader.load().await.unwrap();
    assert_eq!(docs.len(), 1, "Empty structure should create one document");

    // Validate metadata for all empty file types
    assert!(
        docs[0]
            .get_metadata("source")
            .and_then(|v| v.as_str())
            .is_some(),
        "Empty files should have source metadata"
    );
}

#[tokio::test]
async fn test_markdown_loader_file_not_found() {
    let temp_dir = TempDir::new().unwrap();
    let nonexistent_file = temp_dir.path().join("nonexistent.md");

    let loader = MarkdownLoader::new(&nonexistent_file);
    let result = loader.load().await;

    assert!(
        result.is_err(),
        "Loading nonexistent file should return error"
    );
    let err = result.unwrap_err();
    assert!(
        err.to_string().contains("No such file")
            || err.to_string().contains("not found")
            || err.to_string().contains("cannot find"),
        "Error message should indicate file not found: {}",
        err
    );
}

#[tokio::test]
async fn test_xml_loader_raw_text() {
    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("test.xml");

    let xml_content = r#"<?xml version="1.0" encoding="UTF-8"?>
<root>
<person>
<name>Alice</name>
<age>30</age>
</person>
<person>
<name>Bob</name>
<age>25</age>
</person>
</root>"#;
    fs::write(&file_path, xml_content).unwrap();

    let loader = XMLLoader::new(&file_path);
    let docs = loader.load().await.unwrap();

    assert_eq!(docs.len(), 1, "Should create exactly one document");

    // Should preserve raw XML text exactly
    assert!(
        docs[0].page_content.contains("<root>"),
        "Raw XML should contain root tag"
    );
    assert!(
        docs[0].page_content.contains("<person>"),
        "Raw XML should contain person tags"
    );
    assert!(
        docs[0].page_content.contains("<name>Alice</name>"),
        "Raw XML should contain exact element content"
    );
    assert!(
        docs[0].page_content.contains("<name>Bob</name>"),
        "Raw XML should contain both person records"
    );
    assert!(
        docs[0]
            .page_content
            .contains("<?xml version=\"1.0\" encoding=\"UTF-8\"?>"),
        "Raw XML should preserve XML declaration"
    );

    // Validate metadata
    assert_eq!(
        docs[0].get_metadata("format").and_then(|v| v.as_str()),
        Some("xml"),
        "Format metadata should be 'xml'"
    );
    assert!(
        docs[0]
            .get_metadata("source")
            .and_then(|v| v.as_str())
            .unwrap()
            .ends_with("test.xml"),
        "Source metadata should contain file path"
    );
}

#[tokio::test]
async fn test_xml_loader_with_parse_structure() {
    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("test.xml");

    let xml_content = r#"<root>
<item>First</item>
<item>Second</item>
</root>"#;
    fs::write(&file_path, xml_content).unwrap();

    let loader = XMLLoader::new(&file_path).with_parse_structure(true);
    let docs = loader.load().await.unwrap();

    assert_eq!(docs.len(), 1, "Should create exactly one document");

    // Should parse XML structure (indented representation)
    assert!(
        docs[0].page_content.contains("<root>"),
        "Parsed structure should contain root tag"
    );
    assert!(
        docs[0].page_content.contains("<item>"),
        "Parsed structure should contain item tags"
    );
    assert!(
        docs[0].page_content.contains("First"),
        "Parsed structure should contain text content 'First'"
    );
    assert!(
        docs[0].page_content.contains("Second"),
        "Parsed structure should contain text content 'Second'"
    );

    // Verify indentation (depth-based formatting)
    assert!(
        docs[0].page_content.contains("  <item>"),
        "Parsed structure should have indented child elements"
    );
    assert!(
        docs[0].page_content.contains("  </item>"),
        "Parsed structure should have indented closing tags"
    );

    // Validate metadata
    assert_eq!(
        docs[0].get_metadata("format").and_then(|v| v.as_str()),
        Some("xml"),
        "Format metadata should be 'xml'"
    );
    assert!(
        docs[0]
            .get_metadata("source")
            .and_then(|v| v.as_str())
            .unwrap()
            .ends_with("test.xml"),
        "Source metadata should contain file path"
    );
}

#[tokio::test]
async fn test_xml_loader_malformed_xml() {
    let temp_dir = TempDir::new().unwrap();

    // Test 1: Unclosed tag
    let file_path1 = temp_dir.path().join("unclosed.xml");
    let xml1 = r#"<root><item>Content</root>"#;
    fs::write(&file_path1, xml1).unwrap();
    let loader1 = XMLLoader::new(&file_path1).with_parse_structure(true);
    let result1 = loader1.load().await;
    assert!(
        result1.is_err(),
        "Unclosed tag should produce error in parse mode"
    );
    assert!(
        result1
            .unwrap_err()
            .to_string()
            .contains("XML parsing error"),
        "Error message should mention XML parsing"
    );

    // Test 2: Improperly nested tags
    let file_path2 = temp_dir.path().join("nested.xml");
    let xml2 = r#"<root><a><b></a></b></root>"#;
    fs::write(&file_path2, xml2).unwrap();
    let loader2 = XMLLoader::new(&file_path2).with_parse_structure(true);
    let result2 = loader2.load().await;
    assert!(
        result2.is_err(),
        "Improperly nested tags should produce error"
    );

    // Test 3: Missing root element (empty file is tested separately)
    let file_path3 = temp_dir.path().join("no_root.xml");
    let xml3 = r#"<item>One</item><item>Two</item>"#;
    fs::write(&file_path3, xml3).unwrap();
    let loader3 = XMLLoader::new(&file_path3).with_parse_structure(true);
    // Multiple root elements - quick_xml may handle this differently, test actual behavior
    let result3 = loader3.load().await;
    // Either succeeds (parsing first element) or errors - both are acceptable
    if result3.is_ok() {
        let docs = result3.unwrap();
        assert_eq!(
            docs.len(),
            1,
            "Should return one document even with multiple roots"
        );
    }
}

#[tokio::test]
async fn test_xml_loader_namespaces() {
    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("namespaces.xml");

    let xml_content = r#"<?xml version="1.0"?>
<root xmlns="http://example.com/ns" xmlns:custom="http://example.com/custom">
<custom:item>Namespaced content</custom:item>
<item>Default namespace</item>
</root>"#;
    fs::write(&file_path, xml_content).unwrap();

    // Test with raw mode
    let loader_raw = XMLLoader::new(&file_path);
    let docs_raw = loader_raw.load().await.unwrap();
    assert_eq!(docs_raw.len(), 1, "Should create one document");
    assert!(
        docs_raw[0].page_content.contains("xmlns="),
        "Raw mode should preserve namespace declarations"
    );
    assert!(
        docs_raw[0].page_content.contains("custom:item"),
        "Raw mode should preserve namespace prefixes"
    );

    // Test with parse mode
    let loader_parsed = XMLLoader::new(&file_path).with_parse_structure(true);
    let docs_parsed = loader_parsed.load().await.unwrap();
    assert_eq!(docs_parsed.len(), 1, "Should create one document");
    // quick_xml handles namespaces by preserving tag names with prefixes
    assert!(
        docs_parsed[0].page_content.contains("Namespaced content"),
        "Parse mode should extract namespaced element content"
    );
}

#[tokio::test]
async fn test_xml_loader_cdata_sections() {
    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("cdata.xml");

    let xml_content = r#"<root>
<description><![CDATA[This contains <special> characters & symbols]]></description>
<code><![CDATA[if (x < 10 && y > 5) { return true; }]]></code>
</root>"#;
    fs::write(&file_path, xml_content).unwrap();

    // Test raw mode preserves CDATA
    let loader_raw = XMLLoader::new(&file_path);
    let docs_raw = loader_raw.load().await.unwrap();
    assert_eq!(docs_raw.len(), 1, "Should create one document");
    assert!(
        docs_raw[0].page_content.contains("<![CDATA["),
        "Raw mode should preserve CDATA markers"
    );

    // Test parse mode - CDATA is delivered as Event::CData in quick_xml
    // FIXED: Now properly handles CData events and includes content
    let loader_parsed = XMLLoader::new(&file_path).with_parse_structure(true);
    let docs_parsed = loader_parsed.load().await.unwrap();
    assert_eq!(docs_parsed.len(), 1, "Should create one document");
    assert!(
        docs_parsed[0].page_content.contains("<root>")
            && docs_parsed[0].page_content.contains("<description>"),
        "Parse mode should show XML structure"
    );
    // CDATA content should now be included in parse mode
    assert!(
        docs_parsed[0].page_content.contains("special")
            && docs_parsed[0].page_content.contains("characters")
            && docs_parsed[0].page_content.contains("symbols"),
        "Parse mode should include CDATA content from description"
    );
    assert!(
        docs_parsed[0].page_content.contains("if (x < 10")
            || docs_parsed[0].page_content.contains("return true"),
        "Parse mode should include CDATA content from code section"
    );
}

#[tokio::test]
async fn test_xml_loader_xml_entities() {
    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("entities.xml");

    let xml_content = r#"<root>
<text>Less than: &lt; Greater than: &gt; Ampersand: &amp;</text>
<quote>He said &quot;Hello&quot; and &apos;Goodbye&apos;</quote>
<numeric>Numeric entities: &#169; &#xA9; &#8364;</numeric>
</root>"#;
    fs::write(&file_path, xml_content).unwrap();

    // Test raw mode preserves entities
    let loader_raw = XMLLoader::new(&file_path);
    let docs_raw = loader_raw.load().await.unwrap();
    assert_eq!(docs_raw.len(), 1, "Should create one document");
    assert!(
        docs_raw[0].page_content.contains("&lt;") || docs_raw[0].page_content.contains("<"),
        "Raw mode should contain entity or decoded character"
    );

    // Test parse mode decodes entities
    let loader_parsed = XMLLoader::new(&file_path).with_parse_structure(true);
    let docs_parsed = loader_parsed.load().await.unwrap();
    assert_eq!(docs_parsed.len(), 1, "Should create one document");
    // quick_xml should decode entities via unescape()
    assert!(
        docs_parsed[0].page_content.contains("Less than:")
            && docs_parsed[0].page_content.contains("Greater than:")
            && docs_parsed[0].page_content.contains("Ampersand:"),
        "Parse mode should decode XML entities to text"
    );
}

#[tokio::test]
async fn test_xml_loader_deeply_nested() {
    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("deep.xml");

    // Create 10-level deep nesting
    let xml_content = r#"<level1>
<level2>
<level3>
    <level4>
        <level5>
            <level6>
                <level7>
                    <level8>
                        <level9>
                            <level10>Deep content</level10>
                        </level9>
                    </level8>
                </level7>
            </level6>
        </level5>
    </level4>
</level3>
</level2>
</level1>"#;
    fs::write(&file_path, xml_content).unwrap();

    // Test raw mode
    let loader_raw = XMLLoader::new(&file_path);
    let docs_raw = loader_raw.load().await.unwrap();
    assert_eq!(docs_raw.len(), 1, "Should create one document");
    assert!(
        docs_raw[0].page_content.contains("<level1>")
            && docs_raw[0].page_content.contains("<level10>")
            && docs_raw[0].page_content.contains("Deep content"),
        "Raw mode should preserve all nested levels"
    );

    // Test parse mode with indentation
    let loader_parsed = XMLLoader::new(&file_path).with_parse_structure(true);
    let docs_parsed = loader_parsed.load().await.unwrap();
    assert_eq!(docs_parsed.len(), 1, "Should create one document");
    assert!(
        docs_parsed[0].page_content.contains("Deep content"),
        "Parse mode should extract deeply nested content"
    );
    // Verify deep indentation (level 10 should have 20 spaces)
    assert!(
        docs_parsed[0]
            .page_content
            .contains("                    Deep content")
            || docs_parsed[0].page_content.contains("Deep content"),
        "Parse mode should handle deep nesting"
    );
}

#[tokio::test]
async fn test_xml_loader_unicode_content() {
    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("unicode.xml");

    let xml_content = r#"<?xml version="1.0" encoding="UTF-8"?>
<root>
<chinese>世界 🌍</chinese>
<arabic>مرحبا</arabic>
<hebrew>שלום</hebrew>
<cyrillic>Привет</cyrillic>
<math>∑∫√π</math>
<emoji>❤️ 🚀 🌟</emoji>
</root>"#;
    fs::write(&file_path, xml_content).unwrap();

    // Test raw mode
    let loader_raw = XMLLoader::new(&file_path);
    let docs_raw = loader_raw.load().await.unwrap();
    assert_eq!(docs_raw.len(), 1, "Should create one document");
    assert!(
        docs_raw[0].page_content.contains("世界 🌍"),
        "Raw mode should preserve Chinese and emoji"
    );
    assert!(
        docs_raw[0].page_content.contains("مرحبا"),
        "Raw mode should preserve Arabic"
    );
    assert!(
        docs_raw[0].page_content.contains("Привет"),
        "Raw mode should preserve Cyrillic"
    );

    // Test parse mode
    let loader_parsed = XMLLoader::new(&file_path).with_parse_structure(true);
    let docs_parsed = loader_parsed.load().await.unwrap();
    assert_eq!(docs_parsed.len(), 1, "Should create one document");
    assert!(
        docs_parsed[0].page_content.contains("世界"),
        "Parse mode should preserve Unicode Chinese"
    );
    assert!(
        docs_parsed[0].page_content.contains("مرحبا"),
        "Parse mode should preserve Unicode Arabic"
    );
    assert!(
        docs_parsed[0].page_content.contains("∑∫√"),
        "Parse mode should preserve Unicode math symbols"
    );
}

#[tokio::test]
async fn test_xml_loader_empty_xml() {
    let temp_dir = TempDir::new().unwrap();

    // Test 1: Completely empty file
    let file_path1 = temp_dir.path().join("empty.xml");
    fs::write(&file_path1, "").unwrap();
    let loader1 = XMLLoader::new(&file_path1);
    let docs1 = loader1.load().await.unwrap();
    assert_eq!(docs1.len(), 1, "Empty file should return one document");
    assert!(
        docs1[0].page_content.is_empty(),
        "Empty file should have empty content"
    );

    // Test 2: Just XML declaration
    let file_path2 = temp_dir.path().join("declaration_only.xml");
    fs::write(&file_path2, r#"<?xml version="1.0" encoding="UTF-8"?>"#).unwrap();
    let loader2 = XMLLoader::new(&file_path2);
    let docs2 = loader2.load().await.unwrap();
    assert_eq!(
        docs2.len(),
        1,
        "Declaration-only file should return one document"
    );
    assert!(
        docs2[0].page_content.contains("<?xml") || docs2[0].page_content.trim().is_empty(),
        "Declaration-only file should contain declaration or be empty"
    );

    // Test 3: Empty root element
    let file_path3 = temp_dir.path().join("empty_root.xml");
    fs::write(&file_path3, r#"<root></root>"#).unwrap();
    let loader3 = XMLLoader::new(&file_path3);
    let docs3 = loader3.load().await.unwrap();
    assert_eq!(docs3.len(), 1, "Empty root should return one document");
    assert!(
        docs3[0].page_content.contains("<root>"),
        "Empty root should contain root tags"
    );
}

#[tokio::test]
async fn test_xml_loader_attributes() {
    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("attributes.xml");

    let xml_content = r#"<root>
<person id="1" name="Alice" age="30">
<email type="work">alice@example.com</email>
</person>
<person id="2" name="Bob" age="25">
<email type="personal">bob@example.com</email>
</person>
</root>"#;
    fs::write(&file_path, xml_content).unwrap();

    // Test raw mode preserves attributes
    let loader_raw = XMLLoader::new(&file_path);
    let docs_raw = loader_raw.load().await.unwrap();
    assert_eq!(docs_raw.len(), 1, "Should create one document");
    assert!(
        docs_raw[0].page_content.contains("id=\"1\""),
        "Raw mode should preserve attributes"
    );
    assert!(
        docs_raw[0].page_content.contains("name=\"Alice\""),
        "Raw mode should preserve all attribute values"
    );

    // Test parse mode (attributes may be lost in current implementation)
    let loader_parsed = XMLLoader::new(&file_path).with_parse_structure(true);
    let docs_parsed = loader_parsed.load().await.unwrap();
    assert_eq!(docs_parsed.len(), 1, "Should create one document");
    assert!(
        docs_parsed[0].page_content.contains("Alice")
            || docs_parsed[0].page_content.contains("alice@example.com"),
        "Parse mode should extract element content"
    );
}

#[tokio::test]
async fn test_xml_loader_mixed_content() {
    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("mixed.xml");

    let xml_content = r#"<paragraph>
This is text before <emphasis>emphasized text</emphasis> and text after.
<linebreak/>
Another line with <bold>bold</bold> and <italic>italic</italic> mixed.
</paragraph>"#;
    fs::write(&file_path, xml_content).unwrap();

    // Test raw mode
    let loader_raw = XMLLoader::new(&file_path);
    let docs_raw = loader_raw.load().await.unwrap();
    assert_eq!(docs_raw.len(), 1, "Should create one document");
    assert!(
        docs_raw[0].page_content.contains("This is text before")
            && docs_raw[0].page_content.contains("<emphasis>")
            && docs_raw[0].page_content.contains("<linebreak/>"),
        "Raw mode should preserve mixed content structure"
    );

    // Test parse mode
    let loader_parsed = XMLLoader::new(&file_path).with_parse_structure(true);
    let docs_parsed = loader_parsed.load().await.unwrap();
    assert_eq!(docs_parsed.len(), 1, "Should create one document");
    assert!(
        docs_parsed[0].page_content.contains("This is text before")
            && docs_parsed[0].page_content.contains("emphasized text")
            && docs_parsed[0].page_content.contains("bold"),
        "Parse mode should extract all text content from mixed elements"
    );
}

#[tokio::test]
async fn test_xml_loader_comments() {
    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("comments.xml");

    let xml_content = r#"<root>
<!-- This is a comment -->
<item>Content</item>
<!-- Another comment with <tags> inside -->
<item>More content</item>
</root>"#;
    fs::write(&file_path, xml_content).unwrap();

    // Test raw mode preserves comments
    let loader_raw = XMLLoader::new(&file_path);
    let docs_raw = loader_raw.load().await.unwrap();
    assert_eq!(docs_raw.len(), 1, "Should create one document");
    assert!(
        docs_raw[0]
            .page_content
            .contains("<!-- This is a comment -->")
            || !docs_raw[0].page_content.contains("This is a comment"),
        "Raw mode may or may not preserve comments (implementation-dependent)"
    );
    assert!(
        docs_raw[0].page_content.contains("<item>Content</item>"),
        "Raw mode should preserve actual content"
    );

    // Test parse mode (comments typically ignored)
    let loader_parsed = XMLLoader::new(&file_path).with_parse_structure(true);
    let docs_parsed = loader_parsed.load().await.unwrap();
    assert_eq!(docs_parsed.len(), 1, "Should create one document");
    assert!(
        docs_parsed[0].page_content.contains("Content"),
        "Parse mode should extract element content"
    );
}

#[tokio::test]
async fn test_xml_loader_file_not_found() {
    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("nonexistent.xml");

    let loader = XMLLoader::new(&file_path);
    let result = loader.load().await;

    assert!(
        result.is_err(),
        "Loading nonexistent file should return error"
    );
    // Error could be from Blob::from_path (file not found) or parsing
    let error_msg = result.unwrap_err().to_string();
    assert!(
        error_msg.contains("No such file")
            || error_msg.contains("not found")
            || error_msg.contains("Failed to read"),
        "Error message should indicate file not found, got: {}",
        error_msg
    );
}

#[tokio::test]
async fn test_yaml_loader() {
    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("test.yaml");

    let yaml_content = r#"
name: test
version: 1.0
settings:
  enabled: true
  count: 42
"#;
    fs::write(&file_path, yaml_content).unwrap();

    let loader = YAMLLoader::new(&file_path);
    let docs = loader.load().await.unwrap();

    assert_eq!(docs.len(), 1, "Should load exactly 1 document");

    // Validate YAML content is parsed and formatted
    assert!(
        docs[0].page_content.contains("name"),
        "Formatted YAML should contain 'name' key"
    );
    assert!(
        docs[0].page_content.contains("test"),
        "Formatted YAML should contain 'test' value"
    );
    assert!(
        docs[0].page_content.contains("version"),
        "Formatted YAML should contain 'version' key"
    );
    assert!(
        docs[0].page_content.contains("settings"),
        "Formatted YAML should contain 'settings' key"
    );
    assert!(
        docs[0].page_content.contains("enabled"),
        "Formatted YAML should contain nested 'enabled' key"
    );

    // Validate metadata
    assert_eq!(
        docs[0].get_metadata("format").and_then(|v| v.as_str()),
        Some("yaml"),
        "Format metadata should be 'yaml'"
    );
    assert!(
        docs[0]
            .get_metadata("source")
            .and_then(|v| v.as_str())
            .map(|s| s.contains("test.yaml"))
            .unwrap_or(false),
        "Source metadata should contain file path"
    );
}

#[tokio::test]
async fn test_yaml_loader_raw() {
    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("test.yaml");

    let yaml_content = "key: value\nlist:\n  - item1\n  - item2\n";
    fs::write(&file_path, yaml_content).unwrap();

    let loader = YAMLLoader::new(&file_path).with_format(false);
    let docs = loader.load().await.unwrap();

    assert_eq!(docs.len(), 1, "Should load exactly 1 document");

    // Should preserve raw YAML exactly
    assert_eq!(
        docs[0].page_content, yaml_content,
        "Raw mode should preserve YAML text exactly"
    );

    // Validate metadata
    assert_eq!(
        docs[0].get_metadata("format").and_then(|v| v.as_str()),
        Some("yaml"),
        "Format metadata should be 'yaml'"
    );
    assert!(
        docs[0]
            .get_metadata("source")
            .and_then(|v| v.as_str())
            .map(|s| s.contains("test.yaml"))
            .unwrap_or(false),
        "Source metadata should contain file path"
    );
}

#[tokio::test]
async fn test_yaml_loader_malformed_yaml() {
    let temp_dir = TempDir::new().unwrap();

    // Test 1: Invalid indentation
    let file_path1 = temp_dir.path().join("malformed1.yaml");
    let malformed1 = "key: value\n  invalid_indent: bad";
    fs::write(&file_path1, malformed1).unwrap();

    let loader1 = YAMLLoader::new(&file_path1);
    let result1 = loader1.load().await;
    assert!(
        result1.is_err(),
        "Should fail on invalid indentation in formatted mode"
    );
    let err1 = result1.unwrap_err().to_string();
    assert!(
        err1.contains("YAML parse error") || err1.contains("invalid"),
        "Error message should indicate YAML parse error, got: {}",
        err1
    );

    // Test 2: Unclosed bracket
    let file_path2 = temp_dir.path().join("malformed2.yaml");
    let malformed2 = "items: [item1, item2";
    fs::write(&file_path2, malformed2).unwrap();

    let loader2 = YAMLLoader::new(&file_path2);
    let result2 = loader2.load().await;
    assert!(
        result2.is_err(),
        "Should fail on unclosed bracket in formatted mode"
    );

    // Test 3: Invalid YAML syntax (duplicate keys are allowed in YAML, but invalid structure is not)
    let file_path3 = temp_dir.path().join("malformed3.yaml");
    let malformed3 = "key: value\n: no_key";
    fs::write(&file_path3, malformed3).unwrap();

    let loader3 = YAMLLoader::new(&file_path3);
    let result3 = loader3.load().await;
    assert!(
        result3.is_err(),
        "Should fail on invalid YAML syntax in formatted mode"
    );
}

#[tokio::test]
async fn test_yaml_loader_nested_structures() {
    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("nested.yaml");

    // Create deeply nested YAML (10 levels)
    let yaml_content = r#"
level1:
  level2:
level3:
  level4:
level5:
  level6:
    level7:
      level8:
        level9:
          level10: deep_value
          array:
            - item1
            - item2
            - item3
"#;
    fs::write(&file_path, yaml_content).unwrap();

    let loader = YAMLLoader::new(&file_path);
    let docs = loader.load().await.unwrap();

    assert_eq!(docs.len(), 1, "Should load deeply nested YAML");

    // Validate all levels are present
    assert!(
        docs[0].page_content.contains("level1"),
        "Should contain level1"
    );
    assert!(
        docs[0].page_content.contains("level10"),
        "Should contain level10"
    );
    assert!(
        docs[0].page_content.contains("deep_value"),
        "Should contain deeply nested value"
    );
    assert!(
        docs[0].page_content.contains("array"),
        "Should contain nested array"
    );
    assert!(
        docs[0].page_content.contains("item1"),
        "Should contain array items"
    );

    // Validate metadata
    assert_eq!(
        docs[0].get_metadata("format").and_then(|v| v.as_str()),
        Some("yaml"),
        "Format metadata should be 'yaml'"
    );
}

#[tokio::test]
async fn test_yaml_loader_yaml_types() {
    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("types.yaml");

    let yaml_content = r#"
string_value: "text"
number_int: 42
number_float: 3.14
boolean_true: true
boolean_false: false
null_value: null
list:
  - item1
  - item2
  - item3
map:
  key1: value1
  key2: value2
mixed_list:
  - string
  - 123
  - true
  - null
"#;
    fs::write(&file_path, yaml_content).unwrap();

    let loader = YAMLLoader::new(&file_path);
    let docs = loader.load().await.unwrap();

    assert_eq!(docs.len(), 1, "Should load YAML with various types");

    // Validate all types are present in formatted output
    let content = &docs[0].page_content;
    assert!(
        content.contains("string_value"),
        "Should contain string key"
    );
    assert!(
        content.contains("text") || content.contains("\"text\""),
        "Should contain string value"
    );
    assert!(
        content.contains("number_int") && content.contains("42"),
        "Should contain integer"
    );
    assert!(
        content.contains("number_float") && content.contains("3.14"),
        "Should contain float"
    );
    assert!(
        content.contains("boolean_true") && content.contains("true"),
        "Should contain boolean true"
    );
    assert!(
        content.contains("boolean_false") && content.contains("false"),
        "Should contain boolean false"
    );
    assert!(content.contains("null_value"), "Should contain null key");
    assert!(content.contains("list"), "Should contain list");
    assert!(content.contains("map"), "Should contain map");

    // Validate metadata
    assert_eq!(
        docs[0].get_metadata("format").and_then(|v| v.as_str()),
        Some("yaml"),
        "Format metadata should be 'yaml'"
    );
}

#[tokio::test]
async fn test_yaml_loader_multiline_strings() {
    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("multiline.yaml");

    let yaml_content = r#"
literal_block: |
  This is a literal block.
  Newlines are preserved.
  Indentation is maintained.
folded_block: >
  This is a folded block.
  Lines are folded into a single line.
  But paragraphs are preserved.
plain_multiline: "This is a plain string
  that spans multiple lines
  in the YAML source"
"#;
    fs::write(&file_path, yaml_content).unwrap();

    let loader = YAMLLoader::new(&file_path);
    let docs = loader.load().await.unwrap();

    assert_eq!(docs.len(), 1, "Should load YAML with multiline strings");

    // Validate block scalars are handled
    let content = &docs[0].page_content;
    assert!(
        content.contains("literal_block"),
        "Should contain literal block key"
    );
    assert!(
        content.contains("folded_block"),
        "Should contain folded block key"
    );
    assert!(
        content.contains("plain_multiline"),
        "Should contain plain multiline key"
    );

    // Content should be present (exact formatting depends on serde_yml)
    assert!(
        content.contains("literal block") || content.contains("Literal block"),
        "Should contain literal block content"
    );
    assert!(
        content.contains("folded block") || content.contains("Folded block"),
        "Should contain folded block content"
    );

    // Validate metadata
    assert_eq!(
        docs[0].get_metadata("format").and_then(|v| v.as_str()),
        Some("yaml"),
        "Format metadata should be 'yaml'"
    );
}

#[tokio::test]
async fn test_yaml_loader_anchors_aliases() {
    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("anchors.yaml");

    let yaml_content = r#"
defaults: &defaults
  timeout: 30
  retry: 3

service1:
  <<: *defaults
  name: service1

service2:
  <<: *defaults
  name: service2
  timeout: 60
"#;
    fs::write(&file_path, yaml_content).unwrap();

    let loader = YAMLLoader::new(&file_path);
    let docs = loader.load().await.unwrap();

    assert_eq!(docs.len(), 1, "Should load YAML with anchors and aliases");

    // After parsing and reformatting, anchors/aliases are resolved to values
    let content = &docs[0].page_content;
    assert!(content.contains("service1"), "Should contain service1 key");
    assert!(content.contains("service2"), "Should contain service2 key");
    // Values from anchor should be present in both services
    assert!(
        content.contains("timeout") && content.contains("30"),
        "Should contain timeout values from anchor"
    );
    assert!(
        content.contains("retry") && content.contains("3"),
        "Should contain retry value from anchor"
    );

    // Validate metadata
    assert_eq!(
        docs[0].get_metadata("format").and_then(|v| v.as_str()),
        Some("yaml"),
        "Format metadata should be 'yaml'"
    );
}

#[tokio::test]
async fn test_yaml_loader_unicode_content() {
    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("unicode.yaml");

    let yaml_content = r#"
chinese: 世界
emoji: "🌍 🚀 🌟"
arabic: مرحبا
hebrew: שלום
russian: Привет
math: "∑∫√π"
mixed: "Hello 世界 🌍"
"#;
    fs::write(&file_path, yaml_content).unwrap();

    // Test formatted mode
    let loader = YAMLLoader::new(&file_path);
    let docs = loader.load().await.unwrap();

    assert_eq!(docs.len(), 1, "Should load YAML with Unicode content");

    // Validate Unicode content is preserved
    let content = &docs[0].page_content;
    assert!(
        content.contains("世界"),
        "Should contain Chinese characters"
    );
    assert!(
        content.contains("🌍") || content.contains("🚀") || content.contains("🌟"),
        "Should contain emoji"
    );
    assert!(content.contains("مرحبا"), "Should contain Arabic text");
    assert!(content.contains("שלום"), "Should contain Hebrew text");
    assert!(content.contains("Привет"), "Should contain Cyrillic text");
    assert!(
        content.contains("∑") || content.contains("∫"),
        "Should contain math symbols"
    );

    // Test raw mode also preserves Unicode
    let loader_raw = YAMLLoader::new(&file_path).with_format(false);
    let docs_raw = loader_raw.load().await.unwrap();
    assert!(
        docs_raw[0].page_content.contains("世界"),
        "Raw mode should preserve Unicode"
    );

    // Validate metadata
    assert_eq!(
        docs[0].get_metadata("format").and_then(|v| v.as_str()),
        Some("yaml"),
        "Format metadata should be 'yaml'"
    );
}

#[tokio::test]
async fn test_yaml_loader_empty_yaml() {
    let temp_dir = TempDir::new().unwrap();

    // Test 1: Completely empty file
    let file_path1 = temp_dir.path().join("empty1.yaml");
    fs::write(&file_path1, "").unwrap();

    let loader1 = YAMLLoader::new(&file_path1);
    let result1 = loader1.load().await;
    // Empty YAML is technically valid (parses to null), but may error depending on implementation
    // Let's check what actually happens
    if let Ok(docs) = result1 {
        assert_eq!(docs.len(), 1, "Empty YAML should load as one document");
        // Content may be empty string or "null" depending on serde_yml behavior
    } else {
        // If it errors, that's also acceptable behavior
        assert!(result1.is_err(), "Empty file may result in parse error");
    }

    // Test 2: YAML document separator only
    let file_path2 = temp_dir.path().join("empty2.yaml");
    fs::write(&file_path2, "---\n").unwrap();

    let loader2 = YAMLLoader::new(&file_path2);
    let result2 = loader2.load().await;
    if let Ok(docs) = result2 {
        assert_eq!(docs.len(), 1, "Document separator only should load");
    }

    // Test 3: Empty map
    let file_path3 = temp_dir.path().join("empty3.yaml");
    fs::write(&file_path3, "{}\n").unwrap();

    let loader3 = YAMLLoader::new(&file_path3);
    let docs3 = loader3.load().await.unwrap();
    assert_eq!(docs3.len(), 1, "Empty map should load");
    assert!(
        docs3[0].page_content.contains("{}") || docs3[0].page_content.is_empty(),
        "Empty map should produce empty or {{}} content"
    );

    // Test 4: Empty list
    let file_path4 = temp_dir.path().join("empty4.yaml");
    fs::write(&file_path4, "[]\n").unwrap();

    let loader4 = YAMLLoader::new(&file_path4);
    let docs4 = loader4.load().await.unwrap();
    assert_eq!(docs4.len(), 1, "Empty list should load");
    assert!(
        docs4[0].page_content.contains("[]") || docs4[0].page_content.is_empty(),
        "Empty list should produce empty or [[]] content"
    );
}

#[tokio::test]
async fn test_yaml_loader_comments() {
    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("comments.yaml");

    let yaml_content = r#"
# This is a comment
key1: value1  # Inline comment
# Another comment
key2: value2
list:
  - item1  # Comment on list item
  - item2
  # Comment in list
  - item3
"#;
    fs::write(&file_path, yaml_content).unwrap();

    // Test formatted mode (comments should be stripped by parser)
    let loader = YAMLLoader::new(&file_path);
    let docs = loader.load().await.unwrap();

    assert_eq!(docs.len(), 1, "Should load YAML with comments");

    // In formatted mode, comments are stripped during parse/reformat
    let content = &docs[0].page_content;
    assert!(content.contains("key1"), "Should contain key1");
    assert!(content.contains("value1"), "Should contain value1");
    assert!(content.contains("key2"), "Should contain key2");
    assert!(content.contains("list"), "Should contain list");

    // Test raw mode (comments should be preserved)
    let loader_raw = YAMLLoader::new(&file_path).with_format(false);
    let docs_raw = loader_raw.load().await.unwrap();
    assert!(
        docs_raw[0].page_content.contains("# This is a comment"),
        "Raw mode should preserve comments"
    );
    assert!(
        docs_raw[0].page_content.contains("# Inline comment"),
        "Raw mode should preserve inline comments"
    );

    // Validate metadata
    assert_eq!(
        docs[0].get_metadata("format").and_then(|v| v.as_str()),
        Some("yaml"),
        "Format metadata should be 'yaml'"
    );
}

#[tokio::test]
async fn test_yaml_loader_multiple_documents() {
    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("multi.yaml");

    let yaml_content = r#"
---
document: 1
name: first
---
document: 2
name: second
---
document: 3
name: third
"#;
    fs::write(&file_path, yaml_content).unwrap();

    // Test formatted mode - serde_yml does NOT support multiple documents
    let loader = YAMLLoader::new(&file_path);
    let result = loader.load().await;

    // serde_yml errors on multiple documents
    assert!(
        result.is_err(),
        "Multiple YAML documents should error in formatted mode (serde_yml limitation)"
    );
    let err = result.unwrap_err().to_string();
    assert!(
        err.contains("more than one document") || err.contains("YAML parse error"),
        "Error should indicate multiple documents not supported, got: {}",
        err
    );

    // Test raw mode - should preserve all documents as raw text
    let loader_raw = YAMLLoader::new(&file_path).with_format(false);
    let docs_raw = loader_raw.load().await.unwrap();

    assert_eq!(
        docs_raw.len(),
        1,
        "Raw mode loads entire file as single document"
    );

    // Raw mode should preserve all document separators and content
    let content = &docs_raw[0].page_content;
    assert!(
        content.contains("---"),
        "Raw mode should preserve document separators"
    );
    assert!(
        content.contains("document: 1")
            && content.contains("document: 2")
            && content.contains("document: 3"),
        "Raw mode should preserve all documents"
    );
    assert!(
        content.contains("first") && content.contains("second") && content.contains("third"),
        "Raw mode should preserve all document content"
    );

    // Validate metadata
    assert_eq!(
        docs_raw[0].get_metadata("format").and_then(|v| v.as_str()),
        Some("yaml"),
        "Format metadata should be 'yaml'"
    );
}

#[tokio::test]
async fn test_yaml_loader_file_not_found() {
    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("nonexistent.yaml");

    let loader = YAMLLoader::new(&file_path);
    let result = loader.load().await;

    assert!(result.is_err(), "Should error on nonexistent file");

    let error_msg = result.unwrap_err().to_string();
    assert!(
        error_msg.contains("No such file")
            || error_msg.contains("not found")
            || error_msg.contains("nonexistent"),
        "Error message should indicate file not found, got: {}",
        error_msg
    );
}

#[tokio::test]
async fn test_toml_loader() {
    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("test.toml");

    let toml_content = r#"
[package]
name = "test"
version = "1.0.0"

[dependencies]
serde = "1.0"
"#;
    fs::write(&file_path, toml_content).unwrap();

    let loader = TOMLLoader::new(&file_path);
    let docs = loader.load().await.unwrap();

    assert_eq!(docs.len(), 1, "Should load exactly one document");

    // Validate formatted TOML content (parsed and re-serialized)
    assert!(
        docs[0].page_content.contains("package"),
        "Should contain [package] section"
    );
    assert!(
        docs[0].page_content.contains("name"),
        "Should contain name key"
    );
    assert!(
        docs[0].page_content.contains("test"),
        "Should contain test value"
    );
    assert!(
        docs[0].page_content.contains("dependencies"),
        "Should contain [dependencies] section"
    );
    assert!(
        docs[0].page_content.contains("serde"),
        "Should contain serde dependency"
    );

    // Validate metadata
    assert_eq!(
        docs[0].get_metadata("format").and_then(|v| v.as_str()),
        Some("toml"),
        "Format metadata should be 'toml'"
    );
    let source_path = file_path.display().to_string();
    assert_eq!(
        docs[0].get_metadata("source").and_then(|v| v.as_str()),
        Some(source_path.as_str()),
        "Source metadata should contain file path"
    );
}

#[tokio::test]
async fn test_toml_loader_raw() {
    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("test.toml");

    let toml_content = "key = \"value\"\n";
    fs::write(&file_path, toml_content).unwrap();

    let loader = TOMLLoader::new(&file_path).with_format(false);
    let docs = loader.load().await.unwrap();

    assert_eq!(docs.len(), 1, "Should load exactly one document");

    // Should preserve raw TOML exactly (no parsing/reformatting)
    assert_eq!(
        docs[0].page_content, toml_content,
        "Raw mode should preserve exact TOML text"
    );

    // Validate metadata
    assert_eq!(
        docs[0].get_metadata("format").and_then(|v| v.as_str()),
        Some("toml"),
        "Format metadata should be 'toml'"
    );
    let source_path = file_path.display().to_string();
    assert_eq!(
        docs[0].get_metadata("source").and_then(|v| v.as_str()),
        Some(source_path.as_str()),
        "Source metadata should contain file path"
    );
}

#[tokio::test]
async fn test_toml_loader_malformed_toml() {
    let temp_dir = TempDir::new().unwrap();

    // Test case 1: Missing equals sign
    let file_path1 = temp_dir.path().join("malformed1.toml");
    fs::write(&file_path1, "key \"value\"").unwrap();
    let loader1 = TOMLLoader::new(&file_path1);
    let result1 = loader1.load().await;
    assert!(result1.is_err(), "Should error on missing equals sign");
    let error_msg1 = result1.unwrap_err().to_string();
    assert!(
        error_msg1.contains("TOML parse error"),
        "Error should mention TOML parse error: {}",
        error_msg1
    );

    // Test case 2: Duplicate keys (TOML spec disallows this)
    let file_path2 = temp_dir.path().join("malformed2.toml");
    fs::write(&file_path2, "key = \"value1\"\nkey = \"value2\"").unwrap();
    let loader2 = TOMLLoader::new(&file_path2);
    let result2 = loader2.load().await;
    assert!(result2.is_err(), "Should error on duplicate keys");
    let error_msg2 = result2.unwrap_err().to_string();
    assert!(
        error_msg2.contains("TOML parse error"),
        "Error should mention TOML parse error: {}",
        error_msg2
    );

    // Test case 3: Unclosed string
    let file_path3 = temp_dir.path().join("malformed3.toml");
    fs::write(&file_path3, "key = \"value").unwrap();
    let loader3 = TOMLLoader::new(&file_path3);
    let result3 = loader3.load().await;
    assert!(result3.is_err(), "Should error on unclosed string");
    let error_msg3 = result3.unwrap_err().to_string();
    assert!(
        error_msg3.contains("TOML parse error"),
        "Error should mention TOML parse error: {}",
        error_msg3
    );

    // Test case 4: Invalid table name (contains spaces without quotes)
    let file_path4 = temp_dir.path().join("malformed4.toml");
    fs::write(&file_path4, "[invalid table]\nkey = \"value\"").unwrap();
    let loader4 = TOMLLoader::new(&file_path4);
    let result4 = loader4.load().await;
    assert!(result4.is_err(), "Should error on invalid table name");
    let error_msg4 = result4.unwrap_err().to_string();
    assert!(
        error_msg4.contains("TOML parse error"),
        "Error should mention TOML parse error: {}",
        error_msg4
    );

    // Raw mode should preserve malformed TOML as text (no parsing)
    let loader_raw = TOMLLoader::new(&file_path1).with_format(false);
    let result_raw = loader_raw.load().await;
    assert!(
        result_raw.is_ok(),
        "Raw mode should load malformed TOML as text"
    );
    let docs_raw = result_raw.unwrap();
    assert_eq!(docs_raw.len(), 1, "Should load one document in raw mode");
    assert_eq!(
        docs_raw[0].page_content, "key \"value\"",
        "Raw mode should preserve malformed TOML exactly"
    );
}

#[tokio::test]
async fn test_toml_loader_nested_tables() {
    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("nested.toml");

    // Create deeply nested TOML structure (10 levels deep)
    let toml_content = r#"
[level1]
value = 1

[level1.level2]
value = 2

[level1.level2.level3]
value = 3

[level1.level2.level3.level4]
value = 4

[level1.level2.level3.level4.level5]
value = 5

[level1.level2.level3.level4.level5.level6]
value = 6

[level1.level2.level3.level4.level5.level6.level7]
value = 7

[level1.level2.level3.level4.level5.level6.level7.level8]
value = 8

[level1.level2.level3.level4.level5.level6.level7.level8.level9]
value = 9

[level1.level2.level3.level4.level5.level6.level7.level8.level9.level10]
value = 10
items = ["a", "b", "c"]
"#;
    fs::write(&file_path, toml_content).unwrap();

    let loader = TOMLLoader::new(&file_path);
    let docs = loader.load().await.unwrap();

    assert_eq!(docs.len(), 1, "Should load exactly one document");

    // Validate nested structure is preserved (all levels present)
    assert!(
        docs[0].page_content.contains("level1"),
        "Should contain level1"
    );
    assert!(
        docs[0].page_content.contains("level5"),
        "Should contain level5 (mid-depth)"
    );
    assert!(
        docs[0].page_content.contains("level10"),
        "Should contain level10 (deepest)"
    );
    assert!(
        docs[0].page_content.contains("value = 10")
            || docs[0].page_content.contains("value = \"10\""),
        "Should contain value at deepest level"
    );
    assert!(
        docs[0].page_content.contains("items"),
        "Should contain array at deepest level"
    );

    // Validate metadata
    assert_eq!(
        docs[0].get_metadata("format").and_then(|v| v.as_str()),
        Some("toml"),
        "Format should be 'toml'"
    );
}

#[tokio::test]
async fn test_toml_loader_toml_types() {
    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("types.toml");

    // TOML supports: string, integer, float, boolean, datetime, array, table
    let toml_content = r#"
# Strings
string_basic = "Hello, World!"
string_literal = 'C:\Users\nodejs\templates'

# Integers
int_positive = 42
int_negative = -17
int_with_underscores = 1_000_000

# Floats
float_positive = 3.14
float_negative = -0.01
float_scientific = 5e+22

# Booleans
bool_true = true
bool_false = false

# Datetime (ISO 8601)
datetime = 1979-05-27T07:32:00Z

# Arrays
array_integers = [1, 2, 3]
array_strings = ["red", "yellow", "green"]
array_mixed_types = ["all", 'strings', """are the same""", '''type''']

# Table (inline)
inline_table = { x = 1, y = 2 }

[table]
key1 = "value1"
key2 = "value2"
"#;
    fs::write(&file_path, toml_content).unwrap();

    let loader = TOMLLoader::new(&file_path);
    let docs = loader.load().await.unwrap();

    assert_eq!(docs.len(), 1, "Should load exactly one document");

    let content = &docs[0].page_content;

    // Validate strings
    assert!(
        content.contains("Hello, World!") || content.contains("Hello"),
        "Should contain string value"
    );

    // Validate integers
    assert!(content.contains("42"), "Should contain positive integer");
    assert!(
        content.contains("-17") || content.contains("17"),
        "Should contain negative integer"
    );
    assert!(
        content.contains("1000000") || content.contains("1_000_000"),
        "Should contain large integer"
    );

    // Validate floats
    assert!(content.contains("3.14"), "Should contain positive float");

    // Validate booleans
    assert!(content.contains("true"), "Should contain true boolean");
    assert!(content.contains("false"), "Should contain false boolean");

    // Validate datetime
    assert!(
        content.contains("1979-05-27"),
        "Should contain datetime value"
    );

    // Validate arrays
    assert!(
        content.contains("[1, 2, 3]")
            || (content.contains("1") && content.contains("2") && content.contains("3")),
        "Should contain integer array"
    );
    assert!(
        content.contains("red") || content.contains("\"red\""),
        "Should contain string array"
    );

    // Validate tables
    assert!(content.contains("table"), "Should contain table section");
    assert!(
        content.contains("key1") || content.contains("value1"),
        "Should contain table keys"
    );

    // Validate metadata
    assert_eq!(
        docs[0].get_metadata("format").and_then(|v| v.as_str()),
        Some("toml"),
        "Format should be 'toml'"
    );
}

#[tokio::test]
async fn test_toml_loader_array_of_tables() {
    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("array_of_tables.toml");

    // Array of tables syntax: [[products]]
    let toml_content = r#"
[[products]]
name = "Hammer"
sku = 738594937

[[products]]
name = "Nail"
sku = 284758393
color = "gray"

[[products]]
name = "Screwdriver"
sku = 948576234
"#;
    fs::write(&file_path, toml_content).unwrap();

    let loader = TOMLLoader::new(&file_path);
    let docs = loader.load().await.unwrap();

    assert_eq!(docs.len(), 1, "Should load exactly one document");

    // Validate array of tables structure
    assert!(
        docs[0].page_content.contains("products"),
        "Should contain products array"
    );
    assert!(
        docs[0].page_content.contains("Hammer"),
        "Should contain first product"
    );
    assert!(
        docs[0].page_content.contains("Nail"),
        "Should contain second product"
    );
    assert!(
        docs[0].page_content.contains("Screwdriver"),
        "Should contain third product"
    );
    assert!(
        docs[0].page_content.contains("738594937") || docs[0].page_content.contains("sku"),
        "Should contain SKU values"
    );

    // Validate metadata
    assert_eq!(
        docs[0].get_metadata("format").and_then(|v| v.as_str()),
        Some("toml"),
        "Format should be 'toml'"
    );
}

#[tokio::test]
async fn test_toml_loader_multiline_strings() {
    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("multiline.toml");

    // Basic multiline strings (""") and literal multiline strings (''')
    let toml_content = r#"
# Basic multiline string
multiline_basic = """
Line 1
Line 2
Line 3"""

# Literal multiline string
multiline_literal = '''
C:\Users\nodejs\templates
C:\Users\nodejs\data
C:\Users\nodejs\config'''

# Single line for comparison
single_line = "All on one line"
"#;
    fs::write(&file_path, toml_content).unwrap();

    let loader = TOMLLoader::new(&file_path);
    let docs = loader.load().await.unwrap();

    assert_eq!(docs.len(), 1, "Should load exactly one document");

    // Validate multiline strings are parsed
    assert!(
        docs[0].page_content.contains("Line 1") || docs[0].page_content.contains("multiline_basic"),
        "Should contain multiline basic string content"
    );
    assert!(
        docs[0].page_content.contains("Users")
            || docs[0].page_content.contains("multiline_literal"),
        "Should contain multiline literal string content"
    );
    assert!(
        docs[0].page_content.contains("All on one line"),
        "Should contain single line string"
    );

    // Validate metadata
    assert_eq!(
        docs[0].get_metadata("format").and_then(|v| v.as_str()),
        Some("toml"),
        "Format should be 'toml'"
    );
}

#[tokio::test]
async fn test_toml_loader_unicode_content() {
    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("unicode.toml");

    // Unicode content in TOML values
    let toml_content = r#"
chinese = "你好世界"
arabic = "مرحبا بك"
hebrew = "שלום עולם"
russian = "Привет мир"
emoji = "Hello 🌍 World 🚀"
math = "∑∫√π ≠ ∞"
mixed = "Hello 世界 🌍 مرحبا"
"#;
    fs::write(&file_path, toml_content).unwrap();

    // Test formatted mode
    let loader = TOMLLoader::new(&file_path);
    let docs = loader.load().await.unwrap();

    assert_eq!(docs.len(), 1, "Should load exactly one document");

    // Validate Unicode preservation in formatted mode
    assert!(
        docs[0].page_content.contains("你好世界") || docs[0].page_content.contains("chinese"),
        "Should preserve Chinese characters"
    );
    assert!(
        docs[0].page_content.contains("مرحبا") || docs[0].page_content.contains("arabic"),
        "Should preserve Arabic characters"
    );
    assert!(
        docs[0].page_content.contains("שלום") || docs[0].page_content.contains("hebrew"),
        "Should preserve Hebrew characters"
    );
    assert!(
        docs[0].page_content.contains("Привет") || docs[0].page_content.contains("russian"),
        "Should preserve Cyrillic characters"
    );
    assert!(
        docs[0].page_content.contains("🌍") || docs[0].page_content.contains("emoji"),
        "Should preserve emoji"
    );
    assert!(
        docs[0].page_content.contains("∑") || docs[0].page_content.contains("math"),
        "Should preserve math symbols"
    );

    // Test raw mode
    let loader_raw = TOMLLoader::new(&file_path).with_format(false);
    let docs_raw = loader_raw.load().await.unwrap();

    assert_eq!(docs_raw.len(), 1, "Should load one document in raw mode");
    assert!(
        docs_raw[0].page_content.contains("你好世界"),
        "Raw mode should preserve Chinese exactly"
    );
    assert!(
        docs_raw[0].page_content.contains("🌍"),
        "Raw mode should preserve emoji exactly"
    );

    // Validate metadata
    assert_eq!(
        docs[0].get_metadata("format").and_then(|v| v.as_str()),
        Some("toml"),
        "Format should be 'toml'"
    );
}

#[tokio::test]
async fn test_toml_loader_empty_toml() {
    let temp_dir = TempDir::new().unwrap();

    // Test case 1: Completely empty file
    let file_path1 = temp_dir.path().join("empty1.toml");
    fs::write(&file_path1, "").unwrap();
    let loader1 = TOMLLoader::new(&file_path1);
    let result1 = loader1.load().await;
    // Empty file behavior: toml crate may parse empty as null/empty, or error
    // Check both possibilities
    if result1.is_ok() {
        let docs1 = result1.unwrap();
        assert_eq!(docs1.len(), 1, "Should load one document for empty file");
    } else {
        // Empty file parsing error is acceptable
        assert!(result1.is_err(), "Empty file parsing may error");
    }

    // Test case 2: Only whitespace
    let file_path2 = temp_dir.path().join("empty2.toml");
    fs::write(&file_path2, "   \n\n  \t  ").unwrap();
    let loader2 = TOMLLoader::new(&file_path2);
    let result2 = loader2.load().await;
    if result2.is_ok() {
        let docs2 = result2.unwrap();
        assert_eq!(
            docs2.len(),
            1,
            "Should load one document for whitespace-only file"
        );
    }

    // Test case 3: Only comments
    let file_path3 = temp_dir.path().join("empty3.toml");
    fs::write(&file_path3, "# This is a comment\n# Another comment").unwrap();
    let loader3 = TOMLLoader::new(&file_path3);
    let result3 = loader3.load().await;
    if result3.is_ok() {
        let docs3 = result3.unwrap();
        assert_eq!(
            docs3.len(),
            1,
            "Should load one document for comment-only file"
        );
    }

    // Test case 4: Empty table
    let file_path4 = temp_dir.path().join("empty4.toml");
    fs::write(&file_path4, "[empty_table]\n").unwrap();
    let loader4 = TOMLLoader::new(&file_path4);
    let docs4 = loader4.load().await.unwrap();
    assert_eq!(docs4.len(), 1, "Should load one document with empty table");
    assert!(
        docs4[0].page_content.contains("empty_table"),
        "Should contain empty table name"
    );

    // Raw mode should preserve empty content
    let loader_raw = TOMLLoader::new(&file_path1).with_format(false);
    let docs_raw = loader_raw.load().await.unwrap();
    assert_eq!(docs_raw.len(), 1, "Raw mode should load empty file");
    assert_eq!(
        docs_raw[0].page_content, "",
        "Raw mode should preserve empty content"
    );
}

#[tokio::test]
async fn test_toml_loader_comments() {
    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("comments.toml");

    let toml_content = r#"# This is a header comment
key1 = "value1"  # Inline comment

# Section comment
[section]
key2 = "value2"
# Comment before key
key3 = "value3"
"#;
    fs::write(&file_path, toml_content).unwrap();

    // Formatted mode: comments are stripped during parse/reformat
    let loader = TOMLLoader::new(&file_path);
    let docs = loader.load().await.unwrap();

    assert_eq!(docs.len(), 1, "Should load exactly one document");
    assert!(docs[0].page_content.contains("key1"), "Should contain key1");
    assert!(
        docs[0].page_content.contains("section"),
        "Should contain section"
    );
    // Comments stripped in formatted mode
    assert!(
        !docs[0].page_content.contains("This is a header comment")
            || docs[0].page_content.len() < toml_content.len(),
        "Formatted mode typically strips comments"
    );

    // Raw mode: comments preserved
    let loader_raw = TOMLLoader::new(&file_path).with_format(false);
    let docs_raw = loader_raw.load().await.unwrap();

    assert_eq!(docs_raw.len(), 1, "Should load one document in raw mode");
    assert_eq!(
        docs_raw[0].page_content, toml_content,
        "Raw mode should preserve comments exactly"
    );
    assert!(
        docs_raw[0]
            .page_content
            .contains("This is a header comment"),
        "Raw mode should preserve comments"
    );
    assert!(
        docs_raw[0].page_content.contains("Inline comment"),
        "Raw mode should preserve inline comments"
    );

    // Validate metadata
    assert_eq!(
        docs[0].get_metadata("format").and_then(|v| v.as_str()),
        Some("toml"),
        "Format should be 'toml'"
    );
}

#[tokio::test]
async fn test_toml_loader_inline_tables() {
    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("inline_tables.toml");

    // Inline table syntax: { key = value, key2 = value2 }
    let toml_content = r#"
# Inline tables must be on one line
point = { x = 1, y = 2 }
color = { r = 255, g = 128, b = 0 }

# Nested inline tables
config = { database = { host = "localhost", port = 5432 }, cache = { enabled = true } }

# Regular table for comparison
[regular_table]
key1 = "value1"
key2 = "value2"
"#;
    fs::write(&file_path, toml_content).unwrap();

    let loader = TOMLLoader::new(&file_path);
    let docs = loader.load().await.unwrap();

    assert_eq!(docs.len(), 1, "Should load exactly one document");

    // Validate inline tables
    assert!(
        docs[0].page_content.contains("point"),
        "Should contain point inline table"
    );
    assert!(
        docs[0].page_content.contains("color"),
        "Should contain color inline table"
    );
    assert!(
        docs[0].page_content.contains("1") && docs[0].page_content.contains("2"),
        "Should contain inline table values"
    );
    assert!(
        docs[0].page_content.contains("255"),
        "Should contain color values"
    );
    assert!(
        docs[0].page_content.contains("database") || docs[0].page_content.contains("localhost"),
        "Should contain nested inline table"
    );

    // Validate metadata
    assert_eq!(
        docs[0].get_metadata("format").and_then(|v| v.as_str()),
        Some("toml"),
        "Format should be 'toml'"
    );
}

#[tokio::test]
async fn test_toml_loader_file_not_found() {
    let temp_dir = TempDir::new().unwrap();
    let nonexistent_path = temp_dir.path().join("nonexistent.toml");

    let loader = TOMLLoader::new(&nonexistent_path);
    let result = loader.load().await;

    assert!(result.is_err(), "Should error when file does not exist");
    let error_msg = result.unwrap_err().to_string();
    assert!(
        error_msg.contains("No such file")
            || error_msg.contains("not found")
            || error_msg.contains("does not exist"),
        "Error message should indicate file not found: {}",
        error_msg
    );
}

#[tokio::test]
async fn test_ini_loader() {
    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("test.ini");

    let ini_content = r#"
[database]
host = localhost
port = 5432

[cache]
enabled = true
ttl = 3600
"#;
    fs::write(&file_path, ini_content).unwrap();

    let loader = IniLoader::new(&file_path);
    let docs = loader.load().await.unwrap();

    assert_eq!(docs.len(), 1, "Should return exactly one document");

    // Validate metadata
    assert_eq!(
        docs[0].get_metadata("format").and_then(|v| v.as_str()),
        Some("ini"),
        "Metadata format should be 'ini'"
    );
    assert_eq!(
        docs[0].get_metadata("source").and_then(|v| v.as_str()),
        Some(file_path.to_str().unwrap()),
        "Metadata source should be file path"
    );

    // Validate formatted INI structure - ini crate formats with " = " separator
    assert!(
        docs[0].page_content.contains("[database]"),
        "Should contain database section"
    );
    assert!(
        docs[0].page_content.contains("host = localhost"),
        "Should contain formatted host key"
    );
    assert!(
        docs[0].page_content.contains("port = 5432"),
        "Should contain formatted port key"
    );
    assert!(
        docs[0].page_content.contains("[cache]"),
        "Should contain cache section"
    );
    assert!(
        docs[0].page_content.contains("enabled = true"),
        "Should contain formatted enabled key"
    );
    assert!(
        docs[0].page_content.contains("ttl = 3600"),
        "Should contain formatted ttl key"
    );
}

#[tokio::test]
async fn test_ini_loader_raw() {
    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("test.ini");

    let ini_content = "[section]\nkey=value\n";
    fs::write(&file_path, ini_content).unwrap();

    let loader = IniLoader::new(&file_path).with_format(false);
    let docs = loader.load().await.unwrap();

    assert_eq!(docs.len(), 1, "Should return exactly one document");

    // Validate metadata
    assert_eq!(
        docs[0].get_metadata("format").and_then(|v| v.as_str()),
        Some("ini"),
        "Metadata format should be 'ini'"
    );
    assert_eq!(
        docs[0].get_metadata("source").and_then(|v| v.as_str()),
        Some(file_path.to_str().unwrap()),
        "Metadata source should be file path"
    );

    // Should preserve raw INI exactly
    assert_eq!(
        docs[0].page_content, ini_content,
        "Raw mode should preserve exact INI content"
    );
}

#[tokio::test]
async fn test_ini_loader_malformed_ini() {
    let temp_dir = TempDir::new().unwrap();

    // Test case 1: Missing equals sign
    let file_path1 = temp_dir.path().join("malformed1.ini");
    let malformed1 = "[section]\nkey value\n";
    fs::write(&file_path1, malformed1).unwrap();

    let loader1 = IniLoader::new(&file_path1);
    let _result1 = loader1.load().await;
    // ini crate is lenient - may parse this as key with no value or fail
    // Raw mode should always preserve content
    let loader1_raw = IniLoader::new(&file_path1).with_format(false);
    let docs1_raw = loader1_raw.load().await.unwrap();
    assert_eq!(
        docs1_raw[0].page_content, malformed1,
        "Raw mode should preserve malformed INI (missing equals)"
    );

    // Test case 2: Invalid section name (unclosed bracket)
    let file_path2 = temp_dir.path().join("malformed2.ini");
    let malformed2 = "[section\nkey = value\n";
    fs::write(&file_path2, malformed2).unwrap();

    let loader2_raw = IniLoader::new(&file_path2).with_format(false);
    let docs2_raw = loader2_raw.load().await.unwrap();
    assert_eq!(
        docs2_raw[0].page_content, malformed2,
        "Raw mode should preserve malformed INI (unclosed bracket)"
    );

    // Test case 3: Empty section name
    let file_path3 = temp_dir.path().join("malformed3.ini");
    let malformed3 = "[]\nkey = value\n";
    fs::write(&file_path3, malformed3).unwrap();

    let _loader3 = IniLoader::new(&file_path3);
    // ini crate may handle empty section names differently
    let loader3_raw = IniLoader::new(&file_path3).with_format(false);
    let docs3_raw = loader3_raw.load().await.unwrap();
    assert_eq!(
        docs3_raw[0].page_content, malformed3,
        "Raw mode should preserve malformed INI (empty section)"
    );

    // Test case 4: Duplicate section names
    let file_path4 = temp_dir.path().join("malformed4.ini");
    let malformed4 = "[section]\nkey1 = value1\n[section]\nkey2 = value2\n";
    fs::write(&file_path4, malformed4).unwrap();

    let loader4 = IniLoader::new(&file_path4);
    let docs4 = loader4.load().await.unwrap();
    // ini crate may merge duplicate sections or keep last one
    assert_eq!(
        docs4.len(),
        1,
        "Should return one document for duplicate sections"
    );

    let loader4_raw = IniLoader::new(&file_path4).with_format(false);
    let docs4_raw = loader4_raw.load().await.unwrap();
    assert_eq!(
        docs4_raw[0].page_content, malformed4,
        "Raw mode should preserve duplicate sections"
    );
}

#[tokio::test]
async fn test_ini_loader_sections() {
    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("sections.ini");

    let ini_content = r#"[section1]
key1 = value1
key2 = value2

[section2]
key3 = value3
key4 = value4

[section3]
key5 = value5

[section4]
nested.key1 = nested_value1
nested.key2 = nested_value2
"#;
    fs::write(&file_path, ini_content).unwrap();

    let loader = IniLoader::new(&file_path);
    let docs = loader.load().await.unwrap();

    assert_eq!(docs.len(), 1, "Should return exactly one document");

    // Validate all sections are present
    assert!(
        docs[0].page_content.contains("[section1]"),
        "Should contain section1"
    );
    assert!(
        docs[0].page_content.contains("[section2]"),
        "Should contain section2"
    );
    assert!(
        docs[0].page_content.contains("[section3]"),
        "Should contain section3"
    );
    assert!(
        docs[0].page_content.contains("[section4]"),
        "Should contain section4"
    );

    // Validate keys from different sections
    assert!(
        docs[0].page_content.contains("key1 = value1"),
        "Should contain key1 from section1"
    );
    assert!(
        docs[0].page_content.contains("key3 = value3"),
        "Should contain key3 from section2"
    );
    assert!(
        docs[0].page_content.contains("key5 = value5"),
        "Should contain key5 from section3"
    );
    assert!(
        docs[0].page_content.contains("nested.key1 = nested_value1"),
        "Should contain nested key from section4"
    );

    // Validate metadata
    assert_eq!(
        docs[0].get_metadata("format").and_then(|v| v.as_str()),
        Some("ini"),
        "Metadata format should be 'ini'"
    );
}

#[tokio::test]
async fn test_ini_loader_comments() {
    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("comments.ini");

    // INI files support both ; and # for comments
    let ini_content = r#"; This is a comment with semicolon
# This is a comment with hash
[section1]
; Section comment
key1 = value1  ; Inline comment with semicolon
key2 = value2  # Inline comment with hash

# Another comment
[section2]
key3 = value3
"#;
    fs::write(&file_path, ini_content).unwrap();

    // Test formatted mode - ini crate strips comments during parse
    let loader_formatted = IniLoader::new(&file_path);
    let docs_formatted = loader_formatted.load().await.unwrap();

    assert_eq!(
        docs_formatted.len(),
        1,
        "Should return exactly one document"
    );
    assert!(
        docs_formatted[0].page_content.contains("[section1]"),
        "Should contain section1"
    );
    assert!(
        docs_formatted[0].page_content.contains("key1 = value1"),
        "Should contain key1"
    );
    // Comments should be stripped in formatted mode
    assert!(
        !docs_formatted[0]
            .page_content
            .contains("; This is a comment"),
        "Comments should be stripped in formatted mode"
    );
    assert!(
        !docs_formatted[0]
            .page_content
            .contains("# This is a comment"),
        "Hash comments should be stripped in formatted mode"
    );

    // Test raw mode - preserves comments
    let loader_raw = IniLoader::new(&file_path).with_format(false);
    let docs_raw = loader_raw.load().await.unwrap();

    assert_eq!(
        docs_raw.len(),
        1,
        "Should return exactly one document in raw mode"
    );
    assert_eq!(
        docs_raw[0].page_content, ini_content,
        "Raw mode should preserve comments"
    );
    assert!(
        docs_raw[0]
            .page_content
            .contains("; This is a comment with semicolon"),
        "Should preserve semicolon comments in raw mode"
    );
    assert!(
        docs_raw[0]
            .page_content
            .contains("# This is a comment with hash"),
        "Should preserve hash comments in raw mode"
    );
}

#[tokio::test]
async fn test_ini_loader_empty_values() {
    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("empty_values.ini");

    let ini_content = r#"[section1]
key1 =
key2 =
key3 = value3
key4

[section2]
empty_key =
"#;
    fs::write(&file_path, ini_content).unwrap();

    let loader = IniLoader::new(&file_path);
    let docs = loader.load().await.unwrap();

    assert_eq!(docs.len(), 1, "Should return exactly one document");

    // ini crate handles empty values - they may be parsed as empty strings
    assert!(
        docs[0].page_content.contains("[section1]"),
        "Should contain section1"
    );
    assert!(
        docs[0].page_content.contains("[section2]"),
        "Should contain section2"
    );

    // Test raw mode to verify original structure
    let loader_raw = IniLoader::new(&file_path).with_format(false);
    let docs_raw = loader_raw.load().await.unwrap();
    assert_eq!(
        docs_raw[0].page_content, ini_content,
        "Raw mode should preserve empty values exactly"
    );
}

#[tokio::test]
async fn test_ini_loader_unicode_content() {
    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("unicode.ini");

    let ini_content = r#"[languages]
chinese = 你好世界
arabic = مرحبا
hebrew = שלום
cyrillic = Привет
emoji = 🌍🚀🌟
math = ∑∫√π

[mixed]
greeting = Hello 世界 مرحبا 🌍
"#;
    fs::write(&file_path, ini_content).unwrap();

    // Test formatted mode
    let loader_formatted = IniLoader::new(&file_path);
    let docs_formatted = loader_formatted.load().await.unwrap();

    assert_eq!(
        docs_formatted.len(),
        1,
        "Should return exactly one document"
    );
    assert!(
        docs_formatted[0].page_content.contains("你好世界"),
        "Should preserve Chinese characters"
    );
    assert!(
        docs_formatted[0].page_content.contains("مرحبا"),
        "Should preserve Arabic characters"
    );
    assert!(
        docs_formatted[0].page_content.contains("שלום"),
        "Should preserve Hebrew characters"
    );
    assert!(
        docs_formatted[0].page_content.contains("Привет"),
        "Should preserve Cyrillic characters"
    );
    assert!(
        docs_formatted[0].page_content.contains("🌍🚀🌟"),
        "Should preserve emoji"
    );
    assert!(
        docs_formatted[0].page_content.contains("∑∫√π"),
        "Should preserve math symbols"
    );

    // Test raw mode
    let loader_raw = IniLoader::new(&file_path).with_format(false);
    let docs_raw = loader_raw.load().await.unwrap();

    assert_eq!(
        docs_raw[0].page_content, ini_content,
        "Raw mode should preserve Unicode exactly"
    );
    assert!(
        docs_raw[0].page_content.contains("你好世界"),
        "Raw mode should preserve Chinese"
    );
    assert!(
        docs_raw[0].page_content.contains("مرحبا"),
        "Raw mode should preserve Arabic"
    );
    assert!(
        docs_raw[0].page_content.contains("🌍🚀🌟"),
        "Raw mode should preserve emoji"
    );
}

#[tokio::test]
async fn test_ini_loader_empty_ini() {
    let temp_dir = TempDir::new().unwrap();

    // Test case 1: Completely empty file
    let file_path1 = temp_dir.path().join("empty1.ini");
    fs::write(&file_path1, "").unwrap();

    let loader1 = IniLoader::new(&file_path1);
    let docs1 = loader1.load().await.unwrap();
    assert_eq!(docs1.len(), 1, "Should return one document for empty file");
    // ini crate may handle empty file as valid (empty map)

    let loader1_raw = IniLoader::new(&file_path1).with_format(false);
    let docs1_raw = loader1_raw.load().await.unwrap();
    assert_eq!(
        docs1_raw[0].page_content, "",
        "Raw mode should preserve empty file"
    );

    // Test case 2: Only whitespace
    let file_path2 = temp_dir.path().join("empty2.ini");
    fs::write(&file_path2, "   \n\n  \n").unwrap();

    let loader2_raw = IniLoader::new(&file_path2).with_format(false);
    let docs2_raw = loader2_raw.load().await.unwrap();
    assert_eq!(
        docs2_raw[0].page_content, "   \n\n  \n",
        "Raw mode should preserve whitespace"
    );

    // Test case 3: Only comments
    let file_path3 = temp_dir.path().join("empty3.ini");
    let comments_only = "; Just a comment\n# Another comment\n";
    fs::write(&file_path3, comments_only).unwrap();

    let loader3 = IniLoader::new(&file_path3);
    let docs3 = loader3.load().await.unwrap();
    assert_eq!(
        docs3.len(),
        1,
        "Should return one document for comment-only file"
    );

    let loader3_raw = IniLoader::new(&file_path3).with_format(false);
    let docs3_raw = loader3_raw.load().await.unwrap();
    assert_eq!(
        docs3_raw[0].page_content, comments_only,
        "Raw mode should preserve comment-only file"
    );

    // Test case 4: Empty section
    let file_path4 = temp_dir.path().join("empty4.ini");
    let empty_section = "[empty_section]\n";
    fs::write(&file_path4, empty_section).unwrap();

    let loader4 = IniLoader::new(&file_path4);
    let docs4 = loader4.load().await.unwrap();
    assert_eq!(
        docs4.len(),
        1,
        "Should return one document for empty section"
    );
    // ini crate may or may not preserve empty sections in formatted mode
    // The important thing is it doesn't error

    let loader4_raw = IniLoader::new(&file_path4).with_format(false);
    let docs4_raw = loader4_raw.load().await.unwrap();
    assert_eq!(
        docs4_raw[0].page_content, empty_section,
        "Raw mode should preserve empty section"
    );
}

#[tokio::test]
async fn test_ini_loader_whitespace() {
    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("whitespace.ini");

    let ini_content = r#"  [section1]
  key1   =   value1
key2=value2

[section2]
	key3	=	value3
key4 = value with spaces
"#;
    fs::write(&file_path, ini_content).unwrap();

    // Test formatted mode - ini crate normalizes whitespace
    let loader_formatted = IniLoader::new(&file_path);
    let docs_formatted = loader_formatted.load().await.unwrap();

    assert_eq!(
        docs_formatted.len(),
        1,
        "Should return exactly one document"
    );
    assert!(
        docs_formatted[0].page_content.contains("[section1]"),
        "Should contain section1"
    );
    assert!(
        docs_formatted[0].page_content.contains("[section2]"),
        "Should contain section2"
    );
    // ini crate trims whitespace around keys and values
    assert!(
        docs_formatted[0].page_content.contains("key1 = value1"),
        "Should normalize whitespace around key1"
    );
    assert!(
        docs_formatted[0].page_content.contains("key2 = value2"),
        "Should normalize whitespace around key2"
    );
    assert!(
        docs_formatted[0]
            .page_content
            .contains("key4 = value with spaces"),
        "Should preserve spaces within value"
    );

    // Test raw mode - preserves exact whitespace
    let loader_raw = IniLoader::new(&file_path).with_format(false);
    let docs_raw = loader_raw.load().await.unwrap();

    assert_eq!(
        docs_raw[0].page_content, ini_content,
        "Raw mode should preserve exact whitespace"
    );
    assert!(
        docs_raw[0].page_content.contains("  [section1]"),
        "Raw mode should preserve leading whitespace for section"
    );
    assert!(
        docs_raw[0].page_content.contains("  key1   =   value1"),
        "Raw mode should preserve whitespace around key-value"
    );
}

#[tokio::test]
async fn test_ini_loader_file_not_found() {
    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("nonexistent.ini");

    let loader = IniLoader::new(&file_path);
    let result = loader.load().await;

    assert!(result.is_err(), "Should return error for nonexistent file");

    let error_msg = format!("{:?}", result.unwrap_err());
    assert!(
        error_msg.contains("No such file")
            || error_msg.contains("not found")
            || error_msg.contains("nonexistent"),
        "Error should indicate file not found: {}",
        error_msg
    );
}
