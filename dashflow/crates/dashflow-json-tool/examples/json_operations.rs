//! Examples demonstrating JSON parsing and querying with JsonTool.
//!
//! This example shows how to:
//! - Parse and validate JSON strings
//! - Pretty-print JSON data
//! - Query JSON using JSONPath expressions
//! - Extract specific values from complex structures

use dashflow::core::tools::{Tool, ToolInput};
use dashflow_json_tool::JsonTool;
use serde_json::json;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== JSON Tool Examples ===\n");

    let tool = JsonTool::new();

    // Example 1: Simple JSON parsing
    println!("1. Simple JSON Parsing:");
    println!("   Input: Compact JSON string");
    let compact_json = r#"{"name":"Alice","age":30,"city":"New York"}"#;
    let result = tool._call_str(compact_json.to_string()).await?;
    println!("   Output (pretty-printed):\n{}\n", result);

    // Example 2: Parse nested JSON
    println!("2. Parse Nested JSON:");
    let nested_json = r#"{"user":{"name":"Bob","profile":{"email":"bob@example.com","age":25}}}"#;
    let result = tool._call_str(nested_json.to_string()).await?;
    println!("   Output:\n{}\n", result);

    // Example 3: JSONPath - Extract single value
    println!("3. JSONPath - Extract Single Value:");
    println!("   Query: $.users[0].name");
    let input = json!({
        "json": r#"{"users":[{"name":"Alice","age":30},{"name":"Bob","age":25}]}"#,
        "path": "$.users[0].name"
    });
    let result = tool._call(ToolInput::Structured(input)).await?;
    println!("   Result: {}\n", result);

    // Example 4: JSONPath - Extract all names
    println!("4. JSONPath - Extract All Names:");
    println!("   Query: $.users[*].name");
    let input = json!({
        "json": r#"{"users":[{"name":"Alice","age":30},{"name":"Bob","age":25},{"name":"Charlie","age":35}]}"#,
        "path": "$.users[*].name"
    });
    let result = tool._call(ToolInput::Structured(input)).await?;
    println!("   Result:\n{}\n", result);

    // Example 5: JSONPath - Recursive descent
    println!("5. JSONPath - Recursive Descent:");
    println!("   Query: $..email (find all 'email' fields)");
    let input = json!({
        "json": r#"{
            "user": {
                "profile": {"email": "alice@example.com"},
                "contacts": [
                    {"email": "bob@example.com"},
                    {"email": "charlie@example.com"}
                ]
            }
        }"#,
        "path": "$..email"
    });
    let result = tool._call(ToolInput::Structured(input)).await?;
    println!("   Result:\n{}\n", result);

    // Example 6: JSONPath - Array index
    println!("6. JSONPath - Specific Array Index:");
    println!("   Query: $.products[1]");
    let input = json!({
        "json": r#"{"products":[{"id":1,"name":"Widget"},{"id":2,"name":"Gadget"},{"id":3,"name":"Doohickey"}]}"#,
        "path": "$.products[1]"
    });
    let result = tool._call(ToolInput::Structured(input)).await?;
    println!("   Result:\n{}\n", result);

    // Example 7: Nested field access
    println!("7. JSONPath - Nested Field Access:");
    println!("   Query: $.data.metrics.cpu");
    let input = json!({
        "json": r#"{"data":{"metrics":{"cpu":75,"memory":60,"disk":45}}}"#,
        "path": "$.data.metrics.cpu"
    });
    let result = tool._call(ToolInput::Structured(input)).await?;
    println!("   Result: {}\n", result);

    // Example 8: No matches found
    println!("8. JSONPath - No Matches:");
    println!("   Query: $.nonexistent");
    let input = json!({
        "json": r#"{"name":"Alice","age":30}"#,
        "path": "$.nonexistent"
    });
    let result = tool._call(ToolInput::Structured(input)).await?;
    println!("   Result: {}\n", result);

    // Example 9: Complex real-world JSON (API response)
    println!("9. Real-World API Response:");
    println!("   Query: $.data.repositories[*].name");
    let api_response = r#"{
        "data": {
            "user": "octocat",
            "repositories": [
                {"name": "Hello-World", "stars": 1500, "language": "JavaScript"},
                {"name": "octocat.github.io", "stars": 800, "language": "HTML"},
                {"name": "Spoon-Knife", "stars": 12000, "language": "Ruby"}
            ]
        }
    }"#;
    let input = json!({
        "json": api_response,
        "path": "$.data.repositories[*].name"
    });
    let result = tool._call(ToolInput::Structured(input)).await?;
    println!("   Result:\n{}\n", result);

    // Example 10: Error handling - Invalid JSON
    println!("10. Error Handling - Invalid JSON:");
    let invalid_json = r#"{"name":"Alice""#; // Missing closing brace
    match tool._call_str(invalid_json.to_string()).await {
        Ok(_) => println!("   Unexpectedly succeeded"),
        Err(e) => println!("   Error (expected): {}\n", e),
    }

    // Example 11: Error handling - Invalid JSONPath
    println!("11. Error Handling - Invalid JSONPath:");
    let input = json!({
        "json": r#"{"name":"Alice"}"#,
        "path": "$$invalid["
    });
    match tool._call(ToolInput::Structured(input)).await {
        Ok(_) => println!("   Unexpectedly succeeded"),
        Err(e) => println!("   Error (expected): {}\n", e),
    }

    println!("=== All Examples Complete ===");

    Ok(())
}
