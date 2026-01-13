#!/usr/bin/env python3
# © 2025 Dropbox (created by Andrew Yates <ayates@dropbox.com>)
"""
Batch update all crate READMEs to minimal DRY format.
"""

import os
import re
from pathlib import Path

# Define minimal templates based on crate type
TEMPLATES = {
    "llm": """# {crate_name}

{description}

## Usage

```rust
use dashflow::language_models::ChatModel;
use dashflow::messages::Message;
use {crate_import}::{class_name};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {{
    // Set {env_var} environment variable
    let chat = {class_name}::new(){model_code};

    let messages = vec![Message::human("{example_prompt}")];
    let result = chat.generate(&messages, None).await?;
    println!("{{}}", result.generations[0].message.as_text());
    Ok(())
}}
```

## Features

{features}

## Documentation

- **[Golden Path Guide](../../docs/GOLDEN_PATH.md)** - Recommended API patterns
- **[API Reference](https://docs.rs/{crate_name})** - Complete API documentation
- **[Main Repository](../../README.md)** - Full project documentation

## Installation

Add to your `Cargo.toml`:
```toml
[dependencies]
{crate_name} = "1.6"
```
""",

    "vectorstore": """# {crate_name}

{description}

## Usage

```rust
use dashflow::vector_stores::VectorStore;
use {crate_import}::{class_name};
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {{
    {setup_code}

    // Add documents
    let texts = ["Document 1", "Document 2"];
    let ids = store.add_texts(&texts, None, None).await?;

    // Search
    let results = store.similarity_search("query", 5, None).await?;
    for doc in results {{
        println!("Found: {{}}", doc.page_content);
    }}
    Ok(())
}}
```

## Features

{features}

## Documentation

- **[Golden Path Guide](../../docs/GOLDEN_PATH.md)** - Recommended API patterns
- **[API Reference](https://docs.rs/{crate_name})** - Complete API documentation
- **[Main Repository](../../README.md)** - Full project documentation

## Installation

Add to your `Cargo.toml`:
```toml
[dependencies]
{crate_name} = "1.6"
```
""",

    "tool": """# {crate_name}

{description}

## Usage

```rust
{example_code}
```

## Documentation

- **[Golden Path Guide](../../docs/GOLDEN_PATH.md)** - Recommended API patterns
- **[API Reference](https://docs.rs/{crate_name})** - Complete API documentation
- **[Main Repository](../../README.md)** - Full project documentation

## Installation

Add to your `Cargo.toml`:
```toml
[dependencies]
{crate_name} = "1.6"
```
""",

    "search": """# {crate_name}

{description}

## Usage

```rust
use {crate_import}::{class_name};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {{
    let tool = {class_name}::new(){api_key_code};

    let results = tool.search("{example_query}").await?;
    for result in results {{
        println!("{{}}: {{}}", result.title, result.url);
    }}
    Ok(())
}}
```

## Documentation

- **[Golden Path Guide](../../docs/GOLDEN_PATH.md)** - Recommended API patterns
- **[API Reference](https://docs.rs/{crate_name})** - Complete API documentation
- **[Main Repository](../../README.md)** - Full project documentation

## Installation

Add to your `Cargo.toml`:
```toml
[dependencies]
{crate_name} = "1.6"
```
""",

    "utility": """# {crate_name}

{description}

## Documentation

- **[Golden Path Guide](../../docs/GOLDEN_PATH.md)** - Recommended API patterns
- **[API Reference](https://docs.rs/{crate_name})** - Complete API documentation
- **[Main Repository](../../README.md)** - Full project documentation

## Installation

Add to your `Cargo.toml`:
```toml
[dependencies]
{crate_name} = "1.6"
```
"""
}

