//! Scripting language document loaders.
//!
//! This module provides loaders for scripting languages:
//! - Python
//! - JavaScript
//! - TypeScript
//! - Bash
//! - `PowerShell`
//! - Fish
//! - Zsh

use async_trait::async_trait;
use std::path::{Path, PathBuf};

use crate::core::documents::{Blob, Document, DocumentLoader};
use crate::core::error::Result;

/// `PythonFileLoader` loads Python source files and separates them by function/class definitions.
///
/// Python is a high-level, general-purpose programming language known for its readability
/// and versatility. Created by Guido van Rossum in 1991, Python emphasizes code clarity
/// with its significant whitespace syntax.
///
/// Supports extensions: .py, .pyw
///
/// When `separate_definitions` is true, splits document by `def` and `class` declarations.
/// Python syntax: `def function_name():` and `class ClassName:`
///
/// # Example
///
/// ```no_run
/// use dashflow::core::document_loaders::PythonFileLoader;
/// use dashflow::core::documents::DocumentLoader;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let loader = PythonFileLoader::new("script.py").with_separate_definitions(true);
/// let docs = loader.load().await?;
/// println!("Loaded {} definitions", docs.len());
/// # Ok(())
/// # }
/// ```
pub struct PythonFileLoader {
    /// Path to the Python file
    pub file_path: PathBuf,
    /// Extract docstrings and comments separately (default: false)
    pub extract_docstrings: bool,
    /// Separate documents per function/class (default: false)
    pub separate_definitions: bool,
}

impl PythonFileLoader {
    /// Create a new `PythonFileLoader` for the given file path.
    pub fn new(file_path: impl AsRef<Path>) -> Self {
        Self {
            file_path: file_path.as_ref().to_path_buf(),
            extract_docstrings: false,
            separate_definitions: false,
        }
    }

    /// Extract docstrings and comments separately.
    #[must_use]
    pub fn with_extract_docstrings(mut self, extract: bool) -> Self {
        self.extract_docstrings = extract;
        self
    }

    /// Create separate documents per function/class definition.
    #[must_use]
    pub fn with_separate_definitions(mut self, separate: bool) -> Self {
        self.separate_definitions = separate;
        self
    }
}

#[async_trait]
impl DocumentLoader for PythonFileLoader {
    async fn load(&self) -> Result<Vec<Document>> {
        let blob = Blob::from_path(&self.file_path);
        let content = blob.as_string()?;

        if self.separate_definitions {
            // Simple heuristic: split by "def " and "class " at line start
            let mut documents = Vec::new();
            let mut current_def = String::new();
            let mut def_name = String::new();
            let mut def_index = 0;

            for line in content.lines() {
                let trimmed = line.trim_start();

                if (trimmed.starts_with("def ") || trimmed.starts_with("class "))
                    && !current_def.is_empty()
                {
                    // Save previous definition
                    let doc = Document::new(current_def.clone())
                        .with_metadata("source", self.file_path.display().to_string())
                        .with_metadata("definition_index", def_index)
                        .with_metadata("definition_name", def_name.clone())
                        .with_metadata("format", "python");

                    documents.push(doc);
                    current_def.clear();
                    def_index += 1;
                }

                if trimmed.starts_with("def ") || trimmed.starts_with("class ") {
                    // Extract definition name
                    let parts: Vec<&str> = trimmed.split_whitespace().collect();
                    if parts.len() >= 2 {
                        // Extract name before '(' or ':'
                        let name_part = parts[1];
                        if let Some(paren_pos) = name_part.find('(') {
                            def_name = name_part[..paren_pos].to_string();
                        } else {
                            def_name = name_part.trim_end_matches(':').to_string();
                        }
                    }
                }

                current_def.push_str(line);
                current_def.push('\n');
            }

            // Add last definition
            if !current_def.is_empty() {
                let doc = Document::new(current_def)
                    .with_metadata("source", self.file_path.display().to_string())
                    .with_metadata("definition_index", def_index)
                    .with_metadata("definition_name", def_name)
                    .with_metadata("format", "python");

                documents.push(doc);
            }

            Ok(documents)
        } else if self.extract_docstrings {
            // Extract docstrings (triple-quoted strings)
            let mut documents = Vec::new();
            let mut in_docstring = false;
            let mut docstring = String::new();
            let mut code = String::new();
            let mut docstring_count = 0;

            for line in content.lines() {
                let trimmed = line.trim();

                if trimmed.starts_with("\"\"\"") || trimmed.starts_with("'''") {
                    if in_docstring {
                        // End of docstring
                        in_docstring = false;
                        docstring.push_str(line);
                        docstring.push('\n');

                        let doc = Document::new(docstring.clone())
                            .with_metadata("source", self.file_path.display().to_string())
                            .with_metadata("type", "docstring")
                            .with_metadata("docstring_index", docstring_count)
                            .with_metadata("format", "python");

                        documents.push(doc);
                        docstring.clear();
                        docstring_count += 1;
                    } else {
                        // Start of docstring
                        in_docstring = true;
                        docstring.push_str(line);
                        docstring.push('\n');
                    }
                } else if in_docstring {
                    docstring.push_str(line);
                    docstring.push('\n');
                } else {
                    code.push_str(line);
                    code.push('\n');
                }
            }

            // Add code as document
            if !code.is_empty() {
                let doc = Document::new(code)
                    .with_metadata("source", self.file_path.display().to_string())
                    .with_metadata("type", "code")
                    .with_metadata("format", "python");

                documents.push(doc);
            }

            Ok(documents)
        } else {
            // Load as single document
            let doc = Document::new(content)
                .with_metadata("source", self.file_path.display().to_string())
                .with_metadata("format", "python");

            Ok(vec![doc])
        }
    }
}

