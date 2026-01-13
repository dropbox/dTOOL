// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)
// DashFlow Project - Project Discovery

//! # Project Discovery
//!
//! Core project discovery and context analysis.

use crate::documentation::{Documentation, DocumentationType};
use crate::languages::{BuildSystem, Framework, Language};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use thiserror::Error;
use walkdir::WalkDir;

/// Errors during project discovery
#[derive(Error, Debug)]
#[non_exhaustive]
pub enum ProjectError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Path not found: {0}")]
    PathNotFound(PathBuf),

    #[error("Not a directory: {0}")]
    NotDirectory(PathBuf),

    #[error("Discovery error: {0}")]
    Discovery(String),

    #[error("Project not found: no .dashflow/ directory found in any parent directory from {0}")]
    ProjectNotFound(PathBuf),
}

/// Result type for project operations
pub type ProjectResult<T> = Result<T, ProjectError>;

/// Discovered project context
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectContext {
    /// Root path of the project
    pub root: PathBuf,

    /// Project name (from directory or config)
    pub name: Option<String>,

    /// Detected programming languages with file counts
    pub languages: HashMap<Language, usize>,

    /// Primary language (most files)
    pub primary_language: Option<Language>,

    /// Detected frameworks
    pub frameworks: HashSet<Framework>,

    /// Detected build systems
    pub build_systems: HashSet<BuildSystem>,

    /// Primary build system
    pub primary_build_system: Option<BuildSystem>,

    /// Documentation files found
    pub documentation: Vec<Documentation>,

    /// Source directories
    pub source_dirs: Vec<PathBuf>,

    /// Config files found
    pub config_files: Vec<PathBuf>,

    /// Git repository detected
    pub has_git: bool,

    /// Monorepo detected (multiple workspaces/packages)
    pub is_monorepo: bool,

    /// Total files scanned
    pub total_files: usize,

    /// Total lines of code (approximate)
    pub total_loc: usize,
}

impl ProjectContext {
    /// Create new empty project context
    pub fn new(root: PathBuf) -> Self {
        Self {
            root,
            name: None,
            languages: HashMap::new(),
            primary_language: None,
            frameworks: HashSet::new(),
            build_systems: HashSet::new(),
            primary_build_system: None,
            documentation: Vec::new(),
            source_dirs: Vec::new(),
            config_files: Vec::new(),
            has_git: false,
            is_monorepo: false,
            total_files: 0,
            total_loc: 0,
        }
    }

    /// Get documentation files sorted by priority
    pub fn get_documentation(&self) -> ProjectResult<Vec<&Documentation>> {
        let mut docs: Vec<_> = self.documentation.iter().collect();
        docs.sort_by_key(|d| d.priority());
        Ok(docs)
    }

    /// Get documentation content for LLM context (within token budget)
    pub fn get_documentation_content(&self, max_bytes: usize) -> ProjectResult<String> {
        let mut result = String::new();
        let mut remaining = max_bytes;

        let docs = self.get_documentation()?;
        for doc in docs {
            if remaining == 0 {
                break;
            }

            let content = std::fs::read_to_string(&doc.path)?;
            let header = format!(
                "\n## {} ({})\n\n",
                doc.doc_type.display_name(),
                doc.path.display()
            );

            if header.len() + content.len() <= remaining {
                result.push_str(&header);
                result.push_str(&content);
                remaining -= header.len() + content.len();
            } else if header.len() < remaining {
                // Truncate content
                let available = remaining - header.len();
                result.push_str(&header);
                result.push_str(&content[..available.min(content.len())]);
                result.push_str("\n... (truncated)");
                break;
            }
        }

        Ok(result)
    }

    /// Walk from a starting directory upward to the project root,
    /// collecting files matching the given filename pattern.
    ///
    /// Returns files in order from nearest (start) to farthest (root).
    /// This is useful for finding configuration files that apply to the
    /// current working context, like `AGENTS.md` or `CLAUDE.md`.
    ///
    /// # Arguments
    ///
    /// * `start` - Starting directory to walk up from
    /// * `pattern` - Filename to match (e.g., "AGENTS.md", "CLAUDE.md")
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use dashflow_project::{ProjectContext, discover_project};
    /// use std::path::PathBuf;
    ///
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let project = discover_project(PathBuf::from("/repo")).await?;
    ///
    /// // Find all AGENTS.md files from /repo/src/components up to /repo
    /// let agent_files = project.discover_to_root(
    ///     &PathBuf::from("/repo/src/components"),
    ///     "AGENTS.md",
    /// );
    ///
    /// // Returns: [/repo/src/AGENTS.md, /repo/AGENTS.md]
    /// for file in agent_files {
    ///     println!("Found: {}", file.display());
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub fn discover_to_root(&self, start: &Path, pattern: &str) -> Vec<PathBuf> {
        let mut results = Vec::new();

        // Canonicalize both paths for reliable comparison
        // This handles macOS /var -> /private/var symlinks
        let root = self
            .root
            .canonicalize()
            .unwrap_or_else(|_| self.root.clone());
        let mut current = start.canonicalize().unwrap_or_else(|_| start.to_path_buf());

        // Walk upward from start to root
        while current.starts_with(&root) {
            let candidate = current.join(pattern);
            if candidate.exists() && candidate.is_file() {
                results.push(candidate);
            }

            // Stop if we've reached the root
            if current == root {
                break;
            }

            // Move to parent directory
            if !current.pop() {
                break;
            }
        }

        results
    }

