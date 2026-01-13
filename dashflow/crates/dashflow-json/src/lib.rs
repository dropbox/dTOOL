//! JSON tools for `DashFlow` Rust
//!
//! This crate provides tools for navigating and querying JSON data structures,
//! enabling agents to explore JSON documents by listing keys and retrieving values.
//!
//! # Overview
//!
//! The crate provides three main components:
//!
//! - [`JsonSpec`] - Core JSON specification that holds the data and provides navigation methods
//! - [`JsonListKeysTool`] - Tool for listing keys at a given path
//! - [`JsonGetValueTool`] - Tool for retrieving values at a given path
//! - [`JsonToolkit`] - Toolkit that bundles the JSON tools together
//!
//! # Example
//!
//! ```rust
//! use dashflow_json::{JsonSpec, JsonToolkit};
//! use serde_json::json;
//! use dashflow::core::tools::BaseToolkit;
//!
//! # #[tokio::main]
//! # async fn main() {
//! // Create a JSON spec from data
//! let data = json!({
//!     "users": [
//!         {"name": "Alice", "age": 30},
//!         {"name": "Bob", "age": 25}
//!     ],
//!     "metadata": {
//!         "version": "1.0",
//!         "created": "2024-01-01"
//!     }
//! });
//!
//! let spec = JsonSpec::new(data);
//!
//! // List keys at root
//! let keys = spec.keys("data");
//! // Returns: "['users', 'metadata']"
//!
//! // Get a value
//! let value = spec.value("data[\"users\"][0][\"name\"]");
//! // Returns: "Alice"
//!
//! // Create a toolkit for agent use
//! let toolkit = JsonToolkit::new(spec);
//! let tools = toolkit.get_tools();
//! // Returns: [JsonListKeysTool, JsonGetValueTool]
//! # }
//! ```

mod spec;
mod toolkit;
mod tools;

pub use spec::JsonSpec;
pub use toolkit::JsonToolkit;
pub use tools::{JsonGetValueTool, JsonListKeysTool};
