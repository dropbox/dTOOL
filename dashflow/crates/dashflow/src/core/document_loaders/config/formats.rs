// Allow clippy warnings for format detection
// - expect_used: expect() used after explicit non-empty check, always safe
#![allow(clippy::expect_used)]

//! Configuration format loaders for specialized config file types.
//!
//! This module provides document loaders for various configuration and build
//! system file formats including:
//!
//! - **`EnvLoader`**: .env environment variable files
//! - **`HCLLoader`**: `HashiCorp` Configuration Language (Terraform)
//! - **`DhallLoader`**: Dhall programmable configuration language
//! - **`NixLoader`**: Nix package management language
//! - **`StarlarkLoader`**: Starlark build language (Bazel)
//!
//! Each loader can optionally separate files by their logical units
//! (variables, blocks, definitions, rules) for more granular document processing.
//!
//! © 2025 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

use async_trait::async_trait;
use std::path::{Path, PathBuf};

use crate::core::documents::{Blob, Document, DocumentLoader};
use crate::core::error::Result;

/// Loads .env environment variable files.
///
/// The `EnvLoader` reads .env files containing environment variable definitions.
///
/// # Example
///
/// ```no_run
/// use dashflow::core::document_loaders::EnvLoader;
/// use dashflow::core::documents::DocumentLoader;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let loader = EnvLoader::new(".env");
/// let documents = loader.load().await?;
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone)]
pub struct EnvLoader {
    /// Path to the .env file
    pub file_path: PathBuf,
    /// Create separate documents per variable (default: false)
    pub separate_variables: bool,
}

impl EnvLoader {
    /// Create a new `EnvLoader` for the given file path.
    pub fn new(file_path: impl AsRef<Path>) -> Self {
        Self {
            file_path: file_path.as_ref().to_path_buf(),
            separate_variables: false,
        }
    }

    /// Create separate documents per environment variable.
    #[must_use]
    pub const fn with_separate_variables(mut self, separate: bool) -> Self {
        self.separate_variables = separate;
        self
    }
}

#[async_trait]
impl DocumentLoader for EnvLoader {
    async fn load(&self) -> Result<Vec<Document>> {
        let blob = Blob::from_path(&self.file_path);
        let content = blob.as_string()?;

        let mut documents = Vec::new();
        let mut all_content = String::new();
        let mut var_count = 0;

        for line in content.lines() {
            let trimmed = line.trim();

            // Skip comments and empty lines
            if trimmed.is_empty() || trimmed.starts_with('#') {
                if !self.separate_variables {
                    all_content.push_str(line);
                    all_content.push('\n');
                }
                continue;
            }

            // Parse KEY=VALUE format
            if let Some(eq_pos) = trimmed.find('=') {
                let key = trimmed[..eq_pos].trim();
                let value = trimmed[eq_pos + 1..].trim();

                var_count += 1;

                if self.separate_variables {
                    let doc = Document::new(format!("{key}={value}"))
                        .with_metadata("source", self.file_path.display().to_string())
                        .with_metadata("variable_index", var_count - 1)
                        .with_metadata("key", key)
                        .with_metadata("format", "env");

                    documents.push(doc);
                } else {
                    all_content.push_str(line);
                    all_content.push('\n');
                }
            } else if !self.separate_variables {
                // Malformed line, include in concatenated output
                all_content.push_str(line);
                all_content.push('\n');
            }
        }

        if !self.separate_variables {
            let doc = Document::new(all_content)
                .with_metadata("source", self.file_path.display().to_string())
                .with_metadata("format", "env")
                .with_metadata("variable_count", var_count);

            documents.push(doc);
        }

        Ok(documents)
    }
}

/// Loads HCL (`HashiCorp` Configuration Language) files.
///
/// HCL is a structured configuration language created by `HashiCorp`,
/// primarily used for Terraform infrastructure-as-code definitions.
/// It combines JSON-like syntax with programming language features
/// for more readable and maintainable configuration files.
///
/// Created by `HashiCorp` (2014) for Terraform and other `HashiCorp` tools.
/// Supports loading entire file or separating by resource blocks.
///
/// # Example
///
/// ```no_run
/// use dashflow::core::document_loaders::HCLLoader;
/// use dashflow::core::documents::DocumentLoader;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let loader = HCLLoader::new("terraform.tf");
/// let documents = loader.load().await?;
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone)]
pub struct HCLLoader {
    /// Path to the HCL file (.hcl or .tf)
    pub file_path: PathBuf,
    /// Whether to separate by resource blocks (default: false)
    pub separate_blocks: bool,
}

impl HCLLoader {
    /// Create a new `HCLLoader` for the given file path.
    pub fn new(file_path: impl AsRef<Path>) -> Self {
        Self {
            file_path: file_path.as_ref().to_path_buf(),
            separate_blocks: false,
        }
    }

    /// Enable separation by resource blocks.
    ///
    /// When enabled, splits the file into separate documents for each:
    /// - resource "type" "name" { }
    /// - data "type" "name" { }
    /// - variable "name" { }
    /// - output "name" { }
    /// - module "name" { }
    /// - provider "name" { }
    #[must_use]
    pub const fn with_separate_blocks(mut self) -> Self {
        self.separate_blocks = true;
        self
    }
}

#[async_trait]
impl DocumentLoader for HCLLoader {
    async fn load(&self) -> Result<Vec<Document>> {
        let blob = Blob::from_path(&self.file_path);
        let content = blob.as_string()?;

        if !self.separate_blocks {
            return Ok(vec![Document::new(&content)
                .with_metadata("source", self.file_path.display().to_string())
                .with_metadata("format", "hcl")]);
        }

        // Separate by resource blocks
        let mut documents = Vec::new();
        let lines: Vec<&str> = content.lines().collect();
        let mut i = 0;
        let mut block_index = 0;

        while i < lines.len() {
            let line = lines[i].trim();

            // Detect block types: resource, data, variable, output, module, provider
            if let Some(block_info) = Self::detect_block_start(line) {
                let (block_type, block_name) = block_info;
                let mut block_lines = vec![lines[i]];
                let mut brace_count =
                    line.matches('{').count() as i32 - line.matches('}').count() as i32;
                i += 1;

                // Collect block content until braces balance
                while i < lines.len() && brace_count > 0 {
                    let current_line = lines[i];
                    block_lines.push(current_line);
                    brace_count += current_line.matches('{').count() as i32;
                    brace_count -= current_line.matches('}').count() as i32;
                    i += 1;
                }

                let block_content = block_lines.join("\n");
                documents.push(
                    Document::new(&block_content)
                        .with_metadata("source", self.file_path.display().to_string())
                        .with_metadata("format", "hcl")
                        .with_metadata("block_index", block_index.to_string())
                        .with_metadata("block_type", block_type.to_string())
                        .with_metadata("block_name", block_name.clone()),
                );
                block_index += 1;
            } else {
                i += 1;
            }
        }

        if documents.is_empty() {
            Ok(vec![Document::new(&content)
                .with_metadata("source", self.file_path.display().to_string())
                .with_metadata("format", "hcl")])
        } else {
            Ok(documents)
        }
    }
}