/// Loads reStructuredText (.rst) documentation files.
///
/// The `RSTLoader` reads .rst files commonly used in Python documentation.
/// Can parse sections and directives.
///
/// # Example
///
/// ```no_run
/// use dashflow::core::document_loaders::RSTLoader;
/// use dashflow::core::documents::DocumentLoader;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let loader = RSTLoader::new("README.rst");
/// let documents = loader.load().await?;
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone)]

// ============================================================================
// JavaScript Loader
// ============================================================================

pub struct JavaScriptLoader {
    /// Path to the JavaScript file
    pub file_path: PathBuf,
    /// Separate documents per function (default: false)
    pub separate_functions: bool,
}

impl JavaScriptLoader {
    /// Create a new `JavaScriptLoader` for the given file path.
    pub fn new(file_path: impl AsRef<Path>) -> Self {
        Self {
            file_path: file_path.as_ref().to_path_buf(),
            separate_functions: false,
        }
    }

    /// Create separate documents per function definition.
    #[must_use]
    pub fn with_separate_functions(mut self, separate: bool) -> Self {
        self.separate_functions = separate;
        self
    }
}

#[async_trait]
impl DocumentLoader for JavaScriptLoader {
    async fn load(&self) -> Result<Vec<Document>> {
        let blob = Blob::from_path(&self.file_path);
        let content = blob.as_string()?;

        if self.separate_functions {
            // Split by "function " or "const/let/var name = " patterns
            let mut documents = Vec::new();
            let mut current_fn = String::new();
            let mut fn_name = String::new();
            let mut fn_index = 0;
            let mut brace_depth = 0;
            let mut in_function = false;

            for line in content.lines() {
                let trimmed = line.trim_start();

                // Detect function declarations
                if !in_function
                    && (trimmed.starts_with("function ")
                        || trimmed.starts_with("async function ")
                        || trimmed.contains("= function")
                        || trimmed.contains("=> "))
                {
                    in_function = true;
                    brace_depth = 0;

                    // Extract function name
                    if trimmed.starts_with("function ") || trimmed.starts_with("async function ") {
                        let parts: Vec<&str> = trimmed.split_whitespace().collect();
                        // For "function name()", parts[1] is the name
                        // For "async function name()", parts[2] is the name
                        let name_idx = if trimmed.starts_with("async ") { 2 } else { 1 };
                        if parts.len() > name_idx {
                            if let Some(paren_pos) = parts[name_idx].find('(') {
                                fn_name = parts[name_idx][..paren_pos].to_string();
                            } else {
                                fn_name = parts[name_idx].to_string();
                            }
                        }
                    } else if let Some(eq_pos) = trimmed.find('=') {
                        let name_part = &trimmed[..eq_pos].trim();
                        let name_words: Vec<&str> = name_part.split_whitespace().collect();
                        if let Some(last) = name_words.last() {
                            fn_name = (*last).to_string();
                        }
                    }
                }

                if in_function {
                    current_fn.push_str(line);
                    current_fn.push('\n');

                    // Track braces
                    for ch in line.chars() {
                        if ch == '{' {
                            brace_depth += 1;
                        } else if ch == '}' {
                            brace_depth -= 1;
                            if brace_depth == 0 {
                                // Function complete
                                let doc = Document::new(current_fn.clone())
                                    .with_metadata("source", self.file_path.display().to_string())
                                    .with_metadata("function_index", fn_index)
                                    .with_metadata("function_name", fn_name.clone())
                                    .with_metadata("format", "javascript");

                                documents.push(doc);
                                current_fn.clear();
                                in_function = false;
                                fn_index += 1;
                                break;
                            }
                        }
                    }
                } else if !trimmed.is_empty() {
                    // Non-function code (imports, comments, etc.)
                    current_fn.push_str(line);
                    current_fn.push('\n');
                }
            }

            // Add any remaining content
            if !current_fn.is_empty() {
                let doc = Document::new(current_fn)
                    .with_metadata("source", self.file_path.display().to_string())
                    .with_metadata("type", "global")
                    .with_metadata("format", "javascript");

                documents.push(doc);
            }

            Ok(documents)
        } else {
            // Load as single document
            let doc = Document::new(content)
                .with_metadata("source", self.file_path.display().to_string())
                .with_metadata("format", "javascript");

            Ok(vec![doc])
        }
    }
}

