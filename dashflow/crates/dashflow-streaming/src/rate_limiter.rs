// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

//! Per-tenant rate limiting using token bucket algorithm
//!
//! Prevents DoS by enforcing per-tenant quotas for message production.

use std::sync::LazyLock;
use prometheus::{CounterVec, HistogramOpts, HistogramVec, Opts};
use std::borrow::Cow;
use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tracing::warn;

// M-98: Counter metrics include _total suffix for Prometheus naming convention
// M-624: Use centralized constants
use crate::metrics_constants::{
    METRIC_RATE_LIMITER_REDIS_ERRORS_TOTAL, METRIC_RATE_LIMITER_REDIS_LATENCY_MS,
    METRIC_RATE_LIMIT_ALLOWED_TOTAL, METRIC_RATE_LIMIT_EXCEEDED_TOTAL,
};

static RATE_LIMIT_EXCEEDED: LazyLock<CounterVec> = LazyLock::new(|| {
    crate::metrics_utils::counter_vec(
        Opts::new(
            METRIC_RATE_LIMIT_EXCEEDED_TOTAL,
            "Total messages rejected due to rate limiting",
        ),
        &["tenant_id"],
    )
});
static RATE_LIMIT_ALLOWED: LazyLock<CounterVec> = LazyLock::new(|| {
    crate::metrics_utils::counter_vec(
        Opts::new(
            METRIC_RATE_LIMIT_ALLOWED_TOTAL,
            "Total messages allowed by rate limiter",
        ),
        &["tenant_id"],
    )
});

// Redis-specific metrics (M-647: component-scoped to avoid conflicts with websocket_server)
static REDIS_CONNECTION_ERRORS: LazyLock<CounterVec> = LazyLock::new(|| {
    crate::metrics_utils::counter_vec(
        Opts::new(
            METRIC_RATE_LIMITER_REDIS_ERRORS_TOTAL,
            "Total Redis connection errors in rate limiting",
        ),
        &["operation"],
    )
});

static REDIS_OPERATION_LATENCY: LazyLock<HistogramVec> = LazyLock::new(|| {
    crate::metrics_utils::histogram_vec(
        HistogramOpts::new(
            METRIC_RATE_LIMITER_REDIS_LATENCY_MS,
            "Redis operation latency in milliseconds for rate limiting",
        )
        .buckets(vec![
            0.1, 0.5, 1.0, 2.0, 5.0, 10.0, 20.0, 50.0, 100.0, 200.0, 500.0,
        ]),
        &["operation"],
    )
});

/// Log rate for Redis fallback warnings.
static REDIS_FALLBACK_LOG_COUNT: AtomicU64 = AtomicU64::new(0);

const MAX_SAFE_TENANT_LABEL_LEN: usize = 64;

fn is_safe_tenant_label(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_SAFE_TENANT_LABEL_LEN
        && value.bytes().all(|b| {
            matches!(
                b,
                b'a'..=b'z'
                    | b'A'..=b'Z'
                    | b'0'..=b'9'
                    | b'-'
                    | b'_'
                    | b'.'
            )
        })
}

fn tenant_label_value<'a>(tenant_id: &'a str) -> Cow<'a, str> {
    if is_safe_tenant_label(tenant_id) {
        return Cow::Borrowed(tenant_id);
    }

    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(tenant_id.as_bytes());
    let digest = hasher.finalize();
    let digest_hex = hex::encode(digest);
    Cow::Owned(format!("tenant_{}", &digest_hex[..12]))
}

fn normalize_rate_limit(mut limit: RateLimit) -> RateLimit {
    if !limit.messages_per_second.is_finite() || limit.messages_per_second < 0.0 {
        limit.messages_per_second = 0.0;
    }
    if limit.burst_capacity == 0 && limit.messages_per_second > 0.0 {
        limit.burst_capacity = 1;
    }
    limit
}

/// Rate limit configuration
#[derive(Debug, Clone)]
pub struct RateLimit {
    /// Maximum messages per second
    pub messages_per_second: f64,

    /// Burst capacity (max tokens)
    pub burst_capacity: u64,
}

