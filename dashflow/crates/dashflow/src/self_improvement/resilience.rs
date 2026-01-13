// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

// Allow clippy warnings for resilience module
// - panic: panic!() in circuit breaker for invalid state transitions
#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::clone_on_ref_ptr,
    clippy::panic
)]

//! Resilience Module for Self-Improvement System.
//!
//! This module consolidates resilience-related functionality:
//! - **Circuit Breaker**: Prevent cascade failures with automatic service protection
//! - **Rate Limiter**: Control operation frequency with token bucket and backoff
//!
//! ## Quick Start
//!
//! ```rust,ignore
//! use dashflow::self_improvement::resilience::{
//!     // Circuit Breaker
//!     CircuitBreaker, CircuitBreakerConfig, CircuitState,
//!     // Rate Limiter
//!     RateLimiter, RateLimiterConfig,
//! };
//!
//! // Create a circuit breaker
//! let breaker = CircuitBreaker::new("anthropic-api")
//!     .with_failure_threshold(3)
//!     .with_reset_timeout_secs(30);
//!
//! // Create a rate limiter
//! let limiter = RateLimiter::new(RateLimiterConfig::default());
//! if limiter.try_acquire() {
//!     // Proceed with operation
//! }
//! ```

// =============================================================================
// Circuit Breaker Module (from circuit_breaker.rs)
// =============================================================================

pub mod circuit_breaker {
    //! Circuit Breaker Pattern for External Calls.

    use std::future::Future;
    use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
    use std::sync::RwLock;
    use std::time::{Duration, Instant};
    use thiserror::Error;

    use crate::self_improvement::error::SelfImprovementError;

