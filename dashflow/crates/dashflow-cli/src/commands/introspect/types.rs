// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

//! Type index and capability commands for DashFlow introspection.
//!
//! Provides commands for querying the type index, finding types by capability,
//! and finding platform alternatives for code snippets.

use anyhow::Result;
use crate::output::OutputFormat;
use colored::Colorize;
use dashflow::lint::{TypeIndex, TypeIndexCache};
use dashflow_module_discovery::{
    discover_all_types, find_types_by_capability, get_all_capability_tags, get_capability_stats,
    TypeKind,
};

use super::{
    create_table, get_workspace_root, print_error, print_info, truncate_str, AlternativesArgs,
    CapabilitiesArgs, FindCapabilityArgs, TypeIndexArgs, TypesArgs,
};

/// Format a TypeKind for display
pub(super) fn format_type_kind(kind: TypeKind) -> String {
    match kind {
        TypeKind::Struct => "struct".bright_blue().to_string(),
        TypeKind::Enum => "enum".bright_yellow().to_string(),
        TypeKind::Trait => "trait".bright_magenta().to_string(),
        TypeKind::Function => "fn".bright_green().to_string(),
        TypeKind::TypeAlias => "type".bright_cyan().to_string(),
        TypeKind::Const => "const".dimmed().to_string(),
    }
}

/// Parse type kind from string
fn parse_type_kind(s: &str) -> Option<TypeKind> {
    match s.to_lowercase().as_str() {
        "struct" => Some(TypeKind::Struct),
        "enum" => Some(TypeKind::Enum),
        "trait" => Some(TypeKind::Trait),
        "fn" | "function" => Some(TypeKind::Function),
        "type" | "alias" => Some(TypeKind::TypeAlias),
        "const" => Some(TypeKind::Const),
        _ => None,
    }
}

pub(super) async fn run_types(args: TypesArgs) -> Result<()> {
    let workspace_root = get_workspace_root();
    let mut all_types = discover_all_types(&workspace_root);

    // Filter by crate name if provided
    if let Some(ref crate_name) = args.crate_name {
        let crate_lower = crate_name.to_lowercase();
        all_types.retain(|t| t.crate_name.to_lowercase().contains(&crate_lower));
    }

    // Filter by kind if provided
    if let Some(ref kind_str) = args.kind {
        if let Some(kind) = parse_type_kind(kind_str) {
            all_types.retain(|t| t.kind == kind);
        } else {
            print_error(&format!(
                "Unknown type kind '{}'. Use: struct, enum, trait, fn, type, const",
                kind_str
            ));
            return Ok(());
        }
    }

    // Filter by search query if provided
    if let Some(ref filter) = args.filter {
        let filter_lower = filter.to_lowercase();
        all_types.retain(|t| {
            t.name.to_lowercase().contains(&filter_lower)
                || t.path.to_lowercase().contains(&filter_lower)
                || t.description.to_lowercase().contains(&filter_lower)
                || t.capability_tags
                    .iter()
                    .any(|tag| tag.contains(&filter_lower))
        });
    }

    // Filter by capability tag if provided
    if let Some(ref capability) = args.capability {
        let capability_lower = capability.to_lowercase();
        all_types.retain(|t| {
            t.capability_tags
                .iter()
                .any(|tag| tag.to_lowercase().contains(&capability_lower))
        });
    }

    if matches!(args.format, OutputFormat::Json) {
        println!("{}", serde_json::to_string_pretty(&all_types)?);
        return Ok(());
    }

    // Human-readable output
    if all_types.is_empty() {
        let mut msg = "No types found".to_string();
        if let Some(ref crate_name) = args.crate_name {
            msg.push_str(&format!(" in crate '{}'", crate_name));
        }
        if let Some(ref kind) = args.kind {
            msg.push_str(&format!(" of kind '{}'", kind));
        }
        if let Some(ref filter) = args.filter {
            msg.push_str(&format!(" matching '{}'", filter));
        }
        if let Some(ref capability) = args.capability {
            msg.push_str(&format!(" with capability '{}'", capability));
        }
        print_info(&msg);
        return Ok(());
    }

    println!();
    let title = if let Some(ref crate_name) = args.crate_name {
        format!("Types in '{}': {}", crate_name, all_types.len())
    } else {
        format!("All Types: {} discovered", all_types.len())
    };
    println!("{}", title.bright_cyan().bold());
    println!("{}", "═".repeat(80).bright_cyan());

    // Group by crate
    let mut by_crate: std::collections::BTreeMap<String, Vec<&dashflow_module_discovery::TypeInfo>> =
        std::collections::BTreeMap::new();
    for ty in &all_types {
        by_crate.entry(ty.crate_name.clone()).or_default().push(ty);
    }

    for (crate_name, types) in by_crate {
        println!("\n{} ({})", crate_name.bright_yellow().bold(), types.len());
        println!("{}", "─".repeat(60));

        let mut table = create_table();
        table.set_header(vec!["Name", "Kind", "Description", "Tags"]);

        for ty in types {
            let kind_str = format_type_kind(ty.kind);
            let desc = truncate_str(&ty.description, 35);
            let tags = if ty.capability_tags.is_empty() {
                "-".dimmed().to_string()
            } else {
                ty.capability_tags.join(", ")
            };

            table.add_row(vec![ty.name.clone(), kind_str, desc, tags]);
        }

        println!("{table}");
    }

    Ok(())
}

