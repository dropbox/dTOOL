//! Error types for DashFlow operations
//!
//! # Error Recovery Guide
//!
//! Each error type has specific recovery strategies. Use [`Error::category()`] to
//! determine the appropriate response:
//!
//! ## By Category
//!
//! ### `AccountBilling` - Account/billing issues
//! **Recovery:** Check account credits, upgrade plan, or contact billing support.
//! These are NOT code bugs - the code is working correctly but the account needs attention.
//!
//! ### `Authentication` - Invalid credentials
//! **Recovery:** Verify API keys are correct, check for expiration, ensure proper environment
//! variable setup. Common fixes:
//! - Set `OPENAI_API_KEY`, `ANTHROPIC_API_KEY`, etc. in environment
//! - Check for trailing whitespace in keys
//! - Verify the key has correct permissions/scopes
//!
//! ### `Network` - Transient infrastructure issues
//! **Recovery:** These are usually temporary. Implement retry with exponential backoff:
//! ```rust
//! use dashflow::core::retry::RetryPolicy;
//! let policy = RetryPolicy::exponential(3);
//! ```
//! For rate limits, respect `Retry-After` headers when available.
//!
//! ### `Validation` - Invalid input or configuration
//! **Recovery:** Check input values against API documentation. Common issues:
//! - Missing required fields
//! - Invalid parameter ranges
//! - Incorrect configuration format
//!
//! ### `CodeBug` - Internal logic errors
//! **Recovery:** Report the error with full context. These indicate bugs that need fixing.
//! Include: stack trace, input values, and DashFlow version.
//!
//! ### `ApiFormat` - Response parsing failures
//! **Recovery:** Usually indicates API version mismatch. Check:
//! - DashFlow version compatibility with provider API
//! - Any recent provider API changes
//!
//! ## By Error Variant
//!
//! | Variant | Retryable | Recovery |
//! |---------|-----------|----------|
//! | `RateLimit` | Yes (with backoff) | Wait and retry, reduce request rate |
//! | `Timeout` | Yes | Increase timeout, retry, check network |
//! | `Network` | Yes | Retry with backoff |
//! | `Authentication` | No | Fix credentials |
//! | `AccountBilling` | No | Fix account |
//! | `InvalidInput` | No | Fix input |
//! | `Serialization` | No | Check data format |
//! | `NotImplemented` | No | Use alternative method |
//!
//! ## Programmatic Recovery
//!
//! ```rust,ignore
//! use dashflow::core::error::{Error, ErrorCategory};
//!
//! fn handle_error(err: Error) -> Result<(), Error> {
//!     match err.category() {
//!         ErrorCategory::Network => {
//!             // Retry with backoff
//!             tracing::warn!("Network error, will retry: {}", err);
//!             Err(err) // Let retry logic handle it
//!         }
//!         ErrorCategory::Authentication => {
//!             // Don't retry - fix credentials
//!             tracing::error!("Auth failed - check API keys: {}", err);
//!             Err(err)
//!         }
//!         ErrorCategory::AccountBilling => {
//!             // Don't retry - account issue
//!             tracing::error!("Account issue - check billing: {}", err);
//!             Err(err)
//!         }
//!         _ => Err(err),
//!     }
//! }
//! ```

use thiserror::Error;

/// Result type alias for DashFlow operations
pub type Result<T> = std::result::Result<T, Error>;

/// Error category for systematic error handling and reporting
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ErrorCategory {
    /// Account/billing issues (insufficient credits, quota exceeded)
    /// These are NOT code bugs - account needs credits or upgrade
    AccountBilling,

    /// Authentication/authorization issues (invalid keys, expired tokens)
    /// These are configuration issues, not code bugs
    Authentication,

    /// Actual code bugs (panics, logic errors, type errors)
    /// These need code fixes
    CodeBug,

    /// API format mismatches (parsing errors, unexpected fields)
    /// These indicate API spec changes or implementation issues
    ApiFormat,

    /// Network/infrastructure issues (timeouts, connection refused)
    /// These are environmental, not code issues
    Network,

    /// Validation errors (invalid input, constraint violations)
    /// These are expected errors from bad user input
    Validation,

    /// Other/unknown errors
    Unknown,
}

impl ErrorCategory {
    /// Get human-readable description of error category
    #[must_use]
    pub fn description(&self) -> &'static str {
        match self {
            ErrorCategory::AccountBilling => "Account/Billing Issue (not a code bug)",
            ErrorCategory::Authentication => "Authentication/Authorization Issue",
            ErrorCategory::CodeBug => "Code Bug (needs fixing)",
            ErrorCategory::ApiFormat => "API Format Mismatch",
            ErrorCategory::Network => "Network/Infrastructure Issue",
            ErrorCategory::Validation => "Validation Error",
            ErrorCategory::Unknown => "Unknown Error",
        }
    }

    /// Check if this error is a code bug that needs fixing
    #[must_use]
    pub fn is_code_bug(&self) -> bool {
        matches!(self, ErrorCategory::CodeBug | ErrorCategory::ApiFormat)
    }

    /// Check if this is an environmental/config issue (not code)
    #[must_use]
    pub fn is_environmental(&self) -> bool {
        matches!(
            self,
            ErrorCategory::AccountBilling | ErrorCategory::Authentication | ErrorCategory::Network
        )
    }
}

/// Specific kind of network error for actionable diagnostics.
///
/// Use [`NetworkErrorKind::from_error_message`] to classify network errors,
/// then [`NetworkErrorKind::diagnostic`] for user-facing guidance.
///
/// # Example
/// ```rust,ignore
/// use dashflow::core::error::{Error, NetworkErrorKind};
///
/// fn handle_network_error(err: &Error) {
///     if let Error::Network(msg) = err {
///         let kind = NetworkErrorKind::from_error_message(msg);
///         eprintln!("Network error: {}", kind.diagnostic());
///     }
/// }
/// ```
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NetworkErrorKind {
    /// DNS resolution failed - hostname could not be resolved to an IP address.
    /// Common causes: typo in hostname, DNS server unreachable, no internet.
    DnsResolution,

    /// Connection was actively refused by the remote host.
    /// Common causes: service not running, wrong port, firewall blocking.
    ConnectionRefused,

    /// Connection attempt timed out before completing.
    /// Common causes: network congestion, host unreachable, firewall dropping packets.
    ConnectionTimeout,

    /// TLS/SSL handshake failed.
    /// Common causes: certificate expired, hostname mismatch, unsupported TLS version.
    TlsHandshake,

    /// Connection was reset by the remote host.
    /// Common causes: server crashed, connection dropped, aggressive timeout.
    ConnectionReset,

    /// Connection pool exhausted - no available connections.
    /// Common causes: too many concurrent requests, connections not being released.
    PoolExhausted,

    /// Proxy connection failed.
    /// Common causes: proxy unreachable, proxy authentication required.
    ProxyError,

    /// Request was redirected too many times.
    /// Common causes: redirect loop, misconfigured server.
    TooManyRedirects,

    /// Other/unclassified network error.
    Other,
}

