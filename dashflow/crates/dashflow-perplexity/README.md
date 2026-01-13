# dashflow-perplexity

Perplexity AI integration for DashFlow - access powerful search-enhanced models through an OpenAI-compatible API.

## Overview

Perplexity AI specializes in search-enhanced language models that combine real-time internet access with advanced reasoning capabilities. This crate provides a seamless wrapper around Perplexity's OpenAI-compatible API.

**Key Features:**
- Real-time web search integration
- Access to Sonar, Sonar-Pro, and Sonar-Reasoning models
- OpenAI-compatible API (drop-in replacement)
- Streaming support
- Tool calling capabilities
- Rate limiting support

## Installation

Add to your `Cargo.toml`:
```toml
[dependencies]
dashflow-perplexity = "1.11"
dashflow = "1.11"
tokio = { version = "1", features = ["full"] }
```

## Quick Start

```rust
use dashflow::core::language_models::ChatModel;
use dashflow::core::messages::Message;
use dashflow_perplexity::ChatPerplexity;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Set PPLX_API_KEY environment variable
    let chat = ChatPerplexity::new()
        .with_model("sonar")
        .with_temperature(0.7);

    let messages = vec![
        Message::system("You are a helpful assistant with real-time web search."),
        Message::human("What are the latest developments in quantum computing?")
    ];

    let result = chat.generate(&messages, None).await?;
    println!("{}", result.generations[0].message.as_text());
    Ok(())
}
```

## Configuration

### Environment Variables

```bash
# Required: Your Perplexity API key
export PPLX_API_KEY="pplx-your-api-key-here"

# Optional: Custom API endpoint (defaults to https://api.perplexity.ai)
export PPLX_API_BASE="https://custom-endpoint.com"
```

### Builder Pattern

```rust
use dashflow_perplexity::ChatPerplexity;

let chat = ChatPerplexity::with_api_key("your-api-key")
    .with_model("sonar-pro")
    .with_temperature(0.8)
    .with_max_tokens(2000)
    .with_top_p(0.95)
    .with_frequency_penalty(0.0)
    .with_presence_penalty(0.0);
```

## Available Models

### Sonar (Default)
```rust
use dashflow_perplexity::chat_models::models;

let chat = ChatPerplexity::new()
    .with_model(models::SONAR);
```

General-purpose model optimized for reasoning with real-time web search.

### Sonar-Pro
```rust
use dashflow_perplexity::chat_models::models;

let chat = ChatPerplexity::new()
    .with_model(models::SONAR_PRO);
```

Advanced model with enhanced capabilities and deeper reasoning.

### Sonar-Reasoning
```rust
use dashflow_perplexity::chat_models::models;

let chat = ChatPerplexity::new()
    .with_model(models::SONAR_REASONING);
```

Specialized model for complex reasoning tasks requiring multi-step thinking.

## Examples

### Basic Chat with Web Search

```rust
use dashflow::core::language_models::ChatModel;
use dashflow::core::messages::Message;
use dashflow_perplexity::ChatPerplexity;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let chat = ChatPerplexity::new();

    let messages = vec![
        Message::human("What's happening with AI regulation in 2025?")
    ];

    let result = chat.generate(&messages, None).await?;
    println!("{}", result.generations[0].message.as_text());
    Ok(())
}
```

### Streaming Responses

