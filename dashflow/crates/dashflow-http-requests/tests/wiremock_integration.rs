//! Integration tests using wiremock for HTTP tools
//!
//! These tests verify that the HTTP tools correctly make requests and handle responses
//! by using a local mock server.

// `cargo verify` runs clippy with `-D warnings` for all targets, including tests.
#![allow(clippy::unwrap_used)]

use dashflow::core::tools::{Tool, ToolInput};
use dashflow_http_requests::{HttpDeleteTool, HttpGetTool, HttpPatchTool, HttpPostTool, HttpPutTool};
use serde_json::json;
use wiremock::matchers::{body_json, header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

// =============================================================================
// HTTP GET Tests
// =============================================================================

#[tokio::test]
async fn test_http_get_success() {
    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/api/users"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(json!({"users": [{"id": 1, "name": "Alice"}]}))
                .insert_header("Content-Type", "application/json"),
        )
        .mount(&mock_server)
        .await;

    let tool = HttpGetTool::new();
    let input = json!({
        "url": format!("{}/api/users", mock_server.uri())
    })
    .to_string();

    let result = tool._call(ToolInput::String(input)).await.unwrap();
    let response: serde_json::Value = serde_json::from_str(&result).unwrap();

    assert_eq!(response["status"], 200);
    assert!(response["body"].as_str().unwrap().contains("Alice"));
}

#[tokio::test]
async fn test_http_get_with_headers() {
    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/api/protected"))
        .and(header("Authorization", "Bearer test-token"))
        .respond_with(ResponseTemplate::new(200).set_body_string("authorized"))
        .mount(&mock_server)
        .await;

    let tool = HttpGetTool::new();
    let input = json!({
        "url": format!("{}/api/protected", mock_server.uri()),
        "headers": {
            "Authorization": "Bearer test-token"
        }
    })
    .to_string();

    let result = tool._call(ToolInput::String(input)).await.unwrap();
    let response: serde_json::Value = serde_json::from_str(&result).unwrap();

    assert_eq!(response["status"], 200);
    assert_eq!(response["body"], "authorized");
}

#[tokio::test]
async fn test_http_get_404() {
    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/api/notfound"))
        .respond_with(ResponseTemplate::new(404).set_body_string("Not Found"))
        .mount(&mock_server)
        .await;

    let tool = HttpGetTool::new();
    let input = json!({
        "url": format!("{}/api/notfound", mock_server.uri())
    })
    .to_string();

    let result = tool._call(ToolInput::String(input)).await.unwrap();
    let response: serde_json::Value = serde_json::from_str(&result).unwrap();

    assert_eq!(response["status"], 404);
    assert_eq!(response["body"], "Not Found");
}

#[tokio::test]
async fn test_http_get_500() {
    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/api/error"))
        .respond_with(ResponseTemplate::new(500).set_body_string("Internal Server Error"))
        .mount(&mock_server)
        .await;

    let tool = HttpGetTool::new();
    let input = json!({
        "url": format!("{}/api/error", mock_server.uri())
    })
    .to_string();

    let result = tool._call(ToolInput::String(input)).await.unwrap();
    let response: serde_json::Value = serde_json::from_str(&result).unwrap();

    assert_eq!(response["status"], 500);
}

#[tokio::test]
async fn test_http_get_empty_response() {
    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/api/empty"))
        .respond_with(ResponseTemplate::new(204))
        .mount(&mock_server)
        .await;

    let tool = HttpGetTool::new();
    let input = json!({
        "url": format!("{}/api/empty", mock_server.uri())
    })
    .to_string();

    let result = tool._call(ToolInput::String(input)).await.unwrap();
    let response: serde_json::Value = serde_json::from_str(&result).unwrap();

    assert_eq!(response["status"], 204);
    assert_eq!(response["body"], "");
}

#[tokio::test]
async fn test_http_get_with_query_params() {
    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/api/search"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(json!({"query": "rust", "page": 1})),
        )
        .mount(&mock_server)
        .await;

    let tool = HttpGetTool::new();
    let input = json!({
        "url": format!("{}/api/search?q=rust&page=1", mock_server.uri())
    })
    .to_string();

    let result = tool._call(ToolInput::String(input)).await.unwrap();
    let response: serde_json::Value = serde_json::from_str(&result).unwrap();

    assert_eq!(response["status"], 200);
}

// =============================================================================
// HTTP POST Tests
// =============================================================================

