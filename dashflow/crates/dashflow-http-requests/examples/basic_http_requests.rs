//! Basic HTTP requests example
//!
//! This example demonstrates using HTTP request tools to interact with REST APIs.
//!
//! Run with:
//! ```bash
//! cargo run --example basic_http_requests
//! ```

use dashflow::core::tools::Tool;
use dashflow_http_requests::{
    HttpDeleteTool, HttpGetTool, HttpPatchTool, HttpPostTool, HttpPutTool,
};
use serde_json::json;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== HTTP Requests Tools Example ===\n");

    // Example 1: HTTP GET - Retrieve data
    println!("1. HTTP GET Request");
    println!("-------------------");
    let get_tool = HttpGetTool::new();
    println!("Tool: {}", get_tool.name());
    println!("Description: {}", get_tool.description());

    // Using JSONPlaceholder as a test API
    let get_request = json!({
        "url": "https://jsonplaceholder.typicode.com/posts/1",
        "headers": {
            "User-Agent": "DashFlow-Rust/0.1"
        },
        "timeout": 10
    })
    .to_string();

    match get_tool._call_str(get_request).await {
        Ok(response) => {
            println!("\nGET Response:");
            println!("{}", response);
        }
        Err(e) => eprintln!("GET Error: {}", e),
    }

    println!("\n");

    // Example 2: HTTP POST - Create new resource
    println!("2. HTTP POST Request");
    println!("--------------------");
    let post_tool = HttpPostTool::new();
    println!("Tool: {}", post_tool.name());

    let post_request = json!({
        "url": "https://jsonplaceholder.typicode.com/posts",
        "data": {
            "title": "Test Post from DashFlow",
            "body": "This is a test post created using HTTP POST tool",
            "userId": 1
        },
        "headers": {
            "Content-Type": "application/json",
            "User-Agent": "DashFlow-Rust/0.1"
        }
    })
    .to_string();

    match post_tool._call_str(post_request).await {
        Ok(response) => {
            println!("\nPOST Response:");
            println!("{}", response);
        }
        Err(e) => eprintln!("POST Error: {}", e),
    }

    println!("\n");

    // Example 3: HTTP PUT - Full update
    println!("3. HTTP PUT Request");
    println!("-------------------");
    let put_tool = HttpPutTool::new();
    println!("Tool: {}", put_tool.name());

    let put_request = json!({
        "url": "https://jsonplaceholder.typicode.com/posts/1",
        "data": {
            "id": 1,
            "title": "Updated Post Title",
            "body": "This is the updated body of the post",
            "userId": 1
        },
        "headers": {
            "Content-Type": "application/json"
        }
    })
    .to_string();

    match put_tool._call_str(put_request).await {
        Ok(response) => {
            println!("\nPUT Response:");
            println!("{}", response);
        }
        Err(e) => eprintln!("PUT Error: {}", e),
    }

    println!("\n");

    // Example 4: HTTP PATCH - Partial update
    println!("4. HTTP PATCH Request");
    println!("---------------------");
    let patch_tool = HttpPatchTool::new();
    println!("Tool: {}", patch_tool.name());

    let patch_request = json!({
        "url": "https://jsonplaceholder.typicode.com/posts/1",
        "data": {
            "title": "Partially Updated Title"
        },
        "headers": {
            "Content-Type": "application/json"
        }
    })
    .to_string();

    match patch_tool._call_str(patch_request).await {
        Ok(response) => {
            println!("\nPATCH Response:");
            println!("{}", response);
        }
        Err(e) => eprintln!("PATCH Error: {}", e),
    }

    println!("\n");

    // Example 5: HTTP DELETE - Remove resource
    println!("5. HTTP DELETE Request");
    println!("----------------------");
    let delete_tool = HttpDeleteTool::new();
    println!("Tool: {}", delete_tool.name());

    let delete_request = json!({
        "url": "https://jsonplaceholder.typicode.com/posts/1",
        "headers": {
            "User-Agent": "DashFlow-Rust/0.1"
        }
    })
    .to_string();

    match delete_tool._call_str(delete_request).await {
        Ok(response) => {
            println!("\nDELETE Response:");
            println!("{}", response);
        }
        Err(e) => eprintln!("DELETE Error: {}", e),
    }

    println!("\n=== Example Complete ===");
    Ok(())
}
