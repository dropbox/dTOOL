//! Academic document format loaders.
//!
//! This module provides loaders for academic and documentation formats:
//! - BibTeX (.bib) - Bibliography and citation files
//! - LaTeX (.tex) - LaTeX document files
//! - Texinfo (.texi, .texinfo) - GNU documentation format

use async_trait::async_trait;
use std::path::{Path, PathBuf};

use crate::core::documents::{Blob, Document, DocumentLoader};
use crate::core::error::Result;

/// The `BibTeXLoader` reads .bib files and extracts citation entries.
///
/// # Example
///
/// ```no_run
/// use dashflow::core::document_loaders::BibTeXLoader;
/// use dashflow::core::documents::DocumentLoader;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let loader = BibTeXLoader::new("references.bib");
/// let documents = loader.load().await?;
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone)]
pub struct BibTeXLoader {
    /// Path to the BibTeX file
    pub file_path: PathBuf,
    /// Create separate documents per entry (default: false, concatenate all)
    pub separate_entries: bool,
}

impl BibTeXLoader {
    /// Create a new `BibTeXLoader` for the given file path.
    pub fn new(file_path: impl AsRef<Path>) -> Self {
        Self {
            file_path: file_path.as_ref().to_path_buf(),
            separate_entries: false,
        }
    }

    /// Create separate documents per BibTeX entry.
    #[must_use]
    pub fn with_separate_entries(mut self, separate: bool) -> Self {
        self.separate_entries = separate;
        self
    }
}

#[async_trait]
impl DocumentLoader for BibTeXLoader {
    async fn load(&self) -> Result<Vec<Document>> {
        let blob = Blob::from_path(&self.file_path);
        let content = blob.as_string()?;

        let mut documents = Vec::new();
        let mut all_content = String::new();
        let mut entry_count = 0;

        // Simple BibTeX parser - entries start with @ and end with }
        let mut current_entry = String::new();
        let mut in_entry = false;
        let mut brace_count = 0;

        for line in content.lines() {
            let trimmed = line.trim();

            // Start of entry
            if trimmed.starts_with('@') && !in_entry {
                in_entry = true;
                current_entry.clear();
                current_entry.push_str(line);
                current_entry.push('\n');
                brace_count = line.matches('{').count() as i32 - line.matches('}').count() as i32;
            } else if in_entry {
                current_entry.push_str(line);
                current_entry.push('\n');
                brace_count += line.matches('{').count() as i32 - line.matches('}').count() as i32;

                // End of entry
                if brace_count <= 0 {
                    in_entry = false;
                    entry_count += 1;

                    if self.separate_entries {
                        // Extract entry type and key
                        let entry_type = current_entry
                            .split('{')
                            .next()
                            .and_then(|s| s.strip_prefix('@'))
                            .unwrap_or("unknown")
                            .trim()
                            .to_lowercase();

                        let entry_key = current_entry
                            .split('{')
                            .nth(1)
                            .and_then(|s| s.split(',').next())
                            .unwrap_or("unknown")
                            .trim()
                            .to_string();

                        let doc = Document::new(current_entry.clone())
                            .with_metadata("source", self.file_path.display().to_string())
                            .with_metadata("entry_type", entry_type)
                            .with_metadata("entry_key", entry_key)
                            .with_metadata("entry_index", entry_count - 1);

                        documents.push(doc);
                    } else {
                        all_content.push_str(&current_entry);
                        all_content.push('\n');
                    }
                }
            }
        }

        if !self.separate_entries {
            let doc = Document::new(all_content)
                .with_metadata("source", self.file_path.display().to_string())
                .with_metadata("format", "bibtex")
                .with_metadata("entry_count", entry_count);

            documents.push(doc);
        }

        Ok(documents)
    }
}