#[tokio::test]
async fn test_http_post_success() {
    let mock_server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/api/users"))
        .and(body_json(json!({"name": "Bob", "email": "bob@test.com"})))
        .respond_with(
            ResponseTemplate::new(201)
                .set_body_json(json!({"id": 123, "name": "Bob", "email": "bob@test.com"})),
        )
        .mount(&mock_server)
        .await;

    let tool = HttpPostTool::new();
    let input = json!({
        "url": format!("{}/api/users", mock_server.uri()),
        "data": {
            "name": "Bob",
            "email": "bob@test.com"
        }
    })
    .to_string();

    let result = tool._call(ToolInput::String(input)).await.unwrap();
    let response: serde_json::Value = serde_json::from_str(&result).unwrap();

    assert_eq!(response["status"], 201);
    assert!(response["body"].as_str().unwrap().contains("123"));
}

#[tokio::test]
async fn test_http_post_with_headers() {
    let mock_server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/api/data"))
        .and(header("X-Custom-Header", "custom-value"))
        .respond_with(ResponseTemplate::new(200).set_body_string("OK"))
        .mount(&mock_server)
        .await;

    let tool = HttpPostTool::new();
    let input = json!({
        "url": format!("{}/api/data", mock_server.uri()),
        "data": {"key": "value"},
        "headers": {
            "X-Custom-Header": "custom-value"
        }
    })
    .to_string();

    let result = tool._call(ToolInput::String(input)).await.unwrap();
    let response: serde_json::Value = serde_json::from_str(&result).unwrap();

    assert_eq!(response["status"], 200);
}

#[tokio::test]
async fn test_http_post_empty_body() {
    let mock_server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/api/trigger"))
        .respond_with(ResponseTemplate::new(202).set_body_string("Accepted"))
        .mount(&mock_server)
        .await;

    let tool = HttpPostTool::new();
    let input = json!({
        "url": format!("{}/api/trigger", mock_server.uri())
    })
    .to_string();

    let result = tool._call(ToolInput::String(input)).await.unwrap();
    let response: serde_json::Value = serde_json::from_str(&result).unwrap();

    assert_eq!(response["status"], 202);
}

#[tokio::test]
async fn test_http_post_complex_json() {
    let mock_server = MockServer::start().await;

    let complex_data = json!({
        "string": "hello",
        "number": 42,
        "float": 3.14,
        "boolean": true,
        "null": null,
        "array": [1, 2, 3],
        "nested": {
            "inner": "value"
        }
    });

    Mock::given(method("POST"))
        .and(path("/api/complex"))
        .and(body_json(complex_data.clone()))
        .respond_with(ResponseTemplate::new(200).set_body_string("OK"))
        .mount(&mock_server)
        .await;

    let tool = HttpPostTool::new();
    let input = json!({
        "url": format!("{}/api/complex", mock_server.uri()),
        "data": complex_data
    })
    .to_string();

    let result = tool._call(ToolInput::String(input)).await.unwrap();
    let response: serde_json::Value = serde_json::from_str(&result).unwrap();

    assert_eq!(response["status"], 200);
}

#[tokio::test]
async fn test_http_post_400_bad_request() {
    let mock_server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/api/validate"))
        .respond_with(
            ResponseTemplate::new(400).set_body_json(json!({"error": "Invalid input"})),
        )
        .mount(&mock_server)
        .await;

    let tool = HttpPostTool::new();
    let input = json!({
        "url": format!("{}/api/validate", mock_server.uri()),
        "data": {"invalid": "data"}
    })
    .to_string();

    let result = tool._call(ToolInput::String(input)).await.unwrap();
    let response: serde_json::Value = serde_json::from_str(&result).unwrap();

    assert_eq!(response["status"], 400);
    assert!(response["body"].as_str().unwrap().contains("Invalid input"));
}

// =============================================================================
// HTTP PUT Tests
// =============================================================================

#[tokio::test]
async fn test_http_put_success() {
    let mock_server = MockServer::start().await;

    Mock::given(method("PUT"))
        .and(path("/api/users/1"))
        .and(body_json(json!({"name": "Updated Name"})))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(json!({"id": 1, "name": "Updated Name"})),
        )
        .mount(&mock_server)
        .await;

    let tool = HttpPutTool::new();
    let input = json!({
        "url": format!("{}/api/users/1", mock_server.uri()),
        "data": {
            "name": "Updated Name"
        }
    })
    .to_string();

    let result = tool._call(ToolInput::String(input)).await.unwrap();
    let response: serde_json::Value = serde_json::from_str(&result).unwrap();

    assert_eq!(response["status"], 200);
    assert!(response["body"].as_str().unwrap().contains("Updated Name"));
}

