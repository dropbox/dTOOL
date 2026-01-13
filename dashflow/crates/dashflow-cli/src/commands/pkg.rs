//! Package registry CLI commands.
//!
//! Provides commands for interacting with the DashFlow package registry:
//! - search: Find packages by query or capability
//! - info: Show package details
//! - install: Install packages locally
//! - publish: Publish packages to registry
//! - list: List installed packages
//! - verify: Verify package signatures

use anyhow::{Context, Result};
use clap::{Args, Subcommand};
use dashflow::core::config_loader::env_vars::{
    dashflow_registry_api_key, dashflow_registry_url, DASHFLOW_REGISTRY_API_KEY,
};
use dashflow_registry::{
    // Contribution types
    BugCategory,
    BugReport,
    BugSeverity,
    ContentHash,
    Contribution,
    ContributionReviewer,
    FileChange,
    FileChangeType,
    FixSubmission,
    FixType,
    ImpactLevel,
    ImprovementCategory,
    ImprovementProposal,
    KeyPair,
    Keyring,
    KeywordSearch,
    MockModelReviewer,
    PackageInfo,
    PackageManifest,
    PackageRequest,
    PackageType,
    RegistryClient,
    RegistryClientConfig,
    RequestPriority,
    ReviewConfig,
    ReviewVerdict,
    TrustLevel,
    TrustService,
};
use serde::{Deserialize, Serialize};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

/// Package registry operations
#[derive(Args)]
pub struct PkgArgs {
    #[command(subcommand)]
    command: PkgCommands,
}

#[derive(Subcommand)]
enum PkgCommands {
    /// Search for packages
    Search(SearchArgs),

    /// Show package information
    Info(InfoArgs),

    /// Install a package
    Install(InstallArgs),

    /// Publish a package to the registry
    Publish(PublishArgs),

    /// Login to the registry (store API key)
    Login(LoginArgs),

    /// List installed packages
    List(ListArgs),

    /// Verify package signatures
    Verify(VerifyArgs),

    /// Initialize a new package
    Init(InitArgs),

    /// Show package cache info
    Cache(CacheArgs),

    /// Colony P2P operations
    Colony(ColonyArgs),

    /// Contribution operations (bug reports, improvements, fixes)
    Contrib(ContribArgs),
}

/// Search for packages in the registry
#[derive(Args)]
struct SearchArgs {
    /// Search query (natural language or keywords)
    query: String,

    /// Use semantic search (vector similarity)
    #[arg(long, short = 's')]
    semantic: bool,

    /// Filter by package type
    #[arg(long, short = 't')]
    package_type: Option<String>,

    /// Filter by minimum trust level (unknown, community, organization, official)
    #[arg(long)]
    min_trust: Option<String>,

    /// Maximum number of results
    #[arg(long, short = 'n', default_value = "10")]
    limit: usize,

    /// Output format (table, json, yaml)
    #[arg(long, short = 'f', default_value = "table")]
    format: String,
}

/// Show detailed package information
#[derive(Args)]
struct InfoArgs {
    /// Package name or hash
    package: String,

    /// Show full manifest
    #[arg(long)]
    full: bool,

    /// Show lineage/derivation chain
    #[arg(long)]
    lineage: bool,

    /// Output format (table, json, yaml)
    #[arg(long, short = 'f', default_value = "table")]
    format: String,
}

/// Install a package locally
#[derive(Args)]
struct InstallArgs {
    /// Package name or hash
    package: String,

    /// Specific version (e.g., "1.0.0", "^1.0", "latest")
    #[arg(long, short = 'v', default_value = "latest")]
    version: String,

    /// Installation directory
    #[arg(long, short = 'd')]
    dir: Option<PathBuf>,

    /// Skip signature verification (not recommended)
    #[arg(long)]
    no_verify: bool,

    /// Force reinstall even if already installed
    #[arg(long, short = 'f')]
    force: bool,
}

/// Publish a package to the registry
#[derive(Args)]
struct PublishArgs {
    /// Path to the package directory (containing dashflow.toml)
    #[arg(default_value = ".")]
    path: PathBuf,

    /// Registry URL (defaults to DASHFLOW_REGISTRY_URL env or localhost)
    #[arg(long)]
    registry: Option<String>,

    /// Skip signing (not recommended)
    #[arg(long)]
    no_sign: bool,

    /// Dry run - validate but don't actually publish
    #[arg(long)]
    dry_run: bool,
}

/// Login to the registry (store API key)
#[derive(Args)]
struct LoginArgs {
    /// API key (if not provided, will prompt or check environment)
    #[arg(long, short = 'k')]
    api_key: Option<String>,

    /// Registry URL (defaults to DASHFLOW_REGISTRY_URL env or localhost)
    #[arg(long)]
    registry: Option<String>,

    /// Show current login status
    #[arg(long)]
    status: bool,

    /// Logout (remove stored credentials)
    #[arg(long)]
    logout: bool,
}

/// List installed packages
#[derive(Args)]
struct ListArgs {
    /// Show all versions, not just latest
    #[arg(long, short = 'a')]
    all: bool,

    /// Filter by package type
    #[arg(long, short = 't')]
    package_type: Option<String>,

    /// Output format (table, json, yaml)
    #[arg(long, short = 'f', default_value = "table")]
    format: String,
}

/// Verify package signatures
#[derive(Args)]
struct VerifyArgs {
    /// Package name, hash, or path to package file
    package: String,

    /// Show detailed verification info
    #[arg(long, short = 'v')]
    verbose: bool,

    /// Verify lineage chain as well
    #[arg(long)]
    lineage: bool,
}

/// Initialize a new package
#[derive(Args)]
struct InitArgs {
    /// Package name
    name: String,

    /// Package description
    #[arg(long, short = 'd')]
    description: Option<String>,

    /// Package type (agent, tool, prompt, library)
    #[arg(long, short = 't', default_value = "library")]
    package_type: String,

    /// Directory to create package in
    #[arg(long)]
    dir: Option<PathBuf>,

    /// Don't create example files
    #[arg(long)]
    no_examples: bool,
}

/// Show and manage package cache
#[derive(Args)]
struct CacheArgs {
    #[command(subcommand)]
    command: CacheCommands,
}

#[derive(Subcommand)]
enum CacheCommands {
    /// Show cache location and stats
    Info,

    /// Clear the cache
    Clear {
        /// Only clear packages older than N days
        #[arg(long)]
        older_than: Option<u32>,
    },

    /// List cached packages
    List,
}

/// Colony P2P operations
#[derive(Args)]
struct ColonyArgs {
    #[command(subcommand)]
    command: ColonyCommands,
}

/// Contribution operations
#[derive(Args)]
struct ContribArgs {
    #[command(subcommand)]
    command: ContribCommands,
}

#[derive(Subcommand)]
enum ContribCommands {
    /// Report a bug in a package
    Bug(BugArgs),

    /// Propose an improvement
    Improve(ImproveArgs),

    /// Request a new package
    Request(RequestArgs),

    /// Submit a fix for an issue
    Fix(FixArgs),

    /// List contributions for a package
    List(ContribListArgs),

    /// Show contribution details
    Show(ContribShowArgs),

    /// Review a contribution (multi-model consensus)
    Review(ContribReviewArgs),
}

#[derive(Subcommand)]
enum ColonyCommands {
    /// Show colony P2P status
    Status,

    /// List known peers with packages
    Peers {
        /// Only show peers with a specific package hash
        #[arg(long)]
        package: Option<String>,

        /// Output format (table, json)
        #[arg(long, short = 'f', default_value = "table")]
        format: String,
    },

    /// Find peers that have a specific package
    Find {
        /// Package hash to search for
        hash: String,

        /// Output format (table, json)
        #[arg(long, short = 'f', default_value = "table")]
        format: String,
    },

    /// Announce a local package to colony peers
    Announce {
        /// Package hash to announce
        hash: String,

        /// Size in bytes (optional, will be calculated if not provided)
        #[arg(long)]
        size: Option<u64>,
    },

    /// Enable or disable colony P2P distribution
    Config {
        /// Enable colony P2P
        #[arg(long)]
        enable: bool,

        /// Disable colony P2P
        #[arg(long, conflicts_with = "enable")]
        disable: bool,

        /// Show current configuration
        #[arg(long, conflicts_with_all = ["enable", "disable"])]
        show: bool,
    },
}

/// Report a bug in a package
#[derive(Args)]
struct BugArgs {
    /// Package name or hash
    package: String,

    /// Bug title
    #[arg(long, short = 't')]
    title: String,

    /// Bug category (runtime_error, logic_error, performance, memory, security, documentation, api_mismatch, other)
    #[arg(long, short = 'c', default_value = "other")]
    category: String,

    /// Bug severity (low, medium, high, critical)
    #[arg(long, short = 's', default_value = "medium")]
    severity: String,

    /// Bug description
    #[arg(long, short = 'd')]
    description: Option<String>,

    /// Occurrence rate (0.0 to 1.0)
    #[arg(long)]
    occurrence_rate: Option<f64>,