    /// Walk from a starting directory upward to the project root,
    /// collecting files matching any of the given filename patterns.
    ///
    /// Returns a map from pattern to found files, with each file list
    /// ordered from nearest (start) to farthest (root).
    ///
    /// # Arguments
    ///
    /// * `start` - Starting directory to walk up from
    /// * `patterns` - Filenames to match (e.g., ["AGENTS.md", "CLAUDE.md"])
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use dashflow_project::{ProjectContext, discover_project};
    /// use std::path::PathBuf;
    ///
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let project = discover_project(PathBuf::from("/repo")).await?;
    ///
    /// // Find multiple instruction file types
    /// let files = project.discover_to_root_multi(
    ///     &PathBuf::from("/repo/src/components"),
    ///     &["AGENTS.md", "CLAUDE.md", ".cursorrules"],
    /// );
    ///
    /// for (pattern, paths) in &files {
    ///     println!("{}: {} files found", pattern, paths.len());
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub fn discover_to_root_multi(
        &self,
        start: &Path,
        patterns: &[&str],
    ) -> HashMap<String, Vec<PathBuf>> {
        let mut results: HashMap<String, Vec<PathBuf>> = HashMap::new();

        // Canonicalize both paths for reliable comparison
        let root = self
            .root
            .canonicalize()
            .unwrap_or_else(|_| self.root.clone());
        let mut current = start.canonicalize().unwrap_or_else(|_| start.to_path_buf());

        // Walk upward from start to root
        while current.starts_with(&root) {
            for pattern in patterns {
                let candidate = current.join(pattern);
                if candidate.exists() && candidate.is_file() {
                    results
                        .entry((*pattern).to_string())
                        .or_default()
                        .push(candidate);
                }
            }

            // Stop if we've reached the root
            if current == root {
                break;
            }

            // Move to parent directory
            if !current.pop() {
                break;
            }
        }

        results
    }

    /// Get project summary for LLM context
    pub fn summary(&self) -> String {
        let mut lines = Vec::new();

        if let Some(ref name) = self.name {
            lines.push(format!("Project: {}", name));
        }

        if let Some(lang) = self.primary_language {
            lines.push(format!("Primary Language: {}", lang.display_name()));
        }

        if !self.languages.is_empty() {
            let langs: Vec<_> = self
                .languages
                .iter()
                .map(|(l, c)| format!("{} ({} files)", l.display_name(), c))
                .collect();
            lines.push(format!("Languages: {}", langs.join(", ")));
        }

        if !self.frameworks.is_empty() {
            let frameworks: Vec<_> = self.frameworks.iter().map(|f| f.display_name()).collect();
            lines.push(format!("Frameworks: {}", frameworks.join(", ")));
        }

        if let Some(build) = self.primary_build_system {
            lines.push(format!("Build System: {}", build.display_name()));
        }

        if self.has_git {
            lines.push("Version Control: Git".to_string());
        }

        if self.is_monorepo {
            lines.push("Structure: Monorepo".to_string());
        }

        lines.push(format!("Files: {}", self.total_files));

        lines.join("\n")
    }
}

/// Discover project context from a directory
///
/// This function uses `spawn_blocking` internally to avoid blocking the async
/// runtime during filesystem operations (WalkDir, metadata reads, etc.).
pub async fn discover_project(root: PathBuf) -> ProjectResult<ProjectContext> {
    tokio::task::spawn_blocking(move || discover_project_sync(root))
        .await
        .map_err(|e| {
            ProjectError::Discovery(format!("Discovery task panicked: {e}"))
        })?
}

/// Discover project by walking UP the directory tree until finding `.dashflow/`
///
/// This is useful when you're somewhere inside a project and need to find the
/// project root. Unlike `discover_project()` which requires the root path,
/// this function walks upward from any starting point.
///
/// # Arguments
///
/// * `start` - Any directory inside (or at the root of) a DashFlow project
///
/// # Returns
///
/// A `ProjectContext` with root set to the directory containing `.dashflow/`
///
/// # Example
///
/// ```rust,no_run
/// use dashflow_project::discover_from_anywhere;
/// use std::path::PathBuf;
///
/// #[tokio::main]
/// async fn main() -> Result<(), Box<dyn std::error::Error>> {
///     // From anywhere inside a project (e.g., /repo/src/components)
///     let project = discover_from_anywhere(PathBuf::from(".")).await?;
///
///     println!("Project root: {}", project.root.display());
///     // Output: Project root: /repo
///
///     Ok(())
/// }
/// ```
pub async fn discover_from_anywhere(start: PathBuf) -> ProjectResult<ProjectContext> {
    tokio::task::spawn_blocking(move || discover_from_anywhere_sync(&start))
        .await
        .map_err(|e| {
            ProjectError::Discovery(format!("Discovery task panicked: {e}"))
        })?
}

/// Synchronous implementation of upward project discovery.
fn discover_from_anywhere_sync(start: &Path) -> ProjectResult<ProjectContext> {
    let mut current = start.canonicalize().map_err(|e| {
        if !start.exists() {
            ProjectError::PathNotFound(start.to_path_buf())
        } else {
            ProjectError::Io(e)
        }
    })?;

    let original_start = current.clone();

    loop {
        let dashflow_dir = current.join(".dashflow");
        if dashflow_dir.exists() && dashflow_dir.is_dir() {
            // Found .dashflow/ - this is the project root
            return discover_project_sync(current);
        }

        // Move to parent directory
        match current.parent() {
            Some(parent) if parent != current => {
                current = parent.to_path_buf();
            }
            _ => {
                // Reached filesystem root without finding .dashflow/
                return Err(ProjectError::ProjectNotFound(original_start));
            }
        }
    }
}

