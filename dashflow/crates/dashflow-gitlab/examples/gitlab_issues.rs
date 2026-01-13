//! Example demonstrating GitLab issue management workflow.
//!
//! This example shows how to:
//! 1. List existing issues
//! 2. Create a new issue
//! 3. Get issue details
//! 4. Update an issue
//! 5. Close an issue
//!
//! # Usage
//!
//! Set your GitLab token and run:
//! ```bash
//! export GITLAB_TOKEN="your_token_here"
//! export GITLAB_PROJECT="your_group/your_project"
//! cargo run --example gitlab_issues
//! ```

use dashflow::core::tools::{Tool, ToolInput};
use dashflow_gitlab::{CreateIssueTool, GetIssueTool, ListIssuesTool, UpdateIssueTool};
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

    println!("=== GitLab Issue Management Example ===\n");
    println!("Project: {}", project);
    println!("GitLab URL: {}\n", gitlab_url);

    // 1. List existing issues
    println!("1. Listing opened issues...");
    let list_tool = ListIssuesTool::new(&gitlab_url, &project, &token)?;
    let list_input = ToolInput::Structured(json!({
        "state": "opened"
    }));

    match list_tool._call(list_input).await {
        Ok(result) => println!("✓ {}\n", result),
        Err(e) => println!("✗ Error: {}\n", e),
    }

    // 2. Create a new issue
    println!("2. Creating a new issue...");
    let create_tool = CreateIssueTool::new(&gitlab_url, &project, &token)?;
    let create_input = ToolInput::Structured(json!({
        "title": "Example Issue: Test from DashFlow",
        "description": "This is a test issue created by the DashFlow GitLab integration.\n\n**Created by:** dashflow-gitlab example\n**Purpose:** Demonstrating issue creation"
    }));

    let issue_iid = match create_tool._call(create_input).await {
        Ok(result) => {
            println!("✓ {}\n", result);
            // Try to extract IID from response (for demonstration)
            let json: serde_json::Value = serde_json::from_str(&result)?;
            json.get("iid").and_then(|v| v.as_u64()).unwrap_or(1)
        }
        Err(e) => {
            println!("✗ Error: {}\n", e);
            println!("Note: If creation failed (likely due to permissions), continuing with IID=1 for demonstration\n");
            1
        }
    };

    // 3. Get issue details
    println!("3. Getting issue details (IID: {})...", issue_iid);
    let get_tool = GetIssueTool::new(&gitlab_url, &project, &token)?;
    let get_input = ToolInput::Structured(json!({
        "issue_iid": issue_iid
    }));

    match get_tool._call(get_input).await {
        Ok(result) => println!("✓ {}\n", result),
        Err(e) => println!("✗ Error: {}\n", e),
    }

    // 4. Update the issue
    println!("4. Updating issue title (IID: {})...", issue_iid);
    let update_tool = UpdateIssueTool::new(&gitlab_url, &project, &token)?;
    let update_input = ToolInput::Structured(json!({
        "issue_iid": issue_iid,
        "title": "Example Issue: Test from DashFlow [UPDATED]",
        "description": "This issue has been updated by the DashFlow GitLab integration.\n\n**Status:** Updated\n**Last modified by:** dashflow-gitlab example"
    }));

    match update_tool._call(update_input).await {
        Ok(result) => println!("✓ {}\n", result),
        Err(e) => println!("✗ Error: {}\n", e),
    }

    // 5. Close the issue
    println!("5. Closing issue (IID: {})...", issue_iid);
    let close_input = ToolInput::Structured(json!({
        "issue_iid": issue_iid,
        "state_event": "close"
    }));

    match update_tool._call(close_input).await {
        Ok(result) => println!("✓ {}\n", result),
        Err(e) => println!("✗ Error: {}\n", e),
    }

    println!("=== Example Complete ===");
    println!("\nNote: Some operations may fail due to permissions.");
    println!("Ensure your token has 'api' scope for full functionality.");

    Ok(())
}