/// Manage the type index (status, rebuild)
pub(super) async fn run_type_index(args: TypeIndexArgs) -> Result<()> {
    let workspace_root = get_workspace_root();
    let cache_path = workspace_root.join(TypeIndexCache::CACHE_PATH);

    if args.rebuild {
        // Force rebuild the index
        if !matches!(args.format, OutputFormat::Json) {
            println!("{}", "Rebuilding type index...".bright_cyan());
        }

        let start = std::time::Instant::now();
        let index = TypeIndex::regenerate(workspace_root.clone());
        let type_count = index.type_count();
        let elapsed = start.elapsed();

        if matches!(args.format, OutputFormat::Json) {
            println!(
                r#"{{"status": "rebuilt", "types": {}, "cache_path": "{}", "elapsed_ms": {}}}"#,
                type_count,
                cache_path.display(),
                elapsed.as_millis()
            );
        } else {
            println!(
                "{} Rebuilt type index with {} types in {:.2}s",
                "✓".bright_green(),
                type_count.to_string().bright_yellow(),
                elapsed.as_secs_f64()
            );
            println!(
                "  Cache saved to: {}",
                cache_path.display().to_string().dimmed()
            );
        }
        return Ok(());
    }

    // Show status
    if let Some((index, cache)) = TypeIndex::load(&cache_path) {
        let is_stale = cache.is_stale(&workspace_root).unwrap_or(true);

        if matches!(args.format, OutputFormat::Json) {
            println!(
                r#"{{"status": "{}", "types": {}, "cache_path": "{}", "created_at": "{}"}}"#,
                if is_stale { "stale" } else { "fresh" },
                index.type_count(),
                cache_path.display(),
                cache.created_at
            );
        } else {
            println!();
            println!("{}", "Type Index Status".bright_cyan().bold());
            println!("{}", "═".repeat(50).bright_cyan());

            let status = if is_stale {
                "STALE".bright_red().bold()
            } else {
                "FRESH".bright_green().bold()
            };

            println!("  Status:     {}", status);
            println!(
                "  Types:      {}",
                index.type_count().to_string().bright_yellow()
            );
            println!(
                "  Cache path: {}",
                cache_path.display().to_string().dimmed()
            );
            println!("  Created:    {}", cache.created_at.dimmed());

            if is_stale {
                println!();
                println!(
                    "{}",
                    "Hint: Run `dashflow introspect index --rebuild` to update".dimmed()
                );
            }
        }
    } else {
        // No cache exists
        if matches!(args.format, OutputFormat::Json) {
            println!(
                r#"{{"status": "missing", "cache_path": "{}"}}"#,
                cache_path.display()
            );
        } else {
            println!();
            println!("{}", "Type Index Status".bright_cyan().bold());
            println!("{}", "═".repeat(50).bright_cyan());
            println!("  Status:     {}", "MISSING".bright_red().bold());
            println!(
                "  Cache path: {}",
                cache_path.display().to_string().dimmed()
            );
            println!();
            println!(
                "{}",
                "Hint: Run `dashflow introspect index --rebuild` to build".dimmed()
            );
        }
    }

    Ok(())
}

