//! Web and scripting language document loaders.
//!
//! This module provides loaders for web development and general-purpose scripting languages:
//! - PHP
//! - Ruby
//! - Perl
//! - Lua
//! - R
//! - Julia

use async_trait::async_trait;
use std::path::{Path, PathBuf};

use crate::core::documents::{Blob, Document, DocumentLoader};
use crate::core::error::Result;

/// `PhpLoader` loads PHP source files and separates them by function/class/trait definitions.
///
/// PHP (Hypertext Preprocessor) is a server-side scripting language designed for web development.
/// Created by Rasmus Lerdorf in 1994, PHP powers a large portion of websites and web applications.
///
/// Supports extensions: .php, .php3, .php4, .php5, .phtml
///
/// When `separate_definitions` is true, splits document by `function`, `class`, `trait`,
/// and `interface` declarations.
///
/// # Example
///
/// ```no_run
/// use dashflow::core::document_loaders::PhpLoader;
/// use dashflow::core::documents::DocumentLoader;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let loader = PhpLoader::new("index.php").with_separate_definitions(true);
/// let docs = loader.load().await?;
/// println!("Loaded {} PHP definitions", docs.len());
/// # Ok(())
/// # }
/// ```
pub struct PhpLoader {
    /// Path to the PHP file
    pub file_path: PathBuf,
    /// Separate documents per function/class/trait (default: false)
    pub separate_definitions: bool,
}

impl PhpLoader {
    /// Create a new `PhpLoader` for the given file path.
    pub fn new(file_path: impl AsRef<Path>) -> Self {
        Self {
            file_path: file_path.as_ref().to_path_buf(),
            separate_definitions: false,
        }
    }

    /// Create separate documents per function/class/trait definition.
    #[must_use]
    pub fn with_separate_definitions(mut self, separate: bool) -> Self {
        self.separate_definitions = separate;
        self
    }
}

#[async_trait]
impl DocumentLoader for PhpLoader {
    async fn load(&self) -> Result<Vec<Document>> {
        let blob = Blob::from_path(&self.file_path);
        let content = blob.as_string()?;

        if self.separate_definitions {
            // Split by function/class/trait definitions
            let mut documents = Vec::new();
            let mut current_def = String::new();
            let mut def_name = String::new();
            let mut def_index = 0;
            let mut brace_depth = 0;
            let mut in_definition = false;

            for line in content.lines() {
                let trimmed = line.trim_start();

                // Detect function/class/trait/interface declarations
                if !in_definition
                    && (trimmed.starts_with("function ")
                        || trimmed.starts_with("class ")
                        || trimmed.starts_with("trait ")
                        || trimmed.starts_with("interface ")
                        || trimmed.starts_with("abstract class ")
                        || trimmed.starts_with("final class ")
                        || trimmed.starts_with("public function ")
                        || trimmed.starts_with("private function ")
                        || trimmed.starts_with("protected function "))
                {
                    in_definition = true;
                    brace_depth = 0;

                    // Extract definition name
                    let words: Vec<&str> = trimmed.split_whitespace().collect();
                    for (i, &word) in words.iter().enumerate() {
                        if matches!(word, "function" | "class" | "trait" | "interface")
                            && i + 1 < words.len()
                        {
                            let name_part = words[i + 1];
                            if let Some(paren_pos) = name_part.find('(') {
                                def_name = name_part[..paren_pos].to_string();
                            } else {
                                def_name = name_part.trim_end_matches(['{', ':']).to_string();
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
                                    .with_metadata("format", "php");

                                documents.push(doc);
                                current_def.clear();
                                in_definition = false;
                                def_index += 1;
                                break;
                            }
                        }
                    }
                } else if !trimmed.is_empty() {
                    // Non-definition code (e.g., <?php tags, includes)
                    current_def.push_str(line);
                    current_def.push('\n');
                }
            }

            // Add any remaining content
            if !current_def.is_empty() {
                let doc = Document::new(current_def)
                    .with_metadata("source", self.file_path.display().to_string())
                    .with_metadata("type", "global")
                    .with_metadata("format", "php");

                documents.push(doc);
            }

            Ok(documents)
        } else {
            // Load as single document
            let doc = Document::new(content)
                .with_metadata("source", self.file_path.display().to_string())
                .with_metadata("format", "php");

            Ok(vec![doc])
        }
    }
}

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
/// let loader = SwiftLoader::new("main.swift");
/// let documents = loader.load().await?;
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone)]

// ============================================================================
// Ruby Loader
// ============================================================================

pub struct RubyLoader {
    /// Path to the Ruby file
    pub file_path: PathBuf,
    /// Separate documents per method/class/module (default: false)
    pub separate_definitions: bool,
}

impl RubyLoader {
    /// Create a new `RubyLoader` for the given file path.
    pub fn new(file_path: impl AsRef<Path>) -> Self {
        Self {
            file_path: file_path.as_ref().to_path_buf(),
            separate_definitions: false,
        }
    }

