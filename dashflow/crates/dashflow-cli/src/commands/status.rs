// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

//! Infrastructure status and health check command.
//!
//! Provides holistic introspection into DashFlow's own infrastructure:
//! - Docker daemon status
//! - Container health (Kafka, Grafana, Prometheus, Jaeger)
//! - Service port availability (WebSocket server, UI dev server)
//! - Auto-recovery suggestions

use crate::output::{create_table, print_error, print_info, print_success, print_warning};
use anyhow::Result;
use clap::Args;
use colored::Colorize;
use std::net::TcpStream;
use std::process::Command;
use std::time::Duration;

/// Check DashFlow infrastructure health
#[derive(Args)]
pub struct StatusArgs {
    /// Show detailed output including container logs
    #[arg(short, long)]
    verbose: bool,

    /// Attempt to auto-recover downed services
    #[arg(long)]
    recover: bool,

    /// Check only specific service (docker, kafka, websocket, ui, grafana, prometheus, jaeger)
    #[arg(short, long)]
    service: Option<String>,

    /// Output format (table, json)
    #[arg(short, long, default_value = "table")]
    format: String,

    /// Show active Prometheus alerts with AI-readable explanations
    #[arg(short, long)]
    alerts: bool,
}

/// Service health status
#[derive(Debug, Clone)]
pub struct ServiceStatus {
    pub name: String,
    pub status: HealthStatus,
    pub port: Option<u16>,
    pub details: String,
    pub recovery_hint: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum HealthStatus {
    Healthy,
    Degraded,
    Down,
}

impl HealthStatus {
    fn icon(&self) -> &'static str {
        match self {
            HealthStatus::Healthy => "✓",
            HealthStatus::Degraded => "⚠",
            HealthStatus::Down => "✗",
        }
    }

    fn colored_icon(&self) -> String {
        match self {
            HealthStatus::Healthy => self.icon().bright_green().bold().to_string(),
            HealthStatus::Degraded => self.icon().bright_yellow().bold().to_string(),
            HealthStatus::Down => self.icon().bright_red().bold().to_string(),
        }
    }
}

/// All DashFlow infrastructure services
struct InfrastructureServices {
    docker: ServiceStatus,
    kafka: ServiceStatus,
    websocket: ServiceStatus,
    ui: ServiceStatus,
    grafana: ServiceStatus,
    prometheus: ServiceStatus,
    jaeger: ServiceStatus,
}

impl InfrastructureServices {
    fn all(&self) -> Vec<&ServiceStatus> {
        vec![
            &self.docker,
            &self.kafka,
            &self.websocket,
            &self.ui,
            &self.grafana,
            &self.prometheus,
            &self.jaeger,
        ]
    }

    fn healthy_count(&self) -> usize {
        self.all()
            .iter()
            .filter(|s| s.status == HealthStatus::Healthy)
            .count()
    }

    fn down_count(&self) -> usize {
        self.all()
            .iter()
            .filter(|s| s.status == HealthStatus::Down)
            .count()
    }
}

