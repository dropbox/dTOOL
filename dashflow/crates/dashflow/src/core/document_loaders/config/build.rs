//! Build system configuration loaders.
//!
//! This module provides loaders for build system configuration files:
//! - **`DockerfileLoader`**: Container definition files (Dockerfile)
//! - **`MakefileLoader`**: Make build automation files (Makefile, `GNUmakefile`)
//! - **`CMakeLoader`**: `CMake` build system files (CMakeLists.txt, *.cmake)
//!
//! Each loader can parse and extract structured information from build configuration files,
//! with options to separate content by logical sections (stages, targets, commands).
//!
//! © 2025 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

use async_trait::async_trait;
use std::path::{Path, PathBuf};

use crate::core::documents::{Blob, Document, DocumentLoader};
use crate::core::error::Result;

/// Loads Dockerfile container definition files.
///
/// The `DockerfileLoader` reads Dockerfiles used to build container images.
/// Can optionally separate by stage in multi-stage builds.
///
/// # Example
///
/// ```no_run
/// use dashflow::core::document_loaders::DockerfileLoader;
/// use dashflow::core::documents::DocumentLoader;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let loader = DockerfileLoader::new("Dockerfile");
/// let documents = loader.load().await?;
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone)]
pub struct DockerfileLoader {
    /// Path to the Dockerfile
    pub file_path: PathBuf,
    /// Separate documents per build stage (default: false)
    pub separate_stages: bool,
}

impl DockerfileLoader {
    /// Create a new `DockerfileLoader` for the given file path.
    pub fn new(file_path: impl AsRef<Path>) -> Self {
        Self {
            file_path: file_path.as_ref().to_path_buf(),
            separate_stages: false,
        }
    }

    /// Create separate documents per build stage (FROM instructions).
    #[must_use]
    pub const fn with_separate_stages(mut self, separate: bool) -> Self {
        self.separate_stages = separate;
        self
    }
}

#[async_trait]
impl DocumentLoader for DockerfileLoader {
    async fn load(&self) -> Result<Vec<Document>> {
        let blob = Blob::from_path(&self.file_path);
        let content = blob.as_string()?;

        if self.separate_stages {
            // Split by FROM instructions (build stages)
            let mut documents = Vec::new();
            let mut current_stage = String::new();
            let mut stage_name = String::new();
            let mut stage_index = 0;

            for line in content.lines() {
                let trimmed = line.trim();

                if trimmed.starts_with("FROM ") {
                    // Save previous stage
                    if !current_stage.is_empty() {
                        let doc = Document::new(current_stage.clone())
                            .with_metadata("source", self.file_path.display().to_string())
                            .with_metadata("stage_index", stage_index)
                            .with_metadata("stage_name", stage_name.clone())
                            .with_metadata("format", "dockerfile");

                        documents.push(doc);
                        current_stage.clear();
                        stage_index += 1;
                    }

                    // Extract stage name if present (FROM image AS name)
                    if let Some(as_pos) = trimmed.to_uppercase().find(" AS ") {
                        let after_as = &trimmed[as_pos + 4..];
                        stage_name = after_as
                            .split_whitespace()
                            .next()
                            .unwrap_or("unnamed")
                            .to_string();
                    } else {
                        stage_name = format!("stage-{stage_index}");
                    }
                }

                current_stage.push_str(line);
                current_stage.push('\n');
            }

            // Add last stage
            if !current_stage.is_empty() {
                let doc = Document::new(current_stage)
                    .with_metadata("source", self.file_path.display().to_string())
                    .with_metadata("stage_index", stage_index)
                    .with_metadata("stage_name", stage_name)
                    .with_metadata("format", "dockerfile");

                documents.push(doc);
            }

            Ok(documents)
        } else {
            // Load as single document
            let doc = Document::new(content)
                .with_metadata("source", self.file_path.display().to_string())
                .with_metadata("format", "dockerfile");

            Ok(vec![doc])
        }
    }
}

/// Loads Makefile build script files.
///
/// The `MakefileLoader` reads Makefiles used for build automation.
/// Can optionally separate by target definitions.
///
/// # Example
///
/// ```no_run
/// use dashflow::core::document_loaders::MakefileLoader;
/// use dashflow::core::documents::DocumentLoader;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let loader = MakefileLoader::new("Makefile");
/// let documents = loader.load().await?;
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone)]
pub struct MakefileLoader {
    /// Path to the Makefile
    pub file_path: PathBuf,
    /// Separate documents per target (default: false)
    pub separate_targets: bool,
}

