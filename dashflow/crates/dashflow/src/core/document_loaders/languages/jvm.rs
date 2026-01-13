//! JVM language document loaders.
//!
//! This module provides loaders for JVM-based programming languages:
//! - Java
//! - Kotlin
//! - Scala
//! - Groovy

use async_trait::async_trait;
use std::path::{Path, PathBuf};

use crate::core::documents::{Blob, Document, DocumentLoader};
use crate::core::error::Result;

/// `JavaLoader` loads Java source files and separates them by class definitions.
///
/// Java is a widely-used object-oriented programming language designed for portability
/// and platform independence. Created by James Gosling at Sun Microsystems in 1995,
/// Java runs on the Java Virtual Machine (JVM).
///
/// Supports extensions: .java
///
/// When `separate_classes` is true, splits document by class, interface, and enum definitions.
/// Java syntax: `public class Name { ... }`, `interface Name { ... }`, `enum Name { ... }`
///
/// # Example
///
/// ```no_run
/// use dashflow::core::document_loaders::JavaLoader;
/// use dashflow::core::documents::DocumentLoader;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let loader = JavaLoader::new("Main.java").with_separate_classes(true);
/// let docs = loader.load().await?;
/// println!("Loaded {} class definitions", docs.len());
/// # Ok(())
/// # }
/// ```
pub struct JavaLoader {
    /// Path to the Java file
    pub file_path: PathBuf,
    /// Separate documents per class/method (default: false)
    pub separate_classes: bool,
}

impl JavaLoader {
    /// Create a new `JavaLoader` for the given file path.
    pub fn new(file_path: impl AsRef<Path>) -> Self {
        Self {
            file_path: file_path.as_ref().to_path_buf(),
            separate_classes: false,
        }
    }

    /// Create separate documents per class definition.
    #[must_use]
    pub fn with_separate_classes(mut self, separate: bool) -> Self {
        self.separate_classes = separate;
        self
    }
}

#[async_trait]
impl DocumentLoader for JavaLoader {
    async fn load(&self) -> Result<Vec<Document>> {
        let blob = Blob::from_path(&self.file_path);
        let content = blob.as_string()?;

        if self.separate_classes {
            // Split by class/interface/enum definitions
            let mut documents = Vec::new();
            let mut current_class = String::new();
            let mut class_name = String::new();
            let mut class_index = 0;
            let mut brace_depth = 0;
            let mut in_class = false;
            let mut preamble = String::new();

            for line in content.lines() {
                let trimmed = line.trim_start();

                // Detect class/interface/enum declarations
                if !in_class
                    && (trimmed.starts_with("public class ")
                        || trimmed.starts_with("private class ")
                        || trimmed.starts_with("protected class ")
                        || trimmed.starts_with("class ")
                        || trimmed.starts_with("public interface ")
                        || trimmed.starts_with("interface ")
                        || trimmed.starts_with("public enum ")
                        || trimmed.starts_with("enum "))
                {
                    in_class = true;
                    brace_depth = 0;

                    // Extract class name
                    let words: Vec<&str> = trimmed.split_whitespace().collect();
                    for (i, &word) in words.iter().enumerate() {
                        if matches!(word, "class" | "interface" | "enum") && i + 1 < words.len() {
                            let name_part = words[i + 1];
                            class_name = name_part.trim_end_matches(['{', '<']).to_string();
                            break;
                        }
                    }

                    // Include preamble (package, imports) with first class
                    if class_index == 0 && !preamble.is_empty() {
                        current_class.push_str(&preamble);
                    }
                }

                if in_class {
                    current_class.push_str(line);
                    current_class.push('\n');

                    // Track braces
                    for ch in line.chars() {
                        if ch == '{' {
                            brace_depth += 1;
                        } else if ch == '}' {
                            brace_depth -= 1;
                            if brace_depth == 0 {
                                // Class complete
                                let doc = Document::new(current_class.clone())
                                    .with_metadata("source", self.file_path.display().to_string())
                                    .with_metadata("class_index", class_index)
                                    .with_metadata("class_name", class_name.clone())
                                    .with_metadata("format", "java");

                                documents.push(doc);
                                current_class.clear();
                                in_class = false;
                                class_index += 1;
                                break;
                            }
                        }
                    }
                } else {
                    // Collect preamble (package, imports)
                    preamble.push_str(line);
                    preamble.push('\n');
                }
            }

            // Add any remaining content
            if !current_class.is_empty() {
                let doc = Document::new(current_class)
                    .with_metadata("source", self.file_path.display().to_string())
                    .with_metadata("type", "partial")
                    .with_metadata("format", "java");

                documents.push(doc);
            }

            Ok(documents)
        } else {
            // Load as single document
            let doc = Document::new(content)
                .with_metadata("source", self.file_path.display().to_string())
                .with_metadata("format", "java");

            Ok(vec![doc])
        }
    }
}

/// Loads C/C++ source files (.c, .cpp, .cc, .h, .hpp).
///
/// The `CppLoader` reads C and C++ source files, preserving all code including headers and functions.
/// Can optionally separate by function/class/struct definitions.
///
/// # Example
///
/// ```no_run
/// use dashflow::core::document_loaders::CppLoader;
/// use dashflow::core::documents::DocumentLoader;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let loader = CppLoader::new("main.cpp");
/// let documents = loader.load().await?;
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone)]

