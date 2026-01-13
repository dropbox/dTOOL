//! Systems programming language document loaders.
//!
//! This module provides loaders for systems programming languages:
//! - Rust
//! - C/C++
//! - Go
//! - Swift
//! - Zig
//! - Nim
//! - Crystal
//! - D
//! - V
//! - WebAssembly (WASM)

#![allow(clippy::empty_line_after_doc_comments)]

use async_trait::async_trait;
use std::path::{Path, PathBuf};

use crate::core::documents::{Blob, Document, DocumentLoader};
use crate::core::error::Result;

/// `RustFileLoader` loads Rust source files and separates them by item definitions.
///
/// Rust is a systems programming language focused on safety, concurrency, and performance.
/// Created by Mozilla Research and first released in 2010, Rust uses ownership semantics
/// to guarantee memory safety without garbage collection.
///
/// Supports extensions: .rs
///
/// When `separate_items` is true, splits document by Rust item declarations:
/// `fn`, `struct`, `enum`, `trait`, `impl`, `mod`, `const`, `static`, `type`
///
/// # Example
///
/// ```no_run
/// use dashflow::core::document_loaders::RustFileLoader;
/// use dashflow::core::documents::DocumentLoader;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let loader = RustFileLoader::new("lib.rs").with_separate_items(true);
/// let docs = loader.load().await?;
/// println!("Loaded {} Rust items", docs.len());
/// # Ok(())
/// # }
/// ```
pub struct RustFileLoader {
    /// Path to the Rust file
    pub file_path: PathBuf,
    /// Separate documents per item (fn, struct, enum, trait, impl) (default: false)
    pub separate_items: bool,
}

impl RustFileLoader {
    /// Create a new `RustFileLoader` for the given file path.
    pub fn new(file_path: impl AsRef<Path>) -> Self {
        Self {
            file_path: file_path.as_ref().to_path_buf(),
            separate_items: false,
        }
    }

    /// Create separate documents per Rust item (fn, struct, enum, trait, impl).
    #[must_use]
    pub fn with_separate_items(mut self, separate: bool) -> Self {
        self.separate_items = separate;
        self
    }
}

#[async_trait]
impl DocumentLoader for RustFileLoader {
    async fn load(&self) -> Result<Vec<Document>> {
        let blob = Blob::from_path(&self.file_path);
        let content = blob.as_string()?;

        if self.separate_items {
            // Split by Rust item declarations
            let mut documents = Vec::new();
            let mut current_item = String::new();
            let mut item_name = String::new();
            let mut item_type = String::new();
            let mut item_index = 0;
            let mut brace_depth = 0;
            let mut in_item = false;

            for line in content.lines() {
                let trimmed = line.trim_start();

                // Detect Rust item declarations
                if !in_item
                    && (trimmed.starts_with("fn ")
                        || trimmed.starts_with("pub fn ")
                        || trimmed.starts_with("async fn ")
                        || trimmed.starts_with("pub async fn ")
                        || trimmed.starts_with("struct ")
                        || trimmed.starts_with("pub struct ")
                        || trimmed.starts_with("enum ")
                        || trimmed.starts_with("pub enum ")
                        || trimmed.starts_with("trait ")
                        || trimmed.starts_with("pub trait ")
                        || trimmed.starts_with("impl ")
                        || trimmed.starts_with("impl<"))
                {
                    in_item = true;
                    brace_depth = 0;

                    // Extract item type and name
                    let words: Vec<&str> = trimmed.split_whitespace().collect();
                    for (i, &word) in words.iter().enumerate() {
                        if word == "fn" || word == "struct" || word == "enum" || word == "trait" {
                            item_type = word.to_string();
                            if i + 1 < words.len() {
                                let name_part = words[i + 1];
                                if let Some(paren_pos) = name_part.find('(') {
                                    item_name = name_part[..paren_pos].to_string();
                                } else if let Some(angle_pos) = name_part.find('<') {
                                    item_name = name_part[..angle_pos].to_string();
                                } else {
                                    item_name = name_part.trim_end_matches(['{', ':']).to_string();
                                }
                                break;
                            }
                        } else if word == "impl" {
                            item_type = "impl".to_string();
                            // For impl blocks, extract the type name
                            if i + 1 < words.len() {
                                item_name = words[i + 1].trim_end_matches('{').to_string();
                                break;
                            }
                        }
                    }
                }

                if in_item {
                    current_item.push_str(line);
                    current_item.push('\n');

                    // Track braces
                    for ch in line.chars() {
                        if ch == '{' {
                            brace_depth += 1;
                        } else if ch == '}' {
                            brace_depth -= 1;
                            if brace_depth == 0 {
                                // Item complete
                                let doc = Document::new(current_item.clone())
                                    .with_metadata("source", self.file_path.display().to_string())
                                    .with_metadata("item_index", item_index)
                                    .with_metadata("item_name", item_name.clone())
                                    .with_metadata("item_type", item_type.clone())
                                    .with_metadata("format", "rust");

                                documents.push(doc);
                                current_item.clear();
                                in_item = false;
                                item_index += 1;
                                break;
                            }
                        }
                    }
                } else if !trimmed.is_empty() {
                    // Non-item code (use statements, attributes, comments)
                    current_item.push_str(line);
                    current_item.push('\n');
                }
            }

            // Add any remaining content
            if !current_item.is_empty() {
                let doc = Document::new(current_item)
                    .with_metadata("source", self.file_path.display().to_string())
                    .with_metadata("type", "global")
                    .with_metadata("format", "rust");

                documents.push(doc);
            }

            Ok(documents)
        } else {
            // Load as single document
            let doc = Document::new(content)
                .with_metadata("source", self.file_path.display().to_string())
                .with_metadata("format", "rust");

            Ok(vec![doc])
        }
    }
}

/// Loads Emacs org-mode files (.org).
///
/// The `OrgModeLoader` reads org-mode files, which are hierarchical plain-text notes
/// and TODO lists. Can optionally separate by top-level headings.
///
/// # Example
///
/// ```no_run
/// use dashflow::core::document_loaders::OrgModeLoader;
/// use dashflow::core::documents::DocumentLoader;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let loader = OrgModeLoader::new("notes.org");
/// let documents = loader.load().await?;
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone)]

// ============================================================================
// Go Loader
// ============================================================================

/// `GoLoader` loads Go source files and separates them by definitions.
///
/// Go is a statically typed, compiled language designed at Google. Created by
/// Robert Griesemer, Rob Pike, and Ken Thompson, first released in 2009. Go
/// emphasizes simplicity, concurrency (goroutines), and fast compilation.
///
/// Supports extensions: .go
///
/// When `separate_definitions` is true, splits document by Go declarations:
/// `func`, `type`, `struct`, `interface`, `const`, `var`
///
/// # Example
///
/// ```no_run
/// use dashflow::core::document_loaders::GoLoader;
/// use dashflow::core::documents::DocumentLoader;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let loader = GoLoader::new("main.go").with_separate_definitions(true);
/// let docs = loader.load().await?;
/// println!("Loaded {} Go definitions", docs.len());
/// # Ok(())
/// # }
/// ```
pub struct GoLoader {
    /// Path to the Go file
    pub file_path: PathBuf,
    /// Separate documents per function/type/struct (default: false)
    pub separate_definitions: bool,
}

