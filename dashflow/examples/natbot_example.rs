//! Example of using NatBotChain for browser automation.
//!
//! This example demonstrates how to use NatBotChain with an LLM to automate
//! browser interactions. The chain analyzes webpage content and generates
//! commands to accomplish objectives.
//!
//! # Security Warning
//! NatBotChain can navigate to any URL including internal network URLs and local
//! files. Only expose this to trusted users and in controlled environments.
//!
//! # Prerequisites
//! - Playwright browser installed (run: `playwright install chromium`)
//! - OpenAI API key in OPENAI_API_KEY environment variable
//!
//! # Run this example
//! ```bash
//! cargo run --example natbot_example
//! ```

use dashflow_chains::natbot::NatBotChain;
use dashflow_openai::ChatOpenAI;
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Check for API key
    if std::env::var("OPENAI_API_KEY").is_err() {
        eprintln!("Error: OPENAI_API_KEY environment variable not set");
        eprintln!("Set it with: export OPENAI_API_KEY=your-api-key");
        std::process::exit(1);
    }

    println!("NatBot Browser Automation Example");
    println!("==================================\n");

    // Create LLM instance
    let llm = Arc::new(ChatOpenAI::default());

    // Define the objective
    let objective = "Search for information about Rust programming language";
    println!("Objective: {}", objective);

    // Create NatBotChain
    println!("\nInitializing NatBot with browser...");
    let mut natbot = NatBotChain::from_llm(llm, objective).await?;
    println!("Browser ready!");

    // Step 1: Navigate to Google
    println!("\n--- Step 1: Navigate to Google ---");
    let url = "https://www.google.com";
    natbot.go_to_page(url).await?;
    println!("Navigated to: {}", url);

    // Step 2: Crawl the page to get simplified representation
    println!("\n--- Step 2: Crawl page content ---");
    let page_content = natbot.crawl().await?;
    println!("Found {} interactive elements on the page", page_content.len());

    // Show first few elements
    println!("\nFirst 5 elements:");
    for (i, element) in page_content.iter().take(5).enumerate() {
        println!("  {}: {}", i + 1, element);
    }

    // Convert to string for LLM
    let browser_content = page_content.join("\n");

    // Step 3: Get next command from LLM
    println!("\n--- Step 3: Generate command via LLM ---");
    let command = natbot.execute(url, &browser_content).await?;
    println!("LLM generated command: {}", command);

    // Step 4: Execute the command
    println!("\n--- Step 4: Execute command ---");
    match natbot.execute_command(&command).await {
        Ok(_) => {
            println!("Command executed successfully!");

            // Wait a moment for page to load
            tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

            // Crawl again to see new page state
            println!("\n--- Step 5: Crawl updated page ---");
            let updated_content = natbot.crawl().await?;
            println!("Page updated! Found {} elements", updated_content.len());

            // Show first few elements of updated page
            println!("\nFirst 5 elements on updated page:");
            for (i, element) in updated_content.iter().take(5).enumerate() {
                println!("  {}: {}", i + 1, element);
            }
        }
        Err(e) => {
            println!("Error executing command: {}", e);
            println!("This is expected in automated testing - LLM commands may vary");
        }
    }

    println!("\n--- Example Complete ---");
    println!("Note: This is a simplified example showing one interaction cycle.");
    println!("Real usage would involve multiple cycles until objective is achieved.\n");

    Ok(())
}
