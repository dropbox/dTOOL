//! End-to-end tests for Codex DashFlow
//!
//! These tests verify the full CLI functionality:
//!
//! ## Unit Tests (run always, no external dependencies)
//! - Configuration and enum parsing
//! - Data structure serialization
//!
//! ## Integration Tests (run always, no LLM)
//! - CLI help and argument parsing
//! - Binary compilation check
//!
//! ## E2E Tests (require OPENAI_API_KEY, marked #[ignore])
//! - Generate command with real LLM
//! - Explain command with real LLM
//! - Refactor command with real LLM
//! - Test command with real LLM
//! - Docs command with real LLM
//!
//! ## Running E2E Tests
//!
//! The API key must be **exported** (not just sourced) for cargo to see it:
//!
//! ```bash
//! # Option 1: Export all .env variables
//! set -a && source .env && set +a
//! cargo test -p codex-dashflow --test e2e -- --ignored
//!
//! # Option 2: Source and export individually
//! source .env && export OPENAI_API_KEY
//! cargo test -p codex-dashflow --test e2e -- --ignored
//!
//! # Option 3: Inline
//! OPENAI_API_KEY="sk-..." cargo test -p codex-dashflow --test e2e -- --ignored
//! ```

// `cargo verify` runs clippy with `-D warnings` for all targets, including tests.
#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::print_stdout,
    clippy::print_stderr
)]

use codex_dashflow::{
    docs_generator::DocsStyle,
    explainer::DetailLevel,
    refactor::{RefactorFocus, RefactorSuggestion},
    test_generator::TestStyle,
    ChatConfig, CodeExplainer, CodeGenerator, CodexConfig, DocsGenerator, RefactorSuggester,
    TestGenerator,
};
use common::{create_llm, LLMRequirements};
use tempfile::TempDir;

// =============================================================================
// Unit Tests - Configuration
// =============================================================================

#[test]
fn test_codex_config_default() {
    let config = CodexConfig::default();
    assert_eq!(config.default_language, "rust");
    assert_eq!(config.model, "gpt-4o-mini");
    assert_eq!(config.max_tokens, 2048);
    assert!((config.temperature - 0.3).abs() < 0.001);
    assert!(config.include_comments);
    assert!(config.type_annotations);
}

#[test]
fn test_codex_config_for_languages() {
    let rust = CodexConfig::for_rust();
    assert_eq!(rust.default_language, "rust");

    let python = CodexConfig::for_python();
    assert_eq!(python.default_language, "python");

    let typescript = CodexConfig::for_typescript();
    assert_eq!(typescript.default_language, "typescript");
}

#[test]
fn test_codex_config_serialization() {
    let config = CodexConfig::default();
    let json = serde_json::to_string(&config).expect("Should serialize");
    let parsed: CodexConfig = serde_json::from_str(&json).expect("Should deserialize");
    assert_eq!(config.default_language, parsed.default_language);
    assert_eq!(config.model, parsed.model);
}

// =============================================================================
// Unit Tests - Enum Parsing
// =============================================================================

#[test]
fn test_detail_level_parsing() {
    assert_eq!("brief".parse::<DetailLevel>().unwrap(), DetailLevel::Brief);
    assert_eq!(
        "normal".parse::<DetailLevel>().unwrap(),
        DetailLevel::Normal
    );
    assert_eq!(
        "detailed".parse::<DetailLevel>().unwrap(),
        DetailLevel::Detailed
    );

    // Case insensitive
    assert_eq!("BRIEF".parse::<DetailLevel>().unwrap(), DetailLevel::Brief);

    // Invalid
    assert!("invalid".parse::<DetailLevel>().is_err());
}

#[test]
fn test_refactor_focus_parsing() {
    assert_eq!(
        "performance".parse::<RefactorFocus>().unwrap(),
        RefactorFocus::Performance
    );
    assert_eq!(
        "readability".parse::<RefactorFocus>().unwrap(),
        RefactorFocus::Readability
    );
    assert_eq!(
        "safety".parse::<RefactorFocus>().unwrap(),
        RefactorFocus::Safety
    );
    assert_eq!("all".parse::<RefactorFocus>().unwrap(), RefactorFocus::All);

    // Invalid
    assert!("invalid".parse::<RefactorFocus>().is_err());
}

