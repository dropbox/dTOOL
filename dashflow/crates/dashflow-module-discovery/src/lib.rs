//! # DashFlow Module Discovery - Zero-Marker Auto-Discovery
//!
//! Automatically discovers DashFlow modules by parsing Rust's module system.
//! No markers required - modules are found from `pub mod` declarations.

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

/// Information about a discovered module
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModuleInfo {
    /// User-facing short name for the module (may be overridden via `@name`).
    pub name: String,
    /// Fully qualified Rust module path (e.g., `dashflow::introspection::index`).
    pub path: String,
    /// High-level category for grouping/search (e.g., `core`, `llm`, `vector_store`).
    pub category: String,
    /// One-line summary extracted from module docs.
    pub description: String,
    /// Capability tags inferred from module name/docs and children (e.g., ["kafka", "streaming"])
    pub capability_tags: Vec<String>,
    /// Source file path for this module, relative to the crate `src/` root when possible.
    pub source_path: PathBuf,
    /// Direct `pub mod` children declared by this module.
    pub children: Vec<String>,
    /// CLI command name for this module, when applicable.
    pub cli_command: Option<String>,
    /// Wiring status for the CLI entrypoint associated with this module, when applicable.
    pub cli_status: Option<CliStatus>,
    /// Stability status inferred from docs (e.g., via `@status` markers).
    pub status: ModuleStatus,
}

/// Whether a module's CLI command is implemented and wired.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CliStatus {
    /// Implemented and reachable via the CLI.
    Wired,
    /// Implemented in library code, but the CLI command is not yet wired.
    Stub,
    /// No CLI command exists for this module.
    None,
}

/// Stability designation for a discovered module.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum ModuleStatus {
    #[default]
    /// Stable and supported.
    Stable,
    /// Experimental and subject to change.
    Experimental,
    /// Deprecated; prefer alternatives.
    Deprecated,
}

/// Discover all modules in a crate by parsing Rust source
pub fn discover_modules(src_path: impl AsRef<Path>) -> Vec<ModuleInfo> {
    let src_path = src_path.as_ref();
    let mut modules = Vec::new();

    let lib_rs = src_path.join("lib.rs");
    if lib_rs.exists() {
        if let Ok(content) = fs::read_to_string(&lib_rs) {
            for mod_name in parse_pub_mod_declarations(&content) {
                discover_module_recursive(src_path, src_path, &mod_name, "root", &mut modules);
            }
        }
    }

    propagate_module_capability_tags(&mut modules);
    modules
}

/// Workspace crate configuration for multi-crate discovery
#[derive(Debug, Clone)]
pub struct WorkspaceCrate {
    /// Name of the crate (e.g., "dashflow-streaming")
    pub name: String,
    /// Relative path from workspace root to src/ directory
    pub src_path: PathBuf,
    /// Category prefix for modules from this crate
    pub category_prefix: String,
}

impl WorkspaceCrate {
    /// Construct a workspace crate entry for discovery.
    pub fn new(
        name: impl Into<String>,
        src_path: impl Into<PathBuf>,
        category_prefix: impl Into<String>,
    ) -> Self {
        Self {
            name: name.into(),
            src_path: src_path.into(),
            category_prefix: category_prefix.into(),
        }
    }
}

/// Default workspace crates to scan for modules (core subset for fast queries)
///
/// For comprehensive discovery, use `discover_all_workspace_crates()` instead.
pub fn default_workspace_crates() -> Vec<WorkspaceCrate> {
    vec![
        WorkspaceCrate::new("dashflow", "crates/dashflow/src", "core"),
        WorkspaceCrate::new(
            "dashflow-streaming",
            "crates/dashflow-streaming/src",
            "streaming",
        ),
        WorkspaceCrate::new(
            "dashflow-observability",
            "crates/dashflow-observability/src",
            "observability",
        ),
    ]
}

/// Auto-discover ALL workspace crates from filesystem
///
/// Scans the `crates/` directory and returns WorkspaceCrate entries for each crate
/// that has a `src/lib.rs` file. This ensures comprehensive module discovery.
///
/// # Arguments
///
/// * `workspace_root` - Path to the workspace root (where Cargo.toml is)
///
/// # Example
///
/// ```rust,no_run
/// use dashflow_module_discovery::{discover_all_workspace_crates, discover_workspace_modules};
/// use std::path::Path;
///
/// let workspace = Path::new("/path/to/dashflow");
/// let crates = discover_all_workspace_crates(workspace);
/// let modules = discover_workspace_modules(workspace, &crates);
/// println!("Found {} modules across {} crates", modules.len(), crates.len());
/// ```
pub fn discover_all_workspace_crates(workspace_root: impl AsRef<Path>) -> Vec<WorkspaceCrate> {
    let workspace_root = workspace_root.as_ref();
    let crates_dir = workspace_root.join("crates");

    let mut crates = Vec::new();

    if let Ok(entries) = fs::read_dir(&crates_dir) {
        for entry in entries.filter_map(Result::ok) {
            let path = entry.path();
            if path.is_dir() {
                let src_lib = path.join("src").join("lib.rs");
                if src_lib.exists() {
                    if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                        let category = infer_crate_category(name);
                        let src_path = format!("crates/{}/src", name);
                        crates.push(WorkspaceCrate::new(name, src_path, category));
                    }
                }
            }
        }
    }

    // Sort for deterministic output
    crates.sort_by(|a, b| a.name.cmp(&b.name));

    crates
}

/// Infer category from crate name
fn infer_crate_category(crate_name: &str) -> String {
    // Remove "dashflow-" prefix for category inference
    let name = crate_name.strip_prefix("dashflow-").unwrap_or(crate_name);

    match name {
        // Core framework
        "dashflow" => "core".to_string(),

        // LLM providers
        "anthropic" | "openai" | "azure-openai" | "bedrock" | "cohere" | "deepseek"
        | "fireworks" | "google-genai" | "groq" | "mistral" | "ollama" | "together"
        | "vertexai" | "voyage" => "llm".to_string(),

        // Vector stores
        "annoy" | "cassandra" | "chroma" | "clickhouse" | "elasticsearch" | "faiss" | "lancedb"
        | "milvus" | "mongodb" | "neo4j" | "opensearch" | "pgvector" | "pinecone" | "qdrant"
        | "redis" | "singlestore" | "surrealdb" | "typesense" | "weaviate" => {
            "vector_store".to_string()
        }

        // Search/Tools
        "arxiv" | "bing" | "brave" | "calculator" | "duckduckgo" | "exa" | "google" | "tavily"
        | "wikipedia" => "tool".to_string(),

        // Infrastructure
        "streaming" | "observability" | "telemetry" => "infrastructure".to_string(),

        // Other categories
        "chains" | "context" | "compression" | "document-compressors" => "processing".to_string(),
        "evals" | "benchmarks" => "evaluation".to_string(),
        "cli" | "derive" | "factories" => "framework".to_string(),

        // Default: use first part of name or full name
        _ => name.split('-').next().unwrap_or(name).to_string(),
    }
}

/// Discover modules from multiple workspace crates
///
/// This function scans multiple crate source directories and aggregates
/// all discovered modules. Each module's path is prefixed with the crate name
/// for disambiguation (e.g., "dashflow-streaming::producer").
///
/// # Arguments
///
/// * `workspace_root` - Path to the workspace root (where Cargo.toml is)
/// * `crates` - List of workspace crates to scan (use `default_workspace_crates()` for defaults)
///
/// # Example
///
/// ```rust,no_run
/// use dashflow_module_discovery::{discover_workspace_modules, default_workspace_crates};
/// use std::path::Path;
///
/// let modules = discover_workspace_modules(Path::new("/path/to/workspace"), &default_workspace_crates());
/// ```
pub fn discover_workspace_modules(
    workspace_root: impl AsRef<Path>,
    crates: &[WorkspaceCrate],
) -> Vec<ModuleInfo> {
    let workspace_root = workspace_root.as_ref();
    let mut all_modules = Vec::new();

    for krate in crates {
        let src_path = workspace_root.join(&krate.src_path);
        if !src_path.exists() {
            continue;
        }

        let mut crate_modules = discover_modules(&src_path);

        // Prefix module paths with crate name for disambiguation
        for module in &mut crate_modules {
            if krate.name != "dashflow" {
                // For non-core crates, prefix the path
                module.path = format!("{}::{}", krate.name.replace('-', "_"), module.path);
            }
            // Override category with crate-specific prefix if not already set
            if module.category == "core" && krate.category_prefix != "core" {
                module.category = krate.category_prefix.clone();
            }
        }

        all_modules.extend(crate_modules);
    }

    all_modules
}

// ============================================================================
// BINARY DISCOVERY (M-605)
// ============================================================================

/// Discover binaries in `src/bin/` directories of workspace crates
///
/// This function scans crates for binary targets in `src/bin/` directories
/// and returns them as `ModuleInfo` entries with category "binary".
///
/// # Arguments
///
/// * `workspace_root` - Path to the workspace root (where Cargo.toml is)
/// * `crates` - List of workspace crates to scan for binaries
///
/// # Example
///
/// ```rust,no_run
/// use dashflow_module_discovery::{discover_workspace_binaries, default_workspace_crates};
/// use std::path::Path;
///
/// let binaries = discover_workspace_binaries(Path::new("/path/to/workspace"), &default_workspace_crates());
/// for bin in binaries {
///     println!("Binary: {} ({})", bin.name, bin.description);
/// }
/// ```
pub fn discover_workspace_binaries(
    workspace_root: impl AsRef<Path>,
    crates: &[WorkspaceCrate],
) -> Vec<ModuleInfo> {
    let workspace_root = workspace_root.as_ref();
    let mut binaries = Vec::new();

    for krate in crates {
        let bin_path = workspace_root.join(&krate.src_path).join("bin");
        if !bin_path.exists() || !bin_path.is_dir() {
            continue;
        }

        if let Ok(entries) = fs::read_dir(&bin_path) {
            for entry in entries.filter_map(Result::ok) {
                let path = entry.path();
                if path.extension().is_some_and(|e| e == "rs") {
                    if let Some(bin_info) = discover_binary(&path, &krate.name) {
                        binaries.push(bin_info);
                    }
                }
            }
        }
    }

    // Sort for deterministic output
    binaries.sort_by(|a, b| a.name.cmp(&b.name));
    binaries
}

/// Discover a single binary from its source file
fn discover_binary(file_path: &Path, crate_name: &str) -> Option<ModuleInfo> {
    let content = fs::read_to_string(file_path).ok()?;
    let file_name = file_path.file_stem()?.to_str()?;

    // Parse metadata from doc comments (same format as modules)
    let metadata = parse_binary_metadata(&content);
    let docs = extract_binary_documentation(&content);

    // Infer capability tags from binary name and docs
    let capability_tags = infer_capability_tags_with_methods(file_name, &docs, &[]);

    Some(ModuleInfo {
        name: file_name.to_string(),
        path: format!("{}::bin::{}", crate_name.replace('-', "_"), file_name),
        category: "binary".to_string(),
        description: metadata.description,
        capability_tags,
        source_path: file_path.to_path_buf(),
        children: vec![],
        cli_command: Some(file_name.to_string()),
        cli_status: Some(CliStatus::Wired),
        status: metadata.status,
    })
}

/// Parse metadata from binary file doc comments
fn parse_binary_metadata(content: &str) -> ParsedModuleMetadata {
    let mut description_lines = Vec::new();
    let mut status = ModuleStatus::Stable;
    let mut found_blank = false;

    for line in content.lines() {
        let trimmed = line.trim();

        // Only //! lines are module-level documentation for binaries
        if trimmed.starts_with("//!") {
            let doc_content = trimmed.trim_start_matches("//!").trim();

            if let Some(rest) = doc_content.strip_prefix("@status ") {
                status = match rest.trim() {
                    "deprecated" => ModuleStatus::Deprecated,
                    "experimental" => ModuleStatus::Experimental,
                    _ => ModuleStatus::Stable,
                };
            } else if !doc_content.starts_with('@') {
                if doc_content.is_empty() {
                    if !description_lines.is_empty() {
                        found_blank = true;
                    }
                } else if !found_blank {
                    // First paragraph is the binary description
                    let text = doc_content.trim_start_matches('#').trim();
                    if !text.is_empty() {
                        description_lines.push(text.to_string());
                    }
                }
            }
            continue;
        }

        // Skip these non-doc lines without breaking:
        // - Regular comments (//)
        // - Empty lines
        // - Inner attributes (#![...])
        // - Shebangs (#!/usr/bin/env cargo)
        if trimmed.starts_with("//")
            || trimmed.is_empty()
            || trimmed.starts_with("#![")
            || (trimmed.starts_with("#!") && trimmed.chars().nth(2) == Some('/'))
        {
            continue;
        }

        // Any other line (code, use statements, etc.) means we're past the header
        break;
    }

    ParsedModuleMetadata {
        description: description_lines.join(" "),
        cli_command: None,
        cli_status: None,
        status,
        explicit_name: None,
        explicit_category: None,
    }
}

/// Extract documentation from binary file
fn extract_binary_documentation(content: &str) -> String {
    let mut docs = Vec::new();

    for line in content.lines() {
        let trimmed = line.trim();
        if !trimmed.starts_with("//!") {
            // Skip shebangs (e.g., #!/usr/bin/env cargo) in addition to comments and attributes
            let is_shebang = trimmed.starts_with("#!") && trimmed.chars().nth(2) == Some('/');
            if !trimmed.starts_with("//") && !trimmed.is_empty() && !trimmed.starts_with("#![") && !is_shebang {
                break;
            }
            continue;
        }

        let doc_content = trimmed.trim_start_matches("//!").trim();
        if !doc_content.starts_with('@') {
            docs.push(doc_content.to_string());
        }
    }

    docs.join("\n")
}