    /// Create separate documents per method/class/module definition.
    #[must_use]
    pub fn with_separate_definitions(mut self, separate: bool) -> Self {
        self.separate_definitions = separate;
        self
    }
}

#[async_trait]
impl DocumentLoader for RubyLoader {
    async fn load(&self) -> Result<Vec<Document>> {
        let blob = Blob::from_path(&self.file_path);
        let content = blob.as_string()?;

        if self.separate_definitions {
            // Split by method/class/module definitions
            let mut documents = Vec::new();
            let mut current_def = String::new();
            let mut def_name = String::new();
            let mut def_index = 0;
            let mut in_definition = false;
            let mut depth = 0;

            for line in content.lines() {
                let trimmed = line.trim_start();

                // Detect Ruby declarations
                if trimmed.starts_with("def ")
                    || trimmed.starts_with("class ")
                    || trimmed.starts_with("module ")
                {
                    if in_definition {
                        // Nested definition
                        depth += 1;
                        current_def.push_str(line);
                        current_def.push('\n');
                    } else {
                        in_definition = true;
                        depth = 1;

                        // Extract definition name
                        let words: Vec<&str> = trimmed.split_whitespace().collect();
                        if words.len() >= 2 {
                            let name_part = words[1];
                            // Remove parameters for methods
                            if let Some(paren_pos) = name_part.find('(') {
                                def_name = name_part[..paren_pos].to_string();
                            } else {
                                def_name = name_part.trim_end_matches([';', '<']).to_string();
                            }
                        }

                        current_def.push_str(line);
                        current_def.push('\n');
                    }
                } else if in_definition {
                    current_def.push_str(line);
                    current_def.push('\n');

                    // Check for end keyword
                    if trimmed == "end" {
                        depth -= 1;
                        if depth == 0 {
                            // Definition complete
                            let doc = Document::new(current_def.clone())
                                .with_metadata("source", self.file_path.display().to_string())
                                .with_metadata("definition_index", def_index)
                                .with_metadata("definition_name", def_name.clone())
                                .with_metadata("format", "ruby");

                            documents.push(doc);
                            current_def.clear();
                            in_definition = false;
                            def_index += 1;
                        }
                    }
                } else if !trimmed.is_empty() {
                    // Non-definition code (requires, etc.)
                    current_def.push_str(line);
                    current_def.push('\n');
                }
            }

            // Add any remaining content
            if !current_def.is_empty() {
                let doc = Document::new(current_def)
                    .with_metadata("source", self.file_path.display().to_string())
                    .with_metadata("type", "global")
                    .with_metadata("format", "ruby");

                documents.push(doc);
            }

            Ok(documents)
        } else {
            // Load as single document
            let doc = Document::new(content)
                .with_metadata("source", self.file_path.display().to_string())
                .with_metadata("format", "ruby");

            Ok(vec![doc])
        }
    }
}

/// Loads Perl source files (.pl, .pm).
///
/// The `PerlLoader` reads Perl source files, preserving all code structure.
/// Can optionally separate by subroutine (sub) or package definitions.
///
/// # Example
///
/// ```no_run
/// use dashflow::core::document_loaders::PerlLoader;
/// use dashflow::core::documents::DocumentLoader;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let loader = PerlLoader::new("script.pl");
/// let documents = loader.load().await?;
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone)]

// ============================================================================
// Perl Loader
// ============================================================================

pub struct PerlLoader {
    /// Path to the Perl file
    pub file_path: PathBuf,
    /// Separate documents per subroutine/package (default: false)
    pub separate_definitions: bool,
}

impl PerlLoader {
    /// Create a new `PerlLoader` for the given file path.
    pub fn new(file_path: impl AsRef<Path>) -> Self {
        Self {
            file_path: file_path.as_ref().to_path_buf(),
            separate_definitions: false,
        }
    }

    /// Create separate documents per subroutine/package definition.
    #[must_use]
    pub fn with_separate_definitions(mut self, separate: bool) -> Self {
        self.separate_definitions = separate;
        self
    }
}

