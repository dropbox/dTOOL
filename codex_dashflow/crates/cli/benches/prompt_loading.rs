//! Criterion-based benchmarks for prompt loading operations
//!
//! Run with: cargo bench -p codex-dashflow-cli --bench prompt_loading
//!
//! These benchmarks measure the performance of prompt loading and validation
//! operations in the CLI, helping identify potential bottlenecks.

use std::io::Write;
use std::path::PathBuf;

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};

use codex_dashflow_cli::{resolve_system_prompt, validate_prompt, MAX_PROMPT_LENGTH};

// ============================================================================
// Prompt Validation Benchmarks
// ============================================================================

fn bench_validate_prompt_short(c: &mut Criterion) {
    let prompt = "List files in the current directory";
    c.bench_function("validate_prompt_short", |b| {
        b.iter(|| validate_prompt(prompt))
    });
}

fn bench_validate_prompt_medium(c: &mut Criterion) {
    // Typical prompt size: ~500 chars
    let prompt = r#"
        Please help me refactor this Rust code to be more idiomatic.
        I want to:
        1. Use proper error handling with Result types
        2. Apply the builder pattern for configuration
        3. Add documentation comments
        4. Implement Display and Debug traits
        5. Use async/await where appropriate

        Here is the code to refactor:

        fn process_data(data: Vec<u8>) -> Option<String> {
            if data.is_empty() { return None; }
            Some(String::from_utf8_lossy(&data).to_string())
        }
    "#;
    c.bench_function("validate_prompt_medium", |b| {
        b.iter(|| validate_prompt(prompt))
    });
}

fn bench_validate_prompt_long(c: &mut Criterion) {
    // Large prompt: ~10KB
    let prompt = "x".repeat(10_000);
    c.bench_function("validate_prompt_long", |b| {
        b.iter(|| validate_prompt(&prompt))
    });
}

fn bench_validate_prompt_at_limit(c: &mut Criterion) {
    // Maximum valid prompt size
    let prompt = "x".repeat(MAX_PROMPT_LENGTH);
    c.bench_function("validate_prompt_at_limit", |b| {
        b.iter(|| validate_prompt(&prompt))
    });
}

fn bench_validate_prompt_over_limit(c: &mut Criterion) {
    // Over maximum - should fail fast
    let prompt = "x".repeat(MAX_PROMPT_LENGTH + 1);
    c.bench_function("validate_prompt_over_limit", |b| {
        b.iter(|| validate_prompt(&prompt))
    });
}

fn bench_validate_prompt_scaling(c: &mut Criterion) {
    let mut group = c.benchmark_group("validate_prompt_scaling");

    for size in [100, 1_000, 10_000, 50_000, 100_000].iter() {
        let prompt = "x".repeat(*size);
        group.throughput(Throughput::Bytes(*size as u64));
        group.bench_with_input(BenchmarkId::from_parameter(size), &prompt, |b, p| {
            b.iter(|| validate_prompt(p))
        });
    }

    group.finish();
}

// ============================================================================
// File-based Prompt Loading Benchmarks
// ============================================================================

/// Helper to create a temporary file with content for benchmarking
fn create_temp_prompt_file(content: &str) -> PathBuf {
    let temp_dir = std::env::temp_dir();
    let file_path = temp_dir.join(format!("bench_prompt_{}.txt", std::process::id()));
    let mut file = std::fs::File::create(&file_path).unwrap();
    file.write_all(content.as_bytes()).unwrap();
    file_path
}

fn bench_resolve_system_prompt_inline(c: &mut Criterion) {
    c.bench_function("resolve_system_prompt_inline", |b| {
        b.iter(|| resolve_system_prompt(Some("You are a helpful assistant."), None))
    });
}

fn bench_resolve_system_prompt_from_file_small(c: &mut Criterion) {
    let content = "You are a helpful Rust programming assistant.";
    let file_path = create_temp_prompt_file(content);

    c.bench_function("resolve_system_prompt_from_file_small", |b| {
        b.iter(|| resolve_system_prompt(None, Some(&file_path)))
    });

    // Cleanup
    let _ = std::fs::remove_file(&file_path);
}

fn bench_resolve_system_prompt_from_file_medium(c: &mut Criterion) {
    // ~1KB system prompt
    let content = r#"
You are an expert Rust programming assistant with deep knowledge of:

## Core Rust Concepts
- Ownership and borrowing
- Lifetimes and references
- Traits and generics
- Error handling with Result and Option
- Pattern matching
- Async/await and futures

## Best Practices
- Write idiomatic Rust code
- Follow the official Rust style guide
- Use clippy for linting
- Document public APIs
- Write comprehensive tests

## Libraries and Ecosystem
- Tokio for async runtime
- Serde for serialization
- Clap for CLI argument parsing
- Tracing for logging

When helping users:
1. Explain your reasoning
2. Provide working code examples
3. Point out potential issues
4. Suggest improvements
"#;
    let file_path = create_temp_prompt_file(content);

    c.bench_function("resolve_system_prompt_from_file_medium", |b| {
        b.iter(|| resolve_system_prompt(None, Some(&file_path)))
    });

    // Cleanup
    let _ = std::fs::remove_file(&file_path);
}

