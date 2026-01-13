// Allow clippy warnings for shell loaders
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::clone_on_ref_ptr)]
#![allow(clippy::needless_pass_by_value, clippy::redundant_clone)]

//! Shell language document loaders.
//!
//! This module provides loaders for shell scripting languages:
//! - Tcsh (TENEX C Shell)
//! - Csh (C Shell)
//! - Ksh (Korn Shell)
//! - Tcl (Tool Command Language)
//! - Awk
//! - Sed

#![allow(clippy::empty_line_after_doc_comments)]
#![allow(unused_imports)]

use async_trait::async_trait;
use std::path::{Path, PathBuf};

use crate::core::documents::{Blob, Document, DocumentLoader};
use crate::core::error::Result;

/// `TcshLoader` loads TENEX C Shell (tcsh) scripts and separates them by alias and label definitions.
///
/// Tcsh is an enhanced version of the C shell (csh) with added features like command-line editing,
/// programmable word completion, and spelling correction. It was created by Ken Greer in the early 1980s.
///
/// Supports extensions: .tcsh
///
/// When `separate_definitions` is true, splits document by alias definitions and goto labels.
/// Tcsh syntax: `alias name 'commands'` and `label:` for goto targets
///
/// Example:
/// ```no_run
/// use dashflow::core::document_loaders::TcshLoader;
/// use dashflow::core::documents::DocumentLoader;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let loader = TcshLoader::new("script.tcsh").with_separate_definitions(true);
/// let docs = loader.load().await?;
/// println!("Loaded {} definitions", docs.len());
/// # Ok(())
/// # }
/// ```
pub struct TcshLoader {
    file_path: PathBuf,
    separate_definitions: bool,
}

impl TcshLoader {
    /// Creates a new TENEX C Shell script loader.
    ///
    /// # Arguments
    ///
    /// * `file_path` - Path to the `.tcsh` file to load
    pub fn new<P: Into<PathBuf>>(file_path: P) -> Self {
        Self {
            file_path: file_path.into(),
            separate_definitions: false,
        }
    }

    /// When enabled, creates separate documents for each alias/function definition.
    #[must_use]
    pub fn with_separate_definitions(mut self, separate: bool) -> Self {
        self.separate_definitions = separate;
        self
    }
}

#[async_trait]
impl DocumentLoader for TcshLoader {
    async fn load(&self) -> Result<Vec<Document>> {
        let blob = Blob::from_path(&self.file_path);
        let content = blob.as_string()?;

        if self.separate_definitions {
            let mut documents = Vec::new();
            let lines: Vec<&str> = content.lines().collect();
            let mut current_definition = String::new();
            let mut definition_name = String::new();

            for line in lines {
                let trimmed = line.trim();

                // Check for alias definitions: alias name 'commands'
                if trimmed.starts_with("alias ") {
                    // Save previous definition
                    if !current_definition.is_empty() {
                        let doc = Document::new(current_definition.trim_end())
                            .with_metadata("source", self.file_path.display().to_string())
                            .with_metadata("format", "tcsh")
                            .with_metadata("definition_index", documents.len())
                            .with_metadata("definition_name", definition_name.clone());
                        documents.push(doc);
                        current_definition.clear();
                    }

                    // Extract alias name (word after "alias ")
                    if let Some(rest) = trimmed.strip_prefix("alias ") {
                        if let Some(space_pos) = rest.find(char::is_whitespace) {
                            definition_name = rest[..space_pos].to_string();
                        } else {
                            definition_name = rest.to_string();
                        }
                    }
                    current_definition.push_str(line);
                    current_definition.push('\n');
                    continue;
                }

                // Check for function labels (function_name:)
                if !trimmed.starts_with('#') && trimmed.ends_with(':') && !trimmed.contains(' ') {
                    // Save previous definition
                    if !current_definition.is_empty() {
                        let doc = Document::new(current_definition.trim_end())
                            .with_metadata("source", self.file_path.display().to_string())
                            .with_metadata("format", "tcsh")
                            .with_metadata("definition_index", documents.len())
                            .with_metadata("definition_name", definition_name.clone());
                        documents.push(doc);
                        current_definition.clear();
                    }

                    // Start new function definition
                    definition_name = trimmed.trim_end_matches(':').to_string();
                    current_definition.push_str(line);
                    current_definition.push('\n');
                    continue;
                }

                current_definition.push_str(line);
                current_definition.push('\n');
            }

            // Save last definition if exists
            if !current_definition.is_empty() {
                let doc = Document::new(current_definition.trim_end())
                    .with_metadata("source", self.file_path.display().to_string())
                    .with_metadata("format", "tcsh")
                    .with_metadata("definition_index", documents.len())
                    .with_metadata("definition_name", definition_name);
                documents.push(doc);
            }

            Ok(documents)
        } else {
            // Load entire file as single document
            Ok(vec![Document::new(&content)
                .with_metadata("source", self.file_path.display().to_string())
                .with_metadata("format", "tcsh")])
        }
    }
}

/// `CshLoader` loads C Shell scripts and separates them by alias and label definitions.
///
/// C Shell (csh) is a Unix shell created by Bill Joy while at UC Berkeley in the late 1970s.
/// It introduced C-like syntax to Unix shells and influenced many later shells.
///
/// Supports extensions: .csh
///
/// When `separate_definitions` is true, splits document by alias definitions and goto labels.
/// Csh syntax: `alias name 'commands'` and `label:` for goto targets
///
/// Example:
/// ```no_run
/// use dashflow::core::document_loaders::CshLoader;
/// use dashflow::core::documents::DocumentLoader;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let loader = CshLoader::new("script.csh").with_separate_definitions(true);
/// let docs = loader.load().await?;
/// println!("Loaded {} definitions", docs.len());
/// # Ok(())
/// # }
/// ```