/// Loads TypeScript source files (.ts).
///
/// The `TypeScriptLoader` reads TypeScript source files, preserving all code and type annotations.
/// Can optionally separate by function/class definitions.
///
/// # Example
///
/// ```no_run
/// use dashflow::core::document_loaders::TypeScriptLoader;
/// use dashflow::core::documents::DocumentLoader;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let loader = TypeScriptLoader::new("app.ts");
/// let documents = loader.load().await?;
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone)]

// ============================================================================
// TypeScript Loader
// ============================================================================

pub struct TypeScriptLoader {
    /// Path to the TypeScript file
    pub file_path: PathBuf,
    /// Separate documents per function/class (default: false)
    pub separate_definitions: bool,
}

impl TypeScriptLoader {
    /// Create a new `TypeScriptLoader` for the given file path.
    pub fn new(file_path: impl AsRef<Path>) -> Self {
        Self {
            file_path: file_path.as_ref().to_path_buf(),
            separate_definitions: false,
        }
    }

    /// Create separate documents per function/class definition.
    #[must_use]
    pub fn with_separate_definitions(mut self, separate: bool) -> Self {
        self.separate_definitions = separate;
        self
    }
}

#[async_trait]
impl DocumentLoader for TypeScriptLoader {
    async fn load(&self) -> Result<Vec<Document>> {
        let blob = Blob::from_path(&self.file_path);
        let content = blob.as_string()?;

        if self.separate_definitions {
            // Similar to JavaScript but also handle TypeScript-specific syntax
            let mut documents = Vec::new();
            let mut current_def = String::new();
            let mut def_name = String::new();
            let mut def_index = 0;
            let mut brace_depth = 0;
            let mut in_definition = false;

            for line in content.lines() {
                let trimmed = line.trim_start();

                // Detect function/class/interface/type declarations
                if !in_definition
                    && (trimmed.starts_with("function ")
                        || trimmed.starts_with("async function ")
                        || trimmed.starts_with("class ")
                        || trimmed.starts_with("interface ")
                        || trimmed.starts_with("type ")
                        || trimmed.starts_with("export function ")
                        || trimmed.starts_with("export class ")
                        || trimmed.starts_with("export interface ")
                        || trimmed.starts_with("export type ")
                        || trimmed.contains("= function")
                        || trimmed.contains("=> "))
                {
                    in_definition = true;
                    brace_depth = 0;

                    // Extract definition name
                    let words: Vec<&str> = trimmed.split_whitespace().collect();
                    for (i, &word) in words.iter().enumerate() {
                        if matches!(word, "function" | "class" | "interface" | "type")
                            && i + 1 < words.len()
                        {
                            let name_part = words[i + 1];
                            if let Some(paren_pos) = name_part.find('(') {
                                def_name = name_part[..paren_pos].to_string();
                            } else if let Some(angle_pos) = name_part.find('<') {
                                def_name = name_part[..angle_pos].to_string();
                            } else {
                                def_name = name_part.trim_end_matches(['{', '=']).to_string();
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
                                    .with_metadata("format", "typescript");

                                documents.push(doc);
                                current_def.clear();
                                in_definition = false;
                                def_index += 1;
                                break;
                            }
                        }
                    }
                } else if !trimmed.is_empty() {
                    // Non-definition code
                    current_def.push_str(line);
                    current_def.push('\n');
                }
            }

            // Add any remaining content
            if !current_def.is_empty() {
                let doc = Document::new(current_def)
                    .with_metadata("source", self.file_path.display().to_string())
                    .with_metadata("type", "global")
                    .with_metadata("format", "typescript");

                documents.push(doc);
            }

            Ok(documents)
        } else {
            // Load as single document
            let doc = Document::new(content)
                .with_metadata("source", self.file_path.display().to_string())
                .with_metadata("format", "typescript");

            Ok(vec![doc])
        }
    }
}