/// Synchronous implementation of project discovery.
///
/// All blocking filesystem operations are contained here to be run via
/// `spawn_blocking` in the async wrapper.
fn discover_project_sync(root: PathBuf) -> ProjectResult<ProjectContext> {
    if !root.exists() {
        return Err(ProjectError::PathNotFound(root));
    }

    if !root.is_dir() {
        return Err(ProjectError::NotDirectory(root));
    }

    let mut context = ProjectContext::new(root.clone());

    // Detect git
    context.has_git = root.join(".git").exists();

    // Walk the directory tree
    for entry in WalkDir::new(&root)
        .follow_links(false)
        .max_depth(10)
        .into_iter()
        .filter_entry(|e| !is_ignored(e.path()))
    {
        let entry = match entry {
            Ok(e) => e,
            Err(_) => continue,
        };

        let path = entry.path();

        if path.is_file() {
            context.total_files += 1;

            // Get filename
            let filename = path.file_name().and_then(|n| n.to_str()).unwrap_or("");

            // Detect build system
            if let Some(build_system) = BuildSystem::from_config_file(filename) {
                context.build_systems.insert(build_system);
                context.config_files.push(path.to_path_buf());
            }

            // Detect language from extension
            if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                if let Some(lang) = Language::from_extension(ext) {
                    *context.languages.entry(lang).or_insert(0) += 1;
                }
            }

            // Detect documentation
            if let Some(doc_type) = DocumentationType::from_filename(filename) {
                let metadata = std::fs::metadata(path)?;
                context.documentation.push(Documentation::new(
                    doc_type,
                    path.to_path_buf(),
                    metadata.len(),
                ));
            }
        }
    }

    // Determine primary language
    if let Some((lang, _)) = context.languages.iter().max_by_key(|(_, count)| *count) {
        context.primary_language = Some(*lang);
    }

    // Determine primary build system (prefer explicit ones)
    let build_priority = [
        BuildSystem::Cargo,
        BuildSystem::GoMod,
        BuildSystem::Npm,
        BuildSystem::Poetry,
        BuildSystem::Pip,
        BuildSystem::Maven,
        BuildSystem::Gradle,
    ];

    for build in build_priority {
        if context.build_systems.contains(&build) {
            context.primary_build_system = Some(build);
            break;
        }
    }

    // Detect monorepo
    context.is_monorepo = detect_monorepo(&root);

    // Try to detect project name
    context.name = detect_project_name(&root);

    // Detect frameworks
    context.frameworks = detect_frameworks(&root, &context.build_systems);

    // Sort documentation by priority
    context.documentation.sort_by_key(|d| d.doc_type.priority());

    Ok(context)
}

/// Check if a path should be ignored during scanning
fn is_ignored(path: &Path) -> bool {
    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");

    matches!(
        name,
        "node_modules"
            | "target"
            | ".git"
            | ".hg"
            | ".svn"
            | "__pycache__"
            | ".pytest_cache"
            | ".mypy_cache"
            | "venv"
            | ".venv"
            | "env"
            | ".env"
            | "dist"
            | "build"
            | ".next"
            | ".nuxt"
            | "vendor"
            | ".cargo"
            | ".rustup"
    )
}

/// Detect if this is a monorepo
fn detect_monorepo(root: &Path) -> bool {
    // Cargo workspace
    if let Ok(content) = std::fs::read_to_string(root.join("Cargo.toml")) {
        if content.contains("[workspace]") {
            return true;
        }
    }

    // npm/yarn workspaces
    if let Ok(content) = std::fs::read_to_string(root.join("package.json")) {
        if content.contains("\"workspaces\"") {
            return true;
        }
    }

    // pnpm workspaces
    if root.join("pnpm-workspace.yaml").exists() {
        return true;
    }

    // Lerna
    if root.join("lerna.json").exists() {
        return true;
    }

    // Go workspace
    if root.join("go.work").exists() {
        return true;
    }

    false
}

/// Detect project name from various sources
fn detect_project_name(root: &Path) -> Option<String> {
    // Try Cargo.toml
    if let Ok(content) = std::fs::read_to_string(root.join("Cargo.toml")) {
        if let Some(name) = extract_toml_name(&content) {
            return Some(name);
        }
    }

    // Try package.json
    if let Ok(content) = std::fs::read_to_string(root.join("package.json")) {
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(&content) {
            if let Some(name) = json.get("name").and_then(|n| n.as_str()) {
                return Some(name.to_string());
            }
        }
    }

    // Try pyproject.toml
    if let Ok(content) = std::fs::read_to_string(root.join("pyproject.toml")) {
        if let Some(name) = extract_toml_name(&content) {
            return Some(name);
        }
    }

    // Fall back to directory name
    root.file_name()
        .and_then(|n| n.to_str())
        .map(|s| s.to_string())
}

