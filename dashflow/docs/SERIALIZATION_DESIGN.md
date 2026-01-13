# Serialization Design for DashFlow

**Last Updated:** 2026-01-04 (Worker #2430 - Fix stale chat_models file paths)

## Overview

This document outlines the design for serializing and deserializing DashFlow objects to/from JSON and YAML formats, enabling configuration-driven chain construction and persistence.

## Goals

1. **Serialize common objects**: ChatModels, Embeddings, Retrievers, Tools, Prompts
2. **Support configuration files**: Load chains from JSON/YAML config files
3. **Secret handling**: Never serialize API keys, support environment variable references
4. **Version compatibility**: Add version tags for future migration support
5. **Practical focus**: Prioritize common use cases over complete generality

## Python Baseline Format

Python DashFlow uses this serialization format:

```json
{
  "lc": 1,
  "type": "constructor",
  "id": ["dashflow", "llms", "openai", "ChatOpenAI"],
  "kwargs": {
    "model_name": "gpt-4",
    "temperature": 0.7,
    "openai_api_key": {
      "lc": 1,
      "type": "secret",
      "id": ["OPENAI_API_KEY"]
    }
  }
}
```

**Key Fields:**
- `lc`: Serialization format version (currently 1)
- `type`: "constructor", "secret", or "not_implemented"
- `id`: Namespace path to class (e.g., ["dashflow_openai", "chat_models", "base", "ChatOpenAI"])
- `kwargs`: Constructor arguments
- Secrets are replaced with `{"lc": 1, "type": "secret", "id": ["ENV_VAR_NAME"]}`

## Rust Implementation Strategy

### Phase 1: Config-Based Construction (Practical Approach)

**Rather than full trait-based serialization**, we'll implement a **config-based approach**:

1. Define config structs for each serializable type
2. Implement `from_config()` constructors
3. Support JSON/YAML deserialization
4. Defer full `to_json()` serialization (less common use case)

**Rationale:**
- **Loading chains from config** is much more common than serializing existing chains
- Rust's type system makes generic serialization very complex (trait objects, generics)
- Config-based approach is more idiomatic in Rust (see tokio, actix-web patterns)
- Can add full serialization later if needed

### Example: ChatOpenAI Config

```yaml
# config.yaml
chat_model:
  type: openai
  model: gpt-4
  temperature: 0.7
  api_key: ${OPENAI_API_KEY}  # Environment variable reference

retriever:
  type: qdrant
  collection_name: documents
  url: http://localhost:6333
  embedding:
    type: openai
    model: text-embedding-3-small
    api_key: ${OPENAI_API_KEY}

chain:
  - type: retriever
    ref: retriever
  - type: prompt_template
    template: "Context: {context}\n\nQuestion: {question}\n\nAnswer:"
  - type: chat_model
    ref: chat_model
```

### Rust Implementation

```rust
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// Top-level config
#[derive(Debug, Serialize, Deserialize)]
pub struct DashFlowConfig {
    #[serde(default)]
    pub chat_models: HashMap<String, ChatModelConfig>,

    #[serde(default)]
    pub retrievers: HashMap<String, RetrieverConfig>,

    #[serde(default)]
    pub embeddings: HashMap<String, EmbeddingConfig>,

    #[serde(default)]
    pub chains: HashMap<String, ChainConfig>,
}

// ChatModel config (enum for different providers)
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum ChatModelConfig {
    OpenAI {
        #[serde(default = "default_gpt_model")]
        model: String,
        #[serde(default)]
        temperature: f32,
        api_key: SecretReference,
        #[serde(default)]
        base_url: Option<String>,
    },
    Anthropic {
        #[serde(default = "default_claude_model")]
        model: String,
        api_key: SecretReference,
    },
    Ollama {
        #[serde(default = "default_ollama_model")]
        model: String,
        #[serde(default = "default_ollama_base_url")]
        base_url: String,
    },
    // ... other providers
}

// Secret reference (environment variable or inline - never committed)
#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum SecretReference {
    EnvVar { env: String },          // { "env": "OPENAI_API_KEY" }
    Inline(String),                   // Direct string (for testing only)
}

impl SecretReference {
    pub fn resolve(&self) -> Result<String> {
        match self {
            SecretReference::EnvVar { env } => {
                std::env::var(env)
                    .map_err(|_| Error::ConfigError(format!("Environment variable {} not set", env)))
            }
            SecretReference::Inline(s) => Ok(s.clone()),
        }
    }
}

// Builder pattern for constructing from config
impl ChatModelConfig {
    pub fn build(&self) -> Result<Box<dyn ChatModel>> {
        match self {
            ChatModelConfig::OpenAI { model, temperature, api_key, base_url } => {
                let api_key = api_key.resolve()?;
                let mut builder = ChatOpenAI::builder()
                    .model(model)
                    .temperature(*temperature)
                    .api_key(&api_key);

                if let Some(url) = base_url {
                    builder = builder.base_url(url);
                }

                Ok(Box::new(builder.build()?))
            }
            ChatModelConfig::Anthropic { model, api_key } => {
                let api_key = api_key.resolve()?;
                Ok(Box::new(
                    ChatAnthropic::builder()
                        .model(model)
                        .api_key(&api_key)
                        .build()?
                ))
            }
            // ... other providers
        }
    }
}
```

### Usage

```rust
use dashflow::core::config::DashFlowConfig;

// Load from YAML file
let config_str = std::fs::read_to_string("config.yaml")?;
let config: DashFlowConfig = serde_yaml::from_str(&config_str)?;

// Build chat model from config
let chat_model = config.chat_models.get("default")
    .ok_or(Error::ConfigError("No default chat model".into()))?
    .build()?;

// Use the chat model
let response = chat_model.invoke(messages, None).await?;
```

### Environment Variable Expansion

Support `${VAR}` syntax in YAML:

```rust
pub fn expand_env_vars(input: &str) -> String {
    let re = regex::Regex::new(r"\$\{([A-Z_][A-Z0-9_]*)\}").unwrap();
    re.replace_all(input, |caps: &regex::Captures| {
        std::env::var(&caps[1]).unwrap_or_else(|_| caps[0].to_string())
    }).to_string()
}
```

## Phase 2: Full Serialization (Future)

If needed later, implement full `to_json()` for objects:

```rust
pub trait Serializable {
    fn lc_serializable() -> bool { false }
    fn lc_namespace() -> Vec<String>;
    fn to_json(&self) -> Result<serde_json::Value>;
    fn lc_secrets(&self) -> HashMap<String, String> { HashMap::new() }
}
```

However, this is **deferred** because:
- Less common use case (most users load from config, not save to config)
- Complex with Rust's type system (trait objects, generics)
- Can be added incrementally without breaking changes

## Implementation Plan

### Commit #107: Config Types and Secret Handling

**Files to create:**
- `crates/dashflow/src/core/config_loader.rs` - Config types and loading
- `crates/dashflow/src/core/config_loader/types.rs` - Config struct definitions
- `crates/dashflow/src/core/config_loader/secrets.rs` - Secret resolution
- Add `serde_yaml` dependency to `dashflow/Cargo.toml`

**Implement:**
1. `DashFlowConfig` top-level struct
2. `SecretReference` enum with env var resolution
3. Environment variable expansion (`${VAR}` syntax)
4. Basic YAML loading

**Tests:**
- Load config from YAML
- Resolve environment variables
- Expand `${VAR}` syntax
- Error handling for missing env vars

### Commit #108: ChatModel Config - Builder Methods ✅ COMPLETE

**Architecture Decision:**
Due to circular dependency constraints (dashflow cannot depend on provider crates),
we implemented `from_config()` methods in each provider crate instead of a central `build()`
method in dashflow. This is more idiomatic Rust and avoids dependency issues.

**Files modified:**
- `crates/dashflow/src/core/config_loader/types.rs` - Added helper methods (model(), provider())
- `crates/dashflow-openai/src/chat_models/mod.rs` - Added ChatOpenAI::from_config()
- `crates/dashflow-anthropic/src/chat_models/mod.rs` - Added ChatAnthropic::from_config()
- `crates/dashflow-ollama/src/chat_models.rs` - Added ChatOllama::from_config()
- `crates/dashflow-groq/src/chat_models.rs` - Added ChatGroq::from_config()
- `crates/dashflow-mistral/src/chat_models.rs` - Added ChatMistralAI::from_config()
- `crates/dashflow-deepseek/src/chat_models.rs` - Added ChatDeepSeek::from_config()
- `crates/dashflow-fireworks/src/chat_models.rs` - Added ChatFireworks::from_config()
- `crates/dashflow-xai/src/chat_models.rs` - Added ChatXAI::from_config()
- `crates/dashflow-perplexity/src/chat_models.rs` - Added ChatPerplexity::from_config()
- `crates/dashflow-huggingface/src/chat_models.rs` - Added ChatHuggingFace::from_config()

**Implementation:**
1. ✅ `ChatModelConfig` enum for all 10 providers (already in commit #107)
2. ✅ `from_config()` method for each provider (in provider crates)
3. ✅ Default values for common fields
4. ✅ Helper methods: model(), provider() in ChatModelConfig

**Usage Pattern:**
```rust
use dashflow::core::config_loader::DashFlowConfig;
use dashflow_openai::ChatOpenAI;

let config = DashFlowConfig::from_yaml(&yaml_str)?;
let model = ChatOpenAI::from_config(
    config.get_chat_model("openai").unwrap()
)?;
```

**Tests:**
- ✅ 11 tests in config_from_config.rs
- ✅ All providers verified
- ✅ Config parsing and helper methods tested

### Commit #109: Embedding and Retriever Config

**Files to modify:**
- `crates/dashflow/src/core/config_loader/types.rs`

**Implement:**
1. `EmbeddingConfig` enum
2. `RetrieverConfig` enum
3. `VectorStoreConfig` enum (Chroma, Qdrant)
4. Build methods for each

**Tests:**
- Build embeddings from config
- Build retrievers from config
- Nested config (retriever with embedding)

### Commit #110: Chain Config (N mod 5 - Cleanup) ✅ COMPLETE

**Files modified:**
- `crates/dashflow/src/core/config_loader/types.rs` - Added ChainConfig, PromptConfig, ChainStepConfig
- `crates/dashflow/src/core/config_loader/mod.rs` - Exported new types
- `crates/dashflow/examples/chain_configuration.rs` - Comprehensive chain config example
- `examples/config.yaml` - Added prompts and chains sections

**Implemented:**
1. ✅ `PromptConfig` enum (Simple and Structured variants)
2. ✅ `ChainConfig` for composing runnables
3. ✅ `ChainStepConfig` enum with 7 step types:
   - ChatModel, Retriever, Prompt (references)
   - PromptTemplate (inline)
   - Lambda, Passthrough, Custom
4. ✅ Reference resolution via `ref:` field
5. ✅ Inline vs referenced components supported
6. ✅ Helper methods: template(), input_variables(), reference(), step_type()

**Tests:**
- ✅ 8 new tests in config_loader::types::tests
- ✅ test_parse_prompt_config_simple
- ✅ test_parse_prompt_config_structured
- ✅ test_parse_chain_config
- ✅ test_parse_chain_with_inline_prompt
- ✅ test_chain_step_reference
- ✅ test_prompt_config_methods
- ✅ test_complete_config_with_chains

## Future Enhancements

### Python Compatibility Mode

If needed, add compatibility with Python serialization format:

```rust
#[derive(Debug, Serialize, Deserialize)]
pub struct DashFlowPythonFormat {
    lc: u8,  // Version
    #[serde(rename = "type")]
    type_: String,  // "constructor" | "secret" | "not_implemented"
    id: Vec<String>,  // ["dashflow", "llms", "openai", "ChatOpenAI"]
    #[serde(skip_serializing_if = "Option::is_none")]
    kwargs: Option<HashMap<String, serde_json::Value>>,
}
```

### Migration Utilities

Add version tracking and migration:

```rust
#[derive(Debug, Serialize, Deserialize)]
pub struct VersionedConfig {
    version: String,  // "1.0.0"
    #[serde(flatten)]
    config: DashFlowConfig,
}

pub fn migrate(from_version: &str, to_version: &str, config: serde_json::Value) -> Result<DashFlowConfig> {
    // Migration logic
}
```

## Security Considerations

1. **Never commit secrets**: Validate config files don't contain inline secrets
2. **Environment variables**: Recommend `.env` files (excluded from git)
3. **Audit logging**: Log when secrets are resolved
4. **Validation**: Reject configs with suspicious patterns

## Testing Strategy

1. **Unit tests**: Config parsing, secret resolution, env var expansion
2. **Integration tests**: Build actual objects from config, verify they work
3. **Example configs**: Provide templates for common use cases
4. **Error handling**: Test missing fields, invalid types, missing env vars

## Documentation

Create docs/CONFIG_FILE_REFERENCE.md with:
- Complete config file format
- All available options for each type
- Secret handling best practices
- Example configurations for common use cases

## Success Criteria

- ✅ Load ChatModels from YAML config
- ✅ Load Retrievers from YAML config
- ✅ Load Embeddings from YAML config
- ✅ Environment variable resolution
- ✅ Secret handling (never serialize secrets)
- ✅ Comprehensive tests
- ✅ Documentation and examples

## Non-Goals (Deferred)

- ❌ Full `to_json()` serialization (less common, can add later)
- ❌ Binary serialization formats
- ❌ Chain migration utilities (add when needed)
- ❌ Python format compatibility (add if interop needed)

## Timeline

- Commit #107: Config types and secrets (~12 min AI work)
- Commit #108: ChatModel config (~12 min AI work)
- Commit #109: Embedding/Retriever config (~12 min AI work)
- Commit #110: Chain config + N mod 5 cleanup (~12 min AI work)

**Total: 4 commits, ~48 minutes AI work**

This covers the essential serialization requirements while being practical and maintainable in Rust.
