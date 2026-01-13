// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

//! Health check commands for DashFlow introspection.
//!
//! Provides runtime health checks for the deployed DashFlow instance,
//! including graph engine, checkpointing, module discovery, LLM connectivity,
//! Kafka, and infrastructure services (Grafana, Prometheus, Docker).

use anyhow::Result;
use colored::Colorize;
use dashflow::constants::DEFAULT_HTTP_CONNECT_TIMEOUT;
use dashflow::core::config_loader::env_vars::{
    env_is_set, env_string_or_default, ANTHROPIC_API_KEY, AWS_ACCESS_KEY_ID, AWS_DEFAULT_REGION,
    GRAFANA_PASS, GRAFANA_URL, GRAFANA_USER, KAFKA_BOOTSTRAP_SERVERS, OPENAI_API_KEY,
};

use super::{discover_all_modules_in_workspace, get_workspace_root, HealthArgs, OutputFormat};

/// Health check result for JSON output
#[derive(serde::Serialize)]
pub(super) struct HealthCheckResult {
    pub name: String,
    pub status: String,
    pub message: Option<String>,
}

/// Documentation coverage statistics per crate
#[derive(Debug, Clone, serde::Serialize)]
struct CrateDocStats {
    name: String,
    total: usize,
    documented: usize,
    coverage: f64,
}