impl HCLLoader {
    /// Detect HCL block start and extract type and name.
    /// Returns `Some((block_type`, `block_name`)) if line starts a block.
    fn detect_block_start(line: &str) -> Option<(&str, String)> {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.is_empty() {
            return None;
        }

        match parts[0] {
            "resource" | "data" => {
                // resource "type" "name" { or data "type" "name" {
                if parts.len() >= 3 {
                    let resource_type = parts[1].trim_matches('"');
                    let resource_name = parts[2].trim_matches('"');
                    Some((parts[0], format!("{resource_type}.{resource_name}")))
                } else {
                    None
                }
            }
            "variable" | "output" | "module" | "provider" => {
                // variable "name" { or output "name" { etc.
                if parts.len() >= 2 {
                    let name = parts[1].trim_matches('"');
                    Some((parts[0], name.to_string()))
                } else {
                    None
                }
            }
            _ => None,
        }
    }
}

/// Loads Dhall programmable configuration language files.
///
/// Dhall is a programmable configuration language created by Gabriel Gonzalez,
/// designed as a safer alternative to YAML/JSON with type checking and
/// functions. It guarantees termination (no infinite loops) and has strong
/// static typing.
///
/// Created by Gabriel Gonzalez (2017).
/// Used for type-safe configuration in cloud infrastructure and CI/CD.
/// Supports loading entire file or separating by let bindings.
///
/// # Example
///
/// ```no_run
/// use dashflow::core::document_loaders::DhallLoader;
/// use dashflow::core::documents::DocumentLoader;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let loader = DhallLoader::new("config.dhall");
/// let documents = loader.load().await?;
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone)]
pub struct DhallLoader {
    /// Path to the Dhall file (.dhall)
    pub file_path: PathBuf,
    /// Whether to separate by let bindings (default: false)
    pub separate_bindings: bool,
}

impl DhallLoader {
    /// Create a new `DhallLoader` for the given file path.
    pub fn new(file_path: impl AsRef<Path>) -> Self {
        Self {
            file_path: file_path.as_ref().to_path_buf(),
            separate_bindings: false,
        }
    }

    /// Enable separation by let bindings.
    ///
    /// When enabled, splits the file into separate documents for each:
    /// - let name = expression
    /// - let name : Type = expression
    #[must_use]
    pub const fn with_separate_bindings(mut self) -> Self {
        self.separate_bindings = true;
        self
    }
}

#[async_trait]
impl DocumentLoader for DhallLoader {
    async fn load(&self) -> Result<Vec<Document>> {
        let blob = Blob::from_path(&self.file_path);
        let content = blob.as_string()?;

        if !self.separate_bindings {
            return Ok(vec![Document::new(&content)
                .with_metadata("source", self.file_path.display().to_string())
                .with_metadata("format", "dhall")]);
        }

        // Separate by let bindings
        let mut documents = Vec::new();
        let lines: Vec<&str> = content.lines().collect();
        let mut i = 0;
        let mut binding_index = 0;

        while i < lines.len() {
            let line = lines[i].trim();

            // Detect let bindings: let name = ... or let name : Type = ...
            if line.starts_with("let ") {
                let binding_name = Self::extract_binding_name(line);
                let mut binding_lines = vec![lines[i]];
                i += 1;

                // Collect continuation lines (indented or until "in" keyword)
                let mut found_in = line.contains(" in ");
                while i < lines.len() && !found_in {
                    let current_line = lines[i];
                    let trimmed = current_line.trim();

                    // Stop at next let binding or "in" keyword
                    if trimmed.starts_with("let ") || trimmed.starts_with("in ") {
                        break;
                    }

                    // Stop at unindented lines (except comments)
                    if !current_line.is_empty()
                        && !current_line.starts_with(' ')
                        && !current_line.starts_with('\t')
                        && !trimmed.starts_with("--")
                    {
                        break;
                    }

                    binding_lines.push(current_line);
                    found_in = current_line.contains(" in ");
                    i += 1;
                }

                let binding_content = binding_lines.join("\n");
                documents.push(
                    Document::new(&binding_content)
                        .with_metadata("source", self.file_path.display().to_string())
                        .with_metadata("format", "dhall")
                        .with_metadata("binding_index", binding_index.to_string())
                        .with_metadata("binding_name", binding_name),
                );
                binding_index += 1;
            } else {
                i += 1;
            }
        }

        if documents.is_empty() {
            Ok(vec![Document::new(&content)
                .with_metadata("source", self.file_path.display().to_string())
                .with_metadata("format", "dhall")])
        } else {
            Ok(documents)
        }
    }
}

impl DhallLoader {
    /// Extract binding name from let statement.
    fn extract_binding_name(line: &str) -> String {
        // let name = ... or let name : Type = ...
        let after_let = line.strip_prefix("let ").unwrap_or(line);
        let before_eq_or_colon = after_let.split(&['=', ':'][..]).next().unwrap_or(after_let);
        before_eq_or_colon.trim().to_string()
    }
}

/// Loader for Nix package management language files (.nix).
///
/// Nix is a purely functional package management language created by Eelco Dolstra in 2003.
/// Used by NixOS, nixpkgs, and the Nix package manager. Declarative, deterministic builds.
///
/// # Example
///
/// ```rust,no_run
/// # use dashflow::core::document_loaders::NixLoader;
/// # use dashflow::core::documents::DocumentLoader;
/// # async fn example() -> dashflow::core::error::Result<()> {
/// let loader = NixLoader::new("default.nix");
/// let docs = loader.load().await?;
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone)]
pub struct NixLoader {
    file_path: PathBuf,
    separate_definitions: bool,
}

