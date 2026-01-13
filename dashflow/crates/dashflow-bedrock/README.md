# dashflow-bedrock

AWS Bedrock integration for DashFlow.

This crate provides access to foundation models available through AWS Bedrock, including:
- **Anthropic Claude** (Sonnet 4.5, 3.5, 3 Opus, 3 Sonnet, 3 Haiku, 2.1, 2)
- **Meta Llama** (3.3, 3.2, 3.1)
- **Mistral AI** (Large, Small, Mixtral)
- **Cohere** (Command R, Command R+)
- **Amazon Titan** (Premier, Express, Lite)

## Features

- ✅ Standard AWS SDK authentication (credentials, SSO, IAM roles)
- ✅ Streaming support
- ✅ Tool/function calling
- ✅ Rate limiting
- ✅ Usage metadata tracking
- ✅ Response metadata

## Installation

Add to your `Cargo.toml`:
```toml
[dependencies]
dashflow-bedrock = "1.11"
```

## Quick Start

### 1. Configure AWS Authentication

```bash
# Option A: AWS SSO (recommended for development)
aws configure sso
aws sso login --profile claude
export AWS_PROFILE=claude

# Option B: Access keys (for CI/CD)
export AWS_ACCESS_KEY_ID=your_key
export AWS_SECRET_ACCESS_KEY=your_secret
export AWS_REGION=us-west-2
```

### 2. Basic Usage

```rust
use dashflow_bedrock::{ChatBedrock, models};
use dashflow::core::language_models::ChatModel;
use dashflow::core::messages::Message;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let bedrock = ChatBedrock::new("us-west-2")
        .await?
        .with_model(models::CLAUDE_SONNET_4_5);

    let messages = vec![Message::human("What is AWS Bedrock?")];
    let result = bedrock.generate(&messages, None, None, None, None).await?;

    println!("{}", result.generations[0].message.content());
    Ok(())
}
```

## Examples

Run with your AWS profile:

```bash
export AWS_PROFILE=claude

# Basic usage
cargo run --example bedrock_basic

# Streaming responses
cargo run --example bedrock_streaming

# Tool/function calling
cargo run --example bedrock_tools

# Claude Sonnet 4.5
cargo run --example bedrock_sonnet_4_5
```

## Available Models

```rust
use dashflow_bedrock::models;

// Claude models
models::CLAUDE_SONNET_4_5       // Latest Sonnet 4.5
models::CLAUDE_3_5_SONNET_V2    // Claude 3.5 Sonnet v2
models::CLAUDE_3_5_HAIKU        // Claude 3.5 Haiku
models::CLAUDE_3_OPUS           // Claude 3 Opus

// Other models
models::LLAMA_3_3_70B           // Meta Llama 3.3 70B
models::MISTRAL_LARGE_2407      // Mistral Large
models::COHERE_COMMAND_R_PLUS   // Cohere Command R+
```

## Documentation

- **[Examples](examples/)** - Working code examples
- **API Reference** - Generate with `cargo doc --package dashflow-bedrock --open`
- **[Main Repository](../../README.md)** - Full project documentation
