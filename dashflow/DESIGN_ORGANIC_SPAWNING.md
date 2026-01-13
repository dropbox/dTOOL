# DashFlow Colony Expansion

**Version:** 1.0
**Date:** 2025-12-09
**Priority:** P3 - Future Capability
**Status:** DESIGN
**Prerequisite:** DESIGN_NETWORK_COORDINATION.md

---

## Executive Summary

DashFlow apps can introspect system resources and architecture, then spawn new
DashFlow instances when resources are available. Like an ant or bee colony that
grows by adding specialized workers, DashFlow apps expand their capabilities by
spawning task-specific agents.

**Key principle:** Apps spawn new instances of THEMSELVES or approved templates.
They do not modify other apps' code (that's handled by locks + self-editing).

---

## Naming: Why "Colony"

Like an ant or bee colony:
- **Queen spawns workers** - Parent app creates specialized worker apps
- **Division of labor** - Different workers for different tasks (scouts, foragers, builders)
- **Resource-aware growth** - Colony only grows when food/space available
- **Coordinated activity** - Workers communicate, share discoveries
- **Collective intelligence** - Simple individuals, smart collective

This captures what we want: purposeful, coordinated, resource-aware expansion.

---

## Architecture

```
┌─────────────────────────────────────────────────────────────────────┐
│                        Host System                                   │
├─────────────────────────────────────────────────────────────────────┤
│                                                                      │
│   ┌────────────────────────────────────────────────────────────┐   │
│   │              System Resource Monitor                        │   │
│   │  CPU: 8 cores (4 available)  Memory: 32GB (16GB free)      │   │
│   │  Disk: 500GB (200GB free)    Network: 1Gbps                │   │
│   └────────────────────────────────────────────────────────────┘   │
│                              │                                      │
│                              ▼                                      │
│   ┌──────────────┐    ┌──────────────┐    ┌──────────────┐        │
│   │  App A       │    │  App B       │    │  App C       │        │
│   │  (Primary)   │    │ (Spawned by A)│   │ (Spawned by A)│        │
│   │              │    │              │    │              │        │
│   │ Colony:    │    │ Task:        │    │ Task:        │        │
│   │ - Monitor    │───►│ Run tests    │    │ Optimize     │        │
│   │ - Spawn      │    │              │    │              │        │
│   │ - Coordinate │    │              │    │              │        │
│   └──────────────┘    └──────────────┘    └──────────────┘        │
│          │                   │                   │                  │
│          └───────────────────┴───────────────────┘                  │
│                    Network Coordination                             │
│                                                                      │
└─────────────────────────────────────────────────────────────────────┘
```

---

## Capabilities

### 1. System Introspection

Apps can query available resources:

```rust
let resources = system.available_resources()?;
// ResourceSnapshot {
//     // Local compute
//     cpu_cores_available: 4,
//     memory_available_mb: 16384,
//     disk_available_mb: 204800,
//     network_bandwidth_mbps: 1000,
//     gpu_available: Some(GpuInfo { ... }),
//
//     // LLM service stats
//     llm_services: vec![
//         LlmServiceStats {
//             provider: "aws_bedrock",
//             models_available: vec!["claude-3-sonnet", "claude-3-haiku"],
//             rate_limit_remaining: 1000,        // requests/minute remaining
//             tokens_remaining: Some(100_000),   // if quota-based
//             avg_latency_ms: 450,
//             error_rate_1h: 0.02,               // 2% errors last hour
//             cost_accumulated_usd: 12.50,      // session cost so far
//         },
//         LlmServiceStats {
//             provider: "openai",
//             models_available: vec!["gpt-4", "gpt-4-turbo"],
//             rate_limit_remaining: 500,
//             tokens_remaining: Some(50_000),
//             avg_latency_ms: 380,
//             error_rate_1h: 0.01,
//             cost_accumulated_usd: 8.25,
//         },
//     ],
// }

// Check if spawning is viable (considers LLM capacity too)
if resources.can_spawn(SpawnRequirements {
    min_cpu_cores: 1,
    min_memory_mb: 2048,
    min_disk_mb: 1024,
    min_llm_rate_limit: 100,  // Need at least 100 req/min available
}) {
    // Spawn new worker
}
```

### LLM Service Stats

Track real-time LLM service health and capacity:

```rust
pub struct LlmServiceStats {
    /// Provider identifier (aws_bedrock, openai, anthropic, google)
    pub provider: String,

    /// Models available through this provider
    pub models_available: Vec<String>,

    /// Rate limit: requests remaining in current window
    pub rate_limit_remaining: u32,

    /// Token quota remaining (if applicable)
    pub tokens_remaining: Option<u64>,

    /// Average latency over last 100 requests
    pub avg_latency_ms: u32,

    /// Error rate in last hour (0.0 - 1.0)
    pub error_rate_1h: f64,

    /// Accumulated cost this session (USD)
    pub cost_accumulated_usd: f64,

    /// Is service currently healthy?
    pub healthy: bool,

    /// Last successful request timestamp
    pub last_success: Option<DateTime<Utc>>,
}

// Query specific provider
let bedrock_stats = resources.llm_service("aws_bedrock")?;
if bedrock_stats.rate_limit_remaining < 50 {
    // Switch to fallback provider or wait
}

// Check total LLM capacity across all providers
let total_capacity = resources.total_llm_capacity()?;
// TotalLlmCapacity {
//     total_rate_limit: 1500,
//     providers_healthy: 2,
//     providers_unhealthy: 0,
//     estimated_cost_per_1k_requests: 0.15,
// }
```

### Colony-Wide LLM Coordination

Apps share LLM usage stats across the colony network. This prevents all apps
from saturating the same account/region simultaneously.