impl NixLoader {
    /// Create a new Nix loader for the given file path.
    pub fn new<P: AsRef<Path>>(path: P) -> Self {
        Self {
            file_path: path.as_ref().to_path_buf(),
            separate_definitions: false,
        }
    }

    /// Enable separation by top-level definitions (let bindings, function defs, attribute sets).
    #[must_use]
    pub const fn with_separate_definitions(mut self) -> Self {
        self.separate_definitions = true;
        self
    }
}

#[async_trait]
impl DocumentLoader for NixLoader {
    async fn load(&self) -> Result<Vec<Document>> {
        let blob = Blob::from_path(&self.file_path);
        let content = blob.as_string()?;

        if !self.separate_definitions {
            return Ok(vec![Document::new(&content)
                .with_metadata("source", self.file_path.display().to_string())
                .with_metadata("format", "nix")]);
        }

        // Separate by top-level definitions
        let lines: Vec<&str> = content.lines().collect();
        let mut documents = Vec::new();
        let mut i = 0;
        let mut definition_index = 0;

        while i < lines.len() {
            let line = lines[i].trim();

            // Skip empty lines and comments
            if line.is_empty() || line.starts_with('#') {
                i += 1;
                continue;
            }

            // Detect top-level definitions: let bindings or assignments (name = ...)
            if line.starts_with("let ") || Self::is_definition(line) {
                let definition_name = Self::extract_definition_name(line);
                let mut definition_lines = vec![lines[i]];
                i += 1;

                // Continue collecting lines for this definition
                // Nix uses ; or in keywords, or balanced braces for attr sets
                let mut brace_count =
                    line.matches('{').count() as i32 - line.matches('}').count() as i32;
                let mut paren_count =
                    line.matches('(').count() as i32 - line.matches(')').count() as i32;
                let mut bracket_count =
                    line.matches('[').count() as i32 - line.matches(']').count() as i32;

                // For let bindings, continue until "in" or balanced expression
                let is_let = line.starts_with("let ");
                let mut found_semicolon = line.trim_end().ends_with(';');
                let mut found_in = line.contains(" in ");

                while i < lines.len() {
                    let next_line = lines[i];
                    let trimmed = next_line.trim();

                    // Skip comments
                    if trimmed.starts_with('#') {
                        i += 1;
                        continue;
                    }

                    definition_lines.push(next_line);

                    // Update counts
                    brace_count += next_line.matches('{').count() as i32
                        - next_line.matches('}').count() as i32;
                    paren_count += next_line.matches('(').count() as i32
                        - next_line.matches(')').count() as i32;
                    bracket_count += next_line.matches('[').count() as i32
                        - next_line.matches(']').count() as i32;

                    // Check for terminators
                    if trimmed.ends_with(';') {
                        found_semicolon = true;
                    }
                    if trimmed.contains(" in ") || trimmed.starts_with("in ") {
                        found_in = true;
                    }

                    i += 1;

                    // Break conditions
                    if is_let && found_in {
                        break;
                    }
                    if found_semicolon && brace_count == 0 && paren_count == 0 && bracket_count == 0
                    {
                        break;
                    }
                    if brace_count == 0 && paren_count == 0 && bracket_count == 0 && !is_let {
                        // For non-let definitions, balanced delimiters mean we're done
                        if trimmed.ends_with(';') || trimmed.ends_with('}') {
                            break;
                        }
                    }

                    // Safety: break on next definition
                    if brace_count == 0
                        && paren_count == 0
                        && bracket_count == 0
                        && (trimmed.starts_with("let ") || Self::is_definition(trimmed))
                    {
                        // Push back the line for next iteration
                        definition_lines.pop();
                        i -= 1;
                        break;
                    }
                }

                let definition_content = definition_lines.join("\n");
                documents.push(
                    Document::new(&definition_content)
                        .with_metadata("source", self.file_path.display().to_string())
                        .with_metadata("format", "nix")
                        .with_metadata("definition_index", definition_index.to_string())
                        .with_metadata("definition_name", definition_name),
                );
                definition_index += 1;
            } else {
                i += 1;
            }
        }

        if documents.is_empty() {
            Ok(vec![Document::new(&content)
                .with_metadata("source", self.file_path.display().to_string())
                .with_metadata("format", "nix")])
        } else {
            Ok(documents)
        }
    }
}

impl NixLoader {
    /// Check if line is a top-level definition (name = ...)
    fn is_definition(line: &str) -> bool {
        if let Some(eq_pos) = line.find('=') {
            let before_eq = line[..eq_pos].trim();
            // Must have a valid identifier before =
            // Valid Nix identifiers: start with letter or underscore, contain alphanumeric or - or _
            if before_eq.is_empty() {
                return false;
            }
            let first_char = before_eq.chars().next().expect("checked non-empty above");
            if !first_char.is_alphabetic() && first_char != '_' {
                return false;
            }
            // Check all characters are valid
            before_eq
                .chars()
                .all(|c| c.is_alphanumeric() || c == '_' || c == '-')
        } else {
            false
        }
    }

    /// Extract definition name from line
    fn extract_definition_name(line: &str) -> String {
        if line.starts_with("let ") {
            // let name = ... or let { ... }
            let after_let = line.strip_prefix("let ").unwrap_or(line);
            if after_let.trim().starts_with('{') {
                "let_block".to_string()
            } else {
                after_let
                    .split('=')
                    .next()
                    .unwrap_or("let")
                    .trim()
                    .to_string()
            }
        } else if let Some(eq_pos) = line.find('=') {
            line[..eq_pos].trim().to_string()
        } else {
            "definition".to_string()
        }
    }
}

/// Loader for Starlark build language files (.bzl, BUILD, WORKSPACE).
///
/// Starlark is a Python-like configuration language created by Google for Bazel build system.
/// Subset of Python: deterministic, no recursion, used for build rules and configuration.
///
/// # Example
///
/// ```rust,no_run
/// # use dashflow::core::document_loaders::StarlarkLoader;
/// # use dashflow::core::documents::DocumentLoader;
/// # async fn example() -> dashflow::core::error::Result<()> {
/// let loader = StarlarkLoader::new("BUILD");
/// let docs = loader.load().await?;
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone)]
pub struct StarlarkLoader {
    file_path: PathBuf,
    separate_rules: bool,
}

impl StarlarkLoader {
    /// Create a new Starlark loader for the given file path.
    pub fn new<P: AsRef<Path>>(path: P) -> Self {
        Self {
            file_path: path.as_ref().to_path_buf(),
            separate_rules: false,
        }
    }