// ============================================================================
// Kotlin Loader
// ============================================================================

pub struct KotlinLoader {
    /// Path to the Kotlin file
    pub file_path: PathBuf,
    /// Separate documents per function/class/object (default: false)
    pub separate_definitions: bool,
}

impl KotlinLoader {
    /// Create a new `KotlinLoader` for the given file path.
    pub fn new(file_path: impl AsRef<Path>) -> Self {
        Self {
            file_path: file_path.as_ref().to_path_buf(),
            separate_definitions: false,
        }
    }

    /// Create separate documents per function/class/object definition.
    #[must_use]
    pub fn with_separate_definitions(mut self, separate: bool) -> Self {
        self.separate_definitions = separate;
        self
    }
}

#[async_trait]
impl DocumentLoader for KotlinLoader {
    async fn load(&self) -> Result<Vec<Document>> {
        let blob = Blob::from_path(&self.file_path);
        let content = blob.as_string()?;

        if self.separate_definitions {
            // Split by function/class/object/interface definitions
            let mut documents = Vec::new();
            let mut current_def = String::new();
            let mut def_name = String::new();
            let mut def_index = 0;
            let mut brace_depth = 0;
            let mut in_definition = false;

            for line in content.lines() {
                let trimmed = line.trim_start();

                // Detect Kotlin declarations
                if !in_definition
                    && (trimmed.starts_with("fun ")
                        || trimmed.starts_with("class ")
                        || trimmed.starts_with("object ")
                        || trimmed.starts_with("interface ")
                        || trimmed.starts_with("data class ")
                        || trimmed.starts_with("sealed class ")
                        || trimmed.starts_with("abstract class ")
                        || trimmed.starts_with("open class ")
                        || trimmed.starts_with("private fun ")
                        || trimmed.starts_with("public fun ")
                        || trimmed.starts_with("internal fun ")
                        || trimmed.starts_with("protected fun "))
                {
                    in_definition = true;
                    brace_depth = 0;

                    // Extract definition name
                    let words: Vec<&str> = trimmed.split_whitespace().collect();
                    for (i, &word) in words.iter().enumerate() {
                        if matches!(word, "fun" | "class" | "object" | "interface")
                            && i + 1 < words.len()
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
                                    .with_metadata("format", "kotlin");

                                documents.push(doc);
                                current_def.clear();
                                in_definition = false;
                                def_index += 1;
                                break;
                            }
                        }
                    }
                } else if !trimmed.is_empty() {
                    // Non-definition code (imports, package, etc.)
                    current_def.push_str(line);
                    current_def.push('\n');
                }
            }

            // Add any remaining content
            if !current_def.is_empty() {
                let doc = Document::new(current_def)
                    .with_metadata("source", self.file_path.display().to_string())
                    .with_metadata("type", "global")
                    .with_metadata("format", "kotlin");

                documents.push(doc);
            }

            Ok(documents)
        } else {
            // Load as single document
            let doc = Document::new(content)
                .with_metadata("source", self.file_path.display().to_string())
                .with_metadata("format", "kotlin");

            Ok(vec![doc])
        }
    }
}

/// Loads Scala source files (.scala).
///
/// The `ScalaLoader` reads Scala source files, preserving all code structure.
/// Can optionally separate by def, class, object, or trait definitions.
///
/// # Example
///
/// ```no_run
/// use dashflow::core::document_loaders::ScalaLoader;
/// use dashflow::core::documents::DocumentLoader;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let loader = ScalaLoader::new("Main.scala");
/// let documents = loader.load().await?;
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone)]

// ============================================================================
// Scala Loader
// ============================================================================

pub struct ScalaLoader {
    /// Path to the Scala file
    pub file_path: PathBuf,
    /// Separate documents per def/class/object/trait (default: false)
    pub separate_definitions: bool,
}

