//! Custom prompt discovery and loading
//!
//! This module provides functionality to discover and load custom prompt templates
//! from a prompts directory. Prompts are Markdown files that can include optional
//! YAML frontmatter for metadata (description, argument hints).
//!
//! # Example
//!
//! A custom prompt file `~/.codex/prompts/review.md`:
//! ```markdown
//! ---
//! description: "Code review assistant"
//! argument-hint: "\[file\] \[priority\]"
//! ---
//! Please review the following code: $1
//! Additional context: $ARGUMENTS
//! ```

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use tokio::fs;

/// Base namespace for custom prompt slash commands (without trailing colon).
/// Example usage forms constructed in code:
/// - Command token after '/': `"{PROMPTS_CMD_PREFIX}:name"`
/// - Full slash prefix: `"/{PROMPTS_CMD_PREFIX}:"`
pub const PROMPTS_CMD_PREFIX: &str = "prompts";

/// A custom prompt template loaded from a Markdown file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CustomPrompt {
    /// The prompt name (derived from filename without extension)
    pub name: String,
    /// Full path to the prompt file
    pub path: PathBuf,
    /// The prompt content (body after frontmatter is stripped)
    pub content: String,
    /// Optional short description shown in UI
    pub description: Option<String>,
    /// Optional hint for arguments (e.g., `[file] [priority]`)
    pub argument_hint: Option<String>,
}

impl CustomPrompt {
    /// Create a new CustomPrompt with the given name and content.
    pub fn new(name: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            path: PathBuf::new(),
            content: content.into(),
            description: None,
            argument_hint: None,
        }
    }

    /// Set the path for this prompt.
    pub fn with_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.path = path.into();
        self
    }

    /// Set the description for this prompt.
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Set the argument hint for this prompt.
    pub fn with_argument_hint(mut self, hint: impl Into<String>) -> Self {
        self.argument_hint = Some(hint.into());
        self
    }
}

/// Return the default prompts directory: `$CODEX_HOME/prompts` or `~/.codex-dashflow/prompts`.
/// If the home directory cannot be resolved, returns `None`.
pub fn default_prompts_dir() -> Option<PathBuf> {
    // Honor the `CODEX_HOME` environment variable when set
    if let Ok(val) = std::env::var("CODEX_HOME") {
        if !val.is_empty() {
            return Some(PathBuf::from(val).join("prompts"));
        }
    }
    // Fall back to ~/.codex-dashflow/prompts
    dirs::home_dir().map(|home| home.join(".codex-dashflow").join("prompts"))
}

/// Discover prompt files in the given directory, returning entries sorted by name.
/// Non-files are ignored. If the directory does not exist or cannot be read, returns empty.
pub async fn discover_prompts_in(dir: &Path) -> Vec<CustomPrompt> {
    discover_prompts_in_excluding(dir, &HashSet::new()).await
}