fn discover_module_recursive(
    crate_src_root: &Path,
    base_path: &Path,
    mod_name: &str,
    parent_path: &str,
    modules: &mut Vec<ModuleInfo>,
) {
    let mod_dir = base_path.join(mod_name);
    let mod_file = mod_dir.join("mod.rs");
    let single_file = base_path.join(format!("{}.rs", mod_name));

    let (source_path, content) = if mod_file.exists() {
        (mod_file.clone(), fs::read_to_string(&mod_file).ok())
    } else if single_file.exists() {
        (single_file.clone(), fs::read_to_string(&single_file).ok())
    } else {
        return;
    };

    let content = match content {
        Some(c) => c,
        None => return,
    };

    let full_path = if parent_path == "root" {
        mod_name.to_string()
    } else {
        format!("{}::{}", parent_path, mod_name)
    };

    let metadata = parse_module_metadata(&content);
    let docs = extract_module_documentation(&content);
    let capability_tags = infer_capability_tags_with_methods(mod_name, &docs, &[]);
    let children: Vec<String> = parse_pub_mod_declarations(&content);

    // M-599: Use explicit name if provided via @name marker, otherwise use filename-derived name
    let module_name = metadata
        .explicit_name
        .unwrap_or_else(|| mod_name.to_string());

    // M-599: Use explicit category if provided via @category marker, otherwise infer from path
    let inferred_category = if parent_path == "root" {
        "core".to_string()
    } else {
        parent_path.split("::").next().unwrap_or("core").to_string()
    };
    let category = metadata.explicit_category.unwrap_or(inferred_category);

    let description = build_module_description(&source_path, &content, &docs);

    let info = ModuleInfo {
        name: module_name,
        path: full_path.clone(),
        category,
        description,
        capability_tags,
        source_path: source_path
            .strip_prefix(crate_src_root)
            .unwrap_or(&source_path)
            .to_path_buf(),
        children: children.clone(),
        cli_command: metadata.cli_command,
        cli_status: metadata.cli_status,
        status: metadata.status,
    };

    modules.push(info);

    for child in children {
        discover_module_recursive(crate_src_root, &mod_dir, &child, &full_path, modules);
    }
}

fn parse_pub_mod_declarations(content: &str) -> Vec<String> {
    let mut mods = Vec::new();
    for line in content.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("pub mod ") {
            if let Some(name) = rest
                .split(|c: char| c == ';' || c == '{' || c.is_whitespace())
                .next()
            {
                if !name.is_empty() {
                    mods.push(name.to_string());
                }
            }
        }
    }
    mods
}

/// Parsed module metadata from doc comments
#[derive(Debug, Default)]
struct ParsedModuleMetadata {
    description: String,
    cli_command: Option<String>,
    cli_status: Option<CliStatus>,
    status: ModuleStatus,
    /// Explicit module name from @name marker (M-599)
    explicit_name: Option<String>,
    /// Explicit category from @category marker (M-599)
    explicit_category: Option<String>,
}

fn parse_module_metadata(content: &str) -> ParsedModuleMetadata {
    let mut description_lines = Vec::new();
    let mut cli_command = None;
    let mut cli_status = None;
    let mut status = ModuleStatus::Stable;
    let mut explicit_name = None;
    let mut explicit_category = None;
    let mut found_blank = false;

    for line in content.lines() {
        let trimmed = line.trim();
        if !trimmed.starts_with("//!") {
            // Skip regular comments, empty lines, and inner attributes (#![...])
            // Inner attributes like #![allow(...)] often appear before doc comments
            if !trimmed.starts_with("//") && !trimmed.is_empty() && !trimmed.starts_with("#![") {
                break;
            }
            continue;
        }

        let doc_content = trimmed.trim_start_matches("//!").trim();

        if let Some(rest) = doc_content.strip_prefix("@cli ") {
            cli_command = Some(rest.trim().to_string());
        } else if let Some(rest) = doc_content.strip_prefix("@cli-status ") {
            cli_status = Some(match rest.trim() {
                "wired" => CliStatus::Wired,
                "stub" => CliStatus::Stub,
                _ => CliStatus::None,
            });
        } else if let Some(rest) = doc_content.strip_prefix("@status ") {
            status = match rest.trim() {
                "deprecated" => ModuleStatus::Deprecated,
                "experimental" => ModuleStatus::Experimental,
                _ => ModuleStatus::Stable,
            };
        } else if let Some(rest) = doc_content.strip_prefix("@name ") {
            // M-599: Parse @name marker for explicit module name
            let name = rest.trim();
            if !name.is_empty() {
                explicit_name = Some(name.to_string());
            }
        } else if let Some(rest) = doc_content.strip_prefix("@category ") {
            // M-599: Parse @category marker for explicit category
            let category = rest.trim();
            if !category.is_empty() {
                explicit_category = Some(category.to_string());
            }
        } else if doc_content == "@dashflow-module" {
            // M-599: @dashflow-module is just a presence marker, no value to extract
            // This is handled implicitly - the module is already being discovered
        } else if !doc_content.starts_with('@') {
            if doc_content.is_empty() {
                if !description_lines.is_empty() {
                    found_blank = true;
                }
            } else if !found_blank {
                let text = doc_content.trim_start_matches('#').trim();
                if !text.is_empty() {
                    description_lines.push(text.to_string());
                }
            }
        }
    }

    ParsedModuleMetadata {
        description: description_lines.join(" "),
        cli_command,
        cli_status,
        status,
        explicit_name,
        explicit_category,
    }
}

fn extract_module_documentation(content: &str) -> String {
    let mut docs = Vec::new();

    for line in content.lines() {
        let trimmed = line.trim();
        if !trimmed.starts_with("//!") {
            // Skip regular comments, empty lines, and inner attributes (#![...])
            if !trimmed.starts_with("//") && !trimmed.is_empty() && !trimmed.starts_with("#![") {
                break;
            }
            continue;
        }

        let doc_content = trimmed.trim_start_matches("//!").trim();
        if !doc_content.starts_with('@') {
            docs.push(doc_content.to_string());
        }
    }

    docs.join("\n")
}

const MAX_MODULE_DESCRIPTION_LEN: usize = 800;

fn build_module_description(source_path: &Path, content: &str, module_docs: &str) -> String {
    let normalized_module_docs = normalize_doc_text(module_docs);
    if !normalized_module_docs.is_empty() {
        return truncate_text(&normalized_module_docs, MAX_MODULE_DESCRIPTION_LEN);
    }

    if let Some(type_docs) = extract_primary_exported_type_docs(content) {
        let normalized = normalize_doc_text(&type_docs);
        if !normalized.is_empty() {
            return truncate_text(&normalized, MAX_MODULE_DESCRIPTION_LEN);
        }
    }

    if let Some(readme) = extract_readme_fallback(source_path) {
        let normalized = normalize_doc_text(&readme);
        if !normalized.is_empty() {
            return truncate_text(&normalized, MAX_MODULE_DESCRIPTION_LEN);
        }
    }

    String::new()
}

fn normalize_doc_text(text: &str) -> String {
    // Keep this single-line to avoid breaking tabular CLI output, but include all paragraphs.
    let mut parts = Vec::new();
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let without_md_heading = trimmed.trim_start_matches('#').trim();
        if !without_md_heading.is_empty() {
            parts.push(without_md_heading);
        }
    }
    parts.join(" ")
}

fn truncate_text(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        return s.to_string();
    }

    let target_len = max_len.saturating_sub(3);
    let truncate_at = s
        .char_indices()
        .take_while(|(i, _)| *i < target_len)
        .last()
        .map(|(i, c)| i + c.len_utf8())
        .unwrap_or(0);
    format!("{}...", &s[..truncate_at])
}

fn extract_primary_exported_type_docs(content: &str) -> Option<String> {
    let syntax = syn::parse_file(content).ok()?;
    for item in &syntax.items {
        match item {
            syn::Item::Struct(s) if is_vis_public(&s.vis) => {
                let docs = extract_doc_comments(&s.attrs);
                if !docs.trim().is_empty() {
                    return Some(docs);
                }
            }
            syn::Item::Enum(e) if is_vis_public(&e.vis) => {
                let docs = extract_doc_comments(&e.attrs);
                if !docs.trim().is_empty() {
                    return Some(docs);
                }
            }
            syn::Item::Trait(t) if is_vis_public(&t.vis) => {
                let docs = extract_doc_comments(&t.attrs);
                if !docs.trim().is_empty() {
                    return Some(docs);
                }
            }
            syn::Item::Type(t) if is_vis_public(&t.vis) => {
                let docs = extract_doc_comments(&t.attrs);
                if !docs.trim().is_empty() {
                    return Some(docs);
                }
            }
            _ => {}
        }
    }
    None
}

fn extract_readme_fallback(source_path: &Path) -> Option<String> {
    let module_dir = source_path.parent()?;
    if let Ok(content) = fs::read_to_string(module_dir.join("README.md")) {
        let summary = summarize_readme(&content);
        if !summary.trim().is_empty() {
            return Some(summary);
        }
    }

    let crate_root = find_crate_root_from_source(source_path)?;
    if let Ok(content) = fs::read_to_string(crate_root.join("README.md")) {
        let summary = summarize_readme(&content);
        if !summary.trim().is_empty() {
            return Some(summary);
        }
    }

    None
}

fn find_crate_root_from_source(source_path: &Path) -> Option<PathBuf> {
    for ancestor in source_path.ancestors() {
        if ancestor.file_name().is_some_and(|name| name == "src") {
            return ancestor.parent().map(Path::to_path_buf);
        }
    }

    for ancestor in source_path.ancestors() {
        if ancestor.join("Cargo.toml").exists() {
            return Some(ancestor.to_path_buf());
        }
    }

    None
}

fn summarize_readme(content: &str) -> String {
    let mut paragraphs = Vec::<String>::new();
    let mut current = Vec::<String>::new();
    let mut in_code_block = false;

    for raw_line in content.lines() {
        let line = raw_line.trim();

        if line.starts_with("```") {
            in_code_block = !in_code_block;
            if in_code_block {
                break;
            }
            continue;
        }

        if in_code_block {
            continue;
        }

        if line.is_empty() {
            if !current.is_empty() {
                paragraphs.push(current.join(" "));
                current.clear();
                if paragraphs.len() >= 2 {
                    break;
                }
            }
            continue;
        }

        // Skip headings and common badge lines.
        if line.starts_with('#')
            || line.starts_with("[![")
            || line.starts_with("![")
            || line.starts_with("<img")
            || line.starts_with("<!--")
        {
            continue;
        }

        current.push(line.to_string());
    }

    if paragraphs.is_empty() && !current.is_empty() {
        paragraphs.push(current.join(" "));
    }

    paragraphs.join(" ")
}

fn propagate_module_capability_tags(modules: &mut [ModuleInfo]) {
    let mut path_to_index = std::collections::HashMap::new();
    for (idx, module) in modules.iter().enumerate() {
        path_to_index.insert(module.path.clone(), idx);
    }

    let mut indices: Vec<usize> = (0..modules.len()).collect();
    indices.sort_by_key(|idx| std::cmp::Reverse(modules[*idx].path.split("::").count()));

    for idx in indices {
        let child_tags = modules[idx].capability_tags.clone();
        if child_tags.is_empty() {
            continue;
        }

        let Some((parent_path, _)) = modules[idx].path.rsplit_once("::") else {
            continue;
        };

        let Some(&parent_idx) = path_to_index.get(parent_path) else {
            continue;
        };

        modules[parent_idx].capability_tags.extend(child_tags);
        modules[parent_idx].capability_tags.sort();
        modules[parent_idx].capability_tags.dedup();
    }
}

/// Generate Rust code for the discovered modules (for build.rs codegen)
pub fn generate_registry_code(modules: &[ModuleInfo]) -> String {
    let mut code = String::new();
    code.push_str("// AUTO-GENERATED BY dashflow-introspection - DO NOT EDIT\n\n");
    code.push_str("pub static DISCOVERED_MODULES: &[ModuleInfo] = &[\n");

    for module in modules {
        let cli_cmd = module
            .cli_command
            .as_ref()
            .map_or("None".to_string(), |c| format!("Some(\"{}\")", c));
        let cli_st = module
            .cli_status
            .map_or("None".to_string(), |s| format!("Some(CliStatus::{:?})", s));
        // Escape description for use in raw string
        let desc_escaped = module.description.replace('#', "");
        code.push_str("    ModuleInfo {\n");
        code.push_str(&format!("        name: \"{}\",\n", module.name));
        code.push_str(&format!("        path: \"{}\",\n", module.path));
        code.push_str(&format!("        category: \"{}\",\n", module.category));
        code.push_str(&format!("        description: \"{}\",\n", desc_escaped));
        code.push_str(&format!(
            "        capability_tags: &{:?},\n",
            module.capability_tags
        ));
        code.push_str(&format!(
            "        source_path: \"{}\",\n",
            module.source_path.display()
        ));
        code.push_str(&format!("        cli_command: {},\n", cli_cmd));
        code.push_str(&format!("        cli_status: {},\n", cli_st));
        code.push_str(&format!(
            "        status: ModuleStatus::{:?},\n",
            module.status
        ));
        code.push_str("    },\n");
    }

    code.push_str("];\n");
    code
}

/// Export modules as JSON
pub fn to_json(modules: &[ModuleInfo]) -> String {
    serde_json::to_string_pretty(modules).unwrap_or_else(|_| "[]".to_string())
}

// ============================================================================
// TYPE-LEVEL DISCOVERY
// ============================================================================

/// Type kind for discovered types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TypeKind {
    /// A struct definition
    Struct,
    /// An enum definition
    Enum,
    /// A trait definition
    Trait,
    /// A function definition
    Function,
    /// A type alias
    TypeAlias,
    /// A constant
    Const,
}

