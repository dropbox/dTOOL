//! Centralized environment variable names and helpers for `dashflow-streaming`.
//!
//! This mirrors naming in `dashflow::core::config_loader::env_vars` to keep
//! configuration consistent across binaries/crates, without introducing a
//! dependency cycle (the `dashflow` crate optionally depends on `dashflow-streaming`).

// =============================================================================
// Environment Variable Name Constants
// =============================================================================

/// Kafka bootstrap servers (preferred).
pub const KAFKA_BROKERS: &str = "KAFKA_BROKERS";
/// Kafka bootstrap servers (legacy alternative to `KAFKA_BROKERS`).
pub const KAFKA_BOOTSTRAP_SERVERS: &str = "KAFKA_BOOTSTRAP_SERVERS";
/// Kafka topic name (preferred).
pub const KAFKA_TOPIC: &str = "KAFKA_TOPIC";
/// Kafka topic name (legacy alternative to `KAFKA_TOPIC`).
pub const DASHSTREAM_TOPIC: &str = "DASHSTREAM_TOPIC";
/// Kafka partition number for consumers (default: 0).
pub const KAFKA_PARTITION: &str = "KAFKA_PARTITION";
/// Kafka offset reset policy (default: "earliest").
pub const KAFKA_AUTO_OFFSET_RESET: &str = "KAFKA_AUTO_OFFSET_RESET";
/// Tenant ID for multi-tenant deployments (default: "default").
pub const KAFKA_TENANT_ID: &str = "KAFKA_TENANT_ID";
/// Timeout for Kafka admin operations (create/delete topic), in seconds.
pub const KAFKA_OPERATION_TIMEOUT_SECS: &str = "KAFKA_OPERATION_TIMEOUT_SECS";
/// Timeout for Kafka metadata operations (list topics), in seconds.
pub const KAFKA_METADATA_TIMEOUT_SECS: &str = "KAFKA_METADATA_TIMEOUT_SECS";
/// Force broker address family: "any", "v4", "v6".
pub const KAFKA_BROKER_ADDRESS_FAMILY: &str = "KAFKA_BROKER_ADDRESS_FAMILY";

/// Security protocol: plaintext, ssl, sasl_plaintext, sasl_ssl.
pub const KAFKA_SECURITY_PROTOCOL: &str = "KAFKA_SECURITY_PROTOCOL";
/// SASL mechanism: PLAIN, SCRAM-SHA-256, etc.
pub const KAFKA_SASL_MECHANISM: &str = "KAFKA_SASL_MECHANISM";
/// SASL username.
pub const KAFKA_SASL_USERNAME: &str = "KAFKA_SASL_USERNAME";
/// SASL password.
pub const KAFKA_SASL_PASSWORD: &str = "KAFKA_SASL_PASSWORD";
/// Path to SSL CA certificate file.
pub const KAFKA_SSL_CA_LOCATION: &str = "KAFKA_SSL_CA_LOCATION";
/// Path to SSL client certificate file (mTLS).
pub const KAFKA_SSL_CERTIFICATE_LOCATION: &str = "KAFKA_SSL_CERTIFICATE_LOCATION";
/// Path to SSL client key file (mTLS).
pub const KAFKA_SSL_KEY_LOCATION: &str = "KAFKA_SSL_KEY_LOCATION";
/// Password for SSL client key file (if encrypted).
pub const KAFKA_SSL_KEY_PASSWORD: &str = "KAFKA_SSL_KEY_PASSWORD";
/// SSL endpoint identification algorithm (default: https).
pub const KAFKA_SSL_ENDPOINT_ALGORITHM: &str = "KAFKA_SSL_ENDPOINT_ALGORITHM";

/// Health endpoint port for streaming binaries.
pub const HEALTH_PORT: &str = "HEALTH_PORT";

// =============================================================================
// Typed helpers
// =============================================================================

/// Reads an environment variable as a string, returning `None` if unset.
#[must_use]
pub fn env_string(name: &str) -> Option<String> {
    std::env::var(name).ok()
}

/// Reads an environment variable as a string, returning `default` if unset.
#[must_use]
pub fn env_string_or_default(name: &str, default: &str) -> String {
    std::env::var(name).unwrap_or_else(|_| default.to_string())
}

