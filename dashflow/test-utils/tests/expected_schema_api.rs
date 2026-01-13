//! Expected Schema API Integration Test
//!
//! Tests the PUT/GET/DELETE /api/expected-schema endpoints on the websocket server.
//!
//! # Prerequisites
//! - Docker and docker-compose installed
//! - WebSocket server running (via docker-compose.dashstream.yml)
//!
//! # Configuration
//! Set `WEBSOCKET_SERVER_URL` environment variable to override the default:
//! ```bash
//! export WEBSOCKET_SERVER_URL="http://custom-host:3002"
//! ```
//!
//! # Running
//! ```bash
//! cargo test -p dashflow-test-utils --test expected_schema_api -- --ignored --nocapture
//! ```

use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Get the WebSocket server URL from environment or use default.
/// Override with WEBSOCKET_SERVER_URL env var for non-default deployments.
fn websocket_server_url() -> String {
    std::env::var("WEBSOCKET_SERVER_URL").unwrap_or_else(|_| "http://localhost:3002".to_string())
}

/// Expected schema entry returned by the API
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExpectedSchemaEntry {
    pub schema_id: String,
    pub graph_name: String,
    pub environment: Option<String>,
    pub pinned_at: i64,
    pub pinned_by: Option<String>,
    pub note: Option<String>,
}