// ============================================================================
// Csh Loader
// ============================================================================

/// Loader for C Shell (csh) script files.
pub struct CshLoader {
    file_path: PathBuf,
    separate_definitions: bool,
}

impl CshLoader {
    /// Creates a new C Shell script loader.
    ///
    /// # Arguments
    ///
    /// * `file_path` - Path to the `.csh` file to load
    pub fn new<P: Into<PathBuf>>(file_path: P) -> Self {
        Self {
            file_path: file_path.into(),
            separate_definitions: false,
        }
    }

    /// When enabled, creates separate documents for each alias/function definition.
    #[must_use]
    pub fn with_separate_definitions(mut self, separate: bool) -> Self {
        self.separate_definitions = separate;
        self
    }
}

#[async_trait]
impl DocumentLoader for CshLoader {
    async fn load(&self) -> Result<Vec<Document>> {
        let blob = Blob::from_path(&self.file_path);
        let content = blob.as_string()?;

        if self.separate_definitions {
            let mut documents = Vec::new();
            let lines: Vec<&str> = content.lines().collect();
            let mut current_definition = String::new();
            let mut definition_name = String::new();

            for line in lines {
                let trimmed = line.trim();

                // Check for alias definitions: alias name 'commands'
                if trimmed.starts_with("alias ") {
                    // Save previous definition
                    if !current_definition.is_empty() {
                        let doc = Document::new(current_definition.trim_end())
                            .with_metadata("source", self.file_path.display().to_string())
                            .with_metadata("format", "csh")
                            .with_metadata("definition_index", documents.len())
                            .with_metadata("definition_name", definition_name.clone());
                        documents.push(doc);
                        current_definition.clear();
                    }

                    // Extract alias name (word after "alias ")
                    if let Some(rest) = trimmed.strip_prefix("alias ") {
                        if let Some(space_pos) = rest.find(char::is_whitespace) {
                            definition_name = rest[..space_pos].to_string();
                        } else {
                            definition_name = rest.to_string();
                        }
                    }
                    current_definition.push_str(line);
                    current_definition.push('\n');
                    continue;
                }

                // Check for labels (label_name:) used for goto
                if !trimmed.starts_with('#') && trimmed.ends_with(':') && !trimmed.contains(' ') {
                    // Save previous definition
                    if !current_definition.is_empty() {
                        let doc = Document::new(current_definition.trim_end())
                            .with_metadata("source", self.file_path.display().to_string())
                            .with_metadata("format", "csh")
                            .with_metadata("definition_index", documents.len())
                            .with_metadata("definition_name", definition_name.clone());
                        documents.push(doc);
                        current_definition.clear();
                    }

                    // Start new label block
                    definition_name = trimmed.trim_end_matches(':').to_string();
                    current_definition.push_str(line);
                    current_definition.push('\n');
                    continue;
                }

                current_definition.push_str(line);
                current_definition.push('\n');
            }

            // Save last definition if exists
            if !current_definition.is_empty() {
                let doc = Document::new(current_definition.trim_end())
                    .with_metadata("source", self.file_path.display().to_string())
                    .with_metadata("format", "csh")
                    .with_metadata("definition_index", documents.len())
                    .with_metadata("definition_name", definition_name);
                documents.push(doc);
            }

            Ok(documents)
        } else {
            // Load entire file as single document
            Ok(vec![Document::new(&content)
                .with_metadata("source", self.file_path.display().to_string())
                .with_metadata("format", "csh")])
        }
    }
}

/// `KshLoader` loads Korn Shell scripts and separates them by function definitions.
///
/// Korn Shell (ksh) was developed by David Korn at Bell Labs in the early 1980s.
/// It combines features from the Bourne shell and C shell, and was one of the first
/// shells to add command-line editing and scripting improvements.
///
/// Supports extensions: .ksh, .sh (when ksh is the interpreter)
///
/// When `separate_definitions` is true, splits document by function definitions.
/// Ksh syntax: `function name { ... }` or `name() { ... }`
///
/// Example:
/// ```no_run
/// use dashflow::core::document_loaders::KshLoader;
/// use dashflow::core::documents::DocumentLoader;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let loader = KshLoader::new("script.ksh").with_separate_definitions(true);
/// let docs = loader.load().await?;
/// println!("Loaded {} function definitions", docs.len());
/// # Ok(())
/// # }
/// ```

// ============================================================================
// Ksh Loader
// ============================================================================

/// Loader for Korn Shell (ksh) script files.
pub struct KshLoader {
    file_path: PathBuf,
    separate_definitions: bool,
}

impl KshLoader {
    /// Creates a new Korn Shell script loader.
    ///
    /// # Arguments
    ///
    /// * `file_path` - Path to the `.ksh` file to load
    pub fn new<P: Into<PathBuf>>(file_path: P) -> Self {
        Self {
            file_path: file_path.into(),
            separate_definitions: false,
        }
    }

    /// When enabled, creates separate documents for each function definition.
    #[must_use]
    pub fn with_separate_definitions(mut self, separate: bool) -> Self {
        self.separate_definitions = separate;
        self
    }
}