impl Default for RateLimit {
    fn default() -> Self {
        Self {
            messages_per_second: 100.0, // 100 msg/sec default
            burst_capacity: 1000,       // Allow bursts up to 1000
        }
    }
}

/// Token bucket for rate limiting
#[derive(Debug)]
struct TokenBucket {
    capacity: u64,
    tokens: f64,
    last_refill: Instant,
    refill_rate: f64, // tokens per second
    /// S-16: Track last access time for LRU eviction
    last_access: Instant,
}

impl TokenBucket {
    fn new(capacity: u64, refill_rate: f64) -> Self {
        let refill_rate = if refill_rate.is_finite() && refill_rate >= 0.0 {
            refill_rate
        } else {
            0.0
        };
        let now = Instant::now();
        Self {
            capacity,
            tokens: capacity as f64, // Start full
            last_refill: now,
            refill_rate,
            last_access: now, // S-16: Initialize last access
        }
    }

    fn refill(&mut self) {
        let now = Instant::now();
        let elapsed = now.duration_since(self.last_refill).as_secs_f64();

        // Add tokens based on elapsed time
        let new_tokens = elapsed * self.refill_rate;
        let updated = self.tokens + new_tokens;
        self.tokens = updated.clamp(0.0, self.capacity as f64);
        self.last_refill = now;
    }

    fn try_consume(&mut self, count: u64) -> bool {
        self.refill();
        self.last_access = Instant::now(); // S-16: Update last access for LRU

        if self.tokens >= count as f64 {
            self.tokens -= count as f64;
            true
        } else {
            false
        }
    }

    fn available_tokens(&mut self) -> u64 {
        self.refill();
        self.tokens as u64
    }
}

/// Maximum number of tenants to track locally before pruning (prevents unbounded growth).
const MAX_TENANT_BUCKETS: usize = 10_000;
/// Maximum number of distinct tenant labels to emit in Prometheus.
/// New tenants beyond this are aggregated under the "overflow" label.
const MAX_TENANT_METRIC_LABELS: usize = 1000;
/// Number of entries to prune when over capacity.
const PRUNE_BATCH: usize = 1000;

/// Per-tenant rate limiter (supports both in-memory and distributed Redis modes)
pub struct TenantRateLimiter {
    /// Local token buckets (tenant_id -> bucket) - used when Redis is not available
    buckets: Arc<RwLock<HashMap<String, TokenBucket>>>,

    /// Redis connection manager for distributed rate limiting (optional)
    redis: Option<Arc<redis::aio::ConnectionManager>>,

    /// Default quota for tenants
    default_limit: RateLimit,

    /// Tenant-specific limits (tenant_id -> limit)
    custom_limits: Arc<RwLock<HashMap<String, RateLimit>>>,

    /// Track which tenant IDs we emit as labels to cap metric cardinality.
    metric_tenants: Arc<RwLock<HashSet<String>>>,
}

impl TenantRateLimiter {
    /// Create new rate limiter with default quota (in-memory mode)
    pub fn new(default_limit: RateLimit) -> Self {
        Self {
            buckets: Arc::new(RwLock::new(HashMap::new())),
            redis: None,
            default_limit,
            custom_limits: Arc::new(RwLock::new(HashMap::new())),
            metric_tenants: Arc::new(RwLock::new(HashSet::new())),
        }
    }

    /// Create new rate limiter with Redis-backed distributed rate limiting
    ///
    /// This enables rate limiting across multiple servers by storing token buckets in Redis.
    /// Uses Lua scripts for atomic token bucket operations.
    pub async fn new_with_redis(
        default_limit: RateLimit,
        redis_url: &str,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let start = Instant::now();

        let client = redis::Client::open(redis_url).inspect_err(|_e| {
            REDIS_CONNECTION_ERRORS
                .with_label_values(&["new_connection"])
                .inc();
        })?;

        let conn_manager = redis::aio::ConnectionManager::new(client)
            .await
            .inspect_err(|_e| {
                REDIS_CONNECTION_ERRORS
                    .with_label_values(&["new_connection"])
                    .inc();
            })?;

        let latency_ms = start.elapsed().as_secs_f64() * 1000.0;
        REDIS_OPERATION_LATENCY
            .with_label_values(&["new_connection"])
            .observe(latency_ms);

        Ok(Self {
            buckets: Arc::new(RwLock::new(HashMap::new())),
            redis: Some(Arc::new(conn_manager)),
            default_limit,
            custom_limits: Arc::new(RwLock::new(HashMap::new())),
            metric_tenants: Arc::new(RwLock::new(HashSet::new())),
        })
    }

