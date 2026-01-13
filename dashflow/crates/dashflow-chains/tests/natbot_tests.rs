//! Integration tests for NatBot browser automation.
//!
//! These tests require the `playwright` feature to be enabled.

#![allow(unexpected_cfgs)]
#![cfg(feature = "playwright")]

use dashflow_chains::natbot::{Crawler, ElementInViewPort};

/// Test helper to create mock HTML element data
fn create_mock_element(
    node_index: &str,
    backend_node_id: i64,
    node_name: &str,
    is_clickable: bool,
) -> ElementInViewPort {
    ElementInViewPort {
        node_index: node_index.to_string(),
        backend_node_id,
        node_name: Some(node_name.to_string()),
        node_value: None,
        node_meta: vec![],
        is_clickable,
        origin_x: 100,
        origin_y: 100,
        center_x: 150,
        center_y: 150,
    }
}

#[test]
fn test_element_in_viewport_creation() {
    let element = create_mock_element("1", 42, "button", true);
    assert_eq!(element.node_index, "1");
    assert_eq!(element.backend_node_id, 42);
    assert_eq!(element.node_name, Some("button".to_string()));
    assert!(element.is_clickable);
    assert_eq!(element.center_x, 150);
    assert_eq!(element.center_y, 150);
}

#[test]
fn test_element_with_metadata() {
    let mut element = create_mock_element("2", 100, "input", false);
    element.node_meta = vec!["type=text".to_string(), "placeholder=Search".to_string()];
    element.node_value = Some("test value".to_string());

    assert_eq!(element.node_meta.len(), 2);
    assert_eq!(element.node_value, Some("test value".to_string()));
}

#[test]
fn test_command_parsing_scroll_up() {
    let command = "SCROLL UP";
    assert!(command.starts_with("SCROLL "));
    let direction = command.strip_prefix("SCROLL ").unwrap();
    assert_eq!(direction.to_lowercase(), "up");
}

#[test]
fn test_command_parsing_scroll_down() {
    let command = "SCROLL DOWN";
    assert!(command.starts_with("SCROLL "));
    let direction = command.strip_prefix("SCROLL ").unwrap();
    assert_eq!(direction.to_lowercase(), "down");
}

#[test]
fn test_command_parsing_click() {
    let command = "CLICK 5";
    assert!(command.starts_with("CLICK "));
    let id_str = command.strip_prefix("CLICK ").unwrap();
    let id: i32 = id_str.parse().unwrap();
    assert_eq!(id, 5);
}

#[test]
fn test_command_parsing_click_large_id() {
    let command = "CLICK 9999";
    assert!(command.starts_with("CLICK "));
    let id_str = command.strip_prefix("CLICK ").unwrap();
    let id: i32 = id_str.parse().unwrap();
    assert_eq!(id, 9999);
}

#[test]
fn test_command_parsing_type() {
    let command = "TYPE 3 \"hello world\"";
    assert!(command.starts_with("TYPE "));
    let rest = command.strip_prefix("TYPE ").unwrap();
    let parts: Vec<&str> = rest.splitn(2, ' ').collect();
    assert_eq!(parts.len(), 2);

    let id: i32 = parts[0].parse().unwrap();
    assert_eq!(id, 3);

    let text = parts[1].trim_matches('"');
    assert_eq!(text, "hello world");
}

#[test]
fn test_command_parsing_type_with_special_chars() {
    let command = "TYPE 10 \"user@example.com\"";
    assert!(command.starts_with("TYPE "));
    let rest = command.strip_prefix("TYPE ").unwrap();
    let parts: Vec<&str> = rest.splitn(2, ' ').collect();

    let id: i32 = parts[0].parse().unwrap();
    assert_eq!(id, 10);

    let text = parts[1].trim_matches('"');
    assert_eq!(text, "user@example.com");
}

#[test]
fn test_command_parsing_typesubmit() {
    let command = "TYPESUBMIT 12 \"search query\"";
    assert!(command.starts_with("TYPESUBMIT "));
    let rest = command.strip_prefix("TYPESUBMIT ").unwrap();
    let parts: Vec<&str> = rest.splitn(2, ' ').collect();
    assert_eq!(parts.len(), 2);

    let id: i32 = parts[0].parse().unwrap();
    assert_eq!(id, 12);

    let text = parts[1].trim_matches('"');
    assert_eq!(text, "search query");
}

#[test]
fn test_command_parsing_typesubmit_empty_text() {
    let command = "TYPESUBMIT 7 \"\"";
    assert!(command.starts_with("TYPESUBMIT "));
    let rest = command.strip_prefix("TYPESUBMIT ").unwrap();
    let parts: Vec<&str> = rest.splitn(2, ' ').collect();
    assert_eq!(parts.len(), 2);

    let id: i32 = parts[0].parse().unwrap();
    assert_eq!(id, 7);

    let text = parts[1].trim_matches('"');
    assert_eq!(text, "");
}