#[async_trait]
impl DocumentLoader for KshLoader {
    async fn load(&self) -> Result<Vec<Document>> {
        let blob = Blob::from_path(&self.file_path);
        let content = blob.as_string()?;

        if self.separate_definitions {
            let mut documents = Vec::new();
            let lines: Vec<&str> = content.lines().collect();
            let mut current_definition = String::new();
            let mut definition_name = String::new();
            let mut brace_count = 0;
            let mut in_definition = false;

            for line in lines {
                let trimmed = line.trim();

                // Check for function keyword: function name { ... }
                if trimmed.starts_with("function ") {
                    // Save previous definition
                    if !current_definition.is_empty() {
                        let doc = Document::new(current_definition.trim_end())
                            .with_metadata("source", self.file_path.display().to_string())
                            .with_metadata("format", "ksh")
                            .with_metadata("definition_index", documents.len())
                            .with_metadata("definition_name", definition_name.clone());
                        documents.push(doc);
                        current_definition.clear();
                    }

                    // Extract function name
                    let rest = trimmed
                        .strip_prefix("function ")
                        .expect("checked starts_with above");
                    if let Some(space_or_brace) = rest.find(|c: char| c.is_whitespace() || c == '{')
                    {
                        definition_name = rest[..space_or_brace].trim().to_string();
                    } else {
                        definition_name = rest.trim().to_string();
                    }

                    current_definition.push_str(line);
                    current_definition.push('\n');
                    brace_count =
                        line.matches('{').count() as i32 - line.matches('}').count() as i32;
                    in_definition = true;
                    continue;
                }

                // Check for POSIX function syntax: name() { ... }
                if let Some(paren_pos) = trimmed.find("()") {
                    let func_name = trimmed[..paren_pos].trim();
                    if !func_name.is_empty()
                        && !func_name.contains(char::is_whitespace)
                        && !func_name.starts_with('#')
                    {
                        // Save previous definition
                        if !current_definition.is_empty() {
                            let doc = Document::new(current_definition.trim_end())
                                .with_metadata("source", self.file_path.display().to_string())
                                .with_metadata("format", "ksh")
                                .with_metadata("definition_index", documents.len())
                                .with_metadata("definition_name", definition_name.clone());
                            documents.push(doc);
                            current_definition.clear();
                        }

                        definition_name = func_name.to_string();
                        current_definition.push_str(line);
                        current_definition.push('\n');
                        brace_count =
                            line.matches('{').count() as i32 - line.matches('}').count() as i32;
                        in_definition = true;
                        continue;
                    }
                }

                // Track braces in function body
                if in_definition {
                    brace_count += line.matches('{').count() as i32;
                    brace_count -= line.matches('}').count() as i32;

                    current_definition.push_str(line);
                    current_definition.push('\n');

                    // Function definition complete when braces balance
                    if brace_count == 0 {
                        let doc = Document::new(current_definition.trim_end())
                            .with_metadata("source", self.file_path.display().to_string())
                            .with_metadata("format", "ksh")
                            .with_metadata("definition_index", documents.len())
                            .with_metadata("definition_name", definition_name.clone());
                        documents.push(doc);
                        current_definition.clear();
                        definition_name.clear();
                        in_definition = false;
                    }
                }
                // else: Skip lines outside of function definitions
            }

            // Save last definition if incomplete
            if !current_definition.is_empty() {
                let doc = Document::new(current_definition.trim_end())
                    .with_metadata("source", self.file_path.display().to_string())
                    .with_metadata("format", "ksh")
                    .with_metadata("definition_index", documents.len())
                    .with_metadata("definition_name", definition_name);

                documents.push(doc);
            }

            Ok(documents)
        } else {
            // Load entire file as single document
            Ok(vec![Document::new(&content)
                .with_metadata("source", self.file_path.display().to_string())
                .with_metadata("format", "ksh")])
        }
    }
}

/// `TclLoader` loads TCL (Tool Command Language) scripts and separates them by procedure definitions.
///
/// TCL (Tool Command Language) was created by John Ousterhout in 1988 while at UC Berkeley.
/// It's a powerful scripting language with simple syntax, often used for test automation,
/// GUI development (Tk), and rapid prototyping.
///
/// Supports extensions: .tcl
///
/// When `separate_definitions` is true, splits document by procedure definitions.
/// TCL syntax: `proc name {args} { body }`
///
/// Example:
/// ```no_run
/// use dashflow::core::document_loaders::TclLoader;
/// use dashflow::core::documents::DocumentLoader;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let loader = TclLoader::new("script.tcl").with_separate_definitions(true);
/// let docs = loader.load().await?;
/// println!("Loaded {} procedure definitions", docs.len());
/// # Ok(())
/// # }
/// ```

// ============================================================================
// Tcl Loader
// ============================================================================

/// Loader for TCL (Tool Command Language) script files.
pub struct TclLoader {
    file_path: PathBuf,
    separate_definitions: bool,
}

impl TclLoader {
    /// Creates a new TCL script loader.
    ///
    /// # Arguments
    ///
    /// * `file_path` - Path to the `.tcl` file to load
    pub fn new<P: Into<PathBuf>>(file_path: P) -> Self {
        Self {
            file_path: file_path.into(),
            separate_definitions: false,
        }
    }

    /// When enabled, creates separate documents for each procedure definition.
    #[must_use]
    pub fn with_separate_definitions(mut self, separate: bool) -> Self {
        self.separate_definitions = separate;
        self
    }
}