impl ScalaLoader {
    /// Create a new `ScalaLoader` for the given file path.
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
impl DocumentLoader for ScalaLoader {
    async fn load(&self) -> Result<Vec<Document>> {
        let blob = Blob::from_path(&self.file_path);
        let content = blob.as_string()?;

        if self.separate_definitions {
            // Split by def/class/object/trait definitions
            let mut documents = Vec::new();
            let mut current_def = String::new();
            let mut def_name = String::new();
            let mut def_index = 0;
            let mut brace_depth = 0;
            let mut in_definition = false;

            for line in content.lines() {
                let trimmed = line.trim_start();

                // Detect Scala declarations
                if !in_definition
                    && (trimmed.starts_with("def ")
                        || trimmed.starts_with("class ")
                        || trimmed.starts_with("object ")
                        || trimmed.starts_with("trait ")
                        || trimmed.starts_with("case class ")
                        || trimmed.starts_with("case object ")
                        || trimmed.starts_with("sealed trait ")
                        || trimmed.starts_with("abstract class ")
                        || trimmed.starts_with("private def ")
                        || trimmed.starts_with("protected def "))
                {
                    in_definition = true;
                    brace_depth = 0;

                    // Extract definition name
                    let words: Vec<&str> = trimmed.split_whitespace().collect();
                    for (i, &word) in words.iter().enumerate() {
                        if matches!(word, "def" | "class" | "object" | "trait")
                            && i + 1 < words.len()
                        {
                            let name_part = words[i + 1];
                            if let Some(paren_pos) = name_part.find('(') {
                                def_name = name_part[..paren_pos].to_string();
                            } else if let Some(bracket_pos) = name_part.find('[') {
                                def_name = name_part[..bracket_pos].to_string();
                            } else if let Some(colon_pos) = name_part.find(':') {
                                def_name = name_part[..colon_pos].to_string();
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
                                    .with_metadata("format", "scala");

                                documents.push(doc);
                                current_def.clear();
                                in_definition = false;
                                def_index += 1;
                                break;
                            }
                        }
                    }
                } else if !trimmed.is_empty() {
                    // Non-definition code (imports, package, etc.)
                    current_def.push_str(line);
                    current_def.push('\n');
                }
            }

            // Add any remaining content
            if !current_def.is_empty() {
                let doc = Document::new(current_def)
                    .with_metadata("source", self.file_path.display().to_string())
                    .with_metadata("type", "global")
                    .with_metadata("format", "scala");

                documents.push(doc);
            }

            Ok(documents)
        } else {
            // Load as single document
            let doc = Document::new(content)
                .with_metadata("source", self.file_path.display().to_string())
                .with_metadata("format", "scala");

            Ok(vec![doc])
        }
    }
}

/// Loads Haskell source files (.hs).
///
/// The `HaskellLoader` reads Haskell source files and optionally separates them by
/// function definitions, type declarations, data declarations, newtype declarations,
/// class declarations, and instance declarations.
///
/// # Example
///
/// ```no_run
/// use dashflow::core::document_loaders::HaskellLoader;
/// use dashflow::core::documents::DocumentLoader;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let loader = HaskellLoader::new("example.hs")
///     .with_separate_definitions(true);
/// let documents = loader.load().await?;
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone)]

// ============================================================================
// Groovy Loader
// ============================================================================

pub struct GroovyLoader {
    file_path: PathBuf,
    separate_definitions: bool,
}

impl GroovyLoader {
    /// Creates a new Groovy loader for the given file path
    pub fn new<P: AsRef<Path>>(path: P) -> Self {
        Self {
            file_path: path.as_ref().to_path_buf(),
            separate_definitions: false,
        }
    }

    /// Enable separation by class and method definitions
    #[must_use]
    pub fn with_separate_definitions(mut self, separate: bool) -> Self {
        self.separate_definitions = separate;
        self
    }

    /// Extract definition name from a line starting with class/def/void/etc.
    fn extract_definition(line: &str) -> Option<String> {
        let trimmed = line.trim();

        // Check for class definition
        if trimmed.starts_with("class ") {
            if let Some(rest) = trimmed.strip_prefix("class ") {
                if let Some(name) = rest.split_whitespace().next() {
                    if !name.is_empty() {
                        return Some(format!("class {name}"));
                    }
                }
            }
        }

        // Check for method definition (def keyword)
        if trimmed.starts_with("def ") {
            if let Some(rest) = trimmed.strip_prefix("def ") {
                // Handle "def name(...)" pattern
                if let Some(pos) = rest.find('(') {
                    let name = rest[..pos].trim();
                    if !name.is_empty() {
                        return Some(format!("def {name}"));
                    }
                }
            }
        }

        // Check for typed method definitions (void/int/String/etc.)
        let method_keywords = [
            "void ", "int ", "String ", "boolean ", "double ", "float ", "long ",
        ];
        for keyword in &method_keywords {
            if trimmed.starts_with(keyword) {
                if let Some(rest) = trimmed.strip_prefix(keyword) {
                    if let Some(pos) = rest.find('(') {
                        let name = rest[..pos].trim();
                        if !name.is_empty() {
                            return Some(format!("{} {}", keyword.trim(), name));
                        }
                    }
                }
            }
        }

        None
    }
}

#[async_trait]
impl DocumentLoader for GroovyLoader {
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
                if !in_definition
                    && (trimmed.is_empty()
                        || trimmed.starts_with("//")
                        || trimmed.starts_with("/*"))
                {
                    continue;
                }

