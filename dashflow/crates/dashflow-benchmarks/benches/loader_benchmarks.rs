//! Performance benchmarks for document loaders
//!
//! Run with: cargo bench -p dashflow-benchmarks loader_benchmarks

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use criterion::{criterion_group, criterion_main, Criterion};
use dashflow::core::document_loaders::{CSVLoader, DocumentLoader, JSONLoader, TextLoader};
use std::io::Write;
use tempfile::NamedTempFile;

// ============================================================================
// Text Loader Performance
// ============================================================================

fn bench_text_loaders(c: &mut Criterion) {
    let mut group = c.benchmark_group("text_loaders");
    let runtime = tokio::runtime::Runtime::new().unwrap();

    // Create all files first and keep them alive
    let small_text = "Hello, world!\nThis is a test.\n";
    let mut small_file = NamedTempFile::new().unwrap();
    small_file.write_all(small_text.as_bytes()).unwrap();
    small_file.flush().unwrap();
    let small_path = small_file.path().to_path_buf();

    let medium_text = "Lorem ipsum dolor sit amet, consectetur adipiscing elit.\n".repeat(100);
    let mut medium_file = NamedTempFile::new().unwrap();
    medium_file.write_all(medium_text.as_bytes()).unwrap();
    medium_file.flush().unwrap();
    let medium_path = medium_file.path().to_path_buf();

    let large_text = "Lorem ipsum dolor sit amet, consectetur adipiscing elit. ".repeat(10000);
    let mut large_file = NamedTempFile::new().unwrap();
    large_file.write_all(large_text.as_bytes()).unwrap();
    large_file.flush().unwrap();
    let large_path = large_file.path().to_path_buf();

    // Run benchmarks with files kept alive
    group.bench_function("text_loader_small", |b| {
        b.to_async(&runtime).iter(|| async {
            let loader = TextLoader::new(&small_path);
            loader.load().await.unwrap()
        });
    });

    group.bench_function("text_loader_medium", |b| {
        b.to_async(&runtime).iter(|| async {
            let loader = TextLoader::new(&medium_path);
            loader.load().await.unwrap()
        });
    });

    group.bench_function("text_loader_large", |b| {
        b.to_async(&runtime).iter(|| async {
            let loader = TextLoader::new(&large_path);
            loader.load().await.unwrap()
        });
    });

    group.finish();

    // Files automatically cleaned up when dropped here
    drop(small_file);
    drop(medium_file);
    drop(large_file);
}

// ============================================================================
// CSV Loader Performance
// ============================================================================

fn bench_csv_loaders(c: &mut Criterion) {
    let mut group = c.benchmark_group("csv_loaders");
    let runtime = tokio::runtime::Runtime::new().unwrap();

    // Create all files first and keep them alive
    let small_csv = "name,age,city\nAlice,30,NYC\nBob,25,LA\nCarol,35,SF\n";
    let mut small_file = NamedTempFile::new().unwrap();
    small_file.write_all(small_csv.as_bytes()).unwrap();
    small_file.flush().unwrap();
    let small_path = small_file.path().to_path_buf();

    let medium_csv = {
        let mut csv = String::from("name,age,city,occupation\n");
        for i in 0..100 {
            csv.push_str(&format!("Person{},{},NYC,Engineer\n", i, 20 + (i % 50)));
        }
        csv
    };
    let mut medium_file = NamedTempFile::new().unwrap();
    medium_file.write_all(medium_csv.as_bytes()).unwrap();
    medium_file.flush().unwrap();
    let medium_path = medium_file.path().to_path_buf();

    let large_csv = {
        let mut csv = String::from("name,age,city,occupation,salary\n");
        for i in 0..1000 {
            csv.push_str(&format!(
                "Person{},{},City{},Job{},{}\n",
                i,
                20 + (i % 50),
                i % 10,
                i % 20,
                50000 + (i * 100)
            ));
        }
        csv
    };
    let mut large_file = NamedTempFile::new().unwrap();
    large_file.write_all(large_csv.as_bytes()).unwrap();
    large_file.flush().unwrap();
    let large_path = large_file.path().to_path_buf();

    // Run benchmarks with files kept alive
    group.bench_function("csv_loader_small", |b| {
        b.to_async(&runtime).iter(|| async {
            let loader = CSVLoader::new(&small_path);
            loader.load().await.unwrap()
        });
    });

    group.bench_function("csv_loader_medium", |b| {
        b.to_async(&runtime).iter(|| async {
            let loader = CSVLoader::new(&medium_path);
            loader.load().await.unwrap()
        });
    });

    group.bench_function("csv_loader_large", |b| {
        b.to_async(&runtime).iter(|| async {
            let loader = CSVLoader::new(&large_path);
            loader.load().await.unwrap()
        });
    });

    group.finish();

    // Files automatically cleaned up when dropped here
    drop(small_file);
    drop(medium_file);
    drop(large_file);
}

// ============================================================================
// JSON Loader Performance
// ============================================================================

fn bench_json_loaders(c: &mut Criterion) {
    let mut group = c.benchmark_group("json_loaders");
    let runtime = tokio::runtime::Runtime::new().unwrap();

    // Create all files first and keep them alive
    let small_json = r#"{"name": "Alice", "age": 30, "city": "NYC"}"#;
    let mut small_file = NamedTempFile::new().unwrap();
    small_file.write_all(small_json.as_bytes()).unwrap();
    small_file.flush().unwrap();
    let small_path = small_file.path().to_path_buf();

    let medium_json = {
        let mut json = String::from("[");
        for i in 0..100 {
            if i > 0 {
                json.push(',');
            }
            json.push_str(&format!(
                r#"{{"id":{},"name":"Person{}","age":{},"active":true}}"#,
                i,
                i,
                20 + (i % 50)
            ));
        }
        json.push(']');
        json
    };
    let mut medium_file = NamedTempFile::new().unwrap();
    medium_file.write_all(medium_json.as_bytes()).unwrap();
    medium_file.flush().unwrap();
    let medium_path = medium_file.path().to_path_buf();

    let large_json = {
        let mut json = String::from(r#"{"users":["#);
        for i in 0..1000 {
            if i > 0 {
                json.push(',');
            }
            json.push_str(&format!(
                r#"{{"id":{},"name":"User{}","email":"user{}@example.com","metadata":{{"created":"2025-01-01","active":true}}}}"#,
                i, i, i
            ));
        }
        json.push_str("]}");
        json
    };
    let mut large_file = NamedTempFile::new().unwrap();
    large_file.write_all(large_json.as_bytes()).unwrap();
    large_file.flush().unwrap();
    let large_path = large_file.path().to_path_buf();

    // Run benchmarks with files kept alive
    group.bench_function("json_loader_small", |b| {
        b.to_async(&runtime).iter(|| async {
            let loader = JSONLoader::new(&small_path);
            loader.load().await.unwrap()
        });
    });

    group.bench_function("json_loader_medium", |b| {
        b.to_async(&runtime).iter(|| async {
            let loader = JSONLoader::new(&medium_path);
            loader.load().await.unwrap()
        });
    });

    group.bench_function("json_loader_large", |b| {
        b.to_async(&runtime).iter(|| async {
            let loader = JSONLoader::new(&large_path);
            loader.load().await.unwrap()
        });
    });

    group.finish();

    // Files automatically cleaned up when dropped here
    drop(small_file);
    drop(medium_file);
    drop(large_file);
}

criterion_group!(
    benches,
    bench_text_loaders,
    bench_csv_loaders,
    bench_json_loaders,
);
criterion_main!(benches);
