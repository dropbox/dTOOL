//! Configuration and build system document loaders.
//!
//! This module provides loaders for configuration files and build systems:
//! - Configuration formats (Env, HCL, Dhall, Nix, Starlark, Jsonnet)
//! - Build systems (Dockerfile, Makefile, `CMake`)
//! - Template engines (Jinja2, Mustache, Handlebars, etc.)

mod build;
mod formats;
mod templates;

pub use build::{CMakeLoader, DockerfileLoader, MakefileLoader};
pub use formats::{DhallLoader, EnvLoader, HCLLoader, JsonnetLoader, NixLoader, StarlarkLoader};
pub use templates::{
    ERBLoader, HandlebarsLoader, Jinja2Loader, LiquidLoader, MustacheLoader, PugLoader,
};