#[async_trait]
impl DocumentLoader for PerlLoader {
    async fn load(&self) -> Result<Vec<Document>> {
        let blob = Blob::from_path(&self.file_path);
        let content = blob.as_string()?;

        if self.separate_definitions {
            // Split by subroutine/package definitions
            let mut documents = Vec::new();
            let mut current_def = String::new();
            let mut def_name = String::new();
            let mut def_index = 0;
            let mut brace_depth = 0;
            let mut in_definition = false;

            for line in content.lines() {
                let trimmed = line.trim_start();

                // Detect subroutine or package declarations
                if !in_definition
                    && (trimmed.starts_with("sub ") || trimmed.starts_with("package "))
                {
                    in_definition = true;
                    brace_depth = 0;

                    // Extract definition name
                    let words: Vec<&str> = trimmed.split_whitespace().collect();
                    if words.len() >= 2 {
                        let name_part = words[1];
                        // Remove parameters or semicolons
                        if let Some(paren_pos) = name_part.find('(') {
                            def_name = name_part[..paren_pos].to_string();
                        } else {
                            def_name = name_part.trim_end_matches([';', '{']).to_string();
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
                                    .with_metadata("format", "perl");

                                documents.push(doc);
                                current_def.clear();
                                in_definition = false;
                                def_index += 1;
                                break;
                            }
                        }
                    }
                } else if !trimmed.is_empty() {
                    // Non-definition code (use statements, etc.)
                    current_def.push_str(line);
                    current_def.push('\n');
                }
            }

            // Add any remaining content
            if !current_def.is_empty() {
                let doc = Document::new(current_def)
                    .with_metadata("source", self.file_path.display().to_string())
                    .with_metadata("type", "global")
                    .with_metadata("format", "perl");

                documents.push(doc);
            }

            Ok(documents)
        } else {
            // Load as single document
            let doc = Document::new(content)
                .with_metadata("source", self.file_path.display().to_string())
                .with_metadata("format", "perl");

            Ok(vec![doc])
        }
    }
}

/// Loads Kotlin source files (.kt, .kts).
///
/// The `KotlinLoader` reads Kotlin source files, preserving all code structure.
/// Can optionally separate by function, class, object, or interface definitions.
///
/// # Example
///
/// ```no_run
/// use dashflow::core::document_loaders::KotlinLoader;
/// use dashflow::core::documents::DocumentLoader;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let loader = KotlinLoader::new("Main.kt");
/// let documents = loader.load().await?;
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone)]

// ============================================================================
// Lua Loader
// ============================================================================

pub struct LuaLoader {
    file_path: PathBuf,
    separate_definitions: bool,
}

impl LuaLoader {
    /// Creates a new Lua loader for the given file path
    pub fn new<P: AsRef<Path>>(path: P) -> Self {
        Self {
            file_path: path.as_ref().to_path_buf(),
            separate_definitions: false,
        }
    }

    /// Enable separation by function definitions
    #[must_use]
    pub fn with_separate_definitions(mut self, separate: bool) -> Self {
        self.separate_definitions = separate;
        self
    }
}

#[async_trait]
impl DocumentLoader for LuaLoader {
    async fn load(&self) -> Result<Vec<Document>> {
        let blob = Blob::from_path(&self.file_path);
        let content = blob.as_string()?;

        if self.separate_definitions {
            let mut documents = Vec::new();
            let mut current_function = String::new();
            let mut function_name = String::new();
            let mut in_function = false;

            for line in content.lines() {
                let trimmed = line.trim();

                // Check for function definition start
                if in_function {
                    current_function.push_str(line);
                    current_function.push('\n');

                    // Check for function end
                    if trimmed == "end"
                        || trimmed.starts_with("end ")
                        || trimmed.starts_with("end,")
                    {
                        // Create document for completed function
                        let doc = Document::new(current_function.trim_end())
                            .with_metadata("source", self.file_path.display().to_string())
                            .with_metadata("format", "lua")
                            .with_metadata("definition_index", documents.len())
                            .with_metadata("definition_name", function_name.clone());

                        documents.push(doc);

                        current_function.clear();
                        function_name.clear();
                        in_function = false;
                    }
                } else if let Some(name) = Self::extract_function_name(trimmed) {
                    in_function = true;
                    function_name = name;
                    current_function.push_str(line);
                    current_function.push('\n');
                    continue;
                }
            }

            // If still in function at EOF, save it
            if !current_function.is_empty() {
                let doc = Document::new(current_function.trim_end())
                    .with_metadata("source", self.file_path.display().to_string())
                    .with_metadata("format", "lua")
                    .with_metadata("definition_index", documents.len())
                    .with_metadata("definition_name", function_name);

                documents.push(doc);
            }

            Ok(documents)
        } else {
            // Load as single document
            let doc = Document::new(content)
                .with_metadata("source", self.file_path.display().to_string())
                .with_metadata("format", "lua");

            Ok(vec![doc])
        }
    }
}

impl LuaLoader {
    /// Extract function name from Lua function definitions
    fn extract_function_name(line: &str) -> Option<String> {
        let line = line.trim();

        // Match: function name(...) or local function name(...) or name = function(...)
        if let Some(pos) = line.find("function ") {
            let after_function = &line[pos + 9..]; // "function ".len() == 9
                                                   // Skip "local" if present
            let after_function = after_function.trim_start_matches("local").trim();

            // Extract name up to opening parenthesis
            if let Some(paren_pos) = after_function.find('(') {
                let name = after_function[..paren_pos].trim();
                if !name.is_empty() {
                    return Some(name.to_string());
                }
            }
        }

        // Match: name = function(...) pattern
        if line.contains(" = function(") || line.contains("= function(") {
            if let Some(eq_pos) = line.find(" = function(") {
                let name = line[..eq_pos].trim();
                // Remove "local " prefix if present
                let name = name.trim_start_matches("local").trim();
                if !name.is_empty() {
                    return Some(name.to_string());
                }
            }
        }

        None
    }
}