```rust
/// LLM endpoint with account and region specificity
pub struct LlmEndpoint {
    /// Provider (aws_bedrock, openai, anthropic, google)
    pub provider: String,

    /// Specific account identifier
    pub account_id: String,

    /// Region (us-east-1, eu-west-1, etc.)
    pub region: String,

    /// Model identifier (claude-3-5-sonnet, gpt-4-turbo, etc.)
    pub model: String,

    /// Current rate limit status
    pub rate_limit: RateLimitStatus,

    /// Credentials reference (not the actual credentials)
    pub credential_ref: String,
}

pub struct RateLimitStatus {
    /// Requests remaining in current window
    pub remaining: u32,

    /// Window reset time
    pub resets_at: DateTime<Utc>,

    /// Tokens remaining (if token-based quota)
    pub tokens_remaining: Option<u64>,

    /// Is this endpoint currently saturated?
    pub saturated: bool,

    /// Last updated (from local observation or colony report)
    pub last_updated: DateTime<Utc>,

    /// Source of this info (Local or ColonyPeer(uuid))
    pub source: RateLimitSource,
}

/// Colony-wide LLM registry
pub struct ColonyLlmRegistry {
    /// All known LLM endpoints across accounts/regions
    endpoints: Vec<LlmEndpoint>,

    /// Colony-reported usage (aggregated from all apps)
    colony_usage: HashMap<EndpointKey, ColonyUsageStats>,
}

/// Stats aggregated from all colony members
pub struct ColonyUsageStats {
    /// Total requests in last minute from all colony apps
    pub colony_requests_per_min: u32,

    /// Number of apps currently using this endpoint
    pub active_apps: u32,

    /// Predicted saturation time at current rate
    pub estimated_saturation: Option<DateTime<Utc>>,
}

impl ColonyLlmRegistry {
    /// Get the best endpoint for a model (least loaded)
    pub fn best_endpoint(&self, model: &str) -> Option<&LlmEndpoint> {
        self.endpoints
            .iter()
            .filter(|e| e.model == model && !e.rate_limit.saturated)
            .min_by_key(|e| {
                // Score: lower is better
                // Prefer: high remaining capacity, low colony usage
                let remaining_pct = e.rate_limit.remaining as f64 / 1000.0;
                let colony_load = self.colony_usage
                    .get(&e.key())
                    .map(|u| u.colony_requests_per_min)
                    .unwrap_or(0) as f64;

                // Inverse remaining + colony load
                ((1.0 / (remaining_pct + 0.01)) + colony_load) as u64
            })
    }

    /// Report local usage to colony (broadcast via network)
    pub async fn report_usage(&self, network: &DashflowNetwork) -> Result<()> {
        network.broadcast("_llm_usage", json!({
            "endpoints": self.local_usage_stats(),
            "timestamp": Utc::now(),
        }), Priority::Background).await
    }

    /// Handle incoming colony usage report
    pub fn handle_colony_report(&mut self, from: Uuid, report: LlmUsageReport) {
        for (endpoint_key, stats) in report.endpoints {
            self.colony_usage
                .entry(endpoint_key)
                .or_default()
                .merge(from, stats);
        }
    }
}
```

### Multi-Account Failover

```rust
/// Configure multiple accounts for the same provider
let llm_config = LlmConfig {
    provider: "aws_bedrock",
    accounts: vec![
        AccountConfig {
            id: "prod-account-1",
            regions: vec!["us-east-1", "us-west-2"],
            credentials: CredentialRef::EnvVar("AWS_CREDS_PROD1"),
            priority: 1,  // Primary
        },
        AccountConfig {
            id: "prod-account-2",
            regions: vec!["us-east-1", "eu-west-1"],
            credentials: CredentialRef::EnvVar("AWS_CREDS_PROD2"),
            priority: 2,  // Failover
        },
        AccountConfig {
            id: "dev-account",
            regions: vec!["us-east-1"],
            credentials: CredentialRef::EnvVar("AWS_CREDS_DEV"),
            priority: 3,  // Last resort
        },
    ],
    failover_strategy: FailoverStrategy::LeastLoaded,
};

pub enum FailoverStrategy {
    /// Use primary until saturated, then next
    Priority,

    /// Distribute load across all accounts
    RoundRobin,

    /// Use account with most remaining capacity
    LeastLoaded,

    /// Use account with lowest latency
    LowestLatency,

    /// Use cheapest account first
    CostOptimized,
}

// Automatic failover in action
let response = llm_client.complete(prompt).await?;
// Internally:
// 1. Check colony registry for best endpoint
// 2. Try primary account us-east-1
// 3. If rate limited, automatically try us-west-2
// 4. If account-1 saturated, switch to account-2
// 5. Report usage to colony network
```

### Automatic Health-Based Failover

Apps automatically switch models, accounts, or regions when experiencing issues:

```rust
/// Health metrics tracked per endpoint
pub struct EndpointHealth {
    /// Average latency over last 100 requests (ms)
    pub avg_latency_ms: u32,

    /// P95 latency (ms) - 95% of requests faster than this
    pub p95_latency_ms: u32,

    /// P99 latency (ms) - 99% of requests faster than this
    pub p99_latency_ms: u32,

    /// Timeout count in last 5 minutes
    pub timeouts_5m: u32,

    /// Error count in last 5 minutes
    pub errors_5m: u32,

    /// Error rate (0.0 - 1.0) over last hour
    pub error_rate_1h: f64,

    /// Consecutive failures (resets on success)
    pub consecutive_failures: u32,

    /// Last successful request
    pub last_success: Option<DateTime<Utc>>,

    /// Last error message
    pub last_error: Option<String>,

    /// Is endpoint healthy?
    pub healthy: bool,
}

/// Thresholds for automatic failover
pub struct FailoverThresholds {
    /// Switch if avg latency exceeds this (ms)
    pub max_avg_latency_ms: u32,        // default: 2000

    /// Switch if P95 latency exceeds this (ms)
    pub max_p95_latency_ms: u32,        // default: 5000

    /// Switch after this many consecutive failures
    pub max_consecutive_failures: u32,   // default: 3

    /// Switch if error rate exceeds this
    pub max_error_rate: f64,             // default: 0.10 (10%)

    /// Switch after this many timeouts in 5 minutes
    pub max_timeouts_5m: u32,            // default: 5

    /// Consider unhealthy if no success in this duration
    pub stale_threshold: Duration,       // default: 5 minutes
}

impl Default for FailoverThresholds {
    fn default() -> Self {
        Self {
            max_avg_latency_ms: 2000,
            max_p95_latency_ms: 5000,
            max_consecutive_failures: 3,
            max_error_rate: 0.10,
            max_timeouts_5m: 5,
            stale_threshold: Duration::minutes(5),
        }
    }
}

/// Automatic failover logic
impl LlmClient {
    pub async fn complete(&self, prompt: &str) -> Result<Response> {
        let mut attempts = 0;
        let max_attempts = self.config.max_failover_attempts; // default: 3

        loop {
            // Select best endpoint based on health + capacity
            let endpoint = self.registry.select_endpoint(&self.model)?;

            match self.try_request(endpoint, prompt).await {
                Ok(response) => {
                    // Record success, update health metrics
                    self.registry.record_success(endpoint, response.latency_ms);
                    return Ok(response);
                }
                Err(e) => {
                    // Record failure
                    self.registry.record_failure(endpoint, &e);

                    // Check if we should failover
                    let health = self.registry.health(endpoint);

                    if health.should_failover(&self.thresholds) {
                        // Mark endpoint as unhealthy
                        self.registry.mark_unhealthy(endpoint);

                        // Broadcast to colony
                        self.network.broadcast("_llm_usage", json!({
                            "event": "endpoint_unhealthy",
                            "endpoint": endpoint.key(),
                            "reason": e.to_string(),
                            "health": health,
                        }), Priority::Normal).await?;

                        attempts += 1;
                        if attempts >= max_attempts {
                            return Err(LlmError::AllEndpointsFailed);
                        }

                        // Loop will select next best endpoint
                        continue;
                    }

                    return Err(e);
                }
            }
        }
    }
}

impl EndpointHealth {
    /// Check if failover thresholds are exceeded
    pub fn should_failover(&self, thresholds: &FailoverThresholds) -> bool {
        // High latency
        self.avg_latency_ms > thresholds.max_avg_latency_ms
        // Too many consecutive failures
        || self.consecutive_failures >= thresholds.max_consecutive_failures
        // High error rate
        || self.error_rate_1h > thresholds.max_error_rate
        // Too many timeouts
        || self.timeouts_5m >= thresholds.max_timeouts_5m
        // No recent success
        || self.last_success
            .map(|t| Utc::now() - t > thresholds.stale_threshold)
            .unwrap_or(true)
    }
}
```

