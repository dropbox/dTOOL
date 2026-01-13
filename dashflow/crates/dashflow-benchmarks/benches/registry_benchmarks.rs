//! Performance benchmarks for registry operations
//!
//! Run with: cargo bench -p dashflow-benchmarks --bench registry_benchmarks
//!
//! These benchmarks focus on CPU-bound operations in the registry client:
//! - Content hashing (SHA-256)
//! - JSON serialization/deserialization of manifests
//! - Tarball compression/decompression
//!
//! Note: Network operations are not benchmarked here as they would require
//! mocking the registry server, and would primarily measure mock server latency.

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;

// ============================================================================
// Test Data Structures (Mirror registry types)
// ============================================================================

/// Minimal package manifest for benchmarking
#[derive(Clone, Debug, Serialize, Deserialize)]
struct BenchPackageManifest {
    name: String,
    version: String,
    description: String,
    authors: Vec<String>,
    license: String,
    keywords: Vec<String>,
    dependencies: HashMap<String, String>,
    capabilities: Vec<String>,
    metadata: HashMap<String, serde_json::Value>,
}

impl BenchPackageManifest {
    fn small() -> Self {
        Self {
            name: "test-package".to_string(),
            version: "1.0.0".to_string(),
            description: "A test package".to_string(),
            authors: vec!["Test Author <test@example.com>".to_string()],
            license: "MIT".to_string(),
            keywords: vec!["test".to_string()],
            dependencies: HashMap::new(),
            capabilities: vec!["llm".to_string()],
            metadata: HashMap::new(),
        }
    }

    fn medium() -> Self {
        let mut deps = HashMap::new();
        for i in 0..10 {
            deps.insert(format!("dep-{}", i), format!("^{}.0.0", i + 1));
        }

        let mut metadata = HashMap::new();
        for i in 0..20 {
            metadata.insert(
                format!("key_{}", i),
                serde_json::json!({
                    "nested": format!("value_{}", i),
                    "number": i,
                    "array": [1, 2, 3, i]
                }),
            );
        }

        Self {
            name: "medium-package".to_string(),
            version: "2.5.3".to_string(),
            description: "A medium-sized package with multiple dependencies and metadata fields for realistic benchmarking.".to_string(),
            authors: vec![
                "Alice <alice@example.com>".to_string(),
                "Bob <bob@example.com>".to_string(),
                "Charlie <charlie@example.com>".to_string(),
            ],
            license: "Apache-2.0".to_string(),
            keywords: vec![
                "ai".to_string(),
                "ml".to_string(),
                "nlp".to_string(),
                "embeddings".to_string(),
                "vectors".to_string(),
            ],
            dependencies: deps,
            capabilities: vec![
                "llm".to_string(),
                "embeddings".to_string(),
                "retriever".to_string(),
                "tool".to_string(),
            ],
            metadata,
        }
    }

    fn large() -> Self {
        let mut deps = HashMap::new();
        for i in 0..50 {
            deps.insert(format!("dependency-{}", i), format!("~{}.{}.0", i / 10, i % 10));
        }

        let mut metadata = HashMap::new();
        for i in 0..100 {
            metadata.insert(
                format!("metadata_key_{}", i),
                serde_json::json!({
                    "description": format!("This is a detailed description for metadata field {}", i),
                    "config": {
                        "enabled": i % 2 == 0,
                        "priority": i,
                        "tags": ["tag1", "tag2", "tag3"]
                    },
                    "data": (0..10).map(|j| format!("item_{}_{}", i, j)).collect::<Vec<_>>()
                }),
            );
        }

        Self {
            name: "large-enterprise-package".to_string(),
            version: "10.25.100".to_string(),
            description: "A large enterprise-grade package with extensive dependencies, \
                          capabilities, and metadata. This represents the upper bound of \
                          package manifest complexity in realistic usage scenarios.".to_string(),
            authors: (0..10)
                .map(|i| format!("Developer {} <dev{}@enterprise.com>", i, i))
                .collect(),
            license: "Commercial".to_string(),
            keywords: (0..20).map(|i| format!("keyword-{}", i)).collect(),
            dependencies: deps,
            capabilities: vec![
                "llm".to_string(),
                "embeddings".to_string(),
                "retriever".to_string(),
                "tool".to_string(),
                "agent".to_string(),
                "chain".to_string(),
                "memory".to_string(),
                "vectorstore".to_string(),
            ],
            metadata,
        }
    }
}