    /// Set custom limit for a specific tenant
    ///
    /// Note: If capacity is exceeded, arbitrary entries are pruned (not LRU).
    /// This is acceptable for configuration data where fallback to default_limit
    /// is safe. The primary LRU eviction (S-16) is on token buckets, not configs.
    pub async fn set_tenant_limit(&self, tenant_id: String, limit: RateLimit) {
        let mut limits = self.custom_limits.write().await;
        if !limits.contains_key(&tenant_id) && limits.len() >= MAX_TENANT_BUCKETS {
            let keys: Vec<String> = limits.keys().take(PRUNE_BATCH).cloned().collect();
            for key in keys {
                limits.remove(&key);
            }
        }
        limits.insert(tenant_id, limit);
    }

    async fn metric_tenant_label<'a>(&'a self, tenant_id: &'a str) -> Cow<'a, str> {
        let label = tenant_label_value(tenant_id);
        let mut tenants = self.metric_tenants.write().await;
        if tenants.contains(tenant_id) {
            return label;
        }

        if tenants.len() < MAX_TENANT_METRIC_LABELS {
            tenants.insert(tenant_id.to_string());
            label
        } else {
            Cow::Borrowed("overflow")
        }
    }

    /// Check if tenant can send N messages
    ///
    /// Returns true if allowed, false if rate limited.
    /// Uses Redis for distributed rate limiting if available, otherwise falls back to in-memory.
    pub async fn check_rate_limit(
        &self,
        tenant_id: &str,
        count: u64,
    ) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
        let allowed = if let Some(redis) = &self.redis {
            // Distributed mode: use Redis, but fall back to local limiting on Redis errors.
            match self.check_rate_limit_redis(tenant_id, count, redis).await {
                Ok(allowed) => allowed,
                Err(e) => {
                    REDIS_CONNECTION_ERRORS
                        .with_label_values(&["fallback_to_local"])
                        .inc();

                    let n = REDIS_FALLBACK_LOG_COUNT.fetch_add(1, Ordering::Relaxed) + 1;
                    let error_string = e.to_string();
                    if n == 1 || n % 100 == 0 {
                        warn!(
                            tenant_id = tenant_id,
                            error = %error_string,
                            "Redis rate limiter unavailable; falling back to local token bucket"
                        );
                    }

                    self.check_rate_limit_local(tenant_id, count).await?
                }
            }
        } else {
            // Single-server mode: use in-memory
            self.check_rate_limit_local(tenant_id, count).await?
        };

        // Update metrics with cardinality cap.
        let label = self.metric_tenant_label(tenant_id).await;
        if allowed {
            RATE_LIMIT_ALLOWED
                .with_label_values(&[label.as_ref()])
                .inc();
        } else {
            RATE_LIMIT_EXCEEDED
                .with_label_values(&[label.as_ref()])
                .inc();
        }

