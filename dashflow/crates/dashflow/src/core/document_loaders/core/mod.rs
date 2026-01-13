//! Core document loaders.
//!
//! This module provides fundamental loaders:
//! - Directory and URL loaders
//! - Data structure loaders (`DataFrame`, Excel)
//! - Utility loaders (`LogFile`, Diff, SQL, Forth, XAML, SVG)

pub mod data;
pub mod directory;
pub mod utility;

pub use data::{DataFrameLoader, ExcelLoader};
pub use directory::{DirectoryLoader, URLLoader};
pub use utility::{DiffLoader, ForthLoader, LogFileLoader, SQLLoader, SVGLoader, XAMLLoader};