    /// The current state of a circuit breaker.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum CircuitState {
        /// Normal operation state. Calls pass through to the underlying service.
        /// The circuit transitions to [`Open`](Self::Open) after reaching the failure threshold.
        Closed,
        /// Circuit tripped due to failures. All calls fail immediately without attempting
        /// the underlying operation. After the reset timeout, transitions to [`HalfOpen`](Self::HalfOpen).
        Open,
        /// Testing state after timeout expires. Limited calls are allowed through to test
        /// if the service has recovered. Success closes the circuit; failure reopens it.
        HalfOpen,
    }

    impl std::fmt::Display for CircuitState {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            match self {
                Self::Closed => write!(f, "closed"),
                Self::Open => write!(f, "open"),
                Self::HalfOpen => write!(f, "half-open"),
            }
        }
    }

    /// Configuration for a circuit breaker.
    #[derive(Debug, Clone)]
    #[non_exhaustive]
    pub struct CircuitBreakerConfig {
        /// Number of consecutive failures required to open the circuit (default: 5).
        pub failure_threshold: u32,
        /// Number of consecutive successes in half-open state required to close the circuit (default: 2).
        pub success_threshold: u32,
        /// Duration to wait before transitioning from open to half-open state (default: 30s).
        pub reset_timeout: Duration,
        /// Optional timeout for individual calls through the circuit (default: 30s).
        pub call_timeout: Option<Duration>,
    }

    impl Default for CircuitBreakerConfig {
        fn default() -> Self {
            Self {
                failure_threshold: 5,
                success_threshold: 2,
                reset_timeout: Duration::from_secs(30),
                call_timeout: Some(Duration::from_secs(30)),
            }
        }
    }

    impl CircuitBreakerConfig {
        /// Creates a new configuration with default values.
        #[must_use]
        pub fn new() -> Self {
            Self::default()
        }

        /// Sets the number of failures required to open the circuit.
        #[must_use]
        pub fn with_failure_threshold(mut self, threshold: u32) -> Self {
            self.failure_threshold = threshold;
            self
        }

        /// Sets the number of successes in half-open state required to close the circuit.
        #[must_use]
        pub fn with_success_threshold(mut self, threshold: u32) -> Self {
            self.success_threshold = threshold;
            self
        }

        /// Sets the reset timeout in seconds (time before open → half-open transition).
        #[must_use]
        pub fn with_reset_timeout_secs(mut self, secs: u64) -> Self {
            self.reset_timeout = Duration::from_secs(secs);
            self
        }

        /// Sets the reset timeout duration (time before open → half-open transition).
        #[must_use]
        pub fn with_reset_timeout(mut self, timeout: Duration) -> Self {
            self.reset_timeout = timeout;
            self
        }

        /// Sets a timeout for individual calls through the circuit.
        #[must_use]
        pub fn with_call_timeout(mut self, timeout: Duration) -> Self {
            self.call_timeout = Some(timeout);
            self
        }

        /// Disables the call timeout (calls can take unlimited time).
        #[must_use]
        pub fn without_call_timeout(mut self) -> Self {
            self.call_timeout = None;
            self
        }
    }

    /// Circuit breaker specific error type.
    #[derive(Debug, Clone, Error)]
    #[error(
        "circuit '{}' is open (failures: {}, open for: {:?})",
        circuit_name,
        failure_count,
        open_duration
    )]
    pub struct CircuitOpenError {
        /// Name of the circuit that is open.
        pub circuit_name: String,
        /// Duration the circuit has been in the open state.
        pub open_duration: Duration,
        /// Number of consecutive failures that caused the circuit to open.
        pub failure_count: u32,
        /// The last error message that triggered the circuit to open, if available.
        pub last_error: Option<String>,
    }

    impl From<CircuitOpenError> for SelfImprovementError {
        fn from(e: CircuitOpenError) -> Self {
            SelfImprovementError::Network(e.to_string())
        }
    }

    /// A circuit breaker for protecting external service calls from cascade failures.
    ///
    /// The circuit breaker pattern prevents an application from repeatedly calling
    /// a failing service. It monitors failures and "opens" the circuit after a threshold
    /// is reached, failing fast until the service recovers.
    ///
    /// # States
    ///
    /// - **Closed**: Normal operation, calls pass through
    /// - **Open**: Circuit tripped, calls fail immediately without attempting the operation
    /// - **Half-Open**: Testing if the service recovered, allowing limited calls through
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use dashflow::self_improvement::CircuitBreaker;
    ///
    /// // Create a circuit breaker for an API
    /// let breaker = CircuitBreaker::new("openai-api")
    ///     .with_failure_threshold(3)
    ///     .with_reset_timeout_secs(30);
    ///
    /// // Check if we can make a call
    /// if breaker.can_execute() {
    ///     match api_call().await {
    ///         Ok(result) => {
    ///             breaker.record_success();
    ///             // use result
    ///         }
    ///         Err(e) => {
    ///             breaker.record_failure(Some(&e.to_string()));
    ///             // handle error
    ///         }
    ///     }
    /// } else {
    ///     // Circuit is open, fail fast
    ///     return Err("Service unavailable".into());
    /// }
    ///
    /// // Or use the async wrapper for automatic recording
    /// let result = breaker.call_async(|| api_call()).await?;
    /// ```
    ///
    /// # Errors
    ///
    /// - [`CircuitOpenError`] - Returned when attempting to call through an open circuit
    /// - [`CircuitBreakerError::CircuitOpen`] - Wraps `CircuitOpenError` in the generic error type
    /// - [`CircuitBreakerError::Inner`] - Wraps errors from the underlying operation
    ///
    /// # See Also
    ///
    /// - [`CircuitBreakerConfig`] - Configuration options for thresholds and timeouts
    /// - [`CircuitState`] - The three states a circuit can be in
    /// - [`CircuitBreakerRegistry`] - Manage multiple circuit breakers
    /// - [`super::rate_limiter::RateLimiter`] - For rate limiting rather than failure protection
    #[derive(Debug)]
    pub struct CircuitBreaker {
        name: String,
        config: CircuitBreakerConfig,
        failure_count: AtomicU32,
        success_count: AtomicU32,
        opened_at: AtomicU64,
        last_error: RwLock<Option<String>>,
        last_transition: RwLock<Option<Instant>>,
    }

    impl CircuitBreaker {
        /// Creates a new circuit breaker with the given name and default configuration.
        #[must_use]
        pub fn new(name: impl Into<String>) -> Self {
            Self {
                name: name.into(),
                config: CircuitBreakerConfig::default(),
                failure_count: AtomicU32::new(0),
                success_count: AtomicU32::new(0),
                opened_at: AtomicU64::new(0),
                last_error: RwLock::new(None),
                last_transition: RwLock::new(None),
            }
        }

        /// Creates a new circuit breaker with the given name and custom configuration.
        #[must_use]
        pub fn with_config(name: impl Into<String>, config: CircuitBreakerConfig) -> Self {
            Self {
                name: name.into(),
                config,
                failure_count: AtomicU32::new(0),
                success_count: AtomicU32::new(0),
                opened_at: AtomicU64::new(0),
                last_error: RwLock::new(None),
                last_transition: RwLock::new(None),
            }
        }

        /// Sets the failure threshold for this circuit breaker.
        #[must_use]
        pub fn with_failure_threshold(mut self, threshold: u32) -> Self {
            self.config.failure_threshold = threshold;
            self
        }

        /// Sets the success threshold for this circuit breaker.
        #[must_use]
        pub fn with_success_threshold(mut self, threshold: u32) -> Self {
            self.config.success_threshold = threshold;
            self
        }

        /// Sets the reset timeout in seconds for this circuit breaker.
        #[must_use]
        pub fn with_reset_timeout_secs(mut self, secs: u64) -> Self {
            self.config.reset_timeout = Duration::from_secs(secs);
            self
        }

        /// Sets the reset timeout duration for this circuit breaker.
        #[must_use]
        pub fn with_reset_timeout(mut self, timeout: Duration) -> Self {
            self.config.reset_timeout = timeout;
            self
        }

        /// Returns the name of this circuit breaker.
        #[must_use]
        pub fn name(&self) -> &str {
            &self.name
        }

        /// Returns the current state of the circuit breaker.
        #[must_use]
        pub fn state(&self) -> CircuitState {
            let opened_at = self.opened_at.load(Ordering::SeqCst);

            if opened_at == 0 {
                return CircuitState::Closed;
            }

            let now = current_time_millis();
            let elapsed = Duration::from_millis(now.saturating_sub(opened_at));

            if elapsed >= self.config.reset_timeout {
                CircuitState::HalfOpen
            } else {
                CircuitState::Open
            }
        }

        /// Returns whether the circuit allows calls to pass through.
        /// Returns `true` for closed and half-open states, `false` for open.
        #[must_use]
        pub fn can_execute(&self) -> bool {
            match self.state() {
                CircuitState::Closed => true,
                CircuitState::Open => false,
                CircuitState::HalfOpen => true,
            }
        }

        /// Returns the current failure count for this circuit breaker.
        #[must_use]
        pub fn failure_count(&self) -> u32 {
            self.failure_count.load(Ordering::SeqCst)
        }

        /// Returns the current success count (relevant in half-open state).
        #[must_use]
        pub fn success_count(&self) -> u32 {
            self.success_count.load(Ordering::SeqCst)
        }

        /// Returns the last error message that was recorded, if any.
        #[must_use]
        pub fn last_error(&self) -> Option<String> {
            self.last_error.read().ok().and_then(|g| g.clone())
        }

        /// Records a successful operation. In closed state, resets failure count.
        /// In half-open state, increments success count and may close the circuit.
        pub fn record_success(&self) {
            let state = self.state();

            match state {
                CircuitState::Closed => {
                    self.failure_count.store(0, Ordering::SeqCst);
                }
                CircuitState::HalfOpen => {
                    let new_count = self.success_count.fetch_add(1, Ordering::SeqCst) + 1;

                    if new_count >= self.config.success_threshold {
                        self.close_circuit();
                    }
                }
                CircuitState::Open => {
                    tracing::warn!(
                        circuit = %self.name,
                        "Success recorded while circuit is open"
                    );
                }
            }
        }

        /// Records a failed operation. In closed state, increments failure count
        /// and may open the circuit. In half-open state, immediately reopens the circuit.
        pub fn record_failure(&self, error: Option<&str>) {
            if let Ok(mut guard) = self.last_error.write() {
                *guard = error.map(String::from);
            }

            let state = self.state();

            match state {
                CircuitState::Closed => {
                    let new_count = self.failure_count.fetch_add(1, Ordering::SeqCst) + 1;

                    if new_count >= self.config.failure_threshold {
                        self.open_circuit();
                    }
                }
                CircuitState::HalfOpen => {
                    self.open_circuit();
                }
                CircuitState::Open => {
                    self.failure_count.fetch_add(1, Ordering::SeqCst);
                }
            }
        }

        /// Executes an async operation through the circuit breaker.
        /// Automatically records success or failure based on the result.
        pub async fn execute<T, E, F, Fut>(&self, f: F) -> Result<T, CircuitBreakerError<E>>
        where
            F: FnOnce() -> Fut,
            Fut: Future<Output = Result<T, E>>,
            E: std::fmt::Display,
        {
            if !self.can_execute() {
                let opened_at = self.opened_at.load(Ordering::SeqCst);
                let open_duration =
                    Duration::from_millis(current_time_millis().saturating_sub(opened_at));

                return Err(CircuitBreakerError::CircuitOpen(CircuitOpenError {
                    circuit_name: self.name.clone(),
                    open_duration,
                    failure_count: self.failure_count(),
                    last_error: self.last_error(),
                }));
            }

            match f().await {
                Ok(result) => {
                    self.record_success();
                    Ok(result)
                }
                Err(e) => {
                    self.record_failure(Some(&e.to_string()));
                    Err(CircuitBreakerError::Inner(e))
                }
            }
        }

        /// Manually forces the circuit to the open state, rejecting all calls.
        pub fn force_open(&self) {
            self.open_circuit();
        }

        /// Manually forces the circuit to the closed state, allowing all calls.
        pub fn force_close(&self) {
            self.close_circuit();
        }

        /// Resets the circuit breaker to its initial closed state, clearing all counters.
        pub fn reset(&self) {
            self.failure_count.store(0, Ordering::SeqCst);
            self.success_count.store(0, Ordering::SeqCst);
            self.opened_at.store(0, Ordering::SeqCst);
            if let Ok(mut guard) = self.last_error.write() {
                *guard = None;
            }
            if let Ok(mut guard) = self.last_transition.write() {
                *guard = Some(Instant::now());
            }
        }

        /// Returns current statistics about this circuit breaker.
        #[must_use]
        pub fn stats(&self) -> CircuitBreakerStats {
            let opened_at = self.opened_at.load(Ordering::SeqCst);
            let open_duration = if opened_at > 0 {
                Some(Duration::from_millis(
                    current_time_millis().saturating_sub(opened_at),
                ))
            } else {
                None
            };

            CircuitBreakerStats {
                name: self.name.clone(),
                state: self.state(),
                failure_count: self.failure_count(),
                success_count: self.success_count(),
                failure_threshold: self.config.failure_threshold,
                success_threshold: self.config.success_threshold,
                reset_timeout: self.config.reset_timeout,
                open_duration,
                last_error: self.last_error(),
            }
        }

        fn open_circuit(&self) {
            let now = current_time_millis();
            self.opened_at.store(now, Ordering::SeqCst);
            self.success_count.store(0, Ordering::SeqCst);

            if let Ok(mut guard) = self.last_transition.write() {
                *guard = Some(Instant::now());
            }

            tracing::warn!(
                circuit = %self.name,
                failures = self.failure_count(),
                "Circuit breaker opened"
            );
        }

        fn close_circuit(&self) {
            self.opened_at.store(0, Ordering::SeqCst);
            self.failure_count.store(0, Ordering::SeqCst);
            self.success_count.store(0, Ordering::SeqCst);

            if let Ok(mut guard) = self.last_transition.write() {
                *guard = Some(Instant::now());
            }

            tracing::info!(
                circuit = %self.name,
                "Circuit breaker closed"
            );
        }
    }

    /// Error type for circuit breaker operations.
    #[derive(Debug)]
    #[non_exhaustive]
    pub enum CircuitBreakerError<E> {
        /// The circuit is open and not accepting calls.
        CircuitOpen(CircuitOpenError),
        /// The underlying operation failed with this error.
        Inner(E),
    }

    impl<E: std::fmt::Display> std::fmt::Display for CircuitBreakerError<E> {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            match self {
                Self::CircuitOpen(e) => write!(f, "{e}"),
                Self::Inner(e) => write!(f, "{e}"),
            }
        }
    }

    impl<E: std::error::Error + 'static> std::error::Error for CircuitBreakerError<E> {
        fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
            match self {
                Self::CircuitOpen(e) => Some(e),
                Self::Inner(e) => Some(e),
            }
        }
    }

    impl<E> CircuitBreakerError<E> {
        /// Returns `true` if this error is due to an open circuit.
        #[must_use]
        pub fn is_circuit_open(&self) -> bool {
            matches!(self, Self::CircuitOpen(_))
        }

        /// Returns the circuit open error if this is a `CircuitOpen` variant.
        #[must_use]
        pub fn as_circuit_open(&self) -> Option<&CircuitOpenError> {
            match self {
                Self::CircuitOpen(e) => Some(e),
                Self::Inner(_) => None,
            }
        }

        /// Returns the inner error if this is an `Inner` variant.
        #[must_use]
        pub fn as_inner(&self) -> Option<&E> {
            match self {
                Self::CircuitOpen(_) => None,
                Self::Inner(e) => Some(e),
            }
        }

        /// Consumes the error and returns the inner error.
        ///
        /// # Panics
        ///
        /// Panics if this is a `CircuitOpen` error. Use [`try_into_inner`](Self::try_into_inner)
        /// for a non-panicking alternative.
        pub fn into_inner(self) -> E {
            self.try_into_inner().expect(
                "CircuitBreakerError::into_inner called on CircuitOpen variant (use try_into_inner() for Result)",
            )
        }

        /// Consumes the error and returns the inner error if available.
        ///
        /// Returns `Ok(E)` if this is an `Inner` error, or `Err(CircuitOpenError)`
        /// if this is a `CircuitOpen` error.
        ///
        /// # Example
        ///
        /// ```rust,ignore
        /// match error.try_into_inner() {
        ///     Ok(inner) => handle_inner_error(inner),
        ///     Err(circuit_err) => handle_circuit_open(circuit_err),
        /// }
        /// ```
        pub fn try_into_inner(self) -> Result<E, CircuitOpenError> {
            match self {
                Self::CircuitOpen(e) => Err(e),
                Self::Inner(e) => Ok(e),
            }
        }
    }

    /// Statistics about a circuit breaker's current state.
    #[derive(Debug, Clone)]
    pub struct CircuitBreakerStats {
        /// Name of the circuit breaker.
        pub name: String,
        /// Current state of the circuit (Closed, Open, or HalfOpen).
        pub state: CircuitState,
        /// Current consecutive failure count.
        pub failure_count: u32,
        /// Current consecutive success count (relevant in half-open state).
        pub success_count: u32,
        /// Configured failure threshold to open the circuit.
        pub failure_threshold: u32,
        /// Configured success threshold to close the circuit.
        pub success_threshold: u32,
        /// Configured timeout before transition from open to half-open.
        pub reset_timeout: Duration,
        /// Duration the circuit has been open, if currently open.
        pub open_duration: Option<Duration>,
        /// Last error message that triggered a failure, if any.
        pub last_error: Option<String>,
    }

    impl std::fmt::Display for CircuitBreakerStats {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(
                f,
                "CircuitBreaker[{}]: state={}, failures={}/{}, successes={}/{}",
                self.name,
                self.state,
                self.failure_count,
                self.failure_threshold,
                self.success_count,
                self.success_threshold
            )?;
            if let Some(duration) = self.open_duration {
                write!(f, ", open_for={:?}", duration)?;
            }
            Ok(())
        }
    }

    /// A registry of circuit breakers for managing multiple circuits.
    #[derive(Debug, Default)]
    pub struct CircuitBreakerRegistry {
        breakers: RwLock<std::collections::HashMap<String, std::sync::Arc<CircuitBreaker>>>,
    }

    impl CircuitBreakerRegistry {
        /// Creates a new empty circuit breaker registry.
        #[must_use]
        pub fn new() -> Self {
            Self {
                breakers: RwLock::new(std::collections::HashMap::new()),
            }
        }

        /// Returns an existing circuit breaker by name, or creates a new one with defaults.
        pub fn get_or_create(&self, name: &str) -> std::sync::Arc<CircuitBreaker> {
            if let Ok(guard) = self.breakers.read() {
                if let Some(breaker) = guard.get(name) {
                    return breaker.clone();
                }
            }

            if let Ok(mut guard) = self.breakers.write() {
                if let Some(breaker) = guard.get(name) {
                    return breaker.clone();
                }

                let breaker = std::sync::Arc::new(CircuitBreaker::new(name));
                guard.insert(name.to_string(), breaker.clone());
                breaker
            } else {
                // M-949: Log warning when write lock fails - breaker will not be in registry
                tracing::warn!(
                    name = %name,
                    "CircuitBreakerRegistry write lock poisoned; creating unregistered breaker"
                );
                std::sync::Arc::new(CircuitBreaker::new(name))
            }
        }

        /// Returns an existing circuit breaker by name, or creates a new one with custom config.
        pub fn get_or_create_with_config(
            &self,
            name: &str,
            config: CircuitBreakerConfig,
        ) -> std::sync::Arc<CircuitBreaker> {
            if let Ok(guard) = self.breakers.read() {
                if let Some(breaker) = guard.get(name) {
                    return breaker.clone();
                }
            }

            if let Ok(mut guard) = self.breakers.write() {
                if let Some(breaker) = guard.get(name) {
                    return breaker.clone();
                }

                let breaker = std::sync::Arc::new(CircuitBreaker::with_config(name, config));
                guard.insert(name.to_string(), breaker.clone());
                breaker
            } else {
                // M-949: Log warning when write lock fails - breaker will not be in registry
                tracing::warn!(
                    name = %name,
                    "CircuitBreakerRegistry write lock poisoned; creating unregistered breaker"
                );
                std::sync::Arc::new(CircuitBreaker::with_config(name, config))
            }
        }

        /// Returns an existing circuit breaker by name, if it exists.
        #[must_use]
        pub fn get(&self, name: &str) -> Option<std::sync::Arc<CircuitBreaker>> {
            self.breakers
                .read()
                .ok()
                .and_then(|guard| guard.get(name).cloned())
        }

        /// Returns statistics for all registered circuit breakers.
        #[must_use]
        pub fn all_stats(&self) -> Vec<CircuitBreakerStats> {
            self.breakers
                .read()
                .map(|guard| guard.values().map(|b| b.stats()).collect())
                .unwrap_or_default()
        }

        /// Returns the names of all currently open circuit breakers.
        #[must_use]
        pub fn open_circuits(&self) -> Vec<String> {
            self.breakers
                .read()
                .map(|guard| {
                    guard
                        .iter()
                        .filter(|(_, b)| matches!(b.state(), CircuitState::Open))
                        .map(|(name, _)| name.clone())
                        .collect()
                })
                .unwrap_or_default()
        }

        /// Resets all registered circuit breakers to their initial closed state.
        pub fn reset_all(&self) {
            if let Ok(guard) = self.breakers.read() {
                for breaker in guard.values() {
                    breaker.reset();
                }
            }
        }
    }

    fn current_time_millis() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or_else(|e| {
                // M-950: Log warning on SystemTime failure - returning 0 will make circuits appear closed
                tracing::warn!(
                    error = %e,
                    "SystemTime::now() before UNIX_EPOCH; circuit breaker time will be unreliable"
                );
                0
            })
    }

    /// Default configuration for API calls.
    pub const API_CIRCUIT_CONFIG: CircuitBreakerConfig = CircuitBreakerConfig {
        failure_threshold: 3,
        success_threshold: 2,
        reset_timeout: Duration::from_secs(30),
        call_timeout: Some(Duration::from_secs(60)),
    };

    /// Default configuration for Prometheus queries.
    pub const PROMETHEUS_CIRCUIT_CONFIG: CircuitBreakerConfig = CircuitBreakerConfig {
        failure_threshold: 5,
        success_threshold: 3,
        reset_timeout: Duration::from_secs(15),
        call_timeout: Some(Duration::from_secs(10)),
    };

    /// Default configuration for webhook calls.
    pub const WEBHOOK_CIRCUIT_CONFIG: CircuitBreakerConfig = CircuitBreakerConfig {
        failure_threshold: 3,
        success_threshold: 1,
        reset_timeout: Duration::from_secs(60),
        call_timeout: Some(Duration::from_secs(30)),
    };

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn test_circuit_breaker_initial_state() {
            let breaker = CircuitBreaker::new("test");
            assert_eq!(breaker.state(), CircuitState::Closed);
            assert!(breaker.can_execute());
            assert_eq!(breaker.failure_count(), 0);
            assert_eq!(breaker.success_count(), 0);
        }

        #[test]
        fn test_circuit_opens_after_failures() {
            let breaker = CircuitBreaker::new("test").with_failure_threshold(3);

            breaker.record_failure(Some("error 1"));
            assert_eq!(breaker.state(), CircuitState::Closed);
            breaker.record_failure(Some("error 2"));
            assert_eq!(breaker.state(), CircuitState::Closed);

            breaker.record_failure(Some("error 3"));
            assert_eq!(breaker.state(), CircuitState::Open);
            assert!(!breaker.can_execute());
        }

        #[test]
        fn test_success_resets_failure_count() {
            let breaker = CircuitBreaker::new("test").with_failure_threshold(3);

            breaker.record_failure(Some("error"));
            breaker.record_failure(Some("error"));
            assert_eq!(breaker.failure_count(), 2);

            breaker.record_success();
            assert_eq!(breaker.failure_count(), 0);
        }

        #[test]
        fn test_force_open_and_close() {
            let breaker = CircuitBreaker::new("test");

            breaker.force_open();
            assert_eq!(breaker.state(), CircuitState::Open);

            breaker.force_close();
            assert_eq!(breaker.state(), CircuitState::Closed);
        }

        #[test]
        fn test_reset() {
            let breaker = CircuitBreaker::new("test").with_failure_threshold(1);

            breaker.record_failure(Some("error"));
            assert_eq!(breaker.state(), CircuitState::Open);

            breaker.reset();
            assert_eq!(breaker.state(), CircuitState::Closed);
            assert_eq!(breaker.failure_count(), 0);
        }

        #[test]
        fn test_registry_get_or_create() {
            let registry = CircuitBreakerRegistry::new();

            let breaker1 = registry.get_or_create("api");
            let breaker2 = registry.get_or_create("api");

            assert!(std::sync::Arc::ptr_eq(&breaker1, &breaker2));
        }

        #[test]
        fn test_circuit_state_display() {
            assert_eq!(CircuitState::Closed.to_string(), "closed");
            assert_eq!(CircuitState::Open.to_string(), "open");
            assert_eq!(CircuitState::HalfOpen.to_string(), "half-open");
        }
    }
}

