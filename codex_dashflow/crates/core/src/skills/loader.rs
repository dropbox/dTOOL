//! Skill discovery and loading

use super::model::{SkillError, SkillLoadOutcome, SkillMetadata};
use std::collections::VecDeque;
use std::error::Error;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use tracing::error;

/// The expected skill definition filename.
const SKILLS_FILENAME: &str = "SKILL.md";

/// The name of the skills directory under codex home.
const SKILLS_DIR_NAME: &str = "skills";

/// Maximum allowed length for skill name.
const MAX_NAME_LEN: usize = 100;

/// Maximum allowed length for skill description.
const MAX_DESCRIPTION_LEN: usize = 500;

/// Errors that can occur when parsing a skill file.
#[derive(Debug)]
enum SkillParseError {
    Read(std::io::Error),
    MissingFrontmatter,
    MissingField(&'static str),
    InvalidField { field: &'static str, reason: String },
}

impl fmt::Display for SkillParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SkillParseError::Read(e) => write!(f, "failed to read file: {e}"),
            SkillParseError::MissingFrontmatter => {
                write!(f, "missing YAML frontmatter delimited by ---")
            }
            SkillParseError::MissingField(field) => write!(f, "missing field `{field}`"),
            SkillParseError::InvalidField { field, reason } => {
                write!(f, "invalid {field}: {reason}")
            }
        }
    }
}

impl Error for SkillParseError {}

/// Return the default skills directory.
///
/// Checks `$CODEX_HOME/skills` first, then falls back to `~/.codex-dashflow/skills`.
pub fn default_skills_dir() -> Option<PathBuf> {
    if let Ok(val) = std::env::var("CODEX_HOME") {
        if !val.is_empty() {
            return Some(PathBuf::from(val).join(SKILLS_DIR_NAME));
        }
    }
    dirs::home_dir().map(|home| home.join(".codex-dashflow").join(SKILLS_DIR_NAME))
}

/// Load all skills from the default skills directory.
///
/// Returns a `SkillLoadOutcome` containing both successfully loaded skills
/// and any errors encountered during loading.
pub fn load_skills() -> SkillLoadOutcome {
    let mut outcome = SkillLoadOutcome::default();

    if let Some(root) = default_skills_dir() {
        discover_skills_under_root(&root, &mut outcome);
    }

    // Sort skills by name for consistent ordering
    outcome
        .skills
        .sort_by(|a, b| a.name.cmp(&b.name).then_with(|| a.path.cmp(&b.path)));

    outcome
}

/// Load skills from a specific directory.
///
/// This is useful for testing or when using a custom skills location.
pub fn load_skills_from(root: &Path) -> SkillLoadOutcome {
    let mut outcome = SkillLoadOutcome::default();
    discover_skills_under_root(root, &mut outcome);

    outcome
        .skills
        .sort_by(|a, b| a.name.cmp(&b.name).then_with(|| a.path.cmp(&b.path)));

    outcome
}

/// Recursively discover skills under a root directory.
fn discover_skills_under_root(root: &Path, outcome: &mut SkillLoadOutcome) {
    // Normalize the path if possible (canonicalize resolves symlinks and makes absolute)
    let root = match fs::canonicalize(root) {
        Ok(p) => p,
        Err(_) => return,
    };

    if !root.is_dir() {
        return;
    }

    // BFS through directories
    let mut queue: VecDeque<PathBuf> = VecDeque::from([root]);

    while let Some(dir) = queue.pop_front() {
        let entries = match fs::read_dir(&dir) {
            Ok(entries) => entries,
            Err(e) => {
                error!("failed to read skills dir {}: {e:#}", dir.display());
                continue;
            }
        };

        for entry in entries.flatten() {
            let path = entry.path();
            let file_name = match path.file_name().and_then(|f| f.to_str()) {
                Some(name) => name,
                None => continue,
            };

            // Skip hidden files/directories
            if file_name.starts_with('.') {
                continue;
            }

            let Ok(file_type) = entry.file_type() else {
                continue;
            };

            // Skip symlinks
            if file_type.is_symlink() {
                continue;
            }

            // Queue subdirectories for recursive search
            if file_type.is_dir() {
                queue.push_back(path);
                continue;
            }

            // Process SKILL.md files
            if file_type.is_file() && file_name == SKILLS_FILENAME {
                match parse_skill_file(&path) {
                    Ok(skill) => outcome.skills.push(skill),
                    Err(err) => outcome.errors.push(SkillError {
                        path,
                        message: err.to_string(),
                    }),
                }
            }
        }
    }
}

