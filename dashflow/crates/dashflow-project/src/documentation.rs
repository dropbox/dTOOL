// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)
// DashFlow Project - Documentation Discovery

// Allow clippy warnings for documentation discovery
// - float_cmp: Documentation score comparisons use exact thresholds
// - unwrap_used: Path operations on known valid paths
#![allow(clippy::float_cmp, clippy::unwrap_used)]

//! # Documentation Discovery
//!
//! Types and logic for discovering project documentation files.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Types of documentation files recognized
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DocumentationType {
    /// README files (README, README.md, etc.)
    Readme,
    /// Claude-specific instructions (CLAUDE.md)
    Claude,
    /// Agent instructions (AGENTS.md, .agents/)
    Agents,
    /// Contributing guidelines (CONTRIBUTING.md)
    Contributing,
    /// Changelog (CHANGELOG.md, HISTORY.md)
    Changelog,
    /// License file
    License,
    /// Code of conduct
    CodeOfConduct,
    /// Architecture documentation
    Architecture,
    /// API documentation
    Api,
    /// Security policy
    Security,
    /// Other documentation
    Other,
}

impl DocumentationType {
    /// Detect documentation type from filename
    pub fn from_filename(filename: &str) -> Option<Self> {
        let lower = filename.to_lowercase();
        let stem = Path::new(&lower)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or(&lower);

        match stem {
            "readme" => Some(DocumentationType::Readme),
            "claude" => Some(DocumentationType::Claude),
            "agents" | "agent" => Some(DocumentationType::Agents),
            "contributing" => Some(DocumentationType::Contributing),
            "changelog" | "history" | "changes" => Some(DocumentationType::Changelog),
            "license" | "licence" | "copying" => Some(DocumentationType::License),
            "code_of_conduct" | "code-of-conduct" => Some(DocumentationType::CodeOfConduct),
            "architecture" | "design" => Some(DocumentationType::Architecture),
            "api" => Some(DocumentationType::Api),
            "security" => Some(DocumentationType::Security),
            _ => None,
        }
    }

    /// Get priority for context inclusion (lower = higher priority)
    pub fn priority(&self) -> u32 {
        match self {
            DocumentationType::Claude => 1, // Highest priority - agent instructions
            DocumentationType::Agents => 2, // Agent-specific docs
            DocumentationType::Readme => 3, // Project overview
            DocumentationType::Architecture => 4, // Technical context
            DocumentationType::Api => 5,
            DocumentationType::Contributing => 6,
            DocumentationType::Changelog => 7,
            DocumentationType::Security => 8,
            DocumentationType::License => 9,
            DocumentationType::CodeOfConduct => 10,
            DocumentationType::Other => 11,
        }
    }

    /// Get display name
    pub fn display_name(&self) -> &'static str {
        match self {
            DocumentationType::Readme => "README",
            DocumentationType::Claude => "Claude Instructions",
            DocumentationType::Agents => "Agent Instructions",
            DocumentationType::Contributing => "Contributing Guide",
            DocumentationType::Changelog => "Changelog",
            DocumentationType::License => "License",
            DocumentationType::CodeOfConduct => "Code of Conduct",
            DocumentationType::Architecture => "Architecture",
            DocumentationType::Api => "API Documentation",
            DocumentationType::Security => "Security Policy",
            DocumentationType::Other => "Documentation",
        }
    }
}

/// A discovered documentation file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Documentation {
    /// Type of documentation
    pub doc_type: DocumentationType,
    /// Path to the file
    pub path: PathBuf,
    /// Size in bytes
    pub size: u64,
    /// Content (loaded on demand)
    #[serde(skip)]
    content: Option<String>,
}

impl Documentation {
    /// Create new documentation reference
    pub fn new(doc_type: DocumentationType, path: PathBuf, size: u64) -> Self {
        Self {
            doc_type,
            path,
            size,
            content: None,
        }
    }

    /// Load content from file
    pub fn load_content(&mut self) -> std::io::Result<&str> {
        if self.content.is_none() {
            let content = std::fs::read_to_string(&self.path)?;
            self.content = Some(content);
        }
        Ok(self.content.as_ref().unwrap())
    }

    /// Get content if already loaded
    pub fn content(&self) -> Option<&str> {
        self.content.as_deref()
    }

    /// Get priority for context inclusion
    pub fn priority(&self) -> u32 {
        self.doc_type.priority()
    }
}

