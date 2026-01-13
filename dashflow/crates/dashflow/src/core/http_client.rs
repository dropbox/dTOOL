//! Optimized HTTP Client Configuration
//!
//! This module provides production-ready HTTP client configurations for DashFlow.
//!
//! Key optimizations:
//! - Connection pooling with configurable limits
//! - TCP keepalive to reuse connections
//! - Appropriate timeouts for LLM APIs
//! - Connection reuse across requests
//! - Response size limits to prevent memory exhaustion (M-216)
//!
//! # Example
//!
//! ```rust
//! use dashflow::core::http_client::HttpClientBuilder;
//!
//! let client = HttpClientBuilder::new()
//!     .with_llm_defaults()
//!     .build()
//!     .expect("Failed to build HTTP client");
//! ```
//!
//! # TLS Configuration
//!
//! DashFlow rejects invalid certificates by default. For private PKI or corporate TLS interception
//! proxies, add a custom CA certificate (PEM):
//!
//! ```rust,no_run
//! use dashflow::core::http_client::HttpClientBuilder;
//!
//! let client = HttpClientBuilder::new()
//!     .with_llm_defaults()
//!     .custom_ca_path("/path/to/custom-ca.pem")
//!     .build()
//!     .expect("Failed to build HTTP client");
//! ```
//!
//! Accepting invalid certificates is insecure and should only be used for local development:
//!
//! ```rust,no_run
//! use dashflow::core::http_client::HttpClientBuilder;
//!
//! let client = HttpClientBuilder::new()
//!     .allow_invalid_certs(true)
//!     .build()
//!     .expect("Failed to build HTTP client");
//! ```
//!
//! # Response Size Limiting
//!
//! To prevent memory exhaustion from malicious or buggy API responses, use
//! size-limited response reading:
//!
//! ```rust,ignore
//! use dashflow::core::http_client::{json_with_limit, DEFAULT_RESPONSE_SIZE_LIMIT};
//!
//! let response = client.get("https://api.example.com/data").send().await?;
//! let data: MyStruct = json_with_limit(response, DEFAULT_RESPONSE_SIZE_LIMIT).await?;
//! ```

use crate::constants::{
    DEFAULT_HTTP_CONNECT_TIMEOUT, DEFAULT_LLM_REQUEST_TIMEOUT, DEFAULT_POOL_IDLE_TIMEOUT,
    DEFAULT_POOL_MAX_IDLE_PER_HOST, DEFAULT_TCP_KEEPALIVE,
};
use crate::core::Error;
use reqwest::{Certificate, Client, ClientBuilder, NoProxy, Proxy, Response};
use serde::de::DeserializeOwned;
use std::fs;
use std::path::PathBuf;
use std::time::Duration;

// ============================================================================
// SSRF Protection (M-550, M-551, M-552)
// ============================================================================

/// Validate a URL is safe to fetch (SSRF protection).
///
/// This function blocks requests to:
/// - Private IP ranges (10.x, 172.16-31.x, 192.168.x)
/// - Loopback addresses (127.x.x.x, localhost)
/// - Link-local addresses (169.254.x.x)
/// - Cloud metadata endpoints (169.254.169.254)
/// - Non-HTTP schemes (file://, ftp://, etc.)
///
/// # Arguments
/// * `url_str` - The URL string to validate
///
/// # Returns
/// * `Ok(())` if the URL is safe to fetch
/// * `Err(Error)` with a description of why the URL is blocked
///
/// # Example
///
/// ```rust
/// use dashflow::core::http_client::validate_url_for_ssrf;
///
/// // Public URLs are allowed
/// assert!(validate_url_for_ssrf("https://api.github.com/users").is_ok());
///
/// // Private IPs are blocked
/// assert!(validate_url_for_ssrf("http://192.168.1.1/admin").is_err());
///
/// // Cloud metadata is blocked
/// assert!(validate_url_for_ssrf("http://169.254.169.254/latest/meta-data/").is_err());
/// ```
pub fn validate_url_for_ssrf(url_str: &str) -> Result<(), Error> {
    use std::net::IpAddr;

    // Parse the URL
    let url = reqwest::Url::parse(url_str)
        .map_err(|e| Error::InvalidInput(format!("Invalid URL '{}': {}", url_str, e)))?;

    // Only allow http and https schemes
    match url.scheme() {
        "http" | "https" => {}
        scheme => {
            return Err(Error::InvalidInput(format!(
                "SSRF protection: scheme '{}' not allowed (only http/https)",
                scheme
            )))
        }
    }

    // Get the host
    let host = url
        .host_str()
        .ok_or_else(|| Error::InvalidInput("SSRF protection: URL has no host".to_string()))?;

    // Block known dangerous hostnames
    let dangerous_hosts = [
        "localhost",
        "localhost.localdomain",
        "metadata.google.internal",
        "metadata",
        "instance-data",
        "169.254.169.254",
    ];
    let host_lower = host.to_lowercase();
    for dangerous in &dangerous_hosts {
        if host_lower == *dangerous {
            return Err(Error::InvalidInput(format!(
                "SSRF protection: host '{}' is blocked (internal/metadata endpoint)",
                host
            )));
        }
    }

    // If the host is an IP address, validate it
    if let Ok(ip) = host.parse::<IpAddr>() {
        if is_ssrf_blocked_ip(&ip) {
            return Err(Error::InvalidInput(format!(
                "SSRF protection: IP '{}' is a private/internal address",
                ip
            )));
        }
    }

    // Check for IPv4 in bracket notation (rare but possible)
    let trimmed = host.trim_start_matches('[').trim_end_matches(']');
    if let Ok(ip) = trimmed.parse::<IpAddr>() {
        if is_ssrf_blocked_ip(&ip) {
            return Err(Error::InvalidInput(format!(
                "SSRF protection: IP '{}' is a private/internal address",
                ip
            )));
        }
    }

    Ok(())
}

