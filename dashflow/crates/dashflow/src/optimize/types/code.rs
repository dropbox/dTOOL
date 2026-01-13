// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

//! Code type for language-tagged code generation

use serde::{Deserialize, Serialize};

/// Programming languages for code generation
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Language {
    /// Rust - systems programming language focused on safety and performance
    Rust,
    /// Python - high-level, interpreted programming language
    Python,
    /// JavaScript - web scripting language for browsers and Node.js
    JavaScript,
    /// TypeScript - typed superset of JavaScript
    TypeScript,
    /// Go - statically typed language designed at Google
    Go,
    /// Java - object-oriented language running on the JVM
    Java,
    /// C++ - systems programming language with object-oriented features
    Cpp,
    /// C - low-level systems programming language
    C,
    /// C# - object-oriented language for .NET platform
    CSharp,
    /// Ruby - dynamic, interpreted scripting language
    Ruby,
    /// Swift - Apple's modern programming language
    Swift,
    /// Kotlin - modern JVM language, Android development
    Kotlin,
    /// Scala - functional/object-oriented JVM language
    Scala,
    /// Haskell - purely functional programming language
    Haskell,
    /// SQL - structured query language for databases
    Sql,
    /// HTML - markup language for web pages
    Html,
    /// CSS - styling language for web pages
    Css,
    /// JSON - JavaScript Object Notation data format
    Json,
    /// XML - extensible markup language
    Xml,
    /// YAML - human-readable data serialization format
    Yaml,
    /// TOML - configuration file format
    Toml,
    /// Markdown - lightweight markup language
    Markdown,
    /// Shell - generic shell scripting (POSIX)
    Shell,
    /// Bash - Bourne Again Shell scripting
    Bash,
    /// PowerShell - Windows command shell and scripting language
    Powershell,
    /// Other - unknown or unsupported language
    Other,
}

impl Language {
    /// Get the language name for code blocks
    pub fn name(&self) -> &'static str {
        match self {
            Self::Rust => "rust",
            Self::Python => "python",
            Self::JavaScript => "javascript",
            Self::TypeScript => "typescript",
            Self::Go => "go",
            Self::Java => "java",
            Self::Cpp => "cpp",
            Self::C => "c",
            Self::CSharp => "csharp",
            Self::Ruby => "ruby",
            Self::Swift => "swift",
            Self::Kotlin => "kotlin",
            Self::Scala => "scala",
            Self::Haskell => "haskell",
            Self::Sql => "sql",
            Self::Html => "html",
            Self::Css => "css",
            Self::Json => "json",
            Self::Xml => "xml",
            Self::Yaml => "yaml",
            Self::Toml => "toml",
            Self::Markdown => "markdown",
            Self::Shell => "shell",
            Self::Bash => "bash",
            Self::Powershell => "powershell",
            Self::Other => "text",
        }
    }

    /// Get common file extension
    pub fn extension(&self) -> &'static str {
        match self {
            Self::Rust => "rs",
            Self::Python => "py",
            Self::JavaScript => "js",
            Self::TypeScript => "ts",
            Self::Go => "go",
            Self::Java => "java",
            Self::Cpp => "cpp",
            Self::C => "c",
            Self::CSharp => "cs",
            Self::Ruby => "rb",
            Self::Swift => "swift",
            Self::Kotlin => "kt",
            Self::Scala => "scala",
            Self::Haskell => "hs",
            Self::Sql => "sql",
            Self::Html => "html",
            Self::Css => "css",
            Self::Json => "json",
            Self::Xml => "xml",
            Self::Yaml => "yaml",
            Self::Toml => "toml",
            Self::Markdown => "md",
            Self::Shell => "sh",
            Self::Bash => "sh",
            Self::Powershell => "ps1",
            Self::Other => "txt",
        }
    }

    /// Detect language from file extension
    pub fn from_extension(ext: &str) -> Option<Self> {
        match ext.to_lowercase().as_str() {
            "rs" => Some(Self::Rust),
            "py" => Some(Self::Python),
            "js" | "mjs" | "cjs" => Some(Self::JavaScript),
            "ts" | "tsx" => Some(Self::TypeScript),
            "go" => Some(Self::Go),
            "java" => Some(Self::Java),
            "cpp" | "cc" | "cxx" | "hpp" => Some(Self::Cpp),
            "c" | "h" => Some(Self::C),
            "cs" => Some(Self::CSharp),
            "rb" => Some(Self::Ruby),
            "swift" => Some(Self::Swift),
            "kt" | "kts" => Some(Self::Kotlin),
            "scala" => Some(Self::Scala),
            "hs" => Some(Self::Haskell),
            "sql" => Some(Self::Sql),
            "html" | "htm" => Some(Self::Html),
            "css" | "scss" | "sass" => Some(Self::Css),
            "json" => Some(Self::Json),
            "xml" => Some(Self::Xml),
            "yaml" | "yml" => Some(Self::Yaml),
            "toml" => Some(Self::Toml),
            "md" | "markdown" => Some(Self::Markdown),
            "sh" => Some(Self::Shell),
            "bash" => Some(Self::Bash),
            "ps1" => Some(Self::Powershell),
            _ => None,
        }
    }

    /// Get MIME type for this language
    pub fn mime_type(&self) -> &'static str {
        match self {
            Self::Html => "text/html",
            Self::Css => "text/css",
            Self::JavaScript => "text/javascript",
            Self::TypeScript => "text/typescript",
            Self::Json => "application/json",
            Self::Xml => "application/xml",
            Self::Yaml => "text/yaml",
            Self::Markdown => "text/markdown",
            _ => "text/plain",
        }
    }

    /// Check if this is a systems language
    pub fn is_systems(&self) -> bool {
        matches!(self, Self::Rust | Self::C | Self::Cpp | Self::Go)
    }

    /// Check if this is a scripting language
    pub fn is_scripting(&self) -> bool {
        matches!(
            self,
            Self::Python | Self::JavaScript | Self::Ruby | Self::Shell | Self::Bash
        )
    }
}

