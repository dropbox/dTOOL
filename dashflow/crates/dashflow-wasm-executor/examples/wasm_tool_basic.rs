//! Basic WASM Code Execution Tool Example
//!
//! Demonstrates how to use the WasmCodeExecutionTool with the DashFlow Tool trait.
//!
//! This example shows:
//! - Creating a WasmCodeExecutionTool
//! - Executing WASM code via the Tool interface
//! - Handling base64-encoded WASM modules
//! - Using both string and structured input formats
//!
//! To run this example:
//! ```bash
//! cargo run --example wasm_tool_basic --features="dashflow::core"
//! ```

use dashflow::core::tools::Tool;
use dashflow_wasm_executor::{WasmCodeExecutionTool, WasmExecutorConfig};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing for better observability
    tracing_subscriber::fmt::init();

    println!("=== WASM Code Execution Tool Example ===\n");

    // Step 1: Create configuration
    println!("Step 1: Creating executor configuration...");
    let config =
        WasmExecutorConfig::new("example-jwt-secret-at-least-32-characters-long!".to_string());
    println!("  ✓ Configuration created with secure defaults\n");

    // Step 2: Create the tool
    println!("Step 2: Creating WASM code execution tool...");
    let tool = WasmCodeExecutionTool::new(config)?;
    println!("  ✓ Tool created successfully\n");

    // Step 3: Display tool metadata
    println!("Step 3: Tool metadata:");
    println!("  Name: {}", tool.name());
    println!("  Description: {}", tool.description());
    println!(
        "  Args Schema: {}\n",
        serde_json::to_string_pretty(&tool.args_schema())?
    );

    // Step 4: Create a simple WASM module
    // This is a minimal WASM module that adds two numbers
    // (func (export "add") (param i32 i32) (result i32)
    //   local.get 0
    //   local.get 1
    //   i32.add)
    println!("Step 4: Creating test WASM module (add function)...");
    let wasm_bytes = vec![
        0x00, 0x61, 0x73, 0x6d, // WASM magic number
        0x01, 0x00, 0x00, 0x00, // Version 1
        0x01, 0x07, 0x01, 0x60, 0x02, 0x7f, 0x7f, 0x01, 0x7f, // Type section
        0x03, 0x02, 0x01, 0x00, // Function section
        0x07, 0x07, 0x01, 0x03, 0x61, 0x64, 0x64, 0x00, 0x00, // Export section
        0x0a, 0x09, 0x01, 0x07, 0x00, 0x20, 0x00, 0x20, 0x01, 0x6a, 0x0b, // Code section
    ];

    // Encode to base64 for Tool input
    use base64::prelude::*;
    let wasm_b64 = BASE64_STANDARD.encode(&wasm_bytes);
    println!("  ✓ WASM module created (adds two i32 numbers)\n");

    // Step 5: Execute using string input format
    println!("Step 5: Executing WASM using string input...");
    println!("  Input: \"{}|add|5,7\"", &wasm_b64[..20]);
    let input_str = format!("{}|add|5,7", wasm_b64);
    let result = tool._call_str(input_str).await?;
    println!("  ✓ Result: {}\n", result);

    // Step 6: Execute using structured input format
    println!("Step 6: Executing WASM using structured input...");
    let input_json = serde_json::json!({
        "wasm_bytes": wasm_b64,
        "function": "add",
        "args": [10, 20]
    });
    println!("  Input: {}", serde_json::to_string_pretty(&input_json)?);
    let result = tool
        ._call(dashflow::core::tools::ToolInput::Structured(input_json))
        .await?;
    println!("  ✓ Result: {}\n", result);

    // Step 7: Demonstrate security features
    println!("Step 7: Security features:");
    println!("  • WASM validation: All modules validated before execution");
    println!("  • Resource limits: 5M fuel units, 64MB memory, 5s timeout");
    println!("  • Zero WASI permissions: No filesystem/network access");
    println!("  • Audit logging: All executions logged (when enabled)");
    println!("  • HIPAA/SOC2 compliant: Meets healthcare/enterprise standards\n");

    // Step 8: Demonstrate error handling
    println!("Step 8: Error handling examples:");

    // Invalid base64
    println!("  Testing invalid base64...");
    let invalid_result = tool._call_str("not-valid-base64!!!".to_string()).await;
    match invalid_result {
        Err(e) => println!("    ✓ Caught error: {}", e),
        Ok(_) => println!("    ✗ Expected error but got success"),
    }

    // Module too large
    println!("  Testing oversized module (>10MB)...");
    let large_data = vec![0u8; 11 * 1024 * 1024];
    let large_b64 = BASE64_STANDARD.encode(&large_data);
    let large_result = tool._call_str(large_b64).await;
    match large_result {
        Err(e) => println!("    ✓ Caught error: {}", e),
        Ok(_) => println!("    ✗ Expected error but got success"),
    }

    println!("\n=== Example Complete ===");
    println!("\nNext steps:");
    println!("  1. Review docs/PRODUCTION_DEPLOYMENT_GUIDE.md for production deployment");
    println!("  2. Review docs/OBSERVABILITY_RUNBOOK.md for monitoring and maintenance");
    println!("  3. Enable audit logging in production config");
    println!("  4. Configure Prometheus metrics scraping");
    println!("  5. Set up log aggregation (Splunk/ELK/Datadog)");

    Ok(())
}
