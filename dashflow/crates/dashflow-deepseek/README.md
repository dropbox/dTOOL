# dashflow-deepseek

DeepSeek integration for DashFlow - access DeepSeek's powerful reasoning models through an OpenAI-compatible API.

## Overview

DeepSeek provides advanced language models with strong reasoning capabilities. This crate wraps the OpenAI-compatible DeepSeek API, making it easy to integrate DeepSeek models into your Rust applications.

**Key Features:**
- OpenAI-compatible API (drop-in replacement)
- Access to DeepSeek-Chat and DeepSeek-Coder models
- Streaming support for real-time responses
- Tool calling capabilities
- Rate limiting support
- Full DashFlow trait compatibility

## Installation

Add to your `Cargo.toml`:
```toml
[dependencies]
dashflow-deepseek = "1.11"
dashflow = "1.11"
tokio = { version = "1", features = ["full"] }
```

## Quick Start

```rust
use dashflow::core::language_models::ChatModel;
use dashflow::core::messages::Message;
use dashflow_deepseek::ChatDeepSeek;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Set DEEPSEEK_API_KEY environment variable
    let chat = ChatDeepSeek::new()
        .with_model("deepseek-chat")
        .with_temperature(0.7);

    let messages = vec![
        Message::system("You are a helpful AI assistant."),
        Message::human("Explain quantum computing in simple terms.")
    ];

    let result = chat.generate(&messages, None).await?;
    println!("{}", result.generations[0].message.as_text());
    Ok(())
}
```

## Configuration

### Environment Variables

```bash
# Required: Your DeepSeek API key
export DEEPSEEK_API_KEY="your-api-key-here"

# Optional: Custom API endpoint (defaults to https://api.deepseek.com/v1)
export DEEPSEEK_API_BASE="https://custom-endpoint.com/v1"
```

### Builder Pattern

```rust
use dashflow_deepseek::ChatDeepSeek;

let chat = ChatDeepSeek::with_api_key("your-api-key")
    .with_model("deepseek-chat")
    .with_temperature(0.8)
    .with_max_tokens(2000)
    .with_top_p(0.95)
    .with_frequency_penalty(0.0)
    .with_presence_penalty(0.0);
```

## Available Models

### DeepSeek-Chat
```rust
use dashflow_deepseek::chat_models::models;

let chat = ChatDeepSeek::new()
    .with_model(models::DEEPSEEK_CHAT);
```

General-purpose conversational model with strong reasoning capabilities.

### DeepSeek-Coder
```rust
use dashflow_deepseek::chat_models::models;

let chat = ChatDeepSeek::new()
    .with_model(models::DEEPSEEK_CODER);
```

Specialized model optimized for code generation and programming tasks.

## Examples

### Basic Chat

```rust
use dashflow::core::language_models::ChatModel;
use dashflow::core::messages::Message;
use dashflow_deepseek::ChatDeepSeek;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let chat = ChatDeepSeek::new();

    let messages = vec![Message::human("What is Rust?")];
    let result = chat.generate(&messages, None).await?;

    println!("{}", result.generations[0].message.as_text());
    Ok(())
}
```

### Streaming Responses

```rust
use dashflow::core::language_models::ChatModel;
use dashflow::core::messages::Message;
use dashflow_deepseek::ChatDeepSeek;
use futures::StreamExt;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let chat = ChatDeepSeek::new()
        .with_model("deepseek-chat");

    let messages = vec![
        Message::human("Write a short poem about programming.")
    ];

    let mut stream = chat.stream(&messages, None).await?;

    while let Some(chunk) = stream.next().await {
        match chunk {
            Ok(chunk) => print!("{}", chunk.generation_info.as_text()),
            Err(e) => eprintln!("Error: {}", e),
        }
    }

    Ok(())
}
```

### Code Generation with DeepSeek-Coder

```rust
use dashflow::core::language_models::ChatModel;
use dashflow::core::messages::Message;
use dashflow_deepseek::ChatDeepSeek;
use dashflow_deepseek::chat_models::models;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let chat = ChatDeepSeek::new()
        .with_model(models::DEEPSEEK_CODER)
        .with_temperature(0.2); // Lower temperature for code

    let messages = vec![
        Message::system("You are an expert Rust programmer."),
        Message::human("Write a function to calculate Fibonacci numbers using memoization.")
    ];

    let result = chat.generate(&messages, None).await?;
    println!("{}", result.generations[0].message.as_text());
    Ok(())
}
```