impl std::fmt::Display for Language {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name())
    }
}

/// Code with language tagging
///
/// Represents a code snippet with its programming language,
/// enabling proper syntax highlighting and language-specific
/// handling in prompts and outputs.
///
/// # Example
///
/// ```rust
/// use dashflow::optimize::types::{Code, Language};
///
/// let code = Code::new("fn main() { println!(\"Hello!\"); }", Language::Rust)
///     .with_filename("main.rs");
///
/// // Format as markdown code block
/// let markdown = code.to_markdown();
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Code {
    /// Source code
    pub source: String,

    /// Programming language
    pub language: Language,

    /// Optional filename
    #[serde(skip_serializing_if = "Option::is_none")]
    pub filename: Option<String>,

    /// Optional description/purpose
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// Optional line number start (for excerpts)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line_start: Option<usize>,
}

impl Code {
    /// Create new code with language
    ///
    /// # Arguments
    /// * `source` - Source code
    /// * `language` - Programming language
    pub fn new(source: impl Into<String>, language: Language) -> Self {
        Self {
            source: source.into(),
            language,
            filename: None,
            description: None,
            line_start: None,
        }
    }

    /// Create code with auto-detected language from filename
    pub fn from_filename(source: impl Into<String>, filename: impl Into<String>) -> Self {
        let filename = filename.into();
        let ext = filename.rsplit('.').next().unwrap_or("");
        let language = Language::from_extension(ext).unwrap_or(Language::Other);

        Self {
            source: source.into(),
            language,
            filename: Some(filename),
            description: None,
            line_start: None,
        }
    }

    /// Set filename
    #[must_use]
    pub fn with_filename(mut self, filename: impl Into<String>) -> Self {
        self.filename = Some(filename.into());
        self
    }

    /// Set description
    #[must_use]
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Set line number start
    #[must_use]
    pub fn with_line_start(mut self, line: usize) -> Self {
        self.line_start = Some(line);
        self
    }

    /// Get number of lines
    pub fn line_count(&self) -> usize {
        self.source.lines().count()
    }

    /// Get character count
    pub fn char_count(&self) -> usize {
        self.source.len()
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.source.is_empty()
    }

