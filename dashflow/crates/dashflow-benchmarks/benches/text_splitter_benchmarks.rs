//! Performance benchmarks for text splitters
//!
//! Run with: cargo bench -p dashflow-benchmarks --bench text_splitter_benchmarks

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use dashflow_text_splitters::{
    CharacterTextSplitter, HTMLTextSplitter, MarkdownTextSplitter, RecursiveCharacterTextSplitter,
    TextSplitter,
};

// ============================================================================
// Character Text Splitter Benchmarks
// ============================================================================

fn bench_character_splitter(c: &mut Criterion) {
    let mut group = c.benchmark_group("character_splitter");

    // Small text (100 chars)
    let small_text = "Lorem ipsum dolor sit amet, consectetur adipiscing elit. Sed do eiusmod tempor incididunt ut labore.";

    group.bench_function("split_small_chunk100_overlap20", |b| {
        let splitter = CharacterTextSplitter::new()
            .with_chunk_size(100)
            .with_chunk_overlap(20);
        b.iter(|| splitter.split_text(small_text));
    });

    // Medium text (~5KB)
    let medium_text = "Lorem ipsum dolor sit amet, consectetur adipiscing elit. Sed do eiusmod tempor incididunt ut labore et dolore magna aliqua.\n\n".repeat(50);

    group.bench_function("split_medium_chunk1000_overlap200", |b| {
        let splitter = CharacterTextSplitter::new()
            .with_chunk_size(1000)
            .with_chunk_overlap(200);
        b.iter(|| splitter.split_text(&medium_text));
    });

    // Large text (~50KB)
    let large_text = "Lorem ipsum dolor sit amet, consectetur adipiscing elit. Sed do eiusmod tempor incididunt ut labore et dolore magna aliqua.\n\n".repeat(500);

    group.bench_function("split_large_chunk1000_overlap200", |b| {
        let splitter = CharacterTextSplitter::new()
            .with_chunk_size(1000)
            .with_chunk_overlap(200);
        b.iter(|| splitter.split_text(&large_text));
    });

    // Test different chunk sizes (medium text)
    for chunk_size in [500, 1000, 2000, 4000].iter() {
        group.bench_with_input(
            BenchmarkId::new("split_medium_chunk", chunk_size),
            chunk_size,
            |b, &chunk_size| {
                let splitter = CharacterTextSplitter::new()
                    .with_chunk_size(chunk_size)
                    .with_chunk_overlap(chunk_size / 10);
                b.iter(|| splitter.split_text(&medium_text));
            },
        );
    }

    group.finish();
}

// ============================================================================
// Recursive Character Text Splitter Benchmarks
// ============================================================================

fn bench_recursive_splitter(c: &mut Criterion) {
    let mut group = c.benchmark_group("recursive_splitter");

    // Small text with structure
    let small_text = "# Heading\n\nParagraph 1.\n\nParagraph 2.\n\n## Subheading\n\nMore content.";

    group.bench_function("split_small_recursive", |b| {
        let splitter = RecursiveCharacterTextSplitter::new()
            .with_chunk_size(100)
            .with_chunk_overlap(20);
        b.iter(|| splitter.split_text(small_text));
    });

    // Medium text with nested structure
    let medium_text = format!(
        "# Main Heading\n\n{}\n\n## Section 1\n\n{}\n\n## Section 2\n\n{}",
        "Lorem ipsum dolor sit amet, consectetur adipiscing elit.\n\n".repeat(10),
        "Sed do eiusmod tempor incididunt ut labore et dolore magna aliqua.\n\n".repeat(10),
        "Ut enim ad minim veniam, quis nostrud exercitation ullamco.\n\n".repeat(10)
    );

    group.bench_function("split_medium_recursive", |b| {
        let splitter = RecursiveCharacterTextSplitter::new()
            .with_chunk_size(1000)
            .with_chunk_overlap(200);
        b.iter(|| splitter.split_text(&medium_text));
    });

    // Large text (~50KB) with deep nesting
    let large_text = format!(
        "# Chapter 1\n\n{}\n\n## Section 1.1\n\n{}\n\n### Subsection 1.1.1\n\n{}\n\n# Chapter 2\n\n{}",
        "Lorem ipsum dolor sit amet.\n\n".repeat(100),
        "Consectetur adipiscing elit.\n\n".repeat(100),
        "Sed do eiusmod tempor.\n\n".repeat(100),
        "Incididunt ut labore.\n\n".repeat(100)
    );

    group.bench_function("split_large_recursive", |b| {
        let splitter = RecursiveCharacterTextSplitter::new()
            .with_chunk_size(1000)
            .with_chunk_overlap(200);
        b.iter(|| splitter.split_text(&large_text));
    });

    group.finish();
}