/// Reads the first set env var from `primary` and `fallback`, else returns `default`.
#[must_use]
pub fn env_string_one_of_or_default(primary: &str, fallback: &str, default: &str) -> String {
    std::env::var(primary)
        .or_else(|_| std::env::var(fallback))
        .unwrap_or_else(|_| default.to_string())
}

/// Reads an environment variable as a `u16`, returning `default` if unset or invalid.
#[must_use]
pub fn env_u16_or_default(name: &str, default: u16) -> u16 {
    std::env::var(name)
        .ok()
        .and_then(|v| v.parse::<u16>().ok())
        .unwrap_or(default)
}

/// Reads an environment variable as a `u64`, returning `default` if unset or invalid.
#[must_use]
pub fn env_u64_or_default(name: &str, default: u64) -> u64 {
    std::env::var(name)
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(default)
}

/// Reads an environment variable as an `i32`, returning `default` if unset or invalid.
#[must_use]
pub fn env_i32_or_default(name: &str, default: i32) -> i32 {
    std::env::var(name)
        .ok()
        .and_then(|v| v.parse::<i32>().ok())
        .unwrap_or(default)
}

#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    // Helper to set and unset env vars safely for tests
    fn with_env_var<F, R>(name: &str, value: Option<&str>, f: F) -> R
    where
        F: FnOnce() -> R,
    {
        // Save original value
        let original = env::var(name).ok();

        // Set or unset the env var
        match value {
            Some(v) => env::set_var(name, v),
            None => env::remove_var(name),
        }

        // Run the test
        let result = f();

        // Restore original value
        match original {
            Some(v) => env::set_var(name, v),
            None => env::remove_var(name),
        }

        result
    }

    // =============================================================================
    // Constants Tests
    // =============================================================================

    #[test]
    #[allow(clippy::const_is_empty)]
    fn test_kafka_constants_defined() {
        // Verify all Kafka-related constants are non-empty strings
        assert!(!KAFKA_BROKERS.is_empty());
        assert!(!KAFKA_BOOTSTRAP_SERVERS.is_empty());
        assert!(!KAFKA_TOPIC.is_empty());
        assert!(!DASHSTREAM_TOPIC.is_empty());
        assert!(!KAFKA_PARTITION.is_empty());
        assert!(!KAFKA_AUTO_OFFSET_RESET.is_empty());
        assert!(!KAFKA_TENANT_ID.is_empty());
        assert!(!KAFKA_OPERATION_TIMEOUT_SECS.is_empty());
        assert!(!KAFKA_METADATA_TIMEOUT_SECS.is_empty());
        assert!(!KAFKA_BROKER_ADDRESS_FAMILY.is_empty());
    }

    #[test]
    #[allow(clippy::const_is_empty)]
    fn test_kafka_security_constants_defined() {
        assert!(!KAFKA_SECURITY_PROTOCOL.is_empty());
        assert!(!KAFKA_SASL_MECHANISM.is_empty());
        assert!(!KAFKA_SASL_USERNAME.is_empty());
        assert!(!KAFKA_SASL_PASSWORD.is_empty());
        assert!(!KAFKA_SSL_CA_LOCATION.is_empty());
        assert!(!KAFKA_SSL_CERTIFICATE_LOCATION.is_empty());
        assert!(!KAFKA_SSL_KEY_LOCATION.is_empty());
        assert!(!KAFKA_SSL_KEY_PASSWORD.is_empty());
        assert!(!KAFKA_SSL_ENDPOINT_ALGORITHM.is_empty());
    }

    #[test]
    #[allow(clippy::const_is_empty)]
    fn test_health_port_constant_defined() {
        assert!(!HEALTH_PORT.is_empty());
        assert_eq!(HEALTH_PORT, "HEALTH_PORT");
    }

    #[test]
    fn test_constants_have_expected_values() {
        // Verify constants match their expected string values
        assert_eq!(KAFKA_BROKERS, "KAFKA_BROKERS");
        assert_eq!(KAFKA_BOOTSTRAP_SERVERS, "KAFKA_BOOTSTRAP_SERVERS");
        assert_eq!(KAFKA_TOPIC, "KAFKA_TOPIC");
        assert_eq!(DASHSTREAM_TOPIC, "DASHSTREAM_TOPIC");
        assert_eq!(KAFKA_PARTITION, "KAFKA_PARTITION");
        assert_eq!(KAFKA_AUTO_OFFSET_RESET, "KAFKA_AUTO_OFFSET_RESET");
        assert_eq!(KAFKA_TENANT_ID, "KAFKA_TENANT_ID");
    }

    // =============================================================================
    // env_string Tests
    // =============================================================================

    #[test]
    fn test_env_string_returns_none_when_unset() {
        with_env_var("DASHFLOW_STREAMING_TEST_UNSET", None, || {
            assert!(env_string("DASHFLOW_STREAMING_TEST_UNSET").is_none());
        });
    }

    #[test]
    fn test_env_string_returns_value_when_set() {
        with_env_var("DASHFLOW_STREAMING_TEST_SET", Some("test_value"), || {
            assert_eq!(
                env_string("DASHFLOW_STREAMING_TEST_SET"),
                Some("test_value".to_string())
            );
        });
    }

    #[test]
    fn test_env_string_returns_empty_string_when_empty() {
        with_env_var("DASHFLOW_STREAMING_TEST_EMPTY", Some(""), || {
            assert_eq!(
                env_string("DASHFLOW_STREAMING_TEST_EMPTY"),
                Some(String::new())
            );
        });
    }

    // =============================================================================
    // env_string_or_default Tests
    // =============================================================================

    #[test]
    fn test_env_string_or_default_returns_default_when_unset() {
        with_env_var("DASHFLOW_STREAMING_TEST_UNSET2", None, || {
            assert_eq!(
                env_string_or_default("DASHFLOW_STREAMING_TEST_UNSET2", "default_value"),
                "default_value"
            );
        });
    }

    #[test]
    fn test_env_string_or_default_returns_value_when_set() {
        with_env_var(
            "DASHFLOW_STREAMING_TEST_SET2",
            Some("actual_value"),
            || {
                assert_eq!(
                    env_string_or_default("DASHFLOW_STREAMING_TEST_SET2", "default_value"),
                    "actual_value"
                );
            },
        );
    }

    #[test]
    fn test_env_string_or_default_returns_empty_over_default() {
        // Empty string is still a valid value, not the default
        with_env_var("DASHFLOW_STREAMING_TEST_EMPTY2", Some(""), || {
            assert_eq!(
                env_string_or_default("DASHFLOW_STREAMING_TEST_EMPTY2", "default_value"),
                ""
            );
        });
    }

    // =============================================================================
    // env_string_one_of_or_default Tests
    // =============================================================================

    #[test]
    fn test_env_string_one_of_or_default_uses_primary() {
        with_env_var("DASHFLOW_PRIMARY", Some("primary_value"), || {
            with_env_var("DASHFLOW_FALLBACK", Some("fallback_value"), || {
                assert_eq!(
                    env_string_one_of_or_default(
                        "DASHFLOW_PRIMARY",
                        "DASHFLOW_FALLBACK",
                        "default"
                    ),
                    "primary_value"
                );
            });
        });
    }

    #[test]
    fn test_env_string_one_of_or_default_uses_fallback() {
        with_env_var("DASHFLOW_PRIMARY2", None, || {
            with_env_var("DASHFLOW_FALLBACK2", Some("fallback_value"), || {
                assert_eq!(
                    env_string_one_of_or_default(
                        "DASHFLOW_PRIMARY2",
                        "DASHFLOW_FALLBACK2",
                        "default"
                    ),
                    "fallback_value"
                );
            });
        });
    }

    #[test]
    fn test_env_string_one_of_or_default_uses_default() {
        with_env_var("DASHFLOW_PRIMARY3", None, || {
            with_env_var("DASHFLOW_FALLBACK3", None, || {
                assert_eq!(
                    env_string_one_of_or_default(
                        "DASHFLOW_PRIMARY3",
                        "DASHFLOW_FALLBACK3",
                        "default_value"
                    ),
                    "default_value"
                );
            });
        });
    }

    // =============================================================================
    // env_u16_or_default Tests
    // =============================================================================

    #[test]
    fn test_env_u16_or_default_returns_default_when_unset() {
        with_env_var("DASHFLOW_U16_UNSET", None, || {
            assert_eq!(env_u16_or_default("DASHFLOW_U16_UNSET", 8080), 8080);
        });
    }

    #[test]
    fn test_env_u16_or_default_parses_valid_value() {
        with_env_var("DASHFLOW_U16_VALID", Some("9090"), || {
            assert_eq!(env_u16_or_default("DASHFLOW_U16_VALID", 8080), 9090);
        });
    }

    #[test]
    fn test_env_u16_or_default_returns_default_for_invalid() {
        with_env_var("DASHFLOW_U16_INVALID", Some("not_a_number"), || {
            assert_eq!(env_u16_or_default("DASHFLOW_U16_INVALID", 8080), 8080);
        });
    }

    #[test]
    fn test_env_u16_or_default_returns_default_for_negative() {
        with_env_var("DASHFLOW_U16_NEG", Some("-100"), || {
            assert_eq!(env_u16_or_default("DASHFLOW_U16_NEG", 8080), 8080);
        });
    }

    #[test]
    fn test_env_u16_or_default_returns_default_for_overflow() {
        with_env_var("DASHFLOW_U16_OVERFLOW", Some("70000"), || {
            assert_eq!(env_u16_or_default("DASHFLOW_U16_OVERFLOW", 8080), 8080);
        });
    }

    // =============================================================================
    // env_u64_or_default Tests
    // =============================================================================

    #[test]
    fn test_env_u64_or_default_returns_default_when_unset() {
        with_env_var("DASHFLOW_U64_UNSET", None, || {
            assert_eq!(env_u64_or_default("DASHFLOW_U64_UNSET", 1000), 1000);
        });
    }

    #[test]
    fn test_env_u64_or_default_parses_valid_value() {
        with_env_var("DASHFLOW_U64_VALID", Some("5000"), || {
            assert_eq!(env_u64_or_default("DASHFLOW_U64_VALID", 1000), 5000);
        });
    }

    #[test]
    fn test_env_u64_or_default_parses_large_value() {
        with_env_var(
            "DASHFLOW_U64_LARGE",
            Some("18446744073709551615"),
            || {
                assert_eq!(
                    env_u64_or_default("DASHFLOW_U64_LARGE", 0),
                    u64::MAX
                );
            },
        );
    }

    #[test]
    fn test_env_u64_or_default_returns_default_for_invalid() {
        with_env_var("DASHFLOW_U64_INVALID", Some("invalid"), || {
            assert_eq!(env_u64_or_default("DASHFLOW_U64_INVALID", 1000), 1000);
        });
    }

    // =============================================================================
    // env_i32_or_default Tests
    // =============================================================================

    #[test]
    fn test_env_i32_or_default_returns_default_when_unset() {
        with_env_var("DASHFLOW_I32_UNSET", None, || {
            assert_eq!(env_i32_or_default("DASHFLOW_I32_UNSET", 100), 100);
        });
    }

    #[test]
    fn test_env_i32_or_default_parses_positive() {
        with_env_var("DASHFLOW_I32_POS", Some("500"), || {
            assert_eq!(env_i32_or_default("DASHFLOW_I32_POS", 100), 500);
        });
    }

    #[test]
    fn test_env_i32_or_default_parses_negative() {
        with_env_var("DASHFLOW_I32_NEG", Some("-500"), || {
            assert_eq!(env_i32_or_default("DASHFLOW_I32_NEG", 100), -500);
        });
    }

    #[test]
    fn test_env_i32_or_default_parses_zero() {
        with_env_var("DASHFLOW_I32_ZERO", Some("0"), || {
            assert_eq!(env_i32_or_default("DASHFLOW_I32_ZERO", 100), 0);
        });
    }

    #[test]
    fn test_env_i32_or_default_returns_default_for_invalid() {
        with_env_var("DASHFLOW_I32_INVALID", Some("abc"), || {
            assert_eq!(env_i32_or_default("DASHFLOW_I32_INVALID", 100), 100);
        });
    }

    #[test]
    fn test_env_i32_or_default_returns_default_for_overflow() {
        with_env_var(
            "DASHFLOW_I32_OVERFLOW",
            Some("99999999999999999999"),
            || {
                assert_eq!(env_i32_or_default("DASHFLOW_I32_OVERFLOW", 100), 100);
            },
        );
    }
}
