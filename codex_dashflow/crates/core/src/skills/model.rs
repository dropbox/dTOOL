//! Data models for skills

use std::path::PathBuf;

/// Metadata for a discovered skill.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkillMetadata {
    /// The skill name (from YAML frontmatter)
    pub name: String,
    /// Short description of what the skill does
    pub description: String,
    /// Full path to the SKILL.md file
    pub path: PathBuf,
}

/// Error information when a skill file fails to load.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkillError {
    /// Path to the problematic file
    pub path: PathBuf,
    /// Human-readable error message
    pub message: String,
}

/// Result of loading skills from a directory.
#[derive(Debug, Clone, Default)]
pub struct SkillLoadOutcome {
    /// Successfully loaded skills
    pub skills: Vec<SkillMetadata>,
    /// Errors encountered during loading
    pub errors: Vec<SkillError>,
}
