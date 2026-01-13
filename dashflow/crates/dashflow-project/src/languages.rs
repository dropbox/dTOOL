// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)
// DashFlow Project - Language, Framework, and Build System Detection

//! # Language Detection
//!
//! Types and detection logic for programming languages, frameworks, and build systems.

use serde::{Deserialize, Serialize};

/// Programming languages detected in a project
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Language {
    Rust,
    Python,
    TypeScript,
    JavaScript,
    Go,
    Java,
    CSharp,
    Cpp,
    C,
    Ruby,
    PHP,
    Swift,
    Kotlin,
    Scala,
    Shell,
    Markdown,
    Unknown,
}

impl Language {
    /// Detect language from file extension
    pub fn from_extension(ext: &str) -> Option<Self> {
        match ext.to_lowercase().as_str() {
            "rs" => Some(Language::Rust),
            "py" | "pyw" | "pyi" => Some(Language::Python),
            "ts" | "tsx" => Some(Language::TypeScript),
            "js" | "jsx" | "mjs" | "cjs" => Some(Language::JavaScript),
            "go" => Some(Language::Go),
            "java" => Some(Language::Java),
            "cs" => Some(Language::CSharp),
            "cpp" | "cc" | "cxx" | "hpp" | "hxx" => Some(Language::Cpp),
            "c" | "h" => Some(Language::C),
            "rb" | "rake" => Some(Language::Ruby),
            "php" => Some(Language::PHP),
            "swift" => Some(Language::Swift),
            "kt" | "kts" => Some(Language::Kotlin),
            "scala" | "sc" => Some(Language::Scala),
            "sh" | "bash" | "zsh" | "fish" => Some(Language::Shell),
            "md" | "markdown" => Some(Language::Markdown),
            _ => None,
        }
    }

    /// Get display name for the language
    pub fn display_name(&self) -> &'static str {
        match self {
            Language::Rust => "Rust",
            Language::Python => "Python",
            Language::TypeScript => "TypeScript",
            Language::JavaScript => "JavaScript",
            Language::Go => "Go",
            Language::Java => "Java",
            Language::CSharp => "C#",
            Language::Cpp => "C++",
            Language::C => "C",
            Language::Ruby => "Ruby",
            Language::PHP => "PHP",
            Language::Swift => "Swift",
            Language::Kotlin => "Kotlin",
            Language::Scala => "Scala",
            Language::Shell => "Shell",
            Language::Markdown => "Markdown",
            Language::Unknown => "Unknown",
        }
    }
}

/// Frameworks detected in a project
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Framework {
    // Rust
    Actix,
    Axum,
    Rocket,
    Warp,
    Tokio,

    // Python
    Django,
    Flask,
    FastAPI,
    PyTorch,
    TensorFlow,

    // JavaScript/TypeScript
    React,
    Vue,
    Angular,
    NextJs,
    Express,
    NestJs,

    // Go
    Gin,
    Echo,
    Fiber,

    // Java
    Spring,
    Quarkus,

    // Ruby
    Rails,
    Sinatra,

    Unknown,
}

impl Framework {
    /// Get display name for the framework
    pub fn display_name(&self) -> &'static str {
        match self {
            Framework::Actix => "Actix Web",
            Framework::Axum => "Axum",
            Framework::Rocket => "Rocket",
            Framework::Warp => "Warp",
            Framework::Tokio => "Tokio",
            Framework::Django => "Django",
            Framework::Flask => "Flask",
            Framework::FastAPI => "FastAPI",
            Framework::PyTorch => "PyTorch",
            Framework::TensorFlow => "TensorFlow",
            Framework::React => "React",
            Framework::Vue => "Vue.js",
            Framework::Angular => "Angular",
            Framework::NextJs => "Next.js",
            Framework::Express => "Express",
            Framework::NestJs => "NestJS",
            Framework::Gin => "Gin",
            Framework::Echo => "Echo",
            Framework::Fiber => "Fiber",
            Framework::Spring => "Spring",
            Framework::Quarkus => "Quarkus",
            Framework::Rails => "Ruby on Rails",
            Framework::Sinatra => "Sinatra",
            Framework::Unknown => "Unknown",
        }
    }
}