pub async fn run(args: StatusArgs) -> Result<()> {
    if let Some(service) = &args.service {
        // Check single service
        let status = check_single_service(service)?;
        print_service_status(&status);
        if args.recover && status.status == HealthStatus::Down {
            attempt_recovery(&status)?;
        }
        // M-616: Exit with error code when service is not healthy
        match status.status {
            HealthStatus::Down => anyhow::bail!("Service '{}' is down", status.name),
            HealthStatus::Degraded => anyhow::bail!("Service '{}' is degraded", status.name),
            HealthStatus::Healthy => {}
        }
        return Ok(());
    }

    // Check all services
    let services = check_all_services().await?;

    if args.format == "json" {
        print_json_status(&services)?;
    } else {
        print_table_status(&services, args.verbose);
    }

    // Summary
    let total = services.all().len();
    let healthy = services.healthy_count();
    let down = services.down_count();

    // Check alerts if requested
    if args.alerts {
        println!();
        check_and_display_alerts(args.verbose).await;
    }

    println!();
    if healthy == total {
        print_success(&format!("All {total} services healthy"));
    } else if down > 0 {
        print_error(&format!("{down}/{total} services down, {healthy} healthy"));

        // Show recovery hints
        println!();
        println!("{}", "Recovery suggestions:".bright_yellow().bold());
        for service in services.all() {
            if service.status == HealthStatus::Down {
                if let Some(hint) = &service.recovery_hint {
                    println!("  {} {}: {}", "→".bright_cyan(), service.name.white(), hint);
                }
            }
        }

        if args.recover {
            println!();
            print_info("Attempting auto-recovery...");
            attempt_full_recovery(&services)?;
        }

        // M-616: Exit with error code when services are down (after displaying recovery info)
        anyhow::bail!("{down} service(s) down");
    } else {
        // M-616: Some services are degraded (not healthy but not down)
        let degraded = total - healthy;
        print_warning(&format!(
            "{degraded} service(s) degraded, {healthy} healthy"
        ));
        anyhow::bail!("{degraded} service(s) degraded");
    }

    #[allow(unreachable_code)]
    Ok(())
}

async fn check_all_services() -> Result<InfrastructureServices> {
    let docker = check_docker();
    let docker_running = docker.status == HealthStatus::Healthy;

    Ok(InfrastructureServices {
        docker,
        kafka: if docker_running {
            check_container_and_port("kafka", 9092)
        } else {
            ServiceStatus {
                name: "Kafka".to_string(),
                status: HealthStatus::Down,
                port: Some(9092),
                details: "Docker not running".to_string(),
                recovery_hint: Some("Start Docker first".to_string()),
            }
        },
        websocket: check_port_service(
            "WebSocket Server",
            3002,
            "cargo run --release -p dashflow-observability --features websocket-server --bin websocket_server",
        ),
        ui: check_port_service(
            "Observability UI",
            5173,
            "cd observability-ui && npm run dev",
        ),
        grafana: if docker_running {
            check_container_and_port("grafana", 3000)
        } else {
            ServiceStatus {
                name: "Grafana".to_string(),
                status: HealthStatus::Down,
                port: Some(3000),
                details: "Docker not running".to_string(),
                recovery_hint: Some("Start Docker first".to_string()),
            }
        },
        prometheus: if docker_running {
            check_container_and_port("prometheus", 9090)
        } else {
            ServiceStatus {
                name: "Prometheus".to_string(),
                status: HealthStatus::Down,
                port: Some(9090),
                details: "Docker not running".to_string(),
                recovery_hint: Some("Start Docker first".to_string()),
            }
        },
        jaeger: if docker_running {
            check_container_and_port("jaeger", 16686)
        } else {
            ServiceStatus {
                name: "Jaeger".to_string(),
                status: HealthStatus::Down,
                port: Some(16686),
                details: "Docker not running".to_string(),
                recovery_hint: Some("Start Docker first".to_string()),
            }
        },
    })
}

/// Valid service names for the --service flag
const VALID_SERVICES: &[&str] = &[
    "docker",
    "kafka",
    "websocket",
    "ui",
    "grafana",
    "prometheus",
    "jaeger",
];

fn check_single_service(name: &str) -> Result<ServiceStatus> {
    let name_lower = name.to_lowercase();
    match name_lower.as_str() {
        "docker" => Ok(check_docker()),
        "kafka" => Ok(check_container_and_port("kafka", 9092)),
        "websocket" => Ok(check_port_service(
            "WebSocket Server",
            3002,
            "cargo run --release -p dashflow-observability --features websocket-server --bin websocket_server",
        )),
        "ui" => Ok(check_port_service(
            "Observability UI",
            5173,
            "cd observability-ui && npm run dev",
        )),
        "grafana" => Ok(check_container_and_port("grafana", 3000)),
        "prometheus" => Ok(check_container_and_port("prometheus", 9090)),
        "jaeger" => Ok(check_container_and_port("jaeger", 16686)),
        // M-507: Return error for unknown services so users notice typos in --service flag
        _ => Err(anyhow::anyhow!(
            "Unknown service: '{}'. Valid services: {}",
            name,
            VALID_SERVICES.join(", ")
        )),
    }
}