impl GoLoader {
    /// Create a new `GoLoader` for the given file path.
    pub fn new(file_path: impl AsRef<Path>) -> Self {
        Self {
            file_path: file_path.as_ref().to_path_buf(),
            separate_definitions: false,
        }
    }

    /// Create separate documents per function/type/struct definition.
    #[must_use]
    pub fn with_separate_definitions(mut self, separate: bool) -> Self {
        self.separate_definitions = separate;
        self
    }
}

#[async_trait]
impl DocumentLoader for GoLoader {
    async fn load(&self) -> Result<Vec<Document>> {
        let blob = Blob::from_path(&self.file_path);
        let content = blob.as_string()?;

        if self.separate_definitions {
            // Split by function/type/struct definitions
            let mut documents = Vec::new();
            let mut current_def = String::new();
            let mut def_name = String::new();
            let mut def_index = 0;
            let mut brace_depth = 0;
            let mut in_definition = false;

            for line in content.lines() {
                let trimmed = line.trim_start();

                // Detect function/type/struct declarations
                if !in_definition
                    && (trimmed.starts_with("func ")
                        || trimmed.starts_with("type ")
                        || trimmed.starts_with("struct "))
                {
                    in_definition = true;
                    brace_depth = 0;

                    // Extract definition name
                    let words: Vec<&str> = trimmed.split_whitespace().collect();
                    if words.len() >= 2 {
                        let name_part = words[1];
                        // For methods like "func (r *Receiver) Method()"
                        if name_part.starts_with('(') && words.len() >= 4 {
                            def_name = words[3].trim_end_matches('(').to_string();
                        } else {
                            def_name = name_part.trim_end_matches(['(', '{']).to_string();
                        }
                    }
                }

                if in_definition {
                    current_def.push_str(line);
                    current_def.push('\n');

                    // Track braces
                    for ch in line.chars() {
                        if ch == '{' {
                            brace_depth += 1;
                        } else if ch == '}' {
                            brace_depth -= 1;
                            if brace_depth == 0 {
                                // Definition complete
                                let doc = Document::new(current_def.clone())
                                    .with_metadata("source", self.file_path.display().to_string())
                                    .with_metadata("definition_index", def_index)
                                    .with_metadata("definition_name", def_name.clone())
                                    .with_metadata("format", "go");

                                documents.push(doc);
                                current_def.clear();
                                in_definition = false;
                                def_index += 1;
                                break;
                            }
                        }
                    }
                } else if !trimmed.is_empty() {
                    // Non-definition code (package, imports, etc.)
                    current_def.push_str(line);
                    current_def.push('\n');
                }
            }

            // Add any remaining content
            if !current_def.is_empty() {
                let doc = Document::new(current_def)
                    .with_metadata("source", self.file_path.display().to_string())
                    .with_metadata("type", "global")
                    .with_metadata("format", "go");

                documents.push(doc);
            }

            Ok(documents)
        } else {
            // Load as single document
            let doc = Document::new(content)
                .with_metadata("source", self.file_path.display().to_string())
                .with_metadata("format", "go");

            Ok(vec![doc])
        }
    }
}

/// Loads Java source files (.java).
///
/// The `JavaLoader` reads Java source files, preserving all code including packages, imports, and classes.
/// Can optionally separate by class/method definitions.
///
/// # Example
///
/// ```no_run
/// use dashflow::core::document_loaders::JavaLoader;
/// use dashflow::core::documents::DocumentLoader;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let loader = JavaLoader::new("Main.java");
/// let documents = loader.load().await?;
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone)]

// ============================================================================
// C++ Loader
// ============================================================================

/// `CppLoader` loads C and C++ source files and separates them by definitions.
///
/// C++ is a general-purpose programming language created by Bjarne Stroustrup
/// as an extension of C. First released in 1985, C++ supports object-oriented,
/// generic, and functional programming paradigms.
///
/// Supports extensions: .c, .cc, .cpp, .cxx, .c++, .h, .hpp, .hxx
///
/// When `separate_definitions` is true, splits document by C++ declarations:
/// `class`, `struct`, `namespace`, `template`, and function definitions
///
/// # Example
///
/// ```no_run
/// use dashflow::core::document_loaders::CppLoader;
/// use dashflow::core::documents::DocumentLoader;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let loader = CppLoader::new("main.cpp").with_separate_definitions(true);
/// let docs = loader.load().await?;
/// println!("Loaded {} C++ definitions", docs.len());
/// # Ok(())
/// # }
/// ```
pub struct CppLoader {
    /// Path to the C/C++ file
    pub file_path: PathBuf,
    /// Separate documents per function/class/struct (default: false)
    pub separate_definitions: bool,
}

impl CppLoader {
    /// Create a new `CppLoader` for the given file path.
    pub fn new(file_path: impl AsRef<Path>) -> Self {
        Self {
            file_path: file_path.as_ref().to_path_buf(),
            separate_definitions: false,
        }
    }

    /// Create separate documents per function/class/struct definition.
    #[must_use]
    pub fn with_separate_definitions(mut self, separate: bool) -> Self {
        self.separate_definitions = separate;
        self
    }
}

