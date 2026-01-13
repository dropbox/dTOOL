//! Example demonstrating GitLab merge request workflow.
//!
//! This example shows how to:
//! 1. List existing merge requests
//! 2. Create a new merge request
//! 3. Get merge request details
//!
//! # Usage
//!
//! Set your GitLab token and run:
//! ```bash
//! export GITLAB_TOKEN="your_token_here"
//! export GITLAB_PROJECT="your_group/your_project"
//! cargo run --example gitlab_merge_requests
//! ```

use dashflow::core::tools::{Tool, ToolInput};
use dashflow_gitlab::{CreateMergeRequestTool, GetMergeRequestTool, ListMergeRequestsTool};
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

    println!("=== GitLab Merge Request Management Example ===\n");
    println!("Project: {}", project);
    println!("GitLab URL: {}\n", gitlab_url);

    // 1. List existing merge requests
    println!("1. Listing opened merge requests...");
    let list_tool = ListMergeRequestsTool::new(&gitlab_url, &project, &token)?;
    let list_input = ToolInput::Structured(json!({
        "state": "opened"
    }));

    match list_tool._call(list_input).await {
        Ok(result) => println!("✓ {}\n", result),
        Err(e) => println!("✗ Error: {}\n", e),
    }

    // 2. List merged requests (for comparison)
    println!("2. Listing merged merge requests...");
    let merged_input = ToolInput::Structured(json!({
        "state": "merged"
    }));

    match list_tool._call(merged_input).await {
        Ok(result) => println!("✓ {}\n", result),
        Err(e) => println!("✗ Error: {}\n", e),
    }

    // 3. Create a new merge request (likely to fail without proper branches)
    println!("3. Creating a new merge request...");
    let create_tool = CreateMergeRequestTool::new(&gitlab_url, &project, &token)?;
    let create_input = ToolInput::Structured(json!({
        "title": "Example MR: Test from DashFlow",
        "source_branch": "feature/dashflow-test",
        "target_branch": "main",
        "description": "This is a test merge request created by the DashFlow GitLab integration.\n\n**Created by:** dashflow-gitlab example\n**Purpose:** Demonstrating MR creation"
    }));

    let mr_iid = match create_tool._call(create_input).await {
        Ok(result) => {
            println!("✓ {}\n", result);
            // Try to extract IID from response
            let json: serde_json::Value = serde_json::from_str(&result)?;
            json.get("iid").and_then(|v| v.as_u64()).unwrap_or(1)
        }
        Err(e) => {
            println!("✗ Error: {}", e);
            println!("Note: MR creation likely failed because source branch doesn't exist.");
            println!("Continuing with IID=1 for demonstration\n");
            1
        }
    };

    // 4. Get merge request details
    println!("4. Getting merge request details (IID: {})...", mr_iid);
    let get_tool = GetMergeRequestTool::new(&gitlab_url, &project, &token)?;
    let get_input = ToolInput::Structured(json!({
        "merge_request_iid": mr_iid
    }));

    match get_tool._call(get_input).await {
        Ok(result) => println!("✓ {}\n", result),
        Err(e) => println!("✗ Error: {}\n", e),
    }

    println!("=== Example Complete ===");
    println!("\nNote: MR creation requires valid source/target branches.");
    println!("To successfully create MRs, ensure branches exist in your project.");

    Ok(())
}