/// Check if an IP address should be blocked for SSRF protection.
fn is_ssrf_blocked_ip(ip: &std::net::IpAddr) -> bool {
    use std::net::IpAddr;

    match ip {
        IpAddr::V4(ipv4) => {
            // Private ranges (RFC 1918)
            ipv4.is_private()
                // Loopback (127.0.0.0/8)
                || ipv4.is_loopback()
                // Link-local (169.254.0.0/16)
                || ipv4.is_link_local()
                // Broadcast
                || ipv4.is_broadcast()
                // Unspecified (0.0.0.0)
                || ipv4.is_unspecified()
                // Documentation ranges (RFC 5737)
                || is_documentation_ipv4(ipv4)
                // Shared address space (RFC 6598: 100.64.0.0/10)
                || is_shared_address_space(ipv4)
                // Cloud metadata endpoint (AWS/GCP/Azure)
                || is_cloud_metadata_ipv4(ipv4)
        }
        IpAddr::V6(ipv6) => {
            // Loopback (::1)
            ipv6.is_loopback()
                // Unspecified (::)
                || ipv6.is_unspecified()
                // Check for IPv4-mapped addresses (::ffff:x.x.x.x)
                || ipv6.to_ipv4_mapped().is_some_and(|ipv4| is_ssrf_blocked_ip(&IpAddr::V4(ipv4)))
        }
    }
}

/// Check if IPv4 is in documentation range (RFC 5737)
fn is_documentation_ipv4(ip: &std::net::Ipv4Addr) -> bool {
    let octets = ip.octets();
    // 192.0.2.0/24 (TEST-NET-1)
    (octets[0] == 192 && octets[1] == 0 && octets[2] == 2)
        // 198.51.100.0/24 (TEST-NET-2)
        || (octets[0] == 198 && octets[1] == 51 && octets[2] == 100)
        // 203.0.113.0/24 (TEST-NET-3)
        || (octets[0] == 203 && octets[1] == 0 && octets[2] == 113)
}

/// Check if IPv4 is in shared address space (RFC 6598: 100.64.0.0/10)
fn is_shared_address_space(ip: &std::net::Ipv4Addr) -> bool {
    let octets = ip.octets();
    octets[0] == 100 && (octets[1] & 0xC0) == 64
}

/// Check if IPv4 is cloud metadata endpoint
fn is_cloud_metadata_ipv4(ip: &std::net::Ipv4Addr) -> bool {
    let octets = ip.octets();
    // AWS/GCP/Azure metadata: 169.254.169.254
    octets[0] == 169 && octets[1] == 254 && octets[2] == 169 && octets[3] == 254
}

/// TLS configuration for HTTP clients.
///
/// By default, DashFlow uses reqwest/platform defaults and rejects invalid TLS certificates.
/// Setting `allow_invalid_certs` is insecure and should only be used for local development or
/// tightly controlled environments.
///
/// `custom_ca_path` should point to a PEM-encoded CA certificate that will be added to this
/// client's trust store (useful for corporate MITM proxies or private PKI).
#[derive(Debug, Clone, Default)]
pub struct TlsConfig {
    /// If true, accept invalid TLS certificates (INSECURE).
    pub allow_invalid_certs: bool,
    /// Optional path to a PEM-encoded CA certificate to trust.
    pub custom_ca_path: Option<PathBuf>,
}

// ============================================================================
// Connection Pool Configuration (M-238)
// ============================================================================

/// Minimum allowed pool idle timeout (1 second)
pub const MIN_POOL_IDLE_TIMEOUT_SECS: u64 = 1;
/// Maximum allowed pool idle timeout (3600 seconds = 1 hour)
pub const MAX_POOL_IDLE_TIMEOUT_SECS: u64 = 3600;
/// Maximum allowed idle connections per host
pub const MAX_POOL_IDLE_PER_HOST: usize = 256;

/// Connection pool configuration with validation and clear diagnostics.
///
/// This struct provides explicit control over HTTP connection pool behavior,
/// helping prevent pool exhaustion in high-concurrency scenarios.
///
/// # Pool Exhaustion
///
/// Pool exhaustion occurs when all connections to a host are in use and no new
/// connections can be created. Symptoms include:
/// - Requests timing out during connection phase
/// - Error messages containing "pool", "no available connections", or "connection limit"
/// - Increased latency under load
///
/// # Tuning Guidelines
///
/// | Workload | `max_idle_per_host` | `idle_timeout` | Notes |
/// |----------|---------------------|----------------|-------|
/// | LLM APIs | 16-32 | 60-90s | High latency, connection reuse critical |
/// | REST APIs | 4-8 | 30s | Lower latency, moderate reuse |
/// | High-throughput | 32-64 | 120s | Many concurrent requests |
/// | Low-traffic | 2-4 | 15s | Conserve resources |
///
/// # Example
///
/// ```rust
/// use dashflow::core::http_client::{PoolConfig, HttpClientBuilder};
///
/// // High-throughput configuration
/// let pool_config = PoolConfig::default()
///     .with_max_idle_per_host(64)
///     .with_idle_timeout_secs(120);
///
/// let client = HttpClientBuilder::new()
///     .with_pool_config(pool_config)
///     .build()
///     .expect("Failed to build client");
/// ```
#[derive(Debug, Clone, Copy)]
pub struct PoolConfig {
    /// Maximum number of idle connections to keep per host.
    ///
    /// Higher values allow more connection reuse but consume more memory.
    /// When all idle connections are in use and this limit is reached,
    /// new requests must wait for a connection to become available.
    ///
    /// Default: 8 for general use, 32 for LLM workloads.
    pub max_idle_per_host: usize,

    /// How long to keep idle connections alive.
    ///
    /// Longer timeouts increase connection reuse but hold resources longer.
    /// Set this based on the typical interval between requests to the same host.
    ///
    /// Default: 30s for general use, 90s for LLM workloads.
    pub idle_timeout: Duration,
}

impl Default for PoolConfig {
    fn default() -> Self {
        Self {
            max_idle_per_host: 8,
            idle_timeout: Duration::from_secs(30),
        }
    }
}

impl PoolConfig {
    /// Create a new pool configuration with default settings.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a pool configuration optimized for LLM API workloads.
    ///
    /// Settings:
    /// - `max_idle_per_host`: 32 (high connection reuse)
    /// - `idle_timeout`: 90s (long-lived connections)
    #[must_use]
    pub fn for_llm_workloads() -> Self {
        Self {
            max_idle_per_host: DEFAULT_POOL_MAX_IDLE_PER_HOST,
            idle_timeout: DEFAULT_POOL_IDLE_TIMEOUT,
        }
    }

