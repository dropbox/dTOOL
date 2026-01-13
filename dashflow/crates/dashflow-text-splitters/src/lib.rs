//! Text Splitters for `DashFlow` Rust
//!
//! This crate provides utilities for splitting text into chunks for RAG pipelines.
//! Text splitters are essential for processing large documents that exceed LLM context windows.
//!
//! # Available Splitters
//!
//! - [`CharacterTextSplitter`]: Split on a single separator (e.g., "\n\n")
//! - [`RecursiveCharacterTextSplitter`]: Recursively split on multiple separators
//! - [`MarkdownTextSplitter`]: Specialized splitter for Markdown documents
//! - [`HTMLTextSplitter`]: Specialized splitter for HTML documents
//! - [`MarkdownHeaderTextSplitter`]: Split Markdown with header metadata extraction
//!
//! # Example
//!
//! ```
//! use dashflow_text_splitters::{TextSplitter, CharacterTextSplitter};
//!
//! let splitter = CharacterTextSplitter::new()
//!     .with_chunk_size(100)
//!     .with_chunk_overlap(20);
//!
//! let text = "This is a long document that needs to be split into smaller chunks.";
//! let chunks = splitter.split_text(text);
//! ```

mod character;
mod error;
mod html;
mod language;
mod markdown;
mod split_utils;
mod traits;

pub use character::{CharacterTextSplitter, RecursiveCharacterTextSplitter};
pub use dashflow::core::documents::Document;
pub use error::{Error, Result};
pub use html::{HTMLHeaderTextSplitter, HTMLTextSplitter};
pub use language::Language;
pub use markdown::{MarkdownHeaderTextSplitter, MarkdownTextSplitter};
pub use traits::{KeepSeparator, TextSplitter};