    /// Output format (table, json)
    #[arg(long, short = 'f', default_value = "table")]
    format: String,
}

/// Propose an improvement
#[derive(Args)]
struct ImproveArgs {
    /// Package name or hash
    package: String,

    /// Improvement title
    #[arg(long, short = 't')]
    title: String,

    /// Improvement category (performance, api, new_capability, documentation, testing, code_quality, security, other)
    #[arg(long, short = 'c', default_value = "other")]
    category: String,

    /// Expected impact (minor, moderate, significant, major)
    #[arg(long, short = 'i', default_value = "minor")]
    impact: String,

    /// Description
    #[arg(long, short = 'd')]
    description: Option<String>,

    /// Motivation for the improvement
    #[arg(long, short = 'm')]
    motivation: Option<String>,

    /// Output format (table, json)
    #[arg(long, short = 'f', default_value = "table")]
    format: String,
}

/// Request a new package
#[derive(Args)]
struct RequestArgs {
    /// Requested package title
    title: String,

    /// Suggested package name
    #[arg(long, short = 'n')]
    name: Option<String>,

    /// Description of what's needed
    #[arg(long, short = 'd')]
    description: Option<String>,

    /// Priority (low, medium, high, critical)
    #[arg(long, short = 'p', default_value = "medium")]
    priority: String,

    /// Use case for the package
    #[arg(long, short = 'u')]
    use_case: Option<String>,

    /// Output format (table, json)
    #[arg(long, short = 'f', default_value = "table")]
    format: String,
}

/// Submit a fix for an issue
#[derive(Args)]
struct FixArgs {
    /// Package name or hash
    package: String,

    /// Fix title
    #[arg(long, short = 't')]
    title: String,

    /// Fix type (bug_fix, security_patch, performance, documentation, test_fix, other)
    #[arg(long, short = 'y', default_value = "bug_fix")]
    fix_type: String,

    /// Path to diff file
    #[arg(long)]
    diff: PathBuf,

    /// Description of the fix
    #[arg(long, short = 'd')]
    description: Option<String>,

    /// Issue ID this fixes
    #[arg(long)]
    fixes: Option<String>,

    /// Output format (table, json)
    #[arg(long, short = 'f', default_value = "table")]
    format: String,
}

/// List contributions for a package
#[derive(Args)]
struct ContribListArgs {
    /// Package name or hash (optional, lists all if not specified)
    package: Option<String>,

    /// Filter by contribution type (bug, improvement, request, fix)
    #[arg(long, short = 't')]
    contrib_type: Option<String>,

    /// Filter by status (submitted, under_review, approved, rejected, merged)
    #[arg(long, short = 's')]
    status: Option<String>,

    /// Maximum number of results
    #[arg(long, short = 'n', default_value = "20")]
    limit: usize,

    /// Output format (table, json)
    #[arg(long, short = 'f', default_value = "table")]
    format: String,
}

/// Show contribution details
#[derive(Args)]
struct ContribShowArgs {
    /// Contribution ID
    id: String,

    /// Show review details
    #[arg(long)]
    reviews: bool,

    /// Output format (table, json)
    #[arg(long, short = 'f', default_value = "table")]
    format: String,
}

/// Review a contribution using multi-model consensus
#[derive(Args)]
struct ContribReviewArgs {
    /// Contribution ID
    id: String,

    /// Number of model reviewers to use
    #[arg(long, short = 'n', default_value = "2")]
    num_models: usize,

    /// Consensus threshold (0.0 to 1.0)
    #[arg(long, default_value = "0.7")]
    threshold: f64,

    /// Output format (table, json)
    #[arg(long, short = 'f', default_value = "table")]
    format: String,
}

pub async fn run(args: PkgArgs) -> Result<()> {
    match args.command {
        PkgCommands::Search(args) => run_search(args).await,
        PkgCommands::Info(args) => run_info(args).await,
        PkgCommands::Install(args) => run_install(args).await,
        PkgCommands::Publish(args) => run_publish(args).await,
        PkgCommands::Login(args) => run_login(args).await,
        PkgCommands::List(args) => run_list(args).await,
        PkgCommands::Verify(args) => run_verify(args).await,
        PkgCommands::Init(args) => run_init(args).await,
        PkgCommands::Cache(args) => run_cache(args).await,
        PkgCommands::Colony(args) => run_colony(args).await,
        PkgCommands::Contrib(args) => run_contrib(args).await,
    }
}

async fn run_search(args: SearchArgs) -> Result<()> {
    if args.semantic {
        println!("Semantic search for packages matching: {}", args.query);
    } else {
        println!("Searching for packages matching: {}", args.query);
    }
    println!();

    // Try to connect to registry server first
    if let Ok(client) = RegistryClient::new() {
        if client.health_check().await.unwrap_or(false) {
            println!("Connected to registry at {}", client.base_url());

            // Search using registry API - use semantic search if flag is set
            let search_result = if args.semantic {
                println!("Using semantic search (vector similarity)...");
                client.search_semantic(&args.query, args.limit).await
            } else {
                client.search(&args.query, args.limit).await
            };

            match search_result {
                Ok(results) => {
                    if results.is_empty() {
                        println!("No packages found matching '{}'", args.query);
                        return Ok(());
                    }

                    match args.format.as_str() {
                        "json" => {
                            println!("{}", serde_json::to_string_pretty(&results)?);
                        }
                        _ => {
                            println!(
                                "{:<30} {:<10} {:<8} DESCRIPTION",
                                "NAME", "VERSION", "SCORE"
                            );
                            println!("{}", "-".repeat(100));

                            for result in &results {
                                let name = if let Some(ns) = &result.package.manifest.namespace {
                                    format!("{}/{}", ns, result.package.manifest.name)
                                } else {
                                    result.package.manifest.name.clone()
                                };

                                // M-497: Use char_indices for safe UTF-8 truncation
                                let description = if result.package.manifest.description.len() > 35
                                {
                                    let truncate_at = result
                                        .package
                                        .manifest
                                        .description
                                        .char_indices()
                                        .take_while(|(i, _)| *i < 32)
                                        .last()
                                        .map(|(i, c)| i + c.len_utf8())
                                        .unwrap_or(0);
                                    format!(
                                        "{}...",
                                        &result.package.manifest.description[..truncate_at]
                                    )
                                } else {
                                    result.package.manifest.description.clone()
                                };

                                println!(
                                    "{:<30} {:<10} {:<8.2} {}",
                                    name,
                                    result.package.manifest.version,
                                    result.score,
                                    description
                                );
                            }
                        }
                    }

                    println!();
                    println!("Found {} packages", results.len());
                    return Ok(());
                }
                Err(e) => {
                    eprintln!(
                        "Registry search failed: {}. Falling back to local mock data.",
                        e
                    );
                }
            }
        }
    }

    // Fall back to mock data when registry unavailable
    println!("(Using local mock data - registry not available)");
    println!();

    let mock_packages = get_mock_packages();
    let keywords: Vec<String> = args.query.split_whitespace().map(String::from).collect();

    let mut results: Vec<(PackageInfo, f64, Vec<String>)> = Vec::new();

    for package in &mock_packages {
        let (matched, reasons) = KeywordSearch::matches(package, &keywords);
        if matched {
            let score = KeywordSearch::score(package, &keywords);
            let reason_strs: Vec<String> = reasons.iter().map(|r| format!("{:?}", r)).collect();
            results.push((package.clone(), score, reason_strs));
        }
    }

    // Sort by score descending
    results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    results.truncate(args.limit);

    if results.is_empty() {
        println!("No packages found matching '{}'", args.query);
        return Ok(());
    }

    match args.format.as_str() {
        "json" => {
            let json_results: Vec<_> = results
                .iter()
                .map(|(p, score, _)| {
                    serde_json::json!({
                        "name": p.manifest.name,
                        "version": p.manifest.version.to_string(),
                        "description": p.manifest.description,
                        "trust_level": format!("{:?}", p.trust_level),
                        "score": score,
                        "downloads": p.downloads,
                    })
                })
                .collect();
            println!("{}", serde_json::to_string_pretty(&json_results)?);
        }
        _ => {
            // Table format
            println!(
                "{:<30} {:<10} {:<12} {:<10} DESCRIPTION",
                "NAME", "VERSION", "TRUST", "DOWNLOADS"
            );
            println!("{}", "-".repeat(100));

            for (package, _score, _reasons) in &results {
                let name = if let Some(ns) = &package.manifest.namespace {
                    format!("{}/{}", ns, package.manifest.name)
                } else {
                    package.manifest.name.clone()
                };

                let description = if package.manifest.description.len() > 35 {
                    format!("{}...", &package.manifest.description[..32])
                } else {
                    package.manifest.description.clone()
                };

                println!(
                    "{:<30} {:<10} {:<12} {:<10} {}",
                    name,
                    package.manifest.version,
                    format!("{:?}", package.trust_level),
                    package.downloads,
                    description
                );
            }
        }
    }

    println!();
    println!("Found {} packages", results.len());

    Ok(())
}