### Flexible Resource-Based Failover

Different colonies have different resources. Failover is configuration-driven
and adapts to what's actually available:

```rust
/// Colony resource configuration - defines what's available
pub struct ColonyResources {
    /// All available LLM endpoints this colony can use
    pub endpoints: Vec<EndpointConfig>,

    /// Failover strategy
    pub failover: FailoverConfig,

    /// Cost constraints
    pub cost_limits: Option<CostLimits>,
}

/// Individual endpoint configuration
pub struct EndpointConfig {
    /// Unique identifier
    pub id: String,

    /// Provider (aws_bedrock, openai, anthropic, google, azure, local)
    pub provider: String,

    /// Account (for multi-account setups)
    pub account: Option<String>,

    /// Region
    pub region: Option<String>,

    /// Model identifier
    pub model: String,

    /// Credentials reference
    pub credentials: CredentialRef,

    /// Capability tags (used for smart routing)
    pub capabilities: Vec<String>,  // ["reasoning", "coding", "fast", "cheap"]

    /// Priority (lower = preferred)
    pub priority: u32,

    /// Cost per 1K tokens (input/output)
    pub cost_per_1k: Option<CostPer1k>,

    /// Is this endpoint enabled?
    pub enabled: bool,
}

/// Flexible failover configuration
pub struct FailoverConfig {
    /// How to select endpoints
    pub strategy: FailoverStrategy,

    /// Endpoint selection rules (evaluated in order)
    pub rules: Vec<FailoverRule>,

    /// Health thresholds (customizable per colony)
    pub health_thresholds: FailoverThresholds,

    /// Maximum failover attempts before giving up
    pub max_attempts: u32,
}

/// Rule-based endpoint selection
pub struct FailoverRule {
    /// Rule name for logging
    pub name: String,

    /// Condition to match
    pub condition: FailoverCondition,

    /// Action to take
    pub action: FailoverAction,
}

pub enum FailoverCondition {
    /// Always match
    Always,

    /// Match if primary endpoint unhealthy
    PrimaryUnhealthy,

    /// Match if latency exceeds threshold
    HighLatency { threshold_ms: u32 },

    /// Match if error rate exceeds threshold
    HighErrorRate { threshold: f64 },

    /// Match if cost budget exceeded
    BudgetExceeded,

    /// Match based on request type
    RequestType { types: Vec<String> },  // ["reasoning", "simple", "code"]

    /// Custom expression
    Custom { expression: String },
}

pub enum FailoverAction {
    /// Try next endpoint by priority
    NextByPriority,

    /// Try endpoint with specific capability
    WithCapability { capability: String },

    /// Try specific endpoint by ID
    SpecificEndpoint { id: String },

    /// Try any healthy endpoint
    AnyHealthy,

    /// Try cheapest healthy endpoint
    CheapestHealthy,

    /// Try fastest healthy endpoint
    FastestHealthy,

    /// Queue request and wait for recovery
    QueueAndWait { max_wait: Duration },

    /// Fail immediately
    Fail { message: String },
}
```

### Example Configurations

**Small team with just AWS Bedrock:**
```rust
let config = ColonyResources {
    endpoints: vec![
        EndpointConfig {
            id: "bedrock-east".into(),
            provider: "aws_bedrock".into(),
            region: Some("us-east-1".into()),
            model: "claude-3-5-sonnet".into(),
            priority: 1,
            capabilities: vec!["reasoning".into(), "coding".into()],
            ..
        },
        EndpointConfig {
            id: "bedrock-west".into(),
            provider: "aws_bedrock".into(),
            region: Some("us-west-2".into()),
            model: "claude-3-5-sonnet".into(),
            priority: 2,  // Failover
            capabilities: vec!["reasoning".into(), "coding".into()],
            ..
        },
    ],
    failover: FailoverConfig {
        strategy: FailoverStrategy::Priority,
        rules: vec![
            FailoverRule {
                name: "region-failover".into(),
                condition: FailoverCondition::PrimaryUnhealthy,
                action: FailoverAction::NextByPriority,
            },
        ],
        ..
    },
    cost_limits: None,  // No budget constraints
};
```

**Enterprise with multiple providers and cost management:**
```rust
let config = ColonyResources {
    endpoints: vec![
        // Primary: Claude on Bedrock (two accounts)
        EndpointConfig {
            id: "bedrock-prod1-east".into(),
            provider: "aws_bedrock".into(),
            account: Some("prod-1".into()),
            region: Some("us-east-1".into()),
            model: "claude-3-5-sonnet".into(),
            priority: 1,
            cost_per_1k: Some(CostPer1k { input: 0.003, output: 0.015 }),
            ..
        },
        EndpointConfig {
            id: "bedrock-prod2-east".into(),
            provider: "aws_bedrock".into(),
            account: Some("prod-2".into()),
            region: Some("us-east-1".into()),
            model: "claude-3-5-sonnet".into(),
            priority: 2,
            ..
        },
        // Fallback: Haiku (cheaper)
        EndpointConfig {
            id: "bedrock-haiku".into(),
            model: "claude-3-haiku".into(),
            priority: 10,
            capabilities: vec!["fast".into(), "cheap".into()],
            cost_per_1k: Some(CostPer1k { input: 0.00025, output: 0.00125 }),
            ..
        },
        // Alternative provider: OpenAI
        EndpointConfig {
            id: "openai-gpt4".into(),
            provider: "openai".into(),
            model: "gpt-4-turbo".into(),
            priority: 20,  // Last resort
            capabilities: vec!["reasoning".into()],
            ..
        },
    ],
    failover: FailoverConfig {
        strategy: FailoverStrategy::RuleBased,
        rules: vec![
            // Rule 1: If budget exceeded, use cheap model
            FailoverRule {
                name: "budget-fallback".into(),
                condition: FailoverCondition::BudgetExceeded,
                action: FailoverAction::WithCapability { capability: "cheap".into() },
            },
            // Rule 2: For simple requests, prefer fast/cheap
            FailoverRule {
                name: "simple-requests".into(),
                condition: FailoverCondition::RequestType { types: vec!["simple".into()] },
                action: FailoverAction::CheapestHealthy,
            },
            // Rule 3: High latency -> try different region/account
            FailoverRule {
                name: "latency-failover".into(),
                condition: FailoverCondition::HighLatency { threshold_ms: 3000 },
                action: FailoverAction::NextByPriority,
            },
            // Rule 4: High errors -> try different provider
            FailoverRule {
                name: "provider-failover".into(),
                condition: FailoverCondition::HighErrorRate { threshold: 0.15 },
                action: FailoverAction::AnyHealthy,
            },
        ],
        health_thresholds: FailoverThresholds {
            max_avg_latency_ms: 3000,  // Higher tolerance
            max_error_rate: 0.15,
            ..Default::default()
        },
        max_attempts: 5,
    },
    cost_limits: Some(CostLimits {
        daily_budget_usd: 100.0,
        alert_at_percent: 80,
    }),
};
```