/// Build systems detected in a project
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum BuildSystem {
    // Rust
    Cargo,

    // Python
    Pip,
    Poetry,
    Pipenv,
    Conda,
    Uv,

    // JavaScript/TypeScript
    Npm,
    Yarn,
    Pnpm,
    Bun,

    // Go
    GoMod,

    // Java
    Maven,
    Gradle,

    // C/C++
    Make,
    CMake,
    Bazel,

    // Ruby
    Bundler,

    // .NET
    DotNet,

    Unknown,
}

impl BuildSystem {
    /// Detect build system from config file name
    pub fn from_config_file(filename: &str) -> Option<Self> {
        match filename.to_lowercase().as_str() {
            "cargo.toml" => Some(BuildSystem::Cargo),
            "requirements.txt" | "setup.py" | "setup.cfg" => Some(BuildSystem::Pip),
            "pyproject.toml" => Some(BuildSystem::Poetry), // Could also be pip/uv
            "pipfile" | "pipfile.lock" => Some(BuildSystem::Pipenv),
            "environment.yml" | "environment.yaml" => Some(BuildSystem::Conda),
            "uv.lock" => Some(BuildSystem::Uv),
            "package.json" => Some(BuildSystem::Npm),
            "yarn.lock" => Some(BuildSystem::Yarn),
            "pnpm-lock.yaml" => Some(BuildSystem::Pnpm),
            "bun.lockb" => Some(BuildSystem::Bun),
            "go.mod" => Some(BuildSystem::GoMod),
            "pom.xml" => Some(BuildSystem::Maven),
            "build.gradle" | "build.gradle.kts" | "settings.gradle" | "settings.gradle.kts" => {
                Some(BuildSystem::Gradle)
            }
            "makefile" | "gnumakefile" => Some(BuildSystem::Make),
            "cmakelists.txt" => Some(BuildSystem::CMake),
            "build" | "workspace" => Some(BuildSystem::Bazel),
            "gemfile" => Some(BuildSystem::Bundler),
            // Use case-insensitive comparison for file extensions (Windows uses .CSPROJ, .SLN, etc.)
            _ if {
                let lower = filename.to_ascii_lowercase();
                lower.ends_with(".csproj") || lower.ends_with(".fsproj")
            } =>
            {
                Some(BuildSystem::DotNet)
            }
            _ if filename.to_ascii_lowercase().ends_with(".sln") => Some(BuildSystem::DotNet),
            _ => None,
        }
    }