impl NetworkErrorKind {
    /// Classify a network error from an error message string.
    ///
    /// Examines the error message for common patterns to determine the specific
    /// kind of network failure.
    #[must_use]
    pub fn from_error_message(msg: &str) -> Self {
        let msg_lower = msg.to_lowercase();

        // DNS resolution failures
        if msg_lower.contains("dns error")
            || msg_lower.contains("failed to resolve")
            || msg_lower.contains("no such host")
            || msg_lower.contains("name or service not known")
            || msg_lower.contains("nodename nor servname provided")
            || msg_lower.contains("getaddrinfo")
            || (msg_lower.contains("resolve") && msg_lower.contains("failed"))
        {
            return NetworkErrorKind::DnsResolution;
        }

        // Connection refused
        if msg_lower.contains("connection refused")
            || msg_lower.contains("actively refused")
            || msg_lower.contains("no connection could be made")
        {
            return NetworkErrorKind::ConnectionRefused;
        }

        // Pool exhaustion - check BEFORE connection timeout because pool errors
        // often contain "timeout" or "connection" but should be classified as pool issues.
        // Pool exhaustion in reqwest often manifests as a connect timeout when
        // all connections are busy. These patterns detect explicit pool errors.
        if msg_lower.contains("pool exhausted")
            || msg_lower.contains("pool is full")
            || msg_lower.contains("no available connections")
            || msg_lower.contains("connection limit")
            || msg_lower.contains("max connections")
            || msg_lower.contains("connection pool")
            || msg_lower.contains("acquire connection")
            || msg_lower.contains("acquiring connection")
            || msg_lower.contains("pool timeout")
            || msg_lower.contains("connection checkout")
            || (msg_lower.contains("pool") && msg_lower.contains("exhausted"))
            || (msg_lower.contains("pool") && msg_lower.contains("limit"))
            || (msg_lower.contains("connections") && msg_lower.contains("busy"))
        {
            return NetworkErrorKind::PoolExhausted;
        }

        // Connection timeout
        if msg_lower.contains("connection timed out")
            || msg_lower.contains("connect timeout")
            || msg_lower.contains("operation timed out")
            || (msg_lower.contains("timeout") && msg_lower.contains("connect"))
        {
            return NetworkErrorKind::ConnectionTimeout;
        }

        // TLS/SSL errors
        if msg_lower.contains("tls")
            || msg_lower.contains("ssl")
            || msg_lower.contains("certificate")
            || msg_lower.contains("handshake")
            || msg_lower.contains("secure connection")
        {
            return NetworkErrorKind::TlsHandshake;
        }

        // Connection reset
        if msg_lower.contains("connection reset")
            || msg_lower.contains("reset by peer")
            || msg_lower.contains("broken pipe")
            || msg_lower.contains("connection aborted")
        {
            return NetworkErrorKind::ConnectionReset;
        }

        // Proxy errors
        if msg_lower.contains("proxy") {
            return NetworkErrorKind::ProxyError;
        }

        // Too many redirects
        if msg_lower.contains("redirect") && msg_lower.contains("too many") {
            return NetworkErrorKind::TooManyRedirects;
        }

        NetworkErrorKind::Other
    }

    /// Classify a network error from a reqwest error.
    ///
    /// Uses both reqwest's built-in classification methods and message inspection
    /// for comprehensive error categorization.
    #[must_use]
    pub fn from_reqwest_error(err: &reqwest::Error) -> Self {
        // Check reqwest's built-in classifications first
        if err.is_timeout() {
            return NetworkErrorKind::ConnectionTimeout;
        }

        if err.is_redirect() {
            return NetworkErrorKind::TooManyRedirects;
        }

        // For connect errors, inspect the message for specifics
        if err.is_connect() {
            return Self::from_error_message(&err.to_string());
        }

        // Fall back to message inspection
        Self::from_error_message(&err.to_string())
    }

    /// Get actionable diagnostic message for this error kind.
    ///
    /// Returns a user-friendly message explaining the problem and suggesting fixes.
    #[must_use]
    pub fn diagnostic(&self) -> &'static str {
        match self {
            NetworkErrorKind::DnsResolution => {
                "DNS resolution failed: The hostname could not be resolved to an IP address. \
                Check: (1) hostname spelling, (2) internet connectivity, (3) DNS server availability, \
                (4) /etc/hosts or local DNS configuration."
            }
            NetworkErrorKind::ConnectionRefused => {
                "Connection refused: The remote host actively rejected the connection. \
                Check: (1) service is running on the target host, (2) correct port number, \
                (3) firewall rules allow the connection, (4) service is listening on expected interface."
            }
            NetworkErrorKind::ConnectionTimeout => {
                "Connection timeout: The connection attempt did not complete in time. \
                Check: (1) host is reachable (try ping), (2) network congestion, \
                (3) firewall may be dropping packets silently, (4) increase timeout if appropriate."
            }
            NetworkErrorKind::TlsHandshake => {
                "TLS/SSL handshake failed: Could not establish a secure connection. \
                Check: (1) server certificate validity, (2) hostname matches certificate, \
                (3) TLS version compatibility, (4) system CA certificates are up to date."
            }
            NetworkErrorKind::ConnectionReset => {
                "Connection reset: The remote host closed the connection unexpectedly. \
                Check: (1) server logs for crashes or errors, (2) server-side timeout settings, \
                (3) intermediate proxies/load balancers, (4) request size limits."
            }
            NetworkErrorKind::PoolExhausted => {
                "Connection pool exhausted: No available connections in the pool. \
                This typically occurs under high concurrency when all connections are in use. \
                Check: (1) reduce concurrent requests or add rate limiting, \
                (2) increase pool size via PoolConfig::with_max_idle_per_host() or HttpClientBuilder, \
                (3) ensure connections are being released (responses fully consumed), \
                (4) check for connection leaks (long-running requests blocking pool), \
                (5) consider using PoolConfig::for_high_throughput() for high-concurrency workloads."
            }
            NetworkErrorKind::ProxyError => {
                "Proxy error: Could not connect through the configured proxy. \
                Check: (1) proxy server is reachable, (2) proxy authentication credentials, \
                (3) proxy allows the target host/port, (4) HTTP_PROXY/HTTPS_PROXY env vars."
            }
            NetworkErrorKind::TooManyRedirects => {
                "Too many redirects: The request was redirected too many times. \
                Check: (1) redirect loop on server, (2) correct URL (http vs https), \
                (3) cookie handling for auth redirects, (4) increase redirect limit if legitimate."
            }
            NetworkErrorKind::Other => {
                "Network error: An unspecified network error occurred. \
                Check general connectivity, firewall rules, and server availability."
            }
        }
    }

    /// Get a short label for this error kind.
    #[must_use]
    pub fn label(&self) -> &'static str {
        match self {
            NetworkErrorKind::DnsResolution => "dns_resolution",
            NetworkErrorKind::ConnectionRefused => "connection_refused",
            NetworkErrorKind::ConnectionTimeout => "connection_timeout",
            NetworkErrorKind::TlsHandshake => "tls_handshake",
            NetworkErrorKind::ConnectionReset => "connection_reset",
            NetworkErrorKind::PoolExhausted => "pool_exhausted",
            NetworkErrorKind::ProxyError => "proxy_error",
            NetworkErrorKind::TooManyRedirects => "too_many_redirects",
            NetworkErrorKind::Other => "other",
        }
    }
}