#[async_trait]
impl DocumentLoader for CppLoader {
    async fn load(&self) -> Result<Vec<Document>> {
        let blob = Blob::from_path(&self.file_path);
        let content = blob.as_string()?;

        if self.separate_definitions {
            // Split by function/class/struct definitions
            let mut documents = Vec::new();
            let mut current_def = String::new();
            let mut def_name = String::new();
            let mut def_index = 0;
            let mut brace_depth = 0;
            let mut in_definition = false;
            let mut preamble = String::new();

            for line in content.lines() {
                let trimmed = line.trim_start();

                // Skip preprocessor directives and comments in detection
                let is_preprocessor = trimmed.starts_with('#');
                let is_comment = trimmed.starts_with("//") || trimmed.starts_with("/*");

                // Detect function/class/struct/namespace declarations
                if !in_definition && !is_preprocessor && !is_comment {
                    let has_definition = trimmed.starts_with("class ")
                        || trimmed.starts_with("struct ")
                        || trimmed.starts_with("namespace ")
                        || trimmed.starts_with("template")
                        || (trimmed.contains('(')
                            && trimmed.contains(')')
                            && !trimmed.ends_with(';')
                            && line.contains('{'));

                    if has_definition {
                        in_definition = true;
                        brace_depth = 0;

                        // Extract definition name (simplified)
                        if trimmed.starts_with("class ") || trimmed.starts_with("struct ") {
                            let words: Vec<&str> = trimmed.split_whitespace().collect();
                            if words.len() >= 2 {
                                def_name = words[1].trim_end_matches(['{', ':']).to_string();
                            }
                        } else if trimmed.starts_with("namespace ") {
                            let words: Vec<&str> = trimmed.split_whitespace().collect();
                            if words.len() >= 2 {
                                def_name = words[1].trim_end_matches('{').to_string();
                            }
                        } else if let Some(paren_pos) = trimmed.find('(') {
                            // Function - extract name before (
                            let before_paren = &trimmed[..paren_pos];
                            let words: Vec<&str> = before_paren.split_whitespace().collect();
                            if let Some(&last_word) = words.last() {
                                def_name = last_word.trim_start_matches('*').to_string();
                            }
                        }

                        // Include preamble (includes) with first definition
                        if def_index == 0 && !preamble.is_empty() {
                            current_def.push_str(&preamble);
                        }
                    }
                }

                if in_definition {
                    current_def.push_str(line);
                    current_def.push('\n');

                    // Track braces (ignore braces in comments/strings)
                    for ch in line.chars() {
                        if ch == '{' {
                            brace_depth += 1;
                        } else if ch == '}' {
                            brace_depth -= 1;
                            if brace_depth == 0 {
                                // Definition complete
                                let doc = Document::new(current_def.clone())
                                    .with_metadata("source", self.file_path.display().to_string())
                                    .with_metadata("definition_index", def_index)
                                    .with_metadata("definition_name", def_name.clone())
                                    .with_metadata("format", "cpp");

                                documents.push(doc);
                                current_def.clear();
                                in_definition = false;
                                def_index += 1;
                                break;
                            }
                        }
                    }
                } else if !trimmed.is_empty() {
                    // Collect preamble (includes, defines)
                    preamble.push_str(line);
                    preamble.push('\n');
                }
            }

            // Add any remaining content
            if !current_def.is_empty() {
                let doc = Document::new(current_def)
                    .with_metadata("source", self.file_path.display().to_string())
                    .with_metadata("type", "partial")
                    .with_metadata("format", "cpp");

                documents.push(doc);
            }

            Ok(documents)
        } else {
            // Load as single document
            let doc = Document::new(content)
                .with_metadata("source", self.file_path.display().to_string())
                .with_metadata("format", "cpp");

            Ok(vec![doc])
        }
    }
}

/// Loads PHP source files (.php).
///
/// The `PhpLoader` reads PHP source files, preserving all code structure.
/// Can optionally separate by function, class, or trait definitions.
///
/// # Example
///
/// ```no_run
/// use dashflow::core::document_loaders::PhpLoader;
/// use dashflow::core::documents::DocumentLoader;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let loader = PhpLoader::new("index.php");
/// let documents = loader.load().await?;
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone)]

// ============================================================================
// Zig Loader
// ============================================================================

/// `ZigLoader` loads Zig source files and separates them by definitions.
///
/// Zig is a systems programming language designed for simplicity and performance.
/// Created by Andrew Kelley, first released in 2016. Zig aims to replace C/C++
/// with better safety guarantees while maintaining manual memory control.
///
/// Supports extensions: .zig
///
/// When `separate_definitions` is true, splits document by Zig declarations:
/// `fn`, `pub fn`, `const`, `pub const`, `struct`, `enum`, `union`
///
/// # Example
///
/// ```no_run
/// use dashflow::core::document_loaders::ZigLoader;
/// use dashflow::core::documents::DocumentLoader;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let loader = ZigLoader::new("main.zig").with_separate_definitions(true);
/// let docs = loader.load().await?;
/// println!("Loaded {} Zig definitions", docs.len());
/// # Ok(())
/// # }
/// ```
pub struct ZigLoader {
    file_path: PathBuf,
    separate_definitions: bool,
}

impl ZigLoader {
    /// Creates a new Zig loader for the given file path
    pub fn new<P: AsRef<Path>>(path: P) -> Self {
        Self {
            file_path: path.as_ref().to_path_buf(),
            separate_definitions: false,
        }
    }

    /// Enable separation by function/struct/enum definitions
    #[must_use]
    pub fn with_separate_definitions(mut self, separate: bool) -> Self {
        self.separate_definitions = separate;
        self
    }
}

#[async_trait]
impl DocumentLoader for ZigLoader {
    async fn load(&self) -> Result<Vec<Document>> {
        let blob = Blob::from_path(&self.file_path);
        let content = blob.as_string()?;

        if self.separate_definitions {
            let mut documents = Vec::new();
            let mut current_definition = String::new();
            let mut definition_name = String::new();
            let mut brace_count = 0;
            let mut in_definition = false;

            for line in content.lines() {
                let trimmed = line.trim();

                // Skip blank lines and comments when not in definition
                if !in_definition && (trimmed.is_empty() || trimmed.starts_with("//")) {
                    continue;
                }

                // Check for definition start (fn, pub fn, const, struct, enum, etc.)
                if !in_definition {
                    if let Some(name) = Self::extract_definition(trimmed) {
                        in_definition = true;
                        definition_name = name;
                        current_definition.push_str(line);
                        current_definition.push('\n');

                        // Count braces
                        brace_count =
                            line.matches('{').count() as i32 - line.matches('}').count() as i32;

                        // Check if definition is complete on one line (no braces or balanced)
                        if brace_count == 0 && !line.contains('{') {
                            let doc = Document::new(current_definition.trim_end())
                                .with_metadata("source", self.file_path.display().to_string())
                                .with_metadata("format", "zig")
                                .with_metadata("definition_index", documents.len())
                                .with_metadata("definition_name", definition_name.clone());

                            documents.push(doc);

                            current_definition.clear();
                            definition_name.clear();
                            in_definition = false;
                        }
                        continue;
                    }
                }

                if in_definition {
                    current_definition.push_str(line);
                    current_definition.push('\n');

                    // Update brace count
                    brace_count += line.matches('{').count() as i32;
                    brace_count -= line.matches('}').count() as i32;

                    // Check if definition is complete
                    if brace_count == 0 {
                        let doc = Document::new(current_definition.trim_end())
                            .with_metadata("source", self.file_path.display().to_string())
                            .with_metadata("format", "zig")
                            .with_metadata("definition_index", documents.len())
                            .with_metadata("definition_name", definition_name.clone());

                        documents.push(doc);

                        current_definition.clear();
                        definition_name.clear();
                        in_definition = false;
                    }
                }
            }

            // Save last definition if incomplete
            if !current_definition.is_empty() {
                let doc = Document::new(current_definition.trim_end())
                    .with_metadata("source", self.file_path.display().to_string())
                    .with_metadata("format", "zig")
                    .with_metadata("definition_index", documents.len())
                    .with_metadata("definition_name", definition_name);

                documents.push(doc);
            }

            Ok(documents)
        } else {
            // Load as single document
            let doc = Document::new(content)
                .with_metadata("source", self.file_path.display().to_string())
                .with_metadata("format", "zig");

            Ok(vec![doc])
        }
    }
}