/// Parse a SKILL.md file and extract metadata.
fn parse_skill_file(path: &Path) -> Result<SkillMetadata, SkillParseError> {
    let contents = fs::read_to_string(path).map_err(SkillParseError::Read)?;

    let (name, description) = parse_frontmatter(&contents)?;

    // Validate field lengths
    validate_field(&name, MAX_NAME_LEN, "name")?;
    validate_field(&description, MAX_DESCRIPTION_LEN, "description")?;

    // Normalize the path
    let resolved_path = fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());

    Ok(SkillMetadata {
        name,
        description,
        path: resolved_path,
    })
}

/// Parse YAML frontmatter from content.
///
/// Returns (name, description) if found, or an error.
fn parse_frontmatter(content: &str) -> Result<(String, String), SkillParseError> {
    let mut segments = content.split_inclusive('\n');
    let Some(first_segment) = segments.next() else {
        return Err(SkillParseError::MissingFrontmatter);
    };

    let first_line = first_segment.trim();
    if first_line != "---" {
        return Err(SkillParseError::MissingFrontmatter);
    }

    let mut name: Option<String> = None;
    let mut description: Option<String> = None;
    let mut frontmatter_closed = false;

    for segment in segments {
        let line = segment.trim_end_matches(['\n', '\r']);
        let trimmed = line.trim();

        if trimmed == "---" {
            frontmatter_closed = true;
            break;
        }

        // Skip comments
        if trimmed.starts_with('#') {
            continue;
        }

        // Parse key: value pairs
        if let Some((key, val)) = trimmed.split_once(':') {
            let key = key.trim().to_lowercase();
            let val = parse_yaml_value(val.trim());

            if !val.is_empty() {
                match key.as_str() {
                    "name" => name = Some(sanitize_single_line(&val)),
                    "description" => description = Some(sanitize_single_line(&val)),
                    _ => {}
                }
            }
        }
    }

    if !frontmatter_closed {
        return Err(SkillParseError::MissingFrontmatter);
    }

    let name = name.ok_or(SkillParseError::MissingField("name"))?;
    let description = description.ok_or(SkillParseError::MissingField("description"))?;

    Ok((name, description))
}

/// Parse a YAML value, handling quoted strings.
fn parse_yaml_value(val: &str) -> String {
    let trimmed = val.trim();

    // Handle quoted strings
    if ((trimmed.starts_with('"') && trimmed.ends_with('"'))
        || (trimmed.starts_with('\'') && trimmed.ends_with('\'')))
        && trimmed.len() >= 2
    {
        return trimmed[1..trimmed.len() - 1].to_string();
    }

    // Handle multiline indicator (|-) - just return empty, we'll get it from next lines
    if trimmed == "|-" || trimmed == "|" {
        return String::new();
    }

    trimmed.to_string()
}