// =============================================================================
// Rate Limiter Module (from rate_limiter.rs)
// =============================================================================

pub mod rate_limiter {
    //! Rate limiting for the self-improvement daemon.

    use rand::Rng;
    use std::collections::VecDeque;
    use std::sync::Mutex;
    use std::time::{Duration, Instant};

    /// Configuration for rate limiting.
    #[derive(Debug, Clone)]
    #[non_exhaustive]
    pub struct RateLimiterConfig {
        /// Maximum number of operations allowed within the window (default: 60).
        pub max_rate: u32,
        /// Time window for rate limiting (default: 60 seconds).
        pub window: Duration,
        /// Additional burst capacity above `max_rate` (default: 10).
        pub burst: u32,
        /// Initial backoff duration after errors (default: 1 second).
        pub initial_backoff: Duration,
        /// Maximum backoff duration (default: 300 seconds).
        pub max_backoff: Duration,
        /// Multiplier for exponential backoff (default: 2.0).
        pub backoff_multiplier: f64,
        /// Number of consecutive errors before triggering backoff (default: 3).
        pub error_threshold: u32,
        /// Jitter factor for `acquire_blocking()` sleep times (0.0 to 1.0).
        /// A value of 0.25 means ±25% random variation on wait times.
        /// This prevents synchronized wakeups ("thundering herd") when multiple
        /// callers are rate-limited simultaneously.
        /// Default: 0.25 (25% jitter)
        pub jitter_factor: f64,
    }

