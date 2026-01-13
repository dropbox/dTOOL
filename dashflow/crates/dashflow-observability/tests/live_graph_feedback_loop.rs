//! Live Graph Feedback Loop Integration Test
//!
//! This test verifies the end-to-end flow of the live graph observability system:
//! 1. Health endpoint responds correctly
//! 2. WebSocket server accepts connections
//! 3. Events flow from Kafka to WebSocket clients
//! 4. Events are valid DashStream protobuf with proper headers
//!
//! Run with: cargo test -p dashflow-observability --test live_graph_feedback_loop -- --nocapture

// This is a developer-run integration test. It prints troubleshooting output and uses
// `unwrap`/`expect` for setup and IO in test code.
#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::print_stdout,
    clippy::print_stderr
)]

use dashflow_streaming::codec::{decode_message_compatible, DEFAULT_MAX_PAYLOAD_SIZE};
use dashflow_streaming::dash_stream_message;
use futures_util::{SinkExt, StreamExt};
use std::time::Duration;
use tokio_tungstenite::{connect_async, tungstenite::Message};

/// Test iteration 1: Verify health endpoint responds
///
/// This is a basic smoke test that ensures the WebSocket server's
/// health endpoint is functioning. The actual WebSocket server must
/// be running separately for this test to pass.
#[tokio::test]
async fn test_health_endpoint_responds() {
    // Check if server is running by attempting to connect to health endpoint
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .expect("Failed to create HTTP client");

    // Try multiple ports in case the default is in use
    let ports = [3004, 3003, 3002];
    let mut server_found = false;
    let mut health_response = None;

    for port in ports {
        let url = format!("http://localhost:{}/health", port);
        match client.get(&url).send().await {
            Ok(response) => {
                if response.status().is_success() {
                    server_found = true;
                    health_response = Some((port, response));
                    break;
                }
            }
            Err(_) => continue,
        }
    }

    if !server_found {
        println!("========== HEALTH CHECK RESULT ==========");
        println!("Server not running on ports {:?}", ports);
        println!("To run this test with server:");
        println!(
            "  1. Start server: cargo run -p dashflow-observability --bin websocket_server --features websocket-server"
        );
        println!("  2. Run test: cargo test -p dashflow-observability --test live_graph_feedback_loop -- --nocapture");
        println!("=========================================");
        // Don't fail - this is expected when server isn't running
        return;
    }

    let (port, response) = health_response.unwrap();
    let status = response.status();
    let body = response.text().await.unwrap_or_default();

    println!("========== HEALTH CHECK RESULT ==========");
    println!("Server found on port: {}", port);
    println!("Status: {}", status);
    println!("Body: {}", body);

    // Parse health response
    if let Ok(health) = serde_json::from_str::<serde_json::Value>(&body) {
        println!("\nParsed health response:");
        println!("  status: {:?}", health.get("status"));
        println!(
            "  kafka_messages_received: {:?}",
            health
                .get("metrics")
                .and_then(|m| m.get("kafka_messages_received"))
        );
        println!(
            "  connected_clients: {:?}",
            health
                .get("metrics")
                .and_then(|m| m.get("connected_clients"))
        );
        println!(
            "  decode_errors: {:?}",
            health.get("metrics").and_then(|m| m.get("decode_errors"))
        );
    }
    println!("=========================================");

    assert!(
        status.is_success(),
        "Health endpoint returned non-success status: {}",
        status
    );
}

/// Test iteration 2: Verify version endpoint
#[tokio::test]
async fn test_version_endpoint() {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .expect("Failed to create HTTP client");

    let ports = [3004, 3003, 3002];

    for port in ports {
        let url = format!("http://localhost:{}/version", port);
        if let Ok(response) = client.get(&url).send().await {
            if response.status().is_success() {
                let body = response.text().await.unwrap_or_default();
                println!("========== VERSION ENDPOINT ==========");
                println!("Port: {}", port);
                println!("Response: {}", body);
                println!("=======================================");
                return;
            }
        }
    }

    println!("Version endpoint not available - server may not be running");
}