    /// Enable separation by build rules and function definitions.
    #[must_use]
    pub const fn with_separate_rules(mut self) -> Self {
        self.separate_rules = true;
        self
    }
}

#[async_trait]
impl DocumentLoader for StarlarkLoader {
    async fn load(&self) -> Result<Vec<Document>> {
        let blob = Blob::from_path(&self.file_path);
        let content = blob.as_string()?;

        if !self.separate_rules {
            return Ok(vec![Document::new(&content)
                .with_metadata("source", self.file_path.display().to_string())
                .with_metadata("format", "starlark")]);
        }

        // Separate by function definitions and build rules
        let lines: Vec<&str> = content.lines().collect();
        let mut documents = Vec::new();
        let mut i = 0;
        let mut rule_index = 0;

        while i < lines.len() {
            let line = lines[i];
            let trimmed = line.trim();

            // Skip empty lines and comments
            if trimmed.is_empty() || trimmed.starts_with('#') {
                i += 1;
                continue;
            }

            // Detect function definitions (def name(...):) or build rules (rule_name(...))
            if trimmed.starts_with("def ") || Self::is_build_rule(trimmed) {
                let rule_name = Self::extract_rule_name(trimmed);
                let mut rule_lines = vec![lines[i]];
                let base_indent = line.len() - line.trim_start().len();
                i += 1;

                // For function defs, collect indented body (Python-like)
                if trimmed.starts_with("def ") {
                    while i < lines.len() {
                        let next_line = lines[i];
                        let next_trimmed = next_line.trim();

                        // Skip empty lines and comments
                        if next_trimmed.is_empty() || next_trimmed.starts_with('#') {
                            rule_lines.push(next_line);
                            i += 1;
                            continue;
                        }

                        let next_indent = next_line.len() - next_line.trim_start().len();

                        // Continue if indented more than def line
                        if next_indent > base_indent {
                            rule_lines.push(next_line);
                            i += 1;
                        } else {
                            break;
                        }
                    }
                } else {
                    // For build rules, collect until closing paren
                    let mut paren_count =
                        trimmed.matches('(').count() as i32 - trimmed.matches(')').count() as i32;

                    while i < lines.len() && paren_count > 0 {
                        let next_line = lines[i];
                        rule_lines.push(next_line);
                        paren_count += next_line.matches('(').count() as i32
                            - next_line.matches(')').count() as i32;
                        i += 1;
                    }
                }

                let rule_content = rule_lines.join("\n");
                documents.push(
                    Document::new(&rule_content)
                        .with_metadata("source", self.file_path.display().to_string())
                        .with_metadata("format", "starlark")
                        .with_metadata("rule_index", rule_index.to_string())
                        .with_metadata("rule_name", rule_name),
                );
                rule_index += 1;
            } else {
                i += 1;
            }
        }

        if documents.is_empty() {
            Ok(vec![Document::new(&content)
                .with_metadata("source", self.file_path.display().to_string())
                .with_metadata("format", "starlark")])
        } else {
            Ok(documents)
        }
    }
}

impl StarlarkLoader {
    /// Check if line is a build rule call (identifier followed by opening paren)
    fn is_build_rule(line: &str) -> bool {
        // Build rules like: cc_library(, py_binary(, load(, etc.
        // Must start with identifier and have opening paren
        if let Some(paren_pos) = line.find('(') {
            let before_paren = line[..paren_pos].trim();
            // Check it's a valid identifier (letters, numbers, underscore)
            !before_paren.is_empty()
                && before_paren
                    .chars()
                    .all(|c| c.is_alphanumeric() || c == '_')
                && before_paren.chars().next().is_some_and(|c| !c.is_numeric())
        } else {
            false
        }
    }

    /// Extract rule name from line
    fn extract_rule_name(line: &str) -> String {
        if line.starts_with("def ") {
            // def function_name(...):
            let after_def = line.strip_prefix("def ").unwrap_or(line);
            after_def
                .split('(')
                .next()
                .unwrap_or("def")
                .trim()
                .to_string()
        } else if let Some(paren_pos) = line.find('(') {
            // rule_name(...)
            line[..paren_pos].trim().to_string()
        } else {
            "rule".to_string()
        }
    }
}

/// Loader for Jsonnet configuration template files (.jsonnet, .libsonnet).
///
/// Jsonnet is a data templating language created by Google,
/// designed to generate JSON/YAML configuration with programming
/// language features like variables, functions, and inheritance.
///
/// Created by Dave Cunningham at Google (2014).
/// Used for Kubernetes configs, monitoring configs, and configuration management.
/// Supports loading entire file or separating by function/object definitions.
///
/// # Example
///
/// ```no_run
/// use dashflow::core::document_loaders::JsonnetLoader;
/// use dashflow::core::documents::DocumentLoader;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let loader = JsonnetLoader::new("config.jsonnet");
/// let documents = loader.load().await?;
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone)]
pub struct JsonnetLoader {
    /// Path to the Jsonnet file (.jsonnet or .libsonnet)
    pub file_path: PathBuf,
    /// Whether to separate by function/object definitions (default: false)
    pub separate_definitions: bool,
}

impl JsonnetLoader {
    /// Create a new `JsonnetLoader` for the given file path.
    pub fn new(file_path: impl AsRef<Path>) -> Self {
        Self {
            file_path: file_path.as_ref().to_path_buf(),
            separate_definitions: false,
        }
    }

    /// Enable separation by function and object definitions.
    ///
    /// When enabled, splits the file into separate documents for each:
    /// - local name = { } (object definitions)
    /// - local name(args) = (function definitions)
    #[must_use]
    pub const fn with_separate_definitions(mut self) -> Self {
        self.separate_definitions = true;
        self
    }
}