// ============================================================================
// Markdown Text Splitter Benchmarks
// ============================================================================

fn bench_markdown_splitter(c: &mut Criterion) {
    let mut group = c.benchmark_group("markdown_splitter");

    let markdown_text = r#"# Main Title

This is an introduction paragraph.

## Section 1

Content for section 1 with **bold** and *italic* text.

```python
def hello():
    print("Hello, world!")
```

## Section 2

- Bullet point 1
- Bullet point 2
- Bullet point 3

### Subsection 2.1

More detailed content here.

## Section 3

Final section content.
"#;

    group.bench_function("split_markdown_small", |b| {
        let splitter = MarkdownTextSplitter::new()
            .with_chunk_size(500)
            .with_chunk_overlap(50);
        b.iter(|| splitter.split_text(markdown_text));
    });

    // Larger markdown document
    let large_markdown = format!(
        "{}\n\n{}\n\n{}\n\n{}",
        markdown_text, markdown_text, markdown_text, markdown_text
    );

    group.bench_function("split_markdown_large", |b| {
        let splitter = MarkdownTextSplitter::new()
            .with_chunk_size(1000)
            .with_chunk_overlap(200);
        b.iter(|| splitter.split_text(&large_markdown));
    });

    group.finish();
}

// ============================================================================
// HTML Text Splitter Benchmarks
// ============================================================================

fn bench_html_splitter(c: &mut Criterion) {
    let mut group = c.benchmark_group("html_splitter");

    let html_text = r#"<html>
<head><title>Test Page</title></head>
<body>
    <h1>Main Heading</h1>
    <p>This is a paragraph with some content.</p>

    <h2>Section 1</h2>
    <p>More content here with <strong>bold</strong> and <em>italic</em> text.</p>

    <div class="content">
        <h3>Subsection</h3>
        <p>Nested content in a div.</p>
    </div>

    <h2>Section 2</h2>
    <ul>
        <li>List item 1</li>
        <li>List item 2</li>
        <li>List item 3</li>
    </ul>
</body>
</html>"#;

    group.bench_function("split_html_small", |b| {
        let splitter = HTMLTextSplitter::new()
            .with_chunk_size(500)
            .with_chunk_overlap(50);
        b.iter(|| splitter.split_text(html_text));
    });

    // Larger HTML document
    let large_html = format!(
        "<html><body>{}{}{}</body></html>",
        html_text, html_text, html_text
    );

    group.bench_function("split_html_large", |b| {
        let splitter = HTMLTextSplitter::new()
            .with_chunk_size(1000)
            .with_chunk_overlap(200);
        b.iter(|| splitter.split_text(&large_html));
    });

    group.finish();
}

// ============================================================================
// Overlap Performance Benchmarks
// ============================================================================

fn bench_overlap_impact(c: &mut Criterion) {
    let mut group = c.benchmark_group("overlap_impact");

    let text = "Lorem ipsum dolor sit amet, consectetur adipiscing elit. Sed do eiusmod tempor incididunt ut labore.\n\n".repeat(100);

    // Test different overlap ratios
    for overlap_pct in [0, 10, 20, 50].iter() {
        let chunk_size = 1000;
        let overlap = (chunk_size * overlap_pct) / 100;

        group.bench_with_input(
            BenchmarkId::new("overlap_pct", overlap_pct),
            overlap_pct,
            |b, _| {
                let splitter = CharacterTextSplitter::new()
                    .with_chunk_size(chunk_size)
                    .with_chunk_overlap(overlap);
                b.iter(|| splitter.split_text(&text));
            },
        );
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_character_splitter,
    bench_recursive_splitter,
    bench_markdown_splitter,
    bench_html_splitter,
    bench_overlap_impact,
);
criterion_main!(benches);