impl std::fmt::Display for NetworkErrorKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.label())
    }
}

/// Core error type for DashFlow operations.
///
/// Use [`Error::category()`] to determine recovery strategy programmatically.
/// See module-level documentation for comprehensive recovery guide.
#[non_exhaustive]
#[derive(Debug, Error)]
pub enum Error {
    /// Account/billing error (insufficient credits, quota exceeded).
    ///
    /// **Recovery:** Check account balance, upgrade plan, or contact support.
    /// Not retryable - requires account action.
    #[error("Account/Billing error: {0}")]
    AccountBilling(String),

    /// Input validation error.
    ///
    /// **Recovery:** Check input against API requirements. Not retryable.
    #[error("Invalid input: {0}")]
    InvalidInput(String),

    /// Configuration error.
    ///
    /// **Recovery:** Review configuration file or builder parameters. Not retryable.
    #[error("Configuration error: {0}")]
    Configuration(String),

    /// Serialization/deserialization error.
    ///
    /// **Recovery:** Check data format matches expected schema. Not retryable.
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    /// I/O error.
    ///
    /// **Recovery:** Check file permissions, disk space, path validity.
    /// May be retryable for transient issues.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// ZIP archive error.
    ///
    /// **Recovery:** Verify archive integrity, check for corruption. Not retryable.
    #[error("ZIP error: {0}")]
    Zip(String),

    /// HTTP request error.
    ///
    /// **Recovery:** Check URL validity, network connectivity. May be retryable.
    #[error("HTTP error: {0}")]
    Http(String),

    /// Network error.
    ///
    /// **Recovery:** Retry with exponential backoff. Check network connectivity.
    /// Usually transient and retryable.
    #[error("Network error: {0}")]
    Network(String),

    /// Authentication/authorization error.
    ///
    /// **Recovery:** Verify API keys, check environment variables, ensure proper scopes.
    /// Not retryable - requires credential fix.
    #[error("Authentication error: {0}")]
    Authentication(String),

    /// API error (from LLM providers) - generic.
    ///
    /// **Recovery:** Check [`Error::category()`] for specific handling - may be
    /// auth, billing, or network depending on message content.
    #[error("API error: {0}")]
    Api(String),

    /// API format mismatch (parsing error, unexpected response).
    ///
    /// **Recovery:** Check DashFlow version compatibility with provider API.
    /// May indicate provider API changes. Report if persistent.
    #[error("API format error: {0}")]
    ApiFormat(String),

    /// Rate limit error.
    ///
    /// **Recovery:** Implement exponential backoff, reduce request rate,
    /// respect `Retry-After` headers. Retryable after waiting.
    #[error("Rate limit exceeded: {0}")]
    RateLimit(String),

    /// Timeout error.
    ///
    /// **Recovery:** Increase timeout setting, retry, or check network latency.
    /// Retryable - consider larger timeout for complex operations.
    #[error("Operation timed out: {0}")]
    Timeout(String),

    /// Callback execution error.
    ///
    /// **Recovery:** Check callback function for panics or errors.
    /// Review callback logic and input handling.
    #[error("Callback error: {0}")]
    Callback(String),

    /// Runnable execution error.
    ///
    /// **Recovery:** Check runnable chain configuration. May indicate logic bug.
    /// Review input/output types in the chain.
    #[error("Runnable execution failed: {0}")]
    RunnableExecution(String),

    /// Tool execution error.
    ///
    /// **Recovery:** Check tool implementation and input format.
    /// Verify tool dependencies are available.
    #[error("Tool execution failed: {0}")]
    ToolExecution(String),

    /// Output parsing error.
    ///
    /// **Recovery:** Check output parser matches LLM output format.
    /// Consider using more forgiving parser or structured output.
    #[error("Output parsing failed: {0}")]
    OutputParsing(String),

    /// Query parsing error (structured queries).
    ///
    /// **Recovery:** Check query syntax against expected format.
    #[error("Query parsing failed: {0}")]
    ParseError(String),

    /// Agent execution error.
    ///
    /// **Recovery:** Check agent configuration, tool bindings, and LLM settings.
    /// May indicate tool errors or response parsing issues.
    #[error("Agent error: {0}")]
    Agent(String),

    /// Not implemented error.
    ///
    /// **Recovery:** Use alternative method or wait for feature implementation.
    /// Check if feature is available in newer version.
    #[error("Not implemented: {0}")]
    NotImplemented(String),

    /// Context limit exceeded error.
    ///
    /// **Recovery:** Reduce input size, use shorter messages, or enable auto-truncation.
    /// Consider using a model with larger context window.
    #[error("Context limit exceeded: {token_count} tokens > {limit} limit for model {model}")]
    ContextLimitExceeded {
        /// Number of tokens in the input
        token_count: usize,
        /// Maximum allowed tokens
        limit: usize,
        /// Model name
        model: String,
    },

    /// Generic error for anything else.
    ///
    /// **Recovery:** Examine error message for details. Consider filing a bug
    /// report if the error should have a more specific type.
    #[error("{0}")]
    Other(String),
}