#[async_trait]
impl DocumentLoader for TclLoader {
    async fn load(&self) -> Result<Vec<Document>> {
        let blob = Blob::from_path(&self.file_path);
        let content = blob.as_string()?;

        if self.separate_definitions {
            let mut documents = Vec::new();
            let mut current_definition = String::new();
            let mut definition_name = String::new();
            let mut in_definition = false;
            let mut brace_count = 0i32;

            for line in content.lines() {
                let trimmed = line.trim();

                // Check for proc definition: proc name {args} { body }
                if !in_definition && trimmed.starts_with("proc ") {
                    // Save previous definition
                    if !current_definition.is_empty() {
                        let doc = Document::new(current_definition.trim_end())
                            .with_metadata("source", self.file_path.display().to_string())
                            .with_metadata("format", "tcl")
                            .with_metadata("definition_index", documents.len())
                            .with_metadata("definition_name", definition_name.clone());
                        documents.push(doc);
                        current_definition.clear();
                    }

                    // Extract procedure name
                    let rest = trimmed
                        .strip_prefix("proc ")
                        .expect("checked starts_with above");
                    if let Some(space_or_brace) = rest.find(|c: char| c.is_whitespace() || c == '{')
                    {
                        definition_name = rest[..space_or_brace].trim().to_string();
                    } else {
                        definition_name = rest.trim().to_string();
                    }

                    current_definition.push_str(line);
                    current_definition.push('\n');
                    brace_count =
                        line.matches('{').count() as i32 - line.matches('}').count() as i32;
                    in_definition = true;
                    continue;
                }

                // Track braces in procedure body
                if in_definition {
                    brace_count += line.matches('{').count() as i32;
                    brace_count -= line.matches('}').count() as i32;

                    current_definition.push_str(line);
                    current_definition.push('\n');

                    // Procedure definition complete when braces balance
                    if brace_count == 0 {
                        let doc = Document::new(current_definition.trim_end())
                            .with_metadata("source", self.file_path.display().to_string())
                            .with_metadata("format", "tcl")
                            .with_metadata("definition_index", documents.len())
                            .with_metadata("definition_name", definition_name.clone());
                        documents.push(doc);
                        current_definition.clear();
                        definition_name.clear();
                        in_definition = false;
                    }
                }
                // else: Skip lines outside of procedure definitions
            }

            // Save last definition if incomplete
            if !current_definition.is_empty() {
                let doc = Document::new(current_definition.trim_end())
                    .with_metadata("source", self.file_path.display().to_string())
                    .with_metadata("format", "tcl")
                    .with_metadata("definition_index", documents.len())
                    .with_metadata("definition_name", definition_name);
                documents.push(doc);
            }

            Ok(documents)
        } else {
            // Load entire file as single document
            Ok(vec![Document::new(&content)
                .with_metadata("source", self.file_path.display().to_string())
                .with_metadata("format", "tcl")])
        }
    }
}

/// `AwkLoader` loads AWK scripts and separates them by function definitions.
///
/// AWK is a domain-specific language designed for text processing and data extraction.
/// Created by Alfred Aho, Peter Weinberger, and Brian Kernighan in 1977.
/// Named after their initials, AWK is a standard Unix tool for pattern scanning and processing.
///
/// Supports extensions: .awk
///
/// When `separate_definitions` is true, splits document by function definitions and BEGIN/END blocks.
/// AWK syntax: `function name(args) { body }`, `BEGIN { ... }`, `END { ... }`
///
/// Example:
/// ```no_run
/// use dashflow::core::document_loaders::AwkLoader;
/// use dashflow::core::documents::DocumentLoader;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let loader = AwkLoader::new("script.awk").with_separate_definitions(true);
/// let docs = loader.load().await?;
/// println!("Loaded {} AWK definitions", docs.len());
/// # Ok(())
/// # }
/// ```

// ============================================================================
// Awk Loader
// ============================================================================

/// Document loader for AWK scripts.
///
/// Loads AWK files and can optionally split them into separate documents
/// per function/block definition.
pub struct AwkLoader {
    file_path: PathBuf,
    separate_definitions: bool,
}

impl AwkLoader {
    /// Creates a new AWK loader for the given file path.
    pub fn new<P: Into<PathBuf>>(file_path: P) -> Self {
        Self {
            file_path: file_path.into(),
            separate_definitions: false,
        }
    }

    /// Splits AWK functions and blocks into separate documents when enabled.
    #[must_use]
    pub fn with_separate_definitions(mut self, separate: bool) -> Self {
        self.separate_definitions = separate;
        self
    }
}

