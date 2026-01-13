//! @dashflow-module
//! @name config_loader
//! @category core
//! @status stable
//!
//! Configuration file loading and secret management
//!
//! This module provides functionality for loading DashFlow objects from
//! configuration files (YAML/JSON) with environment variable resolution
//! and secret handling.
//!
//! # Example
//!
//! ```yaml
//! # config.yaml
//! chat_model:
//!   type: openai
//!   model: gpt-4
//!   temperature: 0.7
//!   api_key:
//!     env: OPENAI_API_KEY
//! ```
//!
//! ```rust,ignore
//! use dashflow::core::config_loader::DashFlowConfig;
//!
//! // Load from YAML
//! let config_str = std::fs::read_to_string("config.yaml")?;
//! let config: DashFlowConfig = serde_yml::from_str(&config_str)?;
//! ```

pub mod env_vars;
pub mod provider_helpers;
mod secrets;
mod types;

pub use env_vars::{
    env_bool, env_duration_secs, env_f64, env_is_set, env_string, env_string_or_default, env_u64,
    env_usize, has_any_llm_api_key, has_api_key,
};
pub use secrets::{expand_env_vars, SecretReference};
pub use types::{
    ChainConfig, ChainStepConfig, ChatModelConfig, ChatModelConfigExt, DashFlowConfig,
    EmbeddingConfig, EmbeddingConfigExt, LLMNodeConfig, OptimizationConfig, PromptConfig,
    RerankerConfig, RerankerConfigExt, RetrieverConfig, SignatureConfig, ToolConfig, ToolConfigExt,
    VectorStoreConfig,
};