/// Find types by capability tag
pub(super) async fn run_find_capability(args: FindCapabilityArgs) -> Result<()> {
    let workspace_root = get_workspace_root();

    println!(
        "{} Searching for types with capability '{}'...",
        "Info:".bright_cyan().bold(),
        args.capability.bright_white()
    );

    let mut results = find_types_by_capability(&workspace_root, &args.capability);

    // Apply limit if specified
    if let Some(limit) = args.limit {
        results.truncate(limit);
    }

    if matches!(args.format, OutputFormat::Json) {
        let json_results: Vec<_> = results
            .iter()
            .map(|t| {
                serde_json::json!({
                    "name": t.name,
                    "path": t.path,
                    "crate": t.crate_name,
                    "kind": format!("{:?}", t.kind).to_lowercase(),
                    "description": t.description,
                    "capability_tags": t.capability_tags,
                })
            })
            .collect();
        println!("{}", serde_json::to_string_pretty(&json_results)?);
        return Ok(());
    }

    // Human-readable output
    if results.is_empty() {
        print_info(&format!(
            "No types found with capability '{}'",
            args.capability
        ));
        println!();
        println!(
            "{} Run `dashflow introspect capabilities` to see all available tags.",
            "Tip:".bright_cyan()
        );
        return Ok(());
    }

    println!();
    println!(
        "{} {} types with capability '{}'",
        "Found:".bright_green().bold(),
        results.len().to_string().bright_green(),
        args.capability.bright_white()
    );
    println!("{}", "═".repeat(80).bright_green());

    let mut table = create_table();
    if args.show_tags {
        table.set_header(vec!["Type", "Kind", "Crate", "Tags", "Description"]);
    } else {
        table.set_header(vec!["Type", "Kind", "Crate", "Description"]);
    }

    for ty in &results {
        let kind_str = format_type_kind(ty.kind);
        let desc = truncate_str(&ty.description, 35);

        if args.show_tags {
            let tags = ty.capability_tags.join(", ");
            table.add_row(vec![
                ty.name.clone(),
                kind_str,
                ty.crate_name.clone(),
                tags,
                desc,
            ]);
        } else {
            table.add_row(vec![ty.name.clone(), kind_str, ty.crate_name.clone(), desc]);
        }
    }

    println!("{table}");

    // Show usage hint
    println!();
    println!(
        "{} Use `dashflow introspect show <type_path>` to see type details.",
        "Tip:".dimmed()
    );

    Ok(())
}

/// List all available capability tags
pub(super) async fn run_capabilities(args: CapabilitiesArgs) -> Result<()> {
    let workspace_root = get_workspace_root();

    if args.with_counts {
        let stats = get_capability_stats(&workspace_root);

        if matches!(args.format, OutputFormat::Json) {
            println!("{}", serde_json::to_string_pretty(&stats)?);
            return Ok(());
        }

        // Human-readable output with counts
        println!();
        println!("{}", "DashFlow Capability Tags".bright_cyan().bold());
        println!("{}", "═".repeat(60).bright_cyan());

        let mut sorted: Vec<_> = stats.into_iter().collect();
        sorted.sort_by(|a, b| b.1.cmp(&a.1)); // Sort by count descending

        let mut table = create_table();
        table.set_header(vec!["Capability", "Type Count"]);

        for (tag, count) in sorted {
            table.add_row(vec![tag, count.to_string()]);
        }

        println!("{table}");
    } else {
        let tags = get_all_capability_tags(&workspace_root);

        if matches!(args.format, OutputFormat::Json) {
            println!("{}", serde_json::to_string_pretty(&tags)?);
            return Ok(());
        }

        // Human-readable output
        println!();
        println!(
            "{} {} capability tags available",
            "DashFlow:".bright_cyan().bold(),
            tags.len().to_string().bright_green()
        );
        println!("{}", "═".repeat(60).bright_cyan());

        // Display in columns
        let cols = 3;
        let rows = tags.len().div_ceil(cols);

        for row in 0..rows {
            let mut line = String::new();
            for col in 0..cols {
                let idx = row + col * rows;
                if idx < tags.len() {
                    line.push_str(&format!("{:25}", tags[idx]));
                }
            }
            println!("  {}", line);
        }
    }

    println!();
    println!(
        "{} Use `dashflow introspect find-capability <tag>` to find types with a capability.",
        "Tip:".dimmed()
    );

    Ok(())
}