#[async_trait]
impl DocumentLoader for JsonnetLoader {
    async fn load(&self) -> Result<Vec<Document>> {
        let blob = Blob::from_path(&self.file_path);
        let content = blob.as_string()?;

        if !self.separate_definitions {
            return Ok(vec![Document::new(&content)
                .with_metadata("source", self.file_path.display().to_string())
                .with_metadata("format", "jsonnet")]);
        }

        // Separate by local definitions
        let mut documents = Vec::new();
        let lines: Vec<&str> = content.lines().collect();
        let mut i = 0;
        let mut def_index = 0;

        while i < lines.len() {
            let line = lines[i].trim();

            // Detect local definitions: local name = ... or local name(args) = ...
            if line.starts_with("local ") && line.contains('=') {
                let def_name = Self::extract_definition_name(line);
                let mut def_lines = vec![lines[i]];

                // Determine if this is a simple value, object, or function
                let has_brace = line.contains('{');
                let has_bracket = line.contains('[');
                let ends_with_semicolon = line.trim_end().ends_with(';');
                let ends_with_comma = line.trim_end().ends_with(',');

                i += 1;

                // If it's an object or array, collect until braces/brackets balance
                if has_brace || has_bracket {
                    let mut brace_count =
                        line.matches('{').count() as i32 - line.matches('}').count() as i32;
                    let mut bracket_count =
                        line.matches('[').count() as i32 - line.matches(']').count() as i32;

                    while i < lines.len() && (brace_count > 0 || bracket_count > 0) {
                        let current_line = lines[i];
                        def_lines.push(current_line);
                        brace_count += current_line.matches('{').count() as i32;
                        brace_count -= current_line.matches('}').count() as i32;
                        bracket_count += current_line.matches('[').count() as i32;
                        bracket_count -= current_line.matches(']').count() as i32;
                        i += 1;
                    }
                } else if !ends_with_semicolon && !ends_with_comma {
                    // Multi-line expression without braces, collect until semicolon or comma
                    while i < lines.len() {
                        let current_line = lines[i];
                        def_lines.push(current_line);
                        let trimmed = current_line.trim();
                        if trimmed.ends_with(';') || trimmed.ends_with(',') {
                            i += 1;
                            break;
                        }
                        i += 1;
                    }
                }

                let def_content = def_lines.join("\n");
                documents.push(
                    Document::new(&def_content)
                        .with_metadata("source", self.file_path.display().to_string())
                        .with_metadata("format", "jsonnet")
                        .with_metadata("definition_index", def_index.to_string())
                        .with_metadata("definition_name", def_name),
                );
                def_index += 1;
            } else {
                i += 1;
            }
        }

        if documents.is_empty() {
            Ok(vec![Document::new(&content)
                .with_metadata("source", self.file_path.display().to_string())
                .with_metadata("format", "jsonnet")])
        } else {
            Ok(documents)
        }
    }
}

impl JsonnetLoader {
    /// Extract definition name from local statement.
    fn extract_definition_name(line: &str) -> String {
        // local name = ... or local name(args) = ...
        let after_local = line.strip_prefix("local ").unwrap_or(line);
        let before_eq = after_local.split('=').next().unwrap_or(after_local);
        let name_part = before_eq.split('(').next().unwrap_or(before_eq);
        name_part.trim().to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::documents::DocumentLoader;
    use std::fs;
    use tempfile::TempDir;

    // ============================================================================
    // EnvLoader Tests
    // ============================================================================

    #[tokio::test]
    async fn test_env_loader_basic() {
        let temp_dir = TempDir::new().unwrap();
        let env_path = temp_dir.path().join(".env");

        let env_content = r#"# Database config
DATABASE_URL=postgres://localhost:5432/mydb
API_KEY=secret123
DEBUG=true"#;

        fs::write(&env_path, env_content).unwrap();

        let loader = EnvLoader::new(&env_path);
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1);
        assert!(docs[0].page_content.contains("DATABASE_URL"));
        assert!(docs[0].page_content.contains("API_KEY"));
        assert_eq!(
            docs[0].get_metadata("format").and_then(|v| v.as_str()),
            Some("env")
        );
        assert_eq!(
            docs[0]
                .get_metadata("variable_count")
                .and_then(|v| v.as_u64()),
            Some(3)
        );
    }

    #[tokio::test]
    async fn test_env_loader_separate_variables() {
        let temp_dir = TempDir::new().unwrap();
        let env_path = temp_dir.path().join(".env");

        let env_content = r#"HOST=localhost
PORT=8080
DEBUG=false"#;

        fs::write(&env_path, env_content).unwrap();

        let loader = EnvLoader::new(&env_path).with_separate_variables(true);
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 3);
        assert_eq!(docs[0].page_content, "HOST=localhost");
        assert_eq!(docs[1].page_content, "PORT=8080");
        assert_eq!(docs[2].page_content, "DEBUG=false");
        assert_eq!(
            docs[0].get_metadata("key").and_then(|v| v.as_str()),
            Some("HOST")
        );
        assert_eq!(
            docs[1].get_metadata("key").and_then(|v| v.as_str()),
            Some("PORT")
        );
    }

    #[tokio::test]
    async fn test_env_loader_with_comments() {
        let temp_dir = TempDir::new().unwrap();
        let env_path = temp_dir.path().join(".env");

        let env_content = r#"# This is a comment
KEY1=value1
# Another comment
KEY2=value2"#;

        fs::write(&env_path, env_content).unwrap();

        let loader = EnvLoader::new(&env_path);
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1);
        assert!(docs[0].page_content.contains("# This is a comment"));
        assert!(docs[0].page_content.contains("KEY1=value1"));
    }

    #[tokio::test]
    async fn test_env_loader_empty_values() {
        let temp_dir = TempDir::new().unwrap();
        let env_path = temp_dir.path().join(".env");

        let env_content = "EMPTY_KEY=\nKEY_WITH_VALUE=something";
        fs::write(&env_path, env_content).unwrap();

        let loader = EnvLoader::new(&env_path).with_separate_variables(true);
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 2);
        assert_eq!(docs[0].page_content, "EMPTY_KEY=");
    }

    #[tokio::test]
    async fn test_env_loader_quoted_values() {
        let temp_dir = TempDir::new().unwrap();
        let env_path = temp_dir.path().join(".env");

        let env_content = r#"QUOTED="hello world"
SINGLE='single quotes'"#;
        fs::write(&env_path, env_content).unwrap();

        let loader = EnvLoader::new(&env_path).with_separate_variables(true);
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 2);
        assert!(docs[0].page_content.contains(r#""hello world""#));
    }

    // ============================================================================
    // HCLLoader Tests
    // ============================================================================

    #[tokio::test]
    async fn test_hcl_loader_basic() {
        let temp_dir = TempDir::new().unwrap();
        let hcl_path = temp_dir.path().join("main.tf");

        let hcl_content = r#"provider "aws" {
  region = "us-west-2"
}

resource "aws_instance" "example" {
  ami           = "ami-12345678"
  instance_type = "t2.micro"
}"#;

        fs::write(&hcl_path, hcl_content).unwrap();

        let loader = HCLLoader::new(&hcl_path);
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1);
        assert!(docs[0].page_content.contains("provider"));
        assert!(docs[0].page_content.contains("resource"));
        assert_eq!(
            docs[0].get_metadata("format").and_then(|v| v.as_str()),
            Some("hcl")
        );
    }

    #[tokio::test]
    async fn test_hcl_loader_separate_blocks() {
        let temp_dir = TempDir::new().unwrap();
        let hcl_path = temp_dir.path().join("terraform.tf");

        let hcl_content = r#"variable "region" {
  default = "us-west-2"
}

