# dashflow-xai

xAI Grok integration for DashFlow - access Grok models from X.AI through an OpenAI-compatible API.

## Overview

xAI provides the Grok family of models, designed with humor, personality, and up-to-date knowledge. This crate integrates Grok models into DashFlow through xAI's OpenAI-compatible API.

**Key Features:**
- Access to Grok-Beta and Grok-Vision-Beta models
- OpenAI-compatible API
- Streaming support for real-time responses
- Tool calling capabilities
- JSON mode and structured output
- Vision support (image understanding)
- Rate limiting support

## Installation

Add to your `Cargo.toml`:
```toml
[dependencies]
dashflow-xai = "1.11"
dashflow = "1.11"
tokio = { version = "1", features = ["full"] }
```

## Quick Start

```rust
use dashflow::core::language_models::ChatModel;
use dashflow::core::messages::Message;
use dashflow_xai::ChatXAI;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Set XAI_API_KEY environment variable
    let chat = ChatXAI::new()
        .with_model("grok-beta")
        .with_temperature(0.7);

    let messages = vec![
        Message::system("You are Grok, a witty AI assistant."),
        Message::human("Tell me a joke about programming.")
    ];

    let result = chat.generate(&messages, None).await?;
    println!("{}", result.generations[0].message.as_text());
    Ok(())
}
```

## Configuration

### Environment Variables

```bash
# Required: Your xAI API key
export XAI_API_KEY="xai-your-api-key-here"

# Optional: Custom API endpoint (defaults to https://api.x.ai/v1)
export XAI_API_BASE="https://custom-endpoint.com/v1"
```

### Builder Pattern

```rust
use dashflow_xai::ChatXAI;

let chat = ChatXAI::new()
    .with_model("grok-beta")
    .with_temperature(0.8)
    .with_max_tokens(2000)
    .with_top_p(0.95)
    .with_frequency_penalty(0.0)
    .with_presence_penalty(0.0);
```

## Available Models

### Grok-Beta (Default)
```rust
let chat = ChatXAI::new()
    .with_model("grok-beta");
```

General-purpose conversational model with personality and up-to-date knowledge.

### Grok-Vision-Beta
```rust
let chat = ChatXAI::new()
    .with_model("grok-vision-beta");
```

Vision-enabled model capable of understanding and analyzing images.

## Examples

### Basic Chat

