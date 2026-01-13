// Allow clippy warnings for this module
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::clone_on_ref_ptr)]
#![allow(clippy::needless_pass_by_value, clippy::redundant_clone)]
// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

//! Core types for colony resource management and system introspection.

use serde::{Deserialize, Serialize};
use std::time::Duration;

/// A snapshot of currently available system resources.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceSnapshot {
    /// Available CPU cores (not currently in heavy use)
    pub cpu_cores_available: u32,

    /// Total CPU cores on the system
    pub cpu_cores_total: u32,

    /// CPU usage percentage (0.0 - 100.0)
    pub cpu_usage_percent: f64,

    /// Available memory in MB
    pub memory_available_mb: u64,

    /// Total memory in MB
    pub memory_total_mb: u64,

    /// Available disk space in MB
    pub disk_available_mb: u64,

    /// Total disk space in MB
    pub disk_total_mb: u64,

    /// Network bandwidth in Mbps (estimated)
    pub network_bandwidth_mbps: Option<u32>,

    /// GPU information if available
    pub gpu_available: Option<GpuInfo>,

    /// LLM service statistics
    pub llm_services: Vec<LlmServiceStats>,

    /// Timestamp of this snapshot
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

impl ResourceSnapshot {
    /// Check if resources are sufficient to spawn a new worker.
    pub fn can_spawn(&self, requirements: SpawnRequirements) -> bool {
        // Check CPU
        if self.cpu_cores_available < requirements.min_cpu_cores {
            return false;
        }

        // Check memory
        if self.memory_available_mb < requirements.min_memory_mb {
            return false;
        }

        // Check disk
        if self.disk_available_mb < requirements.min_disk_mb {
            return false;
        }

        // Check LLM rate limit if required
        if requirements.min_llm_rate_limit > 0 {
            let total_rate_limit: u32 = self
                .llm_services
                .iter()
                .filter(|s| s.healthy)
                .map(|s| s.rate_limit_remaining)
                .sum();

            if total_rate_limit < requirements.min_llm_rate_limit {
                return false;
            }
        }

        true
    }

    /// Get total LLM capacity across all providers.
    pub fn total_llm_capacity(&self) -> TotalLlmCapacity {
        let healthy_services: Vec<_> = self.llm_services.iter().filter(|s| s.healthy).collect();

        TotalLlmCapacity {
            total_rate_limit: healthy_services
                .iter()
                .map(|s| s.rate_limit_remaining)
                .sum(),
            providers_healthy: healthy_services.len() as u32,
            providers_unhealthy: (self.llm_services.len() - healthy_services.len()) as u32,
            estimated_cost_per_1k_requests: self.estimate_cost_per_1k(),
        }
    }

    /// Estimate average cost per 1K requests across all healthy providers.
    fn estimate_cost_per_1k(&self) -> f64 {
        let costs: Vec<_> = self
            .llm_services
            .iter()
            .filter(|s| s.healthy)
            .filter_map(|s| s.cost_per_1k.as_ref())
            .collect();

        if costs.is_empty() {
            return 0.0;
        }

        let total: f64 = costs.iter().map(|c| (c.input + c.output) / 2.0).sum();

        total / costs.len() as f64
    }

    /// Get memory info as structured data.
    pub fn memory_info(&self) -> MemoryInfo {
        MemoryInfo {
            available_mb: self.memory_available_mb,
            total_mb: self.memory_total_mb,
            used_mb: self
                .memory_total_mb
                .saturating_sub(self.memory_available_mb),
            usage_percent: if self.memory_total_mb > 0 {
                ((self.memory_total_mb - self.memory_available_mb) as f64
                    / self.memory_total_mb as f64)
                    * 100.0
            } else {
                0.0
            },
        }
    }

    /// Get disk info as structured data.
    pub fn disk_info(&self) -> DiskInfo {
        DiskInfo {
            available_mb: self.disk_available_mb,
            total_mb: self.disk_total_mb,
            used_mb: self.disk_total_mb.saturating_sub(self.disk_available_mb),
            usage_percent: if self.disk_total_mb > 0 {
                ((self.disk_total_mb - self.disk_available_mb) as f64 / self.disk_total_mb as f64)
                    * 100.0
            } else {
                0.0
            },
        }
    }

    /// Get LLM service by provider name.
    pub fn llm_service(&self, provider: &str) -> Option<&LlmServiceStats> {
        self.llm_services.iter().find(|s| s.provider == provider)
    }
}