**Startup with local model fallback:**
```rust
let config = ColonyResources {
    endpoints: vec![
        // Cloud primary
        EndpointConfig {
            id: "anthropic-direct".into(),
            provider: "anthropic".into(),
            model: "claude-3-5-sonnet".into(),
            priority: 1,
            ..
        },
        // Local fallback (Ollama/vLLM)
        EndpointConfig {
            id: "local-llama".into(),
            provider: "local".into(),
            model: "llama-3-70b".into(),
            priority: 100,  // Only when cloud fails
            capabilities: vec!["offline".into(), "free".into()],
            ..
        },
    ],
    failover: FailoverConfig {
        rules: vec![
            FailoverRule {
                name: "cloud-down-local".into(),
                condition: FailoverCondition::HighErrorRate { threshold: 0.5 },
                action: FailoverAction::WithCapability { capability: "offline".into() },
            },
        ],
        ..
    },
    ..
};
```

### Colony Resource Discovery & Sharing

Apps broadcast their available resources. Other apps discover and request access.

**Generic Framework:** The resource discovery system is designed to handle ANY resource type.
LLM endpoints are the first implementation, but the same patterns apply to:
- **Compute:** GPU clusters, CPU pools, container runtimes
- **Storage:** Shared filesystems, object storage, databases
- **Services:** Vector DBs, embedding services, search indices
- **Data:** Training datasets, model weights, cached results

```rust
/// Generic resource type - LLM is just one variant
pub enum ResourceType {
    /// LLM inference endpoints (first implementation)
    Llm(LlmResourceInfo),

    /// GPU compute resources (future)
    Gpu(GpuResourceInfo),

    /// Storage resources (future)
    Storage(StorageResourceInfo),

    /// Vector database (future)
    VectorDb(VectorDbResourceInfo),

    /// Generic service (future)
    Service(ServiceResourceInfo),

    /// Custom resource type
    Custom { type_name: String, info: serde_json::Value },
}

/// Resource advertisement broadcast by each app
pub struct ResourceAdvertisement {
    /// App identity
    pub app_id: Uuid,
    pub app_name: String,

    /// Available resources (any type)
    pub resources: Vec<AdvertisedResource>,

    /// Sharing policy
    pub sharing: SharingPolicy,

    /// Last updated
    pub timestamp: DateTime<Utc>,
}

/// A single advertised resource
pub struct AdvertisedResource {
    /// Unique identifier
    pub id: String,

    /// Resource type and type-specific info
    pub resource_type: ResourceType,

    /// Generic capabilities (for filtering)
    pub capabilities: Vec<String>,

    /// Is this resource available for sharing?
    pub shareable: bool,

    /// Health status
    pub healthy: bool,

    /// Type-specific metrics (latency, capacity, etc.)
    pub metrics: serde_json::Value,
}

// === LLM-SPECIFIC (First Implementation) ===

pub struct LlmResourceInfo {
    pub provider: String,
    pub account: Option<String>,
    pub region: Option<String>,
    pub model: String,
    pub rate_limit_remaining: Option<u32>,
    pub latency_ms: Option<u32>,
    pub cost_per_1k: Option<CostPer1k>,
}

// === FUTURE: GPU Resources ===

pub struct GpuResourceInfo {
    pub gpu_type: String,           // "A100", "H100", "RTX4090"
    pub vram_gb: u32,
    pub available_slots: u32,
    pub cuda_version: String,
}

// === FUTURE: Storage Resources ===

pub struct StorageResourceInfo {
    pub storage_type: String,       // "s3", "gcs", "local", "nfs"
    pub available_gb: u64,
    pub read_throughput_mbps: u32,
    pub write_throughput_mbps: u32,
}

// === FUTURE: Vector DB Resources ===

pub struct VectorDbResourceInfo {
    pub db_type: String,            // "pinecone", "weaviate", "qdrant", "pgvector"
    pub dimensions: u32,
    pub index_count: u64,
    pub queries_per_sec: u32,
}

pub struct AdvertisedEndpoint {
    /// Endpoint identifier
    pub id: String,

    /// Provider info
    pub provider: String,
    pub account: Option<String>,  // May be redacted for security
    pub region: Option<String>,
    pub model: String,

    /// Capabilities
    pub capabilities: Vec<String>,

    /// Current health (shared with colony)
    pub healthy: bool,
    pub latency_ms: Option<u32>,
    pub rate_limit_remaining: Option<u32>,

    /// Is this endpoint available for sharing?
    pub shareable: bool,

    /// Cost per 1K tokens (if sharing is metered)
    pub cost_per_1k: Option<CostPer1k>,
}

pub enum SharingPolicy {
    /// No sharing - endpoints are private
    Private,

    /// Share with any colony member
    ColonyOpen,

    /// Share with specific apps only
    AllowList { app_ids: Vec<Uuid> },

    /// Share but require approval for each request
    RequestApproval,

    /// Share with metered billing
    Metered { rate_per_1k: f64 },
}
```

### Broadcasting Resources

```rust
// Apps automatically broadcast their resources on join and periodically
impl DashflowNetwork {
    /// Broadcast available resources to colony (automatic on join)
    pub async fn advertise_resources(&self) -> Result<()> {
        let advertisement = ResourceAdvertisement {
            app_id: self.identity.id,
            app_name: self.identity.name.clone(),
            endpoints: self.collect_shareable_endpoints(),
            sharing: self.config.sharing_policy.clone(),
            timestamp: Utc::now(),
        };

        self.broadcast("_resources", advertisement, Priority::Background).await
    }
}

// Periodic broadcast (every 60 seconds or on significant change)
network.on_interval(Duration::seconds(60), |net| {
    net.advertise_resources()
});

// Broadcast immediately when health changes significantly
network.on_endpoint_health_change(|endpoint, old_health, new_health| {
    if old_health.healthy != new_health.healthy {
        network.advertise_resources()
    }
});
```

### Discovering Colony Resources