#[async_trait]
impl DocumentLoader for AwkLoader {
    async fn load(&self) -> Result<Vec<Document>> {
        let blob = Blob::from_path(&self.file_path);
        let content = blob.as_string()?;

        if self.separate_definitions {
            let mut documents = Vec::new();
            let mut current_definition = String::new();
            let mut definition_name = String::new();
            let mut in_definition = false;
            let mut brace_count = 0i32;

            for line in content.lines() {
                let trimmed = line.trim();

                // Check for function definition: function name(args) { body }
                if !in_definition && trimmed.starts_with("function ") {
                    // Save previous definition (only if non-empty)
                    if !current_definition.trim().is_empty() {
                        let doc = Document::new(current_definition.trim_end())
                            .with_metadata("source", self.file_path.display().to_string())
                            .with_metadata("format", "awk")
                            .with_metadata("definition_index", documents.len())
                            .with_metadata("definition_name", definition_name.clone());
                        documents.push(doc);
                        current_definition.clear();
                    }

                    // Extract function name
                    let rest = trimmed
                        .strip_prefix("function ")
                        .expect("checked starts_with above");
                    if let Some(paren_pos) = rest.find('(') {
                        definition_name = rest[..paren_pos].trim().to_string();
                    } else {
                        definition_name = rest.trim().to_string();
                    }

                    current_definition.push_str(line);
                    current_definition.push('\n');
                    brace_count =
                        line.matches('{').count() as i32 - line.matches('}').count() as i32;
                    in_definition = true;
                    continue;
                }

                // Check for BEGIN block
                if !in_definition && (trimmed.starts_with("BEGIN") && trimmed.contains('{')) {
                    // Save previous definition (only if non-empty)
                    if !current_definition.trim().is_empty() {
                        let doc = Document::new(current_definition.trim_end())
                            .with_metadata("source", self.file_path.display().to_string())
                            .with_metadata("format", "awk")
                            .with_metadata("definition_index", documents.len())
                            .with_metadata("definition_name", definition_name.clone());
                        documents.push(doc);
                        current_definition.clear();
                    }

                    definition_name = "BEGIN".to_string();
                    current_definition.push_str(line);
                    current_definition.push('\n');
                    brace_count =
                        line.matches('{').count() as i32 - line.matches('}').count() as i32;
                    in_definition = true;
                    continue;
                }

                // Check for END block
                if !in_definition && (trimmed.starts_with("END") && trimmed.contains('{')) {
                    // Save previous definition (only if non-empty)
                    if !current_definition.trim().is_empty() {
                        let doc = Document::new(current_definition.trim_end())
                            .with_metadata("source", self.file_path.display().to_string())
                            .with_metadata("format", "awk")
                            .with_metadata("definition_index", documents.len())
                            .with_metadata("definition_name", definition_name.clone());
                        documents.push(doc);
                        current_definition.clear();
                    }

                    definition_name = "END".to_string();
                    current_definition.push_str(line);
                    current_definition.push('\n');
                    brace_count =
                        line.matches('{').count() as i32 - line.matches('}').count() as i32;
                    in_definition = true;
                    continue;
                }

                // Track braces in definition body
                if in_definition {
                    brace_count += line.matches('{').count() as i32;
                    brace_count -= line.matches('}').count() as i32;

                    current_definition.push_str(line);
                    current_definition.push('\n');

                    // Definition complete when braces balance
                    if brace_count == 0 {
                        let doc = Document::new(current_definition.trim_end())
                            .with_metadata("source", self.file_path.display().to_string())
                            .with_metadata("format", "awk")
                            .with_metadata("definition_index", documents.len())
                            .with_metadata("definition_name", definition_name.clone());
                        documents.push(doc);
                        current_definition.clear();
                        definition_name.clear();
                        in_definition = false;
                    }
                } else {
                    // Accumulate content (pattern-action rules)
                    current_definition.push_str(line);
                    current_definition.push('\n');
                }
            }

            // Save last content (only if non-empty)
            if !current_definition.trim().is_empty() {
                let doc = Document::new(current_definition.trim_end())
                    .with_metadata("source", self.file_path.display().to_string())
                    .with_metadata("format", "awk")
                    .with_metadata("definition_index", documents.len())
                    .with_metadata("definition_name", definition_name);
                documents.push(doc);
            }

            Ok(documents)
        } else {
            // Load entire file as single document
            Ok(vec![Document::new(&content)
                .with_metadata("source", self.file_path.display().to_string())
                .with_metadata("format", "awk")])
        }
    }
}

/// `SedLoader` loads Sed (Stream Editor) scripts as documents.
///
/// Sed is a stream editor for filtering and transforming text.
/// Created by Lee E. `McMahon` at Bell Labs in 1973-74 for Unix.
/// Sed processes text line-by-line using editing commands.
///
/// Supports extensions: .sed
///
/// Sed scripts are typically small command sequences, so `separate_definitions` is not supported.
/// All sed commands are loaded as a single document.
///
/// Example:
/// ```no_run
/// use dashflow::core::document_loaders::SedLoader;
/// use dashflow::core::documents::DocumentLoader;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let loader = SedLoader::new("script.sed");
/// let docs = loader.load().await?;
/// println!("Loaded sed script");
/// # Ok(())
/// # }
/// ```

// ============================================================================
// Sed Loader
// ============================================================================

/// Document loader for sed (stream editor) scripts.
///
/// Loads sed script files as single documents since sed scripts
/// are typically command sequences without function definitions.
pub struct SedLoader {
    file_path: PathBuf,
}

impl SedLoader {
    /// Creates a new sed loader for the given file path.
    pub fn new<P: Into<PathBuf>>(file_path: P) -> Self {
        Self {
            file_path: file_path.into(),
        }
    }
}

#[async_trait]
impl DocumentLoader for SedLoader {
    async fn load(&self) -> Result<Vec<Document>> {
        let blob = Blob::from_path(&self.file_path);
        let content = blob.as_string()?;

        // Sed scripts are typically command sequences without function definitions
        // Load entire file as single document
        Ok(vec![Document::new(&content)
            .with_metadata("source", self.file_path.display().to_string())
            .with_metadata("format", "sed")])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    // ============================================================================
    // TcshLoader Tests
    // ============================================================================

    #[tokio::test]
    async fn test_tcsh_loader_basic() {
        let mut file = NamedTempFile::new().unwrap();
        write!(file, "#!/bin/tcsh\necho hello\nset foo = bar").unwrap();

        let loader = TcshLoader::new(file.path());
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1);
        assert!(docs[0].page_content.contains("echo hello"));
        assert_eq!(docs[0].metadata.get("format").unwrap(), "tcsh");
    }

