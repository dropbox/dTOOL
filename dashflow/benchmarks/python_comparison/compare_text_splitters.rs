/// Performance comparison: Rust `DashFlow` vs Python `DashFlow` (Text Splitters)
///
/// Tests CPU-bound text splitting operations without API calls.
/// To run: cargo run --release --bin `compare_text_splitters`
use dashflow_text_splitters::{
    CharacterTextSplitter, RecursiveCharacterTextSplitter, TextSplitter,
};
use serde_json::json;
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

fn benchmark<F>(name: &str, mut func: F, iterations: usize) -> BenchmarkResult
where
    F: FnMut(),
{
    let mut times = Vec::with_capacity(iterations);

    // Warmup
    for _ in 0..10 {
        func();
    }

    // Measure
    for _ in 0..iterations {
        let start = Instant::now();
        func();
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

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Rust DashFlow Text Splitter Benchmarks");
    println!("{}", "=".repeat(60));

    let mut results = Vec::new();

    // Test data
    let short_text = "Hello world. This is a test. Another sentence here.";

    let medium_text = r"
Lorem ipsum dolor sit amet, consectetur adipiscing elit. Sed do eiusmod tempor
incididunt ut labore et dolore magna aliqua. Ut enim ad minim veniam, quis nostrud
exercitation ullamco laboris nisi ut aliquip ex ea commodo consequat.

Duis aute irure dolor in reprehenderit in voluptate velit esse cillum dolore eu
fugiat nulla pariatur. Excepteur sint occaecat cupidatat non proident, sunt in
culpa qui officia deserunt mollit anim id est laborum.

Sed ut perspiciatis unde omnis iste natus error sit voluptatem accusantium doloremque
laudantium, totam rem aperiam, eaque ipsa quae ab illo inventore veritatis et quasi
architecto beatae vitae dicta sunt explicabo.
"
    .trim();

    let long_text = medium_text.repeat(10); // ~5KB

    // Test 1: CharacterTextSplitter - Short text
    let splitter = CharacterTextSplitter::new()
        .with_chunk_size(50)
        .with_chunk_overlap(10)
        .with_separator(". ");
    results.push(benchmark(
        "CharacterTextSplitter (short, 50 chars)",
        || {
            let _ = splitter.split_text(short_text);
        },
        1000,
    ));

    // Test 2: CharacterTextSplitter - Medium text
    let splitter = CharacterTextSplitter::new()
        .with_chunk_size(100)
        .with_chunk_overlap(20)
        .with_separator("\n\n");
    results.push(benchmark(
        "CharacterTextSplitter (medium, 100 chars)",
        || {
            let _ = splitter.split_text(medium_text);
        },
        1000,
    ));

    // Test 3: CharacterTextSplitter - Long text
    let splitter = CharacterTextSplitter::new()
        .with_chunk_size(200)
        .with_chunk_overlap(50)
        .with_separator("\n\n");
    results.push(benchmark(
        "CharacterTextSplitter (long, 200 chars)",
        || {
            let _ = splitter.split_text(&long_text);
        },
        1000,
    ));

    // Test 4: RecursiveCharacterTextSplitter - Short text
    let splitter = RecursiveCharacterTextSplitter::new()
        .with_chunk_size(50)
        .with_chunk_overlap(10);
    results.push(benchmark(
        "RecursiveCharacterTextSplitter (short, 50 chars)",
        || {
            let _ = splitter.split_text(short_text);
        },
        1000,
    ));

    // Test 5: RecursiveCharacterTextSplitter - Medium text
    let splitter = RecursiveCharacterTextSplitter::new()
        .with_chunk_size(100)
        .with_chunk_overlap(20);
    results.push(benchmark(
        "RecursiveCharacterTextSplitter (medium, 100 chars)",
        || {
            let _ = splitter.split_text(medium_text);
        },
        1000,
    ));

    // Test 6: RecursiveCharacterTextSplitter - Long text
    let splitter = RecursiveCharacterTextSplitter::new()
        .with_chunk_size(200)
        .with_chunk_overlap(50);
    results.push(benchmark(
        "RecursiveCharacterTextSplitter (long, 200 chars)",
        || {
            let _ = splitter.split_text(&long_text);
        },
        1000,
    ));

    // Print results
    println!();
    println!(
        "{:<50} {:<12} {:<12} {:<12}",
        "Test Name", "Mean (μs)", "Median (μs)", "StdDev (μs)"
    );
    println!("{}", "-".repeat(90));
    for result in &results {
        println!(
            "{:<50} {:<12.2} {:<12.2} {:<12.2}",
            result.name, result.mean_us, result.median_us, result.stdev_us
        );
    }

    // Save results to JSON for comparison
    let output = json!({
        "results": results.iter().map(|r| {
            json!({
                "name": r.name,
                "iterations": r.iterations,
                "mean_us": r.mean_us,
                "median_us": r.median_us,
                "stdev_us": r.stdev_us,
                "min_us": r.min_us,
                "max_us": r.max_us,
            })
        }).collect::<Vec<_>>()
    });

    // Use absolute path or try both relative and absolute
    let output_file = "text_splitter_results_rust.json";
    let workspace_output = "benchmarks/python_comparison/text_splitter_results_rust.json";

    // Try workspace-relative path first
    if let Ok(()) = std::fs::write(workspace_output, serde_json::to_string_pretty(&output)?) {
        println!();
        println!("Results saved to: {workspace_output}");
    } else {
        // Fall back to current directory
        std::fs::write(output_file, serde_json::to_string_pretty(&output)?)?;
        println!();
        println!("Results saved to: {output_file}");
    }

    Ok(())
}
