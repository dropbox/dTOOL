//! Web scraping example using Playwright tools.
//!
//! This example demonstrates how to use the Playwright tools to:
//! 1. Navigate to a website
//! 2. Extract text content
//! 3. Extract hyperlinks
//! 4. Get specific elements
//! 5. Get current page info
//!
//! Run with: cargo run --example web_scraping

use anyhow::Result;
use dashflow::core::tools::Tool;
use dashflow_playwright::{
    CurrentWebPageTool, ExtractHyperlinksTool, ExtractTextTool, GetElementsTool, NavigateTool,
};

#[tokio::main]
async fn main() -> Result<()> {
    println!("=== Playwright Web Scraping Example ===\n");

    // Initialize the navigate tool (this will start a browser)
    println!("Starting browser...");
    let nav_tool = NavigateTool::new().await?;
    let state = nav_tool.state().clone();

    // Create all other tools with the same browser state
    let current_page_tool = CurrentWebPageTool::with_state(state.clone());
    let extract_text_tool = ExtractTextTool::with_state(state.clone());
    let extract_links_tool = ExtractHyperlinksTool::with_state(state.clone());
    let get_elements_tool = GetElementsTool::with_state(state.clone());

    // Navigate to example.com
    println!("\n1. Navigating to example.com...");
    let result = nav_tool
        ._call_str("https://www.example.com".to_string())
        .await?;
    println!("   {}", result);

    // Get current page info
    println!("\n2. Getting current page info...");
    let page_info = current_page_tool._call_str("".to_string()).await?;
    println!("   {}", page_info);

    // Extract all text
    println!("\n3. Extracting page text...");
    let text = extract_text_tool._call_str("".to_string()).await?;
    println!("   Text preview (first 200 chars):");
    println!("   {}...", text.chars().take(200).collect::<String>());

    // Extract hyperlinks
    println!("\n4. Extracting hyperlinks...");
    let links = extract_links_tool._call_str("".to_string()).await?;
    println!("   Found links:");
    println!("   {}", links);

    // Get specific elements (h1 headers)
    println!("\n5. Getting h1 elements...");
    let elements = get_elements_tool._call_str("h1".to_string()).await?;
    println!("   Headers:");
    println!("   {}", elements);

    // Navigate to Rust website
    println!("\n6. Navigating to rust-lang.org...");
    let result = nav_tool
        ._call_str("https://www.rust-lang.org".to_string())
        .await?;
    println!("   {}", result);

    // Get page info
    println!("\n7. Getting current page info...");
    let page_info = current_page_tool._call_str("".to_string()).await?;
    println!("   {}", page_info);

    // Extract some headers
    println!("\n8. Extracting headers (h1, h2)...");
    let headers = get_elements_tool._call_str("h1, h2".to_string()).await?;
    println!("   Found headers:");
    println!("   {}", headers);

    // Extract hyperlinks from Rust site
    println!("\n9. Extracting navigation links...");
    let nav_links = extract_links_tool._call_str("".to_string()).await?;
    println!("   Found {} links", nav_links.matches("href").count());

    // Show first 5 links
    if let Ok(parsed_links) = serde_json::from_str::<Vec<serde_json::Value>>(&nav_links) {
        println!("   First 5 links:");
        for link in parsed_links.iter().take(5) {
            println!("     - {}: {}", link["text"], link["href"]);
        }
    }

    println!("\n=== Example Complete ===");
    println!("Note: Browser will close when this program exits.");

    Ok(())
}