resource "aws_instance" "web" {
  ami = "ami-123"
}

output "instance_id" {
  value = aws_instance.web.id
}"#;

        fs::write(&hcl_path, hcl_content).unwrap();

        let loader = HCLLoader::new(&hcl_path).with_separate_blocks();
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 3);
        assert!(docs[0].page_content.contains("variable"));
        assert!(docs[1].page_content.contains("resource"));
        assert!(docs[2].page_content.contains("output"));
        assert_eq!(
            docs[0].get_metadata("block_type").and_then(|v| v.as_str()),
            Some("variable")
        );
        assert_eq!(
            docs[1].get_metadata("block_name").and_then(|v| v.as_str()),
            Some("aws_instance.web")
        );
    }

    #[tokio::test]
    async fn test_hcl_loader_nested_blocks() {
        let temp_dir = TempDir::new().unwrap();
        let hcl_path = temp_dir.path().join("nested.tf");

        let hcl_content = r#"resource "aws_security_group" "example" {
  name = "example"

  ingress {
    from_port = 443
    to_port   = 443
    protocol  = "tcp"
  }

  egress {
    from_port = 0
    to_port   = 0
    protocol  = "-1"
  }
}"#;

        fs::write(&hcl_path, hcl_content).unwrap();

        let loader = HCLLoader::new(&hcl_path).with_separate_blocks();
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1);
        assert!(docs[0].page_content.contains("ingress"));
        assert!(docs[0].page_content.contains("egress"));
    }

    #[tokio::test]
    async fn test_hcl_loader_module_and_data() {
        let temp_dir = TempDir::new().unwrap();
        let hcl_path = temp_dir.path().join("modules.tf");

        let hcl_content = r#"module "vpc" {
  source = "./modules/vpc"
}

data "aws_ami" "ubuntu" {
  most_recent = true
}"#;

        fs::write(&hcl_path, hcl_content).unwrap();

        let loader = HCLLoader::new(&hcl_path).with_separate_blocks();
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 2);
        assert_eq!(
            docs[0].get_metadata("block_type").and_then(|v| v.as_str()),
            Some("module")
        );
        assert_eq!(
            docs[1].get_metadata("block_type").and_then(|v| v.as_str()),
            Some("data")
        );
    }

    // ============================================================================
    // DhallLoader Tests
    // ============================================================================

    #[tokio::test]
    async fn test_dhall_loader_basic() {
        let temp_dir = TempDir::new().unwrap();
        let dhall_path = temp_dir.path().join("config.dhall");

        let dhall_content = r#"let name = "app"
let port = 8080
in { name, port }"#;

        fs::write(&dhall_path, dhall_content).unwrap();

        let loader = DhallLoader::new(&dhall_path);
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1);
        assert!(docs[0].page_content.contains("let name"));
        assert_eq!(
            docs[0].get_metadata("format").and_then(|v| v.as_str()),
            Some("dhall")
        );
    }

    #[tokio::test]
    async fn test_dhall_loader_separate_bindings() {
        let temp_dir = TempDir::new().unwrap();
        let dhall_path = temp_dir.path().join("bindings.dhall");

        let dhall_content = r#"let serverConfig = {
  host = "localhost",
  port = 8080
}

let databaseConfig = {
  url = "postgres://localhost"
}

in { server = serverConfig, database = databaseConfig }"#;

        fs::write(&dhall_path, dhall_content).unwrap();

        let loader = DhallLoader::new(&dhall_path).with_separate_bindings();
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 2);
        assert!(docs[0].page_content.contains("serverConfig"));
        assert!(docs[1].page_content.contains("databaseConfig"));
        assert_eq!(
            docs[0]
                .get_metadata("binding_name")
                .and_then(|v| v.as_str()),
            Some("serverConfig")
        );
    }

    #[tokio::test]
    async fn test_dhall_loader_typed_bindings() {
        let temp_dir = TempDir::new().unwrap();
        let dhall_path = temp_dir.path().join("typed.dhall");

        let dhall_content = r#"let Config : Type = { name : Text, port : Natural }
let myConfig : Config = { name = "app", port = 8080 }
in myConfig"#;

        fs::write(&dhall_path, dhall_content).unwrap();

        let loader = DhallLoader::new(&dhall_path).with_separate_bindings();
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 2);
        assert_eq!(
            docs[0]
                .get_metadata("binding_name")
                .and_then(|v| v.as_str()),
            Some("Config")
        );
        assert_eq!(
            docs[1]
                .get_metadata("binding_name")
                .and_then(|v| v.as_str()),
            Some("myConfig")
        );
    }

    // ============================================================================
    // NixLoader Tests
    // ============================================================================

    #[tokio::test]
    async fn test_nix_loader_basic() {
        let temp_dir = TempDir::new().unwrap();
        let nix_path = temp_dir.path().join("default.nix");

        let nix_content = r#"{ pkgs ? import <nixpkgs> {} }:
pkgs.mkShell {
  buildInputs = [ pkgs.rustc pkgs.cargo ];
}"#;

        fs::write(&nix_path, nix_content).unwrap();

        let loader = NixLoader::new(&nix_path);
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1);
        assert!(docs[0].page_content.contains("mkShell"));
        assert_eq!(
            docs[0].get_metadata("format").and_then(|v| v.as_str()),
            Some("nix")
        );
    }

    #[tokio::test]
    async fn test_nix_loader_separate_definitions() {
        let temp_dir = TempDir::new().unwrap();
        let nix_path = temp_dir.path().join("defs.nix");

        let nix_content = r#"let
  pkgs = import <nixpkgs> {};

  myPackage = pkgs.stdenv.mkDerivation {
    name = "my-package";
  };

in myPackage"#;

        fs::write(&nix_path, nix_content).unwrap();

        let loader = NixLoader::new(&nix_path).with_separate_definitions();
        let docs = loader.load().await.unwrap();

        // Should find the let block
        assert!(!docs.is_empty());
        assert!(docs
            .iter()
            .any(|d| d.page_content.contains("pkgs") || d.page_content.contains("myPackage")));
    }

    #[tokio::test]
    async fn test_nix_loader_attribute_set() {
        let temp_dir = TempDir::new().unwrap();
        let nix_path = temp_dir.path().join("attrs.nix");

        let nix_content = r#"{
  name = "example";
  version = "1.0.0";
  src = fetchurl {
    url = "https://example.com/source.tar.gz";
    sha256 = "abc123";
  };
}"#;

        fs::write(&nix_path, nix_content).unwrap();

        let loader = NixLoader::new(&nix_path);
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1);
        assert!(docs[0].page_content.contains("fetchurl"));
    }

    // ============================================================================
    // StarlarkLoader Tests
    // ============================================================================

    #[tokio::test]
    async fn test_starlark_loader_basic() {
        let temp_dir = TempDir::new().unwrap();
        let bzl_path = temp_dir.path().join("BUILD");

        let starlark_content = r#"load("@rules_rust//rust:defs.bzl", "rust_binary")

rust_binary(
    name = "hello",
    srcs = ["main.rs"],
)"#;

        fs::write(&bzl_path, starlark_content).unwrap();

        let loader = StarlarkLoader::new(&bzl_path);
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1);
        assert!(docs[0].page_content.contains("rust_binary"));
        assert_eq!(
            docs[0].get_metadata("format").and_then(|v| v.as_str()),
            Some("starlark")
        );
    }

    #[tokio::test]
    async fn test_starlark_loader_separate_rules() {
        let temp_dir = TempDir::new().unwrap();
        let bzl_path = temp_dir.path().join("BUILD.bazel");

        let starlark_content = r#"cc_library(
    name = "mylib",
    srcs = ["lib.cc"],
    hdrs = ["lib.h"],
)

