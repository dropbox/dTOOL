//! @dashflow-module
//! @name prompts
//! @category core
//! @status stable
//!
//! Prompt templates for language models
//!
//! This module provides composable prompt templates for building
//! structured prompts for LLMs and chat models. Templates support
//! variable substitution using multiple formats (`FString`, Jinja2, Mustache).
//!
//! # Overview
//!
//! - [`PromptTemplate`] - Simple string templates with variable substitution
//! - [`ChatPromptTemplate`] - Chat templates with multiple message roles
//! - [`MessagesPlaceholder`] - Placeholder for dynamic message lists
//! - [`PromptTemplateFormat`] - Template format (`FString`, Jinja2, Mustache)
//!
//! # Examples
//!
//! ## Simple String Template
//!
//! ```rust
//! use dashflow::core::prompts::{PromptTemplate, PromptTemplateFormat};
//! use std::collections::HashMap;
//!
//! let template = PromptTemplate::new(
//!     "Tell me a joke about {topic}",
//!     vec!["topic".to_string()],
//!     PromptTemplateFormat::FString,
//! );
//!
//! let mut values = HashMap::new();
//! values.insert("topic".to_string(), "rust".to_string());
//!
//! let result = template.format(&values).unwrap();
//! assert!(result.contains("rust"));
//! ```
//!
//! ## Chat Template
//!
//! ```rust
//! use dashflow::core::prompts::ChatPromptTemplate;
//! use std::collections::HashMap;
//!
//! let template = ChatPromptTemplate::from_messages(vec![
//!     ("system", "You are a helpful assistant."),
//!     ("human", "Tell me about {topic}"),
//! ]).unwrap();
//!
//! let mut values = HashMap::new();
//! values.insert("topic".to_string(), "Rust programming".to_string());
//!
//! let messages = template.format_messages(&values).unwrap();
//! assert_eq!(messages.len(), 2);
//! ```

pub mod base;
pub mod chat;
pub mod example_selector;
pub mod string;

pub use base::{BasePromptTemplate, PromptTemplateFormat};
pub use chat::{ChatPromptTemplate, MessagesPlaceholder};
pub use string::PromptTemplate;