#[test]
fn test_test_style_parsing() {
    assert_eq!("unit".parse::<TestStyle>().unwrap(), TestStyle::Unit);
    assert_eq!(
        "integration".parse::<TestStyle>().unwrap(),
        TestStyle::Integration
    );
    assert_eq!(
        "property".parse::<TestStyle>().unwrap(),
        TestStyle::Property
    );

    // Invalid
    assert!("invalid".parse::<TestStyle>().is_err());
}

#[test]
fn test_docs_style_parsing() {
    assert_eq!("rustdoc".parse::<DocsStyle>().unwrap(), DocsStyle::Rustdoc);
    assert_eq!(
        "docstring".parse::<DocsStyle>().unwrap(),
        DocsStyle::Docstring
    );
    assert_eq!(
        "markdown".parse::<DocsStyle>().unwrap(),
        DocsStyle::Markdown
    );
    assert_eq!("md".parse::<DocsStyle>().unwrap(), DocsStyle::Markdown);

    // Invalid
    assert!("invalid".parse::<DocsStyle>().is_err());
}

// =============================================================================
// Unit Tests - Data Structures
// =============================================================================

#[test]
fn test_refactor_suggestion_serialization() {
    let suggestion = RefactorSuggestion {
        description: "Use iterator instead of loop".to_string(),
        category: "readability".to_string(),
        priority: "medium".to_string(),
        original: Some("for i in 0..vec.len() { ... }".to_string()),
        suggested: Some("for item in &vec { ... }".to_string()),
        lines: Some((10, 15)),
    };

    let json = serde_json::to_string(&suggestion).expect("Should serialize");
    assert!(json.contains("iterator"));
    assert!(json.contains("readability"));

    let parsed: RefactorSuggestion = serde_json::from_str(&json).expect("Should deserialize");
    assert_eq!(suggestion.description, parsed.description);
    assert_eq!(suggestion.category, parsed.category);
}

#[test]
fn test_chat_config_default() {
    let config = ChatConfig::default();
    assert!(config.context_dir.is_none());
    assert!(config.system_prompt.is_none());
}

// =============================================================================
// Integration Tests - CLI
// =============================================================================

/// Get path to the codex-dashflow binary (must be built first)
fn get_binary_path() -> std::path::PathBuf {
    // Look for binary in target/debug or target/release
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| std::path::PathBuf::from("."));

    // Navigate to workspace root
    let workspace_root = manifest_dir
        .ancestors()
        .nth(3) // examples/apps/codex-dashflow -> root
        .unwrap_or(manifest_dir.as_path());

    let debug_path = workspace_root.join("target/debug/codex-dashflow");

    if debug_path.exists() {
        debug_path
    } else {
        // Fall back to cargo run if binary not found
        std::path::PathBuf::from("cargo")
    }
}

#[test]
fn test_cli_help() {
    use std::process::Command;

    let binary = get_binary_path();
    let output = if binary.to_string_lossy().contains("cargo") {
        Command::new("cargo")
            .args(["run", "-p", "codex-dashflow", "-q", "--", "--help"])
            .output()
            .expect("Failed to run CLI")
    } else {
        Command::new(&binary)
            .args(["--help"])
            .output()
            .expect("Failed to run CLI")
    };

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("Codex DashFlow"),
        "Help should mention Codex DashFlow: {}",
        stdout
    );
    assert!(
        stdout.contains("generate"),
        "Help should list generate command"
    );
    assert!(
        stdout.contains("explain"),
        "Help should list explain command"
    );
    assert!(
        stdout.contains("refactor"),
        "Help should list refactor command"
    );
    assert!(stdout.contains("test"), "Help should list test command");
    assert!(stdout.contains("docs"), "Help should list docs command");
    assert!(stdout.contains("chat"), "Help should list chat command");
}