/// Loader for R source files (.R, .r)
///
/// R is a language for statistical computing and graphics. Features:
/// - Vectorized operations
/// - Statistical functions
/// - Data frames and matrices
/// - Functional programming
/// - Extensive package ecosystem
///
/// Supports:
/// - Loading entire file as single document
/// - Optional separation by function definitions
/// - Supports both <- and = assignment operators
///
/// # Example
///
/// ```no_run
/// use dashflow::core::document_loaders::RLoader;
/// use dashflow::core::documents::DocumentLoader;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let loader = RLoader::new("analysis.R").with_separate_definitions(true);
/// let docs = loader.load().await?;
/// for doc in docs {
///     println!("Content: {}", doc.page_content);
/// }
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone)]

// ============================================================================
// R Loader
// ============================================================================

pub struct RLoader {
    file_path: PathBuf,
    separate_definitions: bool,
}

impl RLoader {
    /// Creates a new R loader for the given file path
    pub fn new<P: AsRef<Path>>(path: P) -> Self {
        Self {
            file_path: path.as_ref().to_path_buf(),
            separate_definitions: false,
        }
    }

    /// Enable separation by function definitions
    #[must_use]
    pub fn with_separate_definitions(mut self, separate: bool) -> Self {
        self.separate_definitions = separate;
        self
    }
}

#[async_trait]
impl DocumentLoader for RLoader {
    async fn load(&self) -> Result<Vec<Document>> {
        let blob = Blob::from_path(&self.file_path);
        let content = blob.as_string()?;

        if self.separate_definitions {
            let mut documents = Vec::new();
            let mut current_function = String::new();
            let mut function_name = String::new();
            let mut brace_count = 0;
            let mut in_function = false;

            for line in content.lines() {
                let trimmed = line.trim();

                // Skip comment lines
                if trimmed.starts_with('#') {
                    continue;
                }

                // Check for function definition start
                if in_function {
                    current_function.push_str(line);
                    current_function.push('\n');

                    // Track braces
                    brace_count += line.matches('{').count() as i32;
                    brace_count -= line.matches('}').count() as i32;

                    // Check for function end
                    if brace_count == 0 {
                        // Create document for completed function
                        let doc = Document::new(current_function.trim_end())
                            .with_metadata("source", self.file_path.display().to_string())
                            .with_metadata("format", "r")
                            .with_metadata("definition_index", documents.len())
                            .with_metadata("definition_name", function_name.clone());

                        documents.push(doc);

                        current_function.clear();
                        function_name.clear();
                        in_function = false;
                    }
                } else if let Some(name) = Self::extract_function_name(trimmed) {
                    in_function = true;
                    function_name = name;
                    current_function.push_str(line);
                    current_function.push('\n');

                    // Count braces in this line
                    brace_count += line.matches('{').count() as i32;
                    brace_count -= line.matches('}').count() as i32;

                    // Check if function definition is complete on single line
                    if brace_count == 0 && (line.contains("function(") && !line.contains('{')) {
                        // Single-line function, complete it
                        let doc = Document::new(current_function.trim_end())
                            .with_metadata("source", self.file_path.display().to_string())
                            .with_metadata("format", "r")
                            .with_metadata("definition_index", documents.len())
                            .with_metadata("definition_name", function_name.clone());

                        documents.push(doc);

                        current_function.clear();
                        function_name.clear();
                        in_function = false;
                    }
                    continue;
                }
            }

            // If still in function at EOF, save it
            if !current_function.is_empty() {
                let doc = Document::new(current_function.trim_end())
                    .with_metadata("source", self.file_path.display().to_string())
                    .with_metadata("format", "r")
                    .with_metadata("definition_index", documents.len())
                    .with_metadata("definition_name", function_name);

                documents.push(doc);
            }

            Ok(documents)
        } else {
            // Load as single document
            let doc = Document::new(content)
                .with_metadata("source", self.file_path.display().to_string())
                .with_metadata("format", "r");

            Ok(vec![doc])
        }
    }
}

impl RLoader {
    /// Extract function name from R function definitions
    fn extract_function_name(line: &str) -> Option<String> {
        let line = line.trim();

        // Match: name <- function(...) or name = function(...)
        for op in &[" <- function(", " = function(", "<-function(", "=function("] {
            if let Some(pos) = line.find(op) {
                let name = line[..pos].trim();
                if !name.is_empty() && name.chars().next()?.is_alphabetic() {
                    return Some(name.to_string());
                }
            }
        }

        None
    }
}