/// Requirements for spawning a new worker.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SpawnRequirements {
    /// Minimum CPU cores needed
    pub min_cpu_cores: u32,

    /// Minimum memory in MB
    pub min_memory_mb: u64,

    /// Minimum disk space in MB
    pub min_disk_mb: u64,

    /// Minimum LLM rate limit (requests/minute available)
    pub min_llm_rate_limit: u32,

    /// Minimum GPU VRAM in GB (if GPU required)
    pub min_gpu_vram_gb: Option<u32>,
}

impl SpawnRequirements {
    /// Create minimal requirements for a lightweight worker.
    pub fn minimal() -> Self {
        Self {
            min_cpu_cores: 1,
            min_memory_mb: 512,
            min_disk_mb: 256,
            min_llm_rate_limit: 0,
            min_gpu_vram_gb: None,
        }
    }

    /// Create standard requirements for a typical worker.
    pub fn standard() -> Self {
        Self {
            min_cpu_cores: 2,
            min_memory_mb: 2048,
            min_disk_mb: 1024,
            min_llm_rate_limit: 50,
            min_gpu_vram_gb: None,
        }
    }

    /// Create requirements for a heavy worker (builds, optimization).
    pub fn heavy() -> Self {
        Self {
            min_cpu_cores: 4,
            min_memory_mb: 8192,
            min_disk_mb: 10240,
            min_llm_rate_limit: 100,
            min_gpu_vram_gb: None,
        }
    }
}

/// GPU information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GpuInfo {
    /// GPU model name
    pub model: String,

    /// VRAM in GB
    pub vram_gb: u32,

    /// Available VRAM in GB
    pub vram_available_gb: u32,

    /// CUDA version if available
    pub cuda_version: Option<String>,

    /// ROCm version if available
    pub rocm_version: Option<String>,

    /// GPU utilization percentage
    pub utilization_percent: Option<f64>,
}

/// Memory information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryInfo {
    /// Available memory in MB
    pub available_mb: u64,

    /// Total memory in MB
    pub total_mb: u64,

    /// Used memory in MB
    pub used_mb: u64,

    /// Usage percentage
    pub usage_percent: f64,
}

/// Disk information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiskInfo {
    /// Available disk space in MB
    pub available_mb: u64,

    /// Total disk space in MB
    pub total_mb: u64,

    /// Used disk space in MB
    pub used_mb: u64,

    /// Usage percentage
    pub usage_percent: f64,
}

/// LLM service statistics for a single provider.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmServiceStats {
    /// Provider identifier (aws_bedrock, openai, anthropic, google)
    pub provider: String,

    /// Account identifier (if multi-account)
    pub account: Option<String>,

    /// Region (us-east-1, eu-west-1, etc.)
    pub region: Option<String>,

    /// Models available through this provider
    pub models_available: Vec<String>,

    /// Rate limit: requests remaining in current window
    pub rate_limit_remaining: u32,

    /// Rate limit window size
    pub rate_limit_window: Duration,

    /// Token quota remaining (if applicable)
    pub tokens_remaining: Option<u64>,

    /// Average latency over last 100 requests (ms)
    pub avg_latency_ms: u32,

    /// Error rate in last hour (0.0 - 1.0)
    pub error_rate_1h: f64,

    /// Accumulated cost this session (USD)
    pub cost_accumulated_usd: f64,

    /// Cost per 1K tokens
    pub cost_per_1k: Option<CostPer1k>,

    /// Is service currently healthy?
    pub healthy: bool,

    /// Last successful request timestamp
    pub last_success: Option<chrono::DateTime<chrono::Utc>>,

    /// Last error message if any
    pub last_error: Option<String>,
}

impl LlmServiceStats {
    /// Create a new LLM service stats entry.
    pub fn new(provider: impl Into<String>) -> Self {
        Self {
            provider: provider.into(),
            account: None,
            region: None,
            models_available: Vec::new(),
            rate_limit_remaining: 0,
            rate_limit_window: Duration::from_secs(60),
            tokens_remaining: None,
            avg_latency_ms: 0,
            error_rate_1h: 0.0,
            cost_accumulated_usd: 0.0,
            cost_per_1k: None,
            healthy: true,
            last_success: None,
            last_error: None,
        }
    }