```rust
/// Aggregated view of all resources in the colony
pub struct ColonyResourceRegistry {
    /// Resources by app
    apps: HashMap<Uuid, ResourceAdvertisement>,

    /// Aggregated endpoints (deduplicated by provider/model)
    endpoints: HashMap<EndpointKey, Vec<EndpointSource>>,

    /// Last full sync
    last_sync: DateTime<Utc>,
}

pub struct EndpointSource {
    pub app_id: Uuid,
    pub app_name: String,
    pub endpoint: AdvertisedEndpoint,
    pub shareable: bool,
}

impl ColonyResourceRegistry {
    /// Get all available models across the colony
    pub fn available_models(&self) -> Vec<ModelInfo> {
        // Returns: claude-3-5-sonnet (3 sources), claude-3-haiku (2 sources), gpt-4 (1 source)
    }

    /// Get all endpoints for a specific model
    pub fn endpoints_for_model(&self, model: &str) -> Vec<&EndpointSource> {
        self.endpoints
            .iter()
            .filter(|(k, _)| k.model == model)
            .flat_map(|(_, sources)| sources.iter())
            .collect()
    }

    /// Get healthiest endpoint for a model
    pub fn healthiest_endpoint(&self, model: &str) -> Option<&EndpointSource> {
        self.endpoints_for_model(model)
            .into_iter()
            .filter(|s| s.endpoint.healthy && s.shareable)
            .min_by_key(|s| s.endpoint.latency_ms.unwrap_or(u32::MAX))
    }

    /// Get all providers in the colony
    pub fn providers(&self) -> Vec<ProviderSummary> {
        // Returns: aws_bedrock (5 endpoints, 2 apps), openai (2 endpoints, 1 app)
    }
}

// Query colony resources
let registry = network.resource_registry();

// What models are available?
let models = registry.available_models();
// [
//   { model: "claude-3-5-sonnet", sources: 3, healthy: 2, providers: ["aws_bedrock"] },
//   { model: "claude-3-haiku", sources: 2, healthy: 2, providers: ["aws_bedrock"] },
//   { model: "gpt-4-turbo", sources: 1, healthy: 1, providers: ["openai"] },
// ]

// Who has claude-3-5-sonnet?
let sources = registry.endpoints_for_model("claude-3-5-sonnet");
// [
//   { app: "CodeAgent", endpoint: "bedrock-east", healthy: true, latency: 420ms, shareable: true },
//   { app: "TestRunner", endpoint: "bedrock-west", healthy: true, latency: 380ms, shareable: true },
//   { app: "Optimizer", endpoint: "anthropic-direct", healthy: false, shareable: false },
// ]
```

### Requesting Access to Shared Resources

```rust
/// Request to use another app's endpoint
pub struct ResourceRequest {
    pub requester: Uuid,
    pub target_app: Uuid,
    pub endpoint_id: String,
    pub purpose: String,
    pub estimated_requests: Option<u32>,
    pub duration: Option<Duration>,
}

pub enum ResourceResponse {
    /// Access granted
    Granted {
        /// Token for accessing the endpoint
        access_token: String,
        /// Expiry time
        expires_at: DateTime<Utc>,
        /// Proxy endpoint to use
        proxy_url: String,
    },

    /// Access denied
    Denied { reason: String },

    /// Requires approval (async)
    PendingApproval { request_id: Uuid },

    /// Endpoint not available
    Unavailable { reason: String },
}

// Request access to another app's endpoint
let request = ResourceRequest {
    requester: my_app_id,
    target_app: other_app_id,
    endpoint_id: "bedrock-west".into(),
    purpose: "Running optimization batch".into(),
    estimated_requests: Some(100),
    duration: Some(Duration::hours(1)),
};

let response = network.request_resource_access(request).await?;

match response {
    ResourceResponse::Granted { access_token, proxy_url, .. } => {
        // Use the endpoint via proxy
        let client = LlmClient::with_proxy(proxy_url, access_token);
        let result = client.complete(prompt).await?;
    }
    ResourceResponse::Denied { reason } => {
        log::warn!("Access denied: {}", reason);
        // Fall back to own endpoints
    }
    ResourceResponse::PendingApproval { request_id } => {
        // Wait for approval or use fallback
    }
    _ => {}
}
```

### Resource Proxy (for shared endpoints)

```rust
/// Proxy server for sharing endpoints securely
/// The owning app runs this to allow access without sharing credentials
pub struct ResourceProxy {
    /// Endpoints being proxied
    endpoints: HashMap<String, ProxiedEndpoint>,

    /// Active access grants
    grants: HashMap<String, AccessGrant>,
}

pub struct ProxiedEndpoint {
    /// The actual endpoint config (with credentials)
    endpoint: EndpointConfig,

    /// Rate limit for proxied requests
    rate_limit: RateLimit,

    /// Usage tracking
    usage: UsageTracker,
}

pub struct AccessGrant {
    /// Who has access
    app_id: Uuid,

    /// Which endpoint
    endpoint_id: String,

    /// Token for authentication
    token: String,

    /// Expiry
    expires_at: DateTime<Utc>,

    /// Usage limits
    max_requests: Option<u32>,
    requests_used: u32,
}

impl ResourceProxy {
    /// Handle proxied request
    pub async fn handle_request(
        &self,
        token: &str,
        endpoint_id: &str,
        request: LlmRequest,
    ) -> Result<LlmResponse> {
        // Validate token
        let grant = self.grants.get(token).ok_or(AccessError::InvalidToken)?;

        if grant.is_expired() {
            return Err(AccessError::TokenExpired);
        }

        if grant.requests_used >= grant.max_requests.unwrap_or(u32::MAX) {
            return Err(AccessError::QuotaExceeded);
        }

        // Forward request to actual endpoint
        let endpoint = self.endpoints.get(endpoint_id)?;
        let response = endpoint.forward_request(request).await?;

        // Track usage
        self.grants.get_mut(token).unwrap().requests_used += 1;

        Ok(response)
    }
}
```

### Network Channel: `_resources`

```rust
// Apps subscribe to resource advertisements
network.subscribe("_resources").await?;

network.on_message(|msg| {
    if msg.channel == "_resources" {
        let advertisement: ResourceAdvertisement = msg.parse()?;
        registry.update(msg.from, advertisement);
    }
});

// Request resources via direct message
network.send_to(target_app, "_resource_request", ResourceRequest { .. }).await?;
```

### Example: Dynamic Resource Discovery

```rust
// App starts with minimal config
let network = DashflowNetwork::join(AppConfig {
    name: "NewWorker",
    // No LLM endpoints configured locally!
}).await?;

// Discover what's available in the colony
let registry = network.resource_registry();

if let Some(source) = registry.healthiest_endpoint("claude-3-5-sonnet") {
    if source.shareable {
        // Request access to the healthiest endpoint
        let response = network.request_resource_access(ResourceRequest {
            target_app: source.app_id,
            endpoint_id: source.endpoint.id.clone(),
            purpose: "Code analysis".into(),
            ..
        }).await?;

        if let ResourceResponse::Granted { proxy_url, access_token, .. } = response {
            // Now we can use Claude without configuring our own credentials!
            let client = LlmClient::with_proxy(proxy_url, access_token);
            let result = client.complete("Analyze this code...").await?;
        }
    }
}
```

