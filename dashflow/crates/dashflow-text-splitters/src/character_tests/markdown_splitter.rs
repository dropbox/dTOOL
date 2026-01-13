use super::*;

    #[test]
    fn test_markdown_splitter_basic() {
        let splitter = MarkdownTextSplitter::new()
            .with_chunk_size(100)
            .with_chunk_overlap(20);

        let markdown = "# Header 1\n\nSome content here.\n\n## Header 2\n\nMore content under header 2.\n\n### Header 3\n\nAnd even more content.";
        let chunks = splitter.split_text(markdown);

        // Exact chunk count validation (splits on markdown headers with chunk size constraints)
        assert_eq!(
            chunks.len(),
            2,
            "Expected 2 chunks with chunk_size=100, overlap=20"
        );

        // Exact chunk content validation
        assert_eq!(
            chunks[0],
            "# Header 1\n\nSome content here.\n\n## Header 2\n\nMore content under header 2.",
            "Chunk 0 should contain first 3 sections (73 chars, under limit)"
        );
        assert_eq!(
            chunks[1], "### Header 3\n\nAnd even more content.",
            "Chunk 1 should contain Header 3 section (36 chars, under limit)"
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

        // Verify markdown structure preservation
        assert!(
            chunks[0].contains("# Header 1"),
            "Chunk 0 contains H1 header"
        );
        assert!(
            chunks[0].contains("## Header 2"),
            "Chunk 0 contains H2 header"
        );
        assert!(
            chunks[1].contains("### Header 3"),
            "Chunk 1 contains H3 header"
        );

        // Verify content preservation (all paragraphs present)
        assert!(
            chunks[0].contains("Some content here"),
            "Content under Header 1 preserved"
        );
        assert!(
            chunks[0].contains("More content under header 2"),
            "Content under Header 2 preserved"
        );
        assert!(
            chunks[1].contains("And even more content"),
            "Content under Header 3 preserved"
        );

        // Verify no empty chunks
        for (i, chunk) in chunks.iter().enumerate() {
            assert!(!chunk.trim().is_empty(), "Chunk {} is not empty", i);
        }

        // Verify separators work correctly (splits on headers, not arbitrary positions)
        // Headers should be at start of chunks (except when grouped due to size)
        assert!(
            chunks[1].starts_with("###"),
            "Chunk 1 starts with header marker"
        );
    }

    #[test]
    fn test_markdown_splitter_code_blocks() {
        let splitter = MarkdownTextSplitter::new()
            .with_chunk_size(200)
            .with_chunk_overlap(20);

        let markdown = "# Code Example\n\nHere is some code:\n\n```rust\nfn main() {\n    println!(\"Hello\");\n}\n```\n\nThat was the code.";
        let chunks = splitter.split_text(markdown);

        // Exact chunk count validation (entire markdown fits in one chunk)
        assert_eq!(
            chunks.len(),
            1,
            "Expected 1 chunk with chunk_size=200 (markdown len=104)"
        );

        // Exact chunk content validation
        let expected = "# Code Example\n\nHere is some code:\n\n```rust\nfn main() {\n    println!(\"Hello\");\n}\n```\n\nThat was the code.";
        assert_eq!(
            chunks[0], expected,
            "Chunk should contain entire markdown (104 chars, under 200 limit)"
        );

        // Verify chunk size constraint
        assert!(
            chunks[0].len() <= 200,
            "Chunk within size: {} <= 200",
            chunks[0].len()
        );

        // Verify code block structure preservation
        assert!(
            chunks[0].contains("```rust"),
            "Code block opening fence with language preserved"
        );
        assert!(
            chunks[0].contains("```\n\nThat was"),
            "Code block closing fence preserved"
        );
        assert!(chunks[0].contains("fn main()"), "Code content preserved");
        assert!(
            chunks[0].contains("println!(\"Hello\")"),
            "Code function call preserved"
        );

        // Verify markdown structure preservation
        assert!(chunks[0].contains("# Code Example"), "Header preserved");
        assert!(
            chunks[0].contains("Here is some code:"),
            "Intro text preserved"
        );
        assert!(
            chunks[0].contains("That was the code."),
            "Closing text preserved"
        );

        // Verify code block is not split (critical for code integrity)
        let fence_count = chunks[0].matches("```").count();
        assert_eq!(
            fence_count, 2,
            "Code block has both opening and closing fences (not split)"
        );
    }