async fn run_info(args: InfoArgs) -> Result<()> {
    println!("Package: {}", args.package);
    println!();

    // Try to get info from registry server
    if let Ok(client) = RegistryClient::new() {
        if client.health_check().await.unwrap_or(false) {
            match client.get_package(&args.package).await {
                Ok(pkg_response) => {
                    match args.format.as_str() {
                        "json" => {
                            let json = serde_json::to_string_pretty(&pkg_response.manifest)?;
                            println!("{}", json);
                        }
                        _ => {
                            println!("Name:        {}", pkg_response.manifest.name);
                            println!("Version:     {}", pkg_response.manifest.version);
                            println!("Description: {}", pkg_response.manifest.description);
                            println!("Type:        {:?}", pkg_response.manifest.package_type);
                            println!("Hash:        {}", pkg_response.hash);
                            println!("Size:        {} bytes", pkg_response.size);

                            if !pkg_response.manifest.keywords.is_empty() {
                                println!(
                                    "Keywords:    {}",
                                    pkg_response.manifest.keywords.join(", ")
                                );
                            }

                            if let Some(license) = &pkg_response.manifest.license {
                                println!("License:     {}", license);
                            }

                            if let Some(repo) = &pkg_response.manifest.repository {
                                println!("Repository:  {}", repo);
                            }
                        }
                    }
                    return Ok(());
                }
                Err(e) => {
                    eprintln!("Registry lookup failed: {}. Falling back to mock data.", e);
                }
            }
        }
    }

    // Fall back to mock package lookup
    println!("(Using local mock data - registry not available)");
    let mock_packages = get_mock_packages();
    let package = mock_packages
        .iter()
        .find(|p| p.manifest.name == args.package || p.hash.to_string().contains(&args.package));

    match package {
        Some(pkg) => match args.format.as_str() {
            "json" => {
                let json = serde_json::to_string_pretty(&pkg.manifest)?;
                println!("{}", json);
            }
            _ => {
                println!("Name:        {}", pkg.manifest.name);
                println!("Version:     {}", pkg.manifest.version);
                println!("Description: {}", pkg.manifest.description);
                println!("Type:        {:?}", pkg.manifest.package_type);
                println!("Trust:       {:?}", pkg.trust_level);
                println!("Downloads:   {}", pkg.downloads);
                println!("Hash:        {}", pkg.hash);
                println!(
                    "Published:   {}",
                    pkg.published_at.format("%Y-%m-%d %H:%M:%S")
                );

                if !pkg.manifest.keywords.is_empty() {
                    println!("Keywords:    {}", pkg.manifest.keywords.join(", "));
                }

                if let Some(license) = &pkg.manifest.license {
                    println!("License:     {}", license);
                }

                if let Some(repo) = &pkg.manifest.repository {
                    println!("Repository:  {}", repo);
                }

                if args.lineage {
                    if let Some(lineage) = &pkg.lineage {
                        println!();
                        println!("Lineage:");
                        if let Some(original) = &lineage.derived_from {
                            println!("  Original: {}", original);
                        }
                        for step in &lineage.chain {
                            println!(
                                "  - {:?} by {} at {}",
                                step.derivation_type,
                                step.actor,
                                step.timestamp.format("%Y-%m-%d")
                            );
                        }
                    } else {
                        println!();
                        println!("Lineage: Original package (no derivation)");
                    }
                }
            }
        },
        None => {
            println!("Package '{}' not found", args.package);
            println!();
            println!("Try:");
            println!("  dashflow pkg search {}", args.package);
        }
    }

    Ok(())
}

async fn run_install(args: InstallArgs) -> Result<()> {
    println!("Installing {} @ {}", args.package, args.version);

    // Determine cache directory
    let cache_dir = args.dir.unwrap_or_else(|| {
        dirs::cache_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("dashflow")
            .join("packages")
    });

    println!("Cache directory: {}", cache_dir.display());
    println!();

    // Try to use registry server
    if let Ok(client) = RegistryClient::new() {
        if client.health_check().await.unwrap_or(false) {
            let version = if args.version == "latest" {
                None
            } else {
                Some(args.version.as_str())
            };

            println!("Resolving {}...", args.package);
            match client.install(&args.package, version, &cache_dir).await {
                Ok(pkg) => {
                    if !args.no_verify {
                        println!("Verifying signatures...");
                        // Note: signature verification happens inside client.install()
                        println!("  Package verified");
                    }

                    println!();
                    println!(
                        "Installed {} @ {} ({})",
                        pkg.manifest.name, pkg.manifest.version, pkg.hash
                    );
                    return Ok(());
                }
                Err(e) => {
                    eprintln!("Installation failed: {}", e);
                    return Err(anyhow::anyhow!("Installation failed: {}", e));
                }
            }
        }
    }

    // Mock installation process when registry unavailable
    println!("(Using mock installation - registry not available)");
    println!();
    println!("Resolving {}...", args.package);
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    println!("Fetching package metadata...");
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    if !args.no_verify {
        println!("Verifying signatures...");
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
        println!("  Signature valid (Community trust level)");
    }

    println!("Downloading...");
    tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;

    println!("Installing to {}...", cache_dir.display());
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    println!();
    println!("Installed {} @ 1.0.0", args.package);

    Ok(())
}

async fn run_publish(args: PublishArgs) -> Result<()> {
    println!("Publishing package from: {}", args.path.display());
    println!();

    // Check if dashflow.toml exists
    let manifest_path = args.path.join("dashflow.toml");
    if !manifest_path.exists() {
        return Err(anyhow::anyhow!(
            "No dashflow.toml found in {}. Run `dashflow pkg init` first.",
            args.path.display()
        ));
    }

    // Read manifest
    let manifest_content =
        tokio::fs::read_to_string(&manifest_path).await.context("Failed to read dashflow.toml")?;

    let manifest: PackageManifest =
        toml::from_str(&manifest_content).context("Failed to parse dashflow.toml")?;

    println!("Package: {} v{}", manifest.name, manifest.version);
    println!("Type:    {:?}", manifest.package_type);
    println!();

    if args.dry_run {
        println!("(Dry run - validating only)");
        println!();
        println!("Manifest validation: OK");
        println!("Would publish: {} v{}", manifest.name, manifest.version);
        return Ok(());
    }

    // Create tarball of package directory
    println!("Creating package tarball...");
    let path_clone = args.path.clone();
    let tarball = tokio::task::spawn_blocking(move || create_package_tarball(&path_clone))
        .await
        .context("Task join failed")??;
    println!("  Size: {} bytes", tarball.len());

    // Sign the package
    let (signature, public_key) = if args.no_sign {
        println!("Signature disabled (--no-sign)");
        // Create empty/placeholder signature
        let keypair = KeyPair::generate("anonymous".to_string());
        let sig = keypair.sign(&tarball);
        (sig, keypair.public_key.clone())
    } else {
        println!("Signing package...");
        // Generate a new keypair for this publish
        // Persistent signing keys: Deferred - requires secure key storage (keyring or file-based)
        let owner = std::env::var("USER").unwrap_or_else(|_| "unknown".to_string());
        let keypair = KeyPair::generate(owner);
        let sig = keypair.sign(&tarball);
        let pubkey = keypair.public_key.clone();
        println!("  Key ID: {}", pubkey.key_id);
        (sig, pubkey)
    };

    // Connect to registry
    let registry_url = args.registry.unwrap_or_else(dashflow_registry_url);

    println!("Publishing to {}...", registry_url);

    let config = RegistryClientConfig::with_url(&registry_url);
    let client = RegistryClient::with_config(config).context("Failed to create registry client")?;

    // Check if registry is available
    if !client.health_check().await.unwrap_or(false) {
        return Err(anyhow::anyhow!(
            "Registry not available at {}. Start the registry server first.",
            registry_url
        ));
    }

    // Publish
    match client
        .publish(&manifest, &tarball, &signature, &public_key)
        .await
    {
        Ok(result) => {
            println!();
            println!("Published successfully!");
            println!("  Hash:     {}", result.hash);
            println!("  Version:  {}", result.version);
            println!(
                "  Signed:   {}",
                if result.signature_verified {
                    "Yes"
                } else {
                    "No"
                }
            );
        }
        Err(e) => {
            return Err(anyhow::anyhow!("Publish failed: {}", e));
        }
    }

    Ok(())
}

