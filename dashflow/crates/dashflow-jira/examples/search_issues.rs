//! Example demonstrating Jira issue search using JQL
//!
//! This example shows how to use the JiraSearchTool to search for issues
//! using various JQL (Jira Query Language) queries.
//!
//! ## Setup
//!
//! 1. Create a Jira API token: https://id.atlassian.com/manage-profile/security/api-tokens
//! 2. Set environment variables:
//!    ```bash
//!    export JIRA_BASE_URL="https://your-domain.atlassian.net"
//!    export JIRA_EMAIL="your-email@example.com"
//!    export JIRA_API_TOKEN="your-api-token"
//!    ```
//! 3. Run the example:
//!    ```bash
//!    cargo run --example search_issues
//!    ```

use dashflow::core::tools::{Tool, ToolInput};
use dashflow_jira::JiraSearchTool;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Load credentials from environment
    let base_url = std::env::var("JIRA_BASE_URL")
        .unwrap_or_else(|_| "https://example.atlassian.net".to_string());
    let email = std::env::var("JIRA_EMAIL").unwrap_or_else(|_| "user@example.com".to_string());
    let api_token =
        std::env::var("JIRA_API_TOKEN").unwrap_or_else(|_| "your-api-token".to_string());

    // Create the tool
    let tool = JiraSearchTool::new(&base_url, &email, &api_token);

    println!("=== Jira Search Tool Examples ===\n");

    // Example 1: Search for your assigned issues
    println!("Example 1: Your assigned issues that are not done");
    println!("JQL: assignee = currentUser() AND status != Done");
    let input1 = serde_json::json!({
        "jql": "assignee = currentUser() AND status != Done",
        "max_results": 5
    });
    match tool._call(ToolInput::Structured(input1)).await {
        Ok(result) => println!("{}\n", result),
        Err(e) => eprintln!("Error: {}\n", e),
    }

    // Example 2: Recent issues in a specific project
    println!("Example 2: Recent issues in project (replace PROJECT with your project key)");
    println!("JQL: project = PROJECT AND created >= -7d ORDER BY created DESC");
    let input2 = serde_json::json!({
        "jql": "project = PROJECT AND created >= -7d ORDER BY created DESC",
        "max_results": 5
    });
    match tool._call(ToolInput::Structured(input2)).await {
        Ok(result) => println!("{}\n", result),
        Err(e) => eprintln!("Error: {}\n", e),
    }

    // Example 3: High priority bugs
    println!("Example 3: High priority bugs");
    println!("JQL: priority = High AND issuetype = Bug");
    let input3 = serde_json::json!({
        "jql": "priority = High AND issuetype = Bug",
        "max_results": 5
    });
    match tool._call(ToolInput::Structured(input3)).await {
        Ok(result) => println!("{}\n", result),
        Err(e) => eprintln!("Error: {}\n", e),
    }

    // Example 4: Issues updated today
    println!("Example 4: Issues updated today");
    println!("JQL: updated >= startOfDay() ORDER BY updated DESC");
    let input4 = serde_json::json!({
        "jql": "updated >= startOfDay() ORDER BY updated DESC",
        "max_results": 10
    });
    match tool._call(ToolInput::Structured(input4)).await {
        Ok(result) => println!("{}\n", result),
        Err(e) => eprintln!("Error: {}\n", e),
    }

    // Example 5: Search with pagination
    println!("Example 5: Search with pagination");
    println!("JQL: order by created DESC (showing results 10-15)");
    let input5 = serde_json::json!({
        "jql": "order by created DESC",
        "max_results": 5,
        "start_at": 10
    });
    match tool._call(ToolInput::Structured(input5)).await {
        Ok(result) => println!("{}\n", result),
        Err(e) => eprintln!("Error: {}\n", e),
    }

    println!("=== Examples Complete ===");

    Ok(())
}