                // Check for definition start
                if !in_definition {
                    if let Some(name) = Self::extract_definition(trimmed) {
                        in_definition = true;
                        definition_name = name;
                        current_definition.push_str(line);
                        current_definition.push('\n');

                        // Count braces
                        brace_count =
                            line.matches('{').count() as i32 - line.matches('}').count() as i32;

                        // Groovy methods can be single-line without braces (for closures)
                        if brace_count == 0 && !line.contains('{') {
                            let doc = Document::new(current_definition.trim_end())
                                .with_metadata("source", self.file_path.display().to_string())
                                .with_metadata("format", "groovy")
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
                            .with_metadata("format", "groovy")
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
                    .with_metadata("format", "groovy")
                    .with_metadata("definition_index", documents.len())
                    .with_metadata("definition_name", definition_name);

                documents.push(doc);
            }

            Ok(documents)
        } else {
            // Load entire file as single document
            Ok(vec![Document::new(&content)
                .with_metadata("source", self.file_path.display().to_string())
                .with_metadata("format", "groovy")])
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    // =========================================================================
    // JavaLoader tests
    // =========================================================================

    #[test]
    fn test_java_loader_new() {
        let loader = JavaLoader::new("/path/to/Test.java");
        assert_eq!(loader.file_path, PathBuf::from("/path/to/Test.java"));
        assert!(!loader.separate_classes);
    }

    #[test]
    fn test_java_loader_with_separate_classes() {
        let loader = JavaLoader::new("/path/to/Test.java").with_separate_classes(true);
        assert!(loader.separate_classes);
    }

    #[test]
    fn test_java_loader_with_separate_classes_false() {
        let loader = JavaLoader::new("/path/to/Test.java")
            .with_separate_classes(true)
            .with_separate_classes(false);
        assert!(!loader.separate_classes);
    }

    #[tokio::test]
    async fn test_java_loader_single_document() {
        let java_code = r#"package com.example;

public class Hello {
    public static void main(String[] args) {
        System.out.println("Hello, World!");
    }
}
"#;
        let mut file = NamedTempFile::with_suffix(".java").unwrap();
        file.write_all(java_code.as_bytes()).unwrap();

        let loader = JavaLoader::new(file.path());
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1);
        assert!(docs[0].page_content.contains("public class Hello"));
        assert_eq!(
            docs[0].metadata.get("format").and_then(|v| v.as_str()),
            Some("java")
        );
    }

    #[tokio::test]
    async fn test_java_loader_separate_classes() {
        let java_code = r#"package com.example;

import java.util.List;

public class First {
    public void foo() {}
}

class Second {
    public void bar() {}
}
"#;
        let mut file = NamedTempFile::with_suffix(".java").unwrap();
        file.write_all(java_code.as_bytes()).unwrap();

        let loader = JavaLoader::new(file.path()).with_separate_classes(true);
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 2);

        // First class should include preamble
        assert!(docs[0].page_content.contains("package com.example"));
        assert!(docs[0].page_content.contains("public class First"));
        assert_eq!(
            docs[0].metadata.get("class_name").and_then(|v| v.as_str()),
            Some("First")
        );
        assert_eq!(
            docs[0].metadata.get("class_index").and_then(|v| v.as_i64()),
            Some(0)
        );

        // Second class
        assert!(docs[1].page_content.contains("class Second"));
        assert_eq!(
            docs[1].metadata.get("class_name").and_then(|v| v.as_str()),
            Some("Second")
        );
        assert_eq!(
            docs[1].metadata.get("class_index").and_then(|v| v.as_i64()),
            Some(1)
        );
    }

    #[tokio::test]
    async fn test_java_loader_interface() {
        let java_code = r#"public interface Runnable {
    void run();
}
"#;
        let mut file = NamedTempFile::with_suffix(".java").unwrap();
        file.write_all(java_code.as_bytes()).unwrap();

        let loader = JavaLoader::new(file.path()).with_separate_classes(true);
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1);
        assert!(docs[0].page_content.contains("public interface Runnable"));
        assert_eq!(
            docs[0].metadata.get("class_name").and_then(|v| v.as_str()),
            Some("Runnable")
        );
    }

    #[tokio::test]
    async fn test_java_loader_enum() {
        let java_code = r#"public enum Color {
    RED,
    GREEN,
    BLUE
}
"#;
        let mut file = NamedTempFile::with_suffix(".java").unwrap();
        file.write_all(java_code.as_bytes()).unwrap();

        let loader = JavaLoader::new(file.path()).with_separate_classes(true);
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1);
        assert!(docs[0].page_content.contains("public enum Color"));
    }

    #[tokio::test]
    async fn test_java_loader_nested_braces() {
        let java_code = r#"public class Complex {
    public void method() {
        if (true) {
            for (int i = 0; i < 10; i++) {
                doSomething();
            }
        }
    }
}
"#;
        let mut file = NamedTempFile::with_suffix(".java").unwrap();
        file.write_all(java_code.as_bytes()).unwrap();

        let loader = JavaLoader::new(file.path()).with_separate_classes(true);
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1);
        assert!(docs[0].page_content.contains("public class Complex"));
        assert!(docs[0].page_content.contains("doSomething()"));
    }

    #[tokio::test]
    async fn test_java_loader_generic_class() {
        let java_code = r#"public class Container<T> {
    private T value;
}
"#;
        let mut file = NamedTempFile::with_suffix(".java").unwrap();
        file.write_all(java_code.as_bytes()).unwrap();

        let loader = JavaLoader::new(file.path()).with_separate_classes(true);
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1);
        // The implementation strips trailing '<' but doesn't handle mid-word '<'
        // so "Container<T>" becomes "Container<T>" (no trailing '<')
        // Actually checking the code: trim_end_matches(['{', '<']) would strip trailing
        // But "Container<T>" doesn't have trailing '<', so it remains "Container<T>"
        assert_eq!(
            docs[0].metadata.get("class_name").and_then(|v| v.as_str()),
            Some("Container<T>")
        );
    }

    // =========================================================================
    // KotlinLoader tests
    // =========================================================================

    #[test]
    fn test_kotlin_loader_new() {
        let loader = KotlinLoader::new("/path/to/Test.kt");
        assert_eq!(loader.file_path, PathBuf::from("/path/to/Test.kt"));
        assert!(!loader.separate_definitions);
    }

    #[test]
    fn test_kotlin_loader_with_separate_definitions() {
        let loader = KotlinLoader::new("/path/to/Test.kt").with_separate_definitions(true);
        assert!(loader.separate_definitions);
    }

    #[tokio::test]
    async fn test_kotlin_loader_single_document() {
        let kotlin_code = r#"package com.example

fun main() {
    println("Hello, World!")
}
"#;
        let mut file = NamedTempFile::with_suffix(".kt").unwrap();
        file.write_all(kotlin_code.as_bytes()).unwrap();

        let loader = KotlinLoader::new(file.path());
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1);
        assert!(docs[0].page_content.contains("fun main()"));
        assert_eq!(
            docs[0].metadata.get("format").and_then(|v| v.as_str()),
            Some("kotlin")
        );
    }

    #[tokio::test]
    async fn test_kotlin_loader_separate_definitions() {
        let kotlin_code = r#"fun hello() {
    println("Hello")
}