    /// Format as markdown code block
    pub fn to_markdown(&self) -> String {
        let mut result = String::new();

        if let Some(filename) = &self.filename {
            result.push_str(&format!("**{}**\n", filename));
        }

        if let Some(desc) = &self.description {
            result.push_str(&format!("{}\n", desc));
        }

        result.push_str(&format!(
            "```{}\n{}\n```",
            self.language.name(),
            self.source
        ));
        result
    }

    /// Format for inclusion in prompts (without markdown)
    pub fn to_prompt_format(&self) -> String {
        let header = match (&self.filename, self.line_start) {
            (Some(f), Some(l)) => format!("[{} starting at line {}]\n", f, l),
            (Some(f), None) => format!("[{}]\n", f),
            (None, Some(l)) => format!("[{} starting at line {}]\n", self.language.name(), l),
            (None, None) => format!("[{}]\n", self.language.name()),
        };

        format!("{}{}", header, self.source)
    }

    /// Extract a range of lines
    pub fn extract_lines(&self, start: usize, end: usize) -> Self {
        let lines: Vec<&str> = self.source.lines().collect();
        let start_idx = start.saturating_sub(1).min(lines.len());
        let end_idx = end.min(lines.len());

        let source = lines[start_idx..end_idx].join("\n");

        Self {
            source,
            language: self.language,
            filename: self.filename.clone(),
            description: self.description.clone(),
            line_start: Some(start),
        }
    }
}

impl std::fmt::Display for Code {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.to_markdown())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_code_new() {
        let code = Code::new("fn main() {}", Language::Rust);
        assert_eq!(code.source, "fn main() {}");
        assert_eq!(code.language, Language::Rust);
    }

    #[test]
    fn test_code_from_filename() {
        let code = Code::from_filename("print('hello')", "script.py");
        assert_eq!(code.language, Language::Python);
        assert_eq!(code.filename, Some("script.py".to_string()));
    }

    #[test]
    fn test_code_builder() {
        let code = Code::new("SELECT * FROM users", Language::Sql)
            .with_filename("query.sql")
            .with_description("User query")
            .with_line_start(10);

        assert_eq!(code.filename, Some("query.sql".to_string()));
        assert_eq!(code.description, Some("User query".to_string()));
        assert_eq!(code.line_start, Some(10));
    }

    #[test]
    fn test_code_line_count() {
        let code = Code::new("line1\nline2\nline3", Language::Other);
        assert_eq!(code.line_count(), 3);
    }

    #[test]
    fn test_code_to_markdown() {
        let code = Code::new("fn main() {}", Language::Rust).with_filename("main.rs");

        let md = code.to_markdown();
        assert!(md.contains("**main.rs**"));
        assert!(md.contains("```rust"));
        assert!(md.contains("fn main() {}"));
    }

    #[test]
    fn test_code_extract_lines() {
        let code = Code::new("line1\nline2\nline3\nline4\nline5", Language::Other);
        let excerpt = code.extract_lines(2, 4);

        assert_eq!(excerpt.source, "line2\nline3\nline4");
        assert_eq!(excerpt.line_start, Some(2));
    }

    #[test]
    fn test_language_detection() {
        assert_eq!(Language::from_extension("rs"), Some(Language::Rust));
        assert_eq!(Language::from_extension("py"), Some(Language::Python));
        assert_eq!(Language::from_extension("js"), Some(Language::JavaScript));
        assert_eq!(Language::from_extension("ts"), Some(Language::TypeScript));
        assert_eq!(Language::from_extension("go"), Some(Language::Go));
        assert_eq!(Language::from_extension("unknown"), None);
    }

    #[test]
    fn test_language_properties() {
        assert!(Language::Rust.is_systems());
        assert!(!Language::Rust.is_scripting());
        assert!(Language::Python.is_scripting());
        assert!(!Language::Python.is_systems());
    }

    #[test]
    fn test_serialization() {
        let code = Code::new("print('hi')", Language::Python);
        let json = serde_json::to_string(&code).unwrap();
        assert!(json.contains("python"));

        let deserialized: Code = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.language, Language::Python);
    }
}