    /// Check if this service is saturated (low rate limit remaining).
    pub fn is_saturated(&self) -> bool {
        self.rate_limit_remaining < 10 || !self.healthy
    }

    /// Builder: set account
    #[must_use]
    pub fn with_account(mut self, account: impl Into<String>) -> Self {
        self.account = Some(account.into());
        self
    }

    /// Builder: set region
    #[must_use]
    pub fn with_region(mut self, region: impl Into<String>) -> Self {
        self.region = Some(region.into());
        self
    }

    /// Builder: set models
    #[must_use]
    pub fn with_models(mut self, models: Vec<String>) -> Self {
        self.models_available = models;
        self
    }

    /// Builder: set rate limit
    #[must_use]
    pub fn with_rate_limit(mut self, remaining: u32, window: Duration) -> Self {
        self.rate_limit_remaining = remaining;
        self.rate_limit_window = window;
        self
    }

    /// Builder: set healthy status
    #[must_use]
    pub fn with_healthy(mut self, healthy: bool) -> Self {
        self.healthy = healthy;
        self
    }

    /// Builder: set latency
    #[must_use]
    pub fn with_latency(mut self, avg_ms: u32) -> Self {
        self.avg_latency_ms = avg_ms;
        self
    }
}

/// Cost per 1K tokens (input and output).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CostPer1k {
    /// Cost per 1K input tokens (USD)
    pub input: f64,

    /// Cost per 1K output tokens (USD)
    pub output: f64,
}

impl CostPer1k {
    /// Create a new cost structure.
    pub fn new(input: f64, output: f64) -> Self {
        Self { input, output }
    }

    /// Average cost per 1K tokens.
    pub fn average(&self) -> f64 {
        (self.input + self.output) / 2.0
    }
}

/// Total LLM capacity across all providers.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TotalLlmCapacity {
    /// Total rate limit remaining across all healthy providers
    pub total_rate_limit: u32,

    /// Number of healthy providers
    pub providers_healthy: u32,

    /// Number of unhealthy providers
    pub providers_unhealthy: u32,

    /// Estimated average cost per 1K requests
    pub estimated_cost_per_1k_requests: f64,
}

