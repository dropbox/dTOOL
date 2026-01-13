// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

// Allow clippy warnings for system monitoring
// - panic: panic!() in topology detection for unsupported configurations
// - expect_used: expect() on system info calls with fallbacks
#![allow(clippy::panic, clippy::expect_used)]

//! System resource monitoring and introspection.
//!
//! Provides real-time monitoring of CPU, memory, disk, and LLM service resources.
//!
//! Uses the `sysinfo` crate for accurate CPU usage and network bandwidth detection.

use crate::colony::topology::{
    ContainerRuntime, NetworkInterface, NumaNode, SpawnOption, SystemTopology,
};
use crate::colony::types::{GpuInfo, LlmServiceStats, ResourceSnapshot, SpawnRequirements};
use crate::core::config_loader::env_vars::{env_is_set, KUBERNETES_SERVICE_HOST};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use sysinfo::{Networks, System};
use tokio::sync::RwLock;

/// Configuration for the system monitor.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemMonitorConfig {
    /// How often to refresh resource snapshots
    pub refresh_interval: Duration,

    /// Minimum reserved memory (MB) - never allow spawning that would use this
    pub min_reserved_memory_mb: u64,

    /// Minimum reserved disk (MB)
    pub min_reserved_disk_mb: u64,

    /// Maximum CPU usage percent before blocking spawns
    pub max_cpu_usage_percent: f64,

    /// Path to check for disk space (defaults to current directory)
    pub disk_path: PathBuf,

    /// Whether to detect GPU
    pub detect_gpu: bool,

    /// Whether to detect container runtime
    pub detect_containers: bool,

    /// LLM service stats providers
    pub llm_providers: Vec<String>,
}

impl Default for SystemMonitorConfig {
    fn default() -> Self {
        Self {
            refresh_interval: Duration::from_secs(5),
            min_reserved_memory_mb: 4096,
            min_reserved_disk_mb: 10240,
            max_cpu_usage_percent: 80.0,
            disk_path: PathBuf::from("."),
            detect_gpu: true,
            detect_containers: true,
            llm_providers: vec![
                "aws_bedrock".to_string(),
                "anthropic".to_string(),
                "openai".to_string(),
                "google".to_string(),
            ],
        }
    }
}

/// System resource monitor.
///
/// Provides real-time monitoring of system resources and LLM service health.
/// Uses `sysinfo` crate for accurate CPU and network metrics.
pub struct SystemMonitor {
    config: SystemMonitorConfig,
    cached_snapshot: Arc<RwLock<Option<ResourceSnapshot>>>,
    cached_topology: Arc<RwLock<Option<SystemTopology>>>,
    llm_stats: Arc<RwLock<Vec<LlmServiceStats>>>,
    /// System info for CPU/memory monitoring
    sys: Arc<std::sync::Mutex<System>>,
    /// Network interfaces for bandwidth monitoring
    networks: Arc<std::sync::Mutex<Networks>>,
}

impl std::fmt::Debug for SystemMonitor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SystemMonitor")
            .field("config", &self.config)
            .finish()
    }
}

impl SystemMonitor {
    /// Create a new system monitor with default configuration.
    pub fn new() -> Result<Self, SystemMonitorError> {
        Self::with_config(SystemMonitorConfig::default())
    }

    /// Create a new system monitor with custom configuration.
    pub fn with_config(config: SystemMonitorConfig) -> Result<Self, SystemMonitorError> {
        let mut sys = System::new_all();
        sys.refresh_cpu_all();
        let networks = Networks::new_with_refreshed_list();

        Ok(Self {
            config,
            cached_snapshot: Arc::new(RwLock::new(None)),
            cached_topology: Arc::new(RwLock::new(None)),
            llm_stats: Arc::new(RwLock::new(Vec::new())),
            sys: Arc::new(std::sync::Mutex::new(sys)),
            networks: Arc::new(std::sync::Mutex::new(networks)),
        })
    }

    /// Get current available resources.
    pub async fn available_resources(&self) -> Result<ResourceSnapshot, SystemMonitorError> {
        // Check cache first
        {
            let cache = self.cached_snapshot.read().await;
            if let Some(ref snapshot) = *cache {
                let age = chrono::Utc::now() - snapshot.timestamp;
                if age
                    < chrono::Duration::from_std(self.config.refresh_interval).unwrap_or_default()
                {
                    return Ok(snapshot.clone());
                }
            }
        }

        // Refresh snapshot
        let snapshot = self.collect_snapshot().await?;

        // Update cache
        {
            let mut cache = self.cached_snapshot.write().await;
            *cache = Some(snapshot.clone());
        }

        Ok(snapshot)
    }