/// Discover prompt files in the given directory, excluding any with names in `exclude`.
/// Returns entries sorted by name. Non-files are ignored. Missing/unreadable dir yields empty.
pub async fn discover_prompts_in_excluding(
    dir: &Path,
    exclude: &HashSet<String>,
) -> Vec<CustomPrompt> {
    let mut out: Vec<CustomPrompt> = Vec::new();
    let mut entries = match fs::read_dir(dir).await {
        Ok(entries) => entries,
        Err(_) => return out,
    };

    while let Ok(Some(entry)) = entries.next_entry().await {
        let path = entry.path();
        let is_file_like = fs::metadata(&path)
            .await
            .map(|m| m.is_file())
            .unwrap_or(false);
        if !is_file_like {
            continue;
        }
        // Only include Markdown files with a .md extension.
        let is_md = path
            .extension()
            .and_then(|s| s.to_str())
            .map(|ext| ext.eq_ignore_ascii_case("md"))
            .unwrap_or(false);
        if !is_md {
            continue;
        }
        let Some(name) = path
            .file_stem()
            .and_then(|s| s.to_str())
            .map(str::to_string)
        else {
            continue;
        };
        if exclude.contains(&name) {
            continue;
        }
        let content = match fs::read_to_string(&path).await {
            Ok(s) => s,
            Err(_) => continue,
        };
        let (description, argument_hint, body) = parse_frontmatter(&content);
        out.push(CustomPrompt {
            name,
            path,
            content: body,
            description,
            argument_hint,
        });
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

/// Discover prompts from the default prompts directory.
/// Returns an empty vector if the directory doesn't exist or can't be read.
pub async fn discover_default_prompts() -> Vec<CustomPrompt> {
    match default_prompts_dir() {
        Some(dir) => discover_prompts_in(&dir).await,
        None => Vec::new(),
    }
}

/// Discover prompts from the default directory, excluding built-in names.
pub async fn discover_default_prompts_excluding(exclude: &HashSet<String>) -> Vec<CustomPrompt> {
    match default_prompts_dir() {
        Some(dir) => discover_prompts_in_excluding(&dir, exclude).await,
        None => Vec::new(),
    }
}

/// Parse optional YAML-like frontmatter at the beginning of `content`.
/// Supported keys:
/// - `description`: short description shown in the slash popup
/// - `argument-hint` or `argument_hint`: brief hint string shown after the description
///
/// Returns (description, argument_hint, body_without_frontmatter).
fn parse_frontmatter(content: &str) -> (Option<String>, Option<String>, String) {
    let mut segments = content.split_inclusive('\n');
    let Some(first_segment) = segments.next() else {
        return (None, None, String::new());
    };
    let first_line = first_segment.trim_end_matches(['\r', '\n']);
    if first_line.trim() != "---" {
        return (None, None, content.to_string());
    }

    let mut desc: Option<String> = None;
    let mut hint: Option<String> = None;
    let mut frontmatter_closed = false;
    let mut consumed = first_segment.len();

    for segment in segments {
        let line = segment.trim_end_matches(['\r', '\n']);
        let trimmed = line.trim();

        if trimmed == "---" {
            frontmatter_closed = true;
            consumed += segment.len();
            break;
        }

        if trimmed.is_empty() || trimmed.starts_with('#') {
            consumed += segment.len();
            continue;
        }

        if let Some((k, v)) = trimmed.split_once(':') {
            let key = k.trim().to_ascii_lowercase();
            let mut val = v.trim().to_string();
            if val.len() >= 2 {
                let bytes = val.as_bytes();
                let first = bytes[0];
                let last = bytes[bytes.len() - 1];
                if (first == b'\"' && last == b'\"') || (first == b'\'' && last == b'\'') {
                    val = val[1..val.len().saturating_sub(1)].to_string();
                }
            }
            match key.as_str() {
                "description" => desc = Some(val),
                "argument-hint" | "argument_hint" => hint = Some(val),
                _ => {}
            }
        }

        consumed += segment.len();
    }

    if !frontmatter_closed {
        // Unterminated frontmatter: treat input as-is.
        return (None, None, content.to_string());
    }

    let body = if consumed >= content.len() {
        String::new()
    } else {
        content[consumed..].to_string()
    };
    (desc, hint, body)
}

/// Apply argument substitution to a prompt template.
/// Replaces `$1`, `$2`, etc. with positional arguments, and `$ARGUMENTS` with all arguments.
pub fn substitute_arguments(template: &str, arguments: &[&str]) -> String {
    let mut result = template.to_string();

    // Replace $ARGUMENTS with all arguments joined by space
    let all_args = arguments.join(" ");
    result = result.replace("$ARGUMENTS", &all_args);

    // Replace $1, $2, etc. with positional arguments
    for (i, arg) in arguments.iter().enumerate() {
        let placeholder = format!("${}", i + 1);
        result = result.replace(&placeholder, arg);
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[tokio::test]
    async fn empty_when_dir_missing() {
        let tmp = tempdir().expect("create TempDir");
        let missing = tmp.path().join("nope");
        let found = discover_prompts_in(&missing).await;
        assert!(found.is_empty());
    }

    #[tokio::test]
    async fn discovers_and_sorts_files() {
        let tmp = tempdir().expect("create TempDir");
        let dir = tmp.path();
        fs::write(dir.join("b.md"), b"b").unwrap();
        fs::write(dir.join("a.md"), b"a").unwrap();
        fs::create_dir(dir.join("subdir")).unwrap();
        let found = discover_prompts_in(dir).await;
        let names: Vec<String> = found.into_iter().map(|e| e.name).collect();
        assert_eq!(names, vec!["a", "b"]);
    }

    #[tokio::test]
    async fn excludes_specified_names() {
        let tmp = tempdir().expect("create TempDir");
        let dir = tmp.path();
        fs::write(dir.join("init.md"), b"ignored").unwrap();
        fs::write(dir.join("foo.md"), b"ok").unwrap();
        let mut exclude = HashSet::new();
        exclude.insert("init".to_string());
        let found = discover_prompts_in_excluding(dir, &exclude).await;
        let names: Vec<String> = found.into_iter().map(|e| e.name).collect();
        assert_eq!(names, vec!["foo"]);
    }

    #[tokio::test]
    async fn skips_non_utf8_files() {
        let tmp = tempdir().expect("create TempDir");
        let dir = tmp.path();
        // Valid UTF-8 file
        fs::write(dir.join("good.md"), b"hello").unwrap();
        // Invalid UTF-8 content in .md file (e.g., lone 0xFF byte)
        fs::write(dir.join("bad.md"), vec![0xFF, 0xFE, b'\n']).unwrap();
        let found = discover_prompts_in(dir).await;
        let names: Vec<String> = found.into_iter().map(|e| e.name).collect();
        assert_eq!(names, vec!["good"]);
    }

    #[tokio::test]
    async fn skips_non_md_files() {
        let tmp = tempdir().expect("create TempDir");
        let dir = tmp.path();
        fs::write(dir.join("prompt.md"), b"markdown").unwrap();
        fs::write(dir.join("readme.txt"), b"text").unwrap();
        fs::write(dir.join("script.rs"), b"rust").unwrap();
        let found = discover_prompts_in(dir).await;
        let names: Vec<String> = found.into_iter().map(|e| e.name).collect();
        assert_eq!(names, vec!["prompt"]);
    }

    #[tokio::test]
    #[cfg(unix)]
    async fn discovers_symlinked_md_files() {
        let tmp = tempdir().expect("create TempDir");
        let dir = tmp.path();

        // Create a real file
        fs::write(dir.join("real.md"), b"real content").unwrap();

        // Create a symlink to the real file
        std::os::unix::fs::symlink(dir.join("real.md"), dir.join("link.md")).unwrap();

        let found = discover_prompts_in(dir).await;
        let names: Vec<String> = found.into_iter().map(|e| e.name).collect();

        // Both real and link should be discovered, sorted alphabetically
        assert_eq!(names, vec!["link", "real"]);
    }

    #[tokio::test]
    async fn parses_frontmatter_and_strips_from_body() {
        let tmp = tempdir().expect("create TempDir");
        let dir = tmp.path();
        let file = dir.join("withmeta.md");
        let text = "---\nname: ignored\ndescription: \"Quick review command\"\nargument-hint: \"[file] [priority]\"\n---\nActual body with $1 and $ARGUMENTS";
        fs::write(&file, text).unwrap();

        let found = discover_prompts_in(dir).await;
        assert_eq!(found.len(), 1);
        let p = &found[0];
        assert_eq!(p.name, "withmeta");
        assert_eq!(p.description.as_deref(), Some("Quick review command"));
        assert_eq!(p.argument_hint.as_deref(), Some("[file] [priority]"));
        // Body should not include the frontmatter delimiters.
        assert_eq!(p.content, "Actual body with $1 and $ARGUMENTS");
    }

    #[test]
    fn parse_frontmatter_preserves_body_newlines() {
        let content =
            "---\r\ndescription: \"Line endings\"\r\nargument_hint: \"[arg]\"\r\n---\r\nFirst line\r\nSecond line\r\n";
        let (desc, hint, body) = parse_frontmatter(content);
        assert_eq!(desc.as_deref(), Some("Line endings"));
        assert_eq!(hint.as_deref(), Some("[arg]"));
        assert_eq!(body, "First line\r\nSecond line\r\n");
    }

    #[test]
    fn parse_frontmatter_no_frontmatter() {
        let content = "Just plain content\nNo frontmatter here";
        let (desc, hint, body) = parse_frontmatter(content);
        assert!(desc.is_none());
        assert!(hint.is_none());
        assert_eq!(body, content);
    }

    #[test]
    fn parse_frontmatter_unterminated() {
        let content = "---\ndescription: \"Incomplete\"\nNo closing delimiter";
        let (desc, hint, body) = parse_frontmatter(content);
        // Unterminated frontmatter should return content as-is
        assert!(desc.is_none());
        assert!(hint.is_none());
        assert_eq!(body, content);
    }

    #[test]
    fn parse_frontmatter_empty_body() {
        let content = "---\ndescription: \"All metadata\"\n---\n";
        let (desc, hint, body) = parse_frontmatter(content);
        assert_eq!(desc.as_deref(), Some("All metadata"));
        assert!(hint.is_none());
        assert_eq!(body, "");
    }

    #[test]
    fn parse_frontmatter_single_quoted_values() {
        let content = "---\ndescription: 'Single quoted'\nargument-hint: 'hint'\n---\nbody";
        let (desc, hint, _body) = parse_frontmatter(content);
        assert_eq!(desc.as_deref(), Some("Single quoted"));
        assert_eq!(hint.as_deref(), Some("hint"));
    }

    #[test]
    fn parse_frontmatter_comments_ignored() {
        let content =
            "---\n# This is a comment\ndescription: \"Actual value\"\n# Another comment\n---\nbody";
        let (desc, hint, body) = parse_frontmatter(content);
        assert_eq!(desc.as_deref(), Some("Actual value"));
        assert!(hint.is_none());
        assert_eq!(body, "body");
    }

    #[test]
    fn substitute_arguments_positional() {
        let template = "Review $1 with priority $2";
        let result = substitute_arguments(template, &["file.rs", "high"]);
        assert_eq!(result, "Review file.rs with priority high");
    }

    #[test]
    fn substitute_arguments_all() {
        let template = "Process these: $ARGUMENTS";
        let result = substitute_arguments(template, &["a", "b", "c"]);
        assert_eq!(result, "Process these: a b c");
    }

    #[test]
    fn substitute_arguments_mixed() {
        let template = "First: $1, All: $ARGUMENTS, Second: $2";
        let result = substitute_arguments(template, &["one", "two", "three"]);
        assert_eq!(result, "First: one, All: one two three, Second: two");
    }

    #[test]
    fn substitute_arguments_empty() {
        let template = "No args: $ARGUMENTS";
        let result = substitute_arguments(template, &[]);
        assert_eq!(result, "No args: ");
    }

    #[test]
    fn custom_prompt_builder() {
        let prompt = CustomPrompt::new("test", "test content")
            .with_path("/path/to/test.md")
            .with_description("A test prompt")
            .with_argument_hint("[arg1] [arg2]");

        assert_eq!(prompt.name, "test");
        assert_eq!(prompt.content, "test content");
        assert_eq!(prompt.path, PathBuf::from("/path/to/test.md"));
        assert_eq!(prompt.description.as_deref(), Some("A test prompt"));
        assert_eq!(prompt.argument_hint.as_deref(), Some("[arg1] [arg2]"));
    }

    // ============================================================================
    // Additional test coverage (N=289)
    // ============================================================================

    // PROMPTS_CMD_PREFIX constant tests

    #[test]
    fn test_prompts_cmd_prefix_value() {
        assert_eq!(PROMPTS_CMD_PREFIX, "prompts");
    }

    #[test]
    fn test_prompts_cmd_prefix_no_trailing_colon() {
        assert!(!PROMPTS_CMD_PREFIX.ends_with(':'));
    }

    // CustomPrompt struct tests

    #[test]
    fn test_custom_prompt_new_minimal() {
        let prompt = CustomPrompt::new("minimal", "content");
        assert_eq!(prompt.name, "minimal");
        assert_eq!(prompt.content, "content");
        assert_eq!(prompt.path, PathBuf::new());
        assert!(prompt.description.is_none());
        assert!(prompt.argument_hint.is_none());
    }

    #[test]
    fn test_custom_prompt_new_with_empty_strings() {
        let prompt = CustomPrompt::new("", "");
        assert_eq!(prompt.name, "");
        assert_eq!(prompt.content, "");
    }

    #[test]
    fn test_custom_prompt_with_string_types() {
        let prompt = CustomPrompt::new(String::from("name"), String::from("content"))
            .with_path(PathBuf::from("/path"))
            .with_description(String::from("desc"))
            .with_argument_hint(String::from("hint"));

        assert_eq!(prompt.name, "name");
        assert_eq!(prompt.content, "content");
        assert_eq!(prompt.description.as_deref(), Some("desc"));
        assert_eq!(prompt.argument_hint.as_deref(), Some("hint"));
    }

    #[test]
    fn test_custom_prompt_clone() {
        let prompt1 = CustomPrompt::new("test", "content")
            .with_description("desc")
            .with_argument_hint("hint");
        let prompt2 = prompt1.clone();

        assert_eq!(prompt1.name, prompt2.name);
        assert_eq!(prompt1.content, prompt2.content);
        assert_eq!(prompt1.description, prompt2.description);
        assert_eq!(prompt1.argument_hint, prompt2.argument_hint);
    }

    #[test]
    fn test_custom_prompt_debug() {
        let prompt = CustomPrompt::new("test", "content");
        let debug = format!("{:?}", prompt);
        assert!(debug.contains("CustomPrompt"));
        assert!(debug.contains("test"));
    }

    #[test]
    fn test_custom_prompt_partial_eq() {
        let p1 = CustomPrompt::new("test", "content").with_description("desc");
        let p2 = CustomPrompt::new("test", "content").with_description("desc");
        let p3 = CustomPrompt::new("test", "different");

        assert_eq!(p1, p2);
        assert_ne!(p1, p3);
    }

    #[test]
    fn test_custom_prompt_chained_builders() {
        let prompt = CustomPrompt::new("n", "c")
            .with_path("p1")
            .with_path("p2") // Override
            .with_description("d1")
            .with_description("d2") // Override
            .with_argument_hint("h1")
            .with_argument_hint("h2"); // Override

        assert_eq!(prompt.path, PathBuf::from("p2"));
        assert_eq!(prompt.description.as_deref(), Some("d2"));
        assert_eq!(prompt.argument_hint.as_deref(), Some("h2"));
    }

    // default_prompts_dir tests

    #[test]
    fn test_default_prompts_dir_with_codex_home_env() {
        // Save original value
        let original = std::env::var("CODEX_HOME").ok();

        // Set CODEX_HOME
        std::env::set_var("CODEX_HOME", "/custom/codex");
        let dir = default_prompts_dir();
        assert_eq!(dir, Some(PathBuf::from("/custom/codex/prompts")));

        // Restore original
        match original {
            Some(val) => std::env::set_var("CODEX_HOME", val),
            None => std::env::remove_var("CODEX_HOME"),
        }
    }

    #[test]
    fn test_default_prompts_dir_with_empty_codex_home() {
        let original = std::env::var("CODEX_HOME").ok();

        // Set empty CODEX_HOME - should fall back to home dir
        std::env::set_var("CODEX_HOME", "");
        let dir = default_prompts_dir();
        // Should return Some with home-based path
        assert!(dir.is_some());
        assert!(dir.unwrap().to_string_lossy().contains(".codex-dashflow"));

        match original {
            Some(val) => std::env::set_var("CODEX_HOME", val),
            None => std::env::remove_var("CODEX_HOME"),
        }
    }

    #[test]
    fn test_default_prompts_dir_without_codex_home() {
        let original = std::env::var("CODEX_HOME").ok();

        std::env::remove_var("CODEX_HOME");
        let dir = default_prompts_dir();
        // Should return home-based path
        assert!(dir.is_some());
        let path = dir.unwrap();
        assert!(path.to_string_lossy().contains(".codex-dashflow"));
        assert!(path.to_string_lossy().ends_with("prompts"));

        if let Some(val) = original {
            std::env::set_var("CODEX_HOME", val);
        }
    }

    // parse_frontmatter edge cases

    #[test]
    fn test_parse_frontmatter_empty_input() {
        let (desc, hint, body) = parse_frontmatter("");
        assert!(desc.is_none());
        assert!(hint.is_none());
        assert_eq!(body, "");
    }

    #[test]
    fn test_parse_frontmatter_only_dashes() {
        let content = "---\n---\n";
        let (desc, hint, body) = parse_frontmatter(content);
        assert!(desc.is_none());
        assert!(hint.is_none());
        assert_eq!(body, "");
    }

    #[test]
    fn test_parse_frontmatter_with_extra_whitespace() {
        // The code trims the first line before checking for "---"
        // So whitespace-padded delimiters ARE recognized as frontmatter
        let content = "  ---  \ndescription: test\n  ---  \nbody";
        let (desc, hint, body) = parse_frontmatter(content);
        // First line is trimmed, so frontmatter IS detected
        assert_eq!(desc.as_deref(), Some("test"));
        assert!(hint.is_none());
        assert_eq!(body, "body");
    }

    #[test]
    fn test_parse_frontmatter_only_opening_delimiter() {
        let content = "---\ndescription: test";
        let (desc, hint, body) = parse_frontmatter(content);
        // Unterminated
        assert!(desc.is_none());
        assert!(hint.is_none());
        assert_eq!(body, content);
    }

    #[test]
    fn test_parse_frontmatter_empty_lines_in_frontmatter() {
        let content = "---\n\n\ndescription: test\n\n---\nbody";
        let (desc, hint, body) = parse_frontmatter(content);
        assert_eq!(desc.as_deref(), Some("test"));
        assert!(hint.is_none());
        assert_eq!(body, "body");
    }

    #[test]
    fn test_parse_frontmatter_unknown_keys_ignored() {
        let content = "---\nunknown_key: value\nfoo: bar\ndescription: actual\n---\nbody";
        let (desc, hint, body) = parse_frontmatter(content);
        assert_eq!(desc.as_deref(), Some("actual"));
        assert!(hint.is_none());
        assert_eq!(body, "body");
    }

    #[test]
    fn test_parse_frontmatter_argument_underscore_variant() {
        let content = "---\nargument_hint: underscore variant\n---\nbody";
        let (desc, hint, body) = parse_frontmatter(content);
        assert!(desc.is_none());
        assert_eq!(hint.as_deref(), Some("underscore variant"));
        assert_eq!(body, "body");
    }

    #[test]
    fn test_parse_frontmatter_argument_dash_variant() {
        let content = "---\nargument-hint: dash variant\n---\nbody";
        let (desc, hint, body) = parse_frontmatter(content);
        assert!(desc.is_none());
        assert_eq!(hint.as_deref(), Some("dash variant"));
        assert_eq!(body, "body");
    }

    #[test]
    fn test_parse_frontmatter_case_insensitive_keys() {
        let content = "---\nDESCRIPTION: upper\nARGUMENT-HINT: also upper\n---\nbody";
        let (desc, hint, body) = parse_frontmatter(content);
        assert_eq!(desc.as_deref(), Some("upper"));
        assert_eq!(hint.as_deref(), Some("also upper"));
        assert_eq!(body, "body");
    }

    #[test]
    fn test_parse_frontmatter_value_with_colons() {
        let content = "---\ndescription: value:with:colons\n---\nbody";
        let (desc, _, body) = parse_frontmatter(content);
        assert_eq!(desc.as_deref(), Some("value:with:colons"));
        assert_eq!(body, "body");
    }

    #[test]
    fn test_parse_frontmatter_value_without_quotes() {
        let content = "---\ndescription: unquoted value\n---\nbody";
        let (desc, _, body) = parse_frontmatter(content);
        assert_eq!(desc.as_deref(), Some("unquoted value"));
        assert_eq!(body, "body");
    }

    #[test]
    fn test_parse_frontmatter_mixed_quotes() {
        let content =
            "---\ndescription: \"double quoted\"\nargument-hint: 'single quoted'\n---\nbody";
        let (desc, hint, body) = parse_frontmatter(content);
        assert_eq!(desc.as_deref(), Some("double quoted"));
        assert_eq!(hint.as_deref(), Some("single quoted"));
        assert_eq!(body, "body");
    }

    #[test]
    fn test_parse_frontmatter_single_char_value() {
        // Edge case: value is exactly 1 character (less than 2, quote stripping skipped)
        let content = "---\ndescription: x\n---\nbody";
        let (desc, _, body) = parse_frontmatter(content);
        assert_eq!(desc.as_deref(), Some("x"));
        assert_eq!(body, "body");
    }

    #[test]
    fn test_parse_frontmatter_two_char_quoted_value() {
        // Exactly 2 chars including quotes
        let content = "---\ndescription: \"\"\n---\nbody";
        let (desc, _, body) = parse_frontmatter(content);
        // Empty string after stripping quotes
        assert_eq!(desc.as_deref(), Some(""));
        assert_eq!(body, "body");
    }

    #[test]
    fn test_parse_frontmatter_mismatched_quotes() {
        // Mismatched quotes should not strip
        let content = "---\ndescription: \"value'\n---\nbody";
        let (desc, _, body) = parse_frontmatter(content);
        // Quotes are mismatched, so value should include them
        assert_eq!(desc.as_deref(), Some("\"value'"));
        assert_eq!(body, "body");
    }

    #[test]
    fn test_parse_frontmatter_multiline_body() {
        let content = "---\ndescription: test\n---\nLine 1\nLine 2\nLine 3\n";
        let (desc, hint, body) = parse_frontmatter(content);
        assert_eq!(desc.as_deref(), Some("test"));
        assert!(hint.is_none());
        assert_eq!(body, "Line 1\nLine 2\nLine 3\n");
    }

    #[test]
    fn test_parse_frontmatter_body_with_dashes() {
        let content = "---\ndescription: test\n---\n---this is not frontmatter---\n";
        let (desc, _, body) = parse_frontmatter(content);
        assert_eq!(desc.as_deref(), Some("test"));
        assert_eq!(body, "---this is not frontmatter---\n");
    }

    #[test]
    fn test_parse_frontmatter_windows_line_endings() {
        let content = "---\r\ndescription: test\r\n---\r\nbody\r\n";
        let (desc, _, body) = parse_frontmatter(content);
        assert_eq!(desc.as_deref(), Some("test"));
        assert_eq!(body, "body\r\n");
    }

    #[test]
    fn test_parse_frontmatter_no_newline_after_closing() {
        let content = "---\ndescription: test\n---";
        let (desc, _, body) = parse_frontmatter(content);
        assert_eq!(desc.as_deref(), Some("test"));
        assert_eq!(body, "");
    }

    // substitute_arguments edge cases

    #[test]
    fn test_substitute_arguments_no_placeholders() {
        let result = substitute_arguments("No placeholders here", &["arg1", "arg2"]);
        assert_eq!(result, "No placeholders here");
    }

    #[test]
    fn test_substitute_arguments_unused_positional() {
        let result = substitute_arguments("$1 and $2 and $3", &["a", "b"]);
        // $3 should remain as-is since no arg provided
        assert_eq!(result, "a and b and $3");
    }

    #[test]
    fn test_substitute_arguments_multiple_same_placeholder() {
        let result = substitute_arguments("$1 $1 $1", &["repeat"]);
        assert_eq!(result, "repeat repeat repeat");
    }

    #[test]
    fn test_substitute_arguments_double_digit() {
        // Note: The current implementation replaces $1 before $10, so $10 becomes "zero0"
        // This is a known limitation of the simple string replacement approach
        let args: Vec<&str> = (0..12)
            .map(|i| match i {
                0 => "zero",
                1 => "one",
                2 => "two",
                3 => "three",
                4 => "four",
                5 => "five",
                6 => "six",
                7 => "seven",
                8 => "eight",
                9 => "nine",
                10 => "ten",
                11 => "eleven",
                _ => "other",
            })
            .collect();
        // $1 is replaced first, so $10 -> "zero0", $11 -> "zero1"
        let result = substitute_arguments("$1 $10 $11", &args);
        assert_eq!(result, "zero zero0 zero1");
    }

    #[test]
    fn test_substitute_arguments_dollar_sign_not_followed_by_number() {
        let result = substitute_arguments("$notanumber $", &["arg"]);
        // Should not replace $notanumber or standalone $
        assert_eq!(result, "$notanumber $");
    }

    #[test]
    fn test_substitute_arguments_arguments_first() {
        // $ARGUMENTS is replaced before positional
        let result = substitute_arguments("$ARGUMENTS then $1", &["first", "second"]);
        assert_eq!(result, "first second then first");
    }

    #[test]
    fn test_substitute_arguments_special_chars_in_args() {
        let result = substitute_arguments("$1", &["arg with $1 in it"]);
        // After replacing $1 with the arg, the "$1" inside should remain
        assert_eq!(result, "arg with $1 in it");
    }

    #[test]
    fn test_substitute_arguments_empty_arg() {
        let result = substitute_arguments("[$1]", &[""]);
        assert_eq!(result, "[]");
    }

    #[test]
    fn test_substitute_arguments_unicode_args() {
        let result = substitute_arguments("$1 $2", &["日本語", "emoji 🎉"]);
        assert_eq!(result, "日本語 emoji 🎉");
    }

    #[test]
    fn test_substitute_arguments_newlines_in_args() {
        let result = substitute_arguments("$1", &["line1\nline2\nline3"]);
        assert_eq!(result, "line1\nline2\nline3");
    }

    // discover_prompts_in edge cases

    #[tokio::test]
    async fn test_discover_prompts_empty_directory() {
        let tmp = tempdir().expect("create TempDir");
        let found = discover_prompts_in(tmp.path()).await;
        assert!(found.is_empty());
    }

    #[tokio::test]
    async fn test_discover_prompts_only_non_md_files() {
        let tmp = tempdir().expect("create TempDir");
        fs::write(tmp.path().join("file.txt"), "text").unwrap();
        fs::write(tmp.path().join("file.rs"), "rust").unwrap();
        fs::write(tmp.path().join("file"), "no extension").unwrap();
        let found = discover_prompts_in(tmp.path()).await;
        assert!(found.is_empty());
    }

    #[tokio::test]
    async fn test_discover_prompts_case_insensitive_extension() {
        let tmp = tempdir().expect("create TempDir");
        fs::write(tmp.path().join("lower.md"), "lower").unwrap();
        fs::write(tmp.path().join("upper.MD"), "upper").unwrap();
        fs::write(tmp.path().join("mixed.Md"), "mixed").unwrap();
        let found = discover_prompts_in(tmp.path()).await;
        let names: Vec<&str> = found.iter().map(|p| p.name.as_str()).collect();
        assert_eq!(names.len(), 3);
        assert!(names.contains(&"lower"));
        assert!(names.contains(&"upper"));
        assert!(names.contains(&"mixed"));
    }

    #[tokio::test]
    async fn test_discover_prompts_preserves_content() {
        let tmp = tempdir().expect("create TempDir");
        let content = "This is the full content\nWith multiple lines\n";
        fs::write(tmp.path().join("test.md"), content).unwrap();
        let found = discover_prompts_in(tmp.path()).await;
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].content, content);
    }

    #[tokio::test]
    async fn test_discover_prompts_sets_path() {
        let tmp = tempdir().expect("create TempDir");
        fs::write(tmp.path().join("prompt.md"), "content").unwrap();
        let found = discover_prompts_in(tmp.path()).await;
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].path, tmp.path().join("prompt.md"));
    }

    #[tokio::test]
    async fn test_discover_prompts_excluding_multiple() {
        let tmp = tempdir().expect("create TempDir");
        fs::write(tmp.path().join("a.md"), "a").unwrap();
        fs::write(tmp.path().join("b.md"), "b").unwrap();
        fs::write(tmp.path().join("c.md"), "c").unwrap();
        fs::write(tmp.path().join("d.md"), "d").unwrap();

        let mut exclude = HashSet::new();
        exclude.insert("a".to_string());
        exclude.insert("c".to_string());

        let found = discover_prompts_in_excluding(tmp.path(), &exclude).await;
        let names: Vec<&str> = found.iter().map(|p| p.name.as_str()).collect();
        assert_eq!(names, vec!["b", "d"]);
    }

    #[tokio::test]
    async fn test_discover_prompts_excluding_empty_set() {
        let tmp = tempdir().expect("create TempDir");
        fs::write(tmp.path().join("a.md"), "a").unwrap();
        fs::write(tmp.path().join("b.md"), "b").unwrap();

        let exclude = HashSet::new();
        let found = discover_prompts_in_excluding(tmp.path(), &exclude).await;
        assert_eq!(found.len(), 2);
    }

    #[tokio::test]
    async fn test_discover_prompts_sorted_alphabetically() {
        let tmp = tempdir().expect("create TempDir");
        fs::write(tmp.path().join("zebra.md"), "z").unwrap();
        fs::write(tmp.path().join("alpha.md"), "a").unwrap();
        fs::write(tmp.path().join("beta.md"), "b").unwrap();

        let found = discover_prompts_in(tmp.path()).await;
        let names: Vec<&str> = found.iter().map(|p| p.name.as_str()).collect();
        assert_eq!(names, vec!["alpha", "beta", "zebra"]);
    }

    #[tokio::test]
    async fn test_discover_prompts_ignores_subdirectories() {
        let tmp = tempdir().expect("create TempDir");
        fs::write(tmp.path().join("root.md"), "root").unwrap();

        let subdir = tmp.path().join("subdir");
        fs::create_dir(&subdir).unwrap();
        fs::write(subdir.join("nested.md"), "nested").unwrap();

        let found = discover_prompts_in(tmp.path()).await;
        let names: Vec<&str> = found.iter().map(|p| p.name.as_str()).collect();
        // Should only find root.md, not nested.md
        assert_eq!(names, vec!["root"]);
    }

    #[tokio::test]
    async fn test_discover_prompts_with_frontmatter() {
        let tmp = tempdir().expect("create TempDir");
        let content =
            "---\ndescription: Test description\nargument-hint: [file]\n---\nBody content";
        fs::write(tmp.path().join("test.md"), content).unwrap();

        let found = discover_prompts_in(tmp.path()).await;
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].description.as_deref(), Some("Test description"));
        assert_eq!(found[0].argument_hint.as_deref(), Some("[file]"));
        assert_eq!(found[0].content, "Body content");
    }

    #[tokio::test]
    async fn test_discover_prompts_special_filename_chars() {
        let tmp = tempdir().expect("create TempDir");
        // Filename with spaces and dashes
        fs::write(tmp.path().join("my-prompt.md"), "content1").unwrap();
        fs::write(tmp.path().join("another_prompt.md"), "content2").unwrap();

        let found = discover_prompts_in(tmp.path()).await;
        let names: Vec<&str> = found.iter().map(|p| p.name.as_str()).collect();
        assert!(names.contains(&"my-prompt"));
        assert!(names.contains(&"another_prompt"));
    }

    #[tokio::test]
    async fn test_discover_prompts_unicode_content() {
        let tmp = tempdir().expect("create TempDir");
        let content = "日本語のプロンプト\n中文提示\n한국어 프롬프트\n";
        fs::write(tmp.path().join("unicode.md"), content).unwrap();

        let found = discover_prompts_in(tmp.path()).await;
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].content, content);
    }
}
