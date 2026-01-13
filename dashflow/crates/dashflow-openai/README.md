# dashflow-openai

OpenAI integration for DashFlow - GPT-4, GPT-3.5, and OpenAI embeddings.

## Usage

### Recommended: Config-Driven Instantiation

```rust
use dashflow::core::language_models::ChatModel;
use dashflow::core::messages::Message;
use dashflow_openai::build_chat_model;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Set OPENAI_API_KEY environment variable
    // Uses YAML/JSON config for production flexibility
    let yaml = r#"
        type: openai
        model: gpt-4o
        api_key: { env: OPENAI_API_KEY }
    "#;
    let config: dashflow::core::config_loader::ChatModelConfig = serde_yaml::from_str(yaml)?;
    let chat = build_chat_model(&config)?;

    let messages = vec![Message::human("Hello, GPT-4!")];
    let result = chat.generate(&messages, None, None, None, None).await?;
    println!("{}", result.generations[0].message.as_text());
    Ok(())
}
```

### Provider-Agnostic Alternative

For applications that need to work across multiple providers, use the config-driven approach:

```rust
use dashflow::core::config_loader::build_chat_model;
use dashflow::core::config::LLMConfig;

let config = LLMConfig::from_env()?;  // Reads OPENAI_API_KEY, etc.
let llm = build_chat_model(&config).await?;
```

## Features

- **Chat Models**: GPT-4, GPT-4 Turbo, GPT-3.5 Turbo, O1 series
- **Embeddings**: text-embedding-3-small/large, ada-002
- **Tool Calling**: Function calling with JSON mode
- **Streaming**: Real-time responses
- **Vision**: Image understanding with GPT-4 Vision

## Documentation

- **API Reference** - Generate with `cargo doc --package dashflow-openai --open`
- **[Main Repository](../../README.md)** - Full project documentation

## Installation

Add to your `Cargo.toml`:
```toml
[dependencies]
dashflow-openai = "1.11"
```