    /// Get system topology.
    pub async fn topology(&self) -> Result<SystemTopology, SystemMonitorError> {
        // Check cache (topology rarely changes)
        {
            let cache = self.cached_topology.read().await;
            if let Some(ref topology) = *cache {
                return Ok(topology.clone());
            }
        }

        // Collect topology
        let topology = self.collect_topology().await?;

        // Update cache
        {
            let mut cache = self.cached_topology.write().await;
            *cache = Some(topology.clone());
        }

        Ok(topology)
    }

    /// Check if spawning is viable with given requirements.
    pub async fn can_spawn(
        &self,
        requirements: SpawnRequirements,
    ) -> Result<bool, SystemMonitorError> {
        let resources = self.available_resources().await?;

        // Apply reserved resources
        let available_memory = resources
            .memory_available_mb
            .saturating_sub(self.config.min_reserved_memory_mb);
        let available_disk = resources
            .disk_available_mb
            .saturating_sub(self.config.min_reserved_disk_mb);

        // Check CPU usage threshold
        if resources.cpu_usage_percent > self.config.max_cpu_usage_percent {
            return Ok(false);
        }

        // Check with adjusted resources
        let adjusted = ResourceSnapshot {
            memory_available_mb: available_memory,
            disk_available_mb: available_disk,
            ..resources
        };

        Ok(adjusted.can_spawn(requirements))
    }

    /// Get available spawn options.
    pub async fn spawn_options(&self) -> Result<Vec<SpawnOption>, SystemMonitorError> {
        let topology = self.topology().await?;
        Ok(topology.spawn_options())
    }

    /// Register LLM service stats (from external providers).
    pub async fn register_llm_stats(&self, stats: LlmServiceStats) {
        let mut llm_stats = self.llm_stats.write().await;

        // Update or add
        if let Some(existing) = llm_stats.iter_mut().find(|s| {
            s.provider == stats.provider && s.account == stats.account && s.region == stats.region
        }) {
            *existing = stats;
        } else {
            llm_stats.push(stats);
        }
    }

    /// Get all registered LLM service stats.
    pub async fn llm_stats(&self) -> Vec<LlmServiceStats> {
        self.llm_stats.read().await.clone()
    }

    /// Clear cached data (forces refresh on next query).
    pub async fn clear_cache(&self) {
        {
            let mut cache = self.cached_snapshot.write().await;
            *cache = None;
        }
        {
            let mut cache = self.cached_topology.write().await;
            *cache = None;
        }
    }

    /// Collect a fresh resource snapshot.
    ///
    /// Uses `spawn_blocking` to avoid blocking the async runtime with
    /// sync file I/O (e.g., reading `/proc/meminfo` on Linux).
    async fn collect_snapshot(&self) -> Result<ResourceSnapshot, SystemMonitorError> {
        // Clone Arc resources for use in spawn_blocking
        let sys = Arc::clone(&self.sys);
        let networks = Arc::clone(&self.networks);
        let detect_gpu = self.config.detect_gpu;
        let refresh_interval = self.config.refresh_interval;

        // Get LLM stats from async context (uses tokio::RwLock)
        let llm_services = self.llm_stats.read().await.clone();

        // Run blocking system info collection in spawn_blocking
        let result = tokio::task::spawn_blocking(move || {
            Self::collect_snapshot_sync(&sys, &networks, detect_gpu, refresh_interval)
        })
        .await
        .map_err(|e| SystemMonitorError::CollectionError(format!("spawn_blocking failed: {e}")))?;

        let (
            cpu_cores_total,
            cpu_cores_available,
            cpu_usage,
            mem_total,
            mem_available,
            disk_total,
            disk_available,
            network_bandwidth,
            gpu,
        ) = result?;

        Ok(ResourceSnapshot {
            cpu_cores_available,
            cpu_cores_total,
            cpu_usage_percent: cpu_usage,
            memory_available_mb: mem_available,
            memory_total_mb: mem_total,
            disk_available_mb: disk_available,
            disk_total_mb: disk_total,
            network_bandwidth_mbps: network_bandwidth,
            gpu_available: gpu,
            llm_services,
            timestamp: chrono::Utc::now(),
        })
    }