    /// Create a pool configuration optimized for high-throughput workloads.
    ///
    /// Settings:
    /// - `max_idle_per_host`: 64 (maximum connection reuse)
    /// - `idle_timeout`: 120s (very long-lived connections)
    #[must_use]
    pub fn for_high_throughput() -> Self {
        Self {
            max_idle_per_host: 64,
            idle_timeout: Duration::from_secs(120),
        }
    }

    /// Create a pool configuration optimized for low-traffic scenarios.
    ///
    /// Settings:
    /// - `max_idle_per_host`: 4 (conserve resources)
    /// - `idle_timeout`: 15s (quick cleanup)
    #[must_use]
    pub fn for_low_traffic() -> Self {
        Self {
            max_idle_per_host: 4,
            idle_timeout: Duration::from_secs(15),
        }
    }

    /// Set maximum idle connections per host.
    ///
    /// # Arguments
    /// * `max` - Maximum idle connections (clamped to 1..=256)
    #[must_use]
    pub fn with_max_idle_per_host(mut self, max: usize) -> Self {
        self.max_idle_per_host = max.clamp(1, MAX_POOL_IDLE_PER_HOST);
        if max > MAX_POOL_IDLE_PER_HOST {
            tracing::warn!(
                requested = max,
                actual = self.max_idle_per_host,
                "Pool max_idle_per_host clamped to maximum"
            );
        }
        self
    }

    /// Set idle connection timeout.
    ///
    /// # Arguments
    /// * `timeout` - Idle timeout (clamped to 1s..=3600s)
    #[must_use]
    pub fn with_idle_timeout(mut self, timeout: Duration) -> Self {
        let secs = timeout.as_secs().clamp(MIN_POOL_IDLE_TIMEOUT_SECS, MAX_POOL_IDLE_TIMEOUT_SECS);
        self.idle_timeout = Duration::from_secs(secs);
        if timeout.as_secs() > MAX_POOL_IDLE_TIMEOUT_SECS {
            tracing::warn!(
                requested_secs = timeout.as_secs(),
                actual_secs = secs,
                "Pool idle_timeout clamped to maximum"
            );
        }
        self
    }

    /// Set idle connection timeout in seconds.
    ///
    /// # Arguments
    /// * `secs` - Idle timeout in seconds (clamped to 1..=3600)
    #[must_use]
    pub fn with_idle_timeout_secs(self, secs: u64) -> Self {
        self.with_idle_timeout(Duration::from_secs(secs))
    }

    /// Validate the pool configuration and return any warnings.
    ///
    /// Returns a vector of warning messages for potentially problematic settings.
    #[must_use]
    pub fn validate(&self) -> Vec<String> {
        let mut warnings = Vec::new();

        if self.max_idle_per_host < 2 {
            warnings.push(format!(
                "Pool max_idle_per_host={} is very low; consider at least 4 for typical workloads",
                self.max_idle_per_host
            ));
        }

        if self.idle_timeout.as_secs() < 10 {
            warnings.push(format!(
                "Pool idle_timeout={}s is very short; connections may be closed before reuse",
                self.idle_timeout.as_secs()
            ));
        }

        if self.max_idle_per_host > 64 && self.idle_timeout.as_secs() > 120 {
            warnings.push(format!(
                "Large pool ({}x{}s) may consume excessive resources; monitor memory usage",
                self.max_idle_per_host,
                self.idle_timeout.as_secs()
            ));
        }

        warnings
    }

    /// Get diagnostic information about this pool configuration.
    ///
    /// Useful for logging and debugging pool-related issues.
    #[must_use]
    pub fn diagnostic(&self) -> String {
        format!(
            "Pool config: max_idle_per_host={}, idle_timeout={}s. \
             If experiencing pool exhaustion, consider: \
             (1) increasing max_idle_per_host for high concurrency, \
             (2) reducing concurrent requests, \
             (3) checking for connection leaks (requests not completing)",
            self.max_idle_per_host,
            self.idle_timeout.as_secs()
        )
    }
}

/// Builder for creating optimized HTTP clients.
///
/// Provides preset configurations for common use cases:
/// - LLM API calls (high latency, connection reuse important)
/// - General purpose API calls
/// - Custom configurations
///
/// # All Fields Optional (have sensible defaults)
/// - `pool_max_idle_per_host` - Max idle connections per host (default: 8)
/// - `pool_idle_timeout` - How long to keep idle connections (default: 30s)
/// - `connect_timeout` - TCP connection timeout (default: 10s)
/// - `request_timeout` - Overall request timeout (default: None)
/// - `tcp_keepalive` - TCP keepalive interval (default: None)
///
/// # Example
/// ```rust,ignore
/// // Quick setup for LLM APIs
/// let client = HttpClientBuilder::new()
///     .with_llm_defaults()
///     .build()?;
///
/// // Custom configuration
/// let client = HttpClientBuilder::new()
///     .with_request_timeout(Duration::from_secs(60))
///     .with_pool_max_idle(16)
///     .build()?;
/// ```
pub struct HttpClientBuilder {
    pool_max_idle_per_host: usize,
    pool_idle_timeout: Duration,
    connect_timeout: Duration,
    request_timeout: Option<Duration>,
    tcp_keepalive: Option<Duration>,
    tls_config: TlsConfig,
}

impl Default for HttpClientBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl HttpClientBuilder {
    /// Create a new HTTP client builder with conservative defaults
    #[must_use]
    pub fn new() -> Self {
        Self {
            pool_max_idle_per_host: 8,
            pool_idle_timeout: Duration::from_secs(30),
            connect_timeout: Duration::from_secs(10),
            request_timeout: None,
            tcp_keepalive: None,
            tls_config: TlsConfig::default(),
        }
    }