impl ZigLoader {
    /// Extract definition name from Zig definitions
    fn extract_definition(line: &str) -> Option<String> {
        let line = line.trim();

        // Match: pub fn name, fn name, pub const name, const name, struct, enum, union
        if let Some(after) = line.strip_prefix("pub fn ") {
            Self::extract_name(after, "pub fn")
        } else if let Some(after) = line.strip_prefix("fn ") {
            Self::extract_name(after, "fn")
        } else if let Some(after) = line.strip_prefix("pub const ") {
            Self::extract_name(after, "pub const")
        } else if let Some(after) = line.strip_prefix("const ") {
            Self::extract_name(after, "const")
        } else if line.starts_with("pub struct ") {
            Some("pub struct".to_string())
        } else if line.starts_with("struct ") {
            Some("struct".to_string())
        } else if line.starts_with("pub enum ") {
            Some("pub enum".to_string())
        } else if line.starts_with("enum ") {
            Some("enum".to_string())
        } else if line.starts_with("pub union ") {
            Some("pub union".to_string())
        } else if line.starts_with("union ") {
            Some("union".to_string())
        } else {
            None
        }
    }

    fn extract_name(after_keyword: &str, keyword: &str) -> Option<String> {
        // Extract name up to ( or : or space
        if let Some(end) = after_keyword.find(|c: char| c == '(' || c == ':' || c.is_whitespace()) {
            let name = after_keyword[..end].trim().to_string();
            if !name.is_empty() {
                return Some(format!("{keyword} {name}"));
            }
        }
        None
    }
}

/// Loader for Nim source files (.nim)
///
/// Nim is a statically typed systems programming language. Features:
/// - Python-like syntax with indentation
/// - Compiles to C, C++, JavaScript
/// - Powerful metaprogramming
/// - Manual and automatic memory management
/// - FFI to C
///
/// Supports:
/// - Loading entire file as single document
/// - Optional separation by proc, func, method, type definitions
/// - Indentation-based parsing
///
/// # Example
///
/// ```no_run
/// use dashflow::core::document_loaders::NimLoader;
/// use dashflow::core::documents::DocumentLoader;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let loader = NimLoader::new("main.nim").with_separate_definitions(true);
/// let docs = loader.load().await?;
/// for doc in docs {
///     println!("Content: {}", doc.page_content);
/// }
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone)]

// ============================================================================
// Nim Loader
// ============================================================================

pub struct NimLoader {
    file_path: PathBuf,
    separate_definitions: bool,
}

impl NimLoader {
    /// Creates a new Nim loader for the given file path
    pub fn new<P: AsRef<Path>>(path: P) -> Self {
        Self {
            file_path: path.as_ref().to_path_buf(),
            separate_definitions: false,
        }
    }

    /// Enable separation by proc/func/method/type definitions
    #[must_use]
    pub fn with_separate_definitions(mut self, separate: bool) -> Self {
        self.separate_definitions = separate;
        self
    }
}

#[async_trait]
impl DocumentLoader for NimLoader {
    async fn load(&self) -> Result<Vec<Document>> {
        let blob = Blob::from_path(&self.file_path);
        let content = blob.as_string()?;

        if self.separate_definitions {
            let mut documents = Vec::new();
            let mut current_definition = String::new();
            let mut definition_name = String::new();
            let mut base_indent: Option<usize> = None;
            let mut in_definition = false;

            for line in content.lines() {
                let trimmed = line.trim();

                // Skip blank lines and comments when not in definition
                if !in_definition && (trimmed.is_empty() || trimmed.starts_with('#')) {
                    continue;
                }

                let line_indent = line.len() - line.trim_start().len();

                // Check for definition start (proc, func, method, type, etc.)
                if !in_definition {
                    if let Some(name) = Self::extract_definition(trimmed) {
                        in_definition = true;
                        definition_name = name;
                        base_indent = Some(line_indent);
                        current_definition.push_str(line);
                        current_definition.push('\n');
                        continue;
                    }
                }

                if in_definition {
                    // Check if we're back to base indentation (or less) with non-empty content
                    if let Some(base) = base_indent {
                        if !trimmed.is_empty() && line_indent <= base && !trimmed.starts_with('#') {
                            // New definition at same or lower level - save previous one
                            let doc = Document::new(current_definition.trim_end())
                                .with_metadata("source", self.file_path.display().to_string())
                                .with_metadata("format", "nim")
                                .with_metadata("definition_index", documents.len())
                                .with_metadata("definition_name", definition_name.clone());

                            documents.push(doc);

                            // Check if this line starts a new definition
                            if let Some(name) = Self::extract_definition(trimmed) {
                                current_definition.clear();
                                definition_name = name;
                                base_indent = Some(line_indent);
                                current_definition.push_str(line);
                                current_definition.push('\n');
                            } else {
                                current_definition.clear();
                                definition_name.clear();
                                in_definition = false;
                            }
                        } else {
                            // Still inside definition
                            current_definition.push_str(line);
                            current_definition.push('\n');
                        }
                    }
                }
            }

            // Save last definition if any
            if !current_definition.is_empty() {
                let doc = Document::new(current_definition.trim_end())
                    .with_metadata("source", self.file_path.display().to_string())
                    .with_metadata("format", "nim")
                    .with_metadata("definition_index", documents.len())
                    .with_metadata("definition_name", definition_name);

                documents.push(doc);
            }

            Ok(documents)
        } else {
            // Load as single document
            let doc = Document::new(content)
                .with_metadata("source", self.file_path.display().to_string())
                .with_metadata("format", "nim");

            Ok(vec![doc])
        }
    }
}

impl NimLoader {
    /// Extract definition name from Nim definitions
    fn extract_definition(line: &str) -> Option<String> {
        let line = line.trim();

        // Match: proc name, func name, method name, type name, iterator name, macro name, template name
        if let Some(after) = line.strip_prefix("proc ") {
            Self::extract_name(after, "proc")
        } else if let Some(after) = line.strip_prefix("func ") {
            Self::extract_name(after, "func")
        } else if let Some(after) = line.strip_prefix("method ") {
            Self::extract_name(after, "method")
        } else if let Some(after) = line.strip_prefix("type ") {
            Self::extract_name(after, "type")
        } else if let Some(after) = line.strip_prefix("iterator ") {
            Self::extract_name(after, "iterator")
        } else if let Some(after) = line.strip_prefix("macro ") {
            Self::extract_name(after, "macro")
        } else if let Some(after) = line.strip_prefix("template ") {
            Self::extract_name(after, "template")
        } else {
            None
        }
    }