/// Loads Rust source files (.rs).
///
/// The `RustFileLoader` reads Rust source files, preserving all code structure.
/// Can optionally separate by function, struct, enum, trait, or impl blocks.
///
/// # Example
///
/// ```no_run
/// use dashflow::core::document_loaders::RustFileLoader;
/// use dashflow::core::documents::DocumentLoader;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let loader = RustFileLoader::new("lib.rs");
/// let documents = loader.load().await?;
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone)]

// ============================================================================
// Bash Script Loader
// ============================================================================

pub struct BashScriptLoader {
    /// Path to the bash script file
    pub file_path: PathBuf,
    /// Separate documents per function (default: false)
    pub separate_functions: bool,
}

impl BashScriptLoader {
    /// Create a new `BashScriptLoader` for the given file path.
    pub fn new(file_path: impl AsRef<Path>) -> Self {
        Self {
            file_path: file_path.as_ref().to_path_buf(),
            separate_functions: false,
        }
    }

    /// Create separate documents per bash function definition.
    #[must_use]
    pub fn with_separate_functions(mut self, separate: bool) -> Self {
        self.separate_functions = separate;
        self
    }
}

#[async_trait]
impl DocumentLoader for BashScriptLoader {
    async fn load(&self) -> Result<Vec<Document>> {
        let blob = Blob::from_path(&self.file_path);
        let content = blob.as_string()?;

        if self.separate_functions {
            // Split by function definitions (function name() or name())
            let mut documents = Vec::new();
            let mut current_fn = String::new();
            let mut fn_name = String::new();
            let mut fn_index = 0;
            let mut brace_depth = 0;
            let mut in_function = false;

            for line in content.lines() {
                let trimmed = line.trim();

                // Detect function declarations: "function name()" or "name()"
                if !in_function {
                    let is_function = if trimmed.starts_with("function ") {
                        // "function name()" or "function name {"
                        let parts: Vec<&str> = trimmed.split_whitespace().collect();
                        if parts.len() >= 2 {
                            let name_part = parts[1];
                            if let Some(paren_pos) = name_part.find('(') {
                                fn_name = name_part[..paren_pos].to_string();
                            } else {
                                fn_name = name_part.to_string();
                            }
                            true
                        } else {
                            false
                        }
                    } else if trimmed.contains("()") {
                        // "name()" format
                        if let Some(paren_pos) = trimmed.find("()") {
                            let name_part = &trimmed[..paren_pos];
                            // Ensure it's a valid function name (alphanumeric + underscore)
                            if name_part.chars().all(|c| c.is_alphanumeric() || c == '_') {
                                fn_name = name_part.to_string();
                                true
                            } else {
                                false
                            }
                        } else {
                            false
                        }
                    } else {
                        false
                    };

                    if is_function {
                        in_function = true;
                        brace_depth = 0;
                    }
                }

                if in_function {
                    current_fn.push_str(line);
                    current_fn.push('\n');

                    // Track braces
                    for ch in line.chars() {
                        if ch == '{' {
                            brace_depth += 1;
                        } else if ch == '}' {
                            brace_depth -= 1;
                            if brace_depth == 0 {
                                // Function complete
                                let doc = Document::new(current_fn.clone())
                                    .with_metadata("source", self.file_path.display().to_string())
                                    .with_metadata("function_index", fn_index)
                                    .with_metadata("function_name", fn_name.clone())
                                    .with_metadata("format", "bash");

                                documents.push(doc);
                                current_fn.clear();
                                in_function = false;
                                fn_index += 1;
                                break;
                            }
                        }
                    }
                } else if !trimmed.is_empty() {
                    // Non-function code
                    current_fn.push_str(line);
                    current_fn.push('\n');
                }
            }

            // Add any remaining content
            if !current_fn.is_empty() {
                let doc = Document::new(current_fn)
                    .with_metadata("source", self.file_path.display().to_string())
                    .with_metadata("type", "global")
                    .with_metadata("format", "bash");

                documents.push(doc);
            }

            Ok(documents)
        } else {
            // Load as single document
            let doc = Document::new(content)
                .with_metadata("source", self.file_path.display().to_string())
                .with_metadata("format", "bash");

            Ok(vec![doc])
        }
    }
}

/// Loads Dockerfile container definition files.
///
/// The `DockerfileLoader` reads Dockerfiles used to build container images.
/// Can optionally separate by stage in multi-stage builds.
///
/// # Example
///
/// ```no_run
/// use dashflow::core::document_loaders::DockerfileLoader;
/// use dashflow::core::documents::DocumentLoader;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let loader = DockerfileLoader::new("Dockerfile");
/// let documents = loader.load().await?;
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone)]

// ============================================================================
// PowerShell Loader
// ============================================================================

pub struct PowerShellLoader {
    file_path: PathBuf,
    separate_definitions: bool,
}

