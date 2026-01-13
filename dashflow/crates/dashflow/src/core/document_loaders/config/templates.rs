//! Template language loaders for various templating engines.
//!
//! This module provides document loaders for popular template languages including:
//! - Jinja2 (Python templating)
//! - Mustache (logic-less templates)
//! - Handlebars (Mustache with helpers)
//! - Pug/Jade (indentation-based)
//! - ERB (Embedded Ruby)
//! - Liquid (Shopify/Jekyll)
//!
//! Each loader can optionally separate template files by blocks, sections, or other structural elements.
//!
//! © 2025 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

use async_trait::async_trait;
use std::path::{Path, PathBuf};

use crate::core::documents::{Blob, Document, DocumentLoader};
use crate::core::error::Result;

/// Loader for Jinja2 template files (.j2, .jinja, .jinja2).
///
/// Jinja2 is a modern templating language for Python, created by Armin Ronacher in 2008.
/// Inspired by Django's templating system but more flexible. Widely used in Flask, Ansible, Salt.
///
/// # Example
///
/// ```rust,no_run
/// # use dashflow::core::document_loaders::Jinja2Loader;
/// # use dashflow::core::documents::DocumentLoader;
/// # async fn example() -> dashflow::core::error::Result<()> {
/// let loader = Jinja2Loader::new("template.j2");
/// let docs = loader.load().await?;
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone)]
pub struct Jinja2Loader {
    file_path: PathBuf,
    separate_blocks: bool,
}

impl Jinja2Loader {
    /// Create a new Jinja2 loader for the given file path.
    pub fn new<P: AsRef<Path>>(path: P) -> Self {
        Self {
            file_path: path.as_ref().to_path_buf(),
            separate_blocks: false,
        }
    }

    /// Enable separation by template blocks ({% block name %}, {% macro name %}).
    #[must_use]
    pub const fn with_separate_blocks(mut self) -> Self {
        self.separate_blocks = true;
        self
    }
}

#[async_trait]
impl DocumentLoader for Jinja2Loader {
    async fn load(&self) -> Result<Vec<Document>> {
        let blob = Blob::from_path(&self.file_path);
        let content = blob.as_string()?;

        if !self.separate_blocks {
            return Ok(vec![Document::new(&content)
                .with_metadata("source", self.file_path.display().to_string())
                .with_metadata("format", "jinja2")]);
        }

        // Separate by blocks and macros
        let lines: Vec<&str> = content.lines().collect();
        let mut documents = Vec::new();
        let mut i = 0;
        let mut block_index = 0;

        while i < lines.len() {
            let line = lines[i];
            let trimmed = line.trim();

            // Detect block start: {% block name %} or {% macro name(...) %}
            if let Some(block_info) = Self::detect_block_start(trimmed) {
                let (block_type, block_name) = block_info;
                let mut block_lines = vec![lines[i]];
                i += 1;

                // Collect until {% endblock %} or {% endmacro %}
                let end_tag = format!("{{% end{block_type} %}}");

                while i < lines.len() {
                    let next_line = lines[i];
                    block_lines.push(next_line);

                    if next_line.trim().contains(&end_tag) {
                        i += 1;
                        break;
                    }

                    i += 1;
                }

                let block_content = block_lines.join("\n");
                documents.push(
                    Document::new(&block_content)
                        .with_metadata("source", self.file_path.display().to_string())
                        .with_metadata("format", "jinja2")
                        .with_metadata("block_index", block_index.to_string())
                        .with_metadata("block_type", block_type)
                        .with_metadata("block_name", block_name),
                );
                block_index += 1;
            } else {
                i += 1;
            }
        }

        if documents.is_empty() {
            Ok(vec![Document::new(&content)
                .with_metadata("source", self.file_path.display().to_string())
                .with_metadata("format", "jinja2")])
        } else {
            Ok(documents)
        }
    }
}

impl Jinja2Loader {
    /// Detect block or macro start, return (type, name)
    fn detect_block_start(line: &str) -> Option<(String, String)> {
        // {% block name %}
        if line.contains("{%") && line.contains("block ") {
            if let Some(start) = line.find("block ") {
                let after_block = &line[start + 6..];
                if let Some(end) = after_block.find("%}") {
                    let name = after_block[..end].trim().to_string();
                    return Some(("block".to_string(), name));
                }
            }
        }

        // {% macro name(...) %}
        if line.contains("{%") && line.contains("macro ") {
            if let Some(start) = line.find("macro ") {
                let after_macro = &line[start + 6..];
                if let Some(paren_pos) = after_macro.find('(') {
                    let name = after_macro[..paren_pos].trim().to_string();
                    return Some(("macro".to_string(), name));
                }
            }
        }

        None
    }
}

