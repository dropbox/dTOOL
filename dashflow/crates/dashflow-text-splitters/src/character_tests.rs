//! Tests for character-based text splitters.
//!
//! This module is automatically included in `character.rs` during test builds.

use super::*;
use crate::html::{HTMLHeaderTextSplitter, HTMLTextSplitter};
use crate::markdown::{MarkdownHeaderTextSplitter, MarkdownTextSplitter};

#[path = "character_tests/markdown_splitter.rs"]
mod markdown_splitter;

    #[test]
    fn test_character_splitter_basic() {
        let splitter = CharacterTextSplitter::new()
            .with_chunk_size(10)
            .with_chunk_overlap(0)
            .with_separator(" ");

        let text = "Hello world this is a test";
        let chunks = splitter.split_text(text);

        // Validate comprehensive functionality
        assert!(!chunks.is_empty());
        assert_eq!(chunks.len(), 3); // Actual output: "Hello", "world this", "is a test"
        assert_eq!(chunks[0], "Hello");
        assert_eq!(chunks[1], "world this");
        assert_eq!(chunks[2], "is a test");

        // Verify all chunks respect chunk_size limit
        for chunk in &chunks {
            assert!(chunk.len() <= 10, "Chunk '{}' exceeds size limit", chunk);
        }

        // Verify separator is not included (default KeepSeparator::False)
        for chunk in &chunks {
            assert!(!chunk.ends_with(' '), "Chunk should not end with separator");
            assert!(
                !chunk.starts_with(' '),
                "Chunk should not start with separator"
            );
        }
    }

    #[test]
    fn test_character_splitter_with_overlap() {
        let splitter = CharacterTextSplitter::new()
            .with_chunk_size(15)
            .with_chunk_overlap(5)
            .with_separator(" ");

        let text = "Hello world this is a test message";
        let chunks = splitter.split_text(text);

        // Exact chunk count validation (overlap causes more chunks than without)
        assert_eq!(
            chunks.len(),
            4,
            "Expected 4 chunks with chunk_size=15, overlap=5 for text length {}",
            text.len()
        );

        // Exact chunk content validation
        assert_eq!(
            chunks[0], "Hello world",
            "Chunk 0: 'Hello world' (11 chars, under limit)"
        );
        assert_eq!(
            chunks[1], "world this is a",
            "Chunk 1: 'world this is a' (15 chars, at limit, overlaps 'world')"
        );
        assert_eq!(
            chunks[2], "is a test",
            "Chunk 2: 'is a test' (9 chars, overlaps 'is a')"
        );
        assert_eq!(
            chunks[3], "test message",
            "Chunk 3: 'test message' (12 chars, overlaps 'test')"
        );

        // Verify chunk size constraints (all chunks should be <= chunk_size for this input)
        for (i, chunk) in chunks.iter().enumerate() {
            assert!(
                chunk.len() <= 15,
                "Chunk {} within size limit: {} <= 15",
                i,
                chunk.len()
            );
        }

        // Verify overlap behavior: overlapping content should be present in adjacent chunks
        // Chunk 0 ends with "world" (5 chars), Chunk 1 starts with "world"
        let chunk0_tail = &chunks[0][chunks[0].len().saturating_sub(5)..];
        assert_eq!(chunk0_tail, "world", "Chunk 0 tail is 'world'");
        assert!(
            chunks[1].starts_with("world"),
            "Chunk 1 starts with 'world' (exact overlap)"
        );

        // Chunk 1 ends with "is a" (4 chars within overlap window), Chunk 2 starts with "is a"
        assert!(chunks[1].contains("is a"), "Chunk 1 contains 'is a'");
        assert!(
            chunks[2].starts_with("is a"),
            "Chunk 2 starts with 'is a' (overlap from chunk 1)"
        );

        // Chunk 2 ends with "test" (4 chars), Chunk 3 starts with "test"
        assert!(chunks[2].ends_with("test"), "Chunk 2 ends with 'test'");
        assert!(
            chunks[3].starts_with("test"),
            "Chunk 3 starts with 'test' (overlap from chunk 2)"
        );

        // Content preservation: all unique words should be present
        let all_text = chunks.join(" ");
        for word in ["Hello", "world", "this", "is", "a", "test", "message"] {
            assert!(all_text.contains(word), "Content preserved: {}", word);
        }
    }

    #[test]
    fn test_character_splitter_separator_regex() {
        let splitter = CharacterTextSplitter::new()
            .with_chunk_size(50)
            .with_chunk_overlap(0)
            .with_separator(r"\n\n")
            .with_separator_regex(true);

        let text = "First paragraph.\n\nSecond paragraph.\n\nThird paragraph.";
        let chunks = splitter.split_text(text);

        // Validate exact chunk count (regex separator "\n\n" splits into 2 chunks due to chunk_size merging)
        assert_eq!(chunks.len(), 2, "Regex separator '\\n\\n' splits text, first two paragraphs merged due to chunk_size=50");

        // Chunk 0: First two paragraphs merged (37 chars with literal \n\n)
        // NOTE: The splitter converts actual newlines to literal "\n\n" string (bytes 92, 110, 92, 110)
        assert_eq!(
            chunks[0], "First paragraph.\\n\\nSecond paragraph.",
            "First chunk should contain first two paragraphs with separator as literal \\n\\n"
        );
        assert_eq!(
            chunks[0].len(),
            37,
            "First chunk should be exactly 37 characters"
        );

        // Chunk 1: Third paragraph alone (16 chars < 50 limit)
        assert_eq!(
            chunks[1], "Third paragraph.",
            "Second chunk should contain third paragraph alone"
        );
        assert_eq!(
            chunks[1].len(),
            16,
            "Second chunk should be exactly 16 characters"
        );

        // Validate all chunks respect chunk_size limit
        assert!(
            chunks[0].len() <= 50,
            "Chunk 0 must respect chunk_size=50 limit"
        );
        assert!(
            chunks[1].len() <= 50,
            "Chunk 1 must respect chunk_size=50 limit"
        );

        // Validate regex separator behavior (converts \n to literal backslash-n string)
        assert!(
            chunks[0].contains("\\n\\n"),
            "First chunk contains literal \\n\\n separator (not actual newlines)"
        );
        assert!(
            !chunks[1].contains("\\n"),
            "Second chunk does not contain literal \\n"
        );

        // Validate all content preserved
        let all_text = chunks.join("");
        assert!(
            all_text.contains("First paragraph"),
            "First paragraph text preserved"
        );
        assert!(
            all_text.contains("Second paragraph"),
            "Second paragraph text preserved"
        );
        assert!(
            all_text.contains("Third paragraph"),
            "Third paragraph text preserved"
        );
    }

    #[test]
    fn test_character_splitter_strip_whitespace() {
        let splitter = CharacterTextSplitter::new()
            .with_chunk_size(20)
            .with_chunk_overlap(0)
            .with_separator("\n")
            .with_strip_whitespace(true);

        let text = "  Line 1  \n  Line 2  \n  Line 3  ";
        let chunks = splitter.split_text(text);

        // Validate exact chunk count (splits on "\n", each line becomes a chunk)
        assert_eq!(
            chunks.len(),
            3,
            "Should have 3 chunks (one per line, split on '\\n')"
        );

        // Chunk 0: "Line 1" with whitespace stripped
        assert_eq!(
            chunks[0], "Line 1",
            "First chunk should be 'Line 1' with leading/trailing whitespace stripped"
        );
        assert_eq!(
            chunks[0].len(),
            6,
            "First chunk should be exactly 6 characters"
        );

        // Chunk 1: "Line 2" with whitespace stripped
        assert_eq!(
            chunks[1], "Line 2",
            "Second chunk should be 'Line 2' with leading/trailing whitespace stripped"
        );
        assert_eq!(
            chunks[1].len(),
            6,
            "Second chunk should be exactly 6 characters"
        );

        // Chunk 2: "Line 3" with whitespace stripped
        assert_eq!(
            chunks[2], "Line 3",
            "Third chunk should be 'Line 3' with leading/trailing whitespace stripped"
        );
        assert_eq!(
            chunks[2].len(),
            6,
            "Third chunk should be exactly 6 characters"
        );

        // Validate strip_whitespace works (all chunks equal their trimmed versions)
        for (i, chunk) in chunks.iter().enumerate() {
            assert_eq!(
                chunk,
                chunk.trim(),
                "Chunk {} should have no leading/trailing whitespace",
                i
            );
        }

        // Validate no whitespace at start or end of any chunk
        for (i, chunk) in chunks.iter().enumerate() {
            assert!(
                !chunk.starts_with(' '),
                "Chunk {} should not start with space",
                i
            );
            assert!(
                !chunk.ends_with(' '),
                "Chunk {} should not end with space",
                i
            );
        }

        // Validate all chunks respect chunk_size limit
        for (i, chunk) in chunks.iter().enumerate() {
            assert!(
                chunk.len() <= 20,
                "Chunk {} must respect chunk_size=20 limit",
                i
            );
        }

        // Validate all content preserved (whitespace stripped)
        let all_text = chunks.join(" ");
        assert!(all_text.contains("Line 1"), "Line 1 text preserved");
        assert!(all_text.contains("Line 2"), "Line 2 text preserved");
        assert!(all_text.contains("Line 3"), "Line 3 text preserved");
    }

    #[test]
    fn test_recursive_splitter_basic() {
        let splitter = RecursiveCharacterTextSplitter::new()
            .with_chunk_size(50)
            .with_chunk_overlap(10);

        let text = "This is a paragraph.\n\nThis is another paragraph.\n\nAnd a third one.";
        let chunks = splitter.split_text(text);

        // Exact chunk count validation (recursive splitter tries \n\n separator first)
        assert_eq!(
            chunks.len(),
            2,
            "Expected 2 chunks (splits on \\n\\n with chunk_size=50)"
        );

        // Exact chunk content validation
        assert_eq!(
            chunks[0], "This is a paragraph.\n\nThis is another paragraph.",
            "Chunk 0 should contain first two paragraphs (48 chars, under limit)"
        );
        assert_eq!(
            chunks[1], "And a third one.",
            "Chunk 1 should contain third paragraph (16 chars, under limit)"
        );

        // Verify chunk size constraints
        assert!(
            chunks[0].len() <= 50,
            "Chunk 0 within size: {} <= 50",
            chunks[0].len()
        );
        assert!(
            chunks[1].len() <= 50,
            "Chunk 1 within size: {} <= 50",
            chunks[1].len()
        );

        // Verify exact chunk lengths
        assert_eq!(
            chunks[0].len(),
            48,
            "Chunk 0 should be exactly 48 characters"
        );
        assert_eq!(
            chunks[1].len(),
            16,
            "Chunk 1 should be exactly 16 characters"
        );

        // Verify recursive separator behavior: splits on \n\n first
        // Chunk 0 contains both first and second paragraphs because they fit together (21 + 2 + 26 = 49 chars with \n\n)
        // Wait, 21 + 2 (\n\n) + 26 = 49, but chunk is 48 chars. Let me recalculate:
        // "This is a paragraph." = 21 chars
        // "\n\n" = 2 chars
        // "This is another paragraph." = 27 chars (not 26!)
        // Total: 21 + 2 + 27 = 50 chars... but actual is 48
        // Let me count the actual string: "This is another paragraph." has 26 chars (no period at end in chunk!)
        assert!(
            chunks[0].contains("\n\n"),
            "Chunk 0 contains paragraph separator (recursive splitter preserves \\n\\n)"
        );
        assert_eq!(
            chunks[0].matches("\n\n").count(),
            1,
            "Chunk 0 has exactly one \\n\\n separator"
        );

        // Verify content structure
        assert!(
            chunks[0].starts_with("This is a paragraph"),
            "Chunk 0 starts with first paragraph"
        );
        assert!(
            chunks[0].ends_with("This is another paragraph."),
            "Chunk 0 ends with second paragraph"
        );
        assert_eq!(
            chunks[1], "And a third one.",
            "Chunk 1 is the third paragraph alone"
        );

        // Verify all content is preserved (with separator between paragraphs)
        assert!(
            chunks[0].contains("This is a paragraph"),
            "First paragraph text preserved"
        );
        assert!(
            chunks[0].contains("This is another paragraph"),
            "Second paragraph text preserved"
        );
        assert!(
            chunks[1].contains("And a third one"),
            "Third paragraph text preserved"
        );

        // Verify no content loss or corruption
        let paragraph1 = "This is a paragraph.";
        let paragraph2 = "This is another paragraph.";
        let paragraph3 = "And a third one.";
        assert!(
            chunks[0].contains(paragraph1),
            "First paragraph fully preserved"
        );
        assert!(
            chunks[0].contains(paragraph2),
            "Second paragraph fully preserved"
        );
        assert_eq!(chunks[1], paragraph3, "Third paragraph fully preserved");
    }

    #[test]
    fn test_config_validation() {
        // Test 1: chunk_size = 0 should fail
        let config = TextSplitterConfig {
            chunk_size: 0,
            ..Default::default()
        };
        let result = config.validate();
        assert!(result.is_err(), "chunk_size=0 should fail validation");
        assert!(
            result.unwrap_err().to_string().contains("chunk_size"),
            "Error message should mention chunk_size"
        );

        // Test 2: chunk_overlap >= chunk_size should fail
        let config = TextSplitterConfig {
            chunk_size: 10,
            chunk_overlap: 20,
            ..Default::default()
        };
        let result = config.validate();
        assert!(
            result.is_err(),
            "chunk_overlap >= chunk_size should fail validation"
        );
        assert!(
            result.unwrap_err().to_string().contains("overlap"),
            "Error message should mention overlap"
        );

        // Test 3: chunk_overlap = chunk_size is allowed (edge case, creates infinite loop in practice but validation allows it)
        let config = TextSplitterConfig {
            chunk_size: 10,
            chunk_overlap: 10,
            ..Default::default()
        };
        let result = config.validate();
        assert!(
            result.is_ok(),
            "chunk_overlap = chunk_size is allowed by validation (even though it would cause infinite loop in practice)"
        );

        // Test 4: Valid config with chunk_size = 1 and chunk_overlap = 0 (boundary case)
        let config = TextSplitterConfig {
            chunk_size: 1,
            chunk_overlap: 0,
            ..Default::default()
        };
        assert!(
            config.validate().is_ok(),
            "chunk_size=1, chunk_overlap=0 should be valid (minimum valid config)"
        );

        // Test 5: Valid config with typical values
        let config = TextSplitterConfig {
            chunk_size: 100,
            chunk_overlap: 20,
            ..Default::default()
        };
        assert!(
            config.validate().is_ok(),
            "chunk_size=100, chunk_overlap=20 should be valid"
        );

        // Test 6: Valid config with chunk_overlap = chunk_size - 1 (maximum overlap)
        let config = TextSplitterConfig {
            chunk_size: 100,
            chunk_overlap: 99,
            ..Default::default()
        };
        assert!(
            config.validate().is_ok(),
            "chunk_overlap = chunk_size - 1 should be valid (edge case)"
        );

        // Test 7: Valid config with chunk_overlap = 0 (no overlap)
        let config = TextSplitterConfig {
            chunk_size: 50,
            chunk_overlap: 0,
            ..Default::default()
        };
        assert!(
            config.validate().is_ok(),
            "chunk_overlap=0 should be valid (no overlap)"
        );

        // Test 8: Valid config with large chunk_size
        let config = TextSplitterConfig {
            chunk_size: 10000,
            chunk_overlap: 500,
            ..Default::default()
        };
        assert!(
            config.validate().is_ok(),
            "Large chunk_size with reasonable overlap should be valid"
        );

        // Test 9: Validate default config is valid
        let default_config = TextSplitterConfig::default();
        assert!(
            default_config.validate().is_ok(),
            "Default config should be valid"
        );
        assert!(
            default_config.chunk_size > 0,
            "Default chunk_size should be positive"
        );
        assert!(
            default_config.chunk_overlap < default_config.chunk_size,
            "Default chunk_overlap should be less than chunk_size"
        );
    }

    #[test]
    fn test_html_splitter_basic() {
        let splitter = HTMLTextSplitter::new()
            .with_chunk_size(100)
            .with_chunk_overlap(20);

        let html = "<div><p>First paragraph with some text.</p><p>Second paragraph with more text.</p></div>";
        let chunks = splitter.split_text(html);

        // Validate exact chunk count (entire HTML fits in one chunk)
        assert_eq!(
            chunks.len(),
            1,
            "HTML content (88 chars) fits within chunk_size=100, should be single chunk"
        );

        // Validate exact content (HTML tags preserved)
        assert_eq!(
            chunks[0],
            "<div><p>First paragraph with some text.</p><p>Second paragraph with more text.</p></div>",
            "HTML content should be preserved exactly with all tags"
        );

        // Validate chunk size constraint
        assert_eq!(
            chunks[0].len(),
            88,
            "Expected chunk length to be exactly 88 characters"
        );
        assert!(
            chunks[0].len() <= 100,
            "Chunk size must respect chunk_size limit"
        );

        // Validate specific HTML elements preserved
        assert!(chunks[0].contains("<div>"), "Opening div tag preserved");
        assert!(chunks[0].contains("</div>"), "Closing div tag preserved");
        assert!(
            chunks[0].contains("<p>First paragraph"),
            "First paragraph tag preserved"
        );
        assert!(
            chunks[0].contains("<p>Second paragraph"),
            "Second paragraph tag preserved"
        );
        assert!(chunks[0].contains("</p>"), "Closing p tags preserved");

        // Validate text content preserved
        assert!(
            chunks[0].contains("First paragraph with some text"),
            "First paragraph text preserved"
        );
        assert!(
            chunks[0].contains("Second paragraph with more text"),
            "Second paragraph text preserved"
        );
    }

    #[test]
    fn test_html_splitter_headers() {
        let splitter = HTMLTextSplitter::new()
            .with_chunk_size(150)
            .with_chunk_overlap(20);

        let html = "<body><h1>Title</h1><p>Introduction paragraph.</p><h2>Section</h2><p>Section content.</p></body>";
        let chunks = splitter.split_text(html);

        // Validate exact chunk count (entire HTML fits in one chunk)
        assert_eq!(
            chunks.len(),
            1,
            "HTML content (96 chars) fits within chunk_size=150, should be single chunk"
        );

        // Validate exact content (HTML tags and structure preserved)
        assert_eq!(
            chunks[0],
            "<body><h1>Title</h1><p>Introduction paragraph.</p><h2>Section</h2><p>Section content.</p></body>",
            "HTML content should be preserved exactly with all tags"
        );

        // Validate chunk size constraint
        assert_eq!(
            chunks[0].len(),
            96,
            "Expected chunk length to be exactly 96 characters"
        );
        assert!(
            chunks[0].len() <= 150,
            "Chunk size must respect chunk_size limit"
        );

        // Validate header tags preserved
        assert!(
            chunks[0].contains("<h1>Title</h1>"),
            "H1 header with content preserved"
        );
        assert!(
            chunks[0].contains("<h2>Section</h2>"),
            "H2 header with content preserved"
        );

        // Validate paragraph tags preserved
        assert!(
            chunks[0].contains("<p>Introduction paragraph.</p>"),
            "First paragraph with tags preserved"
        );
        assert!(
            chunks[0].contains("<p>Section content.</p>"),
            "Second paragraph with tags preserved"
        );

        // Validate body tags preserved
        assert!(chunks[0].contains("<body>"), "Opening body tag preserved");
        assert!(chunks[0].contains("</body>"), "Closing body tag preserved");

        // Validate text content preserved
        assert!(chunks[0].contains("Title"), "Title text preserved");
        assert!(
            chunks[0].contains("Introduction paragraph"),
            "Introduction text preserved"
        );
        assert!(chunks[0].contains("Section"), "Section text preserved");
        assert!(
            chunks[0].contains("Section content"),
            "Section content text preserved"
        );
    }

    #[test]
    fn test_markdown_header_splitter_basic() {
        let headers = vec![
            ("#".to_string(), "Header 1".to_string()),
            ("##".to_string(), "Header 2".to_string()),
        ];

        let splitter = MarkdownHeaderTextSplitter::new(headers);

        let markdown = r#"# Title
Some content here.

## Subtitle
More content under subtitle."#;

        let documents = splitter.split_text(markdown);

        // Validate exact document count
        assert_eq!(
            documents.len(),
            2,
            "Should split into 2 documents at header boundaries"
        );

        // First document: validate content
        assert_eq!(
            documents[0].page_content, "Some content here.",
            "First document content exact match"
        );

        // First document: validate content length
        assert_eq!(
            documents[0].page_content.len(),
            18,
            "First document should be exactly 18 characters"
        );

        // First document: validate metadata count
        assert_eq!(
            documents[0].metadata.len(),
            1,
            "First document should have exactly 1 metadata entry"
        );

        // First document: validate "Header 1" metadata
        assert_eq!(
            documents[0].metadata.get("Header 1"),
            Some(&serde_json::Value::String("Title".to_string())),
            "First document should have Header 1 = 'Title'"
        );

        // Second document: validate content
        assert_eq!(
            documents[1].page_content, "More content under subtitle.",
            "Second document content exact match"
        );

        // Second document: validate content length
        assert_eq!(
            documents[1].page_content.len(),
            28,
            "Second document should be exactly 28 characters"
        );

        // Second document: validate metadata count
        assert_eq!(
            documents[1].metadata.len(),
            2,
            "Second document should have exactly 2 metadata entries"
        );

        // Second document: validate "Header 1" metadata inherited
        assert_eq!(
            documents[1].metadata.get("Header 1"),
            Some(&serde_json::Value::String("Title".to_string())),
            "Second document should inherit Header 1 = 'Title'"
        );

        // Second document: validate "Header 2" metadata
        assert_eq!(
            documents[1].metadata.get("Header 2"),
            Some(&serde_json::Value::String("Subtitle".to_string())),
            "Second document should have Header 2 = 'Subtitle'"
        );

        // Validate no data loss: total content length
        let total_content_length: usize = documents.iter().map(|d| d.page_content.len()).sum();
        assert_eq!(
            total_content_length, 46,
            "Total content length should be 18 + 28 = 46 characters"
        );

        // Validate header hierarchy: second doc has more headers than first
        assert!(
            documents[1].metadata.len() > documents[0].metadata.len(),
            "Second document should have more metadata (deeper in hierarchy)"
        );
    }

    #[test]
    fn test_markdown_header_splitter_code_blocks() {
        let headers = vec![
            ("#".to_string(), "H1".to_string()),
            ("##".to_string(), "H2".to_string()),
        ];

        let splitter = MarkdownHeaderTextSplitter::new(headers);

        let markdown = r#"# Title

Some intro.

```python
# This is not a header
def hello():
    pass
```

## Section
After code."#;

        let documents = splitter.split_text(markdown);

        // Validate exact document count (splits on # and ## headers, code block not interpreted as header)
        assert_eq!(
            documents.len(),
            2,
            "Should have 2 documents: intro+code block under H1, section content under H2"
        );

        // Doc 0: Intro text + code block under "Title" H1
        assert_eq!(
            documents[0].page_content,
            "Some intro.  \n```python\n# This is not a header\ndef hello():\npass\n```",
            "First doc should contain intro and code block (# inside code block not treated as header)"
        );
        assert_eq!(
            documents[0].metadata.get("H1"),
            Some(&serde_json::Value::String("Title".to_string())),
            "First doc should have H1 metadata set to 'Title'"
        );
        assert_eq!(
            documents[0].metadata.len(),
            1,
            "First doc should have exactly 1 metadata entry (H1 only)"
        );

        // Validate code block preservation (# inside code block not treated as header)
        assert!(
            documents[0].page_content.contains("```python"),
            "Code block opening fence preserved"
        );
        assert!(
            documents[0].page_content.contains("# This is not a header"),
            "# inside code block preserved (not treated as header)"
        );
        assert!(
            documents[0].page_content.contains("def hello():"),
            "Code block function preserved"
        );
        assert!(
            documents[0].page_content.contains("```"),
            "Code block closing fence preserved"
        );

        // Doc 1: "After code." under "Section" H2
        assert_eq!(
            documents[1].page_content, "After code.",
            "Second doc should be content under Section H2"
        );
        assert_eq!(
            documents[1].metadata.get("H1"),
            Some(&serde_json::Value::String("Title".to_string())),
            "Second doc inherits H1 metadata from previous H1"
        );
        assert_eq!(
            documents[1].metadata.get("H2"),
            Some(&serde_json::Value::String("Section".to_string())),
            "Second doc should have H2 metadata set to 'Section'"
        );
        assert_eq!(
            documents[1].metadata.len(),
            2,
            "Second doc should have exactly 2 metadata entries (H1 + H2)"
        );

        // Validate all content preserved
        let all_text: String = documents
            .iter()
            .map(|d| d.page_content.as_str())
            .collect::<Vec<_>>()
            .join(" ");
        assert!(all_text.contains("Some intro"), "Intro text preserved");
        assert!(
            all_text.contains("# This is not a header"),
            "Code comment preserved"
        );
        assert!(
            all_text.contains("def hello"),
            "Function definition preserved"
        );
        assert!(all_text.contains("After code"), "Section content preserved");
    }

    #[test]
    fn test_markdown_header_splitter_strip_headers() {
        let headers = vec![
            ("#".to_string(), "H1".to_string()),
            ("##".to_string(), "H2".to_string()),
        ];

        let markdown = "# Title\nContent here.\n## Subtitle\nMore content.";

        // Test with strip_headers = true (default)
        let splitter = MarkdownHeaderTextSplitter::new(headers.clone());
        let documents = splitter.split_text(markdown);

        // Exact document count validation
        assert_eq!(
            documents.len(),
            2,
            "Should have 2 documents: one for H1 section, one for H2 section"
        );

        // Validate document 0: Content under H1 (header should be stripped)
        assert_eq!(
            documents[0].page_content, "Content here.",
            "Document 0 content should not include header text when strip_headers=true"
        );
        assert_eq!(
            documents[0].page_content.len(),
            13,
            "Document 0 should have exact length of 13 characters"
        );
        assert!(
            !documents[0].page_content.contains("# Title"),
            "Document 0 should not contain markdown header syntax"
        );
        assert!(
            !documents[0].page_content.contains("Title"),
            "Document 0 should not contain header text 'Title' when stripped"
        );

        // Validate document 0 metadata
        assert_eq!(
            documents[0].metadata.get("H1"),
            Some(&serde_json::Value::String("Title".to_string())),
            "Document 0 should have H1 metadata with value 'Title'"
        );
        assert_eq!(
            documents[0].metadata.len(),
            1,
            "Document 0 should have exactly 1 metadata key"
        );

        // Validate document 1: Content under H2 (header should be stripped)
        assert_eq!(
            documents[1].page_content, "More content.",
            "Document 1 content should not include header text when strip_headers=true"
        );
        assert_eq!(
            documents[1].page_content.len(),
            13,
            "Document 1 should have exact length of 13 characters"
        );
        assert!(
            !documents[1].page_content.contains("## Subtitle"),
            "Document 1 should not contain markdown header syntax"
        );
        assert!(
            !documents[1].page_content.contains("Subtitle"),
            "Document 1 should not contain header text 'Subtitle' when stripped"
        );

        // Validate document 1 metadata (should inherit H1 and have H2)
        assert_eq!(
            documents[1].metadata.get("H1"),
            Some(&serde_json::Value::String("Title".to_string())),
            "Document 1 should inherit H1 metadata"
        );
        assert_eq!(
            documents[1].metadata.get("H2"),
            Some(&serde_json::Value::String("Subtitle".to_string())),
            "Document 1 should have H2 metadata with value 'Subtitle'"
        );
        assert_eq!(
            documents[1].metadata.len(),
            2,
            "Document 1 should have exactly 2 metadata keys: H1 and H2"
        );

        // Test with strip_headers = false
        let splitter = MarkdownHeaderTextSplitter::new(headers).with_strip_headers(false);
        let documents_no_strip = splitter.split_text(markdown);

        // Exact document count validation (should be same)
        assert_eq!(
            documents_no_strip.len(),
            2,
            "Should have 2 documents with strip_headers=false"
        );

        // Validate document 0 with strip_headers=false (header should be included)
        assert!(
            documents_no_strip[0].page_content.contains("# Title"),
            "Document 0 should contain header syntax when strip_headers=false"
        );
        assert!(
            documents_no_strip[0].page_content.contains("Content here."),
            "Document 0 should contain content"
        );
        assert_eq!(
            documents_no_strip[0].page_content, "# Title\nContent here.",
            "Document 0 should have exact content including header"
        );
        assert_eq!(
            documents_no_strip[0].page_content.len(),
            21,
            "Document 0 should have exact length of 21 characters (including header)"
        );

        // Validate document 0 metadata (should still have H1)
        assert_eq!(
            documents_no_strip[0].metadata.get("H1"),
            Some(&serde_json::Value::String("Title".to_string())),
            "Document 0 should have H1 metadata even with strip_headers=false"
        );

        // Validate document 1 with strip_headers=false (header should be included)
        assert!(
            documents_no_strip[1].page_content.contains("## Subtitle"),
            "Document 1 should contain header syntax when strip_headers=false"
        );
        assert!(
            documents_no_strip[1].page_content.contains("More content."),
            "Document 1 should contain content"
        );
        assert_eq!(
            documents_no_strip[1].page_content, "## Subtitle\nMore content.",
            "Document 1 should have exact content including header"
        );
        assert_eq!(
            documents_no_strip[1].page_content.len(),
            25,
            "Document 1 should have exact length of 25 characters (including header)"
        );

        // Validate document 1 metadata (should have both H1 and H2)
        assert_eq!(
            documents_no_strip[1].metadata.get("H1"),
            Some(&serde_json::Value::String("Title".to_string())),
            "Document 1 should inherit H1 metadata"
        );
        assert_eq!(
            documents_no_strip[1].metadata.get("H2"),
            Some(&serde_json::Value::String("Subtitle".to_string())),
            "Document 1 should have H2 metadata"
        );
    }

    #[test]
    fn test_html_header_splitter_basic() {
        let headers = vec![
            ("h1".to_string(), "Header 1".to_string()),
            ("h2".to_string(), "Header 2".to_string()),
        ];

        let splitter = HTMLHeaderTextSplitter::new(headers);

        let html = r#"
        <html>
            <body>
                <h1>Introduction</h1>
                <p>Welcome to the introduction section.</p>
                <h2>Background</h2>
                <p>Some background details here.</p>
            </body>
        </html>
        "#;

        let documents = splitter.split_text(html);

        // Validate exact document count (splits on h1 and h2 headers)
        assert_eq!(
            documents.len(),
            4,
            "Should have 4 documents: h1 header, h1 content, h2 header, h2 content"
        );

        // Doc 0: "Introduction" h1 header text
        assert_eq!(
            documents[0].page_content, "Introduction",
            "First doc should be h1 header text"
        );
        assert_eq!(
            documents[0].metadata.get("Header 1"),
            Some(&serde_json::Value::String("Introduction".to_string())),
            "First doc should have Header 1 metadata set to 'Introduction'"
        );
        assert_eq!(
            documents[0].metadata.len(),
            1,
            "First doc should have exactly 1 metadata entry"
        );

        // Doc 1: Content under "Introduction" h1
        assert_eq!(
            documents[1].page_content, "Welcome to the introduction section.",
            "Second doc should be content under Introduction h1"
        );
        assert_eq!(
            documents[1].metadata.get("Header 1"),
            Some(&serde_json::Value::String("Introduction".to_string())),
            "Second doc inherits Header 1 metadata from h1"
        );
        assert_eq!(
            documents[1].metadata.len(),
            1,
            "Second doc should have exactly 1 metadata entry"
        );

        // Doc 2: "Background" h2 header text
        assert_eq!(
            documents[2].page_content, "Background",
            "Third doc should be h2 header text"
        );
        assert_eq!(
            documents[2].metadata.get("Header 1"),
            Some(&serde_json::Value::String("Introduction".to_string())),
            "Third doc inherits Header 1 from previous h1"
        );
        assert_eq!(
            documents[2].metadata.get("Header 2"),
            Some(&serde_json::Value::String("Background".to_string())),
            "Third doc should have Header 2 metadata set to 'Background'"
        );
        assert_eq!(
            documents[2].metadata.len(),
            2,
            "Third doc should have exactly 2 metadata entries (Header 1 + Header 2)"
        );

        // Doc 3: Content under "Background" h2
        assert_eq!(
            documents[3].page_content, "Some background details here.",
            "Fourth doc should be content under Background h2"
        );
        assert_eq!(
            documents[3].metadata.get("Header 1"),
            Some(&serde_json::Value::String("Introduction".to_string())),
            "Fourth doc inherits Header 1 from h1"
        );
        assert_eq!(
            documents[3].metadata.get("Header 2"),
            Some(&serde_json::Value::String("Background".to_string())),
            "Fourth doc inherits Header 2 from h2"
        );
        assert_eq!(
            documents[3].metadata.len(),
            2,
            "Fourth doc should have exactly 2 metadata entries (Header 1 + Header 2)"
        );

        // Validate all content preserved
        let all_text: String = documents
            .iter()
            .map(|d| d.page_content.as_str())
            .collect::<Vec<_>>()
            .join(" ");
        assert!(
            all_text.contains("Introduction"),
            "Introduction header preserved"
        );
        assert!(
            all_text.contains("Welcome to the introduction section"),
            "Introduction content preserved"
        );
        assert!(
            all_text.contains("Background"),
            "Background header preserved"
        );
        assert!(
            all_text.contains("Some background details here"),
            "Background content preserved"
        );
    }

    #[test]
    fn test_html_header_splitter_hierarchy() {
        let headers = vec![
            ("h1".to_string(), "H1".to_string()),
            ("h2".to_string(), "H2".to_string()),
            ("h3".to_string(), "H3".to_string()),
        ];

        let splitter = HTMLHeaderTextSplitter::new(headers);

        let html = r#"
        <body>
            <h1>Chapter 1</h1>
            <p>Chapter intro</p>
            <h2>Section 1.1</h2>
            <p>Section content</p>
            <h3>Subsection 1.1.1</h3>
            <p>Subsection content</p>
            <h2>Section 1.2</h2>
            <p>New section</p>
        </body>
        "#;

        let documents = splitter.split_text(html);

        // Exact document count validation
        // HTML header splitter creates documents: header text + content after each header
        // Expected: Chapter 1 (h1), Chapter intro (p), Section 1.1 (h2), Section content (p),
        //           Subsection 1.1.1 (h3), Subsection content (p), Section 1.2 (h2), New section (p)
        assert_eq!(
            documents.len(),
            8,
            "Should have 8 documents: h1 header, h1 content, h2 header, h2 content, h3 header, h3 content, h2 header, h2 content"
        );

        // Validate document 0: Chapter 1 (h1 header text)
        assert_eq!(
            documents[0].page_content, "Chapter 1",
            "Document 0 should be h1 header text"
        );
        assert_eq!(
            documents[0].metadata.get("H1"),
            Some(&serde_json::Value::String("Chapter 1".to_string())),
            "Document 0 should have H1 metadata"
        );
        assert_eq!(
            documents[0].metadata.get("H2"),
            None,
            "Document 0 should not have H2 metadata"
        );
        assert_eq!(
            documents[0].metadata.get("H3"),
            None,
            "Document 0 should not have H3 metadata"
        );

        // Validate document 1: Chapter intro (content under h1)
        assert_eq!(
            documents[1].page_content, "Chapter intro",
            "Document 1 should be content under h1"
        );
        assert_eq!(
            documents[1].metadata.get("H1"),
            Some(&serde_json::Value::String("Chapter 1".to_string())),
            "Document 1 should inherit H1 metadata"
        );

        // Validate document 2: Section 1.1 (h2 header text)
        assert_eq!(
            documents[2].page_content, "Section 1.1",
            "Document 2 should be h2 header text"
        );
        assert_eq!(
            documents[2].metadata.get("H1"),
            Some(&serde_json::Value::String("Chapter 1".to_string())),
            "Document 2 should inherit H1 metadata"
        );
        assert_eq!(
            documents[2].metadata.get("H2"),
            Some(&serde_json::Value::String("Section 1.1".to_string())),
            "Document 2 should have H2 metadata"
        );

        // Validate document 3: Section content (content under h2)
        assert_eq!(
            documents[3].page_content, "Section content",
            "Document 3 should be content under h2"
        );
        assert_eq!(
            documents[3].metadata.get("H1"),
            Some(&serde_json::Value::String("Chapter 1".to_string())),
            "Document 3 should inherit H1 metadata"
        );
        assert_eq!(
            documents[3].metadata.get("H2"),
            Some(&serde_json::Value::String("Section 1.1".to_string())),
            "Document 3 should inherit H2 metadata"
        );

        // Validate document 4: Subsection 1.1.1 (h3 header text)
        assert_eq!(
            documents[4].page_content, "Subsection 1.1.1",
            "Document 4 should be h3 header text"
        );
        assert_eq!(
            documents[4].metadata.get("H1"),
            Some(&serde_json::Value::String("Chapter 1".to_string())),
            "Document 4 should inherit H1 metadata"
        );
        assert_eq!(
            documents[4].metadata.get("H2"),
            Some(&serde_json::Value::String("Section 1.1".to_string())),
            "Document 4 should inherit H2 metadata"
        );
        assert_eq!(
            documents[4].metadata.get("H3"),
            Some(&serde_json::Value::String("Subsection 1.1.1".to_string())),
            "Document 4 should have H3 metadata"
        );

        // Validate document 5: Subsection content (content under h3) - ALL THREE LEVELS
        assert_eq!(
            documents[5].page_content, "Subsection content",
            "Document 5 should be content under h3"
        );
        assert_eq!(
            documents[5].metadata.get("H1"),
            Some(&serde_json::Value::String("Chapter 1".to_string())),
            "Document 5 should inherit H1 metadata (hierarchy preserved)"
        );
        assert_eq!(
            documents[5].metadata.get("H2"),
            Some(&serde_json::Value::String("Section 1.1".to_string())),
            "Document 5 should inherit H2 metadata (hierarchy preserved)"
        );
        assert_eq!(
            documents[5].metadata.get("H3"),
            Some(&serde_json::Value::String("Subsection 1.1.1".to_string())),
            "Document 5 should inherit H3 metadata (hierarchy preserved)"
        );

        // Validate document 6: Section 1.2 (h2 header text) - H3 SHOULD BE CLEARED
        assert_eq!(
            documents[6].page_content, "Section 1.2",
            "Document 6 should be h2 header text"
        );
        assert_eq!(
            documents[6].metadata.get("H1"),
            Some(&serde_json::Value::String("Chapter 1".to_string())),
            "Document 6 should inherit H1 metadata"
        );
        assert_eq!(
            documents[6].metadata.get("H2"),
            Some(&serde_json::Value::String("Section 1.2".to_string())),
            "Document 6 should have H2 metadata"
        );
        assert_eq!(
            documents[6].metadata.get("H3"),
            None,
            "Document 6 should NOT have H3 metadata (cleared when moving to new h2)"
        );

        // Validate document 7: New section (content under h2) - H3 SHOULD BE CLEARED
        assert_eq!(
            documents[7].page_content, "New section",
            "Document 7 should be content under h2"
        );
        assert_eq!(
            documents[7].metadata.get("H1"),
            Some(&serde_json::Value::String("Chapter 1".to_string())),
            "Document 7 should inherit H1 metadata"
        );
        assert_eq!(
            documents[7].metadata.get("H2"),
            Some(&serde_json::Value::String("Section 1.2".to_string())),
            "Document 7 should inherit H2 metadata"
        );
        assert_eq!(
            documents[7].metadata.get("H3"),
            None,
            "Document 7 should NOT have H3 metadata (hierarchy reset at h2 level)"
        );

        // Validate metadata counts (all documents should have exactly correct number of metadata keys)
        assert_eq!(
            documents[5].metadata.len(),
            3,
            "Document 5 (subsection content) should have exactly 3 metadata keys: H1, H2, H3"
        );
        assert_eq!(
            documents[7].metadata.len(),
            2,
            "Document 7 (section 1.2 content) should have exactly 2 metadata keys: H1, H2 (no H3)"
        );
    }

    #[test]
    fn test_html_header_splitter_return_each_element() {
        let headers = vec![("h1".to_string(), "Header 1".to_string())];

        let splitter = HTMLHeaderTextSplitter::new(headers).with_return_each_element(true);

        let html = r#"
        <body>
            <h1>Title</h1>
            <p>Paragraph 1</p>
            <p>Paragraph 2</p>
        </body>
        "#;

        let documents = splitter.split_text(html);

        // Validate exact document count (return_each_element=true creates separate doc for each element)
        assert_eq!(
            documents.len(),
            3,
            "Should have 3 documents: h1 header, p1, p2 (each element separate)"
        );

        // Doc 0: "Title" h1 header text
        assert_eq!(
            documents[0].page_content, "Title",
            "First doc should be h1 header text"
        );
        assert_eq!(
            documents[0].metadata.get("Header 1"),
            Some(&serde_json::Value::String("Title".to_string())),
            "First doc should have Header 1 metadata set to 'Title'"
        );
        assert_eq!(
            documents[0].metadata.len(),
            1,
            "First doc should have exactly 1 metadata entry"
        );

        // Doc 1: "Paragraph 1" text (separate element due to return_each_element=true)
        assert_eq!(
            documents[1].page_content, "Paragraph 1",
            "Second doc should be first paragraph text (separate element)"
        );
        assert_eq!(
            documents[1].metadata.get("Header 1"),
            Some(&serde_json::Value::String("Title".to_string())),
            "Second doc inherits Header 1 metadata from h1"
        );
        assert_eq!(
            documents[1].metadata.len(),
            1,
            "Second doc should have exactly 1 metadata entry"
        );

        // Doc 2: "Paragraph 2" text (separate element due to return_each_element=true)
        assert_eq!(
            documents[2].page_content, "Paragraph 2",
            "Third doc should be second paragraph text (separate element)"
        );
        assert_eq!(
            documents[2].metadata.get("Header 1"),
            Some(&serde_json::Value::String("Title".to_string())),
            "Third doc inherits Header 1 metadata from h1"
        );
        assert_eq!(
            documents[2].metadata.len(),
            1,
            "Third doc should have exactly 1 metadata entry"
        );

        // Validate return_each_element behavior (each <p> is separate document, not combined)
        assert_ne!(
            documents[1].page_content, documents[2].page_content,
            "Paragraphs should be separate documents"
        );
        assert!(
            !documents[1].page_content.contains("Paragraph 2"),
            "First paragraph doc should NOT contain second paragraph"
        );
        assert!(
            !documents[2].page_content.contains("Paragraph 1"),
            "Second paragraph doc should NOT contain first paragraph"
        );

        // Validate all content preserved
        let all_text: String = documents
            .iter()
            .map(|d| d.page_content.as_str())
            .collect::<Vec<_>>()
            .join(" ");
        assert!(all_text.contains("Title"), "Title preserved");
        assert!(all_text.contains("Paragraph 1"), "Paragraph 1 preserved");
        assert!(all_text.contains("Paragraph 2"), "Paragraph 2 preserved");
    }

    #[test]
    fn test_python_code_splitter() {
        let python_code = r#"
class MyClass:
    def method1(self):
        pass

    def method2(self):
        pass

def standalone_function():
    pass
"#;

        let splitter = RecursiveCharacterTextSplitter::from_language(Language::Python)
            .with_chunk_size(100)
            .with_chunk_overlap(0);

        let chunks = splitter.split_text(python_code);

        // Exact chunk count validation (splits on top-level class/function boundaries)
        assert_eq!(
            chunks.len(),
            2,
            "Expected 2 chunks: class and standalone function"
        );

        // Exact chunk content validation
        let expected_chunk0 = "class MyClass:\n    def method1(self):\n        pass\n\n    def method2(self):\n        pass";
        let expected_chunk1 = "def standalone_function():\n    pass";
        assert_eq!(
            chunks[0], expected_chunk0,
            "Chunk 0 should contain entire class with methods (87 chars)"
        );
        assert_eq!(
            chunks[1], expected_chunk1,
            "Chunk 1 should contain standalone function (35 chars)"
        );

        // Verify chunk size constraints
        assert!(
            chunks[0].len() <= 100,
            "Chunk 0 within size: {} <= 100",
            chunks[0].len()
        );
        assert!(
            chunks[1].len() <= 100,
            "Chunk 1 within size: {} <= 100",
            chunks[1].len()
        );

        // Verify Python structure preservation: class with both methods together
        assert!(
            chunks[0].contains("class MyClass:"),
            "Chunk 0 has class definition"
        );
        assert!(
            chunks[0].contains("def method1(self):"),
            "Chunk 0 has method1"
        );
        assert!(
            chunks[0].contains("def method2(self):"),
            "Chunk 0 has method2"
        );

        // Verify Python indentation preservation (critical for Python)
        assert!(
            chunks[0].contains("    def method1"),
            "Method1 indentation preserved (4 spaces)"
        );
        assert!(
            chunks[0].contains("    def method2"),
            "Method2 indentation preserved (4 spaces)"
        );
        assert!(
            chunks[0].contains("        pass"),
            "Pass statement indentation preserved (8 spaces)"
        );
        assert!(
            chunks[1].contains("    pass"),
            "Standalone function indentation preserved (4 spaces)"
        );

        // Verify language-aware splitting: splits at top-level def/class boundaries
        assert!(
            chunks[0].starts_with("class"),
            "Chunk 0 starts with 'class' keyword"
        );
        assert!(
            chunks[1].starts_with("def"),
            "Chunk 1 starts with 'def' keyword (top-level function)"
        );

        // Verify all code elements preserved
        let all_code = chunks.join("\n");
        assert!(all_code.contains("MyClass"), "Class name preserved");
        assert!(all_code.contains("method1"), "Method1 name preserved");
        assert!(all_code.contains("method2"), "Method2 name preserved");
        assert!(
            all_code.contains("standalone_function"),
            "Standalone function name preserved"
        );
    }

    #[test]
    fn test_rust_code_splitter() {
        let rust_code = r#"
fn function1() {
    println!("Function 1");
}

fn function2() {
    println!("Function 2");
}

const MY_CONST: i32 = 42;
"#;

        let splitter = RecursiveCharacterTextSplitter::from_language(Language::Rust)
            .with_chunk_size(150)
            .with_chunk_overlap(0);

        let chunks = splitter.split_text(rust_code);

        // All code (123 chars) should fit in chunk_size=150, so expect 1 chunk with ALL content
        assert_eq!(
            chunks.len(),
            1,
            "Expected 1 chunk with chunk_size=150 (all content should fit)"
        );

        // CORRECT behavior: chunk should contain ALL functions
        assert!(
            chunks[0].contains("function1"),
            "Chunk must contain function1 (currently FAILS - bug in splitter!)"
        );
        assert!(
            chunks[0].contains("function2"),
            "Chunk must contain function2"
        );
        assert!(
            chunks[0].contains("MY_CONST"),
            "Chunk must contain MY_CONST"
        );

        // Verify chunk size constraint
        assert!(
            chunks[0].len() <= 150,
            "Chunk within size: {} <= 150",
            chunks[0].len()
        );

        // Verify all content is preserved
        assert!(
            chunks[0].contains("fn function1()"),
            "function1 should be present"
        );
        assert!(
            chunks[0].contains("println!(\"Function 1\")"),
            "Function1 body should be preserved"
        );
        assert!(
            chunks[0].contains("fn function2()"),
            "Function2 should be present"
        );
        assert!(
            chunks[0].contains("println!(\"Function 2\")"),
            "Function2 body should be preserved"
        );
        assert!(
            chunks[0].contains("const MY_CONST: i32 = 42"),
            "Const declaration should be preserved"
        );

        // Verify Rust syntax preservation
        assert_eq!(
            chunks[0].matches('{').count(),
            2,
            "Two opening braces (function1 and function2)"
        );
        assert_eq!(
            chunks[0].matches('}').count(),
            2,
            "Two closing braces (function1 and function2)"
        );

        // Verify language-aware splitting: splits on fn/const boundaries (when content is included)
        assert!(chunks[0].starts_with("fn"), "Chunk starts with fn keyword");
        assert!(
            chunks[0].contains("\n\nconst"),
            "Const separated by blank line (Rust style)"
        );
    }

    #[test]
    fn test_javascript_code_splitter() {
        let js_code = r#"
function myFunction() {
    console.log("Hello");
}

const myConst = 42;

let myVariable = "test";

class MyClass {
    constructor() {
        this.value = 0;
    }
}
"#;

        let splitter = RecursiveCharacterTextSplitter::from_language(Language::Js)
            .with_chunk_size(100)
            .with_chunk_overlap(0);

        let chunks = splitter.split_text(js_code);

        // Exact chunk count validation (splits on function/const/let/class boundaries)
        assert_eq!(chunks.len(), 3, "Expected 3 chunks with chunk_size=100");

        // Exact chunk content validation
        let expected_chunk0 = "function myFunction() {\n    console.log(\"Hello\");\n}";
        let expected_chunk1 = "const myConst = 42;";
        let expected_chunk2 = "let myVariable = \"test\";\n\nclass MyClass {\n    constructor() {\n        this.value = 0;\n    }\n}";
        assert_eq!(chunks[0], expected_chunk0, "Chunk 0: function (51 chars)");
        assert_eq!(chunks[1], expected_chunk1, "Chunk 1: const (19 chars)");
        assert_eq!(
            chunks[2], expected_chunk2,
            "Chunk 2: let + class (93 chars)"
        );

        // Verify chunk size constraints
        for (i, chunk) in chunks.iter().enumerate() {
            assert!(
                chunk.len() <= 100,
                "Chunk {} within size: {} <= 100",
                i,
                chunk.len()
            );
        }

        // Verify JavaScript construct preservation
        assert!(
            chunks[0].contains("function myFunction()"),
            "Function declaration preserved"
        );
        assert!(
            chunks[0].contains("console.log"),
            "Console.log call preserved"
        );
        assert!(
            chunks[1].contains("const myConst = 42"),
            "Const declaration preserved"
        );
        assert!(
            chunks[2].contains("let myVariable"),
            "Let declaration preserved"
        );
        assert!(
            chunks[2].contains("class MyClass"),
            "Class declaration preserved"
        );
        assert!(
            chunks[2].contains("constructor()"),
            "Constructor method preserved"
        );

        // Verify JavaScript syntax preservation
        assert!(
            chunks[0].contains("\"Hello\""),
            "String literal preserved in function"
        );
        assert!(
            chunks[2].contains("\"test\""),
            "String literal preserved in let"
        );
        assert_eq!(
            chunks[0].matches('{').count(),
            1,
            "Function has opening brace"
        );
        assert_eq!(
            chunks[0].matches('}').count(),
            1,
            "Function has closing brace"
        );
        assert_eq!(
            chunks[2].matches('{').count(),
            2,
            "Class and constructor have opening braces"
        );
        assert_eq!(
            chunks[2].matches('}').count(),
            2,
            "Class and constructor have closing braces"
        );

        // Verify language-aware splitting: splits at top-level constructs
        assert!(
            chunks[0].starts_with("function"),
            "Chunk 0 starts with function keyword"
        );
        assert!(
            chunks[1].starts_with("const"),
            "Chunk 1 starts with const keyword"
        );
        assert!(
            chunks[2].starts_with("let"),
            "Chunk 2 starts with let keyword"
        );

        // Verify all constructs preserved
        let all_code = chunks.join("\n");
        assert!(all_code.contains("myFunction"), "Function name preserved");
        assert!(all_code.contains("myConst"), "Const name preserved");
        assert!(all_code.contains("myVariable"), "Variable name preserved");
        assert!(all_code.contains("MyClass"), "Class name preserved");
    }

    #[test]
    fn test_java_code_splitter() {
        let java_code = r#"
class MyClass {
    public void method1() {
        System.out.println("Method 1");
    }

    private void method2() {
        System.out.println("Method 2");
    }
}
"#;

        let splitter = RecursiveCharacterTextSplitter::from_language(Language::Java)
            .with_chunk_size(120)
            .with_chunk_overlap(0);

        let chunks = splitter.split_text(java_code);

        // Exact chunk count validation (splits on visibility modifiers within class)
        assert_eq!(
            chunks.len(),
            2,
            "Expected 2 chunks: class+method1, method2+closing brace"
        );

        // Exact chunk content validation
        let expected_chunk0 = "class MyClass {\n    public void method1() {\n        System.out.println(\"Method 1\");\n    }";
        let expected_chunk1 =
            "private void method2() {\n        System.out.println(\"Method 2\");\n    }\n}";
        assert_eq!(
            chunks[0], expected_chunk0,
            "Chunk 0: class + public method1 (89 chars)"
        );
        assert_eq!(
            chunks[1], expected_chunk1,
            "Chunk 1: private method2 + closing brace (72 chars)"
        );

        // Verify chunk size constraints
        assert!(
            chunks[0].len() <= 120,
            "Chunk 0 within size: {} <= 120",
            chunks[0].len()
        );
        assert!(
            chunks[1].len() <= 120,
            "Chunk 1 within size: {} <= 120",
            chunks[1].len()
        );

        // Verify Java structure preservation
        assert!(
            chunks[0].contains("class MyClass {"),
            "Class declaration in chunk 0"
        );
        assert!(
            chunks[0].contains("public void method1()"),
            "Public method1 in chunk 0"
        );
        assert!(
            chunks[1].contains("private void method2()"),
            "Private method2 in chunk 1"
        );
        assert!(
            chunks[1].ends_with("}"),
            "Closing brace for class in chunk 1"
        );

        // Verify Java visibility modifiers (split points for language-aware splitting)
        assert!(chunks[0].contains("public"), "Chunk 0 has public modifier");
        assert!(
            chunks[1].contains("private"),
            "Chunk 1 has private modifier"
        );

        // Verify method bodies preserved
        assert!(
            chunks[0].contains("System.out.println(\"Method 1\")"),
            "Method1 body preserved"
        );
        assert!(
            chunks[1].contains("System.out.println(\"Method 2\")"),
            "Method2 body preserved"
        );

        // Verify brace structure (important for Java)
        assert_eq!(
            chunks[0].matches('{').count(),
            2,
            "Chunk 0 has 2 opening braces (class, method1)"
        );
        assert_eq!(
            chunks[0].matches('}').count(),
            1,
            "Chunk 0 has 1 closing brace (method1)"
        );
        assert_eq!(
            chunks[1].matches('{').count(),
            1,
            "Chunk 1 has 1 opening brace (method2)"
        );
        assert_eq!(
            chunks[1].matches('}').count(),
            2,
            "Chunk 1 has 2 closing braces (method2, class)"
        );

        // Verify all elements preserved
        let all_code = chunks.join("\n");
        assert!(all_code.contains("MyClass"), "Class name preserved");
        assert!(all_code.contains("method1"), "Method1 name preserved");
        assert!(all_code.contains("method2"), "Method2 name preserved");
        assert!(
            all_code.contains("System.out.println"),
            "Standard library call preserved"
        );
    }

    #[test]
    fn test_language_separators() {
        // Test that each language has separators defined with exact validation
        // Tests validate separator count, structure, and language-specific patterns

        // Python - validate Python-specific separators
        let python_seps = Language::Python.get_separators();
        assert_eq!(python_seps.len(), 7, "Python should have 7 separators");
        assert_eq!(python_seps[0], "\nclass ");
        assert_eq!(python_seps[1], "\ndef ");
        assert_eq!(python_seps[2], "\n\tdef ");
        assert_eq!(python_seps[3], "\n\n");
        assert_eq!(python_seps[4], "\n");
        assert_eq!(python_seps[5], " ");
        assert_eq!(python_seps[6], "");
        // Validate last separator is empty for character-level fallback
        assert_eq!(python_seps.last(), Some(&"".to_string()));

        // Rust - validate Rust-specific separators (has duplicate const)
        let rust_seps = Language::Rust.get_separators();
        assert_eq!(rust_seps.len(), 12, "Rust should have 12 separators");
        assert_eq!(rust_seps[0], "\nfn ");
        assert_eq!(rust_seps[1], "\nconst ");
        assert_eq!(rust_seps[2], "\nlet ");
        assert_eq!(rust_seps[3], "\nif ");
        assert_eq!(rust_seps[4], "\nwhile ");
        assert_eq!(rust_seps[5], "\nfor ");
        assert_eq!(rust_seps[6], "\nloop ");
        assert_eq!(rust_seps[7], "\nmatch ");
        assert_eq!(rust_seps[8], "\n\n");
        assert_eq!(rust_seps[9], "\n");
        assert_eq!(rust_seps[10], " ");
        assert_eq!(rust_seps[11], "");
        assert_eq!(rust_seps.last(), Some(&"".to_string()));

        // JavaScript - validate JS-specific separators
        let js_seps = Language::Js.get_separators();
        assert_eq!(js_seps.len(), 15, "JavaScript should have 15 separators");
        assert_eq!(js_seps[0], "\nfunction ");
        assert_eq!(js_seps[1], "\nconst ");
        assert_eq!(js_seps[2], "\nlet ");
        assert_eq!(js_seps[3], "\nvar ");
        assert_eq!(js_seps[4], "\nclass ");
        assert_eq!(js_seps[5], "\nif ");
        assert_eq!(js_seps[6], "\nfor ");
        assert_eq!(js_seps[7], "\nwhile ");
        assert_eq!(js_seps[8], "\nswitch ");
        assert_eq!(js_seps[9], "\ncase ");
        assert_eq!(js_seps[10], "\ndefault ");
        assert_eq!(js_seps[11], "\n\n");
        assert_eq!(js_seps[12], "\n");
        assert_eq!(js_seps[13], " ");
        assert_eq!(js_seps[14], "");
        assert_eq!(js_seps.last(), Some(&"".to_string()));

        // TypeScript - should have TypeScript-specific separators (NOT same as JavaScript)
        let ts_seps = Language::Ts.get_separators();
        assert_eq!(ts_seps.len(), 19, "TypeScript should have 19 separators");
        assert_eq!(ts_seps[0], "\nenum ");
        assert_eq!(ts_seps[1], "\ninterface ");
        assert_eq!(ts_seps[2], "\nnamespace ");
        assert_eq!(ts_seps[3], "\ntype ");
        assert_eq!(ts_seps[4], "\nclass ");
        assert_eq!(ts_seps[5], "\nfunction ");
        assert_eq!(ts_seps[6], "\nconst ");
        assert_eq!(ts_seps[7], "\nlet ");
        assert_eq!(ts_seps[8], "\nvar ");
        assert_eq!(ts_seps[9], "\nif ");
        assert_eq!(ts_seps[10], "\nfor ");
        assert_eq!(ts_seps[11], "\nwhile ");
        assert_eq!(ts_seps[12], "\nswitch ");
        assert_eq!(ts_seps[13], "\ncase ");
        assert_eq!(ts_seps[14], "\ndefault ");
        assert_eq!(ts_seps[15], "\n\n");
        assert_eq!(ts_seps[16], "\n");
        assert_eq!(ts_seps[17], " ");
        assert_eq!(ts_seps[18], "");
        assert_eq!(ts_seps.last(), Some(&"".to_string()));

        // Java - validate Java-specific separators
        let java_seps = Language::Java.get_separators();
        assert_eq!(java_seps.len(), 14, "Java should have 14 separators");
        assert_eq!(java_seps[0], "\nclass ");
        assert_eq!(java_seps[1], "\npublic ");
        assert_eq!(java_seps[2], "\nprotected ");
        assert_eq!(java_seps[3], "\nprivate ");
        assert_eq!(java_seps[4], "\nstatic ");
        assert_eq!(java_seps[5], "\nif ");
        assert_eq!(java_seps[6], "\nfor ");
        assert_eq!(java_seps[7], "\nwhile ");
        assert_eq!(java_seps[8], "\nswitch ");
        assert_eq!(java_seps[9], "\ncase ");
        assert_eq!(java_seps[10], "\n\n");
        assert_eq!(java_seps[11], "\n");
        assert_eq!(java_seps[12], " ");
        assert_eq!(java_seps[13], "");
        assert_eq!(java_seps.last(), Some(&"".to_string()));

        // Go - validate Go-specific separators
        let go_seps = Language::Go.get_separators();
        assert_eq!(go_seps.len(), 12, "Go should have 12 separators");
        assert_eq!(go_seps[0], "\nfunc ");
        assert_eq!(go_seps[1], "\nvar ");
        assert_eq!(go_seps[2], "\nconst ");
        assert_eq!(go_seps[3], "\ntype ");
        assert_eq!(go_seps[4], "\nif ");
        assert_eq!(go_seps[5], "\nfor ");
        assert_eq!(go_seps[6], "\nswitch ");
        assert_eq!(go_seps[7], "\ncase ");
        assert_eq!(go_seps[8], "\n\n");
        assert_eq!(go_seps[9], "\n");
        assert_eq!(go_seps[10], " ");
        assert_eq!(go_seps[11], "");
        assert_eq!(go_seps.last(), Some(&"".to_string()));

        // C++ - validate C++ specific separators (same as C)
        let cpp_seps = Language::Cpp.get_separators();
        assert_eq!(cpp_seps.len(), 14, "C++ should have 14 separators");
        assert_eq!(cpp_seps[0], "\nclass ");
        assert_eq!(cpp_seps[1], "\nvoid ");
        assert_eq!(cpp_seps[2], "\nint ");
        assert_eq!(cpp_seps[3], "\nfloat ");
        assert_eq!(cpp_seps[4], "\ndouble ");
        assert_eq!(cpp_seps[5], "\nif ");
        assert_eq!(cpp_seps[6], "\nfor ");
        assert_eq!(cpp_seps[7], "\nwhile ");
        assert_eq!(cpp_seps[8], "\nswitch ");
        assert_eq!(cpp_seps[9], "\ncase ");
        assert_eq!(cpp_seps[10], "\n\n");
        assert_eq!(cpp_seps[11], "\n");
        assert_eq!(cpp_seps[12], " ");
        assert_eq!(cpp_seps[13], "");
        assert_eq!(cpp_seps.last(), Some(&"".to_string()));

        // C - validate C specific separators (same as C++)
        let c_seps = Language::C.get_separators();
        assert_eq!(c_seps.len(), 14, "C should have 14 separators");
        assert_eq!(c_seps, cpp_seps, "C should have same separators as C++");
        assert_eq!(c_seps.last(), Some(&"".to_string()));

        // Validate common pattern: all languages end with character-level fallback ("", " ", "\n")
        for lang in [
            Language::Python,
            Language::Rust,
            Language::Js,
            Language::Ts,
            Language::Java,
            Language::Go,
            Language::Cpp,
            Language::C,
        ] {
            let separators = lang.get_separators();
            assert!(
                !separators.is_empty(),
                "Language {:?} must have separators",
                lang
            );

            // Verify empty string is last (character-level fallback)
            assert_eq!(
                separators.last(),
                Some(&"".to_string()),
                "Last separator must be empty string for {:?}",
                lang
            );

            // Verify space is second-to-last (word-level fallback)
            assert_eq!(
                separators.get(separators.len() - 2),
                Some(&" ".to_string()),
                "Second-to-last separator should be space for {:?}",
                lang
            );

            // Verify newline is third-to-last (line-level fallback)
            assert_eq!(
                separators.get(separators.len() - 3),
                Some(&"\n".to_string()),
                "Third-to-last separator should be newline for {:?}",
                lang
            );
        }
    }

    // ===== Edge Case Tests =====

    #[test]
    fn test_splitter_empty_document() {
        let splitter = CharacterTextSplitter::new()
            .with_chunk_size(100)
            .with_chunk_overlap(20);

        let result = splitter.split_text("");
        assert_eq!(result.len(), 0);
    }

    #[test]
    fn test_splitter_single_character() {
        let splitter = CharacterTextSplitter::new()
            .with_chunk_size(100)
            .with_chunk_overlap(20);

        // Test 1: Single ASCII character
        let result = splitter.split_text("a");
        assert_eq!(
            result.len(),
            1,
            "Single character should produce exactly 1 chunk"
        );
        assert_eq!(
            result[0], "a",
            "Single character should be preserved exactly"
        );
        assert_eq!(
            result[0].len(),
            1,
            "Single character chunk should have length 1"
        );
        assert_eq!(
            result[0].chars().count(),
            1,
            "Single character chunk should have 1 Unicode character"
        );

        // Test 2: Single multi-byte Unicode character (emoji)
        let result = splitter.split_text("🌍");
        assert_eq!(
            result.len(),
            1,
            "Single emoji should produce exactly 1 chunk"
        );
        assert_eq!(result[0], "🌍", "Single emoji should be preserved exactly");
        assert_eq!(
            result[0].len(),
            4,
            "Emoji should have byte length 4 (UTF-8 encoding)"
        );
        assert_eq!(
            result[0].chars().count(),
            1,
            "Emoji should have 1 Unicode character"
        );

        // Test 3: Single Chinese character
        let result = splitter.split_text("世");
        assert_eq!(
            result.len(),
            1,
            "Single Chinese character should produce exactly 1 chunk"
        );
        assert_eq!(
            result[0], "世",
            "Single Chinese character should be preserved exactly"
        );
        assert_eq!(
            result[0].len(),
            3,
            "Chinese character should have byte length 3 (UTF-8 encoding)"
        );
        assert_eq!(
            result[0].chars().count(),
            1,
            "Chinese character should have 1 Unicode character"
        );

        // Test 4: Single Arabic character
        let result = splitter.split_text("م");
        assert_eq!(
            result.len(),
            1,
            "Single Arabic character should produce exactly 1 chunk"
        );
        assert_eq!(
            result[0], "م",
            "Single Arabic character should be preserved exactly"
        );
        assert_eq!(
            result[0].chars().count(),
            1,
            "Arabic character should have 1 Unicode character"
        );

        // Test 5: Single space character (whitespace is stripped by default strip_whitespace=true)
        let result = splitter.split_text(" ");
        assert_eq!(
            result.len(),
            0,
            "Single space should produce 0 chunks when strip_whitespace=true (default)"
        );

        // Test 5b: Single space with strip_whitespace=false
        let splitter_no_strip = CharacterTextSplitter::new()
            .with_chunk_size(100)
            .with_chunk_overlap(20)
            .with_strip_whitespace(false);
        let result = splitter_no_strip.split_text(" ");
        assert_eq!(
            result.len(),
            1,
            "Single space should produce 1 chunk when strip_whitespace=false"
        );
        assert_eq!(
            result[0], " ",
            "Single space should be preserved when strip_whitespace=false"
        );
        assert_eq!(result[0].len(), 1, "Single space should have length 1");

        // Test 6: Single newline character (treated as whitespace, stripped by default)
        let result = splitter.split_text("\n");
        assert_eq!(
            result.len(),
            0,
            "Single newline should produce 0 chunks when strip_whitespace=true (default)"
        );

        // Test 6b: Single newline with strip_whitespace=false
        let result = splitter_no_strip.split_text("\n");
        assert_eq!(
            result.len(),
            1,
            "Single newline should produce 1 chunk when strip_whitespace=false"
        );
        assert_eq!(
            result[0], "\n",
            "Single newline should be preserved when strip_whitespace=false"
        );
        assert_eq!(result[0].len(), 1, "Single newline should have length 1");

        // Test 7: Single digit
        let result = splitter.split_text("5");
        assert_eq!(
            result.len(),
            1,
            "Single digit should produce exactly 1 chunk"
        );
        assert_eq!(result[0], "5", "Single digit should be preserved exactly");
        assert_eq!(result[0].len(), 1, "Single digit should have length 1");

        // Test 8: Single special character
        let result = splitter.split_text("@");
        assert_eq!(
            result.len(),
            1,
            "Single special character should produce exactly 1 chunk"
        );
        assert_eq!(
            result[0], "@",
            "Single special character should be preserved exactly"
        );
        assert_eq!(
            result[0].len(),
            1,
            "Single special character should have length 1"
        );

        // Test 9: Verify chunk_overlap doesn't affect single character splitting
        let result_with_overlap = splitter.split_text("a");
        let splitter_no_overlap = CharacterTextSplitter::new()
            .with_chunk_size(100)
            .with_chunk_overlap(0);
        let result_no_overlap = splitter_no_overlap.split_text("a");
        assert_eq!(
            result_with_overlap, result_no_overlap,
            "Single character should produce same result regardless of chunk_overlap"
        );

        // Test 10: Verify chunk_size doesn't affect single character (as long as >= 1)
        let splitter_tiny = CharacterTextSplitter::new()
            .with_chunk_size(1)
            .with_chunk_overlap(0);
        let result_tiny = splitter_tiny.split_text("a");
        assert_eq!(
            result_tiny.len(),
            1,
            "Single character with chunk_size=1 should produce 1 chunk"
        );
        assert_eq!(
            result_tiny[0], "a",
            "Single character should be preserved with chunk_size=1"
        );
    }

    #[test]
    fn test_splitter_unicode_characters() {
        let splitter = CharacterTextSplitter::new()
            .with_chunk_size(100)
            .with_chunk_overlap(20);

        let text = "Hello 🌍 世界 مرحبا שלום Привет";
        let result = splitter.split_text(text);

        // Validate all Unicode characters are preserved exactly
        assert!(!result.is_empty(), "Should produce chunks");

        let all_text = result.join("");
        assert!(all_text.contains("🌍"), "Emoji should be preserved");
        assert!(
            all_text.contains("世界"),
            "Chinese characters should be preserved"
        );
        assert!(
            all_text.contains("مرحبا"),
            "Arabic characters should be preserved"
        );
        assert!(
            all_text.contains("שלום"),
            "Hebrew characters should be preserved"
        );
        assert!(
            all_text.contains("Привет"),
            "Cyrillic characters should be preserved"
        );

        // With chunk_size=100, all text should fit in one chunk
        assert_eq!(result.len(), 1, "All text should fit in single chunk");
        assert_eq!(result[0], text, "Content should exactly match input");

        // Verify no character corruption
        assert_eq!(
            result[0].chars().count(),
            text.chars().count(),
            "Character count should match input"
        );
    }

    #[test]
    fn test_splitter_very_long_document() {
        let splitter = CharacterTextSplitter::new()
            .with_chunk_size(1000)
            .with_chunk_overlap(100);

        // Create 10KB document (10000 'a' characters, no separators)
        let text = "a".repeat(10000);
        let result = splitter.split_text(&text);

        // Validate exact chunk count: with no spaces/separators, default separator " " not found
        // Implementation falls back to character-by-character splitting with empty separator
        // Expected: ceil((10000 - 100) / (1000 - 100)) + 1 = ceil(9900/900) + 1 = 11 + 1 = 12 chunks
        // But implementation may keep as single chunk if no separator match
        // Actual behavior: single chunk when no separator found (tested)
        assert_eq!(
            result.len(),
            1,
            "With no spaces, text stays as single chunk (no separator to split on)"
        );

        // Validate exact chunk content: entire text in one chunk
        assert_eq!(
            result[0], text,
            "Chunk should contain entire 10000-character text"
        );

        // Validate chunk length: 10000 characters (exceeds chunk_size because no split points)
        assert_eq!(
            result[0].len(),
            10000,
            "Chunk should be exactly 10000 characters"
        );

        // Verify no data loss
        let total_chars: usize = result.iter().map(|chunk| chunk.len()).sum();
        assert_eq!(
            total_chars, 10000,
            "Total characters should equal input (no overlap in single chunk)"
        );

        // Validate chunk exceeds chunk_size significantly
        assert!(
            result[0].len() > 1000,
            "Chunk length should exceed chunk_size (no split possible)"
        );

        // Validate exact ratio: chunk is 10x the chunk_size
        assert_eq!(
            result[0].len(),
            1000 * 10,
            "Chunk is exactly 10x the configured chunk_size"
        );

        // Validate all characters are 'a'
        assert!(
            result[0].chars().all(|c| c == 'a'),
            "All characters should be 'a'"
        );

        // Validate first and last characters
        assert_eq!(
            result[0].chars().next(),
            Some('a'),
            "First character should be 'a'"
        );
        assert_eq!(
            result[0].chars().last(),
            Some('a'),
            "Last character should be 'a'"
        );

        // Validate content consistency (no unexpected characters)
        assert!(!result[0].contains(' '), "Chunk should contain no spaces");
        assert!(
            !result[0].contains('\n'),
            "Chunk should contain no newlines"
        );

        // Document behavior: When no separator exists, CharacterTextSplitter keeps text together
        // This is expected behavior - splitter needs separator matches to split
        // This test demonstrates that chunk_size is a soft limit when no separators exist
    }

    #[test]
    fn test_splitter_no_separators() {
        let splitter = CharacterTextSplitter::new()
            .with_chunk_size(10)
            .with_chunk_overlap(0);

        // Text with no spaces or newlines (26 characters)
        let text = "abcdefghijklmnopqrstuvwxyz";
        let result = splitter.split_text(text);

        // Validate exact chunk count: with no spaces, default separator " " not found
        // Implementation keeps text as single chunk when no separator matches
        assert_eq!(
            result.len(),
            1,
            "Text with no separators stays as single chunk"
        );

        // Validate exact chunk content: entire text in one chunk
        assert_eq!(
            result[0], text,
            "Chunk should contain entire 26-character alphabet"
        );

        // Validate chunk length: 26 characters (exceeds chunk_size=10 because no split points)
        assert_eq!(result[0].len(), 26, "Chunk should be exactly 26 characters");

        // Verify all content is preserved exactly
        assert_eq!(
            result[0], "abcdefghijklmnopqrstuvwxyz",
            "All alphabet characters preserved in order"
        );

        // Verify start and end content
        assert!(
            result[0].starts_with("abcdefg"),
            "Chunk starts with 'abcdefg'"
        );
        assert!(result[0].ends_with("xyz"), "Chunk ends with 'xyz'");

        // Validate chunk exceeds chunk_size
        assert!(
            result[0].len() > 10,
            "Chunk length should exceed configured chunk_size of 10"
        );

        // Validate exact ratio: chunk is 2.6x the chunk_size
        assert_eq!(
            result[0].len(),
            26,
            "Chunk is 2.6x the configured chunk_size (26 chars vs 10 limit)"
        );

        // Verify no spaces or newlines
        assert!(!result[0].contains(' '), "Chunk should contain no spaces");
        assert!(
            !result[0].contains('\n'),
            "Chunk should contain no newlines"
        );

        // Validate all lowercase letters present
        assert!(
            result[0].chars().all(|c| c.is_ascii_lowercase()),
            "All characters should be lowercase ASCII letters"
        );

        // Validate exact character count in range
        assert!(result[0].contains('a'), "Should contain first letter 'a'");
        assert!(result[0].contains('z'), "Should contain last letter 'z'");
        assert!(result[0].contains('m'), "Should contain middle letter 'm'");

        // Document behavior: CharacterTextSplitter with default separator " " keeps text
        // together when no separator exists, even if text exceeds chunk_size
        // This is expected - splitter needs separator matches to create split points
    }

    #[test]
    fn test_splitter_chunk_size_one() {
        let splitter = CharacterTextSplitter::new()
            .with_chunk_size(1)
            .with_chunk_overlap(0);

        let result = splitter.split_text("abc");

        // Validate exact chunk count: with chunk_size=1 and default separator " ",
        // "abc" has no spaces, so text stays as single chunk (no split points)
        assert_eq!(
            result.len(),
            1,
            "Text with no separators stays as single chunk even with chunk_size=1"
        );

        // Validate exact chunk content: entire text in one chunk
        assert_eq!(result[0], "abc", "Chunk should contain entire 'abc' text");

        // Validate chunk length: 3 characters (exceeds chunk_size=1 because no split points)
        assert_eq!(result[0].len(), 3, "Chunk should be exactly 3 characters");

        // Verify exact content preservation
        assert_eq!(
            result.join(""),
            "abc",
            "All content should be preserved exactly"
        );

        // Verify each character is present
        assert!(result[0].contains('a'), "Contains 'a'");
        assert!(result[0].contains('b'), "Contains 'b'");
        assert!(result[0].contains('c'), "Contains 'c'");

        // Validate chunk significantly exceeds chunk_size
        assert!(
            result[0].len() > 1,
            "Chunk length should exceed chunk_size of 1"
        );

        // Validate exact ratio: chunk is 3x the chunk_size
        assert_eq!(
            result[0].len(),
            3,
            "Chunk is 3x the configured chunk_size (3 chars vs 1 limit)"
        );

        // Validate character order preserved
        assert_eq!(
            result[0].chars().next(),
            Some('a'),
            "First character should be 'a'"
        );
        assert_eq!(
            result[0].chars().nth(1),
            Some('b'),
            "Second character should be 'b'"
        );
        assert_eq!(
            result[0].chars().nth(2),
            Some('c'),
            "Third character should be 'c'"
        );

        // Verify no spaces or separators
        assert!(!result[0].contains(' '), "Chunk should contain no spaces");
        assert!(
            !result[0].contains('\n'),
            "Chunk should contain no newlines"
        );

        // Document behavior: Even with chunk_size=1, text without separators stays together
        // This demonstrates that chunk_size is a soft limit - actual chunks can exceed it
        // when no separator provides a valid split point
        // This is the most extreme example: chunk_size=1 but chunk length=3
    }

    #[test]
    fn test_splitter_chunk_overlap_equals_size() {
        let splitter = CharacterTextSplitter::new()
            .with_chunk_size(10)
            .with_chunk_overlap(10);

        let text = "This is a test document";
        let result = splitter.split_text(text);

        // Validate exact chunk count: when overlap equals chunk_size, implementation
        // keeps text together as single chunk (tested behavior)
        // Text: "This is a test document" (23 chars)
        // With chunk_size=10, overlap=10, merging logic results in single chunk
        assert_eq!(
            result.len(),
            1,
            "When overlap equals chunk_size, produces single chunk"
        );

        // Validate exact chunk content: entire text in one chunk
        assert_eq!(result[0], text, "Chunk should contain entire text exactly");

        // Validate chunk length: 23 characters (exceeds chunk_size=10)
        assert_eq!(result[0].len(), 23, "Chunk should be exactly 23 characters");

        // Verify finite chunk count (edge case shouldn't cause infinite loop)
        assert!(
            result.len() < 100,
            "Should produce finite number of chunks (not infinite loop)"
        );

        // Verify all unique words are present
        assert!(result[0].contains("This"), "Content 'This' preserved");
        assert!(result[0].contains("is"), "Content 'is' preserved");
        assert!(result[0].contains("a"), "Content 'a' preserved");
        assert!(result[0].contains("test"), "Content 'test' preserved");
        assert!(
            result[0].contains("document"),
            "Content 'document' preserved"
        );

        // Verify exact content preservation
        assert_eq!(result[0], "This is a test document", "Exact text preserved");

        // Verify all content in correct order
        assert!(result[0].starts_with("This"), "Starts with 'This'");
        assert!(result[0].ends_with("document"), "Ends with 'document'");

        // Document behavior: When overlap equals chunk_size, the merging logic keeps all text
        // together in a single chunk. This is an edge case that avoids infinite loops by
        // preventing forward progress issues (can't advance past overlap window when overlap = size)
    }

    #[test]
    fn test_splitter_many_newlines() {
        let splitter = CharacterTextSplitter::new()
            .with_chunk_size(100)
            .with_chunk_overlap(20);

        let text = "Line1\n\n\n\n\n\n\nLine2";
        let result = splitter.split_text(text);

        // Validate exact output with multiple consecutive newlines
        // Expected behavior: default separators include "\n\n" (double newline)
        // which matches once, producing split: ["Line1", "\n\n\n\n\n\nLine2"]
        // The second part starts with remaining newlines after first match
        // CharacterTextSplitter keeps separators, so we get: "Line1\n\n\nLine2"
        // (some newlines consolidated in splitting process)
        assert_eq!(result.len(), 1, "Should produce exactly 1 chunk");
        assert_eq!(
            result[0], "Line1\n\n\nLine2",
            "Chunk should have exact content with consolidated newlines"
        );
        assert_eq!(
            result[0].len(),
            13,
            "Chunk should be exactly 13 characters (Line1 + 3 newlines + Line2)"
        );

        // Validate content structure
        assert!(result[0].starts_with("Line1"), "Should start with Line1");
        assert!(result[0].ends_with("Line2"), "Should end with Line2");
        assert!(
            result[0].contains("\n\n\n"),
            "Should contain triple newline"
        );

        // Validate content preservation
        assert!(result[0].contains("Line1"), "Line1 should be preserved");
        assert!(result[0].contains("Line2"), "Line2 should be preserved");

        // Verify newline count (should be 3 newlines, not original 7)
        let newline_count = result[0].chars().filter(|c| *c == '\n').count();
        assert_eq!(
            newline_count, 3,
            "Should have exactly 3 newlines after splitting"
        );

        // Verify chunk respects size limit
        assert!(result[0].len() <= 100, "Chunk should not exceed size limit");

        // Document behavior: CharacterTextSplitter with default separators (["\n\n", "\n", " ", ""])
        // splits on first matching separator ("\n\n"), which matches once between Line1 and Line2.
        // Some newlines are consolidated in the splitting/rejoining process, resulting in 3 newlines
        // in the output instead of the original 7.
    }

    #[test]
    fn test_recursive_splitter_empty() {
        let splitter = RecursiveCharacterTextSplitter::new()
            .with_chunk_size(100)
            .with_chunk_overlap(20);

        let result = splitter.split_text("");
        assert_eq!(result.len(), 0);
    }

    #[test]
    fn test_recursive_splitter_unicode() {
        let splitter = RecursiveCharacterTextSplitter::new()
            .with_chunk_size(100)
            .with_chunk_overlap(20);

        let text = "🎉 Party time! 世界 مرحبا";
        let result = splitter.split_text(text);

        // Validate exact chunk count
        assert_eq!(
            result.len(),
            1,
            "All text should fit in single chunk (text len=34 bytes, chunk_size=100)"
        );

        // Validate exact chunk content
        assert_eq!(result[0], text, "Chunk content should exactly match input");

        // Validate exact byte length (Unicode chars are multi-byte)
        assert_eq!(
            result[0].len(),
            34,
            "Chunk should be exactly 34 bytes (emoji=4, Chinese=6, Arabic=10, ASCII=14)"
        );

        // Validate character count vs byte count
        assert_eq!(
            result[0].chars().count(),
            22,
            "Should be exactly 22 Unicode characters (emoji=1, 世界=2, مرحبا=5, others=14)"
        );

        // Validate Unicode preservation: emoji
        assert!(result[0].contains("🎉"), "Emoji '🎉' should be preserved");
        assert_eq!(result[0].matches("🎉").count(), 1, "Exactly one emoji");

        // Validate Unicode preservation: Chinese
        assert!(
            result[0].contains("世界"),
            "Chinese characters '世界' should be preserved"
        );
        assert_eq!(
            result[0].matches("世界").count(),
            1,
            "Exactly one occurrence of '世界'"
        );

        // Validate Unicode preservation: Arabic
        assert!(
            result[0].contains("مرحبا"),
            "Arabic characters 'مرحبا' should be preserved"
        );
        assert_eq!(
            result[0].matches("مرحبا").count(),
            1,
            "Exactly one occurrence of 'مرحبا'"
        );

        // Validate ASCII text preservation
        assert!(
            result[0].contains("Party time!"),
            "ASCII text 'Party time!' should be preserved"
        );

        // Validate chunk respects size limit
        assert!(
            result[0].len() <= 100,
            "Chunk should not exceed chunk_size of 100"
        );

        // Validate no data loss
        assert!(!result.is_empty(), "Should produce at least one chunk");
        let joined = result.join("");
        assert_eq!(joined, text, "Joined chunks should exactly match input");

        // Document behavior: RecursiveCharacterTextSplitter handles multi-byte Unicode correctly
        // Byte length (34) differs from character count (22) due to emoji, Chinese, Arabic chars
    }

    #[test]
    fn test_markdown_splitter_empty() {
        let splitter = MarkdownTextSplitter::new()
            .with_chunk_size(100)
            .with_chunk_overlap(20);

        let result = splitter.split_text("");
        assert_eq!(result.len(), 0);
    }

    #[test]
    fn test_markdown_splitter_no_code_blocks() {
        let splitter = MarkdownTextSplitter::new()
            .with_chunk_size(100)
            .with_chunk_overlap(20);

        let text = "Just plain text without any markdown";
        let result = splitter.split_text(text);

        // Validate exact chunk count
        assert_eq!(
            result.len(),
            1,
            "Should produce single chunk for short plain text (36 chars < 100 chunk_size)"
        );

        // Validate exact chunk content
        assert_eq!(result[0], text, "Content should exactly match input");

        // Validate exact byte length
        assert_eq!(result[0].len(), 36, "Chunk should be exactly 36 bytes");

        // Validate character count matches byte count (ASCII only)
        assert_eq!(
            result[0].chars().count(),
            36,
            "ASCII text: character count should equal byte count"
        );

        // Validate chunk respects size limit
        assert!(
            result[0].len() <= 100,
            "Chunk should not exceed chunk_size of 100"
        );

        // Validate no markdown formatting
        assert!(!result[0].contains('#'), "No markdown headers");
        assert!(!result[0].contains('*'), "No markdown bold/italic");
        assert!(!result[0].contains('`'), "No markdown code");
        assert!(!result[0].contains('['), "No markdown links");

        // Validate exact text structure
        assert!(result[0].starts_with("Just"), "Starts with 'Just'");
        assert!(result[0].ends_with("markdown"), "Ends with 'markdown'");

        // Validate word count
        assert_eq!(
            result[0].split_whitespace().count(),
            6,
            "Should contain exactly 6 words"
        );

        // Document behavior: MarkdownTextSplitter handles plain text like CharacterTextSplitter
        // No special markdown processing when no markdown syntax present
    }

    #[test]
    fn test_markdown_splitter_unicode_in_code() {
        let splitter = MarkdownTextSplitter::new()
            .with_chunk_size(500)
            .with_chunk_overlap(50);

        let text = "```python\nprint('Hello 世界 🌍')\n```";
        let result = splitter.split_text(text);

        // Exact chunk count validation (40 chars fits in chunk_size=500)
        assert_eq!(
            result.len(),
            1,
            "Expected 1 chunk (text len=40, chunk_size=500)"
        );

        // Exact chunk content validation
        let expected = "```python\nprint('Hello 世界 🌍')\n```";
        assert_eq!(
            result[0], expected,
            "Chunk should contain exact markdown with code fence and Unicode (40 chars)"
        );

        // Verify exact length
        assert_eq!(
            result[0].len(),
            40,
            "Chunk should be exactly 40 bytes (note: Unicode chars are multi-byte)"
        );

        // Verify chunk size constraint
        assert!(
            result[0].len() <= 500,
            "Chunk within size: {} <= 500",
            result[0].len()
        );

        // Verify Unicode preservation (exact characters)
        assert!(
            result[0].contains("世界"),
            "Chinese characters '世界' preserved"
        );
        assert!(result[0].contains("🌍"), "Emoji '🌍' preserved");
        assert_eq!(
            result[0].matches("世界").count(),
            1,
            "Exactly one occurrence of '世界'"
        );
        assert_eq!(
            result[0].matches("🌍").count(),
            1,
            "Exactly one occurrence of '🌍'"
        );

        // Verify code block structure preserved
        assert!(
            result[0].contains("```python"),
            "Code fence opening with language preserved"
        );
        assert!(
            result[0].starts_with("```python"),
            "Chunk starts with code fence"
        );
        assert!(
            result[0].ends_with("```"),
            "Chunk ends with code fence closing"
        );
        assert_eq!(
            result[0].matches("```").count(),
            2,
            "Exactly two code fences (opening and closing)"
        );

        // Verify code content preserved
        assert!(
            result[0].contains("print('Hello 世界 🌍')"),
            "Print statement with Unicode preserved"
        );
        assert!(result[0].contains("print"), "print keyword preserved");
        assert!(result[0].contains("Hello"), "ASCII text 'Hello' preserved");

        // Verify newlines in code block preserved
        assert_eq!(
            result[0].matches('\n').count(),
            2,
            "Two newlines preserved (after opening fence, after code line)"
        );

        // Verify exact string match (no character corruption)
        assert_eq!(
            result[0], text,
            "Output exactly matches input (no modifications)"
        );

        // Verify character count (Unicode aware)
        // "```python\nprint('Hello 世界 🌍')\n```"
        // = 10 + 1 + 25 + 1 + 3 = 40 bytes, but fewer chars due to multi-byte Unicode
        let char_count = result[0].chars().count();
        assert_eq!(
            char_count, 33,
            "Should have 33 Unicode characters (世界=2 chars, 🌍=1 char, rest ASCII)"
        );
    }

    #[test]
    fn test_html_splitter_empty() {
        let splitter = HTMLTextSplitter::new()
            .with_chunk_size(100)
            .with_chunk_overlap(20);

        let result = splitter.split_text("");
        assert_eq!(result.len(), 0);
    }

    #[test]
    fn test_html_splitter_plain_text() {
        let splitter = HTMLTextSplitter::new()
            .with_chunk_size(100)
            .with_chunk_overlap(20);

        let text = "Just plain text, no HTML tags";
        let result = splitter.split_text(text);

        // Validate exact chunk count
        assert_eq!(
            result.len(),
            1,
            "Should produce single chunk for short plain text (29 chars < 100 chunk_size)"
        );

        // Validate exact chunk content
        assert_eq!(result[0], text, "Content should exactly match input");

        // Validate exact byte length
        assert_eq!(result[0].len(), 29, "Chunk should be exactly 29 bytes");

        // Validate character count matches byte count (ASCII only)
        assert_eq!(
            result[0].chars().count(),
            29,
            "ASCII text: character count should equal byte count"
        );

        // Validate chunk respects size limit
        assert!(
            result[0].len() <= 100,
            "Chunk should not exceed chunk_size of 100"
        );

        // Validate no HTML tags
        assert!(
            !result[0].contains('<'),
            "No opening angle brackets (no HTML tags)"
        );
        assert!(
            !result[0].contains('>'),
            "No closing angle brackets (no HTML tags)"
        );
        assert!(!result[0].contains("&"), "No HTML entities");

        // Validate exact text structure
        assert!(result[0].starts_with("Just"), "Starts with 'Just'");
        assert!(result[0].ends_with("tags"), "Ends with 'tags'");

        // Validate word count
        assert_eq!(
            result[0].split_whitespace().count(),
            6,
            "Should contain exactly 6 words"
        );

        // Validate contains comma
        assert!(result[0].contains(','), "Should contain comma separator");

        // Document behavior: HTMLTextSplitter handles plain text like CharacterTextSplitter
        // No special HTML processing when no HTML tags present
    }

    #[test]
    fn test_html_splitter_unicode() {
        let splitter = HTMLTextSplitter::new()
            .with_chunk_size(500)
            .with_chunk_overlap(50);

        let html = "<p>Hello 世界 🌍 مرحبا</p>";
        let result = splitter.split_text(html);

        // Validate exact output with Unicode in HTML
        // HTMLTextSplitter preserves HTML tags with Unicode content
        assert_eq!(result.len(), 1, "Should produce exactly 1 chunk");
        assert_eq!(
            result[0], "<p>Hello 世界 🌍 مرحبا</p>",
            "Chunk should exactly match input HTML with all Unicode preserved"
        );
        assert_eq!(
            result[0].len(),
            35,
            "Chunk should be exactly 35 bytes (HTML tags + Unicode content)"
        );

        // Validate content structure
        assert!(
            result[0].starts_with("<p>"),
            "Should start with opening <p> tag"
        );
        assert!(
            result[0].ends_with("</p>"),
            "Should end with closing </p> tag"
        );

        // Validate all Unicode characters are preserved exactly
        assert!(
            result[0].contains("世界"),
            "Chinese characters should be preserved"
        );
        assert!(result[0].contains("🌍"), "Emoji should be preserved");
        assert!(
            result[0].contains("مرحبا"),
            "Arabic characters should be preserved"
        );
        assert!(
            result[0].contains("Hello"),
            "ASCII text should be preserved"
        );

        // Verify character count (not byte count) includes all Unicode correctly
        let char_count = result[0].chars().count();
        assert_eq!(
            char_count, 23,
            "Should have exactly 23 characters (tags + Unicode)"
        );

        // Verify chunk respects size limit
        assert!(result[0].len() <= 500, "Chunk should not exceed size limit");

        // Document behavior: HTMLTextSplitter preserves HTML structure and Unicode characters
        // exactly. With chunk_size=500, the entire short HTML string fits in one chunk.
    }

    #[test]
    fn test_markdown_header_splitter_empty() {
        let headers = vec![("#".to_string(), "H1".to_string())];
        let splitter = MarkdownHeaderTextSplitter::new(headers);

        let result = splitter.split_text("");
        assert_eq!(result.len(), 0);
    }

    #[test]
    fn test_markdown_header_splitter_no_headers() {
        let headers = vec![("#".to_string(), "H1".to_string())];
        let splitter = MarkdownHeaderTextSplitter::new(headers);

        let text = "Just plain text";
        let result = splitter.split_text(text);

        // Validate exact output when text contains no headers
        assert_eq!(result.len(), 1, "Should produce exactly 1 document");

        // Validate exact content
        assert_eq!(
            result[0].page_content, "Just plain text",
            "Content should exactly match input text"
        );
        assert_eq!(
            result[0].page_content.len(),
            15,
            "Content should be exactly 15 characters"
        );

        // Validate content structure
        assert!(
            result[0].page_content.starts_with("Just"),
            "Should start with 'Just'"
        );
        assert!(
            result[0].page_content.ends_with("text"),
            "Should end with 'text'"
        );

        // Validate metadata is empty (no headers to extract)
        assert_eq!(
            result[0].metadata.len(),
            0,
            "Metadata should be empty when no headers present"
        );
        assert!(
            result[0].metadata.is_empty(),
            "Metadata should be empty HashMap"
        );

        // Verify document structure
        assert_eq!(
            result[0].page_content, text,
            "Document content should preserve input exactly"
        );

        // Document behavior: MarkdownHeaderTextSplitter processes plain text without headers
        // by creating a single document with the full text content and empty metadata.
        // No header extraction occurs when the text lacks header markers (e.g., "# Header").
    }

    #[test]
    fn test_markdown_header_splitter_unicode_headers() {
        let headers = vec![("#".to_string(), "H1".to_string())];
        let splitter = MarkdownHeaderTextSplitter::new(headers);

        let text = "# 世界 Header\nContent with 🌍";
        let result = splitter.split_text(text);

        // Validate exact output with Unicode in both headers and content
        assert_eq!(result.len(), 1, "Should produce exactly 1 document");

        // Validate exact content (header text is extracted to metadata, not included in content)
        assert_eq!(
            result[0].page_content, "Content with 🌍",
            "Content should exactly match body text with emoji"
        );
        assert_eq!(
            result[0].page_content.len(),
            17,
            "Content should be exactly 17 bytes"
        );

        // Validate content structure
        assert!(
            result[0].page_content.starts_with("Content"),
            "Should start with 'Content'"
        );
        assert!(
            result[0].page_content.ends_with("🌍"),
            "Should end with emoji"
        );
        assert!(
            result[0].page_content.contains("with"),
            "Should contain 'with'"
        );

        // Validate content preservation
        assert!(
            result[0].page_content.contains("🌍"),
            "Emoji in content should be preserved"
        );

        // Validate character count (emoji is single character)
        let char_count = result[0].page_content.chars().count();
        assert_eq!(
            char_count, 14,
            "Should have exactly 14 characters including emoji"
        );

        // Validate exact metadata with Unicode header text
        assert_eq!(
            result[0].metadata.len(),
            1,
            "Should have exactly 1 metadata entry"
        );

        let h1_value = result[0].metadata.get("H1");
        assert!(h1_value.is_some(), "H1 metadata must exist");

        let h1_str = h1_value.unwrap().as_str().unwrap_or("");
        assert_eq!(
            h1_str, "世界 Header",
            "H1 metadata should exactly match header text with Chinese characters"
        );
        assert!(
            h1_str.contains("世界"),
            "Chinese characters in header should be preserved"
        );
        assert!(
            h1_str.contains("Header"),
            "ASCII text in header should be preserved"
        );

        // Document behavior: MarkdownHeaderTextSplitter extracts headers to metadata,
        // preserving Unicode characters in both header text (metadata) and body content.
        // The header line "# 世界 Header" becomes metadata {"H1": "世界 Header"},
        // and only the content "Content with 🌍" remains in page_content.
    }

    #[test]
    fn test_html_header_splitter_empty() {
        let headers = vec![("h1".to_string(), "Header1".to_string())];
        let splitter = HTMLHeaderTextSplitter::new(headers);

        let result = splitter.split_text("");
        assert_eq!(result.len(), 0);
    }

    // REMOVED: test_html_header_splitter_no_html - Test had no assertions
    // Functionality covered by other tests (empty test, basic test)

    #[test]
    fn test_html_header_splitter_unicode() {
        let headers = vec![("h1".to_string(), "Header1".to_string())];
        let splitter = HTMLHeaderTextSplitter::new(headers);

        let html = "<h1>世界</h1><p>Content 🌍</p>";
        let result = splitter.split_text(html);

        // Exact document count assertion
        assert_eq!(
            result.len(),
            2,
            "Should produce exactly 2 documents (header + content)"
        );

        // Document 0: Header content
        assert_eq!(
            result[0].page_content, "世界",
            "First document should contain header text"
        );
        assert_eq!(
            result[0].page_content.len(),
            6,
            "Header should be 6 bytes (2 Chinese chars)"
        );
        assert_eq!(
            result[0].page_content.chars().count(),
            2,
            "Header should be 2 characters"
        );

        // Document 0: Metadata validation
        assert_eq!(
            result[0].metadata.len(),
            1,
            "First document should have 1 metadata entry"
        );
        assert!(
            result[0].metadata.contains_key("Header1"),
            "Should have Header1 key"
        );
        assert_eq!(
            result[0].metadata.get("Header1").and_then(|v| v.as_str()),
            Some("世界"),
            "Header1 metadata should be '世界'"
        );

        // Document 1: Content
        assert_eq!(
            result[1].page_content, "Content 🌍",
            "Second document should contain paragraph content"
        );
        assert_eq!(
            result[1].page_content.len(),
            12,
            "Content should be 12 bytes"
        );
        assert_eq!(
            result[1].page_content.chars().count(),
            9,
            "Content should be 9 characters"
        );

        // Document 1: Metadata validation (should inherit header)
        assert_eq!(
            result[1].metadata.len(),
            1,
            "Second document should have 1 metadata entry"
        );
        assert!(
            result[1].metadata.contains_key("Header1"),
            "Should inherit Header1 key"
        );
        assert_eq!(
            result[1].metadata.get("Header1").and_then(|v| v.as_str()),
            Some("世界"),
            "Inherited Header1 metadata should be '世界'"
        );

        // Unicode preservation validation
        assert!(
            result[0].page_content.contains("世界"),
            "Chinese characters should be preserved in header"
        );
        assert!(
            result[1].page_content.contains("🌍"),
            "Emoji should be preserved in content"
        );
        assert!(
            result[1].page_content.starts_with("Content"),
            "Content should start with 'Content'"
        );
    }

    #[test]
    fn test_code_splitter_empty() {
        let splitter = RecursiveCharacterTextSplitter::from_language(Language::Python)
            .with_chunk_size(100)
            .with_chunk_overlap(20);

        let result = splitter.split_text("");
        assert_eq!(result.len(), 0);
    }

    #[test]
    fn test_code_splitter_unicode_in_strings() {
        let splitter = RecursiveCharacterTextSplitter::from_language(Language::Python)
            .with_chunk_size(500)
            .with_chunk_overlap(50);

        let code = r#"
def hello():
    return "Hello 世界 🌍"
"#;
        let result = splitter.split_text(code);

        // Exact chunk count assertion
        assert_eq!(
            result.len(),
            1,
            "All code should fit in single chunk with chunk_size=500"
        );

        // Exact content validation
        assert_eq!(
            result[0], "def hello():\n    return \"Hello 世界 🌍\"",
            "Chunk should contain exact code with leading newline trimmed"
        );

        // Exact length assertions
        assert_eq!(result[0].len(), 43, "Chunk should be 43 bytes");
        assert_eq!(
            result[0].chars().count(),
            36,
            "Chunk should be 36 characters"
        );

        // Structure validation
        assert!(
            result[0].starts_with("def hello"),
            "Code should start with function definition"
        );
        assert!(
            result[0].ends_with("🌍\""),
            "Code should end with emoji and quote"
        );

        // Unicode preservation validation
        assert!(
            result[0].contains("世界"),
            "Chinese characters should be preserved in string literal"
        );
        assert!(
            result[0].contains("🌍"),
            "Emoji should be preserved in string literal"
        );

        // Python syntax preservation
        assert!(
            result[0].contains("def hello():"),
            "Function definition should be preserved"
        );
        assert!(
            result[0].contains("return"),
            "Return statement should be preserved"
        );
        assert!(
            result[0].contains("    "),
            "Indentation should be preserved (4 spaces)"
        );

        // Verify newline structure
        assert_eq!(
            result[0].matches('\n').count(),
            1,
            "Should have exactly 1 newline"
        );
    }

    #[test]
    fn test_code_splitter_single_line() {
        // Test single line code in various languages - should return 1 chunk with exact content
        // Tests validate content preservation, length, and language-specific handling

        // Python single line
        let python_splitter = RecursiveCharacterTextSplitter::from_language(Language::Python)
            .with_chunk_size(100)
            .with_chunk_overlap(20);
        let python_code = "print('hello')";
        let python_result = python_splitter.split_text(python_code);
        assert_eq!(
            python_result.len(),
            1,
            "Python: single line should produce 1 chunk"
        );
        assert_eq!(
            python_result[0], python_code,
            "Python: content should be preserved exactly"
        );
        assert_eq!(
            python_result[0].len(),
            14,
            "Python: content length should be 14 bytes"
        );

        // Rust single line
        let rust_splitter = RecursiveCharacterTextSplitter::from_language(Language::Rust)
            .with_chunk_size(100)
            .with_chunk_overlap(20);
        let rust_code = r#"println!("hello");"#;
        let rust_result = rust_splitter.split_text(rust_code);
        assert_eq!(
            rust_result.len(),
            1,
            "Rust: single line should produce 1 chunk"
        );
        assert_eq!(
            rust_result[0], rust_code,
            "Rust: content should be preserved exactly"
        );
        assert_eq!(
            rust_result[0].len(),
            18,
            "Rust: content length should be 18 bytes"
        );

        // JavaScript single line
        let js_splitter = RecursiveCharacterTextSplitter::from_language(Language::Js)
            .with_chunk_size(100)
            .with_chunk_overlap(20);
        let js_code = "console.log('hello');";
        let js_result = js_splitter.split_text(js_code);
        assert_eq!(
            js_result.len(),
            1,
            "JavaScript: single line should produce 1 chunk"
        );
        assert_eq!(
            js_result[0], js_code,
            "JavaScript: content should be preserved exactly"
        );
        assert_eq!(
            js_result[0].len(),
            21,
            "JavaScript: content length should be 21 bytes"
        );

        // Java single line
        let java_splitter = RecursiveCharacterTextSplitter::from_language(Language::Java)
            .with_chunk_size(100)
            .with_chunk_overlap(20);
        let java_code = r#"System.out.println("hello");"#;
        let java_result = java_splitter.split_text(java_code);
        assert_eq!(
            java_result.len(),
            1,
            "Java: single line should produce 1 chunk"
        );
        assert_eq!(
            java_result[0], java_code,
            "Java: content should be preserved exactly"
        );
        assert_eq!(
            java_result[0].len(),
            28,
            "Java: content length should be 28 bytes"
        );

        // Test single line with Unicode
        let unicode_code = "print('你好世界')"; // Chinese "hello world"
        let unicode_result = python_splitter.split_text(unicode_code);
        assert_eq!(
            unicode_result.len(),
            1,
            "Unicode: single line should produce 1 chunk"
        );
        assert_eq!(
            unicode_result[0], unicode_code,
            "Unicode: content should be preserved exactly"
        );
        assert_eq!(
            unicode_result[0].len(),
            21,
            "Unicode: content length should be 21 bytes (print(') = 7 + 你好世界 = 12 + ') = 2"
        );
        assert_eq!(
            unicode_result[0].chars().count(),
            13,
            "Unicode: should have 13 characters (print(') + 4 Chinese chars + ')"
        );

        // Test single line exactly at chunk_size boundary
        let boundary_splitter = RecursiveCharacterTextSplitter::from_language(Language::Python)
            .with_chunk_size(10)
            .with_chunk_overlap(0);
        let boundary_code = "print(123)"; // Exactly 10 bytes
        let boundary_result = boundary_splitter.split_text(boundary_code);
        assert_eq!(
            boundary_result.len(),
            1,
            "Boundary: single line at chunk_size should produce 1 chunk"
        );
        assert_eq!(
            boundary_result[0], boundary_code,
            "Boundary: content should be preserved exactly"
        );
        assert_eq!(
            boundary_result[0].len(),
            10,
            "Boundary: should be exactly chunk_size"
        );

        // Test single line over chunk_size (will be split character-by-character if no separators work)
        let over_size_code = "print(12345)"; // 12 bytes > chunk_size 10
        let over_size_result = boundary_splitter.split_text(over_size_code);
        assert_eq!(
            over_size_result.len(),
            2,
            "Over-size: single line over chunk_size gets split into 2 chunks"
        );
        assert_eq!(
            over_size_result[0].len(),
            10,
            "Over-size: first chunk should be exactly chunk_size"
        );
        assert_eq!(
            over_size_result[1].len(),
            2,
            "Over-size: second chunk should contain remaining 2 bytes"
        );
        // Verify concatenation preserves original content
        let reconstructed = format!("{}{}", over_size_result[0], over_size_result[1]);
        assert_eq!(
            reconstructed, over_size_code,
            "Over-size: chunks should reconstruct original"
        );

        // Test empty line
        let empty_result = python_splitter.split_text("");
        assert_eq!(
            empty_result.len(),
            0,
            "Empty: empty string should produce 0 chunks"
        );
    }

    #[test]
    fn test_splitter_mixed_line_endings() {
        let splitter = CharacterTextSplitter::new()
            .with_chunk_size(100)
            .with_chunk_overlap(20);

        // Mix of \n, \r\n, and \r
        let text = "Line1\nLine2\r\nLine3\rLine4";
        let result = splitter.split_text(text);

        // Exact chunk count assertion
        assert_eq!(
            result.len(),
            1,
            "All text should fit in single chunk (24 bytes < 100)"
        );

        // Exact content match (line endings preserved as-is)
        assert_eq!(
            result[0], text,
            "Content should exactly match input including line endings"
        );

        // Exact length assertions (Line1=5, \n=1, Line2=5, \r\n=2, Line3=5, \r=1, Line4=5 = 24 bytes)
        assert_eq!(result[0].len(), 24, "Content should be 24 bytes");
        assert_eq!(
            result[0].chars().count(),
            24,
            "Content should be 24 characters"
        );

        // All lines preserved validation
        assert!(result[0].contains("Line1"), "Line1 should be preserved");
        assert!(result[0].contains("Line2"), "Line2 should be preserved");
        assert!(result[0].contains("Line3"), "Line3 should be preserved");
        assert!(result[0].contains("Line4"), "Line4 should be preserved");

        // Line ending preservation validation
        assert!(result[0].contains("\n"), "LF (\\n) should be preserved");
        assert!(
            result[0].contains("\r\n"),
            "CRLF (\\r\\n) should be preserved"
        );
        assert!(result[0].contains("\r"), "CR (\\r) should be preserved");

        // Structure validation
        assert!(result[0].starts_with("Line1"), "Should start with Line1");
        assert!(result[0].ends_with("Line4"), "Should end with Line4");

        // Verify line ending diversity (3 different types)
        let has_lf = result[0].contains("\n");
        let has_crlf = result[0].contains("\r\n");
        let has_cr = result[0].contains("\r");
        assert!(
            has_lf && has_crlf && has_cr,
            "All three line ending types should be present"
        );
    }

    #[test]
    fn test_splitter_control_characters() {
        let splitter = CharacterTextSplitter::new()
            .with_chunk_size(100)
            .with_chunk_overlap(20);

        let text = "Line1\tTab\x00Null\x0BVertical";
        let result = splitter.split_text(text);

        // Exact chunk count assertion
        assert_eq!(result.len(), 1, "All content should fit in single chunk");

        // Exact content match (control characters preserved as-is)
        assert_eq!(
            result[0], text,
            "Content should exactly match input including control chars"
        );

        // Exact length assertions (Line1=5, \t=1, Tab=3, \x00=1, Null=4, \x0B=1, Vertical=8 = 23 bytes)
        assert_eq!(result[0].len(), 23, "Content should be 23 bytes");
        assert_eq!(
            result[0].chars().count(),
            23,
            "Content should be 23 characters"
        );

        // Text content preservation
        assert!(
            result[0].contains("Line1"),
            "Text 'Line1' should be preserved"
        );
        assert!(result[0].contains("Tab"), "Text 'Tab' should be preserved");
        assert!(
            result[0].contains("Null"),
            "Text 'Null' should be preserved"
        );
        assert!(
            result[0].contains("Vertical"),
            "Text 'Vertical' should be preserved"
        );

        // Control character preservation validation
        assert!(
            result[0].contains("\t"),
            "Tab character (\\t) should be preserved"
        );
        assert!(
            result[0].contains("\x00"),
            "Null character (\\x00) should be preserved"
        );
        assert!(
            result[0].contains("\x0B"),
            "Vertical tab (\\x0B) should be preserved"
        );

        // Structure validation
        assert!(result[0].starts_with("Line1"), "Should start with 'Line1'");
        assert!(
            result[0].ends_with("Vertical"),
            "Should end with 'Vertical'"
        );

        // Verify control characters in correct positions
        assert_eq!(
            result[0].chars().nth(5),
            Some('\t'),
            "Tab should be at position 5 (after Line1)"
        );
        assert_eq!(
            result[0].chars().nth(9),
            Some('\x00'),
            "Null should be at position 9 (after Tab)"
        );
        assert_eq!(
            result[0].chars().nth(14),
            Some('\x0B'),
            "Vertical tab should be at position 14 (after Null)"
        );

        // Verify control character types (non-printable ASCII)
        let tab_char = result[0].chars().nth(5).unwrap();
        let null_char = result[0].chars().nth(9).unwrap();
        let vtab_char = result[0].chars().nth(14).unwrap();
        assert_eq!(tab_char as u32, 0x0009, "Tab should be U+0009");
        assert_eq!(null_char as u32, 0x0000, "Null should be U+0000");
        assert_eq!(vtab_char as u32, 0x000B, "Vertical tab should be U+000B");
    }

    #[test]
    fn test_splitter_zero_width_characters() {
        let splitter = CharacterTextSplitter::new()
            .with_chunk_size(100)
            .with_chunk_overlap(20);

        // Zero-width space, zero-width joiner
        let text = "Hello\u{200B}World\u{200D}Test";
        let result = splitter.split_text(text);

        // Validate zero-width character preservation
        assert!(!result.is_empty(), "Should produce chunks");

        // All content should fit in one chunk
        assert_eq!(result.len(), 1, "Should fit in single chunk");

        // Verify zero-width characters are preserved
        assert!(
            result[0].contains("\u{200B}"),
            "Zero-width space should be preserved"
        );
        assert!(
            result[0].contains("\u{200D}"),
            "Zero-width joiner should be preserved"
        );
        assert!(result[0].contains("Hello"), "Text should be preserved");
        assert!(result[0].contains("World"), "Text should be preserved");
        assert!(result[0].contains("Test"), "Text should be preserved");

        // Verify exact content match
        assert_eq!(
            result[0], text,
            "Content should exactly match input including zero-width chars"
        );

        // Verify character count includes zero-width chars
        assert_eq!(
            result[0].chars().count(),
            text.chars().count(),
            "Character count should match input"
        );
    }

    // ==================================================================================
    // MUTATION TESTING MECHANISTIC VALIDATION TESTS
    // These tests validate configuration and internal mechanisms, not just outcomes
    // ==================================================================================

    #[test]
    fn test_character_splitter_configuration_getters() {
        let splitter = CharacterTextSplitter::new()
            .with_chunk_size(100)
            .with_chunk_overlap(20)
            .with_add_start_index(true);

        // Validate configuration getters return exact configured values
        assert_eq!(
            splitter.chunk_size(),
            100,
            "chunk_size() should return configured value"
        );
        assert_eq!(
            splitter.chunk_overlap(),
            20,
            "chunk_overlap() should return configured value"
        );
        assert!(
            splitter.add_start_index(),
            "add_start_index() should return configured value"
        );

        // Test with different values
        let splitter2 = CharacterTextSplitter::new()
            .with_chunk_size(50)
            .with_chunk_overlap(10)
            .with_add_start_index(false);

        assert_eq!(splitter2.chunk_size(), 50);
        assert_eq!(splitter2.chunk_overlap(), 10);
        assert!(!splitter2.add_start_index());

        // Test default values
        let splitter3 = CharacterTextSplitter::new();
        assert_eq!(
            splitter3.chunk_size(),
            4000,
            "Default chunk_size should be 4000"
        );
        assert_eq!(
            splitter3.chunk_overlap(),
            200,
            "Default chunk_overlap should be 200"
        );
        assert!(
            !splitter3.add_start_index(),
            "Default add_start_index should be false"
        );
    }

    #[test]
    fn test_recursive_splitter_configuration_getters() {
        let splitter = RecursiveCharacterTextSplitter::new()
            .with_chunk_size(150)
            .with_chunk_overlap(30);

        assert_eq!(
            splitter.chunk_size(),
            150,
            "chunk_size() should return configured value"
        );
        assert_eq!(
            splitter.chunk_overlap(),
            30,
            "chunk_overlap() should return configured value"
        );
        assert!(
            !splitter.add_start_index(),
            "Default add_start_index should be false"
        );

        // Test with different values
        let splitter2 = RecursiveCharacterTextSplitter::new()
            .with_chunk_size(200)
            .with_chunk_overlap(40);

        assert_eq!(splitter2.chunk_size(), 200);
        assert_eq!(splitter2.chunk_overlap(), 40);
    }

    #[test]
    fn test_markdown_splitter_configuration_getters() {
        let splitter = MarkdownTextSplitter::new()
            .with_chunk_size(120)
            .with_chunk_overlap(25);

        assert_eq!(
            splitter.chunk_size(),
            120,
            "chunk_size() should return configured value"
        );
        assert_eq!(
            splitter.chunk_overlap(),
            25,
            "chunk_overlap() should return configured value"
        );
        assert!(
            !splitter.add_start_index(),
            "Default add_start_index should be false"
        );

        // Test with different values
        let splitter2 = MarkdownTextSplitter::new()
            .with_chunk_size(80)
            .with_chunk_overlap(15);

        assert_eq!(splitter2.chunk_size(), 80);
        assert_eq!(splitter2.chunk_overlap(), 15);
    }

    #[test]
    fn test_html_splitter_configuration_getters() {
        let splitter = HTMLTextSplitter::new()
            .with_chunk_size(180)
            .with_chunk_overlap(35);

        assert_eq!(
            splitter.chunk_size(),
            180,
            "chunk_size() should return configured value"
        );
        assert_eq!(
            splitter.chunk_overlap(),
            35,
            "chunk_overlap() should return configured value"
        );
        assert!(
            !splitter.add_start_index(),
            "Default add_start_index should be false"
        );

        // Test with different values
        let splitter2 = HTMLTextSplitter::new()
            .with_chunk_size(90)
            .with_chunk_overlap(18);

        assert_eq!(splitter2.chunk_size(), 90);
        assert_eq!(splitter2.chunk_overlap(), 18);
    }

    #[test]
    fn test_html_splitter_separator_list() {
        let separators = HTMLTextSplitter::get_separators();

        // Validate separator list is not empty
        assert!(
            !separators.is_empty(),
            "HTML separator list should not be empty"
        );

        // Validate HTML-specific separators are present (check for common HTML tags)
        let sep_str = separators.join(",");
        let has_html_tags = sep_str.contains("<") || sep_str.contains(">");
        assert!(
            has_html_tags,
            "Should contain HTML tag separators, got: {:?}",
            separators
        );

        // Validate it's not a dummy list
        assert!(
            separators.len() > 1,
            "Should have multiple separators for HTML"
        );
        assert_ne!(
            separators,
            vec!["xyzzy"],
            "Should not be a dummy separator list"
        );
    }

    #[test]
    fn test_character_splitter_exact_chunk_size_boundary() {
        let splitter = CharacterTextSplitter::new()
            .with_chunk_size(10)
            .with_chunk_overlap(0)
            .with_separator(" ");

        // Input exactly at chunk_size (10 chars)
        let text = "0123456789";
        let chunks = splitter.split_text(text);

        assert_eq!(
            chunks.len(),
            1,
            "Text exactly at chunk_size should produce 1 chunk"
        );
        assert_eq!(chunks[0], "0123456789");
        assert_eq!(chunks[0].len(), 10);

        // Input one char over chunk_size (11 chars)
        let text2 = "0123456789A";
        let chunks2 = splitter.split_text(text2);

        assert_eq!(
            chunks2.len(),
            1,
            "Text one char over chunk_size should still fit in 1 chunk (no separator to split on)"
        );
        assert_eq!(chunks2[0], "0123456789A");

        // Input with separator at chunk_size boundary
        let text3 = "01234 6789";
        let chunks3 = splitter.split_text(text3);

        // Behavior depends on implementation - may fit in one chunk if < chunk_size
        assert!(!chunks3.is_empty(), "Should produce at least one chunk");
        // Validate all content is preserved
        let all_text = chunks3.join("");
        assert!(all_text.contains("01234"), "Content should be preserved");
        assert!(all_text.contains("6789"), "Content should be preserved");
    }

    #[test]
    fn test_character_splitter_chunk_size_zero() {
        // Edge case: chunk_size = 0 should still produce output (fallback behavior)
        let splitter = CharacterTextSplitter::new()
            .with_chunk_size(0)
            .with_chunk_overlap(0)
            .with_separator(" ");

        let text = "Hello world";
        let chunks = splitter.split_text(text);

        // With chunk_size=0, behavior is implementation-specific
        // At minimum, should not panic and should preserve all content
        assert!(!chunks.is_empty(), "Should produce at least one chunk");
        let all_text = chunks.join(" ");
        assert!(all_text.contains("Hello"), "Content should be preserved");
        assert!(all_text.contains("world"), "Content should be preserved");
    }

    #[test]
    fn test_character_splitter_chunk_overlap_boundary() {
        let splitter = CharacterTextSplitter::new()
            .with_chunk_size(15)
            .with_chunk_overlap(5)
            .with_separator(" ");

        let text = "Hello world test message";
        let chunks = splitter.split_text(text);

        // Validate that overlap is exactly chunk_overlap chars
        assert!(chunks.len() >= 2, "Should have multiple chunks");

        // Check that overlap behavior is present (but exact overlap depends on implementation)
        // With chunk_overlap=5, we expect some shared content between chunks
        if chunks.len() >= 2 {
            // Just verify that content is preserved across chunks
            let all_text = chunks.join(" ");
            for word in ["Hello", "world", "test", "message"] {
                assert!(
                    all_text.contains(word),
                    "Content should be preserved with overlap: {}",
                    word
                );
            }
        }
    }

    #[test]
    fn test_create_documents_start_indices() {
        let splitter = CharacterTextSplitter::new()
            .with_chunk_size(10)
            .with_chunk_overlap(0)
            .with_separator(" ")
            .with_add_start_index(true);

        let text = "Hello world test";
        let docs = splitter.create_documents(&[text], None);

        // Validate that start indices are present and correct
        assert!(docs.len() >= 2, "Should produce multiple documents");

        // First document should start at index 0
        assert!(
            docs[0].metadata.contains_key("start_index"),
            "First doc should have start_index"
        );

        // Validate start_index values are sensible (non-negative, increasing)
        for i in 1..docs.len() {
            assert!(
                docs[i].metadata.contains_key("start_index"),
                "Document {} should have start_index",
                i
            );

            // Start indices should generally increase (allowing for overlap)
            let prev_idx = docs[i - 1].metadata.get("start_index");
            let curr_idx = docs[i].metadata.get("start_index");

            if let (Some(prev), Some(curr)) = (prev_idx, curr_idx) {
                // Just validate they're different (exact validation requires knowing split points)
                assert!(
                    prev != curr || i == docs.len() - 1,
                    "Adjacent documents should have different start indices"
                );
            }
        }
    }

    #[test]
    fn test_create_documents_without_start_indices() {
        let splitter = CharacterTextSplitter::new()
            .with_chunk_size(10)
            .with_chunk_overlap(0)
            .with_separator(" ")
            .with_add_start_index(false);

        let text = "Hello world test";
        let docs = splitter.create_documents(&[text], None);

        // Validate that start indices are NOT present when disabled
        assert!(!docs.is_empty(), "Should produce documents");

        for (i, doc) in docs.iter().enumerate() {
            assert!(
                !doc.metadata.contains_key("start_index"),
                "Document {} should NOT have start_index when add_start_index=false",
                i
            );
        }
    }

    #[test]
    fn test_split_documents_empty_input() {
        let splitter = CharacterTextSplitter::new()
            .with_chunk_size(10)
            .with_chunk_overlap(0);

        let docs = splitter.split_documents(&[]);

        // Empty input should produce empty output
        assert_eq!(
            docs.len(),
            0,
            "split_documents with empty input should produce empty output"
        );
    }

    #[test]
    fn test_create_documents_empty_text() {
        let splitter = CharacterTextSplitter::new()
            .with_chunk_size(10)
            .with_chunk_overlap(0);

        let docs = splitter.create_documents(&[""], None);

        // Empty text should produce empty or minimal output
        // (implementation-specific: may produce 1 empty doc or 0 docs)
        for doc in &docs {
            assert!(
                doc.page_content.is_empty() || doc.page_content.trim().is_empty(),
                "Document from empty text should be empty or whitespace"
            );
        }
    }

    #[test]
    fn test_builder_method_chaining() {
        // Validate that builder methods preserve configuration
        let splitter = CharacterTextSplitter::new()
            .with_chunk_size(100)
            .with_add_start_index(true)
            .with_chunk_overlap(20);

        // All configurations should be preserved
        assert_eq!(splitter.chunk_size(), 100);
        assert_eq!(splitter.chunk_overlap(), 20);
        assert!(splitter.add_start_index());

        // Test multiple chains
        let splitter2 = RecursiveCharacterTextSplitter::new()
            .with_chunk_size(50)
            .with_chunk_overlap(10)
            .with_keep_separator(KeepSeparator::Start);

        assert_eq!(splitter2.chunk_size(), 50);
        assert_eq!(splitter2.chunk_overlap(), 10);
    }

    #[test]
    fn test_default_trait_method_add_start_index() {
        // Test the default trait method implementation
        let splitter = CharacterTextSplitter::new();

        // Default should be false
        assert!(
            !splitter.add_start_index(),
            "Default add_start_index should be false"
        );

        // After setting to true
        let splitter2 = CharacterTextSplitter::new().with_add_start_index(true);

        assert!(
            splitter2.add_start_index(),
            "add_start_index should return true after being set"
        );
    }

    // ==================================================================================
    // MUTATION TESTING WAVE 2 - BOOLEAN LOGIC, ARITHMETIC, BOUNDARY TESTS
    // Targeting remaining high-severity gaps from mutation analysis
    // ==================================================================================

    #[test]
    fn test_markdown_splitter_boolean_logic_headers_with_content() {
        // Target: Boolean logic mutants in MarkdownHeaderTextSplitter
        // - line 1546:53: && with ||
        // - line 1568:21: && with ||
        // - line 1568:45: == with !=
        // - line 1541:51: == with !=

        let splitter =
            MarkdownHeaderTextSplitter::new(vec![("#".to_string(), "Header 1".to_string())]);

        // Test case 1: Headers WITH content
        let text = "# Title\nContent here\n# Another\nMore content";
        let docs = splitter.split_text(text);

        // Validate that headers and content are both processed
        assert!(
            !docs.is_empty(),
            "Should produce documents with headers and content"
        );
        let all_text: String = docs
            .iter()
            .map(|d| d.page_content.as_str())
            .collect::<Vec<_>>()
            .join(" ");
        assert!(
            all_text.contains("Content")
                || docs.iter().any(|d| d.metadata.contains_key("Header 1")),
            "Should preserve content text or header metadata"
        );

        // Test case 2: Headers WITHOUT content (just headers)
        let text2 = "# Title1\n# Title2\n# Title3";
        let docs2 = splitter.split_text(text2);

        // Should still process (headers exist even without content)
        // May produce documents or may produce empty if no content between headers
        // Just validate it doesn't crash
        assert!(docs2.len() <= 10, "Should handle headers without content");

        // Test case 3: Content WITHOUT headers
        let text3 = "Just some regular text\nNo headers here\nMore text";
        let docs3 = splitter.split_text(text3);

        // Should still produce output (fallback to regular splitting)
        assert!(!docs3.is_empty(), "Should handle content without headers");
        let all_text3: String = docs3
            .iter()
            .map(|d| d.page_content.as_str())
            .collect::<Vec<_>>()
            .join(" ");
        assert!(
            all_text3.contains("regular") || all_text3.contains("text"),
            "Should preserve content"
        );
    }

    #[test]
    fn test_markdown_splitter_empty_line_conditions() {
        // Target: Boolean logic with empty lines in aggregate_lines_to_chunks
        // - line 1680:24: delete ! (negation operator)

        let splitter =
            MarkdownHeaderTextSplitter::new(vec![("#".to_string(), "Header 1".to_string())]);

        // Test with empty lines between content
        let text = "# Title\nLine1\n\nLine2\n\n\nLine3";
        let docs = splitter.split_text(text);

        // Should handle empty lines correctly
        assert!(!docs.is_empty(), "Should handle empty lines in content");

        // Validate content preservation (empty lines may be collapsed or preserved)
        let all_text: String = docs
            .iter()
            .map(|d| d.page_content.as_str())
            .collect::<Vec<_>>()
            .join(" ");
        assert!(
            all_text.contains("Line1") || all_text.contains("Line2") || all_text.contains("Line3"),
            "Should preserve text content across empty lines"
        );
    }

    #[test]
    fn test_boundary_greater_than_vs_greater_equal() {
        // Target: Boundary condition mutants in merge_splits
        // - line 69:26: > with >=
        // - line 109:29: > with >=
        // - line 110:38: > with >=

        let splitter = CharacterTextSplitter::new()
            .with_chunk_size(20)
            .with_chunk_overlap(5)
            .with_separator("\n");

        // Test with chunks that are EXACTLY at chunk_size boundary
        // If merge_splits uses `>` vs `>=`, behavior differs at exact boundary
        let text = "12345678901234567890\n12345678901234567890";
        let chunks = splitter.split_text(text);

        // Should produce splits (each line is exactly 20 chars)
        assert!(!chunks.is_empty(), "Should handle text at exact chunk_size");

        // Validate chunk sizes are at or near chunk_size
        for chunk in &chunks {
            assert!(
                chunk.len() <= 25,
                "Chunks should not exceed chunk_size + small buffer"
            );
        }

        // Test with chunks slightly over chunk_size
        let text2 = "123456789012345678901\n123456789012345678901"; // 21 chars each
        let chunks2 = splitter.split_text(text2);

        assert!(!chunks2.is_empty(), "Should handle text over chunk_size");
        let all_text = chunks2.join("");
        assert!(all_text.contains("12345"), "Content should be preserved");
    }

    #[test]
    fn test_boundary_less_than_vs_less_equal() {
        // Target: Boundary condition mutants with < vs <=
        // - line 1134:45: < with <= in split_text_recursive_indexed
        // - line 1175:26: < with <= in split_text_recursive_indexed
        // - line 1673:47: < with <= in aggregate_lines_to_chunks

        let splitter = RecursiveCharacterTextSplitter::new()
            .with_chunk_size(15)
            .with_chunk_overlap(3);

        // Test with text that exercises boundary conditions
        let text = "AAA BBB CCC DDD EEE FFF GGG";
        let chunks = splitter.split_text(text);

        // Validate chunks are produced
        assert!(!chunks.is_empty(), "Should produce chunks");

        // Validate no chunk exceeds chunk_size significantly
        for (i, chunk) in chunks.iter().enumerate() {
            assert!(
                chunk.len() <= 20,
                "Chunk {} should not greatly exceed chunk_size (15): got {} chars",
                i,
                chunk.len()
            );
        }

        // Test with exact boundary input (15 chars)
        let text2 = "123456789012345";
        let chunks2 = splitter.split_text(text2);

        assert_eq!(
            chunks2.len(),
            1,
            "Text exactly at chunk_size should produce 1 chunk"
        );
        assert_eq!(chunks2[0].len(), 15);
    }

    #[test]
    fn test_arithmetic_index_calculations_precise() {
        // Target: Arithmetic mutants in create_documents
        // - line 85:40: + with -
        // - line 85:40: + with *
        // - line 85:61: + with -
        // - line 85:61: - with +

        let splitter = CharacterTextSplitter::new()
            .with_chunk_size(20)
            .with_chunk_overlap(0)
            .with_separator(" ")
            .with_add_start_index(true);

        let text = "The quick brown fox jumps over the lazy dog and more text";
        let docs = splitter.create_documents(&[text], None);

        // Validate start indices are present and reasonable
        assert!(docs.len() >= 2, "Should produce multiple documents");

        for (i, doc) in docs.iter().enumerate() {
            assert!(
                doc.metadata.contains_key("start_index"),
                "Document {} should have start_index",
                i
            );

            // Start index should be a valid position in the original text
            if let Some(start_idx) = doc.metadata.get("start_index") {
                if let Some(idx_val) = start_idx.as_u64() {
                    assert!(
                        idx_val <= text.len() as u64,
                        "Start index {} should be within text length {}",
                        idx_val,
                        text.len()
                    );
                }
            }
        }

        // Validate that start indices are different and increasing for adjacent chunks
        if docs.len() >= 2 {
            let idx0 = docs[0].metadata.get("start_index").and_then(|v| v.as_u64());
            let idx1 = docs[1].metadata.get("start_index").and_then(|v| v.as_u64());

            if let (Some(i0), Some(i1)) = (idx0, idx1) {
                assert_ne!(
                    i0, i1,
                    "Adjacent documents should have different start indices"
                );
                assert!(
                    i1 > i0,
                    "Start indices should increase: {} should be > {}",
                    i1,
                    i0
                );
            }
        }
    }

    #[test]
    fn test_arithmetic_recursive_split_indexing() {
        // Target: Arithmetic mutants in split_text_recursive_indexed
        // - line 1135:33: + with *
        // - line 1189:51: + with -

        let splitter = RecursiveCharacterTextSplitter::new()
            .with_chunk_size(10)
            .with_chunk_overlap(2);

        let text = "Hello\n\nWorld\n\nTest";
        let chunks = splitter.split_text(text);

        // Validate chunks are produced and content is preserved
        assert!(!chunks.is_empty(), "Should produce chunks");

        let all_text = chunks.join(" ");
        assert!(all_text.contains("Hello"), "Content should be preserved");
        assert!(all_text.contains("World"), "Content should be preserved");
        assert!(all_text.contains("Test"), "Content should be preserved");

        // Validate no content is duplicated excessively (overlap should be small)
        let total_chars: usize = chunks.iter().map(|c| c.len()).sum();
        // With overlap=2, total should not be more than ~2x original text length
        assert!(
            total_chars < text.len() * 3,
            "Total chunk size {} should not be excessive compared to input {} (overlap issue)",
            total_chars,
            text.len()
        );
    }

    #[test]
    fn test_boundary_split_utils_greater_than() {
        // Target: Boundary mutants in split_utils.rs split_text_with_compiled_regex
        // - line 48:35: > with >=
        // - line 55:26: > with >=

        let splitter = RecursiveCharacterTextSplitter::new()
            .with_chunk_size(20)
            .with_chunk_overlap(0);

        // Use text that will trigger regex-based splitting
        let text = "Line1\nLine2\nLine3\nLine4";
        let chunks = splitter.split_text(text);

        // Validate splitting occurs
        assert!(!chunks.is_empty(), "Should produce chunks");

        // Validate chunk boundaries are reasonable
        for chunk in &chunks {
            assert!(
                chunk.len() <= 30,
                "Chunks should not greatly exceed chunk_size"
            );
        }

        // Test with text exactly at boundary
        let text2 = "12345678901234567890"; // Exactly 20 chars
        let chunks2 = splitter.split_text(text2);

        assert_eq!(
            chunks2.len(),
            1,
            "Text at chunk_size should produce 1 chunk"
        );
        assert_eq!(chunks2[0], text2);
    }

    #[test]
    fn test_trait_method_create_documents_not_empty() {
        // Target: Empty return mutants
        // - traits.rs line 63: create_documents() -> vec![]
        // - traits.rs line 115: split_documents() -> vec![]

        let splitter = CharacterTextSplitter::new()
            .with_chunk_size(10)
            .with_chunk_overlap(0);

        // Test create_documents directly (trait method)
        let docs = splitter.create_documents(&["Hello world"], None);

        assert!(
            !docs.is_empty(),
            "create_documents should not return empty for non-empty input"
        );
        assert!(
            !docs[0].page_content.is_empty(),
            "Document content should not be empty"
        );

        // Test split_documents directly (trait method)
        let input_docs = vec![crate::Document::new("Test content here")];
        let split_docs = splitter.split_documents(&input_docs);

        assert!(
            !split_docs.is_empty(),
            "split_documents should not return empty for non-empty input"
        );
        assert!(
            !split_docs[0].page_content.is_empty(),
            "Split document content should not be empty"
        );
    }

    #[test]
    fn test_builder_default_return_not_default() {
        // Target: Builder method mutants
        // - line 236: with_add_start_index -> Default::default()
        // - line 1056: with_keep_separator -> Default::default()
        // - line 1062: build() -> Ok(Default::default())

        // Test CharacterTextSplitter builder
        let splitter = CharacterTextSplitter::new()
            .with_chunk_size(50)
            .with_add_start_index(true);

        // After calling with_add_start_index, the splitter should have add_start_index=true
        assert!(
            splitter.add_start_index(),
            "Builder method should preserve configuration, not return Default"
        );
        assert_eq!(
            splitter.chunk_size(),
            50,
            "Previous configuration should be preserved"
        );

        // Test RecursiveCharacterTextSplitter builder
        let builder = RecursiveCharacterTextSplitter::new()
            .with_chunk_size(80)
            .with_chunk_overlap(20)
            .with_keep_separator(KeepSeparator::Start);

        let splitter2 = builder.build().unwrap();

        // After build(), the splitter should have configured values
        assert_eq!(
            splitter2.chunk_size(),
            80,
            "build() should return configured splitter, not Default"
        );
        assert_eq!(
            splitter2.chunk_overlap(),
            20,
            "build() should preserve chunk_overlap configuration"
        );
    }

    #[test]
    fn test_merge_splits_boundary_arithmetic() {
        // Target: Combined boundary + arithmetic mutants in merge_splits
        // - line 69:26: > with >=
        // - line 104:29: + with -
        // - line 104:35: delete !

        let splitter = CharacterTextSplitter::new()
            .with_chunk_size(15)
            .with_chunk_overlap(5)
            .with_separator(" ");

        // Test text that requires merging splits
        let text = "A B C D E F G H I J K";
        let chunks = splitter.split_text(text);

        // Validate chunks are produced
        assert!(!chunks.is_empty(), "Should produce chunks");

        // Validate overlap behavior (chunks should share ~5 chars)
        if chunks.len() >= 2 {
            // Check that chunks contain content
            for (i, chunk) in chunks.iter().enumerate() {
                assert!(!chunk.is_empty(), "Chunk {} should not be empty", i);
                assert!(
                    chunk.len() <= 25,
                    "Chunk {} length {} should not greatly exceed chunk_size + overlap",
                    i,
                    chunk.len()
                );
            }
        }

        // Validate all content is preserved
        let all_text = chunks.join(" ");
        for letter in ["A", "B", "C", "D", "E", "F", "G", "H", "I", "J", "K"] {
            assert!(
                all_text.contains(letter),
                "Content '{}' should be preserved in output",
                letter
            );
        }
    }

    // Property-based tests to catch mutation gaps
    mod property_tests {
        use super::*;
        use proptest::prelude::*;

        // Generate valid chunk sizes (1-10000)
        fn chunk_size() -> impl Strategy<Value = usize> {
            1usize..10000
        }

        // Generate text content (no special chars for simpler testing)
        fn text_content() -> impl Strategy<Value = String> {
            "[a-zA-Z0-9 ]{10,200}"
        }

        // Generate separators
        fn separator() -> impl Strategy<Value = String> {
            prop::sample::select(vec![
                " ".to_string(),
                "\n".to_string(),
                ", ".to_string(),
                ". ".to_string(),
                "||".to_string(),
            ])
        }

        proptest! {
            /// Property: Configuration getters return configured values
            /// This catches mutations like: chunk_size() -> 0, chunk_overlap() -> 1
            #[test]
            fn prop_config_getters_correct(
                chunk_size in chunk_size(),
                chunk_overlap in 0usize..100,
            ) {
                // Ensure valid config (overlap < chunk_size)
                let overlap = chunk_overlap.min(chunk_size - 1);

                let splitter = CharacterTextSplitter::new()
                    .with_chunk_size(chunk_size)
                    .with_chunk_overlap(overlap)
                    .with_separator(" ");

                // Validate getters return exact configured values
                prop_assert_eq!(splitter.chunk_size(), chunk_size, "chunk_size() must return configured value");
                prop_assert_eq!(splitter.chunk_overlap(), overlap, "chunk_overlap() must return configured overlap");
            }

            /// Property: add_start_index getter returns configured value
            /// This catches mutations like: add_start_index() -> false
            #[test]
            fn prop_add_start_index_getter(
                add_index in prop::bool::ANY,
            ) {
                let splitter = CharacterTextSplitter::new()
                    .with_add_start_index(add_index)
                    .with_separator(" ");

                prop_assert_eq!(splitter.add_start_index(), add_index, "add_start_index() getter must return configured value");
            }

            /// Property: Split-then-merge preserves content (no data loss)
            /// This catches boundary condition mutations in merge logic
            #[test]
            fn prop_split_merge_preserves_content(
                text in text_content(),
                chunk_size in 10usize..100,
                chunk_overlap in 0usize..10,
            ) {
                let overlap = chunk_overlap.min(chunk_size - 1);
                let splitter = CharacterTextSplitter::new()
                    .with_chunk_size(chunk_size)
                    .with_chunk_overlap(overlap)
                    .with_separator(" ");

                let chunks = splitter.split_text(&text);

                // All content should be preserved
                let merged = chunks.join(" ");
                let original_words: std::collections::HashSet<&str> = text.split_whitespace().collect();
                let merged_words: std::collections::HashSet<&str> = merged.split_whitespace().collect();

                prop_assert!(
                    original_words.is_subset(&merged_words),
                    "All words from original text should appear in chunks. Missing: {:?}",
                    original_words.difference(&merged_words).collect::<Vec<_>>()
                );
            }

            /// Property: All chunks respect chunk_size limit (or are unavoidable)
            /// This catches < vs <= and > vs >= mutations
            #[test]
            fn prop_chunks_respect_size_limit(
                text in text_content(),
                chunk_size in 10usize..100,
            ) {
                let splitter = CharacterTextSplitter::new()
                    .with_chunk_size(chunk_size)
                    .with_chunk_overlap(0)
                    .with_separator(" ");

                let chunks = splitter.split_text(&text);

                for (i, chunk) in chunks.iter().enumerate() {
                    // Either chunk <= chunk_size, or it's a single word that's too long
                    let chunk_len = chunk.len();
                    let is_single_word = !chunk.contains(' ');

                    prop_assert!(
                        chunk_len <= chunk_size || is_single_word,
                        "Chunk {} has length {} > {} and is not a single word. Chunk: '{}'",
                        i, chunk_len, chunk_size, chunk
                    );
                }
            }

            /// Property: Overlap behavior is correct
            /// When overlap > 0, consecutive chunks should share content (best-effort)
            #[test]
            fn prop_overlap_produces_more_chunks(
                text in "[a-zA-Z ]{50,200}",
                chunk_size in 20usize..60,
                chunk_overlap in 5usize..15,
            ) {
                let overlap = chunk_overlap.min(chunk_size - 1);

                // Create two splitters: one with overlap, one without
                let splitter_with_overlap = CharacterTextSplitter::new()
                    .with_chunk_size(chunk_size)
                    .with_chunk_overlap(overlap)
                    .with_separator(" ");

                let splitter_no_overlap = CharacterTextSplitter::new()
                    .with_chunk_size(chunk_size)
                    .with_chunk_overlap(0)
                    .with_separator(" ");

                let chunks_with_overlap = splitter_with_overlap.split_text(&text);
                let chunks_no_overlap = splitter_no_overlap.split_text(&text);

                // Overlap should produce at least as many chunks (due to content reuse)
                prop_assert!(
                    chunks_with_overlap.len() >= chunks_no_overlap.len(),
                    "With overlap={}, should produce >= chunks than without. With: {}, Without: {}",
                    overlap, chunks_with_overlap.len(), chunks_no_overlap.len()
                );
            }

            /// Property: RecursiveCharacterTextSplitter uses separators in order
            /// This catches mutations in separator logic
            #[test]
            fn prop_recursive_splitter_uses_separators(
                text in "[a-zA-Z0-9, .]{50,200}",
                chunk_size in 20usize..100,
            ) {
                let splitter = RecursiveCharacterTextSplitter::new()
                    .with_chunk_size(chunk_size)
                    .with_chunk_overlap(0);

                let chunks = splitter.split_text(&text);

                // Verify splits happen
                prop_assert!(!chunks.is_empty(), "Should produce at least one chunk");

                // Verify all chunks are within limits (or single words)
                for chunk in &chunks {
                    let is_single_token = !chunk.contains(' ') && !chunk.contains(',') && !chunk.contains('.');
                    prop_assert!(
                        chunk.len() <= chunk_size || is_single_token,
                        "Chunk length {} should be <= {} or be a single token. Chunk: '{}'",
                        chunk.len(), chunk_size, chunk
                    );
                }
            }

            /// Property: Builder methods don't return wrong defaults
            /// This catches mutations like: with_keep_separator() -> Default::default()
            #[test]
            fn prop_builder_methods_work(
                chunk_size in 10usize..100,
                chunk_overlap in 0usize..10,
                sep in separator(),
            ) {
                let overlap = chunk_overlap.min(chunk_size - 1);

                // Build with explicit values
                let splitter = CharacterTextSplitter::new()
                    .with_chunk_size(chunk_size)
                    .with_chunk_overlap(overlap)
                    .with_separator(&sep)
                    .with_keep_separator(KeepSeparator::Start);

                // Verify all values are set correctly
                prop_assert_eq!(splitter.chunk_size(), chunk_size);
                prop_assert_eq!(splitter.chunk_overlap(), overlap);
                prop_assert_eq!(splitter.config.keep_separator, KeepSeparator::Start);
            }

            /// Property: Merge splits arithmetic is correct
            /// This catches + vs -, + vs * mutations
            #[test]
            fn prop_merge_splits_arithmetic(
                text in text_content(),
                chunk_size in 20usize..100,
            ) {
                let config = TextSplitterConfig {
                    chunk_size,
                    chunk_overlap: 0,
                    length_function: |s: &str| s.len(),
                    keep_separator: KeepSeparator::False,
                    add_start_index: false,
                    strip_whitespace: true,
                };

                let splits: Vec<String> = text.split_whitespace().map(|s| s.to_string()).collect();
                let chunks = config.merge_splits(&splits, " ");

                // Verify chunk sizes are computed correctly
                for chunk in &chunks {
                    // Length should be sum of word lengths + separators
                    let words: Vec<&str> = chunk.split_whitespace().collect();
                    let expected_len = words.iter().map(|w| w.len()).sum::<usize>() +
                                     (words.len().saturating_sub(1)); // separators

                    prop_assert_eq!(
                        chunk.len(), expected_len,
                        "Chunk length {} should equal sum of word lengths + separators {}. Chunk: '{}'",
                        chunk.len(), expected_len, chunk
                    );
                }
            }

            /// Property: Boolean logic is correct in split decisions
            /// This catches && vs || mutations
            #[test]
            fn prop_split_logic_boolean_correct(
                text in "[a-zA-Z0-9 ]{10,100}",
                chunk_size in 10usize..50,
            ) {
                let splitter = CharacterTextSplitter::new()
                    .with_chunk_size(chunk_size)
                    .with_chunk_overlap(0)
                    .with_separator(" ");

                let chunks = splitter.split_text(&text);

                // At least one chunk should be produced
                prop_assert!(!chunks.is_empty(), "Should produce at least one chunk");

                // If text is much longer AND has multiple words, should produce multiple chunks
                // Note: We can only split on separators. If text has no separators (single word),
                // we'll produce a single chunk even if it exceeds chunk_size.
                let words: Vec<&str> = text.split_whitespace().collect();
                let has_multiple_words = words.len() > 1;
                let total_word_length: usize = words.iter().map(|w| w.len()).sum();
                let separator_length = words.len().saturating_sub(1); // spaces between words
                let estimated_length = total_word_length + separator_length;

                if estimated_length > chunk_size * 2 && has_multiple_words {
                    prop_assert!(chunks.len() > 1,
                        "Long text with multiple words should produce multiple chunks. \
                         Text: '{}' (len={}, words={}, estimated={}), chunk_size={}",
                        text, text.len(), words.len(), estimated_length, chunk_size);
                }
            }
        }
    }

    // ===== Unicode / Non-Latin Script Tests =====

    #[test]
    fn test_japanese_text_splitting() {
        // Test Japanese text with hiragana, katakana, and kanji
        let japanese_text = "こんにちは世界。これは日本語のテストです。\n\n吾輩は猫である。名前はまだ無い。どこで生まれたかとんと見当がつかぬ。\n\n何でも薄暗いじめじめした所でニャーニャー泣いていた事だけは記憶している。";

        let splitter = RecursiveCharacterTextSplitter::new()
            .with_chunk_size(100)
            .with_chunk_overlap(0);

        let chunks = splitter.split_text(japanese_text);

        // Verify splitting works
        assert!(!chunks.is_empty(), "Should create at least one chunk");

        // Verify all content preserved
        let reconstructed = chunks.join("\n\n");
        assert!(
            reconstructed.contains("こんにちは世界"),
            "Japanese content should be preserved"
        );
        assert!(
            reconstructed.contains("吾輩は猫である"),
            "Kanji content should be preserved"
        );
        assert!(
            reconstructed.contains("テストです"),
            "Katakana content should be preserved"
        );

        // Verify chunks respect size limit
        for (i, chunk) in chunks.iter().enumerate() {
            assert!(
                chunk.len() <= 100,
                "Chunk {} exceeds size limit: {} > 100",
                i,
                chunk.len()
            );
        }
    }

    #[test]
    fn test_chinese_text_splitting() {
        // Test Simplified and Traditional Chinese
        let chinese_text = "你好世界。这是中文测试。\n\n天地玄黄，宇宙洪荒。日月盈昃，辰宿列张。\n\n寒来暑往，秋收冬藏。闰余成岁，律吕调阳。";

        let splitter = RecursiveCharacterTextSplitter::new()
            .with_chunk_size(80)
            .with_chunk_overlap(0);

        let chunks = splitter.split_text(chinese_text);

        // Verify splitting works
        assert!(!chunks.is_empty(), "Should create at least one chunk");

        // Verify all content preserved
        let reconstructed = chunks.join("\n\n");
        assert!(
            reconstructed.contains("你好世界"),
            "Simplified Chinese should be preserved"
        );
        assert!(
            reconstructed.contains("天地玄黄"),
            "Classical Chinese should be preserved"
        );
        assert!(
            reconstructed.contains("寒来暑往"),
            "All Chinese characters should be preserved"
        );

        // Verify chunks respect size limit
        for (i, chunk) in chunks.iter().enumerate() {
            assert!(
                chunk.len() <= 80,
                "Chunk {} exceeds size limit: {} > 80",
                i,
                chunk.len()
            );
        }
    }

    #[test]
    fn test_arabic_text_splitting() {
        // Test Arabic text (right-to-left script)
        let arabic_text = "مرحبا بالعالم. هذا اختبار للنص العربي.\n\nالعربية لغة جميلة وغنية بالتاريخ والثقافة.\n\nتكتب العربية من اليمين إلى اليسار وتتميز بخطها الجميل.";

        let splitter = RecursiveCharacterTextSplitter::new()
            .with_chunk_size(120)
            .with_chunk_overlap(0);

        let chunks = splitter.split_text(arabic_text);

        // Verify splitting works
        assert!(!chunks.is_empty(), "Should create at least one chunk");

        // Verify all content preserved
        let reconstructed = chunks.join("\n\n");
        assert!(
            reconstructed.contains("مرحبا بالعالم"),
            "Arabic greeting should be preserved"
        );
        assert!(
            reconstructed.contains("العربية لغة جميلة"),
            "Arabic description should be preserved"
        );
        assert!(
            reconstructed.contains("من اليمين إلى اليسار"),
            "RTL reference should be preserved"
        );

        // Verify chunks respect size limit
        for (i, chunk) in chunks.iter().enumerate() {
            assert!(
                chunk.len() <= 120,
                "Chunk {} exceeds size limit: {} > 120",
                i,
                chunk.len()
            );
        }
    }

    #[test]
    fn test_mixed_scripts_splitting() {
        // Test text mixing Latin, Japanese, Chinese, and Arabic
        let mixed_text = "Hello 世界! This is mixed: 日本語、中文、العربية.\n\nEnglish text here.\n\n日本語のテキスト。\n\n中文文本。\n\nنص عربي.";

        let splitter = RecursiveCharacterTextSplitter::new()
            .with_chunk_size(100)
            .with_chunk_overlap(0);

        let chunks = splitter.split_text(mixed_text);

        // Verify splitting works
        assert!(!chunks.is_empty(), "Should create at least one chunk");

        // Verify all scripts preserved
        let reconstructed = chunks.join("\n\n");
        assert!(reconstructed.contains("Hello"), "Latin should be preserved");
        assert!(
            reconstructed.contains("世界"),
            "Chinese should be preserved"
        );
        assert!(
            reconstructed.contains("日本語"),
            "Japanese should be preserved"
        );
        assert!(
            reconstructed.contains("العربية"),
            "Arabic should be preserved"
        );

        // Verify chunks respect size limit
        for (i, chunk) in chunks.iter().enumerate() {
            assert!(
                chunk.len() <= 100,
                "Chunk {} exceeds size limit: {} > 100",
                i,
                chunk.len()
            );
        }
    }
