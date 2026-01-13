# dashflow-anthropic

Anthropic Claude integration for DashFlow - access Claude 3.5 Sonnet, Opus, and Haiku models with tool calling and prompt caching.

## Usage

```rust
use dashflow::core::language_models::ChatModel;
use dashflow::core::messages::Message;
use dashflow_anthropic::ChatAnthropic;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Set ANTHROPIC_API_KEY environment variable
    let chat = ChatAnthropic::new()
        .with_model("claude-3-5-haiku-20241022");

    let messages = vec![
        Message::system("You are a helpful assistant."),
        Message::human("What is the capital of France?")
    ];

    let result = chat.generate(&messages, None).await?;
    println!("{}", result.generations[0].message.as_text());
    Ok(())
}
```

## Features

- **Models**: Claude 3.5 Sonnet/Haiku, Claude 3 Opus/Sonnet/Haiku
- **Tool Calling**: Native function calling support
- **Prompt Caching**: 90% cost reduction on repeated context
- **Extended Thinking**: Reasoning tokens for complex tasks
- **Streaming**: Real-time token-by-token responses

## Documentation

- **API Reference** - Generate with `cargo doc --package dashflow-anthropic --open`
- **[Main Repository](../../README.md)** - Full project documentation

## Installation

Add to your `Cargo.toml`:
```toml
[dependencies]
dashflow-anthropic = "1.11"
```