    fn extract_name(after_keyword: &str, keyword: &str) -> Option<String> {
        // Extract name up to * (for pointer), ( (for params), [ (for generics), = (for assignment)
        if let Some(end) = after_keyword
            .find(|c: char| c == '*' || c == '(' || c == '[' || c == '=' || c.is_whitespace())
        {
            let name = after_keyword[..end].trim().to_string();
            if !name.is_empty() {
                return Some(format!("{keyword} {name}"));
            }
        }
        None
    }
}

/// Loader for Crystal source files (.cr)
///
/// Crystal is a statically typed language with Ruby-like syntax. Features:
/// - Ruby-inspired syntax
/// - Static type checking at compile time
/// - Compiles to native code (LLVM)
/// - Fiber-based concurrency
/// - C bindings
///
/// Supports:
/// - Loading entire file as single document
/// - Optional separation by def, class, struct, module definitions
/// - Indentation-aware parsing
///
/// # Example
///
/// ```no_run
/// use dashflow::core::document_loaders::CrystalLoader;
/// use dashflow::core::documents::DocumentLoader;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let loader = CrystalLoader::new("main.cr").with_separate_definitions(true);
/// let docs = loader.load().await?;
/// for doc in docs {
///     println!("Content: {}", doc.page_content);
/// }
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone)]

// ============================================================================
// Crystal Loader
// ============================================================================

pub struct CrystalLoader {
    file_path: PathBuf,
    separate_definitions: bool,
}

impl CrystalLoader {
    /// Creates a new Crystal loader for the given file path
    pub fn new<P: AsRef<Path>>(path: P) -> Self {
        Self {
            file_path: path.as_ref().to_path_buf(),
            separate_definitions: false,
        }
    }

    /// Enable separation by def/class/struct/module definitions
    #[must_use]
    pub fn with_separate_definitions(mut self, separate: bool) -> Self {
        self.separate_definitions = separate;
        self
    }
}

#[async_trait]
impl DocumentLoader for CrystalLoader {
    async fn load(&self) -> Result<Vec<Document>> {
        let blob = Blob::from_path(&self.file_path);
        let content = blob.as_string()?;

        if self.separate_definitions {
            let mut documents = Vec::new();
            let mut current_definition = String::new();
            let mut definition_name = String::new();
            let mut in_definition = false;
            let mut end_count = 0; // Track 'end' keywords for nested structures

            for line in content.lines() {
                let trimmed = line.trim();

                // Skip blank lines and comments when not in definition
                if !in_definition && (trimmed.is_empty() || trimmed.starts_with('#')) {
                    continue;
                }

                // Check for definition start
                if !in_definition {
                    if let Some(name) = Self::extract_definition(trimmed) {
                        in_definition = true;
                        definition_name = name;
                        current_definition.push_str(line);
                        current_definition.push('\n');
                        end_count = 1; // Expecting 1 'end'
                        continue;
                    }
                }

                if in_definition {
                    current_definition.push_str(line);
                    current_definition.push('\n');

                    // Track nested structures (class, module, def, if, case, begin, etc.)
                    if trimmed.starts_with("class ")
                        || trimmed.starts_with("module ")
                        || trimmed.starts_with("def ")
                        || trimmed.starts_with("struct ")
                        || trimmed.starts_with("if ")
                        || trimmed.starts_with("unless ")
                        || trimmed.starts_with("case ")
                        || trimmed.starts_with("begin")
                        || trimmed.starts_with("while ")
                        || trimmed.starts_with("until ")
                        || trimmed.starts_with("for ")
                    {
                        end_count += 1;
                    }

                    // Check for 'end' keyword
                    if trimmed == "end" {
                        end_count -= 1;
                        if end_count == 0 {
                            // Definition complete
                            let doc = Document::new(current_definition.trim_end())
                                .with_metadata("source", self.file_path.display().to_string())
                                .with_metadata("format", "crystal")
                                .with_metadata("definition_index", documents.len())
                                .with_metadata("definition_name", definition_name.clone());

                            documents.push(doc);

                            current_definition.clear();
                            definition_name.clear();
                            in_definition = false;
                        }
                    }
                }
            }

            // Save last definition if incomplete
            if !current_definition.is_empty() {
                let doc = Document::new(current_definition.trim_end())
                    .with_metadata("source", self.file_path.display().to_string())
                    .with_metadata("format", "crystal")
                    .with_metadata("definition_index", documents.len())
                    .with_metadata("definition_name", definition_name);

                documents.push(doc);
            }

            Ok(documents)
        } else {
            // Load as single document
            let doc = Document::new(content)
                .with_metadata("source", self.file_path.display().to_string())
                .with_metadata("format", "crystal");

            Ok(vec![doc])
        }
    }
}

impl CrystalLoader {
    /// Extract definition name from Crystal definitions
    fn extract_definition(line: &str) -> Option<String> {
        let line = line.trim();

        // Match: def name, class Name, struct Name, module Name
        if let Some(after) = line.strip_prefix("def ") {
            Self::extract_name(after, "def")
        } else if let Some(after) = line.strip_prefix("class ") {
            Self::extract_name(after, "class")
        } else if let Some(after) = line.strip_prefix("struct ") {
            Self::extract_name(after, "struct")
        } else if let Some(after) = line.strip_prefix("module ") {
            Self::extract_name(after, "module")
        } else {
            None
        }
    }

    fn extract_name(after_keyword: &str, keyword: &str) -> Option<String> {
        // Extract name up to ( or < or : or whitespace
        if let Some(end) =
            after_keyword.find(|c: char| c == '(' || c == '<' || c == ':' || c.is_whitespace())
        {
            let name = after_keyword[..end].trim().to_string();
            if !name.is_empty() {
                return Some(format!("{keyword} {name}"));
            }
        } else if !after_keyword.is_empty() {
            // Name until end of line
            let name = after_keyword.split_whitespace().next()?.to_string();
            return Some(format!("{keyword} {name}"));
        }
        None
    }
}

/// Loader for `PowerShell` source files (.ps1, .psm1, .psd1)
///
/// `PowerShell` is a task automation and configuration management framework. Features:
/// - Object-oriented pipeline
/// - .NET integration
/// - Cmdlet-based commands
/// - Scripting and automation
/// - Module system
///
/// Supports:
/// - Loading entire `PowerShell` file as single document
/// - Optional separation by function, filter, workflow definitions
/// - Brace-based parsing with nested scope support
///
/// # Example
///
/// ```no_run
/// use dashflow::core::document_loaders::PowerShellLoader;
/// use dashflow::core::documents::DocumentLoader;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let loader = PowerShellLoader::new("script.ps1").with_separate_definitions(true);
/// let docs = loader.load().await?;
/// for doc in docs {
///     println!("Content: {}", doc.page_content);
/// }
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone)]

// ============================================================================
// D Loader
// ============================================================================