    impl Default for RateLimiterConfig {
        fn default() -> Self {
            Self {
                max_rate: 60,
                window: Duration::from_secs(60),
                burst: 10,
                initial_backoff: Duration::from_secs(1),
                max_backoff: Duration::from_secs(300),
                backoff_multiplier: 2.0,
                error_threshold: 3,
                jitter_factor: 0.25, // 25% jitter to prevent thundering herd
            }
        }
    }

    impl RateLimiterConfig {
        /// Creates a new configuration with default values.
        #[must_use]
        pub fn new() -> Self {
            Self::default()
        }

        /// Sets the maximum rate (operations per window).
        #[must_use]
        pub fn with_max_rate(mut self, rate: u32) -> Self {
            self.max_rate = rate;
            self
        }

        /// Sets the rate limiting window duration.
        #[must_use]
        pub fn with_window(mut self, window: Duration) -> Self {
            self.window = window;
            self
        }

        /// Sets the burst capacity (extra operations above max_rate).
        #[must_use]
        pub fn with_burst(mut self, burst: u32) -> Self {
            self.burst = burst;
            self
        }

        /// Sets the initial backoff duration for error handling.
        #[must_use]
        pub fn with_initial_backoff(mut self, backoff: Duration) -> Self {
            self.initial_backoff = backoff;
            self
        }