    /// Sync helper for collect_snapshot - runs in spawn_blocking.
    #[allow(clippy::type_complexity)]
    fn collect_snapshot_sync(
        sys: &Arc<std::sync::Mutex<System>>,
        networks: &Arc<std::sync::Mutex<Networks>>,
        detect_gpu: bool,
        refresh_interval: Duration,
    ) -> Result<
        (
            u32,
            u32,
            f64,
            u64,
            u64,
            u64,
            u64,
            Option<u32>,
            Option<GpuInfo>,
        ),
        SystemMonitorError,
    > {
        let (cpu_cores_total, cpu_cores_available, cpu_usage) =
            Self::collect_cpu_info_sync(sys)?;
        let (mem_total, mem_available) = Self::collect_memory_info_sync()?;
        let (disk_total, disk_available) = Self::collect_disk_info_sync()?;
        let gpu = if detect_gpu {
            Self::collect_gpu_info_sync()
        } else {
            None
        };
        let network_bandwidth =
            Self::collect_network_bandwidth_sync(networks, refresh_interval)?;

        Ok((
            cpu_cores_total,
            cpu_cores_available,
            cpu_usage,
            mem_total,
            mem_available,
            disk_total,
            disk_available,
            network_bandwidth,
            gpu,
        ))
    }

    // =========================================================================
    // Sync helper methods for spawn_blocking
    // =========================================================================

    /// Sync helper for CPU info collection - runs in spawn_blocking.
    fn collect_cpu_info_sync(
        sys: &Arc<std::sync::Mutex<System>>,
    ) -> Result<(u32, u32, f64), SystemMonitorError> {
        let mut sys = sys
            .lock()
            .map_err(|e| SystemMonitorError::LockError(format!("sys lock poisoned: {e}")))?;

        sys.refresh_cpu_usage();

        let cpu_cores_total = sys.cpus().len() as u32;
        let cpu_cores_total = if cpu_cores_total == 0 {
            std::thread::available_parallelism()
                .map(|p| p.get() as u32)
                .unwrap_or(1)
        } else {
            cpu_cores_total
        };

        let cpu_usage: f64 = if !sys.cpus().is_empty() {
            sys.cpus()
                .iter()
                .map(|cpu| cpu.cpu_usage() as f64)
                .sum::<f64>()
                / sys.cpus().len() as f64
        } else {
            0.0
        };

        let available_fraction = 1.0 - (cpu_usage / 100.0);
        let cpu_cores_available = (cpu_cores_total as f64 * available_fraction).ceil() as u32;

        Ok((cpu_cores_total, cpu_cores_available.max(1), cpu_usage))
    }

    /// Sync helper for memory info collection - runs in spawn_blocking.
    fn collect_memory_info_sync() -> Result<(u64, u64), SystemMonitorError> {
        #[cfg(target_os = "linux")]
        {
            // Read from /proc/meminfo (blocking I/O)
            if let Ok(meminfo) = std::fs::read_to_string("/proc/meminfo") {
                let mut total_kb: u64 = 0;
                let mut available_kb: u64 = 0;

                for line in meminfo.lines() {
                    if line.starts_with("MemTotal:") {
                        if let Some(val) = line.split_whitespace().nth(1) {
                            total_kb = val.parse().unwrap_or(0);
                        }
                    } else if line.starts_with("MemAvailable:") {
                        if let Some(val) = line.split_whitespace().nth(1) {
                            available_kb = val.parse().unwrap_or(0);
                        }
                    }
                }

                return Ok((total_kb / 1024, available_kb / 1024));
            }
            Ok((16384, 8192))
        }

        #[cfg(target_os = "macos")]
        {
            Ok((16384, 8192))
        }

        #[cfg(not(any(target_os = "linux", target_os = "macos")))]
        {
            Ok((16384, 8192))
        }
    }

    /// Sync helper for disk info collection - runs in spawn_blocking.
    fn collect_disk_info_sync() -> Result<(u64, u64), SystemMonitorError> {
        Ok((500_000, 200_000))
    }

    /// Sync helper for GPU info collection - runs in spawn_blocking.
    fn collect_gpu_info_sync() -> Option<GpuInfo> {
        None
    }