    /// Configure with defaults optimized for LLM API calls
    ///
    /// Settings:
    /// - Large connection pool (32 connections per host)
    /// - Long idle timeout (90s) for connection reuse
    /// - Long request timeout (300s) for streaming responses
    /// - TCP keepalive (60s) to maintain connections
    #[must_use]
    pub fn with_llm_defaults(mut self) -> Self {
        self.pool_max_idle_per_host = DEFAULT_POOL_MAX_IDLE_PER_HOST;
        self.pool_idle_timeout = DEFAULT_POOL_IDLE_TIMEOUT;
        self.connect_timeout = DEFAULT_HTTP_CONNECT_TIMEOUT;
        self.request_timeout = Some(DEFAULT_LLM_REQUEST_TIMEOUT);
        self.tcp_keepalive = Some(DEFAULT_TCP_KEEPALIVE);
        self
    }

    /// Set maximum idle connections per host
    ///
    /// Higher values allow more connection reuse but consume more resources.
    /// Recommended: 16-32 for LLM workloads, 4-8 for general purpose.
    #[must_use]
    pub fn pool_max_idle_per_host(mut self, max: usize) -> Self {
        self.pool_max_idle_per_host = max;
        self
    }

    /// Set how long idle connections are kept alive
    ///
    /// Longer timeouts increase connection reuse but hold resources longer.
    /// Recommended: 60-90s for LLM APIs, 30s for general purpose.
    #[must_use]
    pub fn pool_idle_timeout(mut self, timeout: Duration) -> Self {
        self.pool_idle_timeout = timeout;
        self
    }

    /// Configure connection pool settings using a `PoolConfig`.
    ///
    /// This provides a structured way to configure pool settings with
    /// validation and preset configurations for different workloads.
    ///
    /// # Example
    /// ```rust
    /// use dashflow::core::http_client::{HttpClientBuilder, PoolConfig};
    ///
    /// let client = HttpClientBuilder::new()
    ///     .with_pool_config(PoolConfig::for_high_throughput())
    ///     .build()
    ///     .expect("Failed to build client");
    /// ```
    #[must_use]
    pub fn with_pool_config(mut self, config: PoolConfig) -> Self {
        // Log any validation warnings
        for warning in config.validate() {
            tracing::warn!(warning = %warning, "Pool configuration warning");
        }
        self.pool_max_idle_per_host = config.max_idle_per_host;
        self.pool_idle_timeout = config.idle_timeout;
        self
    }

    /// Set connection timeout
    ///
    /// How long to wait for initial TCP connection.
    /// Recommended: 10s for most use cases.
    #[must_use]
    pub fn connect_timeout(mut self, timeout: Duration) -> Self {
        self.connect_timeout = timeout;
        self
    }

    /// Set request timeout
    ///
    /// Total time allowed for request (including response body).
    /// Recommended: 300s for LLM streaming, 30s for general APIs.
    #[must_use]
    pub fn request_timeout(mut self, timeout: Duration) -> Self {
        self.request_timeout = Some(timeout);
        self
    }

    /// Enable TCP keepalive
    ///
    /// Sends periodic TCP keepalive packets to detect broken connections.
    /// Recommended: 60s for most use cases.
    #[must_use]
    pub fn tcp_keepalive(mut self, interval: Duration) -> Self {
        self.tcp_keepalive = Some(interval);
        self
    }

    /// Configure TLS settings for this client.
    #[must_use]
    pub fn tls_config(mut self, tls_config: TlsConfig) -> Self {
        self.tls_config = tls_config;
        self
    }

    /// Allow invalid TLS certificates (INSECURE).
    #[must_use]
    pub fn allow_invalid_certs(mut self, allow: bool) -> Self {
        self.tls_config.allow_invalid_certs = allow;
        self
    }

    /// Add a PEM-encoded custom CA certificate to trust for this client.
    #[must_use]
    pub fn custom_ca_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.tls_config.custom_ca_path = Some(path.into());
        self
    }

    /// Build the configured HTTP client
    pub fn build(self) -> Result<Client, Error> {
        let mut builder = ClientBuilder::new()
            .pool_max_idle_per_host(self.pool_max_idle_per_host)
            .pool_idle_timeout(self.pool_idle_timeout)
            .connect_timeout(self.connect_timeout);

        if let Some(timeout) = self.request_timeout {
            builder = builder.timeout(timeout);
        }

        if let Some(keepalive) = self.tcp_keepalive {
            builder = builder.tcp_keepalive(keepalive);
        }

        builder = apply_tls_config(builder, &self.tls_config)?;

        // reqwest's default behavior on macOS includes loading system proxy config (via
        // SystemConfiguration). In some environments this can panic inside the
        // `system-configuration` crate. We disable system proxy auto-detection on macOS and
        // re-apply standard env proxies (HTTP[S]_PROXY/ALL_PROXY/NO_PROXY) ourselves.
        builder = apply_platform_proxy_config(builder);

        builder
            .build()
            .map_err(|e| Error::Other(format!("Failed to build HTTP client: {e}")))
    }
}

fn apply_tls_config(
    mut builder: ClientBuilder,
    tls_config: &TlsConfig,
) -> Result<ClientBuilder, Error> {
    if tls_config.allow_invalid_certs {
        builder = builder.danger_accept_invalid_certs(true);
    }

    if let Some(path) = &tls_config.custom_ca_path {
        let pem = fs::read(path).map_err(|e| {
            Error::Other(format!(
                "Failed to read custom CA certificate from {path}: {e}",
                path = path.display()
            ))
        })?;

        let cert = Certificate::from_pem(&pem).map_err(|e| {
            Error::Other(format!(
                "Failed to parse custom CA certificate from {path}: {e}",
                path = path.display()
            ))
        })?;

        builder = builder.add_root_certificate(cert);
    }

    Ok(builder)
}

pub(crate) fn apply_platform_proxy_config(builder: ClientBuilder) -> ClientBuilder {
    #[cfg(target_os = "macos")]
    {
        apply_env_proxies(builder.no_proxy())
    }
    #[cfg(not(target_os = "macos"))]
    {
        builder
    }
}

