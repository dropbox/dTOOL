//! GraphQL Tool Example
//!
//! This example demonstrates how to use the GraphQL tool to execute queries and mutations
//! against a GraphQL API. We'll use the public SpaceX GraphQL API for demonstration.
//!
//! Run with: cargo run --example graphql_queries

use dashflow::core::tools::Tool;
use dashflow_graphql::GraphQLTool;
use serde_json::json;
use std::collections::HashMap;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== GraphQL Tool Examples ===\n");

    // Create a GraphQL tool for the SpaceX API
    let spacex_tool = GraphQLTool::new("https://api.spacex.land/graphql".to_string());

    // Example 1: Simple query
    println!("1. Simple Query - Get Company Information");
    println!("-------------------------------------------");

    let simple_query = json!({
        "query": r#"
            query {
                company {
                    name
                    founder
                    founded
                    employees
                    headquarters {
                        city
                        state
                    }
                }
            }
        "#
    })
    .to_string();

    match spacex_tool._call_str(simple_query).await {
        Ok(response) => {
            println!("Response:\n{}\n", response);
        }
        Err(e) => {
            eprintln!("Error: {}\n", e);
        }
    }

    // Example 2: Query with variables
    println!("2. Query with Variables - Get Recent Launches");
    println!("----------------------------------------------");

    let query_with_vars = json!({
        "query": r#"
            query LaunchesPast($limit: Int!) {
                launchesPast(limit: $limit) {
                    mission_name
                    launch_date_local
                    launch_success
                    rocket {
                        rocket_name
                    }
                }
            }
        "#,
        "variables": {
            "limit": 5
        }
    })
    .to_string();

    match spacex_tool._call_str(query_with_vars).await {
        Ok(response) => {
            println!("Response:\n{}\n", response);
        }
        Err(e) => {
            eprintln!("Error: {}\n", e);
        }
    }

    // Example 3: Query with multiple operations and operation name
    println!("3. Multiple Operations with Operation Name");
    println!("--------------------------------------------");

    let multiple_ops = json!({
        "query": r#"
            query GetCompany {
                company {
                    name
                    ceo
                }
            }

            query GetRockets {
                rockets {
                    name
                    active
                }
            }
        "#,
        "operation_name": "GetCompany"
    })
    .to_string();

    match spacex_tool._call_str(multiple_ops).await {
        Ok(response) => {
            println!("Response:\n{}\n", response);
        }
        Err(e) => {
            eprintln!("Error: {}\n", e);
        }
    }

    // Example 4: Using GraphQL tool with authentication headers
    println!("4. GraphQL with Custom Headers (Authentication)");
    println!("------------------------------------------------");

    let mut headers = HashMap::new();
    headers.insert(
        "Authorization".to_string(),
        "Bearer fake_token_123".to_string(),
    );
    headers.insert("X-Api-Version".to_string(), "v1".to_string());

    // Note: SpaceX API doesn't require auth, this is just for demonstration
    let authenticated_tool =
        GraphQLTool::with_headers("https://api.spacex.land/graphql".to_string(), headers);

    let auth_query = json!({
        "query": "query { company { name } }"
    })
    .to_string();

    match authenticated_tool._call_str(auth_query).await {
        Ok(response) => {
            println!("Response:\n{}\n", response);
        }
        Err(e) => {
            eprintln!("Error: {}\n", e);
        }
    }

    // Example 5: Custom timeout
    println!("5. GraphQL with Custom Timeout");
    println!("--------------------------------");

    let timeout_tool =
        GraphQLTool::new("https://api.spacex.land/graphql".to_string()).with_timeout(60); // 60 seconds timeout

    let timeout_query = json!({
        "query": "query { company { name } }"
    })
    .to_string();

    match timeout_tool._call_str(timeout_query).await {
        Ok(response) => {
            println!("Response:\n{}\n", response);
        }
        Err(e) => {
            eprintln!("Error: {}\n", e);
        }
    }

    // Example 6: Plain query string (without JSON)
    println!("6. Plain Query String Input");
    println!("----------------------------");

    let plain_query = "query { company { name founder } }".to_string();

    match spacex_tool._call_str(plain_query).await {
        Ok(response) => {
            println!("Response:\n{}\n", response);
        }
        Err(e) => {
            eprintln!("Error: {}\n", e);
        }
    }

    // Example 7: Error handling - Invalid query
    println!("7. Error Handling - Invalid Query");
    println!("-----------------------------------");

    let invalid_query = json!({
        "query": "query { invalidField { name } }"
    })
    .to_string();

    match spacex_tool._call_str(invalid_query).await {
        Ok(response) => {
            // GraphQL returns errors in the response body
            println!("Response:\n{}\n", response);
        }
        Err(e) => {
            eprintln!("Error: {}\n", e);
        }
    }

    // Example 8: Complex nested query
    println!("8. Complex Nested Query");
    println!("------------------------");

    let complex_query = json!({
        "query": r#"
            query {
                launchesPast(limit: 3) {
                    mission_name
                    launch_date_local
                    rocket {
                        rocket_name
                        rocket_type
                        first_stage {
                            cores {
                                core {
                                    reuse_count
                                    status
                                }
                            }
                        }
                        second_stage {
                            payloads {
                                payload_type
                                payload_mass_kg
                            }
                        }
                    }
                    links {
                        mission_patch
                        article_link
                    }
                }
            }
        "#
    })
    .to_string();

    match spacex_tool._call_str(complex_query).await {
        Ok(response) => {
            println!("Response:\n{}\n", response);
        }
        Err(e) => {
            eprintln!("Error: {}\n", e);
        }
    }

    println!("=== Examples Complete ===");

    Ok(())
}