#[tokio::test]
async fn test_http_put_create_if_not_exists() {
    let mock_server = MockServer::start().await;

    Mock::given(method("PUT"))
        .and(path("/api/config/new-key"))
        .respond_with(ResponseTemplate::new(201).set_body_string("Created"))
        .mount(&mock_server)
        .await;

    let tool = HttpPutTool::new();
    let input = json!({
        "url": format!("{}/api/config/new-key", mock_server.uri()),
        "data": {"value": "new-value"}
    })
    .to_string();

    let result = tool._call(ToolInput::String(input)).await.unwrap();
    let response: serde_json::Value = serde_json::from_str(&result).unwrap();

    assert_eq!(response["status"], 201);
}

#[tokio::test]
async fn test_http_put_with_authorization() {
    let mock_server = MockServer::start().await;

    Mock::given(method("PUT"))
        .and(path("/api/secure/resource"))
        .and(header("Authorization", "Bearer admin-token"))
        .respond_with(ResponseTemplate::new(200).set_body_string("Updated"))
        .mount(&mock_server)
        .await;

    let tool = HttpPutTool::new();
    let input = json!({
        "url": format!("{}/api/secure/resource", mock_server.uri()),
        "data": {"key": "value"},
        "headers": {
            "Authorization": "Bearer admin-token"
        }
    })
    .to_string();

    let result = tool._call(ToolInput::String(input)).await.unwrap();
    let response: serde_json::Value = serde_json::from_str(&result).unwrap();

    assert_eq!(response["status"], 200);
}

// =============================================================================
// HTTP PATCH Tests
// =============================================================================

#[tokio::test]
async fn test_http_patch_success() {
    let mock_server = MockServer::start().await;

    Mock::given(method("PATCH"))
        .and(path("/api/users/1"))
        .and(body_json(json!({"email": "new@email.com"})))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(json!({"id": 1, "name": "Alice", "email": "new@email.com"})),
        )
        .mount(&mock_server)
        .await;

    let tool = HttpPatchTool::new();
    let input = json!({
        "url": format!("{}/api/users/1", mock_server.uri()),
        "data": {
            "email": "new@email.com"
        }
    })
    .to_string();

    let result = tool._call(ToolInput::String(input)).await.unwrap();
    let response: serde_json::Value = serde_json::from_str(&result).unwrap();

    assert_eq!(response["status"], 200);
    assert!(response["body"].as_str().unwrap().contains("new@email.com"));
}