/// Run runtime health checks
pub(super) async fn run_health(args: HealthArgs) -> Result<()> {
    let mut results: Vec<HealthCheckResult> = Vec::new();
    let mut passed = 0;
    let mut failed = 0;
    let mut skipped = 0;

    if !matches!(args.format, OutputFormat::Json) {
        println!();
        println!("{}", "DashFlow Health Check".bright_cyan().bold());
        println!("{}", "═".repeat(60).bright_cyan());
        println!();
    }

    // Check 1: Graph Engine (no external deps)
    let name = "Graph Engine";
    if !matches!(args.format, OutputFormat::Json) {
        print!("  {}............ ", name);
    }
    match check_graph_engine().await {
        Ok(_) => {
            if !matches!(args.format, OutputFormat::Json) {
                println!("{}", "OK".bright_green());
            }
            results.push(HealthCheckResult {
                name: name.to_string(),
                status: "ok".to_string(),
                message: None,
            });
            passed += 1;
        }
        Err(e) => {
            if !matches!(args.format, OutputFormat::Json) {
                println!("{}: {}", "FAIL".bright_red(), e);
            }
            results.push(HealthCheckResult {
                name: name.to_string(),
                status: "fail".to_string(),
                message: Some(e.to_string()),
            });
            failed += 1;
        }
    }

    // Check 2: File Checkpointing
    let name = "File Checkpointing";
    if !matches!(args.format, OutputFormat::Json) {
        print!("  {}..... ", name);
    }
    match check_file_checkpointing().await {
        Ok(_) => {
            if !matches!(args.format, OutputFormat::Json) {
                println!("{}", "OK".bright_green());
            }
            results.push(HealthCheckResult {
                name: name.to_string(),
                status: "ok".to_string(),
                message: None,
            });
            passed += 1;
        }
        Err(e) => {
            if !matches!(args.format, OutputFormat::Json) {
                println!("{}: {}", "FAIL".bright_red(), e);
            }
            results.push(HealthCheckResult {
                name: name.to_string(),
                status: "fail".to_string(),
                message: Some(e.to_string()),
            });
            failed += 1;
        }
    }

    // Check 3: Module Discovery
    let name = "Module Discovery";
    if !matches!(args.format, OutputFormat::Json) {
        print!("  {}...... ", name);
    }
    match check_module_discovery() {
        Ok(count) => {
            if !matches!(args.format, OutputFormat::Json) {
                println!("{} ({} modules)", "OK".bright_green(), count);
            }
            results.push(HealthCheckResult {
                name: name.to_string(),
                status: "ok".to_string(),
                message: Some(format!("{} modules discovered", count)),
            });
            passed += 1;
        }
        Err(e) => {
            if !matches!(args.format, OutputFormat::Json) {
                println!("{}: {}", "FAIL".bright_red(), e);
            }
            results.push(HealthCheckResult {
                name: name.to_string(),
                status: "fail".to_string(),
                message: Some(e.to_string()),
            });
            failed += 1;
        }
    }

    // Check 4: LLM Connectivity (optional)
    let name = "LLM Connectivity";
    if args.skip_llm {
        if !matches!(args.format, OutputFormat::Json) {
            print!("  {}...... ", name);
            println!("{}", "SKIPPED".bright_yellow());
        }
        results.push(HealthCheckResult {
            name: name.to_string(),
            status: "skipped".to_string(),
            message: Some("--skip-llm flag set".to_string()),
        });
        skipped += 1;
    } else {
        if !matches!(args.format, OutputFormat::Json) {
            print!("  {}...... ", name);
        }
        match check_llm_connectivity().await {
            Ok(provider) => {
                if !matches!(args.format, OutputFormat::Json) {
                    println!("{} ({})", "OK".bright_green(), provider);
                }
                results.push(HealthCheckResult {
                    name: name.to_string(),
                    status: "ok".to_string(),
                    message: Some(format!("Provider: {}", provider)),
                });
                passed += 1;
            }
            Err(e) => {
                if !matches!(args.format, OutputFormat::Json) {
                    println!("{}: {}", "WARN".bright_yellow(), e);
                }
                results.push(HealthCheckResult {
                    name: name.to_string(),
                    status: "warn".to_string(),
                    message: Some(e.to_string()),
                });
                // LLM is optional, don't count as failure
                skipped += 1;
            }
        }
    }

    // Check 5: Kafka Connectivity (optional)
    let name = "Kafka/Streaming";
    if args.skip_kafka {
        if !matches!(args.format, OutputFormat::Json) {
            print!("  {}....... ", name);
            println!("{}", "SKIPPED".bright_yellow());
        }
        results.push(HealthCheckResult {
            name: name.to_string(),
            status: "skipped".to_string(),
            message: Some("--skip-kafka flag set".to_string()),
        });
        skipped += 1;
    } else {
        if !matches!(args.format, OutputFormat::Json) {
            print!("  {}....... ", name);
        }
        // Kafka is optional, just check if env var is set
        if env_is_set(KAFKA_BOOTSTRAP_SERVERS) {
            if !matches!(args.format, OutputFormat::Json) {
                println!("{}", "OK (configured)".bright_green());
            }
            results.push(HealthCheckResult {
                name: name.to_string(),
                status: "ok".to_string(),
                message: Some(format!("{} set", KAFKA_BOOTSTRAP_SERVERS)),
            });
            passed += 1;
        } else {
            if !matches!(args.format, OutputFormat::Json) {
                println!("{}", "SKIPPED (not configured)".bright_yellow());
            }
            results.push(HealthCheckResult {
                name: name.to_string(),
                status: "skipped".to_string(),
                message: Some(format!("{} not set", KAFKA_BOOTSTRAP_SERVERS)),
            });
            skipped += 1;
        }
    }

    // Infrastructure checks section header
    let skip_all_infra = args.skip_infra;
    if !matches!(args.format, OutputFormat::Json) && !skip_all_infra {
        println!();
        println!("  {}", "Infrastructure:".bright_cyan().dimmed());
    }

    // Check 6: Grafana (infrastructure)
    let name = "Grafana";
    if skip_all_infra || args.skip_grafana {
        if !matches!(args.format, OutputFormat::Json) {
            print!("  {}............. ", name);
            println!("{}", "SKIPPED".bright_yellow());
        }
        results.push(HealthCheckResult {
            name: name.to_string(),
            status: "skipped".to_string(),
            message: Some(if skip_all_infra {
                "--skip-infra flag set".to_string()
            } else {
                "--skip-grafana flag set".to_string()
            }),
        });
        skipped += 1;
    } else {
        if !matches!(args.format, OutputFormat::Json) {
            print!("  {}............. ", name);
        }
        match check_grafana(args.timeout).await {
            Ok(msg) => {
                if !matches!(args.format, OutputFormat::Json) {
                    println!("{} ({})", "OK".bright_green(), msg);
                }
                results.push(HealthCheckResult {
                    name: name.to_string(),
                    status: "ok".to_string(),
                    message: Some(msg),
                });
                passed += 1;
            }
            Err(e) => {
                if !matches!(args.format, OutputFormat::Json) {
                    println!("{}: {}", "FAIL".bright_red(), e);
                }
                results.push(HealthCheckResult {
                    name: name.to_string(),
                    status: "fail".to_string(),
                    message: Some(e.to_string()),
                });
                failed += 1;
            }
        }
    }

    // Check 7: Prometheus (infrastructure)
    let name = "Prometheus";
    if skip_all_infra || args.skip_prometheus {
        if !matches!(args.format, OutputFormat::Json) {
            print!("  {}.......... ", name);
            println!("{}", "SKIPPED".bright_yellow());
        }
        results.push(HealthCheckResult {
            name: name.to_string(),
            status: "skipped".to_string(),
            message: Some(if skip_all_infra {
                "--skip-infra flag set".to_string()
            } else {
                "--skip-prometheus flag set".to_string()
            }),
        });
        skipped += 1;
    } else {
        if !matches!(args.format, OutputFormat::Json) {
            print!("  {}.......... ", name);
        }
        match check_prometheus(args.timeout).await {
            Ok(msg) => {
                if !matches!(args.format, OutputFormat::Json) {
                    println!("{} ({})", "OK".bright_green(), msg);
                }
                results.push(HealthCheckResult {
                    name: name.to_string(),
                    status: "ok".to_string(),
                    message: Some(msg),
                });
                passed += 1;
            }
            Err(e) => {
                if !matches!(args.format, OutputFormat::Json) {
                    println!("{}: {}", "FAIL".bright_red(), e);
                }
                results.push(HealthCheckResult {
                    name: name.to_string(),
                    status: "fail".to_string(),
                    message: Some(e.to_string()),
                });
                failed += 1;
            }
        }
    }

    // Check 8: Docker services (infrastructure)
    let name = "Docker Services";
    if skip_all_infra || args.skip_docker {
        if !matches!(args.format, OutputFormat::Json) {
            print!("  {}.... ", name);
            println!("{}", "SKIPPED".bright_yellow());
        }
        results.push(HealthCheckResult {
            name: name.to_string(),
            status: "skipped".to_string(),
            message: Some(if skip_all_infra {
                "--skip-infra flag set".to_string()
            } else {
                "--skip-docker flag set".to_string()
            }),
        });
        skipped += 1;
    } else {
        if !matches!(args.format, OutputFormat::Json) {
            print!("  {}.... ", name);
        }
        match check_docker_services().await {
            Ok(msg) => {
                if !matches!(args.format, OutputFormat::Json) {
                    println!("{} ({})", "OK".bright_green(), msg);
                }
                results.push(HealthCheckResult {
                    name: name.to_string(),
                    status: "ok".to_string(),
                    message: Some(msg),
                });
                passed += 1;
            }
            Err(e) => {
                if !matches!(args.format, OutputFormat::Json) {
                    println!("{}: {}", "FAIL".bright_red(), e);
                }
                results.push(HealthCheckResult {
                    name: name.to_string(),
                    status: "fail".to_string(),
                    message: Some(e.to_string()),
                });
                failed += 1;
            }
        }
    }

    // Check 9: Documentation Coverage
    let name = "Documentation Coverage";
    if args.skip_docs {
        if !matches!(args.format, OutputFormat::Json) {
            print!("  {}. ", name);
            println!("{}", "SKIPPED".bright_yellow());
        }
        results.push(HealthCheckResult {
            name: name.to_string(),
            status: "skipped".to_string(),
            message: Some("--skip-docs flag set".to_string()),
        });
        skipped += 1;
    } else {
        if !matches!(args.format, OutputFormat::Json) {
            print!("  {}. ", name);
        }
        match check_doc_coverage().await {
            Ok((coverage, total, undoc, worst_crates)) => {
                let status_str = if coverage >= 90.0 {
                    "OK".bright_green()
                } else if coverage >= 70.0 {
                    "WARN".bright_yellow()
                } else {
                    "LOW".bright_red()
                };
                if !matches!(args.format, OutputFormat::Json) {
                    println!(
                        "{} ({:.0}% - {} items, {} undocumented)",
                        status_str, coverage, total, undoc
                    );

                    // Show worst crates when coverage is below 90%
                    if coverage < 90.0 && !worst_crates.is_empty() {
                        println!(
                            "    {} {}",
                            "Lowest coverage crates:".dimmed(),
                            "(10+ pub items)".dimmed()
                        );
                        for crate_stat in &worst_crates {
                            println!(
                                "      • {} {:.0}% ({}/{} documented)",
                                crate_stat.name.bright_white(),
                                crate_stat.coverage,
                                crate_stat.documented,
                                crate_stat.total
                            );
                        }
                    }
                }
                results.push(HealthCheckResult {
                    name: name.to_string(),
                    status: if coverage >= 70.0 { "ok" } else { "warn" }.to_string(),
                    message: Some(format!(
                        "{:.1}% coverage ({} items, {} undocumented)",
                        coverage, total, undoc
                    )),
                });
                if coverage >= 70.0 {
                    passed += 1;
                } else {
                    // Low doc coverage is a warning, not a failure
                    skipped += 1;
                }
            }
            Err(e) => {
                if !matches!(args.format, OutputFormat::Json) {
                    println!("{}: {}", "WARN".bright_yellow(), e);
                }
                results.push(HealthCheckResult {
                    name: name.to_string(),
                    status: "warn".to_string(),
                    message: Some(e.to_string()),
                });
                // Doc check failure is a warning, not failure
                skipped += 1;
            }
        }
    }

    // Output results
    if matches!(args.format, OutputFormat::Json) {
        #[derive(serde::Serialize)]
        struct HealthOutput {
            passed: usize,
            failed: usize,
            skipped: usize,
            checks: Vec<HealthCheckResult>,
        }
        let output = HealthOutput {
            passed,
            failed,
            skipped,
            checks: results,
        };
        println!("{}", serde_json::to_string_pretty(&output)?);
    } else {
        println!();
        println!("{}", "─".repeat(60).bright_cyan());
        println!(
            "  {} {} | {} {} | {} {}",
            "Passed:".bright_green(),
            passed,
            "Failed:".bright_red(),
            failed,
            "Skipped:".bright_yellow(),
            skipped
        );

        if failed > 0 {
            println!();
            println!(
                "{}",
                "Some health checks failed. Check the errors above.".bright_red()
            );
        }
    }

    if failed > 0 {
        std::process::exit(1);
    }
    Ok(())
}

