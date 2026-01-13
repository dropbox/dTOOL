//! Skills discovery and loading
//!
//! This module provides functionality to discover and load skill definitions
//! from a skills directory. Skills are organized as directories containing
//! a `SKILL.md` file with YAML frontmatter for metadata.
//!
//! # Directory Structure
//!
//! ```text
//! ~/.codex-dashflow/skills/
//! ├── code-review/
//! │   └── SKILL.md
//! ├── refactor/
//! │   └── SKILL.md
//! └── test-generation/
//!     └── SKILL.md
//! ```
//!
//! # SKILL.md Format
//!
//! ```markdown
//! ---
//! name: code-review
//! description: Reviews code for quality, bugs, and best practices
//! ---
//!
//! # Code Review Skill
//!
//! This skill helps you review code systematically...
//! ```

mod loader;
mod model;
mod render;

pub use loader::{default_skills_dir, load_skills, load_skills_from};
pub use model::{SkillError, SkillLoadOutcome, SkillMetadata};
pub use render::render_skills_section;