### Security Considerations

```rust
/// Resource sharing security config
pub struct ResourceSecurityConfig {
    /// Require authentication for resource discovery
    pub require_auth_for_discovery: bool,

    /// Redact sensitive info from advertisements
    pub redact_account_ids: bool,

    /// Encrypt proxy traffic
    pub encrypt_proxy: bool,

    /// Audit log all access
    pub audit_access: bool,

    /// Auto-revoke grants after inactivity
    pub auto_revoke_after: Duration,
}
```

### Configuration File Format

Colonies can define resources in a config file:

```toml
# .dashflow/colony.toml

[resources]

[[resources.endpoints]]
id = "bedrock-primary"
provider = "aws_bedrock"
account = "prod-1"
region = "us-east-1"
model = "claude-3-5-sonnet"
credentials = { env = "AWS_CREDS_PROD1" }
priority = 1
capabilities = ["reasoning", "coding"]

[[resources.endpoints]]
id = "bedrock-secondary"
provider = "aws_bedrock"
account = "prod-2"
region = "us-west-2"
model = "claude-3-5-sonnet"
credentials = { env = "AWS_CREDS_PROD2" }
priority = 2

[[resources.endpoints]]
id = "haiku-cheap"
provider = "aws_bedrock"
model = "claude-3-haiku"
priority = 10
capabilities = ["fast", "cheap"]

[failover]
strategy = "rule_based"
max_attempts = 4

[[failover.rules]]
name = "budget-exceeded"
condition = { type = "budget_exceeded" }
action = { type = "with_capability", capability = "cheap" }

[[failover.rules]]
name = "high-latency"
condition = { type = "high_latency", threshold_ms = 2500 }
action = { type = "next_by_priority" }

[failover.health_thresholds]
max_avg_latency_ms = 2500
max_consecutive_failures = 3
max_error_rate = 0.12

[cost_limits]
daily_budget_usd = 50.0
alert_at_percent = 75
```

### Colony Health Dashboard

```rust
let health_report = colony.llm_health_report().await?;

// LlmHealthReport {
//     healthy_endpoints: 5,
//     unhealthy_endpoints: 2,
//     degraded_endpoints: 1,  // High latency but working
//
//     issues: [
//         { endpoint: "aws_bedrock/prod-2/us-east-1/claude-3-5-sonnet",
//           issue: "High error rate (15%)",
//           since: "2025-12-09T18:30:00Z",
//           colony_apps_affected: 3 },
//         { endpoint: "openai/prod/gpt-4-turbo",
//           issue: "Timeout (3 consecutive)",
//           since: "2025-12-09T18:45:00Z",
//           colony_apps_affected: 1 },
//     ],
//
//     recommendations: [
//         "Shift traffic from aws_bedrock/prod-2 to aws_bedrock/prod-1",
//         "Consider temporary fallback to claude-3-haiku for latency-sensitive tasks",
//     ],
// }
```

### Real-Time Colony Dashboard

```rust
// Query colony-wide LLM status
let colony_llm_status = colony.llm_status().await?;

// ColonyLlmStatus {
//     endpoints: [
//         { provider: "aws_bedrock", account: "prod-1", region: "us-east-1",
//           model: "claude-3-5-sonnet", remaining: 850, colony_load: 3 apps },
//         { provider: "aws_bedrock", account: "prod-1", region: "us-west-2",
//           model: "claude-3-5-sonnet", remaining: 990, colony_load: 1 app },
//         { provider: "aws_bedrock", account: "prod-2", region: "us-east-1",
//           model: "claude-3-5-sonnet", remaining: 200, colony_load: 5 apps,
//           SATURATED: true },
//     ],
//     recommendations: [
//         "Switch colony apps from prod-2/us-east-1 to prod-1/us-west-2",
//         "Account prod-2 nearing daily quota (82% used)",
//     ],
//     total_capacity: {
//         claude_3_5_sonnet: { remaining: 2040, healthy_endpoints: 2 },
//         claude_3_haiku: { remaining: 5000, healthy_endpoints: 3 },
//     },
// }
```

### 2. Architecture Awareness

Apps understand the system topology:

```rust
let topology = system.topology()?;
// SystemTopology {
//     hostname: "workstation-1",
//     os: "linux",
//     architecture: "x86_64",
//     numa_nodes: 2,
//     network_interfaces: [...],
//     container_runtime: Some("docker"),
// }

// Understand deployment options
let options = topology.spawn_options()?;
// [SpawnOption::Process, SpawnOption::Docker, SpawnOption::Kubernetes]
```

### 3. Organic Spawning

Apps can spawn new instances:

```rust
// Spawn a worker with specific task
let worker = colony.spawn(SpawnConfig {
    template: SpawnTemplate::Self_,  // Clone of current app
    task: Task::RunTests { crate_name: "dashflow-openai" },
    resources: ResourceLimits {
        max_cpu_cores: 2,
        max_memory_mb: 4096,
        max_duration: Duration::hours(1),
    },
    auto_terminate: true,  // Terminate when task complete
}).await?;

// Worker joins the network automatically
// Parent receives updates via network coordination
```

### 4. Spawn Templates

```rust
pub enum SpawnTemplate {
    Self_,              // Clone of current app
    Named(String),      // Named template from registry
    Custom(AppConfig),  // Custom configuration
}

// Pre-defined templates
const TEMPLATES: &[&str] = &[
    "test-runner",      // Runs tests
    "optimizer",        // Runs optimization
    "analyzer",         // Analyzes code
    "builder",          // Builds projects
];
```

---

## Safety Constraints

### Resource Limits

```rust
// Global limits (configurable)
const MAX_SPAWNED_WORKERS: usize = 10;
const MAX_CPU_USAGE_PERCENT: f32 = 80.0;
const MAX_MEMORY_USAGE_PERCENT: f32 = 80.0;
const MIN_RESERVED_MEMORY_MB: usize = 4096;  // Always keep 4GB free
```

### Spawn Approval

```rust
pub enum SpawnApproval {
    Automatic,         // Spawn within limits (default)
    RequireConfirm,    // Ask user before spawning
    Disabled,          // No spawning allowed
}

// Configure per-app
let colony = Colony::new(ColonyConfig {
    approval: SpawnApproval::Automatic,
    max_workers: 5,
    resource_limits: ResourceLimits::default(),
})?;
```

### Termination Policy

```rust
pub enum TerminationPolicy {
    WhenTaskComplete,   // Terminate after task done
    WhenIdle(Duration), // Terminate after idle period
    Manual,             // Only terminate on explicit command
    WhenParentExits,    // Terminate if parent dies
}
```

---

## Worker Lifecycle