/// Check graph engine works (no external deps)
async fn check_graph_engine() -> Result<()> {
    use dashflow::{MergeableState, StateGraph};
    use serde::{Deserialize, Serialize};

    #[derive(Clone, Debug, Serialize, Deserialize, Default)]
    struct TestState {
        value: i32,
    }

    impl MergeableState for TestState {
        fn merge(&mut self, other: &Self) {
            self.value = self.value.max(other.value);
        }
    }

    let mut graph: StateGraph<TestState> = StateGraph::new();
    graph.add_node_from_fn("test", |mut state| {
        Box::pin(async move {
            state.value += 1;
            Ok(state)
        })
    });
    graph.add_edge("test", "__end__");
    graph.set_entry_point("test");

    let app = graph.compile()?;
    let result = app.invoke(TestState::default()).await?;

    if result.state().value != 1 {
        anyhow::bail!("Graph execution produced unexpected result");
    }

    Ok(())
}

/// Check file checkpointing works
async fn check_file_checkpointing() -> Result<()> {
    use dashflow::FileCheckpointer;
    use serde::{Deserialize, Serialize};

    #[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
    struct TestData {
        value: i32,
    }

    let temp_dir = std::env::temp_dir().join("dashflow_health_check");

    // Use tokio::fs for async-friendly file operations
    if tokio::fs::try_exists(&temp_dir).await.unwrap_or(false) {
        tokio::fs::remove_dir_all(&temp_dir).await?;
    }

    // Wrap blocking FileCheckpointer::new in spawn_blocking
    let temp_dir_clone = temp_dir.clone();
    tokio::task::spawn_blocking(move || -> Result<()> {
        let checkpointer: FileCheckpointer<TestData> = FileCheckpointer::new(&temp_dir_clone)?;
        // Just verify it can be created - actual checkpointing requires graph execution
        drop(checkpointer);
        Ok(())
    })
    .await
    .map_err(|e| anyhow::anyhow!("Checkpointer task panicked: {e}"))??;

    // Cleanup using async fs
    if tokio::fs::try_exists(&temp_dir).await.unwrap_or(false) {
        tokio::fs::remove_dir_all(&temp_dir).await?;
    }

    Ok(())
}

