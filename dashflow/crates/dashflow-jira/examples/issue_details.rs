//! Example demonstrating how to get detailed Jira issue information
//!
//! This example shows how to use the JiraIssueTool to retrieve detailed
//! information about a specific Jira issue.
//!
//! ## Setup
//!
//! 1. Create a Jira API token: https://id.atlassian.com/manage-profile/security/api-tokens
//! 2. Set environment variables:
//!    ```bash
//!    export JIRA_BASE_URL="https://your-domain.atlassian.net"
//!    export JIRA_EMAIL="your-email@example.com"
//!    export JIRA_API_TOKEN="your-api-token"
//!    export JIRA_ISSUE_KEY="DEMO-123"  # Optional: specific issue to query
//!    ```
//! 3. Run the example:
//!    ```bash
//!    cargo run --example issue_details
//!    ```

use dashflow::core::tools::{Tool, ToolInput};
use dashflow_jira::JiraIssueTool;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Load credentials from environment
    let base_url = std::env::var("JIRA_BASE_URL")
        .unwrap_or_else(|_| "https://example.atlassian.net".to_string());
    let email = std::env::var("JIRA_EMAIL").unwrap_or_else(|_| "user@example.com".to_string());
    let api_token =
        std::env::var("JIRA_API_TOKEN").unwrap_or_else(|_| "your-api-token".to_string());
    let issue_key = std::env::var("JIRA_ISSUE_KEY").unwrap_or_else(|_| "DEMO-123".to_string());

    // Create the tool
    let tool = JiraIssueTool::new(&base_url, &email, &api_token);

    println!("=== Jira Issue Tool Examples ===\n");

    // Example 1: Get issue details with structured input
    println!("Example 1: Get issue details (structured input)");
    println!("Issue Key: {}", issue_key);
    let input1 = serde_json::json!({
        "issue_key": issue_key
    });
    match tool._call(ToolInput::Structured(input1)).await {
        Ok(result) => println!("{}\n", result),
        Err(e) => eprintln!("Error: {}\n", e),
    }

    // Example 2: Get issue details with string input
    println!("Example 2: Get issue details (string input)");
    println!("Issue Key: {}", issue_key);
    match tool._call(ToolInput::String(issue_key.clone())).await {
        Ok(result) => println!("{}\n", result),
        Err(e) => eprintln!("Error: {}\n", e),
    }

    // Example 3: Try to get a non-existent issue (error handling)
    println!("Example 3: Error handling (non-existent issue)");
    let fake_issue = "NONEXISTENT-999";
    println!("Issue Key: {}", fake_issue);
    match tool._call(ToolInput::String(fake_issue.to_string())).await {
        Ok(result) => println!("{}\n", result),
        Err(e) => eprintln!("Expected error: {}\n", e),
    }

    println!("=== Examples Complete ===");

    Ok(())
}