        /// Sets the maximum backoff duration.
        #[must_use]
        pub fn with_max_backoff(mut self, backoff: Duration) -> Self {
            self.max_backoff = backoff;
            self
        }

        /// Sets the exponential backoff multiplier.
        #[must_use]
        pub fn with_backoff_multiplier(mut self, multiplier: f64) -> Self {
            self.backoff_multiplier = multiplier;
            self
        }

        /// Sets the number of consecutive errors before triggering backoff.
        #[must_use]
        pub fn with_error_threshold(mut self, threshold: u32) -> Self {
            self.error_threshold = threshold;
            self
        }

        /// Set the jitter factor (0.0 to 1.0) for `acquire_blocking()` sleep times.
        /// A value of 0.25 means ±25% random variation.
        /// Set to 0.0 to disable jitter.
        #[must_use]
        pub fn with_jitter_factor(mut self, factor: f64) -> Self {
            self.jitter_factor = factor.clamp(0.0, 1.0);
            self
        }

        /// Creates a strict configuration (10/min, burst 2, aggressive backoff).
        #[must_use]
        pub fn strict() -> Self {
            Self {
                max_rate: 10,
                window: Duration::from_secs(60),
                burst: 2,
                initial_backoff: Duration::from_secs(5),
                max_backoff: Duration::from_secs(600),
                backoff_multiplier: 2.0,
                error_threshold: 2,
                jitter_factor: 0.25,
            }
        }

