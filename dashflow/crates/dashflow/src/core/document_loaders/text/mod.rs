//! Text and markup document loaders
//!
//! This module contains loaders for various text formats including:
//! - Plain text files (`TextLoader`, `DirectoryLoader`, etc.)
//! - Structured data formats (CSV, TSV, JSON)
//! - Markup languages (Markdown, HTML, XML, YAML, TOML, INI)

pub mod markup;
pub mod plain;
pub mod structured;

// Re-export all loaders
pub use markup::{HTMLLoader, IniLoader, MarkdownLoader, TOMLLoader, XMLLoader, YAMLLoader};
pub use plain::{BinaryFileLoader, DirectoryLoader, TextLoader, UnstructuredFileLoader};
pub use structured::{CSVLoader, JSONLoader, TSVLoader};