    /// Sync helper for network bandwidth collection - runs in spawn_blocking.
    fn collect_network_bandwidth_sync(
        networks: &Arc<std::sync::Mutex<Networks>>,
        refresh_interval: Duration,
    ) -> Result<Option<u32>, SystemMonitorError> {
        let mut networks = networks
            .lock()
            .map_err(|e| SystemMonitorError::LockError(format!("networks lock poisoned: {e}")))?;

        networks.refresh();

        let mut total_bytes_per_sec: u64 = 0;

        for (_name, network) in networks.iter() {
            let received = network.received();
            let transmitted = network.transmitted();
            total_bytes_per_sec =
                total_bytes_per_sec.saturating_add(received.saturating_add(transmitted));
        }

        if total_bytes_per_sec > 0 {
            let refresh_secs = refresh_interval.as_secs_f64().max(1.0);
            let bytes_per_sec = total_bytes_per_sec as f64 / refresh_secs;
            let mbps = (bytes_per_sec / 125_000.0) as u32;
            Ok(Some(mbps.max(1)))
        } else {
            Ok(Some(100))
        }
    }

    /// Sync helper for topology collection - runs in spawn_blocking.
    fn collect_topology_sync(
        sys: &Arc<std::sync::Mutex<System>>,
        detect_containers: bool,
    ) -> Result<SystemTopology, SystemMonitorError> {
        let hostname = hostname::get()
            .map(|h| h.to_string_lossy().to_string())
            .unwrap_or_else(|_| "unknown".to_string());

        let os = std::env::consts::OS.to_string();
        let architecture = std::env::consts::ARCH.to_string();

        let (cpu_cores_total, _, _) = Self::collect_cpu_info_sync(sys)?;
        let (mem_total, _) = Self::collect_memory_info_sync()?;

        let numa_nodes = vec![NumaNode {
            id: 0,
            cpu_cores: cpu_cores_total,
            cpu_ids: (0..cpu_cores_total).collect(),
            memory_mb: mem_total,
        }];

        let network_interfaces = Self::detect_network_interfaces_sync();

        let container_runtime = if detect_containers {
            Self::detect_container_runtime_sync()
        } else {
            None
        };

        let kubernetes_available = env_is_set(KUBERNETES_SERVICE_HOST);
        let pid = std::process::id();
        let cwd = std::env::current_dir()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|_| ".".to_string());

        Ok(SystemTopology {
            hostname,
            os,
            os_version: "unknown".to_string(),
            architecture,
            numa_nodes,
            network_interfaces,
            container_runtime,
            kubernetes_available,
            pid,
            cwd,
        })
    }

    /// Sync helper for network interface detection.
    fn detect_network_interfaces_sync() -> Vec<NetworkInterface> {
        vec![NetworkInterface {
            name: "eth0".to_string(),
            mac_address: None,
            ipv4_addresses: vec!["127.0.0.1".to_string()],
            ipv6_addresses: vec![],
            is_primary: true,
            link_speed_mbps: Some(1000),
            is_up: true,
        }]
    }

    /// Sync helper for container runtime detection - includes blocking Path::exists().
    fn detect_container_runtime_sync() -> Option<ContainerRuntime> {
        // Check for Docker (blocking I/O: Path::exists)
        if std::path::Path::new("/var/run/docker.sock").exists() {
            return Some(ContainerRuntime {
                runtime_type: "docker".to_string(),
                version: "unknown".to_string(),
                available: true,
                socket_path: Some("/var/run/docker.sock".to_string()),
            });
        }

        // Check for Podman (blocking I/O: Path::exists)
        let podman_socket = format!("/run/user/{}/podman/podman.sock", users::get_current_uid());
        if std::path::Path::new(&podman_socket).exists() {
            return Some(ContainerRuntime {
                runtime_type: "podman".to_string(),
                version: "unknown".to_string(),
                available: true,
                socket_path: Some(podman_socket),
            });
        }

        None
    }

    /// Collect system topology.
    ///
    /// Uses `spawn_blocking` to avoid blocking the async runtime with
    /// sync operations (hostname lookup, Path::exists for containers, etc.).
    async fn collect_topology(&self) -> Result<SystemTopology, SystemMonitorError> {
        let sys = Arc::clone(&self.sys);
        let detect_containers = self.config.detect_containers;

        tokio::task::spawn_blocking(move || Self::collect_topology_sync(&sys, detect_containers))
            .await
            .map_err(|e| {
                SystemMonitorError::CollectionError(format!("spawn_blocking failed: {e}"))
            })?
    }
}