class MyClass {
    fun greet() {}
}

object Singleton {
    val value = 42
}
"#;
        let mut file = NamedTempFile::with_suffix(".kt").unwrap();
        file.write_all(kotlin_code.as_bytes()).unwrap();

        let loader = KotlinLoader::new(file.path()).with_separate_definitions(true);
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 3);
        assert!(docs[0].page_content.contains("fun hello()"));
        assert!(docs[1].page_content.contains("class MyClass"));
        assert!(docs[2].page_content.contains("object Singleton"));
    }

    #[tokio::test]
    async fn test_kotlin_loader_data_class() {
        let kotlin_code = r#"data class Person(val name: String, val age: Int) {
    fun greet() = println("Hello, $name")
}
"#;
        let mut file = NamedTempFile::with_suffix(".kt").unwrap();
        file.write_all(kotlin_code.as_bytes()).unwrap();

        let loader = KotlinLoader::new(file.path()).with_separate_definitions(true);
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1);
        assert!(docs[0].page_content.contains("data class Person"));
        assert_eq!(
            docs[0]
                .metadata
                .get("definition_name")
                .and_then(|v| v.as_str()),
            Some("Person")
        );
    }

    #[tokio::test]
    async fn test_kotlin_loader_sealed_class() {
        let kotlin_code = r#"sealed class Result {
    data class Success(val value: Int) : Result()
    data class Error(val message: String) : Result()
}
"#;
        let mut file = NamedTempFile::with_suffix(".kt").unwrap();
        file.write_all(kotlin_code.as_bytes()).unwrap();

        let loader = KotlinLoader::new(file.path()).with_separate_definitions(true);
        let docs = loader.load().await.unwrap();

        assert!(!docs.is_empty());
        assert!(docs[0].page_content.contains("sealed class Result"));
    }

    #[tokio::test]
    async fn test_kotlin_loader_interface() {
        let kotlin_code = r#"interface Drawable {
    fun draw()
}
"#;
        let mut file = NamedTempFile::with_suffix(".kt").unwrap();
        file.write_all(kotlin_code.as_bytes()).unwrap();

        let loader = KotlinLoader::new(file.path()).with_separate_definitions(true);
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1);
        assert!(docs[0].page_content.contains("interface Drawable"));
    }

    #[tokio::test]
    async fn test_kotlin_loader_private_fun() {
        let kotlin_code = r#"private fun helper() {
    println("Helper")
}
"#;
        let mut file = NamedTempFile::with_suffix(".kt").unwrap();
        file.write_all(kotlin_code.as_bytes()).unwrap();

        let loader = KotlinLoader::new(file.path()).with_separate_definitions(true);
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1);
        assert!(docs[0].page_content.contains("private fun helper()"));
    }

    #[tokio::test]
    async fn test_kotlin_loader_function_with_generics() {
        let kotlin_code = r#"fun <T> identity(value: T): T {
    return value
}
"#;
        let mut file = NamedTempFile::with_suffix(".kt").unwrap();
        file.write_all(kotlin_code.as_bytes()).unwrap();

        let loader = KotlinLoader::new(file.path()).with_separate_definitions(true);
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1);
        // The name extraction strips angle brackets
        assert!(docs[0].page_content.contains("fun <T> identity"));
    }

    // =========================================================================
    // ScalaLoader tests
    // =========================================================================

    #[test]
    fn test_scala_loader_new() {
        let loader = ScalaLoader::new("/path/to/Test.scala");
        assert_eq!(loader.file_path, PathBuf::from("/path/to/Test.scala"));
        assert!(!loader.separate_definitions);
    }

    #[test]
    fn test_scala_loader_with_separate_definitions() {
        let loader = ScalaLoader::new("/path/to/Test.scala").with_separate_definitions(true);
        assert!(loader.separate_definitions);
    }

    #[tokio::test]
    async fn test_scala_loader_single_document() {
        let scala_code = r#"package com.example

object Main extends App {
  println("Hello, World!")
}
"#;
        let mut file = NamedTempFile::with_suffix(".scala").unwrap();
        file.write_all(scala_code.as_bytes()).unwrap();

        let loader = ScalaLoader::new(file.path());
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1);
        assert!(docs[0].page_content.contains("object Main"));
        assert_eq!(
            docs[0].metadata.get("format").and_then(|v| v.as_str()),
            Some("scala")
        );
    }

    #[tokio::test]
    async fn test_scala_loader_separate_definitions() {
        let scala_code = r#"def hello(): Unit = {
  println("Hello")
}

