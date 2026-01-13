// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

//! @dashflow-module
//! @name func
//! @category api
//! @status stable
//!
//! `DashFlow` Functional API
//!
//! This module provides the Functional API for `DashFlow`, which enables a more
//! concise and Pythonic way to define agents and workflows.
//!
//! # Overview
//!
//! The Functional API consists of two main components:
//!
//! 1. **`#[task]` macro** - Converts async functions into tasks that return `TaskHandle<T>`
//! 2. **`#[entrypoint]` macro** - Converts async functions into agents with `.invoke()` and `.stream()` methods
//!
//! # Example
//!
//! ```rust,ignore
//! use dashflow_macros::{task, entrypoint};
//! use dashflow::core::messages::Message;
//!
//! #[task]
//! async fn call_model(messages: Vec<Message>) -> Result<Message, String> {
//!     // Call LLM
//!     model.invoke(messages).await
//! }
//!
//! #[entrypoint]
//! async fn agent(messages: Vec<Message>) -> Result<Message, String> {
//!     let response = call_model(messages).await?;
//!     Ok(response)
//! }
//!
//! // Usage
//! let result = agent.invoke(initial_messages).await?;
//! ```

mod agent;
mod task_handle;

pub use agent::{Agent, StreamUpdate};
pub use task_handle::TaskHandle;