        /// Creates a permissive configuration (120/min, burst 30, gentle backoff).
        #[must_use]
        pub fn permissive() -> Self {
            Self {
                max_rate: 120,
                window: Duration::from_secs(60),
                burst: 30,
                initial_backoff: Duration::from_millis(100),
                max_backoff: Duration::from_secs(60),
                backoff_multiplier: 1.5,
                error_threshold: 5,
                jitter_factor: 0.25,
            }
        }
    }

    #[derive(Debug, Default)]
    struct RateLimiterState {
        timestamps: VecDeque<Instant>,
        backoff_level: u32,
        backoff_until: Option<Instant>,
        consecutive_errors: u32,
        total_operations: u64,
        total_limited: u64,
    }

    /// A rate limiter with sliding window and exponential backoff.
    ///
    /// Rate limiting controls how frequently operations can be performed, preventing
    /// overwhelming of resources or API rate limits. This implementation uses a
    /// sliding window algorithm with optional burst capacity and automatic backoff
    /// when errors occur.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use dashflow::self_improvement::{RateLimiter, RateLimiterConfig};
    ///
    /// // Create a rate limiter: 60 requests/minute with burst of 10
    /// let limiter = RateLimiter::new(
    ///     RateLimiterConfig::new()
    ///         .with_max_rate(60)
    ///         .with_burst(10)
    /// );
    ///
    /// // Non-blocking: check if we can proceed
    /// if limiter.try_acquire() {
    ///     // Make API call
    ///     match api_call().await {
    ///         Ok(_) => limiter.record_success(),
    ///         Err(_) => limiter.record_error(), // triggers backoff
    ///     }
    /// } else {
    ///     // Rate limited, wait or return error
    ///     let wait_time = limiter.time_until_available();
    ///     println!("Rate limited, wait {:?}", wait_time);
    /// }
    ///
    /// // Blocking: wait until a token is available
    /// limiter.acquire_blocking();
    /// // Now safe to proceed
    /// ```
    ///
    /// # Configuration Presets
    ///
    /// - [`RateLimiterConfig::default()`] - Balanced: 60/min, burst 10
    /// - [`RateLimiterConfig::strict()`] - Conservative: 10/min, burst 2
    /// - [`RateLimiterConfig::permissive()`] - Generous: 120/min, burst 30
    ///
    /// # Backoff Behavior
    ///
    /// When [`RateLimiter::record_error`] is called repeatedly, the limiter enters exponential
    /// backoff mode, temporarily blocking all operations. This helps prevent
    /// hammering a failing service. Call [`RateLimiter::reset`] after recovery.
    ///
    /// # See Also
    ///
    /// - [`RateLimiterConfig`] - Configuration for rate, window, and backoff
    /// - [`RateLimiterStats`] - Statistics about limiter operation
    /// - [`super::circuit_breaker::CircuitBreaker`] - For failure-based protection instead of rate limiting
    #[derive(Debug)]
    pub struct RateLimiter {
        config: RateLimiterConfig,
        state: Mutex<RateLimiterState>,
    }

    impl RateLimiter {
        /// Creates a new rate limiter with the given configuration.
        #[must_use]
        pub fn new(config: RateLimiterConfig) -> Self {
            Self {
                config,
                state: Mutex::new(RateLimiterState::default()),
            }
        }

        /// Creates a new rate limiter with default configuration.
        #[must_use]
        pub fn default_config() -> Self {
            Self::new(RateLimiterConfig::default())
        }

        /// Attempts to acquire a rate limit token. Returns `true` if successful.
        /// Non-blocking: returns immediately even if rate limited.
        pub fn try_acquire(&self) -> bool {
            let mut state = self.state.lock().unwrap();
            let now = Instant::now();

            if let Some(backoff_until) = state.backoff_until {
                if now < backoff_until {
                    state.total_limited += 1;
                    return false;
                }
                state.backoff_until = None;
            }

            let window_start = now - self.config.window;
            while let Some(&ts) = state.timestamps.front() {
                if ts < window_start {
                    state.timestamps.pop_front();
                } else {
                    break;
                }
            }

            // M-948: Use saturating_add to prevent overflow
            let effective_limit = self.config.max_rate.saturating_add(self.config.burst);
            if state.timestamps.len() >= effective_limit as usize {
                state.total_limited += 1;
                return false;
            }

            state.timestamps.push_back(now);
            state.total_operations += 1;
            true
        }