/// Loads LaTeX document files (.tex).
///
/// The `LaTeXLoader` reads LaTeX source files, preserving all markup and commands.
/// Can optionally separate by chapter or section commands.
///
/// # Example
///
/// ```no_run
/// use dashflow::core::document_loaders::LaTeXLoader;
/// use dashflow::core::documents::DocumentLoader;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let loader = LaTeXLoader::new("document.tex");
/// let documents = loader.load().await?;
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone)]
pub struct LaTeXLoader {
    /// Path to the LaTeX file
    pub file_path: PathBuf,
    /// Separate documents per chapter/section (default: false)
    pub separate_sections: bool,
}

impl LaTeXLoader {
    /// Create a new `LaTeXLoader` for the given file path.
    pub fn new(file_path: impl AsRef<Path>) -> Self {
        Self {
            file_path: file_path.as_ref().to_path_buf(),
            separate_sections: false,
        }
    }

    /// Create separate documents per LaTeX section (\\chapter, \\section).
    #[must_use]
    pub fn with_separate_sections(mut self, separate: bool) -> Self {
        self.separate_sections = separate;
        self
    }
}

#[async_trait]
impl DocumentLoader for LaTeXLoader {
    async fn load(&self) -> Result<Vec<Document>> {
        let blob = Blob::from_path(&self.file_path);
        let content = blob.as_string()?;

        if self.separate_sections {
            // Split by section commands
            let mut documents = Vec::new();
            let mut current_section = String::new();
            let mut section_title = String::new();
            let mut section_index = 0;

            for line in content.lines() {
                let trimmed = line.trim();

                // Detect section commands
                if trimmed.starts_with("\\chapter{") || trimmed.starts_with("\\section{") {
                    // Save previous section
                    if !current_section.is_empty() {
                        let doc = Document::new(current_section.clone())
                            .with_metadata("source", self.file_path.display().to_string())
                            .with_metadata("section_index", section_index)
                            .with_metadata("section_title", section_title.clone())
                            .with_metadata("format", "latex");

                        documents.push(doc);
                        current_section.clear();
                        section_index += 1;
                    }

                    // Extract section title
                    if let Some(start) = trimmed.find('{') {
                        if let Some(end) = trimmed.find('}') {
                            section_title = trimmed[start + 1..end].to_string();
                        }
                    }
                }

                current_section.push_str(line);
                current_section.push('\n');
            }

            // Add last section
            if !current_section.is_empty() {
                let doc = Document::new(current_section)
                    .with_metadata("source", self.file_path.display().to_string())
                    .with_metadata("section_index", section_index)
                    .with_metadata("section_title", section_title)
                    .with_metadata("format", "latex");

                documents.push(doc);
            }

            Ok(documents)
        } else {
            // Load as single document
            let doc = Document::new(content)
                .with_metadata("source", self.file_path.display().to_string())
                .with_metadata("format", "latex");

            Ok(vec![doc])
        }
    }
}

/// Loader for Texinfo files (.texi, .texinfo).
///
/// Texinfo is the official documentation format of the GNU Project.
/// Created in 1986 by Richard Stallman based on Brian Reid's Scribe.
/// Can generate multiple output formats from single source (Info, HTML, PDF, DVI).
/// Used for GNU software documentation (GCC, Emacs, Bash, etc).
/// Combines markup with structural elements (@node, @chapter, @section).
///
/// # Example
///
/// ```rust,no_run
/// # use dashflow::core::document_loaders::TexinfoLoader;
/// # use dashflow::core::documents::DocumentLoader;
/// # async fn example() -> dashflow::core::error::Result<()> {
/// let loader = TexinfoLoader::new("manual.texi");
/// let docs = loader.load().await?;
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone)]
pub struct TexinfoLoader {
    file_path: PathBuf,
    separate_nodes: bool,
}

impl TexinfoLoader {
    /// Create a new Texinfo loader for the given file path.
    pub fn new<P: AsRef<Path>>(path: P) -> Self {
        Self {
            file_path: path.as_ref().to_path_buf(),
            separate_nodes: false,
        }
    }