/// `DLoader` loads D programming language source files and separates them by definitions.
///
/// D is a systems programming language designed as a modern evolution of C and C++.
/// Created by Walter Bright and Andrei Alexandrescu, first released in 2001.
/// D combines low-level systems access with high-level constructs and metaprogramming.
///
/// Supports extensions: .d, .di
///
/// When `separate_definitions` is true, splits document by D declarations:
/// `class`, `struct`, `interface`, `template`, and function definitions
///
/// # Example
///
/// ```no_run
/// use dashflow::core::document_loaders::DLoader;
/// use dashflow::core::documents::DocumentLoader;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let loader = DLoader::new("main.d").with_separate_definitions(true);
/// let docs = loader.load().await?;
/// println!("Loaded {} D definitions", docs.len());
/// # Ok(())
/// # }
/// ```
pub struct DLoader {
    file_path: PathBuf,
    separate_definitions: bool,
}

impl DLoader {
    /// Create a new `DLoader` for the given file path.
    pub fn new<P: Into<PathBuf>>(file_path: P) -> Self {
        Self {
            file_path: file_path.into(),
            separate_definitions: false,
        }
    }

    /// Create separate documents per function/class/struct definition.
    #[must_use]
    pub fn with_separate_definitions(mut self, separate: bool) -> Self {
        self.separate_definitions = separate;
        self
    }
}

#[async_trait]
impl DocumentLoader for DLoader {
    async fn load(&self) -> Result<Vec<Document>> {
        let blob = Blob::from_path(&self.file_path);
        let content = blob.as_string()?;

        if !self.separate_definitions {
            return Ok(vec![Document::new(&content)
                .with_metadata("source", self.file_path.display().to_string())
                .with_metadata("format", "d")]);
        }

        // Parse D definitions: functions, classes, structs, interfaces, templates
        // D syntax: [attributes] [return_type] name(...) { body }
        //           [attributes] class/struct/interface name { body }
        //           [attributes] template name(...) { body }
        let mut documents = Vec::new();
        let lines: Vec<&str> = content.lines().collect();
        let mut i = 0;
        let mut definition_index = 0;

        while i < lines.len() {
            let line = lines[i].trim();

            // Skip empty lines and comments
            if line.is_empty() || line.starts_with("//") {
                i += 1;
                continue;
            }

            // Skip block comments
            if line.starts_with("/*") {
                while i < lines.len() && !lines[i].contains("*/") {
                    i += 1;
                }
                i += 1;
                continue;
            }

            // Check for definition keywords
            let is_definition = line.contains("class ")
                || line.contains("struct ")
                || line.contains("interface ")
                || line.contains("template ")
                || (line.contains('(') && line.contains(')') && line.contains('{'));

            if is_definition {
                // Extract definition name
                let def_name = if let Some(class_pos) = line.find("class ") {
                    line[class_pos + 6..]
                        .split_whitespace()
                        .next()
                        .unwrap_or("unknown")
                        .trim_end_matches('{')
                } else if let Some(struct_pos) = line.find("struct ") {
                    line[struct_pos + 7..]
                        .split_whitespace()
                        .next()
                        .unwrap_or("unknown")
                        .trim_end_matches('{')
                } else if let Some(interface_pos) = line.find("interface ") {
                    line[interface_pos + 10..]
                        .split_whitespace()
                        .next()
                        .unwrap_or("unknown")
                        .trim_end_matches('{')
                } else if let Some(template_pos) = line.find("template ") {
                    line[template_pos + 9..]
                        .split('(')
                        .next()
                        .unwrap_or("unknown")
                        .trim()
                } else {
                    // Function definition - extract name before parenthesis
                    line.split('(')
                        .next()
                        .unwrap_or("")
                        .split_whitespace()
                        .last()
                        .unwrap_or("unknown")
                };

                // Collect definition body with brace counting
                let mut definition_lines = vec![lines[i]];
                let mut brace_count =
                    line.matches('{').count() as i32 - line.matches('}').count() as i32;
                i += 1;

                while i < lines.len() && brace_count > 0 {
                    definition_lines.push(lines[i]);
                    brace_count += lines[i].matches('{').count() as i32;
                    brace_count -= lines[i].matches('}').count() as i32;
                    i += 1;
                }

                let definition_content = definition_lines.join("\n");
                documents.push(
                    Document::new(&definition_content)
                        .with_metadata("source", self.file_path.display().to_string())
                        .with_metadata("format", "d")
                        .with_metadata("definition_index", definition_index.to_string())
                        .with_metadata("definition_name", def_name.to_string()),
                );
                definition_index += 1;
            } else {
                i += 1;
            }
        }

        if documents.is_empty() {
            Ok(vec![Document::new(&content)
                .with_metadata("source", self.file_path.display().to_string())
                .with_metadata("format", "d")])
        } else {
            Ok(documents)
        }
    }
}

/// `VLoader` loads V programming language source code as documents.
///
/// V is a simple, fast, compiled language for building maintainable software.
/// Created by Alexander Medvednikov, first released in 2019.
/// Aims for simplicity with Go-like syntax and Rust-like safety.
///
/// Supports extensions: .v, .vsh (V shell script)
///
/// Set `separate_definitions` to true to split by function/struct/interface definitions.
///
/// Example:
/// ```no_run
/// use dashflow::core::document_loaders::VLoader;
/// use dashflow::core::documents::DocumentLoader;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let loader = VLoader::new("module.v").with_separate_definitions(true);
/// let docs = loader.load().await?;
/// println!("Loaded {} V definitions", docs.len());
/// # Ok(())
/// # }
/// ```

// ============================================================================
// V Loader
// ============================================================================

pub struct VLoader {
    file_path: PathBuf,
    separate_definitions: bool,
}

impl VLoader {
    /// Create a new `VLoader` for the given file path.
    pub fn new<P: Into<PathBuf>>(file_path: P) -> Self {
        Self {
            file_path: file_path.into(),
            separate_definitions: false,
        }
    }

    /// Create separate documents per function/struct/interface definition.
    #[must_use]
    pub fn with_separate_definitions(mut self, separate: bool) -> Self {
        self.separate_definitions = separate;
        self
    }
}