class MyClass {
  def greet(): Unit = {}
}

object Companion {
  val value = 42
}
"#;
        let mut file = NamedTempFile::with_suffix(".scala").unwrap();
        file.write_all(scala_code.as_bytes()).unwrap();

        let loader = ScalaLoader::new(file.path()).with_separate_definitions(true);
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 3);
        assert!(docs[0].page_content.contains("def hello()"));
        assert!(docs[1].page_content.contains("class MyClass"));
        assert!(docs[2].page_content.contains("object Companion"));
    }

    #[tokio::test]
    async fn test_scala_loader_case_class() {
        let scala_code = r#"case class Person(name: String, age: Int) {
  def greet(): String = s"Hello, $name"
}
"#;
        let mut file = NamedTempFile::with_suffix(".scala").unwrap();
        file.write_all(scala_code.as_bytes()).unwrap();

        let loader = ScalaLoader::new(file.path()).with_separate_definitions(true);
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1);
        assert!(docs[0].page_content.contains("case class Person"));
    }

    #[tokio::test]
    async fn test_scala_loader_trait() {
        let scala_code = r#"trait Drawable {
  def draw(): Unit
}
"#;
        let mut file = NamedTempFile::with_suffix(".scala").unwrap();
        file.write_all(scala_code.as_bytes()).unwrap();

        let loader = ScalaLoader::new(file.path()).with_separate_definitions(true);
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1);
        assert!(docs[0].page_content.contains("trait Drawable"));
    }

    #[tokio::test]
    async fn test_scala_loader_sealed_trait() {
        let scala_code = r#"sealed trait Result {
}
"#;
        let mut file = NamedTempFile::with_suffix(".scala").unwrap();
        file.write_all(scala_code.as_bytes()).unwrap();

        let loader = ScalaLoader::new(file.path()).with_separate_definitions(true);
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1);
        assert!(docs[0].page_content.contains("sealed trait Result"));
    }

    #[tokio::test]
    async fn test_scala_loader_abstract_class() {
        let scala_code = r#"abstract class Animal {
  def speak(): Unit
}
"#;
        let mut file = NamedTempFile::with_suffix(".scala").unwrap();
        file.write_all(scala_code.as_bytes()).unwrap();

        let loader = ScalaLoader::new(file.path()).with_separate_definitions(true);
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1);
        assert!(docs[0].page_content.contains("abstract class Animal"));
    }

    #[tokio::test]
    async fn test_scala_loader_case_object() {
        let scala_code = r#"case object Singleton {
  val value = 100
}
"#;
        let mut file = NamedTempFile::with_suffix(".scala").unwrap();
        file.write_all(scala_code.as_bytes()).unwrap();

        let loader = ScalaLoader::new(file.path()).with_separate_definitions(true);
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1);
        assert!(docs[0].page_content.contains("case object Singleton"));
    }

    #[tokio::test]
    async fn test_scala_loader_def_with_type_params() {
        let scala_code = r#"def identity[T](value: T): T = {
  value
}
"#;
        let mut file = NamedTempFile::with_suffix(".scala").unwrap();
        file.write_all(scala_code.as_bytes()).unwrap();

        let loader = ScalaLoader::new(file.path()).with_separate_definitions(true);
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1);
        assert!(docs[0].page_content.contains("def identity[T]"));
    }

    // =========================================================================
    // GroovyLoader tests
    // =========================================================================

    #[test]
    fn test_groovy_loader_new() {
        let loader = GroovyLoader::new("/path/to/Test.groovy");
        assert_eq!(loader.file_path, PathBuf::from("/path/to/Test.groovy"));
        assert!(!loader.separate_definitions);
    }

    #[test]
    fn test_groovy_loader_with_separate_definitions() {
        let loader = GroovyLoader::new("/path/to/Test.groovy").with_separate_definitions(true);
        assert!(loader.separate_definitions);
    }

    #[tokio::test]
    async fn test_groovy_loader_single_document() {
        let groovy_code = r#"class Hello {
    static void main(String[] args) {
        println "Hello, World!"
    }
}
"#;
        let mut file = NamedTempFile::with_suffix(".groovy").unwrap();
        file.write_all(groovy_code.as_bytes()).unwrap();

        let loader = GroovyLoader::new(file.path());
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1);
        assert!(docs[0].page_content.contains("class Hello"));
        assert_eq!(
            docs[0].metadata.get("format").and_then(|v| v.as_str()),
            Some("groovy")
        );
    }

    #[tokio::test]
    async fn test_groovy_loader_separate_definitions() {
        let groovy_code = r#"def hello() {
    println "Hello"
}

