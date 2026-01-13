//! File search example
//!
//! Demonstrates searching for files using patterns.
//!
//! Run with: cargo run --example file_search

use dashflow::core::tools::{Tool, ToolInput};
use dashflow_file_tool::{FileSearchTool, WriteFileTool};
use serde_json::json;
use tempfile::TempDir;
use tokio::fs;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== File Search Tool Example ===\n");

    // Create a temporary directory with test files
    let temp_dir = TempDir::new()?;
    let temp_path = temp_dir.path().to_path_buf();
    println!("Using temporary directory: {}\n", temp_path.display());

    // Create a test file structure
    println!("1. Creating test file structure...");
    let write_tool = WriteFileTool::new().with_allowed_dirs(vec![temp_path.clone()]);

    // Root level files
    for (name, content) in [
        ("readme.txt", "Project README"),
        ("config.json", r#"{"key": "value"}"#),
        ("data1.csv", "col1,col2\n1,2"),
        ("data2.csv", "col1,col2\n3,4"),
    ] {
        let file_path = temp_path.join(name);
        let input = json!({
            "file_path": file_path.to_string_lossy(),
            "text": content
        });
        write_tool._call(ToolInput::Structured(input)).await?;
    }

    // Create subdirectory with more files
    let subdir = temp_path.join("logs");
    fs::create_dir(&subdir).await?;

    for (name, content) in [
        ("app.log", "Application log\n"),
        ("error.log", "Error log\n"),
        ("access.txt", "Access records\n"),
    ] {
        let file_path = subdir.join(name);
        let input = json!({
            "file_path": file_path.to_string_lossy(),
            "text": content
        });
        write_tool._call(ToolInput::Structured(input)).await?;
    }

    // Create nested subdirectory
    let nested_dir = subdir.join("archive");
    fs::create_dir(&nested_dir).await?;

    let archive_file = nested_dir.join("old.log");
    let input = json!({
        "file_path": archive_file.to_string_lossy(),
        "text": "Archived log\n"
    });
    write_tool._call(ToolInput::Structured(input)).await?;

    println!("   Created 8 files in nested directory structure\n");

    // Create search tool
    let search_tool = FileSearchTool::new().with_allowed_dirs(vec![temp_path.clone()]);

    // Example 1: Search for all .txt files
    println!("2. Searching for all .txt files...");
    let search_input = json!({
        "dir_path": temp_path.to_string_lossy(),
        "pattern": "*.txt"
    });
    let result = search_tool
        ._call(ToolInput::Structured(search_input))
        .await?;
    println!("   {}\n", result.replace('\n', "\n   "));

    // Example 2: Search for all .log files (recursive)
    println!("3. Searching for all .log files (recursive)...");
    let search_input = json!({
        "dir_path": temp_path.to_string_lossy(),
        "pattern": "*.log"
    });
    let result = search_tool
        ._call(ToolInput::Structured(search_input))
        .await?;
    println!("   {}\n", result.replace('\n', "\n   "));

    // Example 3: Search for CSV files
    println!("4. Searching for CSV files...");
    let search_input = json!({
        "dir_path": temp_path.to_string_lossy(),
        "pattern": "*.csv"
    });
    let result = search_tool
        ._call(ToolInput::Structured(search_input))
        .await?;
    println!("   {}\n", result.replace('\n', "\n   "));

    // Example 4: Search with wildcard (data*.csv)
    println!("5. Searching for files matching 'data*.csv'...");
    let search_input = json!({
        "dir_path": temp_path.to_string_lossy(),
        "pattern": "data*.csv"
    });
    let result = search_tool
        ._call(ToolInput::Structured(search_input))
        .await?;
    println!("   {}\n", result.replace('\n', "\n   "));

    // Example 5: Search with ? wildcard
    println!("6. Searching for 'data?.csv' (single character wildcard)...");
    let search_input = json!({
        "dir_path": temp_path.to_string_lossy(),
        "pattern": "data?.csv"
    });
    let result = search_tool
        ._call(ToolInput::Structured(search_input))
        .await?;
    println!("   {}\n", result.replace('\n', "\n   "));

    // Example 6: Search in subdirectory only
    println!("7. Searching in logs/ subdirectory only...");
    let search_input = json!({
        "dir_path": subdir.to_string_lossy(),
        "pattern": "*.log"
    });
    let result = search_tool
        ._call(ToolInput::Structured(search_input))
        .await?;
    println!("   {}\n", result.replace('\n', "\n   "));

    // Example 7: Search for non-existent pattern
    println!("8. Searching for non-existent pattern '*.xyz'...");
    let search_input = json!({
        "dir_path": temp_path.to_string_lossy(),
        "pattern": "*.xyz"
    });
    let result = search_tool
        ._call(ToolInput::Structured(search_input))
        .await?;
    println!("   {}\n", result);

    println!("\n=== Example Complete ===");
    println!("\nKey Features:");
    println!("- Recursive search through all subdirectories");
    println!("- Wildcard support: * (any chars) and ? (single char)");
    println!("- Can search from any starting directory");
    println!("- Security: Respects allowed directory boundaries");

    Ok(())
}