cc_binary(
    name = "mybin",
    srcs = ["main.cc"],
    deps = [":mylib"],
)

cc_test(
    name = "mytest",
    srcs = ["test.cc"],
)"#;

        fs::write(&bzl_path, starlark_content).unwrap();

        let loader = StarlarkLoader::new(&bzl_path).with_separate_rules();
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 3);
        assert!(docs[0].page_content.contains("cc_library"));
        assert!(docs[1].page_content.contains("cc_binary"));
        assert!(docs[2].page_content.contains("cc_test"));
        assert_eq!(
            docs[0].get_metadata("rule_name").and_then(|v| v.as_str()),
            Some("cc_library")
        );
    }

    #[tokio::test]
    async fn test_starlark_loader_function_definitions() {
        let temp_dir = TempDir::new().unwrap();
        let bzl_path = temp_dir.path().join("rules.bzl");

        let starlark_content = r#"def my_rule(name, srcs):
    """Custom build rule."""
    native.genrule(
        name = name,
        srcs = srcs,
    )

def helper():
    return True"#;

        fs::write(&bzl_path, starlark_content).unwrap();

        let loader = StarlarkLoader::new(&bzl_path).with_separate_rules();
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 2);
        assert!(docs[0].page_content.contains("my_rule"));
        assert!(docs[1].page_content.contains("helper"));
        assert_eq!(
            docs[0].get_metadata("rule_name").and_then(|v| v.as_str()),
            Some("my_rule")
        );
    }

    #[tokio::test]
    async fn test_starlark_loader_with_comments() {
        let temp_dir = TempDir::new().unwrap();
        let bzl_path = temp_dir.path().join("commented.bzl");

        let starlark_content = r#"# This is a comment
load("@rules//defs.bzl", "my_rule")

# Another comment
my_rule(
    name = "example",
)"#;

        fs::write(&bzl_path, starlark_content).unwrap();

        let loader = StarlarkLoader::new(&bzl_path).with_separate_rules();
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 2);
    }

    // ============================================================================
    // JsonnetLoader Tests
    // ============================================================================

    #[tokio::test]
    async fn test_jsonnet_loader() {
        let temp_dir = TempDir::new().unwrap();
        let jsonnet_path = temp_dir.path().join("config.jsonnet");

        let jsonnet_content = r#"local name = "app";
{
  name: name,
  port: 8080
}"#;

        fs::write(&jsonnet_path, jsonnet_content).unwrap();

        let loader = JsonnetLoader::new(&jsonnet_path);
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1);
        assert!(docs[0].page_content.contains("local"));
        assert_eq!(
            docs[0].get_metadata("format").and_then(|v| v.as_str()),
            Some("jsonnet")
        );
    }

    #[tokio::test]
    async fn test_jsonnet_loader_separate_definitions() {
        let temp_dir = TempDir::new().unwrap();
        let jsonnet_path = temp_dir.path().join("library.libsonnet");

        let jsonnet_content = r#"local version = "1.0.0";
local config = {
  host: "localhost",
  port: 8080,
};
local multiply(x, y) = x * y;
{
  version: version,
  config: config,
}"#;

        fs::write(&jsonnet_path, jsonnet_content).unwrap();

        let loader = JsonnetLoader::new(&jsonnet_path).with_separate_definitions();
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 3);
        assert!(docs[0].page_content.contains("version"));
        assert!(docs[1].page_content.contains("config"));
        assert!(docs[2].page_content.contains("multiply"));
        assert_eq!(
            docs[0]
                .get_metadata("definition_name")
                .and_then(|v| v.as_str()),
            Some("version")
        );
        assert_eq!(
            docs[1]
                .get_metadata("definition_name")
                .and_then(|v| v.as_str()),
            Some("config")
        );
        assert_eq!(
            docs[2]
                .get_metadata("definition_name")
                .and_then(|v| v.as_str()),
            Some("multiply")
        );
    }

    #[tokio::test]
    async fn test_jsonnet_loader_nested_objects() {
        let temp_dir = TempDir::new().unwrap();
        let jsonnet_path = temp_dir.path().join("nested.jsonnet");

        let jsonnet_content = r#"local nested = {
  level1: {
    level2: {
      value: "deep"
    }
  }
};
nested"#;

        fs::write(&jsonnet_path, jsonnet_content).unwrap();

        let loader = JsonnetLoader::new(&jsonnet_path).with_separate_definitions();
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1);
        assert!(docs[0].page_content.contains("level2"));
    }

    #[tokio::test]
    async fn test_jsonnet_loader_arrays() {
        let temp_dir = TempDir::new().unwrap();
        let jsonnet_path = temp_dir.path().join("arrays.jsonnet");

        let jsonnet_content = r#"local items = [
  "item1",
  "item2",
  "item3"
];
{ items: items }"#;

        fs::write(&jsonnet_path, jsonnet_content).unwrap();

        let loader = JsonnetLoader::new(&jsonnet_path).with_separate_definitions();
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1);
        assert!(docs[0].page_content.contains("item2"));
    }

    // ============================================================================
    // Edge Cases and Error Handling
    // ============================================================================

    #[tokio::test]
    async fn test_env_loader_empty_file() {
        let temp_dir = TempDir::new().unwrap();
        let env_path = temp_dir.path().join(".env");

        fs::write(&env_path, "").unwrap();

        let loader = EnvLoader::new(&env_path);
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1);
        assert!(docs[0].page_content.is_empty());
    }

    #[tokio::test]
    async fn test_hcl_loader_empty_file() {
        let temp_dir = TempDir::new().unwrap();
        let hcl_path = temp_dir.path().join("empty.tf");

        fs::write(&hcl_path, "").unwrap();

        let loader = HCLLoader::new(&hcl_path).with_separate_blocks();
        let docs = loader.load().await.unwrap();

        // Empty file should return one document with empty content
        assert_eq!(docs.len(), 1);
    }

    #[tokio::test]
    #[allow(clippy::redundant_clone)]
    async fn test_loader_clone_traits() {
        // Test that all loaders implement Clone
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join("test");
        fs::write(&path, "test").unwrap();

        let env_loader = EnvLoader::new(&path);
        let _cloned = env_loader.clone();

        let hcl_loader = HCLLoader::new(&path);
        let _cloned = hcl_loader.clone();

        let dhall_loader = DhallLoader::new(&path);
        let _cloned = dhall_loader.clone();

        let nix_loader = NixLoader::new(&path);
        let _cloned = nix_loader.clone();

        let starlark_loader = StarlarkLoader::new(&path);
        let _cloned = starlark_loader.clone();

        let jsonnet_loader = JsonnetLoader::new(&path);
        let _cloned = jsonnet_loader.clone();
    }

    #[tokio::test]
    async fn test_loader_debug_traits() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join("test");
        fs::write(&path, "test").unwrap();

        let env_loader = EnvLoader::new(&path);
        let debug_str = format!("{:?}", env_loader);
        assert!(debug_str.contains("EnvLoader"));

        let hcl_loader = HCLLoader::new(&path);
        let debug_str = format!("{:?}", hcl_loader);
        assert!(debug_str.contains("HCLLoader"));

        let dhall_loader = DhallLoader::new(&path);
        let debug_str = format!("{:?}", dhall_loader);
        assert!(debug_str.contains("DhallLoader"));

        let nix_loader = NixLoader::new(&path);
        let debug_str = format!("{:?}", nix_loader);
        assert!(debug_str.contains("NixLoader"));

        let starlark_loader = StarlarkLoader::new(&path);
        let debug_str = format!("{:?}", starlark_loader);
        assert!(debug_str.contains("StarlarkLoader"));

        let jsonnet_loader = JsonnetLoader::new(&path);
        let debug_str = format!("{:?}", jsonnet_loader);
        assert!(debug_str.contains("JsonnetLoader"));
    }

    #[tokio::test]
    async fn test_env_loader_special_characters() {
        let temp_dir = TempDir::new().unwrap();
        let env_path = temp_dir.path().join(".env");

        let env_content = r#"URL=https://example.com?foo=bar&baz=qux
JSON={"key": "value"}
MULTILINE=line1\nline2"#;

        fs::write(&env_path, env_content).unwrap();

        let loader = EnvLoader::new(&env_path).with_separate_variables(true);
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 3);
        assert!(docs[0].page_content.contains("https://example.com"));
    }

    #[tokio::test]
    async fn test_hcl_detect_block_start() {
        // Test the internal detect_block_start function
        assert!(HCLLoader::detect_block_start(r#"resource "aws_instance" "web" {"#).is_some());
        assert!(HCLLoader::detect_block_start(r#"data "aws_ami" "ubuntu" {"#).is_some());
        assert!(HCLLoader::detect_block_start(r#"variable "region" {"#).is_some());
        assert!(HCLLoader::detect_block_start(r#"output "id" {"#).is_some());
        assert!(HCLLoader::detect_block_start(r#"module "vpc" {"#).is_some());
        assert!(HCLLoader::detect_block_start(r#"provider "aws" {"#).is_some());
        assert!(HCLLoader::detect_block_start("random_text").is_none());
        assert!(HCLLoader::detect_block_start("").is_none());
    }

    #[tokio::test]
    async fn test_nix_is_definition() {
        // Test the internal is_definition function
        assert!(NixLoader::is_definition("name = value;"));
        assert!(NixLoader::is_definition("my_var = 123"));
        assert!(NixLoader::is_definition("_private = true"));
        assert!(!NixLoader::is_definition("123invalid = x"));
        assert!(!NixLoader::is_definition("no equals here"));
        assert!(!NixLoader::is_definition(""));
    }

    #[tokio::test]
    async fn test_starlark_is_build_rule() {
        // Test the internal is_build_rule function
        assert!(StarlarkLoader::is_build_rule("cc_library("));
        assert!(StarlarkLoader::is_build_rule("rust_binary("));
        assert!(StarlarkLoader::is_build_rule("load("));
        assert!(StarlarkLoader::is_build_rule("my_custom_rule("));
        assert!(!StarlarkLoader::is_build_rule("123invalid("));
        assert!(!StarlarkLoader::is_build_rule("no parens"));
        assert!(!StarlarkLoader::is_build_rule(""));
    }

    #[tokio::test]
    async fn test_jsonnet_extract_definition_name() {
        assert_eq!(
            JsonnetLoader::extract_definition_name("local name = 123;"),
            "name"
        );
        assert_eq!(
            JsonnetLoader::extract_definition_name("local func(x, y) = x + y;"),
            "func"
        );
        assert_eq!(
            JsonnetLoader::extract_definition_name("local config = {"),
            "config"
        );
    }

    #[tokio::test]
    async fn test_dhall_extract_binding_name() {
        assert_eq!(
            DhallLoader::extract_binding_name("let name = value"),
            "name"
        );
        assert_eq!(
            DhallLoader::extract_binding_name("let config : Type = value"),
            "config"
        );
        assert_eq!(DhallLoader::extract_binding_name("let x = 1 in x"), "x");
    }
}