impl Error {
    /// Get the category of this error
    #[must_use]
    pub fn category(&self) -> ErrorCategory {
        match self {
            Error::AccountBilling(_) => ErrorCategory::AccountBilling,
            Error::Authentication(_) => ErrorCategory::Authentication,
            Error::ApiFormat(_) => ErrorCategory::ApiFormat,
            Error::Network(_) => ErrorCategory::Network,
            Error::InvalidInput(_)
            | Error::Configuration(_)
            | Error::ContextLimitExceeded { .. } => ErrorCategory::Validation,
            Error::RunnableExecution(_) | Error::ToolExecution(_) => ErrorCategory::CodeBug,
            Error::Api(msg) => {
                // Inspect API error messages to categorize properly
                let msg_lower = msg.to_lowercase();

                // Check for authentication/authorization issues
                if msg_lower.contains("invalid api key")
                    || msg_lower.contains("invalid_api_key")
                    || msg_lower.contains("unauthorized")
                    || msg_lower.contains("authentication")
                    || msg_lower.contains("invalid api-key")
                    || msg_lower.contains("api key") && msg_lower.contains("invalid")
                {
                    return ErrorCategory::Authentication;
                }

                // Check for account/billing issues
                if msg_lower.contains("credit balance")
                    || msg_lower.contains("insufficient credits")
                    || msg_lower.contains("quota exceeded")
                    || msg_lower.contains("billing")
                    || msg_lower.contains("payment required")
                    || msg_lower.contains("upgrade") && msg_lower.contains("plan")
                {
                    return ErrorCategory::AccountBilling;
                }

                // Check for network/infrastructure issues (transient failures)
                // These include: empty responses, deserialization failures, connection issues
                if msg_lower.contains("failed to deserialize")
                    || msg_lower.contains("expected value at line")
                    || msg_lower.contains("connection refused")
                    || msg_lower.contains("connection reset")
                    || msg_lower.contains("connection closed")
                    || msg_lower.contains("eof while parsing")
                    || msg_lower.contains("unexpected end of")
                    || msg_lower.contains("empty response")
                    || msg_lower.contains("network error")
                    || msg_lower.contains("dns error")
                    || msg_lower.contains("timed out")
                    || msg_lower.contains("timeout")
                    || msg_lower.contains("rate limit")
                    || msg_lower.contains("too many requests")
                    || msg_lower.contains("429")
                {
                    return ErrorCategory::Network;
                }

                // Otherwise, API errors are unknown category
                ErrorCategory::Unknown
            }
            // Rate limits and timeouts are network/infrastructure issues, not code bugs
            Error::RateLimit(_) | Error::Timeout(_) => ErrorCategory::Network,
            _ => ErrorCategory::Unknown,
        }
    }

    /// Get status message about this error for debugging
    #[must_use]
    pub fn status_message(&self) -> String {
        format!("[{}] {}", self.category().description(), self)
    }

    /// Check if this error indicates a code bug that needs fixing
    #[must_use]
    pub fn is_code_bug(&self) -> bool {
        self.category().is_code_bug()
    }

    /// Check if this is an environmental/config issue (not a code bug)
    #[must_use]
    pub fn is_environmental(&self) -> bool {
        self.category().is_environmental()
    }

    /// Check if this error is potentially recoverable via retry.
    ///
    /// Returns `true` for transient errors (network, timeout, rate limit) that
    /// may succeed on retry with appropriate backoff. Returns `false` for errors
    /// that require configuration changes or bug fixes.
    ///
    /// # Example
    /// ```rust,ignore
    /// if err.is_retryable() {
    ///     // Use exponential backoff and retry
    ///     tokio::time::sleep(Duration::from_millis(100 * 2u64.pow(attempt))).await;
    /// } else {
    ///     // Don't retry - fix the underlying issue
    ///     return Err(err);
    /// }
    /// ```
    #[must_use]
    pub fn is_retryable(&self) -> bool {
        matches!(
            self,
            Error::Network(_)
                | Error::Timeout(_)
                | Error::RateLimit(_)
                | Error::Http(_)
                | Error::Io(_)
        ) || (matches!(self, Error::Api(_)) && self.category() == ErrorCategory::Network)
    }

    /// Get the specific kind of network error with actionable diagnostics.
    ///
    /// Returns `Some(NetworkErrorKind)` for network-related errors (`Network`, `Timeout`,
    /// `Http`), or `None` for other error types.
    ///
    /// # Example
    /// ```rust,ignore
    /// use dashflow::core::error::Error;
    ///
    /// fn handle_error(err: &Error) {
    ///     if let Some(kind) = err.network_error_kind() {
    ///         eprintln!("Network error ({}): {}", kind.label(), kind.diagnostic());
    ///     }
    /// }
    /// ```
    #[must_use]
    pub fn network_error_kind(&self) -> Option<NetworkErrorKind> {
        match self {
            Error::Network(msg) => Some(NetworkErrorKind::from_error_message(msg)),
            Error::Timeout(_) => Some(NetworkErrorKind::ConnectionTimeout),
            Error::Http(msg) => Some(NetworkErrorKind::from_error_message(msg)),
            Error::Api(msg) if self.category() == ErrorCategory::Network => {
                Some(NetworkErrorKind::from_error_message(msg))
            }
            _ => None,
        }
    }

    /// Get actionable diagnostic for this error.
    ///
    /// For network-related errors, provides specific guidance based on the failure type.
    /// For other errors, returns the category description.
    ///
    /// # Example
    /// ```rust,ignore
    /// eprintln!("Error diagnostic: {}", err.diagnostic());
    /// ```
    #[must_use]
    pub fn diagnostic(&self) -> String {
        if let Some(kind) = self.network_error_kind() {
            kind.diagnostic().to_string()
        } else {
            self.category().description().to_string()
        }
    }

    /// Create an account/billing error
    pub fn account_billing<S: Into<String>>(msg: S) -> Self {
        Self::AccountBilling(msg.into())
    }

    /// Create an authentication error
    pub fn authentication<S: Into<String>>(msg: S) -> Self {
        Self::Authentication(msg.into())
    }

    /// Create an API format error
    pub fn api_format<S: Into<String>>(msg: S) -> Self {
        Self::ApiFormat(msg.into())
    }

    /// Create an API error
    pub fn api<S: Into<String>>(msg: S) -> Self {
        Self::Api(msg.into())
    }

    /// Create a network error
    pub fn network<S: Into<String>>(msg: S) -> Self {
        Self::Network(msg.into())
    }

    /// Create a configuration error
    pub fn config<S: Into<String>>(msg: S) -> Self {
        Self::Configuration(msg.into())
    }

    /// Create an invalid input error
    pub fn invalid_input<S: Into<String>>(msg: S) -> Self {
        Self::InvalidInput(msg.into())
    }

    /// Create a rate limit error
    pub fn rate_limit<S: Into<String>>(msg: S) -> Self {
        Self::RateLimit(msg.into())
    }

    /// Create a timeout error
    pub fn timeout<S: Into<String>>(msg: S) -> Self {
        Self::Timeout(msg.into())
    }