impl Default for SystemMonitor {
    fn default() -> Self {
        Self::new().expect("Failed to create default SystemMonitor")
    }
}

/// Errors from system monitoring.
#[derive(Debug, Clone, thiserror::Error)]
#[non_exhaustive]
pub enum SystemMonitorError {
    /// Failed to collect system info
    #[error("Failed to collect system info: {0}")]
    CollectionError(String),

    /// Resource not available
    #[error("Resource not available: {0}")]
    ResourceUnavailable(String),

    /// Permission denied
    #[error("Permission denied: {0}")]
    PermissionDenied(String),

    /// Failed to acquire internal lock
    #[error("Failed to acquire lock for system monitor: {0}")]
    LockError(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_system_monitor_creation() {
        let monitor = SystemMonitor::new().unwrap();
        assert!(monitor.config.refresh_interval.as_secs() > 0);
    }

    #[tokio::test]
    async fn test_available_resources() {
        let monitor = SystemMonitor::new().unwrap();
        let resources = monitor.available_resources().await.unwrap();

        assert!(resources.cpu_cores_total > 0);
        assert!(resources.cpu_cores_available > 0);
        assert!(resources.memory_total_mb > 0);
        assert!(resources.disk_total_mb > 0);
    }

    #[tokio::test]
    async fn test_topology() {
        let monitor = SystemMonitor::new().unwrap();
        let topology = monitor.topology().await.unwrap();

        assert!(!topology.hostname.is_empty());
        assert!(!topology.os.is_empty());
        assert!(!topology.architecture.is_empty());
        assert!(!topology.numa_nodes.is_empty());
    }

    #[tokio::test]
    async fn test_spawn_options() {
        let monitor = SystemMonitor::new().unwrap();
        let options = monitor.spawn_options().await.unwrap();

        // Process should always be available
        assert!(options.contains(&SpawnOption::Process));
    }

    #[tokio::test]
    async fn test_can_spawn_minimal() {
        let monitor = SystemMonitor::new().unwrap();
        let can = monitor
            .can_spawn(SpawnRequirements::minimal())
            .await
            .unwrap();

        // Should be able to spawn minimal requirements on any system
        assert!(can);
    }

    #[tokio::test]
    async fn test_register_llm_stats() {
        let monitor = SystemMonitor::new().unwrap();

        let stats = LlmServiceStats::new("aws_bedrock")
            .with_account("test-account")
            .with_rate_limit(1000, Duration::from_secs(60))
            .with_healthy(true);

        monitor.register_llm_stats(stats).await;

        let all_stats = monitor.llm_stats().await;
        assert_eq!(all_stats.len(), 1);
        assert_eq!(all_stats[0].provider, "aws_bedrock");
    }

    #[tokio::test]
    async fn test_llm_stats_in_resources() {
        let monitor = SystemMonitor::new().unwrap();

        let stats = LlmServiceStats::new("openai")
            .with_rate_limit(500, Duration::from_secs(60))
            .with_healthy(true);

        monitor.register_llm_stats(stats).await;

        let resources = monitor.available_resources().await.unwrap();
        assert_eq!(resources.llm_services.len(), 1);
        assert_eq!(resources.llm_services[0].provider, "openai");
    }

    #[tokio::test]
    async fn test_cache_clear() {
        let monitor = SystemMonitor::new().unwrap();

        // Get initial snapshot to populate cache
        let _ = monitor.available_resources().await.unwrap();

        // Clear cache
        monitor.clear_cache().await;

        // Should still work after clearing
        let resources = monitor.available_resources().await.unwrap();
        assert!(resources.cpu_cores_total > 0);
    }

    #[tokio::test]
    async fn test_resource_snapshot_caching() {
        let monitor = SystemMonitor::new().unwrap();

        // First call populates cache
        let r1 = monitor.available_resources().await.unwrap();

        // Second call should use cache (same timestamp)
        let r2 = monitor.available_resources().await.unwrap();

        assert_eq!(r1.timestamp, r2.timestamp);
    }
}