/// Simple TOML name extraction (avoiding full TOML parser dependency)
fn extract_toml_name(content: &str) -> Option<String> {
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum TomlSection {
        CargoPackage,
        PyprojectProject,
        PoetryTool,
    }

    fn section_from_header(header: &str) -> Option<TomlSection> {
        match header.trim() {
            "package" => Some(TomlSection::CargoPackage),
            "project" => Some(TomlSection::PyprojectProject),
            "tool.poetry" => Some(TomlSection::PoetryTool),
            _ => None,
        }
    }

    fn parse_string_value(value: &str) -> Option<String> {
        let value = value.trim();
        if value.is_empty() {
            return None;
        }

        let (quote_char, rest) = match value.as_bytes().first().copied() {
            Some(b'"') => ('"', &value[1..]),
            Some(b'\'') => ('\'', &value[1..]),
            _ => {
                // TOML strings should be quoted; accept bare tokens defensively.
                let bare = value
                    .split_once('#')
                    .map_or(value, |(left, _)| left)
                    .trim();
                return (!bare.is_empty()).then(|| bare.to_string());
            }
        };

        let end = rest.find(quote_char)?;
        let parsed = &rest[..end];
        (!parsed.is_empty()).then(|| parsed.to_string())
    }

    let mut current_section: Option<TomlSection> = None;

    for raw_line in content.lines() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        if line.starts_with('[') && line.ends_with(']') {
            let header = &line[1..line.len() - 1];
            current_section = section_from_header(header);
            continue;
        }

        let Some(section) = current_section else {
            continue;
        };

        // Only accept name within known sections to avoid accidentally reading
        // dependency/package names in other tables.
        match section {
            TomlSection::CargoPackage | TomlSection::PyprojectProject | TomlSection::PoetryTool => {}
        }

        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        if key.trim() != "name" {
            continue;
        }

        if let Some(name) = parse_string_value(value) {
            return Some(name);
        }
    }

    None
}