    /// Create an HTTP error
    pub fn http<S: Into<String>>(msg: S) -> Self {
        Self::Http(msg.into())
    }

    /// Create a generic error
    pub fn other<S: Into<String>>(msg: S) -> Self {
        Self::Other(msg.into())
    }

    /// Create a tool error
    pub fn tool_error<S: Into<String>>(msg: S) -> Self {
        Self::ToolExecution(msg.into())
    }

    /// Create an agent error
    pub fn agent<S: Into<String>>(msg: S) -> Self {
        Self::Agent(msg.into())
    }

    /// Create an Elasticsearch error (maps to Api for proper error handling)
    pub fn elasticsearch<S: Into<String>>(msg: S) -> Self {
        Self::Api(format!("Elasticsearch: {}", msg.into()))
    }
}

#[cfg(test)]
mod tests {
    use crate::test_prelude::*;

    #[test]
    fn test_error_constructors() {
        let err = Error::api("test error");
        assert!(matches!(err, Error::Api(_)));

        let err = Error::invalid_input("bad input");
        assert!(matches!(err, Error::InvalidInput(_)));

        let err = Error::rate_limit("too many requests");
        assert!(matches!(err, Error::RateLimit(_)));
    }

    #[test]
    fn test_error_display() {
        let err = Error::api("test");
        assert_eq!(err.to_string(), "API error: test");

        let err = Error::invalid_input("invalid");
        assert_eq!(err.to_string(), "Invalid input: invalid");
    }
}

#[cfg(test)]
mod tests_categorization {
    use crate::test_prelude::*;

    #[test]
    fn test_error_categories() {
        let billing_err = Error::account_billing("Insufficient credits");
        assert_eq!(billing_err.category(), ErrorCategory::AccountBilling);
        assert!(billing_err.is_environmental());
        assert!(!billing_err.is_code_bug());

        let auth_err = Error::authentication("Invalid API key");
        assert_eq!(auth_err.category(), ErrorCategory::Authentication);
        assert!(auth_err.is_environmental());

        let code_err = Error::RunnableExecution("Logic error".to_string());
        assert_eq!(code_err.category(), ErrorCategory::CodeBug);
        assert!(code_err.is_code_bug());

        let api_format_err = Error::api_format("Unexpected field");
        assert_eq!(api_format_err.category(), ErrorCategory::ApiFormat);
        assert!(api_format_err.is_code_bug());
    }

    #[test]
    fn test_status_messages() {
        let err = Error::account_billing("Insufficient credits");
        let msg = err.status_message();
        assert!(msg.contains("Account/Billing"));
        assert!(msg.contains("not a code bug"));
    }

    #[test]
    fn test_api_error_categorization_authentication() {
        // Test various authentication error message patterns
        let auth_patterns = vec![
            "invalid api key",
            "Invalid API-Key provided",
            "invalid_api_key",
            "Unauthorized access",
            "Authentication failed",
            "API key is invalid",
        ];

        for pattern in auth_patterns {
            let err = Error::api(pattern);
            assert_eq!(
                err.category(),
                ErrorCategory::Authentication,
                "Pattern '{}' should be categorized as Authentication",
                pattern
            );
        }
    }

    #[test]
    fn test_api_error_categorization_billing() {
        // Test various billing error message patterns
        let billing_patterns = vec![
            "credit balance too low",
            "Insufficient credits",
            "quota exceeded",
            "Billing issue detected",
            "Payment required",
            "Please upgrade your plan",
        ];

        for pattern in billing_patterns {
            let err = Error::api(pattern);
            assert_eq!(
                err.category(),
                ErrorCategory::AccountBilling,
                "Pattern '{}' should be categorized as AccountBilling",
                pattern
            );
        }
    }

    #[test]
    fn test_api_error_categorization_unknown() {
        // Test that generic API errors are categorized as Unknown
        let generic_patterns = vec![
            "Something went wrong",
            "Internal server error",
            "Request failed",
        ];

        for pattern in generic_patterns {
            let err = Error::api(pattern);
            assert_eq!(
                err.category(),
                ErrorCategory::Unknown,
                "Pattern '{}' should be categorized as Unknown",
                pattern
            );
        }
    }

    #[test]
    fn test_network_category_errors() {
        let rate_limit = Error::rate_limit("Too many requests");
        assert_eq!(rate_limit.category(), ErrorCategory::Network);
        assert!(rate_limit.is_environmental());

        let timeout = Error::timeout("Request timed out");
        assert_eq!(timeout.category(), ErrorCategory::Network);
        assert!(timeout.is_environmental());

        let network = Error::network("Connection refused");
        assert_eq!(network.category(), ErrorCategory::Network);
        assert!(network.is_environmental());
    }

    #[test]
    fn test_validation_category_errors() {
        let invalid_input = Error::invalid_input("Field missing");
        assert_eq!(invalid_input.category(), ErrorCategory::Validation);

        let config = Error::config("Invalid configuration");
        assert_eq!(config.category(), ErrorCategory::Validation);
    }

    #[test]
    fn test_error_category_description() {
        assert_eq!(
            ErrorCategory::AccountBilling.description(),
            "Account/Billing Issue (not a code bug)"
        );
        assert_eq!(
            ErrorCategory::Authentication.description(),
            "Authentication/Authorization Issue"
        );
        assert_eq!(
            ErrorCategory::CodeBug.description(),
            "Code Bug (needs fixing)"
        );
        assert_eq!(
            ErrorCategory::ApiFormat.description(),
            "API Format Mismatch"
        );
        assert_eq!(
            ErrorCategory::Network.description(),
            "Network/Infrastructure Issue"
        );
        assert_eq!(ErrorCategory::Validation.description(), "Validation Error");
        assert_eq!(ErrorCategory::Unknown.description(), "Unknown Error");
    }

    #[test]
    fn test_error_category_is_code_bug() {
        assert!(!ErrorCategory::AccountBilling.is_code_bug());
        assert!(!ErrorCategory::Authentication.is_code_bug());
        assert!(ErrorCategory::CodeBug.is_code_bug());
        assert!(ErrorCategory::ApiFormat.is_code_bug());
        assert!(!ErrorCategory::Network.is_code_bug());
        assert!(!ErrorCategory::Validation.is_code_bug());
        assert!(!ErrorCategory::Unknown.is_code_bug());
    }

    #[test]
    fn test_error_category_is_environmental() {
        assert!(ErrorCategory::AccountBilling.is_environmental());
        assert!(ErrorCategory::Authentication.is_environmental());
        assert!(!ErrorCategory::CodeBug.is_environmental());
        assert!(!ErrorCategory::ApiFormat.is_environmental());
        assert!(ErrorCategory::Network.is_environmental());
        assert!(!ErrorCategory::Validation.is_environmental());
        assert!(!ErrorCategory::Unknown.is_environmental());
    }