    /// Get display name for the build system
    pub fn display_name(&self) -> &'static str {
        match self {
            BuildSystem::Cargo => "Cargo",
            BuildSystem::Pip => "pip",
            BuildSystem::Poetry => "Poetry",
            BuildSystem::Pipenv => "Pipenv",
            BuildSystem::Conda => "Conda",
            BuildSystem::Uv => "uv",
            BuildSystem::Npm => "npm",
            BuildSystem::Yarn => "Yarn",
            BuildSystem::Pnpm => "pnpm",
            BuildSystem::Bun => "Bun",
            BuildSystem::GoMod => "Go Modules",
            BuildSystem::Maven => "Maven",
            BuildSystem::Gradle => "Gradle",
            BuildSystem::Make => "Make",
            BuildSystem::CMake => "CMake",
            BuildSystem::Bazel => "Bazel",
            BuildSystem::Bundler => "Bundler",
            BuildSystem::DotNet => ".NET",
            BuildSystem::Unknown => "Unknown",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_language_from_extension() {
        assert_eq!(Language::from_extension("rs"), Some(Language::Rust));
        assert_eq!(Language::from_extension("py"), Some(Language::Python));
        assert_eq!(Language::from_extension("ts"), Some(Language::TypeScript));
        assert_eq!(Language::from_extension("js"), Some(Language::JavaScript));
        assert_eq!(Language::from_extension("go"), Some(Language::Go));
        assert_eq!(Language::from_extension("java"), Some(Language::Java));
        assert_eq!(Language::from_extension("xyz"), None);
    }

    #[test]
    fn test_language_from_extension_case_insensitive() {
        assert_eq!(Language::from_extension("RS"), Some(Language::Rust));
        assert_eq!(Language::from_extension("Py"), Some(Language::Python));
        assert_eq!(Language::from_extension("TsX"), Some(Language::TypeScript));
        assert_eq!(Language::from_extension("JSX"), Some(Language::JavaScript));
        assert_eq!(Language::from_extension("MaRkDoWn"), Some(Language::Markdown));
    }

    #[test]
    fn test_language_from_extension_python_variants() {
        assert_eq!(Language::from_extension("pyw"), Some(Language::Python));
        assert_eq!(Language::from_extension("pyi"), Some(Language::Python));
    }

    #[test]
    fn test_language_from_extension_javascript_variants() {
        assert_eq!(Language::from_extension("mjs"), Some(Language::JavaScript));
        assert_eq!(Language::from_extension("cjs"), Some(Language::JavaScript));
    }

    #[test]
    fn test_language_from_extension_cpp_variants() {
        assert_eq!(Language::from_extension("cpp"), Some(Language::Cpp));
        assert_eq!(Language::from_extension("cc"), Some(Language::Cpp));
        assert_eq!(Language::from_extension("cxx"), Some(Language::Cpp));
        assert_eq!(Language::from_extension("hpp"), Some(Language::Cpp));
        assert_eq!(Language::from_extension("hxx"), Some(Language::Cpp));
    }

    #[test]
    fn test_language_from_extension_c_variants() {
        assert_eq!(Language::from_extension("c"), Some(Language::C));
        assert_eq!(Language::from_extension("h"), Some(Language::C));
    }

    #[test]
    fn test_language_from_extension_ruby_variants() {
        assert_eq!(Language::from_extension("rb"), Some(Language::Ruby));
        assert_eq!(Language::from_extension("rake"), Some(Language::Ruby));
    }

    #[test]
    fn test_language_from_extension_shell_variants() {
        assert_eq!(Language::from_extension("sh"), Some(Language::Shell));
        assert_eq!(Language::from_extension("bash"), Some(Language::Shell));
        assert_eq!(Language::from_extension("zsh"), Some(Language::Shell));
        assert_eq!(Language::from_extension("fish"), Some(Language::Shell));
    }

    #[test]
    fn test_language_from_extension_kotlin_variants() {
        assert_eq!(Language::from_extension("kt"), Some(Language::Kotlin));
        assert_eq!(Language::from_extension("kts"), Some(Language::Kotlin));
    }

    #[test]
    fn test_language_from_extension_scala_variants() {
        assert_eq!(Language::from_extension("scala"), Some(Language::Scala));
        assert_eq!(Language::from_extension("sc"), Some(Language::Scala));
    }

    #[test]
    fn test_build_system_from_config() {
        assert_eq!(
            BuildSystem::from_config_file("Cargo.toml"),
            Some(BuildSystem::Cargo)
        );
        assert_eq!(
            BuildSystem::from_config_file("package.json"),
            Some(BuildSystem::Npm)
        );
        assert_eq!(
            BuildSystem::from_config_file("go.mod"),
            Some(BuildSystem::GoMod)
        );
        assert_eq!(
            BuildSystem::from_config_file("requirements.txt"),
            Some(BuildSystem::Pip)
        );
        assert_eq!(BuildSystem::from_config_file("unknown.xyz"), None);
    }

    #[test]
    fn test_build_system_from_config_case_insensitive() {
        assert_eq!(
            BuildSystem::from_config_file("CARGO.TOML"),
            Some(BuildSystem::Cargo)
        );
        assert_eq!(
            BuildSystem::from_config_file("PnPm-LoCk.YaMl"),
            Some(BuildSystem::Pnpm)
        );
        assert_eq!(
            BuildSystem::from_config_file("MAKEFILE"),
            Some(BuildSystem::Make)
        );
        assert_eq!(
            BuildSystem::from_config_file("CMakeLists.TXT"),
            Some(BuildSystem::CMake)
        );
    }

    #[test]
    fn test_build_system_from_config_gradle_variants() {
        assert_eq!(
            BuildSystem::from_config_file("build.gradle"),
            Some(BuildSystem::Gradle)
        );
        assert_eq!(
            BuildSystem::from_config_file("build.gradle.kts"),
            Some(BuildSystem::Gradle)
        );
        assert_eq!(
            BuildSystem::from_config_file("settings.gradle"),
            Some(BuildSystem::Gradle)
        );
        assert_eq!(
            BuildSystem::from_config_file("settings.gradle.kts"),
            Some(BuildSystem::Gradle)
        );
    }

    #[test]
    fn test_build_system_from_config_dotnet_variants() {
        assert_eq!(
            BuildSystem::from_config_file("MyApp.csproj"),
            Some(BuildSystem::DotNet)
        );
        assert_eq!(
            BuildSystem::from_config_file("MYAPP.FSPROJ"),
            Some(BuildSystem::DotNet)
        );
        assert_eq!(
            BuildSystem::from_config_file("Solution.sln"),
            Some(BuildSystem::DotNet)
        );
        assert_eq!(
            BuildSystem::from_config_file("SOLUTION.SLN"),
            Some(BuildSystem::DotNet)
        );
    }

    #[test]
    fn test_build_system_from_config_bazel_variants() {
        assert_eq!(BuildSystem::from_config_file("BUILD"), Some(BuildSystem::Bazel));
        assert_eq!(
            BuildSystem::from_config_file("WORKSPACE"),
            Some(BuildSystem::Bazel)
        );
        assert_eq!(BuildSystem::from_config_file("build"), Some(BuildSystem::Bazel));
        assert_eq!(
            BuildSystem::from_config_file("workspace"),
            Some(BuildSystem::Bazel)
        );
    }

    #[test]
    fn test_language_display_name() {
        assert_eq!(Language::Rust.display_name(), "Rust");
        assert_eq!(Language::Python.display_name(), "Python");
        assert_eq!(Language::Cpp.display_name(), "C++");
    }

    #[test]
    fn test_language_display_name_unknown() {
        assert_eq!(Language::Unknown.display_name(), "Unknown");
    }

    #[test]
    fn test_framework_display_name() {
        assert_eq!(Framework::Actix.display_name(), "Actix Web");
        assert_eq!(Framework::React.display_name(), "React");
        assert_eq!(Framework::FastAPI.display_name(), "FastAPI");
    }

    #[test]
    fn test_framework_display_name_additional() {
        assert_eq!(Framework::NextJs.display_name(), "Next.js");
        assert_eq!(Framework::NestJs.display_name(), "NestJS");
        assert_eq!(Framework::Rails.display_name(), "Ruby on Rails");
        assert_eq!(Framework::Unknown.display_name(), "Unknown");
    }

    #[test]
    fn test_build_system_display_name() {
        assert_eq!(BuildSystem::Cargo.display_name(), "Cargo");
        assert_eq!(BuildSystem::Npm.display_name(), "npm");
        assert_eq!(BuildSystem::Poetry.display_name(), "Poetry");
    }

    #[test]
    fn test_build_system_display_name_additional() {
        assert_eq!(BuildSystem::GoMod.display_name(), "Go Modules");
        assert_eq!(BuildSystem::DotNet.display_name(), ".NET");
        assert_eq!(BuildSystem::Uv.display_name(), "uv");
        assert_eq!(BuildSystem::Unknown.display_name(), "Unknown");
    }
}
