//! `ClickUp` tools for `DashFlow` Rust
//!
//! This crate provides tools for interacting with the `ClickUp` API v2.
//! It allows agents to create tasks, lists, folders, and query information
//! from a `ClickUp` workspace.
//!
//! # Authentication
//!
//! The `ClickUp` API requires an access token obtained via OAuth 2.0.
//! Set the `CLICKUP_ACCESS_TOKEN` environment variable with your token.
//!
//! # Example
//!
//! ```no_run
//! use dashflow_clickup::{ClickupAPIWrapper, ClickupAction};
//! use std::env;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! env::set_var("CLICKUP_ACCESS_TOKEN", "your_token_here");
//!
//! let api = ClickupAPIWrapper::new().await?;
//! let tool = ClickupAction::new(api, "get_teams");
//!
//! let result = tool.run("{}").await?;
//! println!("{}", result);
//! # Ok(())
//! # }
//! ```

mod api;
mod prompts;
mod tool;

pub use api::ClickupAPIWrapper;
pub use tool::ClickupAction;