/// Collapse all whitespace in a string to single spaces.
fn sanitize_single_line(raw: &str) -> String {
    raw.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Validate a field is non-empty and within length limits.
fn validate_field(
    value: &str,
    max_len: usize,
    field_name: &'static str,
) -> Result<(), SkillParseError> {
    if value.is_empty() {
        return Err(SkillParseError::MissingField(field_name));
    }
    if value.len() > max_len {
        return Err(SkillParseError::InvalidField {
            field: field_name,
            reason: format!("exceeds maximum length of {max_len} characters"),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn write_skill(dir: &TempDir, subdir: &str, name: &str, description: &str) -> PathBuf {
        let skill_dir = dir.path().join(subdir);
        fs::create_dir_all(&skill_dir).unwrap();
        // Use simple YAML format
        let content = format!("---\nname: {name}\ndescription: \"{description}\"\n---\n\n# Body\n");
        let path = skill_dir.join(SKILLS_FILENAME);
        fs::write(&path, content).unwrap();
        path
    }

    #[test]
    fn test_loads_valid_skill() {
        let temp_dir = TempDir::new().expect("tempdir");
        write_skill(&temp_dir, "demo", "demo-skill", "does things carefully");

        let outcome = load_skills_from(temp_dir.path());

        assert!(
            outcome.errors.is_empty(),
            "unexpected errors: {:?}",
            outcome.errors
        );
        assert_eq!(outcome.skills.len(), 1);

        let skill = &outcome.skills[0];
        assert_eq!(skill.name, "demo-skill");
        assert_eq!(skill.description, "does things carefully");

        let path_str = skill.path.to_string_lossy().replace('\\', "/");
        assert!(
            path_str.ends_with("demo/SKILL.md"),
            "unexpected path {path_str}"
        );
    }

    #[test]
    fn test_skips_hidden_directories() {
        let temp_dir = TempDir::new().expect("tempdir");

        // Create a hidden directory with a skill
        let hidden_dir = temp_dir.path().join(".hidden");
        fs::create_dir_all(&hidden_dir).unwrap();
        fs::write(
            hidden_dir.join(SKILLS_FILENAME),
            "---\nname: hidden\ndescription: hidden\n---\n",
        )
        .unwrap();

        let outcome = load_skills_from(temp_dir.path());

        assert_eq!(outcome.skills.len(), 0);
        assert!(outcome.errors.is_empty());
    }

    #[test]
    fn test_reports_invalid_frontmatter() {
        let temp_dir = TempDir::new().expect("tempdir");
        let invalid_dir = temp_dir.path().join("invalid");
        fs::create_dir_all(&invalid_dir).unwrap();
        fs::write(invalid_dir.join(SKILLS_FILENAME), "---\nname: bad").unwrap();

        let outcome = load_skills_from(temp_dir.path());

        assert_eq!(outcome.skills.len(), 0);
        assert_eq!(outcome.errors.len(), 1);
        assert!(
            outcome.errors[0]
                .message
                .contains("missing YAML frontmatter"),
            "expected frontmatter error, got: {}",
            outcome.errors[0].message
        );
    }

    #[test]
    fn test_enforces_description_length_limit() {
        let temp_dir = TempDir::new().expect("tempdir");
        let long_desc = "a".repeat(MAX_DESCRIPTION_LEN + 1);
        write_skill(&temp_dir, "too-long", "toolong", &long_desc);

        let outcome = load_skills_from(temp_dir.path());

        assert_eq!(outcome.skills.len(), 0);
        assert_eq!(outcome.errors.len(), 1);
        assert!(
            outcome.errors[0].message.contains("invalid description"),
            "expected length error, got: {}",
            outcome.errors[0].message
        );
    }

    #[test]
    fn test_enforces_name_length_limit() {
        let temp_dir = TempDir::new().expect("tempdir");
        let long_name = "a".repeat(MAX_NAME_LEN + 1);
        write_skill(&temp_dir, "long-name", &long_name, "short desc");

        let outcome = load_skills_from(temp_dir.path());

        assert_eq!(outcome.skills.len(), 0);
        assert_eq!(outcome.errors.len(), 1);
        assert!(
            outcome.errors[0].message.contains("invalid name"),
            "expected length error, got: {}",
            outcome.errors[0].message
        );
    }

    #[test]
    fn test_missing_name_field() {
        let temp_dir = TempDir::new().expect("tempdir");
        let skill_dir = temp_dir.path().join("no-name");
        fs::create_dir_all(&skill_dir).unwrap();
        fs::write(
            skill_dir.join(SKILLS_FILENAME),
            "---\ndescription: has no name\n---\n",
        )
        .unwrap();

        let outcome = load_skills_from(temp_dir.path());

        assert_eq!(outcome.skills.len(), 0);
        assert_eq!(outcome.errors.len(), 1);
        assert!(
            outcome.errors[0].message.contains("missing")
                && outcome.errors[0].message.contains("name"),
            "expected missing name error, got: {}",
            outcome.errors[0].message
        );
    }

    #[test]
    fn test_discovers_nested_skills() {
        let temp_dir = TempDir::new().expect("tempdir");
        write_skill(&temp_dir, "level1", "skill-1", "top level");
        write_skill(&temp_dir, "level1/level2", "skill-2", "nested");
        write_skill(&temp_dir, "other", "skill-3", "another");

        let outcome = load_skills_from(temp_dir.path());

        assert!(
            outcome.errors.is_empty(),
            "unexpected errors: {:?}",
            outcome.errors
        );
        assert_eq!(outcome.skills.len(), 3);

        // Should be sorted by name
        assert_eq!(outcome.skills[0].name, "skill-1");
        assert_eq!(outcome.skills[1].name, "skill-2");
        assert_eq!(outcome.skills[2].name, "skill-3");
    }

    #[test]
    fn test_empty_directory() {
        let temp_dir = TempDir::new().expect("tempdir");

        let outcome = load_skills_from(temp_dir.path());

        assert!(outcome.skills.is_empty());
        assert!(outcome.errors.is_empty());
    }

    #[test]
    fn test_nonexistent_directory() {
        let outcome = load_skills_from(Path::new("/nonexistent/path/that/does/not/exist"));

        assert!(outcome.skills.is_empty());
        assert!(outcome.errors.is_empty());
    }

    #[test]
    fn test_sanitize_single_line() {
        assert_eq!(sanitize_single_line("hello world"), "hello world");
        assert_eq!(sanitize_single_line("hello\nworld"), "hello world");
        assert_eq!(
            sanitize_single_line("  multiple   spaces  "),
            "multiple spaces"
        );
        assert_eq!(sanitize_single_line("tab\there"), "tab here");
    }

    #[test]
    fn test_parse_frontmatter_valid() {
        let content = "---\nname: test\ndescription: desc\n---\n\n# Body";
        let result = parse_frontmatter(content);
        assert!(result.is_ok());
        let (name, desc) = result.unwrap();
        assert_eq!(name, "test");
        assert_eq!(desc, "desc");
    }

    #[test]
    fn test_parse_frontmatter_no_opening() {
        let content = "name: test\n---\n\n# Body";
        assert!(parse_frontmatter(content).is_err());
    }

    #[test]
    fn test_parse_frontmatter_no_closing() {
        let content = "---\nname: test\n\n# Body without closing";
        assert!(parse_frontmatter(content).is_err());
    }

    #[test]
    fn test_parse_frontmatter_quoted_values() {
        let content = "---\nname: \"quoted name\"\ndescription: 'single quoted'\n---\nbody";
        let result = parse_frontmatter(content);
        assert!(result.is_ok());
        let (name, desc) = result.unwrap();
        assert_eq!(name, "quoted name");
        assert_eq!(desc, "single quoted");
    }

    #[test]
    fn test_parse_yaml_value() {
        assert_eq!(parse_yaml_value("simple"), "simple");
        assert_eq!(parse_yaml_value("\"quoted\""), "quoted");
        assert_eq!(parse_yaml_value("'single'"), "single");
        assert_eq!(parse_yaml_value("  spaced  "), "spaced");
    }

    #[test]
    fn test_default_skills_dir_returns_path() {
        // This test just verifies the function doesn't panic
        // Actual path depends on environment
        let _dir = default_skills_dir();
    }
}