    #[test]
    fn test_error_is_retryable() {
        // Retryable errors
        assert!(Error::network("connection refused").is_retryable());
        assert!(Error::timeout("request timed out").is_retryable());
        assert!(Error::rate_limit("too many requests").is_retryable());
        assert!(Error::http("503 Service Unavailable").is_retryable());

        // API errors that are network-related should be retryable
        let api_network_err = Error::api("connection refused");
        assert!(api_network_err.is_retryable());

        // Non-retryable errors
        assert!(!Error::authentication("invalid key").is_retryable());
        assert!(!Error::account_billing("insufficient credits").is_retryable());
        assert!(!Error::invalid_input("bad input").is_retryable());
        assert!(!Error::config("bad config").is_retryable());
        assert!(!Error::api_format("unexpected response").is_retryable());

        // Generic API errors (unknown category) are not retryable
        let generic_api = Error::api("some unknown error");
        assert!(!generic_api.is_retryable());
    }

    #[test]
    fn test_api_error_message_categorization() {
        // API errors with authentication keywords -> Authentication
        let auth_err = Error::api("OpenAI API error: invalid api key");
        assert_eq!(auth_err.category(), ErrorCategory::Authentication);
        assert!(auth_err.is_environmental());

        // API errors with billing keywords -> AccountBilling
        let billing_err = Error::api("OpenAI API error: insufficient credits");
        assert_eq!(billing_err.category(), ErrorCategory::AccountBilling);
        assert!(billing_err.is_environmental());

        // API errors with deserialization/network failures -> Network (environmental)
        let deser_err =
            Error::api("OpenAI API error: failed to deserialize api response: expected value at line 1 column 1");
        assert_eq!(deser_err.category(), ErrorCategory::Network);
        assert!(deser_err.is_environmental());

        let conn_err = Error::api("API error: connection refused");
        assert_eq!(conn_err.category(), ErrorCategory::Network);
        assert!(conn_err.is_environmental());

        let timeout_err = Error::api("API error: request timed out");
        assert_eq!(timeout_err.category(), ErrorCategory::Network);
        assert!(timeout_err.is_environmental());

        let eof_err = Error::api("API error: eof while parsing response");
        assert_eq!(eof_err.category(), ErrorCategory::Network);
        assert!(eof_err.is_environmental());

        // Generic API errors -> Unknown (not environmental)
        let generic_err = Error::api("Some other API error");
        assert_eq!(generic_err.category(), ErrorCategory::Unknown);
        assert!(!generic_err.is_environmental());
    }
}

#[cfg(test)]
mod tests_constructors {
    use crate::test_prelude::*;

    #[test]
    fn test_account_billing_constructor() {
        let err = Error::account_billing("Test billing error");
        assert!(matches!(err, Error::AccountBilling(_)));
        assert_eq!(err.to_string(), "Account/Billing error: Test billing error");
    }

    #[test]
    fn test_authentication_constructor() {
        let err = Error::authentication("Invalid token");
        assert!(matches!(err, Error::Authentication(_)));
        assert_eq!(err.to_string(), "Authentication error: Invalid token");
    }

    #[test]
    fn test_api_format_constructor() {
        let err = Error::api_format("Missing field");
        assert!(matches!(err, Error::ApiFormat(_)));
        assert_eq!(err.to_string(), "API format error: Missing field");
    }

    #[test]
    fn test_network_constructor() {
        let err = Error::network("Connection failed");
        assert!(matches!(err, Error::Network(_)));
        assert_eq!(err.to_string(), "Network error: Connection failed");
    }

    #[test]
    fn test_http_constructor() {
        let err = Error::http("404 Not Found");
        assert!(matches!(err, Error::Http(_)));
        assert_eq!(err.to_string(), "HTTP error: 404 Not Found");
    }

    #[test]
    fn test_other_constructor() {
        let err = Error::other("Generic error");
        assert!(matches!(err, Error::Other(_)));
        assert_eq!(err.to_string(), "Generic error");
    }

    #[test]
    fn test_tool_error_constructor() {
        let err = Error::tool_error("Tool failed");
        assert!(matches!(err, Error::ToolExecution(_)));
        assert_eq!(err.to_string(), "Tool execution failed: Tool failed");
    }

    #[test]
    fn test_agent_constructor() {
        let err = Error::agent("Agent error");
        assert!(matches!(err, Error::Agent(_)));
        assert_eq!(err.to_string(), "Agent error: Agent error");
    }

    #[test]
    fn test_elasticsearch_constructor() {
        let err = Error::elasticsearch("Connection refused");
        assert!(matches!(err, Error::Api(_)));
        assert_eq!(
            err.to_string(),
            "API error: Elasticsearch: Connection refused"
        );
    }
}

#[cfg(test)]
mod tests_display {
    use crate::test_prelude::*;

    #[test]
    fn test_all_error_display_formats() {
        // Test all error variants have proper Display implementations
        let errors = vec![
            (
                Error::AccountBilling("test".into()),
                "Account/Billing error: test",
            ),
            (Error::InvalidInput("test".into()), "Invalid input: test"),
            (
                Error::Configuration("test".into()),
                "Configuration error: test",
            ),
            (Error::Zip("test".into()), "ZIP error: test"),
            (Error::Http("test".into()), "HTTP error: test"),
            (Error::Network("test".into()), "Network error: test"),
            (
                Error::Authentication("test".into()),
                "Authentication error: test",
            ),
            (Error::Api("test".into()), "API error: test"),
            (Error::ApiFormat("test".into()), "API format error: test"),
            (Error::RateLimit("test".into()), "Rate limit exceeded: test"),
            (Error::Timeout("test".into()), "Operation timed out: test"),
            (Error::Callback("test".into()), "Callback error: test"),
            (
                Error::RunnableExecution("test".into()),
                "Runnable execution failed: test",
            ),
            (
                Error::ToolExecution("test".into()),
                "Tool execution failed: test",
            ),
            (
                Error::OutputParsing("test".into()),
                "Output parsing failed: test",
            ),
            (
                Error::ParseError("test".into()),
                "Query parsing failed: test",
            ),
            (Error::Agent("test".into()), "Agent error: test"),
            (
                Error::NotImplemented("test".into()),
                "Not implemented: test",
            ),
            (Error::Other("test".into()), "test"),
        ];

        for (err, expected) in errors {
            assert_eq!(err.to_string(), expected);
        }
    }

    #[test]
    fn test_status_message_format() {
        let err = Error::network("Connection refused");
        let msg = err.status_message();
        assert!(msg.contains("Network/Infrastructure Issue"));
        assert!(msg.contains("Connection refused"));
    }
}