/// Loader for Mustache template files (.mustache).
///
/// Mustache is a logic-less template language created by Chris Wanstrath in 2009.
/// Used across many languages (JavaScript, Ruby, Python, etc.). No logic, just placeholders.
///
/// # Example
///
/// ```rust,no_run
/// # use dashflow::core::document_loaders::MustacheLoader;
/// # use dashflow::core::documents::DocumentLoader;
/// # async fn example() -> dashflow::core::error::Result<()> {
/// let loader = MustacheLoader::new("template.mustache");
/// let docs = loader.load().await?;
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone)]
pub struct MustacheLoader {
    file_path: PathBuf,
    separate_sections: bool,
}

impl MustacheLoader {
    /// Create a new Mustache loader for the given file path.
    pub fn new<P: AsRef<Path>>(path: P) -> Self {
        Self {
            file_path: path.as_ref().to_path_buf(),
            separate_sections: false,
        }
    }

    /// Enable separation by sections ({{#section}}).
    #[must_use]
    pub const fn with_separate_sections(mut self) -> Self {
        self.separate_sections = true;
        self
    }
}

#[async_trait]
impl DocumentLoader for MustacheLoader {
    async fn load(&self) -> Result<Vec<Document>> {
        let blob = Blob::from_path(&self.file_path);
        let content = blob.as_string()?;

        if !self.separate_sections {
            return Ok(vec![Document::new(&content)
                .with_metadata("source", self.file_path.display().to_string())
                .with_metadata("format", "mustache")]);
        }

        // Separate by sections
        let lines: Vec<&str> = content.lines().collect();
        let mut documents = Vec::new();
        let mut i = 0;
        let mut section_index = 0;

        while i < lines.len() {
            let line = lines[i];

            // Detect section start: {{#name}} or {{^name}} (inverted)
            if let Some(section_name) = Self::detect_section_start(line) {
                let mut section_lines = vec![lines[i]];
                let end_tag = format!("{{{{/{section_name}}}}}");
                i += 1;

                // Collect until {{/name}}
                while i < lines.len() {
                    let next_line = lines[i];
                    section_lines.push(next_line);

                    if next_line.contains(&end_tag) {
                        i += 1;
                        break;
                    }

                    i += 1;
                }

                let section_content = section_lines.join("\n");
                documents.push(
                    Document::new(&section_content)
                        .with_metadata("source", self.file_path.display().to_string())
                        .with_metadata("format", "mustache")
                        .with_metadata("section_index", section_index.to_string())
                        .with_metadata("section_name", section_name),
                );
                section_index += 1;
            } else {
                i += 1;
            }
        }

        if documents.is_empty() {
            Ok(vec![Document::new(&content)
                .with_metadata("source", self.file_path.display().to_string())
                .with_metadata("format", "mustache")])
        } else {
            Ok(documents)
        }
    }
}

impl MustacheLoader {
    /// Detect section start {{#name}} or {{^name}}, return name
    fn detect_section_start(line: &str) -> Option<String> {
        // {{#name}} or {{^name}}
        if let Some(start) = line.find("{{#").or_else(|| line.find("{{^")) {
            let after_start = &line[start + 3..];
            if let Some(end) = after_start.find("}}") {
                let name = after_start[..end].trim().to_string();
                return Some(name);
            }
        }
        None
    }
}

/// Loader for Handlebars template files (.hbs, .handlebars).
///
/// Handlebars is a minimal logic template language created by Yehuda Katz in 2010.
/// Extension of Mustache with helpers, partials, and block expressions. Used in Node.js ecosystem.
///
/// # Example
///
/// ```rust,no_run
/// # use dashflow::core::document_loaders::HandlebarsLoader;
/// # use dashflow::core::documents::DocumentLoader;
/// # async fn example() -> dashflow::core::error::Result<()> {
/// let loader = HandlebarsLoader::new("template.hbs");
/// let docs = loader.load().await?;
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone)]
pub struct HandlebarsLoader {
    file_path: PathBuf,
    separate_blocks: bool,
}

impl HandlebarsLoader {
    /// Create a new Handlebars loader for the given file path.
    pub fn new<P: AsRef<Path>>(path: P) -> Self {
        Self {
            file_path: path.as_ref().to_path_buf(),
            separate_blocks: false,
        }
    }

    /// Enable separation by blocks and sections.
    #[must_use]
    pub const fn with_separate_blocks(mut self) -> Self {
        self.separate_blocks = true;
        self
    }
}

