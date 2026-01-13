# dashflow-cohere

Cohere integration for DashFlow - Command R+ chat models and embeddings.

## Usage

```rust
use dashflow::core::language_models::ChatModel;
use dashflow::core::messages::Message;
use dashflow_cohere::ChatCohere;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Set COHERE_API_KEY environment variable
    let chat = ChatCohere::new()
        .with_model("command-r-plus");

    let messages = vec![Message::human("Explain quantum computing")];
    let result = chat.generate(&messages, None).await?;
    println!("{}", result.generations[0].message.as_text());
    Ok(())
}
```

## Features

- **Chat Models**: Command R+, Command R, Command
- **Embeddings**: embed-english-v3.0, embed-multilingual-v3.0
- **RAG Support**: Native grounded generation with citations
- **Streaming**: Real-time responses

## Documentation

- **API Reference** - Generate with `cargo doc --package dashflow-cohere --open`
- **[Main Repository](../../README.md)** - Full project documentation

## Installation

Add to your `Cargo.toml`:
```toml
[dependencies]
dashflow-cohere = "1.11"
```
