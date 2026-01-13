//! Comprehensive examples for Shell Tool with security configurations.

use dashflow::core::tools::{Tool, ToolInput};
use dashflow_shell_tool::ShellTool;
use serde_json::json;
use std::path::PathBuf;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Shell Tool Examples ===\n");

    // Example 1: Basic command with allowlist (RECOMMENDED)
    println!("1. Basic Command with Allowlist");
    println!("{}", "-".repeat(50));
    let safe_tool = ShellTool::new()
        .with_allowed_commands(vec![
            "echo".to_string(),
            "ls".to_string(),
            "pwd".to_string(),
        ])
        .with_timeout(5);

    let input1 = json!({"command": "echo Hello from shell tool!"});
    match safe_tool._call(ToolInput::Structured(input1)).await {
        Ok(output) => println!("Output: {}\n", output),
        Err(e) => println!("Error: {}\n", e),
    }

    // Example 2: Security - Blocked command
    println!("2. Security - Blocked Command");
    println!("{}", "-".repeat(50));
    let input2 = json!({"command": "cat /etc/passwd"});
    match safe_tool._call(ToolInput::Structured(input2)).await {
        Ok(output) => println!("Output: {}\n", output),
        Err(e) => println!("Expected security error: {}\n", e),
    }

    // Example 3: Prefix-based allowlist (Git commands)
    println!("3. Prefix-Based Allowlist (Git Commands)");
    println!("{}", "-".repeat(50));
    let git_tool = ShellTool::new()
        .with_allowed_prefixes(vec!["git ".to_string()])
        .with_timeout(10);

    // This would work if in a git repository
    let input3 = json!({"command": "git --version"});
    match git_tool._call(ToolInput::Structured(input3)).await {
        Ok(output) => println!("Output: {}\n", output),
        Err(e) => println!("Error: {}\n", e),
    }

    // Example 4: Working directory restriction
    println!("4. Working Directory Restriction");
    println!("{}", "-".repeat(50));
    let dir_tool = ShellTool::new()
        .with_working_dir(PathBuf::from("/tmp"))
        .with_allowed_commands(vec!["pwd".to_string()]);

    let input4 = json!({"command": "pwd"});
    match dir_tool._call(ToolInput::Structured(input4)).await {
        Ok(output) => println!("Working directory: {}\n", output),
        Err(e) => println!("Error: {}\n", e),
    }

    // Example 5: Timeout protection
    println!("5. Timeout Protection (1 second timeout)");
    println!("{}", "-".repeat(50));
    let timeout_tool = ShellTool::new()
        .with_allowed_commands(vec!["sleep".to_string()])
        .with_timeout(1);

    // This will timeout
    let input5 = if cfg!(target_os = "windows") {
        json!({"command": "timeout /t 3"})
    } else {
        json!({"command": "sleep 3"})
    };

    match timeout_tool._call(ToolInput::Structured(input5)).await {
        Ok(output) => println!("Output: {}\n", output),
        Err(e) => println!("Expected timeout error: {}\n", e),
    }

    // Example 6: Capturing stderr
    println!("6. Capturing stderr");
    println!("{}", "-".repeat(50));
    let stderr_tool = ShellTool::new().with_allowed_commands(vec!["sh".to_string()]);

    let input6 = if cfg!(target_os = "windows") {
        json!({"command": "echo Error message 1>&2"})
    } else {
        json!({"command": "sh -c 'echo \"Error message\" >&2'"})
    };

    match stderr_tool._call(ToolInput::Structured(input6)).await {
        Ok(output) => println!("Output with stderr:\n{}\n", output),
        Err(e) => println!("Error: {}\n", e),
    }

    // Example 7: Non-zero exit code
    println!("7. Non-Zero Exit Code");
    println!("{}", "-".repeat(50));
    let exit_tool = ShellTool::new().with_allowed_commands(vec!["sh".to_string()]);

    let input7 = if cfg!(target_os = "windows") {
        json!({"command": "exit 42"})
    } else {
        json!({"command": "sh -c 'exit 42'"})
    };

    match exit_tool._call(ToolInput::Structured(input7)).await {
        Ok(output) => println!("Output (with exit code):\n{}\n", output),
        Err(e) => println!("Error: {}\n", e),
    }

    // Example 8: String input (alternative to structured)
    println!("8. String Input Format");
    println!("{}", "-".repeat(50));
    let string_tool = ShellTool::new().with_allowed_commands(vec!["echo".to_string()]);

    match string_tool
        ._call(ToolInput::String("echo String input works!".to_string()))
        .await
    {
        Ok(output) => println!("Output: {}\n", output),
        Err(e) => println!("Error: {}\n", e),
    }

    // Example 9: Output truncation
    println!("9. Output Truncation (100 byte limit)");
    println!("{}", "-".repeat(50));
    let truncate_tool = ShellTool::new()
        .with_allowed_commands(vec!["echo".to_string()])
        .with_max_output_bytes(100);

    // Generate long output
    let long_text = "A".repeat(200);
    let input9 = json!({"command": format!("echo {}", long_text)});

    match truncate_tool._call(ToolInput::Structured(input9)).await {
        Ok(output) => {
            println!("Output (truncated):\n{}", output);
            println!("Output length: {} characters\n", output.len());
        }
        Err(e) => println!("Error: {}\n", e),
    }

    // Example 10: Multiple restrictions combined
    println!("10. Multiple Security Restrictions Combined");
    println!("{}", "-".repeat(50));
    let secure_tool = ShellTool::new()
        .with_allowed_commands(vec!["echo".to_string(), "date".to_string()])
        .with_timeout(5)
        .with_max_output_bytes(1024)
        .with_working_dir(PathBuf::from("/tmp"));

    println!("Security configuration:");
    println!("  - Allowed commands: echo, date");
    println!("  - Timeout: 5 seconds");
    println!("  - Max output: 1024 bytes");
    println!("  - Working dir: /tmp");
    println!();

    let input10 = json!({"command": "date"});
    match secure_tool._call(ToolInput::Structured(input10)).await {
        Ok(output) => println!("Output: {}\n", output),
        Err(e) => println!("Error: {}\n", e),
    }

    println!("=== All examples completed ===");
    Ok(())
}