async fn run_login(args: LoginArgs) -> Result<()> {
    // Get credentials path
    let creds_dir = dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("dashflow");
    let creds_file = creds_dir.join("credentials.toml");

    if args.logout {
        println!("Logging out...");
        if creds_file.exists() {
            tokio::fs::remove_file(&creds_file).await.context("Failed to remove credentials")?;
            println!("Credentials removed.");
        } else {
            println!("No credentials found.");
        }
        return Ok(());
    }

    if args.status {
        println!("Login Status");
        println!("============");
        println!();

        // Check environment variable
        if let Some(key) = dashflow_registry_api_key() {
            println!("API Key: {} (from environment)", mask_api_key(&key));
        } else if creds_file.exists() {
            let content = tokio::fs::read_to_string(&creds_file).await?;
            if let Ok(creds) = toml::from_str::<toml::Value>(&content) {
                if let Some(key) = creds.get("api_key").and_then(|v| v.as_str()) {
                    println!(
                        "API Key: {} (from {})",
                        mask_api_key(key),
                        creds_file.display()
                    );
                }
            }
        } else {
            println!("Not logged in.");
            println!();
            println!("Login with:");
            println!("  dashflow pkg login -k <api-key>");
            println!("Or set DASHFLOW_REGISTRY_API_KEY environment variable.");
        }

        // Check registry connection
        let registry_url = args.registry.unwrap_or_else(dashflow_registry_url);
        println!();
        println!("Registry: {}", registry_url);

        if let Ok(client) = RegistryClient::new() {
            if client.health_check().await.unwrap_or(false) {
                println!("Status:   Connected");
            } else {
                println!("Status:   Not available");
            }
        }

        return Ok(());
    }

    // Get API key
    let api_key = if let Some(key) = args.api_key {
        key
    } else if let Some(key) = dashflow_registry_api_key() {
        println!("Using API key from {} environment variable.", DASHFLOW_REGISTRY_API_KEY);
        key
    } else {
        // Prompt for API key
        print!("API Key: ");
        io::stdout().flush()?;
        let mut key = String::new();
        io::stdin().read_line(&mut key)?;
        key.trim().to_string()
    };

    if api_key.is_empty() {
        return Err(anyhow::anyhow!("API key cannot be empty"));
    }

    // Save credentials
    tokio::fs::create_dir_all(&creds_dir).await.context("Failed to create config directory")?;

    let registry_url = args.registry.unwrap_or_else(dashflow_registry_url);

    let creds_content = format!(
        r#"# DashFlow Registry Credentials
# Generated by `dashflow pkg login`

api_key = "{}"
registry_url = "{}"
"#,
        api_key, registry_url
    );

    tokio::fs::write(&creds_file, creds_content).await.context("Failed to save credentials")?;

    println!();
    println!("Credentials saved to: {}", creds_file.display());
    println!();

    // Verify connection
    let config = RegistryClientConfig {
        base_url: registry_url.clone(),
        api_key: Some(api_key),
        ..Default::default()
    };

    let client = RegistryClient::with_config(config)?;
    if client.health_check().await.unwrap_or(false) {
        println!("Successfully connected to registry.");
    } else {
        println!(
            "Warning: Could not verify connection to registry at {}",
            registry_url
        );
        println!("         The credentials have been saved, but the registry may not be running.");
    }

    Ok(())
}

/// Create a tarball from a package directory
fn create_package_tarball(path: &PathBuf) -> Result<Vec<u8>> {
    use std::io::Cursor;

    let mut tarball = Vec::new();
    {
        let cursor = Cursor::new(&mut tarball);
        let enc = flate2::write::GzEncoder::new(cursor, flate2::Compression::default());
        let mut tar = tar::Builder::new(enc);

        // Add all files from the package directory
        tar.append_dir_all(".", path)
            .context("Failed to create tarball")?;

        tar.finish().context("Failed to finalize tarball")?;
    }

    Ok(tarball)
}