/// Information about a discovered type (struct, trait, enum, fn)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TypeInfo {
    /// Type name (e.g., "OpenSearchBM25Retriever")
    pub name: String,
    /// Fully qualified path (e.g., "dashflow_opensearch::OpenSearchBM25Retriever")
    pub path: String,
    /// Crate containing this type (e.g., "dashflow-opensearch")
    pub crate_name: String,
    /// Type kind (struct, enum, trait, fn)
    pub kind: TypeKind,
    /// Doc comment (first paragraph)
    pub description: String,
    /// Full doc comment
    pub documentation: String,
    /// Source file path
    pub source_path: PathBuf,
    /// Line number where defined
    pub line_number: usize,
    /// Whether it's public
    pub is_public: bool,
    /// Capability tags inferred from name/docs (e.g., ["search", "retriever"])
    pub capability_tags: Vec<String>,
}

/// Discover all public types in a Rust source file
pub fn discover_types_in_file(
    file_path: &Path,
    crate_name: &str,
    module_path: &str,
) -> Vec<TypeInfo> {
    let content = match fs::read_to_string(file_path) {
        Ok(c) => c,
        Err(_) => return Vec::new(),
    };

    let syntax = match syn::parse_file(&content) {
        Ok(s) => s,
        Err(_) => return Vec::new(),
    };

    let mut types = Vec::new();

    // First pass: collect all items to find impl blocks later
    let items = &syntax.items;

    for item in items {
        if let Some(type_info) =
            extract_type_info_enhanced(item, items, crate_name, module_path, file_path)
        {
            types.push(type_info);
        }
    }

    types
}

/// Extract type information from a syn Item (basic version without method signatures)
#[allow(dead_code)] // Test infrastructure: Used by external crate tests
fn extract_type_info(
    item: &syn::Item,
    crate_name: &str,
    module_path: &str,
    file_path: &Path,
) -> Option<TypeInfo> {
    extract_type_info_enhanced(item, &[], crate_name, module_path, file_path)
}

/// Extract type information with enhanced capability detection
///
/// This version:
/// - Extracts method signatures from impl blocks for capability inference
/// - Supports #[dashflow::capability(...)] explicit annotations
/// - Falls back to name/doc inference for types without impl blocks
fn extract_type_info_enhanced(
    item: &syn::Item,
    all_items: &[syn::Item],
    crate_name: &str,
    module_path: &str,
    file_path: &Path,
) -> Option<TypeInfo> {
    // Note: Line numbers require proc-macro2 span-locations feature which has
    // significant performance overhead. Using 0 as placeholder for now.
    let (name, kind, docs, attrs, is_public) = match item {
        syn::Item::Struct(s) if is_vis_public(&s.vis) => {
            let name = s.ident.to_string();
            let docs = extract_doc_comments(&s.attrs);
            (name, TypeKind::Struct, docs, &s.attrs[..], true)
        }
        syn::Item::Enum(e) if is_vis_public(&e.vis) => {
            let name = e.ident.to_string();
            let docs = extract_doc_comments(&e.attrs);
            (name, TypeKind::Enum, docs, &e.attrs[..], true)
        }
        syn::Item::Trait(t) if is_vis_public(&t.vis) => {
            let name = t.ident.to_string();
            let docs = extract_doc_comments(&t.attrs);
            (name, TypeKind::Trait, docs, &t.attrs[..], true)
        }
        syn::Item::Fn(f) if is_vis_public(&f.vis) => {
            let name = f.sig.ident.to_string();
            let docs = extract_doc_comments(&f.attrs);
            (name, TypeKind::Function, docs, &f.attrs[..], true)
        }
        syn::Item::Type(t) if is_vis_public(&t.vis) => {
            let name = t.ident.to_string();
            let docs = extract_doc_comments(&t.attrs);
            (name, TypeKind::TypeAlias, docs, &t.attrs[..], true)
        }
        // Gap 16: Handle pub use re-exports
        syn::Item::Use(u) if is_vis_public(&u.vis) => {
            // Extract re-exported types from use statements
            // This captures pub use foo::Bar and pub use foo::{Bar, Baz}
            return extract_reexported_types(u, crate_name, module_path, file_path);
        }
        _ => return None,
    };

    let full_path = if module_path.is_empty() {
        format!("{}::{}", crate_name.replace('-', "_"), name)
    } else {
        format!(
            "{}::{}::{}",
            crate_name.replace('-', "_"),
            module_path,
            name
        )
    };

    let description = docs.lines().next().unwrap_or("").to_string();

    // Check for explicit capability attributes first
    let mut capability_tags = extract_capability_attribute(attrs);

    // If no explicit attributes, infer from method signatures
    if capability_tags.is_empty() {
        let method_signatures = extract_method_signatures_for_type(all_items, &name);
        capability_tags = infer_capability_tags_with_methods(&name, &docs, &method_signatures);
    } else {
        // If explicit attributes exist, merge with inferred tags
        let method_signatures = extract_method_signatures_for_type(all_items, &name);
        let inferred = infer_capability_tags_with_methods(&name, &docs, &method_signatures);
        for tag in inferred {
            if !capability_tags.contains(&tag) {
                capability_tags.push(tag);
            }
        }
        capability_tags.sort();
        capability_tags.dedup();
    }

    Some(TypeInfo {
        name,
        path: full_path,
        crate_name: crate_name.to_string(),
        kind,
        description,
        documentation: docs,
        source_path: file_path.to_path_buf(),
        line_number: 0, // Line numbers require proc-macro2 span-locations feature
        is_public,
        capability_tags,
    })
}

/// Check if visibility is public
fn is_vis_public(vis: &syn::Visibility) -> bool {
    matches!(vis, syn::Visibility::Public(_))
}

/// Extract re-exported types from pub use statements (Gap 16)
///
/// Handles patterns like:
/// - `pub use foo::Bar;`
/// - `pub use foo::Bar as Qux;`
/// - `pub use foo::{Bar, Baz};` (returns first type only for simplicity)
///
/// Note: For grouped re-exports, this currently returns only the first type.
/// A future enhancement could modify the caller to collect all re-exports.
fn extract_reexported_types(
    use_item: &syn::ItemUse,
    crate_name: &str,
    module_path: &str,
    file_path: &Path,
) -> Option<TypeInfo> {
    let docs = extract_doc_comments(&use_item.attrs);

    // Extract the re-exported name from the use tree
    let (name, source_path_str) = extract_use_tree_name(&use_item.tree)?;

    // PascalCase names are likely types, snake_case are likely modules/functions
    // Only capture likely type names (start with uppercase or all caps)
    let first_char = name.chars().next()?;
    if !first_char.is_uppercase() {
        return None;
    }

    let full_path = if module_path.is_empty() {
        format!("{}::{}", crate_name.replace('-', "_"), name)
    } else {
        format!(
            "{}::{}::{}",
            crate_name.replace('-', "_"),
            module_path,
            name
        )
    };

    // Use doc comment if present, otherwise indicate source
    let description = if let Some(first_line) = docs.lines().next().filter(|s| !s.is_empty()) {
        first_line.to_string()
    } else {
        format!("Re-exported from {}", source_path_str)
    };

    let mut capability_tags = extract_capability_attribute(&use_item.attrs);
    let inferred = infer_capability_tags_with_methods(&name, &docs, &[]);
    for tag in inferred {
        if !capability_tags.contains(&tag) {
            capability_tags.push(tag);
        }
    }
    capability_tags.sort();
    capability_tags.dedup();

    Some(TypeInfo {
        name,
        path: full_path,
        crate_name: crate_name.to_string(),
        kind: TypeKind::Struct, // Assume struct for re-exports, could be trait/enum
        description,
        documentation: docs,
        source_path: file_path.to_path_buf(),
        line_number: 0,
        is_public: true,
        capability_tags,
    })
}

/// Extract the final name and source path from a use tree
fn extract_use_tree_name(tree: &syn::UseTree) -> Option<(String, String)> {
    match tree {
        syn::UseTree::Path(path) => {
            // Recursively follow the path
            let (name, inner_path) = extract_use_tree_name(&path.tree)?;
            let source = format!("{}::{}", path.ident, inner_path);
            Some((name, source))
        }
        syn::UseTree::Name(name) => {
            let ident = name.ident.to_string();
            Some((ident.clone(), ident))
        }
        syn::UseTree::Rename(rename) => {
            // `pub use foo::Bar as Qux;` - use the renamed name
            let local_name = rename.rename.to_string();
            let original = rename.ident.to_string();
            Some((local_name, original))
        }
        syn::UseTree::Group(group) => {
            // `pub use foo::{Bar, Baz};` - return first item only
            group.items.first().and_then(extract_use_tree_name)
        }
        syn::UseTree::Glob(_) => {
            // `pub use foo::*;` - can't determine specific types
            None
        }
    }
}