/// Search result for benchmarking
#[derive(Clone, Debug, Serialize, Deserialize)]
struct BenchSearchResult {
    name: String,
    version: String,
    description: String,
    score: f64,
    downloads: u64,
    keywords: Vec<String>,
}

fn create_search_results(count: usize) -> Vec<BenchSearchResult> {
    (0..count)
        .map(|i| BenchSearchResult {
            name: format!("package-{}", i),
            version: format!("{}.{}.{}", i / 100, (i / 10) % 10, i % 10),
            description: format!("Package {} provides functionality for task {}", i, i % 5),
            score: 1.0 - (i as f64 / count as f64),
            downloads: (count - i) as u64 * 1000,
            keywords: vec![
                format!("kw-{}", i % 10),
                format!("cat-{}", i % 5),
            ],
        })
        .collect()
}

// ============================================================================
// Content Hashing Benchmarks
// ============================================================================

fn bench_content_hashing(c: &mut Criterion) {
    let mut group = c.benchmark_group("registry_hashing");

    // Small content (1KB)
    let small_content = vec![b'x'; 1024];
    group.bench_function("sha256_1kb", |b| {
        b.iter(|| {
            let mut hasher = Sha256::new();
            hasher.update(black_box(&small_content));
            hasher.finalize()
        });
    });

    // Medium content (100KB)
    let medium_content = vec![b'x'; 100 * 1024];
    group.bench_function("sha256_100kb", |b| {
        b.iter(|| {
            let mut hasher = Sha256::new();
            hasher.update(black_box(&medium_content));
            hasher.finalize()
        });
    });

    // Large content (1MB)
    let large_content = vec![b'x'; 1024 * 1024];
    group.bench_function("sha256_1mb", |b| {
        b.iter(|| {
            let mut hasher = Sha256::new();
            hasher.update(black_box(&large_content));
            hasher.finalize()
        });
    });

    // Large content (10MB)
    let xlarge_content = vec![b'x'; 10 * 1024 * 1024];
    group.bench_function("sha256_10mb", |b| {
        b.iter(|| {
            let mut hasher = Sha256::new();
            hasher.update(black_box(&xlarge_content));
            hasher.finalize()
        });
    });

    group.finish();
}

// ============================================================================
// JSON Serialization Benchmarks
// ============================================================================

fn bench_manifest_serialization(c: &mut Criterion) {
    let mut group = c.benchmark_group("registry_manifest_serialize");

    // Small manifest
    let small = BenchPackageManifest::small();
    group.bench_function("small_to_json", |b| {
        b.iter(|| serde_json::to_string(black_box(&small)))
    });

    let small_json = serde_json::to_string(&small).unwrap_or_default();
    group.bench_function("small_from_json", |b| {
        b.iter(|| serde_json::from_str::<BenchPackageManifest>(black_box(&small_json)))
    });

    // Medium manifest
    let medium = BenchPackageManifest::medium();
    group.bench_function("medium_to_json", |b| {
        b.iter(|| serde_json::to_string(black_box(&medium)))
    });

    let medium_json = serde_json::to_string(&medium).unwrap_or_default();
    group.bench_function("medium_from_json", |b| {
        b.iter(|| serde_json::from_str::<BenchPackageManifest>(black_box(&medium_json)))
    });

    // Large manifest
    let large = BenchPackageManifest::large();
    group.bench_function("large_to_json", |b| {
        b.iter(|| serde_json::to_string(black_box(&large)))
    });

    let large_json = serde_json::to_string(&large).unwrap_or_default();
    group.bench_function("large_from_json", |b| {
        b.iter(|| serde_json::from_str::<BenchPackageManifest>(black_box(&large_json)))
    });

    group.finish();
}