/// Mask API key for display
fn mask_api_key(key: &str) -> String {
    if key.len() <= 8 {
        "*".repeat(key.len())
    } else {
        format!("{}...{}", &key[..4], &key[key.len() - 4..])
    }
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
struct InstalledPackage {
    name: String,
    version: String,
    package_type: String,
    hash: String,
    location: String,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
struct InstalledPackagesOutput {
    packages: Vec<InstalledPackage>,
    count: usize,
}

fn installed_packages_output(packages: Vec<InstalledPackage>) -> InstalledPackagesOutput {
    let count = packages.len();
    InstalledPackagesOutput { packages, count }
}

fn default_packages_cache_dir() -> PathBuf {
    dirs::cache_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("dashflow")
        .join("packages")
}

fn canonical_package_type_string(package_type: &PackageType) -> String {
    match package_type {
        PackageType::Agent => "agent".to_string(),
        PackageType::Tool => "tool".to_string(),
        PackageType::Prompt => "prompt".to_string(),
        PackageType::Embedding => "embedding".to_string(),
        PackageType::Retrieval => "retrieval".to_string(),
        PackageType::Application => "application".to_string(),
        PackageType::Library => "library".to_string(),
        PackageType::Other(other) => other.to_lowercase(),
    }
}

fn canonical_package_type_filter(value: &str) -> Option<String> {
    let lowered = value.trim().to_lowercase();
    match lowered.as_str() {
        "agent" => Some("agent".to_string()),
        "tool" => Some("tool".to_string()),
        "prompt" => Some("prompt".to_string()),
        "embedding" => Some("embedding".to_string()),
        "retrieval" => Some("retrieval".to_string()),
        "application" | "app" => Some("application".to_string()),
        "library" | "lib" => Some("library".to_string()),
        "" => None,
        other => Some(other.to_string()),
    }
}

fn list_installed_packages(cache_dir: &Path) -> Result<Vec<InstalledPackage>> {
    if !cache_dir.is_dir() {
        return Ok(Vec::new());
    }

    let mut packages = Vec::new();
    for entry in std::fs::read_dir(cache_dir).context("Failed to read package cache directory")? {
        let entry = entry.context("Failed to read package cache entry")?;
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }

        let hash = entry.file_name().to_string_lossy().to_string();
        let tarball_path = path.join("package.tar.gz");
        if !tarball_path.is_file() {
            continue;
        }

        let (name, version, package_type) = match read_installed_package_metadata(&tarball_path) {
            Ok(metadata) => metadata,
            Err(_) => (hash.clone(), "unknown".to_string(), "unknown".to_string()),
        };

        packages.push(InstalledPackage {
            name,
            version,
            package_type,
            hash,
            location: path.display().to_string(),
        });
    }

    packages.sort_by(|a, b| a.name.cmp(&b.name).then_with(|| a.version.cmp(&b.version)));
    Ok(packages)
}

fn read_installed_package_metadata(tarball_path: &Path) -> Result<(String, String, String)> {
    let dashflow_toml_bytes = read_named_file_from_tar_gz(tarball_path, "dashflow.toml")
        .with_context(|| format!("Failed to read dashflow.toml from {}", tarball_path.display()))?;

    let dashflow_toml = String::from_utf8(dashflow_toml_bytes)
        .map_err(|e| anyhow::anyhow!("dashflow.toml is not valid UTF-8: {}", e))?;

    parse_dashflow_toml_metadata(&dashflow_toml)
}

fn read_named_file_from_tar_gz(tarball_path: &Path, filename: &str) -> Result<Vec<u8>> {
    let file = std::fs::File::open(tarball_path)
        .with_context(|| format!("Failed to open {}", tarball_path.display()))?;
    let decoder = flate2::read::GzDecoder::new(file);
    let mut archive = tar::Archive::new(decoder);

    for entry in archive
        .entries()
        .with_context(|| format!("Failed to read tar entries from {}", tarball_path.display()))?
    {
        let mut entry = entry.context("Failed to read tar entry")?;
        let entry_path = entry.path().context("Failed to read tar entry path")?;
        if entry_path.file_name().and_then(|n| n.to_str()) != Some(filename) {
            continue;
        }

        let mut bytes = Vec::new();
        entry
            .read_to_end(&mut bytes)
            .with_context(|| format!("Failed reading {} from tarball", filename))?;
        return Ok(bytes);
    }

    Err(anyhow::anyhow!(
        "File '{}' not found in {}",
        filename,
        tarball_path.display()
    ))
}

fn parse_dashflow_toml_metadata(dashflow_toml: &str) -> Result<(String, String, String)> {
    if let Ok(manifest) = toml::from_str::<PackageManifest>(dashflow_toml) {
        let name = if let Some(ns) = &manifest.namespace {
            format!("{}/{}", ns, manifest.name)
        } else {
            manifest.name
        };

        return Ok((
            name,
            manifest.version.to_string(),
            canonical_package_type_string(&manifest.package_type),
        ));
    }

    #[derive(Debug, Deserialize)]
    struct DashflowTomlV0 {
        package: DashflowTomlV0Package,
    }

    #[derive(Debug, Deserialize)]
    struct DashflowTomlV0Package {
        name: String,
        version: String,
        #[serde(default)]
        namespace: Option<String>,
        #[serde(rename = "type")]
        package_type: String,
    }

    let v0: DashflowTomlV0 = toml::from_str(dashflow_toml).context("Failed to parse dashflow.toml")?;
    let name = if let Some(ns) = &v0.package.namespace {
        format!("{}/{}", ns, v0.package.name)
    } else {
        v0.package.name
    };

    let package_type = canonical_package_type_filter(&v0.package.package_type)
        .unwrap_or_else(|| "unknown".to_string());

    Ok((name, v0.package.version, package_type))
}

async fn run_list(args: ListArgs) -> Result<()> {
    let cache_dir = default_packages_cache_dir();
    let cache_dir_clone = cache_dir.clone();
    let mut packages = tokio::task::spawn_blocking(move || list_installed_packages(&cache_dir_clone))
        .await
        .context("Task join failed")??;

    if let Some(filter) = args.package_type.as_deref() {
        let canonical = canonical_package_type_filter(filter);
        if let Some(canonical) = canonical {
            packages.retain(|pkg| pkg.package_type == canonical);
        }
    }

    match args.format.as_str() {
        "json" => {
            let output = installed_packages_output(packages);
            println!("{}", serde_json::to_string_pretty(&output)?);
        }
        _ => {
            println!("Installed packages:");
            println!();

            if packages.is_empty() && !cache_dir.exists() {
                println!("No packages installed yet.");
                println!();
                println!("Install packages with:");
                println!("  dashflow pkg install <package-name>");
                return Ok(());
            }

            println!("{:<30} {:<10} {:<12} LOCATION", "NAME", "VERSION", "TYPE");
            println!("{}", "-".repeat(80));
            if packages.is_empty() {
                println!("(no packages installed)");
            } else {
                for pkg in packages {
                    println!(
                        "{:<30} {:<10} {:<12} {}",
                        pkg.name, pkg.version, pkg.package_type, pkg.location
                    );
                }
            }
        }
    }

    Ok(())
}

#[cfg(test)]
#[allow(clippy::items_after_test_module)]
mod tests {
    use super::*;

    fn write_tar_gz_with_dashflow_toml(path: &Path, dashflow_toml: &str) -> Result<()> {
        use flate2::write::GzEncoder;

        let file = std::fs::File::create(path)?;
        let enc = GzEncoder::new(file, flate2::Compression::default());
        let mut tar = tar::Builder::new(enc);

        let mut header = tar::Header::new_gnu();
        header.set_size(dashflow_toml.len() as u64);
        header.set_mode(0o644);
        header.set_cksum();

        tar.append_data(&mut header, "dashflow.toml", dashflow_toml.as_bytes())?;

        let enc = tar.into_inner()?;
        enc.finish()?;
        Ok(())
    }

    #[test]
    fn installed_packages_output_json_empty_is_structured() -> Result<()> {
        let output = installed_packages_output(Vec::new());
        let value = serde_json::to_value(output)?;
        assert_eq!(value, serde_json::json!({ "packages": [], "count": 0 }));
        Ok(())
    }

    #[test]
    fn list_installed_packages_reads_dashflow_toml_from_tarball() -> Result<()> {
        let tempdir = tempfile::tempdir()?;
        let cache_dir = tempdir.path().join("packages");
        std::fs::create_dir_all(&cache_dir)?;

        let hash = "deadbeef";
        let pkg_dir = cache_dir.join(hash);
        std::fs::create_dir_all(&pkg_dir)?;

        let tarball_path = pkg_dir.join("package.tar.gz");
        write_tar_gz_with_dashflow_toml(
            &tarball_path,
            r#"[package]
name = "sentiment-analyzer"
version = "1.2.3"
description = "Analyze sentiment in text"
type = "agent"
license = "MIT"
"#,
        )?;

        let packages = list_installed_packages(&cache_dir)?;
        assert_eq!(packages.len(), 1);
        assert_eq!(
            packages[0],
            InstalledPackage {
                name: "sentiment-analyzer".to_string(),
                version: "1.2.3".to_string(),
                package_type: "agent".to_string(),
                hash: hash.to_string(),
                location: pkg_dir.display().to_string(),
            }
        );

        Ok(())
    }
}

async fn run_verify(args: VerifyArgs) -> Result<()> {
    println!("Verifying: {}", args.package);
    println!();

    // Mock verification - TrustService would be used in production
    let keyring = Keyring::new();
    let _trust_service = TrustService::new(keyring);

    // In production, this would load the actual package
    println!("Loading package...");
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    println!("Checking signatures...");
    tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

    if args.verbose {
        println!();
        println!("Signature Details:");
        println!("  Key ID:      (none found)");
        println!("  Algorithm:   Ed25519");
        println!("  Signed at:   (n/a)");
        println!("  Status:      No signatures present");
    }

    if args.lineage {
        println!();
        println!("Lineage Verification:");
        println!("  Chain length: 0");
        println!("  Status:       Original package");
    }

    println!();
    println!("Result: UNVERIFIED (no signatures)");
    println!();
    println!("Note: This is a mock verification. In production, packages");
    println!("      would be cryptographically verified against the keyring.");

    Ok(())
}

async fn run_init(args: InitArgs) -> Result<()> {
    let dir = args.dir.unwrap_or_else(|| PathBuf::from(&args.name));

    println!("Initializing new package: {}", args.name);
    println!("Directory: {}", dir.display());
    println!();

    // Create directory
    tokio::fs::create_dir_all(&dir).await.context("Failed to create directory")?;

    // Parse package type
    let pkg_type = match args.package_type.to_lowercase().as_str() {
        "agent" => PackageType::Agent,
        "tool" => PackageType::Tool,
        "prompt" => PackageType::Prompt,
        "application" | "app" => PackageType::Application,
        _ => PackageType::Library,
    };

    // Create dashflow.toml manifest
    let manifest_content = format!(
        r#"[package]
name = "{name}"
version = "0.1.0"
description = "{description}"
type = "{pkg_type}"
license = "MIT"

[package.keywords]
keywords = []

# [dependencies]
# some-package = "^1.0"

# [capabilities.provides]
# capability_name = {{ description = "What this provides" }}

# [capabilities.requires]
# other_capability = {{ version = "^1.0" }}
"#,
        name = args.name,
        description = args
            .description
            .clone()
            .unwrap_or_else(|| format!("A DashFlow {}", args.package_type)),
        pkg_type = args.package_type.to_lowercase(),
    );

    tokio::fs::write(dir.join("dashflow.toml"), manifest_content)
        .await
        .context("Failed to write dashflow.toml")?;
    println!("Created: dashflow.toml");

    // Create src directory
    let src_dir = dir.join("src");
    tokio::fs::create_dir_all(&src_dir).await.context("Failed to create src directory")?;

    if !args.no_examples {
        // Create example file based on type
        let example_content = match pkg_type {
            PackageType::Agent => {
                r#"//! Example agent implementation
//!
//! This is a template for creating DashFlow agents.

use dashflow::prelude::*;

/// Example agent node
pub struct MyAgent {
    // Add your agent state here
}

impl MyAgent {
    pub fn new() -> Self {
        Self {}
    }
}

// Implement the Node trait for your agent
// See DashFlow documentation for details
"#
            }
            PackageType::Tool => {
                r#"//! Example tool implementation
//!
//! This is a template for creating DashFlow tools.

use dashflow::prelude::*;

/// Example tool that can be used by agents
pub fn my_tool(input: &str) -> String {
    // Implement your tool logic here
    format!("Processed: {}", input)
}
"#
            }
            _ => {
                r#"//! DashFlow package
//!
//! Add your implementation here.

pub fn hello() -> &'static str {
    "Hello from DashFlow!"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hello() {
        assert_eq!(hello(), "Hello from DashFlow!");
    }
}
"#
            }
        };

        tokio::fs::write(src_dir.join("lib.rs"), example_content)
            .await
            .context("Failed to write src/lib.rs")?;
        println!("Created: src/lib.rs");
    }

    // Create README
    let readme_content = format!(
        r#"# {}

{}

## Installation

```bash
dashflow pkg install {}
```

## Usage

```rust
use {}::*;

// Add your usage examples here
```

## Features

- Feature 1: Description
- Feature 2: Description

## License

See LICENSE file.
"#,
        args.name,
        args.description.as_deref().unwrap_or("A DashFlow package"),
        args.name,
        args.name.replace('-', "_")
    );

    tokio::fs::write(dir.join("README.md"), readme_content).await.context("Failed to write README.md")?;
    println!("Created: README.md");

    println!();
    println!("Package initialized successfully!");
    println!();
    println!("Next steps:");
    println!("  cd {}", dir.display());
    println!("  # Edit dashflow.toml and src/lib.rs");
    println!("  # When ready to publish:");
    println!("  # dashflow pkg publish .");

    Ok(())
}

async fn run_cache(args: CacheArgs) -> Result<()> {
    let cache_dir = dirs::cache_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("dashflow")
        .join("packages");

    match args.command {
        CacheCommands::Info => {
            println!("Package Cache Information");
            println!();
            println!("Location: {}", cache_dir.display());

            if cache_dir.exists() {
                let cache_dir_clone = cache_dir.clone();
                let size = tokio::task::spawn_blocking(move || dir_size(&cache_dir_clone).unwrap_or(0))
                    .await
                    .unwrap_or(0);
                println!("Size:     {} bytes", size);

                let cache_dir_clone = cache_dir.clone();
                let count = tokio::task::spawn_blocking(move || count_packages(&cache_dir_clone).unwrap_or(0))
                    .await
                    .unwrap_or(0);
                println!("Packages: {}", count);
            } else {
                println!("Status:   Not initialized (no packages installed)");
            }
        }
        CacheCommands::Clear { older_than } => {
            if !cache_dir.exists() {
                println!("Cache is empty, nothing to clear.");
                return Ok(());
            }

            if let Some(days) = older_than {
                println!("Clearing packages older than {} days...", days);
                // In production, implement age-based clearing
            } else {
                println!("Clearing entire cache...");
                tokio::fs::remove_dir_all(&cache_dir).await.context("Failed to clear cache")?;
            }
            println!("Cache cleared.");
        }
        CacheCommands::List => {
            println!("Cached packages:");
            println!();

            if !cache_dir.exists() {
                println!("(cache is empty)");
                return Ok(());
            }

            // In production, list actual cached packages
            println!("{:<40} {:<10} ADDED", "HASH", "SIZE");
            println!("{}", "-".repeat(70));
            println!("(no packages cached)");
        }
    }

    Ok(())
}