#[async_trait]
impl DocumentLoader for HandlebarsLoader {
    async fn load(&self) -> Result<Vec<Document>> {
        let blob = Blob::from_path(&self.file_path);
        let content = blob.as_string()?;

        if !self.separate_blocks {
            return Ok(vec![Document::new(&content)
                .with_metadata("source", self.file_path.display().to_string())
                .with_metadata("format", "handlebars")]);
        }

        // Separate by blocks and sections
        let lines: Vec<&str> = content.lines().collect();
        let mut documents = Vec::new();
        let mut i = 0;
        let mut block_index = 0;

        while i < lines.len() {
            let line = lines[i];

            // Detect block/section start: {{#name}}, {{#if}}, {{#each}}, {{#with}}, {{#unless}}
            if let Some((block_type, block_name)) = Self::detect_block_start(line) {
                let mut block_lines = vec![lines[i]];
                let end_tag = format!("{{{{/{block_name}}}}}");
                i += 1;

                // Collect until {{/name}}
                while i < lines.len() {
                    let next_line = lines[i];
                    block_lines.push(next_line);

                    if next_line.contains(&end_tag) {
                        i += 1;
                        break;
                    }

                    i += 1;
                }

                let block_content = block_lines.join("\n");
                documents.push(
                    Document::new(&block_content)
                        .with_metadata("source", self.file_path.display().to_string())
                        .with_metadata("format", "handlebars")
                        .with_metadata("block_index", block_index.to_string())
                        .with_metadata("block_type", block_type)
                        .with_metadata("block_name", block_name),
                );
                block_index += 1;
            } else {
                i += 1;
            }
        }

        if documents.is_empty() {
            Ok(vec![Document::new(&content)
                .with_metadata("source", self.file_path.display().to_string())
                .with_metadata("format", "handlebars")])
        } else {
            Ok(documents)
        }
    }
}

impl HandlebarsLoader {
    /// Detect block start {{#name}}, return (type, name)
    fn detect_block_start(line: &str) -> Option<(String, String)> {
        // {{#if}}, {{#each}}, {{#with}}, {{#unless}}, {{#name}}
        if let Some(start) = line.find("{{#") {
            let after_start = &line[start + 3..];
            if let Some(end) = after_start.find(|c: char| c == '}' || c.is_whitespace()) {
                let name = after_start[..end].trim().to_string();

                // Determine type
                let block_type = match name.as_str() {
                    "if" | "each" | "with" | "unless" => name.clone(),
                    _ => "section".to_string(),
                };

                return Some((block_type, name));
            }
        }
        None
    }
}

/// Loader for Pug/Jade template files (.pug, .jade).
///
/// Pug (formerly Jade) is a high-performance template engine created by TJ Holowaychuk in 2010.
/// Heavily influenced by Haml, features indentation-based syntax (no closing tags).
/// Used in Node.js ecosystem, particularly with Express.js.
///
/// # Example
///
/// ```rust,no_run
/// # use dashflow::core::document_loaders::PugLoader;
/// # use dashflow::core::documents::DocumentLoader;
/// # async fn example() -> dashflow::core::error::Result<()> {
/// let loader = PugLoader::new("template.pug");
/// let docs = loader.load().await?;
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone)]
pub struct PugLoader {
    file_path: PathBuf,
    separate_blocks: bool,
}

impl PugLoader {
    /// Create a new Pug loader for the given file path.
    pub fn new<P: AsRef<Path>>(path: P) -> Self {
        Self {
            file_path: path.as_ref().to_path_buf(),
            separate_blocks: false,
        }
    }

    /// Enable separation by blocks (block name, mixin name).
    #[must_use]
    pub const fn with_separate_blocks(mut self) -> Self {
        self.separate_blocks = true;
        self
    }
}

#[async_trait]
impl DocumentLoader for PugLoader {
    async fn load(&self) -> Result<Vec<Document>> {
        let blob = Blob::from_path(&self.file_path);
        let content = blob.as_string()?;

        if !self.separate_blocks {
            return Ok(vec![Document::new(&content)
                .with_metadata("source", self.file_path.display().to_string())
                .with_metadata("format", "pug")]);
        }

        // Separate by blocks and mixins
        let lines: Vec<&str> = content.lines().collect();
        let mut documents = Vec::new();
        let mut i = 0;
        let mut block_index = 0;

        while i < lines.len() {
            let line = lines[i];
            let trimmed = line.trim();

            // Detect block or mixin definition
            if let Some(block_info) = Self::detect_block_start(trimmed) {
                let (block_type, block_name) = block_info;
                let block_lines = vec![lines[i]];
                let base_indent = line.len() - trimmed.len();
                i += 1;

                // Collect subsequent lines with greater indentation
                let mut block_content_lines = block_lines;
                while i < lines.len() {
                    let next_line = lines[i];
                    let next_trimmed = next_line.trim();

                    // Stop if we hit a non-empty line at same or less indentation
                    if !next_trimmed.is_empty() {
                        let next_indent = next_line.len() - next_trimmed.len();
                        if next_indent <= base_indent {
                            break;
                        }
                    }

                    block_content_lines.push(next_line);
                    i += 1;
                }

                let block_content = block_content_lines.join("\n");
                documents.push(
                    Document::new(&block_content)
                        .with_metadata("source", self.file_path.display().to_string())
                        .with_metadata("format", "pug")
                        .with_metadata("block_index", block_index.to_string())
                        .with_metadata("block_type", block_type)
                        .with_metadata("block_name", block_name),
                );
                block_index += 1;
            } else {
                i += 1;
            }
        }

        if documents.is_empty() {
            Ok(vec![Document::new(&content)
                .with_metadata("source", self.file_path.display().to_string())
                .with_metadata("format", "pug")])
        } else {
            Ok(documents)
        }
    }
}