#[test]
fn test_cli_generate_help() {
    use std::process::Command;

    let binary = get_binary_path();
    let output = if binary.to_string_lossy().contains("cargo") {
        Command::new("cargo")
            .args([
                "run",
                "-p",
                "codex-dashflow",
                "-q",
                "--",
                "generate",
                "--help",
            ])
            .output()
            .expect("Failed to run CLI")
    } else {
        Command::new(&binary)
            .args(["generate", "--help"])
            .output()
            .expect("Failed to run CLI")
    };

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("language"),
        "Generate help should mention language"
    );
    assert!(
        stdout.contains("output"),
        "Generate help should mention output"
    );
    assert!(
        stdout.contains("with-tests"),
        "Generate help should mention with-tests"
    );
}

#[test]
fn test_cli_explain_help() {
    use std::process::Command;

    let binary = get_binary_path();
    let output = if binary.to_string_lossy().contains("cargo") {
        Command::new("cargo")
            .args([
                "run",
                "-p",
                "codex-dashflow",
                "-q",
                "--",
                "explain",
                "--help",
            ])
            .output()
            .expect("Failed to run CLI")
    } else {
        Command::new(&binary)
            .args(["explain", "--help"])
            .output()
            .expect("Failed to run CLI")
    };

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("file"), "Explain help should mention file");
    assert!(
        stdout.contains("symbol"),
        "Explain help should mention symbol"
    );
    assert!(
        stdout.contains("detail"),
        "Explain help should mention detail"
    );
}

// =============================================================================
// E2E Tests - Real LLM Integration (require OPENAI_API_KEY)
// =============================================================================

/// Helper to check if LLM is available
fn llm_available() -> bool {
    std::env::var("OPENAI_API_KEY").is_ok()
        || std::env::var("ANTHROPIC_API_KEY").is_ok()
        || std::env::var("AWS_ACCESS_KEY_ID").is_ok()
}

/// Sample Rust code for testing
const SAMPLE_CODE: &str = r#"
/// Calculate the factorial of a number
pub fn factorial(n: u64) -> u64 {
    if n <= 1 {
        1
    } else {
        n * factorial(n - 1)
    }
}

/// Find the maximum element in a slice
pub fn find_max(items: &[i32]) -> Option<i32> {
    items.iter().copied().max()
}
"#;

#[tokio::test]
#[ignore = "Requires OPENAI_API_KEY - run with --ignored"]
async fn test_e2e_generate_code() {
    if !llm_available() {
        eprintln!("Skipping E2E test: No LLM API key available");
        return;
    }

    let model = create_llm(LLMRequirements::default())
        .await
        .expect("Should create LLM");
    let config = CodexConfig::for_rust();
    let generator = CodeGenerator::new(model, config);

    let result = generator
        .generate("a function that checks if a number is prime")
        .await
        .expect("Should generate code");

    // Verify output contains expected patterns
    assert!(!result.is_empty(), "Generated code should not be empty");
    assert!(
        result.contains("fn") || result.contains("pub fn"),
        "Should contain function definition"
    );
    assert!(
        result.to_lowercase().contains("prime"),
        "Should contain 'prime' in function/comment"
    );
}

#[tokio::test]
#[ignore = "Requires OPENAI_API_KEY - run with --ignored"]
async fn test_e2e_generate_with_tests() {
    if !llm_available() {
        eprintln!("Skipping E2E test: No LLM API key available");
        return;
    }

    let model = create_llm(LLMRequirements::default())
        .await
        .expect("Should create LLM");
    let config = CodexConfig::for_rust();
    let generator = CodeGenerator::new(model, config);

    let result = generator
        .generate_with_tests("a function that reverses a string")
        .await
        .expect("Should generate code with tests");

    assert!(!result.is_empty(), "Generated code should not be empty");
    assert!(
        result.contains("#[test]") || result.contains("#[cfg(test)]"),
        "Should contain test code"
    );
}