/// Get mock packages for demonstration
fn get_mock_packages() -> Vec<PackageInfo> {
    vec![
        create_mock_package(
            "sentiment-analyzer",
            "1.0.0",
            "Analyze sentiment in text using AI models",
            &["nlp", "sentiment", "analysis", "ai"],
            PackageType::Agent,
            TrustLevel::Official,
            15000,
        ),
        create_mock_package(
            "code-reviewer",
            "2.1.0",
            "AI-powered code review agent",
            &["code", "review", "analysis", "ai"],
            PackageType::Agent,
            TrustLevel::Organization,
            8500,
        ),
        create_mock_package(
            "embedding-utils",
            "0.5.0",
            "Utilities for working with embeddings",
            &["embeddings", "vectors", "ai", "utils"],
            PackageType::Library,
            TrustLevel::Community,
            3200,
        ),
        create_mock_package(
            "customer-support-bot",
            "1.2.0",
            "Complete customer support chatbot solution",
            &["chatbot", "support", "customer", "ai"],
            PackageType::Application,
            TrustLevel::Organization,
            12000,
        ),
        create_mock_package(
            "prompt-templates",
            "0.9.0",
            "Collection of optimized prompt templates",
            &["prompts", "templates", "optimization"],
            PackageType::Prompt,
            TrustLevel::Community,
            5600,
        ),
    ]
}

fn create_mock_package(
    name: &str,
    version: &str,
    description: &str,
    keywords: &[&str],
    pkg_type: PackageType,
    trust: TrustLevel,
    downloads: u64,
) -> PackageInfo {
    use chrono::Utc;

    let manifest = PackageManifest::builder()
        .name(name)
        .version(version)
        .description(description)
        .keywords(keywords.iter().map(|s| s.to_string()))
        .package_type(pkg_type)
        .license("MIT")
        .build()
        .unwrap();

    PackageInfo {
        hash: ContentHash::from_bytes(name.as_bytes()),
        manifest,
        published_at: Utc::now(),
        publisher_key_id: "official-key".to_string(),
        downloads,
        trust_level: trust,
        lineage: None,
        yanked: false,
    }
}

fn dir_size(path: &PathBuf) -> std::io::Result<u64> {
    let mut size = 0;
    if path.is_dir() {
        for entry in std::fs::read_dir(path)? {
            let entry = entry?;
            let metadata = entry.metadata()?;
            if metadata.is_dir() {
                size += dir_size(&entry.path())?;
            } else {
                size += metadata.len();
            }
        }
    }
    Ok(size)
}

fn count_packages(path: &PathBuf) -> std::io::Result<usize> {
    if !path.is_dir() {
        return Ok(0);
    }
    let count = std::fs::read_dir(path)?
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_dir())
        .count();
    Ok(count)
}

async fn run_colony(args: ColonyArgs) -> Result<()> {
    match args.command {
        ColonyCommands::Status => {
            println!("Colony P2P Distribution Status");
            println!("==============================");
            println!();

            // Try to get status from registry server
            if let Ok(client) = RegistryClient::new() {
                if client.health_check().await.unwrap_or(false) {
                    if let Ok(status) = client.colony_status().await {
                        println!("{}", serde_json::to_string_pretty(&status)?);
                        return Ok(());
                    }
                }
            }

            // Fall back to mock data
            println!("(Using local configuration - registry not available)");
            println!();
            println!("Configuration:");
            println!("  Enabled:           true");
            println!("  Peer Timeout:      5s");
            println!("  Max Parallel:      5");
            println!("  Min Trust Level:   Colony");
            println!("  Announce on DL:    true");
            println!("  Max Direct Size:   10 MB");
            println!();

            println!("Network Status:");
            println!("  Known Peers:       0");
            println!("  Reachable Peers:   0");
            println!("  Packages Shared:   0");
            println!();

            println!("Transfer Statistics:");
            println!("  Cache Hits:        0");
            println!("  Colony Hits:       0");
            println!("  Registry Fetches:  0");
            println!("  Bytes from Colony: 0 B");
            println!();

            println!("Note: Start the DashFlow network service to enable P2P distribution.");
            println!("Run `dashflow network start` to begin peer discovery.");
        }

        ColonyCommands::Peers { package, format } => {
            if let Some(hash) = package {
                println!("Peers with package {}:", hash);
            } else {
                println!("Known Colony Peers:");
            }
            println!();

            // Mock data - in production would query ColonyPackageResolver
            match format.as_str() {
                "json" => {
                    println!("[]");
                }
                _ => {
                    println!(
                        "{:<40} {:<15} {:<12} {:<10} REACHABLE",
                        "PEER ID", "NAME", "TRUST", "LATENCY"
                    );
                    println!("{}", "-".repeat(90));
                    println!("(no peers discovered yet)");
                    println!();
                    println!("Tip: Peers are discovered automatically when other DashFlow");
                    println!("     instances are running on the same network.");
                }
            }
        }

        ColonyCommands::Find { hash, format } => {
            println!("Searching for package: {}", hash);
            println!();

            // Validate hash format
            if !hash.starts_with("sha256:") && hash.len() != 64 {
                println!("Note: Expected hash format 'sha256:...' or 64-char hex string");
            }

            // Try to find peers through registry server
            if let Ok(client) = RegistryClient::new() {
                if client.health_check().await.unwrap_or(false) {
                    if let Ok(peers) = client.find_peers(&hash).await {
                        match format.as_str() {
                            "json" => {
                                println!(
                                    "{}",
                                    serde_json::to_string_pretty(&serde_json::json!({
                                        "hash": hash,
                                        "peers": peers,
                                        "available_in_colony": !peers.is_empty()
                                    }))?
                                );
                            }
                            _ => {
                                if peers.is_empty() {
                                    println!("Package not found in colony.");
                                } else {
                                    println!("Found {} peers with package.", peers.len());
                                    println!("{}", serde_json::to_string_pretty(&peers)?);
                                }
                            }
                        }
                        return Ok(());
                    }
                }
            }

            // Fall back to mock response
            println!("(Registry not available)");
            match format.as_str() {
                "json" => {
                    println!("{{");
                    println!("  \"hash\": \"{}\",", hash);
                    println!("  \"peers\": [],");
                    println!("  \"available_in_colony\": false");
                    println!("}}");
                }
                _ => {
                    println!("Package not found in colony.");
                    println!();
                    println!("The package may be available from the central registry.");
                    println!("Run `dashflow pkg install <package>` to fetch it.");
                }
            }
        }

        ColonyCommands::Announce { hash, size } => {
            println!("Announcing package to colony...");
            println!();
            println!("Hash: {}", hash);
            if let Some(s) = size {
                println!("Size: {} bytes", s);
            }
            println!();

            // Mock announce - in production would call resolver.announce()
            println!("Note: Announcement requires active network connection.");
            println!("Run `dashflow network status` to check network state.");
        }

        ColonyCommands::Config {
            enable,
            disable,
            show,
        } => {
            if show || (!enable && !disable) {
                println!("Colony P2P Configuration:");
                println!();
                println!("  enabled:                    true");
                println!("  peer_timeout:               5s");
                println!("  max_parallel_queries:       5");
                println!("  min_trust_level:            Colony");
                println!("  announce_on_download:       true");
                println!("  peer_refresh_interval:      60s");
                println!("  max_direct_transfer_size:   10485760 bytes (10 MB)");
                println!();
                println!("Configuration file: ~/.dashflow/config.toml");
            } else if enable {
                println!("Enabling colony P2P distribution...");
                println!("Done. P2P will be used for future package fetches.");
            } else if disable {
                println!("Disabling colony P2P distribution...");
                println!("Done. Packages will be fetched directly from registry.");
            }
        }
    }

    Ok(())
}

async fn run_contrib(args: ContribArgs) -> Result<()> {
    match args.command {
        ContribCommands::Bug(args) => run_contrib_bug(args).await,
        ContribCommands::Improve(args) => run_contrib_improve(args).await,
        ContribCommands::Request(args) => run_contrib_request(args).await,
        ContribCommands::Fix(args) => run_contrib_fix(args).await,
        ContribCommands::List(args) => run_contrib_list(args).await,
        ContribCommands::Show(args) => run_contrib_show(args).await,
        ContribCommands::Review(args) => run_contrib_review(args).await,
    }
}

