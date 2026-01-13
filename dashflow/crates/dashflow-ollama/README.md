# dashflow-ollama

Ollama integration for DashFlow - run Llama, Mistral, and other local models.

## Usage

```rust
use dashflow::core::language_models::ChatModel;
use dashflow::core::messages::Message;
use dashflow_ollama::ChatOllama;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let chat = ChatOllama::new()
        .with_model("llama2")
        .with_base_url("http://localhost:11434");

    let messages = vec![Message::human("Why is the sky blue?")];
    let result = chat.generate(&messages, None).await?;
    println!("{}", result.generations[0].message.as_text());
    Ok(())
}
```

## Features

- **Local Models**: Llama 2/3, Mistral, CodeLlama, Phi, Gemma
- **No API Key**: Runs entirely locally
- **Streaming**: Real-time token generation
- **Embeddings**: Local embedding models

## Documentation

- **API Reference** - Generate with `cargo doc --package dashflow-ollama --open`
- **[Main Repository](../../README.md)** - Full project documentation

## Installation

Add to your `Cargo.toml`:
```toml
[dependencies]
dashflow-ollama = "1.11"
```