fn check_docker() -> ServiceStatus {
    // Try docker info
    let output = Command::new("docker")
        .args(["info", "--format", "{{.ServerVersion}}"])
        .output();

    match output {
        Ok(o) if o.status.success() => {
            let version = String::from_utf8_lossy(&o.stdout).trim().to_string();
            ServiceStatus {
                name: "Docker".to_string(),
                status: HealthStatus::Healthy,
                port: None,
                details: format!("v{version}"),
                recovery_hint: None,
            }
        }
        Ok(o) => {
            let stderr = String::from_utf8_lossy(&o.stderr);
            let is_socket_error = stderr.contains("Cannot connect")
                || stderr.contains("Is the docker daemon running");
            ServiceStatus {
                name: "Docker".to_string(),
                status: HealthStatus::Down,
                port: None,
                details: if is_socket_error {
                    "Daemon not running".to_string()
                } else {
                    stderr.lines().next().unwrap_or("Unknown error").to_string()
                },
                recovery_hint: Some(detect_docker_recovery_command()),
            }
        }
        Err(e) => ServiceStatus {
            name: "Docker".to_string(),
            status: HealthStatus::Down,
            port: None,
            details: format!("Command failed: {e}"),
            recovery_hint: Some("Install Docker or Docker Desktop".to_string()),
        },
    }
}

fn detect_docker_recovery_command() -> String {
    // Check for colima (common on macOS)
    if Command::new("which")
        .arg("colima")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
    {
        return "colima start".to_string();
    }
    // Check for Docker Desktop
    if std::path::Path::new("/Applications/Docker.app").exists() {
        return "open -a Docker".to_string();
    }
    // Linux default
    "sudo systemctl start docker".to_string()
}

fn check_container_and_port(container_name: &str, port: u16) -> ServiceStatus {
    let display_name = match container_name {
        "kafka" => "Kafka",
        "grafana" => "Grafana",
        "prometheus" => "Prometheus",
        "jaeger" => "Jaeger",
        _ => container_name,
    };

    // Check if port is listening (faster than docker inspect)
    let port_open = check_port(port);

    if port_open {
        // Verify it's actually the expected container
        let container_running = Command::new("docker")
            .args([
                "ps",
                "--filter",
                &format!("name={container_name}"),
                "--format",
                "{{.Status}}",
            ])
            .output()
            .map(|o| o.status.success() && !o.stdout.is_empty())
            .unwrap_or(false);

        if container_running {
            ServiceStatus {
                name: display_name.to_string(),
                status: HealthStatus::Healthy,
                port: Some(port),
                details: format!("Port {port} responding"),
                recovery_hint: None,
            }
        } else {
            // Port open but not our container - something else using it
            ServiceStatus {
                name: display_name.to_string(),
                status: HealthStatus::Degraded,
                port: Some(port),
                details: format!("Port {port} in use by another process"),
                recovery_hint: Some(format!("Check what's using port {port}: lsof -i :{port}")),
            }
        }
    } else {
        ServiceStatus {
            name: display_name.to_string(),
            status: HealthStatus::Down,
            port: Some(port),
            details: format!("Port {port} not responding"),
            recovery_hint: Some(
                "docker-compose -f docker-compose.dashstream.yml up -d".to_string(),
            ),
        }
    }
}

fn check_port_service(name: &str, port: u16, recovery_cmd: &str) -> ServiceStatus {
    if check_port(port) {
        ServiceStatus {
            name: name.to_string(),
            status: HealthStatus::Healthy,
            port: Some(port),
            details: format!("Port {port} responding"),
            recovery_hint: None,
        }
    } else {
        ServiceStatus {
            name: name.to_string(),
            status: HealthStatus::Down,
            port: Some(port),
            details: format!("Port {port} not responding"),
            recovery_hint: Some(recovery_cmd.to_string()),
        }
    }
}