    /// Enable separation by nodes (@node directive).
    #[must_use]
    pub fn with_separate_nodes(mut self) -> Self {
        self.separate_nodes = true;
        self
    }

    /// Check if line is a Texinfo @node directive
    fn is_node_directive(line: &str) -> Option<String> {
        let trimmed = line.trim();
        if !trimmed.starts_with("@node ") {
            return None;
        }

        // Extract node name (after @node and before comma or newline)
        let node_part = &trimmed[6..]; // Skip "@node "
        let node_name = node_part
            .split(',')
            .next()
            .unwrap_or(node_part)
            .trim()
            .to_string();

        if node_name.is_empty() {
            return None;
        }

        Some(node_name)
    }
}

#[async_trait]
impl DocumentLoader for TexinfoLoader {
    async fn load(&self) -> Result<Vec<Document>> {
        let blob = Blob::from_path(&self.file_path);
        let content = blob.as_string()?;

        if !self.separate_nodes {
            return Ok(vec![Document::new(&content)
                .with_metadata("source", self.file_path.display().to_string())
                .with_metadata("format", "texinfo")]);
        }

        // Separate by @node directives
        let lines: Vec<&str> = content.lines().collect();
        let mut documents = Vec::new();
        let mut current_node = Vec::new();
        let mut current_name = String::new();

        for line in lines {
            if let Some(node_name) = Self::is_node_directive(line) {
                // Save previous node
                if !current_node.is_empty() {
                    let node_content = current_node.join("\n");
                    documents.push(
                        Document::new(&node_content)
                            .with_metadata("source", self.file_path.display().to_string())
                            .with_metadata("format", "texinfo")
                            .with_metadata("node_index", documents.len().to_string())
                            .with_metadata("node_name", current_name.clone()),
                    );
                    current_node.clear();
                }

                // Start new node
                current_name = node_name;
                current_node.push(line);
            } else {
                current_node.push(line);
            }
        }

        // Save last node
        if !current_node.is_empty() {
            let node_content = current_node.join("\n");
            documents.push(
                Document::new(&node_content)
                    .with_metadata("source", self.file_path.display().to_string())
                    .with_metadata("format", "texinfo")
                    .with_metadata("node_index", documents.len().to_string())
                    .with_metadata("node_name", current_name),
            );
        }

        if documents.is_empty() {
            Ok(vec![Document::new(&content)
                .with_metadata("source", self.file_path.display().to_string())
                .with_metadata("format", "texinfo")])
        } else {
            Ok(documents)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    // ==========================================================================
    // BibTeXLoader Tests
    // ==========================================================================

    #[test]
    fn test_bibtex_loader_new() {
        let loader = BibTeXLoader::new("test.bib");
        assert_eq!(loader.file_path, PathBuf::from("test.bib"));
        assert!(!loader.separate_entries);
    }

    #[test]
    fn test_bibtex_loader_with_separate_entries() {
        let loader = BibTeXLoader::new("test.bib").with_separate_entries(true);
        assert!(loader.separate_entries);

        let loader2 = BibTeXLoader::new("test.bib").with_separate_entries(false);
        assert!(!loader2.separate_entries);
    }

    #[test]
    fn test_bibtex_loader_clone() {
        let loader = BibTeXLoader::new("test.bib").with_separate_entries(true);
        let cloned = loader.clone();
        assert_eq!(cloned.file_path, loader.file_path);
        assert_eq!(cloned.separate_entries, loader.separate_entries);
    }

    #[test]
    fn test_bibtex_loader_debug() {
        let loader = BibTeXLoader::new("test.bib");
        let debug_str = format!("{:?}", loader);
        assert!(debug_str.contains("BibTeXLoader"));
        assert!(debug_str.contains("test.bib"));
    }

    #[tokio::test]
    async fn test_bibtex_loader_single_entry() {
        let mut file = NamedTempFile::new().unwrap();
        let content = r#"@article{smith2020,
  author = {John Smith},
  title = {A Great Paper},
  journal = {Science},
  year = {2020}
}
"#;
        file.write_all(content.as_bytes()).unwrap();

        let loader = BibTeXLoader::new(file.path());
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1);
        assert!(docs[0].page_content.contains("@article"));
        assert!(docs[0].page_content.contains("smith2020"));
        assert_eq!(docs[0].metadata.get("format").unwrap(), "bibtex");
        assert_eq!(docs[0].metadata.get("entry_count").unwrap(), &1);
    }