        Ok(allowed)
    }

    /// Check rate limit using local in-memory token buckets
    async fn check_rate_limit_local(
        &self,
        tenant_id: &str,
        count: u64,
    ) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
        let limit = self.get_limit_for_tenant(tenant_id).await;

        let mut buckets = self.buckets.write().await;
        if !buckets.contains_key(tenant_id) && buckets.len() >= MAX_TENANT_BUCKETS {
            // S-16: LRU eviction - sort by last access time and remove oldest entries
            let mut entries_by_access: Vec<(String, Instant)> = buckets
                .iter()
                .filter(|(k, _)| k.as_str() != tenant_id)
                .map(|(k, b)| (k.clone(), b.last_access))
                .collect();
            // Sort by last_access ascending (oldest first)
            entries_by_access.sort_by_key(|(_, access)| *access);
            // Remove the oldest PRUNE_BATCH entries
            for (key, _) in entries_by_access.into_iter().take(PRUNE_BATCH) {
                buckets.remove(&key);
            }
        }

        let bucket = buckets
            .entry(tenant_id.to_string())
            .or_insert_with(|| TokenBucket::new(limit.burst_capacity, limit.messages_per_second));

        Ok(bucket.try_consume(count))
    }

    /// Check rate limit using Redis-backed distributed token bucket
    ///
    /// Uses Lua script for atomic token bucket operations across multiple servers.
    async fn check_rate_limit_redis(
        &self,
        tenant_id: &str,
        count: u64,
        redis: &Arc<redis::aio::ConnectionManager>,
    ) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
        let start = Instant::now();
        const REDIS_TIMEOUT: Duration = Duration::from_secs(2);

        let limit = self.get_limit_for_tenant(tenant_id).await;
        let key = format!("rate_limit:{}:bucket", tenant_id);
        let mut conn = redis.as_ref().clone();

        // Lua script for atomic token bucket check-and-consume
        // This ensures that multiple servers share the same rate limit
        let script = redis::Script::new(
            r#"
            local key = KEYS[1]
            local count = tonumber(ARGV[1])
            local capacity = tonumber(ARGV[2])
            local refill_rate = tonumber(ARGV[3])
            local now = tonumber(ARGV[4])

            -- Get current tokens and last refill time
            local tokens = redis.call('HGET', key, 'tokens')
            local last_refill = redis.call('HGET', key, 'last_refill')

            -- Initialize if first time
            if tokens == false then
                tokens = capacity
                last_refill = now
            else
                tokens = tonumber(tokens)
                last_refill = tonumber(last_refill)

                -- Refill tokens based on elapsed time
                local elapsed = now - last_refill
                local new_tokens = elapsed * refill_rate
                tokens = math.min(tokens + new_tokens, capacity)
            end

            -- Try to consume tokens
            if tokens >= count then
                tokens = tokens - count
                redis.call('HSET', key, 'tokens', tokens)
                redis.call('HSET', key, 'last_refill', now)
                redis.call('EXPIRE', key, 3600)  -- 1 hour TTL
                return 1  -- Allowed
            else
                return 0  -- Rate limited
            end
            "#,
        );

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_secs_f64();

        // Build the script invocation step by step to avoid temporary lifetime issues
        let mut invocation = script.key(&key);
        invocation.arg(count);
        invocation.arg(limit.burst_capacity);
        invocation.arg(limit.messages_per_second);
        invocation.arg(now);
        let invoke_fut = invocation.invoke_async(&mut conn);

        let result: Result<i32, redis::RedisError> =
            match tokio::time::timeout(REDIS_TIMEOUT, invoke_fut).await {
                Ok(res) => res,
                Err(_) => {
                    REDIS_CONNECTION_ERRORS
                        .with_label_values(&["rate_limit_check_timeout"])
                        .inc();
                    return Err(Box::new(std::io::Error::new(
                        std::io::ErrorKind::TimedOut,
                        format!("Redis rate limit check timed out after {:?}", REDIS_TIMEOUT),
                    )));
                }
            };

        // Record metrics
        let latency_ms = start.elapsed().as_secs_f64() * 1000.0;
        REDIS_OPERATION_LATENCY
            .with_label_values(&["rate_limit_check"])
            .observe(latency_ms);

        match result {
            Ok(allowed) => Ok(allowed == 1),
            Err(e) => {
                REDIS_CONNECTION_ERRORS
                    .with_label_values(&["rate_limit_check"])
                    .inc();
                Err(Box::new(e))
            }
        }
    }

    /// Get available tokens for a tenant (for monitoring)
    ///
    /// Returns current available tokens after refilling based on elapsed time.
    pub async fn available_tokens(&self, tenant_id: &str) -> u64 {
        let limit = self.get_limit_for_tenant(tenant_id).await;

        let mut buckets = self.buckets.write().await;
        if !buckets.contains_key(tenant_id) && buckets.len() >= MAX_TENANT_BUCKETS {
            let keys: Vec<String> = buckets
                .keys()
                .filter(|k| k.as_str() != tenant_id)
                .take(PRUNE_BATCH)
                .cloned()
                .collect();
            for key in keys {
                buckets.remove(&key);
            }
        }

        let bucket = buckets
            .entry(tenant_id.to_string())
            .or_insert_with(|| TokenBucket::new(limit.burst_capacity, limit.messages_per_second));

        bucket.available_tokens()
    }

    /// Get rate limit for tenant (custom or default)
    async fn get_limit_for_tenant(&self, tenant_id: &str) -> RateLimit {
        let limits = self.custom_limits.read().await;
        let limit = limits
            .get(tenant_id)
            .cloned()
            .unwrap_or_else(|| self.default_limit.clone());
        normalize_rate_limit(limit)
    }
}