/// Loader for Julia source files (.jl)
///
/// Julia is a high-level, high-performance language for technical computing. Features:
/// - Multiple dispatch
/// - Optional type annotations
/// - Metaprogramming with macros
/// - Built-in parallel and distributed computing
/// - Excellent performance for numerical computing
///
/// Supports:
/// - Loading entire file as single document
/// - Optional separation by function, struct, module, and macro definitions
/// - Multi-line and single-line function syntax
///
/// # Example
///
/// ```no_run
/// use dashflow::core::document_loaders::JuliaLoader;
/// use dashflow::core::documents::DocumentLoader;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let loader = JuliaLoader::new("analysis.jl").with_separate_definitions(true);
/// let docs = loader.load().await?;
/// for doc in docs {
///     println!("Content: {}", doc.page_content);
/// }
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone)]

// ============================================================================
// Julia Loader
// ============================================================================

pub struct JuliaLoader {
    file_path: PathBuf,
    separate_definitions: bool,
}

impl JuliaLoader {
    /// Creates a new Julia loader for the given file path
    pub fn new<P: AsRef<Path>>(path: P) -> Self {
        Self {
            file_path: path.as_ref().to_path_buf(),
            separate_definitions: false,
        }
    }

    /// Enable separation by function/struct/module/macro definitions
    #[must_use]
    pub fn with_separate_definitions(mut self, separate: bool) -> Self {
        self.separate_definitions = separate;
        self
    }
}

#[async_trait]
impl DocumentLoader for JuliaLoader {
    async fn load(&self) -> Result<Vec<Document>> {
        let blob = Blob::from_path(&self.file_path);
        let content = blob.as_string()?;

        if self.separate_definitions {
            let mut documents = Vec::new();
            let mut current_definition = String::new();
            let mut definition_name = String::new();
            let mut in_definition = false;

            for line in content.lines() {
                let trimmed = line.trim();

                // Skip comment lines
                if trimmed.starts_with('#') {
                    continue;
                }

                // Check for definition start
                if in_definition {
                    current_definition.push_str(line);
                    current_definition.push('\n');

                    // Check for definition end
                    if trimmed == "end"
                        || trimmed.starts_with("end ")
                        || trimmed.starts_with("end#")
                    {
                        // Create document for completed definition
                        let doc = Document::new(current_definition.trim_end())
                            .with_metadata("source", self.file_path.display().to_string())
                            .with_metadata("format", "julia")
                            .with_metadata("definition_index", documents.len())
                            .with_metadata("definition_name", definition_name.clone());

                        documents.push(doc);

                        current_definition.clear();
                        definition_name.clear();
                        in_definition = false;
                    }
                } else if let Some((keyword, name)) = Self::extract_definition(trimmed) {
                    in_definition = true;
                    definition_name = format!("{keyword} {name}");
                    current_definition.push_str(line);
                    current_definition.push('\n');

                    // Check if single-line function (e.g., f(x) = x^2)
                    if trimmed.contains('=')
                        && !trimmed.contains(" function ")
                        && !trimmed.contains("struct ")
                    {
                        // Single-line function definition
                        let doc = Document::new(current_definition.trim_end())
                            .with_metadata("source", self.file_path.display().to_string())
                            .with_metadata("format", "julia")
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

            // If still in definition at EOF, save it
            if !current_definition.is_empty() {
                let doc = Document::new(current_definition.trim_end())
                    .with_metadata("source", self.file_path.display().to_string())
                    .with_metadata("format", "julia")
                    .with_metadata("definition_index", documents.len())
                    .with_metadata("definition_name", definition_name);

                documents.push(doc);
            }

            Ok(documents)
        } else {
            // Load as single document
            let doc = Document::new(content)
                .with_metadata("source", self.file_path.display().to_string())
                .with_metadata("format", "julia");

            Ok(vec![doc])
        }
    }
}

impl JuliaLoader {
    /// Extract definition keyword and name from Julia definitions
    fn extract_definition(line: &str) -> Option<(String, String)> {
        let line = line.trim();

        // Match: function name(...), struct Name, module Name, macro name(...), mutable struct Name
        for keyword in &[
            "function ",
            "struct ",
            "module ",
            "macro ",
            "mutable struct ",
        ] {
            if let Some(pos) = line.find(keyword) {
                let after_keyword = &line[pos + keyword.len()..];
                // Extract name (up to space, parenthesis, or <:)
                if let Some(end) =
                    after_keyword.find(|c: char| c.is_whitespace() || c == '(' || c == '<')
                {
                    let name = after_keyword[..end].trim().to_string();
                    return Some((keyword.trim().to_string(), name));
                } else if !after_keyword.is_empty() {
                    // Name until end of line
                    let name = after_keyword.split_whitespace().next()?.to_string();
                    return Some((keyword.trim().to_string(), name));
                }
            }
        }

        // Match: name(args...) = expr (single-line function)
        if line.contains('(') && line.contains(") =") {
            if let Some(paren_pos) = line.find('(') {
                let name = line[..paren_pos].trim();
                if !name.is_empty() && name.chars().next()?.is_alphabetic() {
                    return Some(("function".to_string(), name.to_string()));
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

    // =====================
    // PhpLoader Tests
    // =====================

    #[tokio::test]
    async fn test_php_loader_basic() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, "<?php").unwrap();
        writeln!(file, "function greet() {{").unwrap();
        writeln!(file, "    echo 'Hello';").unwrap();
        writeln!(file, "}}").unwrap();

        let loader = PhpLoader::new(file.path());
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1);
        assert!(docs[0].page_content.contains("greet"));
        assert_eq!(
            docs[0].metadata.get("format").unwrap().as_str().unwrap(),
            "php"
        );
    }

    #[tokio::test]
    async fn test_php_loader_separate_definitions() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, "<?php").unwrap();
        writeln!(file, "function foo() {{").unwrap();
        writeln!(file, "    return 1;").unwrap();
        writeln!(file, "}}").unwrap();
        writeln!(file, "function bar() {{").unwrap();
        writeln!(file, "    return 2;").unwrap();
        writeln!(file, "}}").unwrap();

        let loader = PhpLoader::new(file.path()).with_separate_definitions(true);
        let docs = loader.load().await.unwrap();

        // Should have 2 functions + possibly global code
        assert!(docs.len() >= 2);
    }

    #[tokio::test]
    async fn test_php_loader_class() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, "<?php").unwrap();
        writeln!(file, "class MyClass {{").unwrap();
        writeln!(file, "    public function method() {{}}").unwrap();
        writeln!(file, "}}").unwrap();

        let loader = PhpLoader::new(file.path()).with_separate_definitions(true);
        let docs = loader.load().await.unwrap();

        assert!(docs
            .iter()
            .any(|d| d.page_content.contains("class MyClass")));
    }