    #[tokio::test]
    async fn test_bibtex_loader_multiple_entries() {
        let mut file = NamedTempFile::new().unwrap();
        let content = r#"@article{smith2020,
  author = {John Smith},
  title = {Paper One},
  year = {2020}
}

@book{doe2021,
  author = {Jane Doe},
  title = {A Book Title},
  publisher = {Publisher},
  year = {2021}
}

@inproceedings{lee2022,
  author = {Bob Lee},
  title = {Conference Paper},
  booktitle = {Conference},
  year = {2022}
}
"#;
        file.write_all(content.as_bytes()).unwrap();

        let loader = BibTeXLoader::new(file.path());
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1);
        assert_eq!(docs[0].metadata.get("entry_count").unwrap(), &3);
        assert!(docs[0].page_content.contains("smith2020"));
        assert!(docs[0].page_content.contains("doe2021"));
        assert!(docs[0].page_content.contains("lee2022"));
    }

    #[tokio::test]
    async fn test_bibtex_loader_separate_entries() {
        let mut file = NamedTempFile::new().unwrap();
        let content = r#"@article{smith2020,
  author = {John Smith},
  title = {Paper One},
  year = {2020}
}

@book{doe2021,
  author = {Jane Doe},
  title = {A Book Title},
  year = {2021}
}
"#;
        file.write_all(content.as_bytes()).unwrap();

        let loader = BibTeXLoader::new(file.path()).with_separate_entries(true);
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 2);

        // First entry
        assert_eq!(docs[0].metadata.get("entry_type").unwrap(), "article");
        assert_eq!(docs[0].metadata.get("entry_key").unwrap(), "smith2020");
        assert_eq!(docs[0].metadata.get("entry_index").unwrap(), &0);
        assert!(docs[0].page_content.contains("John Smith"));

        // Second entry
        assert_eq!(docs[1].metadata.get("entry_type").unwrap(), "book");
        assert_eq!(docs[1].metadata.get("entry_key").unwrap(), "doe2021");
        assert_eq!(docs[1].metadata.get("entry_index").unwrap(), &1);
        assert!(docs[1].page_content.contains("Jane Doe"));
    }

    #[tokio::test]
    async fn test_bibtex_loader_nested_braces() {
        let mut file = NamedTempFile::new().unwrap();
        let content = r#"@article{test2020,
  author = {John {Smith} Jr.},
  title = {{A Title with {Nested} Braces}},
  year = {2020}
}
"#;
        file.write_all(content.as_bytes()).unwrap();

        let loader = BibTeXLoader::new(file.path()).with_separate_entries(true);
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1);
        assert!(docs[0].page_content.contains("John {Smith} Jr."));
        assert!(docs[0].page_content.contains("{A Title with {Nested} Braces}"));
    }

    #[tokio::test]
    async fn test_bibtex_loader_empty_file() {
        let mut file = NamedTempFile::new().unwrap();
        file.write_all(b"").unwrap();

        let loader = BibTeXLoader::new(file.path());
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1);
        assert!(docs[0].page_content.is_empty());
        assert_eq!(docs[0].metadata.get("entry_count").unwrap(), &0);
    }

    #[tokio::test]
    async fn test_bibtex_loader_comments_and_whitespace() {
        let mut file = NamedTempFile::new().unwrap();
        let content = r#"% This is a comment

@article{test2020,
  author = {John Smith},
  title = {Title},
  year = {2020}
}

% Another comment
"#;
        file.write_all(content.as_bytes()).unwrap();

        let loader = BibTeXLoader::new(file.path());
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1);
        assert_eq!(docs[0].metadata.get("entry_count").unwrap(), &1);
    }

    #[tokio::test]
    async fn test_bibtex_loader_file_not_found() {
        let loader = BibTeXLoader::new("/nonexistent/path/file.bib");
        let result = loader.load().await;
        assert!(result.is_err());
    }

    // ==========================================================================
    // LaTeXLoader Tests
    // ==========================================================================

    #[test]
    fn test_latex_loader_new() {
        let loader = LaTeXLoader::new("document.tex");
        assert_eq!(loader.file_path, PathBuf::from("document.tex"));
        assert!(!loader.separate_sections);
    }

    #[test]
    fn test_latex_loader_with_separate_sections() {
        let loader = LaTeXLoader::new("document.tex").with_separate_sections(true);
        assert!(loader.separate_sections);
    }

    #[test]
    fn test_latex_loader_clone() {
        let loader = LaTeXLoader::new("test.tex").with_separate_sections(true);
        let cloned = loader.clone();
        assert_eq!(cloned.file_path, loader.file_path);
        assert_eq!(cloned.separate_sections, loader.separate_sections);
    }

    #[test]
    fn test_latex_loader_debug() {
        let loader = LaTeXLoader::new("test.tex");
        let debug_str = format!("{:?}", loader);
        assert!(debug_str.contains("LaTeXLoader"));
    }

    #[tokio::test]
    async fn test_latex_loader_simple_document() {
        let mut file = NamedTempFile::new().unwrap();
        let content = r#"\documentclass{article}
\begin{document}
Hello, World!
\end{document}
"#;
        file.write_all(content.as_bytes()).unwrap();

        let loader = LaTeXLoader::new(file.path());
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1);
        assert!(docs[0].page_content.contains("\\documentclass{article}"));
        assert!(docs[0].page_content.contains("Hello, World!"));
        assert_eq!(docs[0].metadata.get("format").unwrap(), "latex");
    }

    #[tokio::test]
    async fn test_latex_loader_with_sections() {
        let mut file = NamedTempFile::new().unwrap();
        // Note: LaTeX loader looks for \\section{ at start of trimmed line
        let content = r#"\documentclass{article}
\begin{document}
\section{Introduction}
This is the introduction.
\section{Methods}
This is the methods section.
\end{document}
"#;
        file.write_all(content.as_bytes()).unwrap();

        let loader = LaTeXLoader::new(file.path()).with_separate_sections(true);
        let docs = loader.load().await.unwrap();

        // Should have: preamble up to first section, then section 1, section 2, and remaining
        assert!(docs.len() >= 2);
        assert_eq!(docs[0].metadata.get("format").unwrap(), "latex");
    }

    #[tokio::test]
    async fn test_latex_loader_with_chapters() {
        let mut file = NamedTempFile::new().unwrap();
        let content = r#"\documentclass{book}
\begin{document}

\chapter{First Chapter}
Content of first chapter.

\chapter{Second Chapter}
Content of second chapter.

\end{document}
"#;
        file.write_all(content.as_bytes()).unwrap();

        let loader = LaTeXLoader::new(file.path()).with_separate_sections(true);
        let docs = loader.load().await.unwrap();

        assert!(docs.len() >= 2);
    }

    #[tokio::test]
    async fn test_latex_loader_no_sections() {
        let mut file = NamedTempFile::new().unwrap();
        let content = r#"\documentclass{article}
\begin{document}
Just plain content without sections.
\end{document}
"#;
        file.write_all(content.as_bytes()).unwrap();

        let loader = LaTeXLoader::new(file.path()).with_separate_sections(true);
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1);
    }

    #[tokio::test]
    async fn test_latex_loader_empty_file() {
        let mut file = NamedTempFile::new().unwrap();
        file.write_all(b"").unwrap();

        let loader = LaTeXLoader::new(file.path());
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1);
        assert!(docs[0].page_content.is_empty());
    }

    #[tokio::test]
    async fn test_latex_loader_file_not_found() {
        let loader = LaTeXLoader::new("/nonexistent/path/document.tex");
        let result = loader.load().await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_latex_loader_math_content() {
        let mut file = NamedTempFile::new().unwrap();
        let content = r#"\documentclass{article}
\begin{document}
The equation is $E = mc^2$.

\begin{equation}
\int_0^1 x^2 dx = \frac{1}{3}
\end{equation}
\end{document}
"#;
        file.write_all(content.as_bytes()).unwrap();

        let loader = LaTeXLoader::new(file.path());
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1);
        assert!(docs[0].page_content.contains("E = mc^2"));
        assert!(docs[0].page_content.contains("\\int_0^1"));
    }

    // ==========================================================================
    // TexinfoLoader Tests
    // ==========================================================================

    #[test]
    fn test_texinfo_loader_new() {
        let loader = TexinfoLoader::new("manual.texi");
        assert_eq!(loader.file_path, PathBuf::from("manual.texi"));
        assert!(!loader.separate_nodes);
    }

    #[test]
    fn test_texinfo_loader_with_separate_nodes() {
        let loader = TexinfoLoader::new("manual.texi").with_separate_nodes();
        assert!(loader.separate_nodes);
    }

    #[test]
    fn test_texinfo_loader_clone() {
        let loader = TexinfoLoader::new("test.texi").with_separate_nodes();
        let cloned = loader.clone();
        assert_eq!(cloned.file_path, loader.file_path);
        assert_eq!(cloned.separate_nodes, loader.separate_nodes);
    }

    #[test]
    fn test_texinfo_loader_debug() {
        let loader = TexinfoLoader::new("test.texi");
        let debug_str = format!("{:?}", loader);
        assert!(debug_str.contains("TexinfoLoader"));
    }

    #[test]
    fn test_is_node_directive_valid() {
        assert_eq!(
            TexinfoLoader::is_node_directive("@node Top"),
            Some("Top".to_string())
        );
        assert_eq!(
            TexinfoLoader::is_node_directive("@node Introduction, Next, Prev, Up"),
            Some("Introduction".to_string())
        );
        assert_eq!(
            TexinfoLoader::is_node_directive("  @node Trimmed  "),
            Some("Trimmed".to_string())
        );
    }

    #[test]
    fn test_is_node_directive_invalid() {
        assert_eq!(TexinfoLoader::is_node_directive("@chapter Top"), None);
        assert_eq!(TexinfoLoader::is_node_directive("node Top"), None);
        assert_eq!(TexinfoLoader::is_node_directive("@node "), None);
        assert_eq!(TexinfoLoader::is_node_directive("@@node Top"), None);
        assert_eq!(TexinfoLoader::is_node_directive(""), None);
    }

    #[tokio::test]
    async fn test_texinfo_loader_simple_document() {
        let mut file = NamedTempFile::new().unwrap();
        let content = r#"\input texinfo
@settitle My Manual

This is the manual content.

@bye
"#;
        file.write_all(content.as_bytes()).unwrap();

        let loader = TexinfoLoader::new(file.path());
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1);
        assert!(docs[0].page_content.contains("My Manual"));
        assert_eq!(docs[0].metadata.get("format").unwrap(), "texinfo");
    }

    #[tokio::test]
    async fn test_texinfo_loader_with_nodes() {
        let mut file = NamedTempFile::new().unwrap();
        // Note: @node directive is detected by is_node_directive(), which checks trimmed line starts with "@node "
        let content = "@node Top\nThis is the top node.\n\n@node Introduction\nThis introduces the manual.\n";
        file.write_all(content.as_bytes()).unwrap();

        let loader = TexinfoLoader::new(file.path()).with_separate_nodes();
        let docs = loader.load().await.unwrap();

        // Should have 2 node documents (Top and Introduction)
        assert_eq!(docs.len(), 2);
        assert_eq!(docs[0].metadata.get("format").unwrap(), "texinfo");
        // All docs should have node_name metadata
        for doc in &docs {
            assert!(doc.metadata.get("node_name").is_some());
        }
    }

    #[tokio::test]
    async fn test_texinfo_loader_node_with_navigation() {
        let mut file = NamedTempFile::new().unwrap();
        let content = r#"@node Chapter1, Chapter2, Top, Top
@chapter First Chapter

Content of first chapter.

@node Chapter2, , Chapter1, Top
@chapter Second Chapter

Content of second chapter.
"#;
        file.write_all(content.as_bytes()).unwrap();

        let loader = TexinfoLoader::new(file.path()).with_separate_nodes();
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 2);
        assert_eq!(docs[0].metadata.get("node_name").unwrap(), "Chapter1");
        assert_eq!(docs[1].metadata.get("node_name").unwrap(), "Chapter2");
    }

    #[tokio::test]
    async fn test_texinfo_loader_no_nodes_separate_mode() {
        let mut file = NamedTempFile::new().unwrap();
        let content = r#"\input texinfo
@settitle Simple Manual
Simple content with no nodes.
@bye
"#;
        file.write_all(content.as_bytes()).unwrap();

        let loader = TexinfoLoader::new(file.path()).with_separate_nodes();
        let docs = loader.load().await.unwrap();

        // Should return entire content as one document
        assert_eq!(docs.len(), 1);
        assert!(docs[0].page_content.contains("Simple content"));
    }

    #[tokio::test]
    async fn test_texinfo_loader_empty_file() {
        let mut file = NamedTempFile::new().unwrap();
        file.write_all(b"").unwrap();

        let loader = TexinfoLoader::new(file.path());
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1);
        assert!(docs[0].page_content.is_empty());
    }

    #[tokio::test]
    async fn test_texinfo_loader_file_not_found() {
        let loader = TexinfoLoader::new("/nonexistent/path/manual.texi");
        let result = loader.load().await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_texinfo_loader_node_index_metadata() {
        let mut file = NamedTempFile::new().unwrap();
        let content = r#"@node First
First content.

@node Second
Second content.

@node Third
Third content.
"#;
        file.write_all(content.as_bytes()).unwrap();

        let loader = TexinfoLoader::new(file.path()).with_separate_nodes();
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 3);
        assert_eq!(docs[0].metadata.get("node_index").unwrap(), "0");
        assert_eq!(docs[1].metadata.get("node_index").unwrap(), "1");
        assert_eq!(docs[2].metadata.get("node_index").unwrap(), "2");
    }

    #[tokio::test]
    async fn test_texinfo_loader_special_characters() {
        let mut file = NamedTempFile::new().unwrap();
        let content = r#"@node Top
@top Manual

@example
fn main() {
    println!("Hello, World!");
}
@end example

@itemize
@item First item
@item Second item
@end itemize
"#;
        file.write_all(content.as_bytes()).unwrap();

        let loader = TexinfoLoader::new(file.path());
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1);
        assert!(docs[0].page_content.contains("@example"));
        assert!(docs[0].page_content.contains("println!"));
    }

    #[tokio::test]
    async fn test_texinfo_loader_source_metadata() {
        let file = NamedTempFile::new().unwrap();
        let path = file.path().to_path_buf();
        std::fs::write(&path, "@node Test\nContent").unwrap();

        let loader = TexinfoLoader::new(&path);
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1);
        let source = docs[0].metadata.get("source").unwrap().to_string();
        assert!(source.contains(path.file_name().unwrap().to_str().unwrap()));
    }
}
