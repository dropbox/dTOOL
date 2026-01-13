# Common Utilities for Example Apps

Shared utility library used by DashFlow example applications. Provides factory functions, quality judging utilities, and test tools.

## Overview

This crate centralizes common functionality used across multiple example apps:

- **Factory Re-exports**: Re-exports from `dashflow-factories` for LLM, embeddings, and tools creation
- **Quality Judging**: `QualityJudge` for evaluating LLM response quality
- **Test Tools**: Mock tools for testing (`VectorStoreSearchTool`, `WebSearchTool`)
- **App Builder**: `DashFlowAppBuilder` for streamlined app configuration

## Usage

```toml
[dependencies]
common = { path = "../common" }
```

```rust
use common::{create_llm, QualityJudge, DashFlowAppBuilder};

// Create an LLM using factory
let llm = create_llm("openai", "gpt-4o-mini", None).await?;

// Build a DashFlow app with common configuration
let app = DashFlowAppBuilder::new()
    .with_llm(llm)
    .build()?;
```

## Feature Flags

- `anthropic` - Enable Anthropic Claude models
- `bedrock` - Enable AWS Bedrock models
- `ollama` - Enable local Ollama models
- `duckduckgo` - Enable DuckDuckGo search tool
- `all-providers` - Enable all LLM providers
- `all-tools` - Enable all tools

## Note

For new code, consider using `dashflow-factories` directly instead of this shared crate. The factory re-exports are maintained for backward compatibility.

## See Also

- [Example Apps Overview](../../../docs/EXAMPLE_APPS.md)
- [dashflow-factories Crate](../../../crates/dashflow-factories/README.md)