#[tokio::test]
#[ignore = "Requires OPENAI_API_KEY - run with --ignored"]
async fn test_e2e_explain_code() {
    if !llm_available() {
        eprintln!("Skipping E2E test: No LLM API key available");
        return;
    }

    let model = create_llm(LLMRequirements::default())
        .await
        .expect("Should create LLM");
    let explainer = CodeExplainer::new(model);

    let result = explainer
        .explain(SAMPLE_CODE, DetailLevel::Normal)
        .await
        .expect("Should explain code");

    assert!(!result.is_empty(), "Explanation should not be empty");
    // Should mention key concepts from the code
    assert!(
        result.to_lowercase().contains("factorial")
            || result.to_lowercase().contains("recursive")
            || result.to_lowercase().contains("function"),
        "Should explain the factorial function"
    );
}

#[tokio::test]
#[ignore = "Requires OPENAI_API_KEY - run with --ignored"]
async fn test_e2e_explain_symbol() {
    if !llm_available() {
        eprintln!("Skipping E2E test: No LLM API key available");
        return;
    }

    let model = create_llm(LLMRequirements::default())
        .await
        .expect("Should create LLM");
    let explainer = CodeExplainer::new(model);

    let result = explainer
        .explain_symbol(SAMPLE_CODE, "find_max")
        .await
        .expect("Should explain symbol");

    assert!(!result.is_empty(), "Explanation should not be empty");
    assert!(
        result.to_lowercase().contains("max")
            || result.to_lowercase().contains("maximum")
            || result.to_lowercase().contains("largest"),
        "Should explain the find_max function"
    );
}

#[tokio::test]
#[ignore = "Requires OPENAI_API_KEY - run with --ignored"]
async fn test_e2e_refactor_suggestions() {
    if !llm_available() {
        eprintln!("Skipping E2E test: No LLM API key available");
        return;
    }

    let model = create_llm(LLMRequirements::default())
        .await
        .expect("Should create LLM");
    let suggester = RefactorSuggester::new(model);

    let result = suggester
        .suggest(SAMPLE_CODE, RefactorFocus::All)
        .await
        .expect("Should generate suggestions");

    // May or may not have suggestions depending on LLM response
    // The important thing is it doesn't error
    eprintln!("Generated {} refactoring suggestions", result.len());
}

#[tokio::test]
#[ignore = "Requires OPENAI_API_KEY - run with --ignored"]
async fn test_e2e_generate_tests() {
    if !llm_available() {
        eprintln!("Skipping E2E test: No LLM API key available");
        return;
    }

    let model = create_llm(LLMRequirements::default())
        .await
        .expect("Should create LLM");
    let generator = TestGenerator::new(model);

    let result = generator
        .generate(SAMPLE_CODE, TestStyle::Unit)
        .await
        .expect("Should generate tests");

    assert!(!result.code.is_empty(), "Test code should not be empty");
    assert!(
        result.code.contains("#[test]"),
        "Should contain test attributes"
    );
    assert!(result.test_count > 0, "Should have at least one test");
    eprintln!(
        "Generated {} tests: {}",
        result.test_count, result.description
    );
}

#[tokio::test]
#[ignore = "Requires OPENAI_API_KEY - run with --ignored"]
async fn test_e2e_generate_tests_for_function() {
    if !llm_available() {
        eprintln!("Skipping E2E test: No LLM API key available");
        return;
    }

    let model = create_llm(LLMRequirements::default())
        .await
        .expect("Should create LLM");
    let generator = TestGenerator::new(model);

    let result = generator
        .generate_for_function(SAMPLE_CODE, "factorial", TestStyle::Unit)
        .await
        .expect("Should generate tests for function");

    assert!(!result.code.is_empty(), "Test code should not be empty");
    assert!(
        result.code.to_lowercase().contains("factorial"),
        "Should test the factorial function"
    );
}

#[tokio::test]
#[ignore = "Requires OPENAI_API_KEY - run with --ignored"]
async fn test_e2e_generate_docs() {
    if !llm_available() {
        eprintln!("Skipping E2E test: No LLM API key available");
        return;
    }

    let model = create_llm(LLMRequirements::default())
        .await
        .expect("Should create LLM");
    let generator = DocsGenerator::new(model);

    let result = generator
        .generate(SAMPLE_CODE, DocsStyle::Rustdoc, true)
        .await
        .expect("Should generate docs");

    assert!(
        !result.content.is_empty(),
        "Documentation should not be empty"
    );
    assert_eq!(result.style, "rustdoc");
    assert!(result.has_examples);
}