// ============================================================================
// Search Results Serialization Benchmarks
// ============================================================================

fn bench_search_serialization(c: &mut Criterion) {
    let mut group = c.benchmark_group("registry_search_serialize");

    for count in [10, 50, 100, 500] {
        let results = create_search_results(count);

        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}_results_to_json", count)),
            &results,
            |b, results| b.iter(|| serde_json::to_string(black_box(results))),
        );

        let json = serde_json::to_string(&results).unwrap_or_default();
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}_results_from_json", count)),
            &json,
            |b, json| b.iter(|| serde_json::from_str::<Vec<BenchSearchResult>>(black_box(json))),
        );
    }

    group.finish();
}

// ============================================================================
// Compression Benchmarks (for package tarballs)
// ============================================================================

fn bench_compression(c: &mut Criterion) {
    let mut group = c.benchmark_group("registry_compression");

    // Create compressible data (simulating code/text content)
    let create_compressible_data = |size: usize| -> Vec<u8> {
        let pattern = b"function example() { return 'hello world'; }\n";
        pattern.iter().cycle().take(size).copied().collect()
    };

    // 10KB compressible
    let data_10kb = create_compressible_data(10 * 1024);
    group.bench_function("gzip_compress_10kb", |b| {
        use flate2::write::GzEncoder;
        use flate2::Compression;
        use std::io::Write;

        b.iter(|| {
            let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
            if encoder.write_all(black_box(&data_10kb)).is_err() {
                return Vec::new();
            }
            encoder.finish().unwrap_or_default()
        });
    });

    // 100KB compressible
    let data_100kb = create_compressible_data(100 * 1024);
    group.bench_function("gzip_compress_100kb", |b| {
        use flate2::write::GzEncoder;
        use flate2::Compression;
        use std::io::Write;

        b.iter(|| {
            let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
            if encoder.write_all(black_box(&data_100kb)).is_err() {
                return Vec::new();
            }
            encoder.finish().unwrap_or_default()
        });
    });

    // 1MB compressible
    let data_1mb = create_compressible_data(1024 * 1024);
    group.bench_function("gzip_compress_1mb", |b| {
        use flate2::write::GzEncoder;
        use flate2::Compression;
        use std::io::Write;

        b.iter(|| {
            let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
            if encoder.write_all(black_box(&data_1mb)).is_err() {
                return Vec::new();
            }
            encoder.finish().unwrap_or_default()
        });
    });

    // Decompression benchmarks
    let compressed_100kb = {
        use flate2::write::GzEncoder;
        use flate2::Compression;
        use std::io::Write;

        let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
        if encoder.write_all(&data_100kb).is_err() {
            Vec::new()
        } else {
            encoder.finish().unwrap_or_default()
        }
    };

    group.bench_function("gzip_decompress_100kb", |b| {
        use flate2::read::GzDecoder;
        use std::io::Read;

        b.iter(|| {
            let mut decoder = GzDecoder::new(black_box(&compressed_100kb[..]));
            let mut output = Vec::new();
            if decoder.read_to_end(&mut output).is_err() {
                output.clear();
            }
            output
        });
    });

    let compressed_1mb = {
        use flate2::write::GzEncoder;
        use flate2::Compression;
        use std::io::Write;

        let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
        if encoder.write_all(&data_1mb).is_err() {
            Vec::new()
        } else {
            encoder.finish().unwrap_or_default()
        }
    };

    group.bench_function("gzip_decompress_1mb", |b| {
        use flate2::read::GzDecoder;
        use std::io::Read;

        b.iter(|| {
            let mut decoder = GzDecoder::new(black_box(&compressed_1mb[..]));
            let mut output = Vec::new();
            if decoder.read_to_end(&mut output).is_err() {
                output.clear();
            }
            output
        });
    });

    group.finish();
}

// ============================================================================
// Benchmark Groups
// ============================================================================

criterion_group!(
    benches,
    bench_content_hashing,
    bench_manifest_serialization,
    bench_search_serialization,
    bench_compression,
);

criterion_main!(benches);