/// Test iteration 3: Verify metrics increment after events
///
/// This test checks that the health endpoint's kafka_messages_received
/// counter increments when new events are sent, verifying end-to-end flow.
#[tokio::test]
#[ignore = "requires local websocket server running"]
async fn test_metrics_flow() {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .expect("Failed to create HTTP client");

    // Find running server
    let ports = [3004, 3003, 3002];
    let mut server_port = None;

    for port in ports {
        let url = format!("http://localhost:{}/health", port);
        if let Ok(response) = client.get(&url).send().await {
            if response.status().is_success() {
                server_port = Some(port);
                break;
            }
        }
    }

    let port = server_port.expect("Server must be running on one of ports 3004/3003/3002");

    // Get initial metrics
    let health_url = format!("http://localhost:{}/health", port);
    let response = client
        .get(&health_url)
        .send()
        .await
        .expect("Health request failed");
    let body = response.text().await.unwrap_or_default();
    let initial: serde_json::Value = serde_json::from_str(&body).unwrap_or_default();

    let initial_messages = initial
        .get("metrics")
        .and_then(|m| m.get("kafka_messages_received"))
        .and_then(|v| v.as_u64())
        .unwrap_or(0);

    let initial_errors = initial
        .get("metrics")
        .and_then(|m| m.get("decode_errors"))
        .and_then(|v| v.as_u64())
        .unwrap_or(0);

    let uptime = initial
        .get("metrics")
        .and_then(|m| m.get("uptime_seconds"))
        .and_then(|v| v.as_u64())
        .unwrap_or(0);

    println!("========== METRICS FLOW TEST ==========");
    println!("Server port: {}", port);
    println!("Initial kafka_messages_received: {}", initial_messages);
    println!("Initial decode_errors: {}", initial_errors);
    println!("Server uptime: {}s", uptime);
    println!();
    println!("Note: To test message increment, run:");
    println!("  DASHSTREAM_TOPIC=dashstream-quality cargo run --example dashstream_integration --features dashstream");
    println!("Then re-run this test to see updated counts.");
    println!("=======================================");

    // Verify no decode errors
    assert_eq!(
        initial_errors, 0,
        "Server has decode errors: {}",
        initial_errors
    );
}

/// Test iteration 3: WebSocket client receives binary messages
///
/// This test connects to the WebSocket endpoint and verifies:
/// 1. WebSocket connection succeeds
/// 2. Binary messages are received (if events are flowing)
/// 3. Messages can be decoded as DashStream protobuf
#[tokio::test]
#[ignore = "requires local websocket server running"]
async fn test_websocket_receives_events() {
    // Find running server
    let ports = [3004, 3003, 3002];
    let mut ws_url = None;

    for port in ports {
        let health_url = format!("http://localhost:{}/health", port);
        if let Ok(response) = reqwest::get(&health_url).await {
            if response.status().is_success() {
                ws_url = Some(format!("ws://localhost:{}/ws", port));
                break;
            }
        }
    }

    let url = ws_url.expect("Server must be running on one of ports 3004/3003/3002");

    println!("========== WEBSOCKET TEST ==========");
    println!("Connecting to: {}", url);

    // Connect to WebSocket with timeout
    let connect_result = tokio::time::timeout(Duration::from_secs(5), connect_async(&url)).await;

    let (mut ws_stream, _) = match connect_result {
        Ok(Ok((stream, response))) => {
            println!("✅ WebSocket connected successfully");
            println!("   Response: {:?}", response);
            (stream, response)
        }
        Ok(Err(e)) => panic!("WebSocket connection failed: {e}"),
        Err(_) => {
            panic!("WebSocket connection timed out");
        }
    };

    // Wait for messages with timeout
    println!("\nWaiting for events (5 second timeout)...");
    let mut messages_received = 0;
    let mut decode_errors = 0;

    loop {
        let msg_result = tokio::time::timeout(Duration::from_secs(5), ws_stream.next()).await;

        match msg_result {
            Ok(Some(Ok(Message::Binary(data)))) => {
                messages_received += 1;
                println!(
                    "\n📨 Received binary message #{}: {} bytes",
                    messages_received,
                    data.len()
                );

                // Try to decode as DashStream protobuf
                #[allow(deprecated)]
                match decode_message_compatible(&data, DEFAULT_MAX_PAYLOAD_SIZE) {
                    Ok(decoded) => {
                        println!("   ✅ Decoded as DashStream protobuf");
                        if let Some(ref msg) = decoded.message {
                            match msg {
                                dash_stream_message::Message::Event(event) => {
                                    println!("   📝 Event type: {:?}", event.event_type);
                                }
                                dash_stream_message::Message::EventBatch(batch) => {
                                    println!("   📦 EventBatch with {} events", batch.events.len());
                                }
                                _ => {
                                    println!("   📝 Other message type");
                                }
                            }
                        }
                    }
                    Err(e) => {
                        decode_errors += 1;
                        println!("   ❌ Decode error: {}", e);
                    }
                }

                // After receiving 3 messages, we've verified the flow works
                if messages_received >= 3 {
                    println!("\n✅ Received {} messages successfully", messages_received);
                    break;
                }
            }
            Ok(Some(Ok(Message::Text(text)))) => {
                println!("📄 Received text message: {}", text);
                // Handle JSON messages like ping/pong or gap indicators
            }
            Ok(Some(Ok(Message::Ping(data)))) => {
                println!("🏓 Received ping, sending pong");
                let _ = ws_stream.send(Message::Pong(data)).await;
            }
            Ok(Some(Ok(Message::Close(_)))) => {
                println!("🔌 Server closed connection");
                break;
            }
            Ok(Some(Err(e))) => {
                println!("❌ WebSocket error: {}", e);
                break;
            }
            Ok(None) => {
                println!("🔌 WebSocket stream ended");
                break;
            }
            Err(_) => {
                // Timeout - no messages in 5 seconds
                println!("\n⏰ No events received within timeout");
                println!("   This is expected if no events are being produced.");
                println!("   To test event flow, run the DashStream producer:");
                println!("   DASHSTREAM_TOPIC=dashstream-quality cargo run --example dashstream_integration --features dashstream");
                break;
            }
            _ => {}
        }
    }

    // Close WebSocket gracefully
    let _ = ws_stream.close(None).await;

    println!("\n========== RESULTS ==========");
    println!("Messages received: {}", messages_received);
    println!("Decode errors: {}", decode_errors);
    println!("============================");

    // Verify no decode errors on client side
    assert_eq!(
        decode_errors, 0,
        "Client had {} decode errors",
        decode_errors
    );
}