impl PowerShellLoader {
    /// Creates a new `PowerShell` loader for the given file path
    pub fn new<P: AsRef<Path>>(path: P) -> Self {
        Self {
            file_path: path.as_ref().to_path_buf(),
            separate_definitions: false,
        }
    }

    /// Enable separation by function/filter/workflow definitions
    #[must_use]
    pub fn with_separate_definitions(mut self, separate: bool) -> Self {
        self.separate_definitions = separate;
        self
    }
}

#[async_trait]
impl DocumentLoader for PowerShellLoader {
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
                if !in_definition && (trimmed.is_empty() || trimmed.starts_with('#')) {
                    continue;
                }

                // Check for definition start (function, filter, workflow)
                if !in_definition {
                    if let Some(name) = Self::extract_definition(trimmed) {
                        in_definition = true;
                        definition_name = name;
                        current_definition.push_str(line);
                        current_definition.push('\n');

                        // Count braces
                        brace_count =
                            line.matches('{').count() as i32 - line.matches('}').count() as i32;

                        // PowerShell functions can be single-line without braces
                        if brace_count == 0 && !line.contains('{') {
                            let doc = Document::new(current_definition.trim_end())
                                .with_metadata("source", self.file_path.display().to_string())
                                .with_metadata("format", "powershell")
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
                            .with_metadata("format", "powershell")
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
                    .with_metadata("format", "powershell")
                    .with_metadata("definition_index", documents.len())
                    .with_metadata("definition_name", definition_name);

                documents.push(doc);
            }

            Ok(documents)
        } else {
            // Load as single document
            let doc = Document::new(content)
                .with_metadata("source", self.file_path.display().to_string())
                .with_metadata("format", "powershell");

            Ok(vec![doc])
        }
    }
}

impl PowerShellLoader {
    /// Extract definition name from `PowerShell` definitions
    fn extract_definition(line: &str) -> Option<String> {
        let line_lower = line.to_lowercase();

        // Match: function Name, filter Name, workflow Name
        if let Some(after) = line_lower.strip_prefix("function ") {
            Self::extract_name(after, "function")
        } else if let Some(after) = line_lower.strip_prefix("filter ") {
            Self::extract_name(after, "filter")
        } else if let Some(after) = line_lower.strip_prefix("workflow ") {
            Self::extract_name(after, "workflow")
        } else {
            None
        }
    }

    fn extract_name(after_keyword: &str, keyword: &str) -> Option<String> {
        // Extract name up to { or ( or whitespace
        if let Some(end) = after_keyword.find(|c: char| c == '{' || c == '(' || c.is_whitespace()) {
            let name = after_keyword[..end].trim().to_string();
            if !name.is_empty() {
                return Some(format!("{keyword} {name}"));
            }
        }
        None
    }
}

/// Loader for Fish shell source files (.fish)
///
/// Fish is a smart and user-friendly command line shell. Features:
/// - Syntax highlighting
/// - Autosuggestions
/// - Web-based configuration
/// - Scripting capabilities
/// - Function definitions
///
/// Supports:
/// - Loading entire Fish file as single document
/// - Optional separation by function definitions
/// - End keyword parsing (like Ruby/Crystal)
///
/// # Example
///
/// ```no_run
/// use dashflow::core::document_loaders::FishLoader;
/// use dashflow::core::documents::DocumentLoader;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let loader = FishLoader::new("config.fish").with_separate_definitions(true);
/// let docs = loader.load().await?;
/// for doc in docs {
///     println!("Content: {}", doc.page_content);
/// }
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone)]

// ============================================================================
// Fish Shell Loader
// ============================================================================

pub struct FishLoader {
    file_path: PathBuf,
    separate_definitions: bool,
}

impl FishLoader {
    /// Creates a new Fish loader for the given file path
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
impl DocumentLoader for FishLoader {
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

                // Check for function definition start
                if !in_definition && trimmed.starts_with("function ") {
                    in_definition = true;
                    definition_name = Self::extract_function_name(trimmed);
                    current_definition.push_str(line);
                    current_definition.push('\n');
                    end_count = 1; // Expecting 1 'end'
                    continue;
                }

                if in_definition {
                    current_definition.push_str(line);
                    current_definition.push('\n');

                    // Track nested structures (function, if, for, while, begin, switch)
                    if trimmed.starts_with("function ")
                        || trimmed.starts_with("if ")
                        || trimmed.starts_with("for ")
                        || trimmed.starts_with("while ")
                        || trimmed == "begin"
                        || trimmed.starts_with("switch ")
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
                                .with_metadata("format", "fish")
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
                    .with_metadata("format", "fish")
                    .with_metadata("definition_index", documents.len())
                    .with_metadata("definition_name", definition_name);

                documents.push(doc);
            }