    #[tokio::test]
    async fn test_tcsh_loader_separate_definitions_alias() {
        let mut file = NamedTempFile::new().unwrap();
        write!(file, "alias ll 'ls -la'\nalias la 'ls -A'\necho done").unwrap();

        let loader = TcshLoader::new(file.path()).with_separate_definitions(true);
        let docs = loader.load().await.unwrap();

        // Content after last alias is merged with last alias document (2 docs total)
        assert_eq!(docs.len(), 2);
        assert!(docs[0].page_content.contains("alias ll"));
        assert_eq!(docs[0].metadata.get("definition_name").unwrap(), "ll");
        assert!(docs[1].page_content.contains("alias la"));
        assert_eq!(docs[1].metadata.get("definition_name").unwrap(), "la");
    }

    #[tokio::test]
    async fn test_tcsh_loader_separate_definitions_labels() {
        let mut file = NamedTempFile::new().unwrap();
        write!(file, "start:\necho starting\ngoto end\nend:\necho ending").unwrap();

        let loader = TcshLoader::new(file.path()).with_separate_definitions(true);
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 2);
        assert_eq!(docs[0].metadata.get("definition_name").unwrap(), "start");
        assert_eq!(docs[1].metadata.get("definition_name").unwrap(), "end");
    }

    #[tokio::test]
    async fn test_tcsh_loader_metadata() {
        let mut file = NamedTempFile::new().unwrap();
        write!(file, "echo test").unwrap();

        let loader = TcshLoader::new(file.path());
        let docs = loader.load().await.unwrap();

        assert!(docs[0].metadata.contains_key("source"));
        assert_eq!(docs[0].metadata.get("format").unwrap(), "tcsh");
    }

    #[tokio::test]
    async fn test_tcsh_loader_empty_file() {
        let file = NamedTempFile::new().unwrap();

        let loader = TcshLoader::new(file.path());
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1);
        assert!(docs[0].page_content.is_empty());
    }

    #[tokio::test]
    async fn test_tcsh_loader_builder_chain() {
        let file = NamedTempFile::new().unwrap();

        let loader = TcshLoader::new(file.path())
            .with_separate_definitions(true)
            .with_separate_definitions(false);

        // Just verify the builder chain compiles and works
        let docs = loader.load().await.unwrap();
        assert_eq!(docs.len(), 1);
    }

    // ============================================================================
    // CshLoader Tests
    // ============================================================================

    #[tokio::test]
    async fn test_csh_loader_basic() {
        let mut file = NamedTempFile::new().unwrap();
        write!(
            file,
            "#!/bin/csh\nset path = ($path /usr/local/bin)\necho $path"
        )
        .unwrap();

        let loader = CshLoader::new(file.path());
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1);
        assert!(docs[0].page_content.contains("set path"));
        assert_eq!(docs[0].metadata.get("format").unwrap(), "csh");
    }

    #[tokio::test]
    async fn test_csh_loader_separate_definitions_alias() {
        let mut file = NamedTempFile::new().unwrap();
        write!(file, "alias rm 'rm -i'\nalias cp 'cp -i'\necho aliases set").unwrap();

        let loader = CshLoader::new(file.path()).with_separate_definitions(true);
        let docs = loader.load().await.unwrap();

        // Content after last alias is merged with last alias document (2 docs total)
        assert_eq!(docs.len(), 2);
        assert_eq!(docs[0].metadata.get("definition_name").unwrap(), "rm");
        assert_eq!(docs[1].metadata.get("definition_name").unwrap(), "cp");
    }

    #[tokio::test]
    async fn test_csh_loader_separate_definitions_labels() {
        let mut file = NamedTempFile::new().unwrap();
        write!(
            file,
            "loop:\necho iteration\nif ($count < 5) goto loop\ndone:\necho finished"
        )
        .unwrap();

        let loader = CshLoader::new(file.path()).with_separate_definitions(true);
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 2);
        assert_eq!(docs[0].metadata.get("definition_name").unwrap(), "loop");
        assert_eq!(docs[1].metadata.get("definition_name").unwrap(), "done");
    }

    #[tokio::test]
    async fn test_csh_loader_metadata() {
        let mut file = NamedTempFile::new().unwrap();
        write!(file, "echo test").unwrap();

        let loader = CshLoader::new(file.path());
        let docs = loader.load().await.unwrap();

        assert!(docs[0].metadata.contains_key("source"));
        assert_eq!(docs[0].metadata.get("format").unwrap(), "csh");
    }

    #[tokio::test]
    async fn test_csh_loader_empty_file() {
        let file = NamedTempFile::new().unwrap();

        let loader = CshLoader::new(file.path());
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1);
        assert!(docs[0].page_content.is_empty());
    }

    #[tokio::test]
    async fn test_csh_loader_builder_chain() {
        let file = NamedTempFile::new().unwrap();

        let loader = CshLoader::new(file.path())
            .with_separate_definitions(true)
            .with_separate_definitions(false);

        let docs = loader.load().await.unwrap();
        assert_eq!(docs.len(), 1);
    }

    // ============================================================================
    // KshLoader Tests
    // ============================================================================

    #[tokio::test]
    async fn test_ksh_loader_basic() {
        let mut file = NamedTempFile::new().unwrap();
        write!(file, "#!/bin/ksh\nprint hello\ntypeset -i num=5").unwrap();

        let loader = KshLoader::new(file.path());
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1);
        assert!(docs[0].page_content.contains("print hello"));
        assert_eq!(docs[0].metadata.get("format").unwrap(), "ksh");
    }

    #[tokio::test]
    async fn test_ksh_loader_separate_definitions_function_keyword() {
        let mut file = NamedTempFile::new().unwrap();
        write!(
            file,
            "function greet {{\n    print \"Hello $1\"\n}}\nfunction bye {{\n    print \"Goodbye\"\n}}"
        )
        .unwrap();

        let loader = KshLoader::new(file.path()).with_separate_definitions(true);
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 2);
        assert_eq!(docs[0].metadata.get("definition_name").unwrap(), "greet");
        assert!(docs[0].page_content.contains("Hello"));
        assert_eq!(docs[1].metadata.get("definition_name").unwrap(), "bye");
    }

    #[tokio::test]
    async fn test_ksh_loader_separate_definitions_posix_syntax() {
        let mut file = NamedTempFile::new().unwrap();
        write!(
            file,
            "hello() {{\n    echo \"Hello\"\n}}\nworld() {{\n    echo \"World\"\n}}"
        )
        .unwrap();

        let loader = KshLoader::new(file.path()).with_separate_definitions(true);
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 2);
        assert_eq!(docs[0].metadata.get("definition_name").unwrap(), "hello");
        assert_eq!(docs[1].metadata.get("definition_name").unwrap(), "world");
    }

    #[tokio::test]
    async fn test_ksh_loader_metadata() {
        let mut file = NamedTempFile::new().unwrap();
        write!(file, "echo test").unwrap();

        let loader = KshLoader::new(file.path());
        let docs = loader.load().await.unwrap();

        assert!(docs[0].metadata.contains_key("source"));
        assert_eq!(docs[0].metadata.get("format").unwrap(), "ksh");
    }

    #[tokio::test]
    async fn test_ksh_loader_empty_file() {
        let file = NamedTempFile::new().unwrap();

        let loader = KshLoader::new(file.path());
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1);
        assert!(docs[0].page_content.is_empty());
    }

    #[tokio::test]
    async fn test_ksh_loader_builder_chain() {
        let file = NamedTempFile::new().unwrap();

        let loader = KshLoader::new(file.path())
            .with_separate_definitions(true)
            .with_separate_definitions(false);

        let docs = loader.load().await.unwrap();
        assert_eq!(docs.len(), 1);
    }

    #[tokio::test]
    async fn test_ksh_loader_nested_braces() {
        let mut file = NamedTempFile::new().unwrap();
        write!(
            file,
            "function complex {{\n    if [[ $1 == \"test\" ]]; then\n        {{\n            echo nested\n        }}\n    fi\n}}"
        )
        .unwrap();

        let loader = KshLoader::new(file.path()).with_separate_definitions(true);
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1);
        assert_eq!(docs[0].metadata.get("definition_name").unwrap(), "complex");
        assert!(docs[0].page_content.contains("nested"));
    }

    // ============================================================================
    // TclLoader Tests
    // ============================================================================

    #[tokio::test]
    async fn test_tcl_loader_basic() {
        let mut file = NamedTempFile::new().unwrap();
        write!(file, "#!/usr/bin/env tclsh\nputs \"Hello World\"\nset x 10").unwrap();

        let loader = TclLoader::new(file.path());
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1);
        assert!(docs[0].page_content.contains("puts"));
        assert_eq!(docs[0].metadata.get("format").unwrap(), "tcl");
    }

    #[tokio::test]
    async fn test_tcl_loader_separate_definitions_proc() {
        let mut file = NamedTempFile::new().unwrap();
        write!(
            file,
            "proc greet {{name}} {{\n    puts \"Hello $name\"\n}}\nproc add {{a b}} {{\n    return [expr {{$a + $b}}]\n}}"
        )
        .unwrap();

        let loader = TclLoader::new(file.path()).with_separate_definitions(true);
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 2);
        assert_eq!(docs[0].metadata.get("definition_name").unwrap(), "greet");
        assert_eq!(docs[1].metadata.get("definition_name").unwrap(), "add");
    }

    #[tokio::test]
    async fn test_tcl_loader_metadata() {
        let mut file = NamedTempFile::new().unwrap();
        write!(file, "puts test").unwrap();

        let loader = TclLoader::new(file.path());
        let docs = loader.load().await.unwrap();

        assert!(docs[0].metadata.contains_key("source"));
        assert_eq!(docs[0].metadata.get("format").unwrap(), "tcl");
    }

    #[tokio::test]
    async fn test_tcl_loader_empty_file() {
        let file = NamedTempFile::new().unwrap();

        let loader = TclLoader::new(file.path());
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1);
        assert!(docs[0].page_content.is_empty());
    }

    #[tokio::test]
    async fn test_tcl_loader_builder_chain() {
        let file = NamedTempFile::new().unwrap();

        let loader = TclLoader::new(file.path())
            .with_separate_definitions(true)
            .with_separate_definitions(false);

        let docs = loader.load().await.unwrap();
        assert_eq!(docs.len(), 1);
    }

    #[tokio::test]
    async fn test_tcl_loader_nested_braces() {
        let mut file = NamedTempFile::new().unwrap();
        write!(
            file,
            "proc complex {{}} {{\n    if {{1}} {{\n        puts \"nested\"\n    }}\n}}"
        )
        .unwrap();

        let loader = TclLoader::new(file.path()).with_separate_definitions(true);
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1);
        assert_eq!(docs[0].metadata.get("definition_name").unwrap(), "complex");
    }

    // ============================================================================
    // AwkLoader Tests
    // ============================================================================

    #[tokio::test]
    async fn test_awk_loader_basic() {
        let mut file = NamedTempFile::new().unwrap();
        write!(file, "{{ print $0 }}").unwrap();

        let loader = AwkLoader::new(file.path());
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1);
        assert!(docs[0].page_content.contains("print"));
        assert_eq!(docs[0].metadata.get("format").unwrap(), "awk");
    }

    #[tokio::test]
    async fn test_awk_loader_separate_definitions_function() {
        let mut file = NamedTempFile::new().unwrap();
        write!(
            file,
            "function greet(name) {{\n    print \"Hello \" name\n}}\nfunction add(a, b) {{\n    return a + b\n}}"
        )
        .unwrap();

        let loader = AwkLoader::new(file.path()).with_separate_definitions(true);
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 2);
        assert_eq!(docs[0].metadata.get("definition_name").unwrap(), "greet");
        assert_eq!(docs[1].metadata.get("definition_name").unwrap(), "add");
    }

    #[tokio::test]
    async fn test_awk_loader_separate_definitions_begin_end() {
        let mut file = NamedTempFile::new().unwrap();
        write!(
            file,
            "BEGIN {{\n    print \"Start\"\n}}\n{{ print $0 }}\nEND {{\n    print \"End\"\n}}"
        )
        .unwrap();

        let loader = AwkLoader::new(file.path()).with_separate_definitions(true);
        let docs = loader.load().await.unwrap();

        assert!(docs.len() >= 2);
        // Check that BEGIN and END blocks are captured
        let has_begin = docs.iter().any(|d| {
            d.metadata
                .get("definition_name")
                .map(|n| n == "BEGIN")
                .unwrap_or(false)
        });
        let has_end = docs.iter().any(|d| {
            d.metadata
                .get("definition_name")
                .map(|n| n == "END")
                .unwrap_or(false)
        });
        assert!(has_begin);
        assert!(has_end);
    }

    #[tokio::test]
    async fn test_awk_loader_metadata() {
        let mut file = NamedTempFile::new().unwrap();
        write!(file, "{{ print }}").unwrap();

        let loader = AwkLoader::new(file.path());
        let docs = loader.load().await.unwrap();

        assert!(docs[0].metadata.contains_key("source"));
        assert_eq!(docs[0].metadata.get("format").unwrap(), "awk");
    }

    #[tokio::test]
    async fn test_awk_loader_empty_file() {
        let file = NamedTempFile::new().unwrap();

        let loader = AwkLoader::new(file.path());
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1);
        assert!(docs[0].page_content.is_empty());
    }

    #[tokio::test]
    async fn test_awk_loader_builder_chain() {
        let file = NamedTempFile::new().unwrap();

        let loader = AwkLoader::new(file.path())
            .with_separate_definitions(true)
            .with_separate_definitions(false);

        let docs = loader.load().await.unwrap();
        assert_eq!(docs.len(), 1);
    }

    #[tokio::test]
    async fn test_awk_loader_pattern_action() {
        let mut file = NamedTempFile::new().unwrap();
        write!(file, "/error/ {{ print \"Found error: \" $0 }}").unwrap();

        let loader = AwkLoader::new(file.path());
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1);
        assert!(docs[0].page_content.contains("error"));
    }

    // ============================================================================
    // SedLoader Tests
    // ============================================================================

    #[tokio::test]
    async fn test_sed_loader_basic() {
        let mut file = NamedTempFile::new().unwrap();
        write!(file, "s/foo/bar/g\ns/hello/world/").unwrap();

        let loader = SedLoader::new(file.path());
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1);
        assert!(docs[0].page_content.contains("s/foo/bar/g"));
        assert_eq!(docs[0].metadata.get("format").unwrap(), "sed");
    }

    #[tokio::test]
    async fn test_sed_loader_metadata() {
        let mut file = NamedTempFile::new().unwrap();
        write!(file, "s/test/replace/").unwrap();

        let loader = SedLoader::new(file.path());
        let docs = loader.load().await.unwrap();

        assert!(docs[0].metadata.contains_key("source"));
        assert_eq!(docs[0].metadata.get("format").unwrap(), "sed");
    }

    #[tokio::test]
    async fn test_sed_loader_empty_file() {
        let file = NamedTempFile::new().unwrap();

        let loader = SedLoader::new(file.path());
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1);
        assert!(docs[0].page_content.is_empty());
    }

    #[tokio::test]
    async fn test_sed_loader_complex_script() {
        let mut file = NamedTempFile::new().unwrap();
        write!(
            file,
            "#!/bin/sed -f\n# Remove leading whitespace\ns/^[[:space:]]*//\n# Remove trailing whitespace\ns/[[:space:]]*$//\n/^$/d"
        )
        .unwrap();

        let loader = SedLoader::new(file.path());
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1);
        assert!(docs[0].page_content.contains("Remove leading"));
        assert!(docs[0].page_content.contains("Remove trailing"));
    }

    #[tokio::test]
    async fn test_sed_loader_address_commands() {
        let mut file = NamedTempFile::new().unwrap();
        write!(file, "1,10s/old/new/\n/pattern/d\n$a\\\nappend this").unwrap();

        let loader = SedLoader::new(file.path());
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1);
        assert!(docs[0].page_content.contains("1,10s"));
    }
}