#[async_trait]
impl DocumentLoader for VLoader {
    async fn load(&self) -> Result<Vec<Document>> {
        let blob = Blob::from_path(&self.file_path);
        let content = blob.as_string()?;

        if !self.separate_definitions {
            return Ok(vec![Document::new(&content)
                .with_metadata("source", self.file_path.display().to_string())
                .with_metadata("format", "v")]);
        }

        // Parse V definitions: functions, structs, interfaces
        // V syntax: fn name(...) type { body }
        //           struct Name { fields }
        //           interface Name { methods }
        let mut documents = Vec::new();
        let lines: Vec<&str> = content.lines().collect();
        let mut i = 0;
        let mut definition_index = 0;

        while i < lines.len() {
            let line = lines[i].trim();

            // Skip empty lines and comments
            if line.is_empty() || line.starts_with("//") {
                i += 1;
                continue;
            }

            // Skip block comments
            if line.starts_with("/*") {
                while i < lines.len() && !lines[i].contains("*/") {
                    i += 1;
                }
                i += 1;
                continue;
            }

            // Check for definition keywords
            let is_definition = line.starts_with("fn ")
                || line.starts_with("pub fn ")
                || line.starts_with("struct ")
                || line.starts_with("pub struct ")
                || line.starts_with("interface ")
                || line.starts_with("pub interface ");

            if is_definition {
                // Extract definition name
                let def_name = if line.contains("fn ") {
                    // Function: fn name(...) or pub fn name(...)
                    line.split("fn ")
                        .nth(1)
                        .unwrap_or("")
                        .split('(')
                        .next()
                        .unwrap_or("unknown")
                        .trim()
                } else if line.contains("struct ") {
                    line.split("struct ")
                        .nth(1)
                        .unwrap_or("")
                        .split_whitespace()
                        .next()
                        .unwrap_or("unknown")
                        .trim_end_matches('{')
                } else if line.contains("interface ") {
                    line.split("interface ")
                        .nth(1)
                        .unwrap_or("")
                        .split_whitespace()
                        .next()
                        .unwrap_or("unknown")
                        .trim_end_matches('{')
                } else {
                    "unknown"
                };

                // Collect definition body with brace counting
                let mut definition_lines = vec![lines[i]];
                let mut brace_count =
                    line.matches('{').count() as i32 - line.matches('}').count() as i32;
                i += 1;

                while i < lines.len() && brace_count > 0 {
                    definition_lines.push(lines[i]);
                    brace_count += lines[i].matches('{').count() as i32;
                    brace_count -= lines[i].matches('}').count() as i32;
                    i += 1;
                }

                let definition_content = definition_lines.join("\n");
                documents.push(
                    Document::new(&definition_content)
                        .with_metadata("source", self.file_path.display().to_string())
                        .with_metadata("format", "v")
                        .with_metadata("definition_index", definition_index.to_string())
                        .with_metadata("definition_name", def_name.to_string()),
                );
                definition_index += 1;
            } else {
                i += 1;
            }
        }

        if documents.is_empty() {
            Ok(vec![Document::new(&content)
                .with_metadata("source", self.file_path.display().to_string())
                .with_metadata("format", "v")])
        } else {
            Ok(documents)
        }
    }
}

/// `ForthLoader` loads Forth programming language source code as documents.
///
/// Forth is a stack-based, procedural programming language.
/// Created by Charles H. Moore in the late 1960s.
/// Used in embedded systems, space applications, and boot firmware (`OpenFirmware`).
///
/// Supports extensions: .fth, .forth, .4th, .fs
///
/// Set `separate_definitions` to true to split by word definitions.
/// Forth uses colon definitions: : word-name ... ;
///
/// Example:
/// ```no_run
/// use dashflow::core::document_loaders::ForthLoader;
/// use dashflow::core::documents::DocumentLoader;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let loader = ForthLoader::new("program.fth").with_separate_definitions(true);
/// let docs = loader.load().await?;
/// println!("Loaded {} Forth words", docs.len());
/// # Ok(())
/// # }
/// ```

// ============================================================================
// WASM Loader
// ============================================================================

/// `WASMLoader` loads WebAssembly text format files and separates them by modules.
///
/// WebAssembly (WASM) is a binary instruction format for stack-based virtual machines.
/// Designed as a portable compilation target for high-level languages like C, C++,
/// and Rust. WASM text format (.wat) uses S-expressions for human-readable representation.
///
/// Supports extensions: .wat, .wast
///
/// When `separate_modules` is true (via `with_separate_modules()`), splits document
/// by top-level `(module ...)` declarations.
///
/// # Example
///
/// ```no_run
/// use dashflow::core::document_loaders::WASMLoader;
/// use dashflow::core::documents::DocumentLoader;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let loader = WASMLoader::new("program.wat").with_separate_modules();
/// let docs = loader.load().await?;
/// println!("Loaded {} WASM modules", docs.len());
/// # Ok(())
/// # }
/// ```
pub struct WASMLoader {
    file_path: PathBuf,
    separate_modules: bool,
}

impl WASMLoader {
    /// Create a new WASM text format loader for the given file path.
    pub fn new<P: AsRef<Path>>(path: P) -> Self {
        Self {
            file_path: path.as_ref().to_path_buf(),
            separate_modules: false,
        }
    }

    /// Enable separation by module declarations.
    #[must_use]
    pub fn with_separate_modules(mut self) -> Self {
        self.separate_modules = true;
        self
    }

    /// Check if line starts a new module (top-level (module ...) declaration)
    fn is_module_start(line: &str) -> bool {
        let trimmed = line.trim();
        trimmed.starts_with("(module")
    }
}

#[async_trait]
impl DocumentLoader for WASMLoader {
    async fn load(&self) -> Result<Vec<Document>> {
        let blob = Blob::from_path(&self.file_path);
        let content = blob.as_string()?;

        if !self.separate_modules {
            return Ok(vec![Document::new(&content)
                .with_metadata("source", self.file_path.display().to_string())
                .with_metadata("format", "wasm")]);
        }

        // Separate by module declarations
        // This is a simple implementation - full S-expression parsing would be more robust
        let lines: Vec<&str> = content.lines().collect();
        let mut documents = Vec::new();
        let mut current_module = Vec::new();
        let mut paren_depth = 0;
        let mut in_module = false;

        for line in lines {
            if Self::is_module_start(line) && paren_depth == 0 {
                // Save previous module
                if !current_module.is_empty() {
                    let module_content = current_module.join("\n");
                    documents.push(
                        Document::new(&module_content)
                            .with_metadata("source", self.file_path.display().to_string())
                            .with_metadata("format", "wasm")
                            .with_metadata("module_index", (documents.len()).to_string()),
                    );
                    current_module.clear();
                }
                in_module = true;
            }

            if in_module {
                current_module.push(line);
                // Track parentheses depth for proper module boundary detection
                for ch in line.chars() {
                    match ch {
                        '(' => paren_depth += 1,
                        ')' => {
                            paren_depth -= 1;
                            if paren_depth == 0 {
                                // Module complete
                                let module_content = current_module.join("\n");
                                documents.push(
                                    Document::new(&module_content)
                                        .with_metadata(
                                            "source",
                                            self.file_path.display().to_string(),
                                        )
                                        .with_metadata("format", "wasm")
                                        .with_metadata(
                                            "module_index",
                                            (documents.len()).to_string(),
                                        ),
                                );
                                current_module.clear();
                                in_module = false;
                            }
                        }
                        _ => {}
                    }
                }
            }
        }

        // Save last module if incomplete
        if !current_module.is_empty() {
            let module_content = current_module.join("\n");
            documents.push(
                Document::new(&module_content)
                    .with_metadata("source", self.file_path.display().to_string())
                    .with_metadata("format", "wasm")
                    .with_metadata("module_index", documents.len().to_string()),
            );
        }

        if documents.is_empty() {
            Ok(vec![Document::new(&content)
                .with_metadata("source", self.file_path.display().to_string())
                .with_metadata("format", "wasm")])
        } else {
            Ok(documents)
        }
    }
}

