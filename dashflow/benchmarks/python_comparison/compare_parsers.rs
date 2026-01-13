/// Performance comparison: Rust `DashFlow` vs Python `DashFlow` (Output Parsers)
///
/// Tests CPU-bound parser operations without API calls.
/// To run: cargo run --release --bin `compare_parsers`
use dashflow::core::output_parsers::{
    CommaSeparatedListOutputParser, JsonOutputParser, OutputParser,
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

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Rust DashFlow Parser Benchmarks");
    println!("{}", "=".repeat(60));

    let mut results = Vec::new();

    // Test 1: CommaSeparatedListOutputParser - Simple
    let parser = CommaSeparatedListOutputParser;
    let simple_input = "apple, banana, cherry".to_string();
    results.push(benchmark(
        "CommaSeparatedListOutputParser (simple)",
        || {
            let _ = parser.parse(&simple_input);
        },
        1000,
    ));

    // Test 2: CommaSeparatedListOutputParser - Complex
    let complex_input =
        "apple, banana, cherry, date, elderberry, fig, grape, honeydew, kiwi, lemon".to_string();
    results.push(benchmark(
        "CommaSeparatedListOutputParser (complex)",
        || {
            let _ = parser.parse(&complex_input);
        },
        1000,
    ));

    // Test 3: JsonOutputParser - Simple
    let json_parser = JsonOutputParser;
    let simple_json = r#"{"name": "Alice", "age": 30}"#.to_string();
    results.push(benchmark(
        "SimpleJsonOutputParser (simple)",
        || {
            let _ = json_parser.parse(&simple_json);
        },
        1000,
    ));

    // Test 4: JsonOutputParser - Complex
    let complex_json = r#"
    {
        "person": {
            "name": "Alice",
            "age": 30,
            "address": {
                "street": "123 Main St",
                "city": "Springfield",
                "country": "USA"
            },
            "hobbies": ["reading", "hiking", "coding"]
        }
    }
    "#
    .to_string();
    results.push(benchmark(
        "SimpleJsonOutputParser (complex)",
        || {
            let _ = json_parser.parse(&complex_json);
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
    let output_file = "parser_results_rust.json";
    let workspace_output = "benchmarks/python_comparison/parser_results_rust.json";

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