```rust
use dashflow::core::language_models::ChatModel;
use dashflow::core::messages::Message;
use dashflow_xai::ChatXAI;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let chat = ChatXAI::new();

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
use dashflow_xai::ChatXAI;
use futures::StreamExt;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let chat = ChatXAI::new()
        .with_model("grok-beta");

    let messages = vec![
        Message::human("Write a short poem about AI.")
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

### Vision Understanding

```rust
use dashflow::core::language_models::ChatModel;
use dashflow::core::messages::{Message, MessageContent, ContentBlock, ImageSource};
use dashflow_xai::ChatXAI;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let chat = ChatXAI::new()
        .with_model("grok-vision-beta");

    let messages = vec![
        Message::Human {
            content: MessageContent::Blocks(vec![
                ContentBlock::Text {
                    text: "What's in this image?".to_string(),
                },
                ContentBlock::Image {
                    source: ImageSource::Url {
                        url: "https://example.com/image.jpg".to_string(),
                    },
                    detail: None,
                },
            ]),
            fields: Default::default(),
        }
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
use dashflow_xai::ChatXAI;
use serde_json::json;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let chat = ChatXAI::new();

    let tools = vec![
        ToolDefinition {
            name: "get_weather".to_string(),
            description: "Get the current weather for a location".to_string(),
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
        Message::human("What's the weather in New York?")
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

### JSON Mode

```rust
use dashflow::core::language_models::ChatModel;
use dashflow::core::messages::Message;
use dashflow_xai::ChatXAI;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let chat = ChatXAI::new()
        .with_json_mode();

    let messages = vec![
        Message::system("You are a helpful assistant that outputs JSON."),
        Message::human("List 3 programming languages as a JSON array"),
    ];

    let result = chat.generate(&messages, None).await?;
    // Response will be valid JSON, e.g., ["Rust", "Python", "JavaScript"]
    println!("{}", result.generations[0].message.as_text());
    Ok(())
}
```

### Structured Output

```rust
use dashflow::core::language_models::ChatModel;
use dashflow::core::messages::Message;
use dashflow_xai::ChatXAI;
use serde_json::json;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let schema = json!({
        "type": "object",
        "properties": {
            "name": {"type": "string"},
            "age": {"type": "number"},
            "email": {"type": "string", "format": "email"}
        },
        "required": ["name", "age"],
        "additionalProperties": false
    });

    let chat = ChatXAI::new()
        .with_structured_output(
            "user_info",
            schema,
            Some("User information extraction format".to_string()),
            true // strict mode
        );

    let messages = vec![
        Message::human("Extract user info: John Doe, 30 years old, john@example.com"),
    ];

    let result = chat.generate(&messages, None).await?;
    // Response will match the schema exactly
    println!("{}", result.generations[0].message.as_text());
    Ok(())
}
```

### Rate Limiting

```rust
use dashflow::core::rate_limiters::{InMemoryRateLimiter, RateLimiter};
use dashflow_xai::ChatXAI;
use std::sync::Arc;
use std::time::Duration;

let rate_limiter = Arc::new(InMemoryRateLimiter::new(
    10.0, // 10 requests per second
    Duration::from_millis(100),
    20.0, // max burst of 20 requests
)) as Arc<dyn RateLimiter>;

let chat = ChatXAI::new()
    .with_rate_limiter(rate_limiter);
```

## Advanced Usage

### Custom API Configuration

```rust
use async_openai::config::OpenAIConfig;
use dashflow_xai::ChatXAI;

let config = OpenAIConfig::new()
    .with_api_key("your-key")
    .with_api_base("https://api.x.ai/v1");

let chat = ChatXAI::with_config(config)
    .with_model("grok-beta");
```

### Retry Policy Configuration

```rust
use dashflow::core::retry::RetryPolicy;
use dashflow_xai::ChatXAI;

let chat = ChatXAI::new()
    .with_retry_policy(RetryPolicy::exponential(5)); // 5 retries with exponential backoff
```

### Image Detail Levels

```rust
use dashflow::core::messages::{ImageDetail, ImageSource, ContentBlock};

ContentBlock::Image {
    source: ImageSource::Url {
        url: "https://example.com/image.jpg".to_string(),
    },
    detail: Some(ImageDetail::High), // or Low, Auto
}
```

### Base64 Images

```rust
use dashflow::core::messages::{ImageSource, ContentBlock};

ContentBlock::Image {
    source: ImageSource::Base64 {
        media_type: "image/png".to_string(),
        data: "iVBORw0KGgoAAAANS...".to_string(),
    },
    detail: Some(ImageDetail::High),
}
```

## Parameters

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `model` | String | `"grok-beta"` | Model name to use |
| `temperature` | f32 | - | Sampling temperature (0-2) |
| `max_tokens` | u32 | - | Maximum tokens to generate |
| `top_p` | f32 | - | Nucleus sampling parameter |
| `frequency_penalty` | f32 | - | Penalize frequent tokens (-2 to 2) |
| `presence_penalty` | f32 | - | Penalize present tokens (-2 to 2) |

## Error Handling

```rust
use dashflow::core::error::Error as DashFlowError;
use dashflow_xai::ChatXAI;

#[tokio::main]
async fn main() {
    let chat = ChatXAI::new();

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

The crate includes comprehensive unit tests and standard conformance tests:

```bash
# Run all tests
cargo test -p dashflow-xai

# Run unit tests only
cargo test -p dashflow-xai --lib

# Run standard conformance tests
cargo test -p dashflow-xai standard_tests

# Run with output
cargo test -p dashflow-xai -- --nocapture
```

## API Compatibility

xAI uses an OpenAI-compatible API, which means:
- Request/response formats match OpenAI's API
- Most OpenAI parameters are supported
- Tool calling follows OpenAI's function calling format
- Streaming uses the same protocol
- Vision input follows multimodal message format

## Key Features of Grok

**Personality:** Grok has a unique personality with wit and humor, making interactions more engaging.

**Up-to-Date Knowledge:** Trained on recent data with awareness of current events.

**Vision Capabilities:** Grok-Vision-Beta can understand and analyze images, including:
- Image description and analysis
- Object detection and recognition
- Text extraction from images
- Visual question answering

**Use Cases:**
- Conversational AI with personality
- Content creation requiring creativity
- Image analysis and understanding
- Technical assistance with recent technologies
- Real-time information queries

## Standard Conformance

This crate passes all DashFlow standard tests for ChatModel implementations, including:
- Basic invoke and streaming
- Multi-turn conversations
- Tool calling with various configurations
- JSON mode and structured output
- Unicode and special character handling
- Error recovery and rate limiting
- Concurrent generation
- Long context handling

## Documentation

- **API Reference** - Generate with `cargo doc --package dashflow-xai --open`
- **[Main Repository](../../README.md)** - Full project documentation
- **[xAI API Docs](https://docs.x.ai/)** - Official xAI documentation

## License

This crate is part of the DashFlow project.