/// Find platform alternatives for a code snippet (Gap 13)
pub(super) async fn run_alternatives(args: AlternativesArgs) -> Result<()> {
    let workspace_root = get_workspace_root();
    let type_index = TypeIndex::global(workspace_root.clone());

    // Use semantic search to find relevant types
    let results = type_index.search_semantic(&args.snippet, args.limit);

    // Filter by minimum score
    let filtered: Vec<_> = results
        .into_iter()
        .filter(|(_, score)| *score >= args.min_score)
        .collect();

    if matches!(args.format, OutputFormat::Json) {
        #[derive(serde::Serialize)]
        struct AlternativeResult {
            name: String,
            path: String,
            crate_name: String,
            kind: String,
            description: String,
            score: f32,
            capability_tags: Vec<String>,
        }

        let json_results: Vec<AlternativeResult> = filtered
            .iter()
            .map(|(ty, score)| AlternativeResult {
                name: ty.name.clone(),
                path: ty.path.clone(),
                crate_name: ty.crate_name.clone(),
                kind: format!("{:?}", ty.kind).to_lowercase(),
                description: ty.description.clone(),
                score: *score,
                capability_tags: ty.capability_tags.clone(),
            })
            .collect();

        println!("{}", serde_json::to_string_pretty(&json_results)?);
        return Ok(());
    }

    // Human-readable output
    println!();
    println!(
        "{} Platform Alternatives for: {}",
        "DashFlow".bright_cyan().bold(),
        args.snippet.bright_yellow()
    );
    println!("{}", "═".repeat(70).bright_cyan());

    if filtered.is_empty() {
        println!();
        println!(
            "{} No platform alternatives found matching '{}'.",
            "Info:".dimmed(),
            args.snippet
        );
        println!(
            "{} Try a different query or use `dashflow introspect search <query>` for broader results.",
            "Tip:".dimmed()
        );
    } else {
        println!();
        for (ty, score) in &filtered {
            println!(
                "  {} {} {}",
                format!("{:?}", ty.kind).to_lowercase().bright_blue(),
                ty.name.bright_green().bold(),
                format!("[{:.2} similarity]", score).dimmed()
            );
            println!("    {} {}", "Path:".dimmed(), ty.path);
            println!("    {} {}", "Crate:".dimmed(), ty.crate_name);
            if !ty.description.is_empty() {
                println!("    {}", ty.description.dimmed());
            }
            if !ty.capability_tags.is_empty() {
                println!("    {} {}", "Tags:".dimmed(), ty.capability_tags.join(", "));
            }
            println!();
        }

        println!(
            "{} To see full details, use: dashflow introspect show <path>",
            "Tip:".dimmed()
        );
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_type_kind_accepts_common_spellings_case_insensitively() {
        assert_eq!(parse_type_kind("struct"), Some(TypeKind::Struct));
        assert_eq!(parse_type_kind("ENUM"), Some(TypeKind::Enum));
        assert_eq!(parse_type_kind("Trait"), Some(TypeKind::Trait));
        assert_eq!(parse_type_kind("function"), Some(TypeKind::Function));
        assert_eq!(parse_type_kind("alias"), Some(TypeKind::TypeAlias));
        assert_eq!(parse_type_kind("const"), Some(TypeKind::Const));
        assert_eq!(parse_type_kind("unknown"), None);
    }

    #[test]
    fn format_type_kind_returns_plain_label_when_color_disabled() {
        colored::control::set_override(false);
        assert_eq!(format_type_kind(TypeKind::Struct), "struct");
        assert_eq!(format_type_kind(TypeKind::Function), "fn");
    }
}
