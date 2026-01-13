# Language Models

**Last Updated:** 2025-12-16 (Worker #792 - Fix broken links and GitHub URLs)

Language models in DashFlow provide interfaces to various LLM providers.

## ChatModel Trait

The `ChatModel` trait is the primary interface for conversational models:

```rust
use dashflow::core::language_models::ChatModel;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let llm = ChatOpenAI::default();
    let response = llm.invoke("Hello!").await?;
    println!("{}", response.content);
    Ok(())
}
```

## Supported Providers

- **OpenAI** (`dashflow-openai`) - GPT-3.5, GPT-4, GPT-4 Turbo, GPT-4o
- **Anthropic** (`dashflow-anthropic`) - Claude 3 (Opus, Sonnet, Haiku), Claude 3.5
- **Ollama** (`dashflow-ollama`) - Local models (Llama, Mistral, etc.)
- **Cohere** (`dashflow-cohere`) - Command R, Command R+
- **Google Gemini** (`dashflow-gemini`) - Gemini Pro, Gemini Ultra
- **AWS Bedrock** (`dashflow-bedrock`) - Multiple providers via AWS
- **Azure OpenAI** (`dashflow-azure-openai`) - Azure-hosted OpenAI models

## Features

- **Streaming**: Real-time token generation
- **Tool Calling**: Function calling support
- **Async**: Non-blocking I/O
- **Batch Processing**: Multiple requests in parallel

See [API Reference](../api/rustdoc.md) for complete documentation.
