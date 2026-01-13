# dashflow-groq

Groq integration for DashFlow - ultra-fast LLM inference with Llama, Mixtral, and Gemma.

## Usage

```rust
use dashflow::core::language_models::ChatModel;
use dashflow::core::messages::Message;
use dashflow_groq::ChatGroq;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Set GROQ_API_KEY environment variable
    let chat = ChatGroq::new()
        .with_model("mixtral-8x7b-32768");

    let messages = vec![Message::human("Explain photosynthesis")];
    let result = chat.generate(&messages, None).await?;
    println!("{}", result.generations[0].message.as_text());
    Ok(())
}
```

## Features

- **Ultra-Fast**: 500+ tokens/second inference speed
- **Models**: Llama 3, Mixtral 8x7B, Gemma
- **Large Context**: Up to 32K tokens
- **Tool Calling**: Function calling support

## Documentation

- **API Reference** - Generate with `cargo doc --package dashflow-groq --open`
- **[Main Repository](../../README.md)** - Full project documentation

## Installation

Add to your `Cargo.toml`:
```toml
[dependencies]
dashflow-groq = "1.11"
```