impl PugLoader {
    /// Detect block or mixin start, return (type, name)
    fn detect_block_start(line: &str) -> Option<(String, String)> {
        // block name
        if let Some(stripped) = line.strip_prefix("block ") {
            let name = stripped.trim().to_string();
            return Some(("block".to_string(), name));
        }

        // mixin name(args)
        if let Some(after_mixin) = line.strip_prefix("mixin ") {
            if let Some(paren_pos) = after_mixin.find('(') {
                let name = after_mixin[..paren_pos].trim().to_string();
                return Some(("mixin".to_string(), name));
            } else {
                // mixin without args
                let name = after_mixin.trim().to_string();
                return Some(("mixin".to_string(), name));
            }
        }

        None
    }
}

/// Loader for ERB (Embedded Ruby) template files (.erb, .html.erb).
///
/// ERB is Ruby's templating system, part of the Ruby standard library since 1999.
/// Used extensively in Ruby on Rails for views and templates.
/// Embeds Ruby code within HTML using <% %> tags.
///
/// # Example
///
/// ```rust,no_run
/// # use dashflow::core::document_loaders::ERBLoader;
/// # use dashflow::core::documents::DocumentLoader;
/// # async fn example() -> dashflow::core::error::Result<()> {
/// let loader = ERBLoader::new("template.erb");
/// let docs = loader.load().await?;
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone)]
pub struct ERBLoader {
    file_path: PathBuf,
    separate_blocks: bool,
}

impl ERBLoader {
    /// Create a new ERB loader for the given file path.
    pub fn new<P: AsRef<Path>>(path: P) -> Self {
        Self {
            file_path: path.as_ref().to_path_buf(),
            separate_blocks: false,
        }
    }

    /// Enable separation by Ruby code blocks (methods, classes).
    #[must_use]
    pub const fn with_separate_blocks(mut self) -> Self {
        self.separate_blocks = true;
        self
    }
}

#[async_trait]
impl DocumentLoader for ERBLoader {
    async fn load(&self) -> Result<Vec<Document>> {
        let blob = Blob::from_path(&self.file_path);
        let content = blob.as_string()?;

        if !self.separate_blocks {
            return Ok(vec![Document::new(&content)
                .with_metadata("source", self.file_path.display().to_string())
                .with_metadata("format", "erb")]);
        }

        // Separate by Ruby code blocks (methods, blocks with do...end)
        let lines: Vec<&str> = content.lines().collect();
        let mut documents = Vec::new();
        let mut i = 0;
        let mut block_index = 0;

        while i < lines.len() {
            let line = lines[i];

            // Detect Ruby block start: <% def method_name %> or <% something do %>
            if let Some(block_info) = Self::detect_block_start(line) {
                let (block_type, block_name) = block_info;
                let mut block_lines = vec![lines[i]];
                i += 1;

                // Collect until <% end %>
                while i < lines.len() {
                    let next_line = lines[i];
                    block_lines.push(next_line);

                    // Check for <% end %>
                    if next_line.contains("<%")
                        && next_line.contains("end")
                        && next_line.contains("%>")
                    {
                        i += 1;
                        break;
                    }

                    i += 1;
                }

                let block_content = block_lines.join("\n");
                documents.push(
                    Document::new(&block_content)
                        .with_metadata("source", self.file_path.display().to_string())
                        .with_metadata("format", "erb")
                        .with_metadata("block_index", block_index.to_string())
                        .with_metadata("block_type", block_type)
                        .with_metadata("block_name", block_name),
                );
                block_index += 1;
            } else {
                i += 1;
            }
        }

        if documents.is_empty() {
            Ok(vec![Document::new(&content)
                .with_metadata("source", self.file_path.display().to_string())
                .with_metadata("format", "erb")])
        } else {
            Ok(documents)
        }
    }
}

