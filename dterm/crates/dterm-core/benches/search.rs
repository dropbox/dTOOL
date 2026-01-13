//! Search benchmarks.
//!
//! ## Running
//!
//! ```bash
//! cargo bench --bench search
//! ```
//!
//! ## Metrics
//!
//! - `search/index_line`: Time to index a single line
//! - `search/search_10k`: Search 10K lines
//! - `search/search_100k`: Search 100K lines
//! - `search/search_1m`: Search 1M lines
//! - `search/bloom_check`: Bloom filter negative lookup
//! - `search/find_next`: Find next match

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use dterm_core::search::{BloomFilter, SearchDirection, SearchIndex, TerminalSearch};

/// Generate test lines that simulate realistic terminal output.
fn generate_test_lines(count: usize) -> Vec<String> {
    let templates = [
        "INFO: Processing request from user_{} at timestamp {}",
        "DEBUG: Cache hit for key {} with value {}",
        "WARN: Connection timeout after {} ms for host {}",
        "ERROR: Failed to parse response: {} at line {}",
        "$ git status",
        "$ cargo build --release",
        "Compiling dterm-core v0.1.0",
        "Running `target/release/dterm`",
        "test result: ok. {} passed; {} failed",
        "error[E0382]: borrow of moved value: `x`",
    ];

    (0..count)
        .map(|i| {
            let template = templates[i % templates.len()];
            template.replacen("{}", &i.to_string(), 1).replacen(
                "{}",
                &(i * 17 % 1000).to_string(),
                1,
            )
        })
        .collect()
}

fn bench_index_line(c: &mut Criterion) {
    let lines = generate_test_lines(1000);
    let mut group = c.benchmark_group("search/index");

    group.throughput(Throughput::Elements(1));
    group.bench_function("single_line", |b| {
        let mut index = SearchIndex::new();
        let mut i = 0;
        b.iter(|| {
            index.index_line(i, &lines[i % lines.len()]);
            i += 1;
        });
    });

    group.throughput(Throughput::Elements(1000));
    group.bench_function("1k_lines", |b| {
        b.iter(|| {
            let mut index = SearchIndex::new();
            for (i, line) in lines.iter().enumerate() {
                index.index_line(i, line);
            }
            black_box(&index);
        });
    });

    group.finish();
}

fn bench_search_scaling(c: &mut Criterion) {
    let mut group = c.benchmark_group("search/query");

    for size in [10_000, 100_000, 1_000_000] {
        // Pre-build the index
        let lines = generate_test_lines(size);
        let mut index = SearchIndex::with_capacity(size);
        for (i, line) in lines.iter().enumerate() {
            index.index_line(i, line);
        }

        group.throughput(Throughput::Elements(size as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}k_lines", size / 1000)),
            &index,
            |b, index| {
                b.iter(|| {
                    // Search for a common term
                    let results: Vec<_> = index.search("Processing").collect();
                    black_box(results)
                });
            },
        );
    }

    group.finish();
}

fn bench_search_with_positions(c: &mut Criterion) {
    let mut group = c.benchmark_group("search/positions");

    let size = 100_000;
    let lines = generate_test_lines(size);
    let mut index = SearchIndex::with_capacity(size);
    for (i, line) in lines.iter().enumerate() {
        index.index_line(i, line);
    }

    group.bench_function("100k_lines", |b| {
        b.iter(|| {
            let results = index.search_with_positions("Processing");
            black_box(results)
        });
    });

    group.finish();
}

fn bench_bloom_filter(c: &mut Criterion) {
    let mut group = c.benchmark_group("search/bloom");

    // Build bloom filter with 100K trigrams
    let lines = generate_test_lines(100_000);
    let mut bloom = BloomFilter::with_capacity(100_000);
    for line in &lines {
        for window in line.as_bytes().windows(3) {
            bloom.insert_bytes(window);
        }
    }

    group.bench_function("positive_check", |b| {
        b.iter(|| {
            // Check for trigram that exists
            black_box(bloom.might_contain("Pro"))
        });
    });

    group.bench_function("negative_check", |b| {
        b.iter(|| {
            // Check for trigram that likely doesn't exist
            black_box(bloom.might_contain("xyz"))
        });
    });

    group.finish();
}

fn bench_terminal_search(c: &mut Criterion) {
    let mut group = c.benchmark_group("search/terminal");

    let size = 100_000;
    let lines = generate_test_lines(size);
    let mut search = TerminalSearch::with_capacity(size);
    for line in &lines {
        search.index_scrollback_line(line);
    }

    group.bench_function("find_next", |b| {
        b.iter(|| black_box(search.find_next("ERROR", 50_000, 0)));
    });

    group.bench_function("find_prev", |b| {
        b.iter(|| black_box(search.find_prev("ERROR", 50_000, 0)));
    });

    group.bench_function("search_ordered_forward", |b| {
        b.iter(|| black_box(search.search_ordered("Processing", SearchDirection::Forward)));
    });

    group.bench_function("search_ordered_backward", |b| {
        b.iter(|| black_box(search.search_ordered("Processing", SearchDirection::Backward)));
    });

    group.finish();
}

fn bench_might_contain(c: &mut Criterion) {
    let mut group = c.benchmark_group("search/might_contain");

    let size = 100_000;
    let lines = generate_test_lines(size);
    let mut index = SearchIndex::with_capacity(size);
    for (i, line) in lines.iter().enumerate() {
        index.index_line(i, line);
    }

    group.bench_function("positive", |b| {
        b.iter(|| black_box(index.might_contain("Processing")));
    });

    group.bench_function("negative", |b| {
        b.iter(|| black_box(index.might_contain("xyznonexistent")));
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_index_line,
    bench_search_scaling,
    bench_search_with_positions,
    bench_bloom_filter,
    bench_terminal_search,
    bench_might_contain,
);

criterion_main!(benches);