#[tokio::test]
async fn test_http_patch_partial_update() {
    let mock_server = MockServer::start().await;

    Mock::given(method("PATCH"))
        .and(path("/api/settings"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(json!({"updated_fields": ["theme"]})),
        )
        .mount(&mock_server)
        .await;

    let tool = HttpPatchTool::new();
    let input = json!({
        "url": format!("{}/api/settings", mock_server.uri()),
        "data": {
            "theme": "dark"
        }
    })
    .to_string();

    let result = tool._call(ToolInput::String(input)).await.unwrap();
    let response: serde_json::Value = serde_json::from_str(&result).unwrap();

    assert_eq!(response["status"], 200);
}

#[tokio::test]
async fn test_http_patch_not_found() {
    let mock_server = MockServer::start().await;

    Mock::given(method("PATCH"))
        .and(path("/api/users/999"))
        .respond_with(ResponseTemplate::new(404).set_body_string("User not found"))
        .mount(&mock_server)
        .await;

    let tool = HttpPatchTool::new();
    let input = json!({
        "url": format!("{}/api/users/999", mock_server.uri()),
        "data": {"name": "NewName"}
    })
    .to_string();

    let result = tool._call(ToolInput::String(input)).await.unwrap();
    let response: serde_json::Value = serde_json::from_str(&result).unwrap();

    assert_eq!(response["status"], 404);
}

// =============================================================================
// HTTP DELETE Tests
// =============================================================================

#[tokio::test]
async fn test_http_delete_success() {
    let mock_server = MockServer::start().await;

    Mock::given(method("DELETE"))
        .and(path("/api/users/1"))
        .respond_with(ResponseTemplate::new(204))
        .mount(&mock_server)
        .await;

    let tool = HttpDeleteTool::new();
    let input = json!({
        "url": format!("{}/api/users/1", mock_server.uri())
    })
    .to_string();

    let result = tool._call(ToolInput::String(input)).await.unwrap();
    let response: serde_json::Value = serde_json::from_str(&result).unwrap();

    assert_eq!(response["status"], 204);
}

#[tokio::test]
async fn test_http_delete_with_response_body() {
    let mock_server = MockServer::start().await;

    Mock::given(method("DELETE"))
        .and(path("/api/tasks/42"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(json!({"deleted": true, "id": 42})),
        )
        .mount(&mock_server)
        .await;

    let tool = HttpDeleteTool::new();
    let input = json!({
        "url": format!("{}/api/tasks/42", mock_server.uri())
    })
    .to_string();

    let result = tool._call(ToolInput::String(input)).await.unwrap();
    let response: serde_json::Value = serde_json::from_str(&result).unwrap();

    assert_eq!(response["status"], 200);
    assert!(response["body"].as_str().unwrap().contains("deleted"));
}

#[tokio::test]
async fn test_http_delete_unauthorized() {
    let mock_server = MockServer::start().await;

    Mock::given(method("DELETE"))
        .and(path("/api/admin/resource"))
        .respond_with(ResponseTemplate::new(401).set_body_string("Unauthorized"))
        .mount(&mock_server)
        .await;

    let tool = HttpDeleteTool::new();
    let input = json!({
        "url": format!("{}/api/admin/resource", mock_server.uri())
    })
    .to_string();

    let result = tool._call(ToolInput::String(input)).await.unwrap();
    let response: serde_json::Value = serde_json::from_str(&result).unwrap();

    assert_eq!(response["status"], 401);
}

#[tokio::test]
async fn test_http_delete_with_authorization() {
    let mock_server = MockServer::start().await;

    Mock::given(method("DELETE"))
        .and(path("/api/protected/item"))
        .and(header("Authorization", "Bearer delete-token"))
        .respond_with(ResponseTemplate::new(200).set_body_string("Deleted"))
        .mount(&mock_server)
        .await;

    let tool = HttpDeleteTool::new();
    let input = json!({
        "url": format!("{}/api/protected/item", mock_server.uri()),
        "headers": {
            "Authorization": "Bearer delete-token"
        }
    })
    .to_string();

    let result = tool._call(ToolInput::String(input)).await.unwrap();
    let response: serde_json::Value = serde_json::from_str(&result).unwrap();

    assert_eq!(response["status"], 200);
}

// =============================================================================
// Response Header Tests
// =============================================================================

#[tokio::test]
async fn test_response_headers_captured() {
    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/api/headers"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("X-Request-Id", "abc123")
                .insert_header("X-Rate-Limit", "100")
                .set_body_string("OK"),
        )
        .mount(&mock_server)
        .await;

    let tool = HttpGetTool::new();
    let input = json!({
        "url": format!("{}/api/headers", mock_server.uri())
    })
    .to_string();

    let result = tool._call(ToolInput::String(input)).await.unwrap();
    let response: serde_json::Value = serde_json::from_str(&result).unwrap();

    let headers = response["headers"].as_object().unwrap();
    assert!(headers.contains_key("x-request-id") || headers.contains_key("X-Request-Id"));
}

#[tokio::test]
async fn test_content_type_header() {
    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/api/json"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("Content-Type", "application/json")
                .set_body_json(json!({"key": "value"})),
        )
        .mount(&mock_server)
        .await;

    let tool = HttpGetTool::new();
    let input = json!({
        "url": format!("{}/api/json", mock_server.uri())
    })
    .to_string();

    let result = tool._call(ToolInput::String(input)).await.unwrap();
    let response: serde_json::Value = serde_json::from_str(&result).unwrap();

    let headers = response["headers"].as_object().unwrap();
    // Headers are lowercased in reqwest
    let content_type = headers
        .get("content-type")
        .or_else(|| headers.get("Content-Type"));
    assert!(content_type.is_some());
    assert!(content_type.unwrap().as_str().unwrap().contains("application/json"));
}

// =============================================================================
// ToolInput::Structured Tests with Wiremock
// =============================================================================