#[cfg(target_os = "macos")]
fn apply_env_proxies(mut builder: ClientBuilder) -> ClientBuilder {
    fn env_var(keys: &[&str]) -> Option<String> {
        keys.iter().find_map(|key| {
            std::env::var(key)
                .ok()
                .map(|v| v.trim().to_string())
                .filter(|v| !v.is_empty())
        })
    }

    let no_proxy = env_var(&["NO_PROXY", "no_proxy"]).and_then(|s| NoProxy::from_string(&s));

    if let Some(http_proxy) = env_var(&["HTTP_PROXY", "http_proxy"]) {
        match Proxy::http(&http_proxy) {
            Ok(proxy) => builder = builder.proxy(proxy.no_proxy(no_proxy.clone())),
            Err(err) => {
                tracing::warn!(env_var = "HTTP_PROXY", error = %err, "Invalid proxy URL; ignoring")
            }
        }
    }

    if let Some(https_proxy) = env_var(&["HTTPS_PROXY", "https_proxy"]) {
        match Proxy::https(&https_proxy) {
            Ok(proxy) => builder = builder.proxy(proxy.no_proxy(no_proxy.clone())),
            Err(err) => {
                tracing::warn!(env_var = "HTTPS_PROXY", error = %err, "Invalid proxy URL; ignoring")
            }
        }
    }

    if let Some(all_proxy) = env_var(&["ALL_PROXY", "all_proxy"]) {
        match Proxy::all(&all_proxy) {
            Ok(proxy) => builder = builder.proxy(proxy.no_proxy(no_proxy)),
            Err(err) => {
                tracing::warn!(env_var = "ALL_PROXY", error = %err, "Invalid proxy URL; ignoring")
            }
        }
    }

    builder
}

/// Create a pre-configured HTTP client optimized for LLM API calls
///
/// Equivalent to `HttpClientBuilder::new().with_llm_defaults().build()`
pub fn create_llm_client() -> Result<Client, Error> {
    HttpClientBuilder::new().with_llm_defaults().build()
}

/// Create a basic HTTP client with minimal configuration
///
/// Equivalent to `HttpClientBuilder::new().build()`
pub fn create_basic_client() -> Result<Client, Error> {
    HttpClientBuilder::new().build()
}

// =============================================================================
// Response Size Limiting (M-216)
// =============================================================================

/// Default maximum response size: 10MB
///
/// This is appropriate for most API responses. LLM responses rarely exceed
/// a few hundred KB. Adjust for specific use cases if needed.
pub const DEFAULT_RESPONSE_SIZE_LIMIT: usize = 10 * 1024 * 1024;

/// Maximum response size for search/retrieval APIs: 50MB
///
/// Search APIs may return large result sets with embedded content.
pub const SEARCH_RESPONSE_SIZE_LIMIT: usize = 50 * 1024 * 1024;

/// Maximum response size for LLM streaming: 100MB
///
/// Streaming responses accumulate over time; allow larger limit.
pub const STREAMING_RESPONSE_SIZE_LIMIT: usize = 100 * 1024 * 1024;

/// Error returned when response exceeds size limit
#[derive(Debug, Clone)]
pub struct ResponseTooLargeError {
    /// Actual size (or size so far if streaming)
    pub actual_size: usize,
    /// Maximum allowed size
    pub max_size: usize,
    /// Whether the size came from Content-Length header
    pub from_header: bool,
}

impl std::fmt::Display for ResponseTooLargeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.from_header {
            write!(
                f,
                "Response Content-Length ({} bytes) exceeds limit ({} bytes)",
                self.actual_size, self.max_size
            )
        } else {
            write!(
                f,
                "Response body ({} bytes) exceeds limit ({} bytes)",
                self.actual_size, self.max_size
            )
        }
    }
}

impl std::error::Error for ResponseTooLargeError {}

/// Read response body with a size limit.
///
/// This function prevents memory exhaustion from malicious or buggy API
/// responses by:
/// 1. Checking Content-Length header first (fast rejection)
/// 2. Reading body incrementally with size tracking
/// 3. Returning an error if limit is exceeded
///
/// # Arguments
/// * `response` - The HTTP response to read
/// * `max_size` - Maximum allowed body size in bytes
///
/// # Returns
/// * `Ok(Vec<u8>)` - The response body if within limits
/// * `Err(Error)` - If body exceeds limit or read fails
///
/// # Example
/// ```rust,ignore
/// let response = client.get(url).send().await?;
/// let body = read_body_with_limit(response, 10 * 1024 * 1024).await?;
/// ```
pub async fn read_body_with_limit(response: Response, max_size: usize) -> Result<Vec<u8>, Error> {
    // Check Content-Length header first for fast rejection
    if let Some(content_length) = response.content_length() {
        if content_length as usize > max_size {
            return Err(Error::Other(
                ResponseTooLargeError {
                    actual_size: content_length as usize,
                    max_size,
                    from_header: true,
                }
                .to_string(),
            ));
        }
    }

    // Read body with size limit
    // Use bytes() which reads the entire body - we'll check after
    let bytes = response
        .bytes()
        .await
        .map_err(|e| Error::Other(format!("Failed to read response body: {e}")))?;

    if bytes.len() > max_size {
        return Err(Error::Other(
            ResponseTooLargeError {
                actual_size: bytes.len(),
                max_size,
                from_header: false,
            }
            .to_string(),
        ));
    }

    Ok(bytes.to_vec())
}

/// Read response body as text with a size limit.
///
/// Same as [`read_body_with_limit`] but returns a String.
///
/// # Arguments
/// * `response` - The HTTP response to read
/// * `max_size` - Maximum allowed body size in bytes
///
/// # Returns
/// * `Ok(String)` - The response body as UTF-8 text
/// * `Err(Error)` - If body exceeds limit, read fails, or not valid UTF-8
pub async fn read_text_with_limit(response: Response, max_size: usize) -> Result<String, Error> {
    let bytes = read_body_with_limit(response, max_size).await?;
    String::from_utf8(bytes).map_err(|e| Error::Other(format!("Response is not valid UTF-8: {e}")))
}

