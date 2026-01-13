//! Configuration for WASM executor
//!
//! Provides secure defaults aligned with HIPAA/SOC2 compliance requirements.

use dashflow::core::config_loader::env_vars::{env_string, JWT_SECRET as JWT_SECRET_VAR};
use serde::{Deserialize, Serialize};
use std::time::Duration;

/// SECURITY (M-229): Marker value used when JWT_SECRET env var is not set.
/// validate() will detect this and return an error, preventing operation
/// with a predictable secret.
const INSECURE_DEFAULT_SECRET_MARKER: &str = "__JWT_SECRET_NOT_SET_CHECK_ENV_VAR__";

/// Configuration for WASM executor
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WasmExecutorConfig {
    /// Maximum fuel (computational operations) per execution
    ///
    /// Default: 100,000,000 operations (~10 seconds of CPU time)
    /// HIPAA: Prevents denial of service attacks
    pub max_fuel: u64,

    /// Maximum memory per execution (bytes)
    ///
    /// Default: 256 MB
    /// HIPAA: Prevents resource exhaustion
    pub max_memory_bytes: usize,

    /// Maximum stack size (bytes)
    ///
    /// Default: 2 MB
    pub max_stack_bytes: usize,

    /// Maximum execution timeout
    ///
    /// Default: 30 seconds
    /// HIPAA: Prevents long-running malicious code
    pub max_execution_timeout: Duration,

    /// Maximum WASM module size (bytes)
    ///
    /// Default: 10 MB
    /// HIPAA: Prevents memory exhaustion
    pub max_wasm_size_bytes: usize,

    /// Enable audit logging
    ///
    /// Default: true
    /// HIPAA: §164.312(b) - Audit Controls (REQUIRED)
    pub enable_audit_logging: bool,

    /// Audit log path
    ///
    /// Default: "/var/log/wasm-executor/audit.log"
    /// HIPAA: Tamper-evident log storage
    pub audit_log_path: String,

    /// Enable Prometheus metrics
    ///
    /// Default: true
    /// SOC2: CC7.2 - Detection and Monitoring
    pub enable_metrics: bool,

    /// JWT secret for authentication
    ///
    /// REQUIRED: Load from environment variable
    /// HIPAA: §164.312(d) - Authentication
    pub jwt_secret: String,

    /// JWT token expiry (minutes)
    ///
    /// Default: 30 minutes
    /// HIPAA: Auto-logoff after inactivity
    pub jwt_expiry_minutes: i64,

    /// Enable WASM module signature verification
    ///
    /// Default: false (optional security enhancement)
    /// SOC2: CC6.6 - Vulnerability Management
    pub enable_signature_verification: bool,
}

impl Default for WasmExecutorConfig {
    fn default() -> Self {
        Self {
            max_fuel: 100_000_000,
            max_memory_bytes: 256 * 1024 * 1024,
            max_stack_bytes: 2 * 1024 * 1024,
            max_execution_timeout: Duration::from_secs(30),
            max_wasm_size_bytes: 10 * 1024 * 1024,
            enable_audit_logging: true,
            audit_log_path: "/var/log/wasm-executor/audit.log".to_string(),
            enable_metrics: true,
            // SECURITY (M-229): No default fallback for JWT secret.
            // This will be caught by validate() if not properly set.
            jwt_secret: env_string(JWT_SECRET_VAR)
                .unwrap_or_else(|| INSECURE_DEFAULT_SECRET_MARKER.to_string()),
            jwt_expiry_minutes: 30,
            enable_signature_verification: false,
        }
    }
}

impl WasmExecutorConfig {
    /// Create a new configuration with custom JWT secret
    #[must_use]
    pub fn new(jwt_secret: String) -> Self {
        Self {
            jwt_secret,
            ..Default::default()
        }
    }

    /// Create a testing configuration with relaxed limits
    ///
    /// **WARNING:** Do not use in production!
    #[must_use]
    pub fn for_testing() -> Self {
        Self {
            max_fuel: 10_000_000,
            max_memory_bytes: 64 * 1024 * 1024,
            max_execution_timeout: Duration::from_secs(5),
            enable_audit_logging: false,
            jwt_secret: "test-secret-that-is-at-least-32-characters-long".to_string(),
            audit_log_path: "/tmp/wasm-test-audit.log".to_string(),
            ..Default::default()
        }
    }

    /// Validate configuration
    ///
    /// Ensures all values are within safe bounds
    pub fn validate(&self) -> Result<(), String> {
        if self.max_fuel == 0 {
            return Err("max_fuel must be greater than 0".to_string());
        }
        if self.max_memory_bytes == 0 {
            return Err("max_memory_bytes must be greater than 0".to_string());
        }
        if self.max_stack_bytes == 0 {
            return Err("max_stack_bytes must be greater than 0".to_string());
        }
        if self.max_execution_timeout.as_secs() == 0 {
            return Err("max_execution_timeout must be greater than 0".to_string());
        }
        if self.max_wasm_size_bytes == 0 {
            return Err("max_wasm_size_bytes must be greater than 0".to_string());
        }
        if self.jwt_secret.is_empty() {
            return Err("jwt_secret cannot be empty".to_string());
        }
        // SECURITY (M-229): Detect the insecure default marker and fail.
        // This prevents operation with a predictable JWT secret.
        if self.jwt_secret == INSECURE_DEFAULT_SECRET_MARKER {
            return Err(
                "JWT_SECRET environment variable must be set. \
                 The executor cannot start without a secure secret."
                    .to_string(),
            );
        }
        if self.jwt_secret.len() < 32 {
            return Err("jwt_secret must be at least 32 characters for security".to_string());
        }
        if self.jwt_expiry_minutes < 1 {
            return Err("jwt_expiry_minutes must be at least 1".to_string());
        }
        if self.enable_audit_logging && self.audit_log_path.is_empty() {
            return Err("audit_log_path cannot be empty when audit logging is enabled".to_string());
        }
        Ok(())
    }
}

#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config_validation() {
        let config = WasmExecutorConfig::for_testing();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_jwt_secret_too_short() {
        let mut config = WasmExecutorConfig::for_testing();
        config.jwt_secret = "short".to_string();
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_zero_fuel() {
        let mut config = WasmExecutorConfig::for_testing();
        config.max_fuel = 0;
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_empty_audit_path() {
        let mut config = WasmExecutorConfig::for_testing();
        config.enable_audit_logging = true;
        config.audit_log_path = String::new();
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_insecure_default_secret_rejected() {
        // SECURITY (M-229): Verify that the insecure default marker is rejected
        let mut config = WasmExecutorConfig::for_testing();
        config.jwt_secret = super::INSECURE_DEFAULT_SECRET_MARKER.to_string();
        let result = config.validate();
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .contains("JWT_SECRET environment variable must be set"));
    }
}
