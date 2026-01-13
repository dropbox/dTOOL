//! OpenAPI Tool Example - Petstore API
//!
//! This example demonstrates using the OpenAPI tool with the Swagger Petstore API.
//!
//! Run with:
//! ```bash
//! cargo run --example petstore -p dashflow-openapi
//! ```

use dashflow::core::tools::Tool;
use dashflow_openapi::OpenAPITool;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== OpenAPI Tool - Petstore API Example ===\n");

    // Load the Swagger Petstore v3 OpenAPI spec
    println!("Loading OpenAPI specification...");
    let tool = OpenAPITool::from_url(
        "https://petstore3.swagger.io/api/v3/openapi.json",
        None, // No authentication required for public API
    )
    .await?;

    println!("Tool loaded: {}", tool.name());
    println!("Description: {}", tool.description());
    println!();

    // List all available operations
    println!("=== Available Operations ===");
    let operations = tool.list_operations();
    for (i, op) in operations.iter().enumerate() {
        println!("{}. {}", i + 1, op);
    }
    println!("Total: {} operations\n", operations.len());

    // Example 1: Get pet by ID (this may fail if pet doesn't exist)
    println!("=== Example 1: Get Pet by ID ===");
    let get_pet_input = r#"{
        "operation_id": "getPetById",
        "parameters": {
            "petId": 1
        }
    }"#;

    match tool._call_str(get_pet_input.to_string()).await {
        Ok(result) => println!("Success! Pet data:\n{}\n", result),
        Err(e) => println!("Error (expected if pet ID doesn't exist): {}\n", e),
    }

    // Example 2: Find pets by status
    println!("=== Example 2: Find Pets by Status ===");
    let find_pets_input = r#"{
        "operation_id": "findPetsByStatus",
        "parameters": {
            "status": "available"
        }
    }"#;

    match tool._call_str(find_pets_input.to_string()).await {
        Ok(result) => {
            // Parse and pretty-print first few pets
            if let Ok(pets) = serde_json::from_str::<serde_json::Value>(&result) {
                if let Some(arr) = pets.as_array() {
                    println!("Found {} available pets", arr.len());
                    if !arr.is_empty() {
                        println!("First pet: {}\n", serde_json::to_string_pretty(&arr[0])?);
                    }
                }
            } else {
                println!("Response:\n{}\n", result);
            }
        }
        Err(e) => println!("Error: {}\n", e),
    }

    // Example 3: Create a new pet (POST)
    // Note: This will fail without authentication, but demonstrates the API
    println!("=== Example 3: Create New Pet (will likely fail without auth) ===");
    let create_pet_input = r#"{
        "operation_id": "addPet",
        "parameters": {},
        "body": {
            "name": "Rex",
            "category": {
                "name": "Dogs"
            },
            "photoUrls": ["https://example.com/rex.jpg"],
            "tags": [
                {"name": "friendly"}
            ],
            "status": "available"
        }
    }"#;

    match tool._call_str(create_pet_input.to_string()).await {
        Ok(result) => println!("Success! Created pet:\n{}\n", result),
        Err(e) => println!("Error (expected without authentication): {}\n", e),
    }

    println!("=== Example Complete ===");
    Ok(())
}