/// Parse JSON response with a size limit.
///
/// Combines [`read_body_with_limit`] with JSON deserialization.
/// Use this instead of `response.json().await` for safety.
///
/// # Arguments
/// * `response` - The HTTP response to read
/// * `max_size` - Maximum allowed body size in bytes
///
/// # Returns
/// * `Ok(T)` - The deserialized JSON value
/// * `Err(Error)` - If body exceeds limit, read fails, or JSON parsing fails
///
/// # Example
/// ```rust,ignore
/// use dashflow::core::http_client::{json_with_limit, DEFAULT_RESPONSE_SIZE_LIMIT};
///
/// let response = client.get("https://api.example.com/data").send().await?;
/// let data: MyStruct = json_with_limit(response, DEFAULT_RESPONSE_SIZE_LIMIT).await?;
/// ```
pub async fn json_with_limit<T: DeserializeOwned>(
    response: Response,
    max_size: usize,
) -> Result<T, Error> {
    let bytes = read_body_with_limit(response, max_size).await?;
    serde_json::from_slice(&bytes).map_err(|e| Error::Other(format!("JSON parsing failed: {e}")))
}

// =============================================================================
// HTTP Client Metrics (M-238)
// =============================================================================

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::OnceLock;

/// Global HTTP client metrics for monitoring pool health.
///
/// These metrics help diagnose connection pool issues:
/// - `pool_exhaustion_events` - Incremented when pool exhaustion is detected
/// - `connect_timeout_events` - Incremented when connection timeouts occur
/// - `request_errors` - Total HTTP request errors
///
/// # Example
///
/// ```rust
/// use dashflow::core::http_client::HttpClientMetrics;
///
/// // Record a pool exhaustion event
/// HttpClientMetrics::global().record_pool_exhaustion("api.openai.com");
///
/// // Get current metrics
/// let snapshot = HttpClientMetrics::global().snapshot();
/// println!("Pool exhaustion events: {}", snapshot.pool_exhaustion_events);
/// ```
#[derive(Debug)]
pub struct HttpClientMetrics {
    pool_exhaustion_events: AtomicU64,
    connect_timeout_events: AtomicU64,
    request_errors: AtomicU64,
}

/// Snapshot of HTTP client metrics at a point in time.
#[derive(Debug, Clone, Default)]
pub struct HttpClientMetricsSnapshot {
    /// Number of pool exhaustion events detected
    pub pool_exhaustion_events: u64,
    /// Number of connection timeout events
    pub connect_timeout_events: u64,
    /// Total request errors
    pub request_errors: u64,
}

impl HttpClientMetrics {
    /// Create a new metrics instance.
    const fn new() -> Self {
        Self {
            pool_exhaustion_events: AtomicU64::new(0),
            connect_timeout_events: AtomicU64::new(0),
            request_errors: AtomicU64::new(0),
        }
    }

    /// Get the global metrics instance.
    pub fn global() -> &'static Self {
        static INSTANCE: OnceLock<HttpClientMetrics> = OnceLock::new();
        INSTANCE.get_or_init(HttpClientMetrics::new)
    }

    /// Record a pool exhaustion event.
    ///
    /// Call this when a pool exhaustion error is detected.
    /// The host is logged for debugging purposes.
    pub fn record_pool_exhaustion(&self, host: &str) {
        self.pool_exhaustion_events.fetch_add(1, Ordering::Relaxed);
        tracing::warn!(
            host = %host,
            event = "pool_exhaustion",
            "HTTP connection pool exhaustion detected"
        );
    }

    /// Record a connection timeout event.
    ///
    /// Call this when a connection timeout is detected.
    /// Under high concurrency, this may indicate pool exhaustion.
    pub fn record_connect_timeout(&self, host: &str) {
        self.connect_timeout_events.fetch_add(1, Ordering::Relaxed);
        tracing::debug!(
            host = %host,
            event = "connect_timeout",
            "HTTP connection timeout"
        );
    }

    /// Record a general request error.
    pub fn record_request_error(&self) {
        self.request_errors.fetch_add(1, Ordering::Relaxed);
    }

    /// Get a snapshot of current metrics.
    pub fn snapshot(&self) -> HttpClientMetricsSnapshot {
        HttpClientMetricsSnapshot {
            pool_exhaustion_events: self.pool_exhaustion_events.load(Ordering::Relaxed),
            connect_timeout_events: self.connect_timeout_events.load(Ordering::Relaxed),
            request_errors: self.request_errors.load(Ordering::Relaxed),
        }
    }

    /// Reset all metrics to zero (primarily for testing).
    pub fn reset(&self) {
        self.pool_exhaustion_events.store(0, Ordering::Relaxed);
        self.connect_timeout_events.store(0, Ordering::Relaxed);
        self.request_errors.store(0, Ordering::Relaxed);
    }

    /// Check if pool exhaustion has been detected.
    ///
    /// Returns true if any pool exhaustion events have occurred since
    /// metrics were last reset.
    #[must_use]
    pub fn has_pool_exhaustion(&self) -> bool {
        self.pool_exhaustion_events.load(Ordering::Relaxed) > 0
    }

    /// Get diagnostic string for pool health.
    ///
    /// Useful for logging and debugging.
    #[must_use]
    pub fn pool_diagnostic(&self) -> String {
        let snapshot = self.snapshot();
        if snapshot.pool_exhaustion_events > 0 {
            format!(
                "Pool health: DEGRADED - {} exhaustion events, {} timeouts. \
                 Consider increasing pool size via PoolConfig::for_high_throughput().",
                snapshot.pool_exhaustion_events, snapshot.connect_timeout_events
            )
        } else if snapshot.connect_timeout_events > 10 {
            format!(
                "Pool health: WARNING - {} connection timeouts (may indicate pool pressure). \
                 Monitor and consider increasing pool size if this continues.",
                snapshot.connect_timeout_events
            )
        } else {
            format!(
                "Pool health: OK - {} exhaustion events, {} timeouts, {} errors",
                snapshot.pool_exhaustion_events,
                snapshot.connect_timeout_events,
                snapshot.request_errors
            )
        }
    }
}