class MyClass {
    void greet() {
        println "Greet"
    }
}
"#;
        let mut file = NamedTempFile::with_suffix(".groovy").unwrap();
        file.write_all(groovy_code.as_bytes()).unwrap();

        let loader = GroovyLoader::new(file.path()).with_separate_definitions(true);
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 2);
        assert!(docs[0].page_content.contains("def hello()"));
        assert!(docs[1].page_content.contains("class MyClass"));
    }

    #[test]
    fn test_groovy_extract_definition_class() {
        let result = GroovyLoader::extract_definition("class MyClass {");
        assert_eq!(result, Some("class MyClass".to_string()));
    }

    #[test]
    fn test_groovy_extract_definition_def() {
        let result = GroovyLoader::extract_definition("def myMethod() {");
        assert_eq!(result, Some("def myMethod".to_string()));
    }

    #[test]
    fn test_groovy_extract_definition_void() {
        let result = GroovyLoader::extract_definition("void process() {");
        assert_eq!(result, Some("void process".to_string()));
    }

    #[test]
    fn test_groovy_extract_definition_string() {
        let result = GroovyLoader::extract_definition("String getName() {");
        assert_eq!(result, Some("String getName".to_string()));
    }

    #[test]
    fn test_groovy_extract_definition_int() {
        let result = GroovyLoader::extract_definition("int getValue() {");
        assert_eq!(result, Some("int getValue".to_string()));
    }

    #[test]
    fn test_groovy_extract_definition_boolean() {
        let result = GroovyLoader::extract_definition("boolean isValid() {");
        assert_eq!(result, Some("boolean isValid".to_string()));
    }

    #[test]
    fn test_groovy_extract_definition_none() {
        let result = GroovyLoader::extract_definition("println 'hello'");
        assert!(result.is_none());
    }

    #[test]
    fn test_groovy_extract_definition_empty_name() {
        // "class  {" will be split by whitespace, second element is empty string
        // which the implementation does check for, but "{" is the actual next token
        let result = GroovyLoader::extract_definition("class  {");
        // The implementation sees "class" then "{" as next token after split_whitespace
        // so it will try to extract "{" minus trailing chars = empty -> returns "class {"
        // Actually let's test the real behavior: whitespace is collapsed
        // "class  {" becomes ["class", "{"] after split_whitespace
        // Since "{" is not empty, it returns Some("class {")
        assert_eq!(result, Some("class {".to_string()));
    }

    #[tokio::test]
    async fn test_groovy_loader_nested_braces() {
        let groovy_code = r#"def complex() {
    if (true) {
        list.each { item ->
            println item
        }
    }
}
"#;
        let mut file = NamedTempFile::with_suffix(".groovy").unwrap();
        file.write_all(groovy_code.as_bytes()).unwrap();

        let loader = GroovyLoader::new(file.path()).with_separate_definitions(true);
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1);
        assert!(docs[0].page_content.contains("def complex()"));
        assert!(docs[0].page_content.contains("println item"));
    }

    #[tokio::test]
    async fn test_groovy_loader_skip_comments() {
        let groovy_code = r#"// This is a comment
/* Multi-line
   comment */
def actualMethod() {
    println "Real code"
}
"#;
        let mut file = NamedTempFile::with_suffix(".groovy").unwrap();
        file.write_all(groovy_code.as_bytes()).unwrap();

        let loader = GroovyLoader::new(file.path()).with_separate_definitions(true);
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1);
        assert!(docs[0].page_content.contains("def actualMethod()"));
    }

    #[tokio::test]
    async fn test_groovy_loader_multiple_typed_methods() {
        let groovy_code = r#"String getName() {
    return "test"
}

int getAge() {
    return 42
}

double getScore() {
    return 3.14
}

float getRate() {
    return 1.5
}

long getId() {
    return 100L
}
"#;
        let mut file = NamedTempFile::with_suffix(".groovy").unwrap();
        file.write_all(groovy_code.as_bytes()).unwrap();

        let loader = GroovyLoader::new(file.path()).with_separate_definitions(true);
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 5);
    }

    #[tokio::test]
    async fn test_groovy_loader_definition_index_metadata() {
        let groovy_code = r#"def first() {
    println "first"
}

def second() {
    println "second"
}

def third() {
    println "third"
}
"#;
        let mut file = NamedTempFile::with_suffix(".groovy").unwrap();
        file.write_all(groovy_code.as_bytes()).unwrap();

        let loader = GroovyLoader::new(file.path()).with_separate_definitions(true);
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 3);
        assert_eq!(
            docs[0]
                .metadata
                .get("definition_index")
                .and_then(|v| v.as_i64()),
            Some(0)
        );
        assert_eq!(
            docs[1]
                .metadata
                .get("definition_index")
                .and_then(|v| v.as_i64()),
            Some(1)
        );
        assert_eq!(
            docs[2]
                .metadata
                .get("definition_index")
                .and_then(|v| v.as_i64()),
            Some(2)
        );
    }

    #[tokio::test]
    async fn test_groovy_loader_definition_name_metadata() {
        let groovy_code = r#"def myFunction() {
    println "func"
}