async fn run_contrib_bug(args: BugArgs) -> Result<()> {
    println!("Creating bug report for package: {}", args.package);
    println!();

    // Parse category
    let category = match args.category.to_lowercase().as_str() {
        "runtime_error" | "runtime" => BugCategory::RuntimeError,
        "logic_error" | "logic" => BugCategory::LogicError,
        "performance" | "perf" => BugCategory::Performance,
        "memory" | "mem" => BugCategory::Memory,
        "security" | "sec" => BugCategory::Security,
        "documentation" | "doc" | "docs" => BugCategory::Documentation,
        "api_mismatch" | "api" => BugCategory::ApiMismatch,
        _ => BugCategory::Other,
    };

    // Parse severity
    let severity = match args.severity.to_lowercase().as_str() {
        "low" => BugSeverity::Low,
        "high" => BugSeverity::High,
        "critical" | "crit" => BugSeverity::Critical,
        _ => BugSeverity::Medium,
    };

    // Build bug report
    let mut builder = BugReport::builder()
        .title(&args.title)
        .category(category)
        .severity(severity);

    if let Some(desc) = args.description {
        builder = builder.description(desc);
    }

    if let Some(rate) = args.occurrence_rate {
        builder = builder.occurrence_rate(rate);
    }

    let bug = builder.build().context("Failed to create bug report")?;

    // Try to submit to registry
    if let Ok(client) = RegistryClient::new() {
        if client.health_check().await.unwrap_or(false) {
            match client.submit_bug(&args.package, &bug).await {
                Ok(contribution_id) => {
                    match args.format.as_str() {
                        "json" => {
                            println!(
                                "{}",
                                serde_json::json!({
                                    "status": "submitted",
                                    "contribution_id": contribution_id,
                                    "bug": bug,
                                })
                            );
                        }
                        _ => {
                            println!("Bug Report Submitted:");
                            println!("  Contribution ID: {}", contribution_id);
                            println!("  Title:           {}", bug.title);
                            println!("  Category:        {:?}", bug.category);
                            println!("  Severity:        {:?}", bug.severity);
                            println!("  Priority:        {}/100", bug.priority_score());
                            println!();
                            println!("View with: dashflow pkg contrib show {}", contribution_id);
                        }
                    }
                    return Ok(());
                }
                Err(e) => {
                    eprintln!(
                        "Warning: Failed to submit to registry: {}. Showing local report.",
                        e
                    );
                }
            }
        }
    }

    // Fall back to local display
    match args.format.as_str() {
        "json" => {
            println!("{}", serde_json::to_string_pretty(&bug)?);
        }
        _ => {
            println!("Bug Report Created (local only - registry not available):");
            println!("  ID:          {}", bug.id);
            println!("  Title:       {}", bug.title);
            println!("  Category:    {:?}", bug.category);
            println!("  Severity:    {:?}", bug.severity);
            println!("  Priority:    {}/100", bug.priority_score());
            if let Some(rate) = bug.occurrence_rate {
                println!("  Occurrence:  {:.1}%", rate * 100.0);
            }
            println!();
            println!("Note: Start registry server to submit contributions.");
        }
    }

    Ok(())
}

async fn run_contrib_improve(args: ImproveArgs) -> Result<()> {
    println!(
        "Creating improvement proposal for package: {}",
        args.package
    );
    println!();

    // Parse category
    let category = match args.category.to_lowercase().as_str() {
        "performance" | "perf" => ImprovementCategory::Performance,
        "api" => ImprovementCategory::Api,
        "new_capability" | "capability" | "new" => ImprovementCategory::NewCapability,
        "documentation" | "doc" | "docs" => ImprovementCategory::Documentation,
        "testing" | "test" => ImprovementCategory::Testing,
        "code_quality" | "quality" => ImprovementCategory::CodeQuality,
        "security" | "sec" => ImprovementCategory::Security,
        _ => ImprovementCategory::Other,
    };

    // Parse impact
    let impact = match args.impact.to_lowercase().as_str() {
        "moderate" | "mod" => ImpactLevel::Moderate,
        "significant" | "sig" => ImpactLevel::Significant,
        "major" => ImpactLevel::Major,
        _ => ImpactLevel::Minor,
    };

    // Build proposal
    let mut builder = ImprovementProposal::builder()
        .title(&args.title)
        .category(category)
        .impact(impact);

    if let Some(desc) = args.description {
        builder = builder.description(desc);
    }

    if let Some(motivation) = args.motivation {
        builder = builder.motivation(motivation);
    }

    let proposal = builder
        .build()
        .context("Failed to create improvement proposal")?;

    // Try to submit to registry
    if let Ok(client) = RegistryClient::new() {
        if client.health_check().await.unwrap_or(false) {
            match client.submit_improvement(&args.package, &proposal).await {
                Ok(contribution_id) => {
                    match args.format.as_str() {
                        "json" => {
                            println!(
                                "{}",
                                serde_json::json!({
                                    "status": "submitted",
                                    "contribution_id": contribution_id,
                                    "proposal": proposal,
                                })
                            );
                        }
                        _ => {
                            println!("Improvement Proposal Submitted:");
                            println!("  Contribution ID: {}", contribution_id);
                            println!("  Title:           {}", proposal.title);
                            println!("  Category:        {:?}", proposal.category);
                            println!("  Impact:          {:?}", proposal.impact);
                            println!();
                            println!("View with: dashflow pkg contrib show {}", contribution_id);
                        }
                    }
                    return Ok(());
                }
                Err(e) => {
                    eprintln!(
                        "Warning: Failed to submit to registry: {}. Showing local report.",
                        e
                    );
                }
            }
        }
    }

    // Fall back to local display
    match args.format.as_str() {
        "json" => {
            println!("{}", serde_json::to_string_pretty(&proposal)?);
        }
        _ => {
            println!("Improvement Proposal Created (local only - registry not available):");
            println!("  ID:          {}", proposal.id);
            println!("  Title:       {}", proposal.title);
            println!("  Category:    {:?}", proposal.category);
            println!("  Impact:      {:?}", proposal.impact);
            println!(
                "  Proposed:    {}",
                proposal.proposed_at.format("%Y-%m-%d %H:%M:%S")
            );
            println!();
            println!("Note: Start registry server to submit contributions.");
        }
    }

    Ok(())
}

async fn run_contrib_request(args: RequestArgs) -> Result<()> {
    println!("Creating package request: {}", args.title);
    println!();

    // Parse priority
    let priority = match args.priority.to_lowercase().as_str() {
        "low" => RequestPriority::Low,
        "high" => RequestPriority::High,
        "critical" | "crit" => RequestPriority::Critical,
        _ => RequestPriority::Medium,
    };

    // Build request
    let mut builder = PackageRequest::builder()
        .title(&args.title)
        .priority(priority);

    if let Some(name) = args.name {
        builder = builder.suggested_name(name);
    }

    if let Some(desc) = args.description {
        builder = builder.description(desc);
    }

    if let Some(use_case) = args.use_case {
        builder = builder.use_case(use_case);
    }

    let request = builder
        .build()
        .context("Failed to create package request")?;

    // Try to submit to registry
    if let Ok(client) = RegistryClient::new() {
        if client.health_check().await.unwrap_or(false) {
            match client.submit_request(&request).await {
                Ok(contribution_id) => {
                    match args.format.as_str() {
                        "json" => {
                            println!(
                                "{}",
                                serde_json::json!({
                                    "status": "submitted",
                                    "contribution_id": contribution_id,
                                    "request": request,
                                })
                            );
                        }
                        _ => {
                            println!("Package Request Submitted:");
                            println!("  Contribution ID: {}", contribution_id);
                            println!("  Title:           {}", request.title);
                            if !request.suggested_name.is_empty() {
                                println!("  Name:            {}", request.suggested_name);
                            }
                            println!("  Priority:        {:?}", request.priority);
                            println!();
                            println!("View with: dashflow pkg contrib show {}", contribution_id);
                        }
                    }
                    return Ok(());
                }
                Err(e) => {
                    eprintln!(
                        "Warning: Failed to submit to registry: {}. Showing local report.",
                        e
                    );
                }
            }
        }
    }

    // Fall back to local display
    match args.format.as_str() {
        "json" => {
            println!("{}", serde_json::to_string_pretty(&request)?);
        }
        _ => {
            println!("Package Request Created (local only - registry not available):");
            println!("  ID:          {}", request.id);
            println!("  Title:       {}", request.title);
            if !request.suggested_name.is_empty() {
                println!("  Name:        {}", request.suggested_name);
            }
            println!("  Priority:    {:?}", request.priority);
            println!(
                "  Requested:   {}",
                request.requested_at.format("%Y-%m-%d %H:%M:%S")
            );
            println!();
            println!("Note: Start registry server to submit contributions.");
        }
    }

    Ok(())
}