#[cfg(test)]
mod tests_from_impls {
    use crate::test_prelude::*;

    #[test]
    fn test_from_serde_json_error() {
        let json_err: std::result::Result<serde_json::Value, serde_json::Error> =
            serde_json::from_str("invalid json");
        let err: Error = json_err.unwrap_err().into();
        assert!(matches!(err, Error::Serialization(_)));
    }

    #[test]
    fn test_from_io_error() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
        let err: Error = io_err.into();
        assert!(matches!(err, Error::Io(_)));
        assert!(err.to_string().contains("file not found"));
    }
}

#[cfg(test)]
mod tests_error_category_equality {
    use crate::test_prelude::*;

    #[test]
    fn test_error_category_eq() {
        assert_eq!(ErrorCategory::AccountBilling, ErrorCategory::AccountBilling);
        assert_ne!(ErrorCategory::AccountBilling, ErrorCategory::Authentication);
    }

    #[test]
    fn test_error_category_clone() {
        let cat = ErrorCategory::CodeBug;
        let cloned = cat.clone();
        assert_eq!(cat, cloned);
    }

    #[test]
    fn test_error_category_debug() {
        let cat = ErrorCategory::Network;
        let debug_str = format!("{:?}", cat);
        assert_eq!(debug_str, "Network");
    }
}

#[cfg(test)]
mod tests_network_error_kind {
    use super::*;

    #[test]
    fn test_dns_resolution_detection() {
        assert_eq!(
            NetworkErrorKind::from_error_message("dns error: failed to lookup address"),
            NetworkErrorKind::DnsResolution
        );
        assert_eq!(
            NetworkErrorKind::from_error_message("failed to resolve hostname"),
            NetworkErrorKind::DnsResolution
        );
        assert_eq!(
            NetworkErrorKind::from_error_message("no such host"),
            NetworkErrorKind::DnsResolution
        );
        assert_eq!(
            NetworkErrorKind::from_error_message("name or service not known"),
            NetworkErrorKind::DnsResolution
        );
        assert_eq!(
            NetworkErrorKind::from_error_message("getaddrinfo failed"),
            NetworkErrorKind::DnsResolution
        );
    }

    #[test]
    fn test_connection_refused_detection() {
        assert_eq!(
            NetworkErrorKind::from_error_message("connection refused"),
            NetworkErrorKind::ConnectionRefused
        );
        assert_eq!(
            NetworkErrorKind::from_error_message("actively refused by target machine"),
            NetworkErrorKind::ConnectionRefused
        );
    }

    #[test]
    fn test_connection_timeout_detection() {
        assert_eq!(
            NetworkErrorKind::from_error_message("connection timed out"),
            NetworkErrorKind::ConnectionTimeout
        );
        assert_eq!(
            NetworkErrorKind::from_error_message("connect timeout after 10s"),
            NetworkErrorKind::ConnectionTimeout
        );
        assert_eq!(
            NetworkErrorKind::from_error_message("operation timed out"),
            NetworkErrorKind::ConnectionTimeout
        );
    }

    #[test]
    fn test_tls_detection() {
        assert_eq!(
            NetworkErrorKind::from_error_message("tls handshake failed"),
            NetworkErrorKind::TlsHandshake
        );
        assert_eq!(
            NetworkErrorKind::from_error_message("ssl certificate verify failed"),
            NetworkErrorKind::TlsHandshake
        );
        assert_eq!(
            NetworkErrorKind::from_error_message("certificate has expired"),
            NetworkErrorKind::TlsHandshake
        );
    }

    #[test]
    fn test_connection_reset_detection() {
        assert_eq!(
            NetworkErrorKind::from_error_message("connection reset by peer"),
            NetworkErrorKind::ConnectionReset
        );
        assert_eq!(
            NetworkErrorKind::from_error_message("broken pipe"),
            NetworkErrorKind::ConnectionReset
        );
        assert_eq!(
            NetworkErrorKind::from_error_message("connection aborted"),
            NetworkErrorKind::ConnectionReset
        );
    }

    #[test]
    fn test_pool_exhausted_detection() {
        // Basic patterns
        assert_eq!(
            NetworkErrorKind::from_error_message("connection pool exhausted"),
            NetworkErrorKind::PoolExhausted
        );
        assert_eq!(
            NetworkErrorKind::from_error_message("no available connections in pool"),
            NetworkErrorKind::PoolExhausted
        );
        // Additional M-238 patterns
        assert_eq!(
            NetworkErrorKind::from_error_message("pool exhausted"),
            NetworkErrorKind::PoolExhausted
        );
        assert_eq!(
            NetworkErrorKind::from_error_message("pool is full"),
            NetworkErrorKind::PoolExhausted
        );
        assert_eq!(
            NetworkErrorKind::from_error_message("connection limit reached"),
            NetworkErrorKind::PoolExhausted
        );
        assert_eq!(
            NetworkErrorKind::from_error_message("max connections exceeded"),
            NetworkErrorKind::PoolExhausted
        );
        assert_eq!(
            NetworkErrorKind::from_error_message("connection pool timeout"),
            NetworkErrorKind::PoolExhausted
        );
        assert_eq!(
            NetworkErrorKind::from_error_message("failed to acquire connection"),
            NetworkErrorKind::PoolExhausted
        );
        assert_eq!(
            NetworkErrorKind::from_error_message("acquiring connection timed out"),
            NetworkErrorKind::PoolExhausted
        );
        assert_eq!(
            NetworkErrorKind::from_error_message("pool timeout waiting for connection"),
            NetworkErrorKind::PoolExhausted
        );
        assert_eq!(
            NetworkErrorKind::from_error_message("connection checkout failed"),
            NetworkErrorKind::PoolExhausted
        );
        assert_eq!(
            NetworkErrorKind::from_error_message("all connections busy"),
            NetworkErrorKind::PoolExhausted
        );
        // Combined patterns
        assert_eq!(
            NetworkErrorKind::from_error_message("pool limit exceeded"),
            NetworkErrorKind::PoolExhausted
        );
    }

    #[test]
    fn test_proxy_detection() {
        assert_eq!(
            NetworkErrorKind::from_error_message("proxy connection failed"),
            NetworkErrorKind::ProxyError
        );
    }

    #[test]
    fn test_redirect_detection() {
        assert_eq!(
            NetworkErrorKind::from_error_message("too many redirects"),
            NetworkErrorKind::TooManyRedirects
        );
    }

    #[test]
    fn test_other_detection() {
        assert_eq!(
            NetworkErrorKind::from_error_message("some generic error"),
            NetworkErrorKind::Other
        );
    }