impl MakefileLoader {
    /// Create a new `MakefileLoader` for the given file path.
    pub fn new(file_path: impl AsRef<Path>) -> Self {
        Self {
            file_path: file_path.as_ref().to_path_buf(),
            separate_targets: false,
        }
    }

    /// Create separate documents per Makefile target.
    #[must_use]
    pub const fn with_separate_targets(mut self, separate: bool) -> Self {
        self.separate_targets = separate;
        self
    }
}

#[async_trait]
impl DocumentLoader for MakefileLoader {
    async fn load(&self) -> Result<Vec<Document>> {
        let blob = Blob::from_path(&self.file_path);
        let content = blob.as_string()?;

        if self.separate_targets {
            // Split by target definitions (lines ending with :)
            let mut documents = Vec::new();
            let mut current_target = String::new();
            let mut target_name = String::new();
            let mut target_index = 0;
            let mut in_target = false;

            for line in content.lines() {
                // Check if this is a target definition (not indented, ends with :)
                if !line.is_empty()
                    && !line.starts_with('\t')
                    && !line.starts_with(' ')
                    && line.contains(':')
                {
                    // Save previous target
                    if in_target && !current_target.is_empty() {
                        let doc = Document::new(current_target.clone())
                            .with_metadata("source", self.file_path.display().to_string())
                            .with_metadata("target_index", target_index)
                            .with_metadata("target_name", target_name.clone())
                            .with_metadata("format", "makefile");

                        documents.push(doc);
                        current_target.clear();
                        target_index += 1;
                    }

                    // Start new target (skip special targets like .PHONY)
                    if !line.trim().starts_with('.') || line.contains(".PHONY") {
                        in_target = true;
                        // Extract target name (everything before first :)
                        if let Some(colon_pos) = line.find(':') {
                            target_name = line[..colon_pos].trim().to_string();
                        }
                    }
                }

                if in_target {
                    current_target.push_str(line);
                    current_target.push('\n');
                }
            }

            // Add last target
            if !current_target.is_empty() {
                let doc = Document::new(current_target)
                    .with_metadata("source", self.file_path.display().to_string())
                    .with_metadata("target_index", target_index)
                    .with_metadata("target_name", target_name)
                    .with_metadata("format", "makefile");

                documents.push(doc);
            }

            Ok(documents)
        } else {
            // Load as single document
            let doc = Document::new(content)
                .with_metadata("source", self.file_path.display().to_string())
                .with_metadata("format", "makefile");

            Ok(vec![doc])
        }
    }
}

/// Loader for `CMake` build system files (CMakeLists.txt, *.cmake).
///
/// `CMake` is a cross-platform build system generator created by Kitware in 2000.
/// Uses declarative configuration language for C/C++ projects. Industry standard.
///
/// # Example
///
/// ```rust,no_run
/// # use dashflow::core::document_loaders::CMakeLoader;
/// # use dashflow::core::documents::DocumentLoader;
/// # async fn example() -> dashflow::core::error::Result<()> {
/// let loader = CMakeLoader::new("CMakeLists.txt");
/// let docs = loader.load().await?;
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone)]
pub struct CMakeLoader {
    file_path: PathBuf,
    separate_commands: bool,
}

impl CMakeLoader {
    /// Create a new `CMake` loader for the given file path.
    pub fn new<P: AsRef<Path>>(path: P) -> Self {
        Self {
            file_path: path.as_ref().to_path_buf(),
            separate_commands: false,
        }
    }

    /// Enable separation by major `CMake` commands (project, `add_executable`, `add_library`, etc.).
    #[must_use]
    pub const fn with_separate_commands(mut self) -> Self {
        self.separate_commands = true;
        self
    }

    /// Extract command name from line
    fn extract_command_name(line: &str) -> String {
        if let Some(paren_pos) = line.find('(') {
            line[..paren_pos].trim().to_string()
        } else {
            line.trim().to_string()
        }
    }
}