fn check_port(port: u16) -> bool {
    TcpStream::connect_timeout(
        &format!("127.0.0.1:{port}")
            .parse()
            .expect("valid socket address"),
        Duration::from_millis(500),
    )
    .is_ok()
}

fn print_service_status(status: &ServiceStatus) {
    let port_str = status.port.map(|p| format!(":{p}")).unwrap_or_default();

    println!(
        "{} {}{} - {}",
        status.status.colored_icon(),
        status.name.white().bold(),
        port_str.bright_black(),
        status.details
    );
}

fn print_table_status(services: &InfrastructureServices, verbose: bool) {
    println!();
    println!("{}", "DashFlow Infrastructure Status".bright_white().bold());
    println!("{}", "═".repeat(50).bright_black());
    println!();

    let mut table = create_table();
    table.set_header(vec!["", "Service", "Port", "Status", "Details"]);

    for service in services.all() {
        let port_str = service
            .port
            .map(|p| p.to_string())
            .unwrap_or_else(|| "-".to_string());

        let status_str = match service.status {
            HealthStatus::Healthy => "Healthy".bright_green().to_string(),
            HealthStatus::Degraded => "Degraded".bright_yellow().to_string(),
            HealthStatus::Down => "Down".bright_red().to_string(),
        };

        table.add_row(vec![
            service.status.colored_icon(),
            service.name.clone(),
            port_str,
            status_str,
            if verbose {
                service.details.clone()
            } else {
                service.details.chars().take(30).collect()
            },
        ]);
    }

    println!("{table}");
}

fn print_json_status(services: &InfrastructureServices) -> Result<()> {
    let json = serde_json::json!({
        "timestamp": chrono::Utc::now().to_rfc3339(),
        "services": services.all().iter().map(|s| {
            serde_json::json!({
                "name": s.name,
                "status": format!("{:?}", s.status),
                "port": s.port,
                "details": s.details,
                "recovery_hint": s.recovery_hint,
            })
        }).collect::<Vec<_>>(),
        "summary": {
            "total": services.all().len(),
            "healthy": services.healthy_count(),
            "down": services.down_count(),
        }
    });

    println!("{}", serde_json::to_string_pretty(&json)?);
    Ok(())
}

/// Known safe recovery commands that can be auto-executed.
/// This explicit allowlist is more secure than prefix matching.
struct SafeRecoveryCommand {
    hint_match: &'static str,
    program: &'static str,
    args: &'static [&'static str],
}

/// Explicit allowlist of safe recovery commands.
/// SECURITY: Only add commands here that are safe to auto-execute.
/// Each command must be fully specified - no dynamic argument parsing.
const SAFE_RECOVERY_COMMANDS: &[SafeRecoveryCommand] = &[
    // Colima (Docker alternative for macOS)
    SafeRecoveryCommand {
        hint_match: "colima start",
        program: "colima",
        args: &["start"],
    },
    // Docker Desktop for macOS
    SafeRecoveryCommand {
        hint_match: "open -a Docker",
        program: "open",
        args: &["-a", "Docker"],
    },
];

fn attempt_recovery(service: &ServiceStatus) -> Result<()> {
    if let Some(hint) = &service.recovery_hint {
        print_info(&format!("Running: {hint}"));

        // Check against explicit allowlist of safe commands
        // SECURITY: We use exact match against known commands rather than
        // prefix matching + parsing to prevent injection vulnerabilities.
        let safe_cmd = SAFE_RECOVERY_COMMANDS
            .iter()
            .find(|cmd| hint == cmd.hint_match);

        if let Some(cmd) = safe_cmd {
            let status = Command::new(cmd.program).args(cmd.args).status()?;
            if status.success() {
                print_success(&format!("{} recovery initiated", service.name));
            } else {
                print_error(&format!("{} recovery failed", service.name));
            }
        } else {
            // Unknown command - print for manual execution
            println!("  {}", hint.bright_cyan());
            println!("  (Run manually for safety)");
        }
    }
    Ok(())
}