/// Check module discovery works
fn check_module_discovery() -> Result<usize> {
    let modules = discover_all_modules_in_workspace(None);

    // With full workspace discovery across all 108 crates, we expect many more modules
    if modules.len() < 100 {
        anyhow::bail!("Only {} modules found (expected 100+)", modules.len());
    }

    Ok(modules.len())
}

/// Check LLM connectivity
async fn check_llm_connectivity() -> Result<String> {
    // Check for available providers
    if env_is_set(OPENAI_API_KEY) {
        return Ok("OpenAI (configured)".to_string());
    }
    if env_is_set(ANTHROPIC_API_KEY) {
        return Ok("Anthropic (configured)".to_string());
    }
    if env_is_set(AWS_ACCESS_KEY_ID) || env_is_set(AWS_DEFAULT_REGION) {
        return Ok("AWS Bedrock (configured)".to_string());
    }

    anyhow::bail!(
        "No LLM credentials found ({}, {}, or AWS credentials)",
        OPENAI_API_KEY,
        ANTHROPIC_API_KEY
    )
}

/// Check Grafana health (infrastructure check)
/// Respects environment variables:
/// - GRAFANA_URL (default: http://localhost:3000)
/// - GRAFANA_USER (default: admin)
/// - GRAFANA_PASS (default: admin)
async fn check_grafana(timeout_secs: u64) -> Result<String> {
    let grafana_url = env_string_or_default(GRAFANA_URL, "http://localhost:3000");
    let grafana_user = env_string_or_default(GRAFANA_USER, "admin");
    let grafana_pass = env_string_or_default(GRAFANA_PASS, "admin");

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(timeout_secs))
        .connect_timeout(DEFAULT_HTTP_CONNECT_TIMEOUT)
        .build()?;

    // Check basic health
    let health_resp = client
        .get(format!("{}/api/health", grafana_url))
        .send()
        .await
        .map_err(|e| anyhow::anyhow!("Grafana unreachable at {}: {}", grafana_url, e))?;

    if !health_resp.status().is_success() {
        anyhow::bail!("Grafana returned status {}", health_resp.status());
    }

    // Check if dashstream dashboard exists
    let dashboard_resp = client
        .get(format!(
            "{}/api/dashboards/uid/dashstream-quality",
            grafana_url
        ))
        .basic_auth(&grafana_user, Some(&grafana_pass))
        .send()
        .await;

    match dashboard_resp {
        Ok(resp) if resp.status().is_success() => {
            // Parse dashboard to count panels (DATA verification)
            if let Ok(body) = resp.text().await {
                if let Ok(json) = serde_json::from_str::<serde_json::Value>(&body) {
                    let panel_count = json
                        .get("dashboard")
                        .and_then(|d| d.get("panels"))
                        .and_then(|p| p.as_array())
                        .map(|a| a.len())
                        .unwrap_or(0);

                    if panel_count > 0 {
                        return Ok(format!("healthy + dashboard ({} panels)", panel_count));
                    } else {
                        return Ok("healthy + dashboard (no panels)".to_string());
                    }
                }
            }
            Ok("healthy + dashboard".to_string())
        }
        _ => Ok("healthy (dashboard not found)".to_string()),
    }
}