/// Extract doc comments from attributes
fn extract_doc_comments(attrs: &[syn::Attribute]) -> String {
    attrs
        .iter()
        .filter_map(|attr| {
            if attr.path().is_ident("doc") {
                if let syn::Meta::NameValue(meta) = &attr.meta {
                    if let syn::Expr::Lit(syn::ExprLit {
                        lit: syn::Lit::Str(s),
                        ..
                    }) = &meta.value
                    {
                        return Some(s.value().trim().to_string());
                    }
                }
            }
            None
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Split a type name into component words
///
/// Handles PascalCase, camelCase, and snake_case:
/// - "OpenSearchBM25Retriever" -> ["open", "search", "bm25", "retriever"]
/// - "vector_store" -> ["vector", "store"]
fn split_type_name(name: &str) -> Vec<String> {
    let mut components = Vec::new();
    let mut current = String::new();

    // First split on underscores
    for part in name.split('_') {
        // Then handle camelCase/PascalCase
        let chars: Vec<char> = part.chars().collect();

        for (i, &c) in chars.iter().enumerate() {
            let is_upper = c.is_uppercase();
            let next_is_lower = chars.get(i + 1).is_some_and(|next| next.is_lowercase());

            if is_upper && !current.is_empty() {
                // Start new part if:
                // - Current part has lowercase (transitioning from lower to upper)
                // - Or this uppercase is followed by lowercase (end of acronym like BM25)
                let current_has_lower = current.chars().any(|ch| ch.is_lowercase());
                if current_has_lower || next_is_lower {
                    if !current.is_empty() {
                        components.push(current.to_lowercase());
                    }
                    current = String::new();
                }
            }
            current.push(c);
        }

        if !current.is_empty() {
            components.push(current.to_lowercase());
            current = String::new();
        }
    }

    components
}

/// Infer capability tags from type name and documentation
#[allow(dead_code)] // Public API: Helper function for backwards compatibility
fn infer_capability_tags(name: &str, docs: &str) -> Vec<String> {
    infer_capability_tags_with_methods(name, docs, &[])
}

/// Infer capability tags from type name, documentation, and method signatures
///
/// Uses comprehensive pattern matching for type names, doc comments,
/// and semantic phrases to auto-discover capabilities.
fn infer_capability_tags_with_methods(
    name: &str,
    docs: &str,
    method_signatures: &[String],
) -> Vec<String> {
    let mut tags = Vec::new();
    let name_lower = name.to_lowercase();
    let docs_lower = docs.to_lowercase();
    let combined = format!("{} {}", name_lower, docs_lower);

    // Primary capability patterns for name/docs
    let patterns = [
        // Retrievers & Search
        ("retriev", "retriever"),
        ("search", "search"),
        ("bm25", "bm25"),
        ("tf-idf", "bm25"), // Often used together with BM25
        ("tfidf", "bm25"),
        ("keyword_search", "bm25"),
        ("fulltext", "search"),
        ("full_text", "search"),
        ("hybrid", "hybrid_search"),
        ("rerank", "reranking"),
        ("cross_encoder", "reranking"),
        // Vector stores & embeddings
        ("vector", "vector_store"),
        ("vector_store", "vector_store"),
        ("vectorstore", "vector_store"),
        ("embed", "embeddings"),
        ("embedding", "embeddings"),
        ("similarity", "similarity_search"),
        ("cosine", "similarity_search"),
        ("dense", "dense_retrieval"),
        ("sparse", "sparse_retrieval"),
        // Language models
        ("llm", "llm"),
        ("chat", "chat"),
        ("chatmodel", "chat"),
        ("complet", "completion"),
        ("generation", "text_generation"),
        ("generate", "text_generation"),
        ("inference", "inference"),
        // Processing
        ("chunk", "chunking"),
        ("split", "splitting"),
        ("text_splitter", "chunking"),
        ("textsplitter", "chunking"),
        ("load", "document_loader"),
        ("loader", "document_loader"),
        ("pars", "parsing"),
        ("parser", "parsing"),
        ("extract", "extraction"),
        ("transform", "transformation"),
        ("compress", "compression"),
        // Evaluation & quality
        ("eval", "evaluation"),
        ("score", "scoring"),
        ("metric", "metrics"),
        ("benchmark", "benchmarking"),
        ("test", "testing"),
        ("quality", "quality"),
        // Observability & tracking
        ("cost", "cost_tracking"),
        ("token", "tokenization"),
        ("trace", "tracing"),
        ("span", "tracing"),
        ("log", "logging"),
        ("telemetry", "telemetry"),
        ("prometheus", "metrics"),
        // Optimization
        ("optimi", "optimization"),
        ("distill", "distillation"),
        ("finetun", "finetuning"),
        ("fine_tun", "finetuning"),
        ("bootstrap", "bootstrapping"),
        ("prompt_optim", "prompt_optimization"),
        // Memory & caching
        ("cache", "caching"),
        ("memory", "memory"),
        ("checkpoint", "checkpointing"),
        ("persist", "persistence"),
        // Streaming & async
        ("stream", "streaming"),
        ("kafka", "kafka"),
        ("rdkafka", "kafka"),
        ("callback", "callbacks"),
        ("async", "async_support"),
        ("batch", "batching"),
        // Chains & graphs
        ("chain", "chain"),
        ("graph", "graph"),
        ("workflow", "workflow"),
        ("pipeline", "pipeline"),
        ("runnable", "runnable"),
        // Tools & agents
        ("tool", "tool"),
        ("agent", "agent"),
        ("function_call", "function_calling"),
        ("tool_call", "function_calling"),
        // RAG specific
        ("rag", "rag"),
        ("qa", "qa"),
        ("question_answer", "qa"),
        ("synthes", "synthesis"),
        ("answer", "answer_generation"),
        ("context", "context_management"),
        ("merg", "merging"),
    ];

    for (pattern, tag) in &patterns {
        if combined.contains(pattern) {
            tags.push(tag.to_string());
        }
    }

    // Phrase-based detection in doc comments
    // These are multi-word phrases that indicate specific capabilities
    let doc_phrases = [
        ("keyword search", vec!["bm25", "search", "retriever"]),
        ("bm25 search", vec!["bm25", "search", "retriever"]),
        (
            "semantic search",
            vec!["embeddings", "vector_store", "similarity_search"],
        ),
        (
            "similarity search",
            vec!["vector_store", "similarity_search"],
        ),
        (
            "dense retrieval",
            vec!["embeddings", "dense_retrieval", "retriever"],
        ),
        (
            "sparse retrieval",
            vec!["bm25", "sparse_retrieval", "retriever"],
        ),
        ("hybrid search", vec!["hybrid_search", "retriever"]),
        (
            "answer synthesis",
            vec!["synthesis", "qa", "text_generation"],
        ),
        ("question answering", vec!["qa", "retriever"]),
        ("document retrieval", vec!["retriever", "document_loader"]),
        ("text generation", vec!["llm", "text_generation"]),
        ("chat completion", vec!["chat", "completion"]),
        ("language model", vec!["llm"]),
        (
            "prompt optimization",
            vec!["optimization", "prompt_optimization"],
        ),
        ("cost tracking", vec!["cost_tracking", "observability"]),
        ("token count", vec!["tokenization", "cost_tracking"]),
        ("execution trace", vec!["tracing", "observability"]),
        ("vector database", vec!["vector_store"]),
        ("embedding model", vec!["embeddings"]),
        ("text splitter", vec!["chunking", "splitting"]),
        ("document loader", vec!["document_loader"]),
        ("retrieval augmented", vec!["rag", "retriever"]),
        ("kafka consumer", vec!["kafka", "streaming"]),
        ("kafka producer", vec!["kafka", "streaming"]),
        ("apache kafka", vec!["kafka", "streaming"]),
    ];

    for (phrase, phrase_tags) in &doc_phrases {
        if docs_lower.contains(phrase) {
            for tag in phrase_tags {
                tags.push(tag.to_string());
            }
        }
    }

    // Type name component analysis
    // Split PascalCase/camelCase names and infer from components
    let name_components = split_type_name(&name_lower);
    let component_patterns = [
        ("retriever", "retriever"),
        ("store", "vector_store"),
        ("embedder", "embeddings"),
        ("embeddings", "embeddings"),
        ("loader", "document_loader"),
        ("splitter", "chunking"),
        ("chunker", "chunking"),
        ("parser", "parsing"),
        ("evaluator", "evaluation"),
        ("optimizer", "optimization"),
        ("tracker", "tracking"),
        ("cache", "caching"),
        ("memory", "memory"),
        ("chain", "chain"),
        ("graph", "graph"),
        ("agent", "agent"),
        ("tool", "tool"),
        ("model", "model"),
        ("client", "client"),
    ];

    for component in &name_components {
        for (pattern, tag) in &component_patterns {
            if component == *pattern {
                tags.push(tag.to_string());
            }
        }
    }

    // Method signature patterns
    // These patterns detect capability based on method names that follow
    // LangChain/DashFlow conventions
    let method_patterns = [
        ("get_relevant_documents", "retriever"),
        ("invoke", "runnable"),
        ("ainvoke", "runnable"),
        ("stream", "streaming"),
        ("astream", "streaming"),
        ("batch", "batching"),
        ("abatch", "batching"),
        ("embed_documents", "embeddings"),
        ("embed_query", "embeddings"),
        ("similarity_search", "vector_store"),
        ("add_documents", "vector_store"),
        ("delete", "vector_store"),
        ("generate", "llm"),
        ("chat", "chat"),
        ("complete", "completion"),
        ("split_text", "chunking"),
        ("split_documents", "chunking"),
        ("load", "document_loader"),
        ("score", "evaluation"),
        ("evaluate", "evaluation"),
        ("record_call", "cost_tracking"),
        ("record_cost", "cost_tracking"),
        ("track_cost", "cost_tracking"),
        ("get_metrics", "metrics"),
        ("record_metric", "metrics"),
        ("compress_documents", "compression"),
        ("transform_documents", "transformation"),
    ];

    // Check each method signature against patterns
    for sig in method_signatures {
        let sig_lower = sig.to_lowercase();
        for (method_pattern, tag) in &method_patterns {
            if sig_lower.contains(method_pattern) {
                tags.push(tag.to_string());
            }
        }
    }

    tags.sort();
    tags.dedup();
    tags
}

/// Extract explicit capability tags from #[dashflow::capability(...)] attribute
fn extract_capability_attribute(attrs: &[syn::Attribute]) -> Vec<String> {
    let mut tags = Vec::new();

    for attr in attrs {
        // Check for #[dashflow::capability(...)] or #[capability(...)]
        let is_dashflow_capability = attr.path().segments.len() == 2
            && attr
                .path()
                .segments
                .first()
                .is_some_and(|s| s.ident == "dashflow")
            && attr
                .path()
                .segments
                .last()
                .is_some_and(|s| s.ident == "capability");

        let is_simple_capability = attr.path().is_ident("capability");

        if is_dashflow_capability || is_simple_capability {
            // Parse the attribute arguments by iterating over nested meta
            if let syn::Meta::List(meta_list) = &attr.meta {
                // Parse tokens as comma-separated string literals
                let tokens_str = meta_list.tokens.to_string();
                for token in tokens_str.split(',') {
                    let token = token.trim();
                    // Remove quotes from string literal
                    let tag = token.trim_matches('"').trim_matches('\'').trim();
                    if !tag.is_empty() {
                        tags.push(tag.to_string());
                    }
                }
            }
        }
    }

    tags
}

/// Extract method signatures from impl blocks for a given type
fn extract_method_signatures_for_type(items: &[syn::Item], type_name: &str) -> Vec<String> {
    let mut signatures = Vec::new();

    for item in items {
        if let syn::Item::Impl(impl_block) = item {
            // Check if this impl is for the target type
            let impl_type_name = if let syn::Type::Path(type_path) = impl_block.self_ty.as_ref() {
                type_path.path.segments.last().map(|s| s.ident.to_string())
            } else {
                None
            };

            if impl_type_name.as_deref() != Some(type_name) {
                continue;
            }

            // Extract public method signatures
            for impl_item in &impl_block.items {
                if let syn::ImplItem::Fn(method) = impl_item {
                    if is_vis_public(&method.vis) || impl_block.trait_.is_some() {
                        let sig = format_method_signature(&method.sig);
                        signatures.push(sig);
                    }
                }
            }
        }
    }

    signatures
}

/// Format a method signature as a string (simplified for pattern matching)
fn format_method_signature(sig: &syn::Signature) -> String {
    let name = sig.ident.to_string();
    let is_async = sig.asyncness.is_some();
    let param_count = sig.inputs.len();

    let async_str = if is_async { "async " } else { "" };
    format!("{}fn {}({} params)", async_str, name, param_count)
}

/// Discover all types in a crate
pub fn discover_types_in_crate(crate_src: &Path, crate_name: &str) -> Vec<TypeInfo> {
    let mut types = Vec::new();
    discover_types_recursive(crate_src, crate_name, "", &mut types);
    types
}

/// Recursively discover types in a directory
fn discover_types_recursive(
    dir: &Path,
    crate_name: &str,
    module_path: &str,
    types: &mut Vec<TypeInfo>,
) {
    // Check lib.rs or mod.rs
    let lib_rs = dir.join("lib.rs");
    let mod_rs = dir.join("mod.rs");

    let main_file = if lib_rs.exists() {
        Some(lib_rs)
    } else if mod_rs.exists() {
        Some(mod_rs)
    } else {
        None
    };

    if let Some(main_file) = main_file {
        let discovered = discover_types_in_file(&main_file, crate_name, module_path);
        types.extend(discovered);
    }

    // Scan for other .rs files and subdirectories
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.filter_map(Result::ok) {
            let path = entry.path();
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");

            if path.is_file() && name.ends_with(".rs") && name != "lib.rs" && name != "mod.rs" {
                // Module file (e.g., retriever.rs)
                let mod_name = name.strip_suffix(".rs").unwrap_or(name);
                let child_path = if module_path.is_empty() {
                    mod_name.to_string()
                } else {
                    format!("{}::{}", module_path, mod_name)
                };
                let discovered = discover_types_in_file(&path, crate_name, &child_path);
                types.extend(discovered);
            } else if path.is_dir() && !name.starts_with('.') && name != "target" {
                // Subdirectory (module directory)
                let child_path = if module_path.is_empty() {
                    name.to_string()
                } else {
                    format!("{}::{}", module_path, name)
                };
                discover_types_recursive(&path, crate_name, &child_path, types);
            }
        }
    }
}

/// Discover types from all workspace crates
pub fn discover_all_types(workspace_root: impl AsRef<Path>) -> Vec<TypeInfo> {
    let crates = discover_all_workspace_crates(workspace_root.as_ref());
    let mut all_types = Vec::new();

    for krate in crates {
        let src_path = workspace_root.as_ref().join(&krate.src_path);
        if src_path.exists() {
            let types = discover_types_in_crate(&src_path, &krate.name);
            all_types.extend(types);
        }
    }

    all_types
}

/// Export types as JSON
pub fn types_to_json(types: &[TypeInfo]) -> String {
    serde_json::to_string_pretty(types).unwrap_or_else(|_| "[]".to_string())
}

/// Find types by capability tag
///
/// Returns all types from the workspace that have the specified capability tag.
/// Supports partial matching - e.g., "bm25" will match types tagged with "bm25".
///
/// # Example
///
/// ```rust,no_run
/// use dashflow_module_discovery::find_types_by_capability;
/// use std::path::Path;
///
/// let types = find_types_by_capability(Path::new("."), "retriever");
/// for t in &types {
///     println!("{}: {:?}", t.name, t.capability_tags);
/// }
/// ```
pub fn find_types_by_capability(
    workspace_root: impl AsRef<Path>,
    capability: &str,
) -> Vec<TypeInfo> {
    let all_types = discover_all_types(workspace_root);
    let capability_lower = capability.to_lowercase();

    all_types
        .into_iter()
        .filter(|t| {
            t.capability_tags
                .iter()
                .any(|tag| tag.to_lowercase().contains(&capability_lower))
        })
        .collect()
}

/// Get all unique capability tags in the workspace
///
/// Returns a sorted list of all capability tags discovered across all types.
pub fn get_all_capability_tags(workspace_root: impl AsRef<Path>) -> Vec<String> {
    let all_types = discover_all_types(workspace_root);
    let mut tags: std::collections::HashSet<String> = std::collections::HashSet::new();

    for t in all_types {
        for tag in t.capability_tags {
            tags.insert(tag);
        }
    }

    let mut sorted: Vec<String> = tags.into_iter().collect();
    sorted.sort();
    sorted
}

/// Get capability tag statistics
///
/// Returns a map of capability tag to count of types with that tag.
pub fn get_capability_stats(
    workspace_root: impl AsRef<Path>,
) -> std::collections::HashMap<String, usize> {
    let all_types = discover_all_types(workspace_root);
    let mut stats: std::collections::HashMap<String, usize> = std::collections::HashMap::new();

    for t in all_types {
        for tag in t.capability_tags {
            *stats.entry(tag).or_insert(0) += 1;
        }
    }

    stats
}

#[cfg(test)]
mod tests {
    // `cargo verify` runs clippy with `-D warnings` for *all targets*, including tests.
    // These lints are acceptable in tests where failures should be loud and setup is infallible
    // under normal workspace usage.
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::print_stderr)]

    use super::*;
    use tempfile::TempDir;

    /// Check if a specific function in source code contains TODO comments
    fn check_function_for_todo(content: &str, fn_signature: &str) -> bool {
        let lines: Vec<&str> = content.lines().collect();
        let mut in_function = false;
        let mut brace_depth = 0;

        for line in lines {
            if !in_function {
                // Look for the function signature
                if line.contains(fn_signature) {
                    in_function = true;
                    // Count opening braces on this line
                    brace_depth += line.matches('{').count();
                    brace_depth = brace_depth.saturating_sub(line.matches('}').count());
                }
            } else {
                // We're inside the function
                let trimmed = line.trim();

                // Check for TODO
                if trimmed.starts_with("// TODO")
                    && !trimmed.starts_with("/// TODO")
                    && !trimmed.contains("(future)")
                {
                    return true;
                }

                // Track brace depth
                brace_depth += line.matches('{').count();
                brace_depth = brace_depth.saturating_sub(line.matches('}').count());

                // Exit when we've closed all braces
                if brace_depth == 0 {
                    return false;
                }
            }
        }

        false
    }

    #[test]
    fn test_parse_pub_mod_declarations() {
        let content =
            "//! Module docs\n\npub mod foo;\npub mod bar;\nmod private;\npub mod baz {\n}";
        let mods = parse_pub_mod_declarations(content);
        assert_eq!(mods, vec!["foo", "bar", "baz"]);
    }

    #[test]
    fn test_parse_module_metadata() {
        let content = "//! @cli dashflow train distill\n//! @cli-status stub\n//! @status stable\n//!\n//! Model Distillation Framework\n\npub mod analysis;";
        let metadata = parse_module_metadata(content);
        assert_eq!(metadata.description, "Model Distillation Framework");
        assert_eq!(
            metadata.cli_command,
            Some("dashflow train distill".to_string())
        );
        assert_eq!(metadata.cli_status, Some(CliStatus::Stub));
        assert_eq!(metadata.status, ModuleStatus::Stable);
    }

    #[test]
    fn test_parse_deprecated_module() {
        let content = "//! @status deprecated\n//!\n//! Old module.";
        let metadata = parse_module_metadata(content);
        assert_eq!(metadata.status, ModuleStatus::Deprecated);
    }

    #[test]
    fn test_parse_no_markers() {
        let content =
            "//! Simple module without markers.\n//!\n//! Second paragraph.\n\npub mod child;";
        let metadata = parse_module_metadata(content);
        assert_eq!(metadata.description, "Simple module without markers.");
        assert!(metadata.cli_command.is_none());
        assert!(metadata.cli_status.is_none());
        assert_eq!(metadata.status, ModuleStatus::Stable);
    }

    /// M-599: Test parsing of @dashflow-module, @name, and @category markers
    #[test]
    fn test_parse_dashflow_module_markers() {
        let content = "//! @dashflow-module\n//! @name quality\n//! @category runtime\n//! @status stable\n//!\n//! Quality module for response checking.\n\npub mod gates;";
        let metadata = parse_module_metadata(content);
        assert_eq!(metadata.explicit_name, Some("quality".to_string()));
        assert_eq!(metadata.explicit_category, Some("runtime".to_string()));
        assert_eq!(metadata.status, ModuleStatus::Stable);
        assert_eq!(
            metadata.description,
            "Quality module for response checking."
        );
    }

    /// M-599: Test that explicit category overrides inferred category
    #[test]
    fn test_explicit_category_override() {
        // Module at optimize/optimizers/ would normally get "optimize" as category
        // but @category api should override that
        let content = "//! @dashflow-module\n//! @name optimizer_traits\n//! @category api\n//! @status stable\n//!\n//! Optimizer traits for custom implementations.";
        let metadata = parse_module_metadata(content);
        assert_eq!(metadata.explicit_name, Some("optimizer_traits".to_string()));
        assert_eq!(metadata.explicit_category, Some("api".to_string()));
    }

    /// M-598: Test that shebangs are skipped when parsing binary metadata
    #[test]
    fn test_parse_binary_metadata_with_shebang() {
        // Binary files may start with a shebang line before doc comments
        let content = r#"#!/usr/bin/env cargo
// Copyright 2026 Example
// parse_events - Decode events

//! # DashFlow Streaming Event Parser
//!
//! Command-line tool to consume and decode events.
//! @status stable

use dashflow_streaming::consumer::DashStreamConsumer;
"#;
        let metadata = parse_binary_metadata(content);
        assert_eq!(
            metadata.description,
            "DashFlow Streaming Event Parser"
        );
        assert_eq!(metadata.status, ModuleStatus::Stable);
    }

    /// M-598: Test binary metadata extraction without shebang
    #[test]
    fn test_parse_binary_metadata_no_shebang() {
        let content = r#"// Copyright 2025 Example
//! Binary tool description.
//! @status experimental

fn main() {}
"#;
        let metadata = parse_binary_metadata(content);
        assert_eq!(metadata.description, "Binary tool description.");
        assert_eq!(metadata.status, ModuleStatus::Experimental);
    }

    /// M-598: Test extract_binary_documentation with shebang
    #[test]
    fn test_extract_binary_documentation_with_shebang() {
        let content = r#"#!/usr/bin/env cargo
// Copyright line
//! First doc line
//! Second doc line
//! @status stable
use something;
"#;
        let docs = extract_binary_documentation(content);
        assert!(docs.contains("First doc line"));
        assert!(docs.contains("Second doc line"));
        // @status lines are filtered out
        assert!(!docs.contains("@status"));
    }

    /// M-598: Test parse_binary_metadata on actual parse_events.rs file
    #[test]
    fn test_parse_events_file() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .join("dashflow-streaming/src/bin/parse_events.rs");

        if !path.exists() {
            eprintln!("Skipping: {:?} not found", path);
            return;
        }

        let content = std::fs::read_to_string(&path).expect("read parse_events.rs");
        let metadata = parse_binary_metadata(&content);

        // Should extract the title from //! # `DashFlow Streaming` Event Parser
        assert!(
            !metadata.description.is_empty(),
            "parse_events.rs should have a description, got: '{}'",
            metadata.description
        );
        assert!(
            metadata.description.contains("DashFlow") || metadata.description.contains("Event"),
            "Description should mention DashFlow or Event, got: '{}'",
            metadata.description
        );
    }

    /// M-598: Test discover_binary on actual parse_events.rs file
    #[test]
    fn test_discover_binary_parse_events() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .join("dashflow-streaming/src/bin/parse_events.rs");

        if !path.exists() {
            eprintln!("Skipping: {:?} not found", path);
            return;
        }

        let info = discover_binary(&path, "dashflow-streaming").expect("discover_binary should succeed");

        // Should extract the title from //! # `DashFlow Streaming` Event Parser
        assert!(
            !info.description.is_empty(),
            "parse_events binary should have a description, got: '{}'",
            info.description
        );
        assert!(
            info.description.contains("DashFlow") || info.description.contains("Event"),
            "Description should mention DashFlow or Event, got: '{}'",
            info.description
        );
        assert_eq!(info.name, "parse_events");
        assert_eq!(info.category, "binary");
    }

    #[test]
    fn test_discover_dashflow_modules() {
        let dashflow_src = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .join("dashflow/src");

        if !dashflow_src.exists() {
            return; // Skip if not in workspace context
        }

        let modules = discover_modules(&dashflow_src);
        assert!(
            modules.len() >= 20,
            "Expected at least 20 modules, found {}",
            modules.len()
        );

        // Verify distillation module has markers
        let distillation = modules.iter().find(|m| m.name == "distillation");
        assert!(distillation.is_some(), "Should find distillation module");
        let d = distillation.unwrap();
        assert_eq!(d.cli_command, Some("dashflow train distill".to_string()));
        // Wired the distill CLI to ChatOpenAI
        assert_eq!(d.cli_status, Some(CliStatus::Wired));

        // Verify cost_monitoring is deprecated
        if let Some(cm) = modules.iter().find(|m| m.name == "cost_monitoring") {
            assert_eq!(cm.status, ModuleStatus::Deprecated);
        }
    }

    #[test]
    fn test_module_capability_tags_inferred_and_propagated() {
        let temp_dir = TempDir::new().expect("create temp dir");
        let src_dir = temp_dir.path().join("src");
        std::fs::create_dir_all(src_dir.join("streaming")).expect("create dirs");

        std::fs::write(src_dir.join("lib.rs"), "pub mod streaming;\n").expect("write lib.rs");
        std::fs::write(
            src_dir.join("streaming/mod.rs"),
            "//! Streaming utilities.\n//! \n//! Includes Kafka consumers.\n\npub mod consumer;\n",
        )
        .expect("write streaming/mod.rs");
        std::fs::write(
            src_dir.join("streaming/consumer.rs"),
            "//! Kafka consumer implementation.\n\npub struct KafkaConsumer;\n",
        )
        .expect("write streaming/consumer.rs");

        let modules = discover_modules(&src_dir);
        let streaming = modules
            .iter()
            .find(|m| m.path == "streaming")
            .expect("streaming module discovered");
        let consumer = modules
            .iter()
            .find(|m| m.path == "streaming::consumer")
            .expect("consumer module discovered");

        assert!(
            consumer.capability_tags.iter().any(|t| t == "kafka"),
            "expected consumer module to be tagged with kafka, got {:?}",
            consumer.capability_tags
        );
        assert!(
            streaming.capability_tags.iter().any(|t| t == "kafka"),
            "expected streaming module to inherit kafka tag from child, got {:?}",
            streaming.capability_tags
        );
    }

    #[test]
    fn test_module_description_uses_full_module_docs() {
        let temp_dir = TempDir::new().expect("create temp dir");
        let src_dir = temp_dir.path().join("src");
        std::fs::create_dir_all(src_dir.join("foo")).expect("create dirs");

        std::fs::write(src_dir.join("lib.rs"), "pub mod foo;\n").expect("write lib.rs");
        std::fs::write(
            src_dir.join("foo/mod.rs"),
            "//! First paragraph.\n//!\n//! Second paragraph with more detail.\n\npub mod child;\n",
        )
        .expect("write foo/mod.rs");
        std::fs::write(
            src_dir.join("foo/child.rs"),
            "//! Child module.\n\npub struct Child;\n",
        )
        .expect("write foo/child.rs");

        let modules = discover_modules(&src_dir);
        let foo = modules
            .iter()
            .find(|m| m.path == "foo")
            .expect("foo module discovered");
        assert!(foo.description.contains("First paragraph."));
        assert!(foo.description.contains("Second paragraph with more detail."));
    }

    #[test]
    fn test_module_description_falls_back_to_primary_exported_type_docs() {
        let temp_dir = TempDir::new().expect("create temp dir");
        let src_dir = temp_dir.path().join("src");
        std::fs::create_dir_all(&src_dir).expect("create dirs");

        std::fs::write(src_dir.join("lib.rs"), "pub mod foo;\n").expect("write lib.rs");
        std::fs::write(
            src_dir.join("foo.rs"),
            "/// Primary exported type.\n///\n/// More details here.\npub struct Foo;\n",
        )
        .expect("write foo.rs");

        let modules = discover_modules(&src_dir);
        let foo = modules
            .iter()
            .find(|m| m.path == "foo")
            .expect("foo module discovered");
        assert!(foo.description.contains("Primary exported type."));
        assert!(foo.description.contains("More details here."));
    }

    #[test]
    fn test_module_description_falls_back_to_crate_readme() {
        let temp_dir = TempDir::new().expect("create temp dir");
        let crate_root = temp_dir.path();
        let src_dir = crate_root.join("src");
        std::fs::create_dir_all(&src_dir).expect("create dirs");

        std::fs::write(crate_root.join("README.md"), "# My Crate\n\nThis crate does things.\n\nMore details.\n")
            .expect("write README.md");
        std::fs::write(src_dir.join("lib.rs"), "pub mod foo;\n").expect("write lib.rs");
        std::fs::write(src_dir.join("foo.rs"), "pub fn foo() {}\n").expect("write foo.rs");

        let modules = discover_modules(&src_dir);
        let foo = modules
            .iter()
            .find(|m| m.path == "foo")
            .expect("foo module discovered");
        assert!(foo.description.contains("This crate does things."));
    }

    #[test]
    fn test_source_path_relative_for_nested_modules_under_mod_rs_parent() {
        let temp_dir = TempDir::new().expect("create temp dir");
        let src_dir = temp_dir.path().join("src");
        std::fs::create_dir_all(src_dir.join("foo")).expect("create dirs");

        std::fs::write(src_dir.join("lib.rs"), "pub mod foo;\n").expect("write lib.rs");
        std::fs::write(src_dir.join("foo/mod.rs"), "pub mod child;\n").expect("write foo/mod.rs");
        std::fs::write(
            src_dir.join("foo/child.rs"),
            "//! Child module.\n\npub struct Child;\n",
        )
        .expect("write foo/child.rs");

        let modules = discover_modules(&src_dir);
        let foo = modules
            .iter()
            .find(|m| m.path == "foo")
            .expect("foo module discovered");
        let child = modules
            .iter()
            .find(|m| m.path == "foo::child")
            .expect("foo::child module discovered");

        assert_eq!(foo.source_path, PathBuf::from("foo/mod.rs"));
        assert_eq!(child.source_path, PathBuf::from("foo/child.rs"));
    }

    #[test]
    fn test_discovers_nested_modules_under_file_parent() {
        let temp_dir = TempDir::new().expect("create temp dir");
        let src_dir = temp_dir.path().join("src");
        std::fs::create_dir_all(src_dir.join("foo")).expect("create dirs");

        std::fs::write(src_dir.join("lib.rs"), "pub mod foo;\n").expect("write lib.rs");
        std::fs::write(src_dir.join("foo.rs"), "pub mod child;\n").expect("write foo.rs");
        std::fs::write(
            src_dir.join("foo/child.rs"),
            "//! Child module.\n\npub struct Child;\n",
        )
        .expect("write foo/child.rs");

        let modules = discover_modules(&src_dir);
        let foo = modules
            .iter()
            .find(|m| m.path == "foo")
            .expect("foo module discovered");
        let child = modules
            .iter()
            .find(|m| m.path == "foo::child")
            .expect("foo::child module discovered");

        assert_eq!(foo.source_path, PathBuf::from("foo.rs"));
        assert_eq!(child.source_path, PathBuf::from("foo/child.rs"));
    }

    // ========================================================================
    // CI Verification Tests
    // ========================================================================

    /// CI Test: Verify modules with @cli-status wired don't have TODO in CLI code
    ///
    /// If a module declares @cli-status wired, the corresponding CLI command
    /// implementation must not contain TODO comments.
    #[test]
    fn test_cli_status_accuracy() {
        let dashflow_src = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .join("dashflow/src");
        let cli_src = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .join("dashflow-cli/src");

        if !dashflow_src.exists() || !cli_src.exists() {
            return; // Skip if not in workspace context
        }

        let modules = discover_modules(&dashflow_src);

        // Find modules that claim @cli-status wired
        let wired_modules: Vec<_> = modules
            .iter()
            .filter(|m| m.cli_status == Some(CliStatus::Wired) && m.cli_command.is_some())
            .collect();

        let mut errors = Vec::new();

        for module in wired_modules {
            let cli_cmd = module.cli_command.as_ref().unwrap();

            // Parse CLI command to find the subcommand
            // Format: "dashflow <command> <subcommand>" or "dashflow <command>"
            let parts: Vec<&str> = cli_cmd.split_whitespace().collect();
            if parts.len() < 2 {
                continue;
            }

            // Map CLI command to source file
            let cmd_file = match parts.get(1) {
                Some(&"train") => cli_src.join("commands/train.rs"),
                Some(&"optimize") => cli_src.join("commands/optimize.rs"),
                Some(&"eval") => cli_src.join("commands/eval.rs"),
                Some(&"analyze") => cli_src.join("commands/analyze.rs"),
                Some(&"profile") => cli_src.join("commands/profile.rs"),
                Some(&"export") => cli_src.join("commands/export.rs"),
                Some(&"watch") => cli_src.join("commands/watch.rs"),
                Some(&"costs") => cli_src.join("commands/costs.rs"),
                Some(cmd) => cli_src.join(format!("commands/{}.rs", cmd)),
                None => continue,
            };

            if !cmd_file.exists() {
                errors.push(format!(
                    "Module '{}' claims @cli-status wired but CLI file not found: {}",
                    module.name,
                    cmd_file.display()
                ));
                continue;
            }

            // Check for TODO in the specific CLI function
            // For multi-subcommand files (like train.rs), we only check the specific
            // function that implements the wired command (e.g., run_distill for distill)
            if let Ok(content) = fs::read_to_string(&cmd_file) {
                // Determine the function name to check
                // "dashflow train distill" -> check run_distill function
                let subcommand = parts.get(2).copied();
                let fn_name = subcommand.map(|s| format!("fn run_{}", s));

                // Find the function region and check for TODOs only there
                let has_impl_todo = if let Some(ref fn_name) = fn_name {
                    // Find the function and check only within it
                    check_function_for_todo(&content, fn_name)
                } else {
                    // No subcommand, check the whole file
                    content.lines().any(|line| {
                        let trimmed = line.trim();
                        trimmed.starts_with("// TODO")
                            && !trimmed.starts_with("/// TODO")
                            && !trimmed.contains("(future)")
                    })
                };

                if has_impl_todo {
                    errors.push(format!(
                        "Module '{}' marked @cli-status wired but {} contains TODO in {}",
                        module.name,
                        cmd_file.display(),
                        fn_name.unwrap_or_else(|| "file".to_string())
                    ));
                }
            }
        }

        if !errors.is_empty() {
            panic!("CLI status accuracy check failed:\n{}", errors.join("\n"));
        }
    }

    /// CI Test: Verify all module directories are discovered
    ///
    /// This ensures the auto-discovery finds all `pub mod` directories.
    #[test]
    fn test_all_modules_discovered() {
        let dashflow_src = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .join("dashflow/src");

        if !dashflow_src.exists() {
            return; // Skip if not in workspace context
        }

        let modules = discover_modules(&dashflow_src);

        // Minimum expected modules from major categories
        // Note: "streaming" is in a separate crate (dashflow-streaming), not in dashflow/src
        let expected_modules = [
            // Core modules
            "core",
            "prompts",
            "tracers",
            "schema",
            // Optimize modules
            "optimize",
            "distillation",
            "ab_testing",
            // Other top-level
            "checkpoint",
            "quality",
            "func",
            "colony",
            "parallel",
            "scheduler",
        ];

        let module_names: std::collections::HashSet<_> =
            modules.iter().map(|m| m.name.as_str()).collect();

        let mut missing = Vec::new();
        for expected in &expected_modules {
            if !module_names.contains(expected) {
                missing.push(*expected);
            }
        }

        assert!(
            missing.is_empty(),
            "Missing expected modules: {:?}\nFound modules: {:?}",
            missing,
            module_names
        );
    }

    /// CI Test: Verify discovered modules have reasonable count
    ///
    /// This test ensures that module discovery is working correctly
    /// by verifying we find a minimum number of modules.
    #[test]
    fn test_module_count_reasonable() {
        let dashflow_src = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .join("dashflow/src");

        if !dashflow_src.exists() {
            return; // Skip if not in workspace context
        }

        let modules = discover_modules(&dashflow_src);

        // We expect at least 100 modules to be discovered
        assert!(
            modules.len() >= 100,
            "Expected at least 100 modules, found {}",
            modules.len()
        );

        // Verify all modules have non-empty names
        for module in &modules {
            assert!(!module.name.is_empty(), "Found module with empty name");
        }
    }

    /// CI Test: Verify CLI status markers are accurate
    ///
    /// Tracks the expected count of wired CLI commands. When you wire a new
    /// CLI command, update this test.
    #[test]
    fn test_cli_status_tracking() {
        let dashflow_src = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .join("dashflow/src");

        if !dashflow_src.exists() {
            return; // Skip if not in workspace context
        }

        let modules = discover_modules(&dashflow_src);

        // Count modules by CLI status
        let wired_count = modules
            .iter()
            .filter(|m| m.cli_status == Some(CliStatus::Wired))
            .count();
        let stub_count = modules
            .iter()
            .filter(|m| m.cli_status == Some(CliStatus::Stub))
            .count();

        // Currently wired CLI commands:
        // - distillation (dashflow train distill) - wired
        // - synthetic (dashflow train synthetic) - wired
        // - grpo (dashflow train rl) - wired (uses GRPOConfig from library)
        // Note: openai_finetune is in a nested directory (distillation/student/) and may not be
        // discovered depending on discovery depth settings.
        assert!(
            wired_count >= 3,
            "Expected at least 3 wired CLI commands (distill, synthetic, rl), found {}. Update this test if you wired more.",
            wired_count
        );

        // Currently stub CLI commands (library code works, CLI is placeholder):
        // (All CLI commands are now wired)
        assert_eq!(
            stub_count, 0,
            "Expected 0 stub CLI commands (all wired), found {}. Update this test when adding new CLI markers.",
            stub_count
        );
    }

    #[test]
    fn test_type_discovery() {
        // Test type discovery from the OpenSearch crate
        let workspace_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap();

        let opensearch_src = workspace_root.join("crates/dashflow-opensearch/src");
        let types = discover_types_in_crate(&opensearch_src, "dashflow-opensearch");

        // We should find OpenSearchBM25Retriever
        let retriever_types: Vec<_> = types
            .iter()
            .filter(|t| t.name.contains("Retriever"))
            .collect();

        assert!(
            !retriever_types.is_empty(),
            "Should find at least one Retriever type in dashflow-opensearch"
        );

        // Verify we found the BM25 retriever
        let bm25 = types.iter().find(|t| t.name == "OpenSearchBM25Retriever");
        assert!(bm25.is_some(), "Should find OpenSearchBM25Retriever type");

        if let Some(bm25) = bm25 {
            assert_eq!(bm25.kind, TypeKind::Struct);
            assert!(bm25.capability_tags.contains(&"retriever".to_string()));
            assert!(bm25.capability_tags.contains(&"bm25".to_string()));
        }
    }

    #[test]
    fn test_discover_all_types() {
        let workspace_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap();

        let types = discover_all_types(workspace_root);

        // With 108 crates, we should find many types
        assert!(
            types.len() > 100,
            "Should find at least 100 types across workspace, found {}",
            types.len()
        );

        // Verify capability tags are being inferred
        let types_with_tags: Vec<_> = types
            .iter()
            .filter(|t| !t.capability_tags.is_empty())
            .collect();
        assert!(
            !types_with_tags.is_empty(),
            "Should find types with capability tags"
        );
    }

    // ========================================================================
    // Enhanced capability discovery tests
    // ========================================================================

    #[test]
    fn test_split_type_name_pascal_case() {
        let components = split_type_name("OpenSearchBM25Retriever");
        // Should handle consecutive uppercase (BM25) as single component
        assert!(components.contains(&"open".to_string()));
        assert!(components.contains(&"search".to_string()));
        assert!(components.contains(&"bm25".to_string()));
        assert!(components.contains(&"retriever".to_string()));
    }

    #[test]
    fn test_split_type_name_snake_case() {
        let components = split_type_name("vector_store");
        assert_eq!(components, vec!["vector", "store"]);
    }

    #[test]
    fn test_split_type_name_mixed() {
        let components = split_type_name("ChatOpenAI_client");
        assert!(components.contains(&"chat".to_string()));
        assert!(components.contains(&"client".to_string()));
    }

    #[test]
    fn test_infer_capability_from_doc_phrases() {
        // Test phrase-based detection
        let tags = infer_capability_tags_with_methods(
            "MySearcher",
            "This provides keyword search using BM25 algorithm for full-text retrieval.",
            &[],
        );
        assert!(
            tags.contains(&"bm25".to_string()),
            "Should detect 'bm25' from doc comment"
        );
        assert!(
            tags.contains(&"search".to_string()),
            "Should detect 'search'"
        );
        assert!(
            tags.contains(&"retriever".to_string()),
            "Should detect 'retriever' from 'retrieval'"
        );
    }

    #[test]
    fn test_infer_capability_from_type_name_components() {
        // Type name with meaningful components
        let tags = infer_capability_tags_with_methods("VectorStoreRetriever", "", &[]);
        assert!(
            tags.contains(&"vector_store".to_string()),
            "Should detect 'vector_store' from name"
        );
        assert!(
            tags.contains(&"retriever".to_string()),
            "Should detect 'retriever' from name"
        );
    }

    #[test]
    fn test_infer_capability_semantic_phrase() {
        // Test semantic phrase detection in docs
        let tags = infer_capability_tags_with_methods(
            "AnswerGenerator",
            "This module provides answer synthesis from retrieved documents.",
            &[],
        );
        assert!(
            tags.contains(&"synthesis".to_string()),
            "Should detect synthesis capability"
        );
        assert!(
            tags.contains(&"answer_generation".to_string()),
            "Should detect answer_generation"
        );
    }

    // ========================================================================
    // Additional Unit Tests for Comprehensive Coverage
    // ========================================================================

    // -------------------- infer_crate_category tests --------------------

    #[test]
    fn test_infer_crate_category_core() {
        assert_eq!(infer_crate_category("dashflow"), "core");
    }

    #[test]
    fn test_infer_crate_category_llm_providers() {
        assert_eq!(infer_crate_category("dashflow-anthropic"), "llm");
        assert_eq!(infer_crate_category("dashflow-openai"), "llm");
        assert_eq!(infer_crate_category("dashflow-bedrock"), "llm");
        assert_eq!(infer_crate_category("dashflow-cohere"), "llm");
        assert_eq!(infer_crate_category("dashflow-groq"), "llm");
        assert_eq!(infer_crate_category("dashflow-mistral"), "llm");
        assert_eq!(infer_crate_category("dashflow-ollama"), "llm");
    }

    #[test]
    fn test_infer_crate_category_vector_stores() {
        assert_eq!(infer_crate_category("dashflow-pinecone"), "vector_store");
        assert_eq!(infer_crate_category("dashflow-chroma"), "vector_store");
        assert_eq!(infer_crate_category("dashflow-qdrant"), "vector_store");
        assert_eq!(infer_crate_category("dashflow-weaviate"), "vector_store");
        assert_eq!(infer_crate_category("dashflow-milvus"), "vector_store");
    }

    #[test]
    fn test_infer_crate_category_tools() {
        assert_eq!(infer_crate_category("dashflow-arxiv"), "tool");
        assert_eq!(infer_crate_category("dashflow-wikipedia"), "tool");
        assert_eq!(infer_crate_category("dashflow-duckduckgo"), "tool");
        assert_eq!(infer_crate_category("dashflow-tavily"), "tool");
    }

    #[test]
    fn test_infer_crate_category_infrastructure() {
        assert_eq!(infer_crate_category("dashflow-streaming"), "infrastructure");
        assert_eq!(infer_crate_category("dashflow-observability"), "infrastructure");
        assert_eq!(infer_crate_category("dashflow-telemetry"), "infrastructure");
    }

    #[test]
    fn test_infer_crate_category_processing() {
        assert_eq!(infer_crate_category("dashflow-chains"), "processing");
        assert_eq!(infer_crate_category("dashflow-compression"), "processing");
    }

    #[test]
    fn test_infer_crate_category_fallback() {
        // Unknown crate uses first part of name
        assert_eq!(infer_crate_category("dashflow-custom-thing"), "custom");
        assert_eq!(infer_crate_category("some-other-crate"), "some");
    }

    // -------------------- WorkspaceCrate tests --------------------

    #[test]
    fn test_workspace_crate_new() {
        let wc = WorkspaceCrate::new("dashflow-test", "crates/dashflow-test/src", "test");
        assert_eq!(wc.name, "dashflow-test");
        assert_eq!(wc.src_path, PathBuf::from("crates/dashflow-test/src"));
        assert_eq!(wc.category_prefix, "test");
    }

    #[test]
    fn test_workspace_crate_debug() {
        let wc = WorkspaceCrate::new("test-crate", "src/", "category");
        let debug_str = format!("{:?}", wc);
        assert!(debug_str.contains("test-crate"));
        assert!(debug_str.contains("category"));
    }

    #[test]
    fn test_workspace_crate_clone() {
        let wc = WorkspaceCrate::new("my-crate", "path/to/src", "cat");
        let cloned = wc.clone();
        assert_eq!(cloned.name, wc.name);
        assert_eq!(cloned.src_path, wc.src_path);
        assert_eq!(cloned.category_prefix, wc.category_prefix);
    }

    // -------------------- default_workspace_crates tests --------------------

    #[test]
    fn test_default_workspace_crates_not_empty() {
        let crates = default_workspace_crates();
        assert!(!crates.is_empty(), "Should have default crates");
    }

    #[test]
    fn test_default_workspace_crates_contains_dashflow() {
        let crates = default_workspace_crates();
        assert!(
            crates.iter().any(|c| c.name == "dashflow"),
            "Should contain core dashflow crate"
        );
    }

    // -------------------- parse_pub_mod_declarations edge cases --------------------

    #[test]
    fn test_parse_pub_mod_declarations_empty() {
        let mods = parse_pub_mod_declarations("");
        assert!(mods.is_empty());
    }

    #[test]
    fn test_parse_pub_mod_declarations_only_private() {
        let content = "mod private;\nmod another_private;";
        let mods = parse_pub_mod_declarations(content);
        assert!(mods.is_empty());
    }

    #[test]
    fn test_parse_pub_mod_declarations_mixed_visibility() {
        let content = "pub mod public;\nmod private;\npub mod also_public;";
        let mods = parse_pub_mod_declarations(content);
        assert_eq!(mods, vec!["public", "also_public"]);
    }

    #[test]
    fn test_parse_pub_mod_declarations_inline_block() {
        let content = "pub mod inline { pub fn foo() {} }";
        let mods = parse_pub_mod_declarations(content);
        assert_eq!(mods, vec!["inline"]);
    }

    #[test]
    fn test_parse_pub_mod_declarations_with_attributes() {
        let content = "#[cfg(test)]\npub mod tests;\npub mod normal;";
        let mods = parse_pub_mod_declarations(content);
        assert!(mods.contains(&"tests".to_string()));
        assert!(mods.contains(&"normal".to_string()));
    }

    // -------------------- parse_module_metadata edge cases --------------------

    #[test]
    fn test_parse_module_metadata_empty_content() {
        let metadata = parse_module_metadata("");
        assert!(metadata.description.is_empty());
        assert!(metadata.cli_command.is_none());
        assert_eq!(metadata.status, ModuleStatus::Stable);
    }

    #[test]
    fn test_parse_module_metadata_experimental_status() {
        let content = "//! @status experimental\n//! Experimental feature.";
        let metadata = parse_module_metadata(content);
        assert_eq!(metadata.status, ModuleStatus::Experimental);
    }

    #[test]
    fn test_parse_module_metadata_cli_wired() {
        let content = "//! @cli my-command\n//! @cli-status wired\n//! Command description.";
        let metadata = parse_module_metadata(content);
        assert_eq!(metadata.cli_command, Some("my-command".to_string()));
        assert_eq!(metadata.cli_status, Some(CliStatus::Wired));
    }

    #[test]
    fn test_parse_module_metadata_cli_none_status() {
        let content = "//! @cli-status none\n//! No CLI.";
        let metadata = parse_module_metadata(content);
        assert_eq!(metadata.cli_status, Some(CliStatus::None));
    }

    #[test]
    fn test_parse_module_metadata_skips_inner_attributes() {
        let content = "#![allow(unused)]\n//! Module docs.\n//! More docs.";
        let metadata = parse_module_metadata(content);
        assert!(metadata.description.contains("Module docs."));
    }

    #[test]
    fn test_parse_module_metadata_multiple_paragraphs_first_only() {
        let content = "//! First paragraph line 1.\n//! First paragraph line 2.\n//!\n//! Second paragraph.";
        let metadata = parse_module_metadata(content);
        assert!(metadata.description.contains("First paragraph line 1."));
        assert!(metadata.description.contains("First paragraph line 2."));
        assert!(!metadata.description.contains("Second paragraph"));
    }

    // -------------------- normalize_doc_text tests --------------------

    #[test]
    fn test_normalize_doc_text_empty() {
        assert_eq!(normalize_doc_text(""), "");
    }

    #[test]
    fn test_normalize_doc_text_strips_headings() {
        let text = "# Heading\n\nParagraph text.";
        let normalized = normalize_doc_text(text);
        assert_eq!(normalized, "Heading Paragraph text.");
    }

    #[test]
    fn test_normalize_doc_text_joins_lines() {
        let text = "Line one.\nLine two.\nLine three.";
        let normalized = normalize_doc_text(text);
        assert_eq!(normalized, "Line one. Line two. Line three.");
    }

    #[test]
    fn test_normalize_doc_text_skips_empty_lines() {
        let text = "First.\n\n\nSecond.";
        let normalized = normalize_doc_text(text);
        assert_eq!(normalized, "First. Second.");
    }

    // -------------------- truncate_text tests --------------------

    #[test]
    fn test_truncate_text_short_input() {
        let short = "Hello";
        assert_eq!(truncate_text(short, 100), "Hello");
    }

    #[test]
    fn test_truncate_text_exact_length() {
        let exact = "12345";
        assert_eq!(truncate_text(exact, 5), "12345");
    }

    #[test]
    fn test_truncate_text_adds_ellipsis() {
        let long = "Hello World!";
        let truncated = truncate_text(long, 8);
        assert!(truncated.ends_with("..."));
        assert!(truncated.len() <= 8);
    }

    #[test]
    fn test_truncate_text_unicode() {
        let unicode = "こんにちは世界";
        let truncated = truncate_text(unicode, 15);
        // Should handle multi-byte chars without panicking
        assert!(truncated.len() <= 18); // 15 chars + "..."
    }

    #[test]
    fn test_truncate_text_empty() {
        assert_eq!(truncate_text("", 10), "");
    }

    // -------------------- summarize_readme tests --------------------

    #[test]
    fn test_summarize_readme_empty() {
        assert_eq!(summarize_readme(""), "");
    }

    #[test]
    fn test_summarize_readme_heading_only() {
        let readme = "# My Project\n\n[![Build](badge.svg)]";
        let summary = summarize_readme(readme);
        assert!(summary.is_empty() || !summary.contains('#'));
    }

    #[test]
    fn test_summarize_readme_skips_code_blocks() {
        let readme = "# Title\n\nDescription here.\n\n```rust\nfn main() {}\n```\n\nMore text.";
        let summary = summarize_readme(readme);
        assert!(summary.contains("Description here."));
        assert!(!summary.contains("fn main"));
    }

    #[test]
    fn test_summarize_readme_extracts_first_paragraphs() {
        let readme = "# Title\n\nFirst paragraph.\n\nSecond paragraph.\n\nThird paragraph.";
        let summary = summarize_readme(readme);
        assert!(summary.contains("First paragraph"));
        assert!(summary.contains("Second paragraph"));
    }

    #[test]
    fn test_summarize_readme_skips_badges() {
        let readme = "[![CI](url)](link)\n![Logo](img.png)\n\nActual description.";
        let summary = summarize_readme(readme);
        assert!(summary.contains("Actual description"));
        assert!(!summary.contains("[!["));
    }

    // -------------------- find_crate_root_from_source tests --------------------

    #[test]
    fn test_find_crate_root_from_source_with_src_dir() {
        let temp_dir = TempDir::new().expect("create temp dir");
        let src_dir = temp_dir.path().join("src");
        std::fs::create_dir_all(&src_dir).expect("create src dir");
        let file_path = src_dir.join("lib.rs");

        let root = find_crate_root_from_source(&file_path);
        assert!(root.is_some());
        assert_eq!(root.unwrap(), temp_dir.path());
    }

    #[test]
    fn test_find_crate_root_from_source_with_cargo_toml() {
        let temp_dir = TempDir::new().expect("create temp dir");
        std::fs::write(temp_dir.path().join("Cargo.toml"), "[package]\nname = \"test\"").expect("write Cargo.toml");
        let file_path = temp_dir.path().join("lib.rs");

        let root = find_crate_root_from_source(&file_path);
        assert!(root.is_some());
    }

    // -------------------- generate_registry_code tests --------------------

    #[test]
    fn test_generate_registry_code_empty() {
        let code = generate_registry_code(&[]);
        assert!(code.contains("AUTO-GENERATED"));
        assert!(code.contains("DISCOVERED_MODULES"));
        assert!(code.contains("&[\n];"));
    }

    #[test]
    fn test_generate_registry_code_with_modules() {
        let modules = vec![ModuleInfo {
            name: "test_module".to_string(),
            path: "test::path".to_string(),
            category: "test".to_string(),
            description: "Test description".to_string(),
            capability_tags: vec!["tag1".to_string()],
            source_path: PathBuf::from("test.rs"),
            children: vec![],
            cli_command: Some("test-cmd".to_string()),
            cli_status: Some(CliStatus::Wired),
            status: ModuleStatus::Stable,
        }];
        let code = generate_registry_code(&modules);
        assert!(code.contains("test_module"));
        assert!(code.contains("test::path"));
        assert!(code.contains("test-cmd"));
    }

    // -------------------- to_json tests --------------------

    #[test]
    fn test_to_json_empty() {
        let json = to_json(&[]);
        assert_eq!(json, "[]");
    }

    #[test]
    fn test_to_json_with_module() {
        let modules = vec![ModuleInfo {
            name: "json_test".to_string(),
            path: "path::to::module".to_string(),
            category: "test".to_string(),
            description: "JSON test".to_string(),
            capability_tags: vec![],
            source_path: PathBuf::from("test.rs"),
            children: vec![],
            cli_command: None,
            cli_status: None,
            status: ModuleStatus::Stable,
        }];
        let json = to_json(&modules);
        assert!(json.contains("json_test"));
        assert!(json.contains("path::to::module"));
    }

    // -------------------- Enum serialization tests --------------------

    #[test]
    fn test_cli_status_serialize() {
        let wired = CliStatus::Wired;
        let json = serde_json::to_string(&wired).unwrap();
        assert_eq!(json, "\"wired\"");

        let stub = CliStatus::Stub;
        let json = serde_json::to_string(&stub).unwrap();
        assert_eq!(json, "\"stub\"");

        let none = CliStatus::None;
        let json = serde_json::to_string(&none).unwrap();
        assert_eq!(json, "\"none\"");
    }

    #[test]
    fn test_cli_status_deserialize() {
        let wired: CliStatus = serde_json::from_str("\"wired\"").unwrap();
        assert_eq!(wired, CliStatus::Wired);

        let stub: CliStatus = serde_json::from_str("\"stub\"").unwrap();
        assert_eq!(stub, CliStatus::Stub);
    }

    #[test]
    fn test_module_status_serialize() {
        let stable = ModuleStatus::Stable;
        let json = serde_json::to_string(&stable).unwrap();
        assert_eq!(json, "\"stable\"");

        let experimental = ModuleStatus::Experimental;
        let json = serde_json::to_string(&experimental).unwrap();
        assert_eq!(json, "\"experimental\"");

        let deprecated = ModuleStatus::Deprecated;
        let json = serde_json::to_string(&deprecated).unwrap();
        assert_eq!(json, "\"deprecated\"");
    }

    #[test]
    fn test_module_status_deserialize() {
        let stable: ModuleStatus = serde_json::from_str("\"stable\"").unwrap();
        assert_eq!(stable, ModuleStatus::Stable);

        let experimental: ModuleStatus = serde_json::from_str("\"experimental\"").unwrap();
        assert_eq!(experimental, ModuleStatus::Experimental);
    }

    #[test]
    fn test_module_status_default() {
        let default_status = ModuleStatus::default();
        assert_eq!(default_status, ModuleStatus::Stable);
    }

    #[test]
    fn test_type_kind_serialize() {
        let struct_kind = TypeKind::Struct;
        let json = serde_json::to_string(&struct_kind).unwrap();
        assert_eq!(json, "\"struct\"");

        let enum_kind = TypeKind::Enum;
        let json = serde_json::to_string(&enum_kind).unwrap();
        assert_eq!(json, "\"enum\"");

        let trait_kind = TypeKind::Trait;
        let json = serde_json::to_string(&trait_kind).unwrap();
        assert_eq!(json, "\"trait\"");
    }

    // -------------------- ModuleInfo tests --------------------

    #[test]
    fn test_module_info_debug() {
        let info = ModuleInfo {
            name: "debug_test".to_string(),
            path: "test::debug".to_string(),
            category: "test".to_string(),
            description: "Debug test".to_string(),
            capability_tags: vec![],
            source_path: PathBuf::from("test.rs"),
            children: vec![],
            cli_command: None,
            cli_status: None,
            status: ModuleStatus::Stable,
        };
        let debug_str = format!("{:?}", info);
        assert!(debug_str.contains("debug_test"));
        assert!(debug_str.contains("test::debug"));
    }

    #[test]
    fn test_module_info_clone() {
        let info = ModuleInfo {
            name: "clone_test".to_string(),
            path: "test::clone".to_string(),
            category: "test".to_string(),
            description: "Clone test".to_string(),
            capability_tags: vec!["tag".to_string()],
            source_path: PathBuf::from("test.rs"),
            children: vec!["child".to_string()],
            cli_command: Some("cmd".to_string()),
            cli_status: Some(CliStatus::Stub),
            status: ModuleStatus::Experimental,
        };
        let cloned = info.clone();
        assert_eq!(cloned.name, info.name);
        assert_eq!(cloned.path, info.path);
        assert_eq!(cloned.capability_tags, info.capability_tags);
        assert_eq!(cloned.cli_command, info.cli_command);
    }

    #[test]
    fn test_module_info_serialize_roundtrip() {
        let info = ModuleInfo {
            name: "serialize_test".to_string(),
            path: "test::serialize".to_string(),
            category: "test".to_string(),
            description: "Serialize test".to_string(),
            capability_tags: vec!["a".to_string(), "b".to_string()],
            source_path: PathBuf::from("path/to/test.rs"),
            children: vec!["child1".to_string()],
            cli_command: Some("my-cmd".to_string()),
            cli_status: Some(CliStatus::Wired),
            status: ModuleStatus::Stable,
        };
        let json = serde_json::to_string(&info).unwrap();
        let deserialized: ModuleInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.name, info.name);
        assert_eq!(deserialized.path, info.path);
        assert_eq!(deserialized.cli_command, info.cli_command);
    }

    // -------------------- TypeInfo tests --------------------

    #[test]
    fn test_type_info_debug() {
        let info = TypeInfo {
            name: "MyType".to_string(),
            path: "crate::MyType".to_string(),
            crate_name: "my-crate".to_string(),
            kind: TypeKind::Struct,
            description: "A type".to_string(),
            documentation: "Full docs".to_string(),
            source_path: PathBuf::from("lib.rs"),
            line_number: 42,
            is_public: true,
            capability_tags: vec![],
        };
        let debug_str = format!("{:?}", info);
        assert!(debug_str.contains("MyType"));
        assert!(debug_str.contains("Struct"));
    }

    #[test]
    fn test_type_info_serialize_roundtrip() {
        let info = TypeInfo {
            name: "SerializeType".to_string(),
            path: "crate::SerializeType".to_string(),
            crate_name: "test-crate".to_string(),
            kind: TypeKind::Trait,
            description: "A trait".to_string(),
            documentation: "Docs here".to_string(),
            source_path: PathBuf::from("trait.rs"),
            line_number: 10,
            is_public: true,
            capability_tags: vec!["tag".to_string()],
        };
        let json = serde_json::to_string(&info).unwrap();
        let deserialized: TypeInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.name, info.name);
        assert_eq!(deserialized.kind, info.kind);
    }

    // -------------------- split_type_name edge cases --------------------

    #[test]
    fn test_split_type_name_empty() {
        let components = split_type_name("");
        assert!(components.is_empty());
    }

    #[test]
    fn test_split_type_name_single_word() {
        let components = split_type_name("Retriever");
        assert_eq!(components, vec!["retriever"]);
    }

    #[test]
    fn test_split_type_name_all_uppercase() {
        let components = split_type_name("BM25");
        assert_eq!(components, vec!["bm25"]);
    }

    #[test]
    fn test_split_type_name_with_numbers() {
        let components = split_type_name("GPT4Model");
        assert!(components.contains(&"gpt4".to_string()) || components.contains(&"gpt".to_string()));
        assert!(components.contains(&"model".to_string()));
    }

    // -------------------- infer_capability_tags tests --------------------

    #[test]
    fn test_infer_capability_tags_embeddings() {
        let tags = infer_capability_tags_with_methods("EmbeddingsModel", "", &[]);
        assert!(tags.contains(&"embeddings".to_string()));
    }

    #[test]
    fn test_infer_capability_tags_llm() {
        let tags = infer_capability_tags_with_methods("ChatLLM", "", &[]);
        assert!(tags.contains(&"llm".to_string()));
        assert!(tags.contains(&"chat".to_string()));
    }

    #[test]
    fn test_infer_capability_tags_vector_store() {
        let tags = infer_capability_tags_with_methods("VectorStore", "", &[]);
        assert!(tags.contains(&"vector_store".to_string()));
    }

    #[test]
    fn test_infer_capability_tags_from_docs() {
        let tags = infer_capability_tags_with_methods(
            "MyClass",
            "This provides chunking and text splitting functionality.",
            &[],
        );
        assert!(tags.contains(&"chunking".to_string()) || tags.contains(&"splitting".to_string()));
    }

    #[test]
    fn test_infer_capability_tags_from_methods() {
        let methods = vec!["fn embed_documents".to_string(), "fn similarity_search".to_string()];
        let tags = infer_capability_tags_with_methods("GenericStore", "", &methods);
        assert!(tags.contains(&"embeddings".to_string()) || tags.contains(&"similarity_search".to_string()));
    }

    // -------------------- discover_modules filesystem tests --------------------

    #[test]
    fn test_discover_modules_empty_dir() {
        let temp_dir = TempDir::new().expect("create temp dir");
        let modules = discover_modules(temp_dir.path());
        assert!(modules.is_empty());
    }

    #[test]
    fn test_discover_modules_lib_rs_only() {
        let temp_dir = TempDir::new().expect("create temp dir");
        std::fs::write(temp_dir.path().join("lib.rs"), "//! Root module.\n").expect("write lib.rs");

        let modules = discover_modules(temp_dir.path());
        assert!(modules.is_empty()); // No pub mods declared
    }

    #[test]
    fn test_discover_modules_single_file_module() {
        let temp_dir = TempDir::new().expect("create temp dir");
        std::fs::write(temp_dir.path().join("lib.rs"), "pub mod single;\n").expect("write lib.rs");
        std::fs::write(
            temp_dir.path().join("single.rs"),
            "//! Single file module.\n\npub fn foo() {}\n",
        ).expect("write single.rs");

        let modules = discover_modules(temp_dir.path());
        assert_eq!(modules.len(), 1);
        assert_eq!(modules[0].name, "single");
        assert_eq!(modules[0].source_path, PathBuf::from("single.rs"));
    }

    #[test]
    fn test_discover_modules_directory_module() {
        let temp_dir = TempDir::new().expect("create temp dir");
        std::fs::create_dir_all(temp_dir.path().join("mymod")).expect("create dir");
        std::fs::write(temp_dir.path().join("lib.rs"), "pub mod mymod;\n").expect("write lib.rs");
        std::fs::write(
            temp_dir.path().join("mymod/mod.rs"),
            "//! Directory module.\n",
        ).expect("write mod.rs");

        let modules = discover_modules(temp_dir.path());
        assert_eq!(modules.len(), 1);
        assert_eq!(modules[0].name, "mymod");
        assert_eq!(modules[0].source_path, PathBuf::from("mymod/mod.rs"));
    }

    // -------------------- discover_workspace_binaries tests --------------------

    #[test]
    fn test_discover_workspace_binaries_no_bin_dir() {
        let temp_dir = TempDir::new().expect("create temp dir");
        let crates = vec![WorkspaceCrate::new("test", temp_dir.path().to_str().unwrap(), "test")];
        let binaries = discover_workspace_binaries(temp_dir.path(), &crates);
        assert!(binaries.is_empty());
    }

    #[test]
    fn test_discover_workspace_binaries_empty_bin_dir() {
        let temp_dir = TempDir::new().expect("create temp dir");
        let src_path = temp_dir.path().join("src");
        let bin_path = src_path.join("bin");
        std::fs::create_dir_all(&bin_path).expect("create bin dir");

        let crates = vec![WorkspaceCrate::new("test", "src", "test")];
        let binaries = discover_workspace_binaries(temp_dir.path(), &crates);
        assert!(binaries.is_empty());
    }

    #[test]
    fn test_discover_workspace_binaries_with_binary() {
        let temp_dir = TempDir::new().expect("create temp dir");
        let bin_path = temp_dir.path().join("src").join("bin");
        std::fs::create_dir_all(&bin_path).expect("create bin dir");
        std::fs::write(
            bin_path.join("my_tool.rs"),
            "//! My tool description.\n\nfn main() {}\n",
        ).expect("write binary");

        let crates = vec![WorkspaceCrate::new("test-crate", "src", "test")];
        let binaries = discover_workspace_binaries(temp_dir.path(), &crates);

        assert_eq!(binaries.len(), 1);
        assert_eq!(binaries[0].name, "my_tool");
        assert_eq!(binaries[0].category, "binary");
        assert!(binaries[0].description.contains("My tool description"));
    }

    // -------------------- extract_module_documentation tests --------------------

    #[test]
    fn test_extract_module_documentation_empty() {
        assert_eq!(extract_module_documentation(""), "");
    }

    #[test]
    fn test_extract_module_documentation_basic() {
        let content = "//! First line.\n//! Second line.\n\nuse something;";
        let docs = extract_module_documentation(content);
        assert!(docs.contains("First line."));
        assert!(docs.contains("Second line."));
    }

    #[test]
    fn test_extract_module_documentation_filters_markers() {
        let content = "//! @cli command\n//! @status stable\n//! Actual docs.";
        let docs = extract_module_documentation(content);
        assert!(!docs.contains("@cli"));
        assert!(!docs.contains("@status"));
        assert!(docs.contains("Actual docs."));
    }

    #[test]
    fn test_extract_module_documentation_skips_regular_comments() {
        let content = "// Regular comment\n//! Module doc.\n// Another comment\n//! More doc.";
        let docs = extract_module_documentation(content);
        assert!(docs.contains("Module doc."));
        assert!(docs.contains("More doc."));
        assert!(!docs.contains("Regular comment"));
    }

    // -------------------- propagate_module_capability_tags tests --------------------

    #[test]
    fn test_propagate_capability_tags_single_level() {
        let mut modules = vec![
            ModuleInfo {
                name: "parent".to_string(),
                path: "parent".to_string(),
                category: "test".to_string(),
                description: "".to_string(),
                capability_tags: vec![],
                source_path: PathBuf::from("parent.rs"),
                children: vec!["child".to_string()],
                cli_command: None,
                cli_status: None,
                status: ModuleStatus::Stable,
            },
            ModuleInfo {
                name: "child".to_string(),
                path: "parent::child".to_string(),
                category: "test".to_string(),
                description: "".to_string(),
                capability_tags: vec!["child_tag".to_string()],
                source_path: PathBuf::from("child.rs"),
                children: vec![],
                cli_command: None,
                cli_status: None,
                status: ModuleStatus::Stable,
            },
        ];

        propagate_module_capability_tags(&mut modules);

        let parent = modules.iter().find(|m| m.name == "parent").unwrap();
        assert!(
            parent.capability_tags.contains(&"child_tag".to_string()),
            "Parent should inherit child's tags"
        );
    }

    #[test]
    fn test_propagate_capability_tags_no_children() {
        let mut modules = vec![ModuleInfo {
            name: "lonely".to_string(),
            path: "lonely".to_string(),
            category: "test".to_string(),
            description: "".to_string(),
            capability_tags: vec!["own_tag".to_string()],
            source_path: PathBuf::from("lonely.rs"),
            children: vec![],
            cli_command: None,
            cli_status: None,
            status: ModuleStatus::Stable,
        }];

        propagate_module_capability_tags(&mut modules);

        assert_eq!(modules[0].capability_tags, vec!["own_tag".to_string()]);
    }

    // -------------------- CliStatus equality tests --------------------

    #[test]
    fn test_cli_status_equality() {
        assert_eq!(CliStatus::Wired, CliStatus::Wired);
        assert_eq!(CliStatus::Stub, CliStatus::Stub);
        assert_eq!(CliStatus::None, CliStatus::None);
        assert_ne!(CliStatus::Wired, CliStatus::Stub);
        assert_ne!(CliStatus::Stub, CliStatus::None);
    }

    #[test]
    fn test_cli_status_copy() {
        let status = CliStatus::Wired;
        let copied = status;
        assert_eq!(status, copied);
    }

    // -------------------- ModuleStatus equality tests --------------------

    #[test]
    fn test_module_status_equality() {
        assert_eq!(ModuleStatus::Stable, ModuleStatus::Stable);
        assert_eq!(ModuleStatus::Experimental, ModuleStatus::Experimental);
        assert_eq!(ModuleStatus::Deprecated, ModuleStatus::Deprecated);
        assert_ne!(ModuleStatus::Stable, ModuleStatus::Deprecated);
    }

    #[test]
    fn test_module_status_copy() {
        let status = ModuleStatus::Experimental;
        let copied = status;
        assert_eq!(status, copied);
    }

    // -------------------- TypeKind equality tests --------------------

    #[test]
    fn test_type_kind_equality() {
        assert_eq!(TypeKind::Struct, TypeKind::Struct);
        assert_eq!(TypeKind::Enum, TypeKind::Enum);
        assert_eq!(TypeKind::Trait, TypeKind::Trait);
        assert_eq!(TypeKind::Function, TypeKind::Function);
        assert_ne!(TypeKind::Struct, TypeKind::Enum);
    }

    #[test]
    fn test_type_kind_copy() {
        let kind = TypeKind::Trait;
        let copied = kind;
        assert_eq!(kind, copied);
    }

    // -------------------- discover_workspace_modules tests --------------------

    #[test]
    fn test_discover_workspace_modules_empty_crates() {
        let temp_dir = TempDir::new().expect("create temp dir");
        let modules = discover_workspace_modules(temp_dir.path(), &[]);
        assert!(modules.is_empty());
    }

    #[test]
    fn test_discover_workspace_modules_nonexistent_path() {
        let temp_dir = TempDir::new().expect("create temp dir");
        let crates = vec![WorkspaceCrate::new("ghost", "nonexistent/path", "test")];
        let modules = discover_workspace_modules(temp_dir.path(), &crates);
        assert!(modules.is_empty());
    }

    #[test]
    fn test_discover_workspace_modules_prefixes_non_core_crates() {
        let temp_dir = TempDir::new().expect("create temp dir");
        let src_path = temp_dir.path().join("crates").join("dashflow-test").join("src");
        std::fs::create_dir_all(&src_path).expect("create dirs");
        std::fs::write(src_path.join("lib.rs"), "pub mod mymod;\n").expect("write lib.rs");
        std::fs::write(src_path.join("mymod.rs"), "//! My module.\n").expect("write mymod.rs");

        let crates = vec![WorkspaceCrate::new(
            "dashflow-test",
            "crates/dashflow-test/src",
            "test",
        )];
        let modules = discover_workspace_modules(temp_dir.path(), &crates);

        assert_eq!(modules.len(), 1);
        assert!(
            modules[0].path.starts_with("dashflow_test::"),
            "Non-core crate modules should be prefixed"
        );
    }
}