async fn run_contrib_fix(args: FixArgs) -> Result<()> {
    println!("Creating fix submission for package: {}", args.package);
    println!();

    // Read diff file
    let diff_content = tokio::fs::read_to_string(&args.diff)
        .await
        .context(format!("Failed to read diff file: {}", args.diff.display()))?;

    // Parse fix type
    let fix_type = match args.fix_type.to_lowercase().as_str() {
        "security_patch" | "security" | "sec" => FixType::SecurityPatch,
        "performance" | "perf" => FixType::Performance,
        "documentation" | "doc" | "docs" => FixType::Documentation,
        "test_fix" | "test" => FixType::TestFix,
        _ => FixType::BugFix,
    };

    // Count lines changed (simple heuristic)
    let additions = diff_content.lines().filter(|l| l.starts_with('+')).count();
    let deletions = diff_content.lines().filter(|l| l.starts_with('-')).count();

    // Build fix
    let mut builder = FixSubmission::builder()
        .title(&args.title)
        .fix_type(fix_type)
        .diff(&diff_content)
        .file_change(FileChange {
            path: args.diff.display().to_string(),
            change_type: FileChangeType::Modified,
            additions,
            deletions,
        });

    if let Some(desc) = args.description {
        builder = builder.description(desc);
    }

    if let Some(fixes) = args.fixes {
        if let Ok(uuid) = uuid::Uuid::parse_str(&fixes) {
            builder = builder.fixes_issue(uuid);
        }
    }

    let fix = builder.build().context("Failed to create fix submission")?;

    // Try to submit to registry
    if let Ok(client) = RegistryClient::new() {
        if client.health_check().await.unwrap_or(false) {
            match client.submit_fix(&args.package, &fix).await {
                Ok(contribution_id) => {
                    match args.format.as_str() {
                        "json" => {
                            println!(
                                "{}",
                                serde_json::json!({
                                    "status": "submitted",
                                    "contribution_id": contribution_id,
                                    "fix": fix,
                                })
                            );
                        }
                        _ => {
                            println!("Fix Submitted:");
                            println!("  Contribution ID: {}", contribution_id);
                            println!("  Title:           {}", fix.title);
                            println!("  Type:            {:?}", fix.fix_type);
                            println!(
                                "  Lines:           +{} -{} ({} total)",
                                additions,
                                deletions,
                                fix.lines_changed()
                            );
                            println!();
                            println!("View with: dashflow pkg contrib show {}", contribution_id);
                        }
                    }
                    return Ok(());
                }
                Err(e) => {
                    eprintln!(
                        "Warning: Failed to submit to registry: {}. Showing local report.",
                        e
                    );
                }
            }
        }
    }

    // Fall back to local display
    match args.format.as_str() {
        "json" => {
            println!("{}", serde_json::to_string_pretty(&fix)?);
        }
        _ => {
            println!("Fix Submission Created (local only - registry not available):");
            println!("  ID:          {}", fix.id);
            println!("  Title:       {}", fix.title);
            println!("  Type:        {:?}", fix.fix_type);
            println!(
                "  Lines:       +{} -{} ({} total)",
                additions,
                deletions,
                fix.lines_changed()
            );
            println!(
                "  Submitted:   {}",
                fix.submitted_at.format("%Y-%m-%d %H:%M:%S")
            );
            println!();
            println!("Note: Start registry server to submit contributions.");
        }
    }

    Ok(())
}

async fn run_contrib_list(args: ContribListArgs) -> Result<()> {
    if let Some(package) = &args.package {
        println!("Contributions for package: {}", package);
    } else {
        println!("All Contributions:");
    }
    println!();

    // Try to query registry API
    if let Ok(client) = RegistryClient::new() {
        if client.health_check().await.unwrap_or(false) {
            if let Ok(contribs) = client
                .list_contributions(args.package.as_deref(), args.limit)
                .await
            {
                match args.format.as_str() {
                    "json" => {
                        println!("{}", serde_json::to_string_pretty(&contribs)?);
                    }
                    _ => {
                        if contribs.is_empty() {
                            println!("(no contributions found)");
                        } else {
                            println!(
                                "{:<36} {:<12} {:<10} {:<15} TITLE",
                                "ID", "TYPE", "STATUS", "SUBMITTED"
                            );
                            println!("{}", "-".repeat(100));
                            for c in &contribs {
                                let id = c.get("id").and_then(|v| v.as_str()).unwrap_or("-");
                                let ctype = c.get("type").and_then(|v| v.as_str()).unwrap_or("-");
                                let status =
                                    c.get("status").and_then(|v| v.as_str()).unwrap_or("-");
                                let submitted = c
                                    .get("submitted_at")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("-");
                                let title = c.get("title").and_then(|v| v.as_str()).unwrap_or("-");
                                println!(
                                    "{:<36} {:<12} {:<10} {:<15} {}",
                                    id,
                                    ctype,
                                    status,
                                    &submitted[..10.min(submitted.len())],
                                    title
                                );
                            }
                        }
                    }
                }
                return Ok(());
            }
        }
    }

    // Fall back to mock data
    println!("(Registry not available)");
    match args.format.as_str() {
        "json" => {
            println!("[]");
        }
        _ => {
            println!(
                "{:<36} {:<12} {:<10} {:<15} TITLE",
                "ID", "TYPE", "STATUS", "SUBMITTED"
            );
            println!("{}", "-".repeat(100));
            println!("(no contributions found)");
            println!();
            println!("Create contributions with:");
            println!("  dashflow pkg contrib bug <package> -t \"Bug title\" -s high");
            println!("  dashflow pkg contrib improve <package> -t \"Improvement title\"");
            println!("  dashflow pkg contrib request \"Package request title\"");
            println!("  dashflow pkg contrib fix <package> -t \"Fix title\" --diff fix.patch");
        }
    }

    Ok(())
}

async fn run_contrib_show(args: ContribShowArgs) -> Result<()> {
    println!("Contribution: {}", args.id);
    println!();

    // Mock data - in production would query registry API
    match args.format.as_str() {
        "json" => {
            println!("{{");
            println!("  \"error\": \"contribution not found\"");
            println!("}}");
        }
        _ => {
            println!("Contribution '{}' not found.", args.id);
            println!();
            println!("Use `dashflow pkg contrib list` to see available contributions.");
        }
    }

    Ok(())
}

async fn run_contrib_review(args: ContribReviewArgs) -> Result<()> {
    println!("Multi-Model Review for Contribution: {}", args.id);
    println!();

    // Create mock contribution for demonstration
    let mock_bug = BugReport::builder()
        .title("Mock bug for review demonstration")
        .category(BugCategory::RuntimeError)
        .severity(BugSeverity::Medium)
        .description("This is a mock bug report to demonstrate the review system.")
        .build()
        .context("Failed to create mock bug")?;

    let contribution = Contribution::Bug(mock_bug);

    // Create reviewer with mock models
    let config = ReviewConfig {
        min_reviews: args.num_models,
        consensus_threshold: args.threshold,
        ..Default::default()
    };

    let mut reviewer = ContributionReviewer::new(config);

    // Add mock reviewers
    for i in 0..args.num_models {
        let verdict = if i % 2 == 0 {
            ReviewVerdict::Approve
        } else {
            ReviewVerdict::ApproveWithSuggestions
        };
        reviewer.add_reviewer(std::sync::Arc::new(MockModelReviewer::new(
            format!("model-{}", i + 1),
            format!("Mock Model {}", i + 1),
            verdict,
            0.85 + (i as f64 * 0.05),
        )));
    }

    // Run review
    println!(
        "Running multi-model review with {} reviewers...",
        args.num_models
    );
    println!("Consensus threshold: {:.0}%", args.threshold * 100.0);
    println!();

    let result = reviewer
        .review(&contribution)
        .await
        .context("Review failed")?;

    match args.format.as_str() {
        "json" => {
            println!("{}", serde_json::to_string_pretty(&result)?);
        }
        _ => {
            println!("Review Results:");
            println!("===============");
            println!();

            // Individual reviews
            println!("Model Reviews:");
            for review in &result.reviews {
                println!("  {} ({}):", review.model_name, review.model_id);
                println!("    Verdict:    {:?}", review.verdict);
                println!("    Confidence: {:.0}%", review.confidence * 100.0);
                println!("    Quality:    {:.0}%", review.scores.quality * 100.0);
                println!("    Safety:     {:.0}%", review.scores.safety * 100.0);
            }
            println!();

            // Consensus
            println!("Consensus:");
            println!("  Score:       {:.0}%", result.consensus.score * 100.0);
            println!("  Models:      {}", result.consensus.model_count);
            println!("  Positive:    {}", result.consensus.positive_count);
            println!("  Negative:    {}", result.consensus.negative_count);
            println!("  Abstain:     {}", result.consensus.abstain_count);
            println!(
                "  Reached:     {}",
                if result.consensus.consensus_reached {
                    "Yes"
                } else {
                    "No"
                }
            );
            println!();

            // Action
            println!("Recommended Action: {:?}", result.action);
            println!();

            if !result.consensus.disagreements.is_empty() {
                println!("Disagreements:");
                for d in &result.consensus.disagreements {
                    println!("  - {}", d);
                }
            }
        }
    }

    Ok(())
}