/// Detect frameworks from config files
fn detect_frameworks(root: &Path, build_systems: &HashSet<BuildSystem>) -> HashSet<Framework> {
    let mut frameworks = HashSet::new();

    // Rust frameworks from Cargo.toml
    if build_systems.contains(&BuildSystem::Cargo) {
        if let Ok(content) = std::fs::read_to_string(root.join("Cargo.toml")) {
            if content.contains("actix-web") {
                frameworks.insert(Framework::Actix);
            }
            if content.contains("axum") {
                frameworks.insert(Framework::Axum);
            }
            if content.contains("rocket") {
                frameworks.insert(Framework::Rocket);
            }
            if content.contains("warp") {
                frameworks.insert(Framework::Warp);
            }
            if content.contains("tokio") {
                frameworks.insert(Framework::Tokio);
            }
        }
    }

    // Python frameworks from requirements.txt or pyproject.toml
    if build_systems.contains(&BuildSystem::Pip) || build_systems.contains(&BuildSystem::Poetry) {
        let files = ["requirements.txt", "pyproject.toml", "setup.py"];
        for file in files {
            if let Ok(content) = std::fs::read_to_string(root.join(file)) {
                let lower = content.to_lowercase();
                if lower.contains("django") {
                    frameworks.insert(Framework::Django);
                }
                if lower.contains("flask") {
                    frameworks.insert(Framework::Flask);
                }
                if lower.contains("fastapi") {
                    frameworks.insert(Framework::FastAPI);
                }
                if lower.contains("pytorch") || lower.contains("torch") {
                    frameworks.insert(Framework::PyTorch);
                }
                if lower.contains("tensorflow") {
                    frameworks.insert(Framework::TensorFlow);
                }
            }
        }
    }

    // JavaScript/TypeScript frameworks from package.json
    if build_systems.contains(&BuildSystem::Npm)
        || build_systems.contains(&BuildSystem::Yarn)
        || build_systems.contains(&BuildSystem::Pnpm)
    {
        if let Ok(content) = std::fs::read_to_string(root.join("package.json")) {
            if content.contains("\"react\"") {
                frameworks.insert(Framework::React);
            }
            if content.contains("\"vue\"") {
                frameworks.insert(Framework::Vue);
            }
            if content.contains("\"@angular/core\"") {
                frameworks.insert(Framework::Angular);
            }
            if content.contains("\"next\"") {
                frameworks.insert(Framework::NextJs);
            }
            if content.contains("\"express\"") {
                frameworks.insert(Framework::Express);
            }
            if content.contains("\"@nestjs/core\"") {
                frameworks.insert(Framework::NestJs);
            }
        }
    }

    frameworks
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_discover_empty_project() {
        let temp_dir = TempDir::new().unwrap();
        let context = discover_project(temp_dir.path().to_path_buf())
            .await
            .unwrap();

        assert!(!context.has_git);
        assert!(context.languages.is_empty());
        assert!(context.build_systems.is_empty());
    }

    #[tokio::test]
    async fn test_discover_rust_project() {
        let temp_dir = TempDir::new().unwrap();

        // Create Cargo.toml
        std::fs::write(
            temp_dir.path().join("Cargo.toml"),
            r#"[package]
name = "test-project"
version = "1.0.0"
"#,
        )
        .unwrap();

        // Create src/lib.rs
        std::fs::create_dir(temp_dir.path().join("src")).unwrap();
        std::fs::write(temp_dir.path().join("src/lib.rs"), "// Test").unwrap();

        // Create README.md
        std::fs::write(temp_dir.path().join("README.md"), "# Test Project").unwrap();

        let context = discover_project(temp_dir.path().to_path_buf())
            .await
            .unwrap();

        assert_eq!(context.name, Some("test-project".to_string()));
        assert!(context.build_systems.contains(&BuildSystem::Cargo));
        assert_eq!(context.primary_build_system, Some(BuildSystem::Cargo));
        assert!(context.languages.contains_key(&Language::Rust));
        assert!(!context.documentation.is_empty());
    }

    #[tokio::test]
    async fn test_discover_monorepo() {
        let temp_dir = TempDir::new().unwrap();

        // Create workspace Cargo.toml
        std::fs::write(
            temp_dir.path().join("Cargo.toml"),
            r#"[workspace]
members = ["crates/*"]
"#,
        )
        .unwrap();

        let context = discover_project(temp_dir.path().to_path_buf())
            .await
            .unwrap();

        assert!(context.is_monorepo);
    }

    #[tokio::test]
    async fn test_discover_with_git() {
        let temp_dir = TempDir::new().unwrap();

        // Create .git directory
        std::fs::create_dir(temp_dir.path().join(".git")).unwrap();

        let context = discover_project(temp_dir.path().to_path_buf())
            .await
            .unwrap();

        assert!(context.has_git);
    }

    #[tokio::test]
    async fn test_path_not_found() {
        let result = discover_project(PathBuf::from("/nonexistent/path")).await;
        assert!(matches!(result, Err(ProjectError::PathNotFound(_))));
    }

    #[test]
    fn test_extract_toml_name() {
        let content = r#"
[package]
name = "my-project"
version = "1.0.0"
"#;
        assert_eq!(extract_toml_name(content), Some("my-project".to_string()));
    }

    #[test]
    fn test_extract_toml_name_ignores_non_package_sections() {
        let content = r#"
[dependencies]
name = "this-should-not-win"

[package]
name = "real-project"
version = "1.0.0"
"#;
        assert_eq!(extract_toml_name(content), Some("real-project".to_string()));
    }

    #[test]
    fn test_extract_toml_name_pyproject_project_section() {
        let content = r#"
[project]
name = "pyproject-name"
version = "0.1.0"
"#;
        assert_eq!(extract_toml_name(content), Some("pyproject-name".to_string()));
    }

    #[test]
    fn test_extract_toml_name_poetry_section() {
        let content = r#"
[tool.poetry]
name = "poetry-name"
version = "0.1.0"
"#;
        assert_eq!(extract_toml_name(content), Some("poetry-name".to_string()));
    }

    #[test]
    fn test_extract_toml_name_missing_relevant_section() {
        let content = r#"
[workspace]
members = ["crates/*"]
"#;
        assert_eq!(extract_toml_name(content), None);
    }

    #[test]
    fn test_is_ignored() {
        assert!(is_ignored(Path::new("/project/node_modules")));
        assert!(is_ignored(Path::new("/project/target")));
        assert!(is_ignored(Path::new("/project/.git")));
        assert!(!is_ignored(Path::new("/project/src")));
        assert!(!is_ignored(Path::new("/project/lib")));
    }

    #[tokio::test]
    async fn test_discover_from_anywhere_at_root() {
        let temp_dir = TempDir::new().unwrap();

        // Create .dashflow directory (marks project root)
        std::fs::create_dir(temp_dir.path().join(".dashflow")).unwrap();

        // Create Cargo.toml
        std::fs::write(
            temp_dir.path().join("Cargo.toml"),
            r#"[package]
name = "test-project"
version = "1.0.0"
"#,
        )
        .unwrap();

        // Discover from root
        let context = discover_from_anywhere(temp_dir.path().to_path_buf())
            .await
            .unwrap();

        assert_eq!(context.name, Some("test-project".to_string()));
        assert_eq!(context.root.canonicalize().unwrap(), temp_dir.path().canonicalize().unwrap());
    }

    #[tokio::test]
    async fn test_discover_from_anywhere_from_subdirectory() {
        let temp_dir = TempDir::new().unwrap();

        // Create .dashflow directory at root (marks project root)
        std::fs::create_dir(temp_dir.path().join(".dashflow")).unwrap();

        // Create nested directory structure
        std::fs::create_dir_all(temp_dir.path().join("src/components/ui")).unwrap();

        // Create Cargo.toml
        std::fs::write(
            temp_dir.path().join("Cargo.toml"),
            r#"[package]
name = "nested-test"
version = "1.0.0"
"#,
        )
        .unwrap();

        // Discover from deeply nested subdirectory
        let nested_path = temp_dir.path().join("src/components/ui");
        let context = discover_from_anywhere(nested_path).await.unwrap();

        assert_eq!(context.name, Some("nested-test".to_string()));
        assert_eq!(context.root.canonicalize().unwrap(), temp_dir.path().canonicalize().unwrap());
    }

    #[tokio::test]
    async fn test_discover_from_anywhere_not_found() {
        let temp_dir = TempDir::new().unwrap();

        // Create directory WITHOUT .dashflow
        std::fs::create_dir(temp_dir.path().join("src")).unwrap();

        // Should fail because no .dashflow/ exists
        let result = discover_from_anywhere(temp_dir.path().join("src")).await;
        assert!(matches!(result, Err(ProjectError::ProjectNotFound(_))));
    }

    #[tokio::test]
    async fn test_discover_from_anywhere_nonexistent_path() {
        let result = discover_from_anywhere(PathBuf::from("/nonexistent/path")).await;
        assert!(matches!(result, Err(ProjectError::PathNotFound(_))));
    }

    #[test]
    fn test_discover_to_root_finds_nearest_first() {
        let temp_dir = TempDir::new().unwrap();
        let root = temp_dir.path();
        std::fs::create_dir_all(root.join("src/components/ui")).unwrap();

        std::fs::write(root.join("AGENTS.md"), "root agents").unwrap();
        std::fs::create_dir_all(root.join("src")).unwrap();
        std::fs::write(root.join("src/AGENTS.md"), "src agents").unwrap();

        let context = ProjectContext::new(root.to_path_buf());
        let found = context.discover_to_root(&root.join("src/components/ui"), "AGENTS.md");

        assert_eq!(found.len(), 2);
        assert_eq!(
            found[0],
            root.join("src/AGENTS.md").canonicalize().unwrap()
        );
        assert_eq!(found[1], root.join("AGENTS.md").canonicalize().unwrap());
    }

    #[test]
    fn test_discover_to_root_multi_groups_by_pattern() {
        let temp_dir = TempDir::new().unwrap();
        let root = temp_dir.path();
        std::fs::create_dir_all(root.join("src/components")).unwrap();

        std::fs::write(root.join("AGENTS.md"), "root agents").unwrap();
        std::fs::write(root.join("CLAUDE.md"), "root claude").unwrap();
        std::fs::write(root.join("src/AGENTS.md"), "src agents").unwrap();

        let context = ProjectContext::new(root.to_path_buf());
        let found = context.discover_to_root_multi(
            &root.join("src/components"),
            &["AGENTS.md", "CLAUDE.md", "MISSING.md"],
        );

        assert_eq!(found.len(), 2);
        let agents = found.get("AGENTS.md").unwrap();
        assert_eq!(agents.len(), 2);
        assert_eq!(agents[0], root.join("src/AGENTS.md").canonicalize().unwrap());
        assert_eq!(agents[1], root.join("AGENTS.md").canonicalize().unwrap());

        let claude = found.get("CLAUDE.md").unwrap();
        assert_eq!(claude.len(), 1);
        assert_eq!(claude[0], root.join("CLAUDE.md").canonicalize().unwrap());
    }

    #[test]
    fn test_detect_monorepo_variants() {
        let temp_dir = TempDir::new().unwrap();
        let root = temp_dir.path();

        std::fs::write(root.join("Cargo.toml"), "[workspace]\nmembers=[]\n").unwrap();
        assert!(detect_monorepo(root));

        std::fs::remove_file(root.join("Cargo.toml")).unwrap();
        std::fs::write(root.join("package.json"), r#"{"workspaces":["*"]}"#).unwrap();
        assert!(detect_monorepo(root));

        std::fs::remove_file(root.join("package.json")).unwrap();
        std::fs::write(root.join("pnpm-workspace.yaml"), "packages:\n  - '*'\n").unwrap();
        assert!(detect_monorepo(root));

        std::fs::remove_file(root.join("pnpm-workspace.yaml")).unwrap();
        std::fs::write(root.join("lerna.json"), r#"{"version":"0.0.0"}"#).unwrap();
        assert!(detect_monorepo(root));

        std::fs::remove_file(root.join("lerna.json")).unwrap();
        std::fs::write(root.join("go.work"), "go 1.22\n").unwrap();
        assert!(detect_monorepo(root));
    }

    #[test]
    fn test_detect_frameworks_from_cargo_toml() {
        let temp_dir = TempDir::new().unwrap();
        let root = temp_dir.path();

        std::fs::write(
            root.join("Cargo.toml"),
            r#"
[package]
name = "test"
version = "0.1.0"

[dependencies]
axum = "0.7"
tokio = { version = "1", features = ["full"] }
"#,
        )
        .unwrap();

        let mut build_systems = HashSet::new();
        build_systems.insert(BuildSystem::Cargo);

        let frameworks = detect_frameworks(root, &build_systems);
        assert!(frameworks.contains(&Framework::Axum));
        assert!(frameworks.contains(&Framework::Tokio));
    }

    #[test]
    fn test_get_documentation_content_truncates_with_header() {
        let temp_dir = TempDir::new().unwrap();
        let root = temp_dir.path();

        let claude_path = root.join("CLAUDE.md");
        std::fs::write(&claude_path, "A".repeat(200)).unwrap();

        let mut context = ProjectContext::new(root.to_path_buf());
        context.documentation.push(Documentation::new(
            DocumentationType::Claude,
            claude_path,
            200,
        ));

        let header_len = format!(
            "\n## {} ({})\n\n",
            DocumentationType::Claude.display_name(),
            context.documentation[0].path.display()
        )
        .len();

        let content = context.get_documentation_content(header_len + 10).unwrap();
        assert!(content.contains(DocumentationType::Claude.display_name()));
        assert!(content.contains("... (truncated)"));
    }

    #[tokio::test]
    async fn test_discover_path_is_file() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("not_a_dir");
        std::fs::write(&file_path, "content").unwrap();

        let result = discover_project(file_path).await;
        assert!(matches!(result, Err(ProjectError::NotDirectory(_))));
    }

    #[tokio::test]
    async fn test_discover_ignores_common_build_artifacts() {
        let temp_dir = TempDir::new().unwrap();
        let root = temp_dir.path();

        std::fs::write(
            root.join("Cargo.toml"),
            r#"[package]
name = "ignore-test"
version = "0.1.0"
"#,
        )
        .unwrap();

        std::fs::create_dir_all(root.join("src")).unwrap();
        std::fs::write(root.join("src/lib.rs"), "pub fn ok() {}").unwrap();

        // Ignored dirs should not contribute language counts.
        std::fs::create_dir_all(root.join("target")).unwrap();
        std::fs::write(root.join("target/ignored.rs"), "pub fn nope() {}").unwrap();
        std::fs::create_dir_all(root.join("node_modules/pkg")).unwrap();
        std::fs::write(root.join("node_modules/pkg/index.js"), "console.log('nope')").unwrap();

        let context = discover_project(root.to_path_buf()).await.unwrap();
        assert_eq!(context.languages.get(&Language::Rust), Some(&1));
        assert!(!context.languages.contains_key(&Language::JavaScript));
    }

    #[tokio::test]
    async fn test_discover_primary_build_system_prefers_cargo() {
        let temp_dir = TempDir::new().unwrap();
        let root = temp_dir.path();

        std::fs::write(
            root.join("Cargo.toml"),
            r#"[package]
name = "priority-test"
version = "0.1.0"
"#,
        )
        .unwrap();
        std::fs::write(root.join("package.json"), r#"{"name":"priority-js"}"#).unwrap();
        std::fs::write(root.join("requirements.txt"), "flask\n").unwrap();

        let context = discover_project(root.to_path_buf()).await.unwrap();
        assert!(context.build_systems.contains(&BuildSystem::Cargo));
        assert!(context.build_systems.contains(&BuildSystem::Npm));
        assert!(context.build_systems.contains(&BuildSystem::Pip));
        assert_eq!(context.primary_build_system, Some(BuildSystem::Cargo));
        assert_eq!(context.name, Some("priority-test".to_string()));
    }

    #[tokio::test]
    async fn test_discover_project_name_from_package_json() {
        let temp_dir = TempDir::new().unwrap();
        let root = temp_dir.path();

        std::fs::write(root.join("package.json"), r#"{"name":"pkg-only"}"#).unwrap();
        let context = discover_project(root.to_path_buf()).await.unwrap();
        assert_eq!(context.name, Some("pkg-only".to_string()));
        assert!(context.build_systems.contains(&BuildSystem::Npm));
    }

    #[tokio::test]
    async fn test_discover_project_name_from_pyproject_project() {
        let temp_dir = TempDir::new().unwrap();
        let root = temp_dir.path();

        std::fs::write(
            root.join("pyproject.toml"),
            r#"[project]
name = "pyproject-project"
version = "0.1.0"
"#,
        )
        .unwrap();

        let context = discover_project(root.to_path_buf()).await.unwrap();
        assert_eq!(context.name, Some("pyproject-project".to_string()));
        assert!(context.build_systems.contains(&BuildSystem::Poetry));
    }

    #[tokio::test]
    async fn test_discover_project_name_from_poetry_section() {
        let temp_dir = TempDir::new().unwrap();
        let root = temp_dir.path();

        std::fs::write(
            root.join("pyproject.toml"),
            r#"[tool.poetry]
name = "poetry-project"
version = "0.1.0"
"#,
        )
        .unwrap();

        let context = discover_project(root.to_path_buf()).await.unwrap();
        assert_eq!(context.name, Some("poetry-project".to_string()));
        assert!(context.build_systems.contains(&BuildSystem::Poetry));
    }

    #[test]
    fn test_extract_toml_name_single_quotes_and_comments() {
        let content = r#"
[package]
name = 'single-quoted' # trailing comment
version = "0.1.0"
"#;
        assert_eq!(
            extract_toml_name(content),
            Some("single-quoted".to_string())
        );
    }

    #[test]
    fn test_extract_toml_name_bare_token() {
        let content = r#"
[package]
name = bare-token
version = "0.1.0"
"#;
        assert_eq!(extract_toml_name(content), Some("bare-token".to_string()));
    }

    #[test]
    fn test_extract_toml_name_empty_string_is_none() {
        let content = r#"
[package]
name = ""
version = "0.1.0"
"#;
        assert_eq!(extract_toml_name(content), None);
    }

    #[test]
    fn test_detect_frameworks_python_requirements() {
        let temp_dir = TempDir::new().unwrap();
        let root = temp_dir.path();

        std::fs::write(
            root.join("requirements.txt"),
            "Django\nflask==2.0\nfastapi\npytorch\nTensorFlow\n",
        )
        .unwrap();

        let mut build_systems = HashSet::new();
        build_systems.insert(BuildSystem::Pip);

        let frameworks = detect_frameworks(root, &build_systems);
        assert!(frameworks.contains(&Framework::Django));
        assert!(frameworks.contains(&Framework::Flask));
        assert!(frameworks.contains(&Framework::FastAPI));
        assert!(frameworks.contains(&Framework::PyTorch));
        assert!(frameworks.contains(&Framework::TensorFlow));
    }

    #[test]
    fn test_detect_frameworks_python_torch_alias() {
        let temp_dir = TempDir::new().unwrap();
        let root = temp_dir.path();

        std::fs::write(root.join("requirements.txt"), "torch==2.0\n").unwrap();

        let mut build_systems = HashSet::new();
        build_systems.insert(BuildSystem::Pip);

        let frameworks = detect_frameworks(root, &build_systems);
        assert!(frameworks.contains(&Framework::PyTorch));
    }

    #[test]
    fn test_detect_frameworks_node_package_json() {
        let temp_dir = TempDir::new().unwrap();
        let root = temp_dir.path();

        std::fs::write(
            root.join("package.json"),
            r#"{
  "name": "webapp",
  "dependencies": {
    "react": "18.0.0",
    "vue": "3.0.0",
    "@angular/core": "17.0.0",
    "next": "14.0.0",
    "express": "4.0.0",
    "@nestjs/core": "10.0.0"
  }
}"#,
        )
        .unwrap();

        let mut build_systems = HashSet::new();
        build_systems.insert(BuildSystem::Npm);

        let frameworks = detect_frameworks(root, &build_systems);
        assert!(frameworks.contains(&Framework::React));
        assert!(frameworks.contains(&Framework::Vue));
        assert!(frameworks.contains(&Framework::Angular));
        assert!(frameworks.contains(&Framework::NextJs));
        assert!(frameworks.contains(&Framework::Express));
        assert!(frameworks.contains(&Framework::NestJs));
    }

    #[test]
    fn test_detect_frameworks_rust_multiple() {
        let temp_dir = TempDir::new().unwrap();
        let root = temp_dir.path();

        std::fs::write(
            root.join("Cargo.toml"),
            r#"
[package]
name = "test"
version = "0.1.0"

[dependencies]
actix-web = "4"
rocket = "0.5"
warp = "0.3"
"#,
        )
        .unwrap();

        let mut build_systems = HashSet::new();
        build_systems.insert(BuildSystem::Cargo);

        let frameworks = detect_frameworks(root, &build_systems);
        assert!(frameworks.contains(&Framework::Actix));
        assert!(frameworks.contains(&Framework::Rocket));
        assert!(frameworks.contains(&Framework::Warp));
    }

    #[test]
    fn test_get_documentation_orders_by_priority() {
        let temp_dir = TempDir::new().unwrap();
        let root = temp_dir.path();

        let claude = root.join("CLAUDE.md");
        let readme = root.join("README.md");
        std::fs::write(&claude, "claude").unwrap();
        std::fs::write(&readme, "readme").unwrap();

        let mut context = ProjectContext::new(root.to_path_buf());
        context.documentation.push(Documentation::new(
            DocumentationType::Readme,
            readme,
            6,
        ));
        context.documentation.push(Documentation::new(
            DocumentationType::Claude,
            claude,
            6,
        ));

        let docs = context.get_documentation().unwrap();
        assert_eq!(docs.len(), 2);
        assert_eq!(docs[0].doc_type, DocumentationType::Claude);
        assert_eq!(docs[1].doc_type, DocumentationType::Readme);
    }

    #[test]
    fn test_get_documentation_content_zero_budget() {
        let temp_dir = TempDir::new().unwrap();
        let root = temp_dir.path();
        let claude = root.join("CLAUDE.md");
        std::fs::write(&claude, "x").unwrap();

        let mut context = ProjectContext::new(root.to_path_buf());
        context.documentation.push(Documentation::new(
            DocumentationType::Claude,
            claude,
            1,
        ));

        let content = context.get_documentation_content(0).unwrap();
        assert!(content.is_empty());
    }

    #[test]
    fn test_get_documentation_content_respects_priority() {
        let temp_dir = TempDir::new().unwrap();
        let root = temp_dir.path();

        let claude = root.join("CLAUDE.md");
        let readme = root.join("README.md");
        std::fs::write(&claude, "claude").unwrap();
        std::fs::write(&readme, "readme").unwrap();

        let mut context = ProjectContext::new(root.to_path_buf());
        context.documentation.push(Documentation::new(
            DocumentationType::Readme,
            readme,
            6,
        ));
        context.documentation.push(Documentation::new(
            DocumentationType::Claude,
            claude,
            6,
        ));

        let content = context.get_documentation_content(10_000).unwrap();
        let claude_pos = content.find("Claude Instructions").unwrap();
        let readme_pos = content.find("README").unwrap();
        assert!(claude_pos < readme_pos);
    }

    #[test]
    fn test_discover_to_root_outside_root_is_empty() {
        let root_dir = TempDir::new().unwrap();
        let other_dir = TempDir::new().unwrap();

        std::fs::write(root_dir.path().join("AGENTS.md"), "root").unwrap();

        let context = ProjectContext::new(root_dir.path().to_path_buf());
        let found = context.discover_to_root(other_dir.path(), "AGENTS.md");
        assert!(found.is_empty());
    }

    #[test]
    fn test_discover_to_root_multi_outside_root_is_empty() {
        let root_dir = TempDir::new().unwrap();
        let other_dir = TempDir::new().unwrap();

        std::fs::write(root_dir.path().join("AGENTS.md"), "root").unwrap();
        std::fs::write(root_dir.path().join("CLAUDE.md"), "root").unwrap();

        let context = ProjectContext::new(root_dir.path().to_path_buf());
        let found = context.discover_to_root_multi(
            other_dir.path(),
            &["AGENTS.md", "CLAUDE.md"],
        );
        assert!(found.is_empty());
    }

    #[test]
    fn test_is_ignored_additional_dirs() {
        assert!(is_ignored(Path::new("/project/.cargo")));
        assert!(is_ignored(Path::new("/project/.rustup")));
        assert!(is_ignored(Path::new("/project/.venv")));
        assert!(is_ignored(Path::new("/project/dist")));
        assert!(is_ignored(Path::new("/project/build")));
        assert!(!is_ignored(Path::new("/project/docs")));
    }

    #[test]
    fn test_summary_contains_expected_lines() {
        let mut context = ProjectContext::new(PathBuf::from("/repo"));
        context.name = Some("my-project".to_string());
        context.primary_language = Some(Language::Rust);
        context.total_files = 42;
        context.has_git = true;
        context.is_monorepo = true;

        let summary = context.summary();
        assert!(summary.contains("Project: my-project"));
        assert!(summary.contains("Primary Language: Rust"));
        assert!(summary.contains("Version Control: Git"));
        assert!(summary.contains("Structure: Monorepo"));
        assert!(summary.contains("Files: 42"));
    }
}