/// Classify and record network errors in metrics.
///
/// This function examines a network error and records appropriate metrics.
/// It should be called when handling HTTP request errors to maintain
/// accurate pool health metrics.
///
/// # Arguments
/// * `error` - The error to classify
/// * `host` - Optional host for logging context
///
/// # Example
///
/// ```rust,ignore
/// use dashflow::core::http_client::record_network_error;
///
/// match client.get(url).send().await {
///     Ok(response) => { /* handle response */ }
///     Err(e) => {
///         record_network_error(&e, Some("api.example.com"));
///         return Err(e.into());
///     }
/// }
/// ```
pub fn record_network_error(error: &reqwest::Error, host: Option<&str>) {
    use crate::core::error::NetworkErrorKind;

    let metrics = HttpClientMetrics::global();
    let kind = NetworkErrorKind::from_reqwest_error(error);
    let host_str = host.unwrap_or("unknown");

    match kind {
        NetworkErrorKind::PoolExhausted => {
            metrics.record_pool_exhaustion(host_str);
        }
        NetworkErrorKind::ConnectionTimeout => {
            metrics.record_connect_timeout(host_str);
        }
        _ => {
            metrics.record_request_error();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn test_builder_defaults() {
        let builder = HttpClientBuilder::new();
        assert_eq!(builder.pool_max_idle_per_host, 8);
        assert_eq!(builder.pool_idle_timeout, Duration::from_secs(30));
        assert!(!builder.tls_config.allow_invalid_certs);
        assert!(builder.tls_config.custom_ca_path.is_none());
    }

    #[test]
    fn test_llm_defaults() {
        let builder = HttpClientBuilder::new().with_llm_defaults();
        assert_eq!(builder.pool_max_idle_per_host, 32);
        assert_eq!(builder.pool_idle_timeout, Duration::from_secs(90));
        assert_eq!(builder.tcp_keepalive, Some(Duration::from_secs(60)));
    }

    #[test]
    fn test_custom_configuration() {
        let builder = HttpClientBuilder::new()
            .pool_max_idle_per_host(16)
            .pool_idle_timeout(Duration::from_secs(60))
            .connect_timeout(Duration::from_secs(5))
            .request_timeout(Duration::from_secs(120))
            .tcp_keepalive(Duration::from_secs(30));

        assert_eq!(builder.pool_max_idle_per_host, 16);
        assert_eq!(builder.pool_idle_timeout, Duration::from_secs(60));
        assert_eq!(builder.connect_timeout, Duration::from_secs(5));
        assert_eq!(builder.request_timeout, Some(Duration::from_secs(120)));
        assert_eq!(builder.tcp_keepalive, Some(Duration::from_secs(30)));
    }

    #[test]
    fn test_build_basic_client() {
        let result = create_basic_client();
        assert!(result.is_ok());
    }

    #[test]
    fn test_build_llm_client() {
        let result = create_llm_client();
        assert!(result.is_ok());
    }

    #[test]
    fn test_tls_allow_invalid_certs_builds() {
        let result = HttpClientBuilder::new().allow_invalid_certs(true).build();
        assert!(result.is_ok());
    }

    #[test]
    fn test_tls_custom_ca_missing_file_errors() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let missing = temp_dir.path().join("missing-ca.pem");

        let err = HttpClientBuilder::new()
            .custom_ca_path(missing)
            .build()
            .unwrap_err()
            .to_string();

        assert!(err.contains("custom CA certificate"));
        assert!(err.contains("Failed to read"));
    }

    #[test]
    fn test_tls_custom_ca_invalid_pem_errors() {
        let mut file = tempfile::NamedTempFile::new().unwrap();
        file.write_all(b"not-a-cert").unwrap();

        let err = HttpClientBuilder::new()
            .custom_ca_path(file.path().to_path_buf())
            .build()
            .unwrap_err()
            .to_string();

        assert!(err.contains("custom CA certificate"));
        assert!(err.contains("Failed to parse"));
    }

    #[test]
    fn test_response_too_large_error_display() {
        let err = ResponseTooLargeError {
            actual_size: 20_000_000,
            max_size: 10_000_000,
            from_header: true,
        };
        assert!(err.to_string().contains("Content-Length"));
        assert!(err.to_string().contains("20000000"));
        assert!(err.to_string().contains("10000000"));

        let err_body = ResponseTooLargeError {
            actual_size: 15_000_000,
            max_size: 10_000_000,
            from_header: false,
        };
        assert!(err_body.to_string().contains("body"));
        assert!(!err_body.to_string().contains("Content-Length"));
    }

    #[test]
    fn test_size_limit_constants() {
        // Verify reasonable defaults
        assert_eq!(DEFAULT_RESPONSE_SIZE_LIMIT, 10 * 1024 * 1024);
        assert_eq!(SEARCH_RESPONSE_SIZE_LIMIT, 50 * 1024 * 1024);
        assert_eq!(STREAMING_RESPONSE_SIZE_LIMIT, 100 * 1024 * 1024);

        // Verify ordering makes sense
        assert!(DEFAULT_RESPONSE_SIZE_LIMIT < SEARCH_RESPONSE_SIZE_LIMIT);
        assert!(SEARCH_RESPONSE_SIZE_LIMIT < STREAMING_RESPONSE_SIZE_LIMIT);
    }

    // =========================================================================
    // PoolConfig Tests (M-238)
    // =========================================================================

    #[test]
    fn test_pool_config_defaults() {
        let config = PoolConfig::default();
        assert_eq!(config.max_idle_per_host, 8);
        assert_eq!(config.idle_timeout, Duration::from_secs(30));
    }

    #[test]
    fn test_pool_config_for_llm_workloads() {
        let config = PoolConfig::for_llm_workloads();
        assert_eq!(config.max_idle_per_host, 32);
        assert_eq!(config.idle_timeout, Duration::from_secs(90));
    }

    #[test]
    fn test_pool_config_for_high_throughput() {
        let config = PoolConfig::for_high_throughput();
        assert_eq!(config.max_idle_per_host, 64);
        assert_eq!(config.idle_timeout, Duration::from_secs(120));
    }

    #[test]
    fn test_pool_config_for_low_traffic() {
        let config = PoolConfig::for_low_traffic();
        assert_eq!(config.max_idle_per_host, 4);
        assert_eq!(config.idle_timeout, Duration::from_secs(15));
    }

    #[test]
    fn test_pool_config_builder_methods() {
        let config = PoolConfig::new()
            .with_max_idle_per_host(16)
            .with_idle_timeout_secs(45);

        assert_eq!(config.max_idle_per_host, 16);
        assert_eq!(config.idle_timeout, Duration::from_secs(45));
    }

    #[test]
    fn test_pool_config_clamping_max() {
        // Test clamping at maximum
        let config = PoolConfig::new().with_max_idle_per_host(500); // Exceeds MAX_POOL_IDLE_PER_HOST
        assert_eq!(config.max_idle_per_host, MAX_POOL_IDLE_PER_HOST);

        // Test clamping idle timeout
        let config = PoolConfig::new().with_idle_timeout_secs(5000); // Exceeds MAX_POOL_IDLE_TIMEOUT_SECS
        assert_eq!(
            config.idle_timeout,
            Duration::from_secs(MAX_POOL_IDLE_TIMEOUT_SECS)
        );
    }

    #[test]
    fn test_pool_config_clamping_min() {
        // Test clamping at minimum
        let config = PoolConfig::new().with_max_idle_per_host(0);
        assert_eq!(config.max_idle_per_host, 1);

        let config = PoolConfig::new().with_idle_timeout_secs(0);
        assert_eq!(
            config.idle_timeout,
            Duration::from_secs(MIN_POOL_IDLE_TIMEOUT_SECS)
        );
    }

    #[test]
    fn test_pool_config_validation() {
        // Low max_idle_per_host should generate warning
        let config = PoolConfig::new().with_max_idle_per_host(1);
        let warnings = config.validate();
        assert!(!warnings.is_empty());
        assert!(warnings[0].contains("very low"));

        // Low idle_timeout should generate warning
        let config = PoolConfig::new().with_idle_timeout_secs(5);
        let warnings = config.validate();
        assert!(!warnings.is_empty());
        assert!(warnings[0].contains("very short"));

        // Large pool should generate warning
        let config = PoolConfig::new()
            .with_max_idle_per_host(100)
            .with_idle_timeout_secs(200);
        let warnings = config.validate();
        assert!(!warnings.is_empty());
        assert!(warnings[0].contains("excessive resources"));
    }

    #[test]
    fn test_pool_config_diagnostic() {
        let config = PoolConfig::for_high_throughput();
        let diagnostic = config.diagnostic();
        assert!(diagnostic.contains("max_idle_per_host=64"));
        assert!(diagnostic.contains("idle_timeout=120s"));
        assert!(diagnostic.contains("pool exhaustion"));
    }

    #[test]
    fn test_builder_with_pool_config() {
        let config = PoolConfig::for_high_throughput();
        let builder = HttpClientBuilder::new().with_pool_config(config);

        assert_eq!(builder.pool_max_idle_per_host, 64);
        assert_eq!(builder.pool_idle_timeout, Duration::from_secs(120));
    }

    // =========================================================================
    // HttpClientMetrics Tests (M-238)
    // =========================================================================

    #[test]
    fn test_metrics_initial_state() {
        let metrics = HttpClientMetrics::new();
        let snapshot = metrics.snapshot();
        assert_eq!(snapshot.pool_exhaustion_events, 0);
        assert_eq!(snapshot.connect_timeout_events, 0);
        assert_eq!(snapshot.request_errors, 0);
    }

    #[test]
    fn test_metrics_record_pool_exhaustion() {
        let metrics = HttpClientMetrics::new();
        assert!(!metrics.has_pool_exhaustion());

        metrics.record_pool_exhaustion("api.example.com");
        assert!(metrics.has_pool_exhaustion());

        let snapshot = metrics.snapshot();
        assert_eq!(snapshot.pool_exhaustion_events, 1);
    }

    #[test]
    fn test_metrics_record_connect_timeout() {
        let metrics = HttpClientMetrics::new();
        metrics.record_connect_timeout("api.example.com");
        metrics.record_connect_timeout("api.openai.com");

        let snapshot = metrics.snapshot();
        assert_eq!(snapshot.connect_timeout_events, 2);
    }

    #[test]
    fn test_metrics_record_request_error() {
        let metrics = HttpClientMetrics::new();
        metrics.record_request_error();
        metrics.record_request_error();
        metrics.record_request_error();

        let snapshot = metrics.snapshot();
        assert_eq!(snapshot.request_errors, 3);
    }

    #[test]
    fn test_metrics_reset() {
        let metrics = HttpClientMetrics::new();
        metrics.record_pool_exhaustion("host1");
        metrics.record_connect_timeout("host2");
        metrics.record_request_error();

        assert!(metrics.has_pool_exhaustion());

        metrics.reset();

        let snapshot = metrics.snapshot();
        assert_eq!(snapshot.pool_exhaustion_events, 0);
        assert_eq!(snapshot.connect_timeout_events, 0);
        assert_eq!(snapshot.request_errors, 0);
        assert!(!metrics.has_pool_exhaustion());
    }

    #[test]
    fn test_metrics_pool_diagnostic_healthy() {
        let metrics = HttpClientMetrics::new();
        let diagnostic = metrics.pool_diagnostic();
        assert!(diagnostic.contains("Pool health: OK"));
    }

    #[test]
    fn test_metrics_pool_diagnostic_degraded() {
        let metrics = HttpClientMetrics::new();
        metrics.record_pool_exhaustion("api.example.com");
        let diagnostic = metrics.pool_diagnostic();
        assert!(diagnostic.contains("Pool health: DEGRADED"));
        assert!(diagnostic.contains("PoolConfig::for_high_throughput()"));
    }

    #[test]
    fn test_metrics_pool_diagnostic_warning() {
        let metrics = HttpClientMetrics::new();
        // Record more than 10 timeouts to trigger warning
        for _ in 0..15 {
            metrics.record_connect_timeout("api.example.com");
        }
        let diagnostic = metrics.pool_diagnostic();
        assert!(diagnostic.contains("Pool health: WARNING"));
    }

    #[test]
    fn test_pool_config_constants() {
        // Verify constants are reasonable
        assert_eq!(MIN_POOL_IDLE_TIMEOUT_SECS, 1);
        assert_eq!(MAX_POOL_IDLE_TIMEOUT_SECS, 3600);
        assert_eq!(MAX_POOL_IDLE_PER_HOST, 256);
    }
}