impl TotalLlmCapacity {
    /// Check if there is sufficient capacity.
    pub fn has_capacity(&self, min_rate_limit: u32) -> bool {
        self.total_rate_limit >= min_rate_limit && self.providers_healthy > 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_spawn_requirements_presets() {
        let minimal = SpawnRequirements::minimal();
        assert_eq!(minimal.min_cpu_cores, 1);
        assert_eq!(minimal.min_memory_mb, 512);

        let standard = SpawnRequirements::standard();
        assert_eq!(standard.min_cpu_cores, 2);
        assert_eq!(standard.min_memory_mb, 2048);

        let heavy = SpawnRequirements::heavy();
        assert_eq!(heavy.min_cpu_cores, 4);
        assert_eq!(heavy.min_memory_mb, 8192);
    }

    #[test]
    fn test_resource_snapshot_can_spawn() {
        let snapshot = ResourceSnapshot {
            cpu_cores_available: 4,
            cpu_cores_total: 8,
            cpu_usage_percent: 50.0,
            memory_available_mb: 8192,
            memory_total_mb: 16384,
            disk_available_mb: 100_000,
            disk_total_mb: 500_000,
            network_bandwidth_mbps: Some(1000),
            gpu_available: None,
            llm_services: vec![LlmServiceStats::new("aws_bedrock")
                .with_rate_limit(500, Duration::from_secs(60))
                .with_healthy(true)],
            timestamp: chrono::Utc::now(),
        };

        // Should be able to spawn standard worker
        assert!(snapshot.can_spawn(SpawnRequirements::standard()));

        // Should be able to spawn minimal worker
        assert!(snapshot.can_spawn(SpawnRequirements::minimal()));

        // Should NOT be able to spawn if not enough CPU
        let high_cpu_req = SpawnRequirements {
            min_cpu_cores: 8, // More than available
            ..SpawnRequirements::minimal()
        };
        assert!(!snapshot.can_spawn(high_cpu_req));

        // Should NOT be able to spawn if not enough memory
        let high_mem_req = SpawnRequirements {
            min_memory_mb: 16000, // More than available
            ..SpawnRequirements::minimal()
        };
        assert!(!snapshot.can_spawn(high_mem_req));
    }

    #[test]
    fn test_resource_snapshot_total_llm_capacity() {
        let snapshot = ResourceSnapshot {
            cpu_cores_available: 4,
            cpu_cores_total: 8,
            cpu_usage_percent: 50.0,
            memory_available_mb: 8192,
            memory_total_mb: 16384,
            disk_available_mb: 100_000,
            disk_total_mb: 500_000,
            network_bandwidth_mbps: None,
            gpu_available: None,
            llm_services: vec![
                LlmServiceStats::new("aws_bedrock")
                    .with_rate_limit(500, Duration::from_secs(60))
                    .with_healthy(true),
                LlmServiceStats::new("openai")
                    .with_rate_limit(300, Duration::from_secs(60))
                    .with_healthy(true),
                LlmServiceStats::new("google")
                    .with_rate_limit(200, Duration::from_secs(60))
                    .with_healthy(false), // Unhealthy
            ],
            timestamp: chrono::Utc::now(),
        };

        let capacity = snapshot.total_llm_capacity();
        assert_eq!(capacity.total_rate_limit, 800); // Only healthy services
        assert_eq!(capacity.providers_healthy, 2);
        assert_eq!(capacity.providers_unhealthy, 1);
        assert!(capacity.has_capacity(500));
        assert!(!capacity.has_capacity(1000));
    }

    #[test]
    fn test_llm_service_stats_builder() {
        let stats = LlmServiceStats::new("aws_bedrock")
            .with_account("prod-1")
            .with_region("us-east-1")
            .with_models(vec!["claude-3-5-sonnet".to_string()])
            .with_rate_limit(1000, Duration::from_secs(60))
            .with_latency(450)
            .with_healthy(true);

        assert_eq!(stats.provider, "aws_bedrock");
        assert_eq!(stats.account, Some("prod-1".to_string()));
        assert_eq!(stats.region, Some("us-east-1".to_string()));
        assert_eq!(stats.rate_limit_remaining, 1000);
        assert_eq!(stats.avg_latency_ms, 450);
        assert!(stats.healthy);
        assert!(!stats.is_saturated());
    }

    #[test]
    fn test_llm_service_saturated() {
        let low_rate = LlmServiceStats::new("test")
            .with_rate_limit(5, Duration::from_secs(60))
            .with_healthy(true);
        assert!(low_rate.is_saturated());

        let unhealthy = LlmServiceStats::new("test")
            .with_rate_limit(1000, Duration::from_secs(60))
            .with_healthy(false);
        assert!(unhealthy.is_saturated());

        let good = LlmServiceStats::new("test")
            .with_rate_limit(100, Duration::from_secs(60))
            .with_healthy(true);
        assert!(!good.is_saturated());
    }

    #[test]
    fn test_cost_per_1k() {
        let cost = CostPer1k::new(0.003, 0.015);
        assert!((cost.average() - 0.009).abs() < 0.0001);
    }

    #[test]
    fn test_memory_info() {
        let snapshot = ResourceSnapshot {
            cpu_cores_available: 4,
            cpu_cores_total: 8,
            cpu_usage_percent: 50.0,
            memory_available_mb: 8192,
            memory_total_mb: 16384,
            disk_available_mb: 100_000,
            disk_total_mb: 500_000,
            network_bandwidth_mbps: None,
            gpu_available: None,
            llm_services: vec![],
            timestamp: chrono::Utc::now(),
        };

        let mem = snapshot.memory_info();
        assert_eq!(mem.available_mb, 8192);
        assert_eq!(mem.total_mb, 16384);
        assert_eq!(mem.used_mb, 8192);
        assert!((mem.usage_percent - 50.0).abs() < 0.1);
    }

    #[test]
    fn test_disk_info() {
        let snapshot = ResourceSnapshot {
            cpu_cores_available: 4,
            cpu_cores_total: 8,
            cpu_usage_percent: 50.0,
            memory_available_mb: 8192,
            memory_total_mb: 16384,
            disk_available_mb: 100_000,
            disk_total_mb: 500_000,
            network_bandwidth_mbps: None,
            gpu_available: None,
            llm_services: vec![],
            timestamp: chrono::Utc::now(),
        };

        let disk = snapshot.disk_info();
        assert_eq!(disk.available_mb, 100_000);
        assert_eq!(disk.total_mb, 500_000);
        assert_eq!(disk.used_mb, 400_000);
        assert!((disk.usage_percent - 80.0).abs() < 0.1);
    }
}
