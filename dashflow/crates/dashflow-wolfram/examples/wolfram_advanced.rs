//! Advanced WolframAlpha examples
//!
//! This example demonstrates advanced usage including different query types,
//! unit systems, and tool integration.
//!
//! # Usage
//!
//! ```bash
//! export WOLFRAM_APP_ID=your_app_id_here
//! cargo run --example wolfram_advanced
//! ```

use dashflow::core::tools::{Tool, ToolInput};
use dashflow_wolfram::WolframAlpha;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let app_id = std::env::var("WOLFRAM_APP_ID").unwrap_or_else(|_| "demo".to_string());

    println!("WolframAlpha Advanced Examples\n");
    println!("==============================\n");

    // === Mathematical Queries ===
    println!("=== Mathematical Queries ===\n");

    let tool = WolframAlpha::new(&app_id);

    let math_queries = vec![
        "integrate x^2 from 0 to 10",
        "derivative of sin(x) * cos(x)",
        "solve x^3 - 6x^2 + 11x - 6 = 0",
        "fibonacci 20",
        "prime factorization of 1024",
    ];

    for query in math_queries {
        println!("Query: {}", query);
        let input = ToolInput::String(query.to_string());
        match tool._call(input).await {
            Ok(answer) => println!("Answer: {}\n", answer),
            Err(e) => println!("Error: {}\n", e),
        }
    }

    // === Scientific Queries ===
    println!("\n=== Scientific Queries ===\n");

    let science_queries = vec![
        "atomic weight of carbon",
        "half-life of carbon-14",
        "boiling point of water at 5000 feet",
        "mass of electron",
    ];

    for query in science_queries {
        println!("Query: {}", query);
        let input = ToolInput::String(query.to_string());
        match tool._call(input).await {
            Ok(answer) => println!("Answer: {}\n", answer),
            Err(e) => println!("Error: {}\n", e),
        }
    }

    // === Unit Conversions (Metric) ===
    println!("\n=== Unit Conversions (Metric) ===\n");

    let metric_tool = WolframAlpha::builder()
        .app_id(&app_id)
        .units("metric")
        .build();

    let conversions = vec![
        "100 miles in kilometers",
        "72 fahrenheit in celsius",
        "5 pounds in kilograms",
    ];

    for query in conversions {
        println!("Query: {}", query);
        let input = ToolInput::String(query.to_string());
        match metric_tool._call(input).await {
            Ok(answer) => println!("Answer: {}\n", answer),
            Err(e) => println!("Error: {}\n", e),
        }
    }

    // === Structured Input (JSON-like) ===
    println!("\n=== Using Structured Input ===\n");

    let query = "What is the distance from Earth to Mars?";
    println!("Query: {}", query);

    let params = serde_json::json!({"query": query});
    let input = ToolInput::Structured(params);

    match tool._call(input).await {
        Ok(answer) => println!("Answer: {}\n", answer),
        Err(e) => println!("Error: {}\n", e),
    }

    // === Tool Metadata ===
    println!("\n=== Tool Metadata ===\n");

    println!("Tool Name: {}", tool.name());
    println!("Description: {}", tool.description());
    println!("\nArgs Schema:");
    println!("{}", serde_json::to_string_pretty(&tool.args_schema())?);

    Ok(())
}
