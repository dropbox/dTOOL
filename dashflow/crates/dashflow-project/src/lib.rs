// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)
// DashFlow Project - Project Context Discovery for Coding Agents

//! # DashFlow Project
//!
//! Project context discovery for coding agents. Automatically detects project
//! documentation, languages, frameworks, and build systems.
//!
//! ## Features
//!
//! - **Documentation Discovery**: README, CLAUDE.md, AGENTS.md, etc.
//! - **Language Detection**: Rust, Python, TypeScript, Go, etc.
//! - **Framework Detection**: React, FastAPI, Actix, etc.
//! - **Build System Detection**: Cargo, npm, pip, make, etc.
//! - **Project Structure Analysis**: Source directories, config files
//!
//! ## Example
//!
//! ```rust,no_run
//! use dashflow_project::{ProjectContext, discover_project, discover_from_anywhere};
//! use std::path::PathBuf;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     // Option 1: Discover from known root directory
//!     let project = discover_project(PathBuf::from("/path/to/project")).await?;
//!
//!     // Option 2: Discover from anywhere inside the project
//!     // Walks UP directory tree until finding .dashflow/
//!     let project = discover_from_anywhere(PathBuf::from(".")).await?;
//!
//!     println!("Project name: {:?}", project.name);
//!     println!("Languages: {:?}", project.languages);
//!     println!("Build system: {:?}", project.primary_build_system);
//!
//!     // Get documentation for LLM context
//!     let docs = project.get_documentation()?;
//!     println!("Found {} documentation files", docs.len());
//!
//!     Ok(())
//! }
//! ```

mod discovery;
mod documentation;
mod languages;

pub use discovery::{discover_from_anywhere, discover_project, ProjectContext, ProjectError};
pub use documentation::{Documentation, DocumentationType};
pub use languages::{BuildSystem, Framework, Language};