fn attempt_full_recovery(services: &InfrastructureServices) -> Result<()> {
    // Step 1: Docker first (everything depends on it)
    if services.docker.status == HealthStatus::Down {
        print_info("Step 1: Starting Docker...");
        attempt_recovery(&services.docker)?;
        // Wait for Docker to start
        std::thread::sleep(Duration::from_secs(5));
    }

    // Step 2: Containers via docker-compose
    let containers_down = [
        &services.kafka,
        &services.grafana,
        &services.prometheus,
        &services.jaeger,
    ]
    .iter()
    .any(|s| s.status == HealthStatus::Down);

    if containers_down {
        print_info("Step 2: Starting containers...");
        println!(
            "  {}",
            "docker-compose -f docker-compose.dashstream.yml up -d".bright_cyan()
        );
        println!("  (Run manually for safety)");
    }

    // Step 3: WebSocket server
    if services.websocket.status == HealthStatus::Down {
        print_info("Step 3: WebSocket server needs to be started:");
        if let Some(hint) = &services.websocket.recovery_hint {
            println!("  {}", hint.bright_cyan());
        }
    }

    // Step 4: UI
    if services.ui.status == HealthStatus::Down {
        print_info("Step 4: Observability UI needs to be started:");
        if let Some(hint) = &services.ui.recovery_hint {
            println!("  {}", hint.bright_cyan());
        }
    }

    Ok(())
}

/// Alert information from Prometheus
#[derive(Debug)]
struct AlertInfo {
    name: String,
    state: String,
    severity: String,
    summary: String,
    description: String,
    value: Option<f64>,
}

/// Check Prometheus for active alerts and display with explanations
async fn check_and_display_alerts(verbose: bool) {
    println!("{}", "Prometheus Alerts".bright_white().bold());
    println!("{}", "─".repeat(50).bright_black());

    // Query Prometheus alerts API
    let alerts = match fetch_prometheus_alerts().await {
        Ok(alerts) => alerts,
        Err(e) => {
            print_error(&format!("Failed to fetch alerts: {e}"));
            println!(
                "  {} Prometheus may be down or unreachable at localhost:9090",
                "→".bright_cyan()
            );
            return;
        }
    };

    if alerts.is_empty() {
        print_success("No active alerts");
        return;
    }

    let firing: Vec<_> = alerts.iter().filter(|a| a.state == "firing").collect();
    let pending: Vec<_> = alerts.iter().filter(|a| a.state == "pending").collect();

    if !firing.is_empty() {
        println!();
        println!("{} {} firing alert(s):", "🔥".bright_red(), firing.len());
        for alert in &firing {
            print_alert_with_explanation(alert, verbose);
        }
    }

    if !pending.is_empty() {
        println!();
        println!(
            "{} {} pending alert(s):",
            "⏳".bright_yellow(),
            pending.len()
        );
        for alert in &pending {
            print_alert_with_explanation(alert, verbose);
        }
    }
}

/// Fetch alerts from Prometheus API
async fn fetch_prometheus_alerts() -> Result<Vec<AlertInfo>> {
    // M-502: Add timeout to prevent hanging on slow Prometheus response
    let output = Command::new("curl")
        .args(["-s", "-m", "10", "http://localhost:9090/api/v1/alerts"])
        .output()?;

    if !output.status.success() {
        anyhow::bail!("curl failed");
    }

    let response: serde_json::Value = serde_json::from_slice(&output.stdout)?;

    let mut alerts = Vec::new();
    if let Some(data) = response.get("data") {
        if let Some(alert_list) = data.get("alerts").and_then(|a| a.as_array()) {
            for alert in alert_list {
                let labels = alert.get("labels").unwrap_or(&serde_json::Value::Null);
                let annotations = alert.get("annotations").unwrap_or(&serde_json::Value::Null);

                alerts.push(AlertInfo {
                    name: labels
                        .get("alertname")
                        .and_then(|v| v.as_str())
                        .unwrap_or("unknown")
                        .to_string(),
                    state: alert
                        .get("state")
                        .and_then(|v| v.as_str())
                        .unwrap_or("unknown")
                        .to_string(),
                    severity: labels
                        .get("severity")
                        .and_then(|v| v.as_str())
                        .unwrap_or("unknown")
                        .to_string(),
                    summary: annotations
                        .get("summary")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    description: annotations
                        .get("description")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    value: alert
                        .get("value")
                        .and_then(|v| v.as_str())
                        .and_then(|s| s.parse().ok()),
                });
            }
        }
    }

    Ok(alerts)
}