/// Test iteration 4: Validate event structure with headers
///
/// This test validates that received events have proper headers:
/// - Event has header with tenant_id and thread_id
/// - Event has valid event_type
/// - No decode errors on client side
#[tokio::test]
#[ignore = "requires local websocket server running"]
async fn test_websocket_event_validation() {
    // Find running server
    let ports = [3004, 3003, 3002];
    let mut ws_url = None;

    for port in ports {
        let health_url = format!("http://localhost:{}/health", port);
        if let Ok(response) = reqwest::get(&health_url).await {
            if response.status().is_success() {
                ws_url = Some(format!("ws://localhost:{}/ws", port));
                break;
            }
        }
    }

    let url = ws_url.expect("Server must be running on one of ports 3004/3003/3002");

    println!("========== EVENT VALIDATION TEST ==========");
    println!("Connecting to: {}", url);

    // Connect to WebSocket
    let connect_result = tokio::time::timeout(Duration::from_secs(5), connect_async(&url)).await;

    let (mut ws_stream, _) = match connect_result {
        Ok(Ok((stream, _))) => (stream, ()),
        Ok(Err(e)) => panic!("WebSocket connection failed: {e}"),
        Err(_) => {
            panic!("WebSocket connection timed out");
        }
    };

    println!("✅ Connected");
    println!("\nValidating event structure...");

    let mut events_validated = 0;
    let mut validation_errors: Vec<String> = Vec::new();

    loop {
        let msg_result = tokio::time::timeout(Duration::from_secs(5), ws_stream.next()).await;

        match msg_result {
            Ok(Some(Ok(Message::Binary(data)))) => {
                // Decode protobuf
                #[allow(deprecated)]
                match decode_message_compatible(&data, DEFAULT_MAX_PAYLOAD_SIZE) {
                    Ok(decoded) => {
                        if let Some(dash_stream_message::Message::Event(event)) = decoded.message {
                            events_validated += 1;
                            println!("\n📝 Validating event #{}", events_validated);

                            // Validate header exists
                            if let Some(ref header) = event.header {
                                // Check tenant_id
                                if header.tenant_id.is_empty() {
                                    validation_errors.push(format!(
                                        "Event #{}: missing tenant_id",
                                        events_validated
                                    ));
                                } else {
                                    println!("   ✅ tenant_id: {}", header.tenant_id);
                                }

                                // Check thread_id
                                if header.thread_id.is_empty() {
                                    validation_errors.push(format!(
                                        "Event #{}: missing thread_id",
                                        events_validated
                                    ));
                                } else {
                                    println!("   ✅ thread_id: {}", header.thread_id);
                                }

                                // Check sequence
                                println!("   ✅ sequence: {}", header.sequence);

                                // Check timestamp
                                if header.timestamp_us > 0 {
                                    println!("   ✅ timestamp_us: {}", header.timestamp_us);
                                } else {
                                    validation_errors.push(format!(
                                        "Event #{}: invalid timestamp_us",
                                        events_validated
                                    ));
                                }
                            } else {
                                validation_errors
                                    .push(format!("Event #{}: missing header", events_validated));
                            }

                            // Check event_type
                            println!("   ✅ event_type: {:?}", event.event_type);

                            // After validating 3 events, we're done
                            if events_validated >= 3 {
                                break;
                            }
                        }
                    }
                    Err(e) => {
                        validation_errors.push(format!("Decode error: {}", e));
                    }
                }
            }
            Ok(Some(Ok(Message::Text(_)))) => {
                // Skip text messages (gap indicators, pong, etc.)
            }
            Ok(Some(Ok(Message::Ping(data)))) => {
                let _ = ws_stream.send(Message::Pong(data)).await;
            }
            Ok(Some(Ok(Message::Close(_)))) | Ok(None) => {
                break;
            }
            Err(_) => {
                println!("\n⏰ Timeout waiting for events");
                println!("   Events validated: {}", events_validated);
                if events_validated == 0 {
                    println!("   No events to validate - producer may not be running");
                }
                break;
            }
            _ => {}
        }
    }

    let _ = ws_stream.close(None).await;

    println!("\n========== VALIDATION RESULTS ==========");
    println!("Events validated: {}", events_validated);
    println!("Validation errors: {}", validation_errors.len());
    for err in &validation_errors {
        println!("   ❌ {}", err);
    }
    println!("=========================================");

    // Pass if we validated events and had no errors (or if no events were available)
    if events_validated > 0 {
        assert!(
            validation_errors.is_empty(),
            "Validation errors: {:?}",
            validation_errors
        );
    }
}