impl ERBLoader {
    /// Detect Ruby block start, return (type, name)
    fn detect_block_start(line: &str) -> Option<(String, String)> {
        // <% def method_name %>
        if line.contains("<%") && line.contains("def ") {
            if let Some(start) = line.find("def ") {
                let after_def = &line[start + 4..];
                let name = if let Some(paren_pos) = after_def.find('(') {
                    after_def[..paren_pos].trim().to_string()
                } else if let Some(end) = after_def.find("%>") {
                    after_def[..end].trim().to_string()
                } else {
                    after_def.split_whitespace().next()?.to_string()
                };
                return Some(("method".to_string(), name));
            }
        }

        // <% something do %> or <% something do |var| %>
        if line.contains("<%") && line.contains(" do") {
            // Extract the variable/expression before "do"
            if let Some(start) = line.find("<%") {
                let after_start = &line[start + 2..];
                if let Some(do_pos) = after_start.find(" do") {
                    let expression = after_start[..do_pos].trim();
                    // Take the last word as the name
                    let name = expression.split_whitespace().last()?.to_string();
                    return Some(("block".to_string(), name));
                }
            }
        }

        None
    }
}

/// Loader for Liquid template files (.liquid, .html.liquid).
///
/// Liquid is a template language created by Tobias Lütke (Shopify) in 2006.
/// Open-source, safe, customer-facing template language used by Shopify, Jekyll, GitHub Pages.
/// Ruby-based but portable, focuses on safety (no arbitrary code execution).
///
/// # Example
///
/// ```rust,no_run
/// # use dashflow::core::document_loaders::LiquidLoader;
/// # use dashflow::core::documents::DocumentLoader;
/// # async fn example() -> dashflow::core::error::Result<()> {
/// let loader = LiquidLoader::new("template.liquid");
/// let docs = loader.load().await?;
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone)]
pub struct LiquidLoader {
    file_path: PathBuf,
    separate_blocks: bool,
}

impl LiquidLoader {
    /// Create a new Liquid loader for the given file path.
    pub fn new<P: AsRef<Path>>(path: P) -> Self {
        Self {
            file_path: path.as_ref().to_path_buf(),
            separate_blocks: false,
        }
    }

    /// Enable separation by blocks ({% block name %}, {% for %}, {% if %}).
    #[must_use]
    pub const fn with_separate_blocks(mut self) -> Self {
        self.separate_blocks = true;
        self
    }
}

#[async_trait]
impl DocumentLoader for LiquidLoader {
    async fn load(&self) -> Result<Vec<Document>> {
        let blob = Blob::from_path(&self.file_path);
        let content = blob.as_string()?;

        if !self.separate_blocks {
            return Ok(vec![Document::new(&content)
                .with_metadata("source", self.file_path.display().to_string())
                .with_metadata("format", "liquid")]);
        }

        // Separate by control flow blocks
        let lines: Vec<&str> = content.lines().collect();
        let mut documents = Vec::new();
        let mut i = 0;
        let mut block_index = 0;

        while i < lines.len() {
            let line = lines[i];

            // Detect block start: {% if %}, {% for %}, {% case %}, {% block %}
            if let Some(block_info) = Self::detect_block_start(line) {
                let (block_type, block_name) = block_info;
                let mut block_lines = vec![lines[i]];
                i += 1;

                // Collect until corresponding end tag
                let end_tag = format!("{{% end{block_type} %}}");

                while i < lines.len() {
                    let next_line = lines[i];
                    block_lines.push(next_line);

                    if next_line.contains(&end_tag) {
                        i += 1;
                        break;
                    }

                    i += 1;
                }

                let block_content = block_lines.join("\n");
                documents.push(
                    Document::new(&block_content)
                        .with_metadata("source", self.file_path.display().to_string())
                        .with_metadata("format", "liquid")
                        .with_metadata("block_index", block_index.to_string())
                        .with_metadata("block_type", block_type.clone())
                        .with_metadata("block_name", block_name),
                );
                block_index += 1;
            } else {
                i += 1;
            }
        }

        if documents.is_empty() {
            Ok(vec![Document::new(&content)
                .with_metadata("source", self.file_path.display().to_string())
                .with_metadata("format", "liquid")])
        } else {
            Ok(documents)
        }
    }
}

impl LiquidLoader {
    /// Detect block start, return (type, name)
    fn detect_block_start(line: &str) -> Option<(String, String)> {
        // {% if condition %}
        if line.contains("{%") && line.contains("if ") {
            if let Some(start) = line.find("if ") {
                let after_if = &line[start + 3..];
                if let Some(end) = after_if.find("%}") {
                    let condition = after_if[..end].trim().to_string();
                    return Some(("if".to_string(), condition));
                }
            }
        }

        // {% for item in collection %}
        if line.contains("{%") && line.contains("for ") {
            if let Some(start) = line.find("for ") {
                let after_for = &line[start + 4..];
                if let Some(end) = after_for.find("%}") {
                    let expression = after_for[..end].trim();
                    // Extract variable name (before "in")
                    let name = if let Some(in_pos) = expression.find(" in ") {
                        expression[..in_pos].trim().to_string()
                    } else {
                        expression.to_string()
                    };
                    return Some(("for".to_string(), name));
                }
            }
        }

        // {% case variable %}
        if line.contains("{%") && line.contains("case ") {
            if let Some(start) = line.find("case ") {
                let after_case = &line[start + 5..];
                if let Some(end) = after_case.find("%}") {
                    let variable = after_case[..end].trim().to_string();
                    return Some(("case".to_string(), variable));
                }
            }
        }

        // {% block name %}
        if line.contains("{%") && line.contains("block ") {
            if let Some(start) = line.find("block ") {
                let after_block = &line[start + 6..];
                if let Some(end) = after_block.find("%}") {
                    let name = after_block[..end].trim().to_string();
                    return Some(("block".to_string(), name));
                }
            }
        }

        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    // ============================================================================
    // Jinja2Loader Tests
    // ============================================================================

    #[tokio::test]
    async fn test_jinja2_loader_basic() {
        let mut file = NamedTempFile::new().unwrap();
        write!(file, "<html>{{{{ name }}}}</html>").unwrap();

        let loader = Jinja2Loader::new(file.path());
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1);
        assert!(docs[0].page_content.contains("{{ name }}"));
        assert_eq!(docs[0].metadata.get("format").unwrap(), "jinja2");
    }