/// Print an alert with AI-readable explanation
fn print_alert_with_explanation(alert: &AlertInfo, verbose: bool) {
    let severity_icon = match alert.severity.as_str() {
        "critical" => "🔴".to_string(),
        "warning" => "🟡".to_string(),
        _ => "⚪".to_string(),
    };

    println!();
    println!(
        "  {} {} [{}]",
        severity_icon,
        alert.name.bright_white().bold(),
        alert.severity.bright_red()
    );

    if !alert.summary.is_empty() {
        println!("    {}", alert.summary);
    }

    if verbose && !alert.description.is_empty() {
        println!("    {}", alert.description.bright_black());
    }

    // Add AI-readable explanation
    let explanation = get_alert_explanation(&alert.name, alert.value);
    println!();
    println!(
        "    {} {}",
        "What this means:".bright_cyan().bold(),
        explanation.meaning
    );
    println!(
        "    {} {}",
        "Why it happened:".bright_yellow().bold(),
        explanation.cause
    );
    println!(
        "    {} {}",
        "How to fix:".bright_green().bold(),
        explanation.fix
    );

    if let Some(cmd) = explanation.command {
        println!();
        println!(
            "    {} {}",
            "Run:".bright_magenta().bold(),
            cmd.bright_white()
        );
    }
}

/// AI-readable explanation for an alert
struct AlertExplanation {
    meaning: String,
    cause: String,
    fix: String,
    command: Option<String>,
}

