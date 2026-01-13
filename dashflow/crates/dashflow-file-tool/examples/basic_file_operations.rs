//! Basic file operations example
//!
//! Demonstrates reading, writing, listing, and copying files.
//!
//! Run with: cargo run --example basic_file_operations

use dashflow::core::tools::{Tool, ToolInput};
use dashflow_file_tool::{
    CopyFileTool, DeleteFileTool, ListDirectoryTool, ReadFileTool, WriteFileTool,
};
use serde_json::json;
use std::path::PathBuf;
use tempfile::TempDir;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== File Management Tools Example ===\n");

    // Create a temporary directory for demonstration
    let temp_dir = TempDir::new()?;
    let temp_path = temp_dir.path().to_path_buf();
    println!("Using temporary directory: {}\n", temp_path.display());

    // Configure all tools with allowed directories
    let read_tool = ReadFileTool::new().with_allowed_dirs(vec![temp_path.clone()]);
    let write_tool = WriteFileTool::new().with_allowed_dirs(vec![temp_path.clone()]);
    let list_tool = ListDirectoryTool::new().with_allowed_dirs(vec![temp_path.clone()]);
    let copy_tool = CopyFileTool::new().with_allowed_dirs(vec![temp_path.clone()]);
    let delete_tool = DeleteFileTool::new().with_allowed_dirs(vec![temp_path.clone()]);

    // Example 1: Write a file
    println!("1. Writing a file...");
    let file1 = temp_path.join("greeting.txt");
    let write_input = json!({
        "file_path": file1.to_string_lossy(),
        "text": "Hello, World!\nWelcome to DashFlow file tools."
    });
    let result = write_tool._call(ToolInput::Structured(write_input)).await?;
    println!("   {}\n", result);

    // Example 2: Read the file
    println!("2. Reading the file...");
    let read_input = json!({"file_path": file1.to_string_lossy()});
    let content = read_tool._call(ToolInput::Structured(read_input)).await?;
    println!("   Content:\n   {}\n", content.replace('\n', "\n   "));

    // Example 3: Append to the file
    println!("3. Appending to the file...");
    let append_input = json!({
        "file_path": file1.to_string_lossy(),
        "text": "\nThis line was appended!",
        "append": true
    });
    write_tool._call(ToolInput::Structured(append_input)).await?;

    let content = read_tool
        ._call(ToolInput::String(file1.to_string_lossy().into_owned()))
        .await?;
    println!("   New content:\n   {}\n", content.replace('\n', "\n   "));

    // Example 4: Create another file
    println!("4. Creating another file...");
    let file2 = temp_path.join("notes.txt");
    let write_input2 = json!({
        "file_path": file2.to_string_lossy(),
        "text": "Important notes:\n- File tools work with async I/O\n- Security controls via allowlists"
    });
    write_tool._call(ToolInput::Structured(write_input2)).await?;
    println!("   Created notes.txt\n");

    // Example 5: List directory contents
    println!("5. Listing directory contents...");
    let list_input = json!({"dir_path": temp_path.to_string_lossy()});
    let listing = list_tool._call(ToolInput::Structured(list_input)).await?;
    println!("   {}\n", listing.replace('\n', "\n   "));

    // Example 6: Copy a file
    println!("6. Copying greeting.txt to backup.txt...");
    let backup_file = temp_path.join("backup.txt");
    let copy_input = json!({
        "source_path": file1.to_string_lossy(),
        "destination_path": backup_file.to_string_lossy()
    });
    let result = copy_tool._call(ToolInput::Structured(copy_input)).await?;
    println!("   {}\n", result);

    // Example 7: Verify the copy
    println!("7. Reading the backup file...");
    let read_backup = json!({"file_path": backup_file.to_string_lossy()});
    let backup_content = read_tool._call(ToolInput::Structured(read_backup)).await?;
    println!("   Content matches: {}\n", backup_content == content);

    // Example 8: Delete a file
    println!("8. Deleting notes.txt...");
    let delete_input = json!({"file_path": file2.to_string_lossy()});
    let result = delete_tool
        ._call(ToolInput::Structured(delete_input))
        .await?;
    println!("   {}\n", result);

    // Example 9: Final directory listing
    println!("9. Final directory listing...");
    let final_listing = list_tool
        ._call(ToolInput::String(temp_path.to_string_lossy().into_owned()))
        .await?;
    println!("   {}\n", final_listing.replace('\n', "\n   "));

    // Example 10: Test security - try to read outside allowed directory
    println!("10. Testing security boundaries...");
    let restricted_tool = ReadFileTool::new().with_allowed_dirs(vec![PathBuf::from("/tmp")]);
    let unauthorized_read = json!({"file_path": file1.to_string_lossy()});
    match restricted_tool
        ._call(ToolInput::Structured(unauthorized_read))
        .await
    {
        Ok(_) => println!("    WARNING: Security check failed!"),
        Err(e) => println!("    Security check passed: {}", e),
    }

    println!("\n=== Example Complete ===");
    Ok(())
}