#[tokio::test]
#[ignore = "Requires OPENAI_API_KEY - run with --ignored"]
async fn test_e2e_generate_markdown_docs() {
    if !llm_available() {
        eprintln!("Skipping E2E test: No LLM API key available");
        return;
    }

    let model = create_llm(LLMRequirements::default())
        .await
        .expect("Should create LLM");
    let generator = DocsGenerator::new(model);

    let result = generator
        .generate(SAMPLE_CODE, DocsStyle::Markdown, false)
        .await
        .expect("Should generate markdown docs");

    assert!(
        !result.content.is_empty(),
        "Documentation should not be empty"
    );
    assert_eq!(result.style, "markdown");
    // Should contain markdown elements
    assert!(
        result.content.contains('#') || result.content.contains("```"),
        "Should contain markdown formatting"
    );
}

// =============================================================================
// E2E Tests - CLI with Real LLM
// =============================================================================

#[tokio::test]
#[ignore = "Requires OPENAI_API_KEY - run with --ignored"]
async fn test_e2e_cli_generate() {
    use std::process::Command;

    if !llm_available() {
        eprintln!("Skipping E2E CLI test: No LLM API key available");
        return;
    }

    let temp_dir = TempDir::new().expect("Should create temp dir");
    let output_file = temp_dir.path().join("generated.rs");

    let output = Command::new("cargo")
        .args([
            "run",
            "-p",
            "codex-dashflow",
            "--",
            "generate",
            "a function that adds two numbers",
            "--output",
            output_file.to_str().unwrap(),
        ])
        .output()
        .expect("Failed to run CLI");

    assert!(
        output.status.success(),
        "CLI should succeed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let generated = tokio::fs::read_to_string(&output_file)
        .await
        .expect("Should read output file");
    assert!(!generated.is_empty(), "Generated file should not be empty");
    assert!(
        generated.contains("fn"),
        "Should contain function definition"
    );
}

#[tokio::test]
#[ignore = "Requires OPENAI_API_KEY - run with --ignored"]
async fn test_e2e_cli_explain() {
    use std::process::Command;

    if !llm_available() {
        eprintln!("Skipping E2E CLI test: No LLM API key available");
        return;
    }

    let temp_dir = TempDir::new().expect("Should create temp dir");
    let input_file = temp_dir.path().join("input.rs");
    tokio::fs::write(&input_file, SAMPLE_CODE)
        .await
        .expect("Should write input file");

    let output = Command::new("cargo")
        .args([
            "run",
            "-p",
            "codex-dashflow",
            "--",
            "explain",
            "--file",
            input_file.to_str().unwrap(),
        ])
        .output()
        .expect("Failed to run CLI");

    assert!(
        output.status.success(),
        "CLI should succeed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        !stdout.is_empty(),
        "Should produce explanation output: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[tokio::test]
#[ignore = "Requires OPENAI_API_KEY - run with --ignored"]
async fn test_e2e_cli_test_generation() {
    use std::process::Command;

    if !llm_available() {
        eprintln!("Skipping E2E CLI test: No LLM API key available");
        return;
    }

    let temp_dir = TempDir::new().expect("Should create temp dir");
    let input_file = temp_dir.path().join("input.rs");
    let output_file = temp_dir.path().join("tests.rs");
    tokio::fs::write(&input_file, SAMPLE_CODE)
        .await
        .expect("Should write input file");

    let output = Command::new("cargo")
        .args([
            "run",
            "-p",
            "codex-dashflow",
            "--",
            "test",
            "--file",
            input_file.to_str().unwrap(),
            "--output",
            output_file.to_str().unwrap(),
        ])
        .output()
        .expect("Failed to run CLI");

    assert!(
        output.status.success(),
        "CLI should succeed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let generated = tokio::fs::read_to_string(&output_file)
        .await
        .expect("Should read output file");
    assert!(
        generated.contains("#[test]"),
        "Should contain test attributes"
    );
}

#[tokio::test]
#[ignore = "Requires OPENAI_API_KEY - run with --ignored"]
async fn test_e2e_cli_docs_generation() {
    use std::process::Command;

    if !llm_available() {
        eprintln!("Skipping E2E CLI test: No LLM API key available");
        return;
    }

    let temp_dir = TempDir::new().expect("Should create temp dir");
    let input_file = temp_dir.path().join("input.rs");
    tokio::fs::write(&input_file, SAMPLE_CODE)
        .await
        .expect("Should write input file");

    let output = Command::new("cargo")
        .args([
            "run",
            "-p",
            "codex-dashflow",
            "--",
            "docs",
            "--file",
            input_file.to_str().unwrap(),
            "--style",
            "rustdoc",
        ])
        .output()
        .expect("Failed to run CLI");

    assert!(
        output.status.success(),
        "CLI should succeed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(!stdout.is_empty(), "Should produce documentation output");
}

// =============================================================================
// Multi-Language E2E Tests
// =============================================================================

#[tokio::test]
#[ignore = "Requires OPENAI_API_KEY - run with --ignored"]
async fn test_e2e_generate_python_code() {
    if !llm_available() {
        eprintln!("Skipping E2E test: No LLM API key available");
        return;
    }

    let model = create_llm(LLMRequirements::default())
        .await
        .expect("Should create LLM");
    let config = CodexConfig::for_python();
    let generator = CodeGenerator::new(model, config);

    let result = generator
        .generate("a function that sorts a list using quicksort")
        .await
        .expect("Should generate Python code");

    assert!(!result.is_empty(), "Generated code should not be empty");
    // Python uses 'def' for function definitions
    assert!(
        result.contains("def ") || result.contains("def\t"),
        "Should contain Python function definition"
    );
}

#[tokio::test]
#[ignore = "Requires OPENAI_API_KEY - run with --ignored"]
async fn test_e2e_generate_typescript_code() {
    if !llm_available() {
        eprintln!("Skipping E2E test: No LLM API key available");
        return;
    }

    let model = create_llm(LLMRequirements::default())
        .await
        .expect("Should create LLM");
    let config = CodexConfig::for_typescript();
    let generator = CodeGenerator::new(model, config);

    let result = generator
        .generate("a function that validates an email address")
        .await
        .expect("Should generate TypeScript code");

    assert!(!result.is_empty(), "Generated code should not be empty");
    // TypeScript uses 'function' or arrow functions, may have type annotations
    assert!(
        result.contains("function ")
            || result.contains("=>")
            || result.contains(": boolean")
            || result.contains(": string"),
        "Should contain TypeScript code patterns"
    );
}

// =============================================================================
// Integration Tests - Exec Command
// =============================================================================

#[test]
fn test_cli_exec_help() {
    use std::process::Command;

    let binary = get_binary_path();
    let output = if binary.to_string_lossy().contains("cargo") {
        Command::new("cargo")
            .args(["run", "-p", "codex-dashflow", "-q", "--", "exec", "--help"])
            .output()
            .expect("Failed to run CLI")
    } else {
        Command::new(&binary)
            .args(["exec", "--help"])
            .output()
            .expect("Failed to run CLI")
    };

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("Execute a single prompt non-interactively"),
        "Exec help should describe the command"
    );
    assert!(
        stdout.contains("working-dir"),
        "Exec help should mention working-dir"
    );
    assert!(
        stdout.contains("context"),
        "Exec help should mention context"
    );
    assert!(stdout.contains("format"), "Exec help should mention format");
    assert!(
        stdout.contains("session"),
        "Exec help should mention session persistence"
    );
    assert!(stdout.contains("resume"), "Exec help should mention resume");
}

#[test]
fn test_cli_chat_help_includes_session_flags() {
    use std::process::Command;

    let binary = get_binary_path();
    let output = if binary.to_string_lossy().contains("cargo") {
        Command::new("cargo")
            .args(["run", "-p", "codex-dashflow", "-q", "--", "chat", "--help"])
            .output()
            .expect("Failed to run CLI")
    } else {
        Command::new(&binary)
            .args(["chat", "--help"])
            .output()
            .expect("Failed to run CLI")
    };

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("Interactive chat mode"),
        "Chat help should describe the command: {}",
        stdout
    );
    assert!(
        stdout.contains("session"),
        "Chat help should mention session persistence: {}",
        stdout
    );
    assert!(
        stdout.contains("resume"),
        "Chat help should mention resume: {}",
        stdout
    );
}

#[tokio::test]
#[ignore = "Requires OPENAI_API_KEY - run with --ignored"]
async fn test_e2e_cli_exec() {
    use std::process::Command;

    if !llm_available() {
        eprintln!("Skipping E2E CLI test: No LLM API key available");
        return;
    }

    let output = Command::new("cargo")
        .args([
            "run",
            "-p",
            "codex-dashflow",
            "--",
            "exec",
            "What is 2 + 2? Reply with just the number.",
        ])
        .output()
        .expect("Failed to run CLI");

    assert!(
        output.status.success(),
        "CLI should succeed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("4"),
        "Should answer the question: {}",
        stdout
    );
}

#[tokio::test]
#[ignore = "Requires OPENAI_API_KEY - run with --ignored"]
async fn test_e2e_cli_exec_with_context_file() {
    use std::process::Command;

    if !llm_available() {
        eprintln!("Skipping E2E CLI test: No LLM API key available");
        return;
    }

    let temp_dir = TempDir::new().expect("Should create temp dir");
    let context_file = temp_dir.path().join("context.txt");
    tokio::fs::write(&context_file, "The secret number is 42.")
        .await
        .expect("Should write context file");

    let output = Command::new("cargo")
        .args([
            "run",
            "-p",
            "codex-dashflow",
            "--",
            "exec",
            "What is the secret number mentioned in the context? Reply with just the number.",
            "--context",
            context_file.to_str().unwrap(),
        ])
        .output()
        .expect("Failed to run CLI");

    assert!(
        output.status.success(),
        "CLI should succeed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("42"),
        "Should read from context file: {}",
        stdout
    );
}

#[tokio::test]
#[ignore = "Requires OPENAI_API_KEY - run with --ignored"]
async fn test_e2e_cli_exec_json_format() {
    use std::process::Command;

    if !llm_available() {
        eprintln!("Skipping E2E CLI test: No LLM API key available");
        return;
    }

    let output = Command::new("cargo")
        .args([
            "run",
            "-p",
            "codex-dashflow",
            "--",
            "exec",
            "What is 3 + 3? Reply with just the number.",
            "--format",
            "json",
        ])
        .output()
        .expect("Failed to run CLI");

    assert!(
        output.status.success(),
        "CLI should succeed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    // Should be valid JSON with expected structure
    let parsed: serde_json::Value = serde_json::from_str(&stdout)
        .unwrap_or_else(|_| panic!("Should be valid JSON: {}", stdout));

    assert_eq!(parsed["success"], true, "Should have success: true");
    assert!(parsed["result"].is_string(), "Should have result string");
    assert!(parsed["duration_ms"].is_number(), "Should have duration_ms");
}

#[test]
fn test_cli_apply_help() {
    use std::process::Command;

    let binary = get_binary_path();
    let output = if binary.to_string_lossy().contains("cargo") {
        Command::new("cargo")
            .args(["run", "-p", "codex-dashflow", "-q", "--", "apply", "--help"])
            .output()
            .expect("Failed to run CLI")
    } else {
        Command::new(&binary)
            .args(["apply", "--help"])
            .output()
            .expect("Failed to run CLI")
    };

    assert!(
        output.status.success(),
        "apply --help should succeed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("Apply a git patch"),
        "Should describe apply: {}",
        stdout
    );
    assert!(
        stdout.contains("--dry-run"),
        "Should mention dry-run flag: {}",
        stdout
    );
    assert!(
        stdout.contains("--show-diff"),
        "Should mention show-diff flag: {}",
        stdout
    );
    assert!(
        stdout.contains("--patch"),
        "Should mention patch option: {}",
        stdout
    );
}