```rust
use dashflow::core::language_models::ChatModel;
use dashflow::core::messages::Message;
use dashflow_perplexity::ChatPerplexity;
use futures::StreamExt;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let chat = ChatPerplexity::new()
        .with_model("sonar-pro");

    let messages = vec![
        Message::human("Explain the latest SpaceX launch with real-time data.")
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

### Research Assistant with Citations

```rust
use dashflow::core::language_models::ChatModel;
use dashflow::core::messages::Message;
use dashflow_perplexity::ChatPerplexity;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let chat = ChatPerplexity::new()
        .with_model("sonar-reasoning")
        .with_temperature(0.3); // Lower temperature for factual accuracy

    let messages = vec![
        Message::system("You are a research assistant. Always cite your sources when providing information."),
        Message::human("What are the most recent peer-reviewed findings on mRNA vaccine efficacy?")
    ];

    let result = chat.generate(&messages, None).await?;
    println!("{}", result.generations[0].message.as_text());
    Ok(())
}
```

### Multi-Turn Conversation with Context

```rust
use dashflow::core::language_models::ChatModel;
use dashflow::core::messages::Message;
use dashflow_perplexity::ChatPerplexity;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let chat = ChatPerplexity::new();

    let messages = vec![
        Message::human("Who won the latest Nobel Prize in Physics?"),
    ];

    let result1 = chat.generate(&messages, None).await?;
    let answer1 = result1.generations[0].message.as_text();
    println!("Answer 1: {}", answer1);

    // Continue conversation with context
    let mut messages = vec![
        Message::human("Who won the latest Nobel Prize in Physics?"),
        Message::ai(answer1),
        Message::human("What was their key contribution?"),
    ];

    let result2 = chat.generate(&messages, None).await?;
    println!("Answer 2: {}", result2.generations[0].message.as_text());

    Ok(())
}
```

### Rate Limiting

```rust
use dashflow::core::rate_limiters::{SimpleRateLimiter, RateLimiter};
use dashflow_perplexity::ChatPerplexity;
use std::sync::Arc;

let rate_limiter = Arc::new(SimpleRateLimiter::new(
    10, // max requests per minute
    60, // window in seconds
)) as Arc<dyn RateLimiter>;

let chat = ChatPerplexity::new()
    .with_rate_limiter(rate_limiter);
```

## Advanced Usage

### Custom API Endpoint

```rust
use async_openai::config::OpenAIConfig;
use dashflow_openai::ChatOpenAI;
use dashflow_perplexity::ChatPerplexity;

let config = OpenAIConfig::new()
    .with_api_key("your-key")
    .with_api_base("https://custom.perplexity.com");

let inner = ChatOpenAI::with_config(config)
    .with_model("sonar");

let chat = ChatPerplexity { inner };
```

### Access to Inner ChatOpenAI

Since Perplexity wraps ChatOpenAI, you can access the inner implementation:

```rust
let mut chat = ChatPerplexity::new();

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
| `model` | String | `"sonar"` | Model name to use |
| `temperature` | f32 | `0.7` | Sampling temperature (0-2) |
| `max_tokens` | u32 | - | Maximum tokens to generate |
| `top_p` | f32 | `1.0` | Nucleus sampling parameter |
| `frequency_penalty` | f32 | `0.0` | Penalize frequent tokens (-2 to 2) |
| `presence_penalty` | f32 | `0.0` | Penalize present tokens (-2 to 2) |

## Error Handling

```rust
use dashflow::core::error::Error as DashFlowError;
use dashflow_perplexity::ChatPerplexity;

#[tokio::main]
async fn main() {
    let chat = ChatPerplexity::new();

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
cargo test -p dashflow-perplexity

# Run tests with output
cargo test -p dashflow-perplexity -- --nocapture
```

## API Compatibility

Perplexity uses an OpenAI-compatible API, which means:
- Request/response formats match OpenAI's API
- Most OpenAI parameters are supported
- Tool calling follows OpenAI's function calling format
- Streaming uses the same protocol

## Key Differences from Standard LLMs

**Real-Time Search:** Perplexity models have built-in web search capabilities, making them ideal for:
- Current events and news queries
- Fact-checking with citations
- Research requiring up-to-date information
- Questions about recent developments

**Use Cases:**
- News and current events analysis
- Academic research with citations
- Market research and competitive intelligence
- Technical documentation lookup
- Real-time data queries

## Documentation

- **API Reference** - Generate with `cargo doc --package dashflow-perplexity --open`
- **[Main Repository](../../README.md)** - Full project documentation
- **[Perplexity API Docs](https://docs.perplexity.ai/)** - Official Perplexity documentation

## License

This crate is part of the DashFlow project.