fn bench_resolve_system_prompt_from_file_large(c: &mut Criterion) {
    // ~10KB system prompt
    let content = "x".repeat(10_000);
    let file_path = create_temp_prompt_file(&content);

    c.bench_function("resolve_system_prompt_from_file_large", |b| {
        b.iter(|| resolve_system_prompt(None, Some(&file_path)))
    });

    // Cleanup
    let _ = std::fs::remove_file(&file_path);
}

fn bench_system_prompt_file_scaling(c: &mut Criterion) {
    let mut group = c.benchmark_group("system_prompt_file_scaling");

    // Create temp files of different sizes
    let sizes = [100, 1_000, 5_000, 10_000, 50_000];
    let temp_files: Vec<(usize, PathBuf)> = sizes
        .iter()
        .map(|&size| {
            let content = "x".repeat(size);
            let path = create_temp_prompt_file(&content);
            (size, path)
        })
        .collect();

    for (size, path) in &temp_files {
        group.throughput(Throughput::Bytes(*size as u64));
        group.bench_with_input(BenchmarkId::from_parameter(size), path, |b, p| {
            b.iter(|| resolve_system_prompt(None, Some(p)))
        });
    }

    group.finish();

    // Cleanup
    for (_, path) in temp_files {
        let _ = std::fs::remove_file(path);
    }
}

// ============================================================================
// Prompt File I/O Benchmarks
// ============================================================================

fn bench_prompt_file_read_small(c: &mut Criterion) {
    let content = "Simple prompt";
    let file_path = create_temp_prompt_file(content);

    c.bench_function("prompt_file_read_small", |b| {
        b.iter(|| std::fs::read_to_string(&file_path))
    });

    let _ = std::fs::remove_file(&file_path);
}

fn bench_prompt_file_read_large(c: &mut Criterion) {
    let content = "x".repeat(50_000);
    let file_path = create_temp_prompt_file(&content);

    c.bench_function("prompt_file_read_large", |b| {
        b.iter(|| std::fs::read_to_string(&file_path))
    });

    let _ = std::fs::remove_file(&file_path);
}

// ============================================================================
// Inline vs File Comparison
// ============================================================================

fn bench_prompt_source_comparison(c: &mut Criterion) {
    let prompt_content = "You are a helpful coding assistant with expertise in Rust.";
    let file_path = create_temp_prompt_file(prompt_content);

    let mut group = c.benchmark_group("prompt_source_comparison");

    // Inline prompt (no file I/O)
    group.bench_function("inline", |b| {
        b.iter(|| resolve_system_prompt(Some(prompt_content), None))
    });

    // From file (requires I/O)
    group.bench_function("from_file", |b| {
        b.iter(|| resolve_system_prompt(None, Some(&file_path)))
    });

    group.finish();

    let _ = std::fs::remove_file(&file_path);
}

// ============================================================================
// Whitespace Trimming Benchmarks
// ============================================================================

fn bench_prompt_with_whitespace(c: &mut Criterion) {
    // Prompts with leading/trailing whitespace (common from files)
    let content = format!(
        "\n\n  {}  \n\n",
        "You are a helpful assistant that writes clean code."
    );
    let file_path = create_temp_prompt_file(&content);

    c.bench_function("resolve_prompt_with_whitespace", |b| {
        b.iter(|| resolve_system_prompt(None, Some(&file_path)))
    });

    let _ = std::fs::remove_file(&file_path);
}

fn bench_prompt_heavy_whitespace(c: &mut Criterion) {
    // Edge case: lots of whitespace
    let content = format!(
        "{}\nActual prompt content\n{}",
        " ".repeat(1000),
        " ".repeat(1000)
    );
    let file_path = create_temp_prompt_file(&content);

    c.bench_function("resolve_prompt_heavy_whitespace", |b| {
        b.iter(|| resolve_system_prompt(None, Some(&file_path)))
    });

    let _ = std::fs::remove_file(&file_path);
}

// ============================================================================
// Criterion Groups
// ============================================================================

criterion_group!(
    validation_benches,
    bench_validate_prompt_short,
    bench_validate_prompt_medium,
    bench_validate_prompt_long,
    bench_validate_prompt_at_limit,
    bench_validate_prompt_over_limit,
    bench_validate_prompt_scaling,
);

criterion_group!(
    file_loading_benches,
    bench_resolve_system_prompt_inline,
    bench_resolve_system_prompt_from_file_small,
    bench_resolve_system_prompt_from_file_medium,
    bench_resolve_system_prompt_from_file_large,
    bench_system_prompt_file_scaling,
);

criterion_group!(
    io_benches,
    bench_prompt_file_read_small,
    bench_prompt_file_read_large,
);

criterion_group!(comparison_benches, bench_prompt_source_comparison,);

criterion_group!(
    whitespace_benches,
    bench_prompt_with_whitespace,
    bench_prompt_heavy_whitespace,
);

criterion_main!(
    validation_benches,
    file_loading_benches,
    io_benches,
    comparison_benches,
    whitespace_benches,
);