### Tool Calling

```rust
use dashflow::core::language_models::{ChatModel, ToolChoice, ToolDefinition};
use dashflow::core::messages::Message;
use dashflow_deepseek::ChatDeepSeek;
use serde_json::json;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let chat = ChatDeepSeek::new();

    let tools = vec![
        ToolDefinition {
            name: "get_weather".to_string(),
            description: Some("Get the current weather for a location".to_string()),
            parameters: json!({
                "type": "object",
                "properties": {
                    "location": {
                        "type": "string",
                        "description": "City name"
                    }
                },
                "required": ["location"]
            }),
        }
    ];

    let messages = vec![
        Message::human("What's the weather in San Francisco?")
    ];

    let result = chat.generate_with_tools(
        &messages,
        None,
        Some(&tools),
        Some(&ToolChoice::Auto)
    ).await?;

    println!("{:?}", result.generations[0].message);
    Ok(())
}
```

### Rate Limiting

```rust
use dashflow::core::rate_limiters::{SimpleRateLimiter, RateLimiter};
use dashflow_deepseek::ChatDeepSeek;
use std::sync::Arc;

let rate_limiter = Arc::new(SimpleRateLimiter::new(
    10, // max requests per minute
    60, // window in seconds
)) as Arc<dyn RateLimiter>;

let chat = ChatDeepSeek::new()
    .with_rate_limiter(rate_limiter);
```

## Advanced Usage

### Custom API Endpoint

```rust
use async_openai::config::OpenAIConfig;
use dashflow_openai::ChatOpenAI;
use dashflow_deepseek::ChatDeepSeek;

let config = OpenAIConfig::new()
    .with_api_key("your-key")
    .with_api_base("https://custom.deepseek.com/v1");

let inner = ChatOpenAI::with_config(config)
    .with_model("deepseek-chat");

let chat = ChatDeepSeek { inner };
```

### Access to Inner ChatOpenAI

Since DeepSeek wraps ChatOpenAI, you can access the inner implementation:

```rust
let mut chat = ChatDeepSeek::new();

// Get reference
let inner_ref = chat.inner();

// Get mutable reference
let inner_mut = chat.inner_mut();

// Consume and get inner
let inner = chat.into_inner();
```

## Parameters

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `model` | String | `"deepseek-chat"` | Model name to use |
| `temperature` | f32 | `0.7` | Sampling temperature (0-2) |
| `max_tokens` | u32 | - | Maximum tokens to generate |
| `top_p` | f32 | `1.0` | Nucleus sampling parameter |
| `frequency_penalty` | f32 | `0.0` | Penalize frequent tokens (-2 to 2) |
| `presence_penalty` | f32 | `0.0` | Penalize present tokens (-2 to 2) |

## Error Handling

```rust
use dashflow::core::error::Error as DashFlowError;
use dashflow_deepseek::ChatDeepSeek;

#[tokio::main]
async fn main() {
    let chat = ChatDeepSeek::new();

    match chat.generate(&messages, None).await {
        Ok(result) => {
            println!("Success: {}", result.generations[0].message.as_text());
        }
        Err(DashFlowError::ApiError(e)) => {
            eprintln!("API error: {}", e);
        }
        Err(DashFlowError::RateLimitExceeded) => {
            eprintln!("Rate limit exceeded, please retry");
        }
        Err(e) => {
            eprintln!("Other error: {}", e);
        }
    }
}
```

## Testing

The crate includes unit tests for core functionality:

```bash
# Run tests
cargo test -p dashflow-deepseek

# Run tests with output
cargo test -p dashflow-deepseek -- --nocapture
```

## API Compatibility

DeepSeek uses an OpenAI-compatible API, which means:
- Request/response formats match OpenAI's API
- Most OpenAI parameters are supported
- Tool calling follows OpenAI's function calling format
- Streaming uses the same protocol

## Documentation

- **API Reference** - Generate with `cargo doc --package dashflow-deepseek --open`
- **[Main Repository](../../README.md)** - Full project documentation
- **[DeepSeek API Docs](https://platform.deepseek.com/docs)** - Official DeepSeek documentation

## License

This crate is part of the DashFlow project.
