//! DuckDuckGo Search Example
//!
//! This example demonstrates how to use the DuckDuckGoSearchTool to search the web
//! using DuckDuckGo's privacy-focused search engine.
//!
//! Run with: cargo run --example duckduckgo_search

use dashflow::core::tools::Tool;
use dashflow_duckduckgo::DuckDuckGoSearchTool;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== DuckDuckGo Search Tool Example ===\n");

    // Create a DuckDuckGo search tool with default settings
    let ddg = DuckDuckGoSearchTool::new();

    println!("Tool name: {}", ddg.name());
    println!("Description: {}", ddg.description());
    println!();

    // Example 1: Search for Rust programming language
    println!("--- Example 1: Searching for 'Rust programming language' ---\n");
    match ddg._call_str("Rust programming language".to_string()).await {
        Ok(results) => {
            println!("{}\n", results);
        }
        Err(e) => {
            eprintln!("Search failed: {}", e);
        }
    }

    // Example 2: Search with custom max_results
    println!("--- Example 2: Searching with custom max_results (3) ---\n");
    let ddg_custom = DuckDuckGoSearchTool::builder().max_results(3).build();
    match ddg_custom._call_str("climate change".to_string()).await {
        Ok(results) => {
            println!("{}\n", results);
        }
        Err(e) => {
            eprintln!("Search failed: {}", e);
        }
    }

    // Example 3: Search for a specific topic
    println!("--- Example 3: Searching for 'quantum computing applications' ---\n");
    match ddg
        ._call_str("quantum computing applications".to_string())
        .await
    {
        Ok(results) => {
            println!("{}\n", results);
        }
        Err(e) => {
            eprintln!("Search failed: {}", e);
        }
    }

    // Example 4: Search for news
    println!("--- Example 4: Searching for recent AI news ---\n");
    let ddg_news = DuckDuckGoSearchTool::builder().max_results(5).build();
    match ddg_news
        ._call_str("artificial intelligence news 2025".to_string())
        .await
    {
        Ok(results) => {
            println!("{}\n", results);
        }
        Err(e) => {
            eprintln!("Search failed: {}", e);
        }
    }

    println!("=== Example Complete ===");
    Ok(())
}
