# DashFlow Integration Testing Guide

**Last Updated:** 2025-12-22 (Worker #1413 - M-393)

This guide establishes conventions for writing crate-level integration tests in DashFlow. It covers testcontainers for databases/services and mock servers for HTTP APIs.

---

## Quick Start

```bash
# Run all tests for a specific crate
cargo test -p dashflow-openai

# Run integration tests only (tests/ directory)
cargo test -p dashflow-streaming --test kafka_testcontainers

# Run Docker-requiring tests (marked with #[ignore])
cargo test -p dashflow-postgres-checkpointer --test postgres_testcontainers -- --ignored

# Use nextest for parallel execution
cargo nextest run -p dashflow-chroma
```

**macOS Docker Setup (Colima):**
```bash
export DOCKER_HOST=unix://$HOME/.colima/default/docker.sock
```

---

## Directory Structure

Integration tests live in a `tests/` directory at the crate root:

```
crates/dashflow-my-crate/
├── Cargo.toml
├── src/
│   └── lib.rs
└── tests/
    ├── my_service_testcontainers.rs    # Docker-based integration tests
    └── my_api_mock_server_tests.rs     # Mock HTTP API tests
```

---

## Test File Conventions

### File Header

Every integration test file should start with:

```rust
//! Integration tests for [component] using [testcontainers|wiremock].
//!
//! Run with: cargo test -p dashflow-my-crate --test test_file_name
//! Docker tests: cargo test -p dashflow-my-crate --test test_file_name -- --ignored

use dashflow_my_crate::*;
```

### Test Attributes

| Attribute | Use Case |
|-----------|----------|
| `#[tokio::test]` | Async tests (default for DashFlow) |
| `#[ignore]` | Tests requiring Docker |
| `#[test]` | Synchronous unit tests |

---

## Pattern 1: Mock HTTP APIs with wiremock

Use wiremock for testing HTTP API clients (LLM providers, external services).

### Dependencies

```toml
# In your crate's Cargo.toml
[dev-dependencies]
wiremock = { workspace = true }
serde_json = { workspace = true }
tokio = { workspace = true, features = ["macros", "rt-multi-thread"] }
```

### Template

```rust
//! Mock server integration tests for [API Name].
//!
//! Run with: cargo test -p dashflow-my-crate --test my_api_mock_server_tests

use serde_json::json;
use wiremock::matchers::{body_partial_json, header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

/// Helper: Create a mock response for the API
fn mock_api_response(content: &str) -> serde_json::Value {
    json!({
        "id": "test-123",
        "content": content,
        "usage": { "total_tokens": 100 }
    })
}

/// Helper: Create a client configured to use the mock server
fn create_mock_client(base_url: &str) -> MyApiClient {
    MyApiClient::builder()
        .base_url(base_url)
        .api_key("test-key")
        .build()
        .unwrap()
}

#[tokio::test]
async fn test_basic_api_call() {
    // Start mock server
    let mock_server = MockServer::start().await;

    // Define expected request and response
    Mock::given(method("POST"))
        .and(path("/v1/endpoint"))
        .and(header("authorization", "Bearer test-key"))
        .and(body_partial_json(json!({
            "model": "test-model"
        })))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(mock_api_response("Hello!"))
        )
        .mount(&mock_server)
        .await;

    // Create client pointing to mock server
    let client = create_mock_client(&mock_server.uri());

    // Execute and assert
    let result = client.call().await;
    assert!(result.is_ok());
    assert_eq!(result.unwrap().content, "Hello!");
}

#[tokio::test]
async fn test_api_error_handling() {
    let mock_server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/v1/endpoint"))
        .respond_with(
            ResponseTemplate::new(429)
                .set_body_json(json!({
                    "error": {"message": "Rate limited", "type": "rate_limit_error"}
                }))
        )
        .mount(&mock_server)
        .await;

    let client = create_mock_client(&mock_server.uri());
    let result = client.call().await;

    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), MyError::RateLimited(_)));
}

#[tokio::test]
async fn test_retry_on_transient_error() {
    let mock_server = MockServer::start().await;

    // First call fails, second succeeds
    Mock::given(method("POST"))
        .and(path("/v1/endpoint"))
        .respond_with(ResponseTemplate::new(503))
        .up_to_n_times(1)  // Only fail once
        .mount(&mock_server)
        .await;

    Mock::given(method("POST"))
        .and(path("/v1/endpoint"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(mock_api_response("Success!"))
        )
        .mount(&mock_server)
        .await;

    let client = create_mock_client(&mock_server.uri());
    let result = client.call_with_retry().await;

    assert!(result.is_ok());
}
```

### wiremock Matchers Reference

| Matcher | Description |
|---------|-------------|
| `method("POST")` | HTTP method |
| `path("/v1/endpoint")` | URL path |
| `path_regex(r"/v1/.*")` | Path regex |
| `header("key", "value")` | Header exact match |
| `header_exists("key")` | Header presence |
| `body_partial_json(json)` | JSON body contains |
| `body_json(json)` | JSON body exact match |
| `body_string("text")` | Raw body match |
| `query_param("key", "val")` | Query parameter |

---

## Pattern 2: Testcontainers for Services

Use testcontainers for testing against real databases and message queues.

### Dependencies

```toml
# In your crate's Cargo.toml
[dev-dependencies]
testcontainers = { workspace = true }
testcontainers_modules = { workspace = true, features = ["postgres", "redis", "kafka"] }
tokio = { workspace = true, features = ["macros", "rt-multi-thread", "time"] }
```

### Available Modules

| Module | Use Case | Port |
|--------|----------|------|
| `testcontainers_modules::postgres::Postgres` | PostgreSQL | 5432 |
| `testcontainers_modules::redis::Redis` | Redis | 6379 |
| `testcontainers_modules::kafka::apache` | Apache Kafka | 9093 |
| `testcontainers_modules::localstack::LocalStack` | AWS (S3, DynamoDB) | 4566 |
| `testcontainers::GenericImage` | Any Docker image | varies |

### Template: PostgreSQL

```rust
//! PostgreSQL integration tests using testcontainers.
//!
//! Run with: cargo test -p dashflow-my-crate --test postgres_testcontainers -- --ignored

use std::time::Duration;
use testcontainers::runners::AsyncRunner;
use testcontainers_modules::postgres::Postgres;

/// Helper: Start PostgreSQL container and return connection URL
async fn start_postgres() -> (testcontainers::ContainerAsync<Postgres>, String) {
    let container = Postgres::default()
        .start()
        .await
        .expect("Failed to start PostgreSQL container");

    let host = container.get_host().await.unwrap();
    let port = container.get_host_port_ipv4(5432).await.unwrap();

    let connection_url = format!(
        "postgres://postgres:postgres@{}:{}/postgres",
        host, port
    );

    // Wait for PostgreSQL to be ready
    tokio::time::sleep(Duration::from_secs(2)).await;

    (container, connection_url)
}

#[tokio::test]
#[ignore] // Requires Docker
async fn test_postgres_crud_operations() {
    let (_container, connection_url) = start_postgres().await;

    // Create your checkpointer/store with the connection URL
    let store = MyPostgresStore::new(&connection_url).await.unwrap();

    // Test operations
    store.put("key1", b"value1").await.unwrap();
    let result = store.get("key1").await.unwrap();
    assert_eq!(result, Some(b"value1".to_vec()));
}

#[tokio::test]
#[ignore] // Requires Docker
async fn test_postgres_transactions() {
    let (_container, connection_url) = start_postgres().await;
    let store = MyPostgresStore::new(&connection_url).await.unwrap();

    // Test transaction behavior
    store.begin_transaction().await.unwrap();
    store.put("key", b"value").await.unwrap();
    store.rollback().await.unwrap();

    assert!(store.get("key").await.unwrap().is_none());
}
```

### Template: Redis

```rust
//! Redis integration tests using testcontainers.
//!
//! Run with: cargo test -p dashflow-my-crate --test redis_testcontainers -- --ignored

use std::time::Duration;
use testcontainers::runners::AsyncRunner;
use testcontainers_modules::redis::Redis;

async fn start_redis() -> (testcontainers::ContainerAsync<Redis>, String) {
    let container = Redis::default()
        .start()
        .await
        .expect("Failed to start Redis container");

    let host = container.get_host().await.unwrap();
    let port = container.get_host_port_ipv4(6379).await.unwrap();

    let connection_url = format!("redis://{}:{}", host, port);

    tokio::time::sleep(Duration::from_secs(1)).await;

    (container, connection_url)
}

#[tokio::test]
#[ignore] // Requires Docker
async fn test_redis_cache_operations() {
    let (_container, connection_url) = start_redis().await;

    let cache = MyRedisCache::new(&connection_url).await.unwrap();

    cache.set("key", "value", None).await.unwrap();
    let result = cache.get("key").await.unwrap();
    assert_eq!(result, Some("value".to_string()));
}

#[tokio::test]
#[ignore] // Requires Docker
async fn test_redis_expiration() {
    let (_container, connection_url) = start_redis().await;

    let cache = MyRedisCache::new(&connection_url).await.unwrap();

    cache.set("key", "value", Some(Duration::from_secs(1))).await.unwrap();

    tokio::time::sleep(Duration::from_secs(2)).await;

    let result = cache.get("key").await.unwrap();
    assert!(result.is_none());
}
```

### Template: Kafka

```rust
//! Kafka integration tests using testcontainers.
//!
//! Run with: cargo test -p dashflow-my-crate --test kafka_testcontainers -- --ignored

use std::time::Duration;
use testcontainers::runners::AsyncRunner;
use testcontainers_modules::kafka::apache::{Kafka, KAFKA_PORT};

async fn start_kafka() -> (testcontainers::ContainerAsync<Kafka>, String) {
    let container = Kafka::default()
        .start()
        .await
        .expect("Failed to start Kafka container");

    let host = container.get_host().await.unwrap();
    let port = container.get_host_port_ipv4(KAFKA_PORT).await.unwrap();

    let bootstrap_servers = format!("{}:{}", host, port);

    // Kafka needs time to initialize
    tokio::time::sleep(Duration::from_secs(5)).await;

    (container, bootstrap_servers)
}

#[tokio::test]
#[ignore] // Requires Docker
async fn test_kafka_produce_consume() {
    let (_container, bootstrap_servers) = start_kafka().await;

    let producer = MyKafkaProducer::new(&bootstrap_servers).await.unwrap();
    let consumer = MyKafkaConsumer::new(&bootstrap_servers, "test-topic").await.unwrap();

    producer.send("test-topic", "key", b"value").await.unwrap();

    let message = consumer.receive(Duration::from_secs(10)).await.unwrap();
    assert_eq!(message.payload, b"value");
}
```

### Template: LocalStack (AWS S3/DynamoDB)

```rust
//! AWS service integration tests using LocalStack.
//!
//! Run with: cargo test -p dashflow-my-crate --test localstack_testcontainers -- --ignored

use std::time::Duration;
use testcontainers::runners::AsyncRunner;
use testcontainers_modules::localstack::LocalStack;
use aws_sdk_s3::config::Credentials;

async fn start_localstack() -> (testcontainers::ContainerAsync<LocalStack>, String) {
    let container = LocalStack::default()
        .start()
        .await
        .expect("Failed to start LocalStack container");

    let host = container.get_host().await.unwrap();
    let port = container.get_host_port_ipv4(4566).await.unwrap();

    let endpoint_url = format!("http://{}:{}", host, port);

    // LocalStack needs time to initialize services
    tokio::time::sleep(Duration::from_secs(5)).await;

    (container, endpoint_url)
}

fn test_credentials() -> Credentials {
    Credentials::new("test", "test", None, None, "static")
}

#[tokio::test]
#[ignore] // Requires Docker
async fn test_s3_operations() {
    let (_container, endpoint_url) = start_localstack().await;

    let config = aws_config::defaults(aws_config::BehaviorVersion::latest())
        .endpoint_url(&endpoint_url)
        .credentials_provider(test_credentials())
        .region(aws_config::Region::new("us-east-1"))
        .load()
        .await;

    let s3_client = aws_sdk_s3::Client::new(&config);

    // Create bucket
    s3_client
        .create_bucket()
        .bucket("test-bucket")
        .send()
        .await
        .unwrap();

    // Upload object
    s3_client
        .put_object()
        .bucket("test-bucket")
        .key("test-key")
        .body(aws_sdk_s3::primitives::ByteStream::from_static(b"test-data"))
        .send()
        .await
        .unwrap();

    // Verify
    let result = s3_client
        .get_object()
        .bucket("test-bucket")
        .key("test-key")
        .send()
        .await
        .unwrap();

    let data = result.body.collect().await.unwrap().into_bytes();
    assert_eq!(&data[..], b"test-data");
}
```

### Template: Custom Container (GenericImage)

```rust
//! Custom container integration tests using GenericImage.
//!
//! Run with: cargo test -p dashflow-my-crate --test custom_testcontainers -- --ignored

use std::time::Duration;
use testcontainers::core::{ContainerPort, WaitFor};
use testcontainers::runners::AsyncRunner;
use testcontainers::GenericImage;

async fn start_custom_service() -> (testcontainers::ContainerAsync<GenericImage>, String) {
    let container = GenericImage::new("my-image", "latest")
        .with_exposed_port(ContainerPort::Tcp(8080))
        .with_env_var("CONFIG_VAR", "value")
        .with_wait_for(WaitFor::message_on_stdout("Server started"))
        .start()
        .await
        .expect("Failed to start custom container");

    let host = container.get_host().await.unwrap();
    let port = container.get_host_port_ipv4(8080).await.unwrap();

    let url = format!("http://{}:{}", host, port);

    tokio::time::sleep(Duration::from_secs(3)).await;

    (container, url)
}

#[tokio::test]
#[ignore] // Requires Docker
async fn test_custom_service() {
    let (_container, url) = start_custom_service().await;

    let client = reqwest::Client::new();
    let response = client.get(&format!("{}/health", url)).send().await.unwrap();

    assert!(response.status().is_success());
}
```

---

## Pattern 3: Hybrid Tests

Some tests combine mock servers for external APIs with testcontainers for local services:

```rust
#[tokio::test]
#[ignore] // Requires Docker
async fn test_end_to_end_flow() {
    // Start database container
    let (_pg_container, db_url) = start_postgres().await;

    // Start mock API server
    let mock_server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/process"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"status": "ok"})))
        .mount(&mock_server)
        .await;

    // Configure application with both
    let app = MyApp::builder()
        .database_url(&db_url)
        .api_url(&mock_server.uri())
        .build()
        .await
        .unwrap();

    // Test full flow
    let result = app.process_and_store("input").await;
    assert!(result.is_ok());
}
```

---

## Running Tests

### Single Crate

```bash
# All tests
cargo test -p dashflow-openai

# Specific test file
cargo test -p dashflow-openai --test openai_mock_server_tests

# Specific test function
cargo test -p dashflow-openai --test openai_mock_server_tests -- test_basic_chat
```

### Docker-Requiring Tests

```bash
# Run ignored tests (Docker required)
cargo test -p dashflow-postgres-checkpointer --test postgres_testcontainers -- --ignored

# On macOS with Colima
export DOCKER_HOST=unix://$HOME/.colima/default/docker.sock
cargo test -p dashflow-streaming --test kafka_testcontainers -- --ignored
```

### Parallel Execution with nextest

```bash
# Install nextest
cargo install cargo-nextest

# Run tests with better parallelism
cargo nextest run -p dashflow-chroma

# Run only Docker tests
cargo nextest run -p dashflow-streaming --test kafka_testcontainers -- --ignored
```

### CI/Local Verification

```bash
# Verify all mock server tests pass (no Docker needed)
cargo test --workspace -- --skip testcontainers

# Verify all tests including Docker
cargo test --workspace -- --include-ignored
```

---

## Best Practices

### 1. Test Isolation

Each test should be independent. Don't share state between tests:

```rust
// GOOD: Each test creates its own container/mock
#[tokio::test]
#[ignore]
async fn test_one() {
    let (_container, url) = start_postgres().await;
    // ...
}

#[tokio::test]
#[ignore]
async fn test_two() {
    let (_container, url) = start_postgres().await;
    // ...
}
```

### 2. Container Cleanup

Testcontainers automatically cleans up when the container variable is dropped. Keep the container alive for the test duration:

```rust
#[tokio::test]
async fn test_example() {
    // _container must live until end of test
    let (_container, url) = start_service().await;

    // Use url...

    // Container cleaned up here when _container is dropped
}
```

### 3. Startup Timing

Always add appropriate delays for service readiness:

| Service | Recommended Delay |
|---------|------------------|
| PostgreSQL | 2s |
| Redis | 1s |
| Kafka | 5s |
| LocalStack | 5s |
| Custom | varies |

### 4. Error Messages

Use descriptive panic messages for container startup:

```rust
let container = Postgres::default()
    .start()
    .await
    .expect("Failed to start PostgreSQL container - is Docker running?");
```

### 5. Feature Gating (Optional)

For crates where testcontainers are optional:

```toml
# Cargo.toml
[features]
default = []
testcontainers = ["dep:testcontainers", "dep:testcontainers_modules"]

[dev-dependencies]
testcontainers = { workspace = true, optional = true }
testcontainers_modules = { workspace = true, optional = true }
```

---

## Troubleshooting

### Docker Not Running

```
Error: failed to start container
```

**Solution:** Start Docker Desktop or Colima:
```bash
# macOS with Colima
colima start
export DOCKER_HOST=unix://$HOME/.colima/default/docker.sock
```

### Port Conflicts

```
Error: port already in use
```

**Solution:** Testcontainers uses random ports. If you see conflicts, another instance may be running:
```bash
docker ps  # Check for orphan containers
docker stop $(docker ps -q)  # Stop all containers
```

### Slow Tests

**Solution:** Use `cargo nextest` for parallelism, or consider sharing containers:
```rust
// For test suites that can share a container
static POSTGRES_URL: OnceLock<String> = OnceLock::new();

fn get_postgres_url() -> &'static str {
    POSTGRES_URL.get_or_init(|| {
        // Start container once for all tests
    })
}
```

### Container Startup Timeout

```
Error: container failed to become ready
```

**Solution:** Increase the wait time or check container logs:
```rust
// Increase wait time
tokio::time::sleep(Duration::from_secs(10)).await;

// Or use WaitFor with longer timeout for GenericImage
GenericImage::new("image", "tag")
    .with_wait_for(WaitFor::seconds(30))
```

---

## Existing Test Examples

| Crate | Test File | Pattern |
|-------|-----------|---------|
| dashflow-openai | `openai_mock_server_tests.rs` | wiremock |
| dashflow-huggingface | `huggingface_mock_server_tests.rs` | wiremock |
| dashflow-streaming | `kafka_testcontainers.rs` | testcontainers (Kafka) |
| dashflow-postgres-checkpointer | `postgres_testcontainers.rs` | testcontainers (PostgreSQL) |
| dashflow-redis-checkpointer | `redis_testcontainers.rs` | testcontainers (Redis) |
| dashflow-s3-checkpointer | `s3_testcontainers.rs` | testcontainers (LocalStack) |
| dashflow-chroma | `chroma_testcontainers.rs` | GenericImage |

---

## Adding Tests to a New Crate

1. **Create `tests/` directory** in your crate
2. **Choose pattern** based on what you're testing:
   - HTTP API → wiremock
   - Database → testcontainers
   - Message queue → testcontainers
3. **Copy appropriate template** from this guide
4. **Add dev-dependencies** to Cargo.toml
5. **Run tests** to verify setup
6. **Document** in crate README

---

## See Also

- [BEST_PRACTICES.md](./BEST_PRACTICES.md) - General development practices
- [TESTING_OBSERVABILITY.md](./TESTING_OBSERVABILITY.md) - Observability testing
- [wiremock documentation](https://docs.rs/wiremock)
- [testcontainers documentation](https://docs.rs/testcontainers)
