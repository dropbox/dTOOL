# DashFlow Test Utils

Shared test infrastructure for DashFlow Rust integration tests.

## Features

- **Credential Loading**: Automatic loading and validation of API keys and credentials
- **Service Health Checks**: Wait for services to be ready before running tests
- **Docker Management**: Start/stop docker-compose services programmatically
- **Test Helpers**: Common utilities for integration testing

## Usage

### In Integration Tests

```rust
use dashflow_test_utils::{init_test_env, openai_credentials, check_chroma};

#[tokio::test]
async fn test_with_real_services() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize test environment (loads .env, sets up logging)
    init_test_env()?;

    // Load credentials (will error if OPENAI_API_KEY is missing)
    let creds = openai_credentials()?;
    let api_key = creds.get_required("OPENAI_API_KEY")?;

    // Wait for service to be ready
    check_chroma().await?;

    // Your test code here...

    Ok(())
}
```

### With Custom Credentials

```rust
use dashflow_test_utils::CredentialsLoader;

#[tokio::test]
async fn test_custom_service() -> Result<(), Box<dyn std::error::Error>> {
    let creds = CredentialsLoader::new()
        .require("MY_API_KEY")
        .optional("MY_OPTIONAL_KEY")
        .load()?;

    let api_key = creds.get_required("MY_API_KEY")?;
    let optional = creds.get_optional("MY_OPTIONAL_KEY");

    // Your test code here...

    Ok(())
}
```

### With Docker Services

```rust
use dashflow_test_utils::{setup_docker_services, teardown_docker_services};

#[tokio::test]
async fn test_with_docker() -> Result<(), Box<dyn std::error::Error>> {
    // Start all docker services and wait for health checks
    let services = setup_docker_services().await?;

    // Your test code here...

    // Clean up
    teardown_docker_services(&services).await?;

    Ok(())
}
```

### With Health Checks

```rust
use dashflow_test_utils::HealthChecker;
use std::time::Duration;

#[tokio::test]
async fn test_custom_health_check() -> Result<(), Box<dyn std::error::Error>> {
    let checker = HealthChecker::new()
        .with_max_retries(30)
        .with_retry_delay(Duration::from_secs(2));

    // Check single service
    checker.check_http("http://localhost:8000/health").await?;

    // Check multiple services in parallel
    checker.check_all(&[
        "http://localhost:8000/health",
        "http://localhost:6333/health",
    ]).await?;

    Ok(())
}
```

## Environment Variables

See `.env.test.template` and `CREDENTIALS_GUIDE.md` for full list.

### Common Variables

- `OPENAI_API_KEY` - OpenAI API key
- `ANTHROPIC_API_KEY` - Anthropic API key
- `CHROMA_URL` - Chroma vector store URL (default: http://localhost:8000)
- `QDRANT_URL` - Qdrant vector store URL (default: http://localhost:6333)
- `TEST_TIMEOUT` - Test timeout in seconds (default: 300)

## Testing

Run the test-utils tests:

```bash
cargo test -p dashflow-test-utils
```

## See Also

- [CREDENTIALS_GUIDE.md](../CREDENTIALS_GUIDE.md) - How to get and configure credentials
- [docker-compose.test.yml](../docker-compose.test.yml) - Docker services configuration
- [.env.test.template](../.env.test.template) - Environment template