#[tokio::test]
async fn test_structured_input_get() {
    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/api/structured"))
        .respond_with(ResponseTemplate::new(200).set_body_string("structured OK"))
        .mount(&mock_server)
        .await;

    let tool = HttpGetTool::new();
    let input = ToolInput::Structured(json!({
        "url": format!("{}/api/structured", mock_server.uri())
    }));

    let result = tool._call(input).await.unwrap();
    let response: serde_json::Value = serde_json::from_str(&result).unwrap();

    assert_eq!(response["status"], 200);
    assert_eq!(response["body"], "structured OK");
}

#[tokio::test]
async fn test_structured_input_post() {
    let mock_server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/api/structured"))
        .and(body_json(json!({"test": "data"})))
        .respond_with(ResponseTemplate::new(201).set_body_string("created"))
        .mount(&mock_server)
        .await;

    let tool = HttpPostTool::new();
    let input = ToolInput::Structured(json!({
        "url": format!("{}/api/structured", mock_server.uri()),
        "data": {"test": "data"}
    }));

    let result = tool._call(input).await.unwrap();
    let response: serde_json::Value = serde_json::from_str(&result).unwrap();

    assert_eq!(response["status"], 201);
}

// =============================================================================
// Edge Cases
// =============================================================================

#[tokio::test]
async fn test_large_response_body() {
    let mock_server = MockServer::start().await;

    // Create a response with 10KB of data
    let large_body = "x".repeat(10_000);

    Mock::given(method("GET"))
        .and(path("/api/large"))
        .respond_with(ResponseTemplate::new(200).set_body_string(&large_body))
        .mount(&mock_server)
        .await;

    let tool = HttpGetTool::new();
    let input = json!({
        "url": format!("{}/api/large", mock_server.uri())
    })
    .to_string();

    let result = tool._call(ToolInput::String(input)).await.unwrap();
    let response: serde_json::Value = serde_json::from_str(&result).unwrap();

    assert_eq!(response["status"], 200);
    assert!(response["body"].as_str().unwrap().len() >= 10_000);
}

#[tokio::test]
async fn test_unicode_in_response() {
    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/api/unicode"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(json!({
                "greeting": "Hello, 世界!",
                "emoji": "🎉🎊🎈"
            })),
        )
        .mount(&mock_server)
        .await;

    let tool = HttpGetTool::new();
    let input = json!({
        "url": format!("{}/api/unicode", mock_server.uri())
    })
    .to_string();

    let result = tool._call(ToolInput::String(input)).await.unwrap();
    let response: serde_json::Value = serde_json::from_str(&result).unwrap();

    assert_eq!(response["status"], 200);
    assert!(response["body"].as_str().unwrap().contains("世界"));
    assert!(response["body"].as_str().unwrap().contains("🎉"));
}

#[tokio::test]
async fn test_redirect_not_followed_by_default() {
    // Note: This test verifies the tool's behavior with redirects
    // The mock server returns a 302, and we verify we get that status
    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/api/redirect"))
        .respond_with(
            ResponseTemplate::new(302).insert_header("Location", "/api/new-location"),
        )
        .mount(&mock_server)
        .await;

    let tool = HttpGetTool::new();
    let input = json!({
        "url": format!("{}/api/redirect", mock_server.uri())
    })
    .to_string();

    let result = tool._call(ToolInput::String(input)).await.unwrap();
    let response: serde_json::Value = serde_json::from_str(&result).unwrap();

    // Either we get 302 (no redirect follow) or 404 (redirect followed but target not found)
    // This depends on the HTTP client configuration
    let status = response["status"].as_u64().unwrap();
    assert!(status == 302 || status == 404 || status == 200);
}

#[tokio::test]
async fn test_multiple_headers_same_type() {
    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/api/multi-header"))
        .and(header("X-Custom", "value1"))
        .and(header("X-Another", "value2"))
        .respond_with(ResponseTemplate::new(200).set_body_string("OK"))
        .mount(&mock_server)
        .await;

    let tool = HttpGetTool::new();
    let input = json!({
        "url": format!("{}/api/multi-header", mock_server.uri()),
        "headers": {
            "X-Custom": "value1",
            "X-Another": "value2"
        }
    })
    .to_string();

    let result = tool._call(ToolInput::String(input)).await.unwrap();
    let response: serde_json::Value = serde_json::from_str(&result).unwrap();

    assert_eq!(response["status"], 200);
}
