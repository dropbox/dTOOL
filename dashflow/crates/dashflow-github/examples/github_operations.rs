//! Example demonstrating GitHub tool usage.
//!
//! This example shows how to use various GitHub tools to interact with repositories.
//!
//! # Setup
//!
//! 1. Create a GitHub personal access token at https://github.com/settings/tokens
//! 2. Set the token as an environment variable: `export GITHUB_TOKEN=your_token`
//! 3. Run: `cargo run --example github_operations`
//!
//! # Note
//!
//! This example uses the "octocat/Hello-World" repository for read-only operations.
//! Modify the owner/repo parameters to test on your own repositories.

use dashflow::core::tools::{Tool, ToolInput};
use dashflow_github::{
    GetIssueTool, GetPRTool, ReadFileTool, SearchCodeTool, SearchIssuesAndPRsTool,
};
use serde_json::json;
use std::env;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Get GitHub token from environment
    let token = env::var("GITHUB_TOKEN").unwrap_or_else(|_| {
        eprintln!("Warning: GITHUB_TOKEN not set. Using placeholder (API calls will fail).");
        "placeholder_token".to_string()
    });

    let owner = "octocat";
    let repo = "Hello-World";

    println!("=== GitHub Tools Example ===\n");

    // Example 1: Get issue details
    println!("1. Getting issue #1 details...");
    let get_issue = GetIssueTool::new(owner, repo, &token);
    let input = json!({"issue_number": 1});

    match get_issue._call(ToolInput::Structured(input)).await {
        Ok(result) => println!("Issue details:\n{}\n", result),
        Err(e) => eprintln!("Error getting issue: {}\n", e),
    }

    // Example 2: Search issues and PRs
    println!("2. Searching for issues with 'bug' label...");
    let search_issues = SearchIssuesAndPRsTool::new(owner, repo, &token);
    let input = json!({"query": "is:issue label:bug", "per_page": 5});

    match search_issues._call(ToolInput::Structured(input)).await {
        Ok(result) => println!("Search results:\n{}\n", result),
        Err(e) => eprintln!("Error searching issues: {}\n", e),
    }

    // Example 3: Read a file
    println!("3. Reading README.md...");
    let read_file = ReadFileTool::new(owner, repo, &token);
    let input = json!({"path": "README", "ref": "master"});

    match read_file._call(ToolInput::Structured(input)).await {
        Ok(content) => {
            println!("File content (first 200 chars):");
            let preview = if content.len() > 200 {
                &content[..200]
            } else {
                &content
            };
            println!("{}\n", preview);
        }
        Err(e) => eprintln!("Error reading file: {}\n", e),
    }

    // Example 4: Search code
    println!("4. Searching for code containing 'hello'...");
    let search_code = SearchCodeTool::new(owner, repo, &token);
    let input = json!({"query": "hello", "per_page": 3});

    match search_code._call(ToolInput::Structured(input)).await {
        Ok(result) => println!("Code search results:\n{}\n", result),
        Err(e) => eprintln!("Error searching code: {}\n", e),
    }

    // Example 5: Get PR details (will fail if no PR #1 exists)
    println!("5. Getting PR #1 details (if it exists)...");
    let get_pr = GetPRTool::new(owner, repo, &token);
    let input = json!({"pr_number": 1});

    match get_pr._call(ToolInput::Structured(input)).await {
        Ok(result) => println!("PR details:\n{}\n", result),
        Err(e) => eprintln!("Note: {}\n", e),
    }

    // Example 6: Create PR (commented out - requires write permissions and valid branches)
    println!("6. Create PR example (commented out - requires write permissions)");
    println!("   To use: uncomment code and provide your own repository with valid branches\n");
    /*
    let create_pr = CreatePRTool::new("your_owner", "your_repo", &token);
    let input = json!({
        "title": "Example PR from Rust",
        "head": "feature-branch",
        "base": "main",
        "body": "This is an example PR created using dashflow-github"
    });

    match create_pr._call(ToolInput::Structured(input)).await {
        Ok(result) => println!("PR created:\n{}\n", result),
        Err(e) => eprintln!("Error creating PR: {}\n", e),
    }
    */

    // Example 7: Comment on issue (commented out - requires write permissions)
    println!("7. Comment on issue example (commented out - requires write permissions)");
    println!("   To use: uncomment code and provide your own repository\n");
    /*
    let comment_on_issue = CommentOnIssueTool::new("your_owner", "your_repo", &token);
    let input = json!({
        "issue_number": 1,
        "comment": "This is a comment from dashflow-github Rust tools!"
    });

    match comment_on_issue._call(ToolInput::Structured(input)).await {
        Ok(result) => println!("Comment added:\n{}\n", result),
        Err(e) => eprintln!("Error commenting: {}\n", e),
    }
    */

    println!("=== Example complete ===");
    Ok(())
}