    #[test]
    fn test_label() {
        assert_eq!(NetworkErrorKind::DnsResolution.label(), "dns_resolution");
        assert_eq!(
            NetworkErrorKind::ConnectionRefused.label(),
            "connection_refused"
        );
        assert_eq!(
            NetworkErrorKind::ConnectionTimeout.label(),
            "connection_timeout"
        );
        assert_eq!(NetworkErrorKind::TlsHandshake.label(), "tls_handshake");
        assert_eq!(NetworkErrorKind::ConnectionReset.label(), "connection_reset");
        assert_eq!(NetworkErrorKind::PoolExhausted.label(), "pool_exhausted");
        assert_eq!(NetworkErrorKind::ProxyError.label(), "proxy_error");
        assert_eq!(
            NetworkErrorKind::TooManyRedirects.label(),
            "too_many_redirects"
        );
        assert_eq!(NetworkErrorKind::Other.label(), "other");
    }

    #[test]
    fn test_display() {
        assert_eq!(
            format!("{}", NetworkErrorKind::DnsResolution),
            "dns_resolution"
        );
        assert_eq!(
            format!("{}", NetworkErrorKind::ConnectionRefused),
            "connection_refused"
        );
    }

    #[test]
    fn test_diagnostic_not_empty() {
        // All diagnostics should be non-empty and provide actionable info
        for kind in [
            NetworkErrorKind::DnsResolution,
            NetworkErrorKind::ConnectionRefused,
            NetworkErrorKind::ConnectionTimeout,
            NetworkErrorKind::TlsHandshake,
            NetworkErrorKind::ConnectionReset,
            NetworkErrorKind::PoolExhausted,
            NetworkErrorKind::ProxyError,
            NetworkErrorKind::TooManyRedirects,
            NetworkErrorKind::Other,
        ] {
            let diagnostic = kind.diagnostic();
            assert!(!diagnostic.is_empty(), "{:?} has empty diagnostic", kind);
            assert!(
                diagnostic.contains("Check"),
                "{:?} diagnostic should contain 'Check'",
                kind
            );
        }
    }

    #[test]
    fn test_error_network_error_kind() {
        let dns_err = Error::network("dns error: failed to resolve api.example.com");
        assert_eq!(
            dns_err.network_error_kind(),
            Some(NetworkErrorKind::DnsResolution)
        );

        let timeout_err = Error::timeout("request timed out");
        assert_eq!(
            timeout_err.network_error_kind(),
            Some(NetworkErrorKind::ConnectionTimeout)
        );

        let auth_err = Error::authentication("invalid api key");
        assert_eq!(auth_err.network_error_kind(), None);
    }

    #[test]
    fn test_error_diagnostic() {
        let dns_err = Error::network("dns error: failed to resolve");
        let diagnostic = dns_err.diagnostic();
        assert!(diagnostic.contains("DNS"));
        assert!(diagnostic.contains("Check"));

        let auth_err = Error::authentication("invalid key");
        let diagnostic = auth_err.diagnostic();
        assert!(diagnostic.contains("Authentication"));
    }

    #[test]
    fn test_clone_and_eq() {
        let kind = NetworkErrorKind::DnsResolution;
        let cloned = kind.clone();
        assert_eq!(kind, cloned);
    }

    #[test]
    fn test_debug() {
        let kind = NetworkErrorKind::ConnectionRefused;
        let debug = format!("{:?}", kind);
        assert_eq!(debug, "ConnectionRefused");
    }
}

// Additional From implementations for common error types

impl From<zip::result::ZipError> for Error {
    fn from(err: zip::result::ZipError) -> Self {
        Error::Zip(err.to_string())
    }
}

impl From<reqwest::Error> for Error {
    fn from(err: reqwest::Error) -> Self {
        // Use NetworkErrorKind for detailed classification
        let kind = NetworkErrorKind::from_reqwest_error(&err);
        let base_msg = err.to_string();

        // Extract URL host for better diagnostics
        let url_info = err
            .url()
            .map(|u| format!(" (host: {})", u.host_str().unwrap_or("unknown")))
            .unwrap_or_default();

        if err.is_timeout() {
            Error::Timeout(format!("{}{}", base_msg, url_info))
        } else if err.is_connect() || err.is_request() {
            // Include the specific error kind in the message for easier debugging
            Error::Network(format!("[{}] {}{}", kind.label(), base_msg, url_info))
        } else if err.is_status() {
            Error::Http(format!("{}{}", base_msg, url_info))
        } else if err.is_redirect() {
            Error::Network(format!("[{}] {}{}", kind.label(), base_msg, url_info))
        } else {
            Error::Other(format!("{}{}", base_msg, url_info))
        }
    }
}

/// Convert from graph error to core error
impl From<crate::error::Error> for Error {
    fn from(err: crate::error::Error) -> Self {
        match err {
            // If it's already a core error, unwrap it
            crate::error::Error::Core(core_err) => core_err,
            // Map graph-specific errors to appropriate core errors
            crate::error::Error::Validation(msg) => Error::InvalidInput(msg),
            crate::error::Error::NodeExecution { node, source } => {
                Error::Other(format!("Node execution error in '{}': {}", node, source))
            }
            crate::error::Error::NoEntryPoint => {
                Error::Configuration("Graph has no entry point defined".to_string())
            }
            crate::error::Error::NodeNotFound(name) => {
                Error::InvalidInput(format!("Node '{}' not found in graph", name))
            }
            crate::error::Error::DuplicateNodeName(name) => {
                Error::InvalidInput(format!("Node '{}' already exists in graph", name))
            }
            crate::error::Error::CycleDetected(msg) => Error::InvalidInput(msg),
            crate::error::Error::InvalidEdge(msg) => Error::InvalidInput(msg),
            crate::error::Error::Timeout(duration) => {
                Error::Timeout(format!("Execution timeout after {:?}", duration))
            }
            crate::error::Error::Serialization(e) => Error::Serialization(e),
            crate::error::Error::InterruptWithoutCheckpointer(node) => Error::Configuration(
                format!("Cannot interrupt at node '{}' without checkpointer", node),
            ),
            crate::error::Error::InterruptWithoutThreadId(node) => Error::Configuration(format!(
                "Cannot interrupt at node '{}' without thread_id",
                node
            )),
            crate::error::Error::ResumeWithoutCheckpointer => {
                Error::Configuration("Cannot resume without checkpointer".to_string())
            }
            crate::error::Error::ResumeWithoutThreadId => {
                Error::Configuration("Cannot resume without thread_id".to_string())
            }
            // All other variants - convert to string
            _ => Error::Other(err.to_string()),
        }
    }
}