```
┌─────────┐    ┌─────────┐    ┌─────────┐    ┌─────────┐    ┌─────────┐
│ Planned │───►│Spawning │───►│ Running │───►│Finishing│───►│Terminated│
└─────────┘    └─────────┘    └─────────┘    └─────────┘    └─────────┘
     │              │              │              │
     │              │              │              │
     ▼              ▼              ▼              ▼
  Resource      Process        Network        Task          Cleanup
  check         start          join           complete      resources
```

### State Machine

```rust
pub enum WorkerState {
    Planned { config: SpawnConfig },
    Spawning { started_at: Instant },
    Running { pid: u32, network_id: Uuid },
    Finishing { result: TaskResult },
    Terminated { exit_code: i32, duration: Duration },
    Failed { error: String },
}
```

---

## Communication with Spawned Workers

Workers join the network automatically and communicate via standard channels:

```rust
// Parent spawns worker
let worker = colony.spawn(config).await?;

// Worker joins network, announces itself
// Parent receives on _presence channel:
// { "event": "join", "spawned_by": parent_id, "task": "RunTests" }

// Worker sends progress updates on _status
// { "task_progress": 0.5, "tests_passed": 42, "tests_failed": 2 }

// When done, worker sends result
// { "task_complete": true, "result": { ... } }

// Worker terminates, parent notified via _presence
// { "event": "leave", "reason": "task_complete" }
```

---

## API

### Resource Introspection

```rust
// Get current resources
let resources = colony.resources().await?;

// Get system topology
let topology = colony.topology().await?;

// Check spawn viability
let viable = colony.can_spawn(requirements)?;
```

### Spawning Workers

```rust
// Spawn from template
let worker = colony.spawn(SpawnConfig {
    template: SpawnTemplate::Named("test-runner"),
    task: Task::Custom(json!({ "crate": "dashflow-openai" })),
    ..Default::default()
}).await?;

// Spawn clone of self with task
let worker = colony.spawn_self(Task::Optimize {
    target: "grpo",
    iterations: 100,
}).await?;

// Spawn multiple workers (like sending out scouts)
let workers = colony.spawn_batch(vec![
    SpawnConfig { task: Task::Test("crate-a"), .. },
    SpawnConfig { task: Task::Test("crate-b"), .. },
    SpawnConfig { task: Task::Test("crate-c"), .. },
]).await?;
```

### Managing Workers

```rust
// List active workers
let workers = colony.workers().await?;

// Get worker status
let status = colony.worker_status(worker_id).await?;

// Terminate worker
colony.terminate(worker_id).await?;

// Terminate all workers
colony.terminate_all().await?;
```

### Receiving Results

```rust
// Wait for worker to complete
let result = worker.wait().await?;

// Or receive via network
network.on_message(|msg| {
    if msg.from == worker_id && msg.topic == "task_complete" {
        handle_result(msg.payload);
    }
});
```

---

## Deployment Options

### Process (Default)

```rust
// Spawn as child process
let worker = colony.spawn(SpawnConfig {
    deployment: Deployment::Process,
    ..
}).await?;
```

### Container (Optional)

```rust
// Spawn in Docker container (isolated)
let worker = colony.spawn(SpawnConfig {
    deployment: Deployment::Docker {
        image: "dashflow:latest",
        network: "host",
    },
    ..
}).await?;
```

### Kubernetes (Future)

```rust
// Spawn as Kubernetes job
let worker = colony.spawn(SpawnConfig {
    deployment: Deployment::Kubernetes {
        namespace: "dashflow",
        service_account: "worker",
    },
    ..
}).await?;
```

---

## Use Cases

### 1. Parallel Test Running

```rust
// Split tests across workers
let crates = vec!["dashflow-openai", "dashflow-anthropic", "dashflow"];
let workers = colony.spawn_batch(
    crates.iter().map(|c| SpawnConfig {
        template: SpawnTemplate::Named("test-runner"),
        task: Task::Test(c.to_string()),
        ..
    }).collect()
).await?;

// Collect results
let results = futures::future::join_all(
    workers.iter().map(|w| w.wait())
).await;
```

### 2. Distributed Optimization

```rust
// Spawn optimization workers with different configs
for config in optimization_configs {
    colony.spawn(SpawnConfig {
        template: SpawnTemplate::Named("optimizer"),
        task: Task::Optimize(config),
        ..
    }).await?;
}

// Best result reported via network
```

### 3. Build Farm

```rust
// Spawn builders when resources available
if colony.can_spawn(builder_requirements)? {
    colony.spawn(SpawnConfig {
        template: SpawnTemplate::Named("builder"),
        task: Task::Build { target: "release" },
        ..
    }).await?;
}
```

### 4. Swarm Intelligence

```rust
// Spawn multiple agents for collaborative problem-solving
let agents = colony.spawn_batch((0..5).map(|i| SpawnConfig {
    template: SpawnTemplate::Self_,
    task: Task::Explore {
        search_space: search_space.partition(i, 5),
    },
    ..
}).collect()).await?;

// Agents coordinate via network, share discoveries
```

---

## Implementation Phases

| Phase | Commit | Description | Status |
|-------|--------|-------------|--------|
| 1 | N=344 | System introspection: CPU, memory, disk, topology | ✅ COMPLETE |
| 2 | N=345 | Spawn config, templates, resource limits | ✅ COMPLETE |
| 3 | N=346 | Process spawning, lifecycle management | ✅ COMPLETE |
| 4 | N=347 | Network integration, result collection | ✅ COMPLETE |
| 5 | N=562 | Docker/container support (optional) | ✅ COMPLETE |

### Phase 1 (N=344) - COMPLETE
- `colony/mod.rs` - Module structure and exports
- `colony/types.rs` - Core types:
  - `ResourceSnapshot` - Snapshot of available system resources
  - `SpawnRequirements` - Requirements for spawning (minimal/standard/heavy presets)
  - `GpuInfo`, `MemoryInfo`, `DiskInfo` - Structured resource info
  - `LlmServiceStats` - LLM provider statistics with builder pattern
  - `CostPer1k` - Cost per 1K tokens
  - `TotalLlmCapacity` - Aggregated LLM capacity
- `colony/topology.rs` - System topology:
  - `SystemTopology` - Hostname, OS, architecture, NUMA nodes, network interfaces
  - `NumaNode` - NUMA node info (CPU cores, memory)
  - `NetworkInterface` - Network interface details
  - `ContainerRuntime` - Docker/Podman detection
  - `SpawnOption` - Process/Docker/Kubernetes options
  - `DeploymentOption` - Deployment preferences
- `colony/system.rs` - System monitoring:
  - `SystemMonitor` - Real-time resource monitoring with caching
  - `SystemMonitorConfig` - Configuration (refresh interval, reserved resources)
  - CPU, memory detection for Linux and macOS
  - Container runtime detection (Docker, Podman)
  - Kubernetes detection via environment variable
  - LLM stats registration and retrieval
- 25 new tests (6066 total lib tests)
- 0 clippy warnings