/// Get human/AI-readable explanation for known alerts
fn get_alert_explanation(alert_name: &str, value: Option<f64>) -> AlertExplanation {
    match alert_name {
        "HighMessageProcessingErrorRate" | "HighKafkaErrorRate" => {
            let rate = value
                .map(|v| format!("{:.0}%", v * 100.0))
                .unwrap_or_else(|| "high".to_string());
            AlertExplanation {
                meaning: format!(
                    "The WebSocket server is failing to process messages (primarily decode failures). \
                    Error rate is {}. Real-time event visibility may be degraded.",
                    rate
                ),
                cause: "Most commonly: schema mismatch / old messages / corruption, or a producer emitting an unexpected payload. \
                    Kafka infrastructure outages are tracked separately via websocket_infrastructure_errors_total.".to_string(),
                fix: "Check decode error breakdown and DLQ volume; then confirm producer schema/version alignment. \
                    If infra errors are also rising, investigate Kafka connectivity separately.".to_string(),
                command: Some(
                    "curl -s http://localhost:3002/metrics | grep -E 'websocket_decode_errors_total|websocket_old_data_decode_errors_total|websocket_dlq_sends_total|websocket_infrastructure_errors_total'".to_string(),
                ),
            }
        }
        "KafkaConsumerLagHigh" | "KafkaConsumerLagCritical" | "KafkaConsumerLag" => AlertExplanation {
            meaning: "Messages are piling up in Kafka faster than the WebSocket consumer can process them. \
                This delays event visibility in the dashboard.".to_string(),
            cause: "Common causes: websocket-server CPU/memory constrained, network/Kafka slowness, \
                or lag checks/processing that blocks consumption.".to_string(),
            fix: "Check websocket-server resource usage and backpressure/dropped messages; \
                confirm Kafka is healthy; consider lowering message volume or optimizing consumer work.".to_string(),
            command: Some(
                "curl -s http://localhost:3002/metrics | grep -E 'websocket_kafka_consumer_lag|websocket_dropped_messages_total|websocket_e2e_latency_ms'".to_string(),
            ),
        },
        "HighDecodeErrorRate" => AlertExplanation {
            meaning: "A high fraction of consumed messages fail protobuf decoding. This indicates schema mismatch or corrupted payloads.".to_string(),
            cause: "Producer and consumer schema versions are out of sync, or old messages are still present in the topic and being replayed.".to_string(),
            fix: "Check websocket_old_data_decode_errors_total vs websocket_decode_errors_total; verify schema version and topic retention; inspect DLQ payloads.".to_string(),
            command: Some(
                "curl -s http://localhost:3002/metrics | grep -E 'websocket_decode_errors_total|websocket_old_data_decode_errors_total|websocket_dlq_sends_total'".to_string(),
            ),
        },
        "HighMemoryUsage" => AlertExplanation {
            meaning: "A container is using excessive memory and may be killed by the OOM killer.".to_string(),
            cause: "Memory leak, large data processing, or insufficient memory allocation.".to_string(),
            fix: "Check which container is affected and consider increasing memory limits \
                or investigating memory leaks.".to_string(),
            command: Some("docker stats --no-stream".to_string()),
        },
        "HighCpuUsage" => AlertExplanation {
            meaning: "A service is consuming excessive CPU, which may slow down the entire system.".to_string(),
            cause: "Intensive computation, infinite loop, or insufficient CPU allocation.".to_string(),
            fix: "Identify the high-CPU container and investigate the cause.".to_string(),
            command: Some("docker stats --no-stream".to_string()),
        },
        "ServiceDown" => AlertExplanation {
            meaning: "A critical service has stopped responding to health checks.".to_string(),
            cause: "The service crashed, lost network connectivity, or is overloaded.".to_string(),
            fix: "Check the service logs and restart if necessary.".to_string(),
            command: Some("dashflow status".to_string()),
        },
        "PrometheusTargetDown" => AlertExplanation {
            meaning: "Prometheus cannot scrape metrics from one of its targets. \
                Monitoring data for that target is not being collected.".to_string(),
            cause: "The target service is down, network issue, or misconfigured scrape config.".to_string(),
            fix: "Check if the target service is running and accessible.".to_string(),
            command: Some("curl -s http://localhost:9090/api/v1/targets | jq '.data.activeTargets[] | select(.health != \"up\")'".to_string()),
        },
        _ => AlertExplanation {
            meaning: format!("Alert '{}' is active. Check Prometheus for details.", alert_name),
            cause: "Cause unknown - check alert annotations for more information.".to_string(),
            fix: "Review the alert in Grafana or Prometheus UI for context.".to_string(),
            command: Some(format!("curl -s 'http://localhost:9090/api/v1/alerts' | jq '.data.alerts[] | select(.labels.alertname == \"{}\")'", alert_name)),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_health_status_icon() {
        assert_eq!(HealthStatus::Healthy.icon(), "✓");
        assert_eq!(HealthStatus::Down.icon(), "✗");
    }

    #[test]
    fn test_check_port_closed() {
        // M-506: Use port 0 (OS assigns ephemeral port, then immediately closes)
        // to test that a random closed port returns false.
        // Create a listener on port 0, get the assigned port, close it, then verify.
        use std::net::TcpListener;
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind to ephemeral port");
        let port = listener.local_addr().unwrap().port();
        drop(listener); // Close the listener immediately

        // Port should now be closed
        assert!(!check_port(port));
    }

    #[test]
    fn test_check_single_service_unknown() {
        // M-507: Unknown services now return an error instead of Unknown status
        let result = check_single_service("nonexistent-service");
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("Unknown service"));
        assert!(err_msg.contains("nonexistent-service"));
        assert!(err_msg.contains("Valid services"));
    }

    #[test]
    fn test_check_single_service_valid() {
        // Valid services should return Ok
        let result = check_single_service("docker");
        assert!(result.is_ok());
    }
}