#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_token_bucket_consume() {
        let mut bucket = TokenBucket::new(100, 10.0); // 100 capacity, 10/sec refill

        // Should allow consuming 50
        assert!(bucket.try_consume(50));
        assert_eq!(bucket.available_tokens(), 50);

        // Should allow another 50
        assert!(bucket.try_consume(50));
        assert_eq!(bucket.available_tokens(), 0);

        // Should deny (no tokens left)
        assert!(!bucket.try_consume(1));
    }

    #[tokio::test]
    async fn test_token_bucket_refill() {
        let mut bucket = TokenBucket::new(100, 10.0); // 10 tokens/sec

        // Consume all tokens
        assert!(bucket.try_consume(100));

        // Wait 1 second (should refill ~10 tokens)
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;

        // Should have ~10 tokens available
        let available = bucket.available_tokens();
        assert!(
            (9..=11).contains(&available),
            "Expected ~10 tokens, got {}",
            available
        );
    }

    #[tokio::test]
    async fn test_rate_limiter_respects_quota() {
        let limiter = TenantRateLimiter::new(RateLimit {
            messages_per_second: 10.0,
            burst_capacity: 100,
        });

        // Burst: consume 100 (should succeed)
        for _ in 0..100 {
            assert!(limiter.check_rate_limit("tenant1", 1).await.unwrap());
        }

        // 101st should fail (exceeded capacity)
        assert!(!limiter.check_rate_limit("tenant1", 1).await.unwrap());

        // Wait 1 second (refill ~10 tokens)
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;

        // Should allow ~10 more messages
        let mut allowed = 0;
        for _ in 0..20 {
            if limiter.check_rate_limit("tenant1", 1).await.unwrap() {
                allowed += 1;
            }
        }
        assert!(
            (9..=11).contains(&allowed),
            "Expected ~10 allowed, got {}",
            allowed
        );
    }

    #[tokio::test]
    async fn test_tenant_isolation() {
        let limiter = TenantRateLimiter::new(RateLimit {
            messages_per_second: 10.0,
            burst_capacity: 10,
        });

        // Tenant1 consumes all tokens
        for _ in 0..10 {
            assert!(limiter.check_rate_limit("tenant1", 1).await.unwrap());
        }
        assert!(!limiter.check_rate_limit("tenant1", 1).await.unwrap());

        // Tenant2 should still have full capacity
        for _ in 0..10 {
            assert!(limiter.check_rate_limit("tenant2", 1).await.unwrap());
        }
    }

    #[tokio::test]
    async fn test_custom_tenant_limits() {
        let limiter = TenantRateLimiter::new(RateLimit::default());

        // Set high limit for premium tenant
        limiter
            .set_tenant_limit(
                "premium".to_string(),
                RateLimit {
                    messages_per_second: 1000.0,
                    burst_capacity: 10000,
                },
            )
            .await;

        // Should allow large burst
        assert!(limiter.check_rate_limit("premium", 5000).await.unwrap());
    }

    #[test]
    fn test_rate_limit_default() {
        let limit = RateLimit::default();
        assert_eq!(limit.messages_per_second, 100.0);
        assert_eq!(limit.burst_capacity, 1000);
    }
}