// ============================================================================
// Swift Loader
// ============================================================================

/// Loads Swift source files (.swift).
///
/// The `SwiftLoader` reads Swift source files, preserving all code structure.
/// Can optionally separate by function, class, struct, enum, or protocol definitions.
///
/// # Example
///
/// ```no_run
/// use dashflow::core::document_loaders::SwiftLoader;
/// use dashflow::core::documents::DocumentLoader;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let loader = SwiftLoader::new("Main.swift");
/// let documents = loader.load().await?;
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone)]
pub struct SwiftLoader {
    /// Path to the Swift file
    pub file_path: PathBuf,
    /// Separate documents per function/class/struct/enum/protocol (default: false)
    pub separate_definitions: bool,
}

impl SwiftLoader {
    /// Create a new `SwiftLoader` for the given file path.
    pub fn new(file_path: impl AsRef<Path>) -> Self {
        Self {
            file_path: file_path.as_ref().to_path_buf(),
            separate_definitions: false,
        }
    }

    /// Create separate documents per definition.
    #[must_use]
    pub fn with_separate_definitions(mut self, separate: bool) -> Self {
        self.separate_definitions = separate;
        self
    }
}

#[async_trait]
impl DocumentLoader for SwiftLoader {
    async fn load(&self) -> Result<Vec<Document>> {
        let blob = Blob::from_path(&self.file_path);
        let content = blob.as_string()?;

        if self.separate_definitions {
            // Split by function/class/struct/enum/protocol definitions
            let mut documents = Vec::new();
            let mut current_def = String::new();
            let mut def_name = String::new();
            let mut def_index = 0;
            let mut brace_depth = 0;
            let mut in_definition = false;

            for line in content.lines() {
                let trimmed = line.trim_start();

                // Detect Swift declarations
                if !in_definition
                    && (trimmed.starts_with("func ")
                        || trimmed.starts_with("class ")
                        || trimmed.starts_with("struct ")
                        || trimmed.starts_with("enum ")
                        || trimmed.starts_with("protocol ")
                        || trimmed.starts_with("extension ")
                        || trimmed.starts_with("public func ")
                        || trimmed.starts_with("private func ")
                        || trimmed.starts_with("internal func ")
                        || trimmed.starts_with("public class ")
                        || trimmed.starts_with("public struct ")
                        || trimmed.starts_with("public enum "))
                {
                    in_definition = true;
                    brace_depth = 0;

                    // Extract definition name
                    let words: Vec<&str> = trimmed.split_whitespace().collect();
                    for (i, &word) in words.iter().enumerate() {
                        if matches!(
                            word,
                            "func" | "class" | "struct" | "enum" | "protocol" | "extension"
                        ) && i + 1 < words.len()
                        {
                            let name_part = words[i + 1];
                            if let Some(paren_pos) = name_part.find('(') {
                                def_name = name_part[..paren_pos].to_string();
                            } else if let Some(angle_pos) = name_part.find('<') {
                                def_name = name_part[..angle_pos].to_string();
                            } else if let Some(colon_pos) = name_part.find(':') {
                                def_name = name_part[..colon_pos].to_string();
                            } else {
                                def_name = name_part.trim_end_matches('{').to_string();
                            }
                            break;
                        }
                    }
                }

                if in_definition {
                    current_def.push_str(line);
                    current_def.push('\n');

                    // Track braces
                    for ch in line.chars() {
                        if ch == '{' {
                            brace_depth += 1;
                        } else if ch == '}' {
                            brace_depth -= 1;
                            if brace_depth == 0 {
                                // Definition complete
                                let doc = Document::new(current_def.clone())
                                    .with_metadata("source", self.file_path.display().to_string())
                                    .with_metadata("definition_index", def_index)
                                    .with_metadata("definition_name", def_name.clone())
                                    .with_metadata("format", "swift");

                                documents.push(doc);
                                current_def.clear();
                                in_definition = false;
                                def_index += 1;
                                break;
                            }
                        }
                    }
                } else if !trimmed.is_empty() {
                    // Non-definition code (imports, etc.)
                    current_def.push_str(line);
                    current_def.push('\n');
                }
            }

            // Add any remaining content
            if !current_def.is_empty() {
                let doc = Document::new(current_def)
                    .with_metadata("source", self.file_path.display().to_string())
                    .with_metadata("type", "global")
                    .with_metadata("format", "swift");

                documents.push(doc);
            }

            Ok(documents)
        } else {
            // Load as single document
            let doc = Document::new(content)
                .with_metadata("source", self.file_path.display().to_string())
                .with_metadata("format", "swift");

            Ok(vec![doc])
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::test_prelude::*;
    use std::fs;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_swift_loader() {
        let temp_dir = TempDir::new().unwrap();
        let swift_path = temp_dir.path().join("main.swift");

        let swift_content = r#"import Foundation

func greet(name: String) {
    print("Hello, \(name)")
}

class Person {
    var name: String

    init(name: String) {
        self.name = name
    }

    func sayHello() {
        print("Hello, I'm \(name)")
    }
}

struct Point {
    var x: Int
    var y: Int
}
"#;

        fs::write(&swift_path, swift_content).unwrap();

        let loader = SwiftLoader::new(&swift_path);
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1);
        assert!(docs[0].page_content.contains("import Foundation"));
        assert!(docs[0].page_content.contains("func greet"));
        assert!(docs[0].page_content.contains("class Person"));
        assert!(docs[0].page_content.contains("struct Point"));
        assert_eq!(
            docs[0].get_metadata("format").and_then(|v| v.as_str()),
            Some("swift")
        );
    }

    #[tokio::test]
    async fn test_swift_loader_separate_definitions() {
        let temp_dir = TempDir::new().unwrap();
        let swift_path = temp_dir.path().join("types.swift");

        let swift_content = r#"func foo() -> String {
    return "foo"
}

struct Bar {
    var x: Int
}

class Baz {
    var y: String

    init(y: String) {
        self.y = y
    }
}

enum Status {
    case active
    case inactive
}
"#;

        fs::write(&swift_path, swift_content).unwrap();

        let loader = SwiftLoader::new(&swift_path).with_separate_definitions(true);
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 4);
        assert!(docs[0].page_content.contains("func foo"));
        assert!(docs[1].page_content.contains("struct Bar"));
        assert!(docs[2].page_content.contains("class Baz"));
        assert!(docs[3].page_content.contains("enum Status"));
        assert_eq!(
            docs[0]
                .get_metadata("definition_name")
                .and_then(|v| v.as_str()),
            Some("foo")
        );
        assert_eq!(
            docs[1]
                .get_metadata("definition_name")
                .and_then(|v| v.as_str()),
            Some("Bar")
        );
    }
}