#[async_trait]
impl DocumentLoader for CMakeLoader {
    async fn load(&self) -> Result<Vec<Document>> {
        let blob = Blob::from_path(&self.file_path);
        let content = blob.as_string()?;

        if !self.separate_commands {
            return Ok(vec![Document::new(&content)
                .with_metadata("source", self.file_path.display().to_string())
                .with_metadata("format", "cmake")]);
        }

        // Separate by major CMake commands
        let lines: Vec<&str> = content.lines().collect();
        let mut documents = Vec::new();
        let mut i = 0;
        let mut command_index = 0;

        // Major CMake commands to separate
        let major_commands = [
            "project(",
            "add_executable(",
            "add_library(",
            "target_link_libraries(",
            "target_include_directories(",
            "target_compile_options(",
            "target_compile_definitions(",
            "find_package(",
            "add_subdirectory(",
            "set(",
            "option(",
            "include(",
            "function(",
            "macro(",
            "if(",
            "foreach(",
            "while(",
        ];

        while i < lines.len() {
            let line = lines[i];
            let trimmed = line.trim();

            // Skip empty lines and comments
            if trimmed.is_empty() || trimmed.starts_with('#') {
                i += 1;
                continue;
            }

            // Check if line starts with a major command
            let is_major_command = major_commands
                .iter()
                .any(|cmd| trimmed.to_lowercase().starts_with(&cmd.to_lowercase()));

            if is_major_command {
                let command_name = Self::extract_command_name(trimmed);
                let mut command_lines = vec![lines[i]];
                i += 1;

                // CMake commands can span multiple lines until closing paren
                let mut paren_count =
                    trimmed.matches('(').count() as i32 - trimmed.matches(')').count() as i32;

                while i < lines.len() && paren_count > 0 {
                    let next_line = lines[i];
                    command_lines.push(next_line);

                    let next_trimmed = next_line.trim();
                    if !next_trimmed.starts_with('#') {
                        paren_count += next_line.matches('(').count() as i32
                            - next_line.matches(')').count() as i32;
                    }

                    i += 1;
                }

                // For control flow commands (if, foreach, while, function, macro), collect until end
                let lowercase_name = command_name.to_lowercase();
                if lowercase_name == "if"
                    || lowercase_name == "foreach"
                    || lowercase_name == "while"
                    || lowercase_name == "function"
                    || lowercase_name == "macro"
                {
                    let end_keyword = format!("end{lowercase_name}(");

                    while i < lines.len() {
                        let next_line = lines[i];
                        command_lines.push(next_line);

                        if next_line.trim().to_lowercase().starts_with(&end_keyword) {
                            i += 1;
                            break;
                        }

                        i += 1;
                    }
                }

                let command_content = command_lines.join("\n");
                documents.push(
                    Document::new(&command_content)
                        .with_metadata("source", self.file_path.display().to_string())
                        .with_metadata("format", "cmake")
                        .with_metadata("command_index", command_index.to_string())
                        .with_metadata("command_name", command_name),
                );
                command_index += 1;
            } else {
                i += 1;
            }
        }

        if documents.is_empty() {
            Ok(vec![Document::new(&content)
                .with_metadata("source", self.file_path.display().to_string())
                .with_metadata("format", "cmake")])
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
    // DockerfileLoader Tests
    // ==========================================================================

    #[test]
    fn test_dockerfile_loader_new() {
        let loader = DockerfileLoader::new("Dockerfile");
        assert_eq!(loader.file_path, PathBuf::from("Dockerfile"));
        assert!(!loader.separate_stages);
    }

    #[test]
    fn test_dockerfile_loader_new_from_pathbuf() {
        let path = PathBuf::from("/app/Dockerfile.production");
        let loader = DockerfileLoader::new(&path);
        assert_eq!(loader.file_path, path);
    }

    #[test]
    fn test_dockerfile_loader_with_separate_stages_true() {
        let loader = DockerfileLoader::new("Dockerfile").with_separate_stages(true);
        assert!(loader.separate_stages);
    }

    #[test]
    fn test_dockerfile_loader_with_separate_stages_false() {
        let loader = DockerfileLoader::new("Dockerfile").with_separate_stages(false);
        assert!(!loader.separate_stages);
    }

    #[test]
    fn test_dockerfile_loader_debug_clone() {
        let loader = DockerfileLoader::new("Dockerfile");
        let cloned = loader.clone();
        assert_eq!(loader.file_path, cloned.file_path);
        assert_eq!(loader.separate_stages, cloned.separate_stages);

        let debug_str = format!("{:?}", loader);
        assert!(debug_str.contains("DockerfileLoader"));
        assert!(debug_str.contains("Dockerfile"));
    }

    #[tokio::test]
    async fn test_dockerfile_loader_load_basic() {
        let mut temp_file = NamedTempFile::new().unwrap();
        let content = "FROM ubuntu:22.04\nRUN apt-get update\nCOPY . /app\nCMD [\"./app\"]";
        temp_file.write_all(content.as_bytes()).unwrap();

        let loader = DockerfileLoader::new(temp_file.path());
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1);
        assert!(docs[0].page_content.contains("FROM ubuntu:22.04"));
        assert!(docs[0].page_content.contains("RUN apt-get"));
        assert_eq!(
            docs[0].metadata.get("format").and_then(|v| v.as_str()),
            Some("dockerfile")
        );
    }

    #[tokio::test]
    async fn test_dockerfile_loader_single_stage_separate_mode() {
        let mut temp_file = NamedTempFile::new().unwrap();
        let content = "FROM node:18\nWORKDIR /app\nCOPY package.json .\nRUN npm install";
        temp_file.write_all(content.as_bytes()).unwrap();

        let loader = DockerfileLoader::new(temp_file.path()).with_separate_stages(true);
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1);
        assert_eq!(
            docs[0].metadata.get("stage_index").and_then(|v| v.as_i64()),
            Some(0)
        );
        // No AS clause, so should have auto-generated name
        assert_eq!(
            docs[0].metadata.get("stage_name").and_then(|v| v.as_str()),
            Some("stage-0")
        );
    }