            Ok(documents)
        } else {
            // Load as single document
            let doc = Document::new(content)
                .with_metadata("source", self.file_path.display().to_string())
                .with_metadata("format", "fish");

            Ok(vec![doc])
        }
    }
}

impl FishLoader {
    /// Extract function name from Fish function definitions
    fn extract_function_name(line: &str) -> String {
        if let Some(after) = line.strip_prefix("function ") {
            // Extract name up to whitespace, semicolon, or newline
            if let Some(end) = after.find(|c: char| c.is_whitespace() || c == ';') {
                format!("function {}", after[..end].trim())
            } else {
                format!("function {}", after.trim())
            }
        } else {
            "function".to_string()
        }
    }
}

/// Loader for Zsh source files (.zsh, .zshrc)
///
/// Zsh is an extended Bourne shell with many improvements. Features:
/// - Advanced scripting
/// - Powerful completion system
/// - Themes and plugins
/// - Function definitions
/// - Compatible with bash
///
/// Supports:
/// - Loading entire Zsh file as single document
/// - Optional separation by function definitions
/// - Brace-based and function keyword parsing
///
/// # Example
///
/// ```no_run
/// use dashflow::core::document_loaders::ZshLoader;
/// use dashflow::core::documents::DocumentLoader;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let loader = ZshLoader::new(".zshrc").with_separate_definitions(true);
/// let docs = loader.load().await?;
/// for doc in docs {
///     println!("Content: {}", doc.page_content);
/// }
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone)]

// ============================================================================
// Zsh Loader
// ============================================================================

pub struct ZshLoader {
    file_path: PathBuf,
    separate_definitions: bool,
}

impl ZshLoader {
    /// Creates a new Zsh loader for the given file path
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
impl DocumentLoader for ZshLoader {
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
                if !in_definition && (trimmed.is_empty() || trimmed.starts_with('#')) {
                    continue;
                }