# Define crate configurations
CRATE_CONFIGS = {
    # LLM Providers (already done: anthropic, openai, cohere)
    "dashflow-ollama": {
        "type": "llm",
        "description": "Ollama integration for DashFlow Rust - run Llama, Mistral, and other local models.",
        "class_name": "ChatOllama",
        "model_code": '\n        .with_model("llama2")\n        .with_base_url("http://localhost:11434")',
        "env_var": "None (runs locally)",
        "example_prompt": "Why is the sky blue?",
        "features": "- **Local Models**: Llama 2/3, Mistral, CodeLlama, Phi, Gemma\n- **No API Key**: Runs entirely locally\n- **Streaming**: Real-time token generation\n- **Embeddings**: Local embedding models"
    },
    "dashflow-groq": {
        "type": "llm",
        "description": "Groq integration for DashFlow Rust - ultra-fast LLM inference with Llama, Mixtral, and Gemma.",
        "class_name": "ChatGroq",
        "model_code": '\n        .with_model("mixtral-8x7b-32768")',
        "env_var": "GROQ_API_KEY",
        "example_prompt": "Explain photosynthesis",
        "features": "- **Ultra-Fast**: 500+ tokens/second inference speed\n- **Models**: Llama 3, Mixtral 8x7B, Gemma\n- **Large Context**: Up to 32K tokens\n- **Tool Calling**: Function calling support"
    },
    "dashflow-mistral": {
        "type": "llm",
        "description": "Mistral AI integration for DashFlow Rust - access Mistral models.",
        "class_name": "ChatMistral",
        "model_code": '\n        .with_model("mistral-large-latest")',
        "env_var": "MISTRAL_API_KEY",
        "example_prompt": "What is machine learning?",
        "features": "- **Models**: Mistral Large, Mistral Medium, Mistral Small\n- **Tool Calling**: Function calling support\n- **Streaming**: Real-time responses"
    },
    "dashflow-deepseek": {
        "type": "llm",
        "description": "DeepSeek integration for DashFlow Rust - reasoning-capable models.",
        "class_name": "ChatDeepSeek",
        "model_code": '\n        .with_model("deepseek-chat")',
        "env_var": "DEEPSEEK_API_KEY",
        "example_prompt": "Solve this problem",
        "features": "- **Reasoning**: Advanced reasoning capabilities\n- **Cost-Effective**: Affordable pricing\n- **Streaming**: Real-time responses"
    },
    "dashflow-huggingface": {
        "type": "llm",
        "description": "HuggingFace integration for DashFlow Rust - access 1000+ models.",
        "class_name": "HuggingFaceHub",
        "model_code": '\n        .with_repo_id("gpt2")',
        "env_var": "HUGGINGFACEHUB_API_TOKEN",
        "example_prompt": "Complete this sentence",
        "features": "- **1000+ Models**: Access entire HuggingFace model hub\n- **Inference API**: Fast serverless inference\n- **Text Generation**: Various model architectures"
    },
    "dashflow-fireworks": {
        "type": "llm",
        "description": "Fireworks AI integration for DashFlow Rust - fast inference for Llama and other models.",
        "class_name": "ChatFireworks",
        "model_code": '\n        .with_model("accounts/fireworks/models/llama-v3-70b-instruct")',
        "env_var": "FIREWORKS_API_KEY",
        "example_prompt": "Explain AI",
        "features": "- **Fast Inference**: Optimized for speed\n- **Popular Models**: Llama, Mistral, and more\n- **Tool Calling**: Function calling support"
    },
    "dashflow-perplexity": {
        "type": "llm",
        "description": "Perplexity AI integration for DashFlow Rust - search-augmented LLM.",
        "class_name": "ChatPerplexity",
        "model_code": '\n        .with_model("llama-3.1-sonar-large-128k-online")',
        "env_var": "PERPLEXITY_API_KEY",
        "example_prompt": "What's happening in AI today?",
        "features": "- **Search-Augmented**: Real-time web search integration\n- **Up-to-Date**: Access current information\n- **Large Context**: 128K token windows"
    },
    "dashflow-xai": {
        "type": "llm",
        "description": "xAI (X.AI) integration for DashFlow Rust - Grok models.",
        "class_name": "ChatXAI",
        "model_code": '\n        .with_model("grok-beta")',
        "env_var": "XAI_API_KEY",
        "example_prompt": "Tell me about space",
        "features": "- **Grok Models**: X.AI's conversational models\n- **Real-Time**: Access to real-time X platform data\n- **Streaming**: Real-time responses"
    },
    "dashflow-replicate": {
        "type": "llm",
        "description": "Replicate integration for DashFlow Rust - run ML models in the cloud.",
        "class_name": "Replicate",
        "model_code": '\n        .with_model("meta/llama-2-70b-chat")',
        "env_var": "REPLICATE_API_TOKEN",
        "example_prompt": "Generate a story",
        "features": "- **Cloud Inference**: No infrastructure management\n- **Many Models**: Llama, Stable Diffusion, and more\n- **Flexible**: Text, image, audio generation"
    },
    "dashflow-together": {
        "type": "llm",
        "description": "Together AI integration for DashFlow Rust - fast inference for open-source models.",
        "class_name": "ChatTogether",
        "model_code": '\n        .with_model("mistralai/Mixtral-8x7B-Instruct-v0.1")',
        "env_var": "TOGETHER_API_KEY",
        "example_prompt": "Explain quantum physics",
        "features": "- **Fast Inference**: Optimized for speed\n- **Open Source**: Popular open models\n- **Tool Calling**: Function calling support"
    },
    "dashflow-nomic": {
        "type": "utility",
        "description": "Nomic embeddings for DashFlow Rust - high-quality text embeddings."
    },
}

def update_readme(crate_path: Path, config: dict):
    """Update a single README file."""
    crate_name = crate_path.name
    readme_path = crate_path / "README.md"

    if not readme_path.exists():
        print(f"  ⚠️  README not found: {crate_name}")
        return False

    template_type = config.get("type", "utility")
    template = TEMPLATES[template_type]

    # Prepare template variables
    vars = {
        "crate_name": crate_name,
        "crate_import": crate_name.replace("-", "_"),
        "description": config.get("description", f"{crate_name} integration for DashFlow Rust."),
        **config
    }

    # Generate content
    try:
        content = template.format(**vars)
        readme_path.write_text(content)
        return True
    except KeyError as e:
        print(f"  ❌ Missing template variable for {crate_name}: {e}")
        return False

def main():
    crates_dir = Path(__file__).parent.parent / "crates"

    print("Updating crate READMEs...")
    updated = 0
    skipped = 0

    for crate_name, config in CRATE_CONFIGS.items():
        crate_path = crates_dir / crate_name
        if crate_path.exists():
            if update_readme(crate_path, config):
                print(f"  ✓ {crate_name}")
                updated += 1
            else:
                skipped += 1
        else:
            print(f"  ⚠️  Crate not found: {crate_name}")
            skipped += 1

    print(f"\nUpdated: {updated}")
    print(f"Skipped: {skipped}")

if __name__ == "__main__":
    main()