/// Check Prometheus health (infrastructure check)
async fn check_prometheus(timeout_secs: u64) -> Result<String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(timeout_secs))
        .connect_timeout(DEFAULT_HTTP_CONNECT_TIMEOUT)
        .build()?;

    // Check basic health
    let health_resp = client
        .get("http://localhost:9090/-/healthy")
        .send()
        .await
        .map_err(|e| anyhow::anyhow!("Prometheus unreachable: {}", e))?;

    if !health_resp.status().is_success() {
        anyhow::bail!("Prometheus returned status {}", health_resp.status());
    }

    // Check if dashstream metrics exist (DATA verification)
    let metrics_resp = client
        .get("http://localhost:9090/api/v1/query")
        .query(&[("query", "{__name__=~\"dashstream_.*\"}")])
        .send()
        .await;

    match metrics_resp {
        Ok(resp) if resp.status().is_success() => {
            // Parse response to check result count
            if let Ok(body) = resp.text().await {
                if let Ok(json) = serde_json::from_str::<serde_json::Value>(&body) {
                    let result_count = json
                        .get("data")
                        .and_then(|d| d.get("result"))
                        .and_then(|r| r.as_array())
                        .map(|a| a.len())
                        .unwrap_or(0);

                    if result_count > 0 {
                        return Ok(format!("healthy + {} dashstream metrics", result_count));
                    } else {
                        return Ok("healthy (no dashstream metrics yet)".to_string());
                    }
                }
            }
            Ok("healthy + metrics available".to_string())
        }
        _ => Ok("healthy (metrics query failed)".to_string()),
    }
}