                // Check for function definition start
                // Zsh supports: function name { }, name() { }, function name() { }
                if !in_definition {
                    if let Some(name) = Self::extract_function(trimmed) {
                        in_definition = true;
                        definition_name = name;
                        current_definition.push_str(line);
                        current_definition.push('\n');

                        // Count braces
                        brace_count =
                            line.matches('{').count() as i32 - line.matches('}').count() as i32;

                        // Check if single-line function (rare but possible)
                        if brace_count == 0 && line.contains('}') {
                            let doc = Document::new(current_definition.trim_end())
                                .with_metadata("source", self.file_path.display().to_string())
                                .with_metadata("format", "zsh")
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
                            .with_metadata("format", "zsh")
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
                    .with_metadata("format", "zsh")
                    .with_metadata("definition_index", documents.len())
                    .with_metadata("definition_name", definition_name);

                documents.push(doc);
            }

            Ok(documents)
        } else {
            // Load as single document
            let doc = Document::new(content)
                .with_metadata("source", self.file_path.display().to_string())
                .with_metadata("format", "zsh");

            Ok(vec![doc])
        }
    }
}

impl ZshLoader {
    /// Extract function name from Zsh function definitions
    fn extract_function(line: &str) -> Option<String> {
        let trimmed = line.trim();

        // Match: function name {, name() {, function name() {
        if let Some(after) = trimmed.strip_prefix("function ") {
            // function name { or function name() {
            if let Some(end) = after.find(|c: char| c == '(' || c == '{' || c.is_whitespace()) {
                let name = after[..end].trim();
                if !name.is_empty() {
                    return Some(format!("function {name}"));
                }
            }
        } else if trimmed.contains("()") {
            // name() {
            if let Some(pos) = trimmed.find("()") {
                let name = trimmed[..pos].trim();
                if !name.is_empty() && !name.contains(char::is_whitespace) {
                    return Some(format!("function {name}"));
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

    // ==================== PythonFileLoader Tests ====================

    #[tokio::test]
    async fn test_python_loader_basic() {
        let python_code = r#"
def hello():
    print("Hello, World!")

class MyClass:
    def __init__(self):
        pass
"#;

        let mut file = NamedTempFile::with_suffix(".py").unwrap();
        file.write_all(python_code.as_bytes()).unwrap();

        let loader = PythonFileLoader::new(file.path());
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1);
        assert!(docs[0].page_content.contains("def hello()"));
        assert_eq!(docs[0].metadata.get("format").unwrap(), "python");
    }

    #[tokio::test]
    async fn test_python_loader_separate_definitions() {
        let python_code = r#"
import os

def func_one():
    pass

def func_two():
    pass

class MyClass:
    pass
"#;

        let mut file = NamedTempFile::with_suffix(".py").unwrap();
        file.write_all(python_code.as_bytes()).unwrap();

        let loader = PythonFileLoader::new(file.path()).with_separate_definitions(true);
        let docs = loader.load().await.unwrap();

        // Should split into: imports, func_one, func_two, MyClass
        assert!(docs.len() >= 3);
        assert!(docs.iter().any(
            |d| d.metadata.get("definition_name").map(|v| v.as_str()) == Some(Some("func_one"))
        ));
        assert!(docs.iter().any(
            |d| d.metadata.get("definition_name").map(|v| v.as_str()) == Some(Some("func_two"))
        ));
    }

    #[tokio::test]
    async fn test_python_loader_extract_docstrings() {
        let python_code = r#"
"""Module docstring"""

def hello():
    """Function docstring"""
    pass
"#;

        let mut file = NamedTempFile::with_suffix(".py").unwrap();
        file.write_all(python_code.as_bytes()).unwrap();

        let loader = PythonFileLoader::new(file.path()).with_extract_docstrings(true);
        let docs = loader.load().await.unwrap();

        // Should have docstrings and code separated
        assert!(docs
            .iter()
            .any(|d| d.metadata.get("type").map(|v| v.as_str()) == Some(Some("docstring"))));
        assert!(docs
            .iter()
            .any(|d| d.metadata.get("type").map(|v| v.as_str()) == Some(Some("code"))));
    }

    // ==================== JavaScriptLoader Tests ====================

    #[tokio::test]
    async fn test_javascript_loader_basic() {
        let js_code = r#"
function hello() {
    console.log("Hello!");
}

const greet = (name) => {
    console.log("Hi " + name);
};
"#;

        let mut file = NamedTempFile::with_suffix(".js").unwrap();
        file.write_all(js_code.as_bytes()).unwrap();

        let loader = JavaScriptLoader::new(file.path());
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1);
        assert!(docs[0].page_content.contains("function hello()"));
        assert_eq!(docs[0].metadata.get("format").unwrap(), "javascript");
    }

    #[tokio::test]
    async fn test_javascript_loader_separate_functions() {
        let js_code = r#"
function funcOne() {
    return 1;
}

function funcTwo() {
    return 2;
}
"#;

        let mut file = NamedTempFile::with_suffix(".js").unwrap();
        file.write_all(js_code.as_bytes()).unwrap();

        let loader = JavaScriptLoader::new(file.path()).with_separate_functions(true);
        let docs = loader.load().await.unwrap();

        // Should have separate documents for each function
        assert!(docs.len() >= 2);
        assert!(docs
            .iter()
            .any(|d| d.metadata.get("function_name").map(|v| v.as_str()) == Some(Some("funcOne"))));
        assert!(docs
            .iter()
            .any(|d| d.metadata.get("function_name").map(|v| v.as_str()) == Some(Some("funcTwo"))));
    }

    // ==================== TypeScriptLoader Tests ====================

    #[tokio::test]
    async fn test_typescript_loader_basic() {
        let ts_code = r#"
interface User {
    name: string;
    age: number;
}

function greet(user: User): string {
    return "Hello, " + user.name;
}
"#;

        let mut file = NamedTempFile::with_suffix(".ts").unwrap();
        file.write_all(ts_code.as_bytes()).unwrap();

        let loader = TypeScriptLoader::new(file.path());
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1);
        assert!(docs[0].page_content.contains("interface User"));
        assert_eq!(docs[0].metadata.get("format").unwrap(), "typescript");
    }

    #[tokio::test]
    async fn test_typescript_loader_separate_definitions() {
        let ts_code = r#"
interface IUser {
    name: string;
}

class UserService {
    private users: IUser[] = [];
}

function getUsers() {
    return [];
}
"#;

        let mut file = NamedTempFile::with_suffix(".ts").unwrap();
        file.write_all(ts_code.as_bytes()).unwrap();

        let loader = TypeScriptLoader::new(file.path()).with_separate_definitions(true);
        let docs = loader.load().await.unwrap();

        assert!(docs.len() >= 2);
    }

    // ==================== BashScriptLoader Tests ====================

    #[tokio::test]
    async fn test_bash_loader_basic() {
        let bash_code = r#"#!/bin/bash

echo "Hello World"

function greet() {
    echo "Hi $1"
}

greet "User"
"#;

        let mut file = NamedTempFile::with_suffix(".sh").unwrap();
        file.write_all(bash_code.as_bytes()).unwrap();

        let loader = BashScriptLoader::new(file.path());
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1);
        assert!(docs[0].page_content.contains("#!/bin/bash"));
        assert_eq!(docs[0].metadata.get("format").unwrap(), "bash");
    }