    #[tokio::test]
    async fn test_dockerfile_loader_multi_stage_unnamed() {
        let mut temp_file = NamedTempFile::new().unwrap();
        let content = "FROM node:18\nRUN npm install\n\nFROM nginx:alpine\nCOPY --from=0 /app /usr/share/nginx/html";
        temp_file.write_all(content.as_bytes()).unwrap();

        let loader = DockerfileLoader::new(temp_file.path()).with_separate_stages(true);
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 2);
        assert_eq!(
            docs[0].metadata.get("stage_name").and_then(|v| v.as_str()),
            Some("stage-0")
        );
        assert_eq!(
            docs[1].metadata.get("stage_name").and_then(|v| v.as_str()),
            Some("stage-1")
        );
    }

    #[tokio::test]
    async fn test_dockerfile_loader_multi_stage_named() {
        let mut temp_file = NamedTempFile::new().unwrap();
        let content = "FROM node:18 AS builder\nRUN npm run build\n\nFROM nginx:alpine AS production\nCOPY --from=builder /app/dist /usr/share/nginx/html";
        temp_file.write_all(content.as_bytes()).unwrap();

        let loader = DockerfileLoader::new(temp_file.path()).with_separate_stages(true);
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 2);
        assert_eq!(
            docs[0].metadata.get("stage_name").and_then(|v| v.as_str()),
            Some("builder")
        );
        assert_eq!(
            docs[1].metadata.get("stage_name").and_then(|v| v.as_str()),
            Some("production")
        );
    }

    #[tokio::test]
    async fn test_dockerfile_loader_mixed_case_as() {
        let mut temp_file = NamedTempFile::new().unwrap();
        let content = "FROM ubuntu:22.04 as mybase\nRUN echo test";
        temp_file.write_all(content.as_bytes()).unwrap();

        let loader = DockerfileLoader::new(temp_file.path()).with_separate_stages(true);
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1);
        assert_eq!(
            docs[0].metadata.get("stage_name").and_then(|v| v.as_str()),
            Some("mybase")
        );
    }

    #[tokio::test]
    async fn test_dockerfile_loader_empty_file() {
        let temp_file = NamedTempFile::new().unwrap();

        let loader = DockerfileLoader::new(temp_file.path());
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1);
        assert!(docs[0].page_content.is_empty());
    }

    #[tokio::test]
    async fn test_dockerfile_loader_source_metadata() {
        let mut temp_file = NamedTempFile::new().unwrap();
        temp_file.write_all(b"FROM alpine").unwrap();

        let loader = DockerfileLoader::new(temp_file.path());
        let docs = loader.load().await.unwrap();

        assert!(docs[0].metadata.get("source").is_some());
    }

    // ==========================================================================
    // MakefileLoader Tests
    // ==========================================================================

    #[test]
    fn test_makefile_loader_new() {
        let loader = MakefileLoader::new("Makefile");
        assert_eq!(loader.file_path, PathBuf::from("Makefile"));
        assert!(!loader.separate_targets);
    }

    #[test]
    fn test_makefile_loader_new_gnumakefile() {
        let loader = MakefileLoader::new("GNUmakefile");
        assert_eq!(loader.file_path, PathBuf::from("GNUmakefile"));
    }

    #[test]
    fn test_makefile_loader_with_separate_targets_true() {
        let loader = MakefileLoader::new("Makefile").with_separate_targets(true);
        assert!(loader.separate_targets);
    }

    #[test]
    fn test_makefile_loader_with_separate_targets_false() {
        let loader = MakefileLoader::new("Makefile").with_separate_targets(false);
        assert!(!loader.separate_targets);
    }

    #[test]
    fn test_makefile_loader_debug_clone() {
        let loader = MakefileLoader::new("Makefile");
        let cloned = loader.clone();
        assert_eq!(loader.file_path, cloned.file_path);
        assert_eq!(loader.separate_targets, cloned.separate_targets);

        let debug_str = format!("{:?}", loader);
        assert!(debug_str.contains("MakefileLoader"));
        assert!(debug_str.contains("Makefile"));
    }

    #[tokio::test]
    async fn test_makefile_loader_load_basic() {
        let mut temp_file = NamedTempFile::new().unwrap();
        let content = "CC=gcc\nCFLAGS=-Wall\n\nall:\n\t$(CC) main.c -o main";
        temp_file.write_all(content.as_bytes()).unwrap();

        let loader = MakefileLoader::new(temp_file.path());
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1);
        assert!(docs[0].page_content.contains("CC=gcc"));
        assert!(docs[0].page_content.contains("all:"));
        assert_eq!(
            docs[0].metadata.get("format").and_then(|v| v.as_str()),
            Some("makefile")
        );
    }

    #[tokio::test]
    async fn test_makefile_loader_separate_targets() {
        let mut temp_file = NamedTempFile::new().unwrap();
        let content = "all:\n\techo all\n\nbuild:\n\techo build\n\ntest:\n\techo test";
        temp_file.write_all(content.as_bytes()).unwrap();

        let loader = MakefileLoader::new(temp_file.path()).with_separate_targets(true);
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 3);
        assert_eq!(
            docs[0].metadata.get("target_name").and_then(|v| v.as_str()),
            Some("all")
        );
        assert_eq!(
            docs[1].metadata.get("target_name").and_then(|v| v.as_str()),
            Some("build")
        );
        assert_eq!(
            docs[2].metadata.get("target_name").and_then(|v| v.as_str()),
            Some("test")
        );
    }

    #[tokio::test]
    async fn test_makefile_loader_target_with_dependencies() {
        let mut temp_file = NamedTempFile::new().unwrap();
        let content = "build: src/main.c src/utils.c\n\tgcc -o app src/*.c\n\nclean:\n\trm -f app";
        temp_file.write_all(content.as_bytes()).unwrap();

        let loader = MakefileLoader::new(temp_file.path()).with_separate_targets(true);
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 2);
        assert_eq!(
            docs[0].metadata.get("target_name").and_then(|v| v.as_str()),
            Some("build")
        );
        assert!(docs[0].page_content.contains("src/main.c"));
    }

    #[tokio::test]
    async fn test_makefile_loader_phony_target() {
        let mut temp_file = NamedTempFile::new().unwrap();
        let content = ".PHONY: all clean\n\nall:\n\techo all\n\nclean:\n\trm -f *.o";
        temp_file.write_all(content.as_bytes()).unwrap();

        let loader = MakefileLoader::new(temp_file.path()).with_separate_targets(true);
        let docs = loader.load().await.unwrap();

        // .PHONY is included because it contains ".PHONY"
        assert!(docs.len() >= 2);
    }

    #[tokio::test]
    async fn test_makefile_loader_target_indices() {
        let mut temp_file = NamedTempFile::new().unwrap();
        let content = "first:\n\techo 1\n\nsecond:\n\techo 2";
        temp_file.write_all(content.as_bytes()).unwrap();

        let loader = MakefileLoader::new(temp_file.path()).with_separate_targets(true);
        let docs = loader.load().await.unwrap();

        assert_eq!(
            docs[0].metadata.get("target_index").and_then(|v| v.as_i64()),
            Some(0)
        );
        assert_eq!(
            docs[1].metadata.get("target_index").and_then(|v| v.as_i64()),
            Some(1)
        );
    }

    #[tokio::test]
    async fn test_makefile_loader_empty_file() {
        let temp_file = NamedTempFile::new().unwrap();

        let loader = MakefileLoader::new(temp_file.path());
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1);
        assert!(docs[0].page_content.is_empty());
    }

    #[tokio::test]
    async fn test_makefile_loader_no_targets_separate_mode() {
        let mut temp_file = NamedTempFile::new().unwrap();
        let content = "# Variables only\nCC=gcc\nCFLAGS=-Wall";
        temp_file.write_all(content.as_bytes()).unwrap();

        let loader = MakefileLoader::new(temp_file.path()).with_separate_targets(true);
        let docs = loader.load().await.unwrap();

        // Without any targets (lines with :), separate mode returns empty list
        // This tests that variable assignments (=) are not mistaken for targets (:)
        assert!(docs.is_empty());
    }

    // ==========================================================================
    // CMakeLoader Tests
    // ==========================================================================

    #[test]
    fn test_cmake_loader_new() {
        let loader = CMakeLoader::new("CMakeLists.txt");
        assert_eq!(loader.file_path, PathBuf::from("CMakeLists.txt"));
        assert!(!loader.separate_commands);
    }

    #[test]
    fn test_cmake_loader_new_from_cmake_file() {
        let loader = CMakeLoader::new("config.cmake");
        assert_eq!(loader.file_path, PathBuf::from("config.cmake"));
    }

    #[test]
    fn test_cmake_loader_with_separate_commands() {
        let loader = CMakeLoader::new("CMakeLists.txt").with_separate_commands();
        assert!(loader.separate_commands);
    }

    #[test]
    fn test_cmake_loader_debug_clone() {
        let loader = CMakeLoader::new("CMakeLists.txt");
        let cloned = loader.clone();
        assert_eq!(loader.file_path, cloned.file_path);
        assert_eq!(loader.separate_commands, cloned.separate_commands);

        let debug_str = format!("{:?}", loader);
        assert!(debug_str.contains("CMakeLoader"));
        assert!(debug_str.contains("CMakeLists.txt"));
    }

    #[test]
    fn test_cmake_extract_command_name_simple() {
        let name = CMakeLoader::extract_command_name("project(MyProject)");
        assert_eq!(name, "project");
    }

    #[test]
    fn test_cmake_extract_command_name_with_spaces() {
        let name = CMakeLoader::extract_command_name("  add_executable(main main.cpp)  ");
        assert_eq!(name, "add_executable");
    }

    #[test]
    fn test_cmake_extract_command_name_no_parens() {
        let name = CMakeLoader::extract_command_name("some_text");
        assert_eq!(name, "some_text");
    }

    #[test]
    fn test_cmake_extract_command_name_empty() {
        let name = CMakeLoader::extract_command_name("");
        assert_eq!(name, "");
    }

    #[tokio::test]
    async fn test_cmake_loader_load_basic() {
        let mut temp_file = NamedTempFile::new().unwrap();
        let content = "cmake_minimum_required(VERSION 3.20)\nproject(MyProject)\nadd_executable(main main.cpp)";
        temp_file.write_all(content.as_bytes()).unwrap();

        let loader = CMakeLoader::new(temp_file.path());
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1);
        assert!(docs[0].page_content.contains("cmake_minimum_required"));
        assert!(docs[0].page_content.contains("project(MyProject)"));
        assert_eq!(
            docs[0].metadata.get("format").and_then(|v| v.as_str()),
            Some("cmake")
        );
    }

    #[tokio::test]
    async fn test_cmake_loader_separate_commands() {
        let mut temp_file = NamedTempFile::new().unwrap();
        let content = "project(MyProject)\n\nadd_executable(main main.cpp)\n\nadd_library(utils utils.cpp)";
        temp_file.write_all(content.as_bytes()).unwrap();

        let loader = CMakeLoader::new(temp_file.path()).with_separate_commands();
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 3);
        assert_eq!(
            docs[0].metadata.get("command_name").and_then(|v| v.as_str()),
            Some("project")
        );
        assert_eq!(
            docs[1].metadata.get("command_name").and_then(|v| v.as_str()),
            Some("add_executable")
        );
        assert_eq!(
            docs[2].metadata.get("command_name").and_then(|v| v.as_str()),
            Some("add_library")
        );
    }

    #[tokio::test]
    async fn test_cmake_loader_multiline_command() {
        let mut temp_file = NamedTempFile::new().unwrap();
        let content = "add_executable(main\n    main.cpp\n    utils.cpp\n    config.cpp\n)";
        temp_file.write_all(content.as_bytes()).unwrap();

        let loader = CMakeLoader::new(temp_file.path()).with_separate_commands();
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1);
        assert!(docs[0].page_content.contains("add_executable"));
        assert!(docs[0].page_content.contains("main.cpp"));
        assert!(docs[0].page_content.contains("config.cpp"));
    }

    #[tokio::test]
    async fn test_cmake_loader_if_block() {
        let mut temp_file = NamedTempFile::new().unwrap();
        let content = "if(WIN32)\n    message(STATUS \"Windows\")\nendif()";
        temp_file.write_all(content.as_bytes()).unwrap();

        let loader = CMakeLoader::new(temp_file.path()).with_separate_commands();
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1);
        assert!(docs[0].page_content.contains("if(WIN32)"));
        assert!(docs[0].page_content.contains("endif()"));
    }

    #[tokio::test]
    async fn test_cmake_loader_function_block() {
        let mut temp_file = NamedTempFile::new().unwrap();
        let content = "function(my_func arg)\n    message(${arg})\nendfunction()";
        temp_file.write_all(content.as_bytes()).unwrap();

        let loader = CMakeLoader::new(temp_file.path()).with_separate_commands();
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1);
        assert!(docs[0].page_content.contains("function(my_func"));
        assert!(docs[0].page_content.contains("endfunction()"));
    }

    #[tokio::test]
    async fn test_cmake_loader_foreach_block() {
        let mut temp_file = NamedTempFile::new().unwrap();
        let content = "foreach(item IN ITEMS a b c)\n    message(${item})\nendforeach()";
        temp_file.write_all(content.as_bytes()).unwrap();

        let loader = CMakeLoader::new(temp_file.path()).with_separate_commands();
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1);
        assert!(docs[0].page_content.contains("foreach(item"));
        assert!(docs[0].page_content.contains("endforeach()"));
    }

    #[tokio::test]
    async fn test_cmake_loader_set_command() {
        let mut temp_file = NamedTempFile::new().unwrap();
        let content = "set(CMAKE_CXX_STANDARD 17)";
        temp_file.write_all(content.as_bytes()).unwrap();

        let loader = CMakeLoader::new(temp_file.path()).with_separate_commands();
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1);
        assert_eq!(
            docs[0].metadata.get("command_name").and_then(|v| v.as_str()),
            Some("set")
        );
    }

    #[tokio::test]
    async fn test_cmake_loader_find_package() {
        let mut temp_file = NamedTempFile::new().unwrap();
        let content = "find_package(Boost REQUIRED COMPONENTS filesystem)";
        temp_file.write_all(content.as_bytes()).unwrap();

        let loader = CMakeLoader::new(temp_file.path()).with_separate_commands();
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1);
        assert_eq!(
            docs[0].metadata.get("command_name").and_then(|v| v.as_str()),
            Some("find_package")
        );
    }

    #[tokio::test]
    async fn test_cmake_loader_comments_skipped() {
        let mut temp_file = NamedTempFile::new().unwrap();
        let content = "# This is a comment\nproject(Test)\n# Another comment";
        temp_file.write_all(content.as_bytes()).unwrap();

        let loader = CMakeLoader::new(temp_file.path()).with_separate_commands();
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1);
        assert_eq!(
            docs[0].metadata.get("command_name").and_then(|v| v.as_str()),
            Some("project")
        );
    }

    #[tokio::test]
    async fn test_cmake_loader_empty_separate_mode() {
        let mut temp_file = NamedTempFile::new().unwrap();
        let content = "# Just comments\n# No commands";
        temp_file.write_all(content.as_bytes()).unwrap();

        let loader = CMakeLoader::new(temp_file.path()).with_separate_commands();
        let docs = loader.load().await.unwrap();

        // Falls back to single document when no commands found
        assert_eq!(docs.len(), 1);
    }

    #[tokio::test]
    async fn test_cmake_loader_empty_file() {
        let temp_file = NamedTempFile::new().unwrap();

        let loader = CMakeLoader::new(temp_file.path());
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1);
        assert!(docs[0].page_content.is_empty());
    }

    #[tokio::test]
    async fn test_cmake_loader_command_index() {
        let mut temp_file = NamedTempFile::new().unwrap();
        let content = "project(A)\nproject(B)";
        temp_file.write_all(content.as_bytes()).unwrap();

        let loader = CMakeLoader::new(temp_file.path()).with_separate_commands();
        let docs = loader.load().await.unwrap();

        assert_eq!(
            docs[0]
                .metadata
                .get("command_index")
                .and_then(|v| v.as_str()),
            Some("0")
        );
        assert_eq!(
            docs[1]
                .metadata
                .get("command_index")
                .and_then(|v| v.as_str()),
            Some("1")
        );
    }

    // ==========================================================================
    // Integration Tests
    // ==========================================================================

    #[tokio::test]
    async fn test_dockerfile_realistic_multistage() {
        let mut temp_file = NamedTempFile::new().unwrap();
        // Note: comments before first FROM create an empty pre-stage document
        let content = r#"FROM rust:1.75 AS builder
WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY src ./src
RUN cargo build --release

FROM debian:bookworm-slim AS runtime
RUN apt-get update && apt-get install -y ca-certificates
COPY --from=builder /app/target/release/myapp /usr/local/bin/
CMD ["myapp"]"#;
        temp_file.write_all(content.as_bytes()).unwrap();

        let loader = DockerfileLoader::new(temp_file.path()).with_separate_stages(true);
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 2);
        assert_eq!(
            docs[0].metadata.get("stage_name").and_then(|v| v.as_str()),
            Some("builder")
        );
        assert_eq!(
            docs[1].metadata.get("stage_name").and_then(|v| v.as_str()),
            Some("runtime")
        );
        assert!(docs[0].page_content.contains("cargo build"));
        assert!(docs[1].page_content.contains("apt-get"));
    }

    #[tokio::test]
    async fn test_makefile_realistic() {
        let mut temp_file = NamedTempFile::new().unwrap();
        let content = r#".PHONY: all clean test

CC = gcc
CFLAGS = -Wall -Werror

all: main

main: main.o utils.o
	$(CC) $(CFLAGS) -o $@ $^

%.o: %.c
	$(CC) $(CFLAGS) -c $<

clean:
	rm -f *.o main

test: main
	./main --test"#;
        temp_file.write_all(content.as_bytes()).unwrap();

        let loader = MakefileLoader::new(temp_file.path());
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1);
        assert!(docs[0].page_content.contains("$(CC)"));
        assert!(docs[0].page_content.contains("-Wall"));
    }

    #[tokio::test]
    async fn test_cmake_realistic() {
        let mut temp_file = NamedTempFile::new().unwrap();
        let content = r#"cmake_minimum_required(VERSION 3.20)
project(MyApp VERSION 1.0.0 LANGUAGES CXX)

set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)

find_package(Boost REQUIRED COMPONENTS filesystem system)

add_executable(myapp
    src/main.cpp
    src/utils.cpp
)

target_link_libraries(myapp
    PRIVATE
        Boost::filesystem
        Boost::system
)

if(BUILD_TESTS)
    enable_testing()
    add_subdirectory(tests)
endif()"#;
        temp_file.write_all(content.as_bytes()).unwrap();

        let loader = CMakeLoader::new(temp_file.path()).with_separate_commands();
        let docs = loader.load().await.unwrap();

        // Should separate into multiple commands
        assert!(docs.len() >= 5);

        // Verify some command names
        let command_names: Vec<_> = docs
            .iter()
            .filter_map(|d| d.metadata.get("command_name").and_then(|v| v.as_str()))
            .collect();

        assert!(command_names.contains(&"project"));
        assert!(command_names.contains(&"set"));
        assert!(command_names.contains(&"find_package"));
        assert!(command_names.contains(&"add_executable"));
    }
}