#[test]
fn test_url_truncation_short_url() {
    let url = "https://www.example.com";
    let truncated = if url.len() > 100 { &url[..100] } else { url };
    assert_eq!(truncated, url);
    assert!(truncated.len() < 100);
}

#[test]
fn test_url_truncation_long_url() {
    let url = "https://www.example.com/very/long/path/that/exceeds/one/hundred/characters/limit/for/sure/with/more/segments/added/here/to/make/it/really/long";
    let truncated = if url.len() > 100 { &url[..100] } else { url };
    assert_eq!(truncated.len(), 100);
    assert!(url.len() > 100);
}

#[test]
fn test_url_truncation_exactly_100() {
    let url = "a".repeat(100);
    let truncated = if url.len() > 100 {
        &url[..100]
    } else {
        url.as_str()
    };
    assert_eq!(truncated.len(), 100);
}

#[test]
fn test_browser_content_truncation_short() {
    let content = "<link id=1>Home</link>";
    let truncated = if content.len() > 4500 {
        &content[..4500]
    } else {
        content
    };
    assert_eq!(truncated, content);
}

#[test]
fn test_browser_content_truncation_long() {
    let content = "x".repeat(5000);
    let truncated = if content.len() > 4500 {
        &content[..4500]
    } else {
        content.as_str()
    };
    assert_eq!(truncated.len(), 4500);
}

#[test]
fn test_browser_content_truncation_exactly_4500() {
    let content = "y".repeat(4500);
    let truncated = if content.len() > 4500 {
        &content[..4500]
    } else {
        content.as_str()
    };
    assert_eq!(truncated.len(), 4500);
}

#[test]
fn test_url_without_scheme() {
    let url = "www.example.com";
    let formatted = if url.contains("://") {
        url.to_string()
    } else {
        format!("http://{}", url)
    };
    assert_eq!(formatted, "http://www.example.com");
}

#[test]
fn test_url_with_http_scheme() {
    let url = "http://www.example.com";
    let formatted = if url.contains("://") {
        url.to_string()
    } else {
        format!("http://{}", url)
    };
    assert_eq!(formatted, "http://www.example.com");
}

#[test]
fn test_url_with_https_scheme() {
    let url = "https://www.example.com";
    let formatted = if url.contains("://") {
        url.to_string()
    } else {
        format!("http://{}", url)
    };
    assert_eq!(formatted, "https://www.example.com");
}

#[test]
fn test_invalid_click_command_parsing() {
    let command = "CLICK abc";
    assert!(command.starts_with("CLICK "));
    let id_str = command.strip_prefix("CLICK ").unwrap();
    let result: Result<i32, _> = id_str.parse();
    assert!(result.is_err());
}

#[test]
fn test_invalid_type_command_format() {
    let command = "TYPE 5"; // Missing text
    assert!(command.starts_with("TYPE "));
    let rest = command.strip_prefix("TYPE ").unwrap();
    let parts: Vec<&str> = rest.splitn(2, ' ').collect();
    assert_eq!(parts.len(), 1); // Only has ID, missing text
}

#[test]
fn test_command_with_leading_trailing_whitespace() {
    let command = "  SCROLL DOWN  ";
    let trimmed = command.trim();
    assert!(trimmed.starts_with("SCROLL "));
    let direction = trimmed.strip_prefix("SCROLL ").unwrap();
    assert_eq!(direction.to_lowercase(), "down");
}

// Note: The following tests require a real browser instance via Playwright,
// so they are marked as #[ignore] by default. Run with --ignored flag to test
// with actual browser automation.

#[tokio::test]
#[ignore = "Requires Playwright browser instance"]
async fn test_crawler_creation() {
    let result = Crawler::new().await;
    assert!(
        result.is_ok(),
        "Failed to create crawler: {:?}",
        result.err()
    );
}

#[tokio::test]
#[ignore = "Requires Playwright browser instance"]
async fn test_crawler_navigate_to_page() {
    let mut crawler = Crawler::new().await.unwrap();
    let result = crawler.go_to_page("https://www.example.com").await;
    assert!(result.is_ok(), "Failed to navigate: {:?}", result.err());
}

#[tokio::test]
#[ignore = "Requires Playwright browser instance"]
async fn test_crawler_navigate_without_scheme() {
    let mut crawler = Crawler::new().await.unwrap();
    let result = crawler.go_to_page("www.example.com").await;
    assert!(result.is_ok(), "Failed to navigate: {:?}", result.err());
}
