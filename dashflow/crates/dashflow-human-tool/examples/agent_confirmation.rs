//! Agent with human confirmation example.
//!
//! This example demonstrates how an AI agent can use the HumanTool to get
//! confirmation from a user before performing important actions.
//!
//! Run with: cargo run --example agent_confirmation

use dashflow::core::tools::{Tool, ToolInput};
use dashflow_human_tool::HumanTool;
use serde_json::json;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== AI Agent with Human Confirmation ===\n");

    let human_tool = HumanTool::new();

    // Simulate an AI agent that needs to perform critical actions
    println!("Agent: I'm analyzing your system and found several actions to perform.");
    println!("Agent: Let me walk you through them and get your confirmation.\n");

    // Action 1: Delete old files
    println!("Agent: Action 1 - Delete files older than 90 days");
    println!("Agent: Found 1,247 files (12.3 GB) that can be cleaned up.");
    let response = human_tool
        ._call(ToolInput::String(
            "Should I delete these files? (yes/no)".to_string(),
        ))
        .await?;

    if response.to_lowercase().contains("yes") {
        println!("Agent: ✅ Files deleted successfully.\n");
    } else {
        println!("Agent: Declined file deletion.\n");
    }

    // Action 2: Update packages
    println!("Agent: Action 2 - Update system packages");
    println!("Agent: 23 packages have updates available.");
    let input = json!({
        "prompt": "Should I update all packages? (yes/no)"
    });
    let response = human_tool._call(ToolInput::Structured(input)).await?;

    if response.to_lowercase().contains("yes") {
        println!("Agent: ✅ Packages updated successfully.\n");
    } else {
        println!("Agent: Declined package updates.\n");
    }

    // Action 3: Restart service
    println!("Agent: Action 3 - Restart database service");
    println!("Agent: This will cause ~30 seconds of downtime.");
    let response = human_tool
        ._call(ToolInput::String(
            "Should I restart the service? (yes/no)".to_string(),
        ))
        .await?;

    if response.to_lowercase().contains("yes") {
        println!("Agent: ✅ Service restarted successfully.\n");
    } else {
        println!("Agent: Declined service restart.\n");
    }

    // Action 4: Custom action
    println!("Agent: Action 4 - Custom deployment");
    println!("Agent: I can deploy the application to production or staging.");
    let response = human_tool
        ._call(ToolInput::String(
            "Which environment? (production/staging/skip)".to_string(),
        ))
        .await?;

    match response.to_lowercase().as_str() {
        s if s.contains("production") => {
            println!("Agent: ✅ Deployed to production.\n");
        }
        s if s.contains("staging") => {
            println!("Agent: ✅ Deployed to staging.\n");
        }
        _ => {
            println!("Agent: Declined deployment.\n");
        }
    }

    // Final summary
    println!("Agent: All actions completed. Generating summary...");
    let response = human_tool
        ._call(ToolInput::String(
            "Would you like a detailed report? (yes/no)".to_string(),
        ))
        .await?;

    if response.to_lowercase().contains("yes") {
        println!("\nAgent: === Detailed Report ===");
        println!("Agent: - Files cleaned: 1,247 files (12.3 GB)");
        println!("Agent: - Packages updated: 23");
        println!("Agent: - Services restarted: database");
        println!("Agent: - Deployments: production");
        println!("Agent: - Total duration: 3m 42s");
        println!("Agent: - Status: ✅ All successful\n");
    }

    println!("=== Agent Session Complete ===");

    Ok(())
}