    #[tokio::test]
    async fn test_bash_loader_separate_functions() {
        let bash_code = r#"#!/bin/bash

function func_one() {
    echo "one"
}

func_two() {
    echo "two"
}
"#;

        let mut file = NamedTempFile::with_suffix(".sh").unwrap();
        file.write_all(bash_code.as_bytes()).unwrap();

        let loader = BashScriptLoader::new(file.path()).with_separate_functions(true);
        let docs = loader.load().await.unwrap();

        // Should have separate documents for functions
        assert!(docs.len() >= 2);
    }

    // ==================== PowerShellLoader Tests ====================

    #[tokio::test]
    async fn test_powershell_loader_basic() {
        let ps_code = r#"
function Get-Greeting {
    param (
        [string]$Name
    )
    return "Hello, $Name"
}

Get-Greeting -Name "World"
"#;

        let mut file = NamedTempFile::with_suffix(".ps1").unwrap();
        file.write_all(ps_code.as_bytes()).unwrap();

        let loader = PowerShellLoader::new(file.path());
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1);
        assert!(docs[0].page_content.contains("function Get-Greeting"));
        assert_eq!(docs[0].metadata.get("format").unwrap(), "powershell");
    }

    // ==================== FishLoader Tests ====================

    #[tokio::test]
    async fn test_fish_loader_basic() {
        let fish_code = r#"
function greet
    echo "Hello $argv"
end

greet World
"#;

        let mut file = NamedTempFile::with_suffix(".fish").unwrap();
        file.write_all(fish_code.as_bytes()).unwrap();

        let loader = FishLoader::new(file.path());
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1);
        assert!(docs[0].page_content.contains("function greet"));
        assert_eq!(docs[0].metadata.get("format").unwrap(), "fish");
    }

    // ==================== ZshLoader Tests ====================

    #[tokio::test]
    async fn test_zsh_loader_basic() {
        let zsh_code = r#"#!/bin/zsh

function greet() {
    echo "Hello $1"
}

greet "World"
"#;

        let mut file = NamedTempFile::with_suffix(".zsh").unwrap();
        file.write_all(zsh_code.as_bytes()).unwrap();

        let loader = ZshLoader::new(file.path());
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1);
        assert!(docs[0].page_content.contains("#!/bin/zsh"));
        assert_eq!(docs[0].metadata.get("format").unwrap(), "zsh");
    }

    #[tokio::test]
    async fn test_zsh_extract_function() {
        // Test the extract_function helper
        assert_eq!(
            ZshLoader::extract_function("function hello {"),
            Some("function hello".to_string())
        );
        assert_eq!(
            ZshLoader::extract_function("function hello() {"),
            Some("function hello".to_string())
        );
        assert_eq!(
            ZshLoader::extract_function("hello() {"),
            Some("function hello".to_string())
        );
        assert_eq!(ZshLoader::extract_function("echo hello"), None);
    }

    // ==================== Builder Pattern Tests ====================

    #[tokio::test]
    async fn test_loader_builder_patterns() {
        // Test that builder methods work correctly
        let python = PythonFileLoader::new("test.py")
            .with_extract_docstrings(true)
            .with_separate_definitions(true);
        assert!(python.extract_docstrings);
        assert!(python.separate_definitions);

        let js = JavaScriptLoader::new("test.js").with_separate_functions(true);
        assert!(js.separate_functions);

        let ts = TypeScriptLoader::new("test.ts").with_separate_definitions(true);
        assert!(ts.separate_definitions);

        let bash = BashScriptLoader::new("test.sh").with_separate_functions(true);
        assert!(bash.separate_functions);
    }

    // ==================== Empty File Tests ====================

    #[tokio::test]
    async fn test_python_loader_empty_file() {
        let mut file = NamedTempFile::with_suffix(".py").unwrap();
        file.write_all(b"").unwrap();

        let loader = PythonFileLoader::new(file.path());
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1);
        assert!(docs[0].page_content.is_empty());
    }

    #[tokio::test]
    async fn test_javascript_loader_empty_file() {
        let mut file = NamedTempFile::with_suffix(".js").unwrap();
        file.write_all(b"").unwrap();

        let loader = JavaScriptLoader::new(file.path());
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1);
        assert!(docs[0].page_content.is_empty());
    }

    // ==================== Metadata Tests ====================

    #[tokio::test]
    async fn test_python_loader_metadata() {
        let python_code = "print(\"hello\")";
        let mut file = NamedTempFile::with_suffix(".py").unwrap();
        file.write_all(python_code.as_bytes()).unwrap();

        let loader = PythonFileLoader::new(file.path());
        let docs = loader.load().await.unwrap();

        assert!(docs[0].metadata.contains_key("source"));
        assert!(docs[0].metadata.contains_key("format"));
    }
}