/// Known documentation file patterns
#[allow(dead_code)] // Architectural: Constant list for future file discovery filtering
pub const DOC_PATTERNS: &[&str] = &[
    "README*",
    "readme*",
    "CLAUDE.md",
    "claude.md",
    ".claude/commands/*.md",
    "AGENTS.md",
    "agents.md",
    ".agents/*.md",
    "CONTRIBUTING*",
    "contributing*",
    "CHANGELOG*",
    "changelog*",
    "HISTORY*",
    "history*",
    "LICENSE*",
    "license*",
    "LICENCE*",
    "licence*",
    "COPYING*",
    "CODE_OF_CONDUCT*",
    "code_of_conduct*",
    "ARCHITECTURE*",
    "architecture*",
    "DESIGN*",
    "design*",
    "API*",
    "api*",
    "SECURITY*",
    "security*",
    "docs/README*",
    "docs/*.md",
    "doc/README*",
    "doc/*.md",
];

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_doc_type_from_filename() {
        assert_eq!(
            DocumentationType::from_filename("README.md"),
            Some(DocumentationType::Readme)
        );
        assert_eq!(
            DocumentationType::from_filename("CLAUDE.md"),
            Some(DocumentationType::Claude)
        );
        assert_eq!(
            DocumentationType::from_filename("AGENTS.md"),
            Some(DocumentationType::Agents)
        );
        assert_eq!(
            DocumentationType::from_filename("CONTRIBUTING.md"),
            Some(DocumentationType::Contributing)
        );
        assert_eq!(
            DocumentationType::from_filename("LICENSE"),
            Some(DocumentationType::License)
        );
        assert_eq!(DocumentationType::from_filename("random.txt"), None);
    }

    #[test]
    fn test_doc_type_from_filename_case_insensitive() {
        assert_eq!(
            DocumentationType::from_filename("readme"),
            Some(DocumentationType::Readme)
        );
        assert_eq!(
            DocumentationType::from_filename("ReadMe.MD"),
            Some(DocumentationType::Readme)
        );
        assert_eq!(
            DocumentationType::from_filename("claude.md"),
            Some(DocumentationType::Claude)
        );
        assert_eq!(
            DocumentationType::from_filename("AgEnTs.MD"),
            Some(DocumentationType::Agents)
        );
        assert_eq!(
            DocumentationType::from_filename("LICENSE.TXT"),
            Some(DocumentationType::License)
        );
    }

    #[test]
    fn test_doc_type_from_filename_stem_matching() {
        // from_filename matches on file stem, not extension.
        assert_eq!(
            DocumentationType::from_filename("README.txt"),
            Some(DocumentationType::Readme)
        );
        assert_eq!(
            DocumentationType::from_filename("HISTORY.rst"),
            Some(DocumentationType::Changelog)
        );
        assert_eq!(
            DocumentationType::from_filename("CODE_OF_CONDUCT.md"),
            Some(DocumentationType::CodeOfConduct)
        );
        assert_eq!(
            DocumentationType::from_filename("code-of-conduct.md"),
            Some(DocumentationType::CodeOfConduct)
        );
        assert_eq!(
            DocumentationType::from_filename("architecture.md"),
            Some(DocumentationType::Architecture)
        );
        assert_eq!(
            DocumentationType::from_filename("design.md"),
            Some(DocumentationType::Architecture)
        );
        assert_eq!(
            DocumentationType::from_filename("security.md"),
            Some(DocumentationType::Security)
        );
        assert_eq!(
            DocumentationType::from_filename("api.md"),
            Some(DocumentationType::Api)
        );
    }

    #[test]
    fn test_doc_type_from_filename_license_variants() {
        assert_eq!(
            DocumentationType::from_filename("LICENCE"),
            Some(DocumentationType::License)
        );
        assert_eq!(
            DocumentationType::from_filename("COPYING"),
            Some(DocumentationType::License)
        );
    }

    #[test]
    fn test_doc_type_priority() {
        assert!(DocumentationType::Claude.priority() < DocumentationType::Readme.priority());
        assert!(DocumentationType::Agents.priority() < DocumentationType::Readme.priority());
        assert!(DocumentationType::Readme.priority() < DocumentationType::License.priority());
    }

    #[test]
    fn test_doc_type_display_name() {
        assert_eq!(
            DocumentationType::Claude.display_name(),
            "Claude Instructions"
        );
        assert_eq!(DocumentationType::Readme.display_name(), "README");
        assert_eq!(DocumentationType::License.display_name(), "License");
    }

    #[test]
    fn test_documentation_new() {
        let doc = Documentation::new(DocumentationType::Readme, PathBuf::from("README.md"), 1024);
        assert_eq!(doc.doc_type, DocumentationType::Readme);
        assert_eq!(doc.path, PathBuf::from("README.md"));
        assert_eq!(doc.size, 1024);
        assert!(doc.content().is_none());
    }

    #[test]
    fn test_documentation_load_content_and_cache() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join("README.md");
        std::fs::write(&path, "first").unwrap();

        let mut doc = Documentation::new(DocumentationType::Readme, path.clone(), 5);
        assert!(doc.content().is_none());

        let content_1 = doc.load_content().unwrap().to_string();
        assert_eq!(content_1, "first");
        assert_eq!(doc.content(), Some("first"));

        // Modify the file on disk; cached content should remain unchanged.
        std::fs::write(&path, "second").unwrap();
        let content_2 = doc.load_content().unwrap().to_string();
        assert_eq!(content_2, "first");
        assert_eq!(doc.content(), Some("first"));
    }

    #[test]
    fn test_documentation_load_content_missing_file_errors() {
        let temp_dir = TempDir::new().unwrap();
        let missing = temp_dir.path().join("missing.md");
        let mut doc = Documentation::new(DocumentationType::Readme, missing, 0);
        assert!(doc.load_content().is_err());
    }
}
