/// Comparison test program for validating Rust text splitters against Python baseline
///
/// This generates test outputs that can be manually compared with Python's behavior.
/// Since the separators are ported directly from Python, the behavior should be identical.
use dashflow_text_splitters::{
    CharacterTextSplitter, HTMLTextSplitter, MarkdownTextSplitter, RecursiveCharacterTextSplitter,
    TextSplitter,
};

fn main() {
    println!("{}", "=".repeat(70));
    println!("Rust Text Splitter Validation Output");
    println!("{}", "=".repeat(70));

    test_character_splitter();
    test_recursive_splitter();
    test_markdown_splitter();
    test_html_splitter();

    println!("\n{}", "=".repeat(70));
    println!("Validation complete!");
    println!("{}", "=".repeat(70));
    println!("\nNext steps:");
    println!("1. Compare this output with Python baseline");
    println!("2. Verify separator behavior matches");
    println!("3. Document any differences (if any)");
}

fn test_character_splitter() {
    println!("\n=== CharacterTextSplitter Tests ===\n");

    let text = "Line 1\n\nLine 2\n\nLine 3\n\nLine 4";

    // Test 1: Basic double newline separator
    println!("Test 1 - Basic double newline:");
    println!("Input: {:?}", text);
    println!("Chunk size: 20, Overlap: 0");

    let splitter = CharacterTextSplitter::new()
        .with_separator("\n\n")
        .with_chunk_size(20)
        .with_chunk_overlap(0);

    let chunks = splitter.split_text(text);
    println!("Output chunks ({}):", chunks.len());
    for (i, chunk) in chunks.iter().enumerate() {
        println!("  [{}] ({} chars): {:?}", i, chunk.len(), chunk);
    }

    // Test 2: With overlap
    println!("\nTest 2 - With overlap:");
    println!("Input: {:?}", text);
    println!("Chunk size: 20, Overlap: 5");

    let splitter = CharacterTextSplitter::new()
        .with_separator("\n\n")
        .with_chunk_size(20)
        .with_chunk_overlap(5);

    let chunks = splitter.split_text(text);
    println!("Output chunks ({}):", chunks.len());
    for (i, chunk) in chunks.iter().enumerate() {
        println!("  [{}] ({} chars): {:?}", i, chunk.len(), chunk);
    }
}

fn test_recursive_splitter() {
    println!("\n=== RecursiveCharacterTextSplitter Tests ===\n");

    let text =
        "This is paragraph one.\n\nThis is paragraph two with more content.\n\nParagraph three.";

    // Test 1: Default separators
    println!("Test 1 - Default separators:");
    println!("Input: {:?}", text);
    println!("Chunk size: 30, Overlap: 5");

    let splitter = RecursiveCharacterTextSplitter::new()
        .with_chunk_size(30)
        .with_chunk_overlap(5);

    let chunks = splitter.split_text(text);
    println!("Separators: {:?}", vec!["\n\n", "\n", " ", ""]);
    println!("Output chunks ({}):", chunks.len());
    for (i, chunk) in chunks.iter().enumerate() {
        println!("  [{}] ({} chars): {:?}", i, chunk.len(), chunk);
    }

    // Test 2: Small chunk size forces word-level splitting
    println!("\nTest 2 - Small chunks (word-level):");
    println!("Input: {:?}", text);
    println!("Chunk size: 15, Overlap: 3");

    let splitter = RecursiveCharacterTextSplitter::new()
        .with_chunk_size(15)
        .with_chunk_overlap(3);

    let chunks = splitter.split_text(text);
    println!("Output chunks ({}):", chunks.len());
    for (i, chunk) in chunks.iter().enumerate() {
        println!("  [{}] ({} chars): {:?}", i, chunk.len(), chunk);
    }
}

fn test_markdown_splitter() {
    println!("\n=== MarkdownTextSplitter Tests ===\n");

    let text = r#"# Header 1

Some content under header 1.

## Header 2

Content under header 2.

```python
def hello():
    print("Hello")
```

More text after code block.

### Header 3

Final content."#;

    // Test 1: Basic markdown splitting
    println!("Test 1 - Basic markdown:");
    println!("Input length: {} chars", text.len());
    println!("Chunk size: 100, Overlap: 20");

    let splitter = MarkdownTextSplitter::new()
        .with_chunk_size(100)
        .with_chunk_overlap(20);

    let chunks = splitter.split_text(text);
    println!("Separators: [r\"\\n#{{1,6}} \", \"```\\n\", r\"\\n\\*\\*\\*+\\n\", ...]");
    println!("Output chunks ({}):", chunks.len());
    for (i, chunk) in chunks.iter().enumerate() {
        let preview = if chunk.len() > 80 {
            format!("{}...", &chunk[..80].replace('\n', "\\n"))
        } else {
            chunk.replace('\n', "\\n")
        };
        println!("  [{}] ({} chars):", i, chunk.len());
        println!("    {:?}", preview);
    }

    // Test 2: Smaller chunks
    println!("\nTest 2 - Smaller chunks:");
    println!("Chunk size: 50, Overlap: 10");

    let splitter = MarkdownTextSplitter::new()
        .with_chunk_size(50)
        .with_chunk_overlap(10);

    let chunks = splitter.split_text(text);
    println!("Output chunks ({}):", chunks.len());
    for (i, chunk) in chunks.iter().enumerate() {
        let preview = if chunk.len() > 50 {
            format!("{}...", &chunk[..50].replace('\n', "\\n"))
        } else {
            chunk.replace('\n', "\\n")
        };
        println!("  [{}] ({} chars): {:?}", i, chunk.len(), preview);
    }
}

fn test_html_splitter() {
    println!("\n=== HTMLTextSplitter Tests ===\n");

    let text = r#"<html>
<head><title>Test Page</title></head>
<body>
<h1>Main Header</h1>
<p>First paragraph with some content.</p>
<div>
<h2>Subheader</h2>
<p>Second paragraph inside a div.</p>
<ul>
<li>List item 1</li>
<li>List item 2</li>
</ul>
</div>
</body>
</html>"#;

    // Test 1: Basic HTML splitting
    println!("Test 1 - Basic HTML:");
    println!("Input length: {} chars", text.len());
    println!("Chunk size: 100, Overlap: 20");

    let splitter = HTMLTextSplitter::new()
        .with_chunk_size(100)
        .with_chunk_overlap(20);

    let chunks = splitter.split_text(text);
    println!("Separators: [\"<body\", \"<div\", \"<p\", \"<br\", \"<li\", ...]");
    println!("Output chunks ({}):", chunks.len());
    for (i, chunk) in chunks.iter().enumerate() {
        let preview = if chunk.len() > 80 {
            format!("{}...", &chunk[..80].replace('\n', "\\n"))
        } else {
            chunk.replace('\n', "\\n")
        };
        println!("  [{}] ({} chars):", i, chunk.len());
        println!("    {:?}", preview);
    }

    // Test 2: Smaller chunks
    println!("\nTest 2 - Smaller chunks:");
    println!("Chunk size: 60, Overlap: 10");

    let splitter = HTMLTextSplitter::new()
        .with_chunk_size(60)
        .with_chunk_overlap(10);

    let chunks = splitter.split_text(text);
    println!("Output chunks ({}):", chunks.len());
    for (i, chunk) in chunks.iter().enumerate() {
        let preview = if chunk.len() > 60 {
            format!("{}...", &chunk[..60].replace('\n', "\\n"))
        } else {
            chunk.replace('\n', "\\n")
        };
        println!("  [{}] ({} chars): {:?}", i, chunk.len(), preview);
    }
}