    #[tokio::test]
    async fn test_jinja2_loader_separate_blocks() {
        let mut file = NamedTempFile::new().unwrap();
        write!(
            file,
            "{{% block header %}}\n<h1>Title</h1>\n{{% endblock %}}\n{{% block content %}}\n<p>Body</p>\n{{% endblock %}}"
        )
        .unwrap();

        let loader = Jinja2Loader::new(file.path()).with_separate_blocks();
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 2);
        assert_eq!(docs[0].metadata.get("block_name").unwrap(), "header");
        assert_eq!(docs[0].metadata.get("block_type").unwrap(), "block");
        assert_eq!(docs[1].metadata.get("block_name").unwrap(), "content");
    }

    #[tokio::test]
    async fn test_jinja2_loader_macro() {
        let mut file = NamedTempFile::new().unwrap();
        write!(
            file,
            "{{% macro input(name, value) %}}\n<input name=\"{{ name }}\" value=\"{{ value }}\">\n{{% endmacro %}}"
        )
        .unwrap();

        let loader = Jinja2Loader::new(file.path()).with_separate_blocks();
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1);
        assert_eq!(docs[0].metadata.get("block_type").unwrap(), "macro");
        assert_eq!(docs[0].metadata.get("block_name").unwrap(), "input");
    }

    #[tokio::test]
    async fn test_jinja2_loader_metadata() {
        let mut file = NamedTempFile::new().unwrap();
        write!(file, "test").unwrap();

        let loader = Jinja2Loader::new(file.path());
        let docs = loader.load().await.unwrap();

        assert!(docs[0].metadata.contains_key("source"));
        assert_eq!(docs[0].metadata.get("format").unwrap(), "jinja2");
    }

    #[tokio::test]
    async fn test_jinja2_loader_empty_file() {
        let file = NamedTempFile::new().unwrap();

        let loader = Jinja2Loader::new(file.path());
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1);
        assert!(docs[0].page_content.is_empty());
    }

    // ============================================================================
    // MustacheLoader Tests
    // ============================================================================

    #[tokio::test]
    async fn test_mustache_loader_basic() {
        let mut file = NamedTempFile::new().unwrap();
        write!(file, "<p>{{{{name}}}}</p>").unwrap();

        let loader = MustacheLoader::new(file.path());
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1);
        assert!(docs[0].page_content.contains("{{name}}"));
        assert_eq!(docs[0].metadata.get("format").unwrap(), "mustache");
    }

    #[tokio::test]
    async fn test_mustache_loader_separate_sections() {
        let mut file = NamedTempFile::new().unwrap();
        write!(
            file,
            "{{{{#users}}}}\n<li>{{{{name}}}}</li>\n{{{{/users}}}}\n{{{{#items}}}}\n<span>{{{{title}}}}</span>\n{{{{/items}}}}"
        )
        .unwrap();

        let loader = MustacheLoader::new(file.path()).with_separate_sections();
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 2);
        assert_eq!(docs[0].metadata.get("section_name").unwrap(), "users");
        assert_eq!(docs[1].metadata.get("section_name").unwrap(), "items");
    }

    #[tokio::test]
    async fn test_mustache_loader_inverted_section() {
        let mut file = NamedTempFile::new().unwrap();
        write!(file, "{{{{^empty}}}}\n<p>Not empty</p>\n{{{{/empty}}}}").unwrap();

        let loader = MustacheLoader::new(file.path()).with_separate_sections();
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1);
        assert_eq!(docs[0].metadata.get("section_name").unwrap(), "empty");
    }

    #[tokio::test]
    async fn test_mustache_loader_metadata() {
        let mut file = NamedTempFile::new().unwrap();
        write!(file, "test").unwrap();

        let loader = MustacheLoader::new(file.path());
        let docs = loader.load().await.unwrap();

        assert!(docs[0].metadata.contains_key("source"));
        assert_eq!(docs[0].metadata.get("format").unwrap(), "mustache");
    }

    #[tokio::test]
    async fn test_mustache_loader_empty_file() {
        let file = NamedTempFile::new().unwrap();

        let loader = MustacheLoader::new(file.path());
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1);
        assert!(docs[0].page_content.is_empty());
    }

    // ============================================================================
    // HandlebarsLoader Tests
    // ============================================================================

    #[tokio::test]
    async fn test_handlebars_loader_basic() {
        let mut file = NamedTempFile::new().unwrap();
        write!(file, "<p>{{{{name}}}}</p>").unwrap();

        let loader = HandlebarsLoader::new(file.path());
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1);
        assert!(docs[0].page_content.contains("{{name}}"));
        assert_eq!(docs[0].metadata.get("format").unwrap(), "handlebars");
    }

    #[tokio::test]
    async fn test_handlebars_loader_separate_blocks() {
        let mut file = NamedTempFile::new().unwrap();
        write!(
            file,
            "{{{{#each items}}}}\n<li>{{{{this}}}}</li>\n{{{{/each}}}}\n{{{{#if show}}}}\n<p>Visible</p>\n{{{{/if}}}}"
        )
        .unwrap();

        let loader = HandlebarsLoader::new(file.path()).with_separate_blocks();
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 2);
        assert_eq!(docs[0].metadata.get("block_type").unwrap(), "each");
        assert_eq!(docs[1].metadata.get("block_type").unwrap(), "if");
    }

    #[tokio::test]
    async fn test_handlebars_loader_unless() {
        let mut file = NamedTempFile::new().unwrap();
        write!(
            file,
            "{{{{#unless hidden}}}}\n<p>Not hidden</p>\n{{{{/unless}}}}"
        )
        .unwrap();

        let loader = HandlebarsLoader::new(file.path()).with_separate_blocks();
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1);
        assert_eq!(docs[0].metadata.get("block_type").unwrap(), "unless");
    }

    #[tokio::test]
    async fn test_handlebars_loader_metadata() {
        let mut file = NamedTempFile::new().unwrap();
        write!(file, "test").unwrap();

        let loader = HandlebarsLoader::new(file.path());
        let docs = loader.load().await.unwrap();

        assert!(docs[0].metadata.contains_key("source"));
        assert_eq!(docs[0].metadata.get("format").unwrap(), "handlebars");
    }

    #[tokio::test]
    async fn test_handlebars_loader_empty_file() {
        let file = NamedTempFile::new().unwrap();

        let loader = HandlebarsLoader::new(file.path());
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1);
        assert!(docs[0].page_content.is_empty());
    }

    // ============================================================================
    // PugLoader Tests
    // ============================================================================

    #[tokio::test]
    async fn test_pug_loader_basic() {
        let mut file = NamedTempFile::new().unwrap();
        write!(file, "html\n  body\n    h1 Title").unwrap();

        let loader = PugLoader::new(file.path());
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1);
        assert!(docs[0].page_content.contains("h1 Title"));
        assert_eq!(docs[0].metadata.get("format").unwrap(), "pug");
    }

    #[tokio::test]
    async fn test_pug_loader_separate_blocks() {
        let mut file = NamedTempFile::new().unwrap();
        write!(
            file,
            "block header\n  h1 Header\nblock content\n  p Content"
        )
        .unwrap();

        let loader = PugLoader::new(file.path()).with_separate_blocks();
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 2);
        assert_eq!(docs[0].metadata.get("block_name").unwrap(), "header");
        assert_eq!(docs[1].metadata.get("block_name").unwrap(), "content");
    }

    #[tokio::test]
    async fn test_pug_loader_mixin() {
        let mut file = NamedTempFile::new().unwrap();
        write!(
            file,
            "mixin list(items)\n  ul\n    each item in items\n      li= item"
        )
        .unwrap();

        let loader = PugLoader::new(file.path()).with_separate_blocks();
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1);
        assert_eq!(docs[0].metadata.get("block_type").unwrap(), "mixin");
        assert_eq!(docs[0].metadata.get("block_name").unwrap(), "list");
    }

    #[tokio::test]
    async fn test_pug_loader_metadata() {
        let mut file = NamedTempFile::new().unwrap();
        write!(file, "p test").unwrap();

        let loader = PugLoader::new(file.path());
        let docs = loader.load().await.unwrap();

        assert!(docs[0].metadata.contains_key("source"));
        assert_eq!(docs[0].metadata.get("format").unwrap(), "pug");
    }

    #[tokio::test]
    async fn test_pug_loader_empty_file() {
        let file = NamedTempFile::new().unwrap();

        let loader = PugLoader::new(file.path());
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1);
        assert!(docs[0].page_content.is_empty());
    }

    // ============================================================================
    // ERBLoader Tests
    // ============================================================================

    #[tokio::test]
    async fn test_erb_loader_basic() {
        let mut file = NamedTempFile::new().unwrap();
        write!(file, "<p><%= @name %></p>").unwrap();

        let loader = ERBLoader::new(file.path());
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1);
        assert!(docs[0].page_content.contains("<%= @name %>"));
        assert_eq!(docs[0].metadata.get("format").unwrap(), "erb");
    }

    #[tokio::test]
    async fn test_erb_loader_separate_blocks() {
        let mut file = NamedTempFile::new().unwrap();
        write!(
            file,
            "<% def greet(name) %>\n  Hello, <%= name %>\n<% end %>"
        )
        .unwrap();

        let loader = ERBLoader::new(file.path()).with_separate_blocks();
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1);
        assert_eq!(docs[0].metadata.get("block_type").unwrap(), "method");
        assert_eq!(docs[0].metadata.get("block_name").unwrap(), "greet");
    }

    #[tokio::test]
    async fn test_erb_loader_do_block() {
        let mut file = NamedTempFile::new().unwrap();
        write!(
            file,
            "<% @items.each do |item| %>\n  <li><%= item %></li>\n<% end %>"
        )
        .unwrap();

        let loader = ERBLoader::new(file.path()).with_separate_blocks();
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1);
        assert_eq!(docs[0].metadata.get("block_type").unwrap(), "block");
    }

    #[tokio::test]
    async fn test_erb_loader_metadata() {
        let mut file = NamedTempFile::new().unwrap();
        write!(file, "test").unwrap();

        let loader = ERBLoader::new(file.path());
        let docs = loader.load().await.unwrap();

        assert!(docs[0].metadata.contains_key("source"));
        assert_eq!(docs[0].metadata.get("format").unwrap(), "erb");
    }

    #[tokio::test]
    async fn test_erb_loader_empty_file() {
        let file = NamedTempFile::new().unwrap();

        let loader = ERBLoader::new(file.path());
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1);
        assert!(docs[0].page_content.is_empty());
    }

    // ============================================================================
    // LiquidLoader Tests
    // ============================================================================

    #[tokio::test]
    async fn test_liquid_loader_basic() {
        let mut file = NamedTempFile::new().unwrap();
        write!(file, "<p>{{{{ name }}}}</p>").unwrap();

        let loader = LiquidLoader::new(file.path());
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1);
        assert!(docs[0].page_content.contains("{{ name }}"));
        assert_eq!(docs[0].metadata.get("format").unwrap(), "liquid");
    }

    #[tokio::test]
    async fn test_liquid_loader_separate_blocks_if() {
        let mut file = NamedTempFile::new().unwrap();
        write!(
            file,
            "{{% if user %}}\n  <p>Hello {{ user.name }}</p>\n{{% endif %}}"
        )
        .unwrap();

        let loader = LiquidLoader::new(file.path()).with_separate_blocks();
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1);
        assert_eq!(docs[0].metadata.get("block_type").unwrap(), "if");
    }

    #[tokio::test]
    async fn test_liquid_loader_separate_blocks_for() {
        let mut file = NamedTempFile::new().unwrap();
        write!(
            file,
            "{{% for item in items %}}\n  <li>{{ item }}</li>\n{{% endfor %}}"
        )
        .unwrap();

        let loader = LiquidLoader::new(file.path()).with_separate_blocks();
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1);
        assert_eq!(docs[0].metadata.get("block_type").unwrap(), "for");
        assert_eq!(docs[0].metadata.get("block_name").unwrap(), "item");
    }

    #[tokio::test]
    async fn test_liquid_loader_case() {
        let mut file = NamedTempFile::new().unwrap();
        write!(
            file,
            "{{% case status %}}\n{{% when 'active' %}}\nActive\n{{% endcase %}}"
        )
        .unwrap();

        let loader = LiquidLoader::new(file.path()).with_separate_blocks();
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1);
        assert_eq!(docs[0].metadata.get("block_type").unwrap(), "case");
        assert_eq!(docs[0].metadata.get("block_name").unwrap(), "status");
    }

    #[tokio::test]
    async fn test_liquid_loader_metadata() {
        let mut file = NamedTempFile::new().unwrap();
        write!(file, "test").unwrap();

        let loader = LiquidLoader::new(file.path());
        let docs = loader.load().await.unwrap();

        assert!(docs[0].metadata.contains_key("source"));
        assert_eq!(docs[0].metadata.get("format").unwrap(), "liquid");
    }

    #[tokio::test]
    async fn test_liquid_loader_empty_file() {
        let file = NamedTempFile::new().unwrap();

        let loader = LiquidLoader::new(file.path());
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1);
        assert!(docs[0].page_content.is_empty());
    }
}
