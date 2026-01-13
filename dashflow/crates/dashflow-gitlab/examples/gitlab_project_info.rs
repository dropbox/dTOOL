//! Example demonstrating GitLab project information retrieval.
//!
//! This example shows how to:
//! 1. Get project details
//! 2. List project issues with different filters
//! 3. List project merge requests
//!
//! # Usage
//!
//! Set your GitLab token and run:
//! ```bash
//! export GITLAB_TOKEN="your_token_here"
//! export GITLAB_PROJECT="your_group/your_project"
//! cargo run --example gitlab_project_info
//! ```

use dashflow::core::tools::{Tool, ToolInput};
use dashflow_gitlab::{GetProjectTool, ListIssuesTool, ListMergeRequestsTool};
use serde_json::json;
use std::env;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Get configuration from environment
    let token = match env::var("GITLAB_TOKEN") {
        Ok(token) => token,
        Err(_) => {
            println!("GITLAB_TOKEN is not set.");
            println!("Set it to run this example:");
            println!("  export GITLAB_TOKEN=\"your_token_here\"");
            return Ok(());
        }
    };
    let project = env::var("GITLAB_PROJECT").unwrap_or_else(|_| "gitlab-org/gitlab".to_string());
    let gitlab_url = env::var("GITLAB_URL").unwrap_or_else(|_| "https://gitlab.com".to_string());

    println!("=== GitLab Project Information Example ===\n");
    println!("Project: {}", project);
    println!("GitLab URL: {}\n", gitlab_url);

    // 1. Get project details
    println!("1. Getting project details...");
    let project_tool = GetProjectTool::new(&gitlab_url, &project, &token)?;
    let project_input = ToolInput::Structured(json!({}));

    match project_tool._call(project_input).await {
        Ok(result) => {
            // Parse and display key information
            let json: serde_json::Value = serde_json::from_str(&result)?;
            println!("✓ Project Information:");
            if let Some(name) = json.get("name").and_then(|v| v.as_str()) {
                println!("  Name: {}", name);
            }
            if let Some(desc) = json.get("description").and_then(|v| v.as_str()) {
                println!("  Description: {}", desc);
            }
            if let Some(stars) = json.get("star_count").and_then(|v| v.as_u64()) {
                println!("  Stars: {}", stars);
            }
            if let Some(forks) = json.get("forks_count").and_then(|v| v.as_u64()) {
                println!("  Forks: {}", forks);
            }
            if let Some(url) = json.get("web_url").and_then(|v| v.as_str()) {
                println!("  URL: {}", url);
            }
            println!();
        }
        Err(e) => println!("✗ Error: {}\n", e),
    }

    // 2. List all issues (no filter)
    println!("2. Listing all issues...");
    let issues_tool = ListIssuesTool::new(&gitlab_url, &project, &token)?;
    let all_issues_input = ToolInput::Structured(json!({
        "state": "all"
    }));

    match issues_tool._call(all_issues_input).await {
        Ok(result) => {
            let json: Vec<serde_json::Value> =
                serde_json::from_str(result.split('\n').nth(1).unwrap_or("[]"))?;
            println!("✓ Found {} total issues\n", json.len());
        }
        Err(e) => println!("✗ Error: {}\n", e),
    }

    // 3. List opened issues only
    println!("3. Listing opened issues...");
    let opened_issues_input = ToolInput::Structured(json!({
        "state": "opened"
    }));

    match issues_tool._call(opened_issues_input).await {
        Ok(result) => {
            let first_line = result.split('\n').next().unwrap_or("");
            println!("✓ {}\n", first_line);
        }
        Err(e) => println!("✗ Error: {}\n", e),
    }

    // 4. List closed issues
    println!("4. Listing closed issues...");
    let closed_issues_input = ToolInput::Structured(json!({
        "state": "closed"
    }));

    match issues_tool._call(closed_issues_input).await {
        Ok(result) => {
            let first_line = result.split('\n').next().unwrap_or("");
            println!("✓ {}\n", first_line);
        }
        Err(e) => println!("✗ Error: {}\n", e),
    }

    // 5. List all merge requests
    println!("5. Listing all merge requests...");
    let mrs_tool = ListMergeRequestsTool::new(&gitlab_url, &project, &token)?;
    let all_mrs_input = ToolInput::Structured(json!({
        "state": "all"
    }));

    match mrs_tool._call(all_mrs_input).await {
        Ok(result) => {
            let first_line = result.split('\n').next().unwrap_or("");
            println!("✓ {}\n", first_line);
        }
        Err(e) => println!("✗ Error: {}\n", e),
    }

    // 6. List opened merge requests
    println!("6. Listing opened merge requests...");
    let opened_mrs_input = ToolInput::Structured(json!({
        "state": "opened"
    }));

    match mrs_tool._call(opened_mrs_input).await {
        Ok(result) => {
            let first_line = result.split('\n').next().unwrap_or("");
            println!("✓ {}\n", first_line);
        }
        Err(e) => println!("✗ Error: {}\n", e),
    }

    println!("=== Example Complete ===");
    println!("\nThis example provides a comprehensive view of project activity.");
    println!("You can modify filters to narrow down results (labels, milestones, etc.).");

    Ok(())
}