    #[tokio::test]
    async fn test_php_loader_metadata() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, "<?php echo 'test';").unwrap();

        let loader = PhpLoader::new(file.path());
        let docs = loader.load().await.unwrap();

        assert!(docs[0].metadata.contains_key("source"));
        assert!(docs[0].metadata.contains_key("format"));
    }

    #[test]
    fn test_php_loader_builder_chain() {
        let loader = PhpLoader::new("test.php").with_separate_definitions(true);

        assert!(loader.separate_definitions);
    }

    // =====================
    // RubyLoader Tests
    // =====================

    #[tokio::test]
    async fn test_ruby_loader_basic() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, "def greet").unwrap();
        writeln!(file, "  puts 'Hello'").unwrap();
        writeln!(file, "end").unwrap();

        let loader = RubyLoader::new(file.path());
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1);
        assert!(docs[0].page_content.contains("greet"));
        assert_eq!(
            docs[0].metadata.get("format").unwrap().as_str().unwrap(),
            "ruby"
        );
    }

    #[tokio::test]
    async fn test_ruby_loader_separate_definitions() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, "def foo").unwrap();
        writeln!(file, "  1").unwrap();
        writeln!(file, "end").unwrap();
        writeln!(file, "def bar").unwrap();
        writeln!(file, "  2").unwrap();
        writeln!(file, "end").unwrap();

        let loader = RubyLoader::new(file.path()).with_separate_definitions(true);
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 2);
    }

    #[tokio::test]
    async fn test_ruby_loader_class() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, "class MyClass").unwrap();
        writeln!(file, "  def method").unwrap();
        writeln!(file, "    'hello'").unwrap();
        writeln!(file, "  end").unwrap();
        writeln!(file, "end").unwrap();

        let loader = RubyLoader::new(file.path()).with_separate_definitions(true);
        let docs = loader.load().await.unwrap();

        assert!(docs
            .iter()
            .any(|d| d.page_content.contains("class MyClass")));
    }

    #[tokio::test]
    async fn test_ruby_loader_module() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, "module MyModule").unwrap();
        writeln!(file, "  def helper").unwrap();
        writeln!(file, "    true").unwrap();
        writeln!(file, "  end").unwrap();
        writeln!(file, "end").unwrap();

        let loader = RubyLoader::new(file.path()).with_separate_definitions(true);
        let docs = loader.load().await.unwrap();

        assert!(docs
            .iter()
            .any(|d| d.page_content.contains("module MyModule")));
    }

    #[test]
    fn test_ruby_loader_builder_chain() {
        let loader = RubyLoader::new("test.rb").with_separate_definitions(true);

        assert!(loader.separate_definitions);
    }

    // =====================
    // PerlLoader Tests
    // =====================

    #[tokio::test]
    async fn test_perl_loader_basic() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, "#!/usr/bin/perl").unwrap();
        writeln!(file, "sub greet {{").unwrap();
        writeln!(file, "    print 'Hello';").unwrap();
        writeln!(file, "}}").unwrap();

        let loader = PerlLoader::new(file.path());
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1);
        assert!(docs[0].page_content.contains("greet"));
        assert_eq!(
            docs[0].metadata.get("format").unwrap().as_str().unwrap(),
            "perl"
        );
    }

    #[tokio::test]
    async fn test_perl_loader_separate_definitions() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, "sub foo {{").unwrap();
        writeln!(file, "    return 1;").unwrap();
        writeln!(file, "}}").unwrap();
        writeln!(file, "sub bar {{").unwrap();
        writeln!(file, "    return 2;").unwrap();
        writeln!(file, "}}").unwrap();

        let loader = PerlLoader::new(file.path()).with_separate_definitions(true);
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 2);
    }

    #[tokio::test]
    async fn test_perl_loader_package() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, "package MyPackage {{").unwrap();
        writeln!(file, "    sub method {{}}").unwrap();
        writeln!(file, "}}").unwrap();

        let loader = PerlLoader::new(file.path()).with_separate_definitions(true);
        let docs = loader.load().await.unwrap();

        assert!(docs
            .iter()
            .any(|d| d.page_content.contains("package MyPackage")));
    }

    #[test]
    fn test_perl_loader_builder_chain() {
        let loader = PerlLoader::new("test.pl").with_separate_definitions(true);

        assert!(loader.separate_definitions);
    }

    // =====================
    // LuaLoader Tests
    // =====================

    #[tokio::test]
    async fn test_lua_loader_basic() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, "function greet()").unwrap();
        writeln!(file, "    print('Hello')").unwrap();
        writeln!(file, "end").unwrap();

        let loader = LuaLoader::new(file.path());
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1);
        assert!(docs[0].page_content.contains("greet"));
        assert_eq!(
            docs[0].metadata.get("format").unwrap().as_str().unwrap(),
            "lua"
        );
    }

    #[tokio::test]
    async fn test_lua_loader_separate_definitions() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, "function foo()").unwrap();
        writeln!(file, "    return 1").unwrap();
        writeln!(file, "end").unwrap();
        writeln!(file, "function bar()").unwrap();
        writeln!(file, "    return 2").unwrap();
        writeln!(file, "end").unwrap();

        let loader = LuaLoader::new(file.path()).with_separate_definitions(true);
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 2);
    }

    #[tokio::test]
    async fn test_lua_loader_local_function() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, "local function helper()").unwrap();
        writeln!(file, "    return true").unwrap();
        writeln!(file, "end").unwrap();

        let loader = LuaLoader::new(file.path()).with_separate_definitions(true);
        let docs = loader.load().await.unwrap();

        assert!(docs.iter().any(|d| d.page_content.contains("helper")));
    }

    #[test]
    fn test_lua_loader_builder_chain() {
        let loader = LuaLoader::new("test.lua").with_separate_definitions(true);

        assert!(loader.separate_definitions);
    }

    #[test]
    fn test_lua_extract_function_name() {
        // Standard function
        assert_eq!(
            LuaLoader::extract_function_name("function foo()"),
            Some("foo".to_string())
        );

        // Local function
        assert!(LuaLoader::extract_function_name("local function bar()").is_some());

        // Not a function
        assert_eq!(LuaLoader::extract_function_name("local x = 5"), None);
    }

    // =====================
    // RLoader Tests
    // =====================

    #[tokio::test]
    async fn test_r_loader_basic() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, "greet <- function() {{").unwrap();
        writeln!(file, "    print('Hello')").unwrap();
        writeln!(file, "}}").unwrap();

        let loader = RLoader::new(file.path());
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1);
        assert!(docs[0].page_content.contains("greet"));
        assert_eq!(
            docs[0].metadata.get("format").unwrap().as_str().unwrap(),
            "r"
        );
    }

    #[tokio::test]
    async fn test_r_loader_separate_definitions() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, "foo <- function() {{").unwrap();
        writeln!(file, "    1").unwrap();
        writeln!(file, "}}").unwrap();
        writeln!(file, "bar <- function() {{").unwrap();
        writeln!(file, "    2").unwrap();
        writeln!(file, "}}").unwrap();

        let loader = RLoader::new(file.path()).with_separate_definitions(true);
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 2);
    }

    #[tokio::test]
    async fn test_r_loader_equals_assignment() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, "helper = function() {{").unwrap();
        writeln!(file, "    TRUE").unwrap();
        writeln!(file, "}}").unwrap();

        let loader = RLoader::new(file.path()).with_separate_definitions(true);
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1);
        assert!(docs[0].page_content.contains("helper"));
    }

    #[test]
    fn test_r_loader_builder_chain() {
        let loader = RLoader::new("test.R").with_separate_definitions(true);

        assert!(loader.separate_definitions);
    }

    #[test]
    fn test_r_extract_function_name() {
        // Arrow assignment
        assert_eq!(
            RLoader::extract_function_name("foo <- function()"),
            Some("foo".to_string())
        );

        // Equals assignment
        assert_eq!(
            RLoader::extract_function_name("bar = function()"),
            Some("bar".to_string())
        );

        // Not a function
        assert_eq!(RLoader::extract_function_name("x <- 5"), None);
    }

    // =====================
    // JuliaLoader Tests
    // =====================

    #[tokio::test]
    async fn test_julia_loader_basic() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, "function greet()").unwrap();
        writeln!(file, "    println(\"Hello\")").unwrap();
        writeln!(file, "end").unwrap();

        let loader = JuliaLoader::new(file.path());
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1);
        assert!(docs[0].page_content.contains("greet"));
        assert_eq!(
            docs[0].metadata.get("format").unwrap().as_str().unwrap(),
            "julia"
        );
    }

    #[tokio::test]
    async fn test_julia_loader_separate_definitions() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, "function foo()").unwrap();
        writeln!(file, "    1").unwrap();
        writeln!(file, "end").unwrap();
        writeln!(file, "function bar()").unwrap();
        writeln!(file, "    2").unwrap();
        writeln!(file, "end").unwrap();

        let loader = JuliaLoader::new(file.path()).with_separate_definitions(true);
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 2);
    }

    #[tokio::test]
    async fn test_julia_loader_struct() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, "struct Point").unwrap();
        writeln!(file, "    x::Float64").unwrap();
        writeln!(file, "    y::Float64").unwrap();
        writeln!(file, "end").unwrap();

        let loader = JuliaLoader::new(file.path()).with_separate_definitions(true);
        let docs = loader.load().await.unwrap();

        assert!(docs.iter().any(|d| d.page_content.contains("struct Point")));
    }

    #[tokio::test]
    async fn test_julia_loader_module() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, "module MyModule").unwrap();
        writeln!(file, "    export foo").unwrap();
        writeln!(file, "end").unwrap();

        let loader = JuliaLoader::new(file.path()).with_separate_definitions(true);
        let docs = loader.load().await.unwrap();

        assert!(docs
            .iter()
            .any(|d| d.page_content.contains("module MyModule")));
    }

    #[test]
    fn test_julia_loader_builder_chain() {
        let loader = JuliaLoader::new("test.jl").with_separate_definitions(true);

        assert!(loader.separate_definitions);
    }

    #[test]
    fn test_julia_extract_definition() {
        // Function
        assert_eq!(
            JuliaLoader::extract_definition("function foo()"),
            Some(("function".to_string(), "foo".to_string()))
        );

        // Struct
        assert_eq!(
            JuliaLoader::extract_definition("struct Point"),
            Some(("struct".to_string(), "Point".to_string()))
        );

        // Module
        assert_eq!(
            JuliaLoader::extract_definition("module MyModule"),
            Some(("module".to_string(), "MyModule".to_string()))
        );

        // Single-line function
        let result = JuliaLoader::extract_definition("square(x) = x^2");
        assert!(result.is_some());

        // Not a definition
        assert_eq!(JuliaLoader::extract_definition("x = 5"), None);
    }

    // =====================
    // Empty File Tests
    // =====================

    #[tokio::test]
    async fn test_php_loader_empty_file() {
        let file = NamedTempFile::new().unwrap();

        let loader = PhpLoader::new(file.path());
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1);
        assert!(docs[0].page_content.is_empty());
    }

    #[tokio::test]
    async fn test_ruby_loader_empty_file() {
        let file = NamedTempFile::new().unwrap();

        let loader = RubyLoader::new(file.path());
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1);
        assert!(docs[0].page_content.is_empty());
    }

    #[tokio::test]
    async fn test_perl_loader_empty_file() {
        let file = NamedTempFile::new().unwrap();

        let loader = PerlLoader::new(file.path());
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1);
        assert!(docs[0].page_content.is_empty());
    }

    #[tokio::test]
    async fn test_lua_loader_empty_file() {
        let file = NamedTempFile::new().unwrap();

        let loader = LuaLoader::new(file.path());
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1);
        assert!(docs[0].page_content.is_empty());
    }

    #[tokio::test]
    async fn test_r_loader_empty_file() {
        let file = NamedTempFile::new().unwrap();

        let loader = RLoader::new(file.path());
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1);
        assert!(docs[0].page_content.is_empty());
    }

    #[tokio::test]
    async fn test_julia_loader_empty_file() {
        let file = NamedTempFile::new().unwrap();

        let loader = JuliaLoader::new(file.path());
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1);
        assert!(docs[0].page_content.is_empty());
    }
}