class MyClass {
    void doStuff() {}
}
"#;
        let mut file = NamedTempFile::with_suffix(".groovy").unwrap();
        file.write_all(groovy_code.as_bytes()).unwrap();

        let loader = GroovyLoader::new(file.path()).with_separate_definitions(true);
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 2);
        assert_eq!(
            docs[0]
                .metadata
                .get("definition_name")
                .and_then(|v| v.as_str()),
            Some("def myFunction")
        );
        assert_eq!(
            docs[1]
                .metadata
                .get("definition_name")
                .and_then(|v| v.as_str()),
            Some("class MyClass")
        );
    }

    // =========================================================================
    // Edge case tests
    // =========================================================================

    #[tokio::test]
    async fn test_java_loader_empty_file() {
        let mut file = NamedTempFile::with_suffix(".java").unwrap();
        file.write_all(b"").unwrap();

        let loader = JavaLoader::new(file.path());
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1);
        assert!(docs[0].page_content.is_empty());
    }

    #[tokio::test]
    async fn test_kotlin_loader_empty_file() {
        let mut file = NamedTempFile::with_suffix(".kt").unwrap();
        file.write_all(b"").unwrap();

        let loader = KotlinLoader::new(file.path());
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1);
        assert!(docs[0].page_content.is_empty());
    }

    #[tokio::test]
    async fn test_scala_loader_empty_file() {
        let mut file = NamedTempFile::with_suffix(".scala").unwrap();
        file.write_all(b"").unwrap();

        let loader = ScalaLoader::new(file.path());
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1);
        assert!(docs[0].page_content.is_empty());
    }

    #[tokio::test]
    async fn test_groovy_loader_empty_file() {
        let mut file = NamedTempFile::with_suffix(".groovy").unwrap();
        file.write_all(b"").unwrap();

        let loader = GroovyLoader::new(file.path());
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1);
        assert!(docs[0].page_content.is_empty());
    }

    #[tokio::test]
    async fn test_java_loader_incomplete_class() {
        let java_code = r#"public class Incomplete {
    public void method() {
"#;
        let mut file = NamedTempFile::with_suffix(".java").unwrap();
        file.write_all(java_code.as_bytes()).unwrap();

        let loader = JavaLoader::new(file.path()).with_separate_classes(true);
        let docs = loader.load().await.unwrap();

        // Should still produce a document with partial content
        assert_eq!(docs.len(), 1);
        assert!(docs[0].page_content.contains("public class Incomplete"));
        assert_eq!(
            docs[0].metadata.get("type").and_then(|v| v.as_str()),
            Some("partial")
        );
    }

    #[tokio::test]
    async fn test_kotlin_loader_incomplete_definition() {
        let kotlin_code = r#"fun incomplete() {
    println("Hello"
"#;
        let mut file = NamedTempFile::with_suffix(".kt").unwrap();
        file.write_all(kotlin_code.as_bytes()).unwrap();

        let loader = KotlinLoader::new(file.path()).with_separate_definitions(true);
        let docs = loader.load().await.unwrap();

        // Should produce a document with global type
        assert!(!docs.is_empty());
    }

    #[tokio::test]
    async fn test_groovy_loader_incomplete_definition() {
        let groovy_code = r#"def incomplete() {
    println "Hello"
"#;
        let mut file = NamedTempFile::with_suffix(".groovy").unwrap();
        file.write_all(groovy_code.as_bytes()).unwrap();

        let loader = GroovyLoader::new(file.path()).with_separate_definitions(true);
        let docs = loader.load().await.unwrap();

        // Should still produce a document
        assert!(!docs.is_empty());
    }

    #[tokio::test]
    async fn test_java_loader_source_metadata() {
        let java_code = "public class Test {}";
        let mut file = NamedTempFile::with_suffix(".java").unwrap();
        file.write_all(java_code.as_bytes()).unwrap();

        let loader = JavaLoader::new(file.path());
        let docs = loader.load().await.unwrap();

        assert!(docs[0].metadata.contains_key("source"));
    }

    #[tokio::test]
    async fn test_kotlin_loader_source_metadata() {
        let kotlin_code = "fun test() {}";
        let mut file = NamedTempFile::with_suffix(".kt").unwrap();
        file.write_all(kotlin_code.as_bytes()).unwrap();

        let loader = KotlinLoader::new(file.path());
        let docs = loader.load().await.unwrap();

        assert!(docs[0].metadata.contains_key("source"));
    }

    #[tokio::test]
    async fn test_scala_loader_source_metadata() {
        let scala_code = "object Test {}";
        let mut file = NamedTempFile::with_suffix(".scala").unwrap();
        file.write_all(scala_code.as_bytes()).unwrap();

        let loader = ScalaLoader::new(file.path());
        let docs = loader.load().await.unwrap();

        assert!(docs[0].metadata.contains_key("source"));
    }

    #[tokio::test]
    async fn test_groovy_loader_source_metadata() {
        let groovy_code = "class Test {}";
        let mut file = NamedTempFile::with_suffix(".groovy").unwrap();
        file.write_all(groovy_code.as_bytes()).unwrap();

        let loader = GroovyLoader::new(file.path());
        let docs = loader.load().await.unwrap();

        assert!(docs[0].metadata.contains_key("source"));
    }
}