/// Check Docker services (infrastructure check)
fn summarize_docker_compose_ps(stdout: &str) -> Result<String> {
    // Parse JSON lines (docker compose outputs one JSON per line)
    let mut running = 0;
    let mut total = 0;

    for line in stdout.lines() {
        if line.trim().is_empty() {
            continue;
        }
        total += 1;
        // Check if state is "running"
        if line.contains("\"running\"") || line.contains("\"State\":\"running\"") {
            running += 1;
        }
    }

    if total == 0 {
        anyhow::bail!("No Docker services found (docker compose ps returned empty)");
    }

    if running == total {
        Ok(format!("{}/{} services running", running, total))
    } else {
        anyhow::bail!("{}/{} services running (some stopped)", running, total)
    }
}

/// Check Docker services (infrastructure check)
async fn check_docker_services() -> Result<String> {
    use std::process::Command;

    // Try docker compose ps first (newer syntax)
    let output = Command::new("docker")
        .args(["compose", "ps", "--format", "json"])
        .output();

    let output = match output {
        Ok(o) if o.status.success() => o,
        _ => {
            // Try docker-compose (older syntax)
            Command::new("docker-compose")
                .args(["ps", "--format", "json"])
                .output()
                .map_err(|e| anyhow::anyhow!("docker-compose not available: {}", e))?
        }
    };

    if !output.status.success() {
        anyhow::bail!(
            "docker compose ps failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    summarize_docker_compose_ps(&stdout)
}

/// Check documentation coverage by scanning source files
///
/// Returns (coverage_percentage, total_items, undocumented_items, worst_crates)
/// Wrapped in spawn_blocking to avoid blocking the async runtime
async fn check_doc_coverage() -> Result<(f64, usize, usize, Vec<CrateDocStats>)> {
    tokio::task::spawn_blocking(check_doc_coverage_sync)
        .await
        .map_err(|e| anyhow::anyhow!("Doc coverage task panicked: {e}"))?
}

/// Synchronous implementation of doc coverage check
fn check_doc_coverage_sync() -> Result<(f64, usize, usize, Vec<CrateDocStats>)> {
    use std::collections::HashMap;
    use std::fs;
    use walkdir::WalkDir;

    let workspace_root = get_workspace_root();
    let crates_dir = workspace_root.join("crates");

    if !crates_dir.exists() {
        anyhow::bail!("crates/ directory not found");
    }

    let mut total_pub_items = 0usize;
    let mut documented_items = 0usize;

    // Track per-crate statistics
    let mut crate_stats: HashMap<String, (usize, usize)> = HashMap::new();

    // Pattern to match pub items: pub fn, pub struct, pub enum, pub trait, pub type, pub const
    let pub_pattern = regex::Regex::new(
        r"(?m)^[ \t]*(///[^\n]*\n(?:[ \t]*///[^\n]*\n)*)?[ \t]*pub(?:\([^)]*\))?\s+(?:async\s+)?(?:unsafe\s+)?(?:fn|struct|enum|trait|type|const|static|mod)\s+\w+"
    ).expect("valid regex");

    let doc_pattern = regex::Regex::new(r"^[ \t]*///").expect("valid regex");

    for entry in WalkDir::new(&crates_dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().is_some_and(|ext| ext == "rs"))
    {
        let content = match fs::read_to_string(entry.path()) {
            Ok(c) => c,
            Err(_) => continue,
        };

        // Extract crate name from path (crates/<crate-name>/...)
        let crate_name = entry
            .path()
            .strip_prefix(&crates_dir)
            .ok()
            .and_then(|p| p.components().next())
            .and_then(|c| c.as_os_str().to_str())
            .map(String::from)
            .unwrap_or_else(|| "unknown".to_string());

        for cap in pub_pattern.captures_iter(&content) {
            total_pub_items += 1;

            let entry = crate_stats.entry(crate_name.clone()).or_insert((0, 0));
            entry.0 += 1; // total

            // Check if there's a doc comment (group 1)
            if let Some(doc_match) = cap.get(1) {
                let doc_text = doc_match.as_str();
                // Verify it's actually a doc comment (not just whitespace)
                if doc_pattern.is_match(doc_text) {
                    documented_items += 1;
                    entry.1 += 1; // documented
                }
            }
        }
    }

    if total_pub_items == 0 {
        return Ok((100.0, 0, 0, vec![]));
    }

    // Build sorted list of crates by coverage (worst first)
    let mut crate_list: Vec<CrateDocStats> = crate_stats
        .into_iter()
        .filter(|(_, (total, _))| *total >= 10) // Only crates with 10+ pub items
        .map(|(name, (total, documented))| {
            let coverage = if total > 0 {
                (documented as f64 / total as f64) * 100.0
            } else {
                100.0
            };
            CrateDocStats {
                name,
                total,
                documented,
                coverage,
            }
        })
        .collect();

    // Sort by coverage ascending (worst first)
    crate_list.sort_by(|a, b| {
        a.coverage
            .partial_cmp(&b.coverage)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    // Keep only the worst 5
    crate_list.truncate(5);

    let undocumented = total_pub_items - documented_items;
    let coverage = (documented_items as f64 / total_pub_items as f64) * 100.0;

    Ok((coverage, total_pub_items, undocumented, crate_list))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn summarize_docker_compose_ps_counts_running_services() {
        let stdout = r#"
{"Name":"svc-a","State":"running"}
{"Name":"svc-b","State":"running"}
"#;
        assert_eq!(
            summarize_docker_compose_ps(stdout).expect("summarize"),
            "2/2 services running"
        );
    }

    #[test]
    fn summarize_docker_compose_ps_errors_when_some_services_stopped() {
        let stdout = r#"
{"Name":"svc-a","State":"running"}
{"Name":"svc-b","State":"exited"}
"#;
        let err = summarize_docker_compose_ps(stdout).expect_err("should fail");
        assert!(err.to_string().contains("1/2 services running"));
    }

    #[test]
    fn summarize_docker_compose_ps_errors_on_empty_output() {
        let err = summarize_docker_compose_ps("").expect_err("should fail");
        assert!(err
            .to_string()
            .contains("docker compose ps returned empty"));
    }
}