        /// Blocking acquire that waits until a token is available.
        ///
        /// This method applies jitter to wait times (configurable via `jitter_factor`)
        /// to prevent synchronized wakeups when multiple callers are rate-limited.
        /// With default 25% jitter, a 100ms wait becomes 75-125ms randomly.
        pub fn acquire_blocking(&self) {
            let mut rng = rand::thread_rng();
            loop {
                if self.try_acquire() {
                    return;
                }
                let base_wait = self.time_until_available();
                let wait = self.apply_jitter(base_wait, &mut rng);
                std::thread::sleep(wait);
            }
        }

        /// Apply jitter to a duration based on the configured jitter_factor.
        /// With jitter_factor=0.25, a 100ms duration becomes 75-125ms randomly.
        fn apply_jitter(&self, duration: Duration, rng: &mut impl Rng) -> Duration {
            if self.config.jitter_factor <= 0.0 || duration.is_zero() {
                return duration;
            }
            // Generate random factor in range [1-jitter, 1+jitter]
            let jitter_range = self.config.jitter_factor;
            let factor = 1.0 + rng.gen_range(-jitter_range..=jitter_range);
            // Apply factor, ensuring non-negative result
            let jittered_nanos = (duration.as_nanos() as f64 * factor).max(0.0);
            Duration::from_nanos(jittered_nanos as u64)
        }

        /// Returns the time until a token becomes available (zero if available now).
        #[must_use]
        pub fn time_until_available(&self) -> Duration {
            let state = self.state.lock().unwrap();
            let now = Instant::now();

            if let Some(backoff_until) = state.backoff_until {
                if now < backoff_until {
                    return backoff_until - now;
                }
            }

            // M-948: Use saturating_add to prevent overflow
            let effective_limit = self.config.max_rate.saturating_add(self.config.burst);
            if state.timestamps.len() < effective_limit as usize {
                return Duration::ZERO;
            }

            if let Some(&oldest) = state.timestamps.front() {
                let expiry = oldest + self.config.window;
                if expiry > now {
                    return expiry - now;
                }
            }

            Duration::ZERO
        }

        /// Records a successful operation. Resets consecutive error count and
        /// may reduce backoff level.
        pub fn record_success(&self) {
            let mut state = self.state.lock().unwrap();
            state.consecutive_errors = 0;
            if state.backoff_level > 0 {
                state.backoff_level = state.backoff_level.saturating_sub(1);
            }
        }

        /// Records a failed operation. Increments consecutive error count and
        /// may trigger exponential backoff.
        pub fn record_error(&self) {
            let mut state = self.state.lock().unwrap();
            state.consecutive_errors += 1;

            if state.consecutive_errors >= self.config.error_threshold {
                state.backoff_level = state.backoff_level.saturating_add(1);

                let backoff = self.calculate_backoff(state.backoff_level);
                state.backoff_until = Some(Instant::now() + backoff);
            }
        }

        fn calculate_backoff(&self, level: u32) -> Duration {
            if level == 0 {
                return Duration::ZERO;
            }
            let capped_level = level.saturating_sub(1).min(20) as i32;
            let multiplier = self.config.backoff_multiplier.powi(capped_level);
            let backoff_secs = self.config.initial_backoff.as_secs_f64() * multiplier;
            let capped_secs = backoff_secs.min(self.config.max_backoff.as_secs_f64());
            if !capped_secs.is_finite() || capped_secs < 0.0 {
                return self.config.max_backoff;
            }
            Duration::from_secs_f64(capped_secs)
        }

        /// Resets the rate limiter to its initial state.
        pub fn reset(&self) {
            let mut state = self.state.lock().unwrap();
            *state = RateLimiterState::default();
        }

        /// Returns current statistics about the rate limiter.
        #[must_use]
        pub fn stats(&self) -> RateLimiterStats {
            let state = self.state.lock().unwrap();
            let now = Instant::now();

            let window_start = now - self.config.window;
            let in_window = state
                .timestamps
                .iter()
                .filter(|&&ts| ts >= window_start)
                .count();

            RateLimiterStats {
                current_rate: in_window as u32,
                max_rate: self.config.max_rate,
                total_operations: state.total_operations,
                total_limited: state.total_limited,
                consecutive_errors: state.consecutive_errors,
                backoff_level: state.backoff_level,
                in_backoff: state.backoff_until.is_some_and(|until| now < until),
            }
        }

        /// Returns whether the limiter is currently in backoff mode due to errors.
        #[must_use]
        pub fn is_in_backoff(&self) -> bool {
            let state = self.state.lock().unwrap();
            state
                .backoff_until
                .is_some_and(|until| Instant::now() < until)
        }

        /// Returns the current rate of operations within the window.
        #[must_use]
        pub fn current_rate(&self) -> u32 {
            let state = self.state.lock().unwrap();
            let now = Instant::now();
            let window_start = now - self.config.window;
            state
                .timestamps
                .iter()
                .filter(|&&ts| ts >= window_start)
                .count() as u32
        }
    }

    impl Default for RateLimiter {
        fn default() -> Self {
            Self::default_config()
        }
    }

    /// Statistics from the rate limiter.
    #[derive(Debug, Clone, Default)]
    pub struct RateLimiterStats {
        /// Current number of operations within the rate limiting window.
        pub current_rate: u32,
        /// Maximum rate configured for this limiter.
        pub max_rate: u32,
        /// Total number of successful operations since creation.
        pub total_operations: u64,
        /// Total number of operations that were rate limited.
        pub total_limited: u64,
        /// Current consecutive error count.
        pub consecutive_errors: u32,
        /// Current backoff level (higher = longer backoff).
        pub backoff_level: u32,
        /// Whether the limiter is currently in backoff mode.
        pub in_backoff: bool,
    }

    impl RateLimiterStats {
        /// Returns the percentage of operations that were rate limited.
        #[must_use]
        pub fn limited_percentage(&self) -> f64 {
            if self.total_operations == 0 {
                0.0
            } else {
                (self.total_limited as f64 / (self.total_operations + self.total_limited) as f64)
                    * 100.0
            }
        }

