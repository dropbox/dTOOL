/// Performance comparison: Rust `DashFlow` Document Loaders
///
/// Tests document loading operations without API calls.
/// Compares CSV, JSON, and text file loading performance.
/// To run: cargo run --release --bin `compare_document_loaders`
use dashflow::core::document_loaders::{CSVLoader, DocumentLoader, JSONLoader, TextLoader};
use serde_json::json;
use std::fs;
use std::time::Instant;

struct BenchmarkResult {
    name: String,
    iterations: usize,
    mean_us: f64,
    median_us: f64,
    stdev_us: f64,
    min_us: f64,
    max_us: f64,
}

async fn benchmark_async<F, Fut>(name: &str, mut func: F, iterations: usize) -> BenchmarkResult
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = ()>,
{
    let mut times = Vec::with_capacity(iterations);

    // Warmup
    for _ in 0..10 {
        func().await;
    }

    // Measure
    for _ in 0..iterations {
        let start = Instant::now();
        func().await;
        let duration = start.elapsed();
        times.push(duration.as_secs_f64() * 1_000_000.0); // Convert to microseconds
    }

    times.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

    let mean = times.iter().sum::<f64>() / times.len() as f64;
    let median = times[times.len() / 2];
    let min = times[0];
    let max = times[times.len() - 1];

    // Calculate standard deviation
    let variance = times
        .iter()
        .map(|t| {
            let diff = t - mean;
            diff * diff
        })
        .sum::<f64>()
        / times.len() as f64;
    let stdev = variance.sqrt();

    BenchmarkResult {
        name: name.to_string(),
        iterations,
        mean_us: mean,
        median_us: median,
        stdev_us: stdev,
        min_us: min,
        max_us: max,
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Rust DashFlow Document Loader Benchmarks");
    println!("{}", "=".repeat(60));

    // Create temporary test files
    let temp_dir = "/tmp/dashflow_loader_bench";
    fs::create_dir_all(temp_dir)?;

    // Small text file (100 bytes)
    let small_text_path = format!("{temp_dir}/small.txt");
    fs::write(&small_text_path, "A".repeat(100))?;

    // Medium text file (10KB)
    let medium_text_path = format!("{temp_dir}/medium.txt");
    fs::write(&medium_text_path, "B".repeat(10_000))?;

    // Large text file (1MB)
    let large_text_path = format!("{temp_dir}/large.txt");
    fs::write(&large_text_path, "C".repeat(1_000_000))?;

    // Small CSV file (10 rows)
    let small_csv_path = format!("{temp_dir}/small.csv");
    let mut csv_content = "id,name,value\n".to_string();
    for i in 0..10 {
        csv_content.push_str(&format!("{i},name_{i},value_{i}\n"));
    }
    fs::write(&small_csv_path, csv_content)?;

    // Medium CSV file (100 rows)
    let medium_csv_path = format!("{temp_dir}/medium.csv");
    let mut csv_content = "id,name,value\n".to_string();
    for i in 0..100 {
        csv_content.push_str(&format!("{i},name_{i},value_{i}\n"));
    }
    fs::write(&medium_csv_path, csv_content)?;

    // Small JSON file (10 records)
    let small_json_path = format!("{temp_dir}/small.json");
    let json_data: Vec<_> = (0..10)
        .map(|i| json!({"id": i, "name": format!("name_{}", i), "value": format!("value_{}", i)}))
        .collect();
    fs::write(&small_json_path, serde_json::to_string(&json_data)?)?;

    // Medium JSON file (100 records)
    let medium_json_path = format!("{temp_dir}/medium.json");
    let json_data: Vec<_> = (0..100)
        .map(|i| json!({"id": i, "name": format!("name_{}", i), "value": format!("value_{}", i)}))
        .collect();
    fs::write(&medium_json_path, serde_json::to_string(&json_data)?)?;

    let mut results = Vec::new();

    // Test 1: TextLoader - Small (100B)
    let path = small_text_path.clone();
    results.push(
        benchmark_async(
            "TextLoader (small, 100B)",
            || async {
                let loader = TextLoader::new(&path);
                let _ = loader.load().await;
            },
            1000,
        )
        .await,
    );

    // Test 2: TextLoader - Medium (10KB)
    let path = medium_text_path.clone();
    results.push(
        benchmark_async(
            "TextLoader (medium, 10KB)",
            || async {
                let loader = TextLoader::new(&path);
                let _ = loader.load().await;
            },
            1000,
        )
        .await,
    );

    // Test 3: TextLoader - Large (1MB)
    let path = large_text_path.clone();
    results.push(
        benchmark_async(
            "TextLoader (large, 1MB)",
            || async {
                let loader = TextLoader::new(&path);
                let _ = loader.load().await;
            },
            100,
        )
        .await,
    );

    // Test 4: CSVLoader - Small (10 rows)
    let path = small_csv_path.clone();
    results.push(
        benchmark_async(
            "CSVLoader (small, 10 rows)",
            || async {
                let loader = CSVLoader::new(&path);
                let _ = loader.load().await;
            },
            1000,
        )
        .await,
    );

    // Test 5: CSVLoader - Medium (100 rows)
    let path = medium_csv_path.clone();
    results.push(
        benchmark_async(
            "CSVLoader (medium, 100 rows)",
            || async {
                let loader = CSVLoader::new(&path);
                let _ = loader.load().await;
            },
            1000,
        )
        .await,
    );

    // Test 6: JSONLoader - Small (10 records)
    let path = small_json_path.clone();
    results.push(
        benchmark_async(
            "JSONLoader (small, 10 records)",
            || async {
                let loader = JSONLoader::new(&path);
                let _ = loader.load().await;
            },
            1000,
        )
        .await,
    );

    // Test 7: JSONLoader - Medium (100 records)
    let path = medium_json_path.clone();
    results.push(
        benchmark_async(
            "JSONLoader (medium, 100 records)",
            || async {
                let loader = JSONLoader::new(&path);
                let _ = loader.load().await;
            },
            1000,
        )
        .await,
    );

    // Print results
    println!(
        "\n{:<50} {:<12} {:<12} {:<12}",
        "Test Name", "Mean (μs)", "Median (μs)", "StdDev (μs)"
    );
    println!("{}", "-".repeat(90));

    for result in &results {
        println!(
            "{:<50} {:<12.2} {:<12.2} {:<12.2}",
            result.name, result.mean_us, result.median_us, result.stdev_us
        );
    }

    // Save results to JSON
    let output_file = "document_loader_results_rust.json";
    let json_results: Vec<_> = results
        .iter()
        .map(|r| {
            serde_json::json!({
                "name": r.name,
                "iterations": r.iterations,
                "mean_us": r.mean_us,
                "median_us": r.median_us,
                "stdev_us": r.stdev_us,
                "min_us": r.min_us,
                "max_us": r.max_us,
            })
        })
        .collect();

    fs::write(output_file, serde_json::to_string_pretty(&json_results)?)?;
    println!("\nResults saved to: {output_file}");

    // Cleanup
    fs::remove_dir_all(temp_dir)?;

    Ok(())
}