/// Request body for setting expected schema
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetExpectedSchemaRequest {
    pub schema_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub environment: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pinned_by: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

/// Check if the websocket server is available
async fn is_websocket_server_available() -> bool {
    let client = Client::new();
    client
        .get(format!("{}/health", websocket_server_url()))
        .timeout(Duration::from_secs(5))
        .send()
        .await
        .map(|r| r.status().is_success())
        .unwrap_or(false)
}

/// Test the expected schema API lifecycle (PUT, GET, DELETE)
#[tokio::test]
#[ignore = "requires websocket server"]
async fn test_expected_schema_api_lifecycle() -> dashflow_test_utils::Result<()> {
    println!("\n=== Expected Schema API Integration Test ===\n");

    // Check if server is available
    if !is_websocket_server_available().await {
        println!(
            "Skipping: WebSocket server not available at {}",
            websocket_server_url()
        );
        return Ok(());
    }

    let client = Client::new();
    let test_graph_name = "test-graph-api-test";
    let test_schema_id = "sha256:abc123def456";

    // 1. First, ensure the test graph doesn't exist (cleanup from previous runs)
    println!("1. Cleanup: Deleting any existing schema for test graph...");
    let _ = client
        .delete(format!(
            "{}/api/expected-schema/{}",
            websocket_server_url(), test_graph_name
        ))
        .send()
        .await;

    // 2. GET should return 404 for non-existent graph
    println!("2. Testing GET for non-existent graph (expect 404)...");
    let response = client
        .get(format!(
            "{}/api/expected-schema/{}",
            websocket_server_url(), test_graph_name
        ))
        .send()
        .await?;

    assert_eq!(
        response.status().as_u16(),
        404,
        "Expected 404 for non-existent graph, got {}",
        response.status()
    );
    println!("   GET returned 404 as expected");

    // 3. PUT to create a new expected schema
    println!("3. Testing PUT to create expected schema...");
    let request = SetExpectedSchemaRequest {
        schema_id: test_schema_id.to_string(),
        environment: Some("test".to_string()),
        pinned_by: Some("test-runner".to_string()),
        note: Some("Created by integration test".to_string()),
    };

    let response = client
        .put(format!(
            "{}/api/expected-schema/{}",
            websocket_server_url(), test_graph_name
        ))
        .json(&request)
        .send()
        .await?;

    assert!(
        response.status().is_success(),
        "PUT failed with status {}",
        response.status()
    );

    let created: ExpectedSchemaEntry = response.json().await?;
    assert_eq!(created.graph_name, test_graph_name);
    assert_eq!(created.schema_id, test_schema_id);
    assert_eq!(created.environment, Some("test".to_string()));
    assert_eq!(created.pinned_by, Some("test-runner".to_string()));
    assert!(created.pinned_at > 0, "pinned_at should be set");
    println!(
        "   PUT succeeded, schema created with pinned_at={}",
        created.pinned_at
    );

    // 4. GET should now return the schema
    println!("4. Testing GET for existing schema...");
    let response = client
        .get(format!(
            "{}/api/expected-schema/{}",
            websocket_server_url(), test_graph_name
        ))
        .send()
        .await?;

    assert!(
        response.status().is_success(),
        "GET failed with status {}",
        response.status()
    );

    let retrieved: ExpectedSchemaEntry = response.json().await?;
    assert_eq!(retrieved.graph_name, test_graph_name);
    assert_eq!(retrieved.schema_id, test_schema_id);
    println!(
        "   GET succeeded, retrieved schema_id={}",
        retrieved.schema_id
    );

    // 5. Test list endpoint
    println!("5. Testing GET /api/expected-schema (list all)...");
    let response = client
        .get(format!("{}/api/expected-schema", websocket_server_url()))
        .send()
        .await?;

    assert!(
        response.status().is_success(),
        "List failed with status {}",
        response.status()
    );

    let all_schemas: Vec<ExpectedSchemaEntry> = response
        .json()
        .await
        ?;
    assert!(
        all_schemas.iter().any(|s| s.graph_name == test_graph_name),
        "Test graph should be in the list"
    );
    println!(
        "   List returned {} schemas, test graph found",
        all_schemas.len()
    );

    // 6. DELETE the schema
    println!("6. Testing DELETE to remove expected schema...");
    let response = client
        .delete(format!(
            "{}/api/expected-schema/{}",
            websocket_server_url(), test_graph_name
        ))
        .send()
        .await?;

    assert!(
        response.status().is_success(),
        "DELETE failed with status {}",
        response.status()
    );

    let deleted: ExpectedSchemaEntry = response
        .json()
        .await
        ?;
    assert_eq!(deleted.graph_name, test_graph_name);
    println!(
        "   DELETE succeeded, removed schema for {}",
        deleted.graph_name
    );

    // 7. GET should return 404 again
    println!("7. Testing GET after DELETE (expect 404)...");
    let response = client
        .get(format!(
            "{}/api/expected-schema/{}",
            websocket_server_url(), test_graph_name
        ))
        .send()
        .await?;

    assert_eq!(
        response.status().as_u16(),
        404,
        "Expected 404 after DELETE, got {}",
        response.status()
    );
    println!("   GET returned 404 as expected after DELETE");

    println!("\n=== All Expected Schema API Tests Passed ===\n");
    Ok(())
}

/// Test that DELETE on non-existent graph returns 404
#[tokio::test]
#[ignore = "requires websocket server"]
async fn test_expected_schema_delete_not_found() -> dashflow_test_utils::Result<()> {
    if !is_websocket_server_available().await {
        println!("Skipping: WebSocket server not available");
        return Ok(());
    }

    let client = Client::new();
    let response = client
        .delete(format!(
            "{}/api/expected-schema/non-existent-graph-12345",
            websocket_server_url()
        ))
        .send()
        .await?;

    assert_eq!(
        response.status().as_u16(),
        404,
        "Expected 404 for deleting non-existent graph"
    );
    Ok(())
}

// =============================================================================
// M-261: Strengthened E2E Checks (OBS-19)
// =============================================================================

/// Test verify_expected_schema_content() validates response structure (M-261)
#[tokio::test]
#[ignore = "requires websocket server"]
async fn test_verify_expected_schema_content() -> dashflow_test_utils::Result<()> {
    println!("\n=== M-261: verify_expected_schema_content Test ===\n");

    if !is_websocket_server_available().await {
        println!("Skipping: WebSocket server not available at {}", websocket_server_url());
        return Ok(());
    }

    // Use the new verification function from test-utils
    let verification = dashflow_test_utils::verify_expected_schema_content().await?;

    println!("Verification result:");
    println!("  is_valid_json: {}", verification.is_valid_json);
    println!("  is_array: {}", verification.is_array);
    println!("  schema_count: {}", verification.schema_count);
    println!("  validation_errors: {:?}", verification.validation_errors);

    // Basic checks - the API should return valid JSON array
    assert!(verification.is_valid_json, "Response should be valid JSON");
    assert!(verification.is_array, "Response should be a JSON array");

    // If there are schemas, they should be valid
    if !verification.schemas.is_empty() {
        assert!(
            verification.validation_errors.is_empty(),
            "Schemas should be valid: {:?}",
            verification.validation_errors
        );

        // Check first schema has required fields populated
        let first = &verification.schemas[0];
        assert!(!first.schema_id.is_empty(), "schema_id should not be empty");
        assert!(!first.graph_name.is_empty(), "graph_name should not be empty");
        assert!(first.pinned_at > 0, "pinned_at should be positive");
    }

    println!("\n=== verify_expected_schema_content Test Passed ===\n");
    Ok(())
}

/// Test verify_schema_roundtrip() performs full PUT/GET/DELETE cycle (M-261)
#[tokio::test]
#[ignore = "requires websocket server"]
async fn test_verify_schema_roundtrip() -> dashflow_test_utils::Result<()> {
    println!("\n=== M-261: verify_schema_roundtrip Test ===\n");

    if !is_websocket_server_available().await {
        println!("Skipping: WebSocket server not available at {}", websocket_server_url());
        return Ok(());
    }

    // Use the new roundtrip verification function from test-utils
    let result = dashflow_test_utils::verify_schema_roundtrip().await?;

    println!("Roundtrip result:");
    println!("  put_succeeded: {}", result.put_succeeded);
    println!("  get_succeeded: {}", result.get_succeeded);
    println!("  delete_succeeded: {}", result.delete_succeeded);
    println!("  schema_matched: {}", result.schema_matched);
    if let Some(ref error) = result.error {
        println!("  error: {}", error);
    }
    if let Some(ref schema) = result.retrieved_schema {
        println!("  retrieved schema_id: {}", schema.schema_id);
        println!("  retrieved graph_name: {}", schema.graph_name);
        println!("  retrieved pinned_at: {}", schema.pinned_at);
    }

    // All steps should succeed
    assert!(result.put_succeeded, "PUT should succeed");
    assert!(result.get_succeeded, "GET should succeed");
    assert!(result.delete_succeeded, "DELETE should succeed");
    assert!(result.schema_matched, "Retrieved schema should match sent schema: {:?}", result.error);

    // The overall roundtrip should be successful
    assert!(result.is_success(), "Full roundtrip should succeed");

    println!("\n=== verify_schema_roundtrip Test Passed ===\n");
    Ok(())
}