        /// Returns the current utilization as a percentage of max_rate.
        #[must_use]
        pub fn utilization(&self) -> f64 {
            if self.max_rate == 0 {
                0.0
            } else {
                (self.current_rate as f64 / self.max_rate as f64) * 100.0
            }
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use rand::SeedableRng;

        #[test]
        fn test_basic_rate_limiting() {
            let config = RateLimiterConfig::new()
                .with_max_rate(5)
                .with_burst(0)
                .with_window(Duration::from_secs(1));
            let limiter = RateLimiter::new(config);

            for _ in 0..5 {
                assert!(limiter.try_acquire());
            }

            assert!(!limiter.try_acquire());
        }

        #[test]
        fn test_burst_allowance() {
            let config = RateLimiterConfig::new()
                .with_max_rate(5)
                .with_burst(3)
                .with_window(Duration::from_secs(1));
            let limiter = RateLimiter::new(config);

            for _ in 0..8 {
                assert!(limiter.try_acquire());
            }

            assert!(!limiter.try_acquire());
        }

        #[test]
        fn test_error_tracking() {
            let config = RateLimiterConfig::new().with_error_threshold(3);
            let limiter = RateLimiter::new(config);

            limiter.record_error();
            limiter.record_error();
            assert!(!limiter.is_in_backoff());

            limiter.record_error();
            assert!(limiter.is_in_backoff());
        }

        #[test]
        fn test_success_resets_errors() {
            let config = RateLimiterConfig::new().with_error_threshold(3);
            let limiter = RateLimiter::new(config);

            limiter.record_error();
            limiter.record_error();

            limiter.record_success();

            let stats = limiter.stats();
            assert_eq!(stats.consecutive_errors, 0);
        }

        #[test]
        fn test_stats() {
            let config = RateLimiterConfig::new()
                .with_max_rate(10)
                .with_burst(0)
                .with_window(Duration::from_secs(1));
            let limiter = RateLimiter::new(config);

            for _ in 0..5 {
                limiter.try_acquire();
            }

            let stats = limiter.stats();
            assert_eq!(stats.total_operations, 5);
            assert_eq!(stats.current_rate, 5);
            assert_eq!(stats.max_rate, 10);
            assert!((stats.utilization() - 50.0).abs() < 0.1);
        }

        #[test]
        fn test_reset() {
            let limiter = RateLimiter::default();

            limiter.try_acquire();
            limiter.record_error();

            limiter.reset();

            let stats = limiter.stats();
            assert_eq!(stats.total_operations, 0);
            assert_eq!(stats.consecutive_errors, 0);
        }

        #[test]
        fn test_config_builders() {
            let strict = RateLimiterConfig::strict();
            assert_eq!(strict.max_rate, 10);
            assert!((strict.jitter_factor - 0.25).abs() < f64::EPSILON);

            let permissive = RateLimiterConfig::permissive();
            assert_eq!(permissive.max_rate, 120);
            assert!((permissive.jitter_factor - 0.25).abs() < f64::EPSILON);
        }

        #[test]
        fn test_jitter_factor_config() {
            let config = RateLimiterConfig::new().with_jitter_factor(0.5);
            assert!((config.jitter_factor - 0.5).abs() < f64::EPSILON);

            // Clamps to [0.0, 1.0]
            let config_over = RateLimiterConfig::new().with_jitter_factor(1.5);
            assert!((config_over.jitter_factor - 1.0).abs() < f64::EPSILON);

            let config_under = RateLimiterConfig::new().with_jitter_factor(-0.5);
            assert!(config_under.jitter_factor.abs() < f64::EPSILON);
        }

        #[test]
        fn test_apply_jitter_zero_duration() {
            let limiter = RateLimiter::default();
            // M-573: Use seeded RNG for deterministic test behavior
            let mut rng = rand::rngs::StdRng::seed_from_u64(42);

            // Zero duration should remain zero
            let result = limiter.apply_jitter(Duration::ZERO, &mut rng);
            assert_eq!(result, Duration::ZERO);
        }

        #[test]
        fn test_apply_jitter_zero_factor() {
            let config = RateLimiterConfig::new().with_jitter_factor(0.0);
            let limiter = RateLimiter::new(config);
            // M-573: Use seeded RNG for deterministic test behavior
            let mut rng = rand::rngs::StdRng::seed_from_u64(42);

            // With zero jitter factor, duration should be unchanged
            let base = Duration::from_millis(100);
            let result = limiter.apply_jitter(base, &mut rng);
            assert_eq!(result, base);
        }

        #[test]
        fn test_apply_jitter_produces_variation() {
            let config = RateLimiterConfig::new().with_jitter_factor(0.25);
            let limiter = RateLimiter::new(config);
            // M-573: Use seeded RNG for deterministic test behavior
            // Even with seeded RNG, the jitter produces unique values due to the algorithm
            let mut rng = rand::rngs::StdRng::seed_from_u64(42);

            let base = Duration::from_millis(1000);
            let mut results = std::collections::HashSet::new();

            // Run multiple times to verify jitter produces variation
            for _ in 0..100 {
                let jittered = limiter.apply_jitter(base, &mut rng);
                results.insert(jittered.as_nanos());

                // Verify jittered value is within expected bounds (±25%)
                let min_expected = Duration::from_millis(750); // 1000 * 0.75
                let max_expected = Duration::from_millis(1250); // 1000 * 1.25
                assert!(
                    jittered >= min_expected && jittered <= max_expected,
                    "Jittered value {:?} outside expected bounds [{:?}, {:?}]",
                    jittered,
                    min_expected,
                    max_expected
                );
            }

            // Should have produced some variation (not all the same)
            assert!(
                results.len() > 1,
                "Jitter should produce variation, but got {} unique values",
                results.len()
            );
        }

        #[test]
        fn test_default_jitter_factor() {
            let config = RateLimiterConfig::default();
            assert!(
                (config.jitter_factor - 0.25).abs() < f64::EPSILON,
                "Default jitter_factor should be 0.25"
            );
        }
    }
}

// =============================================================================
// Re-exports for convenience
// =============================================================================

pub use circuit_breaker::*;
pub use rate_limiter::*;