### Phase 2 (N=345) - COMPLETE
- `colony/config.rs` - Spawn configuration:
  - `SpawnConfig` - Full spawn configuration with builder pattern
  - `SpawnTemplate` - Self_, Named, or Custom templates
  - `ResourceLimits` - Min/max CPU, memory, disk, duration, GPU, filesystem access
  - `Task` - Idle, RunTests, Build, Optimize, Analyze, Command, Custom
  - `TerminationPolicy` - WhenTaskComplete, WhenIdle, Manual, WhenParentExits, AfterDuration
  - `SpawnApproval` - Automatic, RequireConfirm, Disabled
  - `ColonyConfig` - Global colony limits and allowed templates
  - `AppConfig` - Custom app configuration with args, env, working dir
  - `DockerConfig` - Docker-specific settings (image, network, volumes, ports)
  - `FilesystemAccess` - None, ReadOnly, ReadWrite, Full
  - `AnalysisType` - Static, Security, Performance, Dependencies, Quality
- `colony/templates.rs` - Template registry:
  - `TemplateRegistry` - Registry with lookup by name, tag, capability
  - `TemplateDefinition` - Template with resources, capabilities, termination policy
  - Predefined templates: test-runner, builder, optimizer, analyzer, worker, scout, benchmarker, doc-generator, linter, watcher
  - Quick helpers: `quick::test_runner()`, `quick::builder()`, `quick::optimizer()`, etc.
- Updated `colony/mod.rs` with new exports
- 25 new tests (6091 total lib tests)
- 0 clippy warnings

### Phase 3 (N=346) - COMPLETE
- `colony/spawner.rs` - Process spawning and lifecycle:
  - `WorkerState` - State machine enum (Planned, Spawning, Running, Finishing, Terminated, Failed)
  - `TaskResult` - Result of completed tasks with success, exit code, stdout/stderr
  - `Worker` - Worker instance with state, process handle, and configuration
  - `Spawner` - Creates workers with resource checking and template validation
  - `WorkerManager` - Manages multiple workers, spawn/terminate/wait operations
  - `WorkerInfo` - Serializable summary of worker state for external use
  - `SpawnError` - Error types for spawn operations
- Process spawning:
  - Spawn current executable (`SpawnTemplate::Self_`)
  - Spawn named templates (`SpawnTemplate::Named`)
  - Spawn custom applications (`SpawnTemplate::Custom`)
  - Docker container spawning (when runtime available)
  - Environment variable inheritance
  - Worker-specific environment (DASHFLOW_WORKER_ID, DASHFLOW_WORKER_MODE, DASHFLOW_TASK)
- Lifecycle management:
  - State transitions: Planned → Spawning → Running → Finishing → Terminated
  - Progress tracking (0.0 - 1.0)
  - Process status checking via try_wait()
  - Worker termination (individual and all)
  - Cleanup of old terminated workers
- Resource validation:
  - Check spawn requirements before spawning
  - Template allowlist/blocklist checking
  - Spawn approval mode (Automatic, RequireConfirm, Disabled)
  - Worker limit enforcement
- Updated `colony/mod.rs` with new exports
- 63 colony tests total
- 0 clippy warnings

### Phase 4 (N=347) - COMPLETE
- `colony/network_integration.rs` - Network integration for colony workers:
  - `WorkerMessage` - Enum for messages between parent and workers:
    - `WorkerJoined` - Worker announces network connection
    - `Progress` - Worker reports progress (0.0-1.0) with optional status
    - `TaskResult` - Worker sends completion result
    - `WorkerLeaving` - Worker announces departure
    - `WorkerError` - Worker reports errors (fatal/non-fatal)
    - `StatusRequest`/`StatusResponse` - Status query protocol
  - `NetworkedWorkerInfo` - Extended WorkerInfo with network status
  - `NetworkedSpawner` - Spawner that configures workers for network
  - `NetworkedWorkerManager` - Manages workers with network coordination
  - Worker-side helper functions:
    - `is_worker_mode()` - Check if running as worker
    - `worker_id()` - Get worker UUID from env
    - `parent_peer_id()` - Get parent's peer ID
    - `worker_task()` - Get assigned task
    - `network_enabled()` - Check if network is enabled
    - `report_progress()` - Send progress to parent
    - `report_result()` - Send task result to parent
    - `announce_worker_joined()` - Announce network connection
- Standard worker channels:
  - `WORKER_CHANNEL` (_workers) - General worker messages
  - `WORKER_JOIN_CHANNEL` (_worker_join) - Join announcements
  - `WORKER_RESULT_CHANNEL` (_worker_result) - Result delivery
- Updated `network/types.rs` with new channels and Channel constructors
- Updated `spawner.rs` with pub(crate) transition methods
- Updated `colony/mod.rs` with network integration exports
- 6 new network_integration tests (69 colony tests total)
- 0 clippy warnings (with network feature)

### Phase 5 (N=562) - COMPLETE
Docker/container support implementation:
- `colony/config.rs` - Docker configuration types:
  - `DockerConfig` - Image, tag, network, volumes, ports, privileged mode
  - `VolumeMount` - Host/container path mappings with read-only option
  - `PortMapping` - TCP/UDP port mappings
  - `DockerNetwork` - Host, Bridge, None modes
  - Builder pattern for DockerConfig
- `colony/spawner.rs` - Docker spawning:
  - `spawn_docker()` - Spawns workers in Docker containers
  - Resource limits passed via --cpus and --memory flags
  - Environment variable injection
  - Volume and port mapping
  - Automatic fallback to process spawning when Docker unavailable
- `colony/system.rs` - Container runtime detection:
  - `detect_container_runtime()` - Detects Docker and Podman
  - Socket path detection (/var/run/docker.sock, podman socket)
- `colony/topology.rs` - Container runtime types:
  - `ContainerRuntime` - Runtime type, version, availability, socket path
  - `SpawnOption` includes Docker variant
- Tests for DockerConfig builder pattern
- Integration with DeploymentOption::Containerized flow

---

## Success Criteria

- [x] Apps can query system resources (Phase 1)
- [x] Apps can spawn worker processes (Phase 3)
- [x] Workers join network automatically (Phase 4)
- [x] Resource limits enforced (Phase 3)
- [x] Parent receives worker results (Phase 4)
- [x] Workers terminate cleanly (Phase 3)
- [x] Docker/Podman runtime detection (Phase 5)
- [x] Docker container spawning (Phase 5)
- [x] All existing tests pass
- [x] 0 clippy warnings

---

## Security Considerations

1. **Resource exhaustion**: Limits prevent runaway spawning
2. **Process isolation**: Workers run in separate processes
3. **Network isolation**: Optional container isolation
4. **Permission inheritance**: Workers get parent's permissions
5. **Audit trail**: All spawns logged

---

## Version History

| Date | Change | Author |
|------|--------|--------|
| 2025-12-09 | Initial design | MANAGER |
