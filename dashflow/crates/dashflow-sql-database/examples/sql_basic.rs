//! Basic SQL Database Tool Example
//!
//! This example demonstrates how to use the SQL database tools to interact with a database.
//!
//! Note: This example requires a running PostgreSQL database.
//! To run against a local PostgreSQL:
//! ```bash
//! # Start PostgreSQL (if not running)
//! # Then create a test database:
//! createdb dashflow_test
//! psql dashflow_test -c "CREATE TABLE users (id SERIAL PRIMARY KEY, name TEXT, email TEXT);"
//! psql dashflow_test -c "INSERT INTO users (name, email) VALUES ('Alice', 'alice@example.com'), ('Bob', 'bob@example.com');"
//!
//! # Run this example:
//! cargo run --example sql_basic --features postgres
//! ```

use dashflow::core::tools::Tool;
use dashflow_sql_database::{InfoSQLDatabaseTool, ListSQLDatabaseTool, QuerySQLDataBaseTool};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Database connection string
    // In production, use environment variables for credentials
    let database_uri = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://localhost/dashflow_test".to_string());

    println!("Connecting to database: {}", database_uri);
    println!();

    // Tool 1: List all available tables
    println!("=== List Tables Tool ===");
    let list_tool = ListSQLDatabaseTool::new(&database_uri).await?;
    println!("Tool: {}", list_tool.name());
    println!("Description: {}", list_tool.description());

    match list_tool._call_str("".to_string()).await {
        Ok(tables) => println!("Available tables: {}", tables),
        Err(e) => println!("Error listing tables: {}", e),
    }
    println!();

    // Tool 2: Get schema information for specific tables
    println!("=== Info Tool ===");
    let info_tool = InfoSQLDatabaseTool::new(&database_uri).await?;
    println!("Tool: {}", info_tool.name());
    println!("Description: {}", info_tool.description());

    match info_tool._call_str("users".to_string()).await {
        Ok(schema) => println!("Schema for 'users' table:\n{}", schema),
        Err(e) => println!("Error getting schema: {}", e),
    }
    println!();

    // Tool 3: Execute SQL queries
    println!("=== Query Tool ===");
    let query_tool = QuerySQLDataBaseTool::new(
        &database_uri,
        None, // No table restrictions
        10,   // Limit results to 10 rows
    )
    .await?;
    println!("Tool: {}", query_tool.name());
    println!("Description: {}", query_tool.description());

    match query_tool
        ._call_str("SELECT * FROM users LIMIT 5".to_string())
        .await
    {
        Ok(results) => println!("Query results:\n{}", results),
        Err(e) => println!("Error executing query: {}", e),
    }
    println!();

    // Demonstrate table restrictions
    println!("=== Query Tool with Table Restrictions ===");
    let restricted_tool = QuerySQLDataBaseTool::new(
        &database_uri,
        Some(vec!["users".to_string()]), // Only allow queries on 'users' table
        10,
    )
    .await?;

    // This query should fail because it tries to access 'orders' table
    match restricted_tool
        ._call_str("SELECT * FROM orders LIMIT 5".to_string())
        .await
    {
        Ok(_) => println!("Query succeeded (unexpected)"),
        Err(e) => println!("Query correctly blocked: {}", e),
    }

    Ok(())
}
